//! Presentation ranges and steps from the settings controls, independent of DSP validation.
use crate::gui::draft::{EqBandField, NumberField, PitchUnit};

#[derive(Clone, Copy, Debug)]
pub(super) struct NumberSpec {
    pub min: f64,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

impl NumberSpec {
    pub fn step_value(self, text: &str, increment: bool) -> String {
        let value = text
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .unwrap_or(0.);
        let step = self.step.unwrap_or(1.);
        let next = if self.step.is_some() {
            let grid = (value - self.min) / step;
            self.min
                + if increment {
                    (grid + 1e-9).floor() + 1.
                } else {
                    (grid - 1e-9).ceil() - 1.
                } * step
        } else {
            value + if increment { step } else { -step }
        };
        let next = next.max(self.min).min(self.max.unwrap_or(f64::MAX));
        // Do not propagate binary floating point artifacts into retained input text.
        format!("{:.10}", next)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

pub(super) fn spec(field: NumberField, unit: PitchUnit) -> NumberSpec {
    let (min, max, step) = match field {
        NumberField::Pitch if unit == PitchUnit::Percent => (50., Some(200.), None),
        NumberField::Pitch | NumberField::FormantShift => (-12., Some(12.), Some(0.01)),
        NumberField::Frames => (1., Some(8192.), Some(1.)),
        NumberField::DeesserFrequencyHz => (2000., Some(12000.), Some(1.)),
        NumberField::DeesserThresholdDb => (-60., Some(0.), Some(0.1)),
        NumberField::DeesserRangeDb => (0., Some(12.), Some(0.5)),
        NumberField::DeesserAttackMs => (0.1, Some(20.), Some(0.1)),
        NumberField::DeesserReleaseMs => (10., Some(500.), Some(1.)),
        NumberField::GateThresholdDb => (-90., Some(0.), None),
        NumberField::GateAttackMs | NumberField::GateReleaseMs => (0.1, None, None),
        NumberField::LevelTargetDb => (-36., Some(-6.), None),
        NumberField::LevelMaxGainDb => (0., Some(18.), None),
        NumberField::LevelFloorDb => (-90., Some(-30.), None),
        NumberField::LevelResponseMs => (100., Some(10000.), None),
        NumberField::PostCompressorThresholdDb | NumberField::CompressorThresholdDb => {
            (-36., Some(0.), None)
        }
        NumberField::PostCompressorRatio | NumberField::CompressorRatio => (1., Some(20.), None),
        NumberField::PostCompressorAttackMs | NumberField::CompressorAttackMs => {
            (0.1, Some(100.), None)
        }
        NumberField::PostCompressorReleaseMs | NumberField::CompressorReleaseMs => {
            (10., Some(2000.), None)
        }
        NumberField::LimiterCeilingDb => (-24., Some(0.), None),
        NumberField::LimiterReleaseMs => (1., Some(2000.), None),
        NumberField::DereverbAmount | NumberField::ReverbMix => (0., Some(1.), None),
        NumberField::DereverbDecayMs => (20., Some(2000.), None),
        NumberField::ReverbPreDelayMs => (0., Some(200.), None),
        NumberField::ReverbDecayMs => (100., Some(10000.), None),
        NumberField::EqBand(_, _, EqBandField::Frequency) => (40., Some(16000.), Some(1.)),
        NumberField::EqBand(_, _, EqBandField::GainDb) => (-12., Some(12.), Some(0.1)),
        NumberField::EqBand(_, _, EqBandField::Q) => (0.2, Some(8.), Some(0.1)),
        NumberField::EqOutputDb(_) => (-18., Some(0.), Some(0.1)),
    };
    NumberSpec { min, max, step }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SliderSpec {
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub neutral: f32,
    pub labels: [&'static str; 3],
}

impl SliderSpec {
    pub fn valid(self, text: &str) -> bool {
        text.parse::<f32>()
            .is_ok_and(|n| n.is_finite() && (self.min..=self.max).contains(&n))
    }

    pub fn text(self, value: f32) -> String {
        format!("{:.*}", if self.step < 0.1 { 2 } else { 1 }, value)
    }
}

pub(super) fn slider(field: NumberField, unit: PitchUnit) -> Option<SliderSpec> {
    let (min, max, step, neutral, labels) = match field {
        NumberField::Pitch if unit == PitchUnit::Percent => {
            (50., 200., 0.1, 100., ["低く", "元の声（100%）", "高く"])
        }
        NumberField::Pitch => (-12., 12., 0.01, 0., ["低く", "元の声", "高く"]),
        NumberField::FormantShift => (-12., 12., 0.01, 0., ["太く", "元の声", "明るく"]),
        NumberField::DeesserRangeDb => (0., 12., 0.5, 6., ["弱く", "標準", "強く"]),
        NumberField::EqBand(_, index, EqBandField::GainDb) => (
            -12.,
            12.,
            0.5,
            0.,
            match index {
                0 => ["すっきり", "そのまま", "ふっくら"],
                1 => ["やわらかく", "そのまま", "くっきり"],
                2 => ["落ち着いた", "そのまま", "明るい"],
                _ => ["抑える", "そのまま", "強める"],
            },
        ),
        _ => return None,
    };
    Some(SliderSpec {
        min,
        max,
        step,
        neutral,
        labels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::draft::EqKind;

    #[test]
    fn numeric_steps_match_limits_without_quantizing_typed_values() {
        let s = spec(
            NumberField::EqBand(EqKind::PostConversion, 0, EqBandField::GainDb),
            PitchUnit::Semitones,
        );
        assert_eq!(s.step_value("1.25", true), "1.3");
        assert_eq!(s.step_value("1.25", false), "1.2");
        assert_eq!(s.step_value("12", true), "12");
        let s = spec(NumberField::DeesserFrequencyHz, PitchUnit::Semitones);
        assert_eq!(s.step_value("2000", false), "2000");
    }

    #[test]
    fn all_slider_ranges_and_invalid_drafts_are_explicit() {
        for (field, unit, min, max, step) in [
            (NumberField::Pitch, PitchUnit::Semitones, -12., 12., 0.01),
            (NumberField::Pitch, PitchUnit::Percent, 50., 200., 0.1),
            (
                NumberField::FormantShift,
                PitchUnit::Percent,
                -12.,
                12.,
                0.01,
            ),
            (
                NumberField::EqBand(EqKind::PostConversion, 0, EqBandField::GainDb),
                PitchUnit::Semitones,
                -12.,
                12.,
                0.5,
            ),
            (
                NumberField::DeesserRangeDb,
                PitchUnit::Semitones,
                0.,
                12.,
                0.5,
            ),
        ] {
            let s = slider(field, unit).unwrap();
            assert_eq!((s.min, s.max, s.step), (min, max, step));
            for invalid in ["", "-", "NaN", "Infinity", "1000"] {
                assert!(!s.valid(invalid));
            }
        }
    }
}
