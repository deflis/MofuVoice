use crate::backend;
use crate::gui::preferences::Preferences;
use gpui_kit::{component::searchable_list::SearchableListItem, SharedString};

pub(super) fn help(path: &str) -> Option<&'static str> {
    match path {
        "/deesser/frequencyHz"=>Some("この周波数より高い音を中心に抑えます。"),
        "/deesser/thresholdDb"=>Some("低くするほど、小さいサ行にも反応します。"),
        "/deesser/rangeDb"=>Some("大きくするほど強く抑えます。0で補正なし。"),
        "/deesser/attackMs"=>Some("小さくするほど素早く抑え始めます。"),
        "/deesser/releaseMs"=>Some("大きくするほど、ゆっくり元の音に戻ります。"),
        "/noise/thresholdDb"=>Some("0 dBFSに近づけるほど、より大きな声を出さないと音が出なくなります（雑音を強力に遮断します）。"),
        "/devices/frames"=>Some("小さくすると遅延が減ります。音切れする場合は大きくしてください。デバイスが対応するサイズを指定してください。"),
        _=>None,
    }
}

#[derive(Clone)]
pub(super) struct Choice {
    pub value: String,
    pub label: SharedString,
}
impl SearchableListItem for Choice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &String {
        &self.value
    }
}
pub(super) fn labeled_choices(path: &str, items: Vec<String>, prefs: &Preferences) -> Vec<Choice> {
    items
        .into_iter()
        .map(|value| {
            let key = match value.as_str() {
                "" if path.ends_with("previewDevice") => "なし",
                "" => "システムの既定",
                "auto" => "自動",
                "manual" => "手動",
                "cpal" => "標準",
                "asio" => "ASIO",
                "semitones" => "半音",
                "percent" => "パーセント",
                "standard" => "標準（高速近似）",
                "accurate" => "精度優先",
                "phase-locked" => "位相ロック（標準）",
                "pvsola-sync" => "スペクトル位相同期",
                "pvsola" => "PVSOLA",
                "td-psola" => "TD-PSOLA＋フェーズボコーダー",
                "td-psola-resampled" => "TD-PSOLA＋リサンプリング",
                "fd-psola" => "FD-PSOLA",
                "yin" => "YIN（標準）",
                "mpm" => "MPM",
                "mpm-fft" => "MPM＋FFT",
                "nccf" => "NCCF",
                _ => &value,
            };
            Choice {
                label: prefs.text(key).into(),
                value,
            }
        })
        .collect()
}
pub(super) fn choices(path: &str, current: &str) -> Option<Vec<String>> {
    let key = path.rsplit('/').next()?;
    let values: Vec<String> = match key {
        "backend" => [
            backend::BackendKind::Auto,
            backend::BackendKind::Cpal,
            backend::BackendKind::Asio,
        ]
        .into_iter()
        .filter(|k| k.available())
        .map(|k| k.name().into())
        .collect(),
        "device" | "outputDevice" | "previewDevice" => vec![String::new(), current.into()],
        "pitchAlgorithm" => crate::pitch::PitchAlgorithm::ALL
            .iter()
            .map(|a| a.name().into())
            .collect(),
        "psolaPitchEstimator" => crate::pitch::PitchEstimator::ALL
            .iter()
            .map(|a| a.name().into())
            .collect(),
        "pitchUnit" => vec!["semitones".into(), "percent".into()],
        "mathPrecision" => vec!["standard".into(), "accurate".into()],
        "mode" => vec!["simple".into(), "pro".into()],
        "thresholdMode" => vec!["auto".into(), "manual".into()],
        "sampleRate" => vec!["44100".into(), "48000".into()],
        "analysisWindow" | "psolaAnalysisWindow" => [1024, 2048, 4096, 8192]
            .into_iter()
            .map(|n| n.to_string())
            .collect(),
        "synthesisWindow" | "psolaSynthesisWindow" => [256, 512, 1024, 2048]
            .into_iter()
            .map(|n| n.to_string())
            .collect(),
        _ => return None,
    };
    Some(values)
}
pub(super) fn field_label(path: &str) -> String {
    if let Some(label) = match path {
        "/deesser/frequencyHz" => Some("分割周波数（Hz）"),
        "/deesser/thresholdDb" | "/noise/thresholdDb" | "/compressor/thresholdDb" => {
            Some("しきい値（dBFS）")
        }
        "/deesser/rangeDb" => Some("最大減衰量（dB）"),
        "/voice/preserveFormants" => Some("フォルマントを独立して調整する"),
        "/level/targetDb" => Some("目標音量（dBFS・RMS）"),
        "/level/maxGainDb" => Some("最大増幅量（dB）"),
        "/level/floorDb" => Some("音声検出しきい値（dBFS）"),
        "/level/responseMs" => Some("音量調整の応答時間（ms）"),
        "/level/thresholdDb" => Some("圧縮しきい値（dBFS）"),
        "/level/ratio" | "/compressor/ratio" => Some("圧縮比"),
        "/reverb/decayMs" => Some("減衰時間（ms・公称RT60）"),
        _ => None,
    } {
        return label.into();
    }
    let key = path.rsplit('/').next().unwrap_or_default();
    let label = match key {
        "enabled" => "有効",
        "compressorEnabled" => "コンプレッサー",
        "backend" => "音声バックエンド",
        "device" => "入力デバイス",
        "outputDevice" => "出力デバイス",
        "previewDevice" => "モニター出力デバイス",
        "inputChannel" => "入力チャンネル",
        "outputChannels" => "出力チャンネル（カンマ区切り）",
        "frames" => "バッファサイズ（サンプル）",
        "sampleRate" => "サンプルレート",
        "pitchUnit" => "ピッチの表示単位",
        "mathPrecision" => "ボコーダーの計算精度",
        "psolaPitchEstimator" => "PSOLAのピッチ推定",
        "psolaAnalysisWindow" => "PSOLA解析窓",
        "psolaSynthesisWindow" => "PSOLA合成窓",
        "analysisWindow" => "ボコーダー解析窓",
        "synthesisWindow" => "ボコーダー合成窓",
        "pitchAlgorithm" => "ピッチ変換方式",
        "pitch" => "声の高さ（半音）",
        "formantShift" => "フォルマント（半音）",
        "preserveFormants" => "フォルマントを保持",
        "rumbleFilter" => "低域ノイズ除去（80 Hz）",
        "thresholdMode" => "しきい値の設定",
        "thresholdDb" => "しきい値（dB）",
        "attackMs" => "アタック（ms）",
        "releaseMs" => "リリース（ms）",
        "frequencyHz" | "frequency" => "周波数（Hz）",
        "rangeDb" => "最大抑制量（dB）",
        "gainDb" => "ゲイン（dB）",
        "q" => "Q",
        "mode" => "モード",
        "outputDb" => "出力トリム（dB）",
        "targetDb" => "目標音量（dB RMS）",
        "maxGainDb" => "最大ゲイン（dB）",
        "floorDb" => "動作下限（dB）",
        "responseMs" => "応答時間（ms）",
        "ratio" => "比率",
        "ceilingDb" => "上限（dBFS）",
        "amount" => "低減量",
        "decayMs" => "減衰時間（ms）",
        "mix" => "ミックス",
        "preDelayMs" => "プリディレイ（ms）",
        _ => key,
    };
    if let Some(index) = path.split('/').nth(3).and_then(|s| s.parse::<usize>().ok()) {
        format!("{} · {label}", index + 1)
    } else {
        label.into()
    }
}
