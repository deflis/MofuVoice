//! Monophonic spectral pitch shifting with identity phase locking.
//! All FFT tables, windows, and overlap storage are allocated before streaming.
//! Wet audio uses a long analysis window and a short right-aligned synthesis window,
//! so the fixed delay is SYNTHESIS samples; the neutral setting bypasses exactly.
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
/// Right-aligned synthesis / COLA length (`2M`); wet delay equals this many samples.
pub const SYNTHESIS: usize = 512;
const M: usize = SYNTHESIS / 2;
pub const LATENCY: usize = SYNTHESIS;
const BINS: usize = WINDOW / 2 + 1;
const QUEUE: usize = WINDOW * 2;
const CEPSTRUM: usize = 80;

fn hann(index: usize, length: usize) -> f64 {
    0.5 - 0.5 * (std::f64::consts::TAU * index as f64 / length as f64).cos()
}

fn asymmetric_windows() -> (Box<[f64; WINDOW]>, Box<[f64; WINDOW]>, f64) {
    let mut analysis = Box::new([0.0; WINDOW]);
    let mut synthesis = Box::new([0.0; WINDOW]);
    let left_length = 2 * (WINDOW - M);
    for i in 0..WINDOW - M {
        analysis[i] = hann(i, left_length).sqrt();
    }
    for i in WINDOW - M..WINDOW {
        let short = hann(i - (WINDOW - SYNTHESIS), SYNTHESIS).sqrt();
        analysis[i] = short;
        synthesis[i] = short;
    }
    for i in WINDOW - SYNTHESIS..WINDOW - M {
        let short = hann(i - (WINDOW - SYNTHESIS), SYNTHESIS);
        // Product hA*hS is the right-aligned Hann of length SYNTHESIS.
        synthesis[i] = short / analysis[i];
    }
    let (mut moment, mut energy) = (0.0, 0.0);
    for (i, &weight) in analysis.iter().enumerate() {
        let power = weight * weight;
        moment += i as f64 * power;
        energy += power;
    }
    (analysis, synthesis, moment / energy)
}

#[derive(Clone, Copy, Default)]
struct Complex {
    re: f64,
    im: f64,
}
impl Complex {
    #[inline(always)]
    fn polar(magnitude: f64, phase: f64) -> Self {
        let (sin, cos) = phase.sin_cos();
        Self {
            re: magnitude * cos,
            im: magnitude * sin,
        }
    }

    #[inline(always)]
    fn mul(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }

    #[inline(always)]
    fn scale(self, weight: f64) -> Self {
        Self {
            re: self.re * weight,
            im: self.im * weight,
        }
    }
}

const HALF: usize = WINDOW / 2;

struct Fft {
    reverse_half: Box<[usize; HALF]>,
    forward_roots_half: Box<[Complex; HALF / 2]>,
    inverse_roots_half: Box<[Complex; HALF / 2]>,
    /// `exp(-2π i k / N)` for real-FFT packing, k = 0 .. HALF-1.
    real_twiddle: Box<[Complex; HALF]>,
    #[cfg(test)]
    reverse: Box<[usize; WINDOW]>,
    #[cfg(test)]
    forward_roots: Box<[Complex; WINDOW / 2]>,
    #[cfg(test)]
    inverse_roots: Box<[Complex; WINDOW / 2]>,
}
impl Fft {
    fn new() -> Self {
        let half_bits = HALF.ilog2();
        Self {
            reverse_half: Box::new(std::array::from_fn(|i| {
                i.reverse_bits() >> (usize::BITS - half_bits)
            })),
            forward_roots_half: Box::new(std::array::from_fn(|i| {
                Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / HALF as f64)
            })),
            inverse_roots_half: Box::new(std::array::from_fn(|i| {
                Complex::polar(1.0, std::f64::consts::TAU * i as f64 / HALF as f64)
            })),
            real_twiddle: Box::new(std::array::from_fn(|i| {
                Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / WINDOW as f64)
            })),
            #[cfg(test)]
            reverse: Box::new(std::array::from_fn(|i| {
                i.reverse_bits() >> (usize::BITS - WINDOW.ilog2())
            })),
            #[cfg(test)]
            forward_roots: Box::new(std::array::from_fn(|i| {
                Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / WINDOW as f64)
            })),
            #[cfg(test)]
            inverse_roots: Box::new(std::array::from_fn(|i| {
                Complex::polar(1.0, std::f64::consts::TAU * i as f64 / WINDOW as f64)
            })),
        }
    }

    fn transform_sized<const N: usize>(
        reverse: &[usize; N],
        roots: &[Complex],
        data: &mut [Complex; N],
        scale: Option<f64>,
    ) {
        for i in 0..N {
            let j = reverse[i];
            if i < j {
                data.swap(i, j);
            }
        }
        let mut size = 2;
        while size <= N {
            let half = size / 2;
            let stride = N / size;
            for start in (0..N).step_by(size) {
                for j in 0..half {
                    let root = roots[j * stride];
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
        if let Some(scale) = scale {
            for value in data {
                value.re *= scale;
                value.im *= scale;
            }
        }
    }

    #[cfg(test)]
    fn transform(&self, data: &mut [Complex; WINDOW], inverse: bool) {
        if inverse {
            Self::transform_sized(
                &self.reverse,
                self.inverse_roots.as_ref(),
                data,
                Some(1.0 / WINDOW as f64),
            );
        } else {
            Self::transform_sized(&self.reverse, self.forward_roots.as_ref(), data, None);
        }
    }

    /// Real forward FFT: `time[n].re` is the real sequence (imag ignored).
    fn forward_real(&self, time: &mut [Complex; WINDOW], work: &mut [Complex; HALF]) {
        for n in 0..HALF {
            work[n] = Complex {
                re: time[2 * n].re,
                im: time[2 * n + 1].re,
            };
        }
        Self::transform_sized(
            &self.reverse_half,
            self.forward_roots_half.as_ref(),
            work,
            None,
        );
        let z0 = work[0];
        time[0] = Complex {
            re: z0.re + z0.im,
            im: 0.0,
        };
        time[HALF] = Complex {
            re: z0.re - z0.im,
            im: 0.0,
        };
        for k in 1..HALF {
            let zk = work[k];
            let zm = work[HALF - k];
            let a = Complex {
                re: zk.re + zm.re,
                im: zk.im - zm.im,
            };
            let b = Complex {
                re: zk.re - zm.re,
                im: zk.im + zm.im,
            };
            let w = self.real_twiddle[k];
            let wb_re = w.re * b.re - w.im * b.im;
            let wb_im = w.re * b.im + w.im * b.re;
            let x = Complex {
                re: 0.5 * (a.re + wb_im),
                im: 0.5 * (a.im - wb_re),
            };
            time[k] = x;
            time[WINDOW - k] = Complex {
                re: x.re,
                im: -x.im,
            };
        }
    }

    /// Inverse of a Hermitian spectrum produced by `forward_real` / mirror fill.
    fn inverse_real(&self, spectrum: &mut [Complex; WINDOW], work: &mut [Complex; HALF]) {
        let x0 = spectrum[0].re;
        let xn = spectrum[HALF].re;
        work[0] = Complex {
            re: 0.5 * (x0 + xn),
            im: 0.5 * (x0 - xn),
        };
        for k in 1..HALF {
            let a = spectrum[k];
            let b = Complex {
                re: spectrum[HALF - k].re,
                im: -spectrum[HALF - k].im,
            };
            let e = Complex {
                re: 0.5 * (a.re + b.re),
                im: 0.5 * (a.im + b.im),
            };
            let o = Complex {
                re: 0.5 * (a.re - b.re),
                im: 0.5 * (a.im - b.im),
            };
            let w = self.real_twiddle[k];
            let wr = w.re;
            let wi = -w.im;
            let t_re = -wi;
            let t_im = wr;
            work[k] = Complex {
                re: e.re + t_re * o.re - t_im * o.im,
                im: e.im + t_re * o.im + t_im * o.re,
            };
        }
        Self::transform_sized(
            &self.reverse_half,
            self.inverse_roots_half.as_ref(),
            work,
            Some(1.0 / HALF as f64),
        );
        for n in 0..HALF {
            spectrum[2 * n] = Complex {
                re: work[n].re,
                im: 0.0,
            };
            spectrum[2 * n + 1] = Complex {
                re: work[n].im,
                im: 0.0,
            };
        }
    }
}

fn wrap(phase: f64) -> f64 {
    (phase + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

pub struct PitchShifter {
    fft: Fft,
    analysis_window: Box<[f64; WINDOW]>,
    /// Synthesis window already scaled by COLA normalization `2H/SYNTHESIS`.
    synthesis_window: Box<[f64; WINDOW]>,
    phase_center: f64,
    /// `exp(-i * centre_phase(bin))` for each analysis bin.
    centre_phasor: Box<[Complex; BINS]>,
    /// Nominal hop phase advance `2π · hop · bin / N` per bin.
    nominal_advance: Box<[f64; BINS]>,
    input: Box<[f64; WINDOW]>,
    output: Box<[f64; QUEUE]>,
    spectrum: Box<[Complex; WINDOW]>,
    shifted: Box<[Complex; WINDOW]>,
    fft_work: Box<[Complex; HALF]>,
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
        let (analysis_window, mut synthesis_window, phase_center) = asymmetric_windows();
        let normalization = 2.0 * HOP as f64 / SYNTHESIS as f64;
        for weight in synthesis_window.iter_mut() {
            *weight *= normalization;
        }
        let centre_step = std::f64::consts::TAU * phase_center / WINDOW as f64;
        let centre_phasor = Box::new(std::array::from_fn(|bin| {
            Complex::polar(1.0, -centre_step * bin as f64)
        }));
        let nominal_advance = Box::new(std::array::from_fn(|bin| {
            std::f64::consts::TAU * HOP as f64 * bin as f64 / WINDOW as f64
        }));
        Ok(Self {
            fft: Fft::new(),
            analysis_window,
            synthesis_window,
            phase_center,
            centre_phasor,
            nominal_advance,
            input: Box::new([0.0; WINDOW]),
            output: Box::new([0.0; QUEUE]),
            spectrum: Box::new([Complex::default(); WINDOW]),
            shifted: Box::new([Complex::default(); WINDOW]),
            fft_work: Box::new([Complex::default(); HALF]),
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
        let clock = self.clock;
        for i in 0..WINDOW {
            self.spectrum[i] = Complex {
                re: self.input[(clock + i) % WINDOW] * self.analysis_window[i],
                im: 0.0,
            };
        }
        self.fft
            .forward_real(&mut self.spectrum, &mut self.fft_work);
        for i in 0..BINS {
            let value = self.spectrum[i];
            self.magnitude[i] = (value.re * value.re + value.im * value.im).sqrt();
            self.phase[i] = value.im.atan2(value.re);
            let advance = self.nominal_advance[i];
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
            self.fft
                .inverse_real(&mut self.envelope_work, &mut self.fft_work);
            self.envelope_work[CEPSTRUM + 1..WINDOW - CEPSTRUM].fill(Complex::default());
            self.fft
                .forward_real(&mut self.envelope_work, &mut self.fft_work);
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
        let centre_step = std::f64::consts::TAU * self.phase_center / WINDOW as f64;
        for &peak in &self.peaks[..count] {
            // Store phase at the product-Hann centre. A moving peak must retain its
            // oscillator phase, rather than adopt the unrelated accumulator of its new bin.
            let input_phase = wrap(self.phase[peak] + centre_step * peak as f64);
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
        let bin_scale = WINDOW as f64 / (std::f64::consts::TAU * HOP as f64);
        let ratio_delta = self.ratio - 1.0;
        let preserve = self.preserve_formants;
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
            let frequency_bin = self.phase_advance[peak] * bin_scale;
            // Shift continuously by measured frequency, including fractional bins.
            let destination = i as f64 + frequency_bin * ratio_delta;
            let lower = destination.floor() as isize;
            let fraction = destination - lower as f64;
            let gain = if preserve {
                let position = (destination / self.formant_ratio).clamp(0.0, (BINS - 1) as f64);
                let env_lower = position.floor() as usize;
                let env_fraction = position - env_lower as f64;
                let desired = self.envelope[env_lower] * (1.0 - env_fraction)
                    + self.envelope[(env_lower + 1).min(BINS - 1)] * env_fraction;
                // Bound envelope correction, rather than clipping the synthesis waveform.
                (desired - self.envelope[i]).clamp(-2.0, 2.0).exp()
            } else {
                1.0
            };
            let locked_phase = if count == 0 {
                self.phase[peak] + centre_step * peak as f64
            } else {
                self.synthesis_phase[peak]
            };
            let phase = locked_phase
                + wrap(self.phase[i] - self.phase[peak] + centre_step * (i as f64 - peak as f64));
            let phasor = Complex::polar(self.magnitude[i] * gain, phase);
            for (bin, weight) in [(lower, 1.0 - fraction), (lower + 1, fraction)] {
                if bin <= 0 || bin >= (BINS - 1) as isize || weight == 0.0 {
                    continue;
                }
                let bin = bin as usize;
                let value = phasor.mul(self.centre_phasor[bin]).scale(weight);
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
        self.fft.inverse_real(&mut self.shifted, &mut self.fft_work);
        let start = WINDOW - SYNTHESIS;
        for i in start..WINDOW {
            self.output[(clock + i - start) % QUEUE] +=
                self.shifted[i].re * self.synthesis_window[i];
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
    fn real_fft_matches_complex_fft_and_round_trips() {
        let fft = Fft::new();
        let samples: [f64; WINDOW] =
            std::array::from_fn(|i| (i as f64 * 0.31).sin() + 0.2 * (i as f64 * 0.07).cos());
        let mut reference = std::array::from_fn(|i| Complex {
            re: samples[i],
            im: 0.0,
        });
        fft.transform(&mut reference, false);
        let mut packed = std::array::from_fn(|i| Complex {
            re: samples[i],
            im: 0.0,
        });
        let mut work = [Complex::default(); HALF];
        fft.forward_real(&mut packed, &mut work);
        for i in 0..BINS {
            assert!((packed[i].re - reference[i].re).abs() < 1e-9);
            assert!((packed[i].im - reference[i].im).abs() < 1e-9);
        }
        fft.inverse_real(&mut packed, &mut work);
        for i in 0..WINDOW {
            assert!((packed[i].re - samples[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn asymmetric_windows_product_is_right_aligned_hann() {
        let (analysis, synthesis, phase_center) = asymmetric_windows();
        assert!((1330.0..1340.0).contains(&phase_center), "{phase_center}");
        for i in 0..WINDOW - SYNTHESIS {
            assert_eq!(synthesis[i], 0.0);
        }
        for i in WINDOW - SYNTHESIS..WINDOW {
            let product = analysis[i] * synthesis[i];
            let expected = hann(i - (WINDOW - SYNTHESIS), SYNTHESIS);
            assert!(
                (product - expected).abs() < 1e-12,
                "product[{i}]={product} expected={expected}"
            );
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
