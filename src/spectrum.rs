//! Formant-style scrolling spectrogram. Audio callbacks only push samples to a wait-free
//! ring; FFT and image assembly run on a dedicated analyzer thread while the window is open.
use crate::RATE;
use std::{
    cell::UnsafeCell,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub const IMAGE_WIDTH: usize = 720;
pub const IMAGE_HEIGHT: usize = 400;
pub const BYTES_PER_PIXEL: usize = 4;

const RING: usize = RATE as usize;
const FREQ_MIN: f64 = 20.0;
const FREQ_MAX: f64 = 8_000.0;
const DB_MIN: f64 = -80.0;
const DB_MAX: f64 = 0.0;
const WIDE_FFT: usize = 256;
const WIDE_HOP: usize = 128;
const NARROW_FFT: usize = 2048;
const NARROW_HOP: usize = 512;

static ACTIVE: AtomicPtr<Shared> = AtomicPtr::new(ptr::null_mut());
static IN_USE: AtomicUsize = AtomicUsize::new(0);

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
    size: usize,
    reverse: Vec<usize>,
    roots: Vec<Complex>,
}
impl Fft {
    fn new(size: usize) -> Self {
        assert!(size.is_power_of_two() && size >= 4);
        let bits = size.ilog2();
        Self {
            size,
            reverse: (0..size)
                .map(|i| i.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            roots: (0..size / 2)
                .map(|i| Complex::polar(1.0, -std::f64::consts::TAU * i as f64 / size as f64))
                .collect(),
        }
    }
    fn forward(&self, data: &mut [Complex]) {
        debug_assert_eq!(data.len(), self.size);
        for i in 0..self.size {
            if i < self.reverse[i] {
                data.swap(i, self.reverse[i]);
            }
        }
        let mut size = 2;
        while size <= self.size {
            let half = size / 2;
            for start in (0..self.size).step_by(size) {
                for j in 0..half {
                    let root = self.roots[j * self.size / size];
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
    }
}

struct Shared {
    open: AtomicBool,
    frozen: AtomicBool,
    wideband: AtomicBool,
    stop: AtomicBool,
    /// Absolute sample counters; slot = counter % RING. Capacity is RING-1.
    write: AtomicUsize,
    read: AtomicUsize,
    /// Single-producer (audio) / single-consumer (analyzer) slots.
    ring: UnsafeCell<Box<[f32]>>,
    image: Mutex<Vec<u8>>,
    generation: AtomicU64,
}

// SAFETY: ring is SPSC; image is Mutex-protected; remaining fields are atomics.
unsafe impl Sync for Shared {}

impl Shared {
    fn new() -> Self {
        Self {
            open: AtomicBool::new(true),
            frozen: AtomicBool::new(false),
            wideband: AtomicBool::new(true),
            stop: AtomicBool::new(false),
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
            ring: UnsafeCell::new(vec![0.0; RING].into_boxed_slice()),
            image: Mutex::new(vec![0; IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL]),
            generation: AtomicU64::new(0),
        }
    }

    fn accepting(&self) -> bool {
        self.open.load(Ordering::Acquire) && !self.frozen.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn push_for_test(&self, samples: &[f32]) {
        let mut write = self.write.load(Ordering::Relaxed);
        let mut read = self.read.load(Ordering::Acquire);
        // SAFETY: test-only single producer.
        let ring = unsafe { &mut *self.ring.get() };
        for &sample in samples {
            if write.wrapping_sub(read) >= RING - 1 {
                read = self.read.load(Ordering::Acquire);
                if write.wrapping_sub(read) >= RING - 1 {
                    continue;
                }
            }
            ring[write % RING] = sample;
            write = write.wrapping_add(1);
        }
        self.write.store(write, Ordering::Release);
    }

    fn drain(&self, output: &mut Vec<f32>) {
        let write = self.write.load(Ordering::Acquire);
        let mut read = self.read.load(Ordering::Relaxed);
        // SAFETY: only the analyzer consumer reads slots behind `write`.
        let ring = unsafe { &*self.ring.get() };
        while read != write {
            output.push(ring[read % RING]);
            read = read.wrapping_add(1);
        }
        self.read.store(read, Ordering::Release);
    }
}

/// Owns the analyzer thread and the shared spectrogram state for one open window.
pub struct SpectrumController {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl SpectrumController {
    pub fn start() -> Result<Self, String> {
        if !ACTIVE.load(Ordering::SeqCst).is_null() {
            return Err("Spectrum window is already open".into());
        }
        let shared = Arc::new(Shared::new());
        let worker_shared = Arc::clone(&shared);
        let installed = Arc::as_ptr(&shared).cast_mut();
        if ACTIVE
            .compare_exchange(
                ptr::null_mut(),
                installed,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return Err("Spectrum window is already open".into());
        }
        let worker = thread::Builder::new()
            .name("spectrum-analyzer".into())
            .spawn(move || analyzer_loop(worker_shared))
            .map_err(|error| {
                ACTIVE.store(ptr::null_mut(), Ordering::SeqCst);
                format!("Cannot start spectrum analyzer thread: {error}")
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub fn set_wideband(&self, wideband: bool) {
        self.shared.wideband.store(wideband, Ordering::Release);
    }

    pub fn set_frozen(&self, frozen: bool) {
        self.shared.frozen.store(frozen, Ordering::Release);
    }

    /// Copies BGRA pixels when the generation advanced. Returns true if `pixels` was updated.
    pub fn copy_image(&self, pixels: &mut [u8], last_generation: &mut u64) -> bool {
        let generation = self.shared.generation.load(Ordering::Acquire);
        if generation == *last_generation {
            return false;
        }
        let image = self.shared.image.lock().expect("spectrum image lock");
        let need = IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL;
        assert_eq!(pixels.len(), need);
        assert_eq!(image.len(), need);
        pixels.copy_from_slice(&image);
        *last_generation = generation;
        true
    }
}

impl Drop for SpectrumController {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.open.store(false, Ordering::Release);
        ACTIVE.store(ptr::null_mut(), Ordering::SeqCst);
        while IN_USE.load(Ordering::SeqCst) != 0 {
            thread::yield_now();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Realtime-safe output tap. No-op when the spectrum window is closed or frozen.
/// Converts in place without heap allocation.
pub fn push_output_samples<T: crate::pitch::Sample>(samples: &[T]) {
    IN_USE.fetch_add(1, Ordering::SeqCst);
    let shared = ACTIVE.load(Ordering::SeqCst);
    if let Some(shared) = unsafe { shared.as_ref() } {
        if shared.accepting() {
            let mut write = shared.write.load(Ordering::Relaxed);
            let mut read = shared.read.load(Ordering::Acquire);
            // SAFETY: only the audio producer writes slots ahead of `read`.
            let ring = unsafe { &mut *shared.ring.get() };
            for &sample in samples {
                if write.wrapping_sub(read) >= RING - 1 {
                    read = shared.read.load(Ordering::Acquire);
                    if write.wrapping_sub(read) >= RING - 1 {
                        continue;
                    }
                }
                ring[write % RING] = sample.normalized() as f32;
                write = write.wrapping_add(1);
            }
            shared.write.store(write, Ordering::Release);
        }
    }
    IN_USE.fetch_sub(1, Ordering::SeqCst);
}

fn hann(size: usize) -> Vec<f64> {
    (0..size)
        .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / size as f64).cos())
        .collect()
}

fn db_to_bgra(db: f64) -> [u8; 4] {
    let t = ((db - DB_MIN) / (DB_MAX - DB_MIN)).clamp(0.0, 1.0);
    // Praat-like light-on-dark grayscale.
    let v = (t * 255.0).round() as u8;
    [v, v, v, 255]
}

fn magnitude_at(bins: &[f64], fft_size: usize, hz: f64) -> f64 {
    let bin = hz * fft_size as f64 / RATE as f64;
    if bin <= 0.0 {
        return bins[0];
    }
    let max = bins.len() - 1;
    if bin >= max as f64 {
        return bins[max];
    }
    let lo = bin.floor() as usize;
    let frac = bin - lo as f64;
    bins[lo] * (1.0 - frac) + bins[lo + 1] * frac
}

fn append_column(image: &mut [u8], column: &[[u8; 4]]) {
    debug_assert_eq!(column.len(), IMAGE_HEIGHT);
    let stride = IMAGE_WIDTH * BYTES_PER_PIXEL;
    for y in 0..IMAGE_HEIGHT {
        let row = &mut image[y * stride..(y + 1) * stride];
        row.copy_within(BYTES_PER_PIXEL.., 0);
        let offset = (IMAGE_WIDTH - 1) * BYTES_PER_PIXEL;
        row[offset..offset + 4].copy_from_slice(&column[y]);
    }
}

fn analyzer_loop(shared: Arc<Shared>) {
    let wide_fft = Fft::new(WIDE_FFT);
    let narrow_fft = Fft::new(NARROW_FFT);
    let wide_window = hann(WIDE_FFT);
    let narrow_window = hann(NARROW_FFT);
    let mut wide_spectrum = vec![Complex::default(); WIDE_FFT];
    let mut narrow_spectrum = vec![Complex::default(); NARROW_FFT];
    let mut wide_bins = vec![0.0; WIDE_FFT / 2 + 1];
    let mut narrow_bins = vec![0.0; NARROW_FFT / 2 + 1];
    let mut pending = Vec::with_capacity(RING / 4);
    let mut time_domain = Vec::with_capacity(NARROW_FFT * 2);
    let mut column = vec![[0_u8; 4]; IMAGE_HEIGHT];
    let mut using_wide = true;

    while !shared.stop.load(Ordering::Acquire) {
        if shared.frozen.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        shared.drain(&mut pending);
        if pending.is_empty() {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        time_domain.append(&mut pending);

        let wideband = shared.wideband.load(Ordering::Acquire);
        if wideband != using_wide {
            using_wide = wideband;
            time_domain.clear();
        }
        let (fft_size, hop, window, spectrum, bins, fft) = if using_wide {
            (
                WIDE_FFT,
                WIDE_HOP,
                wide_window.as_slice(),
                wide_spectrum.as_mut_slice(),
                wide_bins.as_mut_slice(),
                &wide_fft,
            )
        } else {
            (
                NARROW_FFT,
                NARROW_HOP,
                narrow_window.as_slice(),
                narrow_spectrum.as_mut_slice(),
                narrow_bins.as_mut_slice(),
                &narrow_fft,
            )
        };

        while time_domain.len() >= fft_size {
            for i in 0..fft_size {
                spectrum[i] = Complex {
                    re: time_domain[i] as f64 * window[i],
                    im: 0.0,
                };
            }
            fft.forward(spectrum);
            for i in 0..bins.len() {
                bins[i] = spectrum[i].re.hypot(spectrum[i].im);
            }
            for (y, pixel) in column.iter_mut().enumerate() {
                let t = 1.0 - y as f64 / (IMAGE_HEIGHT - 1) as f64;
                let hz = FREQ_MIN + (FREQ_MAX - FREQ_MIN) * t;
                let mag = magnitude_at(bins, fft_size, hz);
                let db = 20.0 * (mag.max(1e-12)).log10();
                *pixel = db_to_bgra(db);
            }
            {
                let mut image = shared.image.lock().expect("spectrum image lock");
                append_column(&mut image, &column);
            }
            shared.generation.fetch_add(1, Ordering::Release);
            time_domain.drain(..hop);
        }
        if time_domain.len() > NARROW_FFT * 4 {
            let keep = NARROW_FFT;
            let drain = time_domain.len() - keep;
            time_domain.drain(..drain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_when_full_without_blocking() {
        let shared = Shared::new();
        let block = vec![0.25_f32; 1024];
        for _ in 0..RING {
            shared.push_for_test(&block);
        }
        let filled = shared
            .write
            .load(Ordering::Relaxed)
            .wrapping_sub(shared.read.load(Ordering::Relaxed));
        assert!(filled > 0);
        assert!(filled < RING);
        let mut out = Vec::new();
        shared.drain(&mut out);
        assert_eq!(out.len(), filled);
    }

    #[test]
    fn append_column_scrolls_left() {
        let mut image = vec![0_u8; IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL];
        let red = vec![[0, 0, 255, 255]; IMAGE_HEIGHT];
        append_column(&mut image, &red);
        let stride = IMAGE_WIDTH * BYTES_PER_PIXEL;
        assert_eq!(&image[stride - 4..stride], &[0, 0, 255, 255]);
        let blue = vec![[255, 0, 0, 255]; IMAGE_HEIGHT];
        append_column(&mut image, &blue);
        assert_eq!(&image[stride - 8..stride - 4], &[0, 0, 255, 255]);
        assert_eq!(&image[stride - 4..stride], &[255, 0, 0, 255]);
    }

    #[test]
    fn controller_start_stop_clears_active_tap() {
        let controller = SpectrumController::start().unwrap();
        assert!(!ACTIVE.load(Ordering::SeqCst).is_null());
        push_output_samples(&[0.1_f32, -0.1, 0.2]);
        drop(controller);
        assert!(ACTIVE.load(Ordering::SeqCst).is_null());
        push_output_samples(&[1.0_f32]);
    }
}
