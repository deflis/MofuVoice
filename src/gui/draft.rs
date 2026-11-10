//! Text is retained even while incomplete. Only a fully validated draft becomes Options.
use super::settings::Settings;
use crate::cli::Options;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Field {
    pub path: String,
    pub value: Value,
    pub text: String,
}

pub fn fields(settings: &Settings) -> Vec<Field> {
    fn visit(value: &Value, path: String, fields: &mut Vec<Field>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    visit(child, format!("{path}/{key}"), fields);
                }
            }
            Value::Array(items) if !path.ends_with("outputChannels") => {
                for (i, child) in items.iter().enumerate() {
                    visit(child, format!("{path}/{i}"), fields);
                }
            }
            _ => fields.push(Field {
                path,
                value: value.clone(),
                text: match value {
                    Value::String(s) => s.clone(),
                    Value::Array(a) => a.iter().map(Value::to_string).collect::<Vec<_>>().join(","),
                    _ => value.to_string(),
                },
            }),
        }
    }
    let mut result = Vec::new();
    visit(
        &serde_json::to_value(settings).expect("finite settings"),
        String::new(),
        &mut result,
    );
    let percent =
        serde_json::to_value(settings).expect("finite settings")["voice"]["pitchUnit"] == "percent";
    if percent {
        for field in &mut result {
            if field.path == "/voice/pitch" {
                field.text =
                    semitones_to_percent(field.text.parse().expect("numeric pitch")).to_string();
            }
        }
    }
    result
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

pub fn parse(settings: &Settings, edits: &BTreeMap<String, String>) -> Result<Options, String> {
    let mut value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    let percent = edits
        .get("/voice/pitchUnit")
        .map(String::as_str)
        .unwrap_or_else(|| value["voice"]["pitchUnit"].as_str().expect("pitch unit"))
        == "percent";
    for (path, text) in edits {
        let slot = value
            .pointer_mut(path)
            .ok_or_else(|| format!("Unknown setting: {path}"))?;
        *slot = if percent && path == "/voice/pitch" {
            Value::from(percent_to_semitones(
                text.parse()
                    .map_err(|_| format!("{path}: enter a percentage"))?,
            )?)
        } else {
            match slot {
                Value::String(_) => Value::String(text.clone()),
                Value::Bool(_) => Value::Bool(
                    text.parse()
                        .map_err(|_| format!("{path}: expected true/false"))?,
                ),
                Value::Array(_) => Value::Array(
                    text.split(',')
                        .map(|s| {
                            s.trim().parse::<u64>().map(Value::from).map_err(|_| {
                                format!("{path}: enter channel numbers separated by commas")
                            })
                        })
                        .collect::<Result<_, _>>()?,
                ),
                Value::Number(number) => {
                    let parsed = text
                        .parse::<f64>()
                        .ok()
                        .filter(|n| n.is_finite())
                        .ok_or_else(|| format!("{path}: enter a finite number"))?;
                    if number.is_u64() {
                        if parsed < 0. || parsed.fract() != 0. || parsed >= u64::MAX as f64 {
                            return Err(format!("{path}: enter a nonnegative integer"));
                        }
                        Value::from(parsed as u64)
                    } else {
                        Value::from(parsed)
                    }
                }
                _ => return Err(format!("Unsupported setting: {path}")),
            }
        };
    }
    serde_json::from_value::<Settings>(value)
        .map_err(|e| e.to_string())?
        .options()
}

pub fn group(path: &str, section: &str) -> bool {
    let root = path.split('/').nth(1).unwrap_or_default();
    match section {
        "noiseGate" => root == "noise" && !path.ends_with("rumbleFilter"),
        "inputEq" => root == "inputEq" || path == "/noise/rumbleFilter",
        "postConversionEq" => root == "eq",
        "autoLevel" => {
            root == "level"
                && ![
                    "compressorEnabled",
                    "thresholdDb",
                    "ratio",
                    "attackMs",
                    "releaseMs",
                ]
                .iter()
                .any(|s| path.ends_with(s))
        }
        "postCompressor" => {
            root == "level"
                && [
                    "compressorEnabled",
                    "thresholdDb",
                    "ratio",
                    "attackMs",
                    "releaseMs",
                ]
                .iter()
                .any(|s| path.ends_with(s))
        }
        _ => root == section,
    }
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
        let settings = Settings::from_options(&original);
        let mut edits: BTreeMap<_, _> = fields(&settings)
            .into_iter()
            .map(|f| (f.path, f.text))
            .collect();
        assert_eq!(parse(&settings, &edits).unwrap(), original);
        for bad in ["-", "NaN", "1e999", "13"] {
            edits.insert("/voice/pitch".into(), bad.into());
            assert!(parse(&settings, &edits).is_err());
        }
        edits.insert("/voice/pitch".into(), "7.5".into());
        assert_eq!(parse(&settings, &edits).unwrap().pitch.semitones, 7.5);
        assert_eq!(settings.options().unwrap(), original);
    }
    #[test]
    fn every_setting_has_an_editable_section() {
        for field in fields(&Settings::from_options(&Options::default())) {
            assert!(
                super::super::preferences::SECTIONS
                    .iter()
                    .any(|(id, _)| group(&field.path, id)),
                "{}",
                field.path
            );
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
    fn percent_display_maps_to_semitones_without_changing_the_engine_unit() {
        for (percent, semitones) in [(50., -12.), (100., 0.), (200., 12.)] {
            assert!((percent_to_semitones(percent).unwrap() - semitones).abs() < 1e-10);
            assert!((semitones_to_percent(semitones) - percent).abs() < 1e-10);
        }
        let options = Options {
            pitch_unit: "percent".into(),
            pitch: crate::pitch::PitchSettings {
                semitones: 7.5,
                ..Default::default()
            },
            ..Default::default()
        };
        let settings = Settings::from_options(&options);
        let edits = fields(&settings)
            .into_iter()
            .map(|f| (f.path, f.text))
            .collect();
        assert!((parse(&settings, &edits).unwrap().pitch.semitones - 7.5).abs() < 1e-10);
        for bad in [0., 49., 201., f64::NAN] {
            assert!(percent_to_semitones(bad).is_err());
        }
    }
    #[test]
    fn numeric_editing_accepts_browser_number_syntax_and_keeps_formants_in_semitones() {
        let settings = Settings::from_options(&Options::default());
        for text in ["+7.5", "7.50", "7.5e0"] {
            let edits = BTreeMap::from([
                ("/voice/pitch".into(), text.into()),
                ("/devices/frames".into(), "480.0".into()),
            ]);
            let options = parse(&settings, &edits).unwrap();
            assert_eq!(options.pitch.semitones, 7.5);
            assert_eq!(options.frames, 480);
        }
        let edits = BTreeMap::from([
            ("/voice/pitchUnit".into(), "percent".into()),
            ("/voice/pitch".into(), "200".into()),
            ("/voice/formantShift".into(), "3".into()),
        ]);
        let options = parse(&settings, &edits).unwrap();
        assert_eq!(options.pitch.semitones, 12.);
        assert_eq!(options.pitch.formant_shift, 3.);
    }
}
