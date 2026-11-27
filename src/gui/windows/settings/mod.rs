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
    draft::{self, Field, Generation},
    layout,
    model::{self, AppModel},
    plots,
    preferences::SECTIONS,
    settings::Settings,
};
use crate::{backend, cli::Options};
use fields::{choices, field_label, labeled_choices, Choice};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
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
enum Widget {
    Input(Entity<InputState>),
    Select(Entity<SelectState<Vec<Choice>>>),
    Toggle,
}
struct Control {
    field: Field,
    widget: Widget,
    slider: Option<Entity<SliderState>>,
    items: Option<Vec<String>>,
    missing: Option<String>,
}
pub struct SettingsView {
    model: Entity<AppModel>,
    base: Settings,
    controls: Vec<Control>,
    subscriptions: Vec<Subscription>,
    _observe: Subscription,
    handle: AnyWindowHandle,
    generation: Generation,
    operation: Option<Operation>,
    discovery_deferred: bool,
    save_failed: bool,
    close: CloseTarget,
    error: String,
    device_error: String,
    notice: String,
    rates: Option<backend::DeviceSampleRates>,
    language: String,
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
        let base = Settings::from_options(&options);
        let handle = window.window_handle();
        let observe = cx.observe(&model, move |view, model, cx| {
            let language = model.read(cx).preferences.language.clone();
            let changed = view.language != language;
            view.language = language;
            let weak = cx.weak_entity();
            cx.notify();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        if changed {
                            view.save_failed = false;
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
        let mut this = Self {
            language: model.read(cx).preferences.language.clone(),
            model,
            base,
            controls: Vec::new(),
            subscriptions: Vec::new(),
            _observe: observe,
            handle: window.window_handle(),
            generation: Generation::default(),
            operation: None,
            discovery_deferred: false,
            save_failed: false,
            close: CloseTarget::None,
            error: String::new(),
            device_error: String::new(),
            notice: String::new(),
            rates: None,
        };
        this.build_controls(window, cx);
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
    pub fn save(&mut self, target: CloseTarget, cx: &mut Context<Self>) {
        self.close = self.close.max(target);
        if self.pending() {
            return;
        }
        let model = self.model.read(cx);
        if model.busy || model.closing {
            return;
        }
        if self.close != CloseTarget::None && self.save_failed {
            let target = self.close;
            self.finish_close(target, None, cx);
            cx.notify();
            return;
        }
        // The draft cannot change during audio; closing it may still persist the same settings.
        let options = match self.options() {
            Ok(o) => o,
            Err(e) => {
                let application_close = self.close == CloseTarget::Application;
                self.error = e;
                self.save_failed = false;
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
        let path = model.path.clone();
        let persisted = model.preferences.clone();
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
                            v.error.clear();
                            v.save_failed = false;
                            v.base = Settings::from_options(&options);
                            v.notice = "設定を保存しました。".into();
                            let target = v.close;
                            v.model.update(cx, |m, cx| {
                                m.saved = options;
                                m.persisted_preferences = persisted;
                                m.quit_save_failed = false;
                                if target != CloseTarget::Application {
                                    m.check_saved_devices(cx);
                                }
                                cx.notify();
                            });
                            v.finish_close(target, Some(window), cx);
                        }
                        Err(e) => {
                            v.error = e;
                            v.save_failed = true;
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
        let section = model.preferences.section.clone();
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
        for (id, label) in SECTIONS {
            let id = id.to_string();
            let selected = section == id;
            nav = nav.child(
                Button::new(SharedString::from(format!("nav-{id}")))
                    .label(t(label))
                    .selected(selected)
                    .disabled(model.shutting_down())
                    .on_click(cx.listener(move |v, _, _, cx| {
                        let mut changed = false;
                        v.model.update(cx, |m, cx| {
                            if !m.shutting_down() && m.preferences.section != id {
                                m.preferences.section = id.clone();
                                m.quit_save_failed = false;
                                changed = true;
                                cx.notify();
                            }
                        });
                        if changed {
                            v.save_failed = false;
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
        if section == "overview" {
            content = overview::render(content, model, self, cx);
        } else {
            content = content.child(self.render_page(&section, cx.entity(), cx));
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
                    .child(if model.active { hint } else { String::new() }),
            )
    }
}

pub fn window_options() -> WindowOptions {
    layout::window_options("設定 / Settings", 1000., 760., 800., 480.)
}
