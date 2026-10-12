use rust_low_latency_voice_changer::{pitch::PitchShifter, RATE};
use std::{hint::black_box, time::Instant};

fn main() {
    let input: Vec<f32> = (0..RATE as usize * 4)
        .map(|i| {
            let phase = std::f64::consts::TAU * 113.0 * i as f64 / RATE as f64;
            (0.12 * phase.sin() + 0.05 * (phase * 3.0).sin() + 0.03 * (phase * 7.0).sin()) as f32
        })
        .collect();
    for pitch in [-12.0, -3.0, 0.0, 3.0, 7.0, 12.0] {
        for preserve in [false, true] {
            let mut times = Vec::new();
            let mut worst_block = 0.0_f64;
            for _ in 0..7 {
                let mut dsp = PitchShifter::new(pitch, preserve, 0.0).unwrap();
                let mut output = [0.0; 512];
                let start = Instant::now();
                for chunk in input.chunks(512) {
                    let block = Instant::now();
                    black_box(
                        dsp.process_block(black_box(chunk), black_box(&mut output[..chunk.len()])),
                    );
                    worst_block = worst_block.max(block.elapsed().as_secs_f64() * 1000.0);
                }
                times.push(start.elapsed().as_secs_f64());
            }
            times.sort_by(f64::total_cmp);
            println!("pitch={pitch:+}, preserve={preserve}: median={:.1} ns/sample, CPU/audio={:.2}%, worst observed 512-frame block={worst_block:.3} ms (deadline 10.667 ms)", times[3] * 1e9 / input.len() as f64, times[3] / 4.0 * 100.0);
        }
    }
}
