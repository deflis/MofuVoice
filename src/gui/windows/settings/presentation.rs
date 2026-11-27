use super::*;
use gpui_kit::base::TestSupportExt as _;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldStyle {
    Number,
    VoiceSlider,
    SimpleSlider,
}

impl SettingsView {
    pub(super) fn render_toggle(
        &self,
        field: ToggleField,
        owner: Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let model = self.model.read(cx);
        let t = |s: &str| model.text(s);
        let value = self.draft.toggle(field);
        let disabled = !self.editable_toggle(field, cx);
        div()
            .id(SharedString::from(format!("field:{}", field.id())))
            .flex()
            .flex_col()
            .gap_2()
            .child(t(field.label()))
            .child(
                Button::new(SharedString::from(format!("toggle:{}", field.id())))
                    .label(t(if value { "有効" } else { "無効" }))
                    .selected(value)
                    .disabled(disabled)
                    .on_click(move |_, w, cx| {
                        owner.update(cx, |v, cx| v.set_toggle(field, !value, w, cx))
                    }),
            )
            .test_support()
            .into_any_element()
    }

    pub(super) fn render_select(&self, field: SelectField, cx: &App) -> AnyElement {
        let model = self.model.read(cx);
        let t = |s: &str| model.text(s);
        let disabled = !self.editable_select(field, cx);
        let bg = crate::gui::theme::palette(cx).raised;
        let label = field.label(self.resolved_backend(cx));
        let control: AnyElement = match field {
            SelectField::Backend => Select::new(&self.selects.backend)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::Device(slot) => Select::new(&self.selects.device(slot).state)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::SampleRate => Select::new(&self.selects.sample_rate)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::PitchUnit => Select::new(&self.selects.pitch_unit)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::MathPrecision => Select::new(&self.selects.math_precision)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::PitchAlgorithm => Select::new(&self.selects.pitch_algorithm)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::PsolaPitchEstimator => Select::new(&self.selects.psola_pitch_estimator)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::AnalysisWindow => Select::new(&self.selects.analysis_window)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::SynthesisWindow => Select::new(&self.selects.synthesis_window)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::PsolaAnalysisWindow => Select::new(&self.selects.psola_analysis_window)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::PsolaSynthesisWindow => Select::new(&self.selects.psola_synthesis_window)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
            SelectField::GateThresholdMode => Select::new(&self.selects.gate_threshold_mode)
                .bg(bg)
                .disabled(disabled)
                .into_any_element(),
        };
        div()
            .id(SharedString::from(format!("field:{}", field.id())))
            .flex()
            .flex_col()
            .gap_2()
            .child(t(label))
            .child(control)
            .test_support()
            .into_any_element()
    }

    pub(super) fn render_number(
        &self,
        field: NumberField,
        style: FieldStyle,
        cx: &App,
    ) -> AnyElement {
        let control = self
            .numbers
            .get(&field)
            .expect("number settings control exists");
        let model = self.model.read(cx);
        let t = |s: &str| model.text(s);
        let disabled = !self.editable_number(field, cx);
        let value = self.draft.number_text(field);
        let label = if style == FieldStyle::SimpleSlider {
            field
                .simple_slider_label(&model.preferences)
                .unwrap_or_else(|| field.label(self.draft.pitch_unit))
        } else {
            field.label(self.draft.pitch_unit)
        };
        let field_id = field.id();
        let mut row = div()
            .id(SharedString::from(format!("field:{field_id}")))
            .flex()
            .flex_col()
            .gap_2()
            .child(t(&label));
        if style != FieldStyle::Number {
            let spec = numbers::slider(field, self.draft.pitch_unit).expect("slider field");
            let valid = spec.valid(value);
            let slider = control.slider.as_ref().expect("retained slider");
            let mut track = div().flex().items_center().gap_3().child(
                div()
                    .id(SharedString::from(format!("slider:{field_id}")))
                    .flex_1()
                    .h(px(28.))
                    .child(Slider::new(slider).disabled(disabled || !valid).w_full())
                    .test_support(),
            );
            if style == FieldStyle::VoiceSlider {
                track = track.child(
                    div().w(px(130.)).child(
                        NumberInput::new(&control.input)
                            .bg(crate::gui::theme::palette(cx).raised)
                            .accessibility_label(t(&label))
                            .disabled(disabled),
                    ),
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
            row = row.child(
                NumberInput::new(&control.input)
                    .bg(crate::gui::theme::palette(cx).raised)
                    .accessibility_label(t(&label))
                    .disabled(disabled),
            );
            if let Some(help) = field.help() {
                row = row.child(div().text_size(px(12.)).child(t(help)));
            }
        }
        row.test_support().into_any_element()
    }

    pub(super) fn detail_button(
        &self,
        section: Section,
        owner: Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let enabled = self.draft.section_enabled(section);
        Button::new(SharedString::from(format!("details:{}", section.id())))
            .label(self.model.read(cx).text("詳細設定"))
            .disabled(self.model.read(cx).locked() || self.pending() || !enabled)
            .on_click(move |_, w, cx| owner.update(cx, |v, cx| v.open_details(section, w, cx)))
            .into_any_element()
    }

    pub(super) fn render_page(
        &self,
        section: Section,
        owner: Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let model = self.model.read(cx);
        let t = |s: &str| model.text(s);
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_lg().child(t(section.label())));
        match section {
            Section::Voice => {
                body = body
                    .child(self.render_toggle(ToggleField::VoiceEnabled, owner.clone(), cx))
                    .child(self.detail_button(section, owner.clone(), cx))
                    .child(self.render_select(SelectField::PitchUnit, cx));
                if self.draft.pitch_unit == PitchUnit::Percent {
                    body = body.child(t(
                        "100%が元の高さ、50%で1オクターブ下、200%で1オクターブ上です。",
                    ));
                }
                body = body
                    .child(self.render_number(NumberField::Pitch, FieldStyle::VoiceSlider, cx))
                    .child(self.render_number(
                        NumberField::FormantShift,
                        FieldStyle::VoiceSlider,
                        cx,
                    ))
                    .child(t(
                        "声の太さ・明るさです。残響（リバーブ）とは別の調整です。",
                    ))
                    .child(self.reset_button(ResetTarget::Voice, "元の声に戻す", owner.clone(), cx))
                    .child(self.render_toggle(ToggleField::PreserveFormants, owner, cx));
                if !self.draft.voice_enabled {
                    body = body.child(t("バイパス中。変換量は保持されます。"));
                }
            }
            Section::InputEq | Section::PreConversionEq | Section::PostConversionEq => {
                let kind = EqKind::from_section(section).expect("EQ section");
                body =
                    body.child(self.render_toggle(ToggleField::EqEnabled(kind), owner.clone(), cx));
                if self.draft.eq(kind).enabled {
                    for index in 0..self.draft.eq(kind).bands.len() {
                        body = body.child(self.render_number(
                            NumberField::EqBand(kind, index, EqBandField::GainDb),
                            FieldStyle::SimpleSlider,
                            cx,
                        ));
                    }
                    body = body.child(self.detail_button(section, owner.clone(), cx));
                }
                if section == Section::InputEq {
                    body = body
                        .child(div().border_t_1().pt_3().child(t("入力EQのハイパス（HPF）")))
                        .child(self.render_toggle(ToggleField::RumbleFilter, owner, cx))
                        .child(t(
                            "80 Hz以下の低域を抑え、マイクの吹かれ・振動ノイズを減らします。EQの帯域調整とは個別に切り替えられます。",
                        ));
                }
            }
            Section::Deesser => {
                body =
                    body.child(self.render_toggle(ToggleField::DeesserEnabled, owner.clone(), cx));
                if self.draft.deesser_enabled {
                    body = body
                        .child(self.render_number(
                            NumberField::DeesserRangeDb,
                            FieldStyle::SimpleSlider,
                            cx,
                        ))
                        .child(self.detail_button(section, owner, cx));
                    if self.deesser_custom() {
                        body = body.child(t(
                            "詳細設定で調整した設定を使っています。ここでは抑える強さだけを変えます。",
                        ));
                    }
                }
            }
            Section::NoiseGate | Section::AutoLevel | Section::PostCompressor => {
                let toggle = match section {
                    Section::NoiseGate => ToggleField::NoiseGateEnabled,
                    Section::AutoLevel => ToggleField::LevelEnabled,
                    _ => ToggleField::PostCompressorEnabled,
                };
                body = body.child(self.render_toggle(toggle, owner.clone(), cx));
                if self.draft.section_enabled(section) {
                    body = body.child(self.detail_button(section, owner, cx));
                }
            }
            Section::Devices => {
                body = body.child(self.render_devices(owner, cx));
            }
            Section::Compressor => {
                body = body
                    .child(self.render_toggle(ToggleField::CompressorEnabled, owner, cx))
                    .child(self.render_number(
                        NumberField::CompressorThresholdDb,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(NumberField::CompressorRatio, FieldStyle::Number, cx))
                    .child(self.render_number(
                        NumberField::CompressorAttackMs,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(
                        NumberField::CompressorReleaseMs,
                        FieldStyle::Number,
                        cx,
                    ));
                if !self.draft.compressor_enabled {
                    body = body.child(t("バイパス中。設定値は保持されます。"));
                }
            }
            Section::Limiter => {
                body = body
                    .child(self.render_toggle(ToggleField::LimiterEnabled, owner, cx))
                    .child(self.render_number(
                        NumberField::LimiterCeilingDb,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(
                        NumberField::LimiterReleaseMs,
                        FieldStyle::Number,
                        cx,
                    ));
                if !self.draft.limiter_enabled {
                    body = body.child(t("バイパス中。設定値は保持されます。"));
                }
            }
            Section::Dereverb => {
                body = body
                    .child(self.render_toggle(ToggleField::DereverbEnabled, owner, cx))
                    .child(self.render_number(NumberField::DereverbAmount, FieldStyle::Number, cx))
                    .child(self.render_number(
                        NumberField::DereverbDecayMs,
                        FieldStyle::Number,
                        cx,
                    ));
                if !self.draft.dereverb_enabled {
                    body = body.child(t("バイパス中。設定値は保持されます。"));
                }
            }
            Section::Reverb => {
                body = body
                    .child(self.render_toggle(ToggleField::ReverbEnabled, owner, cx))
                    .child(self.render_number(NumberField::ReverbMix, FieldStyle::Number, cx))
                    .child(self.render_number(
                        NumberField::ReverbPreDelayMs,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(NumberField::ReverbDecayMs, FieldStyle::Number, cx));
                if !self.draft.reverb_enabled {
                    body = body.child(t("バイパス中。設定値は保持されます。"));
                }
            }
            Section::Overview => unreachable!("overview is rendered by overview::render"),
        }
        body.into_any_element()
    }
}
