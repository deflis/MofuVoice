use mofu_voice::{
    gate::{GateThreshold, NoiseGate},
    RATE,
};
use std::{hint::black_box, time::Instant};

fn main() {
    for (name, amplitude) in [("speech", 0.2), ("noise", 0.001), ("transition", 0.01)] {
        let input: Vec<f32> = (0..RATE as usize * 4)
            .map(|i| {
                let envelope = if name == "transition" {
                    10.0_f64.powf(2.0 * (i as f64 / RATE as f64).sin())
                } else {
                    1.0
                };
                (amplitude
                    * envelope
                    * (std::f64::consts::TAU * 173.0 * i as f64 / RATE as f64).sin())
                    as f32
            })
            .collect();
        for threshold in [GateThreshold::Db(-40.0), GateThreshold::Auto] {
            let mut times = Vec::new();
            let mut output = [0.0_f32; 64];
            for _ in 0..31 {
                let mut gate = NoiseGate::new(true, threshold, 5.0, 80.0).unwrap();
                let start = Instant::now();
                for chunk in input.chunks(64) {
                    gate.process_block(black_box(chunk), black_box(&mut output[..chunk.len()]));
                    black_box(&output);
                }
                times.push(start.elapsed().as_secs_f64() * 1e9 / input.len() as f64);
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{name}, {threshold:?}: {:.2} ns/sample (median of 31, 64-frame blocks)",
                times[15]
            );
        }
    }
}
