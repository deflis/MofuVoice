//! Double-precision phase math, batched across independent spectral bins.
//! SIMD uses range reduction and Chebyshev interpolants, without FMA or f32 arithmetic.
use super::complex::Complex;

#[inline(always)]
pub(crate) fn wrap(phase: f64) -> f64 {
    // Internal phases are bounded (bin advances, window-centre offsets and
    // wrapped accumulators). Quotient reduction avoids a floating remainder
    // call for every analysis bin; roundoff is far below the phase tolerance.
    let shifted = phase + std::f64::consts::PI;
    let period = std::f64::consts::TAU;
    let mut remainder = shifted - (shifted / period).floor() * period;
    if remainder < 0.0 {
        remainder += period;
    } else if remainder >= period {
        remainder -= period;
    }
    remainder - std::f64::consts::PI
}

pub(crate) fn wrap_in_place(values: &mut [f64], vector: bool) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: the caller uses the FFT plan's AVX2 detection.
        unsafe { wrap_avx2(values) };
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if vector {
        // SAFETY: caller selects NEON before processing; kernels check slice bounds.
        unsafe { wrap_neon(values) };
        return;
    }
    let _ = vector;
    for value in values {
        *value = wrap(*value);
    }
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn wrap_avx2(values: &mut [f64]) {
    let (blocks, tail) = values.as_chunks_mut::<4>();
    for block in blocks {
        // SAFETY: four checked f64 values. Keep the scalar operation order.
        unsafe {
            let pi = _mm256_set1_pd(std::f64::consts::PI);
            let period = _mm256_set1_pd(std::f64::consts::TAU);
            let shifted = _mm256_add_pd(_mm256_loadu_pd(block.as_ptr()), pi);
            let quotient = _mm256_floor_pd(_mm256_div_pd(shifted, period));
            let remainder = _mm256_sub_pd(shifted, _mm256_mul_pd(quotient, period));
            let negative = _mm256_cmp_pd::<_CMP_LT_OQ>(remainder, _mm256_setzero_pd());
            let high = _mm256_cmp_pd::<_CMP_GE_OQ>(remainder, period);
            let corrected = _mm256_blendv_pd(remainder, _mm256_add_pd(remainder, period), negative);
            let corrected = _mm256_blendv_pd(corrected, _mm256_sub_pd(remainder, period), high);
            _mm256_storeu_pd(block.as_mut_ptr(), _mm256_sub_pd(corrected, pi));
        }
    }
    for value in tail {
        *value = wrap(*value);
    }
}
pub(crate) fn angles(input: &[Complex], output: &mut [f64], vector: bool) {
    assert_eq!(input.len(), output.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if vector {
        // SAFETY: callers use the FFT plan's runtime AVX2 detection.
        unsafe { angles_avx2(input, output) };
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if vector {
        // SAFETY: caller selects NEON before processing; kernels check slice bounds.
        unsafe { angles_neon(input, output) };
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
    #[cfg(target_arch = "aarch64")]
    if vector {
        // SAFETY: caller selects NEON before processing; kernels check slice bounds.
        unsafe { phasors_neon(input, output) };
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
            // Seven-point Chebyshev interpolation of (atan(z)-z)/z^3 on
            // z^2 in [0,tan(pi/16)^2] gives a degree-15 odd atan polynomial.
            let mut p = _mm256_set1_pd(-0.05909857213116147);
            for coefficient in [
                0.07652130942243261,
                -0.09089837900709528,
                0.11111096372965647,
                -0.14285714188882723,
                0.1999999999976099,
                -0.33333333333333237,
            ] {
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
            // Interpolate (sin(r)-r)/r^3 and (cos(r)-1)/r^2 on
            // r^2 in [0,(pi/4)^2], giving degrees 13 and 14 respectively.
            // Tests include argument reduction and f64 evaluation error.
            let mut s = _mm256_set1_pd(1.5918129294866608e-10);
            for coefficient in [
                -2.5051131845003624e-08,
                2.755731610255244e-06,
                -0.00019841269836758574,
                0.008333333333330948,
                -0.16666666666666666,
            ] {
                s = _mm256_add_pd(_mm256_set1_pd(coefficient), _mm256_mul_pd(r2, s));
            }
            let s = _mm256_add_pd(r, _mm256_mul_pd(_mm256_mul_pd(r, r2), s));
            let mut c = _mm256_set1_pd(-1.1367998654022494e-11);
            for coefficient in [
                2.0875886738047052e-09,
                -2.7557315566341895e-07,
                2.480158729369346e-05,
                -0.0013888888888880775,
                0.04166666666666664,
                -0.5,
            ] {
                c = _mm256_add_pd(_mm256_set1_pd(coefficient), _mm256_mul_pd(r2, c));
            }
            let c = _mm256_add_pd(_mm256_set1_pd(1.0), _mm256_mul_pd(r2, c));
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

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn wrap_neon(values: &mut [f64]) {
    let (blocks, tail) = values.as_chunks_mut::<2>();
    let pi = vdupq_n_f64(std::f64::consts::PI);
    let period = vdupq_n_f64(std::f64::consts::TAU);
    for block in blocks {
        // SAFETY: checked chunks contain two f64s. Keep scalar operation order.
        unsafe {
            let shifted = vaddq_f64(vld1q_f64(block.as_ptr()), pi);
            let quotient = vrndmq_f64(vdivq_f64(shifted, period));
            let remainder = vsubq_f64(shifted, vmulq_f64(quotient, period));
            let negative = vcltq_f64(remainder, vdupq_n_f64(0.0));
            let high = vcgeq_f64(remainder, period);
            let corrected = vbslq_f64(negative, vaddq_f64(remainder, period), remainder);
            let corrected = vbslq_f64(high, vsubq_f64(remainder, period), corrected);
            vst1q_f64(block.as_mut_ptr(), vsubq_f64(corrected, pi));
        }
    }
    for value in tail {
        *value = wrap(*value);
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn angles_neon(input: &[Complex], output: &mut [f64]) {
    let (blocks, tail) = input.as_chunks::<2>();
    let (results, remainder) = output.as_chunks_mut::<2>();
    for (block, result) in blocks.iter().zip(results) {
        // SAFETY: two repr(C) complexes contain four f64s; result has two f64s.
        // Range reduction and polynomial match AVX2, with no fused operations.
        unsafe {
            let xy = vld2q_f64(block.as_ptr().cast());
            let (x, y) = (xy.0, xy.1);
            let ax = vabsq_f64(x);
            let ay = vabsq_f64(y);
            let largest = vmaxq_f64(ax, ay);
            let smallest = vminq_f64(ax, ay);
            let one = vdupq_n_f64(1.0);
            let zero = vdupq_n_f64(0.0);
            let is_zero = vceqq_f64(largest, zero);
            let mut t = vdivq_f64(smallest, vbslq_f64(is_zero, one, largest));
            let both_inf = vceqq_f64(smallest, vdupq_n_f64(f64::INFINITY));
            t = vbslq_f64(both_inf, one, t);
            let middle = vcgtq_f64(t, vdupq_n_f64(0.198912367379658));
            let high = vcgtq_f64(t, vdupq_n_f64(0.6681786379192989));
            let pivot = vbslq_f64(
                high,
                one,
                vbslq_f64(middle, vdupq_n_f64(0.41421356237309503), zero),
            );
            let base = vbslq_f64(
                high,
                vdupq_n_f64(std::f64::consts::FRAC_PI_4),
                vbslq_f64(middle, vdupq_n_f64(std::f64::consts::FRAC_PI_8), zero),
            );
            let z = vdivq_f64(vsubq_f64(t, pivot), vaddq_f64(one, vmulq_f64(t, pivot)));
            let z2 = vmulq_f64(z, z);
            let mut p = vdupq_n_f64(-0.05909857213116147);
            for coefficient in [
                0.07652130942243261,
                -0.09089837900709528,
                0.11111096372965647,
                -0.14285714188882723,
                0.1999999999976099,
                -0.33333333333333237,
            ] {
                p = vaddq_f64(vdupq_n_f64(coefficient), vmulq_f64(z2, p));
            }
            let a = vaddq_f64(base, vaddq_f64(z, vmulq_f64(vmulq_f64(z, z2), p)));
            let a = vbslq_f64(
                vcgtq_f64(ay, ax),
                vsubq_f64(vdupq_n_f64(std::f64::consts::FRAC_PI_2), a),
                a,
            );
            // Compare sign bits as integers to retain atan2's negative-zero axes.
            let negative_x = vcltq_s64(vreinterpretq_s64_f64(x), vdupq_n_s64(0));
            let a = vbslq_f64(
                negative_x,
                vsubq_f64(vdupq_n_f64(std::f64::consts::PI), a),
                a,
            );
            let sign_y = vandq_u64(vreinterpretq_u64_f64(y), vdupq_n_u64(1 << 63));
            let a = vreinterpretq_f64_u64(veorq_u64(vreinterpretq_u64_f64(a), sign_y));
            let ordered = vandq_u64(vceqq_f64(x, x), vceqq_f64(y, y));
            vst1q_f64(
                result.as_mut_ptr(),
                vbslq_f64(ordered, a, vdupq_n_f64(f64::NAN)),
            );
        }
    }
    for (value, angle) in tail.iter().zip(remainder) {
        *angle = value.im.atan2(value.re);
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn phasors_neon(input: &[f64], output: &mut [Complex]) {
    let (blocks, tail) = input.as_chunks::<2>();
    let (results, remainder) = output.as_chunks_mut::<2>();
    for (block, result) in blocks.iter().zip(results) {
        // SAFETY: two checked input f64s and two repr(C) output complexes.
        unsafe {
            let x = vld1q_f64(block.as_ptr());
            let valid = vcleq_f64(vabsq_f64(x), vdupq_n_f64(16384.0));
            assert_eq!(
                vgetq_lane_u64::<0>(valid) & vgetq_lane_u64::<1>(valid),
                u64::MAX,
                "phase rotation out of range"
            );
            let n = vrndnq_f64(vmulq_f64(x, vdupq_n_f64(std::f64::consts::FRAC_2_PI)));
            // Same split pi/2 and Chebyshev interpolants as the AVX2 path.
            let r = vsubq_f64(
                vsubq_f64(x, vmulq_f64(n, vdupq_n_f64(1.5707963267341256))),
                vmulq_f64(n, vdupq_n_f64(6.077100506506192e-11)),
            );
            let r2 = vmulq_f64(r, r);
            let mut s = vdupq_n_f64(1.5918129294866608e-10);
            for coefficient in [
                -2.5051131845003624e-08,
                2.755731610255244e-06,
                -0.00019841269836758574,
                0.008333333333330948,
                -0.16666666666666666,
            ] {
                s = vaddq_f64(vdupq_n_f64(coefficient), vmulq_f64(r2, s));
            }
            let s = vaddq_f64(r, vmulq_f64(vmulq_f64(r, r2), s));
            let mut c = vdupq_n_f64(-1.1367998654022494e-11);
            for coefficient in [
                2.0875886738047052e-09,
                -2.7557315566341895e-07,
                2.480158729369346e-05,
                -0.0013888888888880775,
                0.04166666666666664,
                -0.5,
            ] {
                c = vaddq_f64(vdupq_n_f64(coefficient), vmulq_f64(r2, c));
            }
            let c = vaddq_f64(vdupq_n_f64(1.0), vmulq_f64(r2, c));
            let quadrant = vreinterpretq_u64_s64(vcvtq_s64_f64(n));
            let odd = vceqq_u64(vandq_u64(quadrant, vdupq_n_u64(1)), vdupq_n_u64(1));
            let sign_s = vshlq_n_u64::<62>(vandq_u64(quadrant, vdupq_n_u64(2)));
            let sign_c = vshlq_n_u64::<62>(vandq_u64(
                vaddq_u64(quadrant, vdupq_n_u64(1)),
                vdupq_n_u64(2),
            ));
            let sine = vreinterpretq_f64_u64(veorq_u64(
                vreinterpretq_u64_f64(vbslq_f64(odd, c, s)),
                sign_s,
            ));
            let cosine = vreinterpretq_f64_u64(veorq_u64(
                vreinterpretq_u64_f64(vbslq_f64(odd, s, c)),
                sign_c,
            ));
            vst2q_f64(result.as_mut_ptr().cast(), float64x2x2_t(cosine, sine));
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

    #[test]
    fn batched_wrapping_matches_scalar_bits_at_boundaries_and_tails() {
        let mut input = vec![
            0.0,
            -0.0,
            std::f64::consts::PI,
            -std::f64::consts::PI,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        input.extend((0..100_003).map(|i| -16384.0 + 32768.0 * i as f64 / 100_002.0));
        for period in -2600..=2600 {
            let edge = period as f64 * std::f64::consts::TAU - std::f64::consts::PI;
            for offset in -2_i64..=2 {
                input.push(f64::from_bits(edge.to_bits().wrapping_add_signed(offset)));
            }
        }
        for vector in [false, vector_available()] {
            let mut actual = input.clone();
            wrap_in_place(&mut actual, vector);
            for (&input, actual) in input.iter().zip(actual) {
                let expected = wrap(input);
                if expected.is_nan() {
                    assert!(actual.is_nan());
                } else {
                    assert_eq!(actual.to_bits(), expected.to_bits(), "phase={input}");
                }
            }
            for head in 1..=3 {
                for length in 0..=12 {
                    let mut buffer = vec![12345.0; head + length + 2];
                    for i in 0..length {
                        buffer[head + i] = input[(i * 617 + head) % input.len()];
                    }
                    let expected: Vec<_> = buffer[head..head + length]
                        .iter()
                        .map(|&v| wrap(v))
                        .collect();
                    wrap_in_place(&mut buffer[head..head + length], vector);
                    assert!(buffer[..head].iter().all(|&v| v == 12345.0));
                    assert!(buffer[head + length..].iter().all(|&v| v == 12345.0));
                    for (&actual, expected) in buffer[head..head + length].iter().zip(expected) {
                        if expected.is_nan() {
                            assert!(actual.is_nan());
                        } else {
                            assert_eq!(actual.to_bits(), expected.to_bits());
                        }
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
    fn atan_ratios_match_system_math_at_reduction_boundaries() {
        let mut ratios: Vec<_> = (0..=16_384).map(|i| i as f64 / 16_384.0).collect();
        for boundary in [0.198912367379658_f64, 0.6681786379192989, 1.0] {
            for offset in -2_i64..=2 {
                ratios.push(f64::from_bits(
                    boundary.to_bits().wrapping_add_signed(offset),
                ));
            }
        }
        let mut input = Vec::new();
        for ratio in ratios {
            for x in [-1.0, 1.0] {
                for sign in [-1.0, 1.0] {
                    let y = ratio * sign;
                    input.extend([Complex { re: x, im: y }, Complex { re: y, im: x }]);
                }
            }
        }
        for vector in [false, vector_available()] {
            let mut output = vec![0.0; input.len()];
            angles(&input, &mut output, vector);
            for (value, actual) in input.iter().zip(output) {
                let expected = value.im.atan2(value.re);
                assert!((actual - expected).abs() <= 9e-16);
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
