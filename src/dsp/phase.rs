//! Double-precision phase math, batched across independent spectral bins.
//! SIMD uses range reduction and Taylor series, without FMA or f32 arithmetic.
use super::complex::Complex;

pub(crate) fn angles(input: &[Complex], output: &mut [f64], vector: bool) {
    assert_eq!(input.len(), output.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: callers use the FFT plan's runtime AVX2 detection.
        unsafe { angles_avx2(input, output) };
        return;
    }
    let _ = vector;
    for (value, angle) in input.iter().zip(output) {
        *angle = value.im.atan2(value.re);
    }
}

/// Internal rotations are bounded by 2*pi*(BINS+1). Keep argument reduction
/// exact enough for a double-precision result throughout +/- 16384 radians.
pub(crate) fn unit_phasors(input: &[f64], output: &mut [Complex], vector: bool) {
    assert_eq!(input.len(), output.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: callers use the FFT plan's runtime AVX2 detection.
        unsafe { phasors_avx2(input, output) };
        return;
    }
    let _ = vector;
    for (&angle, value) in input.iter().zip(output) {
        assert!(angle.abs() <= 16384.0, "phase rotation out of range");
        *value = Complex::polar(1.0, angle);
    }
}

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn angles_avx2(input: &[Complex], output: &mut [f64]) {
    let (blocks, tail) = input.as_chunks::<4>();
    let (results, remainder) = output.as_chunks_mut::<4>();
    for (block, result) in blocks.iter().zip(results) {
        // SAFETY: four checked repr(C) complexes and four output f64s.
        unsafe {
            let lo = _mm256_loadu_pd(block.as_ptr().cast());
            let hi = _mm256_loadu_pd(block.as_ptr().add(2).cast());
            let x = _mm256_permute4x64_pd::<0xd8>(_mm256_unpacklo_pd(lo, hi));
            let y = _mm256_permute4x64_pd::<0xd8>(_mm256_unpackhi_pd(lo, hi));
            let sign = _mm256_set1_pd(-0.0);
            let ax = _mm256_andnot_pd(sign, x);
            let ay = _mm256_andnot_pd(sign, y);
            let largest = _mm256_max_pd(ax, ay);
            let smallest = _mm256_min_pd(ax, ay);
            let one = _mm256_set1_pd(1.0);
            let zero = _mm256_setzero_pd();
            let is_zero = _mm256_cmp_pd::<_CMP_EQ_OQ>(largest, zero);
            let mut t = _mm256_div_pd(smallest, _mm256_blendv_pd(largest, one, is_zero));
            // Infinity/infinity has limiting ratio one; a single infinity has
            // ratio zero. Preserve atan2's axes, signed zero and NaN behavior.
            let both_inf = _mm256_cmp_pd::<_CMP_EQ_OQ>(smallest, _mm256_set1_pd(f64::INFINITY));
            t = _mm256_blendv_pd(t, one, both_inf);
            let middle = _mm256_cmp_pd::<_CMP_GT_OQ>(t, _mm256_set1_pd(0.198912367379658));
            let high = _mm256_cmp_pd::<_CMP_GT_OQ>(t, _mm256_set1_pd(0.6681786379192989));
            let pivot = _mm256_blendv_pd(
                _mm256_and_pd(middle, _mm256_set1_pd(0.41421356237309503)),
                one,
                high,
            );
            let base = _mm256_blendv_pd(
                _mm256_and_pd(middle, _mm256_set1_pd(std::f64::consts::FRAC_PI_8)),
                _mm256_set1_pd(std::f64::consts::FRAC_PI_4),
                high,
            );
            let z = _mm256_div_pd(
                _mm256_sub_pd(t, pivot),
                _mm256_add_pd(one, _mm256_mul_pd(t, pivot)),
            );
            let z2 = _mm256_mul_pd(z, z);
            // |z| <= tan(pi/16). The omitted term is <= |z|^25/25 < 1.2e-19.
            let mut p = _mm256_set1_pd(-1.0 / 23.0);
            for k in (1..11).rev() {
                let coefficient = if k % 2 == 0 { 1.0 } else { -1.0 } / (2 * k + 1) as f64;
                p = _mm256_add_pd(_mm256_set1_pd(coefficient), _mm256_mul_pd(z2, p));
            }
            let a = _mm256_add_pd(
                base,
                _mm256_add_pd(z, _mm256_mul_pd(_mm256_mul_pd(z, z2), p)),
            );
            let a = _mm256_blendv_pd(
                a,
                _mm256_sub_pd(_mm256_set1_pd(std::f64::consts::FRAC_PI_2), a),
                _mm256_cmp_pd::<_CMP_GT_OQ>(ay, ax),
            );
            let a = _mm256_blendv_pd(a, _mm256_sub_pd(_mm256_set1_pd(std::f64::consts::PI), a), x);
            let a = _mm256_xor_pd(a, _mm256_and_pd(y, sign));
            let nan = _mm256_cmp_pd::<_CMP_UNORD_Q>(x, y);
            _mm256_storeu_pd(
                result.as_mut_ptr(),
                _mm256_blendv_pd(a, _mm256_set1_pd(f64::NAN), nan),
            );
        }
    }
    for (value, angle) in tail.iter().zip(remainder) {
        *angle = value.im.atan2(value.re);
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn phasors_avx2(input: &[f64], output: &mut [Complex]) {
    let (blocks, tail) = input.as_chunks::<4>();
    let (results, remainder) = output.as_chunks_mut::<4>();
    for (block, result) in blocks.iter().zip(results) {
        // SAFETY: four input f64s and four checked repr(C) output complexes.
        unsafe {
            let x = _mm256_loadu_pd(block.as_ptr());
            let valid = _mm256_cmp_pd::<_CMP_LE_OQ>(
                _mm256_andnot_pd(_mm256_set1_pd(-0.0), x),
                _mm256_set1_pd(16384.0),
            );
            assert_eq!(_mm256_movemask_pd(valid), 15, "phase rotation out of range");
            let n = _mm256_round_pd::<{ _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC }>(
                _mm256_mul_pd(x, _mm256_set1_pd(std::f64::consts::FRAC_2_PI)),
            );
            // Split pi/2: n*HI is exact for this range; LO restores the bits
            // lost by reducing an angle thousands of radians from zero.
            let r = _mm256_sub_pd(
                _mm256_sub_pd(x, _mm256_mul_pd(n, _mm256_set1_pd(1.5707963267341256))),
                _mm256_mul_pd(n, _mm256_set1_pd(6.077100506506192e-11)),
            );
            let r2 = _mm256_mul_pd(r, r);
            let mut s = _mm256_set1_pd(-1.0 / 121645100408832000.0);
            for coefficient in [
                1.0 / 355687428096000.0,
                -1.0 / 1307674368000.0,
                1.0 / 6227020800.0,
                -1.0 / 39916800.0,
                1.0 / 362880.0,
                -1.0 / 5040.0,
                1.0 / 120.0,
                -1.0 / 6.0,
            ] {
                s = _mm256_add_pd(_mm256_set1_pd(coefficient), _mm256_mul_pd(r2, s));
            }
            let s = _mm256_add_pd(r, _mm256_mul_pd(_mm256_mul_pd(r, r2), s));
            let mut c = _mm256_set1_pd(-1.0 / 6402373705728000.0);
            for coefficient in [
                1.0 / 20922789888000.0,
                -1.0 / 87178291200.0,
                1.0 / 479001600.0,
                -1.0 / 3628800.0,
                1.0 / 40320.0,
                -1.0 / 720.0,
                1.0 / 24.0,
                -0.5,
                1.0,
            ] {
                c = _mm256_add_pd(_mm256_set1_pd(coefficient), _mm256_mul_pd(r2, c));
            }
            let quadrant = _mm256_cvtepi32_epi64(_mm256_cvttpd_epi32(n));
            let odd = _mm256_castsi256_pd(_mm256_cmpeq_epi64(
                _mm256_and_si256(quadrant, _mm256_set1_epi64x(1)),
                _mm256_set1_epi64x(1),
            ));
            let sign_s = _mm256_castsi256_pd(_mm256_slli_epi64::<62>(_mm256_and_si256(
                quadrant,
                _mm256_set1_epi64x(2),
            )));
            let sign_c = _mm256_castsi256_pd(_mm256_slli_epi64::<62>(_mm256_and_si256(
                _mm256_add_epi64(quadrant, _mm256_set1_epi64x(1)),
                _mm256_set1_epi64x(2),
            )));
            let sine = _mm256_xor_pd(_mm256_blendv_pd(s, c, odd), sign_s);
            let cosine = _mm256_xor_pd(_mm256_blendv_pd(c, s, odd), sign_c);
            let lo = _mm256_unpacklo_pd(cosine, sine);
            let hi = _mm256_unpackhi_pd(cosine, sine);
            _mm256_storeu_pd(
                result.as_mut_ptr().cast(),
                _mm256_permute2f128_pd::<0x20>(lo, hi),
            );
            _mm256_storeu_pd(
                result.as_mut_ptr().add(2).cast(),
                _mm256_permute2f128_pd::<0x31>(lo, hi),
            );
        }
    }
    for (&angle, value) in tail.iter().zip(remainder) {
        assert!(angle.abs() <= 16384.0, "phase rotation out of range");
        *value = Complex::polar(1.0, angle);
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
    fn phase_matches_system_math_across_quadrants_axes_and_exponents() {
        let mut input = Vec::new();
        for x in [
            0.0_f64,
            -0.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            f64::MAX,
            1.0,
            -1.0,
            f64::from_bits(1),
            -f64::from_bits(1),
        ] {
            for y in [
                0.0_f64,
                -0.0,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NAN,
                f64::MAX,
                1.0,
                -1.0,
                f64::from_bits(1),
                -f64::from_bits(1),
            ] {
                input.push(Complex { re: x, im: y });
            }
        }
        let mut seed = 31_u64;
        for i in 0..100_003 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let scale = 2.0_f64.powi((seed >> 32) as i32 % 1000);
            input.push(Complex {
                re: (i as f64 * 0.31).cos() * scale,
                im: (i as f64 * 0.17).sin() * scale,
            });
        }
        for vector in [false, vector_available()] {
            let mut output = vec![0.0; input.len()];
            angles(&input, &mut output, vector);
            for (value, actual) in input.iter().zip(output) {
                let expected = value.im.atan2(value.re);
                if expected.is_nan() {
                    assert!(actual.is_nan());
                } else {
                    assert!(
                        (actual - expected).abs() <= 9e-16,
                        "atan2({},{}): {actual} != {expected}",
                        value.im,
                        value.re
                    );
                    if expected == 0.0 {
                        assert_eq!(actual.to_bits(), expected.to_bits());
                    }
                }
            }
        }
    }

    #[test]
    fn rotations_match_system_math_at_reduction_boundaries_and_tails() {
        let mut input: Vec<_> = (0..100_003)
            .map(|i| -16384.0 + 32768.0 * i as f64 / 100_002.0)
            .collect();
        for i in -10_430..=10_430 {
            let boundary = i as f64 * std::f64::consts::FRAC_PI_2;
            for delta in [-1e-12, 0.0, 1e-12] {
                input.push(boundary + delta);
            }
        }
        for vector in [false, vector_available()] {
            let mut output = vec![Complex::default(); input.len()];
            unit_phasors(&input, &mut output, vector);
            for (&angle, value) in input.iter().zip(output) {
                assert!(
                    (value.re - angle.cos()).abs() < 5e-16,
                    "cos({angle}): {}",
                    value.re
                );
                assert!(
                    (value.im - angle.sin()).abs() < 5e-16,
                    "sin({angle}): {}",
                    value.im
                );
                assert!((value.re * value.re + value.im * value.im - 1.0).abs() < 8e-16);
            }
        }
    }

    #[test]
    fn checked_lengths_unaligned_subslices_and_scalar_tails() {
        for vector in [false, vector_available()] {
            for length in 0..12 {
                let input: Vec<_> = (0..length + 2)
                    .map(|i| Complex::polar(0.7, i as f64 * 0.17))
                    .collect();
                let mut output = vec![12345.0; length + 2];
                angles(&input[1..length + 1], &mut output[1..length + 1], vector);
                assert_eq!(output[0], 12345.0);
                assert_eq!(output[length + 1], 12345.0);
                let mut rotations = vec![
                    Complex {
                        re: 12345.0,
                        im: -12345.0
                    };
                    length + 2
                ];
                unit_phasors(
                    &output[1..length + 1],
                    &mut rotations[1..length + 1],
                    vector,
                );
                for i in 0..length {
                    assert!((rotations[i + 1].re - input[i + 1].re / 0.7).abs() < 9e-16);
                    assert!((rotations[i + 1].im - input[i + 1].im / 0.7).abs() < 9e-16);
                }
                for i in [0, length + 1] {
                    assert_eq!(rotations[i].re, 12345.0);
                    assert_eq!(rotations[i].im, -12345.0);
                }
            }
            for invalid in [
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
                -16384.1,
                16384.1,
            ] {
                assert!(std::panic::catch_unwind(|| {
                    unit_phasors(&[invalid; 4], &mut [Complex::default(); 4], vector);
                })
                .is_err());
                assert!(std::panic::catch_unwind(|| {
                    unit_phasors(&[invalid], &mut [Complex::default(); 1], vector);
                })
                .is_err());
            }
            assert!(std::panic::catch_unwind(|| {
                angles(&[Complex::default(); 4], &mut [0.0; 3], vector);
            })
            .is_err());
            assert!(std::panic::catch_unwind(|| {
                unit_phasors(&[0.0; 4], &mut [Complex::default(); 3], vector);
            })
            .is_err());
        }
    }
}
