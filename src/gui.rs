//! UI callbacks run on the main thread. The worker owns the entire ASIO lifetime;
//! driver callbacks own DSP. No GUI locks or messages occur in audio callbacks.
use crate::{asio, cli::Options};
use std::{
    error::Error,
    ffi::c_void,
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
        start: extern "C" fn(*mut c_void, *const u16, *const u16, *const u16, *const u16, i32, i32),
        stop: extern "C" fn(*mut c_void),
        poll: extern "C" fn(*mut c_void, *mut u16, usize) -> i32,
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
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
}
impl Session {
    fn new() -> Self {
        Self {
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new("Ready. Inspect settings or start audio.".into())),
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
                "Running: {:.1} s | pitch {} semitones | formant {} | shift {} semitones\n{details}\nPeriod: {:.3} ms | Input 1 -> output 1\n{}\n\n{}\n\nNonzero input counts indicate signal arrival, not listening quality.\nTimestamp blocks=0 means gap detection is unavailable.",
                started.elapsed().as_secs_f64(), options.pitch,
                if options.preserve_formants { "preserve" } else { "off" },
                options.formant_shift,
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
pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let mut session = Session::new();
    let device = wide(&options.device);
    // Multi-string: one null per device and a final null terminator.
    let devices = wide(&asio_sys::Asio::new().driver_names().join("\0"));
    let devices = [devices, vec![0]].concat();
    let frames = wide(&options.frames.to_string());
    let pitch = wide(&options.pitch.to_string());
    let formant_shift = wide(&options.formant_shift.to_string());
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
            start,
            stop,
            poll,
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
    fn worker_cancellation_does_not_require_gui_or_audio_hardware() {
        let mut session = Session::new();
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
        let mut session = Session::new();
        let device = wide("Example ASIO Device");
        let frames = wide("0");
        let pitch = wide("0");
        let context = (&mut session as *mut Session).cast();
        start(
            context,
            device.as_ptr(),
            frames.as_ptr(),
            pitch.as_ptr(),
            pitch.as_ptr(),
            1,
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
