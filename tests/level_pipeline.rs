use rust_low_latency_voice_changer::{
    gate::{GateSettings, GateThreshold},
    level::LevelSettings,
    pitch::PitchShifter,
    RATE,
};

#[test]
fn gate_pitch_and_level_chain_is_independent_of_callback_boundaries() {
    let input: Vec<f32> = (0..RATE as usize)
        .map(|i| {
            let voice = 0.35 * (std::f64::consts::TAU * 150.0 * i as f64 / RATE as f64).sin();
            if (12000..18000).contains(&i) {
                0.0
            } else {
                voice as f32
            }
        })
        .collect();
    for pitch in [-12.0, 0.0, 3.0, 7.0, 12.0] {
        for formant in [0.0, 2.0] {
            let run = |block: usize| {
                let mut gate = GateSettings {
                    enabled: true,
                    threshold: GateThreshold::Db(-40.0),
                    attack_ms: 5.0,
                    release_ms: 80.0,
                }
                .build()
                .unwrap();
                let mut shifter = PitchShifter::new(
                    pitch,
                    true,
                    formant,
                    rust_low_latency_voice_changer::pitch::PitchAlgorithm::PhaseLocked,
                )
                .unwrap();
                let mut level = LevelSettings {
                    enabled: true,
                    compressor_enabled: true,
                    ..Default::default()
                }
                .build()
                .unwrap();
                let mut output = vec![0.0; input.len()];
                let mut scratch = vec![0.0; block];
                for (source, dest) in input.chunks(block).zip(output.chunks_mut(block)) {
                    gate.process_block(source, &mut scratch[..source.len()]);
                    shifter.process_block(&scratch[..source.len()], dest);
                    level.process_in_place(dest);
                }
                output
            };
            let expected = run(512);
            assert!(expected.iter().all(|x| x.is_finite() && x.abs() <= 0.892));
            assert!(expected[24000..].iter().any(|x| x.abs() > 0.01));
            for block in [1, 63, 128, 777] {
                assert_eq!(
                    run(block),
                    expected,
                    "pitch={pitch}, shift={formant}, block={block}"
                );
            }
        }
    }
}
