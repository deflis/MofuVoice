//! Amplitude and power use different decibel scales. Floors belong to callers.
#[inline]
pub(crate) fn amplitude(db: f64) -> f64 {
    (db * (std::f64::consts::LOG2_10 / 20.0)).exp2()
}

#[inline]
pub(crate) fn power(db: f64) -> f64 {
    (db * (std::f64::consts::LOG2_10 / 10.0)).exp2()
}

#[inline]
pub(crate) fn amplitude_db(value: f64, floor: f64) -> f64 {
    20.0 * value.max(floor).log10()
}

/// Calibrate a nearby inverse-dB estimate to the exact floating-point boundary.
/// Returns the largest adjacent positive value whose measured dB is <= threshold.
/// This is setup work; per-sample gates can then use an ordinary comparison.
pub(crate) fn upper_db_boundary(
    mut candidate: f64,
    threshold: f64,
    measure: impl Fn(f64) -> f64,
) -> f64 {
    assert!(candidate.is_finite() && candidate > 0.0 && threshold.is_finite());
    for _ in 0..128 {
        let level = measure(candidate);
        assert!(level.is_finite());
        if level > threshold {
            candidate = candidate.next_down();
        } else {
            let next = candidate.next_up();
            let next_level = measure(next);
            assert!(next_level.is_finite());
            if next_level > threshold {
                return candidate;
            }
            candidate = next;
        }
    }
    panic!("inverse-dB estimate is not within 128 representable values of its boundary");
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_two_conversion_matches_decimal_reference_through_dsp_range() {
        for i in -240_000..=96_000 {
            let db = i as f64 / 1000.0;
            for (actual, expected) in [
                (amplitude(db), 10.0_f64.powf(db / 20.0)),
                (power(db), 10.0_f64.powf(db / 10.0)),
            ] {
                assert!(
                    (actual / expected - 1.0).abs() < 2e-14,
                    "db={db}: {actual} != {expected}"
                );
            }
        }
        for convert in [amplitude, power] {
            assert_eq!(convert(0.0).to_bits(), 1.0_f64.to_bits());
            assert_eq!(convert(f64::NEG_INFINITY), 0.0);
            assert_eq!(convert(f64::INFINITY), f64::INFINITY);
            assert!(convert(f64::NAN).is_nan());
        }
    }

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
