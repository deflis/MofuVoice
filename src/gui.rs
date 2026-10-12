//! React commands control the ASIO session; no WebView messages enter audio callbacks.
use crate::{cli::Options, gate::GateThreshold, gui_settings, native_spectrum, session::Session};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager, State};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    device: String,
    frames: String,
    pitch: String,
    formant_shift: String,
    preserve_formants: bool,
    gate: bool,
    gate_threshold: String,
    gate_attack_ms: String,
    gate_release_ms: String,
}
impl Settings {
    fn from_options(options: &Options) -> Self {
        Self {
            device: options.device.clone(),
            frames: options.frames.to_string(),
            pitch: options.pitch.to_string(),
            formant_shift: options.formant_shift.to_string(),
            preserve_formants: options.preserve_formants,
            gate: options.gate,
            gate_threshold: match options.gate_threshold {
                GateThreshold::Auto => "auto".into(),
                GateThreshold::Db(db) => db.to_string(),
            },
            gate_attack_ms: options.gate_attack_ms.to_string(),
            gate_release_ms: options.gate_release_ms.to_string(),
        }
    }
    fn options(&self) -> Result<Options, String> {
        let mut args = vec![
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
            "--gate".into(),
            if self.gate { "on" } else { "off" }.into(),
            "--gate-threshold".into(),
            self.gate_threshold.clone(),
            "--gate-attack".into(),
            self.gate_attack_ms.clone(),
            "--gate-release".into(),
            self.gate_release_ms.clone(),
        ];
        if !self.device.is_empty() {
            args.extend(["--device".into(), self.device.clone()]);
        }
        if self.device.contains(['\r', '\n', '\0']) {
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    busy: bool,
    status: String,
}
#[derive(Serialize)]
struct Bootstrap {
    settings: Settings,
    devices: Vec<String>,
}

#[tauri::command]
fn bootstrap(state: State<'_, DesktopState>) -> Result<Bootstrap, String> {
    let desktop = state.lock().map_err(|error| error.to_string())?;
    Ok(Bootstrap {
        settings: desktop.settings.clone(),
        devices: asio_sys::Asio::new().driver_names(),
    })
}
#[tauri::command]
fn audio_status(state: State<'_, DesktopState>) -> Result<Snapshot, String> {
    let mut desktop = state.lock().map_err(|error| error.to_string())?;
    Ok(Snapshot {
        busy: desktop.session.poll(),
        status: desktop.session.status(),
    })
}
#[tauri::command]
fn start_audio(
    settings: Settings,
    inspect: bool,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let mut options = settings.options()?;
    if options.device.is_empty() {
        return Err("Select an ASIO device first".into());
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
        unsafe { native_spectrum::shutdown() };
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
            audio_status,
            start_audio,
            stop_audio,
            save_settings,
            open_spectrum,
            close_application
        ])
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
pub fn show_message(message: &str, error: bool) {
    unsafe extern "C" {
        fn voice_gui_message(title: *const u16, message: *const u16, error: i32);
    }
    let title: Vec<_> = "Voice Changer".encode_utf16().chain(Some(0)).collect();
    let message: Vec<_> = message.encode_utf16().chain(Some(0)).collect();
    // SAFETY: both buffers remain alive through the modal native call.
    unsafe { voice_gui_message(title.as_ptr(), message.as_ptr(), i32::from(error)) };
}
#[cfg(test)]
mod tests {
    use super::*;
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
}
