//! React commands control the audio session; no WebView messages enter audio callbacks.
#[cfg(windows)]
use crate::native_spectrum;
use crate::{backend, cli::Options, gate::GateThreshold, gui_settings, session::Session};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::PathBuf, sync::Mutex};
use tauri::{ipc::Channel, Manager, State};

mod streams;
use streams::WindowStreams;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LevelControls {
    enabled: bool,
    compressor_enabled: bool,
    target_db: f64,
    max_gain_db: f64,
    floor_db: f64,
    response_ms: f64,
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
}
impl LevelControls {
    fn from_settings(settings: crate::level::LevelSettings) -> Self {
        Self {
            enabled: settings.enabled,
            compressor_enabled: settings.compressor_enabled,
            target_db: settings.target_db,
            max_gain_db: settings.max_gain_db,
            floor_db: settings.floor_db,
            response_ms: settings.response_ms,
            threshold_db: settings.threshold_db,
            ratio: settings.ratio,
            attack_ms: settings.attack_ms,
            release_ms: settings.release_ms,
        }
    }
    fn settings(&self) -> crate::level::LevelSettings {
        crate::level::LevelSettings {
            enabled: self.enabled,
            compressor_enabled: self.compressor_enabled,
            target_db: self.target_db,
            max_gain_db: self.max_gain_db,
            floor_db: self.floor_db,
            response_ms: self.response_ms,
            threshold_db: self.threshold_db,
            ratio: self.ratio,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EqBandControls {
    frequency: f64,
    gain_db: f64,
    q: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EqControls {
    enabled: bool,
    mode: String,
    bands: Vec<EqBandControls>,
    output_db: f64,
}
impl EqControls {
    fn from_options(options: &Options) -> Self {
        Self {
            enabled: options.eq.enabled,
            mode: options.eq_mode.clone(),
            bands: options
                .eq
                .bands
                .iter()
                .map(|band| EqBandControls {
                    frequency: band.frequency,
                    gain_db: band.gain_db,
                    q: band.q,
                })
                .collect(),
            output_db: options.eq.output_db,
        }
    }
    fn settings(&self) -> crate::eq::EqSettings {
        crate::eq::EqSettings {
            enabled: self.enabled,
            bands: self
                .bands
                .iter()
                .map(|band| crate::eq::Band {
                    frequency: band.frequency,
                    gain_db: band.gain_db,
                    q: band.q,
                })
                .collect(),
            output_db: self.output_db,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GateThresholdMode {
    Auto,
    Manual,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeEsserControls {
    enabled: bool,
    mode: String,
    frequency_hz: f64,
    threshold_db: f64,
    range_db: f64,
    attack_ms: f64,
    release_ms: f64,
}
impl DeEsserControls {
    fn from_options(options: &Options) -> Self {
        let p = options.deesser_parameters;
        Self {
            enabled: options.deesser,
            mode: if options.deesser_pro { "pro" } else { "simple" }.into(),
            frequency_hz: p.frequency_hz,
            threshold_db: p.threshold_db,
            range_db: p.range_db,
            attack_ms: p.attack_ms,
            release_ms: p.release_ms,
        }
    }
    fn parameters(&self) -> crate::deesser::DeEsserParameters {
        crate::deesser::DeEsserParameters {
            frequency_hz: self.frequency_hz,
            threshold_db: self.threshold_db,
            range_db: self.range_db,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    devices: DeviceSettings,
    voice: VoiceSettings,
    noise: NoiseSettings,
    deesser: DeEsserControls,
    eq: EqControls,
    level: LevelControls,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeviceSettings {
    backend: String,
    device: String,
    output_device: String,
    input_channel: usize,
    output_channels: Vec<usize>,
    frames: u32,
    sample_rate: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VoiceSettings {
    pitch_unit: String,
    psola_pitch_estimator: String,
    psola_analysis_window: usize,
    psola_synthesis_window: usize,
    analysis_window: usize,
    synthesis_window: usize,
    pitch_algorithm: String,
    pitch: f64,
    formant_shift: f64,
    preserve_formants: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NoiseSettings {
    rumble_filter: bool,
    enabled: bool,
    threshold_mode: GateThresholdMode,
    threshold_db: f64,
    attack_ms: f64,
    release_ms: f64,
}
impl Settings {
    fn from_options(options: &Options) -> Self {
        Self {
            devices: DeviceSettings {
                backend: options.backend.name().into(),
                device: options.device.clone(),
                output_device: options.output_device.clone(),
                input_channel: options.input_channel,
                output_channels: options.output_channels.clone(),
                frames: options.frames,
                sample_rate: options.sample_rate,
            },
            voice: VoiceSettings {
                pitch_unit: options.pitch_unit.clone(),
                psola_pitch_estimator: options.pitch.psola_pitch_estimator.name().into(),
                psola_analysis_window: options.pitch.psola_windows.analysis,
                psola_synthesis_window: options.pitch.psola_windows.synthesis,
                analysis_window: options.pitch.windows.analysis,
                synthesis_window: options.pitch.windows.synthesis,
                pitch_algorithm: options.pitch.algorithm.name().into(),
                pitch: options.pitch.semitones,
                formant_shift: options.pitch.formant_shift,
                preserve_formants: options.pitch.preserve_formants,
            },
            noise: NoiseSettings {
                rumble_filter: options.rumble_filter,
                enabled: options.gate,
                threshold_mode: match options.gate_threshold {
                    GateThreshold::Auto => GateThresholdMode::Auto,
                    GateThreshold::Db(_) => GateThresholdMode::Manual,
                },
                threshold_db: match options.gate_threshold {
                    GateThreshold::Auto => -50.0,
                    GateThreshold::Db(db) => db,
                },
                attack_ms: options.gate_attack_ms,
                release_ms: options.gate_release_ms,
            },
            deesser: DeEsserControls::from_options(options),
            eq: EqControls::from_options(options),
            level: LevelControls::from_settings(options.level),
        }
    }
    fn options(&self) -> Result<Options, String> {
        let mut options = Options {
            eq: self.eq.settings(),
            eq_mode: self.eq.mode.clone(),
            pitch_unit: self.voice.pitch_unit.clone(),
            level: self.level.settings(),
            device: self.devices.device.clone(),
            output_device: self.devices.output_device.clone(),
            input_channel: self.devices.input_channel,
            output_channels: self.devices.output_channels.clone(),
            backend: backend::BackendKind::parse(&self.devices.backend)
                .map_err(|error| error.to_string())?,
            frames: self.devices.frames,
            sample_rate: self.devices.sample_rate,
            pitch: crate::pitch::PitchSettings {
                psola_pitch_estimator: crate::pitch::PitchEstimator::parse(
                    &self.voice.psola_pitch_estimator,
                )
                .map_err(str::to_string)?,
                psola_windows: crate::pitch::SpectralWindows {
                    analysis: self.voice.psola_analysis_window,
                    synthesis: self.voice.psola_synthesis_window,
                },
                windows: crate::pitch::SpectralWindows {
                    analysis: self.voice.analysis_window,
                    synthesis: self.voice.synthesis_window,
                },
                algorithm: crate::pitch::PitchAlgorithm::parse(&self.voice.pitch_algorithm)
                    .map_err(str::to_string)?,
                semitones: self.voice.pitch,
                formant_shift: self.voice.formant_shift,
                preserve_formants: self.voice.preserve_formants,
            },
            rumble_filter: self.noise.rumble_filter,
            deesser: self.deesser.enabled,
            deesser_pro: match self.deesser.mode.as_str() {
                "simple" => false,
                "pro" => true,
                _ => return Err("De-esser mode requires simple or pro".into()),
            },
            deesser_parameters: self.deesser.parameters(),
            gate: self.noise.enabled,
            gate_threshold: match self.noise.threshold_mode {
                GateThresholdMode::Auto => GateThreshold::Auto,
                GateThresholdMode::Manual => GateThreshold::Db(self.noise.threshold_db),
            },
            gate_attack_ms: self.noise.attack_ms,
            gate_release_ms: self.noise.release_ms,
            ..Options::default()
        };
        options.validate().map_err(|error| error.to_string())?;
        options.output_channels.sort_unstable();
        Ok(options)
    }
}

#[derive(Default)]
struct CloseRequests {
    pending: bool,
}
impl CloseRequests {
    fn request(&mut self) -> bool {
        if self.pending {
            return false;
        }
        self.pending = true;
        true
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}
struct Desktop {
    session: Session,
    settings: Settings,
    path: PathBuf,
    closing: bool,
    close_requests: CloseRequests,
    streams: WindowStreams<WindowMessage>,
    logs: WindowStreams<Option<crate::session::AudioLog>>,
}
type DesktopState = Mutex<Desktop>;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MainSetup {
    audio_available: bool,
    resolved_backend: String,
    input_configured: bool,
    output_configured: bool,
    remembered_device_missing: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MainView {
    can_start: bool,
    spectrum_available: bool,
    setup: MainSetup,
    phase: AudioPhase,
}
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioPhase {
    phase: String,
    error: Option<String>,
    busy: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    level_defaults: LevelControls,
    settings: Settings,
    #[serde(flatten)]
    choices: DeviceChoices,
    backends: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceChoices {
    devices: Vec<String>,
    output_devices: Vec<String>,
    audio_available: bool,
    device_error: String,
    resolved_backend: String,
}
fn choices(kind: backend::BackendKind) -> DeviceChoices {
    let kind = backend::resolve(kind);
    let result = (|| -> backend::Result<_> {
        let selected = backend::select(kind, "")?;
        Ok((selected.device_names()?, selected.output_device_names()?))
    })();
    match result {
        Ok((devices, output_devices)) => DeviceChoices {
            devices,
            output_devices,
            audio_available: true,
            device_error: String::new(),
            resolved_backend: backend::resolve(kind).name().into(),
        },
        Err(error) => DeviceChoices {
            devices: Vec::new(),
            output_devices: Vec::new(),
            audio_available: false,
            device_error: error.to_string(),
            resolved_backend: backend::resolve(kind).name().into(),
        },
    }
}
fn available_backends() -> Vec<String> {
    [
        backend::BackendKind::Auto,
        backend::BackendKind::Cpal,
        backend::BackendKind::Asio,
    ]
    .into_iter()
    .filter(|kind| kind.available())
    .map(|kind| kind.name().into())
    .collect()
}
fn build_main_view(desktop: &mut Desktop) -> Result<MainView, String> {
    let settings = &desktop.settings;
    let kind = backend::BackendKind::parse(&settings.devices.backend)
        .map_err(|error| error.to_string())?;
    let device_choices = choices(kind);
    let resolved = device_choices.resolved_backend.as_str();
    let input_configured = resolved != "asio" || !settings.devices.device.is_empty();
    let output_configured = if resolved == "asio" {
        input_configured
    } else {
        true
    };
    let remembered_device_missing = (!settings.devices.device.is_empty()
        && !device_choices
            .devices
            .iter()
            .any(|device| device == &settings.devices.device))
        || (resolved == "cpal"
            && !settings.devices.output_device.is_empty()
            && !device_choices
                .output_devices
                .iter()
                .any(|device| device == &settings.devices.output_device));
    let busy = desktop.session.poll();
    let info = desktop.session.info();
    Ok(MainView {
        can_start: device_choices.audio_available && input_configured,
        spectrum_available: cfg!(windows),
        setup: MainSetup {
            audio_available: device_choices.audio_available,
            resolved_backend: device_choices.resolved_backend,
            input_configured,
            output_configured,
            remembered_device_missing,
        },
        phase: AudioPhase {
            phase: info.phase.to_string(),
            error: info.error.clone(),
            busy,
        },
    })
}
fn build_settings_view(desktop: &mut Desktop) -> Result<SettingsView, String> {
    let kind = backend::BackendKind::parse(&desktop.settings.devices.backend)
        .map_err(|error| error.to_string())?;
    Ok(SettingsView {
        level_defaults: LevelControls::from_settings(crate::level::LevelSettings::default()),
        settings: desktop.settings.clone(),
        choices: choices(kind),
        backends: available_backends(),
    })
}
#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum WindowMessage {
    Main { view: MainView },
    Info { info: crate::session::AudioInfo },
    Phase { phase: AudioPhase },
    Close,
    ApplicationClose,
    Error { error: String },
}

#[tauri::command]
fn connect_window(
    connection_id: String,
    channel: Channel<WindowMessage>,
    window: tauri::WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    let label = window.label();
    if desktop.closing {
        return Err("Application is closing".into());
    }
    // Registration and the first message share the same lock as every update.
    // No update can overtake the initial state or fall into a snapshot/listen gap.
    let initial = match label {
        "main" => WindowMessage::Main {
            view: build_main_view(&mut desktop)?,
        },
        "settings" => {
            let busy = desktop.session.poll();
            let info = desktop.session.info();
            WindowMessage::Phase {
                phase: AudioPhase {
                    phase: info.phase.to_string(),
                    error: info.error,
                    busy,
                },
            }
        }
        "log" => {
            // A new page starts with log display off, including after reload.
            desktop.logs.remove("log");
            desktop.session.set_logging(false);
            desktop.session.poll();
            WindowMessage::Info {
                info: desktop.session.info(),
            }
        }
        _ => return Err("Unknown window".into()),
    };
    desktop
        .streams
        .connect(label, connection_id, channel, initial)?;
    if label == "settings" && desktop.close_requests.pending {
        desktop
            .streams
            .send("settings", WindowMessage::ApplicationClose)?;
    }
    Ok(())
}

#[tauri::command]
fn disconnect_window(
    connection_id: String,
    window: tauri::WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    // Keep a pending application close across settings remounts so reconnect can
    // re-send ApplicationClose; only Destroyed / explicit cancel aborts it.
    let _ = desktop.streams.disconnect(window.label(), &connection_id);
    Ok(())
}

#[tauri::command]
fn cancel_application_close(state: State<'_, DesktopState>) -> Result<(), String> {
    state
        .lock()
        .map_err(|error| error.to_string())?
        .close_requests
        .cancel();
    Ok(())
}

#[tauri::command]
async fn device_choices(backend: String) -> Result<DeviceChoices, String> {
    let kind = crate::backend::BackendKind::parse(&backend).map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || choices(kind))
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn device_sample_rates(
    backend: String,
    device: String,
    output_device: String,
    app: tauri::AppHandle,
) -> Result<backend::DeviceSampleRates, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DesktopState>();
        let mut desktop = state.lock().map_err(|error| error.to_string())?;
        // Hold the session lock through discovery so audio cannot start midway,
        // and never load or unload an ASIO driver while callbacks own it.
        if desktop.closing || desktop.session.poll() {
            return Err("対応レートの確認は音声を停止してから行ってください。".into());
        }
        let kind = backend::resolve(
            backend::BackendKind::parse(&backend).map_err(|error| error.to_string())?,
        );
        #[cfg(all(windows, feature = "asio"))]
        let _apartment = if kind == backend::BackendKind::Asio {
            Some(crate::session::ComApartment::initialize().map_err(|error| error.to_string())?)
        } else {
            None
        };
        backend::select(kind, &output_device)
            .and_then(|selected| selected.supported_sample_rates(&device))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
fn settings_view(state: State<'_, DesktopState>) -> Result<SettingsView, String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    build_settings_view(&mut desktop)
}

#[tauri::command]
fn connect_logs(
    connection_id: String,
    channel: Channel<Option<crate::session::AudioLog>>,
    window: tauri::WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    if window.label() != "log" {
        return Err("Only the log window can subscribe to logs".into());
    }
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    if desktop.closing {
        return Err("Application is closing".into());
    }
    desktop.session.set_logging(true);
    let initial = desktop.session.log_info();
    if let Err(error) = desktop.logs.connect("log", connection_id, channel, initial) {
        if !desktop.logs.contains("log") {
            desktop.session.set_logging(false);
        }
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
fn disconnect_logs(
    connection_id: String,
    window: tauri::WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    if desktop.logs.disconnect(window.label(), &connection_id) {
        desktop.session.set_logging(false);
    }
    Ok(())
}
#[tauri::command]
fn start_audio(inspect: bool, state: State<'_, DesktopState>) -> Result<(), String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    if desktop.closing {
        return Err("Application is closing".into());
    }
    let mut options = desktop.settings.options()?;
    backend::select_with_sample_rate(options.backend, &options.output_device, options.sample_rate)
        .map_err(|error| error.to_string())?;
    if backend::resolve(options.backend) == backend::BackendKind::Asio && options.device.is_empty()
    {
        return Err("Select an audio device first".into());
    }
    options.inspect = inspect;
    desktop.session.start(options)?;
    Ok(())
}
#[tauri::command]
fn stop_audio(state: State<'_, DesktopState>) -> Result<(), String> {
    state
        .lock()
        .map_err(|error| error.to_string())?
        .session
        .stop();
    Ok(())
}
struct WindowSize {
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
}

fn show_or_create_window(
    app: &tauri::AppHandle,
    label: &str,
    title: &str,
    path: &str,
    size: WindowSize,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|error| error.to_string())?;
        return window.set_focus().map_err(|error| error.to_string());
    }
    tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App(path.into()))
        .title(title)
        .inner_size(size.width, size.height)
        .min_inner_size(size.min_width, size.min_height)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn open_window_on_main(
    app: tauri::AppHandle,
    open: impl FnOnce(&tauri::AppHandle) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(open(&handle));
    })
    .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || receiver.recv())
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    open_window_on_main(app, |app| {
        show_or_create_window(
            app,
            "settings",
            "設定 — Voice Changer",
            "index.html?window=settings",
            WindowSize {
                width: 820.0,
                height: 740.0,
                min_width: 560.0,
                min_height: 460.0,
            },
        )
    })
    .await
}

#[tauri::command]
async fn open_log(app: tauri::AppHandle) -> Result<(), String> {
    open_window_on_main(app, |app| {
        show_or_create_window(
            app,
            "log",
            "ログ — Voice Changer",
            "index.html?window=log",
            WindowSize {
                width: 640.0,
                height: 720.0,
                min_width: 480.0,
                min_height: 520.0,
            },
        )
    })
    .await
}
#[tauri::command]
fn close_settings(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn close_log(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("log") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn save_settings(settings: Settings, state: State<'_, DesktopState>) -> Result<(), String> {
    let options = settings.options()?;
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    if desktop.closing {
        return Err("Application is closing".into());
    }
    gui_settings::save(&desktop.path, &options).map_err(|error| error.to_string())?;
    desktop.settings = settings;
    let view = build_main_view(&mut desktop)?;
    desktop.streams.send("main", WindowMessage::Main { view })
}
#[tauri::command]
#[cfg(windows)]
async fn open_spectrum(app: tauri::AppHandle) -> Result<(), String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        // SAFETY: explicitly dispatched to the desktop main thread.
        let _ = sender.send(unsafe { native_spectrum::open() });
    })
    .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || receiver.recv())
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?
}
#[tauri::command]
#[cfg(not(windows))]
async fn open_spectrum() -> Result<(), String> {
    Err("Spectrum window is available only on Windows".into())
}
#[tauri::command]
async fn close_application(app: tauri::AppHandle) -> Result<(), String> {
    let mut session = {
        let state = app.state::<DesktopState>();
        let mut desktop = state.lock().map_err(|error| error.to_string())?;
        if desktop.closing {
            return Err("Application is already closing".into());
        }
        let options = desktop.settings.options()?;
        gui_settings::save(&desktop.path, &options).map_err(|error| error.to_string())?;
        desktop.closing = true;
        desktop.close_requests.cancel();
        desktop.session.stop();
        std::mem::take(&mut desktop.session)
    };
    // After `closing` is set and the session is taken, failure is terminal: the
    // status worker exits on `closing`, and the session cannot be restored.
    tauri::async_runtime::spawn_blocking(move || session.shutdown())
        .await
        .map_err(|error| error.to_string())?;
    let exit_app = app.clone();
    app.run_on_main_thread(move || {
        // SAFETY: window destruction runs on its owning thread, after ASIO stops.
        #[cfg(windows)]
        unsafe {
            native_spectrum::shutdown()
        };
        exit_app.exit(0);
    })
    .map_err(|error| error.to_string())?;
    Ok(())
}
pub fn initial_options(args: impl IntoIterator<Item = String>) -> Result<Options, Box<dyn Error>> {
    let args: Vec<_> = args.into_iter().collect();
    if args.iter().any(|arg| arg == "--help") {
        return Options::parse_gui(args);
    }
    gui_settings::merge(&gui_settings::load(&gui_settings::path()?)?, args)
}
pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    tauri::Builder::default()
        .manage(Mutex::new(Desktop {
            session: Session::new(),
            settings: Settings::from_options(&options),
            path: gui_settings::path()?,
            closing: false,
            close_requests: CloseRequests::default(),
            streams: WindowStreams::default(),
            logs: WindowStreams::default(),
        }))
        .invoke_handler(tauri::generate_handler![
            connect_window,
            disconnect_window,
            cancel_application_close,
            device_choices,
            device_sample_rates,
            settings_view,
            connect_logs,
            disconnect_logs,
            start_audio,
            stop_audio,
            save_settings,
            open_settings,
            close_settings,
            open_log,
            close_log,
            open_spectrum,
            close_application
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("desktop-status".into())
                .spawn(move || {
                    let mut previous_phase = None;
                    let mut previous_info = None;
                    let mut previous_log = None;
                    loop {
                        {
                            let state = handle.state::<DesktopState>();
                            let Ok(mut desktop) = state.lock() else {
                                break;
                            };
                            if desktop.closing {
                                break;
                            }
                            let busy = desktop.session.poll();
                            let info = desktop.session.info();
                            let phase = AudioPhase {
                                phase: info.phase.to_string(),
                                error: info.error.clone(),
                                busy,
                            };
                            if previous_phase.as_ref() != Some(&phase) {
                                for label in ["main", "settings"] {
                                    if let Err(error) = desktop.streams.send(
                                        label,
                                        WindowMessage::Phase {
                                            phase: phase.clone(),
                                        },
                                    ) {
                                        eprintln!("Cannot send audio phase: {error}");
                                    }
                                }
                                previous_phase = Some(phase);
                            }
                            if previous_info.as_ref() != Some(&info) {
                                if let Err(error) = desktop
                                    .streams
                                    .send("log", WindowMessage::Info { info: info.clone() })
                                {
                                    eprintln!("Cannot send audio info: {error}");
                                }
                                previous_info = Some(info);
                            }
                            let log = desktop.session.log_info();
                            if previous_log != log {
                                if let Err(error) = desktop.logs.send("log", log.clone()) {
                                    desktop.session.set_logging(false);
                                    eprintln!("Cannot send audio log: {error}");
                                }
                                previous_log = log;
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                })?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle().clone();
                let state = app.state::<DesktopState>();
                let label = window.label().to_string();
                let settings_open = app.get_webview_window("settings").is_some();
                enum ClosePlan {
                    None,
                    ShutdownWithPersisted,
                }
                let result = (|| -> Result<ClosePlan, String> {
                    let mut desktop = state.lock().map_err(|error| error.to_string())?;
                    if desktop.closing {
                        return Ok(ClosePlan::None);
                    }
                    if label == "main" && settings_open {
                        if desktop.streams.contains("settings") {
                            if desktop.close_requests.request() {
                                if let Some(settings) = app.get_webview_window("settings") {
                                    settings.show().map_err(|error| error.to_string())?;
                                }
                                if let Err(error) = desktop
                                    .streams
                                    .send("settings", WindowMessage::ApplicationClose)
                                {
                                    desktop.close_requests.cancel();
                                    return Err(error);
                                }
                                Ok(ClosePlan::None)
                            } else {
                                // Settings already received ApplicationClose but never finished;
                                // a second close discards the draft and exits.
                                Ok(ClosePlan::ShutdownWithPersisted)
                            }
                        } else {
                            // Settings WebView exists but its channel is gone; do not block
                            // shutdown on a draft save that can never complete.
                            Ok(ClosePlan::ShutdownWithPersisted)
                        }
                    } else if desktop.streams.contains(&label) {
                        desktop.streams.send(&label, WindowMessage::Close)?;
                        Ok(ClosePlan::None)
                    } else {
                        Err(
                            "画面との接続がありません。画面を再読み込みしてから終了してください。"
                                .into(),
                        )
                    }
                })();
                match result {
                    Ok(ClosePlan::ShutdownWithPersisted) => {
                        if let Ok(mut desktop) = state.lock() {
                            desktop.close_requests.cancel();
                        }
                        if let Some(settings) = app.get_webview_window("settings") {
                            let _ = settings.destroy();
                        }
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = close_application(app).await {
                                show_message(&error, true);
                            }
                        });
                    }
                    Ok(ClosePlan::None) => {}
                    Err(error) => show_message(&error, true),
                }
            }
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.app_handle().state::<DesktopState>();
                if let Ok(mut desktop) = state.lock() {
                    desktop.streams.remove(window.label());
                    if window.label() == "log" {
                        desktop.logs.remove("log");
                        desktop.session.set_logging(false);
                    }
                    if window.label() == "settings" && desktop.close_requests.pending {
                        desktop.close_requests.cancel();
                        if let Err(error) = desktop.streams.send(
                            "main",
                            WindowMessage::Error {
                                error: "設定画面が閉じられたため、終了を中止しました。".into(),
                            },
                        ) {
                            eprintln!("Cannot report interrupted shutdown: {error}");
                        }
                    }
                };
            }
        })
        .run(tauri::generate_context!())?;
    Ok(())
}
#[cfg(windows)]
pub fn show_message(message: &str, error: bool) {
    use windows::{
        core::PCWSTR,
        Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MESSAGEBOX_STYLE,
        },
    };
    let title: Vec<u16> = "Voice Changer".encode_utf16().chain(Some(0)).collect();
    let body: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let icon: MESSAGEBOX_STYLE = if error {
        MB_ICONERROR
    } else {
        MB_ICONINFORMATION
    };
    // SAFETY: both buffers remain alive through the modal native call.
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | icon,
        );
    }
}
#[cfg(not(windows))]
pub fn show_message(message: &str, _error: bool) {
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spectral_windows_round_trip_and_reject_invalid_combinations() {
        let options = Options::parse_gui(
            [
                "--analysis-window",
                "8192",
                "--synthesis-window",
                "2048",
                "--psola-analysis-window",
                "1024",
                "--psola-synthesis-window",
                "256",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.voice.psola_synthesis_window = 2048;
        assert!(settings.options().is_err());
        settings.voice.psola_synthesis_window = 256;
        settings.voice.analysis_window = 1024;
        assert!(settings.options().is_err());
    }
    #[test]
    fn pitch_display_unit_controls_round_trip_and_reject_unknown_values() {
        for unit in ["semitones", "percent"] {
            let options = Options {
                pitch_unit: unit.into(),
                ..Options::default()
            };
            let mut settings = Settings::from_options(&options);
            assert_eq!(settings.voice.pitch_unit, unit);
            assert_eq!(settings.options().unwrap(), options);
            settings.voice.pitch_unit = "invalid".into();
            assert!(settings.options().is_err());
        }
    }

    #[test]
    fn pitch_estimator_controls_round_trip_and_reject_unknown_values() {
        for estimator in crate::pitch::PitchEstimator::ALL {
            let options =
                Options::parse_gui(["--psola-pitch-estimator", estimator.name()].map(String::from))
                    .unwrap();
            let mut settings = Settings::from_options(&options);
            assert_eq!(settings.voice.psola_pitch_estimator, estimator.name());
            assert_eq!(settings.options().unwrap(), options);
            settings.voice.psola_pitch_estimator = "unknown".into();
            assert!(settings.options().is_err());
        }
    }

    #[test]
    fn pitch_algorithm_controls_round_trip_and_reject_unknown_values() {
        for algorithm in crate::pitch::PitchAlgorithm::ALL {
            let options =
                Options::parse_gui(["--pitch-algorithm", algorithm.name()].map(String::from))
                    .unwrap();
            let mut settings = Settings::from_options(&options);
            assert_eq!(settings.voice.pitch_algorithm, algorithm.name());
            assert_eq!(settings.options().unwrap(), options);
            settings.voice.pitch_algorithm = "unknown".into();
            assert!(settings.options().is_err());
        }
    }
    #[test]
    fn settings_payload_uses_numeric_values_in_feature_groups() {
        use tauri::ipc::{InvokeResponseBody, IpcResponse};
        let settings = Settings::from_options(&Options::default());
        let InvokeResponseBody::Json(json) = settings.body().unwrap() else {
            panic!("settings must be serialized as JSON");
        };
        for key in ["devices", "voice", "noise", "deesser", "eq", "level"] {
            assert!(json.contains(&format!("\"{key}\":{{")), "{json}");
        }
        assert!(json.contains("\"sampleRate\":48000"), "{json}");
        assert!(json.contains("\"pitch\":0.0"), "{json}");
        assert!(!json.contains("\"pitch\":\""), "{json}");
        assert!(!json.contains("\"sampleRate\":\""), "{json}");
    }
    #[test]
    fn sample_rate_controls_round_trip_and_validate() {
        let options = Options::parse_gui(["--sample-rate", "44100"].map(String::from)).unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.devices.sample_rate, 44100);
        assert_eq!(settings.options().unwrap(), options);
        settings.devices.sample_rate = 48000;
        assert_eq!(settings.options().unwrap().sample_rate, 48000);
        for value in [0, 32000, u32::MAX] {
            settings.devices.sample_rate = value;
            assert!(settings.options().is_err(), "{value}");
        }
    }
    #[test]
    fn shutdown_requests_coalesce_until_completion_or_failure() {
        let mut requests = CloseRequests::default();
        assert!(requests.request());
        assert!(!requests.request());
        requests.cancel();
        assert!(requests.request());
    }
    #[test]
    fn channel_controls_round_trip_and_validate() {
        let options = Options::parse_gui(
            ["--input-channel", "2", "--output-channels", "1,2"].map(String::from),
        )
        .unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.devices.output_channels.clear();
        assert!(settings.options().is_err());
        settings.devices.output_channels = vec![2];
        assert_eq!(settings.options().unwrap().output_channels, [2]);
    }
    #[test]
    fn gate_mode_and_manual_value_map_to_engine_settings() {
        let options = Options::parse_gui(Vec::new()).unwrap();
        let mut settings = Settings::from_options(&options);
        assert!(matches!(
            settings.noise.threshold_mode,
            GateThresholdMode::Auto
        ));
        assert_eq!(
            settings.options().unwrap().gate_threshold,
            GateThreshold::Auto
        );
        settings.noise.threshold_mode = GateThresholdMode::Manual;
        for value in [-90.0, -35.5, 0.0] {
            settings.noise.threshold_db = value;
            let manual = settings.options().unwrap();
            assert_eq!(manual.gate_threshold, GateThreshold::Db(value));
            let restored = Settings::from_options(&manual);
            assert!(matches!(
                restored.noise.threshold_mode,
                GateThresholdMode::Manual
            ));
            assert_eq!(restored.noise.threshold_db, value);
        }
        for value in [f64::NAN, f64::INFINITY, -91.0, 1.0] {
            settings.noise.threshold_db = value;
            assert!(settings.options().is_err(), "{value}");
        }
        settings.noise.threshold_mode = GateThresholdMode::Auto;
        assert_eq!(
            settings.options().unwrap().gate_threshold,
            GateThreshold::Auto
        );
    }

    #[test]
    fn deesser_controls_round_trip_and_reject_invalid_values() {
        let options = Options::parse_gui(
            [
                "--deesser",
                "on",
                "--deesser-mode",
                "pro",
                "--deesser-frequency",
                "6500",
                "--deesser-range",
                "9",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.deesser.mode = "simple".into();
        assert_eq!(
            settings.options().unwrap().deesser_parameters,
            options.deesser_parameters
        );
        settings.deesser.frequency_hz = f64::NAN;
        assert!(settings.options().is_err());
    }
    #[test]
    fn auto_level_controls_round_trip_validate_and_persist() {
        let options = Options::parse_gui(
            [
                "--deesser",
                "on",
                "--rumble-filter",
                "on",
                "--auto-level",
                "on",
                "--compressor",
                "on",
                "--level-target",
                "-18",
                "--level-max-gain",
                "6",
                "--level-floor",
                "-55",
                "--level-response",
                "1200",
                "--compress-threshold",
                "-10",
                "--compress-ratio",
                "4",
                "--compress-attack",
                "3",
                "--compress-release",
                "100",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut controls = Settings::from_options(&options);
        assert_eq!(controls.options().unwrap(), options);
        let path =
            std::env::temp_dir().join(format!("voice-tauri-level-{}.ini", std::process::id()));
        gui_settings::save(&path, &controls.options().unwrap()).unwrap();
        assert_eq!(gui_settings::load(&path).unwrap(), options);
        controls.level.enabled = false;
        assert!(controls.options().unwrap().level.compressor_enabled);
        assert_eq!(
            controls.options().unwrap().level.values(),
            options.level.values()
        );
        controls.level.ratio = f64::NAN;
        assert!(controls.options().is_err());
        controls.level.ratio = 4.0;
        controls.level.floor_db = -15.0;
        assert!(controls.options().is_err());
        controls.level = LevelControls::from_settings(crate::level::LevelSettings::default());
        assert_eq!(
            controls.options().unwrap().level,
            crate::level::LevelSettings::default()
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn command_settings_use_engine_validation() {
        let options = Options::parse_gui(Vec::new()).unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.voice.pitch = f64::NAN;
        assert!(settings.options().is_err());
        settings.voice.pitch = 3.25;
        settings.voice.formant_shift = 2.0;
        settings.voice.preserve_formants = false;
        assert!(settings.options().is_err());
        settings.voice.preserve_formants = true;
        assert_eq!(settings.options().unwrap().pitch.semitones, 3.25);
        settings.devices.frames = 0;
        assert!(settings.options().is_err());
    }

    #[test]
    fn eq_controls_share_settings_across_modes_and_validate() {
        let options = Options::parse_gui(
            [
                "--eq",
                "on",
                "--eq-band1",
                "180,-4,1.2",
                "--eq-output",
                "-2",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut controls = Settings::from_options(&options);
        assert_eq!(controls.options().unwrap(), options);
        controls.eq.mode = "pro".into();
        assert_eq!(controls.options().unwrap().eq, options.eq);
        controls.eq.mode = "simple".into();
        assert_eq!(controls.options().unwrap().eq, options.eq);
        controls.eq.bands[1].q = f64::NAN;
        assert!(controls.options().is_err());
        controls.eq.bands[1].q = 1.0;
        controls.eq.output_db = f64::NAN;
        assert!(controls.options().is_err());
    }
}
