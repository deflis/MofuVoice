use crate::gate::{validate_time_ms, GateThreshold};
use std::error::Error;

#[derive(Debug, PartialEq)]
pub struct Options {
    pub input_eq: crate::eq::EqSettings,
    pub input_eq_mode: String,
    pub pre_conversion_eq: crate::eq::EqSettings,
    pub pre_conversion_eq_mode: String,
    pub voice_enabled: bool,
    pub compressor: crate::compressor::CompressorSettings,
    pub limiter: crate::limiter::LimiterSettings,
    pub dereverb: crate::dereverb::DereverbSettings,
    pub reverb: crate::reverb::ReverbSettings,
    pub eq: crate::eq::EqSettings,
    pub eq_mode: String,
    pub pitch_unit: String,
    pub device: String,
    pub output_device: String,
    pub preview_device: String,
    pub input_channel: usize,
    pub output_channels: Vec<usize>,
    pub backend: crate::backend::BackendKind,
    pub seconds: Option<u64>,
    pub frames: u32,
    pub sample_rate: u32,
    pub pitch: crate::pitch::PitchSettings,
    pub level: crate::level::LevelSettings,
    pub rumble_filter: bool,
    pub deesser: bool,
    pub deesser_pro: bool,
    pub deesser_parameters: crate::deesser::DeEsserParameters,
    pub gate: bool,
    pub gate_threshold: GateThreshold,
    pub gate_attack_ms: f64,
    pub gate_release_ms: f64,
    pub list: bool,
    pub inspect: bool,
    pub help: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            input_eq: crate::eq::EqSettings::default(),
            input_eq_mode: "simple".into(),
            pre_conversion_eq: crate::eq::EqSettings::default(),
            pre_conversion_eq_mode: "simple".into(),
            voice_enabled: true,
            compressor: crate::compressor::CompressorSettings::default(),
            limiter: crate::limiter::LimiterSettings::default(),
            dereverb: crate::dereverb::DereverbSettings::default(),
            reverb: crate::reverb::ReverbSettings::default(),
            eq: crate::eq::EqSettings::default(),
            eq_mode: "simple".into(),
            pitch_unit: "semitones".into(),
            device: String::new(),
            output_device: String::new(),
            preview_device: String::new(),
            input_channel: 1,
            output_channels: vec![1],
            backend: crate::backend::BackendKind::default(),
            seconds: None,
            frames: crate::DEFAULT_FRAMES,
            sample_rate: crate::RATE,
            pitch: crate::pitch::PitchSettings::default(),
            level: crate::level::LevelSettings::default(),
            rumble_filter: false,
            deesser: false,
            deesser_pro: false,
            deesser_parameters: crate::deesser::DeEsserParameters::default(),
            gate: false,
            gate_threshold: GateThreshold::Auto,
            gate_attack_ms: 5.0,
            gate_release_ms: 80.0,
            list: false,
            inspect: false,
            help: false,
        }
    }
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        let mut device_given = false;
        let mut output_given = false;
        let mut input_channel_given = false;
        let mut output_channels_given = false;
        let mut backend_given = false;
        let mut frames_given = false;
        let mut rate_given = false;
        let mut pitch_given = false;
        let mut algorithm_given = false;
        let mut precision_given = false;
        let mut estimator_given = false;
        let mut window_given = [false; 4];
        let mut formant_given = false;
        let mut shift_given = false;
        let mut filter_bands = std::collections::BTreeMap::new();
        let mut filters_given = std::collections::HashSet::new();
        let mut eq_given = std::collections::HashSet::new();
        let mut eq_bands = std::collections::BTreeMap::new();
        let mut level_given = false;
        let mut compressor_given = false;
        let mut level_fields = [false; 8];
        let mut rumble_given = false;
        let mut deesser_given = false;
        let mut deesser_mode_given = false;
        let mut deesser_fields = [false; 5];
        let mut gate_given = false;
        let mut gate_threshold_given = false;
        let mut gate_attack_given = false;
        let mut gate_release_given = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                flag if crate::filter_settings::is_flag(flag) => {
                    if !filters_given.insert(flag.to_string()) {
                        return Err(format!("Repeated argument {flag}").into());
                    }
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} requires a value"))?;
                    if let Some((prefix, number)) = flag.rsplit_once("-eq-band") {
                        filter_bands.insert((prefix.to_string(), number.parse::<usize>()?), value);
                    } else {
                        crate::filter_settings::set(&mut options, flag, &value)?;
                    }
                }
                flag if crate::eq::is_flag(flag) => {
                    if !eq_given.insert(flag.to_string()) {
                        return Err(format!("Repeated argument {flag}").into());
                    }
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} requires a value"))?;
                    match flag {
                        "--eq" => {
                            options.eq.enabled = match value.as_str() {
                                "on" => true,
                                "off" => false,
                                _ => return Err("--eq requires on or off".into()),
                            }
                        }
                        "--eq-mode" => {
                            if !["simple", "pro"].contains(&value.as_str()) {
                                return Err("--eq-mode requires simple or pro".into());
                            }
                            options.eq_mode = value;
                        }
                        "--eq-output" => options.eq.output_db = value.parse()?,
                        _ => {
                            eq_bands.insert(
                                crate::eq::band_index(flag).unwrap(),
                                crate::eq::Band::parse(&value)?,
                            );
                        }
                    }
                }
                "--formant-shift" if !shift_given => {
                    options.pitch.formant_shift = args
                        .next()
                        .ok_or("--formant-shift requires semitones")?
                        .parse()?;
                    crate::pitch::formant_ratio(options.pitch.formant_shift)
                        .map_err(|error| error.to_string())?;
                    shift_given = true;
                }
                "--formant" if !formant_given => {
                    options.pitch.preserve_formants = match args.next().as_deref() {
                        Some("preserve") => true,
                        Some("off") => false,
                        _ => return Err("--formant requires preserve or off".into()),
                    };
                    formant_given = true;
                }
                flag @ ("--analysis-window"
                | "--synthesis-window"
                | "--psola-analysis-window"
                | "--psola-synthesis-window") => {
                    let psola = flag.starts_with("--psola-");
                    let synthesis = flag.ends_with("synthesis-window");
                    let index = 2 * usize::from(psola) + usize::from(synthesis);
                    if window_given[index] {
                        return Err(format!("Repeated argument {flag}").into());
                    }
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} requires samples"))?
                        .parse()?;
                    let windows = if psola {
                        &mut options.pitch.psola_windows
                    } else {
                        &mut options.pitch.windows
                    };
                    if synthesis {
                        windows.synthesis = value;
                    } else {
                        windows.analysis = value;
                    }
                    window_given[index] = true;
                }
                "--math-precision" if !precision_given => {
                    options.pitch.math_precision = crate::pitch::MathPrecision::parse(
                        &args
                            .next()
                            .ok_or("--math-precision requires standard or accurate")?,
                    )?;
                    precision_given = true;
                }
                "--psola-pitch-estimator" if !estimator_given => {
                    options.pitch.psola_pitch_estimator =
                        crate::pitch::PitchEstimator::parse(&args.next().ok_or(
                            "--psola-pitch-estimator requires yin, mpm, mpm-fft or nccf",
                        )?)?;
                    estimator_given = true;
                }
                "--pitch-algorithm" if !algorithm_given => {
                    options.pitch.algorithm =
                        crate::pitch::PitchAlgorithm::parse(&args.next().ok_or(
                            "--pitch-algorithm requires phase-locked, pvsola, pvsola-sync, td-psola, td-psola-resampled or fd-psola",
                        )?)?;
                    algorithm_given = true;
                }
                "--pitch" if !pitch_given => {
                    options.pitch.semitones =
                        args.next().ok_or("--pitch requires semitones")?.parse()?;
                    crate::pitch::pitch_ratio(options.pitch.semitones)?;
                    pitch_given = true;
                }
                "--auto-level" if !level_given => {
                    options.level.enabled = match args.next().as_deref() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => return Err("--auto-level requires on or off".into()),
                    };
                    level_given = true;
                }
                "--compressor" if !compressor_given => {
                    options.level.compressor_enabled = match args.next().as_deref() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => return Err("--compressor requires on or off".into()),
                    };
                    compressor_given = true;
                }
                flag if crate::level::FLAGS.contains(&flag) => {
                    let index = crate::level::FLAGS.iter().position(|f| *f == flag).unwrap();
                    if level_fields[index] {
                        return Err(format!("Repeated argument {flag}").into());
                    }
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} requires a value"))?
                        .parse()?;
                    match flag {
                        "--level-target" => options.level.target_db = value,
                        "--level-max-gain" => options.level.max_gain_db = value,
                        "--level-floor" => options.level.floor_db = value,
                        "--level-response" => options.level.response_ms = value,
                        "--compress-threshold" => options.level.threshold_db = value,
                        "--compress-ratio" => options.level.ratio = value,
                        "--compress-attack" => options.level.attack_ms = value,
                        "--compress-release" => options.level.release_ms = value,
                        _ => unreachable!(),
                    }
                    level_fields[index] = true;
                }
                "--deesser-mode" if !deesser_mode_given => {
                    options.deesser_pro = match args.next().as_deref() {
                        Some("simple") => false,
                        Some("pro") => true,
                        _ => return Err("--deesser-mode requires simple or pro".into()),
                    };
                    deesser_mode_given = true;
                }
                flag if crate::deesser::FLAGS.contains(&flag) => {
                    let index = crate::deesser::FLAGS
                        .iter()
                        .position(|f| *f == flag)
                        .unwrap();
                    if deesser_fields[index] {
                        return Err(format!("Repeated argument {flag}").into());
                    }
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} requires a value"))?
                        .parse()?;
                    match flag {
                        "--deesser-frequency" => options.deesser_parameters.frequency_hz = value,
                        "--deesser-threshold" => options.deesser_parameters.threshold_db = value,
                        "--deesser-range" => options.deesser_parameters.range_db = value,
                        "--deesser-attack" => options.deesser_parameters.attack_ms = value,
                        "--deesser-release" => options.deesser_parameters.release_ms = value,
                        _ => unreachable!(),
                    }
                    deesser_fields[index] = true;
                }
                "--deesser" if !deesser_given => {
                    options.deesser = match args.next().as_deref() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => return Err("--deesser requires on or off".into()),
                    };
                    deesser_given = true;
                }
                "--rumble-filter" if !rumble_given => {
                    options.rumble_filter = match args.next().as_deref() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => return Err("--rumble-filter requires on or off".into()),
                    };
                    rumble_given = true;
                }
                "--gate" if !gate_given => {
                    options.gate = match args.next().as_deref() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => return Err("--gate requires on or off".into()),
                    };
                    gate_given = true;
                }
                "--gate-threshold" if !gate_threshold_given => {
                    let value = args
                        .next()
                        .ok_or("--gate-threshold requires auto or a dB value")?;
                    options.gate_threshold = GateThreshold::parse(&value)?;
                    gate_threshold_given = true;
                }
                "--gate-attack" if !gate_attack_given => {
                    options.gate_attack_ms = args
                        .next()
                        .ok_or("--gate-attack requires milliseconds")?
                        .parse()?;
                    validate_time_ms(options.gate_attack_ms)?;
                    gate_attack_given = true;
                }
                "--gate-release" if !gate_release_given => {
                    options.gate_release_ms = args
                        .next()
                        .ok_or("--gate-release requires milliseconds")?
                        .parse()?;
                    validate_time_ms(options.gate_release_ms)?;
                    gate_release_given = true;
                }
                "--input-channel" if !input_channel_given => {
                    options.input_channel = args
                        .next()
                        .ok_or("--input-channel requires a channel")?
                        .parse()?;
                    input_channel_given = true;
                }
                "--output-channels" if !output_channels_given => {
                    options.output_channels = args
                        .next()
                        .ok_or("--output-channels requires comma-separated channels")?
                        .split(',')
                        .map(str::parse)
                        .collect::<Result<Vec<usize>, _>>()?;
                    output_channels_given = true;
                }
                "--backend" if !backend_given => {
                    options.backend = crate::backend::BackendKind::parse(
                        &args.next().ok_or("--backend requires auto, cpal or asio")?,
                    )?;
                    backend_given = true;
                }
                "--preview-device" if options.preview_device.is_empty() => {
                    options.preview_device = args
                        .next()
                        .ok_or("--preview-device requires a device name")?;
                    if options.preview_device.is_empty() || options.preview_device.starts_with("--")
                    {
                        return Err("--preview-device requires a device name".into());
                    }
                }
                "--output-device" if !output_given => {
                    options.output_device = args
                        .next()
                        .ok_or("--output-device requires a device name")?;
                    if options.output_device.is_empty() || options.output_device.starts_with("--") {
                        return Err("--output-device requires a device name".into());
                    }
                    output_given = true;
                }
                "--device" if !device_given => {
                    options.device = args
                        .next()
                        .ok_or("--device requires an exact audio device name")?;
                    if options.device.is_empty() || options.device.starts_with("--") {
                        return Err("--device requires an exact audio device name".into());
                    }
                    device_given = true;
                }
                "--sample-rate" if !rate_given => {
                    options.sample_rate =
                        args.next().ok_or("--sample-rate requires Hz")?.parse()?;
                    crate::validate_sample_rate(options.sample_rate)?;
                    rate_given = true;
                }
                "--frames" if !frames_given => {
                    let frames = args
                        .next()
                        .ok_or("--frames requires a positive integer")?
                        .parse::<u32>()?;
                    if frames == 0 || frames > 8192 {
                        return Err("--frames must be between 1 and 8192".into());
                    }
                    options.frames = frames;
                    frames_given = true;
                }
                "--seconds" if options.seconds.is_none() => {
                    let value = args
                        .next()
                        .ok_or("--seconds requires a positive integer")?
                        .parse::<u64>()?;
                    if value == 0 {
                        return Err("--seconds must be positive".into());
                    }
                    options.seconds = Some(value);
                }
                "--list-devices" if !options.list => options.list = true,
                "--inspect" if !options.inspect => options.inspect = true,
                "--help" if !options.help => options.help = true,
                _ => return Err(format!("Unknown or repeated argument {arg}; use --help").into()),
            }
        }
        for (index, band) in eq_bands {
            if index < options.eq.bands.len() {
                options.eq.bands[index] = band;
            } else if index == options.eq.bands.len() {
                options.eq.bands.push(band);
            } else {
                return Err("EQ bands must be contiguous".into());
            }
        }
        for ((prefix, index), value) in filter_bands {
            crate::filter_settings::set(&mut options, &format!("{prefix}-eq-band{index}"), &value)?;
        }
        options.validate()?;
        options.output_channels.sort_unstable();
        let gate_tuned = gate_threshold_given || gate_attack_given || gate_release_given;
        let modes =
            usize::from(options.list) + usize::from(options.inspect) + usize::from(options.help);
        if modes > 1
            || (modes > 0
                && (options.seconds.is_some()
                    || pitch_given
                    || estimator_given
                    || algorithm_given
                    || precision_given
                    || window_given.iter().any(|given| *given)
                    || formant_given
                    || shift_given
                    || !filters_given.is_empty()
                    || !eq_given.is_empty()
                    || level_given
                    || compressor_given
                    || level_fields.iter().any(|given| *given)
                    || rumble_given
                    || deesser_given
                    || deesser_mode_given
                    || deesser_fields.iter().any(|given| *given)
                    || gate_given
                    || gate_tuned))
            || (options.help && backend_given)
            || ((options.list || options.help)
                && (device_given
                    || output_given
                    || frames_given
                    || rate_given
                    || input_channel_given
                    || output_channels_given))
        {
            return Err("Choose one CLI mode: --help, --list-devices, --inspect [--device NAME] [--frames N] [--sample-rate HZ], or [--device NAME] [--frames N] [--sample-rate HZ] [--seconds N] [--pitch SEMITONES] [--math-precision standard|accurate] [--analysis-window 1024|2048|4096|8192] [--synthesis-window 256|512|1024|2048] [--psola-pitch-estimator yin|mpm|mpm-fft|nccf] [--psola-analysis-window 1024|2048|4096|8192] [--psola-synthesis-window 256|512|1024|2048] [--pitch-algorithm phase-locked|pvsola|pvsola-sync|td-psola|td-psola-resampled|fd-psola] [--formant preserve|off] [--formant-shift SEMITONES] [--eq on|off] [--eq-mode simple|pro] [--eq-bandN HZ,DB,Q] [--eq-output DB] [--auto-level on|off] [--compressor on|off] [--level-target DB] [--level-max-gain DB] [--level-floor DB] [--level-response MS] [--compress-threshold DB] [--compress-ratio N] [--compress-attack MS] [--compress-release MS] [--deesser on|off] [--deesser-mode simple|pro] [--deesser-frequency HZ] [--deesser-threshold DBFS] [--deesser-range DB] [--deesser-attack MS] [--deesser-release MS] [--rumble-filter on|off] [--gate on|off] [--gate-threshold auto|DB] [--gate-attack MS] [--gate-release MS]".into());
        }
        Ok(options)
    }
    /// Validate typed settings independently of CLI argument parsing.
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        crate::validate_sample_rate(self.sample_rate)?;
        if !(1..=8192).contains(&self.frames) {
            return Err("--frames must be between 1 and 8192".into());
        }
        self.pitch.windows.validate()?;
        self.pitch.psola_windows.validate()?;
        crate::pitch::pitch_ratio(self.pitch.semitones)?;
        crate::pitch::formant_ratio(self.pitch.formant_shift)?;
        self.pitch
            .algorithm
            .validate_formants(self.pitch.preserve_formants)?;
        if !self.pitch.preserve_formants && self.pitch.formant_shift != 0.0 {
            return Err(crate::pitch::InvalidFormant.into());
        }
        self.eq.validate()?;
        self.input_eq.validate()?;
        self.pre_conversion_eq.validate()?;
        self.compressor.validate()?;
        self.limiter.validate()?;
        self.dereverb.validate()?;
        self.reverb.validate()?;
        for mode in [&self.input_eq_mode, &self.pre_conversion_eq_mode] {
            if !["simple", "pro"].contains(&mode.as_str()) {
                return Err("EQ mode requires simple or pro".into());
            }
        }
        if !["semitones", "percent"].contains(&self.pitch_unit.as_str()) {
            return Err("Pitch display unit requires semitones or percent".into());
        }
        if !["simple", "pro"].contains(&self.eq_mode.as_str()) {
            return Err("EQ mode requires simple or pro".into());
        }
        self.level.validate()?;
        self.deesser_parameters.validate()?;
        if let GateThreshold::Db(db) = self.gate_threshold {
            if !db.is_finite() || !(-90.0..=0.0).contains(&db) {
                return Err(crate::gate::InvalidGate.into());
            }
        }
        validate_time_ms(self.gate_attack_ms)?;
        validate_time_ms(self.gate_release_ms)?;
        if !(1..=512).contains(&self.input_channel)
            || self.output_channels.is_empty()
            || self
                .output_channels
                .iter()
                .any(|channel| !(1..=512).contains(channel))
            || self
                .output_channels
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != self.output_channels.len()
        {
            return Err(
                "Select one input and at least one distinct output channel (1..512)".into(),
            );
        }
        if self.backend == crate::backend::BackendKind::Asio && !self.preview_device.is_empty() {
            return Err("Preview device is only supported by CPAL".into());
        }
        if self.preview_device.contains(['\r', '\n', '\0']) {
            return Err("Device name cannot contain line breaks or NUL".into());
        }
        if self.backend == crate::backend::BackendKind::Asio && !self.output_device.is_empty() {
            return Err("ASIO uses one duplex device; --output-device is for CPAL".into());
        }
        if self.device.contains(['\r', '\n', '\0'])
            || self.output_device.contains(['\r', '\n', '\0'])
        {
            return Err("Device names cannot contain line breaks or NUL".into());
        }
        Ok(())
    }
    pub fn parse_gui(args: impl IntoIterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let options = Self::parse(args)?;
        if options.seconds.is_some() || options.inspect || options.list {
            return Err("GUI accepts device, buffer, pitch, formant, rumble filter, de-esser, gate, EQ, auto-level and compressor settings, or --help. Use the CLI for timed runs, inspection or device listing.".into());
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn selectable_pitch_algorithms_are_strict_and_default_to_phase_locked() {
        use crate::pitch::PitchAlgorithm;
        assert_eq!(
            super::Options::default().pitch.algorithm,
            PitchAlgorithm::PhaseLocked
        );
        for algorithm in PitchAlgorithm::ALL {
            for options in [
                super::Options::parse(["--pitch-algorithm", algorithm.name()].map(String::from)),
                super::Options::parse_gui(
                    ["--pitch-algorithm", algorithm.name()].map(String::from),
                ),
            ] {
                assert_eq!(options.unwrap().pitch.algorithm, algorithm);
            }
        }
        for args in [
            vec!["--pitch-algorithm"],
            vec!["--pitch-algorithm", "unknown"],
            vec![
                "--pitch-algorithm",
                "pvsola",
                "--pitch-algorithm",
                "phase-locked",
            ],
            vec!["--pitch-algorithm", "pvsola", "--inspect"],
            vec!["--pitch-algorithm", "pvsola", "--help"],
        ] {
            assert!(super::Options::parse(args.into_iter().map(String::from)).is_err());
        }
    }
    use super::*;
    fn parse(args: &[&str]) -> Result<Options, Box<dyn Error>> {
        Options::parse(args.iter().map(|s| (*s).into()))
    }
    #[test]
    fn sample_rate_defaults_selection_and_invalid_arguments() {
        assert_eq!(parse(&[]).unwrap().sample_rate, crate::RATE);
        for rate in crate::SAMPLE_RATES {
            let value = rate.to_string();
            assert_eq!(parse(&["--sample-rate", &value]).unwrap().sample_rate, rate);
            assert!(parse(&["--inspect", "--sample-rate", &value]).is_ok());
            assert!(Options::parse_gui(["--sample-rate", &value].map(String::from)).is_ok());
        }
        for args in [
            vec!["--sample-rate"],
            vec!["--sample-rate", "0"],
            vec!["--sample-rate", "32000"],
            vec!["--sample-rate", "88200"],
            vec!["--sample-rate", "96000"],
            vec!["--sample-rate", "176400"],
            vec!["--sample-rate", "192000"],
            vec!["--sample-rate", "NaN"],
            vec!["--sample-rate", "48000.5"],
            vec!["--sample-rate", "44100", "--sample-rate", "48000"],
            vec!["--help", "--sample-rate", "48000"],
            vec!["--list-devices", "--sample-rate", "48000"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn channel_routing_defaults_and_validation() {
        let defaults = parse(&[]).unwrap();
        assert_eq!(defaults.input_channel, 1);
        assert_eq!(defaults.output_channels, [1]);
        let routed = parse(&["--input-channel", "2", "--output-channels", "3,1"]).unwrap();
        assert_eq!(routed.input_channel, 2);
        assert_eq!(routed.output_channels, [1, 3]);
        for args in [
            vec!["--input-channel", "0"],
            vec!["--input-channel", "513"],
            vec!["--input-channel", "-1"],
            vec!["--input-channel"],
            vec!["--output-channels", ""],
            vec!["--output-channels", "1,1"],
            vec!["--output-channels", "1,0"],
            vec!["--output-channels", "1,513"],
            vec!["--output-channels", "1,"],
            vec!["--output-channels", "1.5"],
            vec!["--output-channels", "1", "--output-channels", "2"],
            vec!["--input-channel", "1", "--help"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn backend_output_selection_defaults_and_validation() {
        assert_eq!(
            parse(&[]).unwrap().backend,
            crate::backend::BackendKind::Auto
        );
        let options = parse(&[
            "--backend",
            "cpal",
            "--device",
            "Mic",
            "--output-device",
            "Headphones",
        ])
        .unwrap();
        assert_eq!(options.output_device, "Headphones");
        assert!(parse(&["--backend", "asio", "--output-device", "Headphones"]).is_err());
        assert!(parse(&["--backend", "unknown"]).is_err());
        assert!(parse(&["--backend", "cpal", "--backend", "auto"]).is_err());
        assert!(parse(&["--output-device", "A", "--output-device", "B"]).is_err());
        assert!(parse(&["--output-device", "--frames"]).is_err());
        assert!(parse(&["--device", "bad\nname"]).is_err());
        assert!(parse(&["--list-devices", "--backend", "cpal"]).is_ok());
    }
    #[test]
    fn automatic_level_and_custom_parameters_are_validated() {
        assert!(!parse(&[]).unwrap().level.enabled);
        assert!(!parse(&[]).unwrap().level.compressor_enabled);
        let compressor = parse(&["--compressor", "on"]).unwrap();
        assert!(compressor.level.compressor_enabled);
        assert!(!compressor.level.enabled);
        let options = parse(&[
            "--auto-level",
            "on",
            "--level-target",
            "-18",
            "--compress-ratio",
            "4",
        ])
        .unwrap();
        assert!(options.level.enabled);
        assert!(!options.level.compressor_enabled);
        assert_eq!(options.level.target_db, -18.0);
        assert_eq!(options.level.ratio, 4.0);
        for args in [
            vec!["--auto-level"],
            vec!["--compressor"],
            vec!["--compressor", "maybe"],
            vec!["--compressor", "on", "--compressor", "off"],
            vec!["--compressor", "on", "--inspect"],
            vec!["--compressor", "off", "--help"],
            vec!["--auto-level", "maybe"],
            vec!["--auto-level", "on", "--auto-level", "off"],
            vec!["--compress-ratio", "3", "--compress-ratio", "4"],
            vec!["--level-target", "NaN"],
            vec!["--compress-attack", "0"],
            vec!["--level-floor", "-30", "--level-target", "-36"],
            vec!["--auto-level", "off", "--inspect"],
            vec!["--level-max-gain", "6", "--help"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(
            Options::parse_gui(["--auto-level", "on"].map(String::from))
                .unwrap()
                .level
                .enabled
        );
    }
    #[test]
    fn validates_independent_formant_shift() {
        for shift in ["-12", "-2.5", "0", "+3", "12"] {
            let options = parse(&["--formant-shift", shift]).unwrap();
            assert_eq!(options.pitch.formant_shift, shift.parse::<f64>().unwrap());
            assert_eq!(options.pitch.semitones, 0.0);
            assert!(
                Options::parse_gui(["--formant-shift", shift].into_iter().map(String::from))
                    .is_ok()
            );
        }
        for args in [
            vec!["--formant-shift"],
            vec!["--formant-shift", "NaN"],
            vec!["--formant-shift", "inf"],
            vec!["--formant-shift", "12.1"],
            vec!["--formant-shift", "-12.1"],
            vec!["--formant-shift", "3", "--formant", "off"],
            vec!["--formant", "off", "--formant-shift", "3"],
            vec!["--formant-shift", "3", "--formant-shift", "2"],
            vec!["--formant-shift", "0", "--help"],
            vec!["--formant-shift", "0", "--inspect"],
            vec!["--formant-shift", "0", "--list-devices"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(parse(&["--formant", "off", "--formant-shift", "0"]).is_ok());
    }

    #[test]
    fn rumble_filter_defaults_and_validation() {
        assert!(!parse(&[]).unwrap().rumble_filter);
        assert!(parse(&["--rumble-filter", "on"]).unwrap().rumble_filter);
        assert!(!parse(&["--rumble-filter", "off"]).unwrap().rumble_filter);
        for args in [
            vec!["--rumble-filter"],
            vec!["--rumble-filter", "maybe"],
            vec!["--rumble-filter", "on", "--rumble-filter", "off"],
            vec!["--rumble-filter", "off", "--help"],
            vec!["--rumble-filter", "on", "--inspect"],
            vec!["--rumble-filter", "on", "--list-devices"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(
            Options::parse_gui(["--rumble-filter", "on"].map(String::from))
                .unwrap()
                .rumble_filter
        );
    }
    #[test]
    fn deesser_mode_and_parameters_validate() {
        let options = parse(&[
            "--deesser-mode",
            "pro",
            "--deesser-frequency",
            "6500",
            "--deesser-range",
            "3",
        ])
        .unwrap();
        assert!(options.deesser_pro);
        assert_eq!(options.deesser_parameters.frequency_hz, 6500.0);
        assert_eq!(options.deesser_parameters.range_db, 3.0);
        assert!(!parse(&[]).unwrap().deesser_pro);
        for args in [
            vec!["--deesser-mode", "invalid"],
            vec!["--deesser-mode"],
            vec!["--deesser-mode", "pro", "--deesser-mode", "simple"],
            vec!["--deesser-range", "3", "--deesser-range", "6"],
            vec!["--deesser-mode", "pro", "--help"],
        ] {
            assert!(parse(&args).is_err());
        }
        for flag in crate::deesser::FLAGS {
            assert!(parse(&[flag]).is_err());
            assert!(parse(&[flag, "NaN"]).is_err());
            assert!(parse(&[flag, "999999"]).is_err());
        }
    }
    #[test]
    fn deesser_defaults_and_validation() {
        assert!(!parse(&[]).unwrap().deesser);
        assert!(parse(&["--deesser", "on"]).unwrap().deesser);
        assert!(!parse(&["--deesser", "off"]).unwrap().deesser);
        for args in [
            vec!["--deesser"],
            vec!["--deesser", "maybe"],
            vec!["--deesser", "on", "--deesser", "off"],
            vec!["--deesser", "off", "--help"],
            vec!["--deesser", "on", "--inspect"],
            vec!["--deesser", "on", "--list-devices"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(
            Options::parse_gui(["--deesser", "on"].map(String::from))
                .unwrap()
                .deesser
        );
    }
    #[test]
    fn parses_gate_options() {
        let options = parse(&[
            "--gate",
            "on",
            "--gate-threshold",
            "-35",
            "--gate-attack",
            "3",
            "--gate-release",
            "120",
        ])
        .unwrap();
        assert!(options.gate);
        assert_eq!(options.gate_threshold, GateThreshold::Db(-35.0));
        assert_eq!(options.gate_attack_ms, 3.0);
        assert_eq!(options.gate_release_ms, 120.0);
        assert_eq!(parse(&[]).unwrap().gate_threshold, GateThreshold::Auto);
        assert!(!parse(&[]).unwrap().gate);
        for args in [
            vec!["--gate"],
            vec!["--gate", "maybe"],
            vec!["--gate-threshold"],
            vec!["--gate-threshold", "loud"],
            vec!["--gate-attack", "0"],
            vec!["--gate-release", "NaN"],
            vec!["--gate", "on", "--help"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn explicit_device_name_and_timed_run() {
        let options = parse(&["--seconds", "60", "--device", "Example ASIO Device"]).unwrap();
        assert_eq!(options.device, "Example ASIO Device");
        assert_eq!(options.seconds, Some(60));
        assert_eq!(options.frames, 512);
        assert_eq!(
            parse(&[
                "--device",
                "Example ASIO Device",
                "--frames",
                "512",
                "--seconds",
                "60"
            ])
            .unwrap()
            .frames,
            512
        );
        assert!(!options.inspect);
        assert!(
            parse(&["--device", "Example ASIO Device", "--inspect"])
                .unwrap()
                .inspect
        );
    }
    #[test]
    fn rejects_ambiguous_or_invalid_modes() {
        for args in [
            vec!["--seconds", "0"],
            vec!["--device"],
            vec!["--device", "--inspect"],
            vec!["--inspect", "--seconds", "1"],
            vec!["--list-devices", "--inspect"],
            vec!["--device", "A", "--device", "B"],
            vec!["--seconds", "1", "--seconds", "2"],
            vec!["--frames", "0"],
            vec!["--frames", "8193"],
            vec!["--frames", "512", "--frames", "64"],
            vec!["--formant"],
            vec!["--formant", "invalid"],
            vec!["--formant", "off", "--formant", "preserve"],
            vec!["--formant", "preserve", "--inspect"],
            vec!["--formant", "off", "--help"],
            vec!["--formant", "off", "--list-devices"],
            vec!["--pitch"],
            vec!["--pitch", "NaN"],
            vec!["--pitch", "inf"],
            vec!["--pitch", "12.1"],
            vec!["--pitch", "-12.1"],
            vec!["--pitch", "3", "--pitch", "-3"],
            vec!["--pitch", "3", "--inspect"],
            vec!["--pitch", "3", "--list-devices"],
            vec!["--pitch", "3", "--help"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn accepts_signed_fractional_pitch_and_defaults_to_bypass() {
        assert!(parse(&[]).unwrap().device.is_empty());
        assert_eq!(parse(&[]).unwrap().pitch.semitones, 0.0);
        assert!(parse(&[]).unwrap().pitch.preserve_formants);
        assert!(
            parse(&["--pitch", "3", "--formant", "preserve"])
                .unwrap()
                .pitch
                .preserve_formants
        );
        assert!(
            !parse(&["--pitch", "-3", "--formant", "off"])
                .unwrap()
                .pitch
                .preserve_formants
        );
        for pitch in ["-12", "-3", "0", "2.5", "+3", "12"] {
            assert_eq!(
                parse(&["--pitch", pitch]).unwrap().pitch.semitones,
                pitch.parse::<f64>().unwrap()
            );
        }
    }
    #[test]
    fn gui_accepts_initial_settings_and_rejects_other_modes() {
        let options = Options::parse_gui(
            ["--pitch", "-3", "--formant", "off", "--frames", "512"]
                .into_iter()
                .map(String::from),
        )
        .unwrap();
        assert_eq!(options.pitch.semitones, -3.0);
        assert!(!options.pitch.preserve_formants);
        for args in [
            vec!["--gui", "--gui"],
            vec!["--gui", "--inspect"],
            vec!["--gui", "--list-devices"],
            vec!["--gui", "--help"],
            vec!["--gui", "--seconds", "1"],
        ] {
            assert!(
                Options::parse_gui(args.iter().map(|arg| (*arg).into())).is_err(),
                "{args:?}"
            );
        }
        for args in [
            vec!["--inspect"],
            vec!["--list-devices"],
            vec!["--seconds", "1"],
        ] {
            assert!(
                Options::parse_gui(args.iter().map(|arg| (*arg).into())).is_err(),
                "{args:?}"
            );
        }
        assert!(Options::parse_gui(Vec::<String>::new())
            .unwrap()
            .device
            .is_empty());
        assert!(Options::parse_gui(["--help".into()]).unwrap().help);
        assert!(parse(&["--gui"]).is_err());
    }
}
