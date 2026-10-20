//! Lag-parallel correlation kernels. Each SIMD lane owns one lag, preserving
//! the scalar order of correlation additions. MPM normalization reuses energy
//! across lags instead of summing squared samples again for each candidate.

pub(super) type Kernel = fn(&[f64], &mut [f64], bool);

pub(super) fn select() -> Kernel {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if std::is_x86_feature_detected!("avx2") {
        return vector;
    }
    scalar
}

pub(super) fn scalar(input: &[f64], output: &mut [f64], mpm: bool) {
    assert!(input.len() >= output.len());
    if mpm {
        scalar_impl::<true, true>(input, output);
    } else {
        scalar_impl::<false, true>(input, output);
    }
}

fn scalar_impl<const MPM: bool, const REUSE: bool>(input: &[f64], output: &mut [f64]) {
    let length = input.len() - output.len() + 1;
    for (lag, result) in output.iter_mut().enumerate() {
        let (mut sum, mut energy) = (0.0, 0.0);
        let end = if MPM { input.len() - lag } else { length };
        for (&a, &b) in input[..end].iter().zip(&input[lag..lag + end]) {
            accumulate::<MPM, REUSE>(a, b, &mut sum, &mut energy);
        }
        *result = normalize::<MPM, REUSE>(sum, energy);
    }
    if MPM && REUSE {
        normalize_mpm(input, output);
    }
}

/// m(tau) = sum(x[0..N-tau]^2) + sum(x[tau..N]^2).
/// Advancing tau removes exactly one sample from each side. This costs O(N+L)
/// instead of O(N*L) and uses no scratch allocation or additional lookahead.
fn normalize_mpm(input: &[f64], output: &mut [f64]) {
    let initial = 2.0 * input.iter().map(|x| x * x).sum::<f64>();
    let mut energy = initial;
    // Conservative accumulated roundoff bound. Normally every overlap retains
    // most of the energy. Re-sum pathological edge impulses or values near the
    // silence threshold so cancellation cannot create false clarity or voicing.
    let tolerance = initial * f64::EPSILON * (input.len() + output.len()) as f64 * 4.0;
    for (lag, correlation) in output.iter_mut().enumerate() {
        if energy <= tolerance || (energy - 1e-20).abs() <= tolerance {
            energy = input[..input.len() - lag]
                .iter()
                .zip(&input[lag..])
                .map(|(&a, &b)| a * a + b * b)
                .sum();
        }
        *correlation = if energy > 1e-20 {
            2.0 * *correlation / energy
        } else {
            0.0
        };
        let left = input[lag];
        let right = input[input.len() - lag - 1];
        energy -= left * left + right * right;
    }
}

#[inline]
fn accumulate<const MPM: bool, const REUSE: bool>(a: f64, b: f64, sum: &mut f64, energy: &mut f64) {
    if MPM {
        *sum += a * b;
        if !REUSE {
            *energy += a * a + b * b;
        }
    } else {
        let delta = a - b;
        *sum += delta * delta;
    }
}

#[inline]
fn normalize<const MPM: bool, const REUSE: bool>(sum: f64, energy: f64) -> f64 {
    if MPM && !REUSE {
        if energy > 1e-20 {
            2.0 * sum / energy
        } else {
            0.0
        }
    } else {
        sum
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn vector(input: &[f64], output: &mut [f64], mpm: bool) {
    assert!(input.len() >= output.len());
    // SAFETY: only selected after runtime AVX2 detection. The target-feature
    // function bounds every four-wide load by the shortest lag's overlap.
    unsafe {
        if mpm {
            avx2::<true, true>(input, output);
        } else {
            avx2::<false, true>(input, output);
        }
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn avx2<const MPM: bool, const REUSE: bool>(input: &[f64], output: &mut [f64]) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;
    let length = input.len() - output.len() + 1;
    let batches = output.len() / 4 * 4;
    for lag in (0..batches).step_by(4) {
        let end = if MPM { input.len() - lag - 3 } else { length };
        let mut sum = _mm256_setzero_pd();
        let mut energy = _mm256_setzero_pd();
        for i in 0..end {
            let a = _mm256_set1_pd(input[i]);
            // SAFETY: i + lag + 3 < input.len() for every iteration,
            // including fixed-length YIN's last four-lag batch.
            let b = unsafe { _mm256_loadu_pd(input.as_ptr().add(i + lag)) };
            if MPM {
                sum = _mm256_add_pd(sum, _mm256_mul_pd(a, b));
                if !REUSE {
                    energy = _mm256_add_pd(
                        energy,
                        _mm256_add_pd(_mm256_mul_pd(a, a), _mm256_mul_pd(b, b)),
                    );
                }
            } else {
                let delta = _mm256_sub_pd(a, b);
                sum = _mm256_add_pd(sum, _mm256_mul_pd(delta, delta));
            }
        }
        let mut sums = [0.0; 4];
        let mut energies = [0.0; 4];
        // SAFETY: both arrays have four writable f64 elements; unaligned stores.
        unsafe {
            _mm256_storeu_pd(sums.as_mut_ptr(), sum);
            _mm256_storeu_pd(energies.as_mut_ptr(), energy);
        }
        for lane in 0..4 {
            if MPM {
                // Restore the extra 0..3 samples of each longer overlap in
                // their original order, rather than padding or rounding it.
                for i in end..input.len() - lag - lane {
                    accumulate::<MPM, REUSE>(
                        input[i],
                        input[i + lag + lane],
                        &mut sums[lane],
                        &mut energies[lane],
                    );
                }
            }
            output[lag + lane] = normalize::<MPM, REUSE>(sums[lane], energies[lane]);
        }
    }
    for lag in batches..output.len() {
        let (mut sum, mut energy) = (0.0, 0.0);
        let end = if MPM { input.len() - lag } else { length };
        for (&a, &b) in input[..end].iter().zip(&input[lag..lag + end]) {
            accumulate::<MPM, REUSE>(a, b, &mut sum, &mut energy);
        }
        output[lag] = normalize::<MPM, REUSE>(sum, energy);
    }
    if MPM && REUSE {
        normalize_mpm(input, output);
    }
}

// Retain the previous arithmetic only in test builds for quality and timing
// comparisons against the same scalar/AVX2 implementation without energy reuse.
#[cfg(test)]
pub(super) fn select_reference() -> Kernel {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if std::is_x86_feature_detected!("avx2") {
        return vector_reference;
    }
    scalar_reference
}

#[cfg(test)]
pub(super) fn scalar_reference(input: &[f64], output: &mut [f64], mpm: bool) {
    assert!(input.len() >= output.len());
    if mpm {
        scalar_impl::<true, false>(input, output);
    } else {
        scalar_impl::<false, false>(input, output);
    }
}

#[cfg(all(test, any(target_arch = "x86", target_arch = "x86_64")))]
fn vector_reference(input: &[f64], output: &mut [f64], mpm: bool) {
    assert!(input.len() >= output.len());
    // SAFETY: selected after runtime AVX2 detection; same checked bounds as vector.
    unsafe {
        if mpm {
            avx2::<true, false>(input, output);
        } else {
            avx2::<false, false>(input, output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reused_energy_matches_direct_normalization_with_roundoff_tolerance() {
        let mut seed = 17u32;
        let mut maximum_error = 0.0f64;
        let mut check = |input: &[f64], lags: usize| {
            let mut reference = vec![0.0; lags];
            let mut actual = reference.clone();
            scalar_reference(input, &mut reference, true);
            select()(input, &mut actual, true);
            for (lag, (&expected, &value)) in reference.iter().zip(&actual).enumerate() {
                assert!(value.is_finite(), "lag={lag}");
                let error = (value - expected).abs();
                maximum_error = maximum_error.max(error);
                assert!(
                    error < 2e-12,
                    "lag={lag} expected={expected} actual={value}"
                );
            }
        };
        for window in [1024, 2048, 4096, 8192] {
            let length = window / 4 + 201;
            let random: Vec<_> = (0..length)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    seed as f64 / u32::MAX as f64 - 0.5
                })
                .collect();
            check(&random, 202);
            for frequency in [60.0, 113.0, 203.0, 350.0] {
                let input: Vec<_> = (0..length)
                    .map(|i| {
                        let phase = std::f64::consts::TAU * frequency * (i * 4) as f64 / 48000.0;
                        0.1 * phase.cos() + 0.04 * (3.0 * phase).cos()
                    })
                    .collect();
                check(&input, 202);
            }
            // Both sides of the 1e-20 normalization guard, including rounding
            // at the threshold itself. Tiny input bypasses the streaming gate,
            // but the kernel must still behave consistently.
            for multiplier in [0.0, 0.5, 1.0 - f64::EPSILON, 1.0, 1.0 + f64::EPSILON, 2.0] {
                let amplitude = (1e-20 / (2 * length) as f64).sqrt() * multiplier;
                check(&vec![amplitude; length], 202);
            }
        }
        // Far more lags than PSOLA uses: dominant edge energy can disappear
        // entirely. Direct recomputation guards against cancellation artifacts.
        for peak in [0, 1, 8, 15, 16] {
            let mut input = vec![1e-12; 17];
            input[peak] = 1.0;
            check(&input, 17);
            input.fill(0.0);
            input[peak] = 1.0;
            check(&input, 17);
        }
        eprintln!("maximum NSDF difference after energy reuse: {maximum_error:e}");
    }

    #[test]
    fn selected_kernel_matches_scalar_bits_for_all_windows_and_tail_lengths() {
        let mut seed = 17u32;
        for window in [1024, 2048, 4096, 8192] {
            for lags in [1, 2, 3, 4, 5, 201, 202, 203] {
                let length = window / 4 + lags - 1;
                let storage: Vec<_> = (0..length + 1)
                    .map(|_| {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        (seed as f64 / u32::MAX as f64 - 0.5) * 0.1
                    })
                    .collect();
                for input in [&storage[..length], &storage[1..]] {
                    for mpm in [false, true] {
                        let mut expected = vec![0.0; lags];
                        let mut actual = expected.clone();
                        scalar(input, &mut expected, mpm);
                        select()(input, &mut actual, mpm);
                        for (lag, (a, b)) in expected.iter().zip(&actual).enumerate() {
                            assert_eq!(
                                a.to_bits(),
                                b.to_bits(),
                                "window={window} lags={lags} lag={lag} mpm={mpm}"
                            );
                        }
                    }
                }
            }
        }
        for value in [0.0, 1e-15, 0.1, -0.1] {
            let input = vec![value; 713];
            for mpm in [false, true] {
                let mut expected = [0.0; 202];
                let mut actual = expected;
                scalar(&input, &mut expected, mpm);
                select()(&input, &mut actual, mpm);
                assert_eq!(expected.map(f64::to_bits), actual.map(f64::to_bits));
            }
        }
    }
}
