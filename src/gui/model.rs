use super::{
    draft::Generation, preferences::Preferences, windows, windows::settings::SettingsView,
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
    pub preferences: Preferences,
    pub path: PathBuf,
    pub info: AudioInfo,
    pub log: Option<AudioLog>,
    pub status: String,
    pub error: String,
    pub busy: bool,
    pub active: bool,
    pub closing: bool,
    pub quit_requested: bool,
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
                        || m.log != snapshot.3
                    {
                        m.active = snapshot.0;
                        m.info = snapshot.1;
                        m.status = snapshot.2;
                        m.log = snapshot.3;
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
        Self {
            session,
            saved,
            preferences,
            path,
            info: AudioInfo::default(),
            log: None,
            status: String::new(),
            error: String::new(),
            busy: false,
            active: false,
            closing: false,
            quit_requested: false,
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
        self.active || self.busy || self.closing
    }
    pub fn report(&mut self, error: impl ToString, cx: &mut Context<Self>) {
        self.error = error.to_string();
        cx.notify();
    }
    pub fn start(&mut self, inspect: bool, cx: &mut Context<Self>) {
        if self.locked() {
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
                    return;
                }
                m.busy = false;
                match result {
                    Ok(()) => m.active = true,
                    Err(e) => m.error = e,
                };
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
    pub fn open_settings(&mut self, section: Option<String>, cx: &mut Context<Self>) {
        if self.closing || self.opening_settings {
            return;
        }
        if let Some(section) = section {
            self.preferences.section = section;
        }
        let model = cx.entity();
        if let Some((handle, _)) = &self.settings {
            let handle = *handle;
            cx.defer(move |cx| {
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
            model.update(cx, |m, cx| {
                m.opening_settings = false;
                match result {
                    Ok((handle, view)) => {
                        m.settings = Some((handle, view.clone()));
                        cx.defer(move |cx| view.update(cx, |v, cx| v.discover(cx)));
                    }
                    Err(e) => m.error = e.to_string(),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    pub fn open_log(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.opening_log {
            return;
        }
        let model = cx.entity();
        if let Some(handle) = self.log_window {
            cx.defer(move |cx| {
                if handle.update(cx, |_, w, _| w.activate_window()).is_err() {
                    model.update(cx, |m, cx| {
                        m.log_window = None;
                        m.open_log(cx);
                    });
                }
            });
            return;
        }
        self.opening_log = true;
        cx.defer(move |cx| {
            let close = model.clone();
            let observed = model.clone();
            let result = gpui_kit::open_window(windows::log::window_options(), cx, |window, cx| {
                window.on_window_should_close(cx, move |_, cx| {
                    close.update(cx, |m, cx| {
                        m.log_window = None;
                        m.session
                            .lock()
                            .expect("session control lock")
                            .set_logging(false);
                        cx.notify();
                    });
                    true
                });
                cx.new(|cx| windows::log::LogView::new(observed, cx))
            });
            model.update(cx, |m, cx| {
                m.opening_log = false;
                match result {
                    Ok((handle, _)) => {
                        m.log_window = Some(handle);
                        m.session
                            .lock()
                            .expect("session control lock")
                            .set_logging(true);
                    }
                    Err(e) => m.error = e.to_string(),
                };
                cx.notify();
            });
        });
    }
    pub fn open_spectrum(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.opening_spectrum {
            return;
        }
        let model = cx.entity();
        if let Some(handle) = self.spectrum_window {
            cx.defer(move |cx| {
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
            model.update(cx, |m, cx| {
                m.opening_spectrum = false;
                match result {
                    Ok((handle, _)) => m.spectrum_window = Some(handle),
                    Err(e) => m.error = e.to_string(),
                };
                cx.notify();
            });
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
        self.busy = true;
        let options = self.saved.clone();
        let prefs = self.preferences.clone();
        let path = self.path.clone();
        let task = cx.background_executor().spawn(async move {
            prefs.save(&path.with_file_name("ui.ini"))?;
            crate::gui_settings::save(&path, &options).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            _ = this.update(cx, |m, cx| {
                m.busy = false;
                match result {
                    Ok(()) => m.shutdown(cx),
                    Err(e) => {
                        m.quit_requested = false;
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

pub struct DeviceChoices {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub resolved: backend::BackendKind,
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
