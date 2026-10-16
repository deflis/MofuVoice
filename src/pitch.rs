//! Monophonic spectral pitch shifting with identity phase locking.
//! All FFT tables, windows, and overlap storage are allocated before streaming.
//! Wet audio uses a long analysis window and a short right-aligned synthesis window,
//! so the fixed delay is SYNTHESIS samples; the neutral setting bypasses exactly.
use crate::dsp::{complex::Complex, fft_kernel::FftKernel, phase, window::hann, Sample};
use std::{error::Error, fmt};
#[path = "pitch_math.rs"]
mod math;

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

/// Pitch and independent formant settings, in semitones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchSettings {
    pub semitones: f64,
    pub preserve_formants: bool,
    pub formant_shift: f64,
}

impl Default for PitchSettings {
    fn default() -> Self {
        Self {
            semitones: 0.0,
            preserve_formants: true,
            formant_shift: 0.0,
        }
    }
}

impl PitchSettings {
    pub fn build(self) -> Result<PitchShifter, Box<dyn Error>> {
        PitchShifter::new(self.semitones, self.preserve_formants, self.formant_shift)
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
            unsafe { FftKernel::stages_avx2(roots, data) };
        } else {
            FftKernel::stages(roots, data);
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        FftKernel::stages(roots, data);
        if let Some(scale) = scale {
            for value in data {
                value.re *= scale;
                value.im *= scale;
            }
        }
    }

    fn synthesis_stages(roots: &[Complex], data: &mut [Complex; HALF]) {
        FftKernel::stage::<HALF, 2>(roots, data);
        FftKernel::stage::<HALF, 4>(roots, data);
        FftKernel::stage::<HALF, 8>(roots, data);
        FftKernel::stage::<HALF, 16>(roots, data);
        FftKernel::stage::<HALF, 32>(roots, data);
        FftKernel::stage::<HALF, 64>(roots, data);
        FftKernel::stage::<HALF, 128>(roots, data);
        FftKernel::stage::<HALF, 256>(roots, data);
        Self::right_stage::<512, 0>(roots, data);
        Self::right_stage::<1024, 256>(roots, data);
    }

    fn right_stage<const SIZE: usize, const FIRST: usize>(
        roots: &[Complex],
        data: &mut [Complex; HALF],
    ) {
        let half = SIZE / 2;
        let roots = &roots[half - 1 + FIRST..SIZE - 1];
        for block in data.as_chunks_mut::<SIZE>().0 {
            let (left, right) = block.split_at_mut(half);
            for ((a, b), root) in left[FIRST..].iter().zip(&mut right[FIRST..]).zip(roots) {
                let product = b.mul(*root);
                *b = Complex {
                    re: a.re - product.re,
                    im: a.im - product.im,
                };
            }
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn synthesis_stages_avx2(roots: &[Complex], data: &mut [Complex; HALF]) {
        FftKernel::stage::<HALF, 2>(roots, data);
        // SAFETY: inherited feature contract; each stage checks its slices.
        unsafe {
            FftKernel::stage_avx2::<HALF, 4>(roots, data);
            FftKernel::stage_avx2::<HALF, 8>(roots, data);
            FftKernel::stage_avx2::<HALF, 16>(roots, data);
            FftKernel::stage_avx2::<HALF, 32>(roots, data);
            FftKernel::stage_avx2::<HALF, 64>(roots, data);
            FftKernel::stage_avx2::<HALF, 128>(roots, data);
            FftKernel::stage_avx2::<HALF, 256>(roots, data);
            Self::right_stage_avx2::<512, 0>(roots, data);
            Self::right_stage_avx2::<1024, 256>(roots, data);
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn right_stage_avx2<const SIZE: usize, const FIRST: usize>(
        roots: &[Complex],
        data: &mut [Complex; HALF],
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;
        let half = SIZE / 2;
        let roots = &roots[half - 1 + FIRST..SIZE - 1];
        for block in data.as_chunks_mut::<SIZE>().0 {
            let (left, right) = block.split_at_mut(half);
            for ((a, b), root) in left[FIRST..]
                .as_chunks::<2>()
                .0
                .iter()
                .zip(right[FIRST..].as_chunks_mut::<2>().0)
                .zip(roots.as_chunks::<2>().0)
            {
                // SAFETY: each checked chunk contains two repr(C) complexes;
                // unaligned loads/stores remain inside disjoint stage slices.
                unsafe {
                    let av = _mm256_loadu_pd(a.as_ptr().cast());
                    let bv = _mm256_loadu_pd(b.as_ptr().cast());
                    let w = _mm256_loadu_pd(root.as_ptr().cast());
                    let product = _mm256_addsub_pd(
                        _mm256_mul_pd(_mm256_permute_pd::<0>(bv), w),
                        _mm256_mul_pd(_mm256_permute_pd::<15>(bv), _mm256_permute_pd::<5>(w)),
                    );
                    _mm256_storeu_pd(b.as_mut_ptr().cast(), _mm256_sub_pd(av, product));
                }
            }
        }
    }

    fn envelope_stages(roots: &[Complex], data: &mut [Complex; HALF]) {
        FftKernel::stage::<HALF, 2>(roots, data);
        FftKernel::stage::<HALF, 4>(roots, data);
        FftKernel::stage::<HALF, 8>(roots, data);
        FftKernel::stage::<HALF, 16>(roots, data);
        FftKernel::stage::<HALF, 32>(roots, data);
        FftKernel::stage::<HALF, 64>(roots, data);
        Self::envelope_stage::<128>(roots, data);
        Self::envelope_stage::<256>(roots, data);
        Self::envelope_stage::<512>(roots, data);
        Self::envelope_stage::<1024>(roots, data);
    }

    fn envelope_stage<const SIZE: usize>(roots: &[Complex], data: &mut [Complex; HALF]) {
        const KEEP: usize = CEPSTRUM / 2 + 2; // even, includes the centre boundary
        let half = SIZE / 2;
        let roots = &roots[half - 1..SIZE - 1];
        for block in data.as_chunks_mut::<SIZE>().0 {
            let (left, right) = block.split_at_mut(half);
            for j in (0..KEEP).chain(KEEP.max(half - KEEP)..half) {
                let a = left[j];
                let product = right[j].mul(roots[j]);
                if j < KEEP {
                    left[j] = Complex {
                        re: a.re + product.re,
                        im: a.im + product.im,
                    };
                }
                if j >= half - KEEP {
                    right[j] = Complex {
                        re: a.re - product.re,
                        im: a.im - product.im,
                    };
                }
            }
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn envelope_stages_avx2(roots: &[Complex], data: &mut [Complex; HALF]) {
        FftKernel::stage::<HALF, 2>(roots, data);
        // SAFETY: inherited AVX2 contract and checked stage slices.
        unsafe {
            FftKernel::stage_avx2::<HALF, 4>(roots, data);
            FftKernel::stage_avx2::<HALF, 8>(roots, data);
            FftKernel::stage_avx2::<HALF, 16>(roots, data);
            FftKernel::stage_avx2::<HALF, 32>(roots, data);
            FftKernel::stage_avx2::<HALF, 64>(roots, data);
            Self::envelope_stage_avx2::<128>(roots, data);
            Self::envelope_stage_avx2::<256>(roots, data);
            Self::envelope_stage_avx2::<512>(roots, data);
            Self::envelope_stage_avx2::<1024>(roots, data);
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn envelope_stage_avx2<const SIZE: usize>(
        roots: &[Complex],
        data: &mut [Complex; HALF],
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;
        const KEEP: usize = CEPSTRUM / 2 + 2;
        let half = SIZE / 2;
        let roots = &roots[half - 1..SIZE - 1];
        for block in data.as_chunks_mut::<SIZE>().0 {
            let (left, right) = block.split_at_mut(half);
            for j in (0..KEEP)
                .step_by(2)
                .chain((KEEP.max(half - KEEP)..half).step_by(2))
            {
                // SAFETY: KEEP and all half sizes are even; each j addresses two
                // complexes within checked, disjoint left/right/root slices.
                unsafe {
                    let a = _mm256_loadu_pd(left[j..j + 2].as_ptr().cast());
                    let b = _mm256_loadu_pd(right[j..j + 2].as_ptr().cast());
                    let w = _mm256_loadu_pd(roots[j..j + 2].as_ptr().cast());
                    let product = _mm256_addsub_pd(
                        _mm256_mul_pd(_mm256_permute_pd::<0>(b), w),
                        _mm256_mul_pd(_mm256_permute_pd::<15>(b), _mm256_permute_pd::<5>(w)),
                    );
                    if j < KEEP {
                        _mm256_storeu_pd(
                            left[j..j + 2].as_mut_ptr().cast(),
                            _mm256_add_pd(a, product),
                        );
                    }
                    if j >= half - KEEP {
                        _mm256_storeu_pd(
                            right[j..j + 2].as_mut_ptr().cast(),
                            _mm256_sub_pd(a, product),
                        );
                    }
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
    #[inline(always)]
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
        for k in 1..=HALF / 2 {
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
            if k != HALF / 2 {
                time[HALF - k] = Complex {
                    re: 0.5 * (a.re - wb_im),
                    im: 0.5 * (-a.im - wb_re),
                };
            }
        }
    }

    /// Inverse of a Hermitian spectrum produced by `forward_real` / mirror fill.
    #[cfg(test)]
    fn inverse_real(&self, spectrum: &mut [Complex; WINDOW], work: &mut [Complex; HALF]) {
        self.inverse_real_partial(spectrum, work, false, false);
    }

    /// Envelope estimation retains only +/- CEPSTRUM samples. Prune the later
    /// inverse stages instead of calculating values that the lifter discards.
    #[inline(always)]
    fn inverse_real_partial(
        &self,
        spectrum: &mut [Complex; WINDOW],
        work: &mut [Complex; HALF],
        envelope: bool,
        synthesis: bool,
    ) {
        let x0 = spectrum[0].re;
        let xn = spectrum[HALF].re;
        work[0] = Complex {
            re: 0.5 * (x0 + xn),
            im: 0.5 * (x0 - xn),
        };
        for k in 1..=HALF / 2 {
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
            if k != HALF / 2 {
                work[HALF - k] = Complex {
                    re: e.re - t_re * o.re + t_im * o.im,
                    im: -e.im + t_re * o.im + t_im * o.re,
                };
            }
        }
        if envelope {
            for (i, &j) in self.reverse_half.iter().enumerate() {
                if i < j {
                    work.swap(i, j);
                }
            }
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            if self.avx2 {
                // SAFETY: runtime AVX2 detection and checked transform storage.
                unsafe { Self::envelope_stages_avx2(self.inverse_roots_half.as_ref(), work) };
            } else {
                Self::envelope_stages(self.inverse_roots_half.as_ref(), work);
            }
            #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
            Self::envelope_stages(self.inverse_roots_half.as_ref(), work);
            for n in (0..CEPSTRUM / 2 + 1).chain(HALF - CEPSTRUM / 2..HALF) {
                spectrum[2 * n] = Complex {
                    re: work[n].re / HALF as f64,
                    im: 0.0,
                };
                spectrum[2 * n + 1] = Complex {
                    re: work[n].im / HALF as f64,
                    im: 0.0,
                };
            }
            return;
        }
        if synthesis {
            for (i, &j) in self.reverse_half.iter().enumerate() {
                if i < j {
                    work.swap(i, j);
                }
            }
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            if self.avx2 {
                // SAFETY: inherited runtime AVX2 detection and checked storage.
                unsafe { Self::synthesis_stages_avx2(self.inverse_roots_half.as_ref(), work) };
            } else {
                Self::synthesis_stages(self.inverse_roots_half.as_ref(), work);
            }
            #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
            Self::synthesis_stages(self.inverse_roots_half.as_ref(), work);
            for n in (WINDOW - SYNTHESIS) / 2..HALF {
                spectrum[2 * n] = Complex {
                    re: work[n].re / HALF as f64,
                    im: 0.0,
                };
                spectrum[2 * n + 1] = Complex {
                    re: work[n].im / HALF as f64,
                    im: 0.0,
                };
            }
            return;
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

#[derive(Clone, Copy, Default)]
struct Lobe {
    first: usize,
    last: usize,
    offset: isize,
    fraction: f64,
    rotation: Complex,
    peak: usize,
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
    gains: Box<[f64; BINS]>,
    lobes: Box<[Lobe; BINS]>,
    rotation_angles: Box<[f64; BINS]>,
    rotations: Box<[Complex; BINS]>,
    phase: Box<[f64; BINS]>,
    // Portable processors retain peak-only scalar analysis; evaluating every
    // bin is advantageous only with four-wide phase math.
    phase_valid: Box<[bool; BINS]>,
    previous_spectrum: Box<[Complex; BINS]>,
    #[cfg(test)]
    eager_phase: bool,
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
            gains: Box::new([0.0; BINS]),
            lobes: Box::new([Lobe::default(); BINS]),
            rotation_angles: Box::new([0.0; BINS]),
            rotations: Box::new([Complex::default(); BINS]),
            phase: Box::new([0.0; BINS]),
            phase_valid: Box::new([true; BINS]),
            previous_spectrum: Box::new([Complex::default(); BINS]),
            #[cfg(test)]
            eager_phase: false,
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
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if self.fft.avx2 {
            // SAFETY: the processor detects AVX2 once during construction.
            unsafe { self.frame_avx2() };
            return;
        }
        self.frame_inner();
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn frame_avx2(&mut self) {
        self.frame_inner();
    }

    #[inline(always)]
    fn frame_inner(&mut self) {
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
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let vector = self.fft.avx2;
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let vector = false;
        let neutral = self.ratio == 1.0 && self.formant_ratio == 1.0;
        if self.preserve_formants && !neutral {
            math::log(
                self.magnitude.as_slice(),
                self.envelope.as_mut_slice(),
                vector,
            );
            // Real cepstral low-pass estimates a smooth envelope without fitting harmonics.
            for i in 0..BINS {
                self.envelope_work[i] = Complex {
                    re: self.envelope[i],
                    im: 0.0,
                };
            }
            // inverse_real reads only the nonnegative-frequency bins.
            self.fft
                .inverse_real_partial(&mut self.envelope_work, &mut self.fft_work, true, false);
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
        // Batch independent phases, including bins that may become peaks in the
        // next frame. This replaces scalar atan2 calls and complex history with
        // four-wide double-precision arithmetic and a real phase history.
        #[cfg(test)]
        let eager = self.eager_phase;
        #[cfg(not(test))]
        let eager = false;
        let batched = vector || eager;
        if batched {
            phase::angles(
                &self.spectrum[..BINS],
                self.phase.as_mut_slice(),
                vector && !eager,
            );
        }
        if count == 0 {
            // A spectrum without local maxima uses each bin as its own peak.
            for i in 1..BINS - 1 {
                self.analyze_phase(i, batched);
            }
        } else {
            for p in 0..count {
                self.analyze_phase(self.peaks[p], batched);
            }
        }
        if batched {
            self.previous_phase.copy_from_slice(self.phase.as_slice());
        } else {
            self.previous_spectrum
                .copy_from_slice(&self.spectrum[..BINS]);
            self.phase_valid.fill(count == 0);
            for &peak in &self.peaks[..count] {
                self.phase_valid[peak] = true;
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
        self.shifted[..BINS].fill(Complex::default());
        self.shifted[0] = self.spectrum[0];
        if self.ratio == 1.0 {
            self.shifted[BINS - 1] = self.spectrum[BINS - 1];
        }
        let dc_lobe = self.magnitude[0] > self.magnitude[1];
        if dc_lobe {
            self.shifted[1] = self.spectrum[1];
        }
        if neutral {
            self.shifted[..BINS].copy_from_slice(&self.spectrum[..BINS]);
        } else {
            let bin_scale = WINDOW as f64 / (std::f64::consts::TAU * HOP as f64);
            let ratio_delta = self.ratio - 1.0;
            let preserve = self.preserve_formants;
            let inverse_formant = 1.0 / self.formant_ratio;
            for (region, &peak) in self.peaks[..count].iter().enumerate() {
                self.rotation_angles[region] =
                    self.synthesis_phase[peak] - self.phase[peak] - centre_step * peak as f64;
            }
            phase::unit_phasors(
                &self.rotation_angles[..count],
                &mut self.rotations[..count],
                vector && !eager,
            );
            let relative = |offset: isize| {
                let value = self.centre_phasor[offset.unsigned_abs()];
                if offset < 0 {
                    Complex {
                        re: value.re,
                        im: -value.im,
                    }
                } else {
                    value
                }
            };
            let mut start = if dc_lobe { 2 } else { 1 };
            let mut lobe_count = 0;
            if preserve {
                self.gains.fill(0.0);
            }
            for region in 0..if count == 0 { BINS - 2 } else { count } {
                let peak = if count == 0 {
                    region + 1
                } else {
                    self.peaks[region]
                };
                let end = if count == 0 {
                    peak + 1
                } else if region + 1 == count {
                    BINS - 1
                } else {
                    (peak + self.peaks[region + 1]) / 2 + 1
                };
                if end <= start {
                    continue;
                }
                // Every bin in this peak's lobe shares its measured translation and
                // phase rotation. Calculate them once before visiting the lobe.
                let translation = self.phase_advance[peak] * bin_scale * ratio_delta;
                let offset = translation.floor() as isize;
                let fraction = translation - offset as f64;
                let rotation = if count == 0 {
                    Complex { re: 1.0, im: 0.0 }
                } else {
                    self.rotations[region]
                };
                // Clip the source range before indexing rotation tables. Entire
                // lobes can fall outside the positive-frequency synthesis range.
                let first = (start as isize).max(-offset) as usize;
                let last = (end as isize).min((BINS - 1) as isize - offset).max(0) as usize;
                start = end;
                if first >= last {
                    continue;
                }
                self.lobes[lobe_count] = Lobe {
                    first,
                    last,
                    offset,
                    fraction,
                    rotation,
                    peak,
                };
                lobe_count += 1;
                if preserve {
                    if self.formant_ratio == 1.0 {
                        let target = (first as isize + offset) as usize;
                        let length = last - first;
                        let source = &self.envelope[first..last];
                        let lower = &self.envelope[target..target + length];
                        let upper = &self.envelope[target + 1..target + length + 1];
                        for (((&source, &lower), &upper), gain) in source
                            .iter()
                            .zip(lower)
                            .zip(upper)
                            .zip(&mut self.gains[first..last])
                        {
                            *gain = lower * (1.0 - fraction) + upper * fraction - source;
                        }
                    } else {
                        for i in first..last {
                            let destination = i as f64 + translation;
                            self.gains[i] = {
                                let position =
                                    (destination * inverse_formant).clamp(0.0, (BINS - 1) as f64);
                                let env_lower = position.floor() as usize;
                                let env_fraction = position - env_lower as f64;
                                let desired = self.envelope[env_lower] * (1.0 - env_fraction)
                                    + self.envelope[(env_lower + 1).min(BINS - 1)] * env_fraction;
                                // Bound envelope correction, rather than clipping the synthesis waveform.
                                desired - self.envelope[i]
                            };
                        }
                    }
                }
            }
            // A lobe often contains fewer than four bins. Batch all independent
            // corrections so SIMD does not devolve into scalar exp tails per peak.
            // Unused bins have log-gain zero and do not enter synthesis.
            if preserve {
                math::exp(self.gains.as_mut_slice(), vector);
            }
            for lobe in &self.lobes[..lobe_count] {
                let Lobe {
                    first,
                    last,
                    offset,
                    fraction,
                    rotation,
                    peak,
                } = *lobe;
                #[cfg(not(test))]
                let _ = peak;
                let weighted_lower = rotation.mul(relative(offset)).scale(1.0 - fraction);
                let weighted_upper = rotation.mul(relative(offset + 1)).scale(fraction);
                for i in first..last {
                    let lower = i as isize + offset;
                    let gain = if preserve { self.gains[i] } else { 1.0 };
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
                                    self.spectrum[i].im.atan2(self.spectrum[i].re)
                                        - self.phase[peak]
                                        + centre_step * (i as f64 - peak as f64),
                                ),
                        );
                        let tolerance = 1e-10 * (self.magnitude[i] * gain).max(1.0);
                        assert!((phasor.re - reference.re).abs() < tolerance);
                        assert!((phasor.im - reference.im).abs() < tolerance);
                    }
                    for (bin, weight, rotation) in [
                        (lower, 1.0 - fraction, weighted_lower),
                        (lower + 1, fraction, weighted_upper),
                    ] {
                        if bin <= 0 || bin >= (BINS - 1) as isize || weight == 0.0 {
                            continue;
                        }
                        let bin = bin as usize;
                        let value = self.spectrum[i].mul(rotation).scale(gain);
                        #[cfg(test)]
                        {
                            let reference = phasor.mul(self.centre_phasor[bin]).scale(weight);
                            let tolerance = 1e-10 * (self.magnitude[i] * gain).max(1.0);
                            assert!((value.re - reference.re).abs() < tolerance);
                            assert!((value.im - reference.im).abs() < tolerance);
                        }
                        self.shifted[bin].re += value.re;
                        self.shifted[bin].im += value.im;
                    }
                }
            }
        }
        // inverse_real reconstructs the real sequence from the positive half;
        // it does not need an explicitly mirrored negative-frequency spectrum.
        self.fft
            .inverse_real_partial(&mut self.shifted, &mut self.fft_work, false, true);
        let start = WINDOW - SYNTHESIS;
        for i in start..WINDOW {
            self.output[(clock + i - start) % QUEUE] +=
                self.shifted[i].re * self.synthesis_window[i];
        }
    }

    #[inline]
    fn analyze_phase(&mut self, bin: usize, batched: bool) {
        if !batched {
            let previous = self.previous_spectrum[bin];
            self.previous_phase[bin] = if self.phase_valid[bin] {
                self.phase[bin]
            } else {
                previous.im.atan2(previous.re)
            };
            let value = self.spectrum[bin];
            self.phase[bin] = value.im.atan2(value.re);
        }
        let advance = self.nominal_advance[bin];
        self.phase_advance[bin] =
            advance + wrap(self.phase[bin] - self.previous_phase[bin] - advance);
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
    fn pruned_inverse_preserves_every_retained_sample_bit() {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let mut fft = Fft::new();
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let fft = Fft::new();
        let input = std::array::from_fn(|i| Complex {
            re: (i as f64 * 0.31).sin(),
            im: (i as f64 * 0.17).cos(),
        });
        for vector in [false, true] {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            {
                fft.avx2 = vector && std::is_x86_feature_detected!("avx2");
            }
            let _ = vector;
            let mut expected = input;
            let mut work = [Complex::default(); HALF];
            fft.inverse_real(&mut expected, &mut work);
            for envelope in [false, true] {
                let mut actual = input;
                fft.inverse_real_partial(&mut actual, &mut work, envelope, !envelope);
                let ranges = if envelope {
                    [0..CEPSTRUM + 1, WINDOW - CEPSTRUM..WINDOW]
                } else {
                    [WINDOW - SYNTHESIS..WINDOW, WINDOW..WINDOW]
                };
                for i in ranges.into_iter().flatten() {
                    assert_eq!(
                        actual[i].re.to_bits(),
                        expected[i].re.to_bits(),
                        "sample {i}"
                    );
                    assert_eq!(actual[i].im.to_bits(), expected[i].im.to_bits());
                }
            }
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn vector_frame_matches_portable_across_formant_and_pitch_transitions() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        let mut seed = 7_u32;
        let input: Vec<f32> = (0..8192)
            .map(|i| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = seed as f64 / u32::MAX as f64 - 0.5;
                (0.12 * (i as f64 * 0.013).sin() + 0.03 * noise) as f32
            })
            .collect();
        for preserve in [false, true] {
            for formant in if preserve {
                &[-12.0, 0.0, 12.0][..]
            } else {
                &[0.0][..]
            } {
                let mut vector = PitchShifter::new(-12.0, preserve, *formant).unwrap();
                let mut portable = PitchShifter::new(-12.0, preserve, *formant).unwrap();
                portable.fft.avx2 = false;
                for pitch in [-12.0, -3.0, 0.0, 3.0, 7.0, 12.0, 0.0] {
                    vector.set_semitones(pitch).unwrap();
                    portable.set_semitones(pitch).unwrap();
                    let mut expected = [0.0_f32; 127];
                    let mut actual = expected;
                    for chunk in input.chunks(127) {
                        vector.process_block(chunk, &mut actual[..chunk.len()]);
                        portable.process_block(chunk, &mut expected[..chunk.len()]);
                        for (&a, &b) in actual.iter().zip(&expected) {
                            assert!((a - b).abs() < 1e-7);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn batched_phase_analysis_matches_system_reference() {
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
                        for (&actual, &expected) in actual.iter().zip(&expected) {
                            assert!((actual - expected).abs() < 1e-7);
                        }
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
