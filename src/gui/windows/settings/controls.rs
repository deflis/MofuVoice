use super::*;
use crate::gui::preferences::Preferences;

fn initial_device_items(current: &str) -> Vec<String> {
    device_display_items(&[], current)
}

/// Build the select display list from discovered names plus an optional current.
pub(super) fn device_display_items(discovered: &[String], current: &str) -> Vec<String> {
    let mut items = Vec::with_capacity(discovered.len() + 2);
    items.push(String::new());
    items.extend(discovered.iter().cloned());
    if !current.is_empty() && !items.iter().any(|item| item == current) {
        items.push(current.into());
    }
    items
}

fn new_select<T: Clone + PartialEq + 'static>(
    items: Vec<Choice<T>>,
    selected: &T,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
) -> Entity<SelectState<Vec<Choice<T>>>> {
    cx.new(|cx| {
        let mut state = SelectState::new(items, None, window, cx);
        state.set_selected_value(selected, window, cx);
        state
    })
}

fn subscribe_select<T: Clone + PartialEq + 'static>(
    select: &Entity<SelectState<Vec<Choice<T>>>>,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
    on_change: impl Fn(&mut SettingsView, T, &mut Window, &mut Context<SettingsView>) + 'static,
) -> Subscription {
    cx.subscribe_in(select, window, move |this, _, event, window, cx| {
        if let SelectEvent::Confirm(Some(value)) = event {
            on_change(this, value.clone(), window, cx);
        }
    })
}

impl SettingsView {
    pub(super) fn build_selects(
        draft: &SettingsDraft,
        prefs: &Preferences,
        auto_backend: BackendKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (SelectControls, Vec<Subscription>) {
        let resolved_backend = match draft.backend {
            BackendKind::Auto => auto_backend,
            explicit => explicit,
        };
        let mut subscriptions = Vec::new();

        let backend = new_select(fields::backend_choices(prefs), &draft.backend, window, cx);
        subscriptions.push(subscribe_select(&backend, window, cx, Self::set_backend));

        let build_device = |slot: DeviceSlot,
                            window: &mut Window,
                            cx: &mut Context<Self>,
                            subscriptions: &mut Vec<Subscription>| {
            let current = draft.device(slot).to_string();
            let items = initial_device_items(&current);
            let choices = fields::device_choices(slot, &items, prefs, resolved_backend, None);
            let state = new_select(choices, &current, window, cx);
            subscriptions.push(subscribe_select(
                &state,
                window,
                cx,
                move |this, value, window, cx| this.set_device(slot, value, window, cx),
            ));
            DeviceSelect {
                state,
                items,
                discovered: Vec::new(),
                missing: None,
            }
        };
        let input_device = build_device(DeviceSlot::Input, window, cx, &mut subscriptions);
        let output_device = build_device(DeviceSlot::Output, window, cx, &mut subscriptions);
        let preview_device = build_device(DeviceSlot::Preview, window, cx, &mut subscriptions);

        let sample_rate = new_select(
            fields::sample_rate_choices(draft.sample_rate),
            &draft.sample_rate,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &sample_rate,
            window,
            cx,
            Self::set_sample_rate,
        ));

        let pitch_unit = new_select(
            fields::pitch_unit_choices(prefs),
            &draft.pitch_unit,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &pitch_unit,
            window,
            cx,
            Self::set_pitch_unit,
        ));

        let math_precision = new_select(
            fields::math_precision_choices(prefs),
            &draft.math_precision,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &math_precision,
            window,
            cx,
            Self::set_math_precision,
        ));

        let pitch_algorithm = new_select(
            fields::pitch_algorithm_choices(prefs),
            &draft.pitch_algorithm,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &pitch_algorithm,
            window,
            cx,
            Self::set_pitch_algorithm,
        ));

        let psola_pitch_estimator = new_select(
            fields::pitch_estimator_choices(prefs),
            &draft.psola_pitch_estimator,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &psola_pitch_estimator,
            window,
            cx,
            Self::set_psola_pitch_estimator,
        ));

        let analysis_window = new_select(
            fields::analysis_window_choices(draft.analysis_window),
            &draft.analysis_window,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &analysis_window,
            window,
            cx,
            Self::set_analysis_window,
        ));

        let synthesis_window = new_select(
            fields::synthesis_window_choices(draft.analysis_window, draft.synthesis_window),
            &draft.synthesis_window,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &synthesis_window,
            window,
            cx,
            Self::set_synthesis_window,
        ));

        let psola_analysis_window = new_select(
            fields::analysis_window_choices(draft.psola_analysis_window),
            &draft.psola_analysis_window,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &psola_analysis_window,
            window,
            cx,
            Self::set_psola_analysis_window,
        ));

        let psola_synthesis_window = new_select(
            fields::synthesis_window_choices(
                draft.psola_analysis_window,
                draft.psola_synthesis_window,
            ),
            &draft.psola_synthesis_window,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &psola_synthesis_window,
            window,
            cx,
            Self::set_psola_synthesis_window,
        ));

        let gate_threshold_mode = new_select(
            fields::gate_threshold_mode_choices(prefs),
            &draft.gate_threshold_mode,
            window,
            cx,
        );
        subscriptions.push(subscribe_select(
            &gate_threshold_mode,
            window,
            cx,
            Self::set_gate_threshold_mode,
        ));

        (
            SelectControls {
                backend,
                input_device,
                output_device,
                preview_device,
                sample_rate,
                pitch_unit,
                math_precision,
                pitch_algorithm,
                psola_pitch_estimator,
                analysis_window,
                synthesis_window,
                psola_analysis_window,
                psola_synthesis_window,
                gate_threshold_mode,
            },
            subscriptions,
        )
    }

    pub(super) fn create_number_control(
        &self,
        field: NumberField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> NumberControl {
        let text = self.draft.number_text(field);
        let displayed =
            if field == NumberField::Pitch && self.draft.pitch_unit == PitchUnit::Percent {
                voice::display_percent(text)
            } else {
                text.to_string()
            };
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_step(None, window, cx);
            state.set_value(displayed, window, cx);
            state
        });
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, state, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.on_number_input(field, state.read(cx).value().to_string(), window, cx);
                }
            },
        ));
        subscriptions.push(
            cx.subscribe_in(&input, window, move |this, _, event, window, cx| {
                if !this.editable_number(field, cx) {
                    return;
                }
                let NumberInputEvent::Step(action) = event;
                let spec = numbers::spec(field, this.draft.pitch_unit);
                let next = spec.step_value(
                    this.draft.number_text(field),
                    matches!(action, StepAction::Increment),
                );
                this.set_number(field, next, window, cx);
            }),
        );
        let slider = numbers::slider(field, self.draft.pitch_unit).map(|spec| {
            let value = text
                .parse::<f32>()
                .ok()
                .filter(|_| spec.valid(text))
                .unwrap_or(spec.neutral);
            let state = cx.new(|_| {
                SliderState::new()
                    .min(spec.min)
                    .max(spec.max)
                    .step(spec.step)
                    .default_value(value)
            });
            subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, _, event, window, cx| {
                    if let SliderEvent::Change(SliderValue::Single(value)) = event {
                        if !this.editable_number(field, cx) {
                            return;
                        }
                        let spec = numbers::slider(field, this.draft.pitch_unit)
                            .expect("slider specification");
                        if !spec.valid(this.draft.number_text(field)) {
                            return;
                        }
                        let next = spec.text(*value);
                        this.set_number(field, next, window, cx);
                    }
                },
            ));
            state
        });
        NumberControl {
            input,
            slider_focus: slider.as_ref().map(|_| cx.focus_handle()),
            slider,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn options(&self) -> Result<Options, DraftError> {
        let options = self.draft.to_options()?;
        self.validate_discovered_rates(&options)?;
        Ok(options)
    }

    pub fn audio_start_block_reason(&self, saved: &Options) -> Option<&'static str> {
        match self.options() {
            Ok(options) if options != *saved => Some("設定を保存してから音声を開始してください。"),
            Ok(_) => None,
            Err(error) if error.field == Some(ErrorField::SampleRate) => {
                Some("選択したサンプリングレートには対応していません。")
            }
            Err(_) => Some("設定を保存してから音声を開始してください。"),
        }
    }

    pub fn blocks_audio_start(&self, saved: &Options) -> bool {
        self.audio_start_block_reason(saved).is_some()
    }

    pub(super) fn notify_host(&self, cx: &mut Context<Self>) {
        self.model.update(cx, |_, cx| cx.notify());
    }

    pub(super) fn validate_discovered_rates(&self, options: &Options) -> Result<(), DraftError> {
        let Some(rates) = &self.rates else {
            return Ok(());
        };
        let resolved = match self.draft.backend {
            BackendKind::Auto => self.observed.auto_backend,
            explicit => explicit,
        };
        if resolved == BackendKind::Asio && !rates.common.contains(&options.sample_rate) {
            return Err(DraftError::for_field(
                ErrorField::SampleRate,
                "選択したサンプリングレートには対応していません。",
            ));
        }
        Ok(())
    }

    pub(super) fn error_text(&self, cx: &App) -> String {
        self.error
            .as_ref()
            .map(|error| error.format(&self.model.read(cx).preferences, self.draft.pitch_unit))
            .unwrap_or_default()
    }

    pub(super) fn editable_toggle(&self, field: ToggleField, cx: &App) -> bool {
        if self.pending() || self.model.read(cx).locked() {
            return false;
        }
        match field {
            ToggleField::PreserveFormants => {
                self.draft.voice_enabled && !self.draft.pitch_algorithm.uses_td_psola()
            }
            _ => true,
        }
    }

    pub(super) fn editable_select(&self, field: SelectField, cx: &App) -> bool {
        if self.pending() || self.model.read(cx).locked() {
            return false;
        }
        match field {
            SelectField::Backend | SelectField::Device(_) | SelectField::SampleRate => true,
            SelectField::PitchUnit
            | SelectField::MathPrecision
            | SelectField::PitchAlgorithm
            | SelectField::PsolaPitchEstimator
            | SelectField::AnalysisWindow
            | SelectField::SynthesisWindow
            | SelectField::PsolaAnalysisWindow
            | SelectField::PsolaSynthesisWindow => self.draft.voice_enabled,
            SelectField::GateThresholdMode => self.draft.gate_enabled,
        }
    }

    pub(super) fn editable_number(&self, field: NumberField, cx: &App) -> bool {
        if self.pending() || self.model.read(cx).locked() {
            return false;
        }
        match field {
            NumberField::Frames => true,
            NumberField::Pitch => self.draft.voice_enabled,
            NumberField::FormantShift => self.draft.voice_enabled && self.draft.preserve_formants,
            NumberField::GateThresholdDb => {
                self.draft.gate_enabled
                    && self.draft.gate_threshold_mode == GateThresholdMode::Manual
            }
            NumberField::GateAttackMs | NumberField::GateReleaseMs => self.draft.gate_enabled,
            NumberField::DeesserFrequencyHz
            | NumberField::DeesserThresholdDb
            | NumberField::DeesserRangeDb
            | NumberField::DeesserAttackMs
            | NumberField::DeesserReleaseMs => self.draft.deesser_enabled,
            NumberField::CompressorThresholdDb
            | NumberField::CompressorRatio
            | NumberField::CompressorAttackMs
            | NumberField::CompressorReleaseMs => self.draft.compressor_enabled,
            NumberField::LimiterCeilingDb | NumberField::LimiterReleaseMs => {
                self.draft.limiter_enabled
            }
            NumberField::DereverbAmount | NumberField::DereverbDecayMs => {
                self.draft.dereverb_enabled
            }
            NumberField::ReverbMix | NumberField::ReverbPreDelayMs | NumberField::ReverbDecayMs => {
                self.draft.reverb_enabled
            }
            NumberField::LevelTargetDb
            | NumberField::LevelMaxGainDb
            | NumberField::LevelFloorDb
            | NumberField::LevelResponseMs => self.draft.level_enabled,
            NumberField::PostCompressorThresholdDb
            | NumberField::PostCompressorRatio
            | NumberField::PostCompressorAttackMs
            | NumberField::PostCompressorReleaseMs => self.draft.post_compressor_enabled,
            NumberField::EqBand(kind, _, _) | NumberField::EqOutputDb(kind) => {
                self.draft.eq(kind).enabled
            }
        }
    }

    fn clear_feedback(&mut self) {
        self.error = None;
        self.notice.clear();
        self.discard_target = CloseTarget::None;
    }

    fn finish_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.refresh();
        self.notify_host(cx);
        cx.notify();
    }

    pub(super) fn sync_number_widgets(
        &self,
        field: NumberField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(control) = self.numbers.get(&field) else {
            return;
        };
        let text = self.draft.number_text(field);
        let displayed =
            if field == NumberField::Pitch && self.draft.pitch_unit == PitchUnit::Percent {
                voice::display_percent(text)
            } else {
                text.to_string()
            };
        control
            .input
            .update(cx, |state, cx| state.set_value(displayed, window, cx));
        self.sync_slider(field, window, cx);
    }

    fn sync_slider(&self, field: NumberField, window: &mut Window, cx: &mut Context<Self>) {
        let Some(control) = self.numbers.get(&field) else {
            return;
        };
        let (Some(slider), Some(spec)) = (
            &control.slider,
            numbers::slider(field, self.draft.pitch_unit),
        ) else {
            return;
        };
        let text = self.draft.number_text(field);
        let value = text
            .parse::<f32>()
            .ok()
            .filter(|_| spec.valid(text))
            .unwrap_or(spec.neutral);
        slider.update(cx, |state, cx| state.set_value(value, window, cx));
    }

    pub(super) fn on_number_input(
        &mut self,
        field: NumberField,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_number(field, cx) {
            return;
        }
        // Ignore programmatic display rounding echoing back unchanged percent pitch text.
        if field == NumberField::Pitch
            && self.draft.pitch_unit == PitchUnit::Percent
            && self.draft.pitch_exact_semitones.is_some()
            && text == voice::display_percent(&self.draft.pitch)
        {
            return;
        }
        // Input events already own their text state; do not replace that state's cursor on each keystroke.
        self.draft.set_number_text(field, text);
        self.sync_slider(field, window, cx);
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_number(
        &mut self,
        field: NumberField,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_number(field, cx) {
            return;
        }
        self.draft.set_number_text(field, text);
        self.sync_number_widgets(field, window, cx);
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_toggle(
        &mut self,
        field: ToggleField,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_toggle(field, cx) {
            return;
        }
        self.draft.set_toggle(field, value);
        if field == ToggleField::PreserveFormants && !value {
            self.draft
                .set_number_text(NumberField::FormantShift, "0".into());
            self.sync_number_widgets(NumberField::FormantShift, window, cx);
        }
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_backend(
        &mut self,
        backend: BackendKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::Backend, cx) {
            return;
        }
        let changed = self.draft.backend != backend;
        self.draft.backend = backend;
        self.selects
            .backend
            .update(cx, |s, cx| s.set_selected_value(&backend, window, cx));
        self.clear_feedback();
        if changed {
            self.reset_device_choices(window, cx);
            self.discover(cx);
        }
        self.finish_edit(window, cx);
    }

    pub(super) fn set_device(
        &mut self,
        slot: DeviceSlot,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::Device(slot), cx) {
            return;
        }
        let changed = self.draft.device(slot) != value;
        self.draft.set_device(slot, value.clone());
        let kind = self.resolved_backend(cx);
        let used = slot == DeviceSlot::Input || kind == BackendKind::Cpal;
        let select = self.selects.device_mut(slot);
        select.missing =
            (used && !value.is_empty() && !model::device_listed(&value, &select.discovered, kind))
                .then(|| value.clone());
        select.items = device_display_items(&select.discovered, &value);
        let prefs = self.model.read(cx).preferences;
        let items = select.items.clone();
        let missing = select.missing.clone();
        select.state.update(cx, |s, cx| {
            s.set_items(
                fields::device_choices(slot, &items, &prefs, kind, missing.as_deref()),
                window,
                cx,
            );
            s.set_selected_value(&value, window, cx);
        });
        self.sync_missing_device_error();
        self.clear_feedback();
        if changed && matches!(slot, DeviceSlot::Input | DeviceSlot::Output) {
            self.discover(cx);
        }
        self.finish_edit(window, cx);
    }

    fn sync_missing_device_error(&mut self) {
        let has_missing = DeviceSlot::ALL
            .iter()
            .any(|slot| self.selects.device(*slot).missing.is_some());
        if has_missing {
            self.device_error = model::DeviceWarning::Missing.message().into();
        } else if self.device_error == model::DeviceWarning::Missing.message() {
            self.device_error.clear();
        }
    }

    pub(super) fn set_sample_rate(
        &mut self,
        sample_rate: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::SampleRate, cx) {
            return;
        }
        self.draft.sample_rate = sample_rate;
        self.selects
            .sample_rate
            .update(cx, |s, cx| s.set_selected_value(&sample_rate, window, cx));
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_input_channel(
        &mut self,
        channel: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending() || self.model.read(cx).locked() {
            return;
        }
        self.draft.input_channel = channel;
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_output_channels(
        &mut self,
        channels: Vec<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending() || self.model.read(cx).locked() {
            return;
        }
        self.draft.output_channels = channels;
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_pitch_unit(
        &mut self,
        unit: PitchUnit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::PitchUnit, cx) {
            return;
        }
        if unit == self.draft.pitch_unit {
            return;
        }
        let converted = if self.draft.pitch_unit == PitchUnit::Percent {
            if let Some(exact) = self.draft.pitch_exact_semitones {
                Ok(exact)
            } else {
                self.draft
                    .pitch
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| "ピッチの値を入力してから単位を変更してください。".to_string())
                    .and_then(crate::gui::draft::percent_to_semitones)
            }
        } else {
            self.draft
                .pitch
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .ok_or_else(|| "ピッチの値を入力してから単位を変更してください。".to_string())
                .and_then(|n| {
                    crate::pitch::pitch_ratio(n)
                        .map(|_| n)
                        .map_err(|error| error.to_string())
                })
        };
        match converted {
            Ok(semitones) => {
                self.draft.pitch_unit = unit;
                let (pitch_text, exact, slider_value) = match unit {
                    PitchUnit::Percent => {
                        let pct = crate::gui::draft::semitones_to_percent(semitones);
                        (pct.to_string(), Some(semitones), pct as f32)
                    }
                    PitchUnit::Semitones => (semitones.to_string(), None, semitones as f32),
                };
                self.draft.pitch = pitch_text;
                self.draft.pitch_exact_semitones = exact;
                self.selects
                    .pitch_unit
                    .update(cx, |s, cx| s.set_selected_value(&unit, window, cx));
                if let Some(control) = self.numbers.get(&NumberField::Pitch) {
                    let displayed = match unit {
                        PitchUnit::Percent => voice::display_percent(&self.draft.pitch),
                        PitchUnit::Semitones => self.draft.pitch.clone(),
                    };
                    control
                        .input
                        .update(cx, |s, cx| s.set_value(displayed, window, cx));
                    if let Some(slider) = &control.slider {
                        let spec = numbers::slider(NumberField::Pitch, unit).unwrap();
                        slider.update(cx, |s, cx| {
                            *s = SliderState::new()
                                .min(spec.min)
                                .max(spec.max)
                                .step(spec.step)
                                .default_value(slider_value);
                            cx.notify();
                        });
                    }
                }
                self.clear_feedback();
            }
            Err(message) => {
                let previous = self.draft.pitch_unit;
                self.selects
                    .pitch_unit
                    .update(cx, |s, cx| s.set_selected_value(&previous, window, cx));
                self.error = Some(DraftError::plain(message));
            }
        }
        self.finish_edit(window, cx);
    }

    pub(super) fn set_math_precision(
        &mut self,
        precision: MathPrecision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::MathPrecision, cx) {
            return;
        }
        self.draft.math_precision = precision;
        self.selects
            .math_precision
            .update(cx, |s, cx| s.set_selected_value(&precision, window, cx));
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_pitch_algorithm(
        &mut self,
        algorithm: PitchAlgorithm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::PitchAlgorithm, cx) {
            return;
        }
        self.draft.pitch_algorithm = algorithm;
        self.selects
            .pitch_algorithm
            .update(cx, |s, cx| s.set_selected_value(&algorithm, window, cx));
        if algorithm.uses_td_psola() {
            self.draft.preserve_formants = true;
        }
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_psola_pitch_estimator(
        &mut self,
        estimator: PitchEstimator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::PsolaPitchEstimator, cx) {
            return;
        }
        self.draft.psola_pitch_estimator = estimator;
        self.selects
            .psola_pitch_estimator
            .update(cx, |s, cx| s.set_selected_value(&estimator, window, cx));
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_analysis_window(
        &mut self,
        size: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::AnalysisWindow, cx) {
            return;
        }
        self.draft.analysis_window = size;
        if size == 1024 && self.draft.synthesis_window == 2048 {
            self.draft.synthesis_window = 1024;
        }
        self.selects
            .analysis_window
            .update(cx, |s, cx| s.set_selected_value(&size, window, cx));
        self.refresh_choices(window, cx);
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_synthesis_window(
        &mut self,
        size: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::SynthesisWindow, cx) {
            return;
        }
        self.draft.synthesis_window = size;
        self.selects
            .synthesis_window
            .update(cx, |s, cx| s.set_selected_value(&size, window, cx));
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_psola_analysis_window(
        &mut self,
        size: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::PsolaAnalysisWindow, cx) {
            return;
        }
        self.draft.psola_analysis_window = size;
        if size == 1024 && self.draft.psola_synthesis_window == 2048 {
            self.draft.psola_synthesis_window = 1024;
        }
        self.selects
            .psola_analysis_window
            .update(cx, |s, cx| s.set_selected_value(&size, window, cx));
        self.refresh_choices(window, cx);
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_psola_synthesis_window(
        &mut self,
        size: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::PsolaSynthesisWindow, cx) {
            return;
        }
        self.draft.psola_synthesis_window = size;
        self.selects
            .psola_synthesis_window
            .update(cx, |s, cx| s.set_selected_value(&size, window, cx));
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn set_gate_threshold_mode(
        &mut self,
        mode: GateThresholdMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable_select(SelectField::GateThresholdMode, cx) {
            return;
        }
        self.draft.gate_threshold_mode = mode;
        self.selects
            .gate_threshold_mode
            .update(cx, |s, cx| s.set_selected_value(&mode, window, cx));
        self.clear_feedback();
        self.finish_edit(window, cx);
    }

    pub(super) fn refresh_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prefs = self.model.read(cx).preferences;
        let resolved_backend = self.resolved_backend(cx);

        self.selects.backend.update(cx, |s, cx| {
            s.set_items(fields::backend_choices(&prefs), window, cx);
            s.set_selected_value(&self.draft.backend, window, cx);
        });
        for slot in DeviceSlot::ALL {
            let current = self.draft.device(slot).to_string();
            let select = self.selects.device_mut(slot);
            select.items = device_display_items(&select.discovered, &current);
            let choices = fields::device_choices(
                slot,
                &select.items,
                &prefs,
                resolved_backend,
                select.missing.as_deref(),
            );
            select.state.update(cx, |s, cx| {
                s.set_items(choices, window, cx);
                s.set_selected_value(&current, window, cx);
            });
        }
        self.selects.sample_rate.update(cx, |s, cx| {
            s.set_items(
                fields::sample_rate_choices(self.draft.sample_rate),
                window,
                cx,
            );
            s.set_selected_value(&self.draft.sample_rate, window, cx);
        });
        self.selects.pitch_unit.update(cx, |s, cx| {
            s.set_items(fields::pitch_unit_choices(&prefs), window, cx);
            s.set_selected_value(&self.draft.pitch_unit, window, cx);
        });
        self.selects.math_precision.update(cx, |s, cx| {
            s.set_items(fields::math_precision_choices(&prefs), window, cx);
            s.set_selected_value(&self.draft.math_precision, window, cx);
        });
        self.selects.pitch_algorithm.update(cx, |s, cx| {
            s.set_items(fields::pitch_algorithm_choices(&prefs), window, cx);
            s.set_selected_value(&self.draft.pitch_algorithm, window, cx);
        });
        self.selects.psola_pitch_estimator.update(cx, |s, cx| {
            s.set_items(fields::pitch_estimator_choices(&prefs), window, cx);
            s.set_selected_value(&self.draft.psola_pitch_estimator, window, cx);
        });
        self.selects.analysis_window.update(cx, |s, cx| {
            s.set_items(
                fields::analysis_window_choices(self.draft.analysis_window),
                window,
                cx,
            );
            s.set_selected_value(&self.draft.analysis_window, window, cx);
        });
        self.selects.synthesis_window.update(cx, |s, cx| {
            s.set_items(
                fields::synthesis_window_choices(
                    self.draft.analysis_window,
                    self.draft.synthesis_window,
                ),
                window,
                cx,
            );
            s.set_selected_value(&self.draft.synthesis_window, window, cx);
        });
        self.selects.psola_analysis_window.update(cx, |s, cx| {
            s.set_items(
                fields::analysis_window_choices(self.draft.psola_analysis_window),
                window,
                cx,
            );
            s.set_selected_value(&self.draft.psola_analysis_window, window, cx);
        });
        self.selects.psola_synthesis_window.update(cx, |s, cx| {
            s.set_items(
                fields::synthesis_window_choices(
                    self.draft.psola_analysis_window,
                    self.draft.psola_synthesis_window,
                ),
                window,
                cx,
            );
            s.set_selected_value(&self.draft.psola_synthesis_window, window, cx);
        });
        self.selects.gate_threshold_mode.update(cx, |s, cx| {
            s.set_items(fields::gate_threshold_mode_choices(&prefs), window, cx);
            s.set_selected_value(&self.draft.gate_threshold_mode, window, cx);
        });
    }

    pub(super) fn reset_device_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for slot in DeviceSlot::ALL {
            self.draft.set_device(slot, String::new());
            let select = self.selects.device_mut(slot);
            select.discovered.clear();
            select.items = device_display_items(&select.discovered, "");
            select.missing = None;
        }
        self.refresh_choices(window, cx);
    }
}
