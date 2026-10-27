//! 80 Hz, second-order Butterworth high-pass for mic stand and room rumble.
//! Causal IIR: no lookahead or sample buffering; frequency-dependent phase remains.
use crate::{dsp::biquad::Biquad, RATE};

pub struct RumbleFilter {
    enabled: bool,
    filter: Biquad,
}

impl RumbleFilter {
    pub fn new(enabled: bool) -> Self {
        Self::with_sample_rate(enabled, RATE)
    }
    pub fn with_sample_rate(enabled: bool, rate: u32) -> Self {
        assert!(crate::validate_sample_rate(rate).is_ok());
        // Bilinear transform with prewarping at 80 Hz; Q = 1/sqrt(2).
        let k = (std::f64::consts::PI * 80.0 / rate as f64).tan();
        let norm = 1.0 / (1.0 + std::f64::consts::SQRT_2 * k + k * k);
        Self {
            enabled,
            filter: Biquad::new(
                [norm, -2.0 * norm, norm],
                [
                    2.0 * (k * k - 1.0) * norm,
                    (1.0 - std::f64::consts::SQRT_2 * k + k * k) * norm,
                ],
            ),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn process_in_place(&mut self, samples: &mut [f64]) {
        if !self.enabled {
            return; // Preserve neutral bypass bits, including NaN payloads.
        }
        for sample in samples {
            let input = *sample;
            if !input.is_finite() {
                self.filter.reset();
                *sample = 0.0;
                continue;
            }
            let input = input.clamp(-16.0, 16.0);
            let output = self.filter.process(input);
            *sample = output;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64) -> Vec<f64> {
        (0..RATE as usize)
            .map(|i| 0.2 * (std::f64::consts::TAU * hz * i as f64 / RATE as f64).sin())
            .collect()
    }
    fn gain_db(hz: f64) -> f64 {
        let mut samples = tone(hz);
        RumbleFilter::new(true).process_in_place(&mut samples);
        let power = samples[24000..].iter().map(|x| (*x).powi(2)).sum::<f64>() / 24000.0;
        10.0 * (power / 0.02).log10()
    }
    #[test]
    fn rejects_rumble_and_preserves_voice_band() {
        assert!(gain_db(20.0) < -24.0);
        assert!((gain_db(80.0) + 3.0103).abs() < 0.02);
        assert!(gain_db(200.0) > -0.12);
        assert!(gain_db(1000.0).abs() < 0.01);
        let mut dc = vec![0.2_f64; RATE as usize];
        RumbleFilter::new(true).process_in_place(&mut dc);
        assert!(dc[24000..].iter().all(|x| x.abs() < 1e-6));
    }
    #[test]
    fn bypass_preserves_bits() {
        let mut filter = RumbleFilter::new(false);
        let original = [
            f64::from_bits(0x7ff8_0000_0000_1234),
            -0.0,
            f64::from_bits(1),
            f64::INFINITY,
        ];
        let mut samples = original;
        filter.process_in_place(&mut samples);
        assert_eq!(samples.map(f64::to_bits), original.map(f64::to_bits));
    }
    #[test]
    fn block_boundaries_impulse_and_invalid_sample_recovery() {
        let mut whole = tone(150.0);
        whole[1000] = f64::NAN;
        whole[2000] = f64::INFINITY;
        let mut chunks = whole.clone();
        RumbleFilter::new(true).process_in_place(&mut whole);
        let mut filter = RumbleFilter::new(true);
        for chunk in chunks.chunks_mut(63) {
            filter.process_in_place(chunk);
        }
        assert_eq!(whole, chunks);
        assert!(whole.iter().all(|x| x.is_finite()));
        assert!(whole[3000..].iter().any(|x| x.abs() > 0.1));
        let mut impulse = [0.0_f64, 0.5, 0.0];
        RumbleFilter::new(true).process_in_place(&mut impulse);
        assert_eq!(impulse[0], 0.0);
        assert!(impulse[1] > 0.49);
    }
}
