//! Owns the ASIO worker independently of the presentation layer.
use crate::{asio, cli::Options};
use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
unsafe extern "C" {
    fn voice_asio_com_initialize() -> i32;
    fn voice_asio_com_uninitialize();
}
struct ComApartment;
impl ComApartment {
    fn initialize() -> Result<Self, Box<dyn Error>> {
        // SAFETY: called only on the owning audio worker, before loading ASIO.
        let result = unsafe { voice_asio_com_initialize() };
        if result < 0 {
            return Err(format!(
                "Audio worker COM initialization failed: HRESULT 0x{:08x}",
                result as u32
            )
            .into());
        }
        Ok(Self)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: RAII value is local to run_audio; the driver is dropped first.
        unsafe { voice_asio_com_uninitialize() };
    }
}

pub struct Session {
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
}
impl Session {
    pub fn new() -> Self {
        Self {
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new("Ready. Inspect settings or start audio.".into())),
        }
    }
    pub fn start(&mut self, options: Options) -> Result<(), String> {
        if self.poll() {
            return Err("Audio is already active".into());
        }
        self.stop.store(false, Ordering::Release);
        let stop = Arc::clone(&self.stop);
        let status = Arc::clone(&self.status);
        set_status(&status, "Opening ASIO driver...".into());
        match thread::Builder::new()
            .name("asio-control".into())
            .spawn(move || {
                let result = run_audio(options, &stop, &status);
                if let Err(error) = result {
                    let previous = status.lock().unwrap().clone();
                    set_status(&status, format!("ERROR: {error}\n\n{previous}"));
                }
            }) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => return Err(format!("Cannot start audio thread: {error}")),
        }
        Ok(())
    }
    pub fn poll(&mut self) -> bool {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
            && self.worker.take().unwrap().join().is_err()
        {
            set_status(&self.status, "ERROR: Audio worker panicked.".into());
        }
        self.worker.is_some()
    }
}
impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
impl Session {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }
    pub fn status(&self) -> String {
        self.status.lock().unwrap().clone()
    }
    pub fn shutdown(&mut self) {
        self.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn set_status(status: &Mutex<String>, message: String) {
    *status.lock().unwrap() = message;
}

fn run_audio(
    options: Options,
    stop: &AtomicBool,
    status: &Mutex<String>,
) -> Result<(), Box<dyn Error>> {
    let _apartment = ComApartment::initialize()?;
    let host = asio_sys::Asio::new();
    let name = host
        .driver_names()
        .into_iter()
        .find(|name| name.eq_ignore_ascii_case(&options.device))
        .ok_or_else(|| format!("ASIO device '{}' not found", options.device))?;
    let driver = host.load_driver(&name)?;
    let config = asio::Configuration::query(&driver)?;
    let details = format!("Device: {name}\n{}", config.summary());
    set_status(status, details.clone());
    config.validate(options.frames)?;
    if options.inspect {
        set_status(
            status,
            format!("Inspection OK. No audio buffers created.\n{details}"),
        );
        return Ok(());
    }
    // Cancellation during driver initialization must not start playback afterward.
    if stop.load(Ordering::Acquire) {
        set_status(status, format!("Stopped before playback.\n{details}"));
        return Ok(());
    }
    let mut engine = asio::Engine::open(
        driver,
        config,
        options.frames,
        options.pitch,
        options.preserve_formants,
        options.formant_shift,
        crate::gate::GateSettings {
            enabled: options.gate,
            threshold: options.gate_threshold,
            attack_ms: options.gate_attack_ms,
            release_ms: options.gate_release_ms,
        },
    )?;
    if stop.load(Ordering::Acquire) {
        set_status(status, format!("Stopped before playback.\n{details}"));
        return Ok(());
    }
    engine.start()?;
    let started = Instant::now();
    let mut last_progress = Instant::now();
    let mut last_blocks = 0;
    let result: Result<(), Box<dyn Error>> = (|| {
        while !stop.load(Ordering::Acquire) {
            engine.check_events()?;
            let blocks = engine.blocks();
            if blocks != last_blocks {
                last_progress = Instant::now();
                last_blocks = blocks;
            }
            if last_progress.elapsed() > Duration::from_secs(2) {
                return Err("No ASIO callbacks for 2 seconds".into());
            }
            set_status(status, format!(
                "Running: {:.1} s | pitch {} semitones | formant {} | shift {} semitones | gate {}\n{details}\nPeriod: {:.3} ms | Input 1 -> output 1\n{}\n\n{}\n\nNonzero input counts indicate signal arrival, not listening quality.\nTimestamp blocks=0 means gap detection is unavailable.",
                started.elapsed().as_secs_f64(), options.pitch,
                if options.preserve_formants { "preserve" } else { "off" },
                options.formant_shift,
                if options.gate { "on" } else { "off" },
                options.frames as f64 * 1000.0 / crate::RATE as f64,
                engine.latency_summary()?, engine.summary()
            ));
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    })();
    let stop_result = engine.stop();
    let summary = format!(
        "Stopped: {:.1} s\n{details}\n{}\n{}",
        started.elapsed().as_secs_f64(),
        engine.latency_summary()?,
        engine.summary()
    );
    set_status(status, summary);
    result?;
    stop_result?;
    engine.check_events()?;
    // A user cancelling immediately may stop before the first callback.
    if engine.blocks() > 0 {
        engine.check_test_result()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_and_shutdown_join_worker_without_audio_hardware() {
        let mut session = Session::new();
        let stop = Arc::clone(&session.stop);
        session.worker = Some(thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                thread::yield_now();
            }
        }));
        assert!(session.poll());
        assert!(session
            .start(Options::parse_gui(Vec::new()).unwrap())
            .is_err());
        session.shutdown();
        assert!(!session.poll());
    }
    #[test]
    fn completed_worker_can_be_reaped_and_panics_are_reported() {
        let mut session = Session::new();
        let worker = thread::spawn(|| panic!("test worker failure"));
        while !worker.is_finished() {
            thread::yield_now();
        }
        session.worker = Some(worker);
        assert!(!session.poll());
        assert!(session.status().contains("panicked"));
    }
}
