//! PSOLA-family pitch shifting: shared analysis and scheduling, TD/FD grain
//! synthesis, and the separate phase-vocoder time-stretch (PVSOLA) backend.
//! Keep implementation modules private; the pitch engine uses this entry point.

mod common;
mod correlation;
mod fd;
mod fft;
mod pv;
mod resampler;
mod stream;
mod synthesis;
mod td;

pub(crate) use fd::FdSynthesis;
pub(crate) use pv::{PvsOla, LATENCY as PVSOLA_LATENCY};
pub(crate) use stream::{latency, PsolaStream};
pub(crate) use td::TdSynthesis;
