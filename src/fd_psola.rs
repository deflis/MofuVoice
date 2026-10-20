//! Frequency-domain pitch-synchronous overlap-add frame transformation.
//!
//! A pitch-synchronous analysis frame is transformed, its complex source
//! spectrum is compressed or expanded along the frequency axis, and the frame
//! is reconstructed before pitch-synchronous overlap-add. Pitch marks and the
//! synthesis-mark schedule are owned by the streaming PSOLA stage.
use crate::dsp::{complex::Complex, fft::Fft};

pub(crate) struct FdFrameTransform {
    center: f64,
    fft: Fft,
    spectrum: Vec<Complex>,
    warped: Vec<Complex>,
    envelope: Vec<f64>,
    log_prefix: Vec<f64>,
}

/// FD-specific frame analysis and overlap-add state. The shared PSOLA tracker
/// supplies each source and synthesis mark; TD-PSOLA never owns these buffers.
pub(crate) struct FdSynthesis {
    analysis_window: Vec<f64>,
    synthesis_window: Vec<f64>,
    synthesis: usize,
    transform: FdFrameTransform,
    input: Vec<f64>,
    output: Vec<f64>,
}

impl FdSynthesis {
    pub(crate) fn new(windows: crate::pitch::SpectralWindows) -> Self {
        windows.validate().expect("invalid FD-PSOLA windows");
        let (analysis_window, synthesis_window, _) = crate::pitch::asymmetric_windows(windows);
        let center = (windows.analysis - windows.synthesis / 2) as f64;
        Self {
            analysis_window,
            synthesis_window,
            synthesis: windows.synthesis,
            transform: FdFrameTransform::new(windows.analysis, center),
            input: vec![0.0; windows.analysis],
            output: vec![0.0; windows.analysis],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn grain_impl(
        &mut self,
        source: f64,
        destination: f64,
        _left: f64,
        _right: f64,
        _gain: f64,
        ratio: f64,
        preserve_formants: bool,
        clock: usize,
        input: &[f64],
        output: &mut [f64],
        weights: &mut [f64],
        coverage: &mut [bool],
    ) {
        let size = self.input.len();
        let center = self.transform.center;
        assert!(source + (size as f64 - 1.0 - center) + 2.0 < clock as f64);
        assert!(
            source - center - 1.0 >= clock.saturating_sub(crate::psola_common::RING) as f64
                || clock < crate::psola_common::RING
        );
        for i in 0..size {
            let offset = i as f64 - center;
            self.input[i] =
                crate::psola_common::interpolate(input, source + offset) * self.analysis_window[i];
        }
        self.transform
            .process_grain(&self.input, &mut self.output, ratio, preserve_formants);
        let radius = self.synthesis as f64 / 2.0;
        assert!(
            destination - radius >= clock as f64 - 1.0,
            "FD-PSOLA missed its synthesis deadline"
        );
        let first = (destination - radius).ceil().max(clock as f64) as usize;
        let last = (destination + radius).ceil() as usize;
        assert!(last < clock + crate::psola_common::RING);
        for target in first..last {
            let frame = (target as f64 - destination + center)
                .round()
                .clamp(0.0, (size - 1) as f64) as usize;
            let window = self.synthesis_window[frame];
            let index = target % crate::psola_common::RING;
            output[index] += self.output[frame] * window;
            weights[index] += window * self.analysis_window[frame];
            coverage[index] = true;
        }
    }
}

impl crate::psola_synthesis::PsolaSynthesis for FdSynthesis {
    fn grain_timing(
        &self,
        left_source_span: f64,
        source_period: f64,
        right_source_span: f64,
        pitch_ratio: f64,
        previous_synthesis_period: f64,
    ) -> crate::psola_synthesis::GrainTiming {
        // Output epochs must follow the requested pitch, or overlap-add
        // locks the reconstructed spectrum back to the input fundamental.
        let period = source_period / pitch_ratio;
        let left = left_source_span.min(if previous_synthesis_period > 0.0 {
            previous_synthesis_period
        } else {
            period
        });
        let right = right_source_span.min(period);
        crate::psola_synthesis::GrainTiming {
            period,
            left,
            right,
        }
    }

    fn normalize_output(&self, sample: f64, weight: f64) -> f64 {
        // Pitch-synchronous hops can exceed a short synthesis window. Fade weakly
        // covered edges instead of amplifying FFT leakage by a vanishing weight.
        sample / weight.max(0.25)
    }

    fn has_unvoiced_transform(&self) -> bool {
        false
    }

    fn update_unvoiced(&mut self, _clock: usize, _latency: usize, _input: &[f64]) {}

    fn unvoiced_sample(&self, _index: usize) -> Option<f64> {
        None
    }

    fn clear_output_slot(&mut self, _index: usize) {}

    fn grain(
        &mut self,
        source: f64,
        destination: f64,
        left: f64,
        right: f64,
        gain: f64,
        ratio: f64,
        preserve_formants: bool,
        clock: usize,
        input: &[f64],
        output: &mut [f64],
        weights: &mut [f64],
        coverage: &mut [bool],
    ) {
        self.grain_impl(
            source,
            destination,
            left,
            right,
            gain,
            ratio,
            preserve_formants,
            clock,
            input,
            output,
            weights,
            coverage,
        );
    }
}

impl FdFrameTransform {
    pub(crate) fn new(size: usize, center: f64) -> Self {
        assert!(center.is_finite() && (0.0..size as f64).contains(&center));
        Self {
            center,
            fft: Fft::new(size),
            spectrum: vec![Complex::default(); size],
            warped: vec![Complex::default(); size],
            envelope: vec![0.0; size / 2 + 1],
            log_prefix: vec![0.0; size / 2 + 2],
        }
    }

    pub(crate) fn process_grain(
        &mut self,
        input: &[f64],
        output: &mut [f64],
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
        input: &[f64],
        output: &mut [f64],
        ratio: f64,
        preserve_formants: bool,
    ) {
        let size = self.spectrum.len();
        assert_eq!(input.len(), size);
        assert_eq!(output.len(), size);
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
        let half = size / 2;
        self.warped.fill(Complex::default());
        if preserve_formants {
            // Smooth log magnitude isolates broad vocal-tract resonances from
            // the fine harmonic source before pitch scaling.
            self.log_prefix[0] = 0.0;
            for k in 0..=half {
                let value = self.spectrum[k];
                self.log_prefix[k + 1] =
                    self.log_prefix[k] + value.re.hypot(value.im).max(1e-12).ln();
            }
            let radius = (12 * size / 2048).max(1);
            for k in 0..=half {
                let first = k.saturating_sub(radius);
                let last = (k + radius).min(half);
                self.envelope[k] = (self.log_prefix[last + 1] - self.log_prefix[first])
                    / (last - first + 1) as f64;
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
                -std::f64::consts::TAU * (k as f64 - source) * (self.center / size as f64);
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
                self.warped[size - k] = Complex {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_frames_reconstruct_about_the_configured_pitch_mark() {
        for analysis in [1024, 2048, 4096, 8192] {
            for synthesis in [256, 512, 1024, 2048] {
                if synthesis > analysis {
                    continue;
                }
                let windows = crate::pitch::SpectralWindows {
                    analysis,
                    synthesis,
                };
                let mut dsp = FdSynthesis::new(windows);
                let center = dsp.transform.center;
                let input: Vec<_> = (0..analysis)
                    .map(|i| {
                        (0.1 * (std::f64::consts::TAU * 7.0 * (i as f64 - center)
                            / analysis as f64)
                            .cos()
                            + 0.03 * (i as f64 * 0.19).sin())
                            * dsp.analysis_window[i]
                    })
                    .collect();
                let mut output = vec![0.0; analysis];
                for preserve in [false, true] {
                    dsp.transform
                        .process_grain(&input, &mut output, 1.0, preserve);
                    assert!(
                        input
                            .iter()
                            .zip(&output)
                            .all(|(a, b)| (a - b).abs() < 1e-12),
                        "{windows:?}"
                    );
                }
                for i in 0..analysis {
                    let expected = if i >= analysis - synthesis {
                        crate::dsp::window::hann(i - (analysis - synthesis), synthesis)
                    } else {
                        0.0
                    };
                    assert!(
                        (dsp.analysis_window[i] * dsp.synthesis_window[i] - expected).abs() < 1e-12
                    );
                }
            }
        }
    }
}
