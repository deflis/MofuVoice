//! Shared CLI/persistence mapping for independently stored filter settings.
use crate::cli::Options;
use std::error::Error;
const FLAGS: &[&str] = &[
    "--pre-compressor",
    "--pre-compress-threshold",
    "--pre-compress-ratio",
    "--pre-compress-attack",
    "--pre-compress-release",
    "--voice",
    "--limiter",
    "--limiter-ceiling",
    "--limiter-release",
    "--dereverb",
    "--dereverb-amount",
    "--dereverb-decay",
    "--reverb",
    "--reverb-mix",
    "--reverb-pre-delay",
    "--reverb-decay",
];
fn eq_flag(flag: &str) -> Option<(&str, String)> {
    for prefix in ["input", "pre-conversion"] {
        if let Some(suffix) = flag.strip_prefix(&format!("--{prefix}-eq")) {
            let mapped = format!("--eq{suffix}");
            if crate::eq::is_flag(&mapped) {
                return Some((prefix, mapped));
            }
        }
    }
    None
}
pub(crate) fn is_flag(flag: &str) -> bool {
    FLAGS.contains(&flag) || eq_flag(flag).is_some()
}
#[cfg(any(feature = "desktop", test))]
pub(crate) fn is_key(key: &str) -> bool {
    is_flag(&format!("--{}", key.replace('_', "-")))
}
fn toggle(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("Filter switch requires on or off".into()),
    }
}
pub(crate) fn set(o: &mut Options, flag: &str, value: &str) -> Result<(), Box<dyn Error>> {
    if let Some((prefix, mapped)) = eq_flag(flag) {
        let (eq, mode) = if prefix == "input" {
            (&mut o.input_eq, &mut o.input_eq_mode)
        } else {
            (&mut o.pre_conversion_eq, &mut o.pre_conversion_eq_mode)
        };
        match mapped.as_str() {
            "--eq" => eq.enabled = toggle(value)?,
            "--eq-mode" => *mode = value.into(),
            "--eq-output" => eq.output_db = value.parse()?,
            _ => {
                let index = crate::eq::band_index(&mapped).unwrap();
                let band = crate::eq::Band::parse(value)?;
                if index < eq.bands.len() {
                    eq.bands[index] = band;
                } else if index == eq.bands.len() {
                    eq.bands.push(band);
                } else {
                    return Err("EQ bands must be contiguous".into());
                }
            }
        }
        return Ok(());
    }
    match flag {
        "--pre-compressor" => o.compressor.enabled = toggle(value)?,
        "--pre-compress-threshold" => o.compressor.threshold_db = value.parse()?,
        "--pre-compress-ratio" => o.compressor.ratio = value.parse()?,
        "--pre-compress-attack" => o.compressor.attack_ms = value.parse()?,
        "--pre-compress-release" => o.compressor.release_ms = value.parse()?,
        "--voice" => o.voice_enabled = toggle(value)?,
        "--limiter" => o.limiter.enabled = toggle(value)?,
        "--limiter-ceiling" => o.limiter.ceiling_db = value.parse()?,
        "--limiter-release" => o.limiter.release_ms = value.parse()?,
        "--dereverb" => o.dereverb.enabled = toggle(value)?,
        "--dereverb-amount" => o.dereverb.amount = value.parse()?,
        "--dereverb-decay" => o.dereverb.decay_ms = value.parse()?,
        "--reverb" => o.reverb.enabled = toggle(value)?,
        "--reverb-mix" => o.reverb.mix = value.parse()?,
        "--reverb-pre-delay" => o.reverb.pre_delay_ms = value.parse()?,
        "--reverb-decay" => o.reverb.decay_ms = value.parse()?,
        _ => return Err(format!("Unknown filter argument {flag}").into()),
    }
    Ok(())
}
#[cfg(any(feature = "desktop", test))]
pub(crate) fn arguments(o: &Options) -> Vec<String> {
    let on = |v| {
        if v {
            "on".to_string()
        } else {
            "off".to_string()
        }
    };
    let values = [
        on(o.compressor.enabled),
        o.compressor.threshold_db.to_string(),
        o.compressor.ratio.to_string(),
        o.compressor.attack_ms.to_string(),
        o.compressor.release_ms.to_string(),
        on(o.voice_enabled),
        on(o.limiter.enabled),
        o.limiter.ceiling_db.to_string(),
        o.limiter.release_ms.to_string(),
        on(o.dereverb.enabled),
        o.dereverb.amount.to_string(),
        o.dereverb.decay_ms.to_string(),
        on(o.reverb.enabled),
        o.reverb.mix.to_string(),
        o.reverb.pre_delay_ms.to_string(),
        o.reverb.decay_ms.to_string(),
    ];
    let mut args: Vec<String> = FLAGS
        .iter()
        .zip(values)
        .flat_map(|(k, v)| [k.to_string(), v])
        .collect();
    for (prefix, eq, mode) in [
        ("input", &o.input_eq, &o.input_eq_mode),
        (
            "pre-conversion",
            &o.pre_conversion_eq,
            &o.pre_conversion_eq_mode,
        ),
    ] {
        for (flag, value) in eq.flags().into_iter().zip(eq.arguments(mode)) {
            args.extend([flag.replacen("--eq", &format!("--{prefix}-eq"), 1), value]);
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independently_validate_disabled_filters_and_duplicate_flags() {
        for args in [
            vec!["--voice", "off", "--pitch", "NaN"],
            vec!["--limiter", "off", "--limiter-ceiling", "1"],
            vec!["--dereverb", "off", "--dereverb-decay", "0"],
            vec!["--reverb", "off", "--reverb-mix", "-1"],
            vec!["--pre-compressor", "off", "--pre-compress-ratio", "0"],
            vec!["--input-eq", "off", "--input-eq-band1", "200,NaN,1"],
            vec!["--pre-conversion-eq-mode", "invalid"],
            vec!["--voice", "on", "--voice", "off"],
            vec!["--reverb", "off", "--help"],
        ] {
            assert!(Options::parse(args.into_iter().map(String::from)).is_err());
        }
    }
}
