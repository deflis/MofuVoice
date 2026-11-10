use crate::{backend, cli::Options, gate::GateThreshold};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LevelControls {
    enabled: bool,
    compressor_enabled: bool,
    target_db: f64,
    max_gain_db: f64,
    floor_db: f64,
    response_ms: f64,
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
}
impl LevelControls {
    fn from_settings(settings: crate::level::LevelSettings) -> Self {
        Self {
            enabled: settings.enabled,
            compressor_enabled: settings.compressor_enabled,
            target_db: settings.target_db,
            max_gain_db: settings.max_gain_db,
            floor_db: settings.floor_db,
            response_ms: settings.response_ms,
            threshold_db: settings.threshold_db,
            ratio: settings.ratio,
            attack_ms: settings.attack_ms,
            release_ms: settings.release_ms,
        }
    }
    fn settings(&self) -> crate::level::LevelSettings {
        crate::level::LevelSettings {
            enabled: self.enabled,
            compressor_enabled: self.compressor_enabled,
            target_db: self.target_db,
            max_gain_db: self.max_gain_db,
            floor_db: self.floor_db,
            response_ms: self.response_ms,
            threshold_db: self.threshold_db,
            ratio: self.ratio,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EqBandControls {
    frequency: f64,
    gain_db: f64,
    q: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EqControls {
    enabled: bool,
    mode: String,
    bands: Vec<EqBandControls>,
    output_db: f64,
}
impl EqControls {
    pub(super) fn from_options(options: &Options) -> Self {
        Self::from_settings(&options.eq, &options.eq_mode)
    }
    fn from_settings(settings: &crate::eq::EqSettings, mode: &str) -> Self {
        Self {
            enabled: settings.enabled,
            mode: mode.into(),
            bands: settings
                .bands
                .iter()
                .map(|b| EqBandControls {
                    frequency: b.frequency,
                    gain_db: b.gain_db,
                    q: b.q,
                })
                .collect(),
            output_db: settings.output_db,
        }
    }
    fn settings(&self) -> crate::eq::EqSettings {
        crate::eq::EqSettings {
            enabled: self.enabled,
            bands: self
                .bands
                .iter()
                .map(|band| crate::eq::Band {
                    frequency: band.frequency,
                    gain_db: band.gain_db,
                    q: band.q,
                })
                .collect(),
            output_db: self.output_db,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LimiterControls {
    enabled: bool,
    ceiling_db: f64,
    release_ms: f64,
}
impl LimiterControls {
    fn from_settings(s: crate::limiter::LimiterSettings) -> Self {
        Self {
            enabled: s.enabled,
            ceiling_db: s.ceiling_db,
            release_ms: s.release_ms,
        }
    }
    fn settings(&self) -> crate::limiter::LimiterSettings {
        crate::limiter::LimiterSettings {
            enabled: self.enabled,
            ceiling_db: self.ceiling_db,
            release_ms: self.release_ms,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DereverbControls {
    enabled: bool,
    amount: f64,
    decay_ms: f64,
}
impl DereverbControls {
    fn from_settings(s: crate::dereverb::DereverbSettings) -> Self {
        Self {
            enabled: s.enabled,
            amount: s.amount,
            decay_ms: s.decay_ms,
        }
    }
    fn settings(&self) -> crate::dereverb::DereverbSettings {
        crate::dereverb::DereverbSettings {
            enabled: self.enabled,
            amount: self.amount,
            decay_ms: self.decay_ms,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReverbControls {
    enabled: bool,
    mix: f64,
    pre_delay_ms: f64,
    decay_ms: f64,
}
impl ReverbControls {
    fn from_settings(s: crate::reverb::ReverbSettings) -> Self {
        Self {
            enabled: s.enabled,
            mix: s.mix,
            pre_delay_ms: s.pre_delay_ms,
            decay_ms: s.decay_ms,
        }
    }
    fn settings(&self) -> crate::reverb::ReverbSettings {
        crate::reverb::ReverbSettings {
            enabled: self.enabled,
            mix: self.mix,
            pre_delay_ms: self.pre_delay_ms,
            decay_ms: self.decay_ms,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompressorControls {
    enabled: bool,
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
}
impl CompressorControls {
    fn from_settings(s: crate::compressor::CompressorSettings) -> Self {
        Self {
            enabled: s.enabled,
            threshold_db: s.threshold_db,
            ratio: s.ratio,
            attack_ms: s.attack_ms,
            release_ms: s.release_ms,
        }
    }
    fn settings(&self) -> crate::compressor::CompressorSettings {
        crate::compressor::CompressorSettings {
            enabled: self.enabled,
            threshold_db: self.threshold_db,
            ratio: self.ratio,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GateThresholdMode {
    Auto,
    Manual,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeEsserControls {
    enabled: bool,
    mode: String,
    frequency_hz: f64,
    threshold_db: f64,
    range_db: f64,
    attack_ms: f64,
    release_ms: f64,
}
impl DeEsserControls {
    pub(super) fn from_options(options: &Options) -> Self {
        let p = options.deesser_parameters;
        Self {
            enabled: options.deesser,
            mode: if options.deesser_pro { "pro" } else { "simple" }.into(),
            frequency_hz: p.frequency_hz,
            threshold_db: p.threshold_db,
            range_db: p.range_db,
            attack_ms: p.attack_ms,
            release_ms: p.release_ms,
        }
    }
    fn parameters(&self) -> crate::deesser::DeEsserParameters {
        crate::deesser::DeEsserParameters {
            frequency_hz: self.frequency_hz,
            threshold_db: self.threshold_db,
            range_db: self.range_db,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    compressor: CompressorControls,
    input_eq: EqControls,
    pre_conversion_eq: EqControls,
    limiter: LimiterControls,
    dereverb: DereverbControls,
    reverb: ReverbControls,
    devices: DeviceSettings,
    voice: VoiceSettings,
    noise: NoiseSettings,
    deesser: DeEsserControls,
    eq: EqControls,
    level: LevelControls,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeviceSettings {
    backend: String,
    device: String,
    output_device: String,
    preview_device: String,
    input_channel: usize,
    output_channels: Vec<usize>,
    frames: u32,
    sample_rate: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VoiceSettings {
    enabled: bool,
    pitch_unit: String,
    math_precision: String,
    psola_pitch_estimator: String,
    psola_analysis_window: usize,
    psola_synthesis_window: usize,
    analysis_window: usize,
    synthesis_window: usize,
    pitch_algorithm: String,
    pitch: f64,
    formant_shift: f64,
    preserve_formants: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NoiseSettings {
    rumble_filter: bool,
    enabled: bool,
    threshold_mode: GateThresholdMode,
    threshold_db: f64,
    attack_ms: f64,
    release_ms: f64,
}
impl Settings {
    pub(super) fn from_options(options: &Options) -> Self {
        Self {
            devices: DeviceSettings {
                backend: options.backend.name().into(),
                device: options.device.clone(),
                output_device: options.output_device.clone(),
                preview_device: options.preview_device.clone(),
                input_channel: options.input_channel,
                output_channels: options.output_channels.clone(),
                frames: options.frames,
                sample_rate: options.sample_rate,
            },
            voice: VoiceSettings {
                enabled: options.voice_enabled,
                pitch_unit: options.pitch_unit.clone(),
                math_precision: options.pitch.math_precision.name().into(),
                psola_pitch_estimator: options.pitch.psola_pitch_estimator.name().into(),
                psola_analysis_window: options.pitch.psola_windows.analysis,
                psola_synthesis_window: options.pitch.psola_windows.synthesis,
                analysis_window: options.pitch.windows.analysis,
                synthesis_window: options.pitch.windows.synthesis,
                pitch_algorithm: options.pitch.algorithm.name().into(),
                pitch: options.pitch.semitones,
                formant_shift: options.pitch.formant_shift,
                preserve_formants: options.pitch.preserve_formants,
            },
            noise: NoiseSettings {
                rumble_filter: options.rumble_filter,
                enabled: options.gate,
                threshold_mode: match options.gate_threshold {
                    GateThreshold::Auto => GateThresholdMode::Auto,
                    GateThreshold::Db(_) => GateThresholdMode::Manual,
                },
                threshold_db: match options.gate_threshold {
                    GateThreshold::Auto => -50.0,
                    GateThreshold::Db(db) => db,
                },
                attack_ms: options.gate_attack_ms,
                release_ms: options.gate_release_ms,
            },
            deesser: DeEsserControls::from_options(options),
            input_eq: EqControls::from_settings(&options.input_eq, &options.input_eq_mode),
            pre_conversion_eq: EqControls::from_settings(
                &options.pre_conversion_eq,
                &options.pre_conversion_eq_mode,
            ),
            compressor: CompressorControls::from_settings(options.compressor),
            limiter: LimiterControls::from_settings(options.limiter),
            dereverb: DereverbControls::from_settings(options.dereverb),
            reverb: ReverbControls::from_settings(options.reverb),
            eq: EqControls::from_options(options),
            level: LevelControls::from_settings(options.level),
        }
    }
    pub(super) fn options(&self) -> Result<Options, String> {
        let mut options = Options {
            input_eq: self.input_eq.settings(),
            input_eq_mode: self.input_eq.mode.clone(),
            pre_conversion_eq: self.pre_conversion_eq.settings(),
            pre_conversion_eq_mode: self.pre_conversion_eq.mode.clone(),
            voice_enabled: self.voice.enabled,
            compressor: self.compressor.settings(),
            limiter: self.limiter.settings(),
            dereverb: self.dereverb.settings(),
            reverb: self.reverb.settings(),
            eq: self.eq.settings(),
            eq_mode: self.eq.mode.clone(),
            pitch_unit: self.voice.pitch_unit.clone(),
            level: self.level.settings(),
            device: self.devices.device.clone(),
            output_device: self.devices.output_device.clone(),
            preview_device: self.devices.preview_device.clone(),
            input_channel: self.devices.input_channel,
            output_channels: self.devices.output_channels.clone(),
            backend: backend::BackendKind::parse(&self.devices.backend)
                .map_err(|error| error.to_string())?,
            frames: self.devices.frames,
            sample_rate: self.devices.sample_rate,
            pitch: crate::pitch::PitchSettings {
                math_precision: crate::pitch::MathPrecision::parse(&self.voice.math_precision)
                    .map_err(str::to_string)?,
                psola_pitch_estimator: crate::pitch::PitchEstimator::parse(
                    &self.voice.psola_pitch_estimator,
                )
                .map_err(str::to_string)?,
                psola_windows: crate::pitch::SpectralWindows {
                    analysis: self.voice.psola_analysis_window,
                    synthesis: self.voice.psola_synthesis_window,
                },
                windows: crate::pitch::SpectralWindows {
                    analysis: self.voice.analysis_window,
                    synthesis: self.voice.synthesis_window,
                },
                algorithm: crate::pitch::PitchAlgorithm::parse(&self.voice.pitch_algorithm)
                    .map_err(str::to_string)?,
                semitones: self.voice.pitch,
                formant_shift: self.voice.formant_shift,
                preserve_formants: self.voice.preserve_formants,
            },
            rumble_filter: self.noise.rumble_filter,
            deesser: self.deesser.enabled,
            deesser_pro: match self.deesser.mode.as_str() {
                "simple" => false,
                "pro" => true,
                _ => return Err("De-esser mode requires simple or pro".into()),
            },
            deesser_parameters: self.deesser.parameters(),
            gate: self.noise.enabled,
            gate_threshold: match self.noise.threshold_mode {
                GateThresholdMode::Auto => GateThreshold::Auto,
                GateThresholdMode::Manual => GateThreshold::Db(self.noise.threshold_db),
            },
            gate_attack_ms: self.noise.attack_ms,
            gate_release_ms: self.noise.release_ms,
            ..Options::default()
        };
        options.validate().map_err(|error| error.to_string())?;
        options.output_channels.sort_unstable();
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_settings;
    #[test]
    fn spectral_windows_round_trip_and_reject_invalid_combinations() {
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
            ]
            .map(String::from),
        )
        .unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.voice.psola_synthesis_window = 2048;
        assert!(settings.options().is_err());
        settings.voice.psola_synthesis_window = 256;
        settings.voice.analysis_window = 1024;
        assert!(settings.options().is_err());
    }
    #[test]
    fn pitch_display_unit_controls_round_trip_and_reject_unknown_values() {
        for unit in ["semitones", "percent"] {
            let options = Options {
                pitch_unit: unit.into(),
                ..Options::default()
            };
            let mut settings = Settings::from_options(&options);
            assert_eq!(settings.voice.pitch_unit, unit);
            assert_eq!(settings.options().unwrap(), options);
            settings.voice.pitch_unit = "invalid".into();
            assert!(settings.options().is_err());
        }
    }

    #[test]
    fn math_precision_controls_round_trip_and_reject_unknown_values() {
        for mode in ["standard", "accurate"] {
            let options = Options::parse_gui(["--math-precision", mode].map(String::from)).unwrap();
            let mut settings = Settings::from_options(&options);
            assert_eq!(settings.voice.math_precision, mode);
            assert_eq!(settings.options().unwrap(), options);
            settings.voice.math_precision = "unknown".into();
            assert!(settings.options().is_err());
        }
    }

    #[test]
    fn pitch_estimator_controls_round_trip_and_reject_unknown_values() {
        for estimator in crate::pitch::PitchEstimator::ALL {
            let options =
                Options::parse_gui(["--psola-pitch-estimator", estimator.name()].map(String::from))
                    .unwrap();
            let mut settings = Settings::from_options(&options);
            assert_eq!(settings.voice.psola_pitch_estimator, estimator.name());
            assert_eq!(settings.options().unwrap(), options);
            settings.voice.psola_pitch_estimator = "unknown".into();
            assert!(settings.options().is_err());
        }
    }

    #[test]
    fn pitch_algorithm_controls_round_trip_and_reject_unknown_values() {
        for algorithm in crate::pitch::PitchAlgorithm::ALL {
            let options =
                Options::parse_gui(["--pitch-algorithm", algorithm.name()].map(String::from))
                    .unwrap();
            let mut settings = Settings::from_options(&options);
            assert_eq!(settings.voice.pitch_algorithm, algorithm.name());
            assert_eq!(settings.options().unwrap(), options);
            settings.voice.pitch_algorithm = "unknown".into();
            assert!(settings.options().is_err());
        }
    }
    #[test]
    fn settings_payload_uses_numeric_values_in_feature_groups() {
        let settings = Settings::from_options(&Options::default());
        let json = serde_json::to_string(&settings).unwrap();
        for key in ["devices", "voice", "noise", "deesser", "eq", "level"] {
            assert!(json.contains(&format!("\"{key}\":{{")), "{json}");
        }
        assert!(json.contains("\"sampleRate\":48000"), "{json}");
        assert!(json.contains("\"pitch\":0.0"), "{json}");
        assert!(!json.contains("\"pitch\":\""), "{json}");
        assert!(!json.contains("\"sampleRate\":\""), "{json}");
    }
    #[test]
    fn sample_rate_controls_round_trip_and_validate() {
        let options = Options::parse_gui(["--sample-rate", "44100"].map(String::from)).unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.devices.sample_rate, 44100);
        assert_eq!(settings.options().unwrap(), options);
        settings.devices.sample_rate = 48000;
        assert_eq!(settings.options().unwrap().sample_rate, 48000);
        for value in [0, 32000, u32::MAX] {
            settings.devices.sample_rate = value;
            assert!(settings.options().is_err(), "{value}");
        }
    }

    #[test]
    fn channel_controls_round_trip_and_validate() {
        let options = Options::parse_gui(
            ["--input-channel", "2", "--output-channels", "1,2"].map(String::from),
        )
        .unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.devices.output_channels.clear();
        assert!(settings.options().is_err());
        settings.devices.output_channels = vec![2];
        assert_eq!(settings.options().unwrap().output_channels, [2]);
    }
    #[test]
    fn gate_mode_and_manual_value_map_to_engine_settings() {
        let options = Options::parse_gui(Vec::new()).unwrap();
        let mut settings = Settings::from_options(&options);
        assert!(matches!(
            settings.noise.threshold_mode,
            GateThresholdMode::Auto
        ));
        assert_eq!(
            settings.options().unwrap().gate_threshold,
            GateThreshold::Auto
        );
        settings.noise.threshold_mode = GateThresholdMode::Manual;
        for value in [-90.0, -35.5, 0.0] {
            settings.noise.threshold_db = value;
            let manual = settings.options().unwrap();
            assert_eq!(manual.gate_threshold, GateThreshold::Db(value));
            let restored = Settings::from_options(&manual);
            assert!(matches!(
                restored.noise.threshold_mode,
                GateThresholdMode::Manual
            ));
            assert_eq!(restored.noise.threshold_db, value);
        }
        for value in [f64::NAN, f64::INFINITY, -91.0, 1.0] {
            settings.noise.threshold_db = value;
            assert!(settings.options().is_err(), "{value}");
        }
        settings.noise.threshold_mode = GateThresholdMode::Auto;
        assert_eq!(
            settings.options().unwrap().gate_threshold,
            GateThreshold::Auto
        );
    }

    #[test]
    fn deesser_controls_round_trip_and_reject_invalid_values() {
        let options = Options::parse_gui(
            [
                "--deesser",
                "on",
                "--deesser-mode",
                "pro",
                "--deesser-frequency",
                "6500",
                "--deesser-range",
                "9",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.deesser.mode = "simple".into();
        assert_eq!(
            settings.options().unwrap().deesser_parameters,
            options.deesser_parameters
        );
        settings.deesser.frequency_hz = f64::NAN;
        assert!(settings.options().is_err());
    }
    #[test]
    fn auto_level_controls_round_trip_validate_and_persist() {
        let options = Options::parse_gui(
            [
                "--deesser",
                "on",
                "--rumble-filter",
                "on",
                "--auto-level",
                "on",
                "--compressor",
                "on",
                "--level-target",
                "-18",
                "--level-max-gain",
                "6",
                "--level-floor",
                "-55",
                "--level-response",
                "1200",
                "--compress-threshold",
                "-10",
                "--compress-ratio",
                "4",
                "--compress-attack",
                "3",
                "--compress-release",
                "100",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut controls = Settings::from_options(&options);
        assert_eq!(controls.options().unwrap(), options);
        let path = std::env::temp_dir().join(format!("mofu-gpui-level-{}.ini", std::process::id()));
        gui_settings::save(&path, &controls.options().unwrap()).unwrap();
        assert_eq!(gui_settings::load(&path).unwrap(), options);
        controls.level.enabled = false;
        assert!(controls.options().unwrap().level.compressor_enabled);
        assert_eq!(
            controls.options().unwrap().level.values(),
            options.level.values()
        );
        controls.level.ratio = f64::NAN;
        assert!(controls.options().is_err());
        controls.level.ratio = 4.0;
        controls.level.floor_db = -15.0;
        assert!(controls.options().is_err());
        controls.level = LevelControls::from_settings(crate::level::LevelSettings::default());
        assert_eq!(
            controls.options().unwrap().level,
            crate::level::LevelSettings::default()
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn command_settings_use_engine_validation() {
        let options = Options::parse_gui(Vec::new()).unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.voice.pitch = f64::NAN;
        assert!(settings.options().is_err());
        settings.voice.pitch = 3.25;
        settings.voice.formant_shift = 2.0;
        settings.voice.preserve_formants = false;
        assert!(settings.options().is_err());
        settings.voice.preserve_formants = true;
        assert_eq!(settings.options().unwrap().pitch.semitones, 3.25);
        settings.devices.frames = 0;
        assert!(settings.options().is_err());
    }

    #[test]
    fn eq_controls_share_settings_across_modes_and_validate() {
        let options = Options::parse_gui(
            [
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
        let mut controls = Settings::from_options(&options);
        assert_eq!(controls.options().unwrap(), options);
        controls.eq.mode = "pro".into();
        assert_eq!(controls.options().unwrap().eq, options.eq);
        controls.eq.mode = "simple".into();
        assert_eq!(controls.options().unwrap().eq, options.eq);
        controls.eq.bands[1].q = f64::NAN;
        assert!(controls.options().is_err());
        controls.eq.bands[1].q = 1.0;
        controls.eq.output_db = f64::NAN;
        assert!(controls.options().is_err());
    }
    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn independent_filter_controls_preserve_parameters_and_validate_disabled_values() {
        let mut options = Options::default();
        options.voice_enabled = false;
        options.pitch.semitones = 5.0;
        options.rumble_filter = true;
        options.input_eq.bands[0].gain_db = -4.0;
        options.pre_conversion_eq.bands[0].gain_db = 2.0;
        options.eq.bands[0].gain_db = 1.0;
        options.compressor.threshold_db = -21.0;
        options.level.threshold_db = -9.0;
        options.limiter.ceiling_db = -6.0;
        options.dereverb.amount = 0.7;
        options.reverb.mix = 0.3;
        let controls = Settings::from_options(&options);
        assert_eq!(controls.options().unwrap(), options);
        let mut invalid = controls.clone();
        invalid.input_eq.bands[0].q = f64::NAN;
        assert!(invalid.options().is_err());
        let mut invalid = controls.clone();
        invalid.compressor.ratio = 0.0;
        assert!(invalid.options().is_err());
        let mut invalid = controls.clone();
        invalid.limiter.ceiling_db = 1.0;
        assert!(invalid.options().is_err());
        let mut invalid = controls.clone();
        invalid.dereverb.amount = 2.0;
        assert!(invalid.options().is_err());
        let mut invalid = controls;
        invalid.reverb.decay_ms = 0.0;
        assert!(invalid.options().is_err());
    }
}
