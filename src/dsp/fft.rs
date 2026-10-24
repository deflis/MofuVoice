//! Radix-2 complex FFT with tables allocated at construction.
//! Stage twiddles are packed contiguously so the shared SIMD butterflies can
//! stream them without strided loads.
use super::{complex::Complex, fft_kernel::FftKernel};

pub(crate) struct Fft {
    size: usize,
    reverse: Vec<usize>,
    /// Contiguous stage table of length `size - 1` for [`FftKernel`].
    stages: Vec<Complex>,
    vector: bool,
}

impl Fft {
    pub(crate) fn new(size: usize) -> Self {
        assert!(size.is_power_of_two() && size >= 4 && size <= 8192);
        let bits = size.ilog2();
        let roots: Vec<_> = (0..size / 2)
            .map(|i| Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / size as f64))
            .collect();
        let mut stages = Vec::with_capacity(size - 1);
        let mut width = 2;
        while width <= size {
            for j in 0..width / 2 {
                stages.push(roots[j * size / width]);
            }
            width *= 2;
        }
        Self {
            size,
            reverse: (0..size)
                .map(|i| i.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            stages,
            vector: {
                #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                {
                    std::is_x86_feature_detected!("avx2")
                }
                #[cfg(target_arch = "aarch64")]
                {
                    std::arch::is_aarch64_feature_detected!("neon")
                }
                #[cfg(not(any(
                    target_arch = "x86",
                    target_arch = "x86_64",
                    target_arch = "aarch64"
                )))]
                {
                    false
                }
            },
        }
    }

    pub(crate) fn forward(&self, data: &mut [Complex]) {
        assert_eq!(data.len(), self.size);
        for i in 0..self.size {
            if i < self.reverse[i] {
                data.swap(i, self.reverse[i]);
            }
        }
        match self.size {
            4 => self.stages_n::<4>(data),
            8 => self.stages_n::<8>(data),
            16 => self.stages_n::<16>(data),
            32 => self.stages_n::<32>(data),
            64 => self.stages_n::<64>(data),
            128 => self.stages_n::<128>(data),
            256 => self.stages_n::<256>(data),
            512 => self.stages_n::<512>(data),
            1024 => self.stages_n::<1024>(data),
            2048 => self.stages_n::<2048>(data),
            4096 => self.stages_n::<4096>(data),
            8192 => self.stages_n::<8192>(data),
            _ => unreachable!("Fft::new rejects unsupported sizes"),
        }
    }

    fn stages_n<const N: usize>(&self, data: &mut [Complex]) {
        let data: &mut [Complex; N] = data.try_into().expect("buffer length matches plan");
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if self.vector {
            // SAFETY: AVX2 was detected at construction; N matches the plan size.
            unsafe { FftKernel::stages_avx2(&self.stages, data) };
            return;
        }
        #[cfg(target_arch = "aarch64")]
        if self.vector {
            // SAFETY: NEON was detected at construction; N matches the plan size.
            unsafe { FftKernel::stages_neon(&self.stages, data) };
            return;
        }
        FftKernel::stages(&self.stages, data);
    }

    /// In-place inverse transform. The forward transform is unscaled, so the
    /// inverse applies 1/N after conjugating the input and output.
    pub(crate) fn inverse(&self, data: &mut [Complex]) {
        assert_eq!(data.len(), self.size);
        for value in data.iter_mut() {
            value.im = -value.im;
        }
        self.forward(data);
        let scale = 1.0 / self.size as f64;
        for value in data.iter_mut() {
            value.re *= scale;
            value.im *= -scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_direct_dft_for_complex_input() {
        for size in [4, 8, 32, 256] {
            let input: Vec<_> = (0..size)
                .map(|i| Complex {
                    re: (i as f64 * 0.31).sin(),
                    im: (i as f64 * 0.17).cos(),
                })
                .collect();
            let mut actual = input.clone();
            Fft::new(size).forward(&mut actual);
            for (k, result) in actual.iter().enumerate() {
                let mut expected = Complex::default();
                for (n, value) in input.iter().enumerate() {
                    let phase = -std::f64::consts::TAU * k as f64 * n as f64 / size as f64;
                    expected.re += value.re * phase.cos() - value.im * phase.sin();
                    expected.im += value.re * phase.sin() + value.im * phase.cos();
                }
                assert!((result.re - expected.re).abs() < 1e-10);
                assert!((result.im - expected.im).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn known_signals_fix_bin_order_sign_and_normalization() {
        for size in [4, 16, 256, 2048] {
            let fft = Fft::new(size);
            let bin = size / 4;
            for signal in 0..3 {
                let mut data: Vec<_> = (0..size)
                    .map(|n| match signal {
                        0 => Complex {
                            re: if n == 1 { 1.0 } else { 0.0 },
                            im: 0.0,
                        },
                        1 => Complex { re: 0.25, im: -0.5 },
                        _ => Complex::polar(
                            0.5,
                            std::f64::consts::TAU * bin as f64 * n as f64 / size as f64,
                        ),
                    })
                    .collect();
                fft.forward(&mut data);
                for (k, value) in data.iter().enumerate() {
                    let expected = match signal {
                        0 => Complex::polar(1.0, -std::f64::consts::TAU * k as f64 / size as f64),
                        1 if k == 0 => Complex {
                            re: size as f64 * 0.25,
                            im: -(size as f64) * 0.5,
                        },
                        2 if k == bin => Complex {
                            re: size as f64 * 0.5,
                            im: 0.0,
                        },
                        _ => Complex::default(),
                    };
                    assert!(
                        (value.re - expected.re).abs() < 1e-9,
                        "size={size}, signal={signal}, bin={k}"
                    );
                    assert!((value.im - expected.im).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn parseval_energy_and_reusing_plan_preserve_buffer_boundaries() {
        for size in [4, 32, 256, 2048] {
            let fft = Fft::new(size);
            let guard = Complex {
                re: -1234.5,
                im: 6789.0,
            };
            let mut storage = vec![guard; size + 2];
            for pass in 0..3 {
                let mut input_energy = 0.0;
                for (i, value) in storage[1..=size].iter_mut().enumerate() {
                    *value = Complex {
                        re: (i as f64 * 0.31 + pass as f64).sin(),
                        im: (i as f64 * 0.17).cos(),
                    };
                    input_energy += value.re * value.re + value.im * value.im;
                }
                fft.forward(&mut storage[1..=size]);
                let output_energy: f64 = storage[1..=size]
                    .iter()
                    .map(|v| v.re * v.re + v.im * v.im)
                    .sum();
                assert!((output_energy / size as f64 - input_energy).abs() < input_energy * 1e-13);
                for index in [0, size + 1] {
                    assert_eq!(storage[index].re.to_bits(), guard.re.to_bits());
                    assert_eq!(storage[index].im.to_bits(), guard.im.to_bits());
                }
            }
            let mut silence = vec![Complex::default(); size];
            fft.forward(&mut silence);
            assert!(silence.iter().all(|v| v.re == 0.0 && v.im == 0.0));
        }
    }

    #[test]
    #[should_panic]
    fn rejects_non_power_of_two() {
        Fft::new(6);
    }

    #[test]
    fn rejects_empty_and_undersized_plans_and_oversized_buffers() {
        for size in [0, 1, 2] {
            assert!(std::panic::catch_unwind(|| Fft::new(size)).is_err());
        }
        assert!(
            std::panic::catch_unwind(|| Fft::new(4).forward(&mut [Complex::default(); 8])).is_err()
        );
    }

    #[test]
    #[should_panic]
    fn rejects_wrong_buffer_size() {
        Fft::new(8).forward(&mut [Complex::default(); 4]);
    }

    #[test]
    fn vector_and_scalar_plans_match_bits() {
        for size in [4, 32, 256, 1024, 4096] {
            let mut vector = Fft::new(size);
            let mut scalar = Fft::new(size);
            scalar.vector = false;
            vector.vector = {
                #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                {
                    std::is_x86_feature_detected!("avx2")
                }
                #[cfg(target_arch = "aarch64")]
                {
                    std::arch::is_aarch64_feature_detected!("neon")
                }
                #[cfg(not(any(
                    target_arch = "x86",
                    target_arch = "x86_64",
                    target_arch = "aarch64"
                )))]
                {
                    false
                }
            };
            if !vector.vector {
                continue;
            }
            let input: Vec<_> = (0..size)
                .map(|i| Complex {
                    re: (i as f64 * 0.31).sin(),
                    im: (i as f64 * 0.17).cos(),
                })
                .collect();
            let mut a = input.clone();
            let mut b = input;
            vector.forward(&mut a);
            scalar.forward(&mut b);
            for (actual, expected) in a.iter().zip(&b) {
                assert_eq!(actual.re.to_bits(), expected.re.to_bits());
                assert_eq!(actual.im.to_bits(), expected.im.to_bits());
            }
        }
    }
}
