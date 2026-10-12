//! UI callbacks run on the main thread. The worker owns the entire ASIO lifetime;
//! driver callbacks own DSP. No GUI locks or messages occur in audio callbacks.
use crate::{asio, cli::Options, gui_settings, spectrum::SpectrumController};
use std::{
    error::Error,
    ffi::c_void,
    path::PathBuf,
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
    fn voice_gui_message(title: *const u16, message: *const u16, error: i32);
    fn voice_gui_run(
        context: *mut c_void,
        device: *const u16,
        devices: *const u16,
        frames: *const u16,
        pitch: *const u16,
        formant_shift: *const u16,
        preserve: i32,
        gate: i32,
        gate_threshold: *const u16,
        gate_attack: *const u16,
        gate_release: *const u16,
        save: extern "C" fn(
            *mut c_void,
            *const u16,
            *const u16,
            *const u16,
            *const u16,
            i32,
            i32,
            *const u16,
            *const u16,
            *const u16,
        ) -> i32,
        start: extern "C" fn(
            *mut c_void,
            *const u16,
            *const u16,
            *const u16,
            *const u16,
            i32,
            i32,
            *const u16,
            *const u16,
            *const u16,
            i32,
        ),
        stop: extern "C" fn(*mut c_void),
        poll: extern "C" fn(*mut c_void, *mut u16, usize) -> i32,
        spectrum_open: extern "C" fn(*mut c_void) -> i32,
        spectrum_close: extern "C" fn(*mut c_void),
        spectrum_set_wideband: extern "C" fn(*mut c_void, i32),
        spectrum_set_frozen: extern "C" fn(*mut c_void, i32),
        spectrum_poll: extern "C" fn(*mut c_void, *mut u8, usize, *mut u64) -> i32,
    ) -> u32;
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

struct Session {
    settings_path: PathBuf,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
    spectrum: Option<SpectrumController>,
    spectrum_generation: u64,
}
impl Session {
    fn new(settings_path: PathBuf) -> Self {
        Self {
            settings_path,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new("Ready. Inspect settings or start audio.".into())),
            spectrum: None,
            spectrum_generation: 0,
        }
    }
    fn start(&mut self, options: Options) {
        if self.worker.is_some() {
            return;
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
            Err(error) => set_status(&self.status, format!("Cannot start audio thread: {error}")),
        }
    }
    fn poll(&mut self) -> bool {
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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
// SAFETY: native/gui.cpp passes live null-terminated UTF-16 control text,
// and the context is the unique Session on the UI thread throughout voice_gui_run.
unsafe fn read_wide(pointer: *const u16) -> String {
    let mut length = 0;
    while unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(pointer, length) })
}
extern "C" fn start(
    context: *mut c_void,
    device: *const u16,
    frames: *const u16,
    pitch: *const u16,
    formant_shift: *const u16,
    preserve: i32,
    gate: i32,
    gate_threshold: *const u16,
    gate_attack: *const u16,
    gate_release: *const u16,
    inspect: i32,
) {
    // SAFETY: see the native callback contract above. No worker accesses Session.
    let session = unsafe { &mut *context.cast::<Session>() };
    let args = unsafe {
        vec![
            "--device".into(),
            read_wide(device),
            "--frames".into(),
            read_wide(frames),
            "--pitch".into(),
            read_wide(pitch),
            "--formant-shift".into(),
            read_wide(formant_shift),
            "--formant".into(),
            if preserve != 0 { "preserve" } else { "off" }.into(),
            "--gate".into(),
            if gate != 0 { "on" } else { "off" }.into(),
            "--gate-threshold".into(),
            read_wide(gate_threshold),
            "--gate-attack".into(),
            read_wide(gate_attack),
            "--gate-release".into(),
            read_wide(gate_release),
        ]
    };
    match Options::parse(args) {
        Ok(mut options) => {
            options.inspect = inspect != 0;
            session.start(options);
        }
        Err(error) => set_status(&session.status, format!("ERROR: {error}")),
    }
}
extern "C" fn stop(context: *mut c_void) {
    // SAFETY: Session remains alive until the native message loop exits.
    let session = unsafe { &mut *context.cast::<Session>() };
    session.stop.store(true, Ordering::Release);
}
extern "C" fn save(
    context: *mut c_void,
    device: *const u16,
    frames: *const u16,
    pitch: *const u16,
    formant_shift: *const u16,
    preserve: i32,
    gate: i32,
    gate_threshold: *const u16,
    gate_attack: *const u16,
    gate_release: *const u16,
) -> i32 {
    // SAFETY: the UI supplies live UTF-16 text and exclusively owns Session.
    let session = unsafe { &mut *context.cast::<Session>() };
    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut args = unsafe {
            vec![
                "--frames".into(),
                read_wide(frames),
                "--pitch".into(),
                read_wide(pitch),
                "--formant-shift".into(),
                read_wide(formant_shift),
                "--formant".into(),
                if preserve != 0 { "preserve" } else { "off" }.into(),
                "--gate".into(),
                if gate != 0 { "on" } else { "off" }.into(),
                "--gate-threshold".into(),
                read_wide(gate_threshold),
                "--gate-attack".into(),
                read_wide(gate_attack),
                "--gate-release".into(),
                read_wide(gate_release),
            ]
        };
        let device = unsafe { read_wide(device) };
        if !device.is_empty() {
            args.extend(["--device".into(), device]);
        }
        let options = Options::parse_gui(args)?;
        gui_settings::save(&session.settings_path, &options)
    })();
    match result {
        Ok(()) => 1,
        Err(error) => {
            show_message(&format!("GUI settings were not saved: {error}\nCorrect the settings or storage problem, then close again."), true);
            0
        }
    }
}
extern "C" fn poll(context: *mut c_void, output: *mut u16, capacity: usize) -> i32 {
    // SAFETY: UI thread supplies a unique Session and a writable capacity-sized buffer.
    let session = unsafe { &mut *context.cast::<Session>() };
    let busy = session.poll();
    let text = wide(&session.status.lock().unwrap().replace('\n', "\r\n"));
    if capacity > 0 {
        let count = text.len().min(capacity - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), output, count);
            *output.add(count) = 0;
        }
    }
    i32::from(busy)
}
extern "C" fn spectrum_open(context: *mut c_void) -> i32 {
    // SAFETY: UI thread exclusively owns Session for the native message loop.
    let session = unsafe { &mut *context.cast::<Session>() };
    if session.spectrum.is_some() {
        return 1;
    }
    match SpectrumController::start() {
        Ok(controller) => {
            session.spectrum_generation = 0;
            session.spectrum = Some(controller);
            1
        }
        Err(error) => {
            set_status(&session.status, format!("ERROR: {error}"));
            0
        }
    }
}
extern "C" fn spectrum_close(context: *mut c_void) {
    // SAFETY: see spectrum_open.
    let session = unsafe { &mut *context.cast::<Session>() };
    session.spectrum = None;
    session.spectrum_generation = 0;
}
extern "C" fn spectrum_set_wideband(context: *mut c_void, wideband: i32) {
    // SAFETY: see spectrum_open.
    let session = unsafe { &mut *context.cast::<Session>() };
    if let Some(spectrum) = &session.spectrum {
        spectrum.set_wideband(wideband != 0);
    }
}
extern "C" fn spectrum_set_frozen(context: *mut c_void, frozen: i32) {
    // SAFETY: see spectrum_open.
    let session = unsafe { &mut *context.cast::<Session>() };
    if let Some(spectrum) = &session.spectrum {
        spectrum.set_frozen(frozen != 0);
    }
}
extern "C" fn spectrum_poll(
    context: *mut c_void,
    pixels: *mut u8,
    capacity: usize,
    generation: *mut u64,
) -> i32 {
    // SAFETY: UI thread owns Session; pixels/generation are writable buffers from the native window.
    let session = unsafe { &mut *context.cast::<Session>() };
    let Some(spectrum) = &session.spectrum else {
        return 0;
    };
    let need = crate::spectrum::IMAGE_WIDTH
        * crate::spectrum::IMAGE_HEIGHT
        * crate::spectrum::BYTES_PER_PIXEL;
    if capacity < need || pixels.is_null() || generation.is_null() {
        set_status(
            &session.status,
            "ERROR: Spectrum poll received an undersized or null image buffer.".into(),
        );
        return 0;
    }
    let last = unsafe { *generation };
    let mut last_generation = last;
    let pixels = unsafe { std::slice::from_raw_parts_mut(pixels, need) };
    if spectrum.copy_image(pixels, &mut last_generation) {
        session.spectrum_generation = last_generation;
        unsafe { *generation = last_generation };
        1
    } else {
        0
    }
}
/// Load preferences, then apply only explicitly supplied GUI arguments.
pub fn initial_options(args: impl IntoIterator<Item = String>) -> Result<Options, Box<dyn Error>> {
    let args: Vec<_> = args.into_iter().collect();
    if args.iter().any(|arg| arg == "--help") {
        return Options::parse_gui(args);
    }
    gui_settings::merge(&gui_settings::load(&gui_settings::path()?)?, args)
}

pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let mut session = Session::new(gui_settings::path()?);
    let device = wide(&options.device);
    // Multi-string: one null per device and a final null terminator.
    let devices = wide(&asio_sys::Asio::new().driver_names().join("\0"));
    let devices = [devices, vec![0]].concat();
    let frames = wide(&options.frames.to_string());
    let pitch = wide(&options.pitch.to_string());
    let formant_shift = wide(&options.formant_shift.to_string());
    let gate_threshold = wide(&match options.gate_threshold {
        crate::gate::GateThreshold::Auto => "auto".to_string(),
        crate::gate::GateThreshold::Db(db) => db.to_string(),
    });
    let gate_attack = wide(&options.gate_attack_ms.to_string());
    let gate_release = wide(&options.gate_release_ms.to_string());
    // SAFETY: buffers and Session remain live during this synchronous native UI loop.
    let error = unsafe {
        voice_gui_run(
            (&mut session as *mut Session).cast(),
            device.as_ptr(),
            devices.as_ptr(),
            frames.as_ptr(),
            pitch.as_ptr(),
            formant_shift.as_ptr(),
            i32::from(options.preserve_formants),
            i32::from(options.gate),
            gate_threshold.as_ptr(),
            gate_attack.as_ptr(),
            gate_release.as_ptr(),
            save,
            start,
            stop,
            poll,
            spectrum_open,
            spectrum_close,
            spectrum_set_wideband,
            spectrum_set_frozen,
            spectrum_poll,
        )
    };
    if error != 0 {
        return Err(format!("GUI initialization/message loop failed: Win32 error {error}").into());
    }
    Ok(())
}

/// Show startup errors/help even when the GUI executable has no console.
pub fn show_message(message: &str, error: bool) {
    let title = wide("Rust Low-Latency Voice Changer");
    let message = wide(message);
    // SAFETY: both null-terminated UTF-16 buffers live through the modal call.
    unsafe { voice_gui_message(title.as_ptr(), message.as_ptr(), i32::from(error)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_save_callback_persists_controls_without_starting_worker() {
        let path =
            std::env::temp_dir().join(format!("voice-gui-callback-{}.txt", std::process::id()));
        let mut session = Session::new(path.clone());
        let device = wide("Example ASIO Device");
        let frames = wide("256");
        let pitch = wide("2.5");
        let shift = wide("-3");
        let threshold = wide("auto");
        let attack = wide("5");
        let release = wide("80");
        assert_eq!(
            save(
                (&mut session as *mut Session).cast(),
                device.as_ptr(),
                frames.as_ptr(),
                pitch.as_ptr(),
                shift.as_ptr(),
                1,
                1,
                threshold.as_ptr(),
                attack.as_ptr(),
                release.as_ptr(),
            ),
            1
        );
        assert!(session.worker.is_none());
        let saved = gui_settings::load(&path).unwrap();
        assert_eq!(saved.device, "Example ASIO Device");
        assert_eq!(saved.frames, 256);
        assert_eq!(saved.pitch, 2.5);
        assert_eq!(saved.formant_shift, -3.0);
        assert!(saved.preserve_formants);
        assert!(saved.gate);
        assert_eq!(saved.gate_threshold, crate::gate::GateThreshold::Auto);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn worker_cancellation_does_not_require_gui_or_audio_hardware() {
        let mut session = Session::new(PathBuf::from("unused-test-settings.txt"));
        let stop_flag = Arc::clone(&session.stop);
        session.worker = Some(thread::spawn(move || {
            while !stop_flag.load(Ordering::Acquire) {
                thread::yield_now();
            }
        }));
        stop((&mut session as *mut Session).cast());
        let deadline = Instant::now() + Duration::from_secs(2);
        while session.poll() {
            assert!(Instant::now() < deadline, "worker failed to stop");
            thread::yield_now();
        }
    }
    #[test]
    fn invalid_settings_do_not_spawn_worker_and_status_is_terminated() {
        let mut session = Session::new(PathBuf::from("unused-test-settings.txt"));
        let device = wide("Example ASIO Device");
        let frames = wide("0");
        let pitch = wide("0");
        let context = (&mut session as *mut Session).cast();
        let threshold = wide("auto");
        let attack = wide("5");
        let release = wide("80");
        start(
            context,
            device.as_ptr(),
            frames.as_ptr(),
            pitch.as_ptr(),
            pitch.as_ptr(),
            1,
            0,
            threshold.as_ptr(),
            attack.as_ptr(),
            release.as_ptr(),
            0,
        );
        assert!(session.worker.is_none());
        assert!(session.status.lock().unwrap().contains("ERROR"));
        let mut output = [99_u16; 8];
        assert_eq!(poll(context, output.as_mut_ptr(), output.len()), 0);
        assert_eq!(output[7], 0);
        assert_eq!(String::from_utf16_lossy(&output[..6]), "ERROR:");
    }
}
