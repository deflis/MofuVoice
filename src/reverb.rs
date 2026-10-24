//! Mono Schroeder reverberator: four parallel combs followed by two allpasses.
//! `decay_ms` is the comb feedback's nominal -60 dB decay time, not a measured
//! room response. The dry path has no delay; wet onset includes comb transit.
use crate::dsp::Sample;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReverbSettings {
    pub enabled: bool,
    pub mix: f64,
    pub pre_delay_ms: f64,
    pub decay_ms: f64,
}

impl Default for ReverbSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mix: 0.15,
            pre_delay_ms: 20.0,
            decay_ms: 1200.0,
        }
    }
}

impl ReverbSettings {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.mix.is_finite() || !(0.0..=1.0).contains(&self.mix) {
            return Err("reverb mix must be finite and between 0 and 1");
        }
        if !self.pre_delay_ms.is_finite() || !(0.0..=200.0).contains(&self.pre_delay_ms) {
            return Err("reverb pre-delay must be finite and between 0 and 200 ms");
        }
        if !self.decay_ms.is_finite() || !(100.0..=10000.0).contains(&self.decay_ms) {
            return Err("reverb decay must be finite and between 100 and 10000 ms");
        }
        Ok(())
    }

    pub fn build(self) -> Result<Reverb, &'static str> {
        self.build_with_sample_rate(crate::RATE)
    }

    pub fn build_with_sample_rate(self, sample_rate: u32) -> Result<Reverb, &'static str> {
        self.validate()?;
        crate::validate_sample_rate(sample_rate)?;
        let samples = |ms: f64| (ms * sample_rate as f64 / 1000.0).round() as usize;
        let combs = [29.7, 37.1, 41.1, 43.7].map(|ms| {
            let length = samples(ms).max(1);
            Comb {
                delay: Delay::new(length),
                feedback: 10.0_f64
                    .powf(-3.0 * length as f64 / (sample_rate as f64 * self.decay_ms / 1000.0)),
            }
        });
        Ok(Reverb {
            settings: self,
            pre_delay: Delay::new(samples(self.pre_delay_ms)),
            combs,
            allpasses: [5.0, 1.7].map(|ms| Delay::new(samples(ms).max(1))),
        })
    }
}

struct Delay {
    samples: Vec<f64>,
    position: usize,
}
impl Delay {
    fn new(length: usize) -> Self {
        Self {
            samples: vec![0.0; length],
            position: 0,
        }
    }
    fn read(&self) -> f64 {
        self.samples[self.position]
    }
    fn push(&mut self, value: f64) {
        // Avoid denormal tails without allocating or scanning the delay lines.
        self.samples[self.position] = if value.abs() < 1e-30 { 0.0 } else { value };
        self.position += 1;
        if self.position == self.samples.len() {
            self.position = 0;
        }
    }
    fn delayed(&mut self, input: f64) -> f64 {
        if self.samples.is_empty() {
            return input;
        }
        let output = self.read();
        self.push(input);
        output
    }
}
struct Comb {
    delay: Delay,
    feedback: f64,
}

pub struct Reverb {
    settings: ReverbSettings,
    pre_delay: Delay,
    combs: [Comb; 4],
    allpasses: [Delay; 2],
}
impl Reverb {
    pub fn enabled(&self) -> bool {
        self.settings.enabled && self.settings.mix != 0.0
    }

    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if !self.enabled() {
            return;
        }
        for sample in samples {
            // Reject nonfinite input before it can poison any feedback history.
            let input = sample.normalized();
            let dry = if input.is_finite() {
                input.clamp(-16.0, 16.0)
            } else {
                0.0
            };
            let delayed = self.pre_delay.delayed(dry);
            let mut wet = 0.0;
            for comb in &mut self.combs {
                let output = comb.delay.read();
                comb.delay.push(delayed + comb.feedback * output);
                wet += output * 0.25;
            }
            for delay in &mut self.allpasses {
                let stored = delay.read();
                let output = stored - 0.5 * wet;
                delay.push(wet + 0.5 * output);
                wet = output;
            }
            *sample = T::from_normalized((1.0 - self.settings.mix) * dry + self.settings.mix * wet);
        }
    }

    pub fn process<T: Sample>(&mut self, samples: &mut [T]) {
        self.process_in_place(samples);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active() -> ReverbSettings {
        ReverbSettings {
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn disabled_and_zero_mix_preserve_every_bit() {
        for settings in [
            ReverbSettings::default(),
            ReverbSettings {
                mix: 0.0,
                ..active()
            },
        ] {
            let original = [
                f32::from_bits(0x7fc01234),
                -0.0,
                f32::from_bits(1),
                f32::INFINITY,
            ];
            let mut samples = original;
            settings.build().unwrap().process(&mut samples);
            assert_eq!(samples.map(f32::to_bits), original.map(f32::to_bits));
            let mut shorts = [i16::MIN, -1, 0, 1, i16::MAX];
            let before = shorts;
            settings.build().unwrap().process(&mut shorts);
            assert_eq!(shorts, before);
            let mut ints = [i32::MIN, -1, 0, 1, i32::MAX];
            let before = ints;
            settings.build().unwrap().process(&mut ints);
            assert_eq!(ints, before);
        }
    }

    #[test]
    fn wet_onset_and_nominal_decay_at_supported_rates() {
        for rate in crate::SAMPLE_RATES {
            let settings = ReverbSettings {
                mix: 1.0,
                decay_ms: 500.0,
                ..active()
            };
            let mut data = vec![0.0_f32; rate as usize * 2];
            data[0] = 1.0;
            settings
                .build_with_sample_rate(rate)
                .unwrap()
                .process(&mut data);
            let first =
                (0.020 * rate as f64).round() as usize + (0.0297 * rate as f64).round() as usize;
            assert!(data[..first].iter().all(|x| *x == 0.0));
            assert!(data[first].abs() > 0.01);
            let energy = |range: std::ops::Range<usize>| {
                data[range].iter().map(|x| (*x as f64).powi(2)).sum::<f64>()
            };
            assert!(energy(rate as usize..rate as usize * 2) < energy(0..rate as usize / 2) * 1e-9);
        }
    }

    #[test]
    fn dry_path_is_immediate_and_restart_has_no_tail() {
        let mut data = [1.0_f32, 0.0, 0.0];
        active().build().unwrap().process(&mut data);
        assert_eq!(data, [0.85, 0.0, 0.0]);
        let mut silence = vec![0.0_f32; crate::RATE as usize];
        active().build().unwrap().process(&mut silence);
        assert!(silence.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn blocks_formats_and_nonfinite_recovery() {
        let original: Vec<f32> = (0..16000)
            .map(|i| if i % 1000 == 0 { 0.125 } else { 0.0 })
            .collect();
        let mut whole = original.clone();
        let mut chunks = original.clone();
        active().build().unwrap().process(&mut whole);
        let mut dsp = active().build().unwrap();
        for chunk in chunks.chunks_mut(63) {
            dsp.process(chunk);
        }
        assert_eq!(whole, chunks);
        let mut shorts: Vec<i16> = original
            .iter()
            .map(|x| i16::from_normalized(*x as f64))
            .collect();
        let mut ints: Vec<i32> = original
            .iter()
            .map(|x| i32::from_normalized(*x as f64))
            .collect();
        active().build().unwrap().process(&mut shorts);
        active().build().unwrap().process(&mut ints);
        for ((float, short), int) in whole.iter().zip(shorts).zip(ints) {
            assert!((short.normalized() - *float as f64).abs() < 1.6e-5);
            assert!((int.normalized() - *float as f64).abs() < 1e-8);
        }
        let mut invalid = vec![0.0_f32; 16000];
        invalid[..4].copy_from_slice(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX]);
        invalid[1000] = 0.5;
        active().build().unwrap().process(&mut invalid);
        assert!(invalid.iter().all(|x| x.is_finite()));
        assert!(invalid[1000] > 0.0);
    }

    #[test]
    fn rejects_invalid_settings_even_when_disabled() {
        for invalid in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(ReverbSettings {
                mix: invalid,
                ..Default::default()
            }
            .build()
            .is_err());
            assert!(ReverbSettings {
                pre_delay_ms: invalid,
                ..Default::default()
            }
            .build()
            .is_err());
            assert!(ReverbSettings {
                decay_ms: invalid,
                ..Default::default()
            }
            .build()
            .is_err());
        }
        assert!(ReverbSettings {
            mix: 1.1,
            ..Default::default()
        }
        .build()
        .is_err());
        assert!(ReverbSettings {
            pre_delay_ms: 201.0,
            ..Default::default()
        }
        .build()
        .is_err());
        assert!(ReverbSettings {
            decay_ms: 10001.0,
            ..Default::default()
        }
        .build()
        .is_err());
        assert!(active().build_with_sample_rate(0).is_err());
    }
}
