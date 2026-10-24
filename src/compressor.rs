//! Independent hard-knee peak compressor without makeup gain or a peak guard.
//! A 30 ms peak-hold decay drives attack/release smoothing of attenuation in dB.
//! Attack permits transient overshoot: this stage is not a limiter. No lookahead,
//! buffered delay, or processing-time allocation is used.
use crate::{
    dsp::{
        envelope::{decay, update_rate},
        gain::{amplitude, amplitude_db},
        Sample,
    },
    RATE,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompressorSettings {
    pub enabled: bool,
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
}

impl Default for CompressorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: -12.0,
            ratio: 3.0,
            attack_ms: 5.0,
            release_ms: 150.0,
        }
    }
}

impl CompressorSettings {
    pub fn validate(self) -> Result<(), &'static str> {
        for (value, min, max) in [
            (self.threshold_db, -36.0, 0.0),
            (self.ratio, 1.0, 20.0),
            (self.attack_ms, 0.1, 100.0),
            (self.release_ms, 10.0, 2000.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err("Compressor requires threshold -36..0 dBFS, ratio 1..20, attack 0.1..100 ms and release 10..2000 ms");
            }
        }
        Ok(())
    }

    pub fn build(self) -> Result<Compressor, &'static str> {
        self.build_with_sample_rate(RATE)
    }

    pub fn build_with_sample_rate(self, sample_rate: u32) -> Result<Compressor, &'static str> {
        self.validate()?;
        crate::validate_sample_rate(sample_rate)?;
        Ok(Compressor {
            bypass: !self.enabled || self.ratio == 1.0,
            threshold: amplitude(self.threshold_db),
            threshold_db: self.threshold_db,
            slope: 1.0 - 1.0 / self.ratio,
            peak_decay: decay(30.0, sample_rate as f64),
            attack_rate: update_rate(self.attack_ms, sample_rate as f64),
            release_rate: update_rate(self.release_ms, sample_rate as f64),
            peak: 0.0,
            reduction_db: 0.0,
        })
    }
}

pub struct Compressor {
    bypass: bool,
    threshold: f64,
    threshold_db: f64,
    slope: f64,
    peak_decay: f64,
    attack_rate: f64,
    release_rate: f64,
    peak: f64,
    reduction_db: f64,
}

impl Compressor {
    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if self.bypass {
            return;
        }
        for sample in samples {
            let input = sample.normalized();
            if !input.is_finite() {
                *sample = T::default();
                self.peak = 0.0;
                self.reduction_db = 0.0;
                continue;
            }
            self.peak = (self.peak * self.peak_decay).max(input.abs());
            let target = if self.peak > self.threshold {
                (amplitude_db(self.peak, 1e-30) - self.threshold_db) * self.slope
            } else {
                0.0
            };
            let rate = if target > self.reduction_db {
                self.attack_rate
            } else {
                self.release_rate
            };
            self.reduction_db += (target - self.reduction_db) * rate;
            if self.reduction_db > 0.0 {
                *sample = T::from_normalized(input * amplitude(-self.reduction_db));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active() -> CompressorSettings {
        CompressorSettings {
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn disabled_and_unity_ratio_preserve_all_bits() {
        for settings in [
            CompressorSettings::default(),
            CompressorSettings {
                ratio: 1.0,
                ..active()
            },
        ] {
            let mut dsp = settings.build().unwrap();
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
    }

    #[test]
    fn steady_state_matches_hard_knee_curve_in_all_formats() {
        fn check<T: Sample>(tolerance: f64) {
            for rate in crate::SAMPLE_RATES {
                let settings = active();
                for input in [0.01, 0.25, 0.5, 0.9, -0.5, -0.9] {
                    let input = T::from_normalized(input);
                    let mut samples = vec![input; rate as usize];
                    settings
                        .build_with_sample_rate(rate)
                        .unwrap()
                        .process_in_place(&mut samples);
                    let level = amplitude_db(input.normalized().abs(), 1e-30);
                    let reduction =
                        (level - settings.threshold_db).max(0.0) * (1.0 - 1.0 / settings.ratio);
                    let expected = input.normalized() * amplitude(-reduction);
                    assert!((samples.last().unwrap().normalized() - expected).abs() <= tolerance);
                }
            }
        }
        check::<f32>(1e-7);
        check::<i16>(0.5 / 32768.0 + 1e-12);
        check::<i32>(0.5 / 2147483648.0 + 1e-12);
    }

    #[test]
    fn attack_and_release_follow_time_constants_without_peak_guard() {
        for rate in crate::SAMPLE_RATES {
            let settings = CompressorSettings {
                attack_ms: 10.0,
                release_ms: 100.0,
                ..active()
            };
            let mut dsp = settings.build_with_sample_rate(rate).unwrap();
            let mut loud = vec![1.0_f32; rate as usize / 100];
            dsp.process_in_place(&mut loud);
            assert!(
                loud[0] > 0.99,
                "attack transients must not be limited by an implicit guard"
            );
            let target = -settings.threshold_db * (1.0 - 1.0 / settings.ratio);
            assert!((dsp.reduction_db - target * (1.0 - (-1.0_f64).exp())).abs() < 1e-11);
            // Once the peak detector falls below threshold, gain recovers with
            // the configured release constant independently of detector history.
            dsp.peak = 0.0;
            let initial = dsp.reduction_db;
            let mut quiet = vec![0.01_f32; rate as usize / 10];
            dsp.process_in_place(&mut quiet);
            assert!((dsp.reduction_db - initial * (-1.0_f64).exp()).abs() < 1e-11);
            assert!(quiet.last().unwrap() > quiet.first().unwrap());
        }
    }

    #[test]
    fn block_boundaries_and_empty_blocks_preserve_state() {
        let input: Vec<f32> = (0..4097).map(|i| (i as f32 * 0.31).sin()).collect();
        let mut whole = input.clone();
        active().build().unwrap().process_in_place(&mut whole);
        for size in [1, 7, 64, 511] {
            let mut split = input.clone();
            let mut dsp = active().build().unwrap();
            for block in split.chunks_mut(size) {
                dsp.process_in_place::<f32>(&mut []);
                dsp.process_in_place(block);
            }
            assert_eq!(whole, split);
        }
    }

    #[test]
    fn invalid_samples_reset_and_finite_extremes_do_not_poison_state() {
        let mut dsp = active().build().unwrap();
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut input = [f32::MAX, -f32::MAX, invalid, -0.0, 0.01];
            dsp.process_in_place(&mut input);
            assert!(input.iter().all(|v| v.is_finite()));
            assert_eq!(input[2], 0.0);
            assert_eq!(input[3].to_bits(), (-0.0_f32).to_bits());
            assert_eq!(input[4], 0.01);
        }
    }

    #[test]
    fn invalid_settings_are_rejected_even_when_disabled() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for settings in [
                CompressorSettings {
                    threshold_db: invalid,
                    ..Default::default()
                },
                CompressorSettings {
                    ratio: invalid,
                    ..Default::default()
                },
                CompressorSettings {
                    attack_ms: invalid,
                    ..Default::default()
                },
                CompressorSettings {
                    release_ms: invalid,
                    ..Default::default()
                },
            ] {
                assert!(settings.build().is_err());
            }
        }
        for settings in [
            CompressorSettings {
                threshold_db: -36.1,
                ..active()
            },
            CompressorSettings {
                threshold_db: 0.1,
                ..active()
            },
            CompressorSettings {
                ratio: 0.9,
                ..active()
            },
            CompressorSettings {
                ratio: 20.1,
                ..active()
            },
            CompressorSettings {
                attack_ms: 0.09,
                ..active()
            },
            CompressorSettings {
                attack_ms: 100.1,
                ..active()
            },
            CompressorSettings {
                release_ms: 9.9,
                ..active()
            },
            CompressorSettings {
                release_ms: 2000.1,
                ..active()
            },
        ] {
            assert!(settings.build().is_err());
        }
        assert!(active().build_with_sample_rate(0).is_err());
    }
}
