#[derive(Clone, Copy, Default)]
#[repr(C)]
pub(crate) struct Complex {
    pub(crate) re: f64,
    pub(crate) im: f64,
}

impl Complex {
    /// Batch magnitudes through disjoint slice references so the loop can vectorize.
    #[inline(always)]
    pub(crate) fn magnitudes(input: &[Self], output: &mut [f64]) {
        assert_eq!(input.len(), output.len());
        for (value, magnitude) in input.iter().zip(output) {
            *magnitude = (value.re * value.re + value.im * value.im).sqrt();
        }
    }

    #[inline(always)]
    pub(crate) fn butterfly(self, other: Self, root: Self) -> (Self, Self) {
        let product = other.mul(root);
        (
            Self {
                re: self.re + product.re,
                im: self.im + product.im,
            },
            Self {
                re: self.re - product.re,
                im: self.im - product.im,
            },
        )
    }

    #[inline(always)]
    pub(crate) fn polar(magnitude: f64, phase: f64) -> Self {
        let (sin, cos) = phase.sin_cos();
        Self {
            re: magnitude * cos,
            im: magnitude * sin,
        }
    }

    #[inline(always)]
    pub(crate) fn mul(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }

    #[inline(always)]
    pub(crate) fn scale(self, weight: f64) -> Self {
        Self {
            re: self.re * weight,
            im: self.im * weight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batched_magnitudes_preserve_scalar_bits_and_slice_guards() {
        let values: Vec<_> = (0..40)
            .map(|i| Complex {
                re: (i as f64 * 0.31).sin() * [0.0, 1e-160, 1.0, 1e160][i % 4],
                im: (i as f64 * 0.17).cos() * [1e-160, 1.0, 1e160, 0.0][i % 4],
            })
            .collect();
        for first in 0..4 {
            for length in 0..=32 {
                let mut output = vec![12345.0; first + length + 2];
                Complex::magnitudes(
                    &values[first..first + length],
                    &mut output[first..first + length],
                );
                assert!(output[..first].iter().all(|&value| value == 12345.0));
                assert!(output[first + length..]
                    .iter()
                    .all(|&value| value == 12345.0));
                for (&actual, value) in output[first..first + length]
                    .iter()
                    .zip(&values[first..first + length])
                {
                    let expected = (value.re * value.re + value.im * value.im).sqrt();
                    assert_eq!(actual.to_bits(), expected.to_bits());
                }
            }
        }
        assert!(std::panic::catch_unwind(|| {
            Complex::magnitudes(&[Complex::default(); 2], &mut [0.0]);
        })
        .is_err());
    }

    #[test]
    fn layout_matches_interleaved_f64_simd_contract() {
        assert_eq!(
            std::mem::size_of::<Complex>(),
            2 * std::mem::size_of::<f64>()
        );
        assert_eq!(std::mem::align_of::<Complex>(), std::mem::align_of::<f64>());
        assert_eq!(std::mem::offset_of!(Complex, re), 0);
        assert_eq!(
            std::mem::offset_of!(Complex, im),
            std::mem::size_of::<f64>()
        );
    }

    #[test]
    fn multiplication_and_butterfly_match_known_complex_values() {
        let a = Complex { re: 2.0, im: -3.0 };
        let b = Complex { re: -4.0, im: 5.0 };
        let product = a.mul(b);
        assert_eq!((product.re, product.im), (7.0, 22.0));
        let (sum, difference) = a.butterfly(b, Complex { re: 0.0, im: 1.0 });
        assert_eq!((sum.re, sum.im), (-3.0, -7.0));
        assert_eq!((difference.re, difference.im), (7.0, 1.0));
        let scaled = a.scale(-0.5);
        assert_eq!((scaled.re, scaled.im), (-1.0, 1.5));
    }

    #[test]
    fn polar_rotation_preserves_magnitude_and_adds_phase() {
        for phase in [-100.0, -std::f64::consts::PI, -0.1, 0.0, 0.3, 100.0] {
            let value = Complex::polar(0.7, phase).mul(Complex::polar(1.0, 0.25));
            assert!((value.re.hypot(value.im) - 0.7).abs() < 2e-15);
            let expected = Complex::polar(0.7, phase + 0.25);
            assert!((value.re - expected.re).abs() < 1e-14);
            assert!((value.im - expected.im).abs() < 1e-14);
        }
    }
}
