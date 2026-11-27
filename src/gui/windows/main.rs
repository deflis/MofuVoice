use crate::gui::{layout, model::AppModel};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        Disableable,
    },
    prelude::FluentBuilder,
    *,
};
pub struct MainView {
    model: Entity<AppModel>,
    _observe: Subscription,
    _appearance: Subscription,
}
impl MainView {
    pub fn new(model: Entity<AppModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let appearance = crate::gui::theme::observe(window, cx);
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            _observe: observe,
            _appearance: appearance,
        }
    }
}
impl Render for MainView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let t = |s: &str| m.text(s);
        let can_start = m.can_start();
        let device_guidance = m
            .device_warning
            .map(|warning| warning.message())
            .or_else(|| {
                m.needs_input_device()
                    .then_some("最初に、入力・出力デバイスを設定してください。")
            });
        layout::surface(cx)
            .id("main-content")
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_xl().child(t("もふボイス")))
                    .child(
                        Button::new("language")
                            .label(if m.preferences.language == "ja" {
                                "English"
                            } else {
                                "日本語"
                            })
                            .disabled(m.busy || m.closing)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.model.update(cx, |m, cx| {
                                    m.preferences.language = if m.preferences.language == "ja" {
                                        "en"
                                    } else {
                                        "ja"
                                    }
                                    .into();
                                    m.quit_save_failed = false;
                                    cx.notify();
                                })
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        Button::new("start-audio")
                            .primary()
                            .label(t("音声を開始"))
                            .disabled(!can_start)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.model.update(cx, |m, cx| m.start(false, cx))
                            })),
                    )
                    .child(
                        Button::new("stop-audio")
                            .label(t("音声を停止"))
                            .disabled(!m.active || m.closing)
                            .on_click(
                                cx.listener(|v, _, _, cx| v.model.update(cx, |m, cx| m.stop(cx))),
                            ),
                    )
                    .child(
                        Button::new("inspect-audio")
                            .label(t("デバイスを検査"))
                            .disabled(!can_start)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.model.update(cx, |m, cx| m.start(true, cx))
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("voice-settings")
                            .label(t("声を調整する"))
                            .disabled(m.closing)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.model
                                    .update(cx, |m, cx| m.open_settings(Some("voice".into()), cx))
                            })),
                    )
                    .child(
                        Button::new("log")
                            .label(t("ログ"))
                            .disabled(m.closing)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.model.update(cx, |m, cx| m.open_log(cx))
                            })),
                    )
                    .child(
                        Button::new("spectrum")
                            .label(t("スペアナ"))
                            .disabled(m.closing)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.model.update(cx, |m, cx| m.open_spectrum(cx))
                            })),
                    )
                    .child(
                        Button::new("settings")
                            .label(t("設定"))
                            .disabled(m.closing)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.model.update(cx, |m, cx| m.open_settings(None, cx))
                            })),
                    ),
            )
            .when_some(device_guidance, |body, message| {
                body.child(t(message)).child(
                    Button::new("device-settings")
                        .label(t("入力・出力デバイスを確認する"))
                        .disabled(m.closing)
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.model
                                .update(cx, |m, cx| m.open_settings(Some("devices".into()), cx))
                        })),
                )
            })
            .child(div().child(format!(
                "{} · {} Hz · {} frames",
                t(if m.closing {
                    "終了中"
                } else {
                    crate::gui::model::phase_label(m.info.phase)
                }),
                m.saved.sample_rate,
                m.saved.frames
            )))
            .child(
                div()
                    .text_color(crate::gui::theme::palette(cx).danger)
                    .child(if m.error.is_empty() {
                        m.info.error.clone().unwrap_or_default()
                    } else {
                        m.error.clone()
                    }),
            )
    }
}

pub fn window_options() -> WindowOptions {
    layout::window_options("もふボイス", 560., 310., 420., 240.)
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::{window_options, AppModel, MainView};
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    fn settings_button_restores_section_and_shortcuts_open_their_pages(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;

        let (main, model) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::gui::theme::init(cx);
            let model = cx.new(|cx| {
                AppModel::new(
                    crate::cli::Options {
                        backend: crate::backend::BackendKind::Asio,
                        ..Default::default()
                    },
                    std::path::PathBuf::new(),
                    crate::gui::preferences::Preferences::from_saved(
                        crate::gui_settings::UiPreferences {
                            language: Some("en".into()),
                            section: Some("inputEq".into()),
                        },
                    ),
                    cx,
                )
            });
            model.update(cx, |m, _| m.busy = true);
            let (main, _) = gpui_kit::open_window(window_options(), cx, |window, cx| {
                cx.new(|cx| MainView::new(model.clone(), window, cx))
            })
            .unwrap();
            (main, model)
        });
        main.update(cx, |_, window, cx| window.click("settings", cx))
            .unwrap();
        cx.run_until_parked();
        let settings = model.read_with(cx, |m, _| {
            assert_eq!(m.preferences.section, "inputEq");
            m.settings.as_ref().unwrap().0
        });

        main.update(cx, |_, window, cx| window.click("voice-settings", cx))
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            model.read_with(cx, |m, _| m.preferences.section.clone()),
            "voice"
        );

        main.update(cx, |_, window, cx| window.click("device-settings", cx))
            .unwrap();
        cx.run_until_parked();
        model.update(cx, |m, _| {
            assert_eq!(m.preferences.section, "devices");
            assert_eq!(m.settings.take().unwrap().0, settings);
        });
        settings
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        main.update(cx, |_, window, _| window.remove_window())
            .unwrap();
    }
}
