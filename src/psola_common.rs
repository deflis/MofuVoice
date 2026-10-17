//! Shared timing limits for pitch-synchronous overlap-add implementations.

pub(crate) const LATENCY: usize = 4608;
pub(crate) const RING: usize = 16384;
pub(crate) const HOP: usize = 128;
pub(crate) const ANALYSIS: usize = 3072;
pub(crate) const DECIMATION: usize = 4;
pub(crate) const TAPS: usize = 33;
pub(crate) const LOOKAHEAD: usize = ANALYSIS / 2 + TAPS / 2;
pub(crate) const MIN_PERIOD: usize = 80;
pub(crate) const MAX_PERIOD: usize = 800;

pub(crate) fn interpolate(input: &[f64], position: f64) -> f64 {
    if position < 1.0 {
        return 0.0;
    }
    let index = position.floor() as usize;
    let fraction = position - index as f64;
    let a = input[(index - 1) % RING];
    let b = input[index % RING];
    let c = input[(index + 1) % RING];
    let d = input[(index + 2) % RING];
    b + 0.5
        * fraction
        * (c - a
            + fraction * (2.0 * a - 5.0 * b + 4.0 * c - d + fraction * (3.0 * (b - c) + d - a)))
}
