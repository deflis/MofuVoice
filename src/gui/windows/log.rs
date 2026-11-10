use crate::gui::{layout, model::AppModel};
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
            let mut text = match value {
                serde_json::Value::Null => "—".into(),
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            if !value.is_null() {
                match key.as_str() {
                    "elapsedSeconds" => text.push_str(" s"),
                    "maxCallbackUs" | "maxInputCallbackUs" | "maxOutputCallbackUs" => {
                        text.push_str(" µs")
                    }
                    _ => {}
                }
            }
            let row = div()
                .flex()
                .gap_3()
                .child(div().w(px(290.)).child(m.text(label)))
                .child(text);
            body.extend([row.into_any_element()]);
        }
    }
}

pub fn window_options() -> WindowOptions {
    layout::window_options("ログ / Log", 760., 580., 600., 360.)
}
