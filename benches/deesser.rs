use mofu_voice::{
    deesser::{DeEsser, DeEsserParameters},
    RATE,
};
use std::{hint::black_box, time::Instant};
fn main() {
    for (name, hz, amplitude, range_db) in [
        ("vowel", 250.0, 0.4, 6.0),
        ("quiet highs", 8000.0, 0.01, 6.0),
        ("sibilant", 8000.0, 0.4, 6.0),
        ("transition", 8000.0, 0.08, 6.0),
        ("zero range", 8000.0, 0.4, 0.0),
    ] {
        let input: Vec<f32> = (0..RATE as usize * 4)
            .map(|i| {
                let envelope = if name == "transition" {
                    1.0 + 0.95 * (i as f64 / RATE as f64 * 7.0).sin()
                } else {
                    1.0
                };
                (amplitude * envelope * (std::f64::consts::TAU * hz * i as f64 / RATE as f64).sin())
                    as f32
            })
            .collect();
        let mut times = Vec::new();
        for _ in 0..31 {
            let parameters = DeEsserParameters {
                range_db,
                ..DeEsserParameters::default()
            };
            let mut dsp = DeEsser::with_parameters(true, parameters).unwrap();
            let mut output = [0.0_f32; 64];
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
