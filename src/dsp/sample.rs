/// Integer conversion is normalized in f64 so every i32 input bit is representable.
/// Wet integer output rounds and saturates. Dry output bypasses conversion entirely.
pub trait Sample: Copy + Default + PartialEq {
    fn normalized(self) -> f64;
    fn from_normalized(value: f64) -> Self;
}
impl Sample for f64 {
    fn normalized(self) -> f64 {
        self
    }
    fn from_normalized(value: f64) -> Self {
        value
    }
}
impl Sample for f32 {
    fn normalized(self) -> f64 {
        self as f64
    }
    fn from_normalized(value: f64) -> Self {
        value as f32
    }
}
impl Sample for i32 {
    fn normalized(self) -> f64 {
        self as f64 / 2_147_483_648.0
    }
    fn from_normalized(value: f64) -> Self {
        (value * 2_147_483_648.0).round() as Self
    }
}
impl Sample for i16 {
    fn normalized(self) -> f64 {
        self as f64 / 32_768.0
    }
    fn from_normalized(value: f64) -> Self {
        (value * 32_768.0).round() as Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_normalization_round_trips_without_losing_low_bits() {
        for value in i16::MIN..=i16::MAX {
            assert_eq!(i16::from_normalized(value.normalized()), value);
        }
        let mut seed = 17_u32;
        for value in [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX] {
            assert_eq!(i32::from_normalized(value.normalized()), value);
        }
        for _ in 0..100_000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let value = seed as i32;
            assert_eq!(i32::from_normalized(value.normalized()), value);
        }
        assert_eq!(i16::MIN.normalized(), -1.0);
        assert_eq!(i32::MIN.normalized(), -1.0);
        assert!(i16::MAX.normalized() < 1.0);
        assert!(i32::MAX.normalized() < 1.0);
    }

    #[test]
    fn integer_output_rounds_ties_away_from_zero_and_saturates() {
        for (lsb, expected) in [
            (-1.5, -2),
            (-0.5, -1),
            (-0.49, 0),
            (0.49, 0),
            (0.5, 1),
            (1.5, 2),
        ] {
            assert_eq!(i16::from_normalized(lsb / 32768.0), expected as i16);
            assert_eq!(i32::from_normalized(lsb / 2147483648.0), expected);
        }
        for input in [1.0, 2.0, f64::INFINITY] {
            assert_eq!(i16::from_normalized(input), i16::MAX);
            assert_eq!(i32::from_normalized(input), i32::MAX);
            assert_eq!(i16::from_normalized(-input), i16::MIN);
            assert_eq!(i32::from_normalized(-input), i32::MIN);
        }
        assert_eq!(i16::from_normalized(f64::NAN), 0);
        assert_eq!(i32::from_normalized(f64::NAN), 0);
    }

    #[test]
    fn float_conversion_preserves_signed_zero_and_finite_extremes() {
        for value in [
            0.0_f32,
            -0.0,
            f32::from_bits(1),
            -f32::from_bits(1),
            f32::MIN_POSITIVE,
            -1.0,
            1.0,
            f32::MIN,
            f32::MAX,
            f32::NEG_INFINITY,
            f32::INFINITY,
        ] {
            assert_eq!(
                f32::from_normalized(value.normalized()).to_bits(),
                value.to_bits()
            );
        }
    }
}
