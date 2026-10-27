//! Conservative causal tail attenuation, not direct/reverberant source separation.
//!
//! A short envelope is compared with a slowly decaying peak history. Falling
//! energy is attenuated by at most `amount`; onsets and sustained energy pass.
//! This can also attenuate quiet syllables and natural voice decays. It cannot
//! remove early reflections or distinguish room tails from intentional fades.
//! No lookahead, sample delay, allocation, or I/O occurs during processing.
use crate::RATE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DereverbSettings {
    pub enabled: bool,
    pub amount: f64,
    pub decay_ms: f64,
}
impl Default for DereverbSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            amount: 0.5,
            decay_ms: 250.0,
        }
    }
}
impl DereverbSettings {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.amount.is_finite()
            || !(0.0..=1.0).contains(&self.amount)
            || !self.decay_ms.is_finite()
            || !(20.0..=2000.0).contains(&self.decay_ms)
        {
            return Err("Dereverb requires amount 0..1 and decay 20..2000 ms");
        }
        Ok(())
    }
    pub fn build(self) -> Result<Dereverb, &'static str> {
        self.build_with_sample_rate(RATE)
    }
    pub fn build_with_sample_rate(self, sample_rate: u32) -> Result<Dereverb, &'static str> {
        self.validate()?;
        crate::validate_sample_rate(sample_rate)?;
        let decay = |ms: f64| (-1.0 / (ms * 0.001 * sample_rate as f64)).exp();
        Ok(Dereverb {
            settings: self,
            short_decay: decay(10.0),
            tail_decay: decay(self.decay_ms),
            gain_decay: decay(5.0),
            short: 0.0,
            tail: 0.0,
            gain: 1.0,
        })
    }
}

pub struct Dereverb {
    settings: DereverbSettings,
    short_decay: f64,
    tail_decay: f64,
    gain_decay: f64,
    short: f64,
    tail: f64,
    gain: f64,
}
impl Dereverb {
    pub fn process_in_place(&mut self, samples: &mut [f64]) {
        if !self.settings.enabled || self.settings.amount == 0.0 {
            return;
        }
        for sample in samples {
            let input = *sample;
            if !input.is_finite() {
                self.short = 0.0;
                self.tail = 0.0;
                self.gain = 1.0;
                *sample = 0.0;
                continue;
            }
            // Absolute amplitudes rather than squared powers avoid overflow for
            // finite floating point input. Both envelopes are bounded by input.
            let magnitude = input.abs();
            self.short = magnitude.max(self.short * self.short_decay);
            self.tail = magnitude.max(self.tail * self.tail_decay);
            let ratio = if self.tail > 1e-30 {
                (self.short / self.tail).clamp(0.0, 1.0)
            } else {
                self.short = 0.0;
                self.tail = 0.0;
                1.0
            };
            let target = 1.0 - self.settings.amount * (1.0 - ratio);
            // Recover immediately on renewed attacks, smoothing only attenuation.
            // This prevents a previous tail from swallowing the next transient.
            self.gain = if target >= self.gain {
                target
            } else {
                self.gain_decay * self.gain + (1.0 - self.gain_decay) * target
            };
            if self.gain != 1.0 {
                *sample = input * self.gain;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active() -> DereverbSettings {
        DereverbSettings {
            enabled: true,
            ..DereverbSettings::default()
        }
    }
    #[test]
    fn disabled_and_zero_amount_preserve_all_bits() {
        for settings in [
            DereverbSettings::default(),
            DereverbSettings {
                amount: 0.0,
                ..active()
            },
        ] {
            let mut dsp = settings.build().unwrap();
            let source = [
                -0.0_f64,
                0.0,
                f64::from_bits(1),
                f64::from_bits(0x7ff8_0000_0000_0005),
                f64::INFINITY,
            ];
            let mut actual = source;
            dsp.process_in_place(&mut actual);
            assert_eq!(source.map(f64::to_bits), actual.map(f64::to_bits));
        }
    }
    #[test]
    fn validates_disabled_settings_and_sample_rates() {
        for amount in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
            assert!(DereverbSettings {
                amount,
                ..DereverbSettings::default()
            }
            .build()
            .is_err());
        }
        for decay_ms in [19.0, 2001.0, f64::NAN, f64::NEG_INFINITY] {
            assert!(DereverbSettings {
                decay_ms,
                ..DereverbSettings::default()
            }
            .build()
            .is_err());
        }
        assert!(active().build_with_sample_rate(0).is_err());
        assert!(active().build_with_sample_rate(96_000).is_err());
    }
    #[test]
    fn reduces_synthetic_tail_without_amplification_and_preserves_onsets() {
        for rate in [44_100, 48_000] {
            let source: Vec<f64> = (0..rate / 2)
                .map(|i| {
                    let time = i as f64 / rate as f64;
                    (-time / 0.05).exp() * (std::f64::consts::TAU * 200.0 * time).cos()
                })
                .collect();
            let mut actual = source.clone();
            let mut dsp = active().build_with_sample_rate(rate).unwrap();
            dsp.process_in_place(&mut actual);
            assert_eq!(actual[0], source[0]);
            let start = (rate / 10) as usize;
            let energy = |s: &[f64]| s.iter().map(|x| (*x).powi(2)).sum::<f64>();
            assert!(energy(&actual[start..]) < energy(&source[start..]) * 0.5);
            for (&out, &input) in actual.iter().zip(&source) {
                assert!(out.abs() <= input.abs());
            }
            let mut attack = [1.0_f64];
            dsp.process_in_place(&mut attack);
            assert_eq!(attack[0], 1.0);
        }
    }
    #[test]
    fn sustained_level_and_silence_are_unchanged() {
        let mut dsp = active().build().unwrap();
        let mut signal = [0.25_f64; 2048];
        dsp.process_in_place(&mut signal);
        assert_eq!(signal, [0.25; 2048]);
        let mut silence = [0.0_f64; 4096];
        dsp.process_in_place(&mut silence);
        assert_eq!(silence, [0.0; 4096]);
    }
    #[test]
    fn state_is_independent_of_block_boundaries() {
        fn check(source: Vec<f64>) {
            let mut whole = source.clone();
            let mut split = source;
            active().build().unwrap().process_in_place(&mut whole);
            let mut dsp = active().build().unwrap();
            dsp.process_in_place(&mut []);
            for block in split.chunks_mut(37) {
                dsp.process_in_place(block);
            }
            assert!(whole == split);
        }
        let source: Vec<f64> = (0..8000)
            .map(|i| (i as f64 * 0.09).sin() * (-i as f64 / 1500.0).exp())
            .collect();
        check(source.clone());
    }
    #[test]
    fn abnormal_input_resets_history_and_finite_extremes_remain_finite() {
        let mut dsp = active().build().unwrap();
        let mut input = [
            f64::MAX,
            f64::MIN,
            f64::NAN,
            0.25,
            f64::INFINITY,
            0.5,
            f64::NEG_INFINITY,
            -0.5,
        ];
        dsp.process_in_place(&mut input);
        assert!(input.iter().all(|x| x.is_finite()));
        assert_eq!(&input[2..], &[0.0, 0.25, 0.0, 0.5, 0.0, -0.5]);
    }
}
