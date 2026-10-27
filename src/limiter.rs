//! Causal sample-peak limiter with instantaneous attack and exponential release.
//! No lookahead or added sample delay. This does not constrain inter-sample peaks
//! or peaks introduced by later stages in the chain.
use crate::{
    dsp::{envelope::decay, gain::amplitude},
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
    pub fn process_in_place(&mut self, samples: &mut [f64]) {
        if !self.enabled {
            return;
        }
        // At unity gain, a finite block wholly below the ceiling is an exact
        // dry pass: the per-sample loop would leave both samples and gain
        // unchanged. Check that common case once before entering the limiter.
        if self.gain == 1.0
            && samples.iter().all(|sample| {
                let input = *sample;
                input.is_finite() && input.abs() <= self.ceiling
            })
        {
            return;
        }
        for sample in samples {
            let input = *sample;
            if !input.is_finite() {
                *sample = 0.0;
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
                *sample = (input * self.gain).clamp(-self.ceiling, self.ceiling);
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
    fn disabled_preserves_float_bits() {
        let mut dsp = LimiterSettings::default().build().unwrap();
        let input = [
            0.0_f64,
            -0.0,
            f64::from_bits(0x7ff8_0000_0000_1234),
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::from_bits(1),
            f64::MAX,
        ];
        let mut output = input;
        dsp.process_in_place(&mut output);
        assert_eq!(input.map(f64::to_bits), output.map(f64::to_bits));
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
            let mut peak = [1.0_f64];
            dsp.process_in_place(&mut peak);
            assert!((peak[0] - ceiling).abs() < 1e-7);
            let initial_gain = dsp.gain;
            let mut silence = vec![0.0_f64; rate as usize / 10];
            dsp.process_in_place(&mut silence);
            assert!((dsp.gain - (1.0 - (1.0 - initial_gain) * (-1.0_f64).exp())).abs() < 1e-11);
            assert!(silence.iter().all(|v| *v == 0.0));
        }
    }

    #[test]
    fn output_peak_is_bounded() {
        fn check(tolerance: f64) {
            let settings = LimiterSettings {
                ceiling_db: -12.0,
                ..active()
            };
            let mut dsp = settings.build().unwrap();
            let mut input: Vec<f64> = (0..4096).map(|i| (i as f64 * 0.3).sin()).collect();
            dsp.process_in_place(&mut input);
            assert!(input
                .iter()
                .all(|s| s.abs() <= amplitude(-12.0) + tolerance));
        }
        check(1e-7);
    }

    #[test]
    fn state_crosses_block_boundaries_exactly() {
        let input: Vec<f64> = (0..4097).map(|i| (i as f64 * 0.31).sin()).collect();
        let mut whole = input.clone();
        active().build().unwrap().process_in_place(&mut whole);
        for size in [1, 7, 64, 511] {
            let mut split = input.clone();
            let mut dsp = active().build().unwrap();
            dsp.process_in_place(&mut []);
            for block in split.chunks_mut(size) {
                dsp.process_in_place(block);
            }
            assert_eq!(whole, split);
        }
    }

    #[test]
    fn invalid_inputs_reset_state_and_finite_extremes_are_limited() {
        let mut dsp = active().build().unwrap();
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut input = [f64::MAX, invalid, -0.0, 0.1];
            dsp.process_in_place(&mut input);
            assert!(input[0].abs() <= 0.892);
            assert_eq!(input[1], 0.0);
            assert_eq!(input[2].to_bits(), (-0.0_f64).to_bits());
            assert_eq!(input[3], 0.1);
        }
    }

    #[test]
    fn unity_gain_below_ceiling_preserves_bits() {
        let mut dsp = active().build().unwrap();
        let input = [
            0.0_f64,
            -0.0,
            f64::from_bits(1),
            -f64::from_bits(1),
            0.25,
            -0.75,
        ];
        let mut output = input;
        dsp.process_in_place(&mut output);
        assert_eq!(input.map(f64::to_bits), output.map(f64::to_bits));
        assert_eq!(dsp.gain, 1.0);
    }

    #[test]
    fn unsafe_blocks_fall_back_to_normal_limiter_processing() {
        let settings = LimiterSettings {
            ceiling_db: -6.0,
            ..active()
        };
        let ceiling = amplitude(settings.ceiling_db);
        let above_ceiling = f64::from_bits(ceiling.to_bits() + 1);
        assert!(above_ceiling > ceiling);

        let mut dsp = settings.build().unwrap();
        let mut input = [0.1_f64, above_ceiling];
        dsp.process_in_place(&mut input);
        assert!(input[1].abs() <= ceiling + 1e-7);
        assert!(dsp.gain < 1.0);

        // A later block remains limited while the gain releases.
        let mut input = [0.5_f64; 4];
        dsp.process_in_place(&mut input);
        assert!(input.iter().all(|sample| *sample < 0.5));

        // Invalid values must take the normal path and reset limiter history.
        let mut invalid = [f64::NAN, 0.25];
        dsp.process_in_place(&mut invalid);
        assert_eq!(invalid[0], 0.0);
        assert_eq!(invalid[1], 0.25);
        assert_eq!(dsp.gain, 1.0);
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
