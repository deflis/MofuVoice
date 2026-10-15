//! One-pole time constants. Callers choose decay versus update weight explicitly.
pub(crate) fn decay(ms: f64, sample_rate: f64) -> f64 {
    (-1.0 / (ms * 0.001 * sample_rate)).exp()
}

pub(crate) fn update_rate(ms: f64, sample_rate: f64) -> f64 {
    -(-1.0 / (ms * 0.001 * sample_rate)).exp_m1()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_time_constant_decays_to_inverse_e() {
        for sample_rate in [44100.0, 48000.0, 96000.0] {
            for ms in [1.0, 30.0, 1500.0] {
                let samples = ms * 0.001 * sample_rate;
                assert!((decay(ms, sample_rate).powf(samples) - (-1.0_f64).exp()).abs() < 1e-11);
                assert!(
                    (decay(ms, sample_rate) + update_rate(ms, sample_rate) - 1.0).abs() < 1e-15
                );
            }
        }
        assert!(update_rate(1e18, 48000.0) > 0.0);
    }
}
