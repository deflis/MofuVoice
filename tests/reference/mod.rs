// Frozen pre-optimization DSP for numerical regression and paired benchmarks.
#![allow(dead_code)]
pub mod formant;
pub mod pitch;

pub fn signal(count: usize) -> Vec<f32> {
    let mut seed = 1_u32;
    let (mut a, mut b) = (0.0, 0.0);
    let coefficient = 1.98 * (std::f64::consts::TAU * 1500.0 / crate::RATE as f64).cos();
    (0..count)
        .map(|n| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = seed as f64 / u32::MAX as f64 - 0.5;
            let next = 0.001 * noise + coefficient * a - 0.9801 * b;
            b = a;
            a = next;
            (next + 0.1 * (std::f64::consts::TAU * 203.0 * n as f64 / crate::RATE as f64).sin())
                as f32
        })
        .collect()
}
