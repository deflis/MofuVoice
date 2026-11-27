use crate::gui::{
    layout,
    model::{self, AppModel},
};
use gpui_kit::component::{checkbox::Checkbox, Disableable};
use gpui_kit::*;
pub struct LogView {
    model: Entity<AppModel>,
    _observe: Subscription,
}
impl LogView {
    pub fn new(model: Entity<AppModel>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            _observe: observe,
        }
    }
}
impl Render for LogView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let mut body = div()
            .id("audio-log")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for value in [
            serde_json::to_value(&m.info).unwrap(),
            serde_json::to_value(&m.log).unwrap(),
        ] {
            append_metrics(&value, &mut body, m);
        }
        layout::surface(cx)
            .child(div().text_lg().child(m.text("ログ")))
            .child(
                Checkbox::new("processing-log")
                    .label(m.text("処理ログを表示"))
                    .checked(m.logging)
                    .disabled(m.closing)
                    .on_click(cx.listener(|view, enabled: &bool, _, cx| {
                        view.model.update(cx, |m, cx| m.set_logging(*enabled, cx));
                    })),
            )
            .child(body.child(m.status.clone()))
    }
}
fn append_metrics(value: &serde_json::Value, body: &mut Stateful<Div>, m: &AppModel) {
    if let serde_json::Value::Object(map) = value {
        for (key, value) in map {
            if value.is_object() {
                append_metrics(value, body, m);
                continue;
            }
            let label = match key.as_str() {
                "phase" => "状態",
                "error" => "エラー",
                "device" => "入力デバイス",
                "backend" => "音声バックエンド",
                "outputDevice" => "出力デバイス",
                "sampleRate" => "サンプルレート",
                "inputs" => "入力チャンネル",
                "outputs" => "出力チャンネル",
                "frames" => "バッファサイズ（サンプル）",
                "periodMs" => "バッファ周期（ms）",
                "dspMs" => "DSP遅延（ms）",
                "inputLatencyMs" => "ドライバ報告の入力遅延（ms）",
                "outputLatencyMs" => "ドライバ報告の出力遅延（ms）",
                "elapsedSeconds" => "経過時間",
                "blocks" => "処理ブロック",
                "nonzeroSamples" => "非ゼロ入力サンプル",
                "maxCallbackUs" => "最大コールバック",
                "deadlineMisses" => "期限超過",
                "timestampBlocks" => "タイムスタンプブロック",
                "samplePositionGaps" => "サンプル位置の欠落",
                "faultFlags" => "エラーフラグ",
                "outputBlocks" => "出力コールバック",
                "underruns" => "アンダーラン",
                "overruns" => "オーバーラン",
                "queueFrames" => "現在のキュー",
                "observedInputFrames" => "観測した最大入力バッファ",
                "observedOutputFrames" => "観測した最大出力バッファ",
                "queuePeakFrames" => "キューの最大量",
                "startupQueueFrames" => "開始時のキュー目標",
                "queueCapacityFrames" => "キュー容量",
                "maxInputCallbackUs" => "最大入力コールバック",
                "maxOutputCallbackUs" => "最大出力コールバック",
                _ => key,
            };
            let text = format_metric_value(key, value, m);
            let row = div()
                .flex()
                .gap_3()
                .child(div().w(px(290.)).child(m.text(label)))
                .child(text);
            body.extend([row.into_any_element()]);
        }
    }
}

fn format_metric_value(key: &str, value: &serde_json::Value, m: &AppModel) -> String {
    if value.is_null() {
        return "—".into();
    }
    match key {
        "phase" => value
            .as_str()
            .map(|phase| m.text(model::phase_label(phase)))
            .unwrap_or_else(|| value.to_string()),
        "elapsedSeconds" => value
            .as_f64()
            .map(|seconds| format!("{seconds:.1} s"))
            .unwrap_or_else(|| value.to_string()),
        "maxCallbackUs" | "maxInputCallbackUs" | "maxOutputCallbackUs" => value
            .as_f64()
            .map(|us| format!("{us:.2} µs"))
            .unwrap_or_else(|| value.to_string()),
        "periodMs" | "dspMs" | "inputLatencyMs" | "outputLatencyMs" => value
            .as_f64()
            .map(|ms| format!("{ms:.2}"))
            .unwrap_or_else(|| value.to_string()),
        "faultFlags" => value
            .as_u64()
            .map(|flags| format!("0x{flags:x}"))
            .unwrap_or_else(|| value.to_string()),
        "queueFrames"
        | "observedInputFrames"
        | "observedOutputFrames"
        | "queuePeakFrames"
        | "startupQueueFrames"
        | "queueCapacityFrames" => format!("{value} frames"),
        _ => match value {
            serde_json::Value::String(s) => s.clone(),
            _ => value.to_string(),
        },
    }
}

pub fn window_options() -> WindowOptions {
    layout::window_options("ログ / Log", 760., 580., 600., 360.)
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::AppModel;
    use crate::gui::preferences::Preferences;
    use gpui_kit::{test::TestWindowExt, AppContext, TestAppContext};

    #[gpui_kit::test]
    fn opening_log_leaves_telemetry_off_until_enabled(cx: &mut TestAppContext) {
        let model = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::gui::theme::init(cx);
            cx.new(|cx| {
                AppModel::new(
                    Default::default(),
                    Default::default(),
                    Preferences::system(),
                    cx,
                )
            })
        });
        model.update(cx, |m, cx| m.open_log(cx));
        cx.run_until_parked();
        let window = model.read_with(cx, |m, _| {
            assert!(!m.logging);
            assert!(m.session.lock().unwrap().log_info().is_none());
            m.log_window.unwrap()
        });
        for enabled in [true, false] {
            window
                .update(cx, |_, w, cx| w.click("processing-log", cx))
                .unwrap();
            cx.run_until_parked();
            model.read_with(cx, |m, _| {
                assert_eq!(m.logging, enabled);
                assert_eq!(m.session.lock().unwrap().log_info().is_some(), enabled);
            });
        }
        window.update(cx, |_, w, _| w.remove_window()).unwrap();
    }

    #[gpui_kit::test]
    fn metric_values_format_numbers_flags_units_and_phases(cx: &mut TestAppContext) {
        let model = cx.update(|cx| {
            cx.new(|cx| {
                AppModel::new(
                    Default::default(),
                    Default::default(),
                    Preferences {
                        language: "ja".into(),
                        section: "voice".into(),
                    },
                    cx,
                )
            })
        });
        model.read_with(cx, |m, _| {
            assert_eq!(
                super::format_metric_value("phase", &serde_json::json!("running"), m),
                "動作中"
            );
            assert_eq!(
                super::format_metric_value("elapsedSeconds", &serde_json::json!(12.345), m),
                "12.3 s"
            );
            assert_eq!(
                super::format_metric_value("maxCallbackUs", &serde_json::json!(123.456), m),
                "123.46 µs"
            );
            assert_eq!(
                super::format_metric_value("periodMs", &serde_json::json!(10.666666), m),
                "10.67"
            );
            assert_eq!(
                super::format_metric_value("faultFlags", &serde_json::json!(26), m),
                "0x1a"
            );
            assert_eq!(
                super::format_metric_value("queueFrames", &serde_json::json!(512), m),
                "512 frames"
            );
            assert_eq!(
                super::format_metric_value("error", &serde_json::Value::Null, m),
                "—"
            );
        });
    }
}
