//! In-repository DSP implementations, independent of audio devices.
//! Shared primitives and PSOLA-family pitch processing live here.
pub(crate) mod biquad;
pub(crate) mod complex;
pub(crate) mod envelope;
pub(crate) mod fft;
pub(crate) mod fft_kernel;
pub(crate) mod fft_split;
pub(crate) mod gain;
pub(crate) mod phase;
pub(crate) mod psola;
mod sample;
pub(crate) mod window;

pub use sample::Sample;

#[cfg(feature = "cpal-backend")]
pub(crate) mod resample;
