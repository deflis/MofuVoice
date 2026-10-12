//! Two overlapping variable-delay reads; all storage is allocated before processing.
//! This short window favors latency over bass/transient quality.
//! Bandlimited resampling is not performed. Wet delay is variable, not a fixed latency.
use super::formant::FormantCorrector;
use std::{error::Error, fmt};

pub const MIN_DELAY: usize = 2;
pub const DELAY_SPAN: usize = 476;
pub const MAX_DELAY: usize = MIN_DELAY + DELAY_SPAN;
pub const CENTER_DELAY: usize = MIN_DELAY + DELAY_SPAN / 2;
pub const SMOOTH_SAMPLES: usize = 960; // 20 ms at the application's fixed 48 kHz.
pub const FADE_SAMPLES: usize = 480; // 10 ms for entering/leaving zero-delay bypass.
const HISTORY: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidPitch;
impl fmt::Display for InvalidPitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("pitch must be finite and between -12 and 12 semitones")
    }
}
impl Error for InvalidPitch {}

pub fn pitch_ratio(semitones: f64) -> Result<f64, InvalidPitch> {
    if !semitones.is_finite() || !(-12.0..=12.0).contains(&semitones) {
        return Err(InvalidPitch);
    }
    Ok(2.0_f64.powf(semitones / 12.0))
}

/// Integer conversion is normalized in f64 so every i32 input bit is representable.
/// Wet integer output rounds and saturates. Dry output bypasses conversion entirely.
pub trait Sample: Copy + Default + PartialEq {
    fn normalized(self) -> f64;
    fn from_normalized(value: f64) -> Self;
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

pub struct PitchShifter {
    history: Box<[f64; HISTORY]>,
    formant: Option<FormantCorrector>,
    write: usize,
    phase: f64,
    semitones: f64,
    ratio: f64,
    target_ratio: f64,
    ratio_step: f64,
    ratio_remaining: usize,
    wet: f64,
    target_wet: f64,
    wet_step: f64,
    wet_remaining: usize,
}
impl PitchShifter {
    pub fn new(semitones: f64, preserve_formants: bool) -> Result<Self, InvalidPitch> {
        let ratio = pitch_ratio(semitones)?;
        let wet = f64::from(semitones != 0.0);
        Ok(Self {
            history: Box::new([0.0; HISTORY]),
            formant: preserve_formants.then(FormantCorrector::default),
            write: 0,
            phase: 0.0,
            semitones,
            ratio,
            target_ratio: ratio,
            ratio_step: 0.0,
            ratio_remaining: 0,
            wet,
            target_wet: wet,
            wet_step: 0.0,
            wet_remaining: 0,
        })
    }

    /// Validate on the control side. Repeating a target does not restart a ramp.
    pub fn set_semitones(&mut self, semitones: f64) -> Result<(), InvalidPitch> {
        let ratio = pitch_ratio(semitones)?;
        if self.semitones == semitones {
            return Ok(());
        }
        self.semitones = semitones;
        self.target_ratio = ratio;
        self.ratio_step = (ratio - self.ratio) / SMOOTH_SAMPLES as f64;
        self.ratio_remaining = SMOOTH_SAMPLES;
        let target_wet = f64::from(semitones != 0.0);
        if self.target_wet != target_wet {
            self.target_wet = target_wet;
            self.wet_step = (target_wet - self.wet) / FADE_SAMPLES as f64;
            self.wet_remaining = FADE_SAMPLES;
        }
        Ok(())
    }

    fn read(&self, phase: f64) -> f64 {
        let delay = MIN_DELAY as f64 + phase * DELAY_SPAN as f64;
        let whole = delay.floor() as usize;
        let fraction = delay - whole as f64;
        let recent = (self.write + HISTORY - whole) % HISTORY;
        let older = (recent + HISTORY - 1) % HISTORY;
        self.history[recent] * (1.0 - fraction) + self.history[older] * fraction
    }

    /// Equal-length mono blocks. Returns the number of nonzero input samples.
    /// No allocation, lock, I/O or parameter exponentiation occurs here.
    pub fn process_block<T: Sample>(&mut self, input: &[T], output: &mut [T]) -> u64 {
        assert_eq!(input.len(), output.len());
        let mut nonzero = 0;
        for (&sample, out) in input.iter().zip(output) {
            nonzero += u64::from(sample != T::default());
            let value = sample.normalized();
            // Prevent a nonfinite float from poisoning future wet blocks. Dry bits are preserved.
            let dry = if value.is_finite() { value } else { 0.0 };
            self.history[self.write] = match &mut self.formant {
                Some(formant) => formant.analyze(dry),
                None => dry,
            };
            if self.ratio_remaining != 0 {
                self.ratio_remaining -= 1;
                self.ratio += self.ratio_step;
                if self.ratio_remaining == 0 {
                    self.ratio = self.target_ratio;
                }
            }
            if self.wet_remaining != 0 {
                self.wet_remaining -= 1;
                self.wet += self.wet_step;
                if self.wet_remaining == 0 {
                    self.wet = self.target_wet;
                }
            }
            if self.wet == 0.0 && self.formant.is_none() {
                *out = sample;
            } else {
                let second = (self.phase + 0.5).fract();
                // Complementary Hann weights are zero where each read head wraps.
                let weight = 0.5 - 0.5 * (std::f64::consts::TAU * self.phase).cos();
                let shifted = self.read(self.phase) * weight + self.read(second) * (1.0 - weight);
                let shifted = match &mut self.formant {
                    Some(formant) => formant.synthesize(shifted),
                    None => shifted,
                };
                *out = if self.wet == 0.0 {
                    sample
                } else {
                    T::from_normalized(dry * (1.0 - self.wet) + shifted * self.wet)
                };
            }
            // y[n] = x[n-delay[n]]: read speed = 1 - delta(delay) = ratio.
            self.phase = (self.phase + (1.0 - self.ratio) / DELAY_SPAN as f64).rem_euclid(1.0);
            self.write = (self.write + 1) % HISTORY;
        }
        nonzero
    }
}
