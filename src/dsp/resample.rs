//! Streaming windowed-sinc rate conversion. Storage and kernels are prepared off-thread.
use std::f64::consts::{PI, TAU};

const RADIUS: usize = 64;
const PHASES: usize = 256;

pub(crate) struct RateConverter {
    input_rate: u32,
    output_rate: u32,
    kernels: Vec<f64>,
    history: Vec<f64>,
    radius: usize,
    taps: usize,
    written: u64,
    next: u64,
}

impl RateConverter {
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, &'static str> {
        if !(8000..=192000).contains(&input_rate) || !(8000..=192000).contains(&output_rate) {
            return Err("Resampling rates must be 8000..192000 Hz");
        }
        let radius = RADIUS * (input_rate as f64 / output_rate as f64).max(1.0).ceil() as usize;
        let taps = radius * 2 + 1;
        let cutoff = 0.45 * (output_rate as f64 / input_rate as f64).min(1.0);
        let mut kernels = vec![0.0; (PHASES + 1) * taps];
        for phase in 0..=PHASES {
            let kernel = &mut kernels[phase * taps..(phase + 1) * taps];
            for (i, tap) in kernel.iter_mut().enumerate() {
                let x = i as f64 - radius as f64 - phase as f64 / PHASES as f64;
                let t = x / radius as f64;
                let window = if t.abs() < 1.0 {
                    0.42 + 0.5 * (PI * t).cos() + 0.08 * (TAU * t).cos()
                } else {
                    0.0
                };
                *tap = window
                    * if x.abs() < 1e-12 {
                        2.0 * cutoff
                    } else {
                        (TAU * cutoff * x).sin() / (PI * x)
                    };
            }
            let sum: f64 = kernel.iter().sum();
            for tap in kernel {
                *tap /= sum;
            }
        }
        Ok(Self {
            input_rate,
            output_rate,
            kernels,
            history: vec![0.0; (taps + 1).next_power_of_two()],
            radius,
            taps,
            written: 0,
            next: 0,
        })
    }

    pub fn delay_ms(input_rate: u32, output_rate: u32) -> f64 {
        if input_rate == output_rate {
            0.0
        } else {
            (RADIUS as f64 * (input_rate as f64 / output_rate as f64).max(1.0).ceil()) * 1000.0
                / input_rate as f64
        }
    }

    pub fn push(&mut self, sample: f64, mut emit: impl FnMut(f64)) {
        if self.input_rate == self.output_rate {
            emit(sample);
            return;
        }
        let slot = self.written as usize % self.history.len();
        self.history[slot] = sample;
        self.written += 1;
        // Integer rational time keeps sample counts and phase independent of callback boundaries.
        while self.next / (self.output_rate as u64) < self.written {
            let center = (self.next / self.output_rate as u64) as i64 - self.radius as i64;
            let phase = (self.next % self.output_rate as u64) as f64 * PHASES as f64
                / self.output_rate as f64;
            let index = phase.floor() as usize;
            let fraction = phase - index as f64;
            let mut value = 0.0;
            for i in 0..self.taps {
                let position = center + i as i64 - self.radius as i64;
                if position >= 0 {
                    let a = self.kernels[index * self.taps + i];
                    let b = self.kernels[(index + 1) * self.taps + i];
                    value += self.history[position as usize % self.history.len()]
                        * (a + fraction * (b - a));
                }
            }
            emit(value);
            self.next += self.input_rate as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render(input_rate: u32, output_rate: u32, frequency: f64, block: usize) -> Vec<f64> {
        let input: Vec<_> = (0..input_rate)
            .map(|i| (TAU * frequency * i as f64 / input_rate as f64).sin())
            .collect();
        let mut converter = RateConverter::new(input_rate, output_rate).unwrap();
        let mut output = Vec::new();
        for chunk in input.chunks(block) {
            for &sample in chunk {
                converter.push(sample, |s| output.push(s));
            }
        }
        output
    }
    #[test]
    fn rational_counts_and_phase_are_independent_of_blocks() {
        for (a, b) in [
            (44100, 48000),
            (48000, 44100),
            (96000, 44100),
            (44100, 192000),
        ] {
            let output = render(a, b, 1000.0, 73);
            assert_eq!(output.len(), b as usize);
            assert_eq!(output, render(a, b, 1000.0, 512));
            let delay = RateConverter::delay_ms(a, b) / 1000.0;
            let error = output
                .iter()
                .enumerate()
                .skip(1024)
                .map(|(i, s)| (s - (TAU * 1000.0 * (i as f64 / b as f64 - delay)).sin()).powi(2))
                .sum::<f64>()
                / (output.len() - 1024) as f64;
            assert!(error.sqrt() < 0.001, "{a}->{b}: {error}");
        }
    }
    #[test]
    fn downsampling_rejects_aliases() {
        let output = render(96000, 44100, 30000.0, 127);
        let rms = (output[1024..].iter().map(|s| s * s).sum::<f64>()
            / (output.len() - 1024) as f64)
            .sqrt();
        assert!(rms < 0.0001, "alias RMS {rms}");
    }
    #[test]
    fn validates_rates_and_preserves_equal_rate_bits() {
        assert!(RateConverter::new(0, 48000).is_err());
        assert!(RateConverter::new(48000, 192001).is_err());
        let mut converter = RateConverter::new(48000, 48000).unwrap();
        for sample in [-0.0, f64::from_bits(1), 0.5] {
            converter.push(sample, |s| assert_eq!(s.to_bits(), sample.to_bits()));
        }
    }
}
