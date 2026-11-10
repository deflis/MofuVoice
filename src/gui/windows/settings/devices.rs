use super::*;
impl SettingsView {
    pub fn discover(&mut self, cx: &mut Context<Self>) {
        if self.pending || self.model.read(cx).locked() {
            return;
        }
        let kind = match backend::BackendKind::parse(&self.value("/devices/backend")) {
            Ok(k) => k,
            Err(e) => {
                self.error = e.to_string();
                cx.notify();
                return;
            }
        };
        self.pending = true;
        self.rates = None;
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
            #[cfg(all(windows, feature = "asio"))]
            let _apartment = if choices.resolved == backend::BackendKind::Asio {
                Some(crate::session::ComApartment::initialize().map_err(|e| e.to_string())?)
            } else {
                None
            };
            let rates = backend::select(choices.resolved, &output)
                .and_then(|s| s.supported_sample_rates(&device))
                .map_err(|e| e.to_string());
            Ok::<_, String>((choices, rates))
        });
        let model = self.model.clone();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            // Always release the app-level operation gate, even if the view disappeared.
            model.update(cx, |m, cx| {
                m.busy = false;
                cx.notify();
            });
            _ = handle.update(cx, |_, window, cx| {
                _ = this.update(cx, |v, cx| {
                    if !v.generation.accepts(generation) {
                        return;
                    }
                    v.pending = false;
                    match result {
                        Ok((choices, rates)) => {
                            for (path, mut items) in [
                                ("/devices/device", choices.inputs),
                                ("/devices/outputDevice", choices.outputs.clone()),
                                ("/devices/previewDevice", choices.outputs),
                            ] {
                                items.insert(0, String::new());
                                let current = v.value(path);
                                if !items.contains(&current) {
                                    v.error = format!("Device unavailable: {current}");
                                    items.push(current.clone());
                                }
                                if let Some(control) =
                                    v.controls.iter_mut().find(|c| c.field.path == path)
                                {
                                    if let Widget::Select(state) = &control.widget {
                                        control.items = Some(items.clone());
                                        let choices = labeled_choices(
                                            path,
                                            items,
                                            &v.model.read(cx).preferences,
                                        );
                                        state.update(cx, |s, cx| {
                                            s.set_items(choices, window, cx);
                                            s.set_selected_value(&current, window, cx);
                                        });
                                    }
                                }
                            }
                            match rates {
                                Ok(rates) => v.rates = Some(rates),
                                Err(e) => v.error = e,
                            }
                        }
                        Err(e) => v.error = e,
                    }
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
}

impl SettingsView {
    pub(super) fn render_devices(&self, owner: Entity<Self>, cx: &App) -> AnyElement {
        use super::presentation::FieldStyle;
        let m = self.model.read(cx);
        let t = |s: &str| m.text(s);
        let kind = backend::resolve(
            backend::BackendKind::parse(&self.value("/devices/backend"))
                .expect("backend selection"),
        );
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
                    .child(format!("{} {:?} Hz", t("入力の対応レート："), rates.input))
                    .child(format!("{} {:?} Hz", t("出力の対応レート："), rates.output));
            }
            body = body.child(format!(
                "{} {:?} Hz",
                t("入出力で変換なしに使えるレート"),
                rates.common
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
            let inputs = rates
                .channels
                .iter()
                .filter(|l| l.sample_rate == rate && l.output >= required)
                .map(|l| l.input)
                .max()
                .unwrap_or(0);
            let outputs = rates
                .channels
                .iter()
                .filter(|l| l.sample_rate == rate && l.input >= input)
                .map(|l| l.output)
                .max()
                .unwrap_or(0);
            let mut input_buttons = div().flex().flex_wrap().gap_2();
            for channel in 1..=inputs {
                let owner = owner.clone();
                input_buttons = input_buttons.child(
                    Button::new(SharedString::from(format!("input-channel:{channel}")))
                        .label(format!("{channel}ch"))
                        .selected(input == channel)
                        .disabled(m.locked() || self.pending)
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
            let mut choices = (1..=outputs)
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
                output_buttons = output_buttons.child(
                    Button::new(SharedString::from(format!("output-channel:{channel}")))
                        .label(format!("{channel}ch"))
                        .selected(selected.contains(&channel))
                        .disabled(m.locked() || self.pending)
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
            if input > inputs || selected.iter().any(|n| *n > outputs) {
                body = body.child(
                    div()
                        .text_color(crate::gui::theme::palette(cx).danger)
                        .child(t(
                        "選択中のチャンネルはこのデバイスで利用できません。選び直してください。",
                    )),
                );
            }
        } else {
            body = body.child(t(
                "利用可能なチャンネルを確認中です。音声動作中は停止して確認してください。",
            ));
        }
        let refresh = owner.clone();
        body = body
            .child(
                Button::new("refresh-devices")
                    .label(t("更新"))
                    .disabled(m.locked() || self.pending)
                    .on_click(move |_, _, cx| refresh.update(cx, |v, cx| v.discover(cx))),
            )
            .child(self.detail_button("devices", owner, cx));
        body.into_any_element()
    }
}
