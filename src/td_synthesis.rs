//! Time-domain PSOLA grain synthesis and its formant-ratio resampling path.

#[path = "td_resampler.rs"]
mod resampler;

use crate::{psola_common, psola_synthesis::PsolaSynthesis};

const UNVOICED_RADIUS: usize = 256;
const _: () = assert!(
    psola_common::LATENCY
        >= psola_common::LOOKAHEAD
            + 2 * psola_common::MAX_PERIOD
            + 6 * psola_common::MAX_PERIOD / 5
            + psola_common::HOP
            + resampler::RADIUS
            + 2
);

pub(crate) struct TdSynthesis {
    formant_ratio: f64,
    resampler: Option<resampler::Resampler>,
    unvoiced: Option<Box<[f64; psola_common::RING]>>,
    unvoiced_weight: Option<Box<[f64; psola_common::RING]>>,
    next_unvoiced: usize,
}

impl TdSynthesis {
    pub(crate) fn new(formant_ratio: f64) -> Self {
        Self {
            formant_ratio,
            resampler: (formant_ratio != 1.0).then(|| resampler::Resampler::new(formant_ratio)),
            unvoiced: (formant_ratio != 1.0).then(|| Box::new([0.0; psola_common::RING])),
            unvoiced_weight: (formant_ratio != 1.0).then(|| Box::new([0.0; psola_common::RING])),
            next_unvoiced: 0,
        }
    }

    fn read(&self, input: &[f64], clock: usize, position: f64) -> f64 {
        if let Some(resampler) = &self.resampler {
            let input: &[f64; psola_common::RING] = input
                .try_into()
                .expect("PSOLA input ring has an invalid length");
            resampler.sample(input, clock, position)
        } else {
            psola_common::interpolate(input, position)
        }
    }

    fn unvoiced_grain(&mut self, source: usize, clock: usize, latency: usize, input: &[f64]) {
        let destination = source + latency;
        let first = destination.saturating_sub(UNVOICED_RADIUS).max(clock);
        assert!(
            first >= clock.saturating_sub(1),
            "unvoiced grain missed its synthesis deadline"
        );
        assert!(destination + UNVOICED_RADIUS < clock + psola_common::RING);
        let input_center = source as f64;
        for target in first..=destination + UNVOICED_RADIUS {
            let offset = target as f64 - destination as f64;
            let window = 0.5 + 0.5 * (std::f64::consts::PI * offset / UNVOICED_RADIUS as f64).cos();
            self.unvoiced.as_mut().unwrap()[target % psola_common::RING] +=
                self.read(input, clock, input_center + offset * self.formant_ratio) * window;
            self.unvoiced_weight.as_mut().unwrap()[target % psola_common::RING] += window;
        }
    }
}

impl PsolaSynthesis for TdSynthesis {
    fn grain_timing(
        &self,
        left_source_span: f64,
        source_period: f64,
        right_source_span: f64,
        pitch_ratio: f64,
        previous_synthesis_period: f64,
    ) -> crate::psola_synthesis::GrainTiming {
        let intermediate_ratio = pitch_ratio / self.formant_ratio;
        let intermediate_period = source_period / intermediate_ratio;
        let period = intermediate_period / self.formant_ratio;
        let left =
            (left_source_span / self.formant_ratio).min(if previous_synthesis_period > 0.0 {
                previous_synthesis_period
            } else {
                period
            });
        let right = (right_source_span / self.formant_ratio).min(period);
        crate::psola_synthesis::GrainTiming {
            period,
            left,
            right,
        }
    }

    fn normalize_output(&self, sample: f64, _weight: f64) -> f64 {
        sample
    }

    fn has_unvoiced_transform(&self) -> bool {
        self.resampler.is_some()
    }

    fn update_unvoiced(&mut self, clock: usize, latency: usize, input: &[f64]) {
        if self.unvoiced.is_none() {
            return;
        }
        let source_safe = (clock as f64
            - self.formant_ratio * UNVOICED_RADIUS as f64
            - resampler::RADIUS as f64
            - 2.0)
            .max(0.0);
        while (self.next_unvoiced as f64) <= source_safe
            && self.next_unvoiced + latency + UNVOICED_RADIUS < clock + psola_common::RING
            && (self.next_unvoiced + latency).saturating_sub(UNVOICED_RADIUS)
                >= clock.saturating_sub(1)
        {
            self.unvoiced_grain(self.next_unvoiced, clock, latency, input);
            self.next_unvoiced += psola_common::HOP;
        }
    }

    fn unvoiced_sample(&self, index: usize) -> Option<f64> {
        let weight = self.unvoiced_weight.as_ref()?.get(index).copied()?;
        Some(if weight > 1e-12 {
            self.unvoiced.as_ref().unwrap()[index] / weight
        } else {
            0.0
        })
    }

    fn clear_output_slot(&mut self, index: usize) {
        if let Some(unvoiced) = &mut self.unvoiced {
            unvoiced[index] = 0.0;
            self.unvoiced_weight.as_mut().unwrap()[index] = 0.0;
        }
    }

    fn grain(
        &mut self,
        source: f64,
        destination: f64,
        left: f64,
        right: f64,
        gain: f64,
        _ratio: f64,
        _preserve_formants: bool,
        clock: usize,
        input: &[f64],
        output: &mut [f64],
        weights: &mut [f64],
        coverage: &mut [bool],
    ) {
        assert!(
            destination - left >= clock as f64 - 1.0,
            "TD-PSOLA grain missed its synthesis deadline"
        );
        let first = (destination - left).floor().max(clock as f64) as usize;
        let last = (destination + right).ceil() as usize;
        assert!(
            last < clock + psola_common::RING,
            "TD-PSOLA synthesis exceeded its preallocated queue"
        );
        for target in first..=last {
            let offset = target as f64 - destination;
            let radius = if offset < 0.0 { left } else { right };
            let normalized = (offset / radius).clamp(-1.0, 1.0);
            let window = 0.5 + 0.5 * (std::f64::consts::PI * normalized).cos();
            let sample_position = source + self.formant_ratio * offset;
            let sample = self.read(input, clock, sample_position);
            let index = target % psola_common::RING;
            output[index] += sample * window * gain;
            coverage[index] = true;
            weights[index] += window;
        }
    }
}
