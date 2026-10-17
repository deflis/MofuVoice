//! Shared streaming pitch-mark analysis and PSOLA scheduling.
//!
//! This module owns streaming input history, pitch analysis, voiced/unvoiced
//! transitions, pitch-mark tracking, and synthesis-event scheduling. Algorithm
//! specific grain timing, windowing, resampling, and overlap-add are delegated
//! to the selected synthesis backend.
//!
//! Pitch analysis uses an in-repository FIR decimator and YIN difference function.
//! Pitch marks track a consistent waveform polarity near predicted epochs.
//! Reference: https://praat.org/manual/overlap-add.html
//! Pitch estimator: de Cheveigne & Kawahara, JASA 2002, doi:10.1121/1.1458024.
use std::f64::consts::TAU;

pub const LATENCY: usize = crate::psola_common::LATENCY;
const RING: usize = crate::psola_common::RING;
const ANALYSIS: usize = crate::psola_common::ANALYSIS;
const HOP: usize = crate::psola_common::HOP;
const DECIMATION: usize = crate::psola_common::DECIMATION;
const TAPS: usize = crate::psola_common::TAPS;
pub(crate) const LOOKAHEAD: usize = crate::psola_common::LOOKAHEAD;
const MIN_PERIOD: usize = crate::psola_common::MIN_PERIOD;
pub(crate) const MAX_PERIOD: usize = crate::psola_common::MAX_PERIOD;
const FILTERED: usize = ANALYSIS / DECIMATION;
const MAX_LAG: usize = MAX_PERIOD / DECIMATION;
const MIN_LAG: usize = MIN_PERIOD / DECIMATION;
const MARKS: usize = 128;
// Analysis, both neighbouring epochs, the largest tracked grain support, and
// one analysis hop must all fit before any output sample is consumed.
const _: () = assert!(LATENCY >= LOOKAHEAD + 2 * MAX_PERIOD + 6 * MAX_PERIOD / 5 + HOP + 2);

#[derive(Clone, Copy, Default)]
struct Mark {
    position: f64,
    period: f64,
    segment: u64,
}

pub(crate) struct PsolaStream {
    input: Box<[f64; RING]>,
    output: Box<[f64; RING]>,
    weights: Box<[f64; RING]>,
    synthesis: Box<dyn crate::psola_synthesis::PsolaSynthesis>,
    preserve_formants: bool,
    coverage: Box<[bool; RING]>,
    voicing: Box<[f64; RING]>,
    filtered: Box<[f64; FILTERED]>,
    analysis: Box<[f64; FILTERED]>,
    difference: [f64; MAX_LAG + 2],
    taps: [f64; TAPS],
    marks: [Mark; MARKS],
    first: usize,
    count: usize,
    clock: usize,
    analyzed: usize,
    next_analysis: Option<f64>,
    next_synthesis: f64,
    previous_synthesis_period: f64,
    period: f64,
    polarity: f64,
    segment: u64,
    voiced_mix: f64,
}

impl PsolaStream {
    #[cfg(test)]
    fn new(formant_ratio: f64) -> Self {
        Self::with_synthesis(Box::new(crate::td_synthesis::TdSynthesis::new(
            formant_ratio,
        )))
    }

    pub(crate) fn with_synthesis(
        synthesis: Box<dyn crate::psola_synthesis::PsolaSynthesis>,
    ) -> Self {
        Self::new_inner(synthesis)
    }

    fn new_inner(synthesis: Box<dyn crate::psola_synthesis::PsolaSynthesis>) -> Self {
        let mut taps = std::array::from_fn(|i| {
            let distance = i as f64 - (TAPS / 2) as f64;
            let sinc = if distance == 0.0 {
                0.16
            } else {
                (TAU * 0.08 * distance).sin() / (std::f64::consts::PI * distance)
            };
            sinc * (0.5 - 0.5 * (TAU * i as f64 / (TAPS - 1) as f64).cos())
        });
        let sum: f64 = taps.iter().sum();
        for tap in &mut taps {
            *tap /= sum;
        }
        Self {
            input: Box::new([0.0; RING]),
            output: Box::new([0.0; RING]),
            weights: Box::new([0.0; RING]),
            synthesis,
            preserve_formants: true,
            coverage: Box::new([false; RING]),
            voicing: Box::new([0.0; RING]),
            filtered: Box::new([0.0; FILTERED]),
            analysis: Box::new([0.0; FILTERED]),
            difference: [0.0; MAX_LAG + 2],
            taps,
            marks: [Mark::default(); MARKS],
            first: 0,
            count: 0,
            clock: 0,
            analyzed: 0,
            next_analysis: None,
            next_synthesis: 0.0,
            previous_synthesis_period: 0.0,
            period: 400.0,
            polarity: 1.0,
            segment: 0,
            voiced_mix: 0.0,
        }
    }

    #[cfg(test)]
    pub fn push(&mut self, sample: f64, ratio: f64) -> f64 {
        self.push_with_formants(sample, ratio, true)
    }

    pub fn push_with_formants(&mut self, sample: f64, ratio: f64, preserve_formants: bool) -> f64 {
        self.preserve_formants = preserve_formants;
        let now = self.clock;
        self.input[now % RING] = sample;
        if now.is_multiple_of(DECIMATION) {
            let mut filtered = 0.0;
            for (i, &tap) in self.taps.iter().enumerate() {
                if now >= i {
                    filtered += tap * self.input[(now - i) % RING];
                }
            }
            self.filtered[(now / DECIMATION) % FILTERED] = filtered;
        }
        let index = now % RING;
        let wet = if let Some(source) = now.checked_sub(LATENCY) {
            let dry = self
                .synthesis
                .unvoiced_sample(index)
                .unwrap_or(self.input[source % RING]);
            let mix = if self.coverage[index] || self.synthesis.has_unvoiced_transform() {
                self.voicing[source % RING]
            } else {
                0.0
            };
            let wet = self
                .synthesis
                .normalize_output(self.output[index], self.weights[index]);
            dry * (1.0 - mix) + wet * mix
        } else {
            0.0
        };
        self.output[index] = 0.0;
        self.weights[index] = 0.0;
        self.synthesis.clear_output_slot(index);
        self.coverage[index] = false;
        self.clock += 1;
        if self.clock.is_multiple_of(HOP) && self.clock > LOOKAHEAD {
            self.frame(ratio);
        }
        wet
    }

    fn detect_period(&mut self) -> Option<f64> {
        let start = (self.clock / DECIMATION) % FILTERED;
        for i in 0..FILTERED {
            self.analysis[i] = self.filtered[(start + i) % FILTERED];
        }
        let length = FILTERED - MAX_LAG - 1;
        let energy: f64 = self.analysis.iter().map(|x| x * x).sum();
        if energy / (FILTERED as f64) < 1e-10 {
            return None;
        }
        let mut cumulative = 0.0;
        for lag in 1..=MAX_LAG + 1 {
            let mut difference = 0.0;
            for i in 0..length {
                let delta = self.analysis[i] - self.analysis[i + lag];
                difference += delta * delta;
            }
            cumulative += difference;
            self.difference[lag] = if cumulative > 1e-20 {
                difference * lag as f64 / cumulative
            } else {
                1.0
            };
        }
        let mut best = MIN_LAG;
        for lag in MIN_LAG..=MAX_LAG {
            if self.difference[lag] < self.difference[best] {
                best = lag;
            }
            if self.difference[lag] < 0.15
                && self.difference[lag] <= self.difference[lag - 1]
                && self.difference[lag] < self.difference[lag + 1]
            {
                best = lag;
                break;
            }
        }
        if self.difference[best] > 0.2 {
            return None;
        }
        let left = self.difference[best - 1];
        let middle = self.difference[best];
        let right = self.difference[best + 1];
        let curvature = left - 2.0 * middle + right;
        let fraction = if curvature > 1e-12 {
            (0.5 * (left - right) / curvature).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some(
            ((best as f64 + fraction) * DECIMATION as f64)
                .clamp(MIN_PERIOD as f64, MAX_PERIOD as f64),
        )
    }

    fn mark(&self, index: usize) -> Mark {
        self.marks[(self.first + index) % MARKS]
    }

    fn frame(&mut self, ratio: f64) {
        let center = self.clock - LOOKAHEAD;
        let detected = self.detect_period();
        for source in self.analyzed..center {
            let target = f64::from(detected.is_some());
            self.voiced_mix += (target - self.voiced_mix).clamp(-1.0 / 192.0, 1.0 / 192.0);
            self.voicing[source % RING] = self.voiced_mix;
        }
        if let Some(period) = detected {
            if self.next_analysis.is_none() {
                self.segment += 1;
                self.period = period;
                let start = self.analyzed;
                let end = (start + period.ceil() as usize).min(self.clock - MAX_PERIOD);
                let peak = (start..=end)
                    .max_by(|&a, &b| {
                        self.input[a % RING]
                            .abs()
                            .total_cmp(&self.input[b % RING].abs())
                    })
                    .unwrap();
                self.polarity = if self.input[peak % RING] < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                self.next_analysis = Some(peak as f64);
            } else {
                self.period += 0.35 * (period - self.period);
            }
            while let Some(predicted) = self.next_analysis.filter(|p| *p <= center as f64) {
                let radius = (self.period * 0.2) as usize;
                let minimum = if self.count == 0 {
                    0
                } else {
                    self.mark(self.count - 1).position.ceil() as usize
                        + (self.period * 0.5) as usize
                };
                let first = (predicted.max(radius as f64) as usize - radius).max(minimum);
                let last = (predicted as usize + radius).min(self.clock - 2);
                if first > last {
                    break;
                }
                let peak = (first..=last)
                    .max_by(|&a, &b| {
                        (self.polarity * self.input[a % RING])
                            .total_cmp(&(self.polarity * self.input[b % RING]))
                    })
                    .unwrap();
                let left = if peak > 0 {
                    self.input[(peak - 1) % RING]
                } else {
                    0.0
                };
                let middle = self.input[peak % RING];
                let right = self.input[(peak + 1) % RING];
                let curvature = left - 2.0 * middle + right;
                let fraction = if curvature.abs() > 1e-12 {
                    (0.5 * (left - right) / curvature).clamp(-0.5, 0.5)
                } else {
                    0.0
                };
                let position = peak as f64 + fraction;
                if self.count == MARKS {
                    self.first = (self.first + 1) % MARKS;
                    self.count -= 1;
                }
                self.marks[(self.first + self.count) % MARKS] = Mark {
                    position,
                    period: self.period,
                    segment: self.segment,
                };
                self.count += 1;
                self.next_analysis = Some(position + self.period);
            }
        } else {
            self.next_analysis = None;
        }
        self.analyzed = center;
        while self.count > 3 && self.mark(1).position + (LATENCY as f64) < center as f64 {
            self.first = (self.first + 1) % MARKS;
            self.count -= 1;
        }
        // The nearest source mark can lie half a period ahead of the target;
        // its following mark must also be known before choosing the right window.
        // Do not advance the synthesis clock just because that neighbour is pending.
        let safe = center.saturating_sub(2 * MAX_PERIOD) as f64;
        self.synthesis
            .update_unvoiced(self.clock, LATENCY, &self.input[..]);
        while self.next_synthesis <= safe {
            let nearest = (0..self.count).min_by(|&a, &b| {
                (self.mark(a).position - self.next_synthesis)
                    .abs()
                    .total_cmp(&(self.mark(b).position - self.next_synthesis).abs())
            });
            if let Some(index) = nearest.filter(|&i| i > 0 && i + 1 < self.count) {
                let mark = self.mark(index);
                let previous = self.mark(index - 1);
                let next = self.mark(index + 1);
                if previous.segment == mark.segment
                    && next.segment == mark.segment
                    && (mark.position - self.next_synthesis).abs() < mark.period * 0.75
                {
                    let timing = self.synthesis.grain_timing(
                        mark.position - previous.position,
                        mark.period,
                        next.position - mark.position,
                        ratio,
                        self.previous_synthesis_period,
                    );
                    self.grain(
                        mark.position,
                        self.next_synthesis + LATENCY as f64,
                        timing.left,
                        timing.right,
                        timing.period / ((timing.left + timing.right) * 0.5),
                        ratio,
                    );
                    self.next_synthesis += timing.period;
                    self.previous_synthesis_period = timing.period;
                    continue;
                }
            }
            self.next_synthesis += HOP as f64;
            self.previous_synthesis_period = 0.0;
        }
    }

    fn grain(
        &mut self,
        source: f64,
        destination: f64,
        left: f64,
        right: f64,
        gain: f64,
        ratio: f64,
    ) {
        let clock = self.clock;
        self.synthesis.grain(
            source,
            destination,
            left,
            right,
            gain,
            ratio,
            self.preserve_formants,
            clock,
            &self.input[..],
            &mut self.output[..],
            &mut self.weights[..],
            &mut self.coverage[..],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::PsolaStream as TdPsola;
    use super::*;

    #[test]
    fn low_voice_f32_period_does_not_drift() {
        let mut dsp = TdPsola::new(1.0);
        let ratio = 2.0_f64.powf(3.0 / 12.0);
        let mut output = Vec::new();
        let mut missing = 0;
        for i in 0..48000 {
            let phase = TAU * 80.0 * i as f64 / 48000.0;
            let sample = (0.1 * phase.cos() + 0.04 * (3.0 * phase).cos()) as f32;
            if i > 16384 && !dsp.coverage[i % RING] {
                missing += 1;
            }
            output.push(dsp.push(sample as f64, ratio));
            if i > 16384 && i % 4096 == 4095 {
                assert!((dsp.previous_synthesis_period - 600.0 / ratio).abs() < 0.1);
            }
        }
        let power = |frequency: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &value) in output[16384..].iter().enumerate() {
                let angle = TAU * frequency * i as f64 / 48000.0;
                let window = 0.5 - 0.5 * (TAU * i as f64 / (48000 - 16384) as f64).cos();
                re += value * window * angle.cos();
                im += value * window * angle.sin();
            }
            re * re + im * im
        };
        let expected = 80.0 * ratio;
        let peak = (-12..=12)
            .map(|i| expected + i as f64 * 0.25)
            .max_by(|&a, &b| power(a).total_cmp(&power(b)))
            .unwrap();
        assert_eq!(missing, 0);
        assert!((peak - expected).abs() <= 0.25);
    }

    #[test]
    fn yin_tracks_voice_periods_and_marks_the_actual_waveform_peaks() {
        for rate in crate::SAMPLE_RATES {
            for frequency in [60.0, 80.0, 113.0, 203.0, 350.0] {
                let mut dsp = TdPsola::new(1.0);
                for i in 0..16384 {
                    let phase = TAU * frequency * i as f64 / rate as f64;
                    dsp.push(0.1 * phase.cos() + 0.04 * (3.0 * phase).cos(), 1.5);
                }
                let expected = rate as f64 / frequency;
                let estimated = dsp.detect_period().unwrap();
                assert!(
                    (estimated - expected).abs() < 1.0,
                    "rate={rate}, f={frequency}: period={estimated}, expected={expected}"
                );
                let last = dsp.mark(dsp.count - 1);
                let previous = dsp.mark(dsp.count - 2);
                assert!(((last.position - previous.position) - expected).abs() < 0.02);
                assert!((TAU * frequency * last.position / rate as f64).cos().abs() > 0.999);
            }
        }
    }

    #[test]
    fn unvoiced_noise_is_copied_at_the_declared_sample_delay() {
        let mut dsp = TdPsola::new(1.0);
        let mut seed = 17_u32;
        let input: Vec<_> = (0..RING * 3)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                0.1 * (seed as f64 / u32::MAX as f64 - 0.5)
            })
            .collect();
        for (i, &sample) in input.iter().enumerate() {
            let output = dsp.push(sample, 2.0);
            assert_eq!(
                output,
                if i < LATENCY { 0.0 } else { input[i - LATENCY] },
                "sample {i}"
            );
        }
        assert!(dsp.detect_period().is_none());
    }
}
