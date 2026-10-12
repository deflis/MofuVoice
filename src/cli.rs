use crate::gate::{validate_time_ms, GateThreshold};
use std::error::Error;

#[derive(Debug, PartialEq)]
pub struct Options {
    pub device: String,
    pub seconds: Option<u64>,
    pub frames: u32,
    pub pitch: f64,
    pub preserve_formants: bool,
    pub formant_shift: f64,
    pub level: crate::level::LevelSettings,
    pub gate: bool,
    pub gate_threshold: GateThreshold,
    pub gate_attack_ms: f64,
    pub gate_release_ms: f64,
    pub list: bool,
    pub inspect: bool,
    pub help: bool,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut options = Self {
            device: String::new(),
            seconds: None,
            frames: crate::DEFAULT_FRAMES,
            pitch: 0.0,
            preserve_formants: true,
            formant_shift: 0.0,
            level: crate::level::LevelSettings::default(),
            gate: false,
            gate_threshold: GateThreshold::Auto,
            gate_attack_ms: 5.0,
            gate_release_ms: 80.0,
            list: false,
            inspect: false,
            help: false,
        };
        let mut args = args.into_iter();
        let mut device_given = false;
        let mut frames_given = false;
        let mut pitch_given = false;
        let mut formant_given = false;
        let mut shift_given = false;
        let mut level_given = false;
        let mut level_fields = [false; 8];
        let mut gate_given = false;
        let mut gate_threshold_given = false;
        let mut gate_attack_given = false;
        let mut gate_release_given = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--formant-shift" if !shift_given => {
                    options.formant_shift = args
                        .next()
                        .ok_or("--formant-shift requires semitones")?
                        .parse()?;
                    crate::pitch::formant_ratio(options.formant_shift)
                        .map_err(|error| error.to_string())?;
                    shift_given = true;
                }
                "--formant" if !formant_given => {
                    options.preserve_formants = match args.next().as_deref() {
                        Some("preserve") => true,
                        Some("off") => false,
                        _ => return Err("--formant requires preserve or off".into()),
                    };
                    formant_given = true;
                }
                "--pitch" if !pitch_given => {
                    options.pitch = args.next().ok_or("--pitch requires semitones")?.parse()?;
                    crate::pitch::pitch_ratio(options.pitch)?;
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
                flag if crate::level::FLAGS.contains(&flag) => {
                    let index = crate::level::FLAGS.iter().position(|f| *f == flag).unwrap();
                    if level_fields[index] {
                        return Err(format!("Repeated argument {flag}").into());
                    }
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} requires a value"))?
                        .parse()?;
                    options.level.set(index, value);
                    level_fields[index] = true;
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
                "--device" if !device_given => {
                    options.device = args
                        .next()
                        .ok_or("--device requires an exact ASIO device name")?;
                    if options.device.is_empty() || options.device.starts_with("--") {
                        return Err("--device requires an exact ASIO device name".into());
                    }
                    device_given = true;
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
        if !options.preserve_formants && options.formant_shift != 0.0 {
            return Err(crate::pitch::InvalidFormant.to_string().into());
        }
        options.level.validate()?;
        let gate_tuned = gate_threshold_given || gate_attack_given || gate_release_given;
        let modes =
            usize::from(options.list) + usize::from(options.inspect) + usize::from(options.help);
        if modes > 1
            || (modes > 0
                && (options.seconds.is_some()
                    || pitch_given
                    || formant_given
                    || shift_given
                    || level_given
                    || level_fields.iter().any(|given| *given)
                    || gate_given
                    || gate_tuned))
            || ((options.list || options.help) && (device_given || frames_given))
        {
            return Err("Choose one CLI mode: --help, --list-devices, --inspect [--device NAME] [--frames N], or [--device NAME] [--frames N] [--seconds N] [--pitch SEMITONES] [--formant preserve|off] [--formant-shift SEMITONES] [--auto-level on|off] [--level-target DB] [--level-max-gain DB] [--level-floor DB] [--level-response MS] [--compress-threshold DB] [--compress-ratio N] [--compress-attack MS] [--compress-release MS] [--gate on|off] [--gate-threshold auto|DB] [--gate-attack MS] [--gate-release MS]".into());
        }
        Ok(options)
    }
    pub fn parse_gui(args: impl IntoIterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let options = Self::parse(args)?;
        if options.seconds.is_some() || options.inspect || options.list {
            return Err("GUI accepts device, buffer, pitch, formant, gate and auto-level settings, or --help. Use the CLI for timed runs, inspection or device listing.".into());
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Options, Box<dyn Error>> {
        Options::parse(args.iter().map(|s| (*s).into()))
    }
    #[test]
    fn automatic_level_and_custom_parameters_are_validated() {
        assert!(!parse(&[]).unwrap().level.enabled);
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
        assert_eq!(options.level.target_db, -18.0);
        assert_eq!(options.level.ratio, 4.0);
        for args in [
            vec!["--auto-level"],
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
            assert_eq!(options.formant_shift, shift.parse::<f64>().unwrap());
            assert_eq!(options.pitch, 0.0);
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
        assert_eq!(parse(&[]).unwrap().pitch, 0.0);
        assert!(parse(&[]).unwrap().preserve_formants);
        assert!(
            parse(&["--pitch", "3", "--formant", "preserve"])
                .unwrap()
                .preserve_formants
        );
        assert!(
            !parse(&["--pitch", "-3", "--formant", "off"])
                .unwrap()
                .preserve_formants
        );
        for pitch in ["-12", "-3", "0", "2.5", "+3", "12"] {
            assert_eq!(
                parse(&["--pitch", pitch]).unwrap().pitch,
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
        assert_eq!(options.pitch, -3.0);
        assert!(!options.preserve_formants);
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
