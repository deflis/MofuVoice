//! 80 Hz, second-order Butterworth high-pass for mic stand and room rumble.
//! Causal IIR: no lookahead or sample buffering; frequency-dependent phase remains.
use crate::{pitch::Sample, RATE};

pub struct RumbleFilter {
    enabled: bool,
    b0: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl RumbleFilter {
    pub fn new(enabled: bool) -> Self {
        // Bilinear transform with prewarping at 80 Hz; Q = 1/sqrt(2).
        let k = (std::f64::consts::PI * 80.0 / RATE as f64).tan();
        let norm = 1.0 / (1.0 + std::f64::consts::SQRT_2 * k + k * k);
        Self {
            enabled,
            b0: norm,
            a1: 2.0 * (k * k - 1.0) * norm,
            a2: (1.0 - std::f64::consts::SQRT_2 * k + k * k) * norm,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if !self.enabled {
            return; // Preserve neutral bypass bits, including NaN payloads.
        }
        for sample in samples {
            let input = sample.normalized();
            if !input.is_finite() {
                self.z1 = 0.0;
                self.z2 = 0.0;
                *sample = T::default();
                continue;
            }
            let input = input.clamp(-16.0, 16.0);
            let output = self.b0 * input + self.z1;
            self.z1 = -2.0 * self.b0 * input - self.a1 * output + self.z2;
            self.z2 = self.b0 * input - self.a2 * output;
            // Prevent subnormal tails from increasing callback cost after silence.
            if self.z1.abs() < 1e-30 {
                self.z1 = 0.0;
            }
            if self.z2.abs() < 1e-30 {
                self.z2 = 0.0;
            }
            *sample = T::from_normalized(output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64) -> Vec<f32> {
        (0..RATE as usize)
            .map(|i| (0.2 * (std::f64::consts::TAU * hz * i as f64 / RATE as f64).sin()) as f32)
            .collect()
    }
    fn gain_db(hz: f64) -> f64 {
        let mut samples = tone(hz);
        RumbleFilter::new(true).process_in_place(&mut samples);
        let power = samples[24000..]
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            / 24000.0;
        10.0 * (power / 0.02).log10()
    }
    #[test]
    fn rejects_rumble_and_preserves_voice_band() {
        assert!(gain_db(20.0) < -24.0);
        assert!((gain_db(80.0) + 3.0103).abs() < 0.02);
        assert!(gain_db(200.0) > -0.12);
        assert!(gain_db(1000.0).abs() < 0.01);
        let mut dc = vec![0.2_f32; RATE as usize];
        RumbleFilter::new(true).process_in_place(&mut dc);
        assert!(dc[24000..].iter().all(|x| x.abs() < 1e-6));
    }
    #[test]
    fn bypass_preserves_bits_and_integer_formats() {
        let mut filter = RumbleFilter::new(false);
        let original = [
            f32::from_bits(0x7fc01234),
            -0.0,
            f32::from_bits(1),
            f32::INFINITY,
        ];
        let mut samples = original;
        filter.process_in_place(&mut samples);
        assert_eq!(samples.map(f32::to_bits), original.map(f32::to_bits));
        let mut short = [i16::MIN, -1, 0, 1, i16::MAX];
        let original = short;
        filter.process_in_place(&mut short);
        assert_eq!(short, original);
        let mut int = [i32::MIN, -1, 0, 1, i32::MAX];
        let original = int;
        filter.process_in_place(&mut int);
        assert_eq!(int, original);
    }
    #[test]
    fn block_boundaries_impulse_and_invalid_sample_recovery() {
        let mut whole = tone(150.0);
        whole[1000] = f32::NAN;
        whole[2000] = f32::INFINITY;
        let mut chunks = whole.clone();
        RumbleFilter::new(true).process_in_place(&mut whole);
        let mut filter = RumbleFilter::new(true);
        for chunk in chunks.chunks_mut(63) {
            filter.process_in_place(chunk);
        }
        assert_eq!(whole, chunks);
        assert!(whole.iter().all(|x| x.is_finite()));
        assert!(whole[3000..].iter().any(|x| x.abs() > 0.1));
        let mut impulse = [0.0_f32, 0.5, 0.0];
        RumbleFilter::new(true).process_in_place(&mut impulse);
        assert_eq!(impulse[0], 0.0);
        assert!(impulse[1] > 0.49);
    }
    #[test]
    fn active_integer_formats_match_float() {
        let input = tone(120.0);
        let mut float = input.clone();
        let mut short: Vec<_> = input
            .iter()
            .map(|x| i16::from_normalized(*x as f64))
            .collect();
        let mut int: Vec<_> = input
            .iter()
            .map(|x| i32::from_normalized(*x as f64))
            .collect();
        RumbleFilter::new(true).process_in_place(&mut float);
        RumbleFilter::new(true).process_in_place(&mut short);
        RumbleFilter::new(true).process_in_place(&mut int);
        for i in 0..input.len() {
            assert!((short[i].normalized() - float[i] as f64).abs() < 1e-4);
            assert!((int[i].normalized() - float[i] as f64).abs() < 1e-6);
        }
    }
}
