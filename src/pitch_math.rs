//! Bounded, double-precision formant math. The portable path uses the system math.
//! AVX2 evaluates four independent values; no FMA or reduced-precision arithmetic.

pub(super) fn log(input: &[f64], output: &mut [f64], vector: bool) {
    assert_eq!(input.len(), output.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: the caller uses the FFT's runtime AVX2 detection.
        unsafe { log_avx2(input, output) };
        return;
    }
    let _ = vector;
    for (&value, result) in input.iter().zip(output) {
        *result = value.max(1e-12).ln();
    }
}

pub(super) fn exp(values: &mut [f64], vector: bool) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: the caller uses the FFT's runtime AVX2 detection.
        unsafe { exp_avx2(values) };
        return;
    }
    let _ = vector;
    for value in values {
        *value = value.clamp(-2.0, 2.0).exp();
    }
}

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn log_avx2(input: &[f64], output: &mut [f64]) {
    let (blocks, tail) = input.as_chunks::<4>();
    let (results, remainder) = output.as_chunks_mut::<4>();
    for (block, result) in blocks.iter().zip(results) {
        // SAFETY: checked chunks contain four f64s; unaligned accesses stay inside them.
        unsafe {
            let x = _mm256_max_pd(_mm256_loadu_pd(block.as_ptr()), _mm256_set1_pd(1e-12));
            let bits = _mm256_castpd_si256(x);
            let exponent64 =
                _mm256_sub_epi64(_mm256_srli_epi64::<52>(bits), _mm256_set1_epi64x(1023));
            // Pack the low 32 bits of each signed exponent for conversion to f64.
            let packed = _mm256_permute4x64_epi64::<0xd8>(_mm256_shuffle_epi32::<0x88>(exponent64));
            let exponent = _mm256_cvtepi32_pd(_mm256_castsi256_si128(packed));
            let mantissa = _mm256_castsi256_pd(_mm256_or_si256(
                _mm256_and_si256(bits, _mm256_set1_epi64x((1_i64 << 52) - 1)),
                _mm256_set1_epi64x(1023_i64 << 52),
            ));
            let high =
                _mm256_cmp_pd::<_CMP_GT_OQ>(mantissa, _mm256_set1_pd(std::f64::consts::SQRT_2));
            let m = _mm256_mul_pd(
                mantissa,
                _mm256_blendv_pd(_mm256_set1_pd(1.0), _mm256_set1_pd(0.5), high),
            );
            let e = _mm256_add_pd(exponent, _mm256_and_pd(high, _mm256_set1_pd(1.0)));
            let z = _mm256_div_pd(
                _mm256_sub_pd(m, _mm256_set1_pd(1.0)),
                _mm256_add_pd(m, _mm256_set1_pd(1.0)),
            );
            let z2 = _mm256_mul_pd(z, z);
            // log(m) = 2 atanh(z); |z| <= 0.171573. The omitted tail is < 1e-17.
            let mut p = _mm256_set1_pd(1.0 / 19.0);
            for denominator in [17.0, 15.0, 13.0, 11.0, 9.0, 7.0, 5.0, 3.0, 1.0] {
                p = _mm256_add_pd(_mm256_set1_pd(1.0 / denominator), _mm256_mul_pd(z2, p));
            }
            let y = _mm256_add_pd(
                _mm256_mul_pd(e, _mm256_set1_pd(std::f64::consts::LN_2)),
                _mm256_mul_pd(_mm256_add_pd(z, z), p),
            );
            _mm256_storeu_pd(result.as_mut_ptr(), y);
        }
    }
    for (&value, result) in tail.iter().zip(remainder) {
        *result = value.max(1e-12).ln();
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn exp_avx2(values: &mut [f64]) {
    let (blocks, tail) = values.as_chunks_mut::<4>();
    for block in blocks {
        // SAFETY: each checked chunk contains four f64s. Exponents are in -3..=3.
        unsafe {
            let x = _mm256_min_pd(
                _mm256_max_pd(_mm256_loadu_pd(block.as_ptr()), _mm256_set1_pd(-2.0)),
                _mm256_set1_pd(2.0),
            );
            let n = _mm256_round_pd::<{ _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC }>(
                _mm256_mul_pd(x, _mm256_set1_pd(std::f64::consts::LOG2_E)),
            );
            let r = _mm256_sub_pd(x, _mm256_mul_pd(n, _mm256_set1_pd(std::f64::consts::LN_2)));
            // |r| <= ln(2)/2. Taylor degree 12 has relative error < 4e-16.
            let mut p = _mm256_set1_pd(1.0 / 479001600.0);
            for coefficient in [
                1.0 / 39916800.0,
                1.0 / 3628800.0,
                1.0 / 362880.0,
                1.0 / 40320.0,
                1.0 / 5040.0,
                1.0 / 720.0,
                1.0 / 120.0,
                1.0 / 24.0,
                1.0 / 6.0,
                0.5,
                1.0,
                1.0,
            ] {
                p = _mm256_add_pd(_mm256_set1_pd(coefficient), _mm256_mul_pd(r, p));
            }
            let exponent = _mm256_slli_epi64::<52>(_mm256_add_epi64(
                _mm256_cvtepi32_epi64(_mm256_cvttpd_epi32(n)),
                _mm256_set1_epi64x(1023),
            ));
            _mm256_storeu_pd(
                block.as_mut_ptr(),
                _mm256_mul_pd(p, _mm256_castsi256_pd(exponent)),
            );
        }
    }
    for value in tail {
        *value = value.clamp(-2.0, 2.0).exp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector_available() -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            std::is_x86_feature_detected!("avx2")
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            false
        }
    }

    #[test]
    fn bounded_formant_math_matches_system_math() {
        let mut seed = 31_u64;
        let mut values = vec![0.0, 1e-12, 1.0, std::f64::consts::SQRT_2, 2048.0];
        for _ in 0..100_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            // FFT magnitudes of supported sample formats, including tiny inputs.
            values.push(2.0_f64.powf(-160.0 + 300.0 * (seed >> 11) as f64 / (1_u64 << 53) as f64));
        }
        for vector in [false, vector_available()] {
            let mut result = vec![0.0; values.len()];
            log(&values, &mut result, vector);
            for (&value, actual) in values.iter().zip(result) {
                let expected = value.max(1e-12).ln();
                assert!(
                    (actual - expected).abs() < 3e-14,
                    "log({value}): {actual}, {expected}"
                );
            }
            let mut values: Vec<f64> = (0..100_001)
                .map(|i| -2.1 + 4.2 * i as f64 / 100_000.0)
                .collect();
            let expected: Vec<_> = values.iter().map(|v| v.clamp(-2.0, 2.0).exp()).collect();
            exp(&mut values, vector);
            for (actual, expected) in values.into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 2e-15 * expected,
                    "exp: {actual}, {expected}"
                );
            }
        }
    }
}
