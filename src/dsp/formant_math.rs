//! Bounded, double-precision formant math. The portable path uses the system math.
//! AVX2 and ARM64 NEON evaluate independent values without FMA or reduced precision.

#[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
#[path = "formant_exp_table.rs"]
mod exp_table;

pub(crate) fn log(input: &[f64], output: &mut [f64], vector: bool) {
    assert_eq!(input.len(), output.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: the caller uses the FFT's runtime AVX2 detection.
        unsafe { log_avx2(input, output) };
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if vector {
        // SAFETY: caller selects NEON before processing; kernels bound all loads.
        unsafe { log_neon(input, output) };
        return;
    }
    let _ = vector;
    for (&value, result) in input.iter().zip(output) {
        *result = value.max(1e-12).ln();
    }
}

pub(crate) fn exp(values: &mut [f64], vector: bool) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: the caller uses the FFT's runtime AVX2 detection.
        unsafe { exp_avx2(values) };
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if vector {
        // SAFETY: caller selects NEON before processing; kernels bound all loads.
        unsafe { exp_neon(values) };
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
                _mm256_mul_pd(x, _mm256_set1_pd(32.0 * std::f64::consts::LOG2_E)),
            );
            let integer = _mm256_cvttpd_epi32(n);
            let index = _mm_and_si128(integer, _mm_set1_epi32(31));
            let fraction = _mm256_i32gather_pd::<8>(exp_table::EXP2_FRAC.as_ptr(), index);
            let r = _mm256_sub_pd(
                _mm256_sub_pd(x, _mm256_mul_pd(n, _mm256_set1_pd(exp_table::EXP_STEP_HI))),
                _mm256_mul_pd(n, _mm256_set1_pd(exp_table::EXP_STEP_LO)),
            );
            // |r| <= ln(2)/64. Degree six has truncation error below 4e-18.
            let mut p = _mm256_set1_pd(1.0 / 720.0);
            for coefficient in [1.0 / 120.0, 1.0 / 24.0, 1.0 / 6.0, 0.5, 1.0, 1.0] {
                p = _mm256_add_pd(_mm256_set1_pd(coefficient), _mm256_mul_pd(r, p));
            }
            let exponent = _mm256_slli_epi64::<52>(_mm256_add_epi64(
                _mm256_cvtepi32_epi64(_mm_srai_epi32::<5>(integer)),
                _mm256_set1_epi64x(1023),
            ));
            let scale = _mm256_mul_pd(fraction, _mm256_castsi256_pd(exponent));
            _mm256_storeu_pd(block.as_mut_ptr(), _mm256_mul_pd(p, scale));
        }
    }
    for value in tail {
        *value = value.clamp(-2.0, 2.0).exp();
    }
}

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn log_neon(input: &[f64], output: &mut [f64]) {
    let (blocks, tail) = input.as_chunks::<2>();
    let (results, remainder) = output.as_chunks_mut::<2>();
    for (block, result) in blocks.iter().zip(results) {
        // SAFETY: checked chunks contain two f64s; all loads/stores stay inside.
        // maxNum matches scalar max, including NaN magnitudes clamped to the floor.
        unsafe {
            let x = vmaxnmq_f64(vld1q_f64(block.as_ptr()), vdupq_n_f64(1e-12));
            let bits = vreinterpretq_u64_f64(x);
            let exponent64 = vsubq_s64(
                vreinterpretq_s64_u64(vshrq_n_u64::<52>(bits)),
                vdupq_n_s64(1023),
            );
            let exponent = vcvtq_f64_s64(exponent64);
            let mantissa = vreinterpretq_f64_u64(vorrq_u64(
                vandq_u64(bits, vdupq_n_u64((1 << 52) - 1)),
                vdupq_n_u64(1023 << 52),
            ));
            let high = vcgtq_f64(mantissa, vdupq_n_f64(std::f64::consts::SQRT_2));
            let one = vdupq_n_f64(1.0);
            let m = vmulq_f64(mantissa, vbslq_f64(high, vdupq_n_f64(0.5), one));
            let e = vaddq_f64(exponent, vbslq_f64(high, one, vdupq_n_f64(0.0)));
            let z = vdivq_f64(vsubq_f64(m, one), vaddq_f64(m, one));
            let z2 = vmulq_f64(z, z);
            // Same atanh series as AVX2, without fused multiply/add rounding.
            let mut p = vdupq_n_f64(1.0 / 19.0);
            for denominator in [17.0, 15.0, 13.0, 11.0, 9.0, 7.0, 5.0, 3.0, 1.0] {
                p = vaddq_f64(vdupq_n_f64(1.0 / denominator), vmulq_f64(z2, p));
            }
            let y = vaddq_f64(
                vmulq_f64(e, vdupq_n_f64(std::f64::consts::LN_2)),
                vmulq_f64(vaddq_f64(z, z), p),
            );
            vst1q_f64(result.as_mut_ptr(), y);
        }
    }
    for (&value, result) in tail.iter().zip(remainder) {
        *result = value.max(1e-12).ln();
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn exp_neon(values: &mut [f64]) {
    let (blocks, tail) = values.as_chunks_mut::<2>();
    for block in blocks {
        // SAFETY: checked chunks contain two f64s. Reduction indices are masked
        // to 0..32; two scalar table loads replace AVX2's gather instruction.
        unsafe {
            let x = vminq_f64(
                vmaxq_f64(vld1q_f64(block.as_ptr()), vdupq_n_f64(-2.0)),
                vdupq_n_f64(2.0),
            );
            let n = vrndnq_f64(vmulq_f64(x, vdupq_n_f64(32.0 * std::f64::consts::LOG2_E)));
            let integer = vcvtq_s64_f64(n);
            let index = vandq_u64(vreinterpretq_u64_s64(integer), vdupq_n_u64(31));
            let fraction = vsetq_lane_f64::<1>(
                exp_table::EXP2_FRAC[vgetq_lane_u64::<1>(index) as usize],
                vdupq_n_f64(exp_table::EXP2_FRAC[vgetq_lane_u64::<0>(index) as usize]),
            );
            let r = vsubq_f64(
                vsubq_f64(x, vmulq_f64(n, vdupq_n_f64(exp_table::EXP_STEP_HI))),
                vmulq_f64(n, vdupq_n_f64(exp_table::EXP_STEP_LO)),
            );
            let mut p = vdupq_n_f64(1.0 / 720.0);
            for coefficient in [1.0 / 120.0, 1.0 / 24.0, 1.0 / 6.0, 0.5, 1.0, 1.0] {
                p = vaddq_f64(vdupq_n_f64(coefficient), vmulq_f64(r, p));
            }
            let exponent =
                vshlq_n_s64::<52>(vaddq_s64(vshrq_n_s64::<5>(integer), vdupq_n_s64(1023)));
            let scale = vmulq_f64(fraction, vreinterpretq_f64_s64(exponent));
            vst1q_f64(block.as_mut_ptr(), vmulq_f64(p, scale));
        }
    }
    for value in tail {
        *value = value.clamp(-2.0, 2.0).exp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_matches_system_math_at_table_edges_and_unaligned_tails() {
        let mut input = vec![-2.0, -0.0, 0.0, 2.0, f64::NEG_INFINITY, f64::INFINITY];
        for cell in -94..=94 {
            let edge = (cell as f64 + 0.5) * (std::f64::consts::LN_2 / 32.0);
            for offset in -2_i64..=2 {
                input.push(f64::from_bits(edge.to_bits().wrapping_add_signed(offset)));
            }
        }
        for vector in [false, vector_available()] {
            let mut actual = input.clone();
            exp(&mut actual, vector);
            for (&value, actual) in input.iter().zip(actual) {
                let expected = value.clamp(-2.0, 2.0).exp();
                assert!((actual - expected).abs() < 2e-15 * expected);
                if value == 0.0 {
                    assert_eq!(actual.to_bits(), 1.0_f64.to_bits());
                }
            }
            for head in 1..=3 {
                for length in 0..=12 {
                    let mut buffer = vec![12345.0; head + length + 2];
                    for i in 0..length {
                        buffer[head + i] = input[(i * 73 + head) % input.len()];
                    }
                    let expected: Vec<_> = buffer[head..head + length]
                        .iter()
                        .map(|v| v.clamp(-2.0, 2.0).exp())
                        .collect();
                    exp(&mut buffer[head..head + length], vector);
                    assert!(buffer[..head].iter().all(|&v| v == 12345.0));
                    assert!(buffer[head + length..].iter().all(|&v| v == 12345.0));
                    for (&actual, expected) in buffer[head..head + length].iter().zip(expected) {
                        assert!((actual - expected).abs() < 2e-15 * expected);
                    }
                }
            }
        }
    }

    fn vector_available() -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            std::is_x86_feature_detected!("avx2")
        }
        #[cfg(target_arch = "aarch64")]
        {
            std::arch::is_aarch64_feature_detected!("neon")
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        {
            false
        }
    }

    #[test]
    fn log_clamping_preserves_slice_guards_for_unaligned_inputs_and_tails() {
        let values = [
            -1.0,
            -0.0,
            0.0,
            f64::NEG_INFINITY,
            f64::NAN,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            1e-12,
            1.0,
            std::f64::consts::SQRT_2,
            2.0,
            2048.0,
            2.0_f64.powi(140),
        ];
        let input: Vec<_> = (0..24).map(|i| values[i % values.len()]).collect();
        for vector in [false, vector_available()] {
            for head in 0..4 {
                for length in 0..=17 {
                    let mut output = vec![12345.0; head + length + 2];
                    log(
                        &input[head..head + length],
                        &mut output[head..head + length],
                        vector,
                    );
                    assert!(output[..head].iter().all(|&v| v == 12345.0));
                    assert!(output[head + length..].iter().all(|&v| v == 12345.0));
                    for (&value, &actual) in input[head..head + length]
                        .iter()
                        .zip(&output[head..head + length])
                    {
                        let expected = value.max(1e-12).ln();
                        assert!(
                            (actual - expected).abs() < 3e-14,
                            "log({value}): {actual} != {expected}"
                        );
                    }
                }
            }
            assert!(std::panic::catch_unwind(|| {
                log(&[1.0; 4], &mut [0.0; 3], vector);
            })
            .is_err());
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
