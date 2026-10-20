use rust_low_latency_voice_changer::pitch::{PitchAlgorithm, PitchEstimator, PitchSettings};
use std::f64::consts::TAU;

#[test]
fn estimator_selection_preserves_pitch_mapping_and_block_boundaries() {
    for estimator in PitchEstimator::ALL {
        for algorithm in [
            PitchAlgorithm::TdPsola,
            PitchAlgorithm::TdPsolaResampled,
            PitchAlgorithm::FdPsola,
        ] {
            let settings = PitchSettings {
                algorithm,
                psola_pitch_estimator: estimator,
                semitones: 3.0,
                ..Default::default()
            };
            let input: Vec<f32> = (0..24000)
                .map(|i| (0.1 * (TAU * 113.0 * i as f64 / 48000.0).cos()) as f32)
                .collect();
            let mut whole = vec![0.0; input.len()];
            settings.build().unwrap().process_block(&input, &mut whole);
            let mut split = vec![0.0; input.len()];
            let mut dsp = settings.build().unwrap();
            for (source, target) in input.chunks(137).zip(split.chunks_mut(137)) {
                dsp.process_block(source, target);
            }
            assert_eq!(whole, split, "{algorithm:?}/{estimator:?}");
            let expected = 113.0 * 2.0f64.powf(3.0 / 12.0);
            let power = |frequency: f64| {
                let (mut re, mut im) = (0.0, 0.0);
                for (i, &sample) in whole[12000..].iter().enumerate() {
                    let phase = TAU * frequency * i as f64 / 48000.0;
                    let sample = sample as f64 * (0.5 - 0.5 * (TAU * i as f64 / 12000.0).cos());
                    re += sample * phase.cos();
                    im += sample * phase.sin();
                }
                re * re + im * im
            };
            assert!(
                power(expected) > 20.0 * power(113.0),
                "{algorithm:?}/{estimator:?}"
            );
            let mut bypass = vec![0.0; input.len()];
            PitchSettings {
                semitones: 0.0,
                ..settings
            }
            .build()
            .unwrap()
            .process_block(&input, &mut bypass);
            assert_eq!(input, bypass);
        }
    }
}
