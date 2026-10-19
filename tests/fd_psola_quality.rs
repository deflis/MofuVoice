use rust_low_latency_voice_changer::pitch::{pitch_ratio, PitchAlgorithm, PitchShifter};
use rust_low_latency_voice_changer::SAMPLE_RATES;

fn shifter(pitch: f64, preserve_formants: bool) -> PitchShifter {
    PitchShifter::new(
        pitch,
        preserve_formants,
        0.0,
        PitchAlgorithm::FdPsola,
        rust_low_latency_voice_changer::pitch::SpectralWindows::default(),
    )
    .unwrap()
}

fn power(signal: &[f32], frequency: f64, rate: u32) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    let denominator = (signal.len() - 1).max(1) as f64;
    for (i, &value) in signal.iter().enumerate() {
        let window = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / denominator).cos();
        let phase = std::f64::consts::TAU * frequency * i as f64 / rate as f64;
        re += value as f64 * window * phase.cos();
        im += value as f64 * window * phase.sin();
    }
    re * re + im * im
}

#[test]
fn fd_psola_moves_voiced_fundamental_at_supported_rates() {
    for rate in SAMPLE_RATES {
        for frequency in [80.0, 113.0, 203.0] {
            let pitch = 7.0;
            let input: Vec<f32> = (0..rate as usize)
                .map(|i| {
                    let phase = std::f64::consts::TAU * frequency * i as f64 / rate as f64;
                    (0.12 * phase.cos() + 0.035 * (3.0 * phase).cos()) as f32
                })
                .collect();
            let mut output = vec![0.0; input.len()];
            shifter(pitch, true).process_block(&input, &mut output);
            let signal = &output[16384..];
            let expected = frequency * pitch_ratio(pitch).unwrap();
            let peak = (40..=800)
                .map(|step| step as f64 * 0.5)
                .max_by(|&a, &b| power(signal, a, rate).total_cmp(&power(signal, b, rate)))
                .unwrap();
            assert!(
                (peak - expected).abs() <= 4.0,
                "rate={rate}, input_f0={frequency}, expected={expected}, measured={peak}"
            );
            assert!(output
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 0.9));
        }
    }
}

#[test]
fn fd_psola_keeps_vowel_resonance_with_formant_preservation() {
    let rate = 48000;
    let f0 = 100.0;
    let input: Vec<f32> = (0..rate as usize)
        .map(|i| {
            let phase = std::f64::consts::TAU * f0 * i as f64 / rate as f64;
            (1..=60)
                .map(|harmonic| {
                    let frequency = harmonic as f64 * f0;
                    let envelope = 0.002 / harmonic as f64
                        + 0.018 * (-0.5 * ((frequency - 1500.0) / 220.0).powi(2)).exp();
                    envelope * (phase * harmonic as f64).cos()
                })
                .sum::<f64>() as f32
        })
        .collect();
    let mut output = vec![0.0; input.len()];
    shifter(7.0, true).process_block(&input, &mut output);
    let signal = &output[16384..];
    let shifted_f0 = f0 * pitch_ratio(7.0).unwrap();
    let peak = (1..=40)
        .map(|harmonic| harmonic as f64 * shifted_f0)
        .filter(|frequency| (700.0..2600.0).contains(frequency))
        .max_by(|&a, &b| power(signal, a, rate).total_cmp(&power(signal, b, rate)))
        .unwrap();
    assert!(
        (peak - 1500.0).abs() < 250.0,
        "preserved formant peak={peak}"
    );
}

#[test]
fn fd_psola_preserves_unvoiced_delay_and_neutral_bits() {
    let mut seed = 29_u32;
    let noise: Vec<f32> = (0..24000)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            0.05 * (seed as f64 / u32::MAX as f64 - 0.5) as f32
        })
        .collect();
    let mut output = vec![0.0; noise.len()];
    shifter(7.0, true).process_block(&noise, &mut output);
    let delay = PitchAlgorithm::FdPsola.latency();
    for i in delay..noise.len() {
        assert_eq!(output[i].to_bits(), noise[i - delay].to_bits());
    }

    let input = [f32::NAN, f32::INFINITY, -0.0, f32::from_bits(1), 0.25];
    let mut bypass = [0.0; 5];
    shifter(0.0, true).process_block(&input, &mut bypass);
    assert_eq!(input.map(f32::to_bits), bypass.map(f32::to_bits));
}

#[test]
fn fd_psola_is_independent_of_audio_block_boundaries() {
    let input: Vec<f32> = (0..24000)
        .map(|i| {
            let phase = std::f64::consts::TAU * 137.0 * i as f64 / 48000.0;
            (0.1 * phase.cos() + 0.03 * (4.0 * phase).sin()) as f32
        })
        .collect();
    let mut whole = vec![0.0; input.len()];
    shifter(7.0, true).process_block(&input, &mut whole);
    for block in [1, 7, 127, 512, 4096] {
        let mut dsp = shifter(7.0, true);
        let mut split = vec![0.0; input.len()];
        for (source, destination) in input.chunks(block).zip(split.chunks_mut(block)) {
            dsp.process_block(source, destination);
        }
        assert_eq!(whole, split, "block size={block}");
    }
}

#[test]
fn fd_psola_applies_independent_formant_shift_and_emits_audio() {
    use rust_low_latency_voice_changer::pitch::PitchSettings;
    let rate = 48000;
    let input: Vec<f32> = (0..rate)
        .map(|i| {
            let phase = std::f64::consts::TAU * 100.0 * i as f64 / rate as f64;
            (1..=40)
                .map(|h| {
                    let amplitude =
                        0.02 * (-0.5 * ((h as f64 * 100.0 - 1200.0) / 180.0).powi(2)).exp();
                    amplitude * (phase * h as f64).cos()
                })
                .sum::<f64>() as f32
        })
        .collect();
    for shift in [-4.0, 4.0] {
        let settings = PitchSettings {
            windows: Default::default(),
            algorithm: PitchAlgorithm::FdPsola,
            semitones: 0.0,
            preserve_formants: true,
            formant_shift: shift,
        };
        assert_eq!(settings.latency(), 5120);
        let mut output = vec![0.0; input.len()];
        settings.build().unwrap().process_block(&input, &mut output);
        let signal = &output[16384..];
        assert!(signal.iter().all(|x| x.is_finite()));
        let peak = (5..=20)
            .map(|h| h as f64 * 100.0)
            .max_by(|&a, &b| {
                power(signal, a, rate as u32).total_cmp(&power(signal, b, rate as u32))
            })
            .unwrap();
        assert!(
            power(signal, peak, rate as u32) > 1.0,
            "formant shift produced silence"
        );
        assert!(
            (peak - 1200.0 * pitch_ratio(shift).unwrap()).abs() < 180.0,
            "shift={shift}, peak={peak}"
        );
    }
}
