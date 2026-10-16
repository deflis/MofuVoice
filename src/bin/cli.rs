use rust_low_latency_voice_changer::pitch::LATENCY;
use rust_low_latency_voice_changer::TARGET_FRAMES;
use rust_low_latency_voice_changer::{audio::DspSettings, backend, cli};
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
        println!("Usage: rust-low-latency-voice-changer-cli [--backend auto|cpal|asio] [--device NAME] [--output-device NAME] [--input-channel N] [--output-channels N,N] [--frames N] [--sample-rate HZ] [--seconds N] [--pitch SEMITONES] [--pitch-algorithm phase-locked|pvsola|td-psola] [--formant preserve|off] [--formant-shift SEMITONES] [--eq on|off] [--eq-mode simple|pro] [--eq-bandN HZ,DB,Q] [--eq-output DB] [--auto-level on|off] [--compressor on|off] [--level-target DB] [--level-max-gain DB] [--level-floor DB] [--level-response MS] [--compress-threshold DB] [--compress-ratio N] [--compress-attack MS] [--compress-release MS] [--deesser on|off] [--deesser-mode simple|pro] [--deesser-frequency HZ] [--deesser-threshold DBFS] [--deesser-range DB] [--deesser-attack MS] [--deesser-release MS] [--rumble-filter on|off] [--gate on|off] [--gate-threshold auto|DB] [--gate-attack MS] [--gate-release MS]\n       rust-low-latency-voice-changer-cli --inspect --device NAME [--frames N] [--sample-rate HZ]\n       rust-low-latency-voice-changer-cli --list-devices\nBackend: --backend auto|cpal|asio (default auto, ASIO preferred on Windows when a registered driver is available); CPAL uses the default input/output unless selected with --device INPUT and --output-device OUTPUT.\nSelect a device with --device NAME; use --list-devices to see registered names.\nSample rate: 44100 or 48000 (default) Hz. ASIO driver settings must match.\nDefault: 512 frames / pitch 0 / rumble filter off / de-esser off / gate off / EQ off / auto level off / compressor off. Pitch and formant shift range: -12..12 (fractional allowed). Formant shift defaults to 0; nonzero shift requires preserve.\nAuto level is a post-pitch RMS leveler; --compressor independently enables soft-knee compression with a -1 dBFS peak guard. Defaults: target -20 dBFS RMS, max gain +9 dB, activity floor -50 dBFS, response 1500 ms; compression -12 dBFS / 3:1 / attack 5 ms / release 150 ms. All parameters are customizable. No added sample delay.\nRumble filter is an optional pre-gate 80 Hz Butterworth high-pass (default off); no lookahead or buffered sample delay.\nEQ is a post-pitch peaking filter with 1..16 contiguous bands (--eq-band1..--eq-band16): frequency 40..16000 Hz, gain -12..12 dB, Q 0.2..8; output trim -18..0 dB. Positive gains reserve headroom automatically. No added buffered sample delay. --eq-mode changes GUI controls only.\nGate is a soft pre-pitch expander; threshold defaults to auto noise-floor tracking; attack/release default to 5/80 ms.\nRouting: --input-channel N --output-channels 1,2 (default 1 -> 1); other outputs silent. Pitch 0, formant shift 0, rumble filter off, de-esser off, gate off, EQ off, auto level off, and compressor off: bit-preserving zero-DSP-delay bypass; backend queue and hardware latency are separate.\nPitch algorithm: phase-locked (default), pvsola (experimental synchronized phase restoration), or td-psola (time-domain pitch-synchronous waveform grains).\nWet DSP: spectral phase locking, fixed {LATENCY} samples of delay for spectral modes; TD-PSOLA uses 4608 samples. TD-PSOLA requires --formant preserve and --formant-shift 0. Formant correction: cepstral envelope, default preserve; --formant off disables it.\nEnter stops interactive mode.");
        return Ok(());
    }
    if !options.list
        && backend::resolve(options.backend) == backend::BackendKind::Asio
        && options.device.is_empty()
    {
        return Err("--device requires an exact audio device name; use --list-devices".into());
    }
    let backend = backend::select_with_sample_rate(
        options.backend,
        &options.output_device,
        options.sample_rate,
    )?;
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
            "Requested {} Hz / {} frames supported. No audio buffers created.",
            options.sample_rate, options.frames
        );
        return Ok(());
    }
    let mut engine = backend.open(
        &options.device,
        options.frames,
        &routing,
        DspSettings::from(&options),
    )?;
    #[cfg(windows)]
    rust_low_latency_voice_changer::spectrum::set_sample_rate(options.sample_rate);
    engine.print_latencies()?;
    println!("Pitch algorithm: {}", options.pitch.algorithm.name());
    println!(
        "Rumble filter: {}; 80 Hz high-pass; no lookahead or buffered sample delay.",
        if options.rumble_filter { "on" } else { "off" }
    );
    println!(
        "EQ: {}; automatic headroom={} dB; output trim={} dB; added buffered DSP delay=0 samples.",
        if options.eq.enabled { "on" } else { "off" },
        options.eq.headroom_db(),
        options.eq.output_db
    );
    println!(
        "De-esser: {}; cutoff={} Hz; range={} dB; no buffered sample delay.",
        if options.deesser { "on" } else { "off" },
        options.deesser_parameters.frequency_hz,
        options.deesser_parameters.range_db
    );
    println!(
        "Auto level: {}; compressor: {}; added DSP delay=0 samples.",
        if options.level.enabled { "on" } else { "off" },
        if options.level.compressor_enabled {
            "on"
        } else {
            "off"
        }
    );
    if options.pitch.semitones == 0.0 && options.pitch.formant_shift == 0.0 {
        println!("Pitch/formant DSP=0 samples; rumble filter, de-esser, gate, EQ, auto level and compressor add 0 buffered samples; backend queue latency is reported separately. Selected input -> selected outputs.");
    } else {
        let latency = options.pitch.algorithm.latency();
        println!(
            "Pitch={} semitones; wet DSP delay={latency} samples ({:.3} ms).",
            options.pitch.semitones,
            latency as f64 * 1000.0 / options.sample_rate as f64
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
        options.frames as f64 * 1000.0 / options.sample_rate as f64,
        TARGET_FRAMES,
        TARGET_FRAMES as f64 * 1000.0 / options.sample_rate as f64
    );
    if options.pitch.algorithm == rust_low_latency_voice_changer::pitch::PitchAlgorithm::TdPsola {
        println!("TD-PSOLA: time-domain pitch-synchronous waveform grains; formants preserved, independent formant shift unavailable; fixed delay=4608 samples.");
    } else {
        println!("Formant correction: {}. Cepstral spectral envelope; window=2048 samples, update=128 samples. Listening quality unverified.", if options.pitch.preserve_formants { "preserve" } else { "off" });
    }
    engine.start()?;
    println!(
        "Formant shift: {} semitones (independent of pitch).",
        options.pitch.formant_shift
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
