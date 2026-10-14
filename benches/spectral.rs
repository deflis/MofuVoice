use rust_low_latency_voice_changer::{
    level::LevelSettings,
    pitch::{PitchShifter, LATENCY},
    RATE,
};
use std::{hint::black_box, time::Instant};

// Thread CPU time distinguishes DSP work from preemption on shared Linux hosts.
#[cfg(target_os = "linux")]
fn cpu_seconds() -> f64 {
    #[repr(C)]
    struct Timespec {
        seconds: std::os::raw::c_long,
        nanos: std::os::raw::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
    }
    let mut time = Timespec {
        seconds: 0,
        nanos: 0,
    };
    // SAFETY: CLOCK_THREAD_CPUTIME_ID (3), writable correctly aligned timespec.
    assert_eq!(unsafe { clock_gettime(3, &mut time) }, 0);
    time.seconds as f64 + time.nanos as f64 * 1e-9
}
#[cfg(not(target_os = "linux"))]
fn cpu_seconds() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn main() {
    for broadband in [false, true] {
        let mut seed = 17_u32;
        let input: Vec<f32> = (0..RATE as usize)
            .map(|i| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = seed as f64 / u32::MAX as f64 - 0.5;
                let phase = std::f64::consts::TAU * 113.0 * i as f64 / RATE as f64;
                (0.12 * phase.sin()
                    + 0.05 * (3.0 * phase).sin()
                    + 0.03 * (7.0 * phase).sin()
                    + if broadband { 0.04 * noise } else { 0.0 }) as f32
            })
            .collect();
        for frames in [64, LATENCY, 512] {
            for pitch in [-12.0, -3.0, 0.0, 3.0, 7.0, 12.0] {
                for preserve in [false, true] {
                    for auto_level in [false, true] {
                        let shift = if pitch == 0.0 && preserve { 3.0 } else { 0.0 };
                        let mut times = Vec::new();
                        let mut wall_blocks = Vec::new();
                        let mut cpu_blocks = Vec::new();
                        for _ in 0..3 {
                            let mut dsp = PitchShifter::new(pitch, preserve, shift).unwrap();
                            let mut level = LevelSettings {
                                enabled: auto_level,
                                ..Default::default()
                            }
                            .build()
                            .unwrap();
                            let mut output = vec![0.0; frames];
                            // Touch code/data and settle the estimator before measuring.
                            for chunk in input.chunks(frames) {
                                dsp.process_block(chunk, &mut output[..chunk.len()]);
                                level.process_in_place(&mut output[..chunk.len()]);
                            }
                            let start = cpu_seconds();
                            for chunk in input.chunks(frames) {
                                let wall = Instant::now();
                                let cpu = cpu_seconds();
                                black_box(dsp.process_block(
                                    black_box(chunk),
                                    black_box(&mut output[..chunk.len()]),
                                ));
                                level.process_in_place(black_box(&mut output[..chunk.len()]));
                                cpu_blocks.push((cpu_seconds() - cpu) * 1000.0);
                                wall_blocks.push(wall.elapsed().as_secs_f64() * 1000.0);
                            }
                            times.push(cpu_seconds() - start);
                        }
                        times.sort_by(f64::total_cmp);
                        wall_blocks.sort_by(f64::total_cmp);
                        cpu_blocks.sort_by(f64::total_cmp);
                        let p99 = cpu_blocks[(cpu_blocks.len() - 1) * 99 / 100];
                        let worst_cpu = cpu_blocks[cpu_blocks.len() - 1];
                        let worst_wall = wall_blocks[wall_blocks.len() - 1];
                        let delay = LATENCY as f64 * 1000.0 / RATE as f64;
                        let deadline = frames as f64 * 1000.0 / RATE as f64;
                        println!("broadband={broadband}, frames={frames}, pitch={pitch:+}, preserve={preserve}, shift={shift}, auto_level={auto_level}: median={:.1} ns/sample, CPU/audio={:.2}%, p99 CPU={p99:.3} ms, worst CPU={worst_cpu:.3} ms, worst wall={worst_wall:.3} ms, block period={deadline:.3} ms, DSP delay={delay:.3} ms", times[1]*1e9/input.len() as f64,times[1]*100.0);
                        let dsp_overruns = cpu_blocks.iter().filter(|&&ms| ms >= delay).count();
                        let callback_overruns =
                            wall_blocks.iter().filter(|&&ms| ms >= deadline).count();
                        if dsp_overruns != 0 || callback_overruns != 0 {
                            println!("  DSP CPU overruns={dsp_overruns}, callback wall overruns={callback_overruns}, measured blocks={}",cpu_blocks.len());
                        }
                        assert!(
                            p99 < delay.min(deadline),
                            "p99 processing exceeded DSP or callback deadline"
                        );
                    }
                }
            }
        }
    }
}
