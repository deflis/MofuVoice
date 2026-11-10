//! Presentation ranges and steps from the settings controls, independent of DSP validation.
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
pub(super) fn spec(path: &str, percent: bool) -> Option<NumberSpec> {
    let (min, max, step) = match path {
        "/voice/pitch" if percent => (50., Some(200.), None),
        "/voice/pitch" | "/voice/formantShift" => (-12., Some(12.), Some(0.01)),
        "/devices/frames" => (1., Some(8192.), Some(1.)),
        "/devices/inputChannel" => (1., Some(512.), Some(1.)),
        "/deesser/frequencyHz" => (2000., Some(12000.), Some(1.)),
        "/deesser/thresholdDb" => (-60., Some(0.), Some(0.1)),
        "/deesser/rangeDb" => (0., Some(12.), Some(0.5)),
        "/deesser/attackMs" => (0.1, Some(20.), Some(0.1)),
        "/deesser/releaseMs" => (10., Some(500.), Some(1.)),
        "/noise/thresholdDb" => (-90., Some(0.), None),
        "/noise/attackMs" | "/noise/releaseMs" => (0.1, None, None),
        "/level/targetDb" => (-36., Some(-6.), None),
        "/level/maxGainDb" => (0., Some(18.), None),
        "/level/floorDb" => (-90., Some(-30.), None),
        "/level/responseMs" => (100., Some(10000.), None),
        "/level/thresholdDb" | "/compressor/thresholdDb" => (-36., Some(0.), None),
        "/level/ratio" | "/compressor/ratio" => (1., Some(20.), None),
        "/level/attackMs" | "/compressor/attackMs" => (0.1, Some(100.), None),
        "/level/releaseMs" | "/compressor/releaseMs" => (10., Some(2000.), None),
        "/limiter/ceilingDb" => (-24., Some(0.), None),
        "/limiter/releaseMs" => (1., Some(2000.), None),
        "/dereverb/amount" | "/reverb/mix" => (0., Some(1.), None),
        "/dereverb/decayMs" => (20., Some(2000.), None),
        "/reverb/preDelayMs" => (0., Some(200.), None),
        "/reverb/decayMs" => (100., Some(10000.), None),
        _ if eq_root(path).is_some() => match path.rsplit('/').next()? {
            "frequency" => (40., Some(16000.), Some(1.)),
            "gainDb" => (-12., Some(12.), Some(0.1)),
            "q" => (0.2, Some(8.), Some(0.1)),
            "outputDb" => (-18., Some(0.), Some(0.1)),
            _ => return None,
        },
        _ => return None,
    };
    Some(NumberSpec { min, max, step })
}
pub(super) fn eq_root(path: &str) -> Option<&str> {
    path.split('/')
        .nth(1)
        .filter(|s| ["eq", "inputEq", "preConversionEq"].contains(s))
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
pub(super) fn slider(path: &str, percent: bool) -> Option<SliderSpec> {
    let (min, max, step, neutral, labels) = match path {
        "/voice/pitch" if percent => (50., 200., 0.1, 100., ["低く", "元の声（100%）", "高く"]),
        "/voice/pitch" => (-12., 12., 0.01, 0., ["低く", "元の声", "高く"]),
        "/voice/formantShift" => (-12., 12., 0.01, 0., ["太く", "元の声", "明るく"]),
        "/deesser/rangeDb" => (0., 12., 0.5, 6., ["弱く", "標準", "強く"]),
        _ if eq_root(path).is_some() && path.ends_with("/gainDb") => {
            let i = path.split('/').nth(3)?.parse::<usize>().ok()?;
            (
                -12.,
                12.,
                0.5,
                0.,
                match i {
                    0 => ["すっきり", "そのまま", "ふっくら"],
                    1 => ["やわらかく", "そのまま", "くっきり"],
                    2 => ["落ち着いた", "そのまま", "明るい"],
                    _ => ["抑える", "そのまま", "強める"],
                },
            )
        }
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
    #[test]
    fn numeric_steps_match_limits_without_quantizing_typed_values() {
        let s = spec("/eq/bands/0/gainDb", false).unwrap();
        assert_eq!(s.step_value("1.25", true), "1.3");
        assert_eq!(s.step_value("1.25", false), "1.2");
        assert_eq!(s.step_value("12", true), "12");
        let s = spec("/deesser/frequencyHz", false).unwrap();
        assert_eq!(s.step_value("2000", false), "2000");
    }
    #[test]
    fn all_slider_ranges_and_invalid_drafts_are_explicit() {
        for (path, percent, min, max, step) in [
            ("/voice/pitch", false, -12., 12., 0.01),
            ("/voice/pitch", true, 50., 200., 0.1),
            ("/voice/formantShift", true, -12., 12., 0.01),
            ("/eq/bands/0/gainDb", false, -12., 12., 0.5),
            ("/deesser/rangeDb", false, 0., 12., 0.5),
        ] {
            let s = slider(path, percent).unwrap();
            assert_eq!((s.min, s.max, s.step), (min, max, step));
            for invalid in ["", "-", "NaN", "Infinity", "1000"] {
                assert!(!s.valid(invalid));
            }
        }
    }
}
