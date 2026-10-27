//! Shared polyphase windowed-sinc kernels and interpolated scalar/SIMD dot products.
use std::f64::consts::{PI, TAU};

pub(crate) const PHASES: usize = 256;
pub(crate) type Dot = fn(&[f64], &[f64], &[f64], f64) -> f64;

pub(crate) fn fill_kernels(kernels: &mut [f64], radius: usize, window_radius: f64, cutoff: f64) {
    assert!(radius > 0 && window_radius.is_finite() && window_radius >= radius as f64);
    assert!(cutoff.is_finite() && cutoff > 0.0 && cutoff <= 0.5);
    let taps = 2 * radius + 1;
    assert_eq!(kernels.len(), (PHASES + 1) * taps);
    for phase in 0..=PHASES {
        let fraction = phase as f64 / PHASES as f64;
        let kernel = &mut kernels[phase * taps..(phase + 1) * taps];
        for (i, tap) in kernel.iter_mut().enumerate() {
            let distance = i as f64 - radius as f64 - fraction;
            let normalized = distance / window_radius;
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
}

pub(crate) fn select_dot() -> Dot {
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

pub(crate) fn scalar_dot(samples: &[f64], lo: &[f64], hi: &[f64], fraction: f64) -> f64 {
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
