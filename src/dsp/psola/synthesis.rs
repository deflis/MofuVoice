//! Algorithm-independent interface for PSOLA grain and sample synthesis.

pub(crate) struct GrainTiming {
    pub(crate) period: f64,
    pub(crate) left: f64,
    pub(crate) right: f64,
}

pub(crate) trait PsolaSynthesis: Send {
    /// Source spans are distances to adjacent pitch marks, not differences of periods.
    fn grain_timing(
        &self,
        left_source_span: f64,
        source_period: f64,
        right_source_span: f64,
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

#[cfg(test)]
mod tests {
    use super::PsolaSynthesis;

    #[test]
    fn stable_and_changing_pitch_marks_keep_nonzero_grain_support() {
        for formant in [0.5, 1.0, 2.0] {
            let td = crate::dsp::psola::TdSynthesis::new(
                formant,
                crate::pitch::SpectralWindows::default(),
            );
            for pitch in [0.5, 1.0, 2.0] {
                let timing = td.grain_timing(400.0, 400.0, 400.0, pitch, 0.0);
                assert_eq!(timing.period, 400.0 / pitch);
                assert_eq!(
                    timing.left,
                    (4.0 / 3.0) * (400.0 / formant).min(timing.period)
                );
                assert_eq!(timing.right * 2.0, timing.left);
                let changing = td.grain_timing(390.0, 400.0, 410.0, pitch, 180.0);
                assert_eq!(changing.left, (4.0 / 3.0) * (390.0 / formant).min(180.0));
                assert_eq!(
                    changing.right,
                    (2.0 / 3.0) * (410.0 / formant).min(changing.period)
                );
            }
        }
        let fd = crate::dsp::psola::FdSynthesis::new(crate::pitch::SpectralWindows::default());
        let timing = fd.grain_timing(400.0, 400.0, 400.0, 1.5, 0.0);
        assert_eq!(timing.period, 400.0 / 1.5);
        assert_eq!(timing.left, timing.period);
        assert_eq!(timing.right, timing.period);
        let changing = fd.grain_timing(390.0, 400.0, 410.0, 1.5, 380.0);
        assert_eq!(changing.left, 380.0);
        assert_eq!(changing.right, changing.period);
    }
}
