use super::*;
use gpui_kit::base::TestSupportExt as _;
impl SettingsView {
    pub fn discover(&mut self, cx: &mut Context<Self>) {
        if self.pending() {
            return;
        }
        self.discovery_deferred = self.model.read(cx).locked();
        if self.discovery_deferred {
            return;
        }
        let kind = match backend::BackendKind::parse(&self.value("/devices/backend")) {
            Ok(k) => k,
            Err(e) => {
                self.device_error = e.to_string();
                cx.notify();
                return;
            }
        };
        self.operation = Some(Operation::Discover);
        self.rates = None;
        self.device_error.clear();
        self.model.update(cx, |m, cx| {
            m.busy = true;
            cx.notify();
        });
        let device = self.value("/devices/device");
        let output = self.value("/devices/outputDevice");
        let generation = self.generation.advance();
        let handle = self.handle;
        let task = cx.background_executor().spawn(async move {
            let choices = model::device_choices(kind)?;
            let rates = (|| {
                if choices.resolved == backend::BackendKind::Asio && device.is_empty() {
                    return Ok(None);
                }
                #[cfg(all(windows, feature = "asio"))]
                let _apartment = if choices.resolved == backend::BackendKind::Asio {
                    Some(crate::session::ComApartment::initialize().map_err(|e| e.to_string())?)
                } else {
                    None
                };
                backend::select(choices.resolved, &output)
                    .and_then(|s| s.supported_sample_rates(&device))
                    .map(Some)
                    .map_err(|e| e.to_string())
            })();
            Ok::<_, String>((choices, rates))
        });
        let model = self.model.clone();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            // Always release the app-level operation gate, even if the view disappeared.
            model.update(cx, |m, cx| {
                m.update_saved_device_warning(
                    kind,
                    result.as_ref().ok().map(|(choices, _)| choices),
                );
                m.busy = false;
                cx.notify();
            });
            _ = handle.update(cx, |_, window, cx| {
                _ = this.update(cx, |v, cx| {
                    if !v.generation.accepts(generation) {
                        return;
                    }
                    v.operation = None;
                    v.apply_discovery(result, window, cx);
                    let target = v.close;
                    cx.notify();
                    if target != CloseTarget::None {
                        v.save(target, cx);
                    }
                });
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn apply_discovery(
        &mut self,
        result: Result<
            (
                model::DeviceChoices,
                Result<Option<backend::DeviceSampleRates>, String>,
            ),
            String,
        >,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.device_error.clear();
        match result {
            Ok((choices, rates)) => {
                for (path, mut items) in [
                    ("/devices/device", choices.inputs),
                    ("/devices/outputDevice", choices.outputs.clone()),
                    ("/devices/previewDevice", choices.outputs),
                ] {
                    items.insert(0, String::new());
                    let current = self.value(path);
                    let used =
                        path == "/devices/device" || choices.resolved == backend::BackendKind::Cpal;
                    let missing = (used && !current.is_empty() && !items.contains(&current))
                        .then(|| current.clone());
                    if missing.is_some() {
                        self.device_error =
                            "前回使った音声機器が見つかりません。入力・出力デバイスを確認してください。"
                                .into();
                    }
                    if !current.is_empty() && !items.contains(&current) {
                        items.push(current.clone());
                    }
                    if let Some(control) = self.controls.iter_mut().find(|c| c.field.path == path) {
                        control.missing = missing.clone();
                        if let Widget::Select(state) = &control.widget {
                            control.items = Some(items.clone());
                            let choices = labeled_choices(
                                path,
                                items,
                                &self.model.read(cx).preferences,
                                choices.resolved,
                                missing.as_deref(),
                            );
                            state.update(cx, |s, cx| {
                                s.set_items(choices, window, cx);
                                s.set_selected_value(&current, window, cx);
                            });
                        }
                    }
                }
                match rates {
                    Ok(rates) => self.rates = rates,
                    Err(e) => {
                        if self.device_error.is_empty() {
                            self.device_error = e;
                        }
                    }
                }
            }
            Err(e) => self.device_error = e,
        }
    }
}

impl SettingsView {
    pub(super) fn resolved_backend(&self, cx: &App) -> backend::BackendKind {
        self.model.read(cx).resolve_backend(
            backend::BackendKind::parse(&self.value("/devices/backend"))
                .expect("backend selection"),
        )
    }

    pub(super) fn render_devices(&self, owner: Entity<Self>, cx: &App) -> AnyElement {
        use super::presentation::FieldStyle;
        let m = self.model.read(cx);
        let t = |s: &str| m.text(s);
        let kind = self.resolved_backend(cx);
        let mut body = div().flex().flex_col().gap_3().child(self.render_field(
            "/devices/device",
            FieldStyle::Number,
            owner.clone(),
            cx,
        ));
        if kind == backend::BackendKind::Cpal {
            for path in ["/devices/outputDevice", "/devices/previewDevice"] {
                body = body.child(self.render_field(path, FieldStyle::Number, owner.clone(), cx));
            }
        }
        body = body.child(self.render_field(
            "/devices/sampleRate",
            FieldStyle::Number,
            owner.clone(),
            cx,
        ));
        if let Some(rates) = &self.rates {
            if kind == backend::BackendKind::Cpal {
                body = body
                    .child(format!(
                        "{} {}",
                        t("入力の対応レート："),
                        format_rates(&rates.input)
                    ))
                    .child(format!(
                        "{} {}",
                        t("出力の対応レート："),
                        format_rates(&rates.output)
                    ));
            }
            body = body.child(format!(
                "{} {}",
                t("入出力で変換なしに使えるレート"),
                format_rates(&rates.common)
            ));
            let rate = self
                .value("/devices/sampleRate")
                .parse::<u32>()
                .unwrap_or(0);
            if !rates.common.contains(&rate) {
                body=body.child(t(if kind==backend::BackendKind::Cpal{"CPALでは選択レートに非対応の入出力を自動でリサンプリングします。変換により遅延が追加されます。"}else{"選択したサンプリングレートには対応していません。"}));
            }
            let input = self
                .value("/devices/inputChannel")
                .parse::<usize>()
                .unwrap_or(0);
            let selected = self
                .value("/devices/outputChannels")
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect::<Vec<_>>();
            let required = selected.iter().copied().max().unwrap_or(1);
            let at_rate: Vec<_> = rates
                .channels
                .iter()
                .filter(|l| l.sample_rate == rate)
                .collect();
            let rate_available = !at_rate.is_empty();
            let max_inputs = at_rate.iter().map(|l| l.input).max().unwrap_or(0);
            let max_outputs = at_rate.iter().map(|l| l.output).max().unwrap_or(0);
            let inputs = at_rate
                .iter()
                .filter(|l| l.output >= required)
                .map(|l| l.input)
                .max()
                .unwrap_or(0);
            let outputs = at_rate
                .iter()
                .filter(|l| l.input >= input)
                .map(|l| l.output)
                .max()
                .unwrap_or(0);
            let selectable_inputs = if inputs > 0 { inputs } else { max_inputs };
            let selectable_outputs = if outputs > 0 { outputs } else { max_outputs };
            let mut input_choices: Vec<usize> = (1..=selectable_inputs).collect();
            if input > 0 && !input_choices.contains(&input) {
                input_choices.push(input);
                input_choices.sort_unstable();
            }
            let mut input_buttons = div().flex().flex_wrap().gap_2();
            for channel in input_choices {
                let owner = owner.clone();
                let label = if channel > selectable_inputs {
                    format!("{channel}ch{}", t("（利用不可）"))
                } else {
                    format!("{channel}ch")
                };
                input_buttons = input_buttons.child(
                    Button::new(SharedString::from(format!("input-channel:{channel}")))
                        .label(label)
                        .selected(input == channel)
                        .disabled(m.locked() || self.pending())
                        .on_click(move |_, w, cx| {
                            owner.update(cx, |v, cx| {
                                v.changed("/devices/inputChannel", channel.to_string(), w, cx)
                            })
                        }),
                );
            }
            body = body
                .child(t("入力チャンネル（1つ選択）"))
                .child(input_buttons);
            let mut choices = (1..=selectable_outputs)
                .chain(selected.iter().copied())
                .collect::<Vec<_>>();
            choices.sort_unstable();
            choices.dedup();
            let mut output_buttons = div().flex().flex_wrap().gap_2();
            for channel in choices {
                let owner = owner.clone();
                let mut next = selected.clone();
                if selected.contains(&channel) {
                    next.retain(|n| *n != channel);
                } else {
                    next.push(channel);
                }
                next.sort_unstable();
                let next = next
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let label = if channel > selectable_outputs {
                    format!("{channel}ch{}", t("（利用不可）"))
                } else {
                    format!("{channel}ch")
                };
                output_buttons = output_buttons.child(
                    Button::new(SharedString::from(format!("output-channel:{channel}")))
                        .label(label)
                        .selected(selected.contains(&channel))
                        .disabled(m.locked() || self.pending())
                        .on_click(move |_, w, cx| {
                            owner.update(cx, |v, cx| {
                                v.changed("/devices/outputChannels", next.clone(), w, cx)
                            })
                        }),
                );
            }
            body = body
                .child(t("出力チャンネル（複数選択可）"))
                .child(output_buttons);
            if selected.is_empty() {
                body = body.child(
                    div()
                        .text_color(crate::gui::theme::palette(cx).danger)
                        .child(t("出力チャンネルを1つ以上選択してください。")),
                );
            }
            if !rate_available {
                body = body.child(
                    div()
                        .text_color(crate::gui::theme::palette(cx).danger)
                        .child(t(
                            "このサンプリングレートで利用できる入出力チャンネルがありません。",
                        )),
                );
            } else if input > inputs || selected.iter().any(|n| *n > outputs) {
                body = body.child(
                    div()
                        .text_color(crate::gui::theme::palette(cx).danger)
                        .child(t(
                        "選択中のチャンネルはこのデバイスで利用できません。選び直してください。",
                    )),
                );
            }
        } else if !self.device_error.is_empty() {
            body = body.child(
                div()
                    .id("device-discovery-failed")
                    .test_support()
                    .child(t("対応レートを取得できませんでした。")),
            );
        } else if self.discovery_deferred {
            body = body.child(t("音声を停止すると対応レートを確認できます。"));
        } else if self.operation == Some(Operation::Discover) {
            body = body.child(
                div()
                    .id("device-discovery-pending")
                    .test_support()
                    .child(t("対応レートを確認中…")),
            );
        } else if kind == backend::BackendKind::Asio && self.value("/devices/device").is_empty() {
            body = body.child(
                div()
                    .id("device-select-hint")
                    .test_support()
                    .child(t("デバイスを選ぶと対応レートを表示します。")),
            );
        }
        let refresh = owner.clone();
        body = body
            .child(
                Button::new("refresh-devices")
                    .label(t("更新"))
                    .disabled(m.locked() || self.pending())
                    .on_click(move |_, _, cx| refresh.update(cx, |v, cx| v.discover(cx))),
            )
            .child(self.detail_button("devices", owner, cx));
        body.into_any_element()
    }
}

fn format_rates(rates: &[u32]) -> String {
    if rates.is_empty() {
        return "—".into();
    }
    format!(
        "{} Hz",
        rates
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}
