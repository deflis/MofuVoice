use std::error::Error;

#[derive(Debug, PartialEq)]
pub struct Options {
    pub device: String,
    pub seconds: Option<u64>,
    pub frames: u32,
    pub pitch: f64,
    pub preserve_formants: bool,
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
            list: false,
            inspect: false,
            help: false,
        };
        let mut args = args.into_iter();
        let mut device_given = false;
        let mut frames_given = false;
        let mut pitch_given = false;
        let mut formant_given = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
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
        let modes =
            usize::from(options.list) + usize::from(options.inspect) + usize::from(options.help);
        if modes > 1
            || (modes > 0 && (options.seconds.is_some() || pitch_given || formant_given))
            || ((options.list || options.help) && (device_given || frames_given))
        {
            return Err("Choose one CLI mode: --help, --list-devices, --inspect [--device NAME] [--frames N], or [--device NAME] [--frames N] [--seconds N] [--pitch SEMITONES] [--formant preserve|off]".into());
        }
        Ok(options)
    }
    pub fn parse_gui(args: impl IntoIterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let options = Self::parse(args)?;
        if options.seconds.is_some() || options.inspect || options.list {
            return Err("GUI accepts only --device NAME, --frames N, --pitch SEMITONES, --formant preserve|off, or --help".into());
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
