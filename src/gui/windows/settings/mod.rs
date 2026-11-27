mod controls;
mod deesser;
mod details;
mod devices;
mod eq;
mod fields;
mod numbers;
mod overview;
mod presentation;
#[cfg(all(test, feature = "desktop-tests"))]
mod tests;
mod voice;
use crate::gui::{
    draft::{
        DeviceSlot, DraftError, EqBandField, EqKind, ErrorField, GateThresholdMode, Generation,
        NumberField, PitchUnit, ResetTarget, SelectField, SettingsDraft, ToggleField, UiMode,
    },
    layout,
    model::{self, AppModel},
    plots,
    preferences::{Language, Section},
};
use crate::{
    backend::{self, BackendKind},
    cli::Options,
    pitch::{MathPrecision, PitchAlgorithm, PitchEstimator},
};
use fields::Choice;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
        select::{Select, SelectEvent, SelectState},
        slider::{Slider, SliderEvent, SliderState, SliderValue},
        Disableable, Selectable, WindowExt,
    },
    prelude::FluentBuilder,
    *,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CloseTarget {
    None,
    Settings,
    Application,
}

struct DeviceSelect {
    state: Entity<SelectState<Vec<Choice<String>>>>,
    items: Vec<String>,
    missing: Option<String>,
}

struct SelectControls {
    backend: Entity<SelectState<Vec<Choice<BackendKind>>>>,
    input_device: DeviceSelect,
    output_device: DeviceSelect,
    preview_device: DeviceSelect,
    sample_rate: Entity<SelectState<Vec<Choice<u32>>>>,
    pitch_unit: Entity<SelectState<Vec<Choice<PitchUnit>>>>,
    math_precision: Entity<SelectState<Vec<Choice<MathPrecision>>>>,
    pitch_algorithm: Entity<SelectState<Vec<Choice<PitchAlgorithm>>>>,
    psola_pitch_estimator: Entity<SelectState<Vec<Choice<PitchEstimator>>>>,
    analysis_window: Entity<SelectState<Vec<Choice<usize>>>>,
    synthesis_window: Entity<SelectState<Vec<Choice<usize>>>>,
    psola_analysis_window: Entity<SelectState<Vec<Choice<usize>>>>,
    psola_synthesis_window: Entity<SelectState<Vec<Choice<usize>>>>,
    gate_threshold_mode: Entity<SelectState<Vec<Choice<GateThresholdMode>>>>,
}

impl SelectControls {
    fn device(&self, slot: DeviceSlot) -> &DeviceSelect {
        match slot {
            DeviceSlot::Input => &self.input_device,
            DeviceSlot::Output => &self.output_device,
            DeviceSlot::Preview => &self.preview_device,
        }
    }

    fn device_mut(&mut self, slot: DeviceSlot) -> &mut DeviceSelect {
        match slot {
            DeviceSlot::Input => &mut self.input_device,
            DeviceSlot::Output => &mut self.output_device,
            DeviceSlot::Preview => &mut self.preview_device,
        }
    }
}

struct NumberControl {
    input: Entity<InputState>,
    slider: Option<Entity<SliderState>>,
    _subscriptions: Vec<Subscription>,
}

pub struct SettingsView {
    model: Entity<AppModel>,
    base: Options,
    draft: SettingsDraft,
    selects: SelectControls,
    numbers: BTreeMap<NumberField, NumberControl>,
    _select_subscriptions: Vec<Subscription>,
    _observe: Subscription,
    handle: AnyWindowHandle,
    generation: Generation,
    operation: Option<Operation>,
    discovery_deferred: bool,
    discard_target: CloseTarget,
    close: CloseTarget,
    error: Option<DraftError>,
    device_error: String,
    notice: String,
    rates: Option<backend::DeviceSampleRates>,
    observed: ObservedModel,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ObservedModel {
    language: Language,
    section: Section,
    active: bool,
    busy: bool,
    closing: bool,
    quit_requested: bool,
    auto_backend: BackendKind,
}

impl ObservedModel {
    fn capture(model: &AppModel) -> Self {
        Self {
            language: model.preferences.language,
            section: model.preferences.section,
            active: model.active,
            busy: model.busy,
            closing: model.closing,
            quit_requested: model.quit_requested,
            auto_backend: model.resolve_backend(BackendKind::Auto),
        }
    }

    fn locked(&self) -> bool {
        self.active || self.busy || self.closing || self.quit_requested
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Save,
    Discover,
}

impl SettingsView {
    fn pending(&self) -> bool {
        self.operation.is_some()
    }

    pub fn new(
        model: Entity<AppModel>,
        options: Options,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = SettingsDraft::from_options(&options);
        let handle = window.window_handle();
        let observe = cx.observe(&model, move |view, model, cx| {
            let snapshot = ObservedModel::capture(model.read(cx));
            let language_changed = view.observed.language != snapshot.language;
            let section_changed = view.observed.section != snapshot.section;
            let backend_changed = view.observed.auto_backend != snapshot.auto_backend;
            let model_changed = view.observed != snapshot;
            let resume_close = view.close != CloseTarget::None
                && !view.pending()
                && !snapshot.busy
                && !snapshot.closing;
            let resume_discover = view.discovery_deferred && !snapshot.locked();
            if !model_changed && !resume_close && !resume_discover {
                return;
            }
            view.observed = snapshot;
            let weak = cx.weak_entity();
            cx.notify();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        if language_changed || section_changed {
                            view.discard_target = CloseTarget::None;
                        }
                        if language_changed || backend_changed {
                            view.refresh_choices(window, cx);
                        }
                        if view.close != CloseTarget::None
                            && !view.pending()
                            && !view.model.read(cx).busy
                            && !view.model.read(cx).closing
                        {
                            view.save(view.close, cx);
                        } else if view.discovery_deferred && !view.model.read(cx).locked() {
                            view.discover(cx);
                        }
                    });
                    window.refresh();
                });
            });
        });
        let observed = ObservedModel::capture(model.read(cx));
        let prefs = model.read(cx).preferences;
        let (selects, select_subscriptions) =
            Self::build_selects(&draft, &prefs, observed.auto_backend, window, cx);
        let mut this = Self {
            observed,
            model,
            base: options,
            draft,
            selects,
            numbers: BTreeMap::new(),
            _select_subscriptions: select_subscriptions,
            _observe: observe,
            handle: window.window_handle(),
            generation: Generation::default(),
            operation: None,
            discovery_deferred: false,
            discard_target: CloseTarget::None,
            close: CloseTarget::None,
            error: None,
            device_error: String::new(),
            notice: String::new(),
            rates: None,
        };
        for field in this.draft.number_fields() {
            let control = this.create_number_control(field, window, cx);
            this.numbers.insert(field, control);
        }
        this.refresh_choices(window, cx);
        this
    }

    fn finish_close(
        &mut self,
        target: CloseTarget,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.close = CloseTarget::None;
        match target {
            CloseTarget::None => {}
            CloseTarget::Application => {
                self.model.update(cx, |m, cx| m.shutdown(cx));
            }
            CloseTarget::Settings => {
                self.generation.advance();
                self.model.update(cx, |m, cx| {
                    m.settings = None;
                    cx.notify();
                });
                if let Some(window) = window {
                    window.remove_window();
                } else {
                    let handle = self.handle;
                    cx.defer(move |cx| {
                        _ = handle.update(cx, |_, window, _| window.remove_window());
                    });
                }
            }
        }
    }
    fn discard_and_close(&mut self, cx: &mut Context<Self>) {
        if self.pending() || self.model.read(cx).shutting_down() {
            return;
        }
        self.finish_close(self.discard_target, None, cx);
        cx.notify();
    }

    pub fn save(&mut self, target: CloseTarget, cx: &mut Context<Self>) {
        self.close = self.close.max(target);
        if self.pending() {
            return;
        }
        let model = self.model.read(cx);
        if model.busy || model.closing {
            return;
        }
        let options = self.draft.to_options().and_then(|options| {
            if self.close == CloseTarget::None || options != model.saved {
                self.validate_discovered_rates(&options)?;
            }
            Ok(options)
        });
        let options = match options {
            Ok(o) => o,
            Err(e) => {
                let application_close = self.close == CloseTarget::Application;
                self.error = Some(e);
                self.discard_target = self.close;
                self.close = CloseTarget::None;
                self.model.update(cx, |m, cx| {
                    m.quit_requested = false;
                    cx.notify();
                });
                if application_close {
                    let handle = self.handle;
                    cx.defer(move |cx| {
                        _ = handle.update(cx, |_, window, _| window.activate_window());
                    });
                }
                cx.notify();
                return;
            }
        };
        if self.close != CloseTarget::None
            && options == model.saved
            && model.preferences == model.persisted_preferences
        {
            let target = self.close;
            self.finish_close(target, None, cx);
            cx.notify();
            return;
        }
        self.discard_target = CloseTarget::None;
        let path = model.path.clone();
        let persisted = model.preferences;
        let prefs = persisted.stored();
        let saved = options.clone();
        self.operation = Some(Operation::Save);
        self.model.update(cx, |m, cx| {
            m.busy = true;
            cx.notify();
        });
        let generation = self.generation.advance();
        let handle = self.handle;
        let model = self.model.clone();
        let task = cx.background_executor().spawn(async move {
            crate::gui_settings::save(&path, &saved, &prefs).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            model.update(cx, |m, cx| {
                m.busy = false;
                cx.notify();
            });
            _ = handle.update(cx, |_, window, cx| {
                _ = this.update(cx, |v, cx| {
                    if !v.generation.accepts(generation) {
                        return;
                    }
                    v.operation = None;
                    match result {
                        Ok(()) => {
                            v.error = None;
                            v.discard_target = CloseTarget::None;
                            v.base = options.clone();
                            v.notice = "設定を保存しました。".into();
                            let target = v.close;
                            v.model.update(cx, |m, cx| {
                                let options_changed = m.saved != options;
                                m.saved = options;
                                m.persisted_preferences = persisted;
                                m.quit_save_failed = false;
                                if target != CloseTarget::Application
                                    && !m.active
                                    && options_changed
                                {
                                    m.check_saved_devices(cx);
                                }
                                cx.notify();
                            });
                            v.finish_close(target, Some(window), cx);
                        }
                        Err(e) => {
                            v.error = Some(DraftError::plain(e));
                            v.discard_target = v.close;
                            v.close = CloseTarget::None;
                            v.model.update(cx, |m, cx| {
                                m.quit_requested = false;
                                cx.notify();
                            });
                            window.activate_window();
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let section = model.preferences.section;
        let locked = model.locked() || self.pending();
        let t = |s: &str| model.text(s);
        let mut nav = div()
            .id("settings-navigation")
            .w(px(185.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .overflow_y_scroll();
        for item in Section::ALL {
            let selected = section == item;
            nav = nav.child(
                Button::new(SharedString::from(format!("nav-{}", item.id())))
                    .label(t(item.label()))
                    .selected(selected)
                    .disabled(model.shutting_down())
                    .on_click(cx.listener(move |v, _, _, cx| {
                        let mut changed = false;
                        v.model.update(cx, |m, cx| {
                            if !m.shutting_down() && m.preferences.section != item {
                                m.preferences.section = item;
                                m.quit_save_failed = false;
                                changed = true;
                                cx.notify();
                            }
                        });
                        if changed {
                            v.discard_target = CloseTarget::None;
                        }
                    })),
            );
        }
        let mut content = div()
            .id("settings-content")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .overflow_y_scroll();
        if section == Section::Overview {
            content = overview::render(content, model, self, cx);
        } else {
            content = content.child(self.render_page(section, cx.entity(), cx));
        }

        let error = self.error_text(cx);
        let notice = self.notice.clone();
        let save = t("設定を保存");
        let hint = t("音声を停止してから設定を変更してください。");
        layout::surface(cx)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_4()
                    .child(nav)
                    .child(content),
            )
            .when(!error.is_empty(), |body| {
                body.child(
                    div()
                        .text_color(crate::gui::theme::palette(cx).danger)
                        .child(error),
                )
            })
            .when(!self.device_error.is_empty(), |body| {
                body.child(
                    div()
                        .text_color(crate::gui::theme::palette(cx).danger)
                        .child(t(&self.device_error)),
                )
            })
            .when(!notice.is_empty(), |body| body.child(t(&notice)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("save-settings")
                            .primary()
                            .label(save)
                            .disabled(locked)
                            .on_click(cx.listener(|v, _, _, cx| v.save(CloseTarget::None, cx))),
                    )
                    .when(self.discard_target != CloseTarget::None, |row| {
                        row.child(
                            Button::new("discard-settings")
                                .label(t(if self.discard_target == CloseTarget::Application {
                                    "変更を破棄して終了"
                                } else {
                                    "変更を破棄して閉じる"
                                }))
                                .disabled(self.pending() || model.shutting_down())
                                .on_click(cx.listener(|v, _, _, cx| v.discard_and_close(cx))),
                        )
                    })
                    .child(if model.active { hint } else { String::new() }),
            )
    }
}

pub fn window_options() -> WindowOptions {
    layout::window_options("設定 / Settings", 1000., 760., 800., 480.)
}
