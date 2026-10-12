#![windows_subsystem = "windows"]

use rust_low_latency_voice_changer::gui;
use std::process::ExitCode;

fn main() -> ExitCode {
    let result = gui::initial_options(std::env::args().skip(1)).and_then(|options| {
        if options.help {
            gui::show_message(
                "rust-low-latency-voice-changer-gui [--device NAME] [--frames N] [--pitch SEMITONES] [--formant preserve|off] [--formant-shift SEMITONES] [--auto-level on|off] [--level-target DB] [--level-max-gain DB] [--level-floor DB] [--level-response MS] [--compress-threshold DB] [--compress-ratio N] [--compress-attack MS] [--compress-release MS] [--gate on|off] [--gate-threshold auto|DB] [--gate-attack MS] [--gate-release MS]\n\nSelect an ASIO device in the main window or with --device NAME.\nDefault: 512 frames / pitch 0 / preserve formants / gate off / auto level off.\nPitch and formant shift range: -12..12 (fractional allowed). Formant shift defaults to 0; nonzero shift requires preserve.\nAuto level is a post-pitch RMS leveler and soft-knee compressor with a -1 dBFS peak guard. Defaults: target -20 dBFS RMS, max gain +9 dB, activity floor -50 dBFS, response 1500 ms; compression -12 dBFS / 3:1 / attack 5 ms / release 150 ms. All parameters are customizable. No added sample delay.\nGate is a soft pre-pitch expander; threshold defaults to auto.\nUse Inspect, Start and Stop in the main window. Open the native spectrum window as needed. Stop before changing settings.",
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
