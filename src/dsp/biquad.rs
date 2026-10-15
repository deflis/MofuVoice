//! Normalized transposed direct-form II; coefficients already include 1/a0.
pub(crate) struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z1: f64,
    z2: f64,
}

impl Biquad {
    pub(crate) fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Self {
            b,
            a,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub(crate) fn process(&mut self, input: f64) -> f64 {
        let output = self.b[0] * input + self.z1;
        self.z1 = self.b[1] * input - self.a[0] * output + self.z2;
        self.z2 = self.b[2] * input - self.a[1] * output;
        // Prevent subnormal tails from increasing callback cost after silence.
        if self.z1.abs() < 1e-30 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-30 {
            self.z2 = 0.0;
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_matches_independent_difference_equation() {
        // Direct form I is an independent reference for the transposed state layout.
        for (b, a) in [
            ([1.0, 0.0, 0.0], [0.0, 0.0]),
            ([0.2, -0.3, 0.1], [0.0, 0.0]),
            ([0.25, 0.5, 0.25], [-0.4, 0.2]),
            ([0.9, -1.8, 0.9], [-1.7, 0.75]),
        ] {
            let mut filter = Biquad::new(b, a);
            let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
            for i in 0..8192 {
                let input = if i == 0 {
                    1.0
                } else if i < 4096 {
                    (i as f64 * 0.37).sin() * 0.3
                } else {
                    0.0
                };
                let expected = b[0] * input + b[1] * x1 + b[2] * x2 - a[0] * y1 - a[1] * y2;
                let actual = filter.process(input);
                assert!(
                    (actual - expected).abs() < 2e-14,
                    "sample {i}: {actual} != {expected}"
                );
                (x2, x1, y2, y1) = (x1, input, y1, expected);
            }
        }
    }

    #[test]
    fn reset_discards_history_and_silence_finishes_at_exact_zero() {
        let b = [0.25, 0.5, 0.25];
        let a = [-0.4, 0.2];
        let mut filter = Biquad::new(b, a);
        for _ in 0..100 {
            filter.process(0.5);
        }
        filter.reset();
        let mut fresh = Biquad::new(b, a);
        for input in [0.0, 1.0, -0.2, 0.0, 0.7] {
            assert_eq!(
                filter.process(input).to_bits(),
                fresh.process(input).to_bits()
            );
        }
        // This pole would still have a nonzero tail after 512 samples without
        // the explicit flush; ordinary f64 underflow must not make this pass.
        for _ in 0..512 {
            filter.process(0.0);
        }
        assert_eq!(filter.z1.to_bits(), 0.0_f64.to_bits());
        assert_eq!(filter.z2.to_bits(), 0.0_f64.to_bits());
        assert_eq!(filter.process(0.0).to_bits(), 0.0_f64.to_bits());
    }
}
