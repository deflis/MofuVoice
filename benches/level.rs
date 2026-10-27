use mofu_voice::{level::LevelSettings, RATE};
use std::{hint::black_box, time::Instant};

fn main() {
    for (name, amplitude, ratio) in [
        ("quiet", 0.03, 3.0),
        ("speech", 0.15, 3.0),
        ("loud", 0.9, 3.0),
        ("attack-release", 0.9, 3.0),
        ("ratio-1", 0.9, 1.0),
    ] {
        let input: Vec<f64> = (0..RATE as usize * 4)
            .map(|i| {
                let envelope = if name == "attack-release" && i / 12000 % 2 == 1 {
                    0.01
                } else {
                    1.0
                };
                amplitude
                    * envelope
                    * (std::f64::consts::TAU * 173.0 * i as f64 / RATE as f64).sin()
            })
            .collect();
        let mut times = Vec::new();
        for _ in 0..31 {
            let mut dsp = LevelSettings {
                enabled: true,
                compressor_enabled: true,
                ratio,
                ..Default::default()
            }
            .build()
            .unwrap();
            let mut output = [0.0_f64; 64];
            let start = Instant::now();
            for chunk in input.chunks(64) {
                output[..chunk.len()].copy_from_slice(black_box(chunk));
                dsp.process_in_place(black_box(&mut output[..chunk.len()]));
                black_box(&output);
            }
            times.push(start.elapsed().as_secs_f64() * 1e9 / input.len() as f64);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{name}: {:.2} ns/sample (median of 31, 64-frame blocks)",
            times[15]
        );
    }
}
