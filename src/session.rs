//! Owns the audio worker independently of the presentation layer.
use crate::{audio::DspSettings, backend, cli::Options};
use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[cfg(all(windows, feature = "asio"))]
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
#[cfg(all(windows, feature = "asio"))]
pub(crate) struct ComApartment(std::marker::PhantomData<std::rc::Rc<()>>);
#[cfg(all(windows, feature = "asio"))]
impl ComApartment {
    pub(crate) fn initialize() -> Result<Self, Box<dyn Error>> {
        // SAFETY: called on the owning control thread, before loading ASIO.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result.is_err() {
            return Err(format!(
                "ASIO control thread COM initialization failed: HRESULT 0x{:08x}",
                result.0 as u32
            )
            .into());
        }
        Ok(Self(std::marker::PhantomData))
    }
}
#[cfg(all(windows, feature = "asio"))]
impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: the non-Send guard stays on its initializing thread; the driver drops first.
        unsafe { CoUninitialize() };
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "desktop", derive(serde::Serialize))]
#[cfg_attr(feature = "desktop", serde(rename_all = "camelCase"))]
pub struct AudioInfo {
    pub phase: &'static str,
    pub error: Option<String>,
    pub device: Option<String>,
    pub backend: Option<String>,
    pub output_device: Option<String>,
    pub sample_rate: Option<f64>,
    pub inputs: Option<usize>,
    pub outputs: Option<usize>,
    pub frames: Option<u32>,
    pub period_ms: Option<f64>,
    pub dsp_ms: Option<f64>,
    pub input_latency_ms: Option<f64>,
    pub output_latency_ms: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "desktop", derive(serde::Serialize))]
#[cfg_attr(feature = "desktop", serde(rename_all = "camelCase"))]
pub struct AudioLog {
    pub elapsed_seconds: Option<f64>,
    pub telemetry: Option<backend::AudioTelemetry>,
}

pub struct Session {
    worker: Option<JoinHandle<()>>,
    info: Arc<Mutex<AudioInfo>>,
    log: Arc<Mutex<AudioLog>>,
    logging: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
}
impl Session {
    pub fn new() -> Self {
        Self {
            worker: None,
            info: Arc::new(Mutex::new(AudioInfo {
                phase: "ready",
                ..AudioInfo::default()
            })),
            log: Arc::new(Mutex::new(AudioLog::default())),
            logging: Arc::new(AtomicBool::new(false)),
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
        let info = Arc::clone(&self.info);
        let log = Arc::clone(&self.log);
        *log.lock().unwrap() = AudioLog::default();
        let logging = Arc::clone(&self.logging);
        *info.lock().unwrap() = AudioInfo {
            phase: "opening",
            ..AudioInfo::default()
        };
        set_status(&status, "Opening audio device...".into());
        match thread::Builder::new()
            .name("audio-control".into())
            .spawn(move || {
                let result = run_audio(options, &stop, &status, &info, &log, &logging);
                if let Err(error) = result {
                    let mut info = info.lock().unwrap();
                    info.phase = "error";
                    info.error = Some(error.to_string());
                    let previous = status.lock().unwrap().clone();
                    set_status(&status, format!("ERROR: {error}\n\n{previous}"));
                }
            }) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => {
                let message = format!("Cannot start audio thread: {error}");
                let mut info = self.info.lock().unwrap();
                info.phase = "error";
                info.error = Some(message.clone());
                return Err(message);
            }
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
            let mut info = self.info.lock().unwrap();
            info.phase = "error";
            info.error = Some("Audio worker panicked".into());
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
    pub fn set_logging(&self, logging: bool) {
        self.logging.store(logging, Ordering::Release);
    }
    pub fn info(&self) -> AudioInfo {
        self.info.lock().unwrap().clone()
    }
    pub fn log_info(&self) -> Option<AudioLog> {
        if !self.logging.load(Ordering::Acquire) {
            return None;
        }
        Some(self.log.lock().unwrap().clone())
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

fn update_configuration(
    info: &Mutex<AudioInfo>,
    config: &backend::AudioConfiguration,
    options: &Options,
    phase: &'static str,
    kind: backend::BackendKind,
) {
    *info.lock().unwrap() = AudioInfo {
        phase,
        device: Some(config.device.clone()),
        backend: Some(kind.name().into()),
        output_device: Some(config.output_device.clone()),
        sample_rate: Some(config.sample_rate),
        inputs: Some(config.inputs),
        outputs: Some(config.outputs),
        frames: Some(options.frames),
        period_ms: Some(options.frames as f64 * 1000.0 / config.sample_rate),
        ..AudioInfo::default()
    };
}

fn run_audio(
    options: Options,
    stop: &AtomicBool,
    status: &Mutex<String>,
    info: &Mutex<AudioInfo>,
    log: &Mutex<AudioLog>,
    logging: &AtomicBool,
) -> Result<(), Box<dyn Error>> {
    let kind = backend::resolve(options.backend);
    #[cfg(all(windows, feature = "asio"))]
    let _apartment = if kind == backend::BackendKind::Asio {
        Some(ComApartment::initialize()?)
    } else {
        None
    };
    let backend =
        backend::select_with_sample_rate(kind, &options.output_device, options.sample_rate)?;
    let routing = backend::ChannelRouting::from(&options);
    if options.inspect {
        let inspection =
            backend.inspect_configuration(&options.device, options.frames, &routing)?;
        update_configuration(info, &inspection.configuration, &options, "inspected", kind);
        set_status(
            status,
            format!(
                "Inspection OK. No audio buffers created.\n{}",
                inspection.details
            ),
        );
        return Ok(());
    }
    // Cancellation before or during driver initialization must not start playback afterward.
    if stop.load(Ordering::Acquire) {
        info.lock().unwrap().phase = "stopped";
        set_status(status, "Stopped before playback.".into());
        return Ok(());
    }
    let mut engine = backend.open(
        &options.device,
        options.frames,
        &routing,
        DspSettings::from(&options),
    )?;
    let configuration = engine.configuration();
    update_configuration(info, &configuration, &options, "opening", kind);
    let details = format!(
        "Device: {}\n{}\nPitch algorithm: {}",
        configuration.device,
        engine.details(),
        options.pitch.algorithm.name()
    );
    set_status(status, details.clone());
    if stop.load(Ordering::Acquire) {
        info.lock().unwrap().phase = "stopped";
        set_status(status, format!("Stopped before playback.\n{details}"));
        return Ok(());
    }
    #[cfg(windows)]
    crate::spectrum::set_sample_rate(options.sample_rate);
    engine.start()?;
    let (input, output) = engine.latency_ms()?;
    {
        let mut info = info.lock().unwrap();
        info.phase = "running";
        info.dsp_ms = Some(
            if !options.voice_enabled
                || (options.pitch.semitones == 0.0 && options.pitch.formant_shift == 0.0)
            {
                0.0
            } else {
                options.pitch.latency() as f64 * 1000.0 / configuration.sample_rate
            },
        );
        info.input_latency_ms = input;
        info.output_latency_ms = output;
    }
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
                return Err("No audio callbacks for 2 seconds".into());
            }
            if logging.load(Ordering::Acquire) {
                let (input, output) = engine.latency_ms()?;
                let mut info = info.lock().unwrap();
                *log.lock().unwrap() = AudioLog {
                    elapsed_seconds: Some(started.elapsed().as_secs_f64()),
                    telemetry: Some(engine.telemetry()),
                };
                info.input_latency_ms = input;
                info.output_latency_ms = output;
            }
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    })();
    let stop_result = engine.stop();
    info.lock().unwrap().phase = "stopped";
    if logging.load(Ordering::Acquire) {
        *log.lock().unwrap() = AudioLog {
            elapsed_seconds: Some(started.elapsed().as_secs_f64()),
            telemetry: Some(engine.telemetry()),
        };
    }
    let summary = format!(
        "Stopped.\n{details}\n{}\n{}",
        engine.latency_summary()?,
        engine.summary()
    );
    set_status(status, summary);
    result?;
    stop_result?;
    engine.check_events()?;
    // A user cancelling immediately may stop before the first callback.
    if engine.has_playback() {
        engine.check_test_result()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(windows, feature = "asio"))]
    #[test]
    fn asio_discovery_can_initialize_and_release_com_on_a_fresh_worker() {
        std::thread::spawn(|| {
            let _outer = ComApartment::initialize().unwrap();
            {
                let _nested = ComApartment::initialize().unwrap();
            }
            let _still_initialized = ComApartment::initialize().unwrap();
        })
        .join()
        .unwrap();
    }
    #[test]
    fn status_uses_opened_devices_and_keeps_requested_period_separate() {
        let options = Options::parse(
            ["--backend", "cpal", "--frames", "64"]
                .into_iter()
                .map(String::from),
        )
        .unwrap();
        let configuration = backend::AudioConfiguration {
            device: "Actual microphone".into(),
            output_device: "Actual headphones".into(),
            sample_rate: crate::RATE as f64,
            inputs: 1,
            outputs: 2,
        };
        let info = Mutex::new(AudioInfo::default());
        update_configuration(
            &info,
            &configuration,
            &options,
            "opening",
            backend::BackendKind::Cpal,
        );
        let info = info.lock().unwrap();
        assert_eq!(info.device.as_deref(), Some("Actual microphone"));
        assert_eq!(info.output_device.as_deref(), Some("Actual headphones"));
        assert_eq!(info.backend.as_deref(), Some("cpal"));
        assert_eq!(info.frames, Some(64));
        assert_eq!(info.period_ms, Some(64.0 * 1000.0 / crate::RATE as f64));
    }

    #[test]
    fn logging_is_opt_in_and_keeps_initial_status_stable() {
        let session = Session::new();
        assert!(!session.logging.load(Ordering::Acquire));
        assert_eq!(session.info().phase, "ready");
        assert!(session.log.lock().unwrap().telemetry.is_none());
        assert!(session.log_info().is_none());
        {
            let mut log = session.log.lock().unwrap();
            log.elapsed_seconds = Some(1.0);
            log.telemetry = Some(backend::AudioTelemetry::default());
        }
        session.set_logging(true);
        assert!(session.logging.load(Ordering::Acquire));
        assert_eq!(session.log_info().unwrap().elapsed_seconds, Some(1.0));
        let status = session.info();
        session.log.lock().unwrap().elapsed_seconds = Some(2.0);
        assert_eq!(session.info(), status);
        session.set_logging(false);
        assert!(!session.logging.load(Ordering::Acquire));
        assert!(session.log_info().is_none());
        assert_eq!(session.info().phase, "ready");
    }
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
        assert_eq!(session.info().phase, "error");
        assert_eq!(
            session.info().error.as_deref(),
            Some("Audio worker panicked")
        );
    }
}
