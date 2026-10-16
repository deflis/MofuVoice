use rust_low_latency_voice_changer::{
    pitch::{pitch_ratio, PitchAlgorithm, PitchShifter},
    SAMPLE_RATES,
};

fn shifter(pitch: f64) -> PitchShifter {
    PitchShifter::new(pitch, true, 0.0, PitchAlgorithm::TdPsola).unwrap()
}

#[test]
fn td_psola_keeps_voiced_glides_phase_continuous() {
    let rate = 48000_u32;
    let mut phase = 0.0;
    let phases: Vec<_> = (0..96000)
        .map(|i| {
            phase += std::f64::consts::TAU * (100.0 + 100.0 * i as f64 / 96000.0) / rate as f64;
            phase
        })
        .collect();
    let input: Vec<f32> = phases
        .iter()
        .map(|&p| (0.1 * p.cos() + 0.04 * (3.0 * p).cos()) as f32)
        .collect();
    for pitch in [3.0, 7.0, 12.0] {
        let ratio = pitch_ratio(pitch).unwrap();
        let mut output = vec![0.0; input.len()];
        shifter(pitch).process_block(&input, &mut output);
        let residual: Vec<_> = (16384..input.len() - 1024)
            .step_by(1024)
            .map(|start| {
                let (mut re, mut im) = (0.0, 0.0);
                for i in 0..1024 {
                    let phase = phases[start + i - PitchAlgorithm::TdPsola.latency()] * ratio;
                    let window = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / 1024.0).cos();
                    re += output[start + i] as f64 * window * phase.cos();
                    im += output[start + i] as f64 * window * phase.sin();
                }
                im.atan2(re)
            })
            .collect();
        let maximum = residual
            .windows(2)
            .map(|p| {
                let delta = p[1] - p[0];
                delta.sin().atan2(delta.cos()).abs()
            })
            .fold(0.0_f64, f64::max);
        eprintln!("pitch={pitch}: TD-PSOLA glide residual phase jump={maximum}");
        assert!(maximum < 0.5, "pitch={pitch}, phase jump={maximum}");
    }
}

fn power(signal: &[f32], frequency: f64, rate: u32) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &value) in signal.iter().enumerate() {
        let angle = std::f64::consts::TAU * frequency * i as f64 / rate as f64;
        let window = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / signal.len() as f64).cos();
        re += value as f64 * window * angle.cos();
        im += value as f64 * window * angle.sin();
    }
    re * re + im * im
}

#[test]
fn td_psola_preserves_a_vowel_resonance_when_changing_pitch() {
    for rate in SAMPLE_RATES {
        let input: Vec<f32> = (0..rate as usize)
            .map(|i| {
                let phase = std::f64::consts::TAU * 120.0 * i as f64 / rate as f64;
                (1..=24)
                    .map(|h| {
                        let frequency = h as f64 * 120.0;
                        let amplitude = 0.005 / h as f64
                            + 0.02 * (-0.5 * ((frequency - 1200.0) / 180.0).powi(2)).exp();
                        amplitude * (phase * h as f64).cos()
                    })
                    .sum::<f64>() as f32
            })
            .collect();
        for pitch in [-12.0, -3.0, 3.0, 7.0, 12.0] {
            let mut output = vec![0.0; input.len()];
            shifter(pitch).process_block(&input, &mut output);
            let fundamental = 120.0 * pitch_ratio(pitch).unwrap();
            let peak = (1..=50)
                .map(|h| fundamental * h as f64)
                .filter(|f| (500.0..2700.0).contains(f))
                .max_by(|&a, &b| {
                    power(&output[16384..], a, rate).total_cmp(&power(&output[16384..], b, rate))
                })
                .unwrap();
            eprintln!("rate={rate}, pitch={pitch}, preserved resonance peak={peak}");
            assert!((peak - 1200.0).abs() < 240.0);
        }
    }
}

#[test]
fn td_psola_handles_voicing_boundaries_retargeting_and_flush_without_stale_grains() {
    let mut seed = 19_u32;
    let input: Vec<f32> = (0..48000)
        .map(|i| {
            let phase = std::f64::consts::TAU * 113.0 * i as f64 / 48000.0;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            match i / 12000 {
                0 => 0.0,
                1 | 3 => (0.1 * phase.cos() + 0.03 * (3.0 * phase).cos()) as f32,
                _ => (0.03 * (seed as f64 / u32::MAX as f64 - 0.5)) as f32,
            }
        })
        .collect();
    let mut dsp = shifter(7.0);
    let mut output = vec![0.0; input.len()];
    for pitch in [7.0, -12.0, 12.0] {
        dsp.set_semitones(pitch).unwrap();
        dsp.process_block(&input, &mut output);
        assert!(output.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    }
    let silence = vec![0.0_f32; 20000];
    let mut output = silence.clone();
    dsp.process_block(&silence, &mut output);
    assert!(output[12000..].iter().all(|&x| x == 0.0));
    dsp.set_semitones(0.0).unwrap();
    let mut output = vec![0.0; input.len()];
    dsp.process_block(&input, &mut output);
    assert_eq!(&input[1024..], &output[1024..]);
}

#[test]
fn td_psola_generates_the_requested_fundamental_at_both_device_rates() {
    for rate in SAMPLE_RATES {
        for frequency in [80.0, 113.0, 203.0] {
            for pitch in [-12.0, -3.0, 3.0, 7.0, 12.0] {
                let input: Vec<f32> = (0..rate as usize)
                    .map(|i| {
                        let phase = std::f64::consts::TAU * frequency * i as f64 / rate as f64;
                        (0.1 * phase.cos() + 0.04 * (3.0 * phase).cos()) as f32
                    })
                    .collect();
                let mut output = vec![0.0; input.len()];
                shifter(pitch).process_block(&input, &mut output);
                let expected = frequency * pitch_ratio(pitch).unwrap();
                let signal = &output[16384..];
                let peak = (-12..=12)
                    .map(|i| expected + i as f64 * 0.25)
                    .max_by(|&a, &b| power(signal, a, rate).total_cmp(&power(signal, b, rate)))
                    .unwrap();
                eprintln!(
                    "rate={rate}, f={frequency}, pitch={pitch}: expected={expected}, peak={peak}"
                );
                assert!((peak - expected).abs() <= 0.5);
                assert!(power(signal, expected, rate) > 1.0);
                assert!(output.iter().all(|x| x.is_finite() && x.abs() < 0.9));
            }
        }
    }
}

#[test]
fn td_psola_is_block_independent_and_preserves_neutral_sample_bits() {
    let input: Vec<f32> = (0..20000)
        .map(|i| {
            let phase = std::f64::consts::TAU * 137.0 * i as f64 / 48000.0;
            (0.1 * phase.cos() + 0.03 * (4.0 * phase).sin()) as f32
        })
        .collect();
    let mut whole = vec![0.0; input.len()];
    shifter(7.0).process_block(&input, &mut whole);
    for block in [1, 7, 127, 512, 4096] {
        let mut dsp = shifter(7.0);
        let mut split = vec![0.0; input.len()];
        for (a, b) in input.chunks(block).zip(split.chunks_mut(block)) {
            dsp.process_block(a, b);
        }
        assert_eq!(whole, split);
    }
    let floats = [f32::NAN, f32::INFINITY, -0.0, f32::from_bits(1), 0.25];
    let mut output = [0.0; 5];
    shifter(0.0).process_block(&floats, &mut output);
    assert_eq!(floats.map(f32::to_bits), output.map(f32::to_bits));
    let integers = [i32::MIN, -1, 0, 1, i32::MAX];
    let mut output = [0; 5];
    shifter(0.0).process_block(&integers, &mut output);
    assert_eq!(integers, output);
    let shorts = [i16::MIN, -1, 0, 1, i16::MAX];
    let mut output = [0; 5];
    shifter(0.0).process_block(&shorts, &mut output);
    assert_eq!(shorts, output);
}

#[test]
fn td_psola_formant_settings_and_pipeline_latency_are_validated() {
    use rust_low_latency_voice_changer::cli::Options;
    for (preserve, shift) in [(false, 0.0), (false, 3.0), (false, -3.0)] {
        assert!(PitchShifter::new(7.0, preserve, shift, PitchAlgorithm::TdPsola).is_err());
    }
    let options =
        Options::parse(["--pitch-algorithm", "td-psola", "--formant-shift", "2"].map(String::from))
            .unwrap();
    assert_eq!(options.pitch.latency(), 4608);
    assert_eq!(
        rust_low_latency_voice_changer::pitch::PitchSettings {
            formant_shift: 0.0,
            ..options.pitch
        }
        .latency(),
        4608
    );
    assert!(Options::parse(
        ["--pitch-algorithm", "td-psola", "--formant", "off"].map(String::from)
    )
    .is_err());
    assert_eq!(PitchAlgorithm::TdPsola.latency(), 4608);
    assert_eq!(PitchAlgorithm::PhaseLocked.latency(), 512);
}

#[test]
fn resampled_grains_keep_td_pitch_and_move_vowel_resonance() {
    for rate in SAMPLE_RATES {
        let input: Vec<f32> = (0..rate as usize)
            .map(|i| {
                let phase = std::f64::consts::TAU * 100.0 * i as f64 / rate as f64;
                (1..=60)
                    .map(|h| {
                        let frequency = h as f64 * 100.0;
                        let amplitude = 0.003 / h as f64
                            + 0.015 * (-0.5 * ((frequency - 1500.0) / 230.0).powi(2)).exp();
                        amplitude * (phase * h as f64).cos()
                    })
                    .sum::<f64>() as f32
            })
            .collect();
        for pitch in [-3.0, 7.0, 12.0] {
            for shift in [-4.0, 2.53, 6.0] {
                let mut dsp =
                    PitchShifter::new(pitch, true, shift, PitchAlgorithm::TdPsola).unwrap();
                let mut output = vec![0.0; input.len()];
                dsp.process_block(&input, &mut output);
                let signal = &output[16384..];
                let fundamental = 100.0 * pitch_ratio(pitch).unwrap();
                let expected_resonance = 1500.0 * pitch_ratio(shift).unwrap();
                let peak = (1..=50)
                    .map(|h| h as f64 * fundamental)
                    .filter(|f| (600.0..3500.0).contains(f))
                    .max_by(|&a, &b| power(signal, a, rate).total_cmp(&power(signal, b, rate)))
                    .unwrap();
                eprintln!("resampled TD rate={rate}, pitch={pitch}, shift={shift}: resonance={peak}, expected={expected_resonance}");
                assert!((peak - expected_resonance).abs() < 280.0);
                // A harmonic near the formant stays on the TD-PSOLA frequency grid.
                let measured = (-8..=8)
                    .map(|i| peak + i as f64 * 0.25)
                    .max_by(|&a, &b| power(signal, a, rate).total_cmp(&power(signal, b, rate)))
                    .unwrap();
                assert!((measured - peak).abs() <= 0.5);
                assert!(output.iter().all(|x| x.is_finite() && x.abs() < 0.9));
            }
        }
    }
}

#[test]
fn resampled_grains_are_block_independent_during_pitch_changes() {
    let input: Vec<f32> = (0..24000)
        .map(|i| {
            let p = std::f64::consts::TAU * 113.0 * i as f64 / 48000.0;
            (0.1 * p.cos() + 0.03 * (3.0 * p).cos()) as f32
        })
        .collect();
    let mut reference = None;
    for block in [24000, 1, 127, 512] {
        let mut dsp = PitchShifter::new(7.0, true, 2.53, PitchAlgorithm::TdPsola).unwrap();
        for pitch in [7.0, 0.0, -3.0] {
            dsp.set_semitones(pitch).unwrap();
            let mut output = vec![0.0; input.len()];
            for (a, b) in input.chunks(block).zip(output.chunks_mut(block)) {
                dsp.process_block(a, b);
            }
            if block == 24000 {
                reference.get_or_insert_with(Vec::new).push(output);
            } else {
                let index = match pitch {
                    7.0 => 0,
                    0.0 => 1,
                    _ => 2,
                };
                assert_eq!(reference.as_ref().unwrap()[index], output);
            }
        }
    }
}

#[test]
fn resampling_moves_unvoiced_resonance_without_changing_duration() {
    for rate in SAMPLE_RATES {
        let mut seed = 91_u32;
        let radius = 0.94;
        let coefficient = 2.0 * radius * (std::f64::consts::TAU * 1800.0 / rate as f64).cos();
        let (mut a, mut b) = (0.0, 0.0);
        let input: Vec<f32> = (0..rate as usize)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let x = 0.005 * (seed as f64 / u32::MAX as f64 - 0.5) + coefficient * a
                    - radius * radius * b;
                b = a;
                a = x;
                x as f32
            })
            .collect();
        for shift in [-12.0, -4.0, 2.53, 12.0] {
            let mut output = vec![0.0; input.len()];
            PitchShifter::new(0.0, true, shift, PitchAlgorithm::TdPsola)
                .unwrap()
                .process_block(&input, &mut output);
            let expected = 1800.0 * pitch_ratio(shift).unwrap();
            // Average short spectra: a single noise periodogram has random
            // narrow maxima unrelated to the resonance envelope.
            let averaged_power = |frequency| {
                output[16384..]
                    .windows(4096)
                    .step_by(2048)
                    .map(|block| power(block, frequency, rate))
                    .sum::<f64>()
            };
            let peak = (20..=180)
                .map(|i| i as f64 * 25.0)
                .max_by(|&a, &b| averaged_power(a).total_cmp(&averaged_power(b)))
                .unwrap();
            eprintln!("unvoiced rate={rate}, shift={shift}, resonance={peak}, expected={expected}");
            assert!((peak - expected).abs() < 250.0);
            assert!(output.iter().all(|x| x.is_finite() && x.abs() < 1.0));
        }
    }
}

#[test]
fn resampled_grains_keep_fixed_delay_and_flush_without_drifting() {
    let impulse = 16384;
    for shift in [-12.0, -4.0, 2.53, 12.0] {
        let mut input = vec![0.0_f32; 48000];
        input[impulse] = 0.25;
        let mut output = vec![0.0; input.len()];
        let mut dsp = PitchShifter::new(0.0, true, shift, PitchAlgorithm::TdPsola).unwrap();
        dsp.process_block(&input, &mut output);
        let energy: f64 = output.iter().map(|&x| (x as f64).powi(2)).sum();
        let centroid: f64 = output
            .iter()
            .enumerate()
            .map(|(i, &x)| i as f64 * (x as f64).powi(2))
            .sum::<f64>()
            / energy;
        eprintln!("formant={shift}: impulse centroid={centroid}");
        assert!((centroid - (impulse + 4608) as f64).abs() < 2.0);
        assert!(output[impulse + 4608 + 1024..].iter().all(|&x| x == 0.0));
        for pitch in [-12.0, 12.0, 0.0] {
            dsp.set_semitones(pitch).unwrap();
            let silence = vec![0.0_f32; 48000];
            dsp.process_block(&silence, &mut output);
            assert!(output.iter().all(|&x| x == 0.0));
        }
    }
}

#[test]
fn extreme_independent_ratios_remain_finite_across_many_ring_wraps() {
    let input: Vec<f32> = (0..96000)
        .map(|i| {
            let p = std::f64::consts::TAU * 83.0 * i as f64 / 48000.0;
            if i % 24000 < 16000 {
                (0.07 * p.cos() + 0.03 * (3.0 * p).cos()) as f32
            } else {
                0.0
            }
        })
        .collect();
    for shift in [-12.0, 12.0] {
        for pitch in [-12.0, 12.0] {
            let mut dsp = PitchShifter::new(pitch, true, shift, PitchAlgorithm::TdPsola).unwrap();
            let mut output = vec![0.0; input.len()];
            dsp.process_block(&input, &mut output);
            assert!(output.iter().all(|x| x.is_finite() && x.abs() < 0.9));
            assert!(output[16384..].iter().any(|x| x.abs() > 0.005));
        }
    }
}
