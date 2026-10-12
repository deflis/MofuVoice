//! GUI preferences only; never read or written from an audio callback.
use crate::{cli::Options, gate::GateThreshold};
use std::{
    error::Error,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

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
        "--frames".into(),
        options.frames.to_string(),
        "--pitch".into(),
        options.pitch.to_string(),
        "--formant-shift".into(),
        options.formant_shift.to_string(),
        "--formant".into(),
        if options.preserve_formants {
            "preserve"
        } else {
            "off"
        }
        .into(),
        "--gate".into(),
        if options.gate { "on" } else { "off" }.into(),
        "--gate-threshold".into(),
        threshold_text(options.gate_threshold),
        "--gate-attack".into(),
        options.gate_attack_ms.to_string(),
        "--gate-release".into(),
        options.gate_release_ms.to_string(),
    ];
    if !options.device.is_empty() {
        args.extend(["--device".into(), options.device.clone()]);
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
            "frames",
            "pitch",
            "formant_shift",
            "preserve_formants",
            "gate",
            "gate_threshold",
            "gate_attack_ms",
            "gate_release_ms",
        ]
        .contains(&key)
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
    let mut args = vec![
        "--frames",
        value("frames")?,
        "--pitch",
        value("pitch")?,
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
    if !device.is_empty() {
        args.extend(["--device", device]);
    }
    Options::parse_gui(args.into_iter().map(String::from))
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
            "--frames",
            "--pitch",
            "--formant-shift",
            "--formant",
            "--gate",
            "--gate-threshold",
            "--gate-attack",
            "--gate-release",
        ]
        .contains(&flag.as_str())
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
    Options::parse_gui(base)
}

pub(crate) fn save(path: &Path, options: &Options) -> Result<(), Box<dyn Error>> {
    Options::parse_gui(arguments(options))?;
    if options.device.contains(['\r', '\n', '\0']) {
        return Err("Device name cannot contain line breaks or NUL".into());
    }
    let text = format!(
        "[gui]\ndevice={}\nframes={}\npitch={}\nformant_shift={}\npreserve_formants={}\ngate={}\ngate_threshold={}\ngate_attack_ms={}\ngate_release_ms={}\n",
        options.device,
        options.frames,
        options.pitch,
        options.formant_shift,
        options.preserve_formants,
        options.gate,
        threshold_text(options.gate_threshold),
        options.gate_attack_ms,
        options.gate_release_ms
    );
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
    use super::*;
    #[test]
    fn round_trip_and_replace_without_audio() {
        let path = std::env::temp_dir()
            .join(format!("voice-settings-test-{}", std::process::id()))
            .join("settings.ini");
        let defaults = load(&path).unwrap();
        assert!(defaults.device.is_empty());
        let options = decode("[gui]\ndevice=日本語 ASIO = Device\nframes=256\npitch=-2.5\nformant_shift=3.25\npreserve_formants=true\ngate=true\ngate_threshold=-35\ngate_attack_ms=3\ngate_release_ms=120\n").unwrap();
        save(&path, &options).unwrap();
        assert_eq!(load(&path).unwrap(), options);
        let invalid = Options {
            frames: 0,
            ..decode("[gui]\ndevice=Other\nframes=512\npitch=0\nformant_shift=0\npreserve_formants=true\ngate=false\ngate_threshold=auto\ngate_attack_ms=5\ngate_release_ms=80\n").unwrap()
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
        let saved = decode("[gui]\ndevice=ASIO Device\nframes=256\npitch=-3\nformant_shift=2\npreserve_formants=true\ngate=true\ngate_threshold=auto\ngate_attack_ms=5\ngate_release_ms=80\n").unwrap();
        let result = merge(&saved, vec!["--pitch".into(), "4.5".into()]).unwrap();
        assert_eq!(result.pitch, 4.5);
        assert_eq!(result.device, saved.device);
        assert_eq!(result.frames, 256);
        assert_eq!(result.formant_shift, 2.0);
        assert!(result.gate);
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
        let text = "\u{feff}; preferences\r\n[gui]\r\n# voice\r\npreserve_formants = false\r\nformant_shift=0\r\npitch = -2.5\r\nframes=256\r\ndevice=日本語 ASIO = Device\r\ngate=false\r\ngate_threshold=auto\r\ngate_attack_ms=5\r\ngate_release_ms=80\r\n";
        let options = decode(text).unwrap();
        assert_eq!(options.device, "日本語 ASIO = Device");
        assert_eq!(options.frames, 256);
        assert_eq!(options.pitch, -2.5);
        assert!(!options.preserve_formants);
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
