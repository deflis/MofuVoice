//! Band-limited fractional reads of TD-PSOLA grains, with fixed formant ratio.
//! Tables are built before streaming; no FFT or external DSP implementation.
use std::f64::consts::{PI, TAU};

pub(super) const RADIUS: usize = 64;
const TAPS: usize = 2 * RADIUS + 1;
const PHASES: usize = 256;

pub(super) struct Resampler {
    kernels: Box<[f64]>,
}

impl Resampler {
    pub fn new(speed: f64) -> Self {
        assert!(speed.is_finite() && (0.5..=2.0).contains(&speed));
        // Leave a transition band before the lower input/output Nyquist limit.
        let cutoff = 0.45 / speed.max(1.0);
        let mut kernels = vec![0.0; (PHASES + 1) * TAPS];
        for phase in 0..=PHASES {
            let fraction = phase as f64 / PHASES as f64;
            let kernel = &mut kernels[phase * TAPS..(phase + 1) * TAPS];
            for (i, tap) in kernel.iter_mut().enumerate() {
                let distance = i as f64 - RADIUS as f64 - fraction;
                let normalized = distance / RADIUS as f64;
                let window = if normalized.abs() < 1.0 {
                    0.42 + 0.5 * (PI * normalized).cos() + 0.08 * (TAU * normalized).cos()
                } else {
                    0.0
                };
                let sinc = if distance.abs() < 1e-12 {
                    2.0 * cutoff
                } else {
                    (TAU * cutoff * distance).sin() / (PI * distance)
                };
                *tap = window * sinc;
            }
            let sum: f64 = kernel.iter().sum();
            for tap in kernel {
                *tap /= sum;
            }
        }
        Self {
            kernels: kernels.into_boxed_slice(),
        }
    }

    pub fn sample<const N: usize>(&self, input: &[f64; N], available: usize, position: f64) -> f64 {
        let center = position.floor() as isize;
        let first = center - RADIUS as isize;
        let last = center + RADIUS as isize;
        if last < 0 {
            return 0.0;
        }
        assert!(
            last < available as isize,
            "resampler read ahead: last={last}, available={available}, position={position}"
        );
        assert!(
            first >= 0 || available < N,
            "resampler read expired startup history"
        );
        assert!(
            first < 0 || first as usize >= available.saturating_sub(N),
            "resampler read expired input"
        );
        let phase = (position - center as f64) * PHASES as f64;
        let index = phase.floor() as usize;
        let fraction = phase - index as f64;
        let lower = &self.kernels[index * TAPS..(index + 1) * TAPS];
        let upper = &self.kernels[(index + 1) * TAPS..(index + 2) * TAPS];
        let skip = (-first).max(0) as usize;
        if N < TAPS {
            return (skip..TAPS)
                .map(|i| {
                    input[(first + i as isize) as usize % N]
                        * (lower[i] + fraction * (upper[i] - lower[i]))
                })
                .sum();
        }
        let start = (first + skip as isize) as usize % N;
        let count = (TAPS - skip).min(N - start);
        let dot = |samples: &[f64], lo: &[f64], hi: &[f64]| {
            let mut sums = [0.0; 4];
            for ((samples, lo), hi) in samples
                .chunks_exact(4)
                .zip(lo.chunks_exact(4))
                .zip(hi.chunks_exact(4))
            {
                for lane in 0..4 {
                    sums[lane] += samples[lane] * (lo[lane] + fraction * (hi[lane] - lo[lane]));
                }
            }
            let mut sum = (sums[0] + sums[1]) + (sums[2] + sums[3]);
            for ((&sample, &lo), &hi) in samples
                .chunks_exact(4)
                .remainder()
                .iter()
                .zip(lo.chunks_exact(4).remainder())
                .zip(hi.chunks_exact(4).remainder())
            {
                sum += sample * (lo + fraction * (hi - lo));
            }
            sum
        };
        let split = skip + count;
        dot(
            &input[start..start + count],
            &lower[skip..split],
            &upper[skip..split],
        ) + dot(&input[..TAPS - split], &lower[split..], &upper[split..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_reads_preserve_dc_and_passband_and_reject_aliases() {
        for speed in [0.5, 0.83, 1.157356506200304, 2.0] {
            let sampler = Resampler::new(speed);
            let dc = [0.37; 4096];
            for phase in [0.0, 0.17, 0.5, 0.99999] {
                assert!((sampler.sample(&dc, 4096, 2000.0 + phase) - 0.37).abs() < 1e-12);
            }
            for frequency in [0.02, 0.1, 0.18] {
                let tone =
                    std::array::from_fn::<_, 4096, _>(|i| (TAU * frequency * i as f64).cos());
                let mut maximum = 0.0_f64;
                for i in 0..1024 {
                    let position = 128.371 + speed * i as f64;
                    let expected = (TAU * frequency * position).cos();
                    maximum = maximum.max((sampler.sample(&tone, 4096, position) - expected).abs());
                }
                assert!(
                    maximum < 0.001,
                    "speed={speed}, f={frequency}, error={maximum}"
                );
            }
        }
        let sampler = Resampler::new(2.0);
        for frequency in [0.27, 0.33, 0.43, 0.49] {
            let tone = std::array::from_fn::<_, 4096, _>(|i| (TAU * frequency * i as f64).cos());
            let maximum = (0..1024)
                .map(|i| sampler.sample(&tone, 4096, 128.371 + 2.0 * i as f64).abs())
                .fold(0.0_f64, f64::max);
            assert!(maximum < 0.0001, "alias f={frequency}, peak={maximum}");
        }
    }

    #[test]
    fn parallel_sum_matches_scalar_at_startup_and_ring_boundaries() {
        let sampler = Resampler::new(1.3);
        let input = std::array::from_fn::<_, 1024, _>(|i| (i as f64 * 0.17).sin());
        for position in [-65.2_f64, -63.7, -0.1, 0.0, 12.31, 128.9, 2047.31, 2050.7] {
            let center = position.floor() as isize;
            let first = center - RADIUS as isize;
            let phase = (position - center as f64) * PHASES as f64;
            let index = phase.floor() as usize;
            let fraction = phase - index as f64;
            let mut expected = 0.0;
            for i in 0..TAPS {
                if first + i as isize >= 0 {
                    let lower = sampler.kernels[index * TAPS + i];
                    let upper = sampler.kernels[(index + 1) * TAPS + i];
                    expected += input[(first + i as isize) as usize % input.len()]
                        * (lower + fraction * (upper - lower));
                }
            }
            let available = if position < 1000.0 { 512 } else { 2200 };
            assert!((sampler.sample(&input, available, position) - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn fractional_reads_match_after_ring_wrap() {
        let sampler = Resampler::new(1.3);
        let unwrapped = std::array::from_fn::<_, 4096, _>(|i| (i as f64 * 0.17).sin());
        let mut ring = [0.0; 1024];
        for i in 2048..3072 {
            ring[i % 1024] = unwrapped[i];
        }
        for i in 2200..2900 {
            assert!(
                (sampler.sample(&ring, 3072, i as f64 + 0.31)
                    - sampler.sample(&unwrapped, 4096, i as f64 + 0.31))
                .abs()
                    < 1e-12
            );
        }
    }
}
