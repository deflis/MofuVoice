use rust_low_latency_voice_changer::{
    level::LevelSettings,
    pitch::{PitchAlgorithm, PitchShifter},
    RATE,
};
use std::{hint::black_box, time::Instant};

// CPU time excludes scheduler pauses in shared Linux runners. Keep worst-block
// measurements on the wall clock, because audio deadlines include those pauses.
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
fn thread_cpu_time() -> std::time::Duration {
    #[repr(C)]
    struct Timespec {
        seconds: std::ffi::c_long,
        nanoseconds: std::ffi::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, value: *mut Timespec) -> i32;
    }
    let mut value = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: Linux CLOCK_THREAD_CPUTIME_ID is 3; value is writable timespec storage.
    assert_eq!(unsafe { clock_gettime(3, &mut value) }, 0);
    std::time::Duration::new(value.seconds as u64, value.nanoseconds as u32)
}

struct BenchmarkStart {
    wall: Instant,
    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    cpu: Option<std::time::Duration>,
}
impl BenchmarkStart {
    fn now() -> Self {
        Self {
            wall: Instant::now(),
            #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
            cpu: (std::env::var("DSP_BENCH_CLOCK").as_deref() == Ok("cpu")).then(thread_cpu_time),
        }
    }
    fn elapsed(&self) -> std::time::Duration {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        if let Some(cpu) = self.cpu {
            return thread_cpu_time() - cpu;
        }
        self.wall.elapsed()
    }
}

fn main() {
    eprintln!("DSP_BENCH_CLOCK=cpu selects thread CPU time on 64-bit Linux; default is wall time. Worst blocks always use wall time.");
    let input: Vec<f32> = (0..RATE as usize * 4)
        .map(|i| {
            let phase = std::f64::consts::TAU * 113.0 * i as f64 / RATE as f64;
            (0.12 * phase.sin() + 0.05 * (phase * 3.0).sin() + 0.03 * (phase * 7.0).sin()) as f32
        })
        .collect();
    for algorithm in PitchAlgorithm::ALL {
        for pitch in [-12.0, -3.0, 0.0, 3.0, 7.0, 12.0] {
            for preserve in [false, true] {
                for auto_level in [false, true] {
                    let mut times = Vec::new();
                    let mut worst_block = 0.0_f64;
                    for _ in 0..7 {
                        let mut dsp = PitchShifter::new(pitch, preserve, 0.0, algorithm).unwrap();
                        let mut level = LevelSettings {
                            enabled: auto_level,
                            compressor_enabled: auto_level,
                            ..Default::default()
                        }
                        .build()
                        .unwrap();
                        let mut output = [0.0; 512];
                        let start = BenchmarkStart::now();
                        for chunk in input.chunks(512) {
                            let block = Instant::now();
                            black_box(dsp.process_block(
                                black_box(chunk),
                                black_box(&mut output[..chunk.len()]),
                            ));
                            level.process_in_place(black_box(&mut output[..chunk.len()]));
                            worst_block = worst_block.max(block.elapsed().as_secs_f64() * 1000.0);
                        }
                        times.push(start.elapsed().as_secs_f64());
                    }
                    times.sort_by(f64::total_cmp);
                    println!("algorithm={}, pitch={pitch:+}, preserve={preserve}, auto_level={auto_level}: median={:.1} ns/sample, CPU/audio={:.2}%, worst observed 512-frame block={worst_block:.3} ms (deadline 10.667 ms)", algorithm.name(), times[3] * 1e9 / input.len() as f64, times[3] / 4.0 * 100.0);
                }
            }
        }
    }
}
