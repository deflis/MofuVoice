use rust_low_latency_voice_changer::pitch::{PitchAlgorithm, PitchSettings, SpectralWindows};
use std::f64::consts::TAU;

const ALGORITHMS: [PitchAlgorithm; 3] = [
    PitchAlgorithm::TdPsola,
    PitchAlgorithm::TdPsolaResampled,
    PitchAlgorithm::FdPsola,
];

fn power(signal: &[f32], frequency: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &value) in signal.iter().enumerate() {
        let phase = TAU * frequency * i as f64 / 48000.0;
        let value = value as f64 * (0.5 - 0.5 * (TAU * i as f64 / signal.len() as f64).cos());
        re += value * phase.cos();
        im += value * phase.sin();
    }
    re * re + im * im
}

fn configured_psola_windows_keep_pitch_and_measure_processing_per_setting(
    windows: SpectralWindows,
) {
    for algorithm in ALGORITHMS {
        for semitones in [-12.0, -3.0, 3.0, 7.0, 12.0] {
            let settings = PitchSettings {
                windows,
                psola_windows: windows,
                algorithm,
                semitones,
                preserve_formants: algorithm != PitchAlgorithm::FdPsola,
                ..Default::default()
            };
            let input: Vec<f32> = (0..48000)
                .map(|i| {
                    let phase = TAU * 113.0 * i as f64 / 48000.0;
                    (0.1 * phase.cos() + 0.02 * (3.0 * phase).cos()) as f32
                })
                .collect();
            let mut output = vec![0.0; input.len()];
            let mut dsp = settings.build().unwrap();
            let start = std::time::Instant::now();
            dsp.process_block(&input, &mut output);
            eprintln!(
                "{windows:?} {algorithm:?} pitch={semitones}: {:?}/audio second",
                start.elapsed()
            );
            let signal = &output[24000..];
            let expected = 113.0 * 2.0_f64.powf(semitones / 12.0);
            let peak = (-8..=8)
                .map(|i| expected + i as f64 * 0.5)
                .map(|frequency| (frequency, power(signal, frequency)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0;
            assert!(
                (peak - expected).abs() <= 1.0,
                "{settings:?}: peak={peak}, expected={expected}"
            );
            assert!(
                power(signal, expected) > 1.0,
                "{settings:?}: lost voiced fundamental"
            );
            assert!(
                output.iter().all(|x| x.is_finite() && x.abs() < 0.9),
                "{settings:?}"
            );
        }
    }
}

fn configured_psola_windows_preserve_and_shift_vowel_resonance(windows: SpectralWindows) {
    let input: Vec<f32> = (0..48000)
        .map(|i| {
            let phase = TAU * 100.0 * i as f64 / 48000.0;
            (1..=40)
                .map(|h| {
                    let frequency = h as f64 * 100.0;
                    let amplitude = 0.003 / h as f64
                        + 0.015 * (-0.5 * ((frequency - 1500.0) / 230.0).powi(2)).exp();
                    amplitude * (phase * h as f64).cos()
                })
                .sum::<f64>() as f32
        })
        .collect();
    for algorithm in ALGORITHMS {
        for formant_shift in [-4.0, 0.0, 4.0] {
            let settings = PitchSettings {
                windows,
                psola_windows: windows,
                algorithm,
                semitones: 7.0,
                formant_shift,
                ..Default::default()
            };
            let mut output = vec![0.0; input.len()];
            settings.build().unwrap().process_block(&input, &mut output);
            let signal = &output[24000..];
            let fundamental = 100.0 * 2.0_f64.powf(7.0 / 12.0);
            let expected = 1500.0 * 2.0_f64.powf(formant_shift / 12.0);
            let peak = (4..=20)
                .map(|h| h as f64 * fundamental)
                .map(|frequency| (frequency, power(signal, frequency)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0;
            // A short FD synthesis window broadens the reconstructed spectrum.
            // Keep the existing 280 Hz bound except when two synthesis bins are wider.
            let tolerance = if algorithm == PitchAlgorithm::FdPsola {
                280.0_f64.max(2.0 * 48000.0 / windows.synthesis as f64)
            } else {
                280.0
            };
            assert!(
                (peak - expected).abs() < tolerance,
                "{settings:?}: resonance={peak}, expected={expected}"
            );
            assert!(
                output.iter().all(|x| x.is_finite() && x.abs() < 0.9),
                "{settings:?}"
            );
        }
    }
}

fn configured_psola_unvoiced_delay_matches_settings_after_ring_wrap(windows: SpectralWindows) {
    let mut seed = 17_u32;
    let input: Vec<f32> = (0..72000)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (0.1 * (seed as f64 / u32::MAX as f64 - 0.5)) as f32
        })
        .collect();
    for algorithm in ALGORITHMS {
        let settings = PitchSettings {
            windows,
            psola_windows: windows,
            algorithm,
            semitones: 7.0,
            ..Default::default()
        };
        let mut output = vec![0.0; input.len()];
        settings.build().unwrap().process_block(&input, &mut output);
        assert!(output[..settings.latency()].iter().all(|x| *x == 0.0));
        assert_eq!(
            &output[settings.latency()..],
            &input[..input.len() - settings.latency()],
            "{settings:?}"
        );
    }
}

#[macro_use]
#[path = "common/window_cases.rs"]
mod window_cases;

window_cases!(
    configured_psola_windows_keep_pitch_and_measure_processing_per_setting,
    configured_psola_windows_preserve_and_shift_vowel_resonance,
    configured_psola_unvoiced_delay_matches_settings_after_ring_wrap,
);
