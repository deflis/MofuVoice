//! Band-limited fractional reads of TD-PSOLA grains, with fixed formant ratio.
//! Tables are built before streaming; no FFT or external DSP implementation.
use std::f64::consts::{PI, TAU};

pub(super) const RADIUS: usize = 64;
const TAPS: usize = 2 * RADIUS + 1;
const PHASES: usize = 256;

type Dot = fn(&[f64], &[f64], &[f64], f64) -> f64;

pub(super) struct Resampler {
    kernels: Box<[f64]>,
    dot: Dot,
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
            dot: select_dot(),
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
        let split = skip + count;
        let result = (self.dot)(
            &input[start..start + count],
            &lower[skip..split],
            &upper[skip..split],
            fraction,
        );
        if split == TAPS {
            result
        } else {
            result
                + (self.dot)(
                    &input[..TAPS - split],
                    &lower[split..],
                    &upper[split..],
                    fraction,
                )
        }
    }
}

fn select_dot() -> Dot {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if std::is_x86_feature_detected!("avx2") {
        return vector_dot;
    }
    #[cfg(target_arch = "aarch64")]
    if std::arch::is_aarch64_feature_detected!("neon") {
        return neon_dot;
    }
    scalar_dot
}

fn scalar_dot(samples: &[f64], lo: &[f64], hi: &[f64], fraction: f64) -> f64 {
    let (samples, sample_tail) = samples.as_chunks::<4>();
    let (lo, lo_tail) = lo.as_chunks::<4>();
    let (hi, hi_tail) = hi.as_chunks::<4>();
    let mut sums = [0.0; 4];
    for ((samples, lo), hi) in samples.iter().zip(lo).zip(hi) {
        for lane in 0..4 {
            sums[lane] += samples[lane] * (lo[lane] + fraction * (hi[lane] - lo[lane]));
        }
    }
    let mut sum = (sums[0] + sums[1]) + (sums[2] + sums[3]);
    for ((&sample, &lo), &hi) in sample_tail.iter().zip(lo_tail).zip(hi_tail) {
        sum += sample * (lo + fraction * (hi - lo));
    }
    sum
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn vector_dot(samples: &[f64], lo: &[f64], hi: &[f64], fraction: f64) -> f64 {
    assert_eq!(samples.len(), lo.len());
    assert_eq!(samples.len(), hi.len());
    // SAFETY: selected only after runtime AVX2 detection; equal slice lengths
    // are checked above and each unaligned load stays within those slices.
    unsafe { avx2_dot(samples, lo, hi, fraction) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn avx2_dot(samples: &[f64], lo: &[f64], hi: &[f64], fraction: f64) -> f64 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;
    // Two independent four-lane sums hide multiply/add latency. No FMA is
    // required, so AVX2 CPUs without FMA use the same arithmetic operations.
    unsafe {
        let phase = _mm256_set1_pd(fraction);
        let mut a = _mm256_setzero_pd();
        let mut b = _mm256_setzero_pd();
        let mut i = 0;
        while i + 8 <= samples.len() {
            let lower = _mm256_loadu_pd(lo.as_ptr().add(i));
            let upper = _mm256_loadu_pd(hi.as_ptr().add(i));
            let gain = _mm256_add_pd(lower, _mm256_mul_pd(phase, _mm256_sub_pd(upper, lower)));
            a = _mm256_add_pd(
                a,
                _mm256_mul_pd(_mm256_loadu_pd(samples.as_ptr().add(i)), gain),
            );
            let lower = _mm256_loadu_pd(lo.as_ptr().add(i + 4));
            let upper = _mm256_loadu_pd(hi.as_ptr().add(i + 4));
            let gain = _mm256_add_pd(lower, _mm256_mul_pd(phase, _mm256_sub_pd(upper, lower)));
            b = _mm256_add_pd(
                b,
                _mm256_mul_pd(_mm256_loadu_pd(samples.as_ptr().add(i + 4)), gain),
            );
            i += 8;
        }
        if i + 4 <= samples.len() {
            let lower = _mm256_loadu_pd(lo.as_ptr().add(i));
            let upper = _mm256_loadu_pd(hi.as_ptr().add(i));
            let gain = _mm256_add_pd(lower, _mm256_mul_pd(phase, _mm256_sub_pd(upper, lower)));
            a = _mm256_add_pd(
                a,
                _mm256_mul_pd(_mm256_loadu_pd(samples.as_ptr().add(i)), gain),
            );
            i += 4;
        }
        let mut lanes = [0.0; 4];
        _mm256_storeu_pd(lanes.as_mut_ptr(), _mm256_add_pd(a, b));
        let mut sum = (lanes[0] + lanes[1]) + (lanes[2] + lanes[3]);
        for i in i..samples.len() {
            sum += samples[i] * (lo[i] + fraction * (hi[i] - lo[i]));
        }
        sum
    }
}

#[cfg(target_arch = "aarch64")]
fn neon_dot(samples: &[f64], lo: &[f64], hi: &[f64], fraction: f64) -> f64 {
    assert_eq!(samples.len(), lo.len());
    assert_eq!(samples.len(), hi.len());
    // SAFETY: selected after NEON detection; checked equal lengths bound loads.
    unsafe { neon_dot_inner(samples, lo, hi, fraction) }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn neon_dot_inner(samples: &[f64], lo: &[f64], hi: &[f64], fraction: f64) -> f64 {
    use std::arch::aarch64::*;
    let mut sums = [vdupq_n_f64(0.0); 2];
    let phase = vdupq_n_f64(fraction);
    let end = samples.len() / 4 * 4;
    for i in (0..end).step_by(4) {
        for (lane, sum) in sums.iter_mut().enumerate() {
            let j = i + lane * 2;
            // SAFETY: j..j+2 is inside this checked four-sample block.
            // Separate multiply/add operations preserve scalar rounding (no FMA).
            unsafe {
                let lower = vld1q_f64(lo[j..j + 2].as_ptr());
                let upper = vld1q_f64(hi[j..j + 2].as_ptr());
                let gain = vaddq_f64(lower, vmulq_f64(phase, vsubq_f64(upper, lower)));
                *sum = vaddq_f64(*sum, vmulq_f64(vld1q_f64(samples[j..j + 2].as_ptr()), gain));
            }
        }
    }
    let mut sum = vaddvq_f64(sums[0]) + vaddvq_f64(sums[1]);
    for i in end..samples.len() {
        sum += samples[i] * (lo[i] + fraction * (hi[i] - lo[i]));
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_simd_matches_portable_for_every_tail_and_unaligned_offset() {
        let mut seed = 7_u64;
        let mut random = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as u32 as f64 / u32::MAX as f64 - 0.5
        };
        let samples: [f64; TAPS + 8] = std::array::from_fn(|_| random());
        let lower: [f64; TAPS + 8] = std::array::from_fn(|_| random());
        let upper: [f64; TAPS + 8] = std::array::from_fn(|_| random());
        for offset in 0..8 {
            for length in 0..=TAPS {
                for phase in [0.0, 0.17, 0.5, 0.99999] {
                    let end = offset + length;
                    let a = &samples[offset..end];
                    let lo = &lower[offset..end];
                    let hi = &upper[offset..end];
                    let expected = scalar_dot(a, lo, hi, phase);
                    let actual = select_dot()(a, lo, hi, phase);
                    #[cfg(target_arch = "aarch64")]
                    assert_eq!(actual.to_bits(), expected.to_bits());
                    assert!(
                        (actual - expected).abs() < 1e-12,
                        "offset={offset}, length={length}, phase={phase}"
                    );
                }
            }
        }
        for speed in [0.5, 0.83, 1.3, 2.0] {
            let vector = Resampler::new(speed);
            let mut portable = Resampler::new(speed);
            portable.dot = scalar_dot;
            let input = std::array::from_fn::<_, 1024, _>(|_| random());
            for center in -65..=128 {
                let position = center as f64 + 0.37;
                assert!(
                    (vector.sample(&input, 512, position) - portable.sample(&input, 512, position))
                        .abs()
                        < 1e-12
                );
            }
            for center in 960..=1088 {
                let position = center as f64 + 0.63;
                assert!(
                    (vector.sample(&input, 1200, position)
                        - portable.sample(&input, 1200, position))
                    .abs()
                        < 1e-12
                );
            }
        }
    }

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
