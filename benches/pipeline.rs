//! Device-independent cost of the complete streaming chain. Each case has equal
//! weight in the aggregate; timings include DSP and exclude device I/O and setup.
use rust_low_latency_voice_changer::{
    audio::{AudioProcessor, DspSettings},
    cli::Options,
};
use std::{hint::black_box, time::Instant};

// Count scheduled work separately from wall time: desktop scheduling pauses
// otherwise obscure DSP changes. Cycle counts are not device/round-trip latency.
#[cfg(windows)]
fn thread_cycles() -> u64 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut std::ffi::c_void;
        fn QueryThreadCycleTime(thread: *mut std::ffi::c_void, cycles: *mut u64) -> i32;
    }
    let mut cycles = 0;
    // SAFETY: current-thread pseudo handle is valid, cycles is writable u64 storage.
    assert_ne!(
        unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut cycles) },
        0
    );
    cycles
}

fn selected<T: std::str::FromStr + PartialEq>(name: &str, value: T) -> bool {
    match std::env::var(name) {
        Ok(text) => {
            text.parse::<T>()
                .unwrap_or_else(|_| panic!("invalid {name}"))
                == value
        }
        Err(std::env::VarError::NotPresent) => true,
        Err(error) => panic!("invalid {name}: {error}"),
    }
}

fn main() {
    let mut total = 0.0;
    let mut cases = 0;
    #[cfg(windows)]
    let mut total_cycles = 0.0;
    for rate in [44_100, 48_000] {
        if !selected("DSP_BENCH_RATE", rate) {
            continue;
        }
        let mut phase = 0.0;
        let mut seed = 31_u32;
        let input: Vec<f32> = (0..rate as usize * 2)
            .map(|i| {
                let t = i as f64 / rate as f64;
                phase += std::f64::consts::TAU * (113.0 + 27.0 * (t * 3.0).sin()) / rate as f64;
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = seed as i32 as f64 / i32::MAX as f64;
                let envelope = 0.15 + 0.85 * (t * 5.0).sin().powi(2);
                (envelope
                    * (0.20 * phase.sin()
                        + 0.06 * (3.0 * phase).sin()
                        + 0.03 * (7.0 * phase).sin()
                        + 0.015 * noise)) as f32
            })
            .collect();
        for frames in [64, 512] {
            if !selected("DSP_BENCH_FRAMES", frames) {
                continue;
            }
            for pitch in [-12, -3, 0, 3, 7, 12] {
                if !selected("DSP_BENCH_PITCH", pitch) {
                    continue;
                }
                for shift in [0, 3] {
                    let mut times = Vec::new();
                    let mut worst = 0.0_f64;
                    #[cfg(windows)]
                    let mut cycles = Vec::new();
                    for trial in 0..8 {
                        let options = Options::parse([
                            "--pitch".into(),
                            pitch.to_string(),
                            "--formant".into(),
                            "preserve".into(),
                            "--formant-shift".into(),
                            shift.to_string(),
                            "--gate".into(),
                            "on".into(),
                            "--rumble-filter".into(),
                            "on".into(),
                            "--auto-level".into(),
                            "on".into(),
                            "--deesser".into(),
                            "on".into(),
                            "--eq".into(),
                            "on".into(),
                            "--eq-band1".into(),
                            "250,-3,0.7".into(),
                            "--eq-band2".into(),
                            "3000,3,0.8".into(),
                            "--eq-band3".into(),
                            "8000,2,0.7".into(),
                        ])
                        .unwrap();
                        let mut dsp =
                            AudioProcessor::<f32>::new(rate, frames, DspSettings::from(&options))
                                .unwrap();
                        let mut output = vec![0.0; frames];
                        let start = Instant::now();
                        #[cfg(windows)]
                        let cycle_start = thread_cycles();
                        for chunk in input.chunks(frames) {
                            let block = Instant::now();
                            black_box(
                                dsp.process_block(
                                    black_box(chunk),
                                    black_box(&mut output[..chunk.len()]),
                                )
                                .unwrap(),
                            );
                            worst = worst.max(block.elapsed().as_secs_f64() * 1000.0);
                        }
                        if trial != 0 {
                            times.push(start.elapsed().as_secs_f64() * 1e9 / input.len() as f64);
                            #[cfg(windows)]
                            cycles
                                .push((thread_cycles() - cycle_start) as f64 / input.len() as f64);
                        }
                    }
                    times.sort_by(f64::total_cmp);
                    let median = times[3];
                    total += median;
                    cases += 1;
                    #[cfg(windows)]
                    {
                        cycles.sort_by(f64::total_cmp);
                        total_cycles += cycles[3];
                        print!("cycles/sample={:.2}, ", cycles[3]);
                    }
                    println!("rate={rate}, frames={frames}, pitch={pitch:+}, formant={shift:+}: median={median:.2} ns/sample, wall/audio={:.2}%, worst={worst:.3} ms, block budget={:.3} ms", median * rate as f64 / 1e7, frames as f64 / rate as f64 * 1000.0);
                }
            }
        }
    }
    assert!(cases > 0, "benchmark filters selected no supported cases");
    println!(
        "equal-weight mean of {cases} case medians: {:.2} ns/sample",
        total / cases as f64
    );
    #[cfg(windows)]
    println!(
        "equal-weight mean cycles/sample: {:.2}",
        total_cycles / cases as f64
    );
}
