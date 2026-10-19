//! Correlation-based phase synchronization for direct spectral pitch mapping.
//!
//! Unlike the original time-stretching PVSOLA, the reference is pitch-mapped:
//! inserting an unshifted input frame would restore the wrong pitch. We correlate
//! the dominant sinusoidal components of an input-phase reference and the running
//! synthesis, searching a common time displacement before restoring coherence.
//! The score is their energy-weighted sinusoidal-model cross-correlation.
//! Corrections are bounded and overlap-added by the existing synthesis windows.
//! No time-stretch/resampling stage, sample-clock drift, or lookahead is added.
//! Inspired by Moinet & Dutoit, DAFx-2011, "PVSOLA: A Phase Vocoder with
//! Synchronized Overlap-Add", https://www.dafx.de/paper-archive/2011/Papers/57_e.pdf
use super::HOP;
use crate::dsp::phase::wrap;

const PEAKS: usize = 32;
const STEP: usize = 2;
const INTERVAL: usize = 8;
const MAX_CORRECTION: f64 = 0.2;

#[derive(Clone, Copy, Default)]
struct Component {
    bin: usize,
    weight: f64,
    difference: f64,
    frequency: f64,
}

pub(super) struct Frame<'a> {
    pub peaks: &'a [usize],
    pub magnitude: &'a [f64],
    pub phase: &'a [f64],
    pub advance: &'a [f64],
    pub synthesis: &'a mut [f64],
    pub ratio: f64,
    pub centre_step: f64,
}

pub(super) struct Synchronizer {
    scores: Vec<f64>,
    radius: usize,
    components: [Component; PEAKS],
    until_reset: usize,
}

impl Synchronizer {
    pub fn new(window: usize) -> Self {
        Self {
            scores: vec![0.0; window / STEP + 1],
            radius: window / 2,
            components: [Component::default(); PEAKS],
            until_reset: INTERVAL,
        }
    }

    pub fn synchronize(&mut self, frame: Frame<'_>) {
        self.until_reset -= 1;
        if self.until_reset != 0 {
            return;
        }
        self.until_reset = INTERVAL;
        let Frame {
            peaks,
            magnitude,
            phase,
            advance,
            synthesis,
            ratio,
            centre_step,
        } = frame;
        let maximum = peaks.iter().map(|&p| magnitude[p]).fold(0.0_f64, f64::max);
        if maximum < 1e-8 {
            return;
        }
        // Reject diffuse spectra and single tones. Weak side lobes must not make
        // a pure tone look like a voice with several independently drifting peaks.
        let mut count = 0;
        for &bin in peaks {
            let frequency = advance[bin] * ratio / HOP as f64;
            if magnitude[bin] < maximum * 0.1 || !(0.0..std::f64::consts::PI).contains(&frequency) {
                continue;
            }
            let component = Component {
                bin,
                weight: (magnitude[bin] / maximum).powi(2),
                difference: wrap(phase[bin] + centre_step * bin as f64 - synthesis[bin]),
                frequency,
            };
            if count < PEAKS {
                self.components[count] = component;
                count += 1;
            } else {
                let weakest = (0..PEAKS)
                    .min_by(|&a, &b| {
                        self.components[a]
                            .weight
                            .total_cmp(&self.components[b].weight)
                    })
                    .unwrap();
                if component.weight > self.components[weakest].weight {
                    self.components[weakest] = component;
                }
            }
        }
        let energy: f64 = self.components[..count].iter().map(|p| p.weight).sum();
        let total: f64 = magnitude.iter().map(|m| (m / maximum).powi(2)).sum();
        if count < 3 || energy < 0.2 * total {
            return;
        }
        let components = &self.components[..count];
        self.scores.fill(0.0);
        for component in components {
            let (mut im, mut re) =
                (component.difference - component.frequency * self.radius as f64).sin_cos();
            let (step_im, step_re) = (component.frequency * STEP as f64).sin_cos();
            for score in self.scores.iter_mut() {
                *score += component.weight * re;
                (re, im) = (re * step_re - im * step_im, im * step_re + re * step_im);
            }
        }
        let middle = self.radius / STEP;
        let mut best = middle;
        for i in 0..self.scores.len() {
            // Prefer the smaller displacement when periodic candidates tie.
            if self.scores[i] > self.scores[best] + 1e-10 * energy
                || ((self.scores[i] - self.scores[best]).abs() <= 1e-10 * energy
                    && i.abs_diff(middle) < best.abs_diff(middle))
            {
                best = i;
            }
        }
        if self.scores[best] < 0.35 * energy {
            return;
        }
        let mut offset = (best as f64 - middle as f64) * STEP as f64;
        // Refine to a fractional sample. Integer-only synchronization modulates
        // high harmonics as the selected displacement crosses a sample boundary.
        if best > 0 && best + 1 < self.scores.len() {
            let left = self.scores[best - 1];
            let right = self.scores[best + 1];
            let curvature = left - 2.0 * self.scores[best] + right;
            if curvature < -1e-12 * energy {
                offset += (0.5 * (left - right) / curvature).clamp(-0.5, 0.5) * STEP as f64;
            }
        }
        for component in components {
            let correction = wrap(component.difference + component.frequency * offset);
            synthesis[component.bin] =
                wrap(synthesis[component.bin] + correction.clamp(-MAX_CORRECTION, MAX_CORRECTION));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synchronization_reduces_relative_phase_error_without_following_a_time_shift() {
        let peaks = [1, 2, 3, 4, 5, 6];
        let magnitude = [0.0, 1.0, 0.9, 0.8, 0.7, 0.6, 0.5];
        let phase = [0.0; 7];
        let advance = std::array::from_fn::<_, 7, _>(|i| i as f64 * 0.023 * HOP as f64);
        let mut synthesis = std::array::from_fn::<_, 7, _>(|i| {
            i as f64 * 0.023 * 13.37 + if i % 2 == 0 { 0.45 } else { -0.45 }
        });
        let original = synthesis;
        let mut sync = Synchronizer::new(crate::pitch::WINDOW);
        for _ in 0..INTERVAL * 16 {
            sync.synchronize(Frame {
                peaks: &peaks,
                magnitude: &magnitude,
                phase: &phase,
                advance: &advance,
                synthesis: &mut synthesis,
                ratio: 1.0,
                centre_step: 0.0,
            });
        }
        // A common time shift has zero second phase difference. Independent
        // harmonic offsets do not, even when absolute phase is arbitrary.
        let error = |p: &[f64]| {
            (2..6)
                .map(|i| wrap(p[i - 1] - 2.0 * p[i] + p[i + 1]).powi(2))
                .sum::<f64>()
        };
        assert!(error(&synthesis) < error(&original) * 0.01);
        assert!(
            synthesis[1].abs() > 0.1,
            "alignment must retain a common time displacement"
        );
    }

    #[test]
    fn silent_and_diffuse_frames_do_not_reset_phase() {
        for magnitude in [[0.0; 16], [1.0; 16]] {
            let original = [0.7; 16];
            let mut synthesis = original;
            let mut sync = Synchronizer::new(crate::pitch::WINDOW);
            for _ in 0..INTERVAL * 2 {
                sync.synchronize(Frame {
                    peaks: &[1, 4, 8],
                    magnitude: &magnitude,
                    phase: &[0.0; 16],
                    advance: &[0.1; 16],
                    synthesis: &mut synthesis,
                    ratio: 1.5,
                    centre_step: 0.0,
                });
            }
            assert_eq!(synthesis, original);
        }
    }
}
