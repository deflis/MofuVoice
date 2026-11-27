//! Typed settings draft for the editor window.
//! Only numeric text inputs retain raw unparsed strings while the user types;
//! toggles, enums, selections, and channel lists stay strongly typed.
use super::preferences::{Preferences, Section};
use crate::{
    audio::FilterStage,
    backend,
    cli::Options,
    eq::{Band, EqSettings},
    gate::GateThreshold,
    pitch::{MathPrecision, PitchAlgorithm, PitchEstimator, PitchSettings, SpectralWindows},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EqKind {
    Input,
    PreConversion,
    PostConversion,
}

impl EqKind {
    pub const ALL: [Self; 3] = [Self::Input, Self::PreConversion, Self::PostConversion];

    pub fn id(self) -> &'static str {
        match self {
            Self::Input => "inputEq",
            Self::PreConversion => "preConversionEq",
            Self::PostConversion => "eq",
        }
    }

    pub fn section(self) -> Section {
        match self {
            Self::Input => Section::InputEq,
            Self::PreConversion => Section::PreConversionEq,
            Self::PostConversion => Section::PostConversionEq,
        }
    }

    pub fn from_section(section: Section) -> Option<Self> {
        match section {
            Section::InputEq => Some(Self::Input),
            Section::PreConversionEq => Some(Self::PreConversion),
            Section::PostConversionEq => Some(Self::PostConversion),
            _ => None,
        }
    }

    pub fn mode(self, options: &Options) -> UiMode {
        let raw = match self {
            Self::Input => options.input_eq_mode.as_str(),
            Self::PreConversion => options.pre_conversion_eq_mode.as_str(),
            Self::PostConversion => options.eq_mode.as_str(),
        };
        UiMode::parse(raw).expect("validated EQ mode")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EqBandField {
    Frequency,
    GainDb,
    Q,
}

impl EqBandField {
    pub const ALL: [Self; 3] = [Self::Frequency, Self::GainDb, Self::Q];

    pub fn id(self) -> &'static str {
        match self {
            Self::Frequency => "frequency",
            Self::GainDb => "gainDb",
            Self::Q => "q",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Frequency => "周波数（Hz）",
            Self::GainDb => "ゲイン（dB）",
            Self::Q => "Q",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitchUnit {
    Semitones,
    Percent,
}

impl PitchUnit {
    pub const ALL: [Self; 2] = [Self::Semitones, Self::Percent];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Semitones => "semitones",
            Self::Percent => "percent",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "semitones" => Ok(Self::Semitones),
            "percent" => Ok(Self::Percent),
            _ => Err("Pitch unit must be semitones or percent".into()),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Semitones => "半音",
            Self::Percent => "パーセント",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiMode {
    Simple,
    Pro,
}

impl UiMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Pro => "pro",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "simple" => Ok(Self::Simple),
            "pro" => Ok(Self::Pro),
            _ => Err("Mode must be simple or pro".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateThresholdMode {
    Auto,
    Manual,
}

impl GateThresholdMode {
    pub const ALL: [Self; 2] = [Self::Auto, Self::Manual];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "自動",
            Self::Manual => "手動",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceSlot {
    Input,
    Output,
    Preview,
}

impl DeviceSlot {
    pub const ALL: [Self; 3] = [Self::Input, Self::Output, Self::Preview];

    pub fn id(self) -> &'static str {
        match self {
            Self::Input => "/devices/device",
            Self::Output => "/devices/outputDevice",
            Self::Preview => "/devices/previewDevice",
        }
    }

    pub fn label(self, resolved_backend: backend::BackendKind) -> &'static str {
        match self {
            Self::Input if resolved_backend == backend::BackendKind::Asio => "入出力デバイス",
            Self::Input => "入力デバイス",
            Self::Output => "出力デバイス",
            Self::Preview => "モニター出力デバイス",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetTarget {
    Voice,
    VoiceWindows,
    PsolaWindows,
    Deesser,
    AutoLevel,
    PostCompressor,
    Eq(EqKind),
}

impl ResetTarget {
    pub fn id(self) -> &'static str {
        match self {
            Self::Voice => "voice",
            Self::VoiceWindows => "voice-windows",
            Self::PsolaWindows => "psola-windows",
            Self::Deesser => "deesser",
            Self::AutoLevel => "autoLevel",
            Self::PostCompressor => "postCompressor",
            Self::Eq(EqKind::Input) => "inputEq",
            Self::Eq(EqKind::PreConversion) => "preConversionEq",
            Self::Eq(EqKind::PostConversion) => "postConversionEq",
        }
    }

    pub fn section(self) -> Section {
        match self {
            Self::Voice | Self::VoiceWindows | Self::PsolaWindows => Section::Voice,
            Self::Deesser => Section::Deesser,
            Self::AutoLevel => Section::AutoLevel,
            Self::PostCompressor => Section::PostCompressor,
            Self::Eq(kind) => kind.section(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToggleField {
    VoiceEnabled,
    PreserveFormants,
    RumbleFilter,
    NoiseGateEnabled,
    DeesserEnabled,
    CompressorEnabled,
    LimiterEnabled,
    DereverbEnabled,
    ReverbEnabled,
    LevelEnabled,
    PostCompressorEnabled,
    EqEnabled(EqKind),
}

impl ToggleField {
    pub fn id(self) -> &'static str {
        match self {
            Self::VoiceEnabled => "/voice/enabled",
            Self::PreserveFormants => "/voice/preserveFormants",
            Self::RumbleFilter => "/noise/rumbleFilter",
            Self::NoiseGateEnabled => "/noise/enabled",
            Self::DeesserEnabled => "/deesser/enabled",
            Self::CompressorEnabled => "/compressor/enabled",
            Self::LimiterEnabled => "/limiter/enabled",
            Self::DereverbEnabled => "/dereverb/enabled",
            Self::ReverbEnabled => "/reverb/enabled",
            Self::LevelEnabled => "/level/enabled",
            Self::PostCompressorEnabled => "/level/compressorEnabled",
            Self::EqEnabled(EqKind::Input) => "/inputEq/enabled",
            Self::EqEnabled(EqKind::PreConversion) => "/preConversionEq/enabled",
            Self::EqEnabled(EqKind::PostConversion) => "/eq/enabled",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PreserveFormants => "フォルマントを独立して調整する",
            Self::RumbleFilter => "低域ノイズ除去（80 Hz）",
            Self::PostCompressorEnabled => "コンプレッサー",
            Self::VoiceEnabled
            | Self::NoiseGateEnabled
            | Self::DeesserEnabled
            | Self::CompressorEnabled
            | Self::LimiterEnabled
            | Self::DereverbEnabled
            | Self::ReverbEnabled
            | Self::LevelEnabled
            | Self::EqEnabled(_) => "有効",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectField {
    Backend,
    Device(DeviceSlot),
    SampleRate,
    PitchUnit,
    MathPrecision,
    PitchAlgorithm,
    PsolaPitchEstimator,
    AnalysisWindow,
    SynthesisWindow,
    PsolaAnalysisWindow,
    PsolaSynthesisWindow,
    GateThresholdMode,
}

impl SelectField {
    pub fn id(self) -> &'static str {
        match self {
            Self::Backend => "/devices/backend",
            Self::Device(slot) => slot.id(),
            Self::SampleRate => "/devices/sampleRate",
            Self::PitchUnit => "/voice/pitchUnit",
            Self::MathPrecision => "/voice/mathPrecision",
            Self::PitchAlgorithm => "/voice/pitchAlgorithm",
            Self::PsolaPitchEstimator => "/voice/psolaPitchEstimator",
            Self::AnalysisWindow => "/voice/analysisWindow",
            Self::SynthesisWindow => "/voice/synthesisWindow",
            Self::PsolaAnalysisWindow => "/voice/psolaAnalysisWindow",
            Self::PsolaSynthesisWindow => "/voice/psolaSynthesisWindow",
            Self::GateThresholdMode => "/noise/thresholdMode",
        }
    }

    pub fn label(self, resolved_backend: backend::BackendKind) -> &'static str {
        match self {
            Self::Backend => "音声バックエンド",
            Self::Device(slot) => slot.label(resolved_backend),
            Self::SampleRate => "サンプルレート",
            Self::PitchUnit => "ピッチの表示単位",
            Self::MathPrecision => "ボコーダーの計算精度",
            Self::PitchAlgorithm => "ピッチ変換方式",
            Self::PsolaPitchEstimator => "PSOLAのピッチ推定",
            Self::AnalysisWindow => "ボコーダー解析窓",
            Self::SynthesisWindow => "ボコーダー合成窓",
            Self::PsolaAnalysisWindow => "PSOLA解析窓",
            Self::PsolaSynthesisWindow => "PSOLA合成窓",
            Self::GateThresholdMode => "しきい値の設定",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NumberField {
    Frames,
    Pitch,
    FormantShift,
    GateThresholdDb,
    GateAttackMs,
    GateReleaseMs,
    DeesserFrequencyHz,
    DeesserThresholdDb,
    DeesserRangeDb,
    DeesserAttackMs,
    DeesserReleaseMs,
    CompressorThresholdDb,
    CompressorRatio,
    CompressorAttackMs,
    CompressorReleaseMs,
    LimiterCeilingDb,
    LimiterReleaseMs,
    DereverbAmount,
    DereverbDecayMs,
    ReverbMix,
    ReverbPreDelayMs,
    ReverbDecayMs,
    LevelTargetDb,
    LevelMaxGainDb,
    LevelFloorDb,
    LevelResponseMs,
    PostCompressorThresholdDb,
    PostCompressorRatio,
    PostCompressorAttackMs,
    PostCompressorReleaseMs,
    EqBand(EqKind, usize, EqBandField),
    EqOutputDb(EqKind),
}

impl NumberField {
    pub const FIXED: [Self; 33] = [
        Self::Frames,
        Self::Pitch,
        Self::FormantShift,
        Self::GateThresholdDb,
        Self::GateAttackMs,
        Self::GateReleaseMs,
        Self::DeesserFrequencyHz,
        Self::DeesserThresholdDb,
        Self::DeesserRangeDb,
        Self::DeesserAttackMs,
        Self::DeesserReleaseMs,
        Self::CompressorThresholdDb,
        Self::CompressorRatio,
        Self::CompressorAttackMs,
        Self::CompressorReleaseMs,
        Self::LimiterCeilingDb,
        Self::LimiterReleaseMs,
        Self::DereverbAmount,
        Self::DereverbDecayMs,
        Self::ReverbMix,
        Self::ReverbPreDelayMs,
        Self::ReverbDecayMs,
        Self::LevelTargetDb,
        Self::LevelMaxGainDb,
        Self::LevelFloorDb,
        Self::LevelResponseMs,
        Self::PostCompressorThresholdDb,
        Self::PostCompressorRatio,
        Self::PostCompressorAttackMs,
        Self::PostCompressorReleaseMs,
        Self::EqOutputDb(EqKind::Input),
        Self::EqOutputDb(EqKind::PreConversion),
        Self::EqOutputDb(EqKind::PostConversion),
    ];

    pub fn id(self) -> String {
        match self {
            Self::Frames => "/devices/frames".into(),
            Self::Pitch => "/voice/pitch".into(),
            Self::FormantShift => "/voice/formantShift".into(),
            Self::GateThresholdDb => "/noise/thresholdDb".into(),
            Self::GateAttackMs => "/noise/attackMs".into(),
            Self::GateReleaseMs => "/noise/releaseMs".into(),
            Self::DeesserFrequencyHz => "/deesser/frequencyHz".into(),
            Self::DeesserThresholdDb => "/deesser/thresholdDb".into(),
            Self::DeesserRangeDb => "/deesser/rangeDb".into(),
            Self::DeesserAttackMs => "/deesser/attackMs".into(),
            Self::DeesserReleaseMs => "/deesser/releaseMs".into(),
            Self::CompressorThresholdDb => "/compressor/thresholdDb".into(),
            Self::CompressorRatio => "/compressor/ratio".into(),
            Self::CompressorAttackMs => "/compressor/attackMs".into(),
            Self::CompressorReleaseMs => "/compressor/releaseMs".into(),
            Self::LimiterCeilingDb => "/limiter/ceilingDb".into(),
            Self::LimiterReleaseMs => "/limiter/releaseMs".into(),
            Self::DereverbAmount => "/dereverb/amount".into(),
            Self::DereverbDecayMs => "/dereverb/decayMs".into(),
            Self::ReverbMix => "/reverb/mix".into(),
            Self::ReverbPreDelayMs => "/reverb/preDelayMs".into(),
            Self::ReverbDecayMs => "/reverb/decayMs".into(),
            Self::LevelTargetDb => "/level/targetDb".into(),
            Self::LevelMaxGainDb => "/level/maxGainDb".into(),
            Self::LevelFloorDb => "/level/floorDb".into(),
            Self::LevelResponseMs => "/level/responseMs".into(),
            Self::PostCompressorThresholdDb => "/level/thresholdDb".into(),
            Self::PostCompressorRatio => "/level/ratio".into(),
            Self::PostCompressorAttackMs => "/level/attackMs".into(),
            Self::PostCompressorReleaseMs => "/level/releaseMs".into(),
            Self::EqBand(kind, index, field) => {
                format!("/{}/bands/{index}/{}", kind.id(), field.id())
            }
            Self::EqOutputDb(kind) => format!("/{}/outputDb", kind.id()),
        }
    }

    pub fn label(self, pitch_unit: PitchUnit) -> String {
        match self {
            Self::Frames => "バッファサイズ（サンプル）".into(),
            Self::Pitch => match pitch_unit {
                PitchUnit::Semitones => "声の高さ（半音）".into(),
                PitchUnit::Percent => "声の高さ（%）".into(),
            },
            Self::FormantShift => "フォルマント（半音）".into(),
            Self::GateThresholdDb | Self::DeesserThresholdDb | Self::CompressorThresholdDb => {
                "しきい値（dBFS）".into()
            }
            Self::GateAttackMs
            | Self::DeesserAttackMs
            | Self::CompressorAttackMs
            | Self::PostCompressorAttackMs => "アタック（ms）".into(),
            Self::GateReleaseMs
            | Self::DeesserReleaseMs
            | Self::CompressorReleaseMs
            | Self::LimiterReleaseMs
            | Self::PostCompressorReleaseMs => "リリース（ms）".into(),
            Self::DeesserFrequencyHz => "分割周波数（Hz）".into(),
            Self::DeesserRangeDb => "最大減衰量（dB）".into(),
            Self::CompressorRatio | Self::PostCompressorRatio => "圧縮比".into(),
            Self::LimiterCeilingDb => "上限（dBFS）".into(),
            Self::DereverbAmount => "低減量".into(),
            Self::DereverbDecayMs => "減衰時間（ms）".into(),
            Self::ReverbMix => "ミックス".into(),
            Self::ReverbPreDelayMs => "プリディレイ（ms）".into(),
            Self::ReverbDecayMs => "減衰時間（ms・公称RT60）".into(),
            Self::LevelTargetDb => "目標音量（dBFS・RMS）".into(),
            Self::LevelMaxGainDb => "最大増幅量（dB）".into(),
            Self::LevelFloorDb => "音声検出しきい値（dBFS）".into(),
            Self::LevelResponseMs => "音量調整の応答時間（ms）".into(),
            Self::PostCompressorThresholdDb => "圧縮しきい値（dBFS）".into(),
            Self::EqBand(_, index, field) => format!("{} · {}", index + 1, field.label()),
            Self::EqOutputDb(_) => "出力トリム（dB）".into(),
        }
    }

    pub fn simple_slider_label(self, prefs: &Preferences) -> Option<String> {
        match self {
            Self::DeesserRangeDb => Some("抑える強さ".into()),
            Self::EqBand(_, index, EqBandField::GainDb) => Some(match index {
                0 => "低域の厚み".into(),
                1 => "明瞭さ".into(),
                2 => "明るさ".into(),
                i => format!("{} · {}", i + 1, prefs.text("バンド")),
            }),
            _ => None,
        }
    }

    pub fn help(self) -> Option<&'static str> {
        match self {
            Self::DeesserFrequencyHz => Some("この周波数より高い音を中心に抑えます。"),
            Self::DeesserThresholdDb => Some("低くするほど、小さいサ行にも反応します。"),
            Self::DeesserRangeDb => Some("大きくするほど強く抑えます。0で補正なし。"),
            Self::DeesserAttackMs => Some("小さくするほど素早く抑え始めます。"),
            Self::DeesserReleaseMs => Some("大きくするほど、ゆっくり元の音に戻ります。"),
            Self::GateThresholdDb => Some(
                "0 dBFSに近づけるほど、より大きな声を出さないと音が出なくなります（雑音を強力に遮断します）。",
            ),
            Self::Frames => Some(
                "小さくすると遅延が減ります。音切れする場合は大きくしてください。デバイスが対応するサイズを指定してください。",
            ),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn section(self) -> Section {
        match self {
            Self::Frames => Section::Devices,
            Self::Pitch | Self::FormantShift => Section::Voice,
            Self::GateThresholdDb | Self::GateAttackMs | Self::GateReleaseMs => Section::NoiseGate,
            Self::DeesserFrequencyHz
            | Self::DeesserThresholdDb
            | Self::DeesserRangeDb
            | Self::DeesserAttackMs
            | Self::DeesserReleaseMs => Section::Deesser,
            Self::CompressorThresholdDb
            | Self::CompressorRatio
            | Self::CompressorAttackMs
            | Self::CompressorReleaseMs => Section::Compressor,
            Self::LimiterCeilingDb | Self::LimiterReleaseMs => Section::Limiter,
            Self::DereverbAmount | Self::DereverbDecayMs => Section::Dereverb,
            Self::ReverbMix | Self::ReverbPreDelayMs | Self::ReverbDecayMs => Section::Reverb,
            Self::LevelTargetDb
            | Self::LevelMaxGainDb
            | Self::LevelFloorDb
            | Self::LevelResponseMs => Section::AutoLevel,
            Self::PostCompressorThresholdDb
            | Self::PostCompressorRatio
            | Self::PostCompressorAttackMs
            | Self::PostCompressorReleaseMs => Section::PostCompressor,
            Self::EqBand(kind, _, _) | Self::EqOutputDb(kind) => kind.section(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorField {
    Number(NumberField),
    SampleRate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftError {
    pub field: Option<ErrorField>,
    pub message: String,
}

impl DraftError {
    pub fn plain(message: impl Into<String>) -> Self {
        Self {
            field: None,
            message: message.into(),
        }
    }

    pub fn for_field(field: ErrorField, message: impl Into<String>) -> Self {
        Self {
            field: Some(field),
            message: message.into(),
        }
    }

    pub fn format(&self, prefs: &Preferences, pitch_unit: PitchUnit) -> String {
        let message = prefs.text(&self.message);
        match self.field {
            Some(ErrorField::Number(field)) => {
                format!("{}: {message}", prefs.text(&field.label(pitch_unit)))
            }
            Some(ErrorField::SampleRate) => {
                format!("{}: {message}", prefs.text("サンプルレート"))
            }
            None => message,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EqBandDraft {
    pub frequency: String,
    pub gain_db: String,
    pub q: String,
}

impl EqBandDraft {
    fn from_band(band: Band) -> Self {
        Self {
            frequency: band.frequency.to_string(),
            gain_db: band.gain_db.to_string(),
            q: band.q.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EqDraft {
    pub enabled: bool,
    pub mode: UiMode,
    pub bands: Vec<EqBandDraft>,
    pub output_db: String,
}

impl EqDraft {
    fn from_settings(settings: &EqSettings, mode: UiMode) -> Self {
        Self {
            enabled: settings.enabled,
            mode,
            bands: settings
                .bands
                .iter()
                .copied()
                .map(EqBandDraft::from_band)
                .collect(),
            output_db: settings.output_db.to_string(),
        }
    }

    fn parse(&self, kind: EqKind) -> Result<EqSettings, DraftError> {
        let mut bands = Vec::with_capacity(self.bands.len());
        for (index, band) in self.bands.iter().enumerate() {
            bands.push(Band {
                frequency: parse_f64(
                    &band.frequency,
                    NumberField::EqBand(kind, index, EqBandField::Frequency),
                )?,
                gain_db: parse_f64(
                    &band.gain_db,
                    NumberField::EqBand(kind, index, EqBandField::GainDb),
                )?,
                q: parse_f64(&band.q, NumberField::EqBand(kind, index, EqBandField::Q))?,
            });
        }
        Ok(EqSettings {
            enabled: self.enabled,
            bands,
            output_db: parse_f64(&self.output_db, NumberField::EqOutputDb(kind))?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SettingsDraft {
    pub backend: backend::BackendKind,
    pub device: String,
    pub output_device: String,
    pub preview_device: String,
    pub input_channel: usize,
    pub output_channels: Vec<usize>,
    pub sample_rate: u32,
    pub frames: String,
    pub voice_enabled: bool,
    pub pitch_unit: PitchUnit,
    pub math_precision: MathPrecision,
    pub pitch_algorithm: PitchAlgorithm,
    pub psola_pitch_estimator: PitchEstimator,
    pub analysis_window: usize,
    pub synthesis_window: usize,
    pub psola_analysis_window: usize,
    pub psola_synthesis_window: usize,
    pub pitch: String,
    /// Retains exact semitones when percent mode has not been manually edited,
    /// avoiding powf/log2 ULP drift on unmodified open/close.
    pub pitch_exact_semitones: Option<f64>,
    pub formant_shift: String,
    pub preserve_formants: bool,
    pub rumble_filter: bool,
    pub gate_enabled: bool,
    pub gate_threshold_mode: GateThresholdMode,
    pub gate_threshold_db: String,
    pub gate_attack_ms: String,
    pub gate_release_ms: String,
    pub deesser_enabled: bool,
    pub deesser_mode: UiMode,
    pub deesser_frequency_hz: String,
    pub deesser_threshold_db: String,
    pub deesser_range_db: String,
    pub deesser_attack_ms: String,
    pub deesser_release_ms: String,
    pub input_eq: EqDraft,
    pub pre_conversion_eq: EqDraft,
    pub post_conversion_eq: EqDraft,
    pub compressor_enabled: bool,
    pub compressor_threshold_db: String,
    pub compressor_ratio: String,
    pub compressor_attack_ms: String,
    pub compressor_release_ms: String,
    pub limiter_enabled: bool,
    pub limiter_ceiling_db: String,
    pub limiter_release_ms: String,
    pub dereverb_enabled: bool,
    pub dereverb_amount: String,
    pub dereverb_decay_ms: String,
    pub reverb_enabled: bool,
    pub reverb_mix: String,
    pub reverb_pre_delay_ms: String,
    pub reverb_decay_ms: String,
    pub level_enabled: bool,
    pub post_compressor_enabled: bool,
    pub level_target_db: String,
    pub level_max_gain_db: String,
    pub level_floor_db: String,
    pub level_response_ms: String,
    pub post_compressor_threshold_db: String,
    pub post_compressor_ratio: String,
    pub post_compressor_attack_ms: String,
    pub post_compressor_release_ms: String,
}

impl SettingsDraft {
    pub fn from_options(options: &Options) -> Self {
        let pitch_unit = PitchUnit::parse(&options.pitch_unit).expect("validated pitch unit");
        let (pitch, pitch_exact_semitones) = match pitch_unit {
            PitchUnit::Semitones => (options.pitch.semitones.to_string(), None),
            PitchUnit::Percent => (
                semitones_to_percent(options.pitch.semitones).to_string(),
                Some(options.pitch.semitones),
            ),
        };
        let (gate_threshold_mode, gate_threshold_db) = match options.gate_threshold {
            GateThreshold::Auto => (GateThresholdMode::Auto, (-50.0_f64).to_string()),
            GateThreshold::Db(db) => (GateThresholdMode::Manual, db.to_string()),
        };
        Self {
            backend: options.backend,
            device: options.device.clone(),
            output_device: options.output_device.clone(),
            preview_device: options.preview_device.clone(),
            input_channel: options.input_channel,
            output_channels: options.output_channels.clone(),
            sample_rate: options.sample_rate,
            frames: options.frames.to_string(),
            voice_enabled: options.voice_enabled,
            pitch_unit,
            math_precision: options.pitch.math_precision,
            pitch_algorithm: options.pitch.algorithm,
            psola_pitch_estimator: options.pitch.psola_pitch_estimator,
            analysis_window: options.pitch.windows.analysis,
            synthesis_window: options.pitch.windows.synthesis,
            psola_analysis_window: options.pitch.psola_windows.analysis,
            psola_synthesis_window: options.pitch.psola_windows.synthesis,
            pitch,
            pitch_exact_semitones,
            formant_shift: options.pitch.formant_shift.to_string(),
            preserve_formants: options.pitch.preserve_formants,
            rumble_filter: options.rumble_filter,
            gate_enabled: options.gate,
            gate_threshold_mode,
            gate_threshold_db,
            gate_attack_ms: options.gate_attack_ms.to_string(),
            gate_release_ms: options.gate_release_ms.to_string(),
            deesser_enabled: options.deesser,
            deesser_mode: if options.deesser_pro {
                UiMode::Pro
            } else {
                UiMode::Simple
            },
            deesser_frequency_hz: options.deesser_parameters.frequency_hz.to_string(),
            deesser_threshold_db: options.deesser_parameters.threshold_db.to_string(),
            deesser_range_db: options.deesser_parameters.range_db.to_string(),
            deesser_attack_ms: options.deesser_parameters.attack_ms.to_string(),
            deesser_release_ms: options.deesser_parameters.release_ms.to_string(),
            input_eq: EqDraft::from_settings(&options.input_eq, EqKind::Input.mode(options)),
            pre_conversion_eq: EqDraft::from_settings(
                &options.pre_conversion_eq,
                EqKind::PreConversion.mode(options),
            ),
            post_conversion_eq: EqDraft::from_settings(
                &options.eq,
                EqKind::PostConversion.mode(options),
            ),
            compressor_enabled: options.compressor.enabled,
            compressor_threshold_db: options.compressor.threshold_db.to_string(),
            compressor_ratio: options.compressor.ratio.to_string(),
            compressor_attack_ms: options.compressor.attack_ms.to_string(),
            compressor_release_ms: options.compressor.release_ms.to_string(),
            limiter_enabled: options.limiter.enabled,
            limiter_ceiling_db: options.limiter.ceiling_db.to_string(),
            limiter_release_ms: options.limiter.release_ms.to_string(),
            dereverb_enabled: options.dereverb.enabled,
            dereverb_amount: options.dereverb.amount.to_string(),
            dereverb_decay_ms: options.dereverb.decay_ms.to_string(),
            reverb_enabled: options.reverb.enabled,
            reverb_mix: options.reverb.mix.to_string(),
            reverb_pre_delay_ms: options.reverb.pre_delay_ms.to_string(),
            reverb_decay_ms: options.reverb.decay_ms.to_string(),
            level_enabled: options.level.enabled,
            post_compressor_enabled: options.level.compressor_enabled,
            level_target_db: options.level.target_db.to_string(),
            level_max_gain_db: options.level.max_gain_db.to_string(),
            level_floor_db: options.level.floor_db.to_string(),
            level_response_ms: options.level.response_ms.to_string(),
            post_compressor_threshold_db: options.level.threshold_db.to_string(),
            post_compressor_ratio: options.level.ratio.to_string(),
            post_compressor_attack_ms: options.level.attack_ms.to_string(),
            post_compressor_release_ms: options.level.release_ms.to_string(),
        }
    }

    pub fn eq(&self, kind: EqKind) -> &EqDraft {
        match kind {
            EqKind::Input => &self.input_eq,
            EqKind::PreConversion => &self.pre_conversion_eq,
            EqKind::PostConversion => &self.post_conversion_eq,
        }
    }

    pub fn eq_mut(&mut self, kind: EqKind) -> &mut EqDraft {
        match kind {
            EqKind::Input => &mut self.input_eq,
            EqKind::PreConversion => &mut self.pre_conversion_eq,
            EqKind::PostConversion => &mut self.post_conversion_eq,
        }
    }

    pub fn device(&self, slot: DeviceSlot) -> &str {
        match slot {
            DeviceSlot::Input => &self.device,
            DeviceSlot::Output => &self.output_device,
            DeviceSlot::Preview => &self.preview_device,
        }
    }

    pub fn set_device(&mut self, slot: DeviceSlot, value: String) {
        match slot {
            DeviceSlot::Input => self.device = value,
            DeviceSlot::Output => self.output_device = value,
            DeviceSlot::Preview => self.preview_device = value,
        }
    }

    pub fn toggle(&self, field: ToggleField) -> bool {
        match field {
            ToggleField::VoiceEnabled => self.voice_enabled,
            ToggleField::PreserveFormants => self.preserve_formants,
            ToggleField::RumbleFilter => self.rumble_filter,
            ToggleField::NoiseGateEnabled => self.gate_enabled,
            ToggleField::DeesserEnabled => self.deesser_enabled,
            ToggleField::CompressorEnabled => self.compressor_enabled,
            ToggleField::LimiterEnabled => self.limiter_enabled,
            ToggleField::DereverbEnabled => self.dereverb_enabled,
            ToggleField::ReverbEnabled => self.reverb_enabled,
            ToggleField::LevelEnabled => self.level_enabled,
            ToggleField::PostCompressorEnabled => self.post_compressor_enabled,
            ToggleField::EqEnabled(kind) => self.eq(kind).enabled,
        }
    }

    pub fn set_toggle(&mut self, field: ToggleField, value: bool) {
        match field {
            ToggleField::VoiceEnabled => self.voice_enabled = value,
            ToggleField::PreserveFormants => self.preserve_formants = value,
            ToggleField::RumbleFilter => self.rumble_filter = value,
            ToggleField::NoiseGateEnabled => self.gate_enabled = value,
            ToggleField::DeesserEnabled => self.deesser_enabled = value,
            ToggleField::CompressorEnabled => self.compressor_enabled = value,
            ToggleField::LimiterEnabled => self.limiter_enabled = value,
            ToggleField::DereverbEnabled => self.dereverb_enabled = value,
            ToggleField::ReverbEnabled => self.reverb_enabled = value,
            ToggleField::LevelEnabled => self.level_enabled = value,
            ToggleField::PostCompressorEnabled => self.post_compressor_enabled = value,
            ToggleField::EqEnabled(kind) => self.eq_mut(kind).enabled = value,
        }
    }

    pub fn section_enabled(&self, section: Section) -> bool {
        match section {
            Section::InputEq => self.input_eq.enabled,
            Section::PreConversionEq => self.pre_conversion_eq.enabled,
            Section::PostConversionEq => self.post_conversion_eq.enabled,
            Section::Deesser => self.deesser_enabled,
            Section::NoiseGate => self.gate_enabled,
            Section::Voice => self.voice_enabled,
            Section::AutoLevel => self.level_enabled,
            Section::PostCompressor => self.post_compressor_enabled,
            Section::Compressor => self.compressor_enabled,
            Section::Limiter => self.limiter_enabled,
            Section::Dereverb => self.dereverb_enabled,
            Section::Reverb => self.reverb_enabled,
            Section::Overview | Section::Devices => true,
        }
    }

    pub fn number_fields(&self) -> Vec<NumberField> {
        let mut fields = NumberField::FIXED.to_vec();
        for kind in EqKind::ALL {
            for index in 0..self.eq(kind).bands.len() {
                for field in EqBandField::ALL {
                    fields.push(NumberField::EqBand(kind, index, field));
                }
            }
        }
        fields
    }

    pub fn number_text(&self, field: NumberField) -> &str {
        match field {
            NumberField::Frames => &self.frames,
            NumberField::Pitch => &self.pitch,
            NumberField::FormantShift => &self.formant_shift,
            NumberField::GateThresholdDb => &self.gate_threshold_db,
            NumberField::GateAttackMs => &self.gate_attack_ms,
            NumberField::GateReleaseMs => &self.gate_release_ms,
            NumberField::DeesserFrequencyHz => &self.deesser_frequency_hz,
            NumberField::DeesserThresholdDb => &self.deesser_threshold_db,
            NumberField::DeesserRangeDb => &self.deesser_range_db,
            NumberField::DeesserAttackMs => &self.deesser_attack_ms,
            NumberField::DeesserReleaseMs => &self.deesser_release_ms,
            NumberField::CompressorThresholdDb => &self.compressor_threshold_db,
            NumberField::CompressorRatio => &self.compressor_ratio,
            NumberField::CompressorAttackMs => &self.compressor_attack_ms,
            NumberField::CompressorReleaseMs => &self.compressor_release_ms,
            NumberField::LimiterCeilingDb => &self.limiter_ceiling_db,
            NumberField::LimiterReleaseMs => &self.limiter_release_ms,
            NumberField::DereverbAmount => &self.dereverb_amount,
            NumberField::DereverbDecayMs => &self.dereverb_decay_ms,
            NumberField::ReverbMix => &self.reverb_mix,
            NumberField::ReverbPreDelayMs => &self.reverb_pre_delay_ms,
            NumberField::ReverbDecayMs => &self.reverb_decay_ms,
            NumberField::LevelTargetDb => &self.level_target_db,
            NumberField::LevelMaxGainDb => &self.level_max_gain_db,
            NumberField::LevelFloorDb => &self.level_floor_db,
            NumberField::LevelResponseMs => &self.level_response_ms,
            NumberField::PostCompressorThresholdDb => &self.post_compressor_threshold_db,
            NumberField::PostCompressorRatio => &self.post_compressor_ratio,
            NumberField::PostCompressorAttackMs => &self.post_compressor_attack_ms,
            NumberField::PostCompressorReleaseMs => &self.post_compressor_release_ms,
            NumberField::EqBand(kind, index, field) => {
                let band = &self.eq(kind).bands[index];
                match field {
                    EqBandField::Frequency => &band.frequency,
                    EqBandField::GainDb => &band.gain_db,
                    EqBandField::Q => &band.q,
                }
            }
            NumberField::EqOutputDb(kind) => &self.eq(kind).output_db,
        }
    }

    pub fn set_number_text(&mut self, field: NumberField, text: String) {
        if field == NumberField::Pitch {
            self.pitch_exact_semitones = None;
        }
        let slot = match field {
            NumberField::Frames => &mut self.frames,
            NumberField::Pitch => &mut self.pitch,
            NumberField::FormantShift => &mut self.formant_shift,
            NumberField::GateThresholdDb => &mut self.gate_threshold_db,
            NumberField::GateAttackMs => &mut self.gate_attack_ms,
            NumberField::GateReleaseMs => &mut self.gate_release_ms,
            NumberField::DeesserFrequencyHz => &mut self.deesser_frequency_hz,
            NumberField::DeesserThresholdDb => &mut self.deesser_threshold_db,
            NumberField::DeesserRangeDb => &mut self.deesser_range_db,
            NumberField::DeesserAttackMs => &mut self.deesser_attack_ms,
            NumberField::DeesserReleaseMs => &mut self.deesser_release_ms,
            NumberField::CompressorThresholdDb => &mut self.compressor_threshold_db,
            NumberField::CompressorRatio => &mut self.compressor_ratio,
            NumberField::CompressorAttackMs => &mut self.compressor_attack_ms,
            NumberField::CompressorReleaseMs => &mut self.compressor_release_ms,
            NumberField::LimiterCeilingDb => &mut self.limiter_ceiling_db,
            NumberField::LimiterReleaseMs => &mut self.limiter_release_ms,
            NumberField::DereverbAmount => &mut self.dereverb_amount,
            NumberField::DereverbDecayMs => &mut self.dereverb_decay_ms,
            NumberField::ReverbMix => &mut self.reverb_mix,
            NumberField::ReverbPreDelayMs => &mut self.reverb_pre_delay_ms,
            NumberField::ReverbDecayMs => &mut self.reverb_decay_ms,
            NumberField::LevelTargetDb => &mut self.level_target_db,
            NumberField::LevelMaxGainDb => &mut self.level_max_gain_db,
            NumberField::LevelFloorDb => &mut self.level_floor_db,
            NumberField::LevelResponseMs => &mut self.level_response_ms,
            NumberField::PostCompressorThresholdDb => &mut self.post_compressor_threshold_db,
            NumberField::PostCompressorRatio => &mut self.post_compressor_ratio,
            NumberField::PostCompressorAttackMs => &mut self.post_compressor_attack_ms,
            NumberField::PostCompressorReleaseMs => &mut self.post_compressor_release_ms,
            NumberField::EqBand(kind, index, field) => {
                let band = &mut self.eq_mut(kind).bands[index];
                match field {
                    EqBandField::Frequency => &mut band.frequency,
                    EqBandField::GainDb => &mut band.gain_db,
                    EqBandField::Q => &mut band.q,
                }
            }
            NumberField::EqOutputDb(kind) => &mut self.eq_mut(kind).output_db,
        };
        *slot = text;
    }

    pub fn add_eq_band(&mut self, kind: EqKind) -> Option<usize> {
        let eq = self.eq_mut(kind);
        if eq.bands.len() >= crate::eq::MAX_BANDS {
            return None;
        }
        let index = eq.bands.len();
        eq.bands.push(EqBandDraft::from_band(Band {
            frequency: 1000.0,
            gain_db: 0.0,
            q: 0.7,
        }));
        Some(index)
    }

    pub fn remove_eq_band(&mut self, kind: EqKind) -> Option<usize> {
        let eq = self.eq_mut(kind);
        if eq.bands.len() <= 1 {
            return None;
        }
        eq.bands.pop();
        Some(eq.bands.len())
    }

    fn parse_pitch_semitones(&self) -> Result<f64, DraftError> {
        match self.pitch_unit {
            PitchUnit::Semitones => parse_f64(&self.pitch, NumberField::Pitch),
            PitchUnit::Percent => {
                if let Some(exact) = self.pitch_exact_semitones {
                    return Ok(exact);
                }
                let percentage = self.pitch.parse::<f64>().map_err(|_| {
                    DraftError::for_field(
                        ErrorField::Number(NumberField::Pitch),
                        "enter a percentage",
                    )
                })?;
                percent_to_semitones(percentage).map_err(|message| {
                    DraftError::for_field(ErrorField::Number(NumberField::Pitch), message)
                })
            }
        }
    }

    fn parse_voice(&self) -> Result<PitchSettings, DraftError> {
        Ok(PitchSettings {
            math_precision: self.math_precision,
            psola_pitch_estimator: self.psola_pitch_estimator,
            psola_windows: SpectralWindows {
                analysis: self.psola_analysis_window,
                synthesis: self.psola_synthesis_window,
            },
            windows: SpectralWindows {
                analysis: self.analysis_window,
                synthesis: self.synthesis_window,
            },
            algorithm: self.pitch_algorithm,
            semitones: self.parse_pitch_semitones()?,
            formant_shift: parse_f64(&self.formant_shift, NumberField::FormantShift)?,
            preserve_formants: self.preserve_formants,
        })
    }

    fn parse_gate_threshold(&self) -> Result<GateThreshold, DraftError> {
        match self.gate_threshold_mode {
            GateThresholdMode::Auto => Ok(GateThreshold::Auto),
            GateThresholdMode::Manual => Ok(GateThreshold::Db(parse_f64(
                &self.gate_threshold_db,
                NumberField::GateThresholdDb,
            )?)),
        }
    }

    fn parse_deesser(&self) -> Result<crate::deesser::DeEsserParameters, DraftError> {
        Ok(crate::deesser::DeEsserParameters {
            frequency_hz: parse_f64(&self.deesser_frequency_hz, NumberField::DeesserFrequencyHz)?,
            threshold_db: parse_f64(&self.deesser_threshold_db, NumberField::DeesserThresholdDb)?,
            range_db: parse_f64(&self.deesser_range_db, NumberField::DeesserRangeDb)?,
            attack_ms: parse_f64(&self.deesser_attack_ms, NumberField::DeesserAttackMs)?,
            release_ms: parse_f64(&self.deesser_release_ms, NumberField::DeesserReleaseMs)?,
        })
    }

    fn parse_compressor(&self) -> Result<crate::compressor::CompressorSettings, DraftError> {
        Ok(crate::compressor::CompressorSettings {
            enabled: self.compressor_enabled,
            threshold_db: parse_f64(
                &self.compressor_threshold_db,
                NumberField::CompressorThresholdDb,
            )?,
            ratio: parse_f64(&self.compressor_ratio, NumberField::CompressorRatio)?,
            attack_ms: parse_f64(&self.compressor_attack_ms, NumberField::CompressorAttackMs)?,
            release_ms: parse_f64(
                &self.compressor_release_ms,
                NumberField::CompressorReleaseMs,
            )?,
        })
    }

    fn parse_limiter(&self) -> Result<crate::limiter::LimiterSettings, DraftError> {
        Ok(crate::limiter::LimiterSettings {
            enabled: self.limiter_enabled,
            ceiling_db: parse_f64(&self.limiter_ceiling_db, NumberField::LimiterCeilingDb)?,
            release_ms: parse_f64(&self.limiter_release_ms, NumberField::LimiterReleaseMs)?,
        })
    }

    fn parse_dereverb(&self) -> Result<crate::dereverb::DereverbSettings, DraftError> {
        Ok(crate::dereverb::DereverbSettings {
            enabled: self.dereverb_enabled,
            amount: parse_f64(&self.dereverb_amount, NumberField::DereverbAmount)?,
            decay_ms: parse_f64(&self.dereverb_decay_ms, NumberField::DereverbDecayMs)?,
        })
    }

    fn parse_reverb(&self) -> Result<crate::reverb::ReverbSettings, DraftError> {
        Ok(crate::reverb::ReverbSettings {
            enabled: self.reverb_enabled,
            mix: parse_f64(&self.reverb_mix, NumberField::ReverbMix)?,
            pre_delay_ms: parse_f64(&self.reverb_pre_delay_ms, NumberField::ReverbPreDelayMs)?,
            decay_ms: parse_f64(&self.reverb_decay_ms, NumberField::ReverbDecayMs)?,
        })
    }

    fn parse_level(&self) -> Result<crate::level::LevelSettings, DraftError> {
        Ok(crate::level::LevelSettings {
            enabled: self.level_enabled,
            compressor_enabled: self.post_compressor_enabled,
            target_db: parse_f64(&self.level_target_db, NumberField::LevelTargetDb)?,
            max_gain_db: parse_f64(&self.level_max_gain_db, NumberField::LevelMaxGainDb)?,
            floor_db: parse_f64(&self.level_floor_db, NumberField::LevelFloorDb)?,
            response_ms: parse_f64(&self.level_response_ms, NumberField::LevelResponseMs)?,
            threshold_db: parse_f64(
                &self.post_compressor_threshold_db,
                NumberField::PostCompressorThresholdDb,
            )?,
            ratio: parse_f64(
                &self.post_compressor_ratio,
                NumberField::PostCompressorRatio,
            )?,
            attack_ms: parse_f64(
                &self.post_compressor_attack_ms,
                NumberField::PostCompressorAttackMs,
            )?,
            release_ms: parse_f64(
                &self.post_compressor_release_ms,
                NumberField::PostCompressorReleaseMs,
            )?,
        })
    }

    pub fn to_options(&self) -> Result<Options, DraftError> {
        let mut options = Options {
            input_eq: self.input_eq.parse(EqKind::Input)?,
            input_eq_mode: self.input_eq.mode.as_str().into(),
            pre_conversion_eq: self.pre_conversion_eq.parse(EqKind::PreConversion)?,
            pre_conversion_eq_mode: self.pre_conversion_eq.mode.as_str().into(),
            voice_enabled: self.voice_enabled,
            compressor: self.parse_compressor()?,
            limiter: self.parse_limiter()?,
            dereverb: self.parse_dereverb()?,
            reverb: self.parse_reverb()?,
            eq: self.post_conversion_eq.parse(EqKind::PostConversion)?,
            eq_mode: self.post_conversion_eq.mode.as_str().into(),
            pitch_unit: self.pitch_unit.as_str().into(),
            level: self.parse_level()?,
            device: self.device.clone(),
            output_device: self.output_device.clone(),
            preview_device: self.preview_device.clone(),
            input_channel: self.input_channel,
            output_channels: self.output_channels.clone(),
            backend: self.backend,
            frames: parse_u32(&self.frames, NumberField::Frames)?,
            sample_rate: self.sample_rate,
            pitch: self.parse_voice()?,
            rumble_filter: self.rumble_filter,
            deesser: self.deesser_enabled,
            deesser_pro: self.deesser_mode == UiMode::Pro,
            deesser_parameters: self.parse_deesser()?,
            gate: self.gate_enabled,
            gate_threshold: self.parse_gate_threshold()?,
            gate_attack_ms: parse_f64(&self.gate_attack_ms, NumberField::GateAttackMs)?,
            gate_release_ms: parse_f64(&self.gate_release_ms, NumberField::GateReleaseMs)?,
            ..Options::default()
        };
        options
            .validate()
            .map_err(|error| DraftError::plain(error.to_string()))?;
        options.output_channels.sort_unstable();
        Ok(options)
    }

    /// Parses only the fields belonging to `stage` (plus `sample_rate`) onto `base`,
    /// so an unfinished input on another page does not break previews or latency display.
    pub fn to_stage_options(
        &self,
        stage: FilterStage,
        base: &Options,
    ) -> Result<Options, DraftError> {
        let mut options = base.clone();
        options.sample_rate = self.sample_rate;
        match stage {
            FilterStage::InputEq => {
                options.rumble_filter = self.rumble_filter;
                options.input_eq = self.input_eq.parse(EqKind::Input)?;
                options.input_eq_mode = self.input_eq.mode.as_str().into();
            }
            FilterStage::Limiter => {
                options.limiter = self.parse_limiter()?;
            }
            FilterStage::NoiseGate => {
                options.gate = self.gate_enabled;
                options.gate_threshold = self.parse_gate_threshold()?;
                options.gate_attack_ms =
                    parse_f64(&self.gate_attack_ms, NumberField::GateAttackMs)?;
                options.gate_release_ms =
                    parse_f64(&self.gate_release_ms, NumberField::GateReleaseMs)?;
            }
            FilterStage::Dereverb => {
                options.dereverb = self.parse_dereverb()?;
            }
            FilterStage::Deesser => {
                options.deesser = self.deesser_enabled;
                options.deesser_pro = self.deesser_mode == UiMode::Pro;
                options.deesser_parameters = self.parse_deesser()?;
            }
            FilterStage::Compressor => {
                options.compressor = self.parse_compressor()?;
            }
            FilterStage::PreConversionEq => {
                options.pre_conversion_eq = self.pre_conversion_eq.parse(EqKind::PreConversion)?;
                options.pre_conversion_eq_mode = self.pre_conversion_eq.mode.as_str().into();
            }
            FilterStage::Voice => {
                options.voice_enabled = self.voice_enabled;
                options.pitch_unit = self.pitch_unit.as_str().into();
                options.pitch = self.parse_voice()?;
            }
            FilterStage::PostConversionEq => {
                options.eq = self.post_conversion_eq.parse(EqKind::PostConversion)?;
                options.eq_mode = self.post_conversion_eq.mode.as_str().into();
            }
            FilterStage::Level => {
                options.level = self.parse_level()?;
            }
            FilterStage::Reverb => {
                options.reverb = self.parse_reverb()?;
            }
        }
        options
            .validate()
            .map_err(|error| DraftError::plain(error.to_string()))?;
        Ok(options)
    }
}

fn parse_f64(text: &str, field: NumberField) -> Result<f64, DraftError> {
    text.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| DraftError::for_field(ErrorField::Number(field), "enter a finite number"))
}

fn parse_u32(text: &str, field: NumberField) -> Result<u32, DraftError> {
    let parsed = parse_f64(text, field)?;
    if parsed < 0.0 || parsed.fract() != 0.0 || parsed > u32::MAX as f64 {
        return Err(DraftError::for_field(
            ErrorField::Number(field),
            "enter a nonnegative integer",
        ));
    }
    Ok(parsed as u32)
}

pub fn semitones_to_percent(value: f64) -> f64 {
    100. * 2f64.powf(value / 12.)
}

pub fn percent_to_semitones(value: f64) -> Result<f64, String> {
    if !value.is_finite() || !(50. ..=200.).contains(&value) {
        return Err("Pitch percentage must be between 50 and 200".into());
    }
    Ok(12. * (value / 100.).log2())
}

/// UI operation generations invalidate stale device replies and closed-window callbacks.
#[derive(Default, Debug)]
pub struct Generation(u64);
impl Generation {
    pub fn current(&self) -> u64 {
        self.0
    }
    pub fn advance(&mut self) -> u64 {
        self.0 = self
            .0
            .checked_add(1)
            .expect("operation generation exhausted");
        self.0
    }
    pub fn accepts(&self, generation: u64) -> bool {
        self.0 == generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fields_round_trip_and_incomplete_values_do_not_commit() {
        let original = Options::default();
        let mut draft = SettingsDraft::from_options(&original);
        assert_eq!(draft.to_options().unwrap(), original);
        for bad in ["-", "NaN", "1e999", "13"] {
            draft.set_number_text(NumberField::Pitch, bad.into());
            assert!(draft.to_options().is_err());
        }
        draft.set_number_text(NumberField::Pitch, "7.5".into());
        assert_eq!(draft.to_options().unwrap().pitch.semitones, 7.5);
        assert_eq!(
            SettingsDraft::from_options(&original).to_options().unwrap(),
            original
        );
    }

    #[test]
    fn every_number_field_has_a_section() {
        let draft = SettingsDraft::from_options(&Options::default());
        for field in draft.number_fields() {
            assert!(Section::ALL.contains(&field.section()), "{field:?}");
        }
    }

    #[test]
    fn stale_results_are_rejected() {
        let mut generation = Generation::default();
        let first = generation.advance();
        assert!(generation.accepts(first));
        generation.advance();
        assert!(!generation.accepts(first));
    }

    #[test]
    fn percent_display_maps_to_semitones_without_drifting_unmodified_drafts() {
        for (percent, semitones) in [(50., -12.), (100., 0.), (200., 12.)] {
            assert!((percent_to_semitones(percent).unwrap() - semitones).abs() < 1e-10);
            assert!((semitones_to_percent(semitones) - percent).abs() < 1e-10);
        }
        for semitones in [-7.5, -3.0, 3.0, 7.0, 7.5, 9.0] {
            let options = Options {
                pitch_unit: "percent".into(),
                pitch: PitchSettings {
                    semitones,
                    ..Default::default()
                },
                ..Default::default()
            };
            let draft = SettingsDraft::from_options(&options);
            assert_eq!(draft.to_options().unwrap(), options);
        }
        for bad in [0., 49., 201., f64::NAN] {
            assert!(percent_to_semitones(bad).is_err());
        }
    }

    #[test]
    fn numeric_editing_accepts_float_syntax_and_keeps_formants_in_semitones() {
        let mut draft = SettingsDraft::from_options(&Options::default());
        for text in ["+7.5", "7.50", "7.5e0"] {
            draft.set_number_text(NumberField::Pitch, text.into());
            draft.set_number_text(NumberField::Frames, "480.0".into());
            let options = draft.to_options().unwrap();
            assert_eq!(options.pitch.semitones, 7.5);
            assert_eq!(options.frames, 480);
        }
        draft.pitch_unit = PitchUnit::Percent;
        draft.set_number_text(NumberField::Pitch, "200".into());
        draft.set_number_text(NumberField::FormantShift, "3".into());
        let options = draft.to_options().unwrap();
        assert_eq!(options.pitch.semitones, 12.);
        assert_eq!(options.pitch.formant_shift, 3.);
    }

    #[test]
    fn empty_output_channels_reports_channel_validation_error() {
        let mut draft = SettingsDraft::from_options(&Options::default());
        draft.output_channels.clear();
        let error = draft.to_options().unwrap_err();
        assert!(error.message.contains("output channel"), "{error:?}");
    }

    #[test]
    fn spectral_windows_gate_modes_and_disabled_filters_round_trip_and_validate() {
        let options = Options::parse_gui(
            [
                "--analysis-window",
                "8192",
                "--synthesis-window",
                "2048",
                "--psola-analysis-window",
                "1024",
                "--psola-synthesis-window",
                "256",
                "--deesser",
                "on",
                "--deesser-mode",
                "pro",
                "--deesser-frequency",
                "6500",
                "--deesser-range",
                "9",
                "--gate",
                "on",
                "--gate-threshold",
                "-35.5",
                "--auto-level",
                "on",
                "--compressor",
                "on",
                "--eq",
                "on",
                "--eq-band1",
                "180,-4,1.2",
                "--eq-output",
                "-2",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut draft = SettingsDraft::from_options(&options);
        assert_eq!(draft.to_options().unwrap(), options);
        draft.psola_synthesis_window = 2048;
        assert!(draft.to_options().is_err());
        draft.psola_synthesis_window = 256;
        draft.gate_threshold_mode = GateThresholdMode::Auto;
        assert_eq!(
            draft.to_options().unwrap().gate_threshold,
            GateThreshold::Auto
        );
        draft.gate_threshold_mode = GateThresholdMode::Manual;
        draft.set_number_text(NumberField::GateThresholdDb, "-91".into());
        assert!(draft.to_options().is_err());
    }
}
