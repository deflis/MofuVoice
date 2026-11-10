use crate::gui::{layout, model::AppModel};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        Disableable,
    },
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
        let locked = m.locked();
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
                            .disabled(locked)
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
                            .disabled(locked)
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
                                v.model
                                    .update(cx, |m, cx| m.open_settings(Some("devices".into()), cx))
                            })),
                    ),
            )
            .child(div().child(format!(
                "{} · {} Hz · {} frames",
                t(if m.closing {
                    "終了中"
                } else {
                    match m.info.phase {
                        "opening" => "開始中",
                        "running" => "動作中",
                        "stopped" => "停止済み",
                        "inspected" => "検査完了",
                        "error" => "エラー",
                        _ => "準備完了",
                    }
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
