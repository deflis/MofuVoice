//! Streaming PVSOLA time stretching followed by pitch-rate resampling.
//!
//! The phase vocoder advances by a fixed synthesis hop while its analysis hop
//! follows the requested time scale. At regular intervals a raw input frame is
//! aligned against the completed vocoder output by cross-correlation and
//! overlap-added, resetting the vocoder phases as described by Moinet & Dutoit.
//! All storage is allocated at construction; the audio callback does no I/O.
use crate::dsp::{complex::Complex, fft::Fft, phase, window::hann};
use std::f64::consts::{PI, TAU};

const FRAME: usize = 1024;
const HOP: usize = FRAME / 4;
const RESET_EVERY: usize = 4;
const TOLERANCE: usize = 2 * HOP;
const RESAMPLE_RADIUS: usize = 32;
const RESAMPLE_TAPS: usize = 2 * RESAMPLE_RADIUS + 1;
const RESAMPLE_PHASES: usize = 256;
/// Quantize anti-alias cutoffs so smooth pitch ramps rebuild kernels sparsely.
const CUTOFF_STEPS: f64 = 512.0;
const CORRELATION_SIZE: usize = (2 * FRAME + 2 * TOLERANCE).next_power_of_two();
const RING: usize = 16384;
const CEPSTRUM_CUTOFF: usize = 48;
/// Covers the analysis frame, PVSOLA correlation lookahead at the slowest
/// supported ratio, and final resampler support.
pub(crate) const LATENCY: usize = 4608;
const _: () = assert!(LATENCY >= FRAME + (FRAME + TOLERANCE) * 2 + RESAMPLE_RADIUS);

pub(crate) struct PvsOla {
    fft: Fft,
    correlation_fft: Fft,
    vector_phase: bool,
    correlation_output: Box<[Complex; CORRELATION_SIZE]>,
    correlation_input: Box<[Complex; CORRELATION_SIZE]>,
    input: Box<[f64; RING]>,
    output: Box<[f64; RING]>,
    weight: Box<[f64; RING]>,
    window: Box<[f64; FRAME]>,
    frame: Box<[Complex; FRAME]>,
    previous_phase: Box<[f64; FRAME]>,
    synthesis_phase: Box<[f64; FRAME]>,
    magnitude: Box<[f64; FRAME]>,
    current_phase: Box<[f64; FRAME]>,
    envelope: Box<[f64; FRAME]>,
    /// Polyphase windowed-sinc kernels for pitch-rate reads. Built once per
    /// quantized cutoff so the callback never recomputes sin/cos per sample.
    kernels: Box<[f64]>,
    kernel_cutoff: f64,
    preserve_formants: bool,
    formant_ratio: f64,
    polarity: f64,
    previous_valid: bool,
    previous_analysis_position: f64,
    input_clock: usize,
    analysis_position: f64,
    synthesis_position: usize,
    frames_since_reset: usize,
    synthesis_offset: f64,
    reset_target: Option<usize>,
    reset_input_position: f64,
    reset_nominal_position: f64,
    ratio: f64,
    read_position: f64,
    delayed_samples: usize,
    clear_until: usize,
}

impl PvsOla {
    pub fn new(ratio: f64, preserve_formants: bool, formant_ratio: f64) -> Self {
        assert!(ratio.is_finite() && (0.5..=2.0).contains(&ratio));
        let window = Box::new(std::array::from_fn(|i| hann(i, FRAME)));
        let mut kernels = vec![0.0; (RESAMPLE_PHASES + 1) * RESAMPLE_TAPS].into_boxed_slice();
        let kernel_cutoff = quantized_cutoff(ratio);
        fill_resample_kernels(&mut kernels, kernel_cutoff);
        Self {
            fft: Fft::new(FRAME),
            vector_phase: {
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
            correlation_fft: Fft::new(CORRELATION_SIZE),
            correlation_output: Box::new([Complex::default(); CORRELATION_SIZE]),
            correlation_input: Box::new([Complex::default(); CORRELATION_SIZE]),
            input: Box::new([0.0; RING]),
            output: Box::new([0.0; RING]),
            weight: Box::new([0.0; RING]),
            window,
            frame: Box::new([Complex::default(); FRAME]),
            previous_phase: Box::new([0.0; FRAME]),
            synthesis_phase: Box::new([0.0; FRAME]),
            magnitude: Box::new([0.0; FRAME]),
            current_phase: Box::new([0.0; FRAME]),
            envelope: Box::new([0.0; FRAME]),
            kernels,
            kernel_cutoff,
            preserve_formants,
            formant_ratio,
            polarity: 1.0,
            previous_valid: false,
            previous_analysis_position: 0.0,
            input_clock: 0,
            analysis_position: 0.0,
            synthesis_position: 0,
            frames_since_reset: 0,
            synthesis_offset: 0.0,
            reset_target: None,
            reset_input_position: 0.0,
            reset_nominal_position: 0.0,
            ratio,
            read_position: 0.0,
            delayed_samples: 0,
            clear_until: 0,
        }
    }

    /// Discard queued audio when returning to the exact neutral bypass.
    /// Reuse the allocated buffers so reactivation never allocates in a callback.
    pub fn reset(&mut self) {
        if self.input_clock == 0 {
            return;
        }
        self.input.fill(0.0);
        self.output.fill(0.0);
        self.weight.fill(0.0);
        self.polarity = 1.0;
        self.previous_valid = false;
        self.previous_analysis_position = 0.0;
        self.input_clock = 0;
        self.analysis_position = 0.0;
        self.synthesis_position = 0;
        self.frames_since_reset = 0;
        self.synthesis_offset = 0.0;
        self.reset_target = None;
        self.reset_input_position = 0.0;
        self.reset_nominal_position = 0.0;
        self.read_position = 0.0;
        self.delayed_samples = 0;
        self.clear_until = 0;
    }

    pub fn ready(&self) -> bool {
        self.delayed_samples >= LATENCY
    }

    pub fn push(
        &mut self,
        sample: f64,
        ratio: f64,
        preserve_formants: bool,
        formant_ratio: f64,
    ) -> f64 {
        assert!(ratio.is_finite() && (0.5..=2.0).contains(&ratio));
        assert!(formant_ratio.is_finite() && (0.5..=2.0).contains(&formant_ratio));
        self.ratio = ratio;
        self.preserve_formants = preserve_formants;
        self.formant_ratio = formant_ratio;
        self.refresh_kernels(ratio);
        self.input[self.input_clock % RING] = sample;
        self.input_clock += 1;
        self.generate_available_frames();

        if self.delayed_samples < LATENCY {
            self.delayed_samples += 1;
            return 0.0;
        }
        let position = self.read_position;
        self.read_position += self.ratio;
        let output = self.resample(position);
        let safe_until = (position.floor() as usize).saturating_sub(RESAMPLE_RADIUS);
        while self.clear_until < safe_until {
            let index = self.clear_until % RING;
            self.output[index] = 0.0;
            self.weight[index] = 0.0;
            self.clear_until += 1;
        }
        output
    }

    fn refresh_kernels(&mut self, ratio: f64) {
        let cutoff = quantized_cutoff(ratio);
        if cutoff == self.kernel_cutoff {
            return;
        }
        fill_resample_kernels(&mut self.kernels, cutoff);
        self.kernel_cutoff = cutoff;
    }

    fn generate_available_frames(&mut self) {
        loop {
            if self.analysis_position.ceil() as usize + FRAME > self.input_clock {
                return;
            }
            self.vocode_frame(self.synthesis_position);
            self.analysis_position += HOP as f64 / self.ratio;
            self.synthesis_position += HOP;
            if self.reset_target.is_none() {
                self.frames_since_reset += 1;
                if self.frames_since_reset == RESET_EVERY {
                    // Search around the unshifted timeline, not the last
                    // correlation displacement. Otherwise offsets accumulate
                    // until synthesis overwrites unread ring-buffer samples.
                    self.reset_nominal_position =
                        self.synthesis_position as f64 - self.synthesis_offset;
                    self.reset_target = Some(self.reset_nominal_position.round().max(0.0) as usize);
                    self.reset_input_position = self.analysis_position;
                }
            } else if self.synthesis_position > self.reset_target.unwrap() + FRAME + TOLERANCE {
                self.reset_from_input_frame();
            }
        }
    }

    // Both regular frames and correlation resets use the same spectral analysis.
    // Dispatch is detected once at construction; buffers are reused in callbacks.
    fn analyze_spectrum(&mut self) {
        phase::angles(
            &self.frame[..],
            &mut self.current_phase[..],
            self.vector_phase,
        );
        for (value, magnitude) in self.frame.iter().zip(self.magnitude.iter_mut()) {
            *magnitude = value.re.hypot(value.im);
        }
    }

    fn vocode_frame(&mut self, output_start: usize) {
        for i in 0..FRAME {
            let position = self.analysis_position + i as f64;
            let base = position.floor() as usize;
            let fraction = position - base as f64;
            let a = self.input[base % RING];
            let b = self.input[(base + 1) % RING];
            self.frame[i] = Complex {
                re: (a + (b - a) * fraction) * self.polarity * self.window[i],
                im: 0.0,
            };
        }
        self.fft.forward(&mut self.frame[..]);
        // The previous frame scheduled this position before the live ratio
        // changed. Estimate frequency from the actual distance between frames.
        let alpha = if self.previous_valid {
            self.analysis_position - self.previous_analysis_position
        } else {
            HOP as f64 / self.ratio
        };
        let stretch = HOP as f64 / alpha;
        self.analyze_spectrum();
        if self.preserve_formants && (self.ratio != 1.0 || self.formant_ratio != 1.0) {
            for bin in 0..FRAME {
                self.frame[bin] = Complex {
                    re: self.magnitude[bin].max(1e-12).ln(),
                    im: 0.0,
                };
            }
            self.fft.inverse(&mut self.frame[..]);
            self.frame[CEPSTRUM_CUTOFF + 1..FRAME - CEPSTRUM_CUTOFF].fill(Complex::default());
            self.fft.forward(&mut self.frame[..]);
            for bin in 0..FRAME {
                self.envelope[bin] = self.frame[bin].re;
            }
        } else {
            self.envelope.fill(0.0);
        }
        // Advance only positive-frequency phases. Using FRAME-k for the
        // negative bin adds whole turns at fractional analysis hops and breaks
        // conjugate symmetry, discarding signal energy in the inverse FFT.
        for bin in 0..=FRAME / 2 {
            let phase = self.current_phase[bin];
            let omega = std::f64::consts::TAU * bin as f64 / FRAME as f64;
            if self.previous_valid {
                let delta = wrap(phase - self.previous_phase[bin] - omega * alpha);
                self.synthesis_phase[bin] += omega * HOP as f64 + delta * stretch;
            } else {
                self.synthesis_phase[bin] = phase;
            }
            self.previous_phase[bin] = phase;
            let raw_bin = if bin <= FRAME / 2 { bin } else { FRAME - bin };
            let target_bin =
                (raw_bin as f64 * self.ratio / self.formant_ratio).clamp(0.0, FRAME as f64 / 2.0);
            let lower = target_bin.floor() as usize;
            let upper = (lower + 1).min(FRAME / 2);
            let fraction = target_bin - lower as f64;
            let target_envelope =
                self.envelope[lower] * (1.0 - fraction) + self.envelope[upper] * fraction;
            let formant_gain = if self.preserve_formants {
                (target_envelope - self.envelope[raw_bin]).exp()
            } else {
                1.0
            };
            self.frame[bin] = Complex::polar(
                self.magnitude[bin] * formant_gain,
                self.synthesis_phase[bin],
            );
        }
        self.frame[0].im = 0.0;
        self.frame[FRAME / 2].im = 0.0;
        for bin in 1..FRAME / 2 {
            self.frame[FRAME - bin] = Complex {
                re: self.frame[bin].re,
                im: -self.frame[bin].im,
            };
        }
        self.previous_analysis_position = self.analysis_position;
        self.previous_valid = true;
        self.fft.inverse(&mut self.frame[..]);
        for i in 0..FRAME {
            let index = (output_start + i) % RING;
            let window = self.window[i];
            self.output[index] += self.frame[i].re * window;
            self.weight[index] += window * window;
        }
    }

    /// Linear cross-correlation via zero-padded FFTs. This computes the same
    /// windowed, even-sample dot products as the exhaustive offset search, but
    /// normalizes each output sample once and bounds work to three 4096 FFTs.
    fn correlate_reset_frame(&mut self, target: usize, input_start: usize) {
        self.correlation_output.fill(Complex::default());
        self.correlation_input.fill(Complex::default());
        let first = target as isize - TOLERANCE as isize;
        for i in 0..FRAME + 2 * TOLERANCE {
            let absolute = first + i as isize;
            if absolute >= 0 {
                let index = absolute as usize % RING;
                if self.weight[index] > 1e-12 {
                    self.correlation_output[i].re = self.output[index] / self.weight[index];
                }
            }
        }
        for i in (0..FRAME).step_by(2) {
            self.correlation_input[i].re = self.input[(input_start + i) % RING]
                * self.polarity
                * self.window[i]
                * self.window[i];
        }
        self.correlation_fft
            .forward(&mut self.correlation_output[..]);
        self.correlation_fft
            .forward(&mut self.correlation_input[..]);
        for i in 0..CORRELATION_SIZE {
            let input = self.correlation_input[i];
            self.correlation_output[i] = self.correlation_output[i].mul(Complex {
                re: input.re,
                im: -input.im,
            });
        }
        self.correlation_fft
            .inverse(&mut self.correlation_output[..]);
    }

    fn reset_from_input_frame(&mut self) {
        let target = self.reset_target.take().unwrap();
        let input_start = self.reset_input_position.round() as usize;
        let mut best_offset = 0_isize;
        let mut best_score = 0.0_f64;
        let mut found = false;
        // Samples through the previous fractional read's kernel support have
        // already contributed to emitted audio and cannot be replaced.
        let minimum_start = if self.read_position > 0.0 {
            (self.read_position + RESAMPLE_RADIUS as f64).ceil() as usize
        } else {
            0
        };
        self.correlate_reset_frame(target, input_start);
        for offset in -(TOLERANCE as isize)..=TOLERANCE as isize {
            if target as isize + offset < minimum_start as isize {
                continue;
            }
            let score = self.correlation_output[(offset + TOLERANCE as isize) as usize].re;
            let score_magnitude = score.abs();
            let best_magnitude = best_score.abs();
            let tie = 1e-3 * best_magnitude.max(score_magnitude);
            if !found
                || score_magnitude > best_magnitude + tie
                || ((score_magnitude - best_magnitude).abs() <= tie
                    && offset.abs() < best_offset.abs())
            {
                best_score = score;
                best_offset = offset;
                found = true;
            }
        }
        if !found {
            // A rapidly changing ratio can put the whole search behind the
            // reader. Keep the vocoder output and try a later reset instead.
            self.frames_since_reset = 0;
            return;
        }
        let start = (target as isize + best_offset).max(0) as usize;
        let sign = if best_score < 0.0 { -1.0 } else { 1.0 };
        // synthesis_position is the next frame start, not the end of the
        // audio already overlap-added by the previous temporary frame.
        let temporary_end = self.synthesis_position + FRAME - HOP;
        for absolute in start + FRAME - HOP..temporary_end {
            let index = absolute % RING;
            self.output[index] = 0.0;
            self.weight[index] = 0.0;
        }
        self.polarity *= sign;
        for i in 0..FRAME {
            self.frame[i] = Complex {
                re: self.input[(input_start + i) % RING] * self.polarity * self.window[i],
                im: 0.0,
            };
        }
        self.fft.forward(&mut self.frame[..]);
        self.analyze_spectrum();
        if self.preserve_formants && (self.ratio != 1.0 || self.formant_ratio != 1.0) {
            for bin in 0..FRAME {
                self.frame[bin] = Complex {
                    re: self.magnitude[bin].max(1e-12).ln(),
                    im: 0.0,
                };
            }
            self.fft.inverse(&mut self.frame[..]);
            self.frame[CEPSTRUM_CUTOFF + 1..FRAME - CEPSTRUM_CUTOFF].fill(Complex::default());
            self.fft.forward(&mut self.frame[..]);
            for bin in 0..FRAME {
                self.envelope[bin] = self.frame[bin].re;
            }
        } else {
            self.envelope.fill(0.0);
        }
        for bin in 0..FRAME {
            let raw_bin = if bin <= FRAME / 2 { bin } else { FRAME - bin };
            let target_bin =
                (raw_bin as f64 * self.ratio / self.formant_ratio).clamp(0.0, FRAME as f64 / 2.0);
            let lower = target_bin.floor() as usize;
            let upper = (lower + 1).min(FRAME / 2);
            let fraction = target_bin - lower as f64;
            let target = self.envelope[lower] * (1.0 - fraction) + self.envelope[upper] * fraction;
            let gain = if self.preserve_formants {
                (target - self.envelope[raw_bin]).exp()
            } else {
                1.0
            };
            let phase = self.current_phase[bin];
            self.previous_phase[bin] = phase;
            self.synthesis_phase[bin] = phase;
            self.frame[bin] = Complex::polar(self.magnitude[bin] * gain, phase);
        }
        self.fft.inverse(&mut self.frame[..]);
        for i in 0..FRAME {
            let index = (start + i) % RING;
            let window = self.window[i] * self.window[i];
            let input = self.frame[i].re * self.window[i];
            // Blend the phase reset into the already completed overlap. The
            // corresponding weight blend keeps the output level normalized.
            self.output[index] = self.output[index] * (1.0 - window) + input;
            self.weight[index] = self.weight[index] * (1.0 - window) + window;
        }
        self.previous_analysis_position = input_start as f64;
        self.analysis_position = input_start as f64 + HOP as f64 / self.ratio;
        self.synthesis_position = start + HOP;
        self.frames_since_reset = 0;
        // Keep the displacement from the nominal timeline, including the
        // fractional analysis position rounded to the inserted input frame.
        // Omitting that fraction would introduce a small drift on every reset.
        let rounding = input_start as f64 - self.reset_input_position;
        self.synthesis_offset =
            start as f64 - (self.reset_nominal_position + rounding * self.ratio);

        self.previous_valid = true;
    }

    fn resample(&self, position: f64) -> f64 {
        let center = position.floor() as isize;
        let phase = (position - center as f64) * RESAMPLE_PHASES as f64;
        let index = phase.floor() as usize;
        let fraction = phase - index as f64;
        let lower = &self.kernels[index * RESAMPLE_TAPS..(index + 1) * RESAMPLE_TAPS];
        let upper = &self.kernels[(index + 1) * RESAMPLE_TAPS..(index + 2) * RESAMPLE_TAPS];
        let first = center - RESAMPLE_RADIUS as isize;
        let mut value = 0.0;
        let mut weight = 0.0;
        for tap in 0..RESAMPLE_TAPS {
            let sample_position = first + tap as isize;
            if sample_position < 0 {
                continue;
            }
            let ring = sample_position as usize % RING;
            let frame_weight = self.weight[ring];
            if frame_weight > 1e-12 {
                let kernel = lower[tap] + fraction * (upper[tap] - lower[tap]);
                value += kernel * self.output[ring] / frame_weight;
                weight += kernel;
            }
        }
        if weight.abs() > 1e-12 {
            value / weight
        } else {
            0.0
        }
    }
}

fn quantized_cutoff(ratio: f64) -> f64 {
    ((0.45 / ratio.max(1.0)) * CUTOFF_STEPS).round() / CUTOFF_STEPS
}

fn fill_resample_kernels(kernels: &mut [f64], cutoff: f64) {
    for phase in 0..=RESAMPLE_PHASES {
        let fraction = phase as f64 / RESAMPLE_PHASES as f64;
        let kernel = &mut kernels[phase * RESAMPLE_TAPS..(phase + 1) * RESAMPLE_TAPS];
        for (i, tap) in kernel.iter_mut().enumerate() {
            let distance = i as f64 - RESAMPLE_RADIUS as f64 - fraction;
            let normalized = distance / (RESAMPLE_RADIUS as f64 + 1.0);
            let window = 0.42 + 0.5 * (PI * normalized).cos() + 0.08 * (TAU * normalized).cos();
            let sinc = if distance.abs() < 1e-12 {
                2.0 * cutoff
            } else {
                (TAU * cutoff * distance).sin() / (PI * distance)
            };
            *tap = window * sinc;
        }
    }
}

fn wrap(phase: f64) -> f64 {
    (phase + PI).rem_euclid(TAU) - PI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_analysis_matches_scalar_for_regular_frames_and_resets() {
        for ratio in [0.5, 0.83, 1.3, 2.0] {
            for preserve in [false, true] {
                let mut scalar = PvsOla::new(ratio, preserve, 1.1);
                scalar.vector_phase = false;
                let mut vector = PvsOla::new(ratio, preserve, 1.1);
                for i in 0..24000 {
                    let input = 0.1 * (i as f64 * 0.071).sin() + 0.03 * (i as f64 * 0.213).cos();
                    let expected = scalar.push(input, ratio, preserve, 1.1);
                    let actual = vector.push(input, ratio, preserve, 1.1);
                    assert!(
                        (actual - expected).abs() < 1e-10,
                        "ratio={ratio}, preserve={preserve}, sample={i}: {actual} != {expected}"
                    );
                }
                assert!(vector.previous_valid);
            }
        }
    }

    #[test]
    fn fractional_analysis_hops_keep_the_vocoder_frame_real() {
        for ratio in [0.5, 0.83, 1.3, 1.5, 2.0] {
            let mut dsp = PvsOla::new(ratio, false, 1.0);
            for i in 0..RING {
                dsp.input[i] = 0.1 * (i as f64 * 0.071).sin();
            }
            dsp.vocode_frame(0);
            dsp.analysis_position += HOP as f64 / ratio;
            dsp.vocode_frame(HOP);
            let imaginary = dsp.frame.iter().map(|x| x.im * x.im).sum::<f64>();
            let real = dsp.frame.iter().map(|x| x.re * x.re).sum::<f64>();
            assert!(
                imaginary < 1e-20 * real,
                "ratio={ratio}, imaginary/real={}",
                imaginary / real
            );
        }
    }
    #[test]
    fn sustained_audio_keeps_unread_synthesis_inside_the_ring() {
        for ratio in [0.5, 0.83, 1.3, 2.0] {
            let mut dsp = PvsOla::new(ratio, false, 1.0);
            for i in 0..480000 {
                let phase = std::f64::consts::TAU * 113.0 * i as f64 / 48000.0;
                dsp.push(
                    0.1 * phase.sin() + 0.03 * (3.0 * phase).cos(),
                    ratio,
                    false,
                    1.0,
                );
                if i > LATENCY * 2 {
                    let timeline_error =
                        dsp.synthesis_position as f64 - dsp.analysis_position * ratio;
                    assert!(
                        timeline_error.abs() <= TOLERANCE as f64 + 2.0,
                        "timeline drift: ratio={ratio}, sample={i}, error={timeline_error}"
                    );
                    let read = dsp.read_position as usize;
                    assert!(
                        dsp.synthesis_position + FRAME < read + RING - RESAMPLE_RADIUS,
                        "ratio={ratio}, sample={i}, synthesis={}, read={read}",
                        dsp.synthesis_position
                    );
                    assert!(
                        dsp.synthesis_position > read + RESAMPLE_RADIUS,
                        "output underrun: ratio={ratio}, sample={i}, synthesis={}, read={read}",
                        dsp.synthesis_position
                    );
                }
            }
        }
    }
    #[test]
    fn fft_reset_scores_match_exhaustive_correlation_at_every_offset() {
        let mut dsp = PvsOla::new(1.3, false, 1.0);
        let mut seed = 11_u32;
        for i in 0..RING {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            dsp.input[i] = seed as f64 / u32::MAX as f64 - 0.5;
            dsp.output[i] = (i as f64 * 0.127).sin();
            dsp.weight[i] = if i % 17 == 0 {
                0.0
            } else {
                0.5 + (i % 9) as f64
            };
        }
        for target in [256, 2048, RING + 256] {
            let input_start = target + 701;
            dsp.correlate_reset_frame(target, input_start);
            for offset in -(TOLERANCE as isize)..=TOLERANCE as isize {
                let start = target as isize + offset;
                if start < 0 {
                    continue;
                }
                let mut expected = 0.0;
                for i in (0..FRAME).step_by(2) {
                    let index = (start as usize + i) % RING;
                    if dsp.weight[index] > 1e-12 {
                        expected += dsp.output[index] / dsp.weight[index]
                            * dsp.input[(input_start + i) % RING]
                            * dsp.polarity
                            * dsp.window[i]
                            * dsp.window[i];
                    }
                }
                let actual = dsp.correlation_output[(offset + TOLERANCE as isize) as usize].re;
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "target={target}, offset={offset}, expected={expected}, actual={actual}"
                );
            }
        }
    }
    #[test]
    fn reset_discards_the_entire_temporary_tail_including_ring_wrap() {
        for target in [2048, RING - 512] {
            let mut dsp = PvsOla::new(1.0, false, 1.0);
            dsp.output.fill(0.25);
            dsp.weight.fill(1.0);
            dsp.reset_target = Some(target);
            dsp.reset_nominal_position = target as f64;
            dsp.reset_input_position = target as f64;
            dsp.synthesis_position = target + FRAME + TOLERANCE + HOP;
            let end = dsp.synthesis_position + FRAME - HOP;
            dsp.reset_from_input_frame();
            for i in target + FRAME..end {
                assert_eq!(dsp.output[i % RING], 0.0, "stale output at {i}");
                assert_eq!(dsp.weight[i % RING], 0.0, "stale weight at {i}");
            }
            assert_eq!(dsp.output[end % RING], 0.25);
        }
    }

    #[test]
    fn phase_propagation_keeps_a_stationary_tone_stable_when_ratio_changes() {
        let bin = 17.0;
        let omega = std::f64::consts::TAU * bin / FRAME as f64;
        for (old_ratio, new_ratio) in [(1.3, 2.0), (2.0, 0.5), (0.83, 1.7)] {
            let mut dsp = PvsOla::new(old_ratio, false, 1.0);
            for i in 0..RING {
                dsp.input[i] = 0.1 * (omega * i as f64).cos();
            }
            dsp.vocode_frame(0);
            let phase = dsp.synthesis_phase[bin as usize];
            dsp.analysis_position = HOP as f64 / old_ratio;
            dsp.ratio = new_ratio;
            dsp.vocode_frame(HOP);
            let error = wrap(dsp.synthesis_phase[bin as usize] - phase - omega * HOP as f64);
            assert!(
                error.abs() < 0.002,
                "old={old_ratio}, new={new_ratio}, phase error={error}"
            );
        }
    }

    #[test]
    fn reset_never_rewrites_samples_already_used_by_the_resampler() {
        let mut dsp = PvsOla::new(0.5, false, 1.0);
        dsp.output.fill(0.25);
        dsp.weight.fill(1.0);
        dsp.reset_target = Some(1024);
        dsp.reset_nominal_position = 1024.0;
        dsp.reset_input_position = 2048.0;
        dsp.synthesis_position = 2816;
        dsp.read_position = 1100.0;
        dsp.reset_from_input_frame();
        assert!(dsp.synthesis_position - HOP >= 1100 + RESAMPLE_RADIUS);
        for i in 0..1100 + RESAMPLE_RADIUS {
            assert_eq!(dsp.output[i], 0.25, "read audio rewritten at {i}");
            assert_eq!(dsp.weight[i], 1.0);
        }
    }

    #[test]
    fn polyphase_resampler_matches_direct_windowed_sinc() {
        for ratio in [0.5, 0.83, 1.0, 1.3, 2.0] {
            let mut dsp = PvsOla::new(ratio, false, 1.0);
            for i in 0..RING {
                dsp.output[i] = (i as f64 * 0.137).sin() * 0.4;
                dsp.weight[i] = if i % 23 == 0 { 0.0 } else { 0.7 + (i % 5) as f64 };
            }
            for position in [0.0, 12.31, 128.9, 2047.37, 4096.01, 8000.63] {
                let actual = dsp.resample(position);
                let center = position.floor() as isize;
                let fraction = position - center as f64;
                let cutoff = 0.45 / ratio.max(1.0);
                let mut expected = 0.0;
                let mut weight = 0.0;
                for offset in -(RESAMPLE_RADIUS as isize)..=RESAMPLE_RADIUS as isize {
                    let sample_position = center + offset;
                    if sample_position < 0 {
                        continue;
                    }
                    let distance = offset as f64 - fraction;
                    let normalized = distance / (RESAMPLE_RADIUS as f64 + 1.0);
                    let window =
                        0.42 + 0.5 * (PI * normalized).cos() + 0.08 * (TAU * normalized).cos();
                    let kernel = if distance.abs() < 1e-12 {
                        2.0 * cutoff
                    } else {
                        (TAU * cutoff * distance).sin() / (PI * distance)
                    } * window;
                    let index = sample_position as usize % RING;
                    let frame_weight = dsp.weight[index];
                    if frame_weight > 1e-12 {
                        expected += kernel * dsp.output[index] / frame_weight;
                        weight += kernel;
                    }
                }
                let expected = if weight.abs() > 1e-12 {
                    expected / weight
                } else {
                    0.0
                };
                assert!(
                    (actual - expected).abs() < 2e-4,
                    "ratio={ratio}, position={position}, actual={actual}, expected={expected}"
                );
            }
        }
    }

    #[test]
    fn kernel_rebuild_is_skipped_for_tiny_ratio_changes() {
        let mut dsp = PvsOla::new(1.3, false, 1.0);
        let cutoff = dsp.kernel_cutoff;
        dsp.refresh_kernels(1.3 + 1.0 / CUTOFF_STEPS / 4.0);
        assert_eq!(dsp.kernel_cutoff, cutoff);
        dsp.refresh_kernels(1.6);
        assert_ne!(dsp.kernel_cutoff, cutoff);
    }
}
