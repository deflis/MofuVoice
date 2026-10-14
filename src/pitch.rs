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
pub const HOP: usize = 64;
/// Right-aligned synthesis / COLA length; wet delay equals this many samples.
pub const SYNTHESIS: usize = 256;
const INTERPOLATION_TAPS: usize = 16;
pub const LATENCY: usize = SYNTHESIS;
const BINS: usize = WINDOW / 2 + 1;
const QUEUE: usize = WINDOW * 2;
const CEPSTRUM: usize = 80;

// Barycentric form computes the same 15th-order Lagrange kernel in O(taps).
// The integer case avoids its removable singularity and preserves reconstruction.
fn translation_weights(fraction: f64) -> [f64; INTERPOLATION_TAPS] {
    if fraction == 0.0 {
        let mut weights = [0.0; INTERPOLATION_TAPS];
        weights[7] = 1.0;
        return weights;
    }
    let mut product = 1.0;
    for node in -7..=8 {
        product *= fraction - node as f64;
    }
    const DENOMINATORS: [f64; INTERPOLATION_TAPS] = {
        let mut values = [1.0; INTERPOLATION_TAPS];
        let mut t = 0;
        while t < INTERPOLATION_TAPS {
            let mut j = 1;
            while j <= t {
                values[t] *= j as f64;
                j += 1;
            }
            j = 1;
            while j < INTERPOLATION_TAPS - t {
                values[t] *= -(j as f64);
                j += 1;
            }
            t += 1;
        }
        values
    };
    std::array::from_fn(|t| product / ((fraction - (t as f64 - 7.0)) * DENOMINATORS[t]))
}

fn hann(index: usize, length: usize) -> f64 {
    0.5 - 0.5 * (std::f64::consts::TAU * index as f64 / length as f64).cos()
}

fn asymmetric_windows() -> (Box<[f64; WINDOW]>, Box<[f64; WINDOW]>, f64) {
    let mut analysis = Box::new([0.0; WINDOW]);
    let mut synthesis = Box::new([0.0; WINDOW]);
    // Smooth the analysis taper independently of the output support: the old
    // short tail leaks a negative-frequency image into low-voice peaks.
    const ANALYSIS_TAIL: usize = 768;
    let left_length = 2 * (WINDOW - ANALYSIS_TAIL);
    for i in 0..WINDOW - ANALYSIS_TAIL {
        analysis[i] = hann(i, left_length).sqrt();
    }
    for i in WINDOW - ANALYSIS_TAIL..WINDOW {
        analysis[i] = hann(i - (WINDOW - 2 * ANALYSIS_TAIL), 2 * ANALYSIS_TAIL).sqrt();
    }
    for i in WINDOW - SYNTHESIS..WINDOW {
        synthesis[i] = hann(i - (WINDOW - SYNTHESIS), SYNTHESIS) / analysis[i];
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
#[repr(C)]
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
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    avx2: bool,
    reverse_half: Box<[usize; HALF]>,
    forward_roots_half: Box<[Complex; HALF - 1]>,
    inverse_roots_half: Box<[Complex; HALF - 1]>,
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
    // Store each stage's twiddles contiguously, in the same order used by the
    // butterflies. This removes strided loads and permits vectorization.
    fn stage_roots<const N: usize>(roots: &[Complex]) -> Box<[Complex; N]> {
        Box::new(std::array::from_fn(|i| {
            let half = 1 << (i + 1).ilog2();
            roots[(i - (half - 1)) * (N + 1) / (2 * half)]
        }))
    }

    fn new() -> Self {
        let half_bits = HALF.ilog2();
        let forward: [Complex; HALF / 2] = std::array::from_fn(|i| {
            Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / HALF as f64)
        });
        let inverse: [Complex; HALF / 2] = std::array::from_fn(|i| {
            Complex::polar(1.0, std::f64::consts::TAU * i as f64 / HALF as f64)
        });
        Self {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            avx2: std::is_x86_feature_detected!("avx2"),
            reverse_half: Box::new(std::array::from_fn(|i| {
                i.reverse_bits() >> (usize::BITS - half_bits)
            })),
            forward_roots_half: Self::stage_roots(&forward),
            inverse_roots_half: Self::stage_roots(&inverse),
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
        &self,
        reverse: &[usize; N],
        roots: &[Complex],
        data: &mut [Complex; N],
        scale: Option<f64>,
    ) {
        for (i, &j) in reverse.iter().enumerate() {
            if i < j {
                data.swap(i, j);
            }
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if self.avx2 {
            // SAFETY: detected once before streaming; slices are checked by each stage.
            unsafe { Self::stages_avx2(roots, data) };
        } else {
            Self::stages(roots, data);
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        Self::stages(roots, data);
        if let Some(scale) = scale {
            for value in data {
                value.re *= scale;
                value.im *= scale;
            }
        }
    }

    fn stages<const N: usize>(roots: &[Complex], data: &mut [Complex; N]) {
        // Fixed stage sizes let the compiler specialize strides and butterfly
        // loops for our power-of-two transforms instead of dividing at runtime.
        Self::stage::<N, 2>(roots, data);
        Self::stage::<N, 4>(roots, data);
        Self::stage::<N, 8>(roots, data);
        Self::stage::<N, 16>(roots, data);
        Self::stage::<N, 32>(roots, data);
        Self::stage::<N, 64>(roots, data);
        Self::stage::<N, 128>(roots, data);
        Self::stage::<N, 256>(roots, data);
        Self::stage::<N, 512>(roots, data);
        Self::stage::<N, 1024>(roots, data);
        Self::stage::<N, 2048>(roots, data);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn stages_avx2<const N: usize>(roots: &[Complex], data: &mut [Complex; N]) {
        Self::stage::<N, 2>(roots, data);
        // SAFETY: this function requires AVX2 and each stage checks its bounds.
        unsafe {
            Self::stage_avx2::<N, 4>(roots, data);
            Self::stage_avx2::<N, 8>(roots, data);
            Self::stage_avx2::<N, 16>(roots, data);
            Self::stage_avx2::<N, 32>(roots, data);
            Self::stage_avx2::<N, 64>(roots, data);
            Self::stage_avx2::<N, 128>(roots, data);
            Self::stage_avx2::<N, 256>(roots, data);
            Self::stage_avx2::<N, 512>(roots, data);
            Self::stage_avx2::<N, 1024>(roots, data);
            Self::stage_avx2::<N, 2048>(roots, data);
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn stage_avx2<const N: usize, const SIZE: usize>(
        roots: &[Complex],
        data: &mut [Complex; N],
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;
        if SIZE > N {
            return;
        }
        let half = SIZE / 2;
        let roots = &roots[half - 1..SIZE - 1];
        for block in data.as_chunks_mut::<SIZE>().0 {
            let (left, right) = block.split_at_mut(half);
            for ((left, right), root) in left
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(right.as_chunks_mut::<2>().0)
                .zip(roots.as_chunks::<2>().0)
            {
                // SAFETY: repr(C) Complex is two f64 values. Each checked chunk
                // contains two complexes (four f64s); unaligned loads/stores stay
                // inside disjoint left/right slices. No FMA changes rounding.
                unsafe {
                    let a = _mm256_loadu_pd(left.as_ptr().cast());
                    let b = _mm256_loadu_pd(right.as_ptr().cast());
                    let w = _mm256_loadu_pd(root.as_ptr().cast());
                    let re = _mm256_mul_pd(_mm256_permute_pd::<0>(b), w);
                    let im = _mm256_mul_pd(_mm256_permute_pd::<15>(b), _mm256_permute_pd::<5>(w));
                    let product = _mm256_addsub_pd(re, im);
                    _mm256_storeu_pd(left.as_mut_ptr().cast(), _mm256_add_pd(a, product));
                    _mm256_storeu_pd(right.as_mut_ptr().cast(), _mm256_sub_pd(a, product));
                }
            }
        }
    }

    #[inline(always)]
    fn stage<const N: usize, const SIZE: usize>(roots: &[Complex], data: &mut [Complex; N]) {
        if SIZE <= N {
            let half = SIZE / 2;
            let roots = &roots[half - 1..SIZE - 1];
            for block in data.as_chunks_mut::<SIZE>().0 {
                let (left, right) = block.split_at_mut(half);
                for ((left, right), root) in left.iter_mut().zip(right).zip(roots) {
                    let a = *left;
                    let b = *right;
                    let re = b.re * root.re - b.im * root.im;
                    let im = b.re * root.im + b.im * root.re;
                    *left = Complex {
                        re: a.re + re,
                        im: a.im + im,
                    };
                    *right = Complex {
                        re: a.re - re,
                        im: a.im - im,
                    };
                }
            }
        }
    }

    #[cfg(test)]
    fn transform(&self, data: &mut [Complex; WINDOW], inverse: bool) {
        if inverse {
            self.transform_sized(
                &self.reverse,
                Self::stage_roots::<{ WINDOW - 1 }>(self.inverse_roots.as_ref()).as_ref(),
                data,
                Some(1.0 / WINDOW as f64),
            );
        } else {
            self.transform_sized(
                &self.reverse,
                Self::stage_roots::<{ WINDOW - 1 }>(self.forward_roots.as_ref()).as_ref(),
                data,
                None,
            );
        }
    }

    /// Real forward FFT: `time[n].re` is the real sequence (imag ignored).
    /// Writes only the nonnegative-frequency bins; inverse_real needs no mirror.
    fn forward_real(&self, time: &mut [Complex; WINDOW], work: &mut [Complex; HALF]) {
        for n in 0..HALF {
            work[n] = Complex {
                re: time[2 * n].re,
                im: time[2 * n + 1].re,
            };
        }
        self.transform_sized(
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
        self.transform_sized(
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
    // Internal phases are bounded (bin advances, window-centre offsets and
    // wrapped accumulators). Quotient reduction avoids a floating remainder
    // call for every analysis bin; roundoff is far below the phase tolerance.
    let shifted = phase + std::f64::consts::PI;
    let period = std::f64::consts::TAU;
    let mut remainder = shifted - (shifted / period).floor() * period;
    if remainder < 0.0 {
        remainder += period;
    } else if remainder >= period {
        remainder -= period;
    }
    remainder - std::f64::consts::PI
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
    /// Whether a bin's phase was calculated in the immediately preceding frame.
    phase_valid: Box<[bool; BINS]>,
    previous_spectrum: Box<[Complex; BINS]>,
    #[cfg(test)]
    eager_phase: bool,
    #[cfg(test)]
    previous_phase: Box<[f64; BINS]>,
    synthesis_phase: Box<[f64; BINS]>,
    previous_synthesis_phase: Box<[f64; BINS]>,
    correction_frequency: f64,
    filtered_advance: Box<[f64; BINS]>,
    previous_filtered_advance: Box<[f64; BINS]>,
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
            phase_valid: Box::new([true; BINS]),
            previous_spectrum: Box::new([Complex::default(); BINS]),
            #[cfg(test)]
            eager_phase: false,
            #[cfg(test)]
            previous_phase: Box::new([0.0; BINS]),
            synthesis_phase: Box::new([0.0; BINS]),
            previous_synthesis_phase: Box::new([0.0; BINS]),
            correction_frequency: 0.0,
            filtered_advance: Box::new([0.0; BINS]),
            previous_filtered_advance: Box::new([0.0; BINS]),
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

    /// A real low tone contributes both C*W(k-f) and conj(C)*W(k+f).
    /// Fit C at its strongest bin and subtract the negative-frequency image
    /// before phase tracking. This avoids the f*(ratio-2) sideband amplified
    /// by the short synthesis taper. Both FFT work buffers are reused.
    fn remove_low_frequency_image(&mut self) {
        if self.ratio != 1.0 {
            let peak = (1..BINS - 1)
                .max_by(|&a, &b| self.magnitude[a].total_cmp(&self.magnitude[b]))
                .unwrap();
            // Only overlapping low-frequency lobes need image cancellation.
            if peak as f64 * crate::RATE as f64 / (WINDOW as f64) < 500.0 {
                let frequency = if (self.correction_frequency - peak as f64).abs() <= 2.0
                    && self.correction_frequency > 0.0
                {
                    self.correction_frequency
                } else {
                    peak as f64
                };
                let step = Complex::polar(1.0, std::f64::consts::TAU * frequency / WINDOW as f64);
                let mut oscillator = Complex { re: 1.0, im: 0.0 };
                for i in 0..WINDOW {
                    self.envelope_work[i] = Complex {
                        re: self.analysis_window[i] * oscillator.re,
                        im: 0.0,
                    };
                    self.shifted[i] = Complex {
                        re: self.analysis_window[i] * oscillator.im,
                        im: 0.0,
                    };
                    oscillator = oscillator.mul(step);
                }
                self.fft
                    .forward_real(&mut self.envelope_work, &mut self.fft_work);
                self.fft.forward_real(&mut self.shifted, &mut self.fft_work);
                let c = self.envelope_work[peak];
                let s = self.shifted[peak];
                let positive = Complex {
                    re: c.re - s.im,
                    im: c.im + s.re,
                };
                let negative = Complex {
                    re: c.re + s.im,
                    im: c.im - s.re,
                };
                let x = self.spectrum[peak];
                let a = x.mul(Complex {
                    re: positive.re,
                    im: -positive.im,
                });
                let b = Complex {
                    re: x.re,
                    im: -x.im,
                }
                .mul(negative);
                let denominator = positive.re * positive.re + positive.im * positive.im
                    - negative.re * negative.re
                    - negative.im * negative.im;
                if denominator > 1.0 {
                    let coefficient = Complex {
                        re: (a.re - b.re) / denominator,
                        im: -(a.im - b.im) / denominator,
                    };
                    for i in 0..BINS {
                        let c = self.envelope_work[i];
                        let s = self.shifted[i];
                        let image = coefficient.mul(Complex {
                            re: c.re + s.im,
                            im: c.im - s.re,
                        });
                        self.spectrum[i].re -= image.re;
                        self.spectrum[i].im -= image.im;
                        let x = self.spectrum[i];
                        self.magnitude[i] = (x.re * x.re + x.im * x.im).sqrt();
                    }
                }
            }
        }
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
        }
        self.remove_low_frequency_image();
        if self.preserve_formants {
            // Real cepstral low-pass estimates a smooth envelope without fitting harmonics.
            for i in 0..BINS {
                self.envelope_work[i] = Complex {
                    re: self.magnitude[i].max(1e-12).ln(),
                    im: 0.0,
                };
            }
            // inverse_real reads only the nonnegative-frequency bins.
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
        // Only peak phases drive lobe translation and phase locking. Retain the
        // complex history so a newly detected/moving peak still gets the exact
        // previous-frame phase of its current bin. No magnitude threshold or
        // approximate angle is needed, including for quiet speech and noise.
        #[cfg(test)]
        let eager = self.eager_phase;
        #[cfg(not(test))]
        let eager = false;
        if eager {
            #[cfg(test)]
            for i in 0..BINS {
                let value = self.spectrum[i];
                self.phase[i] = value.im.atan2(value.re);
                let advance = self.nominal_advance[i];
                self.phase_advance[i] =
                    advance + wrap(self.phase[i] - self.previous_phase[i] - advance);
                self.previous_phase[i] = self.phase[i];
            }
        } else if count == 0 {
            // A spectrum without local maxima uses each bin as its own peak.
            for i in 1..BINS - 1 {
                self.analyze_phase(i);
            }
        } else {
            for p in 0..count {
                self.analyze_phase(self.peaks[p]);
            }
        }
        self.previous_spectrum
            .copy_from_slice(&self.spectrum[..BINS]);
        self.phase_valid.fill(count == 0 || eager);
        for &peak in &self.peaks[..count] {
            self.phase_valid[peak] = true;
        }
        self.previous_synthesis_phase
            .copy_from_slice(self.synthesis_phase.as_slice());
        self.previous_filtered_advance
            .copy_from_slice(self.filtered_advance.as_slice());
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
            let continuing =
                self.previous_peak_count != 0 && self.previous_peaks[previous].abs_diff(peak) <= 2;
            self.filtered_advance[peak] = if continuing {
                // Smooth the frequency estimate, without buffering output audio.
                const ALPHA: f64 = 0.7788007830714049; // exp(-64 / 256)
                let alpha = ALPHA;
                alpha * self.previous_filtered_advance[self.previous_peaks[previous]]
                    + (1.0 - alpha) * self.phase_advance[peak]
            } else {
                self.phase_advance[peak]
            };
            self.phase_advance[peak] = self.filtered_advance[peak];
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
        if count > 0 {
            let peak = self.peaks[..count]
                .iter()
                .copied()
                .max_by(|&a, &b| self.magnitude[a].total_cmp(&self.magnitude[b]))
                .unwrap();
            self.correction_frequency =
                self.phase_advance[peak] * WINDOW as f64 / (std::f64::consts::TAU * HOP as f64);
        }
        self.previous_peaks[..count].copy_from_slice(&self.peaks[..count]);
        self.previous_peak_count = count;
        self.shifted[..BINS].fill(Complex::default());
        self.shifted[0] = if self.ratio == 1.0 {
            self.spectrum[0]
        } else {
            Complex::default()
        };
        if self.ratio == 1.0 {
            self.shifted[BINS - 1] = self.spectrum[BINS - 1];
        }
        let dc_lobe = self.ratio == 1.0 && self.magnitude[0] > self.magnitude[1];
        if dc_lobe {
            self.shifted[1] = self.spectrum[1];
        }
        let mut region = 0;
        let bin_scale = WINDOW as f64 / (std::f64::consts::TAU * HOP as f64);
        let ratio_delta = self.ratio - 1.0;
        let preserve = self.preserve_formants;
        let mut rotation_peak = BINS;
        // Also read outside the cache-update branch by the test-only reference.
        #[allow(unused_assignments)]
        let mut rotation = Complex { re: 1.0, im: 0.0 };
        let mut translations = [Complex::default(); INTERPOLATION_TAPS];
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
            // Both interpolation destinations are discarded by synthesis outside
            // the positive-frequency range. Avoid envelope and phase work there.
            if lower >= (BINS - 1) as isize || lower < 0 {
                continue;
            }
            if peak != rotation_peak {
                rotation = if count == 0 {
                    Complex { re: 1.0, im: 0.0 }
                } else {
                    Complex::polar(
                        1.0,
                        self.synthesis_phase[peak] - self.phase[peak] - centre_step * peak as f64,
                    )
                };
                rotation_peak = peak;
                let weights = translation_weights(fraction);
                for (t, value) in translations.iter_mut().enumerate() {
                    let tap = t as isize - 7;
                    let weight = weights[t];
                    let offset = lower - i as isize + tap;
                    *value = if offset.unsigned_abs() < BINS {
                        let relative = self.centre_phasor[offset.unsigned_abs()];
                        let relative = if offset < 0 {
                            Complex {
                                re: relative.re,
                                im: -relative.im,
                            }
                        } else {
                            relative
                        };
                        rotation.mul(relative).scale(weight)
                    } else {
                        Complex::default()
                    };
                }
            }
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
            // Validate the combined rotation against the original two-centre
            // construction and an independent polar phase-locking reference.
            #[cfg(test)]
            let centre = self.centre_phasor[i];
            #[cfg(test)]
            let phasor = self.spectrum[i]
                .mul(Complex {
                    re: centre.re,
                    im: -centre.im,
                })
                .mul(rotation)
                .scale(gain);
            #[cfg(test)]
            {
                // Independent polar reference, including the no-peak path.
                let locked = if count == 0 {
                    self.phase[peak] + centre_step * peak as f64
                } else {
                    self.synthesis_phase[peak]
                };
                let reference = Complex::polar(
                    self.magnitude[i] * gain,
                    locked
                        + wrap(
                            self.spectrum[i].im.atan2(self.spectrum[i].re) - self.phase[peak]
                                + centre_step * (i as f64 - peak as f64),
                        ),
                );
                let tolerance = 1e-10 * (self.magnitude[i] * gain).max(1.0);
                assert!((phasor.re - reference.re).abs() < tolerance);
                assert!((phasor.im - reference.im).abs() < tolerance);
            }
            for (t, &translation) in translations.iter().enumerate() {
                let bin = lower + t as isize - 7;
                if bin <= 0 || bin >= (BINS - 1) as isize {
                    continue;
                }
                let bin = bin as usize;
                let value = self.spectrum[i].mul(translation).scale(gain);
                self.shifted[bin].re += value.re;
                self.shifted[bin].im += value.im;
            }
        }
        // inverse_real reconstructs the real sequence from the positive half;
        // it does not need an explicitly mirrored negative-frequency spectrum.
        self.fft.inverse_real(&mut self.shifted, &mut self.fft_work);
        let start = WINDOW - SYNTHESIS;
        for i in start..WINDOW {
            self.output[(clock + i - start) % QUEUE] +=
                self.shifted[i].re * self.synthesis_window[i];
        }
    }

    #[inline]
    fn analyze_phase(&mut self, bin: usize) {
        let value = self.spectrum[bin];
        let previous = self.previous_spectrum[bin];
        let previous_phase = if self.phase_valid[bin] {
            self.phase[bin]
        } else {
            previous.im.atan2(previous.re)
        };
        self.phase[bin] = value.im.atan2(value.re);
        let advance = self.nominal_advance[bin];
        self.phase_advance[bin] = advance + wrap(self.phase[bin] - previous_phase - advance);
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

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn translation_kernel_preserves_polynomials_and_integer_bins() {
        for fraction in [0.0, 1e-12, 0.1, 0.5, 0.9, 1.0 - 1e-12] {
            let weights = translation_weights(fraction);
            for degree in 0..=5 {
                let value: f64 = weights
                    .iter()
                    .enumerate()
                    .map(|(t, w)| w * (t as f64 - 7.0).powi(degree))
                    .sum();
                assert!((value - fraction.powi(degree)).abs() < 1e-9);
            }
        }
        assert_eq!(
            translation_weights(0.0)
                .iter()
                .filter(|&&w| w != 0.0)
                .count(),
            1
        );
    }

    #[test]
    fn avx2_fft_matches_portable_butterfly_bits() {
        let vector = Fft::new();
        if !vector.avx2 {
            return;
        }
        let mut scalar = Fft::new();
        scalar.avx2 = false;
        let input = std::array::from_fn(|i| Complex {
            re: (i as f64 * 0.31).sin(),
            im: (i as f64 * 0.17).cos(),
        });
        for inverse in [false, true] {
            let mut expected = input;
            let mut actual = input;
            scalar.transform(&mut expected, inverse);
            vector.transform(&mut actual, inverse);
            for (a, b) in actual.iter().zip(expected) {
                assert_eq!(a.re.to_bits(), b.re.to_bits());
                assert_eq!(a.im.to_bits(), b.im.to_bits());
            }
        }
        let mut expected = input;
        let mut actual = input;
        let mut work = [Complex::default(); HALF];
        scalar.forward_real(&mut expected, &mut work);
        vector.forward_real(&mut actual, &mut work);
        scalar.inverse_real(&mut expected, &mut work);
        vector.inverse_real(&mut actual, &mut work);
        for (a, b) in actual.iter().zip(expected) {
            assert_eq!(a.re.to_bits(), b.re.to_bits());
            assert_eq!(a.im.to_bits(), b.im.to_bits());
        }
    }

    #[test]
    fn peak_only_phase_analysis_matches_eager_output_bits() {
        let mut seed = 17_u32;
        let input: Vec<f32> = (0..16_384)
            .map(|i| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = seed as f64 / u32::MAX as f64 - 0.5;
                let chirp = 0.013 * i as f64 + 0.000002 * (i * i) as f64;
                match i / 4096 {
                    0 => 0.0,
                    1 => (0.12 * chirp.sin() + 0.07 * (3.0 * chirp).cos()) as f32,
                    2 => (0.2 * noise) as f32,
                    _ => {
                        if i == 12_288 {
                            0.7
                        } else {
                            0.0
                        }
                    }
                }
            })
            .collect();
        for preserve in [false, true] {
            for formant in if preserve {
                &[-12.0, 0.0, 12.0][..]
            } else {
                &[0.0][..]
            } {
                let mut lazy = PitchShifter::new(-12.0, preserve, *formant).unwrap();
                let mut eager = PitchShifter::new(-12.0, preserve, *formant).unwrap();
                eager.eager_phase = true;
                for pitch in [-12.0, -3.0, 0.0, 3.0, 7.0, 12.0, 0.0] {
                    lazy.set_semitones(pitch).unwrap();
                    eager.set_semitones(pitch).unwrap();
                    let mut actual = [0.0_f32; 127];
                    let mut expected = actual;
                    for chunk in input.chunks(127) {
                        lazy.process_block(chunk, &mut actual[..chunk.len()]);
                        eager.process_block(chunk, &mut expected[..chunk.len()]);
                        assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
                    }
                }
            }
        }
    }

    #[test]
    fn quotient_phase_wrap_matches_remainder_over_internal_range() {
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;
        let check = |phase: f64| {
            let expected = (phase + pi).rem_euclid(tau) - pi;
            let actual = wrap(phase);
            assert!((-pi..pi).contains(&actual));
            let error = (actual - expected).abs();
            let circular_error = error.min((error - tau).abs());
            assert!(
                circular_error < 2e-12,
                "phase={phase}, error={circular_error}"
            );
        };
        for i in -65536..=65536 {
            check(i as f64 / 8.0);
        }
        for turn in -1024..=1024 {
            let boundary = turn as f64 * tau - pi;
            for offset in [-1e-10, -1e-12, 0.0, 1e-12, 1e-10] {
                check(boundary + offset);
            }
        }
    }

    #[test]
    fn shared_phase_rotation_matches_polar_reference_across_retargeting() {
        let input: Vec<f32> = (0..8192)
            .map(|i| {
                let phase = 0.013 * i as f64 + 0.000002 * (i * i) as f64;
                (0.12 * phase.sin() + 0.07 * (phase * 3.0).cos()) as f32
            })
            .collect();
        let mut output = vec![0.0; input.len()];
        for preserve in [false, true] {
            for formant in if preserve {
                &[-12.0, 0.0, 12.0][..]
            } else {
                &[0.0][..]
            } {
                let mut dsp = PitchShifter::new(-12.0, preserve, *formant).unwrap();
                // Silence exercises the spectrum with no detected peaks.
                dsp.process_block(&vec![0.0_f32; WINDOW], &mut output[..WINDOW]);
                for pitch in [-12.0, -3.0, 0.0, 3.0, 7.0, 12.0] {
                    dsp.set_semitones(pitch).unwrap();
                    for (chunk, out) in input.chunks(127).zip(output.chunks_mut(127)) {
                        dsp.process_block(chunk, out);
                    }
                }
            }
        }
    }

    #[test]
    fn inverse_real_ignores_negative_frequency_storage() {
        let fft = Fft::new();
        let mut spectrum = std::array::from_fn(|i| Complex {
            re: (i as f64 * 0.31).sin(),
            im: 0.0,
        });
        let mut work = [Complex::default(); HALF];
        fft.forward_real(&mut spectrum, &mut work);
        let mut positive_only = spectrum;
        positive_only[BINS..].fill(Complex {
            re: f64::NAN,
            im: f64::NAN,
        });
        fft.inverse_real(&mut spectrum, &mut work);
        fft.inverse_real(&mut positive_only, &mut work);
        for (expected, actual) in spectrum.iter().zip(positive_only) {
            assert_eq!(expected.re.to_bits(), actual.re.to_bits());
            assert_eq!(expected.im.to_bits(), actual.im.to_bits());
        }
    }

    #[test]
    fn fft_slice_butterflies_match_indexed_reference_bits() {
        let fft = Fft::new();
        let input = std::array::from_fn(|i| Complex {
            re: (i as f64 * 0.31).sin(),
            im: (i as f64 * 0.17).cos(),
        });
        for inverse in [false, true] {
            let mut reference = input;
            for (i, &j) in fft.reverse.iter().enumerate() {
                if i < j {
                    reference.swap(i, j);
                }
            }
            let roots = if inverse {
                &fft.inverse_roots
            } else {
                &fft.forward_roots
            };
            let mut size = 2;
            while size <= WINDOW {
                let half = size / 2;
                for start in (0..WINDOW).step_by(size) {
                    for j in 0..half {
                        let a = reference[start + j];
                        let b = reference[start + j + half];
                        let root = roots[j * WINDOW / size];
                        let re = b.re * root.re - b.im * root.im;
                        let im = b.re * root.im + b.im * root.re;
                        reference[start + j] = Complex {
                            re: a.re + re,
                            im: a.im + im,
                        };
                        reference[start + j + half] = Complex {
                            re: a.re - re,
                            im: a.im - im,
                        };
                    }
                }
                size *= 2;
            }
            if inverse {
                for value in &mut reference {
                    value.re *= 1.0 / WINDOW as f64;
                    value.im *= 1.0 / WINDOW as f64;
                }
            }
            let mut actual = input;
            fft.transform(&mut actual, inverse);
            for (a, b) in actual.iter().zip(reference) {
                assert_eq!(a.re.to_bits(), b.re.to_bits());
                assert_eq!(a.im.to_bits(), b.im.to_bits());
            }
        }
    }

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
        let energy: f64 = analysis.iter().map(|x| x * x).sum();
        let centre: f64 = analysis
            .iter()
            .enumerate()
            .map(|(i, x)| i as f64 * x * x)
            .sum::<f64>()
            / energy;
        assert!((phase_center - centre).abs() < 1e-10);
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
