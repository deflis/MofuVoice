use rust_low_latency_voice_changer::pitch::LATENCY;
use rust_low_latency_voice_changer::{audio::DspSettings, backend, cli};
use rust_low_latency_voice_changer::{RATE, TARGET_FRAMES};
use std::{
    error::Error,
    io,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn Error>> {
    let options = cli::Options::parse(std::env::args().skip(1))?;
    if options.help {
        println!("Usage: rust-low-latency-voice-changer-cli [--backend auto|cpal|asio] [--device NAME] [--output-device NAME] [--input-channel N] [--output-channels N,N] [--frames N] [--seconds N] [--pitch SEMITONES] [--formant preserve|off] [--formant-shift SEMITONES] [--auto-level on|off] [--level-target DB] [--level-max-gain DB] [--level-floor DB] [--level-response MS] [--compress-threshold DB] [--compress-ratio N] [--compress-attack MS] [--compress-release MS] [--rumble-filter on|off] [--gate on|off] [--gate-threshold auto|DB] [--gate-attack MS] [--gate-release MS]\n       rust-low-latency-voice-changer-cli --inspect --device NAME [--frames N]\n       rust-low-latency-voice-changer-cli --list-devices\nBackend: --backend auto|cpal|asio (default auto, ASIO preferred on Windows when a registered driver is available); CPAL uses the default input/output unless selected with --device INPUT and --output-device OUTPUT.\nSelect a device with --device NAME; use --list-devices to see registered names.\nDefault: 512 frames / pitch 0 / rumble filter off / gate off / auto level off. Pitch and formant shift range: -12..12 (fractional allowed). Formant shift defaults to 0; nonzero shift requires preserve.\nAuto level is a post-pitch RMS leveler and soft-knee compressor with a -1 dBFS peak guard. Defaults: target -20 dBFS RMS, max gain +9 dB, activity floor -50 dBFS, response 1500 ms; compression -12 dBFS / 3:1 / attack 5 ms / release 150 ms. All parameters are customizable. No added sample delay.\nRumble filter is an optional pre-gate 80 Hz Butterworth high-pass (default off); no lookahead or buffered sample delay.\nGate is a soft pre-pitch expander; threshold defaults to auto noise-floor tracking; attack/release default to 5/80 ms.\nRouting: --input-channel N --output-channels 1,2 (default 1 -> 1); other outputs silent. Pitch 0, formant shift 0, rumble filter off, gate off, and auto level off: bit-preserving zero-DSP-delay bypass; backend queue and hardware latency are separate.\nWet DSP: spectral phase locking, fixed {LATENCY} samples of delay. Formant correction: cepstral envelope, default preserve; --formant off disables it.\nEnter stops interactive mode.");
        return Ok(());
    }
    if !options.list
        && backend::resolve(options.backend) == backend::BackendKind::Asio
        && options.device.is_empty()
    {
        return Err("--device requires an exact audio device name; use --list-devices".into());
    }
    let backend = backend::select(options.backend, &options.output_device)?;
    if options.list {
        println!("Available audio devices (not opened):");
        for name in backend.device_names()? {
            println!("  {name}");
        }
        println!("Available output devices (CPAL only):");
        for name in backend.output_device_names()? {
            println!("  {name}");
        }
        return Ok(());
    }
    let routing = backend::ChannelRouting::from(&options);
    let details = backend.inspect(&options.device, options.frames, &routing)?;
    println!(
        "Selected: {}\n{details}",
        if options.device.is_empty() {
            "OS default"
        } else {
            &options.device
        }
    );
    if options.inspect {
        println!(
            "Requested 48 kHz / {} frames supported. No audio buffers created.",
            options.frames
        );
        return Ok(());
    }
    let mut engine = backend.open(
        &options.device,
        options.frames,
        &routing,
        DspSettings::from(&options),
    )?;
    engine.print_latencies()?;
    println!(
        "Rumble filter: {}; 80 Hz high-pass; no lookahead or buffered sample delay.",
        if options.rumble_filter { "on" } else { "off" }
    );
    println!(
        "Auto level: {}; added DSP delay=0 samples.",
        if options.level.enabled { "on" } else { "off" }
    );
    if options.pitch == 0.0 && options.formant_shift == 0.0 {
        println!("Pitch/formant DSP=0 samples; rumble filter, gate and auto level add 0 buffered samples; backend queue latency is reported separately. Selected input -> selected outputs.");
    } else {
        println!(
            "Pitch={} semitones; spectral wet DSP delay={LATENCY} samples ({:.3} ms).",
            options.pitch,
            LATENCY as f64 * 1000.0 / RATE as f64
        );
    }
    println!(
        "Noise gate: {}; threshold={:?}; attack={} ms; release={} ms (pre-pitch soft expander, no lookahead).",
        if options.gate { "on" } else { "off" },
        options.gate_threshold,
        options.gate_attack_ms,
        options.gate_release_ms
    );
    println!(
        "Requested audio period: {:.3} ms; target {} samples / {:.3} ms. Total latency remains unmeasured.",
        options.frames as f64 * 1000.0 / RATE as f64,
        TARGET_FRAMES,
        TARGET_FRAMES as f64 * 1000.0 / RATE as f64
    );
    println!("Formant correction: {}. Cepstral spectral envelope; window=2048 samples, update=128 samples. Listening quality unverified.", if options.preserve_formants { "preserve" } else { "off" });
    engine.start()?;
    println!(
        "Formant shift: {} semitones (independent of pitch).",
        options.formant_shift
    );
    println!("Audio running. Press Enter to stop.");
    let (stop_tx, stop_rx) = mpsc::channel();
    if options.seconds.is_none() {
        thread::spawn(move || {
            let mut line = String::new();
            let _ = stop_tx.send(io::stdin().read_line(&mut line));
        });
    }
    let start = Instant::now();
    let mut next_report = Duration::from_secs(1);
    let mut last_blocks = 0;
    let mut last_progress = Instant::now();
    let result: Result<(), Box<dyn Error>> = (|| {
        loop {
            if options.seconds.is_none() {
                if let Ok(result) = stop_rx.try_recv() {
                    result?;
                    break;
                }
            }
            engine.check_events()?;
            let blocks = engine.blocks();
            if blocks != last_blocks {
                last_progress = Instant::now();
                last_blocks = blocks;
            }
            if last_progress.elapsed() > Duration::from_secs(2) {
                return Err("No audio callbacks for 2 seconds".into());
            }
            let elapsed = start.elapsed();
            if elapsed >= next_report {
                engine.report();
                next_report += Duration::from_secs(1);
            }
            if options
                .seconds
                .is_some_and(|seconds| elapsed >= Duration::from_secs(seconds))
            {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    })();
    let stop_result = engine.stop();
    engine.report();
    result?;
    stop_result?;
    engine.check_events()?;
    engine.check_test_result()?;
    Ok(())
}
