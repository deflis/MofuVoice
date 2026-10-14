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
    unit: String,
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
            unit: settings.unit.name().into(),
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
            pitch: options.pitch.to_string(),
            formant_shift: options.formant_shift.to_string(),
            preserve_formants: options.preserve_formants,
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
            "--level-unit".into(),
            self.level.unit.clone(),
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
struct Desktop {
    session: Session,
    settings: Settings,
    path: PathBuf,
    closing: bool,
}
type DesktopState = Mutex<Desktop>;
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    busy: bool,
    info: crate::session::AudioInfo,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    level_defaults: LevelControls,
    settings: Settings,
    #[serde(flatten)]
    choices: DeviceChoices,
    backends: Vec<String>,
    spectrum_available: bool,
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
#[tauri::command]
fn bootstrap(state: State<'_, DesktopState>) -> Result<Bootstrap, String> {
    let desktop = state.lock().map_err(|error| error.to_string())?;
    let kind = backend::BackendKind::parse(&desktop.settings.backend)
        .map_err(|error| error.to_string())?;
    Ok(Bootstrap {
        level_defaults: LevelControls::from_settings(crate::level::LevelSettings::default()),
        settings: desktop.settings.clone(),
        choices: choices(kind),
        backends: [
            backend::BackendKind::Auto,
            backend::BackendKind::Cpal,
            backend::BackendKind::Asio,
        ]
        .into_iter()
        .filter(|kind| kind.available())
        .map(|kind| kind.name().into())
        .collect(),
        spectrum_available: cfg!(windows),
    })
}
#[tauri::command]
async fn device_choices(backend: String) -> Result<DeviceChoices, String> {
    let kind = crate::backend::BackendKind::parse(&backend).map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || choices(kind))
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
fn audio_status(state: State<'_, DesktopState>) -> Result<Snapshot, String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    Ok(Snapshot {
        busy: desktop.session.poll(),
        info: desktop.session.info(),
    })
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
fn start_audio(
    settings: Settings,
    inspect: bool,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let mut options = settings.options()?;
    backend::select(options.backend, &options.output_device).map_err(|error| error.to_string())?;
    if backend::resolve(options.backend) == backend::BackendKind::Asio && options.device.is_empty()
    {
        return Err("Select an audio device first".into());
    }
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    if desktop.closing {
        return Err("Application is closing".into());
    }
    options.inspect = inspect;
    desktop.session.start(options)?;
    desktop.settings = settings;
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
#[tauri::command]
fn save_settings(settings: Settings, state: State<'_, DesktopState>) -> Result<(), String> {
    let options = settings.options()?;
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    if desktop.closing {
        return Err("Application is closing".into());
    }
    gui_settings::save(&desktop.path, &options).map_err(|error| error.to_string())?;
    desktop.settings = settings;
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
async fn close_application(settings: Settings, app: tauri::AppHandle) -> Result<(), String> {
    let options = settings.options()?;
    {
        let state = app.state::<DesktopState>();
        let mut desktop = state.lock().map_err(|error| error.to_string())?;
        if desktop.closing {
            return Err("Application is already closing".into());
        }
        gui_settings::save(&desktop.path, &options).map_err(|error| error.to_string())?;
        desktop.settings = settings;
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
        }))
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            device_choices,
            audio_status,
            set_logging,
            start_audio,
            stop_audio,
            save_settings,
            open_spectrum,
            close_application
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("desktop-status".into())
                .spawn(move || {
                    let mut previous = None;
                    let mut previous_log = None;
                    loop {
                        let (snapshot, log) = {
                            let state = handle.state::<DesktopState>();
                            let Ok(mut desktop) = state.lock() else {
                                break;
                            };
                            if desktop.closing {
                                break;
                            }
                            (
                                Snapshot {
                                    busy: desktop.session.poll(),
                                    info: desktop.session.info(),
                                },
                                desktop.session.log_info(),
                            )
                        };
                        if previous.as_ref() != Some(&snapshot) {
                            if let Err(error) = handle.emit("audio-status", &snapshot) {
                                eprintln!("Cannot emit audio status: {error}");
                            }
                            previous = Some(snapshot);
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
                if let Err(error) = window.emit("request-close", ()) {
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
                "--level-unit",
                "dbfs",
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
        controls.level.unit = "lufs".into();
        assert_eq!(
            controls.options().unwrap().level.unit,
            crate::level::LevelUnit::Lufs
        );
        assert_eq!(
            controls.options().unwrap().level.values(),
            options.level.values()
        );
        controls.level.unit = "peak".into();
        assert!(controls.options().is_err());
        controls.level.unit = "dbfs".into();
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
        assert_eq!(settings.options().unwrap().pitch, 3.25);
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
