//! Streaming PVSOLA time stretching followed by pitch-rate resampling.
//!
//! The phase vocoder advances by a fixed synthesis hop while its analysis hop
//! follows the requested time scale. At regular intervals a raw input frame is
//! aligned against the completed vocoder output by cross-correlation and
//! overlap-added, resetting the vocoder phases as described by Moinet & Dutoit.
//! All storage is allocated at construction; the audio callback does no I/O.
use crate::dsp::{complex::Complex, fft::Fft, window::hann};

const FRAME: usize = 1024;
const HOP: usize = FRAME / 4;
const RESET_EVERY: usize = 4;
const TOLERANCE: usize = 2 * HOP;
const RESAMPLE_RADIUS: usize = 32;
const RING: usize = 16384;
const CEPSTRUM_CUTOFF: usize = 48;
/// Covers the analysis frame, PVSOLA correlation lookahead at the slowest
/// supported ratio, and final resampler support.
pub(crate) const LATENCY: usize = 4608;
const _: () = assert!(LATENCY >= FRAME + (FRAME + TOLERANCE) * 2 + RESAMPLE_RADIUS);

pub(crate) struct PvsOla {
    fft: Fft,
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
    preserve_formants: bool,
    formant_ratio: f64,
    polarity: f64,
    previous_valid: bool,
    input_clock: usize,
    analysis_position: f64,
    synthesis_position: usize,
    frames_since_reset: usize,
    reset_interval: usize,
    accumulated_offset: isize,
    reset_target: Option<usize>,
    reset_input_position: f64,
    ratio: f64,
    read_position: f64,
    delayed_samples: usize,
    clear_until: usize,
}

impl PvsOla {
    pub fn new(ratio: f64, preserve_formants: bool, formant_ratio: f64) -> Self {
        assert!(ratio.is_finite() && (0.5..=2.0).contains(&ratio));
        let window = Box::new(std::array::from_fn(|i| hann(i, FRAME)));
        Self {
            fft: Fft::new(FRAME),
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
            preserve_formants,
            formant_ratio,
            polarity: 1.0,
            previous_valid: false,
            input_clock: 0,
            analysis_position: 0.0,
            synthesis_position: 0,
            frames_since_reset: 0,
            reset_interval: RESET_EVERY,
            accumulated_offset: 0,
            reset_target: None,
            reset_input_position: 0.0,
            ratio,
            read_position: 0.0,
            delayed_samples: 0,
            clear_until: 0,
        }
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
                if self.frames_since_reset == self.reset_interval {
                    self.reset_target = Some(self.synthesis_position);
                    self.reset_input_position = self.analysis_position;
                }
            } else if self.synthesis_position > self.reset_target.unwrap() + FRAME + TOLERANCE {
                self.reset_from_input_frame();
            }
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
        let alpha = HOP as f64 / self.ratio;
        for bin in 0..FRAME {
            let value = self.frame[bin];
            let magnitude = value.re.hypot(value.im);
            let phase = value.im.atan2(value.re);
            self.current_phase[bin] = phase;
            self.magnitude[bin] = magnitude;
        }
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
            let phase = self.current_phase[bin];
            let omega = std::f64::consts::TAU * bin as f64 / FRAME as f64;
            if self.previous_valid {
                let delta = wrap(phase - self.previous_phase[bin] - omega * alpha);
                self.synthesis_phase[bin] += omega * HOP as f64 + delta * self.ratio;
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
        self.previous_valid = true;
        self.fft.inverse(&mut self.frame[..]);
        for i in 0..FRAME {
            let index = (output_start + i) % RING;
            let window = self.window[i];
            self.output[index] += self.frame[i].re * window;
            self.weight[index] += window * window;
        }
    }

    fn reset_from_input_frame(&mut self) {
        let target = self.reset_target.take().unwrap();
        let input_start = self.reset_input_position.round() as usize;
        let mut best_offset = 0_isize;
        let mut best_score = 0.0_f64;
        for offset in -(TOLERANCE as isize)..=TOLERANCE as isize {
            let start = target as isize + offset;
            if start < 0 {
                continue;
            }
            let mut score = 0.0;
            for i in (0..FRAME).step_by(2) {
                let out_index = (start as usize + i) % RING;
                let input_index = (input_start + i) % RING;
                let output = if self.weight[out_index] > 1e-12 {
                    self.output[out_index] / self.weight[out_index]
                } else {
                    0.0
                };
                score += output
                    * self.input[input_index]
                    * self.polarity
                    * self.window[i]
                    * self.window[i];
            }
            let score_magnitude = score.abs();
            let best_magnitude = best_score.abs();
            let tie = 1e-3 * best_magnitude.max(score_magnitude);
            if score_magnitude > best_magnitude + tie
                || ((score_magnitude - best_magnitude).abs() <= tie
                    && offset.abs() < best_offset.abs())
            {
                best_score = score;
                best_offset = offset;
            }
        }
        let start = (target as isize + best_offset).max(0) as usize;
        let sign = if best_score < 0.0 { -1.0 } else { 1.0 };
        let temporary_end = self.synthesis_position;
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
        for bin in 0..FRAME {
            let value = self.frame[bin];
            self.magnitude[bin] = value.re.hypot(value.im);
            self.current_phase[bin] = value.im.atan2(value.re);
        }
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
        self.analysis_position = input_start as f64 + HOP as f64 / self.ratio;
        self.synthesis_position = start + HOP;
        self.frames_since_reset = 0;
        self.accumulated_offset += best_offset;
        self.reset_interval = RESET_EVERY;
        if self.accumulated_offset > HOP as isize {
            self.reset_interval -= 1;
            self.accumulated_offset -= HOP as isize;
        } else if self.accumulated_offset < -(HOP as isize) {
            self.reset_interval += 1;
            self.accumulated_offset += HOP as isize;
        }

        self.previous_valid = true;
    }

    fn resample(&self, position: f64) -> f64 {
        let center = position.floor() as isize;
        let fraction = position - center as f64;
        let mut value = 0.0;
        let mut weight = 0.0;
        for offset in -(RESAMPLE_RADIUS as isize)..=RESAMPLE_RADIUS as isize {
            let sample_position = center + offset;
            if sample_position < 0 {
                continue;
            }
            let distance = offset as f64 - fraction;
            let normalized = distance / (RESAMPLE_RADIUS as f64 + 1.0);
            let window = 0.42
                + 0.5 * (std::f64::consts::PI * normalized).cos()
                + 0.08 * (std::f64::consts::TAU * normalized).cos();
            let cutoff = 0.45 / self.ratio.max(1.0);
            let kernel = if distance.abs() < 1e-12 {
                2.0 * cutoff
            } else {
                (std::f64::consts::TAU * cutoff * distance).sin()
                    / (std::f64::consts::PI * distance)
            } * window;
            let index = sample_position as usize % RING;
            let frame_weight = self.weight[index];
            if frame_weight > 1e-12 {
                value += kernel * self.output[index] / frame_weight;
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

fn wrap(phase: f64) -> f64 {
    (phase + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}
