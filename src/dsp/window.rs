//! Periodic analysis windows.
pub(crate) fn hann(index: usize, length: usize) -> f64 {
    0.5 - 0.5 * (std::f64::consts::TAU * index as f64 / length as f64).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_hann_has_expected_energy_symmetry_and_overlap_sum() {
        for length in [4, 16, 256, 512, 2048] {
            assert_eq!(hann(0, length), 0.0);
            assert_eq!(hann(length / 2, length), 1.0);
            let mut sum = 0.0;
            let mut energy = 0.0;
            for i in 0..length {
                let value = hann(i, length);
                assert!((0.0..=1.0).contains(&value));
                assert!((value - hann((length - i) % length, length)).abs() < 1e-15);
                sum += value;
                energy += value * value;
            }
            assert!((sum - length as f64 / 2.0).abs() < 1e-10);
            assert!((energy - 3.0 * length as f64 / 8.0).abs() < 1e-10);
            for overlaps in [2, 4] {
                let hop = length / overlaps;
                for i in 0..hop {
                    let total: f64 = (0..overlaps).map(|j| hann(i + j * hop, length)).sum();
                    assert!((total - overlaps as f64 / 2.0).abs() < 1e-14);
                }
            }
        }
    }
}
