#![windows_subsystem = "windows"]

use rust_low_latency_voice_changer::gui;
use std::process::ExitCode;

fn main() -> ExitCode {
    let result = gui::initial_options(std::env::args().skip(1)).and_then(|options| {
        if options.help {
            gui::show_message(
                "rust-low-latency-voice-changer-gui [--device NAME] [--frames N] [--pitch SEMITONES] [--formant preserve|off] [--formant-shift SEMITONES] [--gate on|off] [--gate-threshold auto|DB] [--gate-attack MS] [--gate-release MS]\n\nSelect an ASIO device in Settings or with --device NAME.\nDefault: 512 frames / pitch 0 / preserve formants / gate off.\nPitch and formant shift range: -12..12 (fractional allowed). Formant shift defaults to 0; nonzero shift requires preserve.\nGate is a soft pre-pitch expander; threshold defaults to auto.\nUse Inspect, Start and Stop in the main window. Open the native spectrum window as needed. Stop before changing settings.",
                false,
            );
            Ok(())
        } else {
            gui::run(options)
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            gui::show_message(&error.to_string(), true);
            ExitCode::FAILURE
        }
    }
}
