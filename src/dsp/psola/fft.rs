//! Linear autocorrelation using the in-repository FFT. Storage and tables are
//! allocated before streaming; zero padding prevents circular wrap at all
//! requested lags, without requiring a full 2*N transform.
use crate::dsp::{complex::Complex, fft::Fft};

pub(super) struct FftCorrelation {
    fft: Fft,
    buffer: Vec<Complex>,
    length: usize,
    max_lag: usize,
}

impl FftCorrelation {
    pub(super) fn new(length: usize, max_lag: usize) -> Self {
        assert!(length > max_lag);
        let size = (length + max_lag).next_power_of_two().max(4);
        Self {
            fft: Fft::new(size),
            buffer: vec![Complex::default(); size],
            length,
            max_lag,
        }
    }

    pub(super) fn mpm(&mut self, input: &[f64], output: &mut [f64]) {
        assert_eq!(input.len(), self.length);
        assert!(output.len() <= self.max_lag + 1);
        self.buffer.fill(Complex::default());
        for (value, &sample) in self.buffer.iter_mut().zip(input) {
            value.re = sample;
        }
        self.fft.forward(&mut self.buffer);
        for bin in &mut self.buffer {
            bin.re = bin.re * bin.re + bin.im * bin.im;
            bin.im = 0.0;
        }
        self.fft.inverse(&mut self.buffer);
        for (value, bin) in output.iter_mut().zip(&self.buffer) {
            *value = bin.re;
        }
        super::correlation::normalize_mpm(input, output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_matches_linear_mpm_for_all_windows_without_wrap_or_stale_tail() {
        for window in [1024, 2048, 4096, 8192] {
            let length = window / 4 + 201;
            let mut fft = FftCorrelation::new(length, 201);
            let mut seed = 17u32;
            for kind in 0..6 {
                let input: Vec<_> = (0..length)
                    .map(|i| {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        match kind {
                            0 => seed as f64 / u32::MAX as f64 - 0.5,
                            1 => (i as f64 * 0.059).cos(),
                            2 => {
                                if i == 0 || i == length - 1 {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            3 => 0.1,
                            4 => 0.0,
                            _ => 1e-15 * (i as f64 * 0.13).cos(),
                        }
                    })
                    .collect();
                let mut expected = [0.0; 202];
                let mut actual = expected;
                super::super::correlation::scalar(&input, &mut expected, true);
                fft.mpm(&input, &mut actual);
                for (lag, (&a, &b)) in expected.iter().zip(&actual).enumerate() {
                    assert!(
                        b.is_finite() && (a - b).abs() < 2e-12,
                        "window={window} kind={kind} lag={lag} direct={a} FFT={b}"
                    );
                }
            }
        }
    }
}
