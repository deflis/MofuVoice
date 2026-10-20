//! Shared timing limits for pitch-synchronous overlap-add implementations.

pub(crate) const RING: usize = 32768;
pub(crate) const HOP: usize = 128;
pub(crate) const DECIMATION: usize = 4;
pub(crate) const TAPS: usize = 33;
pub(crate) const MIN_PERIOD: usize = 80;
pub(crate) const MAX_PERIOD: usize = 800;

pub(crate) fn flank_scales(windows: crate::pitch::SpectralWindows) -> (f64, f64) {
    let ratio = (windows.analysis as f64 / windows.synthesis as f64).sqrt();
    // Skew support without lengthening the grain and changing its spectral resolution.
    (2.0 * ratio / (1.0 + ratio), 2.0 / (1.0 + ratio))
}

pub(crate) fn ring<T: Clone>(value: T) -> Box<[T; RING]> {
    vec![value; RING]
        .into_boxed_slice()
        .try_into()
        .ok()
        .expect("PSOLA ring size must match its allocation")
}

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
