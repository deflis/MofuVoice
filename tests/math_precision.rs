use mofu_voice::pitch::{MathPrecision, PitchAlgorithm, PitchSettings};

#[test]
fn precision_modes_preserve_bypass_blocks_and_pitch_transitions() {
    let input: Vec<f64> = (0..24000)
        .map(|i| 0.2 * (std::f64::consts::TAU * 203.0 * i as f64 / 48000.0).sin())
        .collect();
    for algorithm in [PitchAlgorithm::PhaseLocked, PitchAlgorithm::PvsolaSync] {
        for math_precision in [MathPrecision::Standard, MathPrecision::Accurate] {
            let settings = PitchSettings {
                algorithm,
                math_precision,
                ..Default::default()
            };
            let mut bypass = settings.build().unwrap();
            let mut output = vec![0.0; input.len()];
            bypass.process_block(&input, &mut output);
            for (a, b) in input.iter().zip(&output) {
                assert_eq!(a.to_bits(), b.to_bits());
            }
            let settings = PitchSettings {
                semitones: 7.0,
                formant_shift: 3.0,
                ..settings
            };
            let mut whole = settings.build().unwrap();
            let mut chunked = settings.build().unwrap();
            for pitch in [7.0, -3.0, 12.0, 0.0] {
                whole.set_semitones(pitch).unwrap();
                chunked.set_semitones(pitch).unwrap();
                whole.process_block(&input, &mut output);
                let mut split = vec![0.0; input.len()];
                for (a, b) in input.chunks(73).zip(split.chunks_mut(73)) {
                    chunked.process_block(a, b);
                }
                assert_eq!(output, split);
                assert!(output.iter().all(|x| x.is_finite() && x.abs() < 2.0));
            }
            assert_eq!(
                settings.latency(),
                PitchSettings {
                    math_precision: MathPrecision::Standard,
                    ..settings
                }
                .latency()
            );
        }
    }
}

#[test]
fn cli_validates_precision() {
    use mofu_voice::cli::Options;
    for mode in ["standard", "accurate"] {
        let options = Options::parse(["--math-precision", mode].map(String::from)).unwrap();
        assert_eq!(options.pitch.math_precision.name(), mode);
    }
    for args in [
        vec!["--math-precision"],
        vec!["--math-precision", "unknown"],
        vec![
            "--math-precision",
            "accurate",
            "--math-precision",
            "standard",
        ],
        vec!["--math-precision", "accurate", "--inspect"],
    ] {
        assert!(Options::parse(args.into_iter().map(String::from)).is_err());
    }
}

#[test]
fn accurate_matches_standard_signal_quality() {
    for algorithm in [
        PitchAlgorithm::PhaseLocked,
        PitchAlgorithm::PvsolaSync,
        PitchAlgorithm::Pvsola,
    ] {
        for semitones in [-12.0, -3.0, 3.0, 7.0, 12.0] {
            let settings = PitchSettings {
                algorithm,
                semitones,
                formant_shift: 3.0,
                ..Default::default()
            };
            let input: Vec<_> = (0..48000)
                .map(|i| {
                    let phase = std::f64::consts::TAU * 203.0 * i as f64 / 48000.0;
                    0.2 * phase.sin() + 0.05 * (3.0 * phase).sin()
                })
                .collect();
            let mut reference = vec![0.0; input.len()];
            let mut actual = reference.clone();
            settings
                .build()
                .unwrap()
                .process_block(&input, &mut reference);
            PitchSettings {
                math_precision: MathPrecision::Accurate,
                ..settings
            }
            .build()
            .unwrap()
            .process_block(&input, &mut actual);
            let rms = (actual
                .iter()
                .zip(&reference)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                / input.len() as f64)
                .sqrt();
            assert!(rms < 1e-8, "{algorithm:?} {semitones}: {rms}");
        }
    }
}

#[test]
#[ignore = "run separately after quality tests to measure CPU cost"]
fn precision_cost_per_pitch() {
    use std::{hint::black_box, time::Instant};
    let input: Vec<_> = (0..48000)
        .map(|i| 0.2 * (std::f64::consts::TAU * 203.0 * i as f64 / 48000.0).sin())
        .collect();
    for math_precision in [MathPrecision::Standard, MathPrecision::Accurate] {
        for semitones in [-12.0, -3.0, 3.0, 7.0, 12.0] {
            let settings = PitchSettings {
                math_precision,
                semitones,
                formant_shift: 3.0,
                ..Default::default()
            };
            let mut dsp = settings.build().unwrap();
            let mut output = vec![0.0; input.len()];
            dsp.process_block(&input, &mut output);
            let mut times = Vec::new();
            for _ in 0..5 {
                let start = Instant::now();
                dsp.process_block(black_box(&input), black_box(&mut output));
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{math_precision:?}, pitch={semitones:+}: {:.3} ms/audio second",
                times[2]
            );
        }
    }
}
