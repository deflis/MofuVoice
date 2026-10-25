//! Fixed-stage FFT butterflies, shared by real and complex transforms.
use super::complex::Complex;

pub(crate) struct FftKernel;
impl FftKernel {
    pub(crate) fn stages<const N: usize>(roots: &[Complex], data: &mut [Complex; N]) {
        // Fixed stage sizes let the compiler specialize strides and butterfly
        // loops for our power-of-two transforms instead of dividing at runtime.
        Self::stage::<N, 2>(roots, data);
        Self::stage::<N, 4>(roots, data);
        Self::stage::<N, 8>(roots, data);
        Self::stage::<N, 16>(roots, data);
        Self::stage::<N, 32>(roots, data);
        Self::stage::<N, 64>(roots, data);
        Self::stage::<N, 128>(roots, data);
        Self::stage::<N, 256>(roots, data);
        Self::stage::<N, 512>(roots, data);
        Self::stage::<N, 1024>(roots, data);
        Self::stage::<N, 2048>(roots, data);
        Self::stage::<N, 4096>(roots, data);
        Self::stage::<N, 8192>(roots, data);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    pub(crate) unsafe fn stages_avx2<const N: usize>(roots: &[Complex], data: &mut [Complex; N]) {
        Self::stage::<N, 2>(roots, data);
        // SAFETY: this function requires AVX2 and each stage checks its bounds.
        unsafe {
            Self::stage_avx2::<N, 4>(roots, data);
            Self::stage_avx2::<N, 8>(roots, data);
            Self::stage_avx2::<N, 16>(roots, data);
            Self::stage_avx2::<N, 32>(roots, data);
            Self::stage_avx2::<N, 64>(roots, data);
            Self::stage_avx2::<N, 128>(roots, data);
            Self::stage_avx2::<N, 256>(roots, data);
            Self::stage_avx2::<N, 512>(roots, data);
            Self::stage_avx2::<N, 1024>(roots, data);
            Self::stage_avx2::<N, 2048>(roots, data);
            Self::stage_avx2::<N, 4096>(roots, data);
            Self::stage_avx2::<N, 8192>(roots, data);
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    pub(crate) unsafe fn stage_avx2<const N: usize, const SIZE: usize>(
        roots: &[Complex],
        data: &mut [Complex; N],
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;
        if SIZE > N {
            return;
        }
        let half = SIZE / 2;
        let roots = &roots[half - 1..SIZE - 1];
        for block in data.as_chunks_mut::<SIZE>().0 {
            let (left, right) = block.split_at_mut(half);
            for ((left, right), root) in left
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(right.as_chunks_mut::<2>().0)
                .zip(roots.as_chunks::<2>().0)
            {
                // SAFETY: repr(C) Complex is two f64 values. Each checked chunk
                // contains two complexes (four f64s); unaligned loads/stores stay
                // inside disjoint left/right slices. No FMA changes rounding.
                unsafe {
                    let a = _mm256_loadu_pd(left.as_ptr().cast());
                    let b = _mm256_loadu_pd(right.as_ptr().cast());
                    let w = _mm256_loadu_pd(root.as_ptr().cast());
                    let re = _mm256_mul_pd(_mm256_permute_pd::<0>(b), w);
                    let im = _mm256_mul_pd(_mm256_permute_pd::<15>(b), _mm256_permute_pd::<5>(w));
                    let product = _mm256_addsub_pd(re, im);
                    _mm256_storeu_pd(left.as_mut_ptr().cast(), _mm256_add_pd(a, product));
                    _mm256_storeu_pd(right.as_mut_ptr().cast(), _mm256_sub_pd(a, product));
                }
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    pub(crate) unsafe fn stages_neon<const N: usize>(roots: &[Complex], data: &mut [Complex; N]) {
        Self::stage::<N, 2>(roots, data);
        // SAFETY: this function requires NEON and each stage checks its bounds.
        unsafe {
            Self::stage_neon::<N, 4>(roots, data);
            Self::stage_neon::<N, 8>(roots, data);
            Self::stage_neon::<N, 16>(roots, data);
            Self::stage_neon::<N, 32>(roots, data);
            Self::stage_neon::<N, 64>(roots, data);
            Self::stage_neon::<N, 128>(roots, data);
            Self::stage_neon::<N, 256>(roots, data);
            Self::stage_neon::<N, 512>(roots, data);
            Self::stage_neon::<N, 1024>(roots, data);
            Self::stage_neon::<N, 2048>(roots, data);
            Self::stage_neon::<N, 4096>(roots, data);
            Self::stage_neon::<N, 8192>(roots, data);
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    pub(crate) unsafe fn stage_neon<const N: usize, const SIZE: usize>(
        roots: &[Complex],
        data: &mut [Complex; N],
    ) {
        use std::arch::aarch64::*;
        if SIZE > N {
            return;
        }
        let half = SIZE / 2;
        let roots = &roots[half - 1..SIZE - 1];
        for block in data.as_chunks_mut::<SIZE>().0 {
            let (left, right) = block.split_at_mut(half);
            for ((left, right), root) in left
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(right.as_chunks_mut::<2>().0)
                .zip(roots.as_chunks::<2>().0)
            {
                // SAFETY: each chunk contains two repr(C) complexes (four f64s).
                // Deinterleaved loads/stores stay in disjoint checked slices.
                unsafe {
                    let a = vld2q_f64(left.as_ptr().cast());
                    let b = vld2q_f64(right.as_ptr().cast());
                    let w = vld2q_f64(root.as_ptr().cast());
                    let re = vsubq_f64(vmulq_f64(b.0, w.0), vmulq_f64(b.1, w.1));
                    let im = vaddq_f64(vmulq_f64(b.0, w.1), vmulq_f64(b.1, w.0));
                    vst2q_f64(
                        left.as_mut_ptr().cast(),
                        float64x2x2_t(vaddq_f64(a.0, re), vaddq_f64(a.1, im)),
                    );
                    vst2q_f64(
                        right.as_mut_ptr().cast(),
                        float64x2x2_t(vsubq_f64(a.0, re), vsubq_f64(a.1, im)),
                    );
                }
            }
        }
    }

    #[inline(always)]
    pub(crate) fn stage<const N: usize, const SIZE: usize>(
        roots: &[Complex],
        data: &mut [Complex; N],
    ) {
        if SIZE <= N {
            let half = SIZE / 2;
            let roots = &roots[half - 1..SIZE - 1];
            for block in data.as_chunks_mut::<SIZE>().0 {
                let (left, right) = block.split_at_mut(half);
                for ((left, right), root) in left.iter_mut().zip(right).zip(roots) {
                    (*left, *right) = left.butterfly(*right, *root);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_reference_forward<const N: usize>(data: &mut [Complex; N]) {
        let roots: Vec<_> = (0..N / 2)
            .map(|i| Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / N as f64))
            .collect();
        let bits = N.ilog2();
        for i in 0..N {
            let j = i.reverse_bits() >> (usize::BITS - bits);
            if i < j {
                data.swap(i, j);
            }
        }
        let mut size = 2;
        while size <= N {
            let half = size / 2;
            for start in (0..N).step_by(size) {
                for j in 0..half {
                    let root = roots[j * N / size];
                    (data[start + j], data[start + j + half]) =
                        data[start + j].butterfly(data[start + j + half], root);
                }
            }
            size *= 2;
        }
    }

    fn check<const N: usize>() {
        // Build stage tables from the ordinary FFT table; prepare the bit-reversed
        // input explicitly so this test covers the kernel rather than its caller.
        let roots: Vec<_> = (0..N / 2)
            .map(|i| Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / N as f64))
            .collect();
        let mut stages = Vec::new();
        let mut size = 2;
        while size <= N {
            for j in 0..size / 2 {
                stages.push(roots[j * N / size]);
            }
            size *= 2;
        }
        let input: [Complex; N] = std::array::from_fn(|i| {
            let scale = [1e-12, 1.0, 1e6][i % 3];
            Complex {
                re: (i as f64 * 0.31).sin() * scale,
                im: (i as f64 * 0.17).cos() * scale,
            }
        });
        let mut expected = input;
        scalar_reference_forward(&mut expected);
        let reversed: [Complex; N] =
            std::array::from_fn(|i: usize| input[i.reverse_bits() >> (usize::BITS - N.ilog2())]);
        let mut scalar = reversed;
        FftKernel::stages(&stages, &mut scalar);
        for (actual, reference) in scalar.iter().zip(expected) {
            assert_eq!(actual.re.to_bits(), reference.re.to_bits(), "N={N}");
            assert_eq!(actual.im.to_bits(), reference.im.to_bits(), "N={N}");
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if std::is_x86_feature_detected!("avx2") {
            let mut vector = reversed;
            // SAFETY: AVX2 was detected above; N and stage tables match.
            unsafe { FftKernel::stages_avx2(&stages, &mut vector) };
            for (actual, reference) in vector.iter().zip(scalar) {
                assert_eq!(actual.re.to_bits(), reference.re.to_bits(), "AVX2 N={N}");
                assert_eq!(actual.im.to_bits(), reference.im.to_bits(), "AVX2 N={N}");
            }
        }
        #[cfg(target_arch = "aarch64")]
        if std::arch::is_aarch64_feature_detected!("neon") {
            let mut vector = reversed;
            // SAFETY: NEON detected above; stage tables match N.
            unsafe { FftKernel::stages_neon(&stages, &mut vector) };
            for (actual, reference) in vector.iter().zip(scalar) {
                assert_eq!(actual.re.to_bits(), reference.re.to_bits(), "NEON N={N}");
                assert_eq!(actual.im.to_bits(), reference.im.to_bits(), "NEON N={N}");
            }
        }
    }

    #[test]
    fn specialized_scalar_and_simd_stages_match_generic_fft_bits() {
        check::<4>();
        check::<8>();
        check::<32>();
        check::<256>();
        check::<1024>();
        check::<2048>();
        check::<4096>();
    }
}
