//! Shared streaming pitch-mark analysis and PSOLA scheduling.
//!
//! This module owns streaming input history, pitch analysis, voiced/unvoiced
//! transitions, pitch-mark tracking, and synthesis-event scheduling. Algorithm
//! specific grain timing, windowing, resampling, and overlap-add are delegated
//! to the selected synthesis backend.
//!
//! Pitch analysis uses an in-repository FIR decimator with selectable YIN,
//! direct/FFT McLeod NSDF, or causal normalized cross-correlation tracking.
//! Pitch marks track a consistent waveform polarity near predicted epochs.
//! Reference: https://praat.org/manual/overlap-add.html
//! Pitch estimator: de Cheveigne & Kawahara, JASA 2002, doi:10.1121/1.1458024.
use std::f64::consts::TAU;
#[path = "psola_correlation.rs"]
mod correlation;
#[path = "psola_fft.rs"]
mod fft_correlation;

#[cfg(test)]
const LATENCY: usize = 5376;
const RING: usize = crate::psola_common::RING;
const HOP: usize = crate::psola_common::HOP;
const DECIMATION: usize = crate::psola_common::DECIMATION;
const TAPS: usize = crate::psola_common::TAPS;
const MIN_PERIOD: usize = crate::psola_common::MIN_PERIOD;
pub(crate) const MAX_PERIOD: usize = crate::psola_common::MAX_PERIOD;
const MAX_LAG: usize = MAX_PERIOD / DECIMATION;
const MIN_LAG: usize = MIN_PERIOD / DECIMATION;
const MARKS: usize = 128;
// Worst-case output-period support (minimum pitch ratio 0.5), plus tracking,
// frame scheduling, and fractional resampling. Round up to an analysis hop.
pub(crate) fn latency(windows: crate::pitch::SpectralWindows) -> usize {
    let left =
        (2.0 * MAX_PERIOD as f64 * crate::psola_common::flank_scales(windows).0).ceil() as usize;
    (windows.analysis / 2
        + MAX_PERIOD / 2
        + TAPS / 2
        + 2 * MAX_PERIOD
        + left.max(windows.synthesis / 2)
        + HOP
        + 66)
        .div_ceil(HOP)
        * HOP
}

#[derive(Clone, Copy, Default)]
struct Mark {
    position: f64,
    period: f64,
    segment: u64,
}

pub(crate) struct PsolaStream {
    correlation: correlation::Kernel,
    fft_correlation: Option<fft_correlation::FftCorrelation>,
    previous_nccf: Option<f64>,
    estimator: crate::pitch::PitchEstimator,
    latency: usize,
    lookahead: usize,
    input: Box<[f64; RING]>,
    output: Box<[f64; RING]>,
    weights: Box<[f64; RING]>,
    synthesis: Box<dyn crate::psola_synthesis::PsolaSynthesis>,
    preserve_formants: bool,
    coverage: Box<[bool; RING]>,
    voicing: Box<[f64; RING]>,
    filtered: Vec<f64>,
    analysis: Vec<f64>,
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
        let windows = crate::pitch::SpectralWindows::default();
        Self::with_synthesis(
            Box::new(crate::td_synthesis::TdSynthesis::new(
                formant_ratio,
                windows,
            )),
            windows,
        )
    }

    pub(crate) fn with_synthesis(
        synthesis: Box<dyn crate::psola_synthesis::PsolaSynthesis>,
        windows: crate::pitch::SpectralWindows,
    ) -> Self {
        windows.validate().expect("invalid PSOLA windows");
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
            correlation: correlation::select(),
            fft_correlation: None,
            previous_nccf: None,
            estimator: crate::pitch::PitchEstimator::default(),
            latency: latency(windows),
            lookahead: windows.analysis / 2 + MAX_PERIOD / 2 + TAPS / 2,
            input: crate::psola_common::ring(0.0),
            output: crate::psola_common::ring(0.0),
            weights: crate::psola_common::ring(0.0),
            synthesis,
            preserve_formants: true,
            coverage: crate::psola_common::ring(false),
            voicing: crate::psola_common::ring(0.0),
            filtered: vec![0.0; windows.analysis / DECIMATION + MAX_LAG + 1],
            analysis: vec![0.0; windows.analysis / DECIMATION + MAX_LAG + 1],
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
            let filtered_index = (now / DECIMATION) % self.filtered.len();
            self.filtered[filtered_index] = filtered;
        }
        let index = now % RING;
        let wet = if let Some(source) = now.checked_sub(self.latency) {
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
        if self.clock.is_multiple_of(HOP) && self.clock > self.lookahead {
            self.frame(ratio);
        }
        wet
    }

    pub(crate) fn set_pitch_estimator(&mut self, estimator: crate::pitch::PitchEstimator) {
        if estimator == crate::pitch::PitchEstimator::MpmFft && self.fft_correlation.is_none() {
            self.fft_correlation = Some(fft_correlation::FftCorrelation::new(
                self.analysis.len(),
                MAX_LAG + 1,
            ));
        }
        self.previous_nccf = None;
        self.estimator = estimator;
    }

    fn detect_period(&mut self) -> Option<f64> {
        let start = (self.clock / DECIMATION) % self.filtered.len();
        for i in 0..self.filtered.len() {
            self.analysis[i] = self.filtered[(start + i) % self.filtered.len()];
        }
        if self.estimator == crate::pitch::PitchEstimator::Nccf {
            // A DC offset must not turn unvoiced noise into high correlation.
            let mean = self.analysis.iter().sum::<f64>() / self.analysis.len() as f64;
            for sample in &mut self.analysis {
                *sample -= mean;
            }
        }
        let energy: f64 = self.analysis.iter().map(|x| x * x).sum();
        if energy / (self.filtered.len() as f64) < 1e-10 {
            self.previous_nccf = None;
            return None;
        }
        match self.estimator {
            crate::pitch::PitchEstimator::Mpm | crate::pitch::PitchEstimator::MpmFft => {
                return self.detect_mpm()
            }
            crate::pitch::PitchEstimator::Nccf => return self.detect_nccf(),
            crate::pitch::PitchEstimator::Yin => {}
        }
        (self.correlation)(&self.analysis, &mut self.difference, false);
        let mut cumulative = 0.0;
        for lag in 1..=MAX_LAG + 1 {
            let difference = self.difference[lag];
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

    // McLeod & Wyvill, "A smarter way to find pitch", ICMC 2005.
    // Use shrinking overlap so both sides of NSDF have identical support.
    // No allocations or additional lookahead in the streaming callback.
    fn detect_mpm(&mut self) -> Option<f64> {
        if self.estimator == crate::pitch::PitchEstimator::MpmFft {
            self.fft_correlation
                .as_mut()
                .expect("FFT configured before streaming")
                .mpm(&self.analysis, &mut self.difference);
        } else {
            (self.correlation)(&self.analysis, &mut self.difference, true);
        }
        // Skip the zero-lag lobe, then retain one maximum per positive lobe.
        let mut peaks = [0usize; MAX_LAG + 2];
        let mut count = 0;
        let mut lag = 1;
        while lag <= MAX_LAG && self.difference[lag] > 0.0 {
            lag += 1;
        }
        while lag <= MAX_LAG {
            while lag <= MAX_LAG && self.difference[lag] <= 0.0 {
                lag += 1;
            }
            let mut best = lag;
            while lag <= MAX_LAG && self.difference[lag] > 0.0 {
                if self.difference[lag] > self.difference[best] {
                    best = lag;
                }
                lag += 1;
            }
            if best >= MIN_LAG
                && best <= MAX_LAG
                && self.difference[best] >= self.difference[best - 1]
                && self.difference[best] > self.difference[best + 1]
            {
                peaks[count] = best;
                count += 1;
            }
        }
        let maximum = peaks[..count]
            .iter()
            .map(|&i| self.difference[i])
            .fold(0.0, f64::max);
        if maximum < 0.8 {
            return None;
        }
        let best = *peaks[..count]
            .iter()
            .find(|&&i| self.difference[i] >= 0.93 * maximum)?;
        let left = self.difference[best - 1];
        let middle = self.difference[best];
        let right = self.difference[best + 1];
        let curvature = left - 2.0 * middle + right;
        let fraction = if curvature < -1e-12 {
            (0.5 * (left - right) / curvature).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some(
            ((best as f64 + fraction) * DECIMATION as f64)
                .clamp(MIN_PERIOD as f64, MAX_PERIOD as f64),
        )
    }

    // Causal NCCF tracker: prefer high-clarity peaks and shorter fundamental
    // periods, with a weak continuity cost. The octave bias exceeds the maximum
    // history cost, so an exact octave change cannot remain locked to history.
    fn detect_nccf(&mut self) -> Option<f64> {
        correlation::nccf(&self.analysis, &mut self.difference, self.correlation);
        let is_peak = |lag: usize| {
            self.difference[lag] >= self.difference[lag - 1]
                && self.difference[lag] > self.difference[lag + 1]
        };
        let maximum = (MIN_LAG..=MAX_LAG)
            .filter(|&lag| is_peak(lag))
            .map(|lag| self.difference[lag])
            .fold(0.0, f64::max);
        if maximum < 0.8 {
            self.previous_nccf = None;
            return None;
        }
        let score = |lag: usize| {
            let continuity = self.previous_nccf.map_or(0.0, |previous| {
                ((lag * DECIMATION) as f64 / previous).log2().abs().min(1.0)
            });
            self.difference[lag] - 0.025 * (lag as f64 / MIN_LAG as f64).log2() - 0.02 * continuity
        };
        let best = (MIN_LAG..=MAX_LAG)
            .filter(|&lag| is_peak(lag) && self.difference[lag] >= 0.93 * maximum)
            .max_by(|&a, &b| score(a).total_cmp(&score(b)));
        let best = best?;
        let left = self.difference[best - 1];
        let middle = self.difference[best];
        let right = self.difference[best + 1];
        let curvature = left - 2.0 * middle + right;
        let fraction = if curvature < -1e-12 {
            (0.5 * (left - right) / curvature).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let period = ((best as f64 + fraction) * DECIMATION as f64)
            .clamp(MIN_PERIOD as f64, MAX_PERIOD as f64);
        self.previous_nccf = Some(period);
        Some(period)
    }

    fn mark(&self, index: usize) -> Mark {
        self.marks[(self.first + index) % MARKS]
    }

    fn frame(&mut self, ratio: f64) {
        let center = self.clock - self.lookahead;
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
                let end = (start + period.ceil() as usize)
                    .min(self.clock.saturating_sub(MAX_PERIOD).max(start));
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
        while self.count > 3 && self.mark(1).position + (self.latency as f64) < center as f64 {
            self.first = (self.first + 1) % MARKS;
            self.count -= 1;
        }
        // The nearest source mark can lie half a period ahead of the target;
        // its following mark must also be known before choosing the right window.
        // Do not advance the synthesis clock just because that neighbour is pending.
        let safe = center.saturating_sub(2 * MAX_PERIOD) as f64;
        self.synthesis
            .update_unvoiced(self.clock, self.latency, &self.input[..]);
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
                        self.next_synthesis + self.latency as f64,
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
    fn scalar_and_simd_streams_match_bits_through_pitch_and_voicing_changes() {
        for estimator in crate::pitch::PitchEstimator::ALL {
            for analysis in [1024, 2048, 4096, 8192] {
                let windows = crate::pitch::SpectralWindows {
                    analysis,
                    synthesis: 512,
                };
                let make = || {
                    TdPsola::with_synthesis(
                        Box::new(crate::td_synthesis::TdSynthesis::new(1.0, windows)),
                        windows,
                    )
                };
                let (mut scalar, mut vector) = (make(), make());
                scalar.correlation = correlation::scalar;
                scalar.set_pitch_estimator(estimator);
                vector.set_pitch_estimator(estimator);
                let mut seed = 17u32;
                let mut phase = 0.0;
                for i in 0..32000 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let noise = seed as f64 / u32::MAX as f64 - 0.5;
                    phase += TAU * (60.0 + 290.0 * i as f64 / 32000.0) / 48000.0;
                    let sample = match i / 4000 {
                        0 | 4 => 0.0,
                        1 | 5 => 0.1 * noise,
                        _ => 0.1 * phase.cos() + 0.04 * (3.0 * phase).cos() + 0.001 * noise,
                    };
                    let ratio = [0.5, 2.0f64.powf(3.0 / 12.0), 2.0][i / 11000];
                    assert_eq!(
                        scalar.push(sample, ratio).to_bits(),
                        vector.push(sample, ratio).to_bits(),
                        "{estimator:?} window={analysis} sample={i}"
                    );
                    assert_eq!(scalar.period.to_bits(), vector.period.to_bits());
                }
            }
        }
    }

    #[test]
    fn reused_energy_preserves_mpm_voicing_periods_and_audio() {
        for analysis in [1024, 2048, 4096, 8192] {
            for rate in crate::SAMPLE_RATES {
                let windows = crate::pitch::SpectralWindows {
                    analysis,
                    synthesis: 512,
                };
                let make = || {
                    TdPsola::with_synthesis(
                        Box::new(crate::td_synthesis::TdSynthesis::new(1.0, windows)),
                        windows,
                    )
                };
                let (mut reference, mut reused) = (make(), make());
                for dsp in [&mut reference, &mut reused] {
                    dsp.set_pitch_estimator(crate::pitch::PitchEstimator::Mpm);
                }
                reference.correlation = correlation::select_reference();
                let mut seed = 17u32;
                let mut phase = 0.0;
                let mut maximum_audio_error = 0.0f64;
                for i in 0..40000 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let noise = seed as f64 / u32::MAX as f64 - 0.5;
                    phase += TAU * (60.0 + 290.0 * i as f64 / 40000.0) / rate as f64;
                    let sample = match i / 5000 {
                        0 | 4 => 0.0,
                        1 | 5 => 0.1 * noise,
                        _ => {
                            0.001 * phase.cos()
                                + 0.08 * (2.0 * phase).cos()
                                + 0.06 * (3.0 * phase).cos()
                                + 0.0001 * noise
                        }
                    };
                    let ratio = [0.5, 2.0f64.powf(3.0 / 12.0), 2.0][i / 14000];
                    let a = reference.push(sample, ratio);
                    let b = reused.push(sample, ratio);
                    maximum_audio_error = maximum_audio_error.max((a - b).abs());
                    if i % HOP == HOP - 1 {
                        assert_eq!(
                            reference.next_analysis.is_some(),
                            reused.next_analysis.is_some(),
                            "rate={rate} window={analysis} sample={i}"
                        );
                        let cents = 1200.0 * (reference.period / reused.period).log2();
                        assert!(
                            cents.abs() < 1e-8,
                            "rate={rate} window={analysis} sample={i} cents={cents}"
                        );
                        assert_eq!(reference.segment, reused.segment);
                    }
                }
                assert!(
                    maximum_audio_error < 1e-9,
                    "rate={rate} window={analysis} audio error={maximum_audio_error:e}"
                );
                eprintln!("MPM energy reuse rate={rate} window={analysis} maximum audio error={maximum_audio_error:e}");
            }
        }
    }

    #[test]
    fn fft_mpm_preserves_voicing_and_periods_across_windows_and_transitions() {
        for analysis in [1024, 2048, 4096, 8192] {
            let windows = crate::pitch::SpectralWindows {
                analysis,
                synthesis: 512,
            };
            let make = || {
                TdPsola::with_synthesis(
                    Box::new(crate::td_synthesis::TdSynthesis::new(1.0, windows)),
                    windows,
                )
            };
            let (mut direct, mut fft) = (make(), make());
            direct.set_pitch_estimator(crate::pitch::PitchEstimator::Mpm);
            fft.set_pitch_estimator(crate::pitch::PitchEstimator::MpmFft);
            assert_eq!(direct.latency, fft.latency);
            let mut phase = 0.0;
            let mut seed = 17u32;
            for i in 0..40000 {
                phase += TAU * (70.0 + 250.0 * i as f64 / 40000.0) / 48000.0;
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let sample = if i / 5000 % 3 == 0 {
                    0.0
                } else if i / 5000 % 3 == 1 {
                    0.1 * (seed as f64 / u32::MAX as f64 - 0.5)
                } else {
                    0.1 * phase.cos() + 0.04 * (3.0 * phase).cos()
                };
                let ratio = [0.5, 2.0f64.powf(3.0 / 12.0), 2.0][i / 14000];
                let a = direct.push(sample, ratio);
                let b = fft.push(sample, ratio);
                assert!(
                    (a - b).abs() < 1e-8,
                    "window={analysis} sample={i} audio delta={}",
                    a - b
                );
                assert_eq!(direct.next_analysis.is_some(), fft.next_analysis.is_some());
                assert!((1200.0 * (direct.period / fft.period).log2()).abs() < 1e-7);
            }
        }
    }

    #[test]
    fn nccf_does_not_treat_dc_offset_or_offset_noise_as_voiced() {
        let mut dsp = TdPsola::new(1.0);
        dsp.set_pitch_estimator(crate::pitch::PitchEstimator::Nccf);
        dsp.filtered.fill(0.3);
        assert!(dsp.detect_period().is_none());
        let mut seed = 17u32;
        for sample in &mut dsp.filtered {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *sample = 0.3 + 0.01 * (seed as f64 / u32::MAX as f64 - 0.5);
        }
        assert!(dsp.detect_period().is_none());
        for (i, sample) in dsp.filtered.iter_mut().enumerate() {
            *sample = 0.3 + 0.1 * (TAU * 113.0 * (i * DECIMATION) as f64 / 48000.0).cos();
        }
        let period = dsp.detect_period().unwrap();
        assert!((period - 48000.0 / 113.0).abs() < 1.0);
    }

    #[test]
    fn nccf_tracker_follows_octave_changes_and_resets_on_silence() {
        for analysis in [1024, 2048, 4096, 8192] {
            let windows = crate::pitch::SpectralWindows {
                analysis,
                synthesis: 512,
            };
            let mut dsp = TdPsola::with_synthesis(
                Box::new(crate::td_synthesis::TdSynthesis::new(1.0, windows)),
                windows,
            );
            dsp.set_pitch_estimator(crate::pitch::PitchEstimator::Nccf);
            for frequency in [113.0, 226.0, 113.0, 80.0, 160.0] {
                for (i, sample) in dsp.filtered.iter_mut().enumerate() {
                    let phase = TAU * frequency * (i * DECIMATION) as f64 / 48000.0;
                    *sample = 0.1 * phase.cos() + 0.04 * (3.0 * phase).cos();
                }
                let period = dsp.detect_period().unwrap();
                let cents = 1200.0 * (period * frequency / 48000.0).log2();
                assert!(
                    cents.abs() < 10.0,
                    "window={analysis} frequency={frequency} cents={cents}"
                );
                assert!(dsp.previous_nccf.is_some());
            }
            dsp.filtered.fill(0.0);
            assert!(dsp.detect_period().is_none());
            assert!(dsp.previous_nccf.is_none());
            dsp.set_pitch_estimator(crate::pitch::PitchEstimator::MpmFft);
            assert!(dsp.fft_correlation.is_some());
            dsp.set_pitch_estimator(crate::pitch::PitchEstimator::Nccf);
            assert!(dsp.previous_nccf.is_none());
        }
    }

    // Manual performance comparison; no timing threshold in the test suite.
    #[test]
    #[ignore]
    fn benchmark_scalar_and_simd_pitch_analysis() {
        benchmark_pitch_kernels(false, false);
    }

    #[test]
    #[ignore]
    fn benchmark_mpm_energy_reuse() {
        benchmark_pitch_kernels(true, false);
    }

    #[test]
    #[ignore]
    fn benchmark_all_estimators() {
        benchmark_pitch_kernels(false, true);
    }

    fn benchmark_pitch_kernels(reference: bool, all: bool) {
        use std::hint::black_box;
        let baseline_label = if all {
            "MPM"
        } else if reference {
            "previous"
        } else {
            "scalar"
        };
        let optimized_label = if all {
            "candidate"
        } else if reference {
            "reused"
        } else {
            "SIMD"
        };
        let baseline_kernel = if all {
            correlation::select()
        } else if reference {
            correlation::select_reference()
        } else {
            correlation::scalar
        };
        fn median(mut samples: Vec<f64>) -> f64 {
            samples.sort_by(f64::total_cmp);
            samples[samples.len() / 2]
        }
        fn clock() -> std::time::Duration {
            #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
            {
                #[repr(C)]
                struct Timespec {
                    seconds: std::ffi::c_long,
                    nanoseconds: std::ffi::c_long,
                }
                unsafe extern "C" {
                    fn clock_gettime(clock: i32, value: *mut Timespec) -> i32;
                }
                let mut value = Timespec {
                    seconds: 0,
                    nanoseconds: 0,
                };
                // SAFETY: Linux CLOCK_THREAD_CPUTIME_ID=3, writable timespec.
                assert_eq!(unsafe { clock_gettime(3, &mut value) }, 0);
                std::time::Duration::new(value.seconds as u64, value.nanoseconds as u32)
            }
            #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
            {
                static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                START.get_or_init(std::time::Instant::now).elapsed()
            }
        }
        for estimator in crate::pitch::PitchEstimator::ALL {
            if !all
                && !matches!(
                    estimator,
                    crate::pitch::PitchEstimator::Yin | crate::pitch::PitchEstimator::Mpm
                )
            {
                continue;
            }
            if reference && estimator != crate::pitch::PitchEstimator::Mpm {
                continue;
            }
            for analysis in [1024, 2048, 4096, 8192] {
                let windows = crate::pitch::SpectralWindows {
                    analysis,
                    synthesis: 512,
                };
                let mut dsp = TdPsola::with_synthesis(
                    Box::new(crate::td_synthesis::TdSynthesis::new(1.0, windows)),
                    windows,
                );
                dsp.set_pitch_estimator(estimator);
                for i in 0..dsp.filtered.len() {
                    let phase = TAU * 113.0 * (i * DECIMATION) as f64 / 48000.0;
                    dsp.filtered[i] = 0.1 * phase.cos() + 0.04 * (3.0 * phase).cos();
                }
                let mut times = [Vec::new(), Vec::new()];
                for round in 0..8 {
                    for index in [round % 2, 1 - round % 2] {
                        dsp.correlation = if index == 0 {
                            baseline_kernel
                        } else {
                            correlation::select()
                        };
                        if all {
                            dsp.set_pitch_estimator(if index == 0 {
                                crate::pitch::PitchEstimator::Mpm
                            } else {
                                estimator
                            });
                        }
                        let start = clock();
                        for _ in 0..500 {
                            black_box(dsp.detect_period());
                        }
                        if round > 0 {
                            times[index].push((clock() - start).as_secs_f64() * 1e6 / 500.0);
                        }
                    }
                }
                let scalar = median(times[0].clone());
                let simd = median(times[1].clone());
                eprintln!("analysis {estimator:?} window={analysis}: {baseline_label}={scalar:.2}us {optimized_label}={simd:.2}us speedup={:.2}x", scalar / simd);
            }
            let input: Vec<_> = (0..24000)
                .map(|i| {
                    let phase = TAU * 113.0 * i as f64 / 48000.0;
                    0.1 * phase.cos() + 0.04 * (3.0 * phase).cos()
                })
                .collect();
            for semitones in [-12.0, 3.0, 12.0] {
                let mut times = [Vec::new(), Vec::new()];
                for round in 0..8 {
                    for index in [round % 2, 1 - round % 2] {
                        let mut dsp = TdPsola::new(1.0);
                        dsp.set_pitch_estimator(estimator);
                        dsp.correlation = if index == 0 {
                            baseline_kernel
                        } else {
                            correlation::select()
                        };
                        let ratio = 2.0f64.powf(semitones / 12.0);
                        if all {
                            dsp.set_pitch_estimator(if index == 0 {
                                crate::pitch::PitchEstimator::Mpm
                            } else {
                                estimator
                            });
                        }
                        let start = clock();
                        for &sample in &input {
                            black_box(dsp.push(black_box(sample), ratio));
                        }
                        if round > 0 {
                            times[index].push((clock() - start).as_secs_f64() * 1e3);
                        }
                    }
                }
                let scalar = median(times[0].clone());
                let simd = median(times[1].clone());
                eprintln!("stream {estimator:?} {semitones:+}st: {baseline_label}={scalar:.2}ms {optimized_label}={simd:.2}ms / 0.5s audio speedup={:.2}x", scalar / simd);
            }
        }
    }

    #[test]
    fn estimators_cover_low_voice_missing_fundamental_and_noise() {
        for estimator in crate::pitch::PitchEstimator::ALL {
            for frequency in [60.0, 80.0, 113.0, 203.0, 350.0] {
                for fundamental in [0.1, 0.0] {
                    let mut dsp = TdPsola::new(1.0);
                    dsp.set_pitch_estimator(estimator);
                    // Directly exercise the same filtered analysis history used by streaming.
                    for i in 0..dsp.filtered.len() {
                        let phase = TAU * frequency * (i * DECIMATION) as f64 / 48000.0;
                        dsp.filtered[i] = fundamental * phase.cos()
                            + 0.08 * (2.0 * phase).cos()
                            + 0.06 * (3.0 * phase).cos();
                    }
                    let period = dsp.detect_period().unwrap();
                    let cents = 1200.0 * (period * frequency / 48000.0).log2();
                    assert!(
                        cents.abs() < 10.0,
                        "{estimator:?} {frequency}Hz fundamental={fundamental}: {cents} cents"
                    );
                }
            }
            let mut dsp = TdPsola::new(1.0);
            dsp.set_pitch_estimator(estimator);
            assert!(dsp.detect_period().is_none());
            let mut seed = 17u32;
            for sample in &mut dsp.filtered {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *sample = 0.1 * (seed as f64 / u32::MAX as f64 - 0.5);
            }
            assert!(dsp.detect_period().is_none(), "{estimator:?}");
        }
    }

    #[test]
    fn estimator_streaming_quality_and_cost() {
        for estimator in crate::pitch::PitchEstimator::ALL {
            for semitones in [-12.0, 3.0, 12.0] {
                let mut dsp = TdPsola::new(1.0);
                dsp.set_pitch_estimator(estimator);
                let start = std::time::Instant::now();
                for i in 0..24000 {
                    let phase = TAU * 113.0 * i as f64 / 48000.0;
                    let output = dsp.push(
                        0.1 * phase.cos() + 0.04 * (3.0 * phase).cos(),
                        2.0f64.powf(semitones / 12.0),
                    );
                    assert!(output.is_finite());
                }
                assert!((dsp.detect_period().unwrap() - 48000.0 / 113.0).abs() < 1.0);
                eprintln!(
                    "{estimator:?} {semitones:+}st: {:?} / 0.5s audio",
                    start.elapsed()
                );
            }
        }
    }

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
        for estimator in crate::pitch::PitchEstimator::ALL {
            let mut dsp = TdPsola::new(1.0);
            dsp.set_pitch_estimator(estimator);
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
}
