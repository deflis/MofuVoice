use mofu_voice::{
    pitch::{PitchAlgorithm, PitchSettings, SpectralWindows},
    RATE,
};
use std::{f64::consts::TAU, hint::black_box, time::Instant};

fn main() {
    // Keep timing out of the parallel quality tests. This harness-free benchmark
    // runs one configuration at a time, after quality validation in CI.
    let psola_input: Vec<f32> = (0..RATE)
        .map(|i| {
            let phase = TAU * 113.0 * i as f64 / RATE as f64;
            (0.1 * phase.cos() + 0.02 * (3.0 * phase).cos()) as f32
        })
        .collect();
    let spectral_input: Vec<f32> = (0..RATE)
        .map(|i| (0.2 * (TAU * 203.0 * i as f64 / RATE as f64).sin()) as f32)
        .collect();
    for analysis in [1024, 2048, 4096, 8192] {
        for synthesis in [256, 512, 1024, 2048] {
            if synthesis > analysis {
                continue;
            }
            let windows = SpectralWindows {
                analysis,
                synthesis,
            };
            for algorithm in [
                PitchAlgorithm::TdPsola,
                PitchAlgorithm::TdPsolaResampled,
                PitchAlgorithm::FdPsola,
            ]
            .into_iter()
            .chain(PitchAlgorithm::SPECTRAL)
            {
                let spectral = PitchAlgorithm::SPECTRAL.contains(&algorithm);
                let input = if spectral {
                    &spectral_input
                } else {
                    &psola_input
                };
                for semitones in [-12.0, -3.0, 3.0, 7.0, 12.0] {
                    let settings = PitchSettings {
                        windows,
                        psola_windows: windows,
                        psola_pitch_estimator: Default::default(),
                        algorithm,
                        semitones,
                        preserve_formants: !spectral && algorithm != PitchAlgorithm::FdPsola,
                        formant_shift: 0.0,
                    };
                    let mut times = [0.0; 3];
                    for time in &mut times {
                        let mut dsp = settings.build().unwrap();
                        let mut output = vec![0.0; input.len()];
                        let start = Instant::now();
                        dsp.process_block(black_box(input), black_box(&mut output));
                        *time = start.elapsed().as_secs_f64();
                        black_box(output);
                    }
                    times.sort_by(f64::total_cmp);
                    println!(
                        "analysis={analysis}, synthesis={synthesis}, algorithm={}, pitch={semitones:+}: median={:.3} ms/audio second, {:.1} ns/sample",
                        algorithm.name(),
                        times[1] * 1000.0,
                        times[1] * 1e9 / input.len() as f64,
                    );
                }
            }
        }
    }
}
