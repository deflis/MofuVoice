use super::{
    draft::Generation,
    preferences::{Preferences, Section},
    windows,
    windows::settings::SettingsView,
};
use crate::{
    backend,
    cli::Options,
    session::{AudioInfo, AudioLog, Session},
};
use gpui_kit::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct AppModel {
    pub session: Arc<Mutex<Session>>,
    pub saved: Options,
    pub persisted_options: Options,
    pub preferences: Preferences,
    pub persisted_preferences: Preferences,
    pub path: PathBuf,
    pub info: AudioInfo,
    pub log: Option<AudioLog>,
    pub logging: bool,
    auto_backend: backend::BackendKind,
    pub device_warning: Option<DeviceWarning>,
    device_check: Generation,
    pub status: String,
    pub error: String,
    pub busy: bool,
    pub active: bool,
    pub closing: bool,
    pub quit_requested: bool,
    pub quit_save_failed: bool,
    opening_settings: bool,
    opening_log: bool,
    opening_spectrum: bool,
    pub settings: Option<(AnyWindowHandle, Entity<SettingsView>)>,
    pub log_window: Option<AnyWindowHandle>,
    pub spectrum_window: Option<AnyWindowHandle>,
    pub generation: Generation,
    _poll: Task<()>,
}
impl AppModel {
    pub fn new(
        saved: Options,
        persisted_options: Options,
        path: PathBuf,
        preferences: Preferences,
        cx: &mut Context<Self>,
    ) -> Self {
        let session = Arc::new(Mutex::new(Session::new()));
        let poll_session = session.clone();
        let executor = cx.background_executor().clone();
        let poll = cx.spawn(async move |this, cx| loop {
            executor.timer(Duration::from_millis(100)).await;
            let generation = match this.update(cx, |m, _| {
                if m.busy || m.closing {
                    None
                } else {
                    Some(m.generation.current())
                }
            }) {
                Ok(Some(generation)) => generation,
                Ok(None) => continue,
                Err(_) => break,
            };
            let session = poll_session.clone();
            let snapshot = executor
                .spawn(async move {
                    let mut s = session.lock().expect("session control lock");
                    (s.poll(), s.info(), s.status(), s.log_info())
                })
                .await;
            if this
                .update(cx, |m, cx| {
                    if m.closing {
                        return false;
                    }
                    // A start may overtake a previously requested status snapshot.
                    if m.busy || !m.generation.accepts(generation) {
                        return true;
                    }
                    if m.active != snapshot.0
                        || m.info != snapshot.1
                        || m.status != snapshot.2
                        || (m.logging && m.log != snapshot.3)
                    {
                        m.active = snapshot.0;
                        m.info = snapshot.1;
                        m.status = snapshot.2;
                        m.log = if m.logging { snapshot.3 } else { None };
                        cx.notify();
                    }
                    if m.quit_requested && !m.busy {
                        m.request_quit(cx);
                    }
                    true
                })
                .ok()
                != Some(true)
            {
                break;
            }
        });
        let auto_backend = backend::resolve(backend::BackendKind::Auto);
        Self {
            session,
            saved,
            persisted_options,
            persisted_preferences: preferences,
            preferences,
            path,
            info: AudioInfo::default(),
            log: None,
            logging: false,
            auto_backend,
            device_warning: None,
            device_check: Generation::default(),
            status: String::new(),
            error: String::new(),
            busy: false,
            active: false,
            closing: false,
            quit_requested: false,
            quit_save_failed: false,
            opening_settings: false,
            opening_log: false,
            opening_spectrum: false,
            settings: None,
            log_window: None,
            spectrum_window: None,
            generation: Generation::default(),
            _poll: poll,
        }
    }
    pub fn text(&self, key: &str) -> String {
        self.preferences.text(key)
    }
    pub fn locked(&self) -> bool {
        self.active || self.busy || self.shutting_down()
    }
    pub fn shutting_down(&self) -> bool {
        self.closing || self.quit_requested
    }
    pub fn report(&mut self, error: impl ToString, cx: &mut Context<Self>) {
        self.error = error.to_string();
        cx.notify();
    }
    pub fn resolve_backend(&self, kind: backend::BackendKind) -> backend::BackendKind {
        match kind {
            backend::BackendKind::Auto => self.auto_backend,
            explicit => explicit,
        }
    }
    pub fn needs_input_device(&self) -> bool {
        self.saved.device.is_empty()
            && self.resolve_backend(self.saved.backend) == backend::BackendKind::Asio
    }
    pub fn can_start(&self, cx: &App) -> bool {
        !self.locked()
            && !self.needs_input_device()
            && self.device_warning.is_none()
            && !self.has_unsaved_audio_draft(cx)
    }
    pub fn has_unsaved_audio_draft(&self, cx: &App) -> bool {
        self.settings
            .as_ref()
            .is_some_and(|(_, view)| view.read(cx).blocks_audio_start(&self.saved))
    }
    pub fn start_blocked_message(&self, cx: &App) -> Option<&'static str> {
        if let Some((_, view)) = &self.settings {
            if let Some(message) = view.read(cx).audio_start_block_reason(&self.saved) {
                return Some(message);
            }
        }
        self.device_warning.map(DeviceWarning::message).or_else(|| {
            self.needs_input_device()
                .then_some("最初に、入力・出力デバイスを設定してください。")
        })
    }
    pub fn check_saved_devices(&mut self, cx: &mut Context<Self>) {
        let options = self.saved.clone();
        let generation = self.device_check.advance();
        let task = cx.background_executor().spawn(async move {
            let choices = device_choices(options.backend).ok();
            let resolved_auto = (options.backend == backend::BackendKind::Auto)
                .then(|| choices.as_ref().map(|c| c.resolved))
                .flatten();
            let warning = saved_device_warning(&options, choices.as_ref());
            (resolved_auto, warning)
        });
        cx.spawn(async move |this, cx| {
            let (resolved_auto, warning) = task.await;
            _ = this.update(cx, |m, cx| {
                if !m.closing && m.device_check.accepts(generation) {
                    if let Some(resolved) = resolved_auto {
                        m.auto_backend = resolved;
                    }
                    m.device_warning = warning;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    pub fn update_saved_device_warning(
        &mut self,
        kind: backend::BackendKind,
        choices: Option<&DeviceChoices>,
    ) {
        if self.closing {
            return;
        }
        if kind == backend::BackendKind::Auto {
            if let Some(choices) = choices {
                self.auto_backend = choices.resolved;
            }
        }
        if kind == self.saved.backend {
            // A completed discovery supersedes an earlier background check for the saved backend.
            self.device_check.advance();
            self.device_warning = saved_device_warning(&self.saved, choices);
        }
    }
    pub fn start(&mut self, inspect: bool, cx: &mut Context<Self>) {
        if !self.can_start(cx) {
            return;
        }
        self.error.clear();
        self.busy = true;
        let mut options = self.saved.clone();
        options.inspect = inspect;
        let session = self.session.clone();
        let generation = self.generation.advance();
        let task = cx
            .background_executor()
            .spawn(async move { session.lock().expect("session control lock").start(options) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            _ = this.update(cx, |m, cx| {
                if !m.generation.accepts(generation) {
                    // shutdown() advances generation; leave busy alone while quitting.
                    if !m.closing {
                        m.busy = false;
                    }
                    return;
                }
                m.busy = false;
                match result {
                    Ok(()) => {
                        // Worker exists; phase stays "opening" until the device is running.
                        m.active = true;
                    }
                    Err(e) => m.error = e,
                };
                if m.quit_requested {
                    m.request_quit(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.session.lock().expect("session control lock").stop();
        cx.notify();
    }
    pub fn open_settings(&mut self, section: Option<Section>, cx: &mut Context<Self>) {
        if self.shutting_down() || self.opening_settings {
            return;
        }
        if let Some(section) = section {
            if self.preferences.section != section {
                self.preferences.section = section;
                self.quit_save_failed = false;
            }
        }
        let model = cx.entity();
        if let Some((handle, _)) = &self.settings {
            let handle = *handle;
            cx.defer(move |cx| {
                if model.read(cx).shutting_down() {
                    return;
                }
                if handle.update(cx, |_, w, _| w.activate_window()).is_err() {
                    model.update(cx, |m, cx| {
                        m.settings = None;
                        m.open_settings(None, cx);
                    });
                }
            });
            cx.notify();
            return;
        }
        self.opening_settings = true;
        let options = self.saved.clone();
        // Creating/activating a window can render it synchronously. Release the model first.
        cx.defer(move |cx| {
            if model.read(cx).shutting_down() {
                model.update(cx, |m, _| m.opening_settings = false);
                return;
            }
            let result =
                gpui_kit::open_window(windows::settings::window_options(), cx, |window, cx| {
                    let view = cx.new(|cx| SettingsView::new(model.clone(), options, window, cx));
                    let weak = view.downgrade();
                    window.on_window_should_close(cx, move |_, cx| {
                        _ = weak.update(cx, |v, cx| {
                            v.save(windows::settings::CloseTarget::Settings, cx)
                        });
                        false
                    });
                    view
                });
            let mut stale_window = None;
            model.update(cx, |m, cx| {
                m.opening_settings = false;
                match result {
                    Ok((handle, view)) if !m.shutting_down() => {
                        m.settings = Some((handle, view.clone()));
                        cx.defer(move |cx| view.update(cx, |v, cx| v.discover(cx)));
                    }
                    Ok((handle, _)) => stale_window = Some(handle),
                    Err(e) => m.error = e.to_string(),
                }
                cx.notify();
            });
            if let Some(handle) = stale_window {
                _ = handle.update(cx, |_, w, _| w.remove_window());
            }
        });
        cx.notify();
    }
    pub fn open_log(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down() || self.opening_log {
            return;
        }
        let model = cx.entity();
        if let Some(handle) = self.log_window {
            cx.defer(move |cx| {
                if model.read(cx).shutting_down() {
                    return;
                }
                if handle.update(cx, |_, w, _| w.activate_window()).is_err() {
                    model.update(cx, |m, cx| {
                        m.log_window = None;
                        m.set_logging(false, cx);
                        m.open_log(cx);
                    });
                }
            });
            return;
        }
        self.opening_log = true;
        cx.defer(move |cx| {
            if model.read(cx).shutting_down() {
                model.update(cx, |m, _| m.opening_log = false);
                return;
            }
            let close = model.clone();
            let observed = model.clone();
            let result = gpui_kit::open_window(windows::log::window_options(), cx, |window, cx| {
                window.on_window_should_close(cx, move |_, cx| {
                    close.update(cx, |m, cx| {
                        m.log_window = None;
                        m.set_logging(false, cx);
                    });
                    true
                });
                cx.new(|cx| windows::log::LogView::new(observed, cx))
            });
            let mut stale_window = None;
            model.update(cx, |m, cx| {
                m.opening_log = false;
                match result {
                    Ok((handle, _)) if !m.shutting_down() => {
                        m.log_window = Some(handle);
                    }
                    Ok((handle, _)) => stale_window = Some(handle),
                    Err(e) => m.error = e.to_string(),
                };
                cx.notify();
            });
            if let Some(handle) = stale_window {
                _ = handle.update(cx, |_, w, _| w.remove_window());
            }
        });
    }
    pub fn set_logging(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.closing {
            self.logging = false;
            self.log = None;
            cx.notify();
            return;
        }
        self.session
            .lock()
            .expect("session control lock")
            .set_logging(enabled);
        self.logging = enabled;
        if !enabled {
            self.log = None;
        }
        cx.notify();
    }
    pub fn open_spectrum(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down() || self.opening_spectrum {
            return;
        }
        let model = cx.entity();
        if let Some(handle) = self.spectrum_window {
            cx.defer(move |cx| {
                if model.read(cx).shutting_down() {
                    return;
                }
                if handle.update(cx, |_, w, _| w.activate_window()).is_err() {
                    model.update(cx, |m, cx| {
                        m.spectrum_window = None;
                        m.open_spectrum(cx);
                    });
                }
            });
            return;
        }
        self.opening_spectrum = true;
        cx.defer(move |cx| {
            if model.read(cx).shutting_down() {
                model.update(cx, |m, _| m.opening_spectrum = false);
                return;
            }
            let controller = match crate::spectrum::SpectrumController::start() {
                Ok(c) => c,
                Err(e) => {
                    model.update(cx, |m, cx| {
                        m.opening_spectrum = false;
                        m.report(e, cx);
                    });
                    return;
                }
            };
            let close = model.clone();
            let observed = model.clone();
            let result =
                gpui_kit::open_window(windows::spectrum::window_options(), cx, |window, cx| {
                    window.on_window_should_close(cx, move |_, cx| {
                        close.update(cx, |m, cx| {
                            m.spectrum_window = None;
                            cx.notify();
                        });
                        true
                    });
                    cx.new(|cx| windows::spectrum::SpectrumView::new(controller, observed, cx))
                });
            let mut stale_window = None;
            model.update(cx, |m, cx| {
                m.opening_spectrum = false;
                match result {
                    Ok((handle, _)) if !m.shutting_down() => m.spectrum_window = Some(handle),
                    Ok((handle, _)) => stale_window = Some(handle),
                    Err(e) => m.error = e.to_string(),
                };
                cx.notify();
            });
            if let Some(handle) = stale_window {
                _ = handle.update(cx, |_, w, _| w.remove_window());
            }
        });
    }
    pub fn request_quit(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.quit_requested = true;
        if let Some((_, view)) = &self.settings {
            let view = view.clone();
            cx.defer(move |cx| {
                view.update(cx, |v, cx| {
                    v.save(super::windows::settings::CloseTarget::Application, cx)
                })
            });
        } else if !self.busy {
            self.persist_and_quit(cx);
        }
    }
    pub fn persist_and_quit(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.busy {
            return;
        }
        if (self.saved == self.persisted_options && self.preferences == self.persisted_preferences)
            || self.quit_save_failed
        {
            self.shutdown(cx);
            return;
        }
        self.busy = true;
        let options = self.saved.clone();
        let persisted = self.preferences;
        let prefs = persisted.stored();
        let path = self.path.clone();
        let written = options.clone();
        let task = cx.background_executor().spawn(async move {
            crate::gui_settings::save(&path, &written, &prefs).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            _ = this.update(cx, |m, cx| {
                m.busy = false;
                match result {
                    Ok(()) => {
                        m.persisted_options = options;
                        m.persisted_preferences = persisted;
                        m.quit_save_failed = false;
                        m.shutdown(cx);
                    }
                    Err(e) => {
                        m.quit_requested = false;
                        m.quit_save_failed = true;
                        m.report(e, cx);
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }
    pub fn shutdown(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.generation.advance();
        let session = self.session.clone();
        let task = cx.background_executor().spawn(async move {
            session.lock().expect("session control lock").shutdown();
        });
        cx.spawn(async move |_, cx| {
            task.await;
            cx.update(|cx| cx.quit());
        })
        .detach();
        cx.notify();
    }
}

pub fn phase_label(phase: &str) -> &'static str {
    match phase {
        "opening" => "開始中",
        "running" => "動作中",
        "stopped" => "停止済み",
        "inspected" => "検査完了",
        "error" => "エラー",
        _ => "準備完了",
    }
}

pub struct DeviceChoices {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub resolved: backend::BackendKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceWarning {
    Unavailable,
    Missing,
}

impl DeviceWarning {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "使える音声機器が見つかりません。接続と設定を確認してください。",
            Self::Missing => {
                "前回使った音声機器が見つかりません。入力・出力デバイスを確認してください。"
            }
        }
    }
}

/// True when `name` resolves the same way the audio backend selects a device.
///
/// CPAL accepts the list label, the bare device name, or the device id, and
/// requires a unique match. ASIO matches driver names without regard to ASCII case.
pub fn device_listed(name: &str, devices: &[String], kind: backend::BackendKind) -> bool {
    if name.is_empty() {
        return true;
    }
    match kind {
        backend::BackendKind::Asio => devices
            .iter()
            .filter(|device| !device.is_empty())
            .any(|device| device.eq_ignore_ascii_case(name)),
        backend::BackendKind::Cpal | backend::BackendKind::Auto => {
            devices
                .iter()
                .filter(|label| !label.is_empty() && cpal_device_matches(name, label))
                .count()
                == 1
        }
    }
}

fn cpal_device_matches(name: &str, label: &str) -> bool {
    if name == label {
        return true;
    }
    split_cpal_label(label).is_some_and(|(device_name, id)| name == device_name || name == id)
}

fn split_cpal_label(label: &str) -> Option<(&str, &str)> {
    let end = label.rfind(']')?;
    if end + 1 != label.len() {
        return None;
    }
    let start = label[..end].rfind(" [")?;
    Some((&label[..start], &label[start + 2..end]))
}

fn saved_device_warning(
    options: &Options,
    choices: Option<&DeviceChoices>,
) -> Option<DeviceWarning> {
    let Some(choices) = choices else {
        return Some(DeviceWarning::Unavailable);
    };
    let missing = |name: &str, devices: &[String]| {
        !name.is_empty() && !device_listed(name, devices, choices.resolved)
    };
    (missing(&options.device, &choices.inputs)
        || (choices.resolved == backend::BackendKind::Cpal
            && (missing(&options.output_device, &choices.outputs)
                || missing(&options.preview_device, &choices.outputs))))
    .then_some(DeviceWarning::Missing)
}

pub fn device_choices(kind: backend::BackendKind) -> Result<DeviceChoices, String> {
    let kind = backend::resolve(kind);
    #[cfg(all(windows, feature = "asio"))]
    let _apartment = if kind == backend::BackendKind::Asio {
        Some(crate::session::ComApartment::initialize().map_err(|e| e.to_string())?)
    } else {
        None
    };
    let selected = backend::select(kind, "").map_err(|e| e.to_string())?;
    Ok(DeviceChoices {
        inputs: selected.device_names().map_err(|e| e.to_string())?,
        outputs: selected.output_device_names().map_err(|e| e.to_string())?,
        resolved: kind,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        device_listed, saved_device_warning, split_cpal_label, DeviceChoices, DeviceWarning,
    };
    use crate::{backend::BackendKind, cli::Options};

    #[test]
    fn saved_device_warning_checks_each_cpal_route_and_distinguishes_enumeration_failure() {
        let choices = DeviceChoices {
            inputs: vec!["mic [in-1]".into()],
            outputs: vec!["speaker [out-1]".into(), "headphones [out-2]".into()],
            resolved: BackendKind::Cpal,
        };
        let options = Options {
            device: "mic [in-1]".into(),
            output_device: "speaker [out-1]".into(),
            preview_device: "headphones [out-2]".into(),
            ..Default::default()
        };
        assert_eq!(saved_device_warning(&options, Some(&choices)), None);
        assert_eq!(
            saved_device_warning(&Options::default(), Some(&choices)),
            None
        );
        for missing in [
            Options {
                device: "missing".into(),
                ..options.clone()
            },
            Options {
                output_device: "missing".into(),
                ..options.clone()
            },
            Options {
                preview_device: "missing".into(),
                ..options.clone()
            },
        ] {
            assert_eq!(
                saved_device_warning(&missing, Some(&choices)),
                Some(DeviceWarning::Missing)
            );
        }
        assert_eq!(
            saved_device_warning(&options, None),
            Some(DeviceWarning::Unavailable)
        );
        for (device, output, preview) in [
            ("mic", "speaker [out-1]", "headphones [out-2]"),
            ("in-1", "out-1", "out-2"),
            ("mic [in-1]", "speaker", "headphones"),
        ] {
            let aliases = Options {
                device: device.into(),
                output_device: output.into(),
                preview_device: preview.into(),
                ..Default::default()
            };
            assert_eq!(
                saved_device_warning(&aliases, Some(&choices)),
                None,
                "device={device} output={output} preview={preview}"
            );
        }
    }

    #[test]
    fn device_listed_matches_cpal_aliases_and_asio_case() {
        let labels = vec![
            "Built-in Mic [mic-1]".into(),
            "USB Mic [mic-2]".into(),
            "Shared Name [a]".into(),
            "Shared Name [b]".into(),
        ];
        assert!(device_listed(
            "Built-in Mic [mic-1]",
            &labels,
            BackendKind::Cpal
        ));
        assert!(device_listed("Built-in Mic", &labels, BackendKind::Cpal));
        assert!(device_listed("mic-1", &labels, BackendKind::Cpal));
        assert!(!device_listed("Shared Name", &labels, BackendKind::Cpal));
        assert!(!device_listed("missing", &labels, BackendKind::Cpal));
        assert_eq!(
            split_cpal_label("Built-in Mic [mic-1]"),
            Some(("Built-in Mic", "mic-1"))
        );

        let drivers = vec!["ASIO4ALL v2".into(), "Realtek ASIO".into()];
        assert!(device_listed("asio4all v2", &drivers, BackendKind::Asio));
        assert!(device_listed("Realtek ASIO", &drivers, BackendKind::Asio));
        assert!(!device_listed("missing", &drivers, BackendKind::Asio));
    }

    #[cfg(feature = "desktop-tests")]
    #[gpui_kit::test]
    fn discovery_recovers_start_only_for_saved_backend(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext;

        let model = cx.update(|cx| {
            cx.new(|cx| {
                let options = Options {
                    backend: BackendKind::Cpal,
                    device: "mic".into(),
                    ..Default::default()
                };
                super::AppModel::new(
                    options.clone(),
                    options,
                    Default::default(),
                    super::Preferences::system(),
                    cx,
                )
            })
        });
        let choices = DeviceChoices {
            inputs: vec!["mic".into()],
            outputs: vec![],
            resolved: BackendKind::Cpal,
        };
        model.update(cx, |model, cx| {
            let older_check = model.device_check.advance();
            model.device_warning = Some(DeviceWarning::Unavailable);
            assert!(!model.can_start(cx));
            model.update_saved_device_warning(BackendKind::Auto, Some(&choices));
            assert!(!model.can_start(cx));
            assert!(model.device_check.accepts(older_check));
            model.update_saved_device_warning(BackendKind::Cpal, Some(&choices));
            assert!(model.can_start(cx));
            assert!(!model.device_check.accepts(older_check));
            model.saved.device = "missing".into();
            model.update_saved_device_warning(BackendKind::Cpal, Some(&choices));
            assert_eq!(model.device_warning, Some(DeviceWarning::Missing));
            assert!(!model.can_start(cx));
            model.saved.device = "mic".into();
            model.update_saved_device_warning(BackendKind::Cpal, None);
            assert!(!model.can_start(cx));
            assert_eq!(model.device_warning, Some(DeviceWarning::Unavailable));
        });
    }

    #[cfg(feature = "desktop-tests")]
    #[gpui_kit::test]
    fn persist_and_quit_skips_unmodified_writes_and_allows_retry_after_save_failure(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext;

        let blocker = std::env::temp_dir().join(format!(
            "mofu-quit-readonly-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&blocker, "not a directory").unwrap();
        let settings_path = blocker.join("settings.ini");

        let prefs = super::Preferences {
            language: crate::gui::preferences::Language::En,
            section: super::Section::Voice,
        };
        let unmodified = cx.update(|cx| {
            cx.new(|cx| {
                super::AppModel::new(
                    Options::default(),
                    Options::default(),
                    settings_path.clone(),
                    prefs,
                    cx,
                )
            })
        });
        unmodified.update(cx, |m, cx| m.request_quit(cx));
        cx.run_until_parked();
        assert!(unmodified.read_with(cx, |m, _| m.closing && m.error.is_empty()));

        let modified = cx.update(|cx| {
            cx.new(|cx| {
                super::AppModel::new(
                    Options::default(),
                    Options::default(),
                    settings_path,
                    prefs,
                    cx,
                )
            })
        });
        modified.update(cx, |m, cx| {
            m.preferences.language = crate::gui::preferences::Language::Ja;
            m.request_quit(cx);
        });
        cx.run_until_parked();
        assert!(modified.read_with(cx, |m, _| !m.closing
            && !m.quit_requested
            && m.quit_save_failed
            && !m.error.is_empty()));

        modified.update(cx, |m, cx| m.request_quit(cx));
        cx.run_until_parked();
        assert!(modified.read_with(cx, |m, _| m.closing));

        std::fs::remove_file(blocker).unwrap();
    }

    #[cfg(feature = "desktop-tests")]
    #[gpui_kit::test]
    fn persist_and_quit_writes_launch_overrides(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext;

        let directory = std::env::temp_dir().join(format!(
            "mofu-quit-launch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let settings_path = directory.join("settings.ini");
        let disk = Options::default();
        let mut launched = disk.clone();
        launched.pitch.semitones = 7.5;

        let model = cx.update(|cx| {
            cx.new(|cx| {
                super::AppModel::new(
                    launched,
                    disk,
                    settings_path.clone(),
                    super::Preferences {
                        language: crate::gui::preferences::Language::En,
                        section: super::Section::Voice,
                    },
                    cx,
                )
            })
        });
        model.update(cx, |m, cx| m.request_quit(cx));
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.closing && m.error.is_empty()));
        let saved = crate::gui_settings::load(&settings_path).unwrap();
        assert_eq!(saved.options.pitch.semitones, 7.5);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[cfg(feature = "desktop-tests")]
    #[gpui_kit::test]
    fn closing_cancels_deferred_window_open_and_avoids_session_lock_in_set_logging(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext;

        let model = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::gui::theme::init(cx);
            cx.new(|cx| {
                super::AppModel::new(
                    Options::default(),
                    Options::default(),
                    Default::default(),
                    super::Preferences::system(),
                    cx,
                )
            })
        });
        model.update(cx, |m, cx| {
            m.open_log(cx);
            m.closing = true;
        });
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.log_window.is_none() && !m.opening_log));

        model.update(cx, |m, cx| {
            m.closing = false;
            m.open_settings(None, cx);
            m.open_log(cx);
            m.open_spectrum(cx);
            m.quit_requested = true;
        });
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.settings.is_none()
            && !m.opening_settings
            && m.log_window.is_none()
            && !m.opening_log
            && m.spectrum_window.is_none()
            && !m.opening_spectrum));

        model.update(cx, |m, _| m.closing = true);
        let session = model.read_with(cx, |m, _| m.session.clone());
        let _held = session.lock().unwrap();
        model.update(cx, |m, cx| m.set_logging(true, cx));
        assert!(model.read_with(cx, |m, _| !m.logging));
    }

    #[cfg(feature = "desktop-tests")]
    #[gpui_kit::test]
    fn quit_requested_locks_start_and_controls(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext;

        let model = cx.update(|cx| {
            cx.new(|cx| {
                let options = Options {
                    backend: BackendKind::Cpal,
                    ..Default::default()
                };
                super::AppModel::new(
                    options.clone(),
                    options,
                    Default::default(),
                    super::Preferences::system(),
                    cx,
                )
            })
        });
        model.update(cx, |m, cx| {
            assert!(m.can_start(cx));
            assert!(!m.locked());
            m.quit_requested = true;
            assert!(m.shutting_down());
            assert!(m.locked());
            assert!(!m.can_start(cx));
        });
    }
}
