//! Algorithm-independent interface for PSOLA grain and sample synthesis.

pub(crate) struct GrainTiming {
    pub(crate) period: f64,
    pub(crate) left: f64,
    pub(crate) right: f64,
}

pub(crate) trait PsolaSynthesis {
    fn grain_timing(
        &self,
        previous_source_period: f64,
        source_period: f64,
        next_source_period: f64,
        pitch_ratio: f64,
        previous_synthesis_period: f64,
    ) -> GrainTiming;
    fn normalize_output(&self, sample: f64, weight: f64) -> f64;
    fn has_unvoiced_transform(&self) -> bool;
    fn update_unvoiced(&mut self, clock: usize, latency: usize, input: &[f64]);
    fn unvoiced_sample(&self, index: usize) -> Option<f64>;
    fn clear_output_slot(&mut self, index: usize);

    #[allow(clippy::too_many_arguments)]
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
    );
}
