use mofu_voice::pitch::{PitchAlgorithm, PitchSettings, SpectralWindows};

#[test]
fn psola_and_following_vocoder_match_independently_configured_stages() {
    let input: Vec<f64> = (0..48000)
        .map(|i| {
            let phase = std::f64::consts::TAU * 113.0 * i as f64 / 48000.0;
            0.08 * phase.cos() + 0.03 * (3.0 * phase).cos() + 0.015 * (8.0 * phase).cos()
        })
        .collect();
    for algorithm in [PitchAlgorithm::TdPsola, PitchAlgorithm::FdPsola] {
        for (windows, psola_windows) in [
            (
                SpectralWindows {
                    analysis: 8192,
                    synthesis: 2048,
                },
                SpectralWindows {
                    analysis: 1024,
                    synthesis: 256,
                },
            ),
            (
                SpectralWindows {
                    analysis: 1024,
                    synthesis: 1024,
                },
                SpectralWindows {
                    analysis: 8192,
                    synthesis: 2048,
                },
            ),
        ] {
            let settings = PitchSettings {
                windows,
                psola_windows,
                algorithm,
                semitones: 7.0,
                formant_shift: 4.0,
                ..Default::default()
            };
            let base = PitchSettings {
                formant_shift: 0.0,
                ..settings
            };
            assert_eq!(settings.latency(), base.latency() + windows.synthesis);
            let mut hybrid = vec![0.0; input.len()];
            settings.build().unwrap().process_block(&input, &mut hybrid);
            let mut intermediate = vec![0.0; input.len()];
            base.build()
                .unwrap()
                .process_block(&input, &mut intermediate);
            let mut expected = vec![0.0; input.len()];
            PitchSettings {
                algorithm: PitchAlgorithm::PhaseLocked,
                semitones: 0.0,
                ..settings
            }
            .build()
            .unwrap()
            .process_block(&intermediate, &mut expected);
            let error = hybrid
                .iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max);
            assert!(error < 1e-12, "{settings:?}: cascade error={error}");

            let mut reference = vec![0.0; input.len()];
            PitchSettings {
                windows: SpectralWindows::default(),
                ..base
            }
            .build()
            .unwrap()
            .process_block(&input, &mut reference);
            assert_eq!(
                reference, intermediate,
                "unused vocoder windows changed the PSOLA output"
            );
        }
    }
}
