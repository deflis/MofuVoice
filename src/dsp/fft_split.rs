//! Split real/imaginary FFT storage: four complex butterflies per AVX2 vector.
//! Callers select retained outputs; tables and arithmetic match the radix-2 FFT.
use super::complex::Complex;

#[repr(C)]
pub(crate) struct SplitBuffer<const N: usize> {
    re: [f64; N],
    // Offset the components by a cache line instead of a power-of-two stride.
    // This avoids competing for the same L1 sets in large paired stages.
    _padding: [f64; 8],
    im: [f64; N],
}
impl<const N: usize> Default for SplitBuffer<N> {
    fn default() -> Self {
        Self {
            re: [0.0; N],
            _padding: [0.0; 8],
            im: [0.0; N],
        }
    }
}
impl<const N: usize> SplitBuffer<N> {
    #[inline(always)]
    pub(crate) fn get(&self, i: usize) -> Complex {
        Complex {
            re: self.re[i],
            im: self.im[i],
        }
    }
    #[inline(always)]
    pub(crate) fn set(&mut self, i: usize, value: Complex) {
        self.re[i] = value.re;
        self.im[i] = value.im;
    }
    pub(crate) fn swap(&mut self, i: usize, j: usize) {
        self.re.swap(i, j);
        self.im.swap(i, j);
    }
    pub(crate) fn scale(&mut self, scale: f64) {
        for value in self.re.iter_mut().chain(&mut self.im) {
            *value *= scale;
        }
    }
}

pub(crate) struct SplitRoots {
    re: Vec<f64>,
    im: Vec<f64>,
}
impl SplitRoots {
    /// Keep the intermediate results of two full stages in registers.
    pub(crate) fn pair<const N: usize, const SMALL: usize, const LARGE: usize>(
        &self,
        data: &mut SplitBuffer<N>,
        vector: bool,
    ) {
        assert!(
            N.is_power_of_two()
                && SMALL.is_power_of_two()
                && SMALL >= 8
                && LARGE == 2 * SMALL
                && LARGE <= N
        );
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if vector {
            // SAFETY: runtime AVX2 detection; the stage validates root/data ranges.
            unsafe { self.pair_avx2::<N, SMALL, LARGE>(data) };
            return;
        }
        let _ = vector;
        self.stage::<N, SMALL, 0>(data, 0, SMALL / 2, false);
        self.stage::<N, LARGE, 0>(data, 0, LARGE / 2, false);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn pair_avx2<const N: usize, const SMALL: usize, const LARGE: usize>(
        &self,
        data: &mut SplitBuffer<N>,
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;
        let half = SMALL / 2;
        let wr = &self.re[half - 1..SMALL - 1];
        let wi = &self.im[half - 1..SMALL - 1];
        let vr = &self.re[SMALL - 1..LARGE - 1];
        let vi = &self.im[SMALL - 1..LARGE - 1];
        for (re, im) in data
            .re
            .as_chunks_mut::<LARGE>()
            .0
            .iter_mut()
            .zip(data.im.as_chunks_mut::<LARGE>().0)
        {
            let (ab, cd) = re.split_at_mut(SMALL);
            let (ar, br) = ab.split_at_mut(half);
            let (cr, dr) = cd.split_at_mut(half);
            let (ab, cd) = im.split_at_mut(SMALL);
            let (ai, bi) = ab.split_at_mut(half);
            let (ci, di) = cd.split_at_mut(half);
            for j in (0..half).step_by(4) {
                // SAFETY: half is a multiple of four. All component/root
                // sub-slices have checked lengths and the outputs are disjoint.
                unsafe {
                    let w = _mm256_loadu_pd(wr[j..j + 4].as_ptr());
                    let iw = _mm256_loadu_pd(wi[j..j + 4].as_ptr());
                    let a = _mm256_loadu_pd(ar[j..j + 4].as_ptr());
                    let ia = _mm256_loadu_pd(ai[j..j + 4].as_ptr());
                    let b = _mm256_loadu_pd(br[j..j + 4].as_ptr());
                    let ib = _mm256_loadu_pd(bi[j..j + 4].as_ptr());
                    let pr = _mm256_sub_pd(_mm256_mul_pd(b, w), _mm256_mul_pd(ib, iw));
                    let pi = _mm256_add_pd(_mm256_mul_pd(b, iw), _mm256_mul_pd(ib, w));
                    let left0 = _mm256_add_pd(a, pr);
                    let ileft0 = _mm256_add_pd(ia, pi);
                    let left1 = _mm256_sub_pd(a, pr);
                    let ileft1 = _mm256_sub_pd(ia, pi);
                    let c = _mm256_loadu_pd(cr[j..j + 4].as_ptr());
                    let ic = _mm256_loadu_pd(ci[j..j + 4].as_ptr());
                    let d = _mm256_loadu_pd(dr[j..j + 4].as_ptr());
                    let id = _mm256_loadu_pd(di[j..j + 4].as_ptr());
                    let pr = _mm256_sub_pd(_mm256_mul_pd(d, w), _mm256_mul_pd(id, iw));
                    let pi = _mm256_add_pd(_mm256_mul_pd(d, iw), _mm256_mul_pd(id, w));
                    let right0 = _mm256_add_pd(c, pr);
                    let iright0 = _mm256_add_pd(ic, pi);
                    let right1 = _mm256_sub_pd(c, pr);
                    let iright1 = _mm256_sub_pd(ic, pi);
                    let w = _mm256_loadu_pd(vr[j..j + 4].as_ptr());
                    let iw = _mm256_loadu_pd(vi[j..j + 4].as_ptr());
                    let pr = _mm256_sub_pd(_mm256_mul_pd(right0, w), _mm256_mul_pd(iright0, iw));
                    let pi = _mm256_add_pd(_mm256_mul_pd(right0, iw), _mm256_mul_pd(iright0, w));
                    _mm256_storeu_pd(ar[j..j + 4].as_mut_ptr(), _mm256_add_pd(left0, pr));
                    _mm256_storeu_pd(ai[j..j + 4].as_mut_ptr(), _mm256_add_pd(ileft0, pi));
                    _mm256_storeu_pd(cr[j..j + 4].as_mut_ptr(), _mm256_sub_pd(left0, pr));
                    _mm256_storeu_pd(ci[j..j + 4].as_mut_ptr(), _mm256_sub_pd(ileft0, pi));
                    let w = _mm256_loadu_pd(vr[j + half..j + half + 4].as_ptr());
                    let iw = _mm256_loadu_pd(vi[j + half..j + half + 4].as_ptr());
                    let pr = _mm256_sub_pd(_mm256_mul_pd(right1, w), _mm256_mul_pd(iright1, iw));
                    let pi = _mm256_add_pd(_mm256_mul_pd(right1, iw), _mm256_mul_pd(iright1, w));
                    _mm256_storeu_pd(br[j..j + 4].as_mut_ptr(), _mm256_add_pd(left1, pr));
                    _mm256_storeu_pd(bi[j..j + 4].as_mut_ptr(), _mm256_add_pd(ileft1, pi));
                    _mm256_storeu_pd(dr[j..j + 4].as_mut_ptr(), _mm256_sub_pd(left1, pr));
                    _mm256_storeu_pd(di[j..j + 4].as_mut_ptr(), _mm256_sub_pd(ileft1, pi));
                }
            }
        }
    }

    pub(crate) fn new(size: usize, inverse: bool) -> Self {
        assert!(size.is_power_of_two() && size >= 4);
        let sign = if inverse { 1.0 } else { -1.0 };
        let roots: Vec<_> = (0..size / 2)
            .map(|i| Complex::polar(1.0, sign * std::f64::consts::TAU * i as f64 / size as f64))
            .collect();
        let mut re = Vec::with_capacity(size - 1);
        let mut im = Vec::with_capacity(size - 1);
        let mut stage = 2;
        while stage <= size {
            for j in 0..stage / 2 {
                let root = roots[j * (size / stage)];
                re.push(root.re);
                im.push(root.im);
            }
            stage *= 2;
        }
        Self { re, im }
    }

    pub(crate) fn first_four<const N: usize>(&self, data: &mut SplitBuffer<N>, vector: bool) {
        assert!(N.is_power_of_two() && N >= 4);
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if vector {
            // SAFETY: caller uses the plan's runtime AVX2 detection.
            unsafe { self.first_four_avx2(data) };
            return;
        }
        let _ = vector;
        self.stage::<N, 2, 0>(data, 0, 1, false);
        self.stage::<N, 4, 0>(data, 0, 2, false);
    }

    /// MODE: 0 retains both butterfly outputs, 1 only left, 2 only right.
    #[inline]
    pub(crate) fn stage<const N: usize, const SIZE: usize, const MODE: u8>(
        &self,
        data: &mut SplitBuffer<N>,
        first: usize,
        last: usize,
        vector: bool,
    ) {
        assert!(
            N.is_power_of_two() && SIZE.is_power_of_two() && SIZE >= 2 && SIZE <= N && MODE <= 2
        );
        let half = SIZE / 2;
        assert!(first <= last && last <= half);
        let wr = &self.re[half - 1 + first..half - 1 + last];
        let wi = &self.im[half - 1 + first..half - 1 + last];
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if vector {
            // SAFETY: runtime feature detection; checked sub-slices bound loads.
            unsafe { Self::stage_avx2::<N, SIZE, MODE>(data, wr, wi, first, last) };
            return;
        }
        let _ = vector;
        for (re, im) in data
            .re
            .as_chunks_mut::<SIZE>()
            .0
            .iter_mut()
            .zip(data.im.as_chunks_mut::<SIZE>().0)
        {
            let (ar, br) = re.split_at_mut(half);
            let (ai, bi) = im.split_at_mut(half);
            for j in first..last {
                let product = Complex {
                    re: br[j],
                    im: bi[j],
                }
                .mul(Complex {
                    re: wr[j - first],
                    im: wi[j - first],
                });
                let a = Complex {
                    re: ar[j],
                    im: ai[j],
                };
                if MODE != 2 {
                    ar[j] = a.re + product.re;
                    ai[j] = a.im + product.im;
                }
                if MODE != 1 {
                    br[j] = a.re - product.re;
                    bi[j] = a.im - product.im;
                }
            }
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn stage_avx2<const N: usize, const SIZE: usize, const MODE: u8>(
        data: &mut SplitBuffer<N>,
        wr: &[f64],
        wi: &[f64],
        first: usize,
        last: usize,
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;
        for (re, im) in data
            .re
            .as_chunks_mut::<SIZE>()
            .0
            .iter_mut()
            .zip(data.im.as_chunks_mut::<SIZE>().0)
        {
            let (ar, br) = re.split_at_mut(SIZE / 2);
            let (ai, bi) = im.split_at_mut(SIZE / 2);
            let ar = &mut ar[first..last];
            let ai = &mut ai[first..last];
            let br = &mut br[first..last];
            let bi = &mut bi[first..last];
            let end = ar.len() / 4 * 4;
            for j in (0..end).step_by(4) {
                // SAFETY: j..j+4 lies within all checked, disjoint component
                // slices and root slices. Every vector lane is one butterfly.
                unsafe {
                    let a = _mm256_loadu_pd(ar[j..j + 4].as_ptr());
                    let ia = _mm256_loadu_pd(ai[j..j + 4].as_ptr());
                    let b = _mm256_loadu_pd(br[j..j + 4].as_ptr());
                    let ib = _mm256_loadu_pd(bi[j..j + 4].as_ptr());
                    let w = _mm256_loadu_pd(wr[j..j + 4].as_ptr());
                    let iw = _mm256_loadu_pd(wi[j..j + 4].as_ptr());
                    let pr = _mm256_sub_pd(_mm256_mul_pd(b, w), _mm256_mul_pd(ib, iw));
                    let pi = _mm256_add_pd(_mm256_mul_pd(b, iw), _mm256_mul_pd(ib, w));
                    if MODE != 2 {
                        _mm256_storeu_pd(ar[j..j + 4].as_mut_ptr(), _mm256_add_pd(a, pr));
                        _mm256_storeu_pd(ai[j..j + 4].as_mut_ptr(), _mm256_add_pd(ia, pi));
                    }
                    if MODE != 1 {
                        _mm256_storeu_pd(br[j..j + 4].as_mut_ptr(), _mm256_sub_pd(a, pr));
                        _mm256_storeu_pd(bi[j..j + 4].as_mut_ptr(), _mm256_sub_pd(ia, pi));
                    }
                }
            }
            for j in end..ar.len() {
                let product = Complex {
                    re: br[j],
                    im: bi[j],
                }
                .mul(Complex {
                    re: wr[j],
                    im: wi[j],
                });
                let a = Complex {
                    re: ar[j],
                    im: ai[j],
                };
                if MODE != 2 {
                    ar[j] = a.re + product.re;
                    ai[j] = a.im + product.im;
                }
                if MODE != 1 {
                    br[j] = a.re - product.re;
                    bi[j] = a.im - product.im;
                }
            }
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn first_four_avx2<const N: usize>(&self, data: &mut SplitBuffer<N>) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;
        let w2 = _mm256_set1_pd(self.re[0]);
        let iw2 = _mm256_set1_pd(self.im[0]);
        let w4 = _mm256_setr_pd(self.re[1], self.re[2], self.re[1], self.re[2]);
        let iw4 = _mm256_setr_pd(self.im[1], self.im[2], self.im[1], self.im[2]);
        for (re, im) in data
            .re
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(data.im.as_chunks_mut::<4>().0)
        {
            // SAFETY: each checked chunk contains four f64 values. The first
            // two stages stay in registers; the operation order is unchanged.
            unsafe {
                let r = _mm256_loadu_pd(re.as_ptr());
                let i = _mm256_loadu_pd(im.as_ptr());
                let ar = _mm256_permute4x64_pd::<0xa0>(r);
                let ai = _mm256_permute4x64_pd::<0xa0>(i);
                let br = _mm256_permute4x64_pd::<0xf5>(r);
                let bi = _mm256_permute4x64_pd::<0xf5>(i);
                let pr = _mm256_sub_pd(_mm256_mul_pd(br, w2), _mm256_mul_pd(bi, iw2));
                let pi = _mm256_add_pd(_mm256_mul_pd(br, iw2), _mm256_mul_pd(bi, w2));
                let r = _mm256_blend_pd::<10>(_mm256_add_pd(ar, pr), _mm256_sub_pd(ar, pr));
                let i = _mm256_blend_pd::<10>(_mm256_add_pd(ai, pi), _mm256_sub_pd(ai, pi));
                let ar = _mm256_permute4x64_pd::<0x44>(r);
                let ai = _mm256_permute4x64_pd::<0x44>(i);
                let br = _mm256_permute4x64_pd::<0xee>(r);
                let bi = _mm256_permute4x64_pd::<0xee>(i);
                let pr = _mm256_sub_pd(_mm256_mul_pd(br, w4), _mm256_mul_pd(bi, iw4));
                let pi = _mm256_add_pd(_mm256_mul_pd(br, iw4), _mm256_mul_pd(bi, w4));
                _mm256_storeu_pd(
                    re.as_mut_ptr(),
                    _mm256_blend_pd::<12>(_mm256_add_pd(ar, pr), _mm256_sub_pd(ar, pr)),
                );
                _mm256_storeu_pd(
                    im.as_mut_ptr(),
                    _mm256_blend_pd::<12>(_mm256_add_pd(ai, pi), _mm256_sub_pd(ai, pi)),
                );
            }
        }
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

    fn check<const N: usize, const PAIRED: bool>() {
        let input: [Complex; N] = std::array::from_fn(|i| Complex {
            re: (i as f64 * 0.31).sin() * [1e-12, 1.0, 1e6][i % 3],
            im: (i as f64 * 0.17).cos(),
        });
        for inverse in [false, true] {
            let sign = if inverse { 1.0 } else { -1.0 };
            let roots: Vec<_> = (0..N / 2)
                .map(|i| Complex::polar(1.0, sign * std::f64::consts::TAU * i as f64 / N as f64))
                .collect();
            let reversed: [Complex; N] = std::array::from_fn(|i: usize| {
                input[i.reverse_bits() >> (usize::BITS - N.ilog2())]
            });
            // Independent AoS radix-2 reference with the original strided roots.
            let mut expected = reversed;
            let mut size = 2;
            while size <= N {
                for start in (0..N).step_by(size) {
                    for j in 0..size / 2 {
                        (expected[start + j], expected[start + j + size / 2]) = expected[start + j]
                            .butterfly(expected[start + j + size / 2], roots[j * N / size]);
                    }
                }
                size *= 2;
            }
            let plan = SplitRoots::new(N, inverse);
            for vector in [false, vector_available()] {
                let mut data = SplitBuffer::<N>::default();
                for (i, &v) in reversed.iter().enumerate() {
                    data.set(i, v);
                }
                plan.first_four(&mut data, vector);
                if N >= 8 {
                    if PAIRED && N >= 16 {
                        plan.pair::<N, 8, 16>(&mut data, vector);
                    } else {
                        plan.stage::<N, 8, 0>(&mut data, 0, 4, vector);
                    }
                }
                if N >= 16 && !PAIRED {
                    plan.stage::<N, 16, 0>(&mut data, 0, 8, vector);
                }
                if N >= 32 {
                    if PAIRED && N >= 64 {
                        plan.pair::<N, 32, 64>(&mut data, vector);
                    } else {
                        plan.stage::<N, 32, 0>(&mut data, 0, 16, vector);
                    }
                }
                if N >= 64 && !PAIRED {
                    plan.stage::<N, 64, 0>(&mut data, 0, 32, vector);
                }
                if N >= 128 {
                    if PAIRED && N >= 256 {
                        plan.pair::<N, 128, 256>(&mut data, vector);
                    } else {
                        plan.stage::<N, 128, 0>(&mut data, 0, 64, vector);
                    }
                }
                if N >= 256 && !PAIRED {
                    plan.stage::<N, 256, 0>(&mut data, 0, 128, vector);
                }
                if N >= 512 {
                    if PAIRED && N >= 1024 {
                        plan.pair::<N, 512, 1024>(&mut data, vector);
                    } else {
                        plan.stage::<N, 512, 0>(&mut data, 0, 256, vector);
                    }
                }
                if N >= 1024 && !PAIRED {
                    plan.stage::<N, 1024, 0>(&mut data, 0, 512, vector);
                }
                for (i, reference) in expected.iter().enumerate() {
                    let actual = data.get(i);
                    assert_eq!(
                        actual.re.to_bits(),
                        reference.re.to_bits(),
                        "N={N}, bin={i}, inverse={inverse}, vector={vector}"
                    );
                    assert_eq!(
                        actual.im.to_bits(),
                        reference.im.to_bits(),
                        "N={N}, bin={i}, inverse={inverse}, vector={vector}"
                    );
                }
            }
        }
    }

    #[test]
    fn split_scalar_and_vector_transforms_match_original_bits() {
        check::<4, false>();
        check::<4, true>();
        check::<8, false>();
        check::<8, true>();
        check::<16, false>();
        check::<16, true>();
        check::<32, false>();
        check::<32, true>();
        check::<64, false>();
        check::<64, true>();
        check::<256, false>();
        check::<256, true>();
        check::<1024, false>();
        check::<1024, true>();
    }

    #[test]
    fn partial_stages_preserve_untouched_outputs_and_handle_vector_tails() {
        let roots = SplitRoots::new(32, false);
        for vector in [false, vector_available()] {
            for first in 0..=8 {
                for last in first..=8 {
                    for mode in 0..3 {
                        let mut data = SplitBuffer::<32>::default();
                        for i in 0..32 {
                            data.set(
                                i,
                                Complex {
                                    re: i as f64 * 0.31,
                                    im: i as f64 * -0.17,
                                },
                            );
                        }
                        let mut expected: [Complex; 32] = std::array::from_fn(|i| data.get(i));
                        for start in [0, 16] {
                            for j in first..last {
                                let (a, b) = expected[start + j].butterfly(
                                    expected[start + j + 8],
                                    Complex {
                                        re: roots.re[7 + j],
                                        im: roots.im[7 + j],
                                    },
                                );
                                if mode != 2 {
                                    expected[start + j] = a;
                                }
                                if mode != 1 {
                                    expected[start + j + 8] = b;
                                }
                            }
                        }
                        match mode {
                            0 => roots.stage::<32, 16, 0>(&mut data, first, last, vector),
                            1 => roots.stage::<32, 16, 1>(&mut data, first, last, vector),
                            _ => roots.stage::<32, 16, 2>(&mut data, first, last, vector),
                        }
                        for (i, expected) in expected.iter().enumerate() {
                            assert_eq!(data.get(i).re.to_bits(), expected.re.to_bits());
                            assert_eq!(data.get(i).im.to_bits(), expected.im.to_bits());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_invalid_plans_and_stage_ranges() {
        for size in [0, 1, 2, 6] {
            assert!(std::panic::catch_unwind(|| SplitRoots::new(size, false)).is_err());
        }
        for vector in [false, vector_available()] {
            assert!(
                std::panic::catch_unwind(|| SplitRoots::new(8, false).stage::<8, 8, 0>(
                    &mut SplitBuffer::default(),
                    0,
                    5,
                    vector
                ))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(|| SplitRoots::new(8, false).stage::<8, 8, 0>(
                    &mut SplitBuffer::default(),
                    3,
                    2,
                    vector
                ))
                .is_err()
            );
        }
    }
}
