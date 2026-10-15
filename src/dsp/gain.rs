//! Amplitude and power use different decibel scales. Floors belong to callers.
#[inline]
pub(crate) fn amplitude(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}

#[inline]
pub(crate) fn power(db: f64) -> f64 {
    10.0_f64.powf(db / 10.0)
}

#[inline]
pub(crate) fn amplitude_db(value: f64, floor: f64) -> f64 {
    20.0 * value.max(floor).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decibel_scales_round_trip_and_keep_caller_floors() {
        for db in [-120.0, -72.0, -20.0, 0.0, 18.0] {
            let value = amplitude(db);
            assert!((amplitude_db(value, 1e-12) - db).abs() < 1e-12);
            assert!((value * value - power(db)).abs() < 1e-12);
        }
        assert_eq!(amplitude_db(0.0, 1e-10), -200.0);
        assert_eq!(amplitude_db(0.0, 1e-12), -240.0);
    }
}
