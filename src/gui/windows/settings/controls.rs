use super::*;
impl SettingsView {
    pub(super) fn build_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.controls.clear();
        self.subscriptions.clear();
        let percent = self.base.options().expect("validated settings").pitch_unit == "percent";
        for field in draft::fields(&self.base) {
            let path = field.path.clone();
            let items = choices(&path, &field.text);
            let widget = if field.value.is_boolean() {
                Widget::Toggle
            } else if let Some(items) = items.clone() {
                let value = field.text.clone();
                let items = labeled_choices(&path, items, &self.model.read(cx).preferences);
                let select = cx.new(|cx| {
                    let mut s = SelectState::new(items, None, window, cx);
                    s.set_selected_value(&value, window, cx);
                    s
                });
                self.subscriptions.push(cx.subscribe_in(
                    &select,
                    window,
                    move |this, _, event, window, cx| {
                        if let SelectEvent::Confirm(Some(value)) = event {
                            this.changed(&path, value.clone(), window, cx);
                        }
                    },
                ));
                Widget::Select(select)
            } else {
                let value = if field.path == "/voice/pitch" && percent {
                    voice::display_percent(&field.text)
                } else {
                    field.text.clone()
                };
                let input = cx.new(|cx| {
                    let mut s = InputState::new(window, cx);
                    s.set_step(None, window, cx);
                    s.set_value(value, window, cx);
                    s
                });
                let input_path = path.clone();
                self.subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, state, event, window, cx| {
                        if matches!(event, InputEvent::Change) {
                            this.changed(
                                &input_path,
                                state.read(cx).value().to_string(),
                                window,
                                cx,
                            );
                        }
                    },
                ));
                if field.value.is_number() {
                    self.subscriptions.push(cx.subscribe_in(
                        &input,
                        window,
                        move |this, _, event, window, cx| {
                            if !this.editable(&path, cx) {
                                return;
                            }
                            let NumberInputEvent::Step(action) = event;
                            let spec = numbers::spec(&path, this.percent())
                                .expect("numeric field specification");
                            let value = spec.step_value(
                                &this.value(&path),
                                matches!(action, StepAction::Increment),
                            );
                            this.changed(&path, value.clone(), window, cx);
                            this.set_value(&path, value, window, cx);
                        },
                    ));
                }
                Widget::Input(input)
            };
            let slider = if let Some(spec) = numbers::slider(&field.path, percent) {
                let value = field
                    .text
                    .parse::<f32>()
                    .ok()
                    .filter(|_| spec.valid(&field.text))
                    .unwrap_or(spec.neutral);
                let state = cx.new(|_| {
                    SliderState::new()
                        .min(spec.min)
                        .max(spec.max)
                        .step(spec.step)
                        .default_value(value)
                });
                let path = field.path.clone();
                self.subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, _, event, window, cx| {
                        if let SliderEvent::Change(SliderValue::Single(value)) = event {
                            if !this.editable(&path, cx) {
                                return;
                            }
                            let spec = numbers::slider(&path, this.percent())
                                .expect("slider specification");
                            if !spec.valid(&this.value(&path)) {
                                return;
                            }
                            let text = spec.text(*value);
                            this.changed(&path, text.clone(), window, cx);
                            this.set_value(&path, text, window, cx);
                        }
                    },
                ));
                Some(state)
            } else {
                None
            };
            self.controls.push(Control {
                field,
                widget,
                slider,
                items,
            });
        }
        self.refresh_choices(window, cx);
    }
    pub(super) fn percent(&self) -> bool {
        self.value("/voice/pitchUnit") == "percent"
    }
    pub(super) fn edits(&self) -> BTreeMap<String, String> {
        self.controls
            .iter()
            .map(|c| (c.field.path.clone(), c.field.text.clone()))
            .collect()
    }
    pub(super) fn options(&self) -> Result<Options, String> {
        draft::parse(&self.base, &self.edits())
    }
    pub(super) fn value(&self, path: &str) -> String {
        self.controls
            .iter()
            .find(|c| c.field.path == path)
            .map(|c| c.field.text.clone())
            .unwrap_or_default()
    }
    pub(super) fn set_value(
        &mut self,
        path: &str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let percent = self.percent();
        if let Some(control) = self.controls.iter_mut().find(|c| c.field.path == path) {
            control.field.text = value.clone();
            match &control.widget {
                Widget::Select(s) => s.update(cx, |s, cx| s.set_selected_value(&value, window, cx)),
                Widget::Input(s) => {
                    let displayed = if path == "/voice/pitch" && percent {
                        voice::display_percent(&value)
                    } else {
                        value.clone()
                    };
                    s.update(cx, |s, cx| s.set_value(displayed, window, cx));
                }
                Widget::Toggle => {}
            }
            if let (Some(slider), Some(spec)) = (&control.slider, numbers::slider(path, percent)) {
                let n = value
                    .parse::<f32>()
                    .ok()
                    .filter(|_| spec.valid(&value))
                    .unwrap_or(spec.neutral);
                slider.update(cx, |s, cx| s.set_value(n, window, cx));
            }
        }
    }
    pub(super) fn refresh_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prefs = self.model.read(cx).preferences.clone();
        for i in 0..self.controls.len() {
            let path = self.controls[i].field.path.clone();
            let Some(mut items) = self.controls[i].items.clone() else {
                continue;
            };
            let analysis = match path.as_str() {
                "/voice/synthesisWindow" => Some(self.value("/voice/analysisWindow")),
                "/voice/psolaSynthesisWindow" => Some(self.value("/voice/psolaAnalysisWindow")),
                _ => None,
            };
            if let Some(analysis) = analysis.and_then(|s| s.parse::<usize>().ok()) {
                items.retain(|s| s.parse::<usize>().is_ok_and(|n| n <= analysis));
            }
            let control = &self.controls[i];
            let current = control.field.text.clone();
            if !items.contains(&current) {
                items.push(current.clone());
            }
            if let Widget::Select(state) = &control.widget {
                state.update(cx, |s, cx| {
                    s.set_items(labeled_choices(&path, items, &prefs), window, cx);
                    s.set_selected_value(&current, window, cx);
                });
            }
        }
    }
    pub(super) fn error_text(&self, cx: &App) -> String {
        let prefs = &self.model.read(cx).preferences;
        let mut error = prefs.text(&self.error);
        let mut controls: Vec<_> = self.controls.iter().collect();
        controls.sort_by_key(|c| std::cmp::Reverse(c.field.path.len()));
        for c in controls {
            error = error.replace(&c.field.path, &prefs.text(&field_label(&c.field.path)));
        }
        error
    }
    pub(super) fn editable(&self, path: &str, cx: &App) -> bool {
        if self.pending || self.model.read(cx).locked() {
            return false;
        }
        if path == "/voice/enabled"
            || path == "/noise/enabled"
            || path == "/deesser/enabled"
            || path.ends_with("/enabled")
            || path == "/level/compressorEnabled"
            || path == "/noise/rumbleFilter"
        {
            return true;
        }
        if path.starts_with("/voice/") {
            if self.value("/voice/enabled") != "true" {
                return false;
            }
            if path == "/voice/formantShift" && self.value("/voice/preserveFormants") != "true" {
                return false;
            }
            if path == "/voice/preserveFormants"
                && self.value("/voice/pitchAlgorithm").starts_with("td-psola")
            {
                return false;
            }
        }
        if let Some(root) = numbers::eq_root(path) {
            return self.value(&format!("/{root}/enabled")) == "true";
        }
        if path.starts_with("/deesser/") {
            return self.value("/deesser/enabled") == "true";
        }
        if path.starts_with("/noise/") {
            return self.value("/noise/enabled") == "true"
                && (path != "/noise/thresholdDb"
                    || self.value("/noise/thresholdMode") == "manual");
        }
        if path.starts_with("/level/") {
            let flag = if draft::group(path, "postCompressor") {
                "/level/compressorEnabled"
            } else {
                "/level/enabled"
            };
            return self.value(flag) == "true";
        }
        true
    }
    pub(super) fn changed(
        &mut self,
        path: &str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable(path, cx) {
            return;
        }
        if path == "/voice/pitchUnit" && value != self.value(path) {
            let old = self
                .value("/voice/pitch")
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite());
            let converted = old
                .ok_or_else(|| "ピッチの値を入力してから単位を変更してください。".to_string())
                .and_then(|n| {
                    if self.percent() {
                        draft::percent_to_semitones(n)
                    } else {
                        Ok(n)
                    }
                });
            match converted {
                Ok(semitones) => {
                    let pitch = if value == "percent" {
                        draft::semitones_to_percent(semitones)
                    } else {
                        semitones
                    };
                    self.set_value(path, value, window, cx);
                    self.set_value("/voice/pitch", pitch.to_string(), window, cx);
                    if let Some(control) = self
                        .controls
                        .iter()
                        .find(|c| c.field.path == "/voice/pitch")
                    {
                        if let Some(slider) = &control.slider {
                            let spec = numbers::slider("/voice/pitch", self.percent()).unwrap();
                            slider.update(cx, |s, cx| {
                                *s = SliderState::new()
                                    .min(spec.min)
                                    .max(spec.max)
                                    .step(spec.step)
                                    .default_value(pitch as f32);
                                cx.notify();
                            });
                        }
                    }
                    self.error.clear();
                }
                Err(e) => {
                    let previous = self.value(path);
                    self.set_value(path, previous, window, cx);
                    self.error = e;
                }
            }
            window.refresh();
            cx.notify();
            return;
        }
        // Input events already own their text state; do not replace that state's cursor on each keystroke.
        if let Some(control) = self.controls.iter_mut().find(|c| c.field.path == path) {
            control.field.text = value.clone();
        }
        if let Some(spec) = numbers::slider(path, self.percent()) {
            if let Some(slider) = self
                .controls
                .iter()
                .find(|c| c.field.path == path)
                .and_then(|c| c.slider.as_ref())
            {
                let number = value
                    .parse::<f32>()
                    .ok()
                    .filter(|_| spec.valid(&value))
                    .unwrap_or(spec.neutral);
                slider.update(cx, |s, cx| s.set_value(number, window, cx));
            }
        }
        self.error.clear();
        self.notice.clear();
        if path == "/voice/pitchAlgorithm" && value.starts_with("td-psola") {
            self.set_value("/voice/preserveFormants", "true".into(), window, cx);
        }
        if path == "/voice/preserveFormants" && value == "false" {
            self.set_value("/voice/formantShift", "0".into(), window, cx);
        }
        for (analysis, synthesis) in [
            ("/voice/analysisWindow", "/voice/synthesisWindow"),
            ("/voice/psolaAnalysisWindow", "/voice/psolaSynthesisWindow"),
        ] {
            if path == analysis && value == "1024" && self.value(synthesis) == "2048" {
                self.set_value(synthesis, "1024".into(), window, cx);
            }
        }
        if path.ends_with("AnalysisWindow") || path == "/voice/analysisWindow" {
            self.refresh_choices(window, cx);
        }
        if path == "/devices/backend" {
            for p in [
                "/devices/device",
                "/devices/outputDevice",
                "/devices/previewDevice",
            ] {
                self.set_value(p, String::new(), window, cx);
            }
        }
        if [
            "/devices/backend",
            "/devices/device",
            "/devices/outputDevice",
        ]
        .contains(&path)
        {
            self.discover(cx);
        }
        window.refresh();
        cx.notify();
    }
}
