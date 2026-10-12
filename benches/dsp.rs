#[path = "../tests/reference/mod.rs"]
mod reference;
use rust_low_latency_voice_changer::{pitch::PitchShifter, RATE};
use std::{hint::black_box, time::Instant};

const FRAMES: usize = 512;
const BLOCKS: usize = 1024;
const ROUNDS: usize = 11;

fn measure(mut process: impl FnMut(&[f32], &mut [f32]), input: &[f32]) -> f64 {
    let mut output = [0.0; FRAMES];
    for block in input.as_chunks::<FRAMES>().0.iter().take(32) {
        process(black_box(block), black_box(&mut output));
    }
    let start = Instant::now();
    for block in input.as_chunks::<FRAMES>().0 {
        process(black_box(block), black_box(&mut output));
    }
    black_box(output);
    start.elapsed().as_secs_f64() * 1e9 / input.len() as f64
}

fn main() {
    let input = reference::signal(FRAMES * BLOCKS);
    println!("Paired release DSP benchmark: rate={RATE}, frames={FRAMES}, blocks={BLOCKS}, rounds={ROUNDS}; ns/sample; median paired time reduction");
    let mut failures = Vec::new();
    for pitch in [-12.0, -3.0, 0.0, 3.0, 12.0] {
        let mut old_times = Vec::new();
        let mut new_times = Vec::new();
        let mut reductions = Vec::new();
        for round in 0..ROUNDS {
            let old = || {
                let mut dsp = reference::pitch::PitchShifter::new(pitch, true).unwrap();
                measure(
                    |a, b| {
                        black_box(dsp.process_block(a, b));
                    },
                    &input,
                )
            };
            let new = || {
                let mut dsp = PitchShifter::new(pitch, true).unwrap();
                measure(
                    |a, b| {
                        black_box(dsp.process_block(a, b));
                    },
                    &input,
                )
            };
            let (before, after) = if round % 2 == 0 {
                (old(), new())
            } else {
                let after = new();
                (old(), after)
            };
            old_times.push(before);
            new_times.push(after);
            reductions.push(100.0 * (1.0 - after / before));
        }
        old_times.sort_by(f64::total_cmp);
        new_times.sort_by(f64::total_cmp);
        reductions.sort_by(f64::total_cmp);
        let reduction = reductions[ROUNDS / 2];
        println!(
            "pitch={pitch:+}, preserve: before={:.2}, after={:.2}, reduction={reduction:.2}%",
            old_times[ROUNDS / 2],
            new_times[ROUNDS / 2]
        );
        if reduction < 10.0 {
            failures.push(pitch);
        }
    }
    assert!(
        failures.is_empty(),
        "10% processing-time reduction missed for pitches {failures:?}"
    );
}
