//! GPUI presentation. Audio callbacks never access a GPUI entity.
use crate::{cli::Options, gui_settings};
use gpui_kit::*;
use std::error::Error;
mod draft;
mod layout;
mod model;
mod plots;
mod preferences;
mod settings;
mod theme;
mod windows;
gpui_kit::actions!(mofuvoice, [Quit]);

pub fn initial_options(args: impl IntoIterator<Item = String>) -> Result<Options, Box<dyn Error>> {
    let args: Vec<_> = args.into_iter().collect();
    if args.iter().any(|arg| arg == "--help") {
        return Options::parse_gui(args);
    }
    gui_settings::merge(&gui_settings::load(&gui_settings::path()?)?, args)
}
pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let path = gui_settings::path()?;
    let preferences = preferences::Preferences::load(&path.with_file_name("ui.ini"))?;
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::init(cx);
            let model = cx.new(|cx| model::AppModel::new(options, path, preferences, cx));
            let quit_model = model.clone();
            cx.bind_keys([KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-q"
                } else {
                    "ctrl-q"
                },
                Quit,
                None,
            )]);
            cx.on_action(move |_: &Quit, cx| quit_model.update(cx, |m, cx| m.request_quit(cx)));
            match gpui_kit::open_window(windows::main::window_options(), cx, |window, cx| {
                let close_model = model.clone();
                window.on_window_should_close(cx, move |_, cx| {
                    close_model.update(cx, |m, cx| m.request_quit(cx));
                    false
                });
                cx.new(|cx| windows::main::MainView::new(model.clone(), window, cx))
            }) {
                Ok(_) => cx.activate(true),
                Err(error) => {
                    show_message(&format!("Cannot open main window: {error}"), true);
                    cx.quit();
                }
            }
        });
    Ok(())
}
#[cfg(windows)]
pub fn show_message(message: &str, error: bool) {
    use ::windows::{
        core::PCWSTR,
        Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK},
    };
    let title: Vec<_> = "MofuVoice\0".encode_utf16().collect();
    let message: Vec<_> = message.encode_utf16().chain(Some(0)).collect();
    // SAFETY: null-terminated buffers remain alive through this synchronous call.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK
                | if error {
                    MB_ICONERROR
                } else {
                    MB_ICONINFORMATION
                },
        );
    }
}
#[cfg(not(windows))]
pub fn show_message(message: &str, _error: bool) {
    eprintln!("MofuVoice: {message}");
}
