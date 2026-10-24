//! Causal sample-peak limiter with instantaneous attack and exponential release.
//! No lookahead or added sample delay. This does not constrain inter-sample peaks
//! or peaks introduced by later stages in the chain.
use crate::{
    dsp::{envelope::decay, gain::amplitude, Sample},
    RATE,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimiterSettings {
    pub enabled: bool,
    pub ceiling_db: f64,
    pub release_ms: f64,
}

impl Default for LimiterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling_db: -1.0,
            release_ms: 80.0,
        }
    }
}

impl LimiterSettings {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.ceiling_db.is_finite() || !(-24.0..=0.0).contains(&self.ceiling_db) {
            return Err("Limiter ceiling must be between -24 and 0 dBFS");
        }
        if !self.release_ms.is_finite() || !(1.0..=2000.0).contains(&self.release_ms) {
            return Err("Limiter release must be between 1 and 2000 ms");
        }
        Ok(())
    }

    pub fn build(self) -> Result<Limiter, &'static str> {
        self.build_with_sample_rate(RATE)
    }

    pub fn build_with_sample_rate(self, sample_rate: u32) -> Result<Limiter, &'static str> {
        self.validate()?;
        crate::validate_sample_rate(sample_rate)?;
        Ok(Limiter {
            enabled: self.enabled,
            ceiling: amplitude(self.ceiling_db),
            release: decay(self.release_ms, sample_rate as f64),
            gain: 1.0,
        })
    }
}

pub struct Limiter {
    enabled: bool,
    ceiling: f64,
    release: f64,
    gain: f64,
}

impl Limiter {
    pub fn process<T: Sample>(&mut self, samples: &mut [T]) {
        self.process_in_place(samples);
    }

    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if !self.enabled {
            return;
        }
        for sample in samples {
            let input = sample.normalized();
            if !input.is_finite() {
                *sample = T::default();
                self.gain = 1.0;
                continue;
            }
            let peak = input.abs();
            let target = if peak > self.ceiling {
                self.ceiling / peak
            } else {
                1.0
            };
            let released = 1.0 - (1.0 - self.gain) * self.release;
            self.gain = target.min(released);
            // Preserve dry bits (including signed zero) whenever no gain is needed.
            if self.gain < 1.0 {
                *sample =
                    T::from_normalized((input * self.gain).clamp(-self.ceiling, self.ceiling));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active() -> LimiterSettings {
        LimiterSettings {
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn disabled_preserves_all_formats_and_float_bits() {
        let mut dsp = LimiterSettings::default().build().unwrap();
        let input = [
            0.0_f32,
            -0.0,
            f32::from_bits(0x7fc01234),
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(1),
            f32::MAX,
        ];
        let mut output = input;
        dsp.process_in_place(&mut output);
        assert_eq!(input.map(f32::to_bits), output.map(f32::to_bits));
        let input = [i32::MIN, -1, 0, 1, i32::MAX];
        let mut output = input;
        dsp.process_in_place(&mut output);
        assert_eq!(input, output);
        let input = [i16::MIN, -1, 0, 1, i16::MAX];
        let mut output = input;
        dsp.process_in_place(&mut output);
        assert_eq!(input, output);
    }

    #[test]
    fn limits_first_peak_and_recovers_with_configured_time_constant() {
        for rate in crate::SAMPLE_RATES {
            let settings = LimiterSettings {
                ceiling_db: -6.0,
                release_ms: 100.0,
                ..active()
            };
            let mut dsp = settings.build_with_sample_rate(rate).unwrap();
            let ceiling = amplitude(settings.ceiling_db);
            let mut peak = [1.0_f32];
            dsp.process(&mut peak);
            assert!((peak[0] as f64 - ceiling).abs() < 1e-7);
            let initial_gain = dsp.gain;
            let mut silence = vec![0.0_f32; rate as usize / 10];
            dsp.process(&mut silence);
            assert!((dsp.gain - (1.0 - (1.0 - initial_gain) * (-1.0_f64).exp())).abs() < 1e-11);
            assert!(silence.iter().all(|v| *v == 0.0));
        }
    }

    #[test]
    fn output_peak_is_bounded_for_all_formats() {
        fn check<T: Sample>(tolerance: f64) {
            let settings = LimiterSettings {
                ceiling_db: -12.0,
                ..active()
            };
            let mut dsp = settings.build().unwrap();
            let mut input: Vec<T> = (0..4096)
                .map(|i| T::from_normalized((i as f64 * 0.3).sin()))
                .collect();
            dsp.process(&mut input);
            assert!(input
                .iter()
                .all(|s| s.normalized().abs() <= amplitude(-12.0) + tolerance));
        }
        check::<f32>(1e-7);
        check::<i16>(0.5 / 32768.0);
        check::<i32>(0.5 / 2147483648.0);
    }

    #[test]
    fn state_crosses_block_boundaries_exactly() {
        let input: Vec<f32> = (0..4097).map(|i| (i as f32 * 0.31).sin()).collect();
        let mut whole = input.clone();
        active().build().unwrap().process(&mut whole);
        for size in [1, 7, 64, 511] {
            let mut split = input.clone();
            let mut dsp = active().build().unwrap();
            dsp.process::<f32>(&mut []);
            for block in split.chunks_mut(size) {
                dsp.process(block);
            }
            assert_eq!(whole, split);
        }
    }

    #[test]
    fn invalid_inputs_reset_state_and_finite_extremes_are_limited() {
        let mut dsp = active().build().unwrap();
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut input = [f32::MAX, invalid, -0.0, 0.1];
            dsp.process(&mut input);
            assert!(input[0].abs() <= 0.892);
            assert_eq!(input[1], 0.0);
            assert_eq!(input[2].to_bits(), (-0.0_f32).to_bits());
            assert_eq!(input[3], 0.1);
        }
    }

    #[test]
    fn rejects_invalid_settings_even_while_disabled() {
        for value in [f64::NAN, f64::INFINITY, -24.01, 0.01] {
            assert!(LimiterSettings {
                ceiling_db: value,
                ..Default::default()
            }
            .build()
            .is_err());
        }
        for value in [f64::NAN, f64::INFINITY, 0.99, 2000.01] {
            assert!(LimiterSettings {
                release_ms: value,
                ..Default::default()
            }
            .build()
            .is_err());
        }
        assert!(active().build_with_sample_rate(0).is_err());
        assert!(active().build_with_sample_rate(96000).is_err());
    }
}
