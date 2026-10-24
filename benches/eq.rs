use mofu_voice::{
    eq::{Band, EqSettings},
    RATE,
};
use std::{hint::black_box, time::Instant};

fn main() {
    let input: Vec<f32> = (0..RATE as usize)
        .map(|i| {
            let phase = std::f64::consts::TAU * 113.0 * i as f64 / RATE as f64;
            (0.12 * phase.sin() + 0.05 * (phase * 3.0).sin()) as f32
        })
        .collect();

    // Compare a normal three-band EQ with a maximum-sized EQ that has thirteen
    // zero-gain bands, and with the worst case of sixteen active filters.
    for (profile, active_bands, configured_bands) in [
        ("three-active", 3, 3),
        ("one-of-sixteen", 1, 16),
        ("three-of-sixteen", 3, 16),
        ("sixteen-active", 16, 16),
    ] {
        let bands = (1..=configured_bands)
            .map(|band| {
                let (frequency, active_gain) = match band {
                    1 => (250.0, -3.0),
                    2 => (3000.0, 3.0),
                    3 => (8000.0, 2.0),
                    _ => (500.0 + band as f64 * 700.0, 1.0),
                };
                Band {
                    frequency,
                    gain_db: if band <= active_bands {
                        active_gain
                    } else {
                        0.0
                    },
                    q: 0.8,
                }
            })
            .collect();
        let settings = EqSettings {
            enabled: true,
            bands,
            output_db: 0.0,
        };

        let mut times = Vec::new();
        for _ in 0..31 {
            let mut dsp = settings.build().unwrap();
            let mut output = input.clone();
            let start = Instant::now();
            for chunk in output.chunks_mut(64) {
                dsp.process_in_place(black_box(chunk));
                black_box(chunk);
            }
            times.push(start.elapsed().as_secs_f64());
        }
        times.sort_by(f64::total_cmp);
        let seconds = times[15];
        println!(
            "{profile}: {:.2} ns/sample, CPU/audio={:.2}% (median of 31, 64-frame blocks)",
            seconds * 1e9 / input.len() as f64,
            seconds * 100.0
        );
    }
}
