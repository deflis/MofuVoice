//! Frequency-domain pitch-synchronous overlap-add frame transformation.
//!
//! A pitch-synchronous analysis frame is transformed, its complex source
//! spectrum is compressed or expanded along the frequency axis, and the frame
//! is reconstructed before pitch-synchronous overlap-add. Pitch marks and the
//! synthesis-mark schedule are owned by the streaming PSOLA stage.
use crate::dsp::{complex::Complex, fft::Fft};

/// Fixed frame size for the first FD-PSOLA implementation. At 48 kHz this
/// accommodates the longest analysis support used by the current voice range.
pub(crate) const FRAME: usize = 2048;
pub(crate) const LATENCY: usize = crate::td_psola::LATENCY;
const _: () = assert!(
    LATENCY >= crate::td_psola::LOOKAHEAD + 2 * crate::td_psola::MAX_PERIOD + FRAME / 2 + 128
);

pub(crate) struct FdFrameTransform {
    fft: Fft,
    spectrum: Box<[Complex; FRAME]>,
    warped: Box<[Complex; FRAME]>,
    envelope: Box<[f64; FRAME / 2 + 1]>,
}

impl FdFrameTransform {
    pub(crate) fn new() -> Self {
        Self {
            fft: Fft::new(FRAME),
            spectrum: Box::new([Complex::default(); FRAME]),
            warped: Box::new([Complex::default(); FRAME]),
            envelope: Box::new([0.0; FRAME / 2 + 1]),
        }
    }

    pub(crate) fn process_grain(
        &mut self,
        input: &[f64; FRAME],
        output: &mut [f64; FRAME],
        ratio: f64,
        preserve_formants: bool,
    ) {
        self.transform(input, output, ratio, preserve_formants);
    }

    /// Apply FD-PSOLA's compression-expansion source-spectrum mapping to one
    /// real, pitch-synchronous frame. The caller supplies an already-windowed
    /// frame centred on an analysis pitch mark. The output is the reconstructed
    /// real frame, ready for synthesis-windowing and overlap-add.
    pub(crate) fn transform(
        &mut self,
        input: &[f64; FRAME],
        output: &mut [f64; FRAME],
        ratio: f64,
        preserve_formants: bool,
    ) {
        assert!(ratio.is_finite() && ratio > 0.0);
        for (dst, &sample) in self.spectrum.iter_mut().zip(input) {
            *dst = Complex {
                re: sample,
                im: 0.0,
            };
        }
        self.fft.forward(&mut self.spectrum[..]);

        // Resample positive frequencies linearly. Conjugate symmetry is
        // restored explicitly so the inverse transform remains real-valued.
        let half = FRAME / 2;
        self.warped.fill(Complex::default());
        if preserve_formants {
            // Smooth log magnitude isolates broad vocal-tract resonances from
            // the fine harmonic source before pitch scaling.
            for k in 0..=half {
                let first = k.saturating_sub(12);
                let last = (k + 12).min(half);
                let mut sum = 0.0;
                for bin in first..=last {
                    let value = self.spectrum[bin];
                    sum += value.re.hypot(value.im).max(1e-12).ln();
                }
                self.envelope[k] = sum / (last - first + 1) as f64;
            }
        }
        for k in 0..=half {
            let source = k as f64 / ratio;
            if source > half as f64 {
                continue;
            }
            let lower = (source.floor() as usize).min(half);
            let upper = (lower + 1).min(half);
            let fraction = (source - lower as f64).clamp(0.0, 1.0);
            let mut a = self.spectrum[lower];
            let mut b = self.spectrum[upper];
            if preserve_formants {
                a = a.scale((-self.envelope[lower]).exp());
                b = b.scale((-self.envelope[upper]).exp());
            }
            let value = Complex {
                re: a.re + (b.re - a.re) * fraction,
                im: a.im + (b.im - a.im) * fraction,
            };
            let mut value = if preserve_formants {
                value.scale(self.envelope[k].exp())
            } else {
                value
            };
            // Spectra are computed with the frame's left edge as time zero.
            // Re-reference each warped component to the pitch mark at frame
            // centre so neighbouring PSOLA grains keep a coherent excitation.
            let phase_correction =
                -std::f64::consts::TAU * (k as f64 - source) * (half as f64 / FRAME as f64);
            value = value.mul(Complex::polar(1.0, phase_correction));
            self.warped[k] = if k == 0 || k == half {
                Complex {
                    re: value.re,
                    im: 0.0,
                }
            } else {
                value
            };
            if k != 0 && k != half {
                self.warped[FRAME - k] = Complex {
                    re: value.re,
                    im: -value.im,
                };
            }
        }
        self.fft.inverse(&mut self.warped[..]);
        for (dst, value) in output.iter_mut().zip(self.warped.iter()) {
            *dst = value.re;
        }
    }
}
