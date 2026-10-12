//! Two overlapping variable-delay reads; all storage is allocated before processing.
//! This short window favors latency over bass/transient quality.
//! Bandlimited resampling is not performed. Wet delay is variable, not a fixed latency.
use crate::formant::FormantCorrector;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidFormant;
impl fmt::Display for InvalidFormant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("formant shift must be finite and between -12 and 12 semitones; a nonzero shift requires --formant preserve")
    }
}
impl Error for InvalidFormant {}

pub fn formant_ratio(semitones: f64) -> Result<f64, InvalidFormant> {
    if !semitones.is_finite() || !(-12.0..=12.0).contains(&semitones) {
        return Err(InvalidFormant);
    }
    Ok(2.0_f64.powf(semitones / 12.0))
}

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
    formant_shift: f64,
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
    pub fn new(
        semitones: f64,
        preserve_formants: bool,
        formant_shift: f64,
    ) -> Result<Self, Box<dyn Error>> {
        let ratio = pitch_ratio(semitones)?;
        let shift_ratio = formant_ratio(formant_shift)?;
        if !preserve_formants && formant_shift != 0.0 {
            return Err(InvalidFormant.into());
        }
        let wet = f64::from(semitones != 0.0 || formant_shift != 0.0);
        Ok(Self {
            history: Box::new([0.0; HISTORY]),
            formant: preserve_formants.then(|| FormantCorrector::new(shift_ratio)),
            write: 0,
            phase: 0.0,
            semitones,
            formant_shift,
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
        let target_wet = f64::from(semitones != 0.0 || self.formant_shift != 0.0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RATE;

    fn resonant_noise(count: usize) -> Vec<f32> {
        let radius = 0.99_f64;
        let coefficient = 2.0 * radius * (std::f64::consts::TAU * 1500.0 / RATE as f64).cos();
        let (mut a, mut b) = (0.0, 0.0);
        let mut seed = 1_u32;
        (0..count)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = seed as f64 / u32::MAX as f64 - 0.5;
                let next = 0.001 * noise + coefficient * a - radius * radius * b;
                b = a;
                a = next;
                next as f32
            })
            .collect()
    }

    fn spectral_power(signal: &[f32], frequency: f64) -> f64 {
        let coefficient = 2.0 * (std::f64::consts::TAU * frequency / RATE as f64).cos();
        let (mut a, mut b) = (0.0, 0.0);
        for (n, &sample) in signal.iter().enumerate() {
            let window = 0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / signal.len() as f64).cos();
            let next = sample as f64 * window + coefficient * a - b;
            b = a;
            a = next;
        }
        (a * a + b * b - coefficient * a * b).max(0.0)
    }

    #[test]
    fn independent_formant_shift_moves_resonance() {
        let input = resonant_noise(40_960);
        for (pitch, shift) in [
            (0.0, -12.0),
            (0.0, -3.0),
            (0.0, 3.0),
            (0.0, 12.0),
            (-3.0, -3.0),
            (-3.0, 3.0),
            (3.0, -3.0),
            (3.0, 3.0),
        ] {
            let mut output = vec![0.0; input.len()];
            PitchShifter::new(pitch, true, shift)
                .unwrap()
                .process_block(&input, &mut output);
            let expected = 1500.0 * formant_ratio(shift).unwrap();
            let peak = (500..=3500)
                .step_by(10)
                .map(|frequency| {
                    let power: f64 = output[8192..]
                        .as_chunks::<4096>()
                        .0
                        .iter()
                        .map(|chunk| spectral_power(chunk, frequency as f64))
                        .sum();
                    (frequency as f64, power)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0;
            assert!(
                (peak - expected).abs() < 150.0,
                "pitch={pitch}, shift={shift}: peak={peak}, expected={expected}"
            );
            assert!(output
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 1.0));
        }
    }

    #[test]
    fn shifted_formants_recover_from_overloads_and_silence_in_all_formats() {
        for shift in [-12.0, -3.0, 3.0, 12.0] {
            let mut dsp = PitchShifter::new(0.0, true, shift).unwrap();
            let mut input = vec![0.0_f32; 48_000];
            input[..5].copy_from_slice(&[
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::MAX,
                -f32::MAX,
            ]);
            input[512..4096].copy_from_slice(&tone(3584, 730.0));
            let mut output = vec![0.0; input.len()];
            dsp.process_block(&input, &mut output);
            assert!(output.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            assert!(output[47_000..].iter().all(|x| x.abs() < 1e-5));
            let mut output = [0_i32; 4096];
            dsp.process_block(&[i32::MAX; 4096], &mut output);
            let mut output = [0_i16; 4096];
            dsp.process_block(&[i16::MIN; 4096], &mut output);
            let mut recovery = vec![0.0_f32; 48_000];
            dsp.process_block(&vec![0.0; 48_000], &mut recovery);
            assert!(recovery.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            assert!(recovery[47_000..].iter().all(|x| x.abs() < 1e-5));
            // A fresh silence-only stream must produce exact silence at every shift.
            let mut silence = [1.0_f32; 4096];
            PitchShifter::new(0.0, true, shift)
                .unwrap()
                .process_block(&[0.0; 4096], &mut silence);
            assert!(silence.iter().all(|x| *x == 0.0));
        }
    }

    #[test]
    fn formant_shift_keeps_pitch_and_survives_pitch_retargeting() {
        let input = tone(24_000, RATE as f64 / 238.0);
        for shift in [-12.0, -3.0, 3.0, 12.0] {
            let mut dsp = PitchShifter::new(3.0, true, shift).unwrap();
            dsp.set_semitones(0.0).unwrap();
            let mut output = vec![0.0; input.len()];
            dsp.process_block(&input, &mut output);
            assert_eq!(dsp.wet, 1.0);
            let signal = &output[8192..];
            let measured = signal
                .windows(2)
                .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                .count() as f64
                * RATE as f64
                / signal.len() as f64;
            assert!(
                (measured - RATE as f64 / 238.0).abs() < 4.0,
                "shift={shift}: {measured}"
            );
        }
    }

    #[test]
    fn shifted_formants_are_independent_of_block_boundaries() {
        let input = resonant_noise(4096);
        for shift in [-12.0, -2.5, 2.5, 12.0] {
            let mut whole = vec![0.0; input.len()];
            PitchShifter::new(0.0, true, shift)
                .unwrap()
                .process_block(&input, &mut whole);
            for size in [1, 7, 64, 512] {
                let mut dsp = PitchShifter::new(0.0, true, shift).unwrap();
                let mut output = vec![0.0; input.len()];
                for (a, b) in input.chunks(size).zip(output.chunks_mut(size)) {
                    dsp.process_block(a, b);
                }
                assert_eq!(whole, output);
            }
        }
        assert!(PitchShifter::new(0.0, false, 3.0).is_err());
        for shift in [f64::NAN, f64::INFINITY, -12.1, 12.1] {
            assert!(PitchShifter::new(0.0, true, shift).is_err());
        }
    }

    #[test]
    fn correction_keeps_resonance_nearer_original_in_both_pitch_directions() {
        // Broadband excitation through a known 1500 Hz all-pole vocal-tract model.
        // Average independent spectral windows to avoid selecting a random noise bin.
        fn peak(signal: &[f32]) -> f64 {
            (900..=2100)
                .step_by(10)
                .map(|frequency| {
                    let power: f64 = signal[8192..]
                        .as_chunks::<4096>()
                        .0
                        .iter()
                        .map(|chunk| spectral_power(chunk, frequency as f64))
                        .sum();
                    (frequency as f64, power)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0
        }
        let input = resonant_noise(40_960);
        let original = peak(&input);
        assert!((original - 1500.0).abs() < 100.0);
        for pitch in [-3.0, 3.0] {
            let mut corrected = vec![0.0; input.len()];
            let mut uncorrected = vec![0.0; input.len()];
            PitchShifter::new(pitch, true, 0.0)
                .unwrap()
                .process_block(&input, &mut corrected);
            PitchShifter::new(pitch, false, 0.0)
                .unwrap()
                .process_block(&input, &mut uncorrected);
            let restored = peak(&corrected);
            let shifted = peak(&uncorrected);
            assert!((shifted - original * pitch_ratio(pitch).unwrap()).abs() < 100.0);
            assert!(
                (restored - original).abs() < 100.0,
                "pitch={pitch}: original={original}, restored={restored}, shifted={shifted}"
            );
            assert!((restored - original).abs() < (shifted - original).abs() * 0.5);
        }
    }

    #[test]
    fn corrected_periodic_excitation_still_shifts_pitch() {
        let input = tone(24_000, RATE as f64 / 238.0);
        for pitch in [-12.0, -3.0, 3.0, 12.0] {
            let mut output = vec![0.0; input.len()];
            PitchShifter::new(pitch, true, 0.0)
                .unwrap()
                .process_block(&input, &mut output);
            let expected = RATE as f64 / 238.0 * pitch_ratio(pitch).unwrap();
            let signal = &output[8192..];
            let measured = signal
                .windows(2)
                .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                .count() as f64
                * RATE as f64
                / signal.len() as f64;
            assert!(
                (measured - expected).abs() < 4.0,
                "pitch={pitch}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn corrected_processing_is_independent_of_callback_sizes() {
        let input = resonant_noise(4096);
        for pitch in [-12.0, -3.0, 0.0, 3.0, 12.0] {
            let mut whole = vec![0.0; input.len()];
            PitchShifter::new(pitch, true, 0.0)
                .unwrap()
                .process_block(&input, &mut whole);
            for size in [1, 7, 64, 512, 8192] {
                let mut dsp = PitchShifter::new(pitch, true, 0.0).unwrap();
                let mut output = vec![0.0; input.len()];
                for (a, b) in input.chunks(size).zip(output.chunks_mut(size)) {
                    dsp.process_block(a, b);
                }
                assert_eq!(whole, output, "pitch={pitch}, size={size}");
            }
        }
    }

    #[test]
    fn corrected_bypass_preserves_all_formats_and_returns_after_retargeting() {
        let floats = [
            1.0_f32,
            -0.0,
            f32::from_bits(1),
            f32::from_bits(0x7fc01234),
            f32::INFINITY,
        ];
        let mut output = [0.0; 5];
        PitchShifter::new(0.0, true, 0.0)
            .unwrap()
            .process_block(&floats, &mut output);
        assert_eq!(floats.map(f32::to_bits), output.map(f32::to_bits));
        let integers = [i32::MIN, -1, 0, 1, i32::MAX];
        let mut output = [0; 5];
        PitchShifter::new(0.0, true, 0.0)
            .unwrap()
            .process_block(&integers, &mut output);
        assert_eq!(integers, output);
        let shorts = [i16::MIN, -1, 0, 1, i16::MAX];
        let mut output = [0; 5];
        PitchShifter::new(0.0, true, 0.0)
            .unwrap()
            .process_block(&shorts, &mut output);
        assert_eq!(shorts, output);
        let input = resonant_noise(4096);
        let mut output = vec![0.0; input.len()];
        let mut dsp = PitchShifter::new(0.0, true, 0.0).unwrap();
        for pitch in [3.0, -3.0, 12.0, -12.0, 0.0] {
            dsp.set_semitones(pitch).unwrap();
            dsp.process_block(&input[..100], &mut output[..100]);
            dsp.set_semitones(-pitch).unwrap();
            dsp.process_block(&input[100..], &mut output[100..]);
            assert!(output.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        }
        assert_eq!(input[100 + FADE_SAMPLES..], output[100 + FADE_SAMPLES..]);
    }

    #[test]
    fn corrected_silence_nonfinite_input_and_integer_overloads_are_bounded() {
        for pitch in [-12.0, -3.0, 3.0, 12.0] {
            let mut dsp = PitchShifter::new(pitch, true, 0.0).unwrap();
            let mut input = vec![0.0_f32; 8192];
            input[..3].copy_from_slice(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY]);
            let mut output = vec![1.0; input.len()];
            dsp.process_block(&input, &mut output);
            assert!(output.iter().all(|&s| s == 0.0));
            let input: Vec<i32> = (0..8192)
                .map(|n| if n % 2 == 0 { i32::MAX } else { i32::MIN })
                .collect();
            let mut output = vec![0_i32; input.len()];
            dsp.process_block(&input, &mut output);
            assert!(output.iter().any(|&s| s != 0));
            let mut output = vec![0.0_f32; 16_384];
            dsp.process_block(&vec![0.0; output.len()], &mut output);
            assert!(output.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!(output[12_000..].iter().all(|s| s.abs() < 1e-6));
        }
    }

    #[test]
    fn corrected_impulses_extreme_floats_and_changing_envelopes_recover() {
        for pitch in [-12.0, -3.0, 3.0, 12.0] {
            let mut dsp = PitchShifter::new(pitch, true, 0.0).unwrap();
            let mut input = vec![0.0_f32; 48_000];
            input[0] = 1.0;
            for (n, sample) in input[1024..24_000].iter_mut().enumerate() {
                *sample = match (n / 128) % 4 {
                    0 => {
                        (0.9 * (std::f64::consts::TAU * 400.0 * n as f64 / RATE as f64).sin())
                            as f32
                    }
                    1 => {
                        if n % 2 == 0 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    2 => {
                        if n % 2 == 0 {
                            f32::MAX
                        } else {
                            -f32::MAX
                        }
                    }
                    _ => 0.0,
                };
            }
            let mut output = vec![0.0; input.len()];
            dsp.process_block(&input, &mut output);
            assert!(output[..MIN_DELAY].iter().all(|&s| s == 0.0));
            assert!(output.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!(output[40_000..].iter().all(|s| s.abs() < 1e-6));
        }
    }

    fn tone(count: usize, frequency: f64) -> Vec<f32> {
        (0..count)
            .map(|n| {
                (0.7 * (std::f64::consts::TAU * frequency * n as f64 / RATE as f64).sin()) as f32
            })
            .collect()
    }

    #[test]
    fn validates_range_and_octave_ratios() {
        assert_eq!(pitch_ratio(-12.0), Ok(0.5));
        assert_eq!(pitch_ratio(0.0), Ok(1.0));
        assert_eq!(pitch_ratio(12.0), Ok(2.0));
        for invalid in [-12.01, 12.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(PitchShifter::new(invalid, false, 0.0).is_err());
            let mut dsp = PitchShifter::new(3.0, false, 0.0).unwrap();
            assert!(dsp.set_semitones(invalid).is_err());
            assert_eq!(dsp.target_ratio, pitch_ratio(3.0).unwrap());
        }
    }

    #[test]
    fn zero_pitch_preserves_bits_and_has_no_impulse_delay() {
        let input = [
            1.0_f32,
            -0.0,
            f32::from_bits(1),
            f32::from_bits(0x7fc01234),
            -1.0,
        ];
        let mut out = [0.0; 5];
        PitchShifter::new(0.0, false, 0.0)
            .unwrap()
            .process_block(&input, &mut out);
        assert_eq!(input.map(f32::to_bits), out.map(f32::to_bits));
        let integers = [i32::MIN, -1, 0, 1, i32::MAX];
        let mut out = [0; 5];
        PitchShifter::new(0.0, false, 0.0)
            .unwrap()
            .process_block(&integers, &mut out);
        assert_eq!(integers, out);
    }

    #[test]
    fn silence_and_impulse_stay_inside_wet_delay_bounds() {
        for pitch in [-12.0, -3.0, 3.0, 12.0] {
            let mut dsp = PitchShifter::new(pitch, false, 0.0).unwrap();
            let mut out = vec![1.0; 2048];
            dsp.process_block(&vec![0.0_f32; 2048], &mut out);
            assert!(out.iter().all(|&s| s == 0.0));
            // Several initial phases, including immediately before a wrap.
            for phase in [0.0, 0.25, 0.499, 0.75, 0.999] {
                let mut dsp = PitchShifter::new(pitch, false, 0.0).unwrap();
                dsp.phase = phase;
                let mut input = vec![0.0_f32; MAX_DELAY + 16];
                input[0] = 1.0;
                let mut out = vec![0.0; input.len()];
                dsp.process_block(&input, &mut out);
                assert!(out[..MIN_DELAY].iter().all(|&s| s == 0.0));
                assert!(out[MAX_DELAY + 1..].iter().all(|&s| s == 0.0));
                assert!(out.iter().any(|&s| s > 0.0), "pitch={pitch}, phase={phase}");
                assert!(out.iter().all(|&s| (0.0..=1.0).contains(&s)));
            }
        }
    }

    #[test]
    fn processing_is_independent_of_block_boundaries() {
        let input = tone(20_000, 337.0);
        for pitch in [-12.0, -3.0, 0.0, 3.0, 12.0] {
            let mut whole = vec![0.0; input.len()];
            PitchShifter::new(pitch, false, 0.0)
                .unwrap()
                .process_block(&input, &mut whole);
            for size in [1, 7, 64, 512, 8192] {
                let mut dsp = PitchShifter::new(pitch, false, 0.0).unwrap();
                let mut out = vec![0.0; input.len()];
                for (a, b) in input.chunks(size).zip(out.chunks_mut(size)) {
                    dsp.process_block(a, b);
                }
                assert_eq!(whole, out, "pitch={pitch}, size={size}");
            }
        }
    }

    #[test]
    fn periodic_tones_shift_by_requested_ratio_in_both_directions() {
        // Integer periods in the half-window align both grains, isolating read speed
        // from the short-window modulation sidebands tested separately below.
        for frequency in [RATE as f64 / 238.0, RATE as f64 * 5.0 / 238.0] {
            let input = tone(48_000, frequency);
            for pitch in [-12.0, -3.0, 3.0, 12.0] {
                let mut out = vec![0.0; input.len()];
                PitchShifter::new(pitch, false, 0.0)
                    .unwrap()
                    .process_block(&input, &mut out);
                let signal = &out[2000..];
                let crossings = signal
                    .windows(2)
                    .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                    .count();
                let measured = crossings as f64 * RATE as f64 / signal.len() as f64;
                let expected = frequency * pitch_ratio(pitch).unwrap();
                assert!(
                    (measured - expected).abs() < 2.0,
                    "{pitch}: {measured} vs {expected}"
                );
                assert!(signal.iter().all(|s| s.is_finite() && s.abs() <= 0.701));
            }
        }
    }

    #[test]
    fn nonaligned_tone_peak_tracks_pitch_with_short_window_sidebands() {
        fn power(signal: &[f32], frequency: f64) -> f64 {
            let angle = std::f64::consts::TAU * frequency / RATE as f64;
            let coefficient = 2.0 * angle.cos();
            let (mut a, mut b) = (0.0, 0.0);
            for (n, &sample) in signal.iter().enumerate() {
                let window =
                    0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / signal.len() as f64).cos();
                let next = sample as f64 * window + coefficient * a - b;
                b = a;
                a = next;
            }
            a * a + b * b - coefficient * a * b
        }
        let input = tone(16_000, 1000.0);
        for pitch in [-12.0, -3.0, 3.0, 12.0] {
            let ratio = pitch_ratio(pitch).unwrap();
            let expected = 1000.0 * ratio;
            // Repeated two-grain crossfades produce sidebands spaced by this rate.
            let grain_rate = 2.0 * (1.0 - ratio).abs() * RATE as f64 / DELAY_SPAN as f64;
            let mut out = vec![0.0; input.len()];
            PitchShifter::new(pitch, false, 0.0)
                .unwrap()
                .process_block(&input, &mut out);
            let signal = &out[2000..];
            let (peak, peak_power) = (400..=2200)
                .step_by(2)
                .map(|frequency| (frequency as f64, power(signal, frequency as f64)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            assert!(
                (peak - expected).abs() < grain_rate / 2.0 + 4.0,
                "pitch={pitch}: peak={peak}, expected={expected}, spacing={grain_rate}"
            );
            assert!(peak_power > power(signal, 1000.0) * 4.0);
        }
    }

    #[test]
    fn retargeting_mid_ramp_keeps_current_ratio_and_mix() {
        let mut dsp = PitchShifter::new(0.0, false, 0.0).unwrap();
        dsp.set_semitones(12.0).unwrap();
        let mut out = [0.0; 100];
        dsp.process_block(&[0.5_f32; 100], &mut out);
        let (ratio, wet) = (dsp.ratio, dsp.wet);
        dsp.set_semitones(-12.0).unwrap();
        assert_eq!((dsp.ratio, dsp.wet), (ratio, wet));
        dsp.set_semitones(0.0).unwrap();
        assert_eq!((dsp.ratio, dsp.wet), (ratio, wet));
        let mut out = [0.0; SMOOTH_SAMPLES];
        dsp.process_block(&[0.5_f32; SMOOTH_SAMPLES], &mut out);
        assert_eq!((dsp.ratio, dsp.wet), (1.0, 0.0));
        assert!(out[FADE_SAMPLES..].iter().all(|&s| s == 0.5));
    }

    #[test]
    fn parameter_ramps_and_bypass_return_are_bounded_and_exact() {
        let input = tone(4760, 201.6806722689);
        let mut dsp = PitchShifter::new(0.0, false, 0.0).unwrap();
        let mut out = vec![0.0; input.len()];
        dsp.process_block(&input, &mut out);
        for pitch in [3.0, -3.0, 12.0, -12.0, 0.0] {
            let previous = out[out.len() - 1];
            dsp.set_semitones(pitch).unwrap();
            dsp.process_block(&input[..100], &mut out[..100]);
            assert!(dsp.ratio_remaining > 0);
            let remaining = dsp.ratio_remaining;
            dsp.set_semitones(pitch).unwrap();
            assert_eq!(remaining, dsp.ratio_remaining);
            dsp.process_block(&input[100..], &mut out[100..]);
            assert_eq!(dsp.ratio, pitch_ratio(pitch).unwrap());
            assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 0.701));
            // Smooth low-frequency test signal, including head wraps and parameter ramps.
            assert!((out[0] - previous).abs() < 0.08);
            assert!(out.windows(2).all(|w| (w[1] - w[0]).abs() < 0.08));
        }
        assert_eq!(input[FADE_SAMPLES..], out[FADE_SAMPLES..]);
        let extreme = [i16::MIN, -1, 0, 1, i16::MAX];
        let mut out = [0; 5];
        dsp.process_block(&extreme, &mut out);
        assert_eq!(extreme, out);
    }

    #[test]
    fn integer_wet_processing_and_nonfinite_history_remain_safe() {
        assert_eq!(i32::from_normalized(2.0), i32::MAX);
        assert_eq!(i16::from_normalized(-2.0), i16::MIN);
        for value in [i32::MIN, -1, 0, 1, i32::MAX] {
            assert_eq!(i32::from_normalized(value.normalized()), value);
        }
        let mut dsp = PitchShifter::new(3.0, false, 0.0).unwrap();
        let mut out = [0_i32; 1024];
        dsp.process_block(&[i32::MAX; 1024], &mut out);
        assert_eq!(out[600..], [i32::MAX; 424]);
        let mut dsp = PitchShifter::new(3.0, false, 0.0).unwrap();
        let mut input = [0.0_f32; 1024];
        input[0] = f32::NAN;
        input[1] = f32::INFINITY;
        let mut out = [0.0; 1024];
        dsp.process_block(&input, &mut out);
        assert!(out.iter().all(|&s| s == 0.0));
    }
}
