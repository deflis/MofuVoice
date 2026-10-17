use rust_low_latency_voice_changer::{
    pitch::{pitch_ratio, PitchAlgorithm, PitchShifter, LATENCY, WINDOW},
    RATE,
};

fn tone(frequency: f64, count: usize) -> Vec<f32> {
    tone_at_rate(frequency, count, RATE)
}
fn tone_at_rate(frequency: f64, count: usize, rate: u32) -> Vec<f32> {
    (0..count)
        .map(|i| (0.2 * (std::f64::consts::TAU * frequency * i as f64 / rate as f64).sin()) as f32)
        .collect()
}

#[test]
fn pitch_lobes_above_nyquist_are_discarded_without_aliasing() {
    for algorithm in PitchAlgorithm::SPECTRAL {
        for pitch in [7.0, 12.0] {
            for preserve in [false, true] {
                let input = tone(18_000.0, 24_000);
                let mut output = vec![0.0; input.len()];
                PitchShifter::new(pitch, preserve, 0.0, algorithm)
                    .unwrap()
                    .process_block(&input, &mut output);
                let settled = &output[8192..];
                let rms = (settled.iter().map(|&x| (x as f64).powi(2)).sum::<f64>()
                    / settled.len() as f64)
                    .sqrt();
                assert!(rms < 0.002, "pitch={pitch}, preserve={preserve}, RMS={rms}");
            }
        }
    }
}

#[test]
fn moving_pitch_keeps_amplitude_and_phase_continuous() {
    for algorithm in PitchAlgorithm::SPECTRAL {
        let count = 96_000;
        let mut phase = 0.0_f64;
        let mut phases = Vec::with_capacity(count);
        let input: Vec<f32> = (0..count)
            .map(|i| {
                let frequency = 80.0 + 140.0 * i as f64 / count as f64;
                phase += std::f64::consts::TAU * frequency / RATE as f64;
                phases.push(phase);
                (0.2 * phase.sin()) as f32
            })
            .collect();
        for pitch in [3.0, 7.0, 12.0] {
            let mut output = vec![0.0; input.len()];
            PitchShifter::new(pitch, false, 0.0, algorithm)
                .unwrap()
                .process_block(&input, &mut output);
            let levels: Vec<f64> = output[8192..count - LATENCY]
                .as_chunks::<1024>()
                .0
                .iter()
                .map(|chunk| {
                    (chunk.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / chunk.len() as f64)
                        .sqrt()
                })
                .collect();
            let mean = levels.iter().sum::<f64>() / levels.len() as f64;
            let variation = (levels.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                / levels.len() as f64)
                .sqrt()
                / mean;
            let ratio = pitch_ratio(pitch).unwrap();
            // Demodulate against the exact shifted chirp. Its residual phase should
            // stay continuous as the analysis peak crosses FFT bin boundaries.
            let residual: Vec<f64> = (8192..count - LATENCY - 1024)
                .step_by(1024)
                .map(|start| {
                    let (mut re, mut im) = (0.0, 0.0);
                    for i in 0..1024 {
                        let phase = phases[start + i - LATENCY] * ratio;
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
                    ((p[1] - p[0] + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI)
                        .abs()
                })
                .fold(0.0_f64, f64::max);
            eprintln!("moving pitch={pitch}, mean RMS={mean}, relative RMS deviation={variation}, largest residual phase change={maximum}");
            assert!(mean > 0.1, "pitch={pitch}, RMS={mean}");
            // Asymmetric short synthesis windows trade some chirp stability for lower delay.
            assert!(variation < 0.1, "pitch={pitch}, RMS deviation={variation}");
            assert!(maximum < 0.35, "pitch={pitch}, phase jump={maximum}");
        }
    }
}

fn power(signal: &[f32], frequency: f64) -> f64 {
    power_at_rate(signal, frequency, RATE)
}
fn power_at_rate(signal: &[f32], frequency: f64, rate: u32) -> f64 {
    let angle = std::f64::consts::TAU * frequency / rate as f64;
    let coefficient = 2.0 * angle.cos();
    let (mut a, mut b) = (0.0, 0.0);
    for (i, &sample) in signal.iter().enumerate() {
        let window = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / signal.len() as f64).cos();
        let next = sample as f64 * window + coefficient * a - b;
        b = a;
        a = next;
    }
    (a * a + b * b - coefficient * a * b).max(0.0)
}

#[test]
fn low_voice_tones_have_accurate_pitch_without_subharmonic_energy() {
    for algorithm in PitchAlgorithm::SPECTRAL {
        for rate in rust_low_latency_voice_changer::SAMPLE_RATES {
            low_voice_quality_at_rate(rate, algorithm);
        }
    }
}
fn low_voice_quality_at_rate(rate: u32, algorithm: PitchAlgorithm) {
    for frequency in [80.0, 100.0, 120.0, 160.0, 203.0] {
        for pitch in [3.0, 7.0, 12.0, -3.0, -12.0] {
            let input = tone_at_rate(frequency, rate as usize, rate);
            let mut output = vec![0.0; input.len()];
            PitchShifter::new(pitch, false, 0.0, algorithm)
                .unwrap()
                .process_block(&input, &mut output);
            let expected = frequency * pitch_ratio(pitch).unwrap();
            let signal = &output[8192..];
            let peak = ((expected - 15.0).ceil() as usize..=(expected + 15.0).floor() as usize)
                .max_by(|&a, &b| {
                    power_at_rate(signal, a as f64, rate)
                        .total_cmp(&power_at_rate(signal, b as f64, rate))
                })
                .unwrap() as f64;
            assert!(
                (peak - expected).abs() < 1.5,
                "f={frequency}, pitch={pitch}, peak={peak}, expected={expected}"
            );
            let target = power_at_rate(signal, expected, rate);
            let low: f64 = (20..(expected * 0.7) as usize)
                .step_by(2)
                .map(|f| power_at_rate(signal, f as f64, rate))
                .sum();
            assert!(
                low < target * 0.001,
                "f={frequency}, pitch={pitch}: low={low}, target={target}"
            );
        }
    }
}

#[test]
fn preservation_and_shift_move_a_broadband_resonance_independently_of_pitch() {
    for algorithm in PitchAlgorithm::SPECTRAL {
        for rate in rust_low_latency_voice_changer::SAMPLE_RATES {
            formant_quality_at_rate(rate, algorithm);
        }
    }
}
fn formant_quality_at_rate(rate: u32, algorithm: PitchAlgorithm) {
    let mut seed = 1_u32;
    let radius = 0.98_f64;
    let coefficient = 2.0 * radius * (std::f64::consts::TAU * 1500.0 / rate as f64).cos();
    let (mut a, mut b) = (0.0, 0.0);
    let input: Vec<f32> = (0..65_536)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = 0.001 * (seed as f64 / u32::MAX as f64 - 0.5) + coefficient * a
                - radius * radius * b;
            b = a;
            a = x;
            x as f32
        })
        .collect();
    for (pitch, preserve, shift) in [
        (7.0, false, 0.0),
        (7.0, true, 0.0),
        (0.0, true, 3.0),
        (7.0, true, 3.0),
    ] {
        let mut output = vec![0.0; input.len()];
        let options = rust_low_latency_voice_changer::cli::Options::parse([
            "--pitch-algorithm".into(),
            algorithm.name().into(),
            "--pitch".into(),
            pitch.to_string(),
            "--formant-shift".into(),
            shift.to_string(),
            "--formant".into(),
            if preserve { "preserve" } else { "off" }.into(),
        ])
        .unwrap();
        let mut dsp = rust_low_latency_voice_changer::audio::AudioProcessor::new(
            rate,
            512,
            rust_low_latency_voice_changer::audio::DspSettings::from(&options),
        )
        .unwrap();
        for (a, b) in input.chunks(512).zip(output.chunks_mut(512)) {
            dsp.process_block(a, b).unwrap();
        }
        let peak = (1000..=2600)
            .step_by(25)
            .max_by(|&a, &b| {
                let energy = |frequency| {
                    output[8192..]
                        .as_chunks::<2048>()
                        .0
                        .iter()
                        .map(|block| power_at_rate(block, frequency as f64, rate))
                        .sum::<f64>()
                };
                energy(a).total_cmp(&energy(b))
            })
            .unwrap() as f64;
        let expected = 1500.0 * pitch_ratio(if preserve { shift } else { pitch }).unwrap();
        assert!(
            (peak - expected).abs() < 200.0,
            "pitch={pitch}, preserve={preserve}, shift={shift}: peak={peak}, expected={expected}"
        );
        assert!(output.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    }
}

#[test]
fn low_components_are_suppressed_with_formants_enabled() {
    for algorithm in PitchAlgorithm::SPECTRAL {
        let input = tone(100.0, 48_000);
        for shift in [0.0, 3.0] {
            let mut output = vec![0.0; input.len()];
            PitchShifter::new(7.0, true, shift, algorithm)
                .unwrap()
                .process_block(&input, &mut output);
            let expected = 100.0 * pitch_ratio(7.0).unwrap();
            let low: f64 = (20..100)
                .step_by(2)
                .map(|f| power(&output[8192..], f as f64))
                .sum();
            assert!(low < power(&output[8192..], expected) * 0.001);
        }
    }
}

#[test]
fn block_boundaries_formats_bypass_and_retargeting() {
    for algorithm in PitchAlgorithm::SPECTRAL {
        let input = tone(113.0, 12_000);
        for (pitch, preserve, shift) in [(7.0, false, 0.0), (7.0, true, 3.0), (0.0, true, 3.0)] {
            let mut whole = vec![0.0; input.len()];
            PitchShifter::new(pitch, preserve, shift, algorithm)
                .unwrap()
                .process_block(&input, &mut whole);
            for size in [1, 7, 64, 512, 4096] {
                let mut dsp = PitchShifter::new(pitch, preserve, shift, algorithm).unwrap();
                let mut split = vec![0.0; input.len()];
                for (a, b) in input.chunks(size).zip(split.chunks_mut(size)) {
                    dsp.process_block(a, b);
                }
                assert_eq!(whole, split);
            }
        }
        let floats = [f32::NAN, f32::INFINITY, -0.0, f32::from_bits(1), 0.25];
        let mut dsp = PitchShifter::new(0.0, true, 0.0, algorithm).unwrap();
        let mut output = [0.0; 5];
        dsp.process_block(&floats, &mut output);
        assert_eq!(floats.map(f32::to_bits), output.map(f32::to_bits));
        let integers = [i32::MIN, -1, 0, 1, i32::MAX];
        let mut output = [0; 5];
        dsp.process_block(&integers, &mut output);
        assert_eq!(integers, output);
        let shorts = [i16::MIN, -1, 0, 1, i16::MAX];
        let mut output = [0; 5];
        dsp.process_block(&shorts, &mut output);
        assert_eq!(shorts, output);
        let mut output = vec![0.0; input.len()];
        for pitch in [7.0, -12.0, 12.0, 0.0] {
            dsp.set_semitones(pitch).unwrap();
            dsp.process_block(&input, &mut output);
            assert!(output.iter().all(|x| x.is_finite()));
        }
        assert_eq!(input[1024..], output[1024..]);
    }
}

#[test]
fn silence_and_impulse_delay_and_invalid_settings() {
    for algorithm in PitchAlgorithm::SPECTRAL {
        for preserve in [false, true] {
            let mut dsp = PitchShifter::new(7.0, preserve, 0.0, algorithm).unwrap();
            let mut output = vec![1.0; 8192];
            dsp.process_block(&vec![0.0_f32; output.len()], &mut output);
            assert!(output.iter().all(|&x| x == 0.0));
            let mut input = vec![0.0; 8192];
            input[0] = f32::NAN;
            input[1] = f32::INFINITY;
            dsp.process_block(&input, &mut output);
            assert!(output.iter().all(|&x| x == 0.0));
            input[0] = 0.0;
            input[1] = 0.0;
            input[4096] = 0.2;
            dsp.process_block(&input, &mut output);
            assert!(output.iter().all(|x| x.is_finite()));
            // Analysis history is WINDOW samples; synthesis then spans LATENCY samples.
            assert!(output[4096 + WINDOW + LATENCY..]
                .iter()
                .all(|x| x.abs() < 1e-9));
        }
        assert!(PitchShifter::new(0.0, false, 1.0, algorithm).is_err());
        for value in [f64::NAN, f64::INFINITY, -12.1, 12.1] {
            assert!(PitchShifter::new(value, true, 0.0, algorithm).is_err());
            assert!(PitchShifter::new(0.0, true, value, algorithm).is_err());
        }
    }
}
