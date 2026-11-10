use super::*;
use gpui_kit::base::TestSupportExt as _;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldStyle {
    Number,
    VoiceSlider,
    SimpleSlider,
}
impl SettingsView {
    pub(super) fn render_field(
        &self,
        path: &str,
        style: FieldStyle,
        owner: Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let control = self
            .controls
            .iter()
            .find(|c| c.field.path == path)
            .expect("settings control exists");
        let model = self.model.read(cx);
        let t = |s: &str| model.text(s);
        let disabled = !self.editable(path, cx);
        let value = &control.field.text;
        let mut label = field_label(path);
        if path == "/voice/pitch" && self.percent() {
            label = label.replace("半音", "%");
        }
        if style == FieldStyle::SimpleSlider {
            label = if path == "/deesser/rangeDb" {
                "抑える強さ".into()
            } else {
                match path.split('/').nth(3).and_then(|s| s.parse::<usize>().ok()) {
                    Some(0) => "低域の厚み".into(),
                    Some(1) => "明瞭さ".into(),
                    Some(2) => "明るさ".into(),
                    Some(i) => format!("{} · {}", i + 1, t("バンド")),
                    None => label,
                }
            };
        }
        let mut row = div()
            .id(SharedString::from(format!("field:{path}")))
            .flex()
            .flex_col()
            .gap_2()
            .child(t(&label));
        if style != FieldStyle::Number {
            let spec = numbers::slider(path, self.percent()).expect("slider field");
            let valid = spec.valid(value);
            let slider = control.slider.as_ref().expect("retained slider");
            let mut track = div().flex().items_center().gap_3().child(
                div()
                    .id(SharedString::from(format!("slider:{path}")))
                    .flex_1()
                    .h(px(28.))
                    .child(Slider::new(slider).disabled(disabled || !valid).w_full())
                    .test_support(),
            );
            if style == FieldStyle::VoiceSlider {
                track = track.child(
                    div()
                        .w(px(130.))
                        .child(self.render_input(control, &label, disabled, cx)),
                );
            }
            row = row.child(track).child(
                div().flex().justify_between().children(
                    spec.labels
                        .into_iter()
                        .map(|s| div().text_size(px(12.)).child(t(s))),
                ),
            );
            if !valid {
                row = row.child(
                    div()
                        .text_color(crate::gui::theme::palette(cx).danger)
                        .child(t("数値を正しい範囲に直すか、標準に戻してください。")),
                );
            }
        } else {
            row = row.child(match &control.widget {
                Widget::Toggle => {
                    let path = path.to_string();
                    let next = (value != "true").to_string();
                    Button::new(SharedString::from(format!("toggle:{path}")))
                        .label(t(if value == "true" { "有効" } else { "無効" }))
                        .selected(value == "true")
                        .disabled(disabled)
                        .on_click(move |_, w, cx| {
                            owner.update(cx, |v, cx| v.changed(&path, next.clone(), w, cx))
                        })
                        .into_any_element()
                }
                _ => self.render_input(control, &label, disabled, cx),
            });
        }
        if style == FieldStyle::Number {
            if let Some(help) = fields::help(path) {
                row = row.child(div().text_size(px(12.)).child(t(help)));
            }
        }
        row.test_support().into_any_element()
    }
    fn render_input(&self, control: &Control, label: &str, disabled: bool, cx: &App) -> AnyElement {
        match &control.widget {
            Widget::Input(state) if control.field.value.is_number() => NumberInput::new(state)
                .bg(crate::gui::theme::palette(cx).raised)
                .accessibility_label(label.to_string())
                .disabled(disabled)
                .into_any_element(),
            Widget::Input(state) => Input::new(state)
                .bg(crate::gui::theme::palette(cx).raised)
                .disabled(disabled)
                .into_any_element(),
            Widget::Select(state) => Select::new(state)
                .bg(crate::gui::theme::palette(cx).raised)
                .disabled(disabled)
                .into_any_element(),
            Widget::Toggle => unreachable!("toggle is rendered separately"),
        }
    }
    pub(super) fn detail_button(&self, section: &str, owner: Entity<Self>, cx: &App) -> AnyElement {
        let section = section.to_string();
        let enabled = self.section_enabled(&section);
        Button::new(SharedString::from(format!("details:{section}")))
            .label(self.model.read(cx).text("詳細設定"))
            .disabled(self.model.read(cx).locked() || self.pending || !enabled)
            .on_click(move |_, w, cx| owner.update(cx, |v, cx| v.open_details(&section, w, cx)))
            .into_any_element()
    }
    pub(super) fn section_enabled(&self, section: &str) -> bool {
        let path = match section {
            "inputEq" => "/inputEq/enabled",
            "preConversionEq" => "/preConversionEq/enabled",
            "postConversionEq" => "/eq/enabled",
            "deesser" => "/deesser/enabled",
            "noiseGate" => "/noise/enabled",
            "voice" => "/voice/enabled",
            "autoLevel" => "/level/enabled",
            "postCompressor" => "/level/compressorEnabled",
            _ => return true,
        };
        self.value(path) == "true"
    }
    pub(super) fn render_page(&self, section: &str, owner: Entity<Self>, cx: &App) -> AnyElement {
        let model = self.model.read(cx);
        let t = |s: &str| model.text(s);
        let title = SECTIONS
            .iter()
            .find(|(id, _)| *id == section)
            .map(|(_, title)| *title)
            .expect("settings page");
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_lg().child(t(title)));
        let field =
            |path: &str, style: FieldStyle| self.render_field(path, style, owner.clone(), cx);
        match section {
            "voice" => {
                body = body
                    .child(field("/voice/enabled", FieldStyle::Number))
                    .child(self.detail_button(section, owner.clone(), cx))
                    .child(field("/voice/pitchUnit", FieldStyle::Number));
                if self.percent() {
                    body = body.child(t(
                        "100%が元の高さ、50%で1オクターブ下、200%で1オクターブ上です。",
                    ));
                }
                body = body
                    .child(field("/voice/pitch", FieldStyle::VoiceSlider))
                    .child(field("/voice/formantShift", FieldStyle::VoiceSlider))
                    .child(t(
                        "声の太さ・明るさです。残響（リバーブ）とは別の調整です。",
                    ))
                    .child(self.reset_button("voice", "元の声に戻す", owner.clone(), cx))
                    .child(field("/voice/preserveFormants", FieldStyle::Number));
                if !self.section_enabled(section) {
                    body = body.child(t("バイパス中。変換量は保持されます。"));
                }
            }
            "inputEq" | "preConversionEq" | "postConversionEq" => {
                let root = if section == "postConversionEq" {
                    "eq"
                } else {
                    section
                };
                body = body.child(field(&format!("/{root}/enabled"), FieldStyle::Number));
                if self.section_enabled(section) {
                    for c in self.controls.iter().filter(|c| {
                        c.field.path.starts_with(&format!("/{root}/bands/"))
                            && c.field.path.ends_with("/gainDb")
                    }) {
                        body = body.child(field(&c.field.path, FieldStyle::SimpleSlider));
                    }
                    body = body.child(self.detail_button(section, owner.clone(), cx));
                }
                if section == "inputEq" {
                    body=body.child(div().border_t_1().pt_3().child(t("入力EQのハイパス（HPF）"))).child(field("/noise/rumbleFilter",FieldStyle::Number)).child(t("80 Hz以下の低域を抑え、マイクの吹かれ・振動ノイズを減らします。EQの帯域調整とは個別に切り替えられます。"));
                }
            }
            "deesser" => {
                body = body.child(field("/deesser/enabled", FieldStyle::Number));
                if self.section_enabled(section) {
                    body = body
                        .child(field("/deesser/rangeDb", FieldStyle::SimpleSlider))
                        .child(self.detail_button(section, owner.clone(), cx));
                    if self.deesser_custom() {
                        body=body.child(t("詳細設定で調整した設定を使っています。ここでは抑える強さだけを変えます。"));
                    }
                }
            }
            "noiseGate" | "autoLevel" | "postCompressor" => {
                let path = match section {
                    "noiseGate" => "/noise/enabled",
                    "autoLevel" => "/level/enabled",
                    _ => "/level/compressorEnabled",
                };
                body = body.child(field(path, FieldStyle::Number));
                if self.section_enabled(section) {
                    body = body.child(self.detail_button(section, owner.clone(), cx));
                }
            }
            "devices" => {
                body = body.child(self.render_devices(owner.clone(), cx));
            }
            _ => {
                for control in self
                    .controls
                    .iter()
                    .filter(|c| draft::group(&c.field.path, section))
                {
                    body = body.child(field(&control.field.path, FieldStyle::Number));
                }
                if self.value(&format!("/{section}/enabled")) == "false" {
                    body = body.child(t("バイパス中。設定値は保持されます。"));
                }
            }
        }
        body.into_any_element()
    }
}
