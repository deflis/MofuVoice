//! Radix-2 complex FFT with tables allocated at construction.
use super::complex::Complex;

pub(crate) struct Fft {
    size: usize,
    reverse: Vec<usize>,
    roots: Vec<Complex>,
}

impl Fft {
    pub(crate) fn new(size: usize) -> Self {
        assert!(size.is_power_of_two() && size >= 4);
        let bits = size.ilog2();
        Self {
            size,
            reverse: (0..size)
                .map(|i| i.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            roots: (0..size / 2)
                .map(|i| Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / size as f64))
                .collect(),
        }
    }
    pub(crate) fn forward(&self, data: &mut [Complex]) {
        assert_eq!(data.len(), self.size);
        for i in 0..self.size {
            if i < self.reverse[i] {
                data.swap(i, self.reverse[i]);
            }
        }
        let mut size = 2;
        while size <= self.size {
            let half = size / 2;
            for start in (0..self.size).step_by(size) {
                for j in 0..half {
                    let root = self.roots[j * self.size / size];
                    (data[start + j], data[start + j + half]) =
                        data[start + j].butterfly(data[start + j + half], root);
                }
            }
            size *= 2;
        }
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
}
