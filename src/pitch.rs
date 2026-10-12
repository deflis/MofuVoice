//! Monophonic spectral pitch shifting with identity phase locking.
//! All FFT tables, windows, and overlap storage are allocated before streaming.
//! Wet audio has a fixed WINDOW-sample delay; the neutral setting bypasses exactly.
use std::{error::Error, fmt};

pub const SMOOTH_SAMPLES: usize = 960;
pub const FADE_SAMPLES: usize = 480;

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

pub const WINDOW: usize = 2048;
pub const HOP: usize = 128;
pub const LATENCY: usize = WINDOW;
const BINS: usize = WINDOW / 2 + 1;
const QUEUE: usize = WINDOW * 2;
const CEPSTRUM: usize = 80;

#[derive(Clone, Copy, Default)]
struct Complex {
    re: f64,
    im: f64,
}
impl Complex {
    fn polar(magnitude: f64, phase: f64) -> Self {
        Self {
            re: magnitude * phase.cos(),
            im: magnitude * phase.sin(),
        }
    }
}

struct Fft {
    reverse: Box<[usize; WINDOW]>,
    roots: Box<[Complex; WINDOW / 2]>,
}
impl Fft {
    fn new() -> Self {
        Self {
            reverse: Box::new(std::array::from_fn(|i| {
                i.reverse_bits() >> (usize::BITS - WINDOW.ilog2())
            })),
            roots: Box::new(std::array::from_fn(|i| {
                Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / WINDOW as f64)
            })),
        }
    }
    fn transform(&self, data: &mut [Complex; WINDOW], inverse: bool) {
        for i in 0..WINDOW {
            if i < self.reverse[i] {
                data.swap(i, self.reverse[i]);
            }
        }
        let mut size = 2;
        while size <= WINDOW {
            let half = size / 2;
            for start in (0..WINDOW).step_by(size) {
                for j in 0..half {
                    let mut root = self.roots[j * WINDOW / size];
                    if inverse {
                        root.im = -root.im;
                    }
                    let a = data[start + j];
                    let b = data[start + j + half];
                    let re = b.re * root.re - b.im * root.im;
                    let im = b.re * root.im + b.im * root.re;
                    data[start + j] = Complex {
                        re: a.re + re,
                        im: a.im + im,
                    };
                    data[start + j + half] = Complex {
                        re: a.re - re,
                        im: a.im - im,
                    };
                }
            }
            size *= 2;
        }
        if inverse {
            for value in data {
                value.re /= WINDOW as f64;
                value.im /= WINDOW as f64;
            }
        }
    }
}

fn wrap(phase: f64) -> f64 {
    (phase + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

pub struct PitchShifter {
    fft: Fft,
    window: Box<[f64; WINDOW]>,
    input: Box<[f64; WINDOW]>,
    output: Box<[f64; QUEUE]>,
    spectrum: Box<[Complex; WINDOW]>,
    shifted: Box<[Complex; WINDOW]>,
    envelope_work: Box<[Complex; WINDOW]>,
    magnitude: Box<[f64; BINS]>,
    phase: Box<[f64; BINS]>,
    previous_phase: Box<[f64; BINS]>,
    synthesis_phase: Box<[f64; BINS]>,
    previous_synthesis_phase: Box<[f64; BINS]>,
    phase_advance: Box<[f64; BINS]>,
    envelope: Box<[f64; BINS]>,
    peaks: Box<[usize; BINS]>,
    previous_peaks: Box<[usize; BINS]>,
    previous_peak_count: usize,
    clock: usize,
    until_frame: usize,
    ratio: f64,
    target_ratio: f64,
    ratio_step: f64,
    remaining: usize,
    semitones: f64,
    preserve_formants: bool,
    formant_ratio: f64,
    formant_shift: f64,
    wet: f64,
    target_wet: f64,
}

impl PitchShifter {
    pub fn new(
        semitones: f64,
        preserve_formants: bool,
        formant_shift: f64,
    ) -> Result<Self, Box<dyn Error>> {
        let ratio = pitch_ratio(semitones)?;
        let formant_ratio = formant_ratio(formant_shift)?;
        if !preserve_formants && formant_shift != 0.0 {
            return Err(InvalidFormant.into());
        }
        let wet = f64::from(semitones != 0.0 || formant_shift != 0.0);
        Ok(Self {
            fft: Fft::new(),
            window: Box::new(std::array::from_fn(|i| {
                0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / WINDOW as f64).cos()
            })),
            input: Box::new([0.0; WINDOW]),
            output: Box::new([0.0; QUEUE]),
            spectrum: Box::new([Complex::default(); WINDOW]),
            shifted: Box::new([Complex::default(); WINDOW]),
            envelope_work: Box::new([Complex::default(); WINDOW]),
            magnitude: Box::new([0.0; BINS]),
            phase: Box::new([0.0; BINS]),
            previous_phase: Box::new([0.0; BINS]),
            synthesis_phase: Box::new([0.0; BINS]),
            previous_synthesis_phase: Box::new([0.0; BINS]),
            phase_advance: Box::new([0.0; BINS]),
            envelope: Box::new([0.0; BINS]),
            peaks: Box::new([0; BINS]),
            previous_peaks: Box::new([0; BINS]),
            previous_peak_count: 0,
            clock: 0,
            until_frame: HOP,
            ratio,
            target_ratio: ratio,
            ratio_step: 0.0,
            remaining: 0,
            semitones,
            preserve_formants,
            formant_ratio,
            formant_shift,
            wet,
            target_wet: wet,
        })
    }

    pub fn set_semitones(&mut self, semitones: f64) -> Result<(), InvalidPitch> {
        let ratio = pitch_ratio(semitones)?;
        if semitones == self.semitones {
            return Ok(());
        }
        self.semitones = semitones;
        self.target_ratio = ratio;
        self.ratio_step = (ratio - self.ratio) / SMOOTH_SAMPLES as f64;
        self.remaining = SMOOTH_SAMPLES;
        self.target_wet = f64::from(semitones != 0.0 || self.formant_shift != 0.0);
        Ok(())
    }

    fn frame(&mut self) {
        for i in 0..WINDOW {
            self.spectrum[i] = Complex {
                re: self.input[(self.clock + i) % WINDOW] * self.window[i],
                im: 0.0,
            };
        }
        self.fft.transform(&mut self.spectrum, false);
        for i in 0..BINS {
            let value = self.spectrum[i];
            self.magnitude[i] = value.re.hypot(value.im);
            self.phase[i] = value.im.atan2(value.re);
            let advance = std::f64::consts::TAU * HOP as f64 * i as f64 / WINDOW as f64;
            let frequency = advance + wrap(self.phase[i] - self.previous_phase[i] - advance);
            self.phase_advance[i] = frequency;
            self.previous_phase[i] = self.phase[i];
        }
        if self.preserve_formants {
            // Real cepstral low-pass estimates a smooth envelope without fitting harmonics.
            for i in 0..WINDOW {
                let bin = i.min(WINDOW - i);
                self.envelope_work[i] = Complex {
                    re: self.magnitude[bin].max(1e-12).ln(),
                    im: 0.0,
                };
            }
            self.fft.transform(&mut self.envelope_work, true);
            self.envelope_work[CEPSTRUM + 1..WINDOW - CEPSTRUM].fill(Complex::default());
            self.fft.transform(&mut self.envelope_work, false);
            for i in 0..BINS {
                self.envelope[i] = self.envelope_work[i].re;
            }
        }
        let mut count = 0;
        for i in 1..BINS - 1 {
            if self.magnitude[i] > self.magnitude[i - 1]
                && self.magnitude[i] >= self.magnitude[i + 1]
            {
                self.peaks[count] = i;
                count += 1;
            }
        }
        self.previous_synthesis_phase
            .copy_from_slice(self.synthesis_phase.as_slice());
        let mut previous = 0;
        for &peak in &self.peaks[..count] {
            // Store phase at the window centre, removing the alternating pi phase
            // of adjacent FFT bins. A moving peak must retain its oscillator phase,
            // rather than adopt the unrelated accumulator of its new bin.
            let input_phase = wrap(self.phase[peak] + std::f64::consts::PI * peak as f64);
            while previous + 1 < self.previous_peak_count
                && self.previous_peaks[previous + 1].abs_diff(peak)
                    < self.previous_peaks[previous].abs_diff(peak)
            {
                previous += 1;
            }
            self.synthesis_phase[peak] = if self.ratio == 1.0
                || self.previous_peak_count == 0
                || self.previous_peaks[previous].abs_diff(peak) > 2
            {
                input_phase
            } else {
                wrap(
                    self.previous_synthesis_phase[self.previous_peaks[previous]]
                        + self.phase_advance[peak] * self.ratio,
                )
            };
        }
        self.previous_peaks[..count].copy_from_slice(&self.peaks[..count]);
        self.previous_peak_count = count;
        self.shifted.fill(Complex::default());
        self.shifted[0] = self.spectrum[0];
        if self.ratio == 1.0 {
            self.shifted[BINS - 1] = self.spectrum[BINS - 1];
        }
        let dc_lobe = self.magnitude[0] > self.magnitude[1];
        if dc_lobe {
            self.shifted[1] = self.spectrum[1];
        }
        let mut region = 0;
        for i in 1..BINS - 1 {
            if dc_lobe && i == 1 {
                continue;
            }
            while region + 1 < count && 2 * i > self.peaks[region] + self.peaks[region + 1] {
                region += 1;
            }
            let peak = if count == 0 { i } else { self.peaks[region] };
            // Translate each peak's entire main lobe together. Scaling its bin offsets
            // would broaden the lobe and recreate low-frequency modulation products.
            let frequency_bin =
                self.phase_advance[peak] * WINDOW as f64 / (std::f64::consts::TAU * HOP as f64);
            // Shift continuously by measured frequency, including fractional bins.
            let destination = i as f64 + frequency_bin * (self.ratio - 1.0);
            let lower = destination.floor() as isize;
            let fraction = destination - lower as f64;
            let gain = if self.preserve_formants {
                let position = (destination / self.formant_ratio).clamp(0.0, (BINS - 1) as f64);
                let lower = position.floor() as usize;
                let fraction = position - lower as f64;
                let desired = self.envelope[lower] * (1.0 - fraction)
                    + self.envelope[(lower + 1).min(BINS - 1)] * fraction;
                // Bound envelope correction, rather than clipping the synthesis waveform.
                (desired - self.envelope[i]).clamp(-2.0, 2.0).exp()
            } else {
                1.0
            };
            let centre_phase = if count == 0 {
                self.phase[peak] + std::f64::consts::PI * peak as f64
            } else {
                self.synthesis_phase[peak]
            };
            let phase = centre_phase
                + wrap(
                    self.phase[i] - self.phase[peak]
                        + std::f64::consts::PI * (i as f64 - peak as f64),
                );
            for (bin, weight) in [(lower, 1.0 - fraction), (lower + 1, fraction)] {
                if bin <= 0 || bin >= (BINS - 1) as isize {
                    continue;
                }
                let bin = bin as usize;
                let value = Complex::polar(
                    self.magnitude[i] * gain * weight,
                    phase - std::f64::consts::PI * bin as f64,
                );
                self.shifted[bin].re += value.re;
                self.shifted[bin].im += value.im;
            }
        }
        for i in 1..BINS - 1 {
            self.shifted[WINDOW - i] = Complex {
                re: self.shifted[i].re,
                im: -self.shifted[i].im,
            };
        }
        self.fft.transform(&mut self.shifted, true);
        // Periodic Hann squared sums to 3N/(8H), independent of sample position.
        let normalization = 8.0 * HOP as f64 / (3.0 * WINDOW as f64);
        for i in 0..WINDOW {
            self.output[(self.clock + i) % QUEUE] +=
                self.shifted[i].re * self.window[i] * normalization;
        }
    }

    /// Equal-length blocks; callback storage and FFT tables are never allocated here.
    pub fn process_block<T: Sample>(&mut self, input: &[T], output: &mut [T]) -> u64 {
        assert_eq!(input.len(), output.len());
        let mut nonzero = 0;
        for (&sample, out) in input.iter().zip(output) {
            nonzero += u64::from(sample != T::default());
            let value = sample.normalized();
            let dry = if value.is_finite() { value } else { 0.0 };
            self.input[self.clock % WINDOW] = dry;
            let wet = self.output[self.clock % QUEUE];
            self.output[self.clock % QUEUE] = 0.0;
            if self.remaining != 0 {
                self.ratio += self.ratio_step;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.ratio = self.target_ratio;
                }
            }
            let step = 1.0 / FADE_SAMPLES as f64;
            if self.wet < self.target_wet {
                self.wet = (self.wet + step).min(self.target_wet);
            }
            if self.wet > self.target_wet {
                self.wet = (self.wet - step).max(self.target_wet);
            }
            *out = if self.wet == 0.0 {
                sample
            } else {
                T::from_normalized((dry * (1.0 - self.wet) + wet * self.wet).clamp(-1.0, 1.0))
            };
            self.clock = (self.clock + 1) % QUEUE;
            self.until_frame -= 1;
            if self.until_frame == 0 {
                self.frame();
                self.until_frame = HOP;
            }
        }
        nonzero
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_round_trip_preserves_real_and_imaginary_samples() {
        let fft = Fft::new();
        let original = std::array::from_fn(|i| Complex {
            re: (i as f64 * 0.31).sin(),
            im: (i as f64 * 0.17).cos(),
        });
        let mut data = original;
        fft.transform(&mut data, false);
        fft.transform(&mut data, true);
        for (expected, actual) in original.iter().zip(data) {
            assert!((expected.re - actual.re).abs() < 1e-12);
            assert!((expected.im - actual.im).abs() < 1e-12);
        }
    }

    #[test]
    fn neutral_wet_path_reconstructs_at_declared_latency() {
        let mut dsp = PitchShifter::new(0.0, false, 0.0).unwrap();
        // Exercise the transform, rather than the public neutral bypass.
        dsp.wet = 1.0;
        dsp.target_wet = 1.0;
        let mut input: Vec<f32> = (0..12_000)
            .map(|i| (0.13 * (i as f64 * 0.03).sin() + 0.05 * (i as f64 * 0.13).cos()) as f32)
            .collect();
        input[3072] = 0.7;
        let mut output = vec![0.0; input.len()];
        dsp.process_block(&input, &mut output);
        assert!(output[..LATENCY].iter().all(|x| x.abs() < 1e-8));
        for (&expected, &actual) in input.iter().zip(&output[LATENCY..]) {
            assert!((expected - actual).abs() < 1e-6, "{expected} != {actual}");
        }
    }
}
