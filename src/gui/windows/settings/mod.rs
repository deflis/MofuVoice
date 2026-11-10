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
}
pub struct SettingsView {
    model: Entity<AppModel>,
    base: Settings,
    controls: Vec<Control>,
    subscriptions: Vec<Subscription>,
    _observe: Subscription,
    handle: AnyWindowHandle,
    generation: Generation,
    pending: bool,
    close: CloseTarget,
    error: String,
    notice: String,
    rates: Option<backend::DeviceSampleRates>,
    language: String,
}
impl SettingsView {
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
                    if changed {
                        let _ = weak.update(cx, |view, cx| view.refresh_choices(window, cx));
                    }
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
            pending: false,
            close: CloseTarget::None,
            error: String::new(),
            notice: String::new(),
            rates: None,
        };
        this.build_controls(window, cx);
        this
    }
    pub fn save(&mut self, target: CloseTarget, cx: &mut Context<Self>) {
        self.close = self.close.max(target);
        if self.pending {
            return;
        }
        let model = self.model.read(cx);
        if model.busy || model.closing {
            return;
        }
        // The draft cannot change during audio; closing it may still persist the same settings.
        let options = match self.options() {
            Ok(o) => o,
            Err(e) => {
                self.error = e;
                self.close = CloseTarget::None;
                self.model.update(cx, |m, cx| {
                    m.quit_requested = false;
                    cx.notify();
                });
                cx.notify();
                return;
            }
        };
        let path = model.path.clone();
        let prefs = model.preferences.clone();
        let saved = options.clone();
        self.pending = true;
        self.model.update(cx, |m, cx| {
            m.busy = true;
            cx.notify();
        });
        let generation = self.generation.advance();
        let handle = self.handle;
        let model = self.model.clone();
        let task = cx.background_executor().spawn(async move {
            prefs.save(&path.with_file_name("ui.ini"))?;
            crate::gui_settings::save(&path, &saved).map_err(|e| e.to_string())
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
                    v.pending = false;
                    match result {
                        Ok(()) => {
                            v.base = Settings::from_options(&options);
                            v.notice = "設定を保存しました。".into();
                            let target = v.close;
                            v.close = CloseTarget::None;
                            v.model.update(cx, |m, cx| {
                                m.saved = options;
                                if target == CloseTarget::Application {
                                    m.shutdown(cx);
                                } else if target == CloseTarget::Settings {
                                    m.settings = None;
                                }
                                cx.notify();
                            });
                            if target == CloseTarget::Settings {
                                v.generation.advance();
                                window.remove_window();
                            }
                        }
                        Err(e) => {
                            v.error = e;
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
        let locked = model.locked() || self.pending;
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
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.model.update(cx, |m, cx| {
                            m.preferences.section = id.clone();
                            cx.notify();
                        });
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
            .child(
                div()
                    .text_color(crate::gui::theme::palette(cx).danger)
                    .child(error),
            )
            .child(t(&notice))
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
