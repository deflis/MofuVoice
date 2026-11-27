use crate::{
    backend::AudioTelemetry,
    gui::{
        layout,
        model::{self, AppModel},
    },
    session::{AudioInfo, AudioLog},
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
        for (label, value) in info_rows(&m.info, m) {
            append_row(&mut body, label, value, m);
        }
        if let Some(note) = latency_note(m.info.backend.as_deref()) {
            body.extend([div()
                .text_color(crate::gui::theme::palette(cx).muted)
                .child(m.text(note))
                .into_any_element()]);
        }
        if let Some(log) = &m.log {
            for (label, value) in log_rows(log) {
                append_row(&mut body, label, value, m);
            }
        }
        layout::surface(cx)
            .child(div().text_lg().child(m.text("ログ")))
            .child(
                Checkbox::new("processing-log")
                    .label(m.text("処理ログを表示"))
                    .checked(m.logging)
                    .disabled(m.shutting_down())
                    .on_click(cx.listener(|view, enabled: &bool, _, cx| {
                        view.model.update(cx, |m, cx| m.set_logging(*enabled, cx));
                    })),
            )
            .child(body.child(m.status.clone()))
    }
}

fn latency_note(backend: Option<&str>) -> Option<&'static str> {
    match backend {
        Some("cpal") => Some("CPALではドライバー遅延を取得できません。往復遅延は未測定です。"),
        Some(_) => Some("ドライバー報告値です。実測の往復遅延とは異なります。"),
        None => None,
    }
}

fn append_row(body: &mut Stateful<Div>, label: &str, value: String, m: &AppModel) {
    let row = div()
        .flex()
        .gap_3()
        .child(div().w(px(290.)).child(m.text(label)))
        .child(value);
    body.extend([row.into_any_element()]);
}

fn dash() -> String {
    "—".into()
}

fn opt_display<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(dash, |v| v.to_string())
}

fn opt_ms(value: Option<f64>) -> String {
    value.map_or_else(dash, |ms| format!("{ms:.2}"))
}

fn opt_us(value: Option<f64>) -> String {
    value.map_or_else(dash, |us| format!("{us:.2} µs"))
}

fn opt_frames(value: Option<usize>) -> String {
    value.map_or_else(dash, |frames| format!("{frames} frames"))
}

fn info_rows(info: &AudioInfo, m: &AppModel) -> Vec<(&'static str, String)> {
    let output_device = match info.backend.as_deref() {
        Some("asio") => info.device.clone().unwrap_or_else(dash),
        Some("cpal") => match info.output_device.as_deref() {
            Some("") => m.text("システムの既定"),
            Some(name) => name.into(),
            None => dash(),
        },
        _ => info
            .output_device
            .as_deref()
            .filter(|name| !name.is_empty())
            .map(String::from)
            .unwrap_or_else(dash),
    };
    vec![
        ("状態", m.text(model::phase_label(info.phase))),
        ("エラー", info.error.clone().unwrap_or_else(dash)),
        ("入力デバイス", info.device.clone().unwrap_or_else(dash)),
        (
            "音声バックエンド",
            info.backend.clone().unwrap_or_else(dash),
        ),
        ("出力デバイス", output_device),
        ("サンプルレート", opt_display(info.sample_rate)),
        ("入力チャンネル", opt_display(info.inputs)),
        ("出力チャンネル", opt_display(info.outputs)),
        ("バッファサイズ（サンプル）", opt_display(info.frames)),
        ("バッファ周期（ms）", opt_ms(info.period_ms)),
        ("DSP遅延（ms）", opt_ms(info.dsp_ms)),
        (
            "ドライバ報告の入力遅延（ms）",
            opt_ms(info.input_latency_ms),
        ),
        (
            "ドライバ報告の出力遅延（ms）",
            opt_ms(info.output_latency_ms),
        ),
    ]
}

fn log_rows(log: &AudioLog) -> Vec<(&'static str, String)> {
    let mut rows = vec![(
        "経過時間",
        log.elapsed_seconds
            .map_or_else(dash, |seconds| format!("{seconds:.1} s")),
    )];
    if let Some(t) = &log.telemetry {
        rows.extend(telemetry_rows(t));
    }
    rows
}

fn telemetry_rows(t: &AudioTelemetry) -> Vec<(&'static str, String)> {
    vec![
        ("処理ブロック", t.blocks.to_string()),
        ("非ゼロ入力サンプル", t.nonzero_samples.to_string()),
        ("最大コールバック", format!("{:.2} µs", t.max_callback_us)),
        ("期限超過", t.deadline_misses.to_string()),
        ("タイムスタンプブロック", opt_display(t.timestamp_blocks)),
        ("サンプル位置の欠落", opt_display(t.sample_position_gaps)),
        ("エラーフラグ", format!("0x{:x}", t.fault_flags)),
        ("出力コールバック", opt_display(t.output_blocks)),
        ("アンダーラン", opt_display(t.underruns)),
        ("オーバーラン", opt_display(t.overruns)),
        ("現在のキュー", opt_frames(t.queue_frames)),
        (
            "観測した最大入力バッファ",
            opt_frames(t.observed_input_frames),
        ),
        (
            "観測した最大出力バッファ",
            opt_frames(t.observed_output_frames),
        ),
        ("キューの最大量", opt_frames(t.queue_peak_frames)),
        ("開始時のキュー目標", opt_frames(t.startup_queue_frames)),
        ("キュー容量", opt_frames(t.queue_capacity_frames)),
        ("最大入力コールバック", opt_us(t.max_input_callback_us)),
        ("最大出力コールバック", opt_us(t.max_output_callback_us)),
    ]
}

pub fn window_options() -> WindowOptions {
    layout::window_options("ログ / Log", 760., 580., 600., 360.)
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::{info_rows, latency_note, log_rows, AppModel};
    use crate::{
        backend::AudioTelemetry,
        gui::preferences::{Language, Preferences, Section},
        session::{AudioInfo, AudioLog},
    };
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
    fn metric_rows_format_numbers_flags_units_and_phases(cx: &mut TestAppContext) {
        let model = cx.update(|cx| {
            cx.new(|cx| {
                AppModel::new(
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    Preferences {
                        language: Language::Ja,
                        section: Section::Voice,
                    },
                    cx,
                )
            })
        });
        let info = AudioInfo {
            phase: "running",
            backend: Some("cpal".into()),
            output_device: Some(String::new()),
            period_ms: Some(10.666666),
            ..AudioInfo::default()
        };
        let log = AudioLog {
            elapsed_seconds: Some(12.345),
            telemetry: Some(AudioTelemetry {
                max_callback_us: 123.456,
                fault_flags: 26,
                queue_frames: Some(512),
                ..AudioTelemetry::default()
            }),
        };
        model.read_with(cx, |m, _| {
            let info_map: std::collections::HashMap<_, _> =
                info_rows(&info, m).into_iter().collect();
            assert_eq!(info_map["状態"], "動作中");
            assert_eq!(info_map["エラー"], "—");
            assert_eq!(info_map["出力デバイス"], "システムの既定");
            assert_eq!(info_map["バッファ周期（ms）"], "10.67");

            let log_map: std::collections::HashMap<_, _> = log_rows(&log).into_iter().collect();
            assert_eq!(log_map["経過時間"], "12.3 s");
            assert_eq!(log_map["最大コールバック"], "123.46 µs");
            assert_eq!(log_map["エラーフラグ"], "0x1a");
            assert_eq!(log_map["現在のキュー"], "512 frames");
        });
        assert_eq!(
            latency_note(Some("cpal")),
            Some("CPALではドライバー遅延を取得できません。往復遅延は未測定です。")
        );
        assert_eq!(
            latency_note(Some("asio")),
            Some("ドライバー報告値です。実測の往復遅延とは異なります。")
        );
        assert_eq!(latency_note(None), None);
    }
}
