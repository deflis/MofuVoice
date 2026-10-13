use rust_low_latency_voice_changer::{
    audio::{AudioProcessor, DspSettings},
    cli::Options,
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
    for pitch in ["-12", "-3", "0", "3", "7", "12"] {
        for enabled in ["off", "on"] {
            let mut times = Vec::new();
            let mut worst = 0.0_f64;
            for _ in 0..5 {
                let options = Options::parse(
                    [
                        "--pitch",
                        pitch,
                        "--eq",
                        enabled,
                        "--eq-band1",
                        "250,-3,0.7",
                        "--eq-band2",
                        "3000,3,0.8",
                        "--eq-band3",
                        "8000,2,0.7",
                    ]
                    .map(String::from),
                )
                .unwrap();
                let mut processor =
                    AudioProcessor::<f32>::new(RATE, 512, DspSettings::from(&options)).unwrap();
                let mut output = [0.0; 512];
                let start = Instant::now();
                for chunk in input.chunks(512) {
                    let block = Instant::now();
                    black_box(
                        processor
                            .process_block(black_box(chunk), black_box(&mut output[..chunk.len()]))
                            .unwrap(),
                    );
                    worst = worst.max(block.elapsed().as_secs_f64() * 1000.0);
                }
                times.push(start.elapsed().as_secs_f64());
            }
            times.sort_by(f64::total_cmp);
            println!("pitch={pitch}, EQ={enabled}: median={:.1} ns/sample, CPU/audio={:.2}%, worst observed 512-frame block={worst:.3} ms", times[2] * 1e9 / input.len() as f64, times[2] * 100.0);
        }
    }
}
