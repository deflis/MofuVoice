//! GUI preferences only; never read or written from an audio callback.
use crate::{cli::Options, gate::GateThreshold};
#[cfg(feature = "desktop")]
use std::path::PathBuf;
use std::{error::Error, fs, io::ErrorKind, path::Path};

#[cfg(feature = "desktop")]
pub(crate) fn path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(std::env::current_exe()
        .map_err(|error| format!("Cannot locate GUI executable: {error}"))?
        .with_file_name("settings.ini"))
}

fn threshold_text(threshold: GateThreshold) -> String {
    match threshold {
        GateThreshold::Auto => "auto".into(),
        GateThreshold::Db(db) => db.to_string(),
    }
}

fn arguments(options: &Options) -> Vec<String> {
    let mut args = vec![
        "--input-channel".into(),
        options.input_channel.to_string(),
        "--output-channels".into(),
        options
            .output_channels
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        "--backend".into(),
        options.backend.name().into(),
        "--sample-rate".into(),
        options.sample_rate.to_string(),
        "--frames".into(),
        options.frames.to_string(),
        "--pitch".into(),
        options.pitch.semitones.to_string(),
        "--pitch-algorithm".into(),
        options.pitch.algorithm.name().into(),
        "--formant-shift".into(),
        options.pitch.formant_shift.to_string(),
        "--formant".into(),
        if options.pitch.preserve_formants {
            "preserve"
        } else {
            "off"
        }
        .into(),
        "--deesser".into(),
        if options.deesser { "on" } else { "off" }.into(),
        "--rumble-filter".into(),
        if options.rumble_filter { "on" } else { "off" }.into(),
        "--gate".into(),
        if options.gate { "on" } else { "off" }.into(),
        "--gate-threshold".into(),
        threshold_text(options.gate_threshold),
        "--gate-attack".into(),
        options.gate_attack_ms.to_string(),
        "--gate-release".into(),
        options.gate_release_ms.to_string(),
    ];
    args.extend([
        "--auto-level".into(),
        if options.level.enabled { "on" } else { "off" }.into(),
        "--compressor".into(),
        if options.level.compressor_enabled {
            "on"
        } else {
            "off"
        }
        .into(),
    ]);
    args.extend([
        "--deesser-mode".into(),
        if options.deesser_pro { "pro" } else { "simple" }.into(),
    ]);
    for (flag, value) in crate::deesser::FLAGS
        .iter()
        .zip(options.deesser_parameters.values())
    {
        args.extend([(*flag).into(), value.to_string()]);
    }
    for (flag, value) in crate::level::FLAGS.iter().zip(options.level.values()) {
        args.extend([(*flag).into(), value.to_string()]);
    }
    if !options.device.is_empty() {
        args.extend(["--device".into(), options.device.clone()]);
    }
    if !options.output_device.is_empty() {
        args.extend(["--output-device".into(), options.output_device.clone()]);
    }
    for (flag, value) in options
        .eq
        .flags()
        .iter()
        .zip(options.eq.arguments(&options.eq_mode))
    {
        args.extend([flag.clone(), value]);
    }
    args
}

fn decode(text: &str) -> Result<Options, Box<dyn Error>> {
    let mut section = false;
    let mut values = std::collections::HashMap::new();
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with([';', '#']) {
            continue;
        }
        if line == "[gui]" && !section {
            section = true;
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .filter(|_| section)
            .ok_or_else(|| format!("Invalid GUI INI entry on line {}", index + 1))?;
        let key = key.trim();
        if ![
            "device",
            "backend",
            "output_device",
            "input_channel",
            "output_channels",
            "frames",
            "sample_rate",
            "pitch",
            "pitch_algorithm",
            "formant_shift",
            "preserve_formants",
            "rumble_filter",
            "deesser",
            "deesser_mode",
            "gate",
            "gate_threshold",
            "gate_attack_ms",
            "gate_release_ms",
        ]
        .contains(&key)
            && !crate::eq::is_key(key)
            && key != "auto_level"
            && key != "compressor"
            && !crate::level::KEYS.contains(&key)
            && !crate::deesser::KEYS.contains(&key)
            || values.insert(key, value.trim()).is_some()
        {
            return Err(format!("Unknown or repeated GUI setting '{key}'").into());
        }
    }
    let value = |key| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("Missing GUI setting '{key}'"))
    };
    let device = value("device")?;
    if device.contains('\0') {
        return Err("Device name cannot contain NUL".into());
    }
    let formant = match value("preserve_formants")? {
        "true" => "preserve",
        "false" => "off",
        _ => return Err("preserve_formants must be true or false".into()),
    };
    let gate = match value("gate")? {
        "true" | "on" => "on",
        "false" | "off" => "off",
        _ => return Err("gate must be true or false".into()),
    };
    let rumble = match values.get("rumble_filter").copied().unwrap_or("false") {
        "true" => "on",
        "false" => "off",
        _ => return Err("rumble_filter must be true or false".into()),
    };
    let deesser = match values.get("deesser").copied().unwrap_or("false") {
        "true" => "on",
        "false" => "off",
        _ => return Err("deesser must be true or false".into()),
    };
    let mut args = vec![
        "--deesser",
        deesser,
        "--input-channel",
        values.get("input_channel").copied().unwrap_or("1"),
        "--output-channels",
        values.get("output_channels").copied().unwrap_or("1"),
        "--rumble-filter",
        rumble,
        "--backend",
        values.get("backend").copied().unwrap_or("asio"),
        "--sample-rate",
        values.get("sample_rate").copied().unwrap_or("48000"),
        "--frames",
        value("frames")?,
        "--pitch",
        value("pitch")?,
        "--pitch-algorithm",
        values
            .get("pitch_algorithm")
            .copied()
            .unwrap_or("phase-locked"),
        "--formant-shift",
        value("formant_shift")?,
        "--formant",
        formant,
        "--gate",
        gate,
        "--gate-threshold",
        value("gate_threshold")?,
        "--gate-attack",
        value("gate_attack_ms")?,
        "--gate-release",
        value("gate_release_ms")?,
    ];
    args.extend([
        "--deesser-mode",
        values.get("deesser_mode").copied().unwrap_or("simple"),
    ]);
    let defaults = crate::deesser::DeEsserParameters::default()
        .values()
        .map(|v| v.to_string());
    for ((flag, key), default) in crate::deesser::FLAGS
        .iter()
        .zip(crate::deesser::KEYS)
        .zip(&defaults)
    {
        args.extend([*flag, values.get(key).copied().unwrap_or(default)]);
    }
    let level = match value("auto_level")? {
        "true" => "on",
        "false" => "off",
        _ => return Err("auto_level must be true or false".into()),
    };
    args.extend(["--auto-level", level]);
    let compressor = match value("compressor")? {
        "true" => "on",
        "false" => "off",
        _ => return Err("compressor must be true or false".into()),
    };
    args.extend(["--compressor", compressor]);
    for (flag, key) in crate::level::FLAGS.iter().zip(crate::level::KEYS) {
        args.extend([*flag, value(key)?]);
    }
    if !device.is_empty() {
        args.extend(["--device", device]);
    }
    if let Some(output) = values.get("output_device").filter(|name| !name.is_empty()) {
        args.extend(["--output-device", output]);
    }
    for (flag, key) in crate::eq::FLAGS.iter().zip(crate::eq::KEYS) {
        args.extend([*flag, value(key)?]);
    }
    let band_count = values
        .keys()
        .filter(|key| key.starts_with("eq_band"))
        .count();
    if band_count == 0 {
        return Err("EQ requires at least one band".into());
    }
    let band_flags: Vec<_> = (1..=band_count).map(|i| format!("--eq-band{i}")).collect();
    let band_keys: Vec<_> = (1..=band_count).map(|i| format!("eq_band{i}")).collect();
    for (flag, key) in band_flags.iter().zip(&band_keys) {
        args.extend([flag.as_str(), value(key.as_str())?]);
    }
    let mut options = Options::parse_gui(args.into_iter().map(String::from))?;
    options.eq.bands.truncate(band_count);
    Ok(options)
}

pub(crate) fn load(path: &Path) -> Result<Options, Box<dyn Error>> {
    match fs::read_to_string(path) {
        Ok(text) => decode(&text).map_err(|error| {
            format!("Cannot read GUI settings '{}': {error}", path.display()).into()
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Options::parse_gui(Vec::new()),
        Err(error) => Err(format!("Cannot read GUI settings '{}': {error}", path.display()).into()),
    }
}

pub(crate) fn merge(saved: &Options, args: Vec<String>) -> Result<Options, Box<dyn Error>> {
    // Validate duplicates/unknown arguments before replacing individual saved values.
    // Cross-field validation is applied to the merged settings, not CLI defaults.
    let mut base = arguments(saved);
    let mut seen = std::collections::HashSet::new();
    let mut input = args.into_iter();
    while let Some(flag) = input.next() {
        if ![
            "--device",
            "--backend",
            "--output-device",
            "--input-channel",
            "--output-channels",
            "--frames",
            "--sample-rate",
            "--pitch",
            "--pitch-algorithm",
            "--formant-shift",
            "--formant",
            "--rumble-filter",
            "--deesser",
            "--deesser-mode",
            "--gate",
            "--gate-threshold",
            "--gate-attack",
            "--gate-release",
        ]
        .contains(&flag.as_str())
            && !crate::eq::is_flag(&flag)
            && flag != "--auto-level"
            && flag != "--compressor"
            && !crate::level::FLAGS.contains(&flag.as_str())
            && !crate::deesser::FLAGS.contains(&flag.as_str())
            || !seen.insert(flag.clone())
        {
            return Err(format!("Unknown or repeated GUI argument {flag}; use --help").into());
        }
        let value = input
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        if let Some(index) = base.iter().position(|item| item == &flag) {
            base[index + 1] = value;
        } else {
            base.extend([flag, value]);
        }
    }
    let indices: std::collections::BTreeSet<_> = base
        .iter()
        .filter_map(|flag| crate::eq::band_index(flag))
        .collect();
    let band_count = indices.last().map(|i| i + 1).unwrap_or(0);
    if indices.len() != band_count {
        return Err("EQ bands must be contiguous".into());
    }
    let mut options = Options::parse_gui(base)?;
    options.eq.bands.truncate(band_count);
    Ok(options)
}

pub(crate) fn save(path: &Path, options: &Options) -> Result<(), Box<dyn Error>> {
    options.validate()?;
    Options::parse_gui(arguments(options))?;
    if options.device.contains(['\r', '\n', '\0'])
        || options.output_device.contains(['\r', '\n', '\0'])
    {
        return Err("Device name cannot contain line breaks or NUL".into());
    }
    let mut text = format!(
        "[gui]\ndevice={}\nframes={}\npitch={}\nformant_shift={}\npreserve_formants={}\ngate={}\ngate_threshold={}\ngate_attack_ms={}\ngate_release_ms={}\n",
        options.device,
        options.frames,
        options.pitch.semitones,
        options.pitch.formant_shift,
        options.pitch.preserve_formants,
        options.gate,
        threshold_text(options.gate_threshold),
        options.gate_attack_ms,
        options.gate_release_ms
    );
    text.push_str(&format!(
        "backend={}\noutput_device={}\nauto_level={}\ncompressor={}\n",
        options.backend.name(),
        options.output_device,
        options.level.enabled,
        options.level.compressor_enabled
    ));
    text.push_str(&format!(
        "input_channel={}\noutput_channels={}\n",
        options.input_channel,
        options
            .output_channels
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    ));
    text.push_str(&format!(
        "sample_rate={}\npitch_algorithm={}\n",
        options.sample_rate,
        options.pitch.algorithm.name()
    ));
    text.push_str(&format!("rumble_filter={}\n", options.rumble_filter));
    text.push_str(&format!("deesser={}\n", options.deesser));
    text.push_str(&format!(
        "deesser_mode={}\n",
        if options.deesser_pro { "pro" } else { "simple" }
    ));
    for (key, value) in crate::deesser::KEYS
        .iter()
        .zip(options.deesser_parameters.values())
    {
        text.push_str(&format!("{key}={value}\n"));
    }
    for (key, value) in crate::level::KEYS.iter().zip(options.level.values()) {
        text.push_str(&format!("{key}={value}\n"));
    }
    for (key, value) in options
        .eq
        .keys()
        .iter()
        .zip(options.eq.arguments(&options.eq_mode))
    {
        text.push_str(&format!("{key}={value}\n"));
    }
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| std::io::Error::other("Settings path has no parent"))?,
        )?;
        fs::write(&temporary, text)?;
        // On Windows rename replaces the destination, keeping the old file on failure.
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("Cannot save GUI settings '{}': {error}", path.display()).into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn pitch_algorithm_round_trips_and_defaults_for_existing_ini() {
        use crate::pitch::PitchAlgorithm;
        let path = std::env::temp_dir().join(format!("pitch-algorithm-{}.ini", std::process::id()));
        for algorithm in PitchAlgorithm::ALL {
            let mut options = super::Options::default();
            options.pitch.algorithm = algorithm;
            super::save(&path, &options).unwrap();
            assert_eq!(super::load(&path).unwrap().pitch.algorithm, algorithm);
            let overridden = super::merge(
                &options,
                ["--pitch-algorithm", "pvsola"].map(String::from).to_vec(),
            )
            .unwrap();
            assert_eq!(overridden.pitch.algorithm, PitchAlgorithm::Pvsola);
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(super::decode(&text.replace(algorithm.name(), "invalid")).is_err());
            let legacy = super::decode(
                &text.replace(&format!("pitch_algorithm={}\n", algorithm.name()), ""),
            )
            .unwrap();
            assert_eq!(legacy.pitch.algorithm, PitchAlgorithm::PhaseLocked);
            assert_eq!(legacy.pitch.semitones, options.pitch.semitones);
            assert_eq!(legacy.pitch.formant_shift, options.pitch.formant_shift);
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn level_and_compressor_switches_persist_independently() {
        let path =
            std::env::temp_dir().join(format!("independent-level-{}.ini", std::process::id()));
        for enabled in [false, true] {
            for compressor_enabled in [false, true] {
                let mut options = super::Options::default();
                options.level.enabled = enabled;
                options.level.compressor_enabled = compressor_enabled;
                super::save(&path, &options).unwrap();
                assert_eq!(super::load(&path).unwrap(), options);
                let text = std::fs::read_to_string(&path).unwrap();
                assert!(super::decode(
                    &text.replace(&format!("compressor={compressor_enabled}\n"), "")
                )
                .is_err());
                assert!(super::decode(&text.replace(
                    &format!("compressor={compressor_enabled}\n"),
                    "compressor=invalid\n"
                ))
                .is_err());
                let merged = super::merge(
                    &options,
                    vec![
                        "--compressor".into(),
                        if compressor_enabled { "off" } else { "on" }.into(),
                    ],
                )
                .unwrap();
                assert_eq!(merged.level.enabled, enabled);
                assert_eq!(merged.level.compressor_enabled, !compressor_enabled);
            }
        }
        std::fs::remove_file(path).unwrap();
    }
    use super::*;
    #[test]
    fn variable_eq_bands_round_trip_and_merge_without_truncation() {
        let directory =
            std::env::temp_dir().join(format!("variable-eq-settings-{}", std::process::id()));
        let path = directory.join("settings.ini");
        for count in [1, 4, 8] {
            let mut options = Options::default();
            options.eq.bands = vec![
                crate::eq::Band {
                    frequency: 1000.0,
                    gain_db: 2.0,
                    q: 1.0
                };
                count
            ];
            save(&path, &options).unwrap();
            let loaded = load(&path).unwrap();
            assert_eq!(loaded, options);
            let merged = merge(&loaded, vec!["--pitch".into(), "2".into()]).unwrap();
            assert_eq!(merged.eq, options.eq);
        }
        let mut single = Options::default();
        single.eq.bands.truncate(1);
        assert!(merge(&single, vec!["--eq-band3".into(), "2000,1,1".into()]).is_err());
        single.eq.bands.clear();
        assert!(save(&path, &single).is_err());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(decode(&format!("{text}eq-band9=1000,0,1\n")).is_err());
        assert!(decode(&text.replace("eq_band4=1000,2,1\n", "")).is_err());
        assert!(decode(&format!("{text}eq_band17=1000,0,1\n")).is_err());
        let mut too_many = Options::default();
        too_many.eq.bands = vec![
            crate::eq::Band {
                frequency: 1000.0,
                gain_db: 0.0,
                q: 1.0
            };
            crate::eq::MAX_BANDS + 1
        ];
        assert!(save(&path, &too_many).is_err());
        assert!(crate::cli::Options::parse(
            (1..=crate::eq::MAX_BANDS + 1)
                .flat_map(|i| [format!("--eq-band{i}"), "1000,0,1".into()])
        )
        .is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }
    #[test]
    fn sample_rate_persists_overrides_and_defaults_for_legacy_ini() {
        let path =
            std::env::temp_dir().join(format!("voice-sample-rate-{}.ini", std::process::id()));
        let options = Options::parse_gui(["--sample-rate", "44100"].map(String::from)).unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            decode(&text.replace("sample_rate=44100\n", ""))
                .unwrap()
                .sample_rate,
            crate::RATE
        );
        assert_eq!(
            merge(
                &options,
                ["--sample-rate", "48000"].map(String::from).to_vec()
            )
            .unwrap()
            .sample_rate,
            48000
        );
        assert!(decode(&text.replace("sample_rate=44100", "sample_rate=0")).is_err());
        assert!(decode(&format!("{text}sample_rate=48000\n")).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn channel_routing_persists_and_legacy_defaults_are_preserved() {
        let path = std::env::temp_dir().join(format!("voice-routing-{}.ini", std::process::id()));
        let options = Options::parse_gui(
            ["--input-channel", "2", "--output-channels", "1,3"].map(String::from),
        )
        .unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let text = fs::read_to_string(&path).unwrap();
        let legacy = decode(
            &text
                .replace("input_channel=2\n", "")
                .replace("output_channels=1,3\n", ""),
        )
        .unwrap();
        assert_eq!(legacy.input_channel, 1);
        assert_eq!(legacy.output_channels, [1]);
        let changed = merge(
            &options,
            ["--output-channels", "2"].map(String::from).to_vec(),
        )
        .unwrap();
        assert_eq!(changed.input_channel, 2);
        assert_eq!(changed.output_channels, [2]);
        assert!(decode(&text.replace("output_channels=1,3", "output_channels=")).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn eq_round_trip_mode_switch_and_invalid_ini() {
        let path = std::env::temp_dir().join(format!("voice-eq-{}.ini", std::process::id()));
        let options = Options::parse_gui(
            [
                "--eq",
                "on",
                "--eq-mode",
                "pro",
                "--eq-band1",
                "180,-4,1.2",
                "--eq-band3",
                "9000,2,0.5",
                "--eq-output",
                "-2",
            ]
            .map(String::from),
        )
        .unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let simple = merge(&options, ["--eq-mode", "simple"].map(String::from).to_vec()).unwrap();
        assert_eq!(simple.eq, options.eq);
        assert_eq!(simple.eq_mode, "simple");
        let text = fs::read_to_string(&path).unwrap();
        assert!(decode(&text.replace("eq_mode=pro", "eq_mode=invalid")).is_err());
        assert!(decode(&text.replace("eq=on\n", "")).is_err());
        assert!(decode(&text.replace("eq_band1=180,-4,1.2", "eq_band1=NaN,-4,1.2")).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn rumble_round_trip_legacy_and_override() {
        let path =
            std::env::temp_dir().join(format!("voice-rumble-settings-{}.ini", std::process::id()));
        let options = Options::parse_gui(["--rumble-filter", "on"].map(String::from)).unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            !decode(&text.replace("rumble_filter=true\n", ""))
                .unwrap()
                .rumble_filter
        );
        assert!(decode(&text.replace("rumble_filter=true", "rumble_filter=invalid")).is_err());
        assert!(decode(&format!("{text}rumble_filter=false\n")).is_err());
        assert!(
            !merge(
                &options,
                ["--rumble-filter", "off"].map(String::from).to_vec()
            )
            .unwrap()
            .rumble_filter
        );
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn deesser_modes_preserve_parameters_and_round_trip() {
        let path =
            std::env::temp_dir().join(format!("voice-deesser-pro-{}.ini", std::process::id()));
        let options = Options::parse_gui(
            [
                "--deesser",
                "on",
                "--deesser-mode",
                "pro",
                "--deesser-frequency",
                "6500",
                "--deesser-threshold",
                "-30",
                "--deesser-range",
                "9",
                "--deesser-attack",
                "1",
                "--deesser-release",
                "100",
            ]
            .map(String::from),
        )
        .unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let simple = merge(
            &options,
            ["--deesser-mode", "simple"].map(String::from).to_vec(),
        )
        .unwrap();
        assert!(!simple.deesser_pro);
        assert_eq!(simple.deesser_parameters, options.deesser_parameters);
        save(&path, &simple).unwrap();
        assert_eq!(load(&path).unwrap(), simple);
        let text = fs::read_to_string(&path).unwrap();
        assert!(decode(&text.replace("deesser_mode=simple", "deesser_mode=invalid")).is_err());
        assert!(decode(&text.replace("deesser_frequency=6500", "deesser_frequency=NaN")).is_err());
        let legacy: String = text
            .lines()
            .filter(|line| !line.starts_with("deesser_"))
            .map(|line| format!("{line}\n"))
            .collect();
        let restored = decode(&legacy).unwrap();
        assert!(!restored.deesser_pro);
        assert_eq!(
            restored.deesser_parameters,
            crate::deesser::DeEsserParameters::default()
        );
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn deesser_round_trip_legacy_and_override() {
        let path =
            std::env::temp_dir().join(format!("voice-deesser-settings-{}.ini", std::process::id()));
        let options = Options::parse_gui(["--deesser", "on"].map(String::from)).unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let text = fs::read_to_string(&path).unwrap();
        assert!(!decode(&text.replace("deesser=true\n", "")).unwrap().deesser);
        assert!(decode(&text.replace("deesser=true", "deesser=invalid")).is_err());
        assert!(decode(&format!("{text}deesser=false\n")).is_err());
        assert!(
            !merge(&options, ["--deesser", "off"].map(String::from).to_vec())
                .unwrap()
                .deesser
        );
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn backend_and_output_round_trip_with_legacy_asio_settings() {
        let path =
            std::env::temp_dir().join(format!("voice-backend-settings-{}.ini", std::process::id()));
        let options = Options::parse_gui(
            [
                "--backend",
                "cpal",
                "--device",
                "入力 [id:1]",
                "--output-device",
                "出力 = Headphones [id:2]",
            ]
            .map(String::from),
        )
        .unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let asio =
            Options::parse_gui(["--backend", "asio", "--device", "ASIO Device"].map(String::from))
                .unwrap();
        save(&path, &asio).unwrap();
        let legacy = fs::read_to_string(&path)
            .unwrap()
            .replace("backend=asio\n", "")
            .replace("output_device=\n", "");
        assert_eq!(decode(&legacy).unwrap(), asio);
        let auto = Options::parse_gui(Vec::<String>::new()).unwrap();
        save(&path, &auto).unwrap();
        assert_eq!(load(&path).unwrap(), auto);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn round_trip_and_replace_without_audio() {
        let path = std::env::temp_dir()
            .join(format!("voice-settings-test-{}", std::process::id()))
            .join("settings.ini");
        let defaults = load(&path).unwrap();
        assert!(defaults.device.is_empty());
        let options = decode("[gui]\ndevice=日本語 ASIO = Device\nframes=256\npitch_algorithm=phase-locked\npitch=-2.5\nformant_shift=3.25\npreserve_formants=true\ngate=true\ngate_threshold=-35\ngate_attack_ms=3\ngate_release_ms=120\nauto_level=true\ncompressor=true\nlevel_target_db=-18\nlevel_max_gain_db=6\nlevel_floor_db=-55\nlevel_response_ms=1200\ncompress_threshold_db=-10\ncompress_ratio=4\ncompress_attack_ms=3\ncompress_release_ms=100\neq=off\neq_mode=simple\neq_band1=250,0,0.7\neq_band2=3000,0,0.8\neq_band3=8000,0,0.7\neq_output=0\n").unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let invalid = Options {
            frames: 0,
            ..decode("[gui]\ndevice=Other\nframes=512\npitch_algorithm=phase-locked\npitch=0\nformant_shift=0\npreserve_formants=true\ngate=false\ngate_threshold=auto\ngate_attack_ms=5\ngate_release_ms=80\nauto_level=false\ncompressor=false\nlevel_target_db=-20\nlevel_max_gain_db=9\nlevel_floor_db=-50\nlevel_response_ms=1500\ncompress_threshold_db=-12\ncompress_ratio=3\ncompress_attack_ms=5\ncompress_release_ms=150\neq=off\neq_mode=simple\neq_band1=250,0,0.7\neq_band2=3000,0,0.8\neq_band3=8000,0,0.7\neq_output=0\n").unwrap()
        };
        assert!(save(&path, &invalid).is_err());
        assert_eq!(load(&path).unwrap(), options);
        save(&path, &defaults).unwrap();
        assert_eq!(load(&path).unwrap(), defaults);
        fs::write(&path, "broken settings").unwrap();
        assert!(load(&path)
            .unwrap_err()
            .to_string()
            .contains("Cannot read GUI settings"));
        let blocked = path.join("settings.ini");
        assert!(save(&blocked, &defaults)
            .unwrap_err()
            .to_string()
            .contains("Cannot save GUI settings"));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn rejects_corrupt_or_invalid_settings() {
        for text in [
            "",
            "A\n512\n0\n0\n",
            "A\n0\n0\n0\npreserve\n",
            "A\n512\nNaN\n0\npreserve\n",
            "A\n512\n0\n3\noff\n",
            "A\n512\n0\n0\npreserve\nextra\n",
        ] {
            assert!(decode(text).is_err(), "{text:?}");
        }
    }
    #[test]
    fn explicit_values_override_only_their_fields() {
        let saved = decode("[gui]\ndevice=ASIO Device\nframes=256\npitch_algorithm=phase-locked\npitch=-3\nformant_shift=2\npreserve_formants=true\ngate=true\ngate_threshold=auto\ngate_attack_ms=5\ngate_release_ms=80\nauto_level=false\ncompressor=false\nlevel_target_db=-20\nlevel_max_gain_db=9\nlevel_floor_db=-50\nlevel_response_ms=1500\ncompress_threshold_db=-12\ncompress_ratio=3\ncompress_attack_ms=5\ncompress_release_ms=150\neq=off\neq_mode=simple\neq_band1=250,0,0.7\neq_band2=3000,0,0.8\neq_band3=8000,0,0.7\neq_output=0\n").unwrap();
        let result = merge(&saved, vec!["--pitch".into(), "4.5".into()]).unwrap();
        assert_eq!(result.pitch.semitones, 4.5);
        assert_eq!(result.device, saved.device);
        assert_eq!(result.frames, 256);
        assert_eq!(result.pitch.formant_shift, 2.0);
        assert!(result.gate);
        let customized = merge(
            &saved,
            vec![
                "--auto-level".into(),
                "on".into(),
                "--compress-ratio".into(),
                "5".into(),
            ],
        )
        .unwrap();
        assert!(customized.level.enabled);
        assert_eq!(customized.level.ratio, 5.0);
        assert_eq!(customized.level.target_db, saved.level.target_db);
        assert!(merge(&saved, vec!["--compress-attack".into(), "NaN".into()]).is_err());
        assert!(merge(&saved, vec!["--formant".into(), "off".into()]).is_err());
        assert!(merge(
            &saved,
            vec!["--pitch".into(), "0".into(), "--pitch".into(), "1".into()]
        )
        .is_err());
        assert!(merge(&saved, vec!["--inspect".into()]).is_err());
    }
    #[test]
    fn ini_comments_order_and_invalid_entries() {
        let text = "\u{feff}; preferences\r\n[gui]\r\n# voice\r\npreserve_formants = false\r\nformant_shift=0\r\npitch_algorithm=phase-locked\r\npitch = -2.5\r\nframes=256\r\ndevice=日本語 ASIO = Device\r\ngate=false\r\ngate_threshold=auto\r\ngate_attack_ms=5\r\ngate_release_ms=80\r\nauto_level=false\r\ncompressor=false\r\nlevel_target_db=-20\r\nlevel_max_gain_db=9\r\nlevel_floor_db=-50\r\nlevel_response_ms=1500\r\ncompress_threshold_db=-12\r\ncompress_ratio=3\r\ncompress_attack_ms=5\r\ncompress_release_ms=150\r\neq=off\r\neq_mode=simple\r\neq_band1=250,0,0.7\r\neq_band2=3000,0,0.8\r\neq_band3=8000,0,0.7\r\neq_output=0\r\n";
        let options = decode(text).unwrap();
        assert_eq!(options.device, "日本語 ASIO = Device");
        assert_eq!(options.frames, 256);
        assert_eq!(options.pitch.semitones, -2.5);
        assert!(!options.pitch.preserve_formants);
        for invalid in [
            text.replace("false", "invalid"),
            text.replace("256", "0"),
            text.replace("-2.5", "NaN"),
            text.replace("formant_shift=0", "formant_shift=3"),
            format!("{text}frames=512\n"),
            format!("{text}unknown=1\n"),
            format!("{text}[other]\n"),
            text.replace("frames=256\r\n", ""),
            text.replace("[gui]\r\n", ""),
        ] {
            assert!(decode(&invalid).is_err(), "{invalid:?}");
        }
    }
}
