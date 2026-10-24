use mofu_voice::pitch::{PitchAlgorithm, PitchSettings, SpectralWindows};

fn configurations() -> impl Iterator<Item = SpectralWindows> {
    [1024, 2048, 4096, 8192].into_iter().flat_map(|analysis| {
        [256, 512, 1024, 2048]
            .into_iter()
            .filter_map(move |synthesis| {
                (synthesis <= analysis).then_some(SpectralWindows {
                    analysis,
                    synthesis,
                })
            })
    })
}

#[test]
fn windows_validate_and_report_only_the_selected_algorithms_delay() {
    for windows in configurations() {
        windows.validate().unwrap();
        for algorithm in PitchAlgorithm::ALL {
            let settings = PitchSettings {
                windows,
                psola_windows: windows,
                psola_pitch_estimator: Default::default(),
                algorithm,
                ..Default::default()
            };
            let expected = if PitchAlgorithm::SPECTRAL.contains(&algorithm) {
                windows.synthesis
            } else if algorithm == PitchAlgorithm::Pvsola {
                algorithm.latency()
            } else {
                let left = (1600.0 * 2.0
                    / (1.0 + (windows.synthesis as f64 / windows.analysis as f64).sqrt()))
                .ceil() as usize;
                (windows.analysis / 2
                    + 400
                    + 16
                    + 1600
                    + left.max(windows.synthesis / 2)
                    + 128
                    + 66)
                    .div_ceil(128)
                    * 128
            };
            assert_eq!(settings.latency(), expected);
        }
    }
    for (analysis, synthesis) in [
        (0, 512),
        (1536, 512),
        (16384, 512),
        (2048, 0),
        (2048, 128),
        (1024, 2048),
    ] {
        let settings = PitchSettings {
            windows: SpectralWindows {
                analysis,
                synthesis,
            },
            ..Default::default()
        };
        assert!(settings.build().is_err());
    }
}

fn all_windows_preserve_neutral_bits_and_block_boundaries(windows: SpectralWindows) {
    let input: Vec<f32> = (0..32768)
        .map(|i| {
            if i % 113 == 0 {
                -0.0
            } else {
                (1..=12)
                    .map(|h| (0.025 / h as f64) * (i as f64 * 0.017 * h as f64).cos())
                    .sum::<f64>() as f32
            }
        })
        .collect();
    for algorithm in PitchAlgorithm::ALL {
        let mut settings = PitchSettings {
            windows,
            psola_windows: windows,
            psola_pitch_estimator: Default::default(),
            algorithm,
            ..Default::default()
        };
        let mut output = vec![0.0; input.len()];
        settings.build().unwrap().process_block(&input, &mut output);
        assert!(input
            .iter()
            .zip(&output)
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        let integers = [i32::MIN, -123456789, 0, 123456789, i32::MAX];
        let mut integers_out = [0; 5];
        settings
            .build()
            .unwrap()
            .process_block(&integers, &mut integers_out);
        assert_eq!(integers, integers_out);
        let shorts = [i16::MIN, -123, 0, 123, i16::MAX];
        let mut shorts_out = [0; 5];
        settings
            .build()
            .unwrap()
            .process_block(&shorts, &mut shorts_out);
        assert_eq!(shorts, shorts_out);
        settings.semitones = 7.0;
        settings.formant_shift = 2.53;
        settings.build().unwrap().process_block(&input, &mut output);
        let mut chunked = vec![0.0; input.len()];
        let mut dsp = settings.build().unwrap();
        let mut position = 0;
        for size in [1, 127, 129, 511, 3, 2049].into_iter().cycle() {
            let end = (position + size).min(input.len());
            dsp.process_block(&input[position..end], &mut chunked[position..end]);
            position = end;
            if position == input.len() {
                break;
            }
        }
        assert_eq!(output, chunked, "{windows:?} {algorithm:?}");
        assert!(output.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert!(
            output[16384..].iter().any(|x| x.abs() > 0.01),
            "{windows:?} {algorithm:?}"
        );
    }
}

fn configured_windows_keep_pitch_and_phase_continuous(windows: SpectralWindows) {
    let input: Vec<f32> = (0..48000)
        .map(|i| (0.2 * (std::f64::consts::TAU * 203.0 * i as f64 / 48000.0).sin()) as f32)
        .collect();
    for algorithm in PitchAlgorithm::SPECTRAL {
        for semitones in [-12.0, -3.0, 3.0, 7.0, 12.0] {
            let settings = PitchSettings {
                windows,
                psola_windows: windows,
                psola_pitch_estimator: Default::default(),
                algorithm,
                semitones,
                preserve_formants: false,
                formant_shift: 0.0,
            };
            let mut output = vec![0.0; input.len()];
            let mut dsp = settings.build().unwrap();
            dsp.process_block(&input, &mut output);
            let signal = &output[16384..];
            let frequency = 203.0 * 2.0_f64.powf(semitones / 12.0);
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &x) in signal.iter().enumerate() {
                let angle = std::f64::consts::TAU * frequency * i as f64 / 48000.0;
                re += x as f64 * angle.cos();
                im += x as f64 * angle.sin();
            }
            let amplitude = 2.0 * re.hypot(im) / signal.len() as f64;
            let residual = signal.iter().map(|x| (*x as f64).powi(2)).sum::<f64>()
                / signal.len() as f64
                - amplitude.powi(2) / 2.0;
            assert!(
                (0.08..0.25).contains(&amplitude),
                "{windows:?} {algorithm:?} pitch={semitones}: {amplitude}"
            );
            assert!(
                residual < 0.001,
                "{windows:?} {algorithm:?} pitch={semitones}: {residual}"
            );
        }
    }
}

#[macro_use]
#[path = "common/window_cases.rs"]
mod window_cases;

window_cases!(
    all_windows_preserve_neutral_bits_and_block_boundaries,
    configured_windows_keep_pitch_and_phase_continuous,
);
