//! React commands control the audio session; no WebView messages enter audio callbacks.
#[cfg(windows)]
use crate::native_spectrum;
use crate::{backend, cli::Options, gate::GateThreshold, gui_settings, session::Session};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager, State};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LevelControls {
    enabled: bool,
    target_db: String,
    max_gain_db: String,
    floor_db: String,
    response_ms: String,
    threshold_db: String,
    ratio: String,
    attack_ms: String,
    release_ms: String,
}
impl LevelControls {
    fn from_settings(settings: crate::level::LevelSettings) -> Self {
        Self {
            enabled: settings.enabled,
            target_db: settings.target_db.to_string(),
            max_gain_db: settings.max_gain_db.to_string(),
            floor_db: settings.floor_db.to_string(),
            response_ms: settings.response_ms.to_string(),
            threshold_db: settings.threshold_db.to_string(),
            ratio: settings.ratio.to_string(),
            attack_ms: settings.attack_ms.to_string(),
            release_ms: settings.release_ms.to_string(),
        }
    }
    fn values(&self) -> [&str; 8] {
        [
            &self.target_db,
            &self.max_gain_db,
            &self.floor_db,
            &self.response_ms,
            &self.threshold_db,
            &self.ratio,
            &self.attack_ms,
            &self.release_ms,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EqBandControls {
    frequency: String,
    gain_db: String,
    q: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EqControls {
    enabled: bool,
    mode: String,
    bands: [EqBandControls; 3],
    output_db: String,
}
impl EqControls {
    fn from_options(options: &Options) -> Self {
        Self {
            enabled: options.eq.enabled,
            mode: options.eq_mode.clone(),
            bands: options.eq.bands.map(|band| EqBandControls {
                frequency: band.frequency.to_string(),
                gain_db: band.gain_db.to_string(),
                q: band.q.to_string(),
            }),
            output_db: options.eq.output_db.to_string(),
        }
    }
    fn arguments(&self) -> [String; 6] {
        [
            if self.enabled { "on" } else { "off" }.into(),
            self.mode.clone(),
            format!(
                "{},{},{}",
                self.bands[0].frequency, self.bands[0].gain_db, self.bands[0].q
            ),
            format!(
                "{},{},{}",
                self.bands[1].frequency, self.bands[1].gain_db, self.bands[1].q
            ),
            format!(
                "{},{},{}",
                self.bands[2].frequency, self.bands[2].gain_db, self.bands[2].q
            ),
            self.output_db.clone(),
        ]
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
    frequency_hz: String,
    threshold_db: String,
    range_db: String,
    attack_ms: String,
    release_ms: String,
}
impl DeEsserControls {
    fn from_parameters(p: crate::deesser::DeEsserParameters) -> Self {
        Self {
            frequency_hz: p.frequency_hz.to_string(),
            threshold_db: p.threshold_db.to_string(),
            range_db: p.range_db.to_string(),
            attack_ms: p.attack_ms.to_string(),
            release_ms: p.release_ms.to_string(),
        }
    }
    fn values(&self) -> [&str; 5] {
        [
            &self.frequency_hz,
            &self.threshold_db,
            &self.range_db,
            &self.attack_ms,
            &self.release_ms,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    eq: EqControls,
    level: LevelControls,
    device: String,
    output_device: String,
    input_channel: usize,
    output_channels: Vec<usize>,
    backend: String,
    frames: String,
    sample_rate: String,
    pitch: String,
    formant_shift: String,
    preserve_formants: bool,
    rumble_filter: bool,
    deesser: bool,
    deesser_mode: String,
    deesser_parameters: DeEsserControls,
    gate: bool,
    gate_threshold_mode: GateThresholdMode,
    gate_threshold_db: String,
    gate_attack_ms: String,
    gate_release_ms: String,
}
impl Settings {
    fn from_options(options: &Options) -> Self {
        Self {
            eq: EqControls::from_options(options),
            level: LevelControls::from_settings(options.level),
            device: options.device.clone(),
            output_device: options.output_device.clone(),
            input_channel: options.input_channel,
            output_channels: options.output_channels.clone(),
            backend: options.backend.name().into(),
            frames: options.frames.to_string(),
            sample_rate: options.sample_rate.to_string(),
            pitch: options.pitch.semitones.to_string(),
            formant_shift: options.pitch.formant_shift.to_string(),
            preserve_formants: options.pitch.preserve_formants,
            rumble_filter: options.rumble_filter,
            deesser: options.deesser,
            deesser_mode: if options.deesser_pro { "pro" } else { "simple" }.into(),
            deesser_parameters: DeEsserControls::from_parameters(options.deesser_parameters),
            gate: options.gate,
            gate_threshold_mode: match options.gate_threshold {
                GateThreshold::Auto => GateThresholdMode::Auto,
                GateThreshold::Db(_) => GateThresholdMode::Manual,
            },
            gate_threshold_db: match options.gate_threshold {
                GateThreshold::Auto => "-50".into(),
                GateThreshold::Db(db) => db.to_string(),
            },
            gate_attack_ms: options.gate_attack_ms.to_string(),
            gate_release_ms: options.gate_release_ms.to_string(),
        }
    }
    fn options(&self) -> Result<Options, String> {
        let threshold = match self.gate_threshold_mode {
            GateThresholdMode::Auto => GateThreshold::Auto,
            GateThresholdMode::Manual => {
                let db = self
                    .gate_threshold_db
                    .parse::<f64>()
                    .map_err(|_| "ゲートの手動値には−90〜0 dBFSの数値を指定してください。")?;
                if !db.is_finite() || !(-90.0..=0.0).contains(&db) {
                    return Err("ゲートの手動値には−90〜0 dBFSの数値を指定してください。".into());
                }
                GateThreshold::Db(db)
            }
        };
        let mut args = vec![
            "--input-channel".into(),
            self.input_channel.to_string(),
            "--output-channels".into(),
            self.output_channels
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
            "--backend".into(),
            self.backend.clone(),
            "--sample-rate".into(),
            self.sample_rate.clone(),
            "--frames".into(),
            self.frames.clone(),
            "--pitch".into(),
            self.pitch.clone(),
            "--formant-shift".into(),
            self.formant_shift.clone(),
            "--formant".into(),
            if self.preserve_formants {
                "preserve"
            } else {
                "off"
            }
            .into(),
            "--deesser".into(),
            if self.deesser { "on" } else { "off" }.into(),
            "--deesser-mode".into(),
            self.deesser_mode.clone(),
            "--rumble-filter".into(),
            if self.rumble_filter { "on" } else { "off" }.into(),
            "--gate".into(),
            if self.gate { "on" } else { "off" }.into(),
            "--gate-threshold".into(),
            match threshold {
                GateThreshold::Auto => "auto".into(),
                GateThreshold::Db(db) => db.to_string(),
            },
            "--gate-attack".into(),
            self.gate_attack_ms.clone(),
            "--gate-release".into(),
            self.gate_release_ms.clone(),
        ];
        args.extend([
            "--auto-level".into(),
            if self.level.enabled { "on" } else { "off" }.into(),
        ]);
        for (flag, value) in crate::deesser::FLAGS
            .iter()
            .zip(self.deesser_parameters.values())
        {
            args.extend([(*flag).into(), value.into()]);
        }
        for (flag, value) in crate::level::FLAGS.iter().zip(self.level.values()) {
            args.extend([(*flag).into(), value.into()]);
        }
        for (flag, value) in crate::eq::FLAGS.iter().zip(self.eq.arguments()) {
            args.extend([(*flag).into(), value]);
        }
        if !self.device.is_empty() {
            args.extend(["--device".into(), self.device.clone()]);
        }
        if !self.output_device.is_empty() {
            args.extend(["--output-device".into(), self.output_device.clone()]);
        }
        if self.device.contains(['\r', '\n', '\0'])
            || self.output_device.contains(['\r', '\n', '\0'])
        {
            return Err("Invalid device name".into());
        }
        Options::parse_gui(args).map_err(|error| error.to_string())
    }
}
#[derive(Default)]
struct CloseRequests {
    next_id: u32,
    pending: Option<CloseRequest>,
}
struct CloseRequest {
    id: u32,
    deadline: std::time::Instant,
    acknowledged: bool,
}
impl CloseRequests {
    fn request(&mut self) -> Option<u32> {
        if self
            .pending
            .as_ref()
            .is_some_and(|request| !request.acknowledged)
        {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1);
        self.pending = Some(CloseRequest {
            id: self.next_id,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(1),
            acknowledged: false,
        });
        Some(self.next_id)
    }
    fn acknowledge(&mut self, id: u32) -> bool {
        if let Some(request) = self.pending.as_mut().filter(|request| request.id == id) {
            request.acknowledged = true;
            true
        } else {
            false
        }
    }
    fn cancel(&mut self, id: u32) -> bool {
        if self
            .pending
            .as_ref()
            .is_some_and(|request| request.id == id)
        {
            self.pending = None;
            true
        } else {
            false
        }
    }
    fn fallback_due(&mut self, now: std::time::Instant) -> bool {
        if self
            .pending
            .as_ref()
            .is_some_and(|request| !request.acknowledged && now >= request.deadline)
        {
            self.pending = None;
            true
        } else {
            false
        }
    }
}
struct Desktop {
    session: Session,
    settings: Settings,
    path: PathBuf,
    closing: bool,
    close_requests: CloseRequests,
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
    audio_busy: bool,
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
    let kind = backend::BackendKind::parse(&settings.backend).map_err(|error| error.to_string())?;
    let device_choices = choices(kind);
    let resolved = device_choices.resolved_backend.as_str();
    let input_configured = resolved != "asio" || !settings.device.is_empty();
    let output_configured = if resolved == "asio" {
        input_configured
    } else {
        true
    };
    let remembered_device_missing = (!settings.device.is_empty()
        && !device_choices
            .devices
            .iter()
            .any(|device| device == &settings.device))
        || (resolved == "cpal"
            && !settings.output_device.is_empty()
            && !device_choices
                .output_devices
                .iter()
                .any(|device| device == &settings.output_device));
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
    let kind = backend::BackendKind::parse(&desktop.settings.backend)
        .map_err(|error| error.to_string())?;
    Ok(SettingsView {
        level_defaults: LevelControls::from_settings(crate::level::LevelSettings::default()),
        settings: desktop.settings.clone(),
        choices: choices(kind),
        backends: available_backends(),
        audio_busy: desktop.session.poll(),
    })
}
#[tauri::command]
fn main_view(state: State<'_, DesktopState>) -> Result<MainView, String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    build_main_view(&mut desktop)
}
#[tauri::command]
fn settings_view(state: State<'_, DesktopState>) -> Result<SettingsView, String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    build_settings_view(&mut desktop)
}
#[tauri::command]
fn log_view(state: State<'_, DesktopState>) -> Result<crate::session::AudioInfo, String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    let _ = desktop.session.poll();
    Ok(desktop.session.info())
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
fn set_logging(
    logging: bool,
    state: State<'_, DesktopState>,
) -> Result<Option<crate::session::AudioLog>, String> {
    let desktop = state.lock().map_err(|error| error.to_string())?;
    desktop.session.set_logging(logging);
    Ok(desktop.session.log_info())
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
fn acknowledge_application_close(
    request_id: u32,
    window: tauri::WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<bool, String> {
    if window.label() != "settings" {
        return Err("Only the settings window can acknowledge shutdown".into());
    }
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    Ok(desktop.close_requests.acknowledge(request_id))
}
#[tauri::command]
fn fallback_application_close(
    request_id: u32,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("Only the settings window can return shutdown to main".into());
    }
    let cancelled = state
        .lock()
        .map_err(|error| error.to_string())?
        .close_requests
        .cancel(request_id);
    if cancelled {
        app.emit_to("main", "request-close", ())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
fn save_settings(
    app: tauri::AppHandle,
    settings: Settings,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let options = settings.options()?;
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    if desktop.closing {
        return Err("Application is closing".into());
    }
    gui_settings::save(&desktop.path, &options).map_err(|error| error.to_string())?;
    desktop.settings = settings;
    let view = build_main_view(&mut desktop)?;
    drop(desktop);
    if let Some(main) = app.get_webview_window("main") {
        app.emit_to(main.label(), "main-view", &view)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
    {
        let state = app.state::<DesktopState>();
        let mut desktop = state.lock().map_err(|error| error.to_string())?;
        if desktop.closing {
            return Err("Application is already closing".into());
        }
        let options = desktop.settings.options()?;
        gui_settings::save(&desktop.path, &options).map_err(|error| error.to_string())?;
        desktop.closing = true;
        desktop.session.stop();
    }
    let mut session = {
        let state = app.state::<DesktopState>();
        let mut desktop = state.lock().map_err(|error| error.to_string())?;
        std::mem::take(&mut desktop.session)
    };
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
        }))
        .invoke_handler(tauri::generate_handler![
            main_view,
            settings_view,
            log_view,
            device_choices,
            device_sample_rates,
            set_logging,
            start_audio,
            stop_audio,
            save_settings,
            open_settings,
            close_settings,
            open_log,
            close_log,
            acknowledge_application_close,
            fallback_application_close,
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
                        let (phase, info, log, fallback_close) = {
                            let state = handle.state::<DesktopState>();
                            let Ok(mut desktop) = state.lock() else {
                                break;
                            };
                            if desktop.closing {
                                break;
                            }
                            let busy = desktop.session.poll();
                            let info = desktop.session.info();
                            (
                                AudioPhase {
                                    phase: info.phase.to_string(),
                                    error: info.error.clone(),
                                    busy,
                                },
                                info,
                                desktop.session.log_info(),
                                desktop
                                    .close_requests
                                    .fallback_due(std::time::Instant::now()),
                            )
                        };
                        if fallback_close {
                            if let Err(error) = handle.emit_to("main", "request-close", ()) {
                                eprintln!("Cannot return shutdown to main: {error}");
                            }
                        }
                        if previous_phase.as_ref() != Some(&phase) {
                            if let Err(error) = handle.emit("audio-phase", &phase) {
                                eprintln!("Cannot emit audio phase: {error}");
                            }
                            previous_phase = Some(phase);
                        }
                        if previous_info.as_ref() != Some(&info) {
                            if let Err(error) = handle.emit("log-info", &info) {
                                eprintln!("Cannot emit log info: {error}");
                            }
                            previous_info = Some(info);
                        }
                        if let Some(log) = log {
                            if previous_log.as_ref() != Some(&log) {
                                if let Err(error) = handle.emit("audio-log", &log) {
                                    eprintln!("Cannot emit audio log: {error}");
                                }
                                previous_log = Some(log);
                            }
                        } else {
                            previous_log = None;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                })?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                // Let the settings owner save its draft before application shutdown.
                let settings_window = (window.label() == "main")
                    .then(|| window.app_handle().get_webview_window("settings"))
                    .flatten();
                let result = if let Some(settings) = settings_window {
                    let _ = settings.show();
                    let state = window.app_handle().state::<DesktopState>();
                    let request_id = match state.lock() {
                        Ok(mut desktop) => desktop.close_requests.request(),
                        Err(error) => {
                            show_message(&error.to_string(), true);
                            return;
                        }
                    };
                    let Some(request_id) = request_id else {
                        return;
                    };
                    window.app_handle().emit_to(
                        settings.label(),
                        "request-application-close",
                        request_id,
                    )
                } else {
                    window
                        .app_handle()
                        .emit_to(window.label(), "request-close", ())
                };
                if let Err(error) = result {
                    show_message(&error.to_string(), true);
                }
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
    fn sample_rate_controls_round_trip_and_validate() {
        let options = Options::parse_gui(["--sample-rate", "44100"].map(String::from)).unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.sample_rate, "44100");
        assert_eq!(settings.options().unwrap(), options);
        settings.sample_rate = "48000".into();
        assert_eq!(settings.options().unwrap().sample_rate, 48000);
        for value in ["", "0", "32000", "NaN", "48000.5"] {
            settings.sample_rate = value.into();
            assert!(settings.options().is_err(), "{value}");
        }
    }
    #[test]
    fn unacknowledged_shutdown_falls_back_once_and_rejects_late_ack() {
        let mut requests = CloseRequests::default();
        let id = requests.request().unwrap();
        assert!(!requests.fallback_due(std::time::Instant::now()));
        assert!(
            requests.fallback_due(std::time::Instant::now() + std::time::Duration::from_secs(2))
        );
        assert!(
            !requests.fallback_due(std::time::Instant::now() + std::time::Duration::from_secs(3))
        );
        assert!(!requests.acknowledge(id));
    }
    #[test]
    fn repeated_close_does_not_reset_the_fallback_deadline() {
        let mut requests = CloseRequests::default();
        requests.request().unwrap();
        let deadline = requests.pending.as_ref().unwrap().deadline;
        assert!(requests.request().is_none());
        assert_eq!(requests.pending.as_ref().unwrap().deadline, deadline);
        assert!(requests.fallback_due(deadline));
    }
    #[test]
    fn acknowledged_shutdown_waits_for_settings_and_allows_retry() {
        let mut requests = CloseRequests::default();
        let id = requests.request().unwrap();
        assert!(requests.acknowledge(id));
        assert!(
            !requests.fallback_due(std::time::Instant::now() + std::time::Duration::from_secs(60))
        );
        let retry = requests.request().unwrap();
        assert_ne!(id, retry);
        assert!(!requests.acknowledge(id));
        assert!(!requests.cancel(id));
        assert!(requests.cancel(retry));
        assert!(!requests.acknowledge(retry));
    }
    #[test]
    fn channel_controls_round_trip_and_validate() {
        let options = Options::parse_gui(
            ["--input-channel", "2", "--output-channels", "1,2"].map(String::from),
        )
        .unwrap();
        let mut settings = Settings::from_options(&options);
        assert_eq!(settings.options().unwrap(), options);
        settings.output_channels.clear();
        assert!(settings.options().is_err());
        settings.output_channels = vec![2];
        assert_eq!(settings.options().unwrap().output_channels, [2]);
    }
    #[test]
    fn gate_mode_and_manual_value_map_to_engine_settings() {
        let options = Options::parse_gui(Vec::new()).unwrap();
        let mut settings = Settings::from_options(&options);
        assert!(matches!(
            settings.gate_threshold_mode,
            GateThresholdMode::Auto
        ));
        assert_eq!(
            settings.options().unwrap().gate_threshold,
            GateThreshold::Auto
        );
        settings.gate_threshold_mode = GateThresholdMode::Manual;
        for value in ["-90", "-35.5", "0"] {
            settings.gate_threshold_db = value.into();
            let manual = settings.options().unwrap();
            assert_eq!(
                manual.gate_threshold,
                GateThreshold::Db(value.parse().unwrap())
            );
            let restored = Settings::from_options(&manual);
            assert!(matches!(
                restored.gate_threshold_mode,
                GateThresholdMode::Manual
            ));
            assert_eq!(restored.gate_threshold_db, value);
        }
        for value in ["auto", "", "NaN", "inf", "-91", "1"] {
            settings.gate_threshold_db = value.into();
            assert!(settings.options().is_err(), "{value}");
        }
        settings.gate_threshold_mode = GateThresholdMode::Auto;
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
        settings.deesser_mode = "simple".into();
        assert_eq!(
            settings.options().unwrap().deesser_parameters,
            options.deesser_parameters
        );
        settings.deesser_parameters.frequency_hz = "NaN".into();
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
        assert_eq!(
            controls.options().unwrap().level.values(),
            options.level.values()
        );
        controls.level.ratio = "NaN".into();
        assert!(controls.options().is_err());
        controls.level.ratio = "4".into();
        controls.level.floor_db = "-15".into();
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
        settings.pitch = "NaN".into();
        assert!(settings.options().is_err());
        settings.pitch = "3.25".into();
        settings.formant_shift = "2".into();
        settings.preserve_formants = false;
        assert!(settings.options().is_err());
        settings.preserve_formants = true;
        assert_eq!(settings.options().unwrap().pitch.semitones, 3.25);
        settings.frames = "0".into();
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
        controls.eq.bands[1].q = "".into();
        assert!(controls.options().is_err());
        controls.eq.bands[1].q = "1".into();
        controls.eq.output_db = "NaN".into();
        assert!(controls.options().is_err());
    }
}
