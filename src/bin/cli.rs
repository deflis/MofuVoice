use rust_low_latency_voice_changer::pitch::LATENCY;
use rust_low_latency_voice_changer::{asio, cli};
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
        println!("Usage: rust-low-latency-voice-changer-cli --device NAME [--frames N] [--seconds N] [--pitch SEMITONES] [--formant preserve|off] [--formant-shift SEMITONES] [--gate on|off] [--gate-threshold auto|DB] [--gate-attack MS] [--gate-release MS]\n       rust-low-latency-voice-changer-cli --inspect --device NAME [--frames N]\n       rust-low-latency-voice-changer-cli --list-devices\nSelect a device with --device NAME; use --list-devices to see registered names.\nDefault: 512 frames / pitch 0 / gate off. Pitch and formant shift range: -12..12 (fractional allowed). Formant shift defaults to 0; nonzero shift requires preserve.\nGate is a soft pre-pitch expander; threshold defaults to auto noise-floor tracking; attack/release default to 5/80 ms.\nInput 1 -> output 1; other outputs silent. Pitch 0, formant shift 0, and gate off: bit-preserving zero-delay bypass.\nWet DSP: spectral phase locking, fixed {LATENCY} samples of delay. Formant correction: cepstral envelope, default preserve; --formant off disables it.\nEnter stops interactive mode.");
        return Ok(());
    }
    if !options.list && options.device.is_empty() {
        return Err("--device requires an exact ASIO device name; use --list-devices".into());
    }
    let host = asio_sys::Asio::new();
    let names = host.driver_names();
    if options.list {
        println!("Registered ASIO drivers (not opened):");
        for name in names {
            println!("  {name}");
        }
        return Ok(());
    }
    let name = names
        .into_iter()
        .find(|name| name.eq_ignore_ascii_case(&options.device))
        .ok_or_else(|| {
            format!(
                "Requested ASIO device {} not found; use --list-devices",
                options.device
            )
        })?;
    let driver = host.load_driver(&name)?;
    println!("Selected: {}", driver.name());
    let config = asio::Configuration::query(&driver)?;
    config.print();
    config.validate(options.frames)?;
    if options.inspect {
        println!(
            "Requested 48 kHz / {} frames supported. No audio buffers created.",
            options.frames
        );
        return Ok(());
    }
    let mut engine = asio::Engine::open(
        driver,
        config,
        options.frames,
        options.pitch,
        options.preserve_formants,
        options.formant_shift,
        options.gate,
        options.gate_threshold,
        options.gate_attack_ms,
        options.gate_release_ms,
    )?;
    engine.print_latencies()?;
    if options.pitch == 0.0 && options.formant_shift == 0.0 && !options.gate {
        println!("Pitch=0; gate off; DSP=0 samples; FIFO=0 samples. Input 1 -> output 1.");
    } else {
        println!("Pitch={} semitones; ratio={:.6}; spectral wet DSP delay={LATENCY} samples ({:.3} ms). Formant mode reported below.", options.pitch, rust_low_latency_voice_changer::pitch::pitch_ratio(options.pitch)?, LATENCY as f64 * 1000.0 / RATE as f64);
    }
    println!(
        "Noise gate: {}; threshold={:?}; attack={} ms; release={} ms (pre-pitch soft expander, no lookahead).",
        if options.gate { "on" } else { "off" },
        options.gate_threshold,
        options.gate_attack_ms,
        options.gate_release_ms
    );
    println!(
        "ASIO period: {:.3} ms; target {} samples / {:.3} ms. Total latency remains unmeasured.",
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
    println!("Running single full-duplex callback. Press Enter to stop.");
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
                return Err("No ASIO callbacks for 2 seconds".into());
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
