//! Lag-parallel correlation kernels. Each SIMD lane owns one lag, preserving
//! the scalar order of additions (including MPM's energy normalization).

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
        scalar_impl::<true>(input, output);
    } else {
        scalar_impl::<false>(input, output);
    }
}

fn scalar_impl<const MPM: bool>(input: &[f64], output: &mut [f64]) {
    let length = input.len() - output.len() + 1;
    for (lag, result) in output.iter_mut().enumerate() {
        let (mut sum, mut energy) = (0.0, 0.0);
        let end = if MPM { input.len() - lag } else { length };
        for (&a, &b) in input[..end].iter().zip(&input[lag..lag + end]) {
            accumulate::<MPM>(a, b, &mut sum, &mut energy);
        }
        *result = normalize::<MPM>(sum, energy);
    }
}

#[inline]
fn accumulate<const MPM: bool>(a: f64, b: f64, sum: &mut f64, energy: &mut f64) {
    if MPM {
        *sum += a * b;
        *energy += a * a + b * b;
    } else {
        let delta = a - b;
        *sum += delta * delta;
    }
}

#[inline]
fn normalize<const MPM: bool>(sum: f64, energy: f64) -> f64 {
    if MPM {
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
            avx2::<true>(input, output);
        } else {
            avx2::<false>(input, output);
        }
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn avx2<const MPM: bool>(input: &[f64], output: &mut [f64]) {
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
                energy = _mm256_add_pd(
                    energy,
                    _mm256_add_pd(_mm256_mul_pd(a, a), _mm256_mul_pd(b, b)),
                );
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
                    accumulate::<MPM>(
                        input[i],
                        input[i + lag + lane],
                        &mut sums[lane],
                        &mut energies[lane],
                    );
                }
            }
            output[lag + lane] = normalize::<MPM>(sums[lane], energies[lane]);
        }
    }
    for lag in batches..output.len() {
        let (mut sum, mut energy) = (0.0, 0.0);
        let end = if MPM { input.len() - lag } else { length };
        for (&a, &b) in input[..end].iter().zip(&input[lag..lag + end]) {
            accumulate::<MPM>(a, b, &mut sum, &mut energy);
        }
        output[lag] = normalize::<MPM>(sum, energy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
