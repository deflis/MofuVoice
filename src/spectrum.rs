//! Formant-style scrolling spectrogram. Audio callbacks only push samples to a wait-free
//! ring; FFT and image assembly run on a dedicated analyzer thread while the window is open.
use crate::{
    dsp::{complex::Complex, fft::Fft, window},
    RATE,
};
use std::{
    cell::UnsafeCell,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const IMAGE_WIDTH: usize = 720;
pub const IMAGE_HEIGHT: usize = 400;
pub const BYTES_PER_PIXEL: usize = 4;

// A power of two keeps slot mapping continuous when absolute counters wrap.
const RING: usize = 262_144; // More than one second at the highest supported rate.
const FREQ_MIN: f64 = 20.0;
const FREQ_MAX: f64 = 8_000.0;
const DB_MIN: f64 = -80.0;
const DB_MAX: f64 = 0.0;
const WIDE_FFT: usize = 256;
const WIDE_HOP: usize = 128;
const NARROW_FFT: usize = 2048;
const NARROW_HOP: usize = 512;

static SAMPLE_RATE: AtomicUsize = AtomicUsize::new(RATE as usize);
/// Set on the control thread before starting a new stream.
pub fn set_sample_rate(rate: u32) {
    assert!(crate::validate_sample_rate(rate).is_ok());
    SAMPLE_RATE.store(rate as usize, Ordering::Release);
}
static ACTIVE: AtomicPtr<Shared> = AtomicPtr::new(ptr::null_mut());
static IN_USE: AtomicUsize = AtomicUsize::new(0);

struct TapLease;
impl Drop for TapLease {
    fn drop(&mut self) {
        IN_USE.fetch_sub(1, Ordering::SeqCst);
    }
}

struct ProducerGuard<'a>(&'a AtomicBool);
impl Drop for ProducerGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Output-channel levels in dBFS; silence is displayed at -60 dBFS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterLevels {
    pub rms_db: f64,
    pub peak_db: f64,
    pub hold_db: f64,
}

impl Default for MeterLevels {
    fn default() -> Self {
        Self {
            rms_db: -60.0,
            peak_db: -60.0,
            hold_db: -60.0,
        }
    }
}

struct LevelMeter {
    power: f64,
    peak: f64,
    hold: f64,
    hold_seconds: f64,
}

impl LevelMeter {
    fn new() -> Self {
        Self {
            power: 0.0,
            peak: 0.0,
            hold: 0.0,
            hold_seconds: 0.0,
        }
    }

    fn update_hold(&mut self, seconds: f64, decay: f64) {
        if self.peak >= self.hold {
            self.hold = self.peak;
            self.hold_seconds = 1.0;
        } else {
            let decay_seconds = (seconds - self.hold_seconds).max(0.0);
            self.hold_seconds = (self.hold_seconds - seconds).max(0.0);
            if decay_seconds > 0.0 {
                let factor = if decay_seconds == seconds {
                    decay
                } else {
                    10.0_f64.powf(-12.0 * decay_seconds / 20.0)
                };
                self.hold = (self.hold * factor).max(self.peak);
            }
        }
    }

    #[cfg(test)]
    fn update(&mut self, samples: &[f32], elapsed: f64) -> MeterLevels {
        self.update_at_rate(samples, elapsed, RATE)
    }
    fn update_at_rate(&mut self, samples: &[f32], elapsed: f64, rate: u32) -> MeterLevels {
        if samples.is_empty() {
            self.peak *= (-elapsed / 0.3).exp();
            self.power *= (-elapsed / 0.1).exp();
            self.update_hold(elapsed, 10.0_f64.powf(-12.0 * elapsed / 20.0));
        } else {
            let seconds = 1.0 / rate as f64;
            let smoothing = (-seconds / 0.1).exp();
            let peak_release = (-seconds / 0.3).exp();
            let hold_release = 10.0_f64.powf(-12.0 * seconds / 20.0);
            for &sample in samples {
                let value = if sample.is_finite() {
                    sample as f64
                } else {
                    0.0
                };
                self.power = smoothing * self.power + (1.0 - smoothing) * value * value;
                self.peak = (self.peak * peak_release).max(value.abs());
                self.update_hold(seconds, hold_release);
            }
        }
        let db = |amplitude: f64| (20.0 * amplitude.max(1e-3).log10()).max(-60.0);
        MeterLevels {
            rms_db: db(self.power.sqrt()),
            peak_db: db(self.peak),
            hold_db: db(self.hold),
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
    /// Only one producer may own free slots at a time. Contending taps are dropped.
    producing: AtomicBool,
    /// Per-slot interior mutability allows producer and consumer to access distinct slots.
    ring: Box<[UnsafeCell<f32>]>,
    image: Mutex<Vec<u8>>,
    generation: AtomicU64,
    levels: Mutex<MeterLevels>,
}

// SAFETY: producing excludes concurrent producers. Release/acquire cursors transfer
// individual slots between the producer and the sole analyzer consumer. Image and
// levels are Mutex-protected; remaining fields are atomics.
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
            producing: AtomicBool::new(false),
            ring: (0..RING).map(|_| UnsafeCell::new(0.0)).collect(),
            image: Mutex::new(vec![0; IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL]),
            generation: AtomicU64::new(0),
            levels: Mutex::new(MeterLevels::default()),
        }
    }

    fn accepting(&self) -> bool {
        self.open.load(Ordering::Acquire) && !self.frozen.load(Ordering::Acquire)
    }

    fn push<T: crate::dsp::Sample>(&self, samples: &[T]) {
        if self
            .producing
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        let _guard = ProducerGuard(&self.producing);
        let mut write = self.write.load(Ordering::Relaxed);
        let mut read = self.read.load(Ordering::Acquire);
        for &sample in samples {
            if write.wrapping_sub(read) >= RING - 1 {
                read = self.read.load(Ordering::Acquire);
                if write.wrapping_sub(read) >= RING - 1 {
                    continue;
                }
            }
            let value = sample.normalized() as f32;
            // SAFETY: the producer guard owns this free slot until publication.
            unsafe { *self.ring[write % RING].get() = value };
            write = write.wrapping_add(1);
        }
        self.write.store(write, Ordering::Release);
    }

    fn drain(&self, output: &mut Vec<f32>) {
        let write = self.write.load(Ordering::Acquire);
        let mut read = self.read.load(Ordering::Relaxed);
        while read != write {
            // SAFETY: the acquired write cursor published this slot. The producer
            // cannot reuse it until we publish the advanced read cursor below.
            output.push(unsafe { *self.ring[read % RING].get() });
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
        // Do not publish a pointer until spawning succeeds: a failed spawn must
        // never free state still borrowed by an audio tap.
        let worker = thread::Builder::new()
            .name("spectrum-analyzer".into())
            .spawn(move || analyzer_loop(worker_shared))
            .map_err(|error| format!("Cannot start spectrum analyzer thread: {error}"))?;
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
            shared.stop.store(true, Ordering::Release);
            let _ = worker.join();
            return Err("Spectrum window is already open".into());
        }
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub fn levels(&self) -> MeterLevels {
        *self.shared.levels.lock().expect("spectrum levels lock")
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
pub fn push_output_samples<T: crate::dsp::Sample>(samples: &[T]) {
    IN_USE.fetch_add(1, Ordering::SeqCst);
    let _lease = TapLease;
    let shared = ACTIVE.load(Ordering::SeqCst);
    // SAFETY: the lease keeps controller teardown from freeing this pointer.
    if let Some(shared) = unsafe { shared.as_ref() } {
        if shared.accepting() {
            shared.push(samples);
        }
    }
}

fn hann(size: usize) -> Vec<f64> {
    (0..size).map(|i| window::hann(i, size)).collect()
}

fn db_to_bgra(db: f64) -> [u8; 4] {
    let t = ((db - DB_MIN) / (DB_MAX - DB_MIN)).clamp(0.0, 1.0);
    // Blue -> cyan -> green -> yellow -> red, stored in BGRA order.
    let ramp = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let blue = ramp(2.0 - 4.0 * t);
    let green = ramp((4.0 * t).min(4.0 - 4.0 * t));
    let red = ramp(4.0 * t - 2.0);
    [blue, green, red, 255]
}

fn magnitude_at(bins: &[f64], fft_size: usize, hz: f64, rate: u32) -> f64 {
    let bin = hz * fft_size as f64 / rate as f64;
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

const DISPLAY_COLUMNS_PER_SECOND: u64 = 60;
#[derive(Default)]
struct DisplayCadence {
    phase: u64,
}
impl DisplayCadence {
    fn advance(&mut self, frames: usize, rate: u32) -> bool {
        self.phase += frames as u64 * DISPLAY_COLUMNS_PER_SECOND;
        if self.phase >= rate as u64 {
            self.phase %= rate as u64;
            true
        } else {
            false
        }
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
    let mut cadence = DisplayCadence::default();
    let mut meter = LevelMeter::new();
    let mut last_meter = Instant::now();
    let mut last_signal = last_meter;
    let mut rate = SAMPLE_RATE.load(Ordering::Acquire) as u32;

    while !shared.stop.load(Ordering::Acquire) {
        if shared.frozen.load(Ordering::Acquire) {
            last_meter = Instant::now();
            last_signal = last_meter;
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        let selected_rate = SAMPLE_RATE.load(Ordering::Acquire) as u32;
        if rate != selected_rate {
            rate = selected_rate;
            cadence = DisplayCadence::default();
            time_domain.clear();
            meter = LevelMeter::new();
            let mut image = shared.image.lock().expect("spectrum image lock");
            image.fill(0);
        }
        shared.drain(&mut pending);
        let now = Instant::now();
        // Brief empty polls are normal between callbacks; do not count them as
        // extra silence in addition to the sample time already measured.
        if !pending.is_empty() {
            let levels = meter.update_at_rate(&pending, 0.0, rate);
            *shared.levels.lock().expect("spectrum levels lock") = levels;
            last_signal = now;
        } else if now.duration_since(last_signal) >= Duration::from_millis(100) {
            let levels =
                meter.update_at_rate(&[], now.duration_since(last_meter).as_secs_f64(), rate);
            *shared.levels.lock().expect("spectrum levels lock") = levels;
        }
        last_meter = now;
        if pending.is_empty() {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        time_domain.append(&mut pending);

        let wideband = shared.wideband.load(Ordering::Acquire);
        if wideband != using_wide {
            using_wide = wideband;
            cadence = DisplayCadence::default();
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
            // Analyze every overlapping FFT frame, but only shift/copy the image
            // at a fixed rate. The visible time span is independent of sample rate.
            if !cadence.advance(hop, rate) {
                time_domain.drain(..hop);
                continue;
            }
            for (y, pixel) in column.iter_mut().enumerate() {
                let t = 1.0 - y as f64 / (IMAGE_HEIGHT - 1) as f64;
                let hz = FREQ_MIN + (FREQ_MAX - FREQ_MIN) * t;
                let mag = magnitude_at(bins, fft_size, hz, rate);
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
    fn display_cadence_keeps_the_same_time_span_for_both_fft_modes() {
        // Include future high rates to prevent restoring the original 4x copy load.
        for rate in [44100, 48000, 88200, 96000, 192000] {
            for hop in [WIDE_HOP, NARROW_HOP] {
                let mut cadence = DisplayCadence::default();
                let frames = rate as usize * 10 / hop;
                let columns = (0..frames).filter(|_| cadence.advance(hop, rate)).count();
                assert_eq!(
                    columns,
                    frames * hop * DISPLAY_COLUMNS_PER_SECOND as usize / rate as usize
                );
                assert!(
                    (599..=600).contains(&columns),
                    "rate={rate}, hop={hop}, columns={columns}"
                );
            }
        }
    }
    #[test]
    fn frequency_mapping_and_meter_timing_follow_selected_rate() {
        for rate in crate::SAMPLE_RATES {
            let fft = Fft::new(NARROW_FFT);
            let hz = rate as f64 * 43.0 / NARROW_FFT as f64;
            let mut data: Vec<_> = (0..NARROW_FFT)
                .map(|i| Complex {
                    re: (std::f64::consts::TAU * hz * i as f64 / rate as f64).sin(),
                    im: 0.0,
                })
                .collect();
            fft.forward(&mut data);
            let bins: Vec<_> = data[..=NARROW_FFT / 2]
                .iter()
                .map(|c| c.re.hypot(c.im))
                .collect();
            assert!(magnitude_at(&bins, NARROW_FFT, hz, rate) > NARROW_FFT as f64 * 0.49);
            let levels =
                LevelMeter::new().update_at_rate(&vec![0.1; rate as usize / 10], 0.0, rate);
            // After 100 ms the RMS meter has passed one time constant at every rate.
            let expected = 10.0 * (0.01 * (1.0 - (-1.0_f64).exp())).log10();
            assert!((levels.rms_db - expected).abs() < 0.0001);
        }
    }

    #[test]
    fn meter_measures_dbfs_and_releases_after_stop() {
        let mut meter = LevelMeter::new();
        let signal: Vec<f32> = (0..RATE)
            .map(|i| (0.5 * (std::f64::consts::TAU * 1000.0 * i as f64 / RATE as f64).sin()) as f32)
            .collect();
        let levels = meter.update(&signal, 1.0);
        assert!((levels.peak_db + 6.0206).abs() < 0.01);
        assert!((levels.rms_db + 9.0309).abs() < 0.1);
        let silent = meter.update(&[], 0.5);
        assert_eq!(silent.hold_db, levels.hold_db);
        assert!(silent.peak_db < levels.peak_db);
        let silent = meter.update(&[], 10.0);
        assert_eq!(silent, MeterLevels::default());
    }

    #[test]
    fn meter_is_independent_of_block_boundaries() {
        let mut signal = vec![0.0_f32; RATE as usize * 2];
        signal[0] = 1.0;
        signal[6000] = -0.5;
        let mut whole = LevelMeter::new();
        let expected = whole.update(&signal, 0.0);
        for block_size in [1, 64, 512, 2048] {
            let mut split = LevelMeter::new();
            let mut actual = MeterLevels::default();
            for block in signal.chunks(block_size) {
                actual = split.update(block, 0.0);
            }
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn meter_handles_nonfinite_and_overload_samples() {
        let mut meter = LevelMeter::new();
        assert_eq!(
            meter.update(&[f32::NAN, f32::INFINITY], 0.0),
            MeterLevels::default()
        );
        assert!(meter.update(&[2.0], 0.0).peak_db > 0.0);
    }

    #[test]
    fn ring_drops_when_full_without_blocking() {
        let shared = Shared::new();
        let block = vec![0.25_f32; 1024];
        for _ in 0..RING {
            shared.push(&block);
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
    fn ring_transfers_samples_across_counter_overflow() {
        let shared = Shared::new();
        shared.write.store(usize::MAX - 1, Ordering::Relaxed);
        shared.read.store(usize::MAX - 1, Ordering::Relaxed);
        // Fill across the wrap: a non-power-of-two ring would reuse an unread
        // pre-wrap slot before reaching its advertised capacity.
        let input: Vec<_> = (1..RING).map(|i| i as f32).collect();
        shared.push(&input);
        let mut output = Vec::new();
        shared.drain(&mut output);
        assert_eq!(output, input);
        shared.push(&[5.0_f32]);
        output.clear();
        shared.drain(&mut output);
        assert_eq!(output, [5.0]);
    }

    #[test]
    fn concurrent_taps_keep_each_producers_samples_in_order() {
        let shared = Arc::new(Shared::new());
        let done = AtomicUsize::new(0);
        let mut output = Vec::new();
        thread::scope(|scope| {
            for sign in [1.0_f32, -1.0] {
                let shared = &shared;
                let done = &done;
                scope.spawn(move || {
                    let count = if cfg!(miri) { 32 } else { 10_000 };
                    for i in 1..=count {
                        shared.push(&[sign * i as f32]);
                        thread::yield_now();
                    }
                    done.fetch_add(1, Ordering::Release);
                });
            }
            while done.load(Ordering::Acquire) != 2 {
                shared.drain(&mut output);
                thread::yield_now();
            }
            shared.drain(&mut output);
        });
        assert!(!output.is_empty());
        let mut previous = [0.0; 2];
        for sample in output {
            let producer = usize::from(sample < 0.0);
            assert!(sample.abs() > previous[producer]);
            previous[producer] = sample.abs();
        }
    }

    #[derive(Clone, Copy, Default, PartialEq)]
    struct PanickingSample;
    impl crate::dsp::Sample for PanickingSample {
        fn normalized(self) -> f64 {
            panic!("test conversion failure");
        }
        fn from_normalized(_: f64) -> Self {
            Self
        }
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
        assert_eq!(controller.levels(), MeterLevels::default());
        controller.set_frozen(true);
        push_output_samples(&[1.0_f32; 512]);
        assert_eq!(controller.shared.write.load(Ordering::Acquire), 0);
        assert_eq!(controller.levels(), MeterLevels::default());
        controller.set_frozen(false);
        assert!(std::panic::catch_unwind(|| push_output_samples(&[PanickingSample])).is_err());
        assert_eq!(IN_USE.load(Ordering::SeqCst), 0);
        assert!(!controller.shared.producing.load(Ordering::Acquire));
        push_output_samples(&[0.5_f32; 512]);
        let deadline = Instant::now() + Duration::from_secs(2);
        while controller.levels().peak_db <= -60.0 {
            assert!(Instant::now() < deadline, "analyzer did not publish levels");
            thread::sleep(Duration::from_millis(1));
        }
        drop(controller);
        assert!(ACTIVE.load(Ordering::SeqCst).is_null());
        push_output_samples(&[1.0_f32]);
    }
}
