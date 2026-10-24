use mofu_voice::pitch::{PitchAlgorithm, PitchSettings, SpectralWindows};

// Keep the reference cascade at the same precision as the internal connection.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Exact(f64);
impl mofu_voice::dsp::Sample for Exact {
    fn normalized(self) -> f64 {
        self.0
    }
    fn from_normalized(value: f64) -> Self {
        Self(value)
    }
}

#[test]
fn psola_and_following_vocoder_match_independently_configured_stages() {
    let input: Vec<Exact> = (0..48000)
        .map(|i| {
            let phase = std::f64::consts::TAU * 113.0 * i as f64 / 48000.0;
            Exact(0.08 * phase.cos() + 0.03 * (3.0 * phase).cos() + 0.015 * (8.0 * phase).cos())
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
            let mut hybrid = vec![Exact::default(); input.len()];
            settings.build().unwrap().process_block(&input, &mut hybrid);
            let mut intermediate = vec![Exact::default(); input.len()];
            base.build()
                .unwrap()
                .process_block(&input, &mut intermediate);
            let mut expected = vec![Exact::default(); input.len()];
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
                .map(|(a, b)| (a.0 - b.0).abs())
                .fold(0.0_f64, f64::max);
            assert!(error < 1e-12, "{settings:?}: cascade error={error}");

            let mut reference = vec![Exact::default(); input.len()];
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
