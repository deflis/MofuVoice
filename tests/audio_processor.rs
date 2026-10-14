use rust_low_latency_voice_changer::{
    audio::{AudioProcessor, DspSettings},
    cli::Options,
    gate::NoiseGate,
    pitch::{PitchShifter, Sample},
    RATE,
};

fn settings(args: &[&str]) -> DspSettings {
    DspSettings::from(&Options::parse(args.iter().map(|s| s.to_string())).unwrap())
}

fn bypass<T: Sample + std::fmt::Debug>(input: &[T]) -> Vec<T> {
    let mut processor = AudioProcessor::new(RATE, 2, settings(&[])).unwrap();
    let mut output = vec![T::default(); input.len()];
    for (a, b) in input.chunks(2).zip(output.chunks_mut(2)) {
        processor.process_block(a, b).unwrap();
    }
    assert_eq!(input, output);
    output
}

#[test]
fn neutral_bypass_preserves_supported_sample_formats() {
    bypass(&[i16::MIN, -1, 0, 1, i16::MAX]);
    bypass(&[i32::MIN, -1, 0, 1, i32::MAX]);
    let input = [1.0_f32, -0.0, 0.25, -0.5, f32::from_bits(1)];
    let output = bypass(&input);
    assert_eq!(
        input.map(f32::to_bits).to_vec(),
        output.iter().map(|s| s.to_bits()).collect::<Vec<_>>()
    );
}

#[test]
fn gate_only_silences_invalid_samples_without_losing_input_telemetry() {
    let input = [0.25_f32, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0];
    let mut processor = AudioProcessor::new(RATE, 5, settings(&["--gate", "on"])).unwrap();
    let mut output = [0.0; 5];
    assert_eq!(processor.process_block(&input, &mut output).unwrap(), 4);
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert_eq!(&output[1..], &[0.0; 4]);
    assert!(output[0] > 0.0);
}

#[test]
fn auto_gate_rejects_quiet_clicks_across_callback_boundaries() {
    let mut input = vec![0.0_f32; RATE as usize * 3];
    let amplitude = 10.0_f32.powf(-52.0 / 20.0);
    for start in (RATE as usize * 2..input.len()).step_by(RATE as usize / 10) {
        for i in 0..144 {
            input[start + i] = if i % 2 == 0 { amplitude } else { -amplitude };
        }
    }
    let run = |frames| {
        let mut processor =
            AudioProcessor::<f32>::new(RATE, frames, settings(&["--gate", "on"])).unwrap();
        let mut output = vec![0.0; input.len()];
        for (source, dest) in input.chunks(frames).zip(output.chunks_mut(frames)) {
            processor.process_block(source, dest).unwrap();
        }
        output
    };
    let expected = run(512);
    assert!(expected.iter().all(|x| x.abs() < amplitude * 0.1));
    for frames in [1, 63, 128, 777] {
        assert_eq!(run(frames), expected, "frames={frames}");
    }
}

#[test]
fn pipeline_matches_rumble_gate_pitch_eq_level_deesser_order_across_variable_blocks() {
    let args = [
        "--deesser",
        "on",
        "--rumble-filter",
        "on",
        "--pitch",
        "3",
        "--gate",
        "on",
        "--gate-threshold",
        "-40",
        "--auto-level",
        "on",
        "--eq",
        "on",
        "--eq-band1",
        "250,-3,0.7",
        "--eq-band2",
        "3000,2,0.8",
    ];
    let dsp = settings(&args);
    let mut rumble = rust_low_latency_voice_changer::rumble::RumbleFilter::new(dsp.rumble_filter);
    let mut gate: NoiseGate = dsp.gate.build().unwrap();
    let mut pitch =
        PitchShifter::new(dsp.semitones, dsp.preserve_formants, dsp.formant_shift).unwrap();
    let mut eq = dsp.eq.build().unwrap();
    let mut level = dsp.level.build().unwrap();
    let mut deesser = rust_low_latency_voice_changer::deesser::DeEsser::new(dsp.deesser);
    let mut processor = AudioProcessor::<f32>::new(RATE, 512, dsp).unwrap();
    let input: Vec<_> = (0..12000).map(|i| (i as f32 * 0.03).sin() * 0.1).collect();
    let mut gated = vec![0.0; input.len()];
    let mut expected = vec![0.0; input.len()];
    let mut filtered = input.clone();
    rumble.process_in_place(&mut filtered);
    gate.process_block(&filtered, &mut gated);
    pitch.process_block(&gated, &mut expected);
    eq.process_in_place(&mut expected);
    level.process_in_place(&mut expected);
    deesser.process_in_place(&mut expected);
    let mut actual = vec![0.0; input.len()];
    let mut offset = 0;
    for frames in [1, 64, 511, 128, 3, 512].into_iter().cycle() {
        let end = (offset + frames).min(input.len());
        processor
            .process_block(&input[offset..end], &mut actual[offset..end])
            .unwrap();
        offset = end;
        if offset == input.len() {
            break;
        }
    }
    assert_eq!(
        expected.iter().map(|s| s.to_bits()).collect::<Vec<_>>(),
        actual.iter().map(|s| s.to_bits()).collect::<Vec<_>>()
    );
}

#[test]
fn invalid_configuration_and_buffers_fail_without_advancing_state() {
    for (rate, frames) in [(44100, 512), (RATE, 0), (RATE, 8193)] {
        assert!(AudioProcessor::<f32>::new(rate, frames, settings(&[])).is_err());
    }
    let mut processor = AudioProcessor::<f32>::new(RATE, 64, settings(&["--pitch", "3"])).unwrap();
    let mut fresh = AudioProcessor::<f32>::new(RATE, 64, settings(&["--pitch", "3"])).unwrap();
    let mut wrong = [1.0; 65];
    assert!(processor.process_block(&[0.1; 65], &mut wrong).is_err());
    assert_eq!(wrong, [0.0; 65]);
    assert!(processor.process_block(&[0.1; 32], &mut wrong).is_err());
    let (mut actual, mut expected) = ([0.0; 64], [0.0; 64]);
    for _ in 0..100 {
        processor.process_block(&[0.1; 64], &mut actual).unwrap();
        fresh.process_block(&[0.1; 64], &mut expected).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn rumble_only_filters_before_neutral_pitch_without_gate_or_level() {
    let input: Vec<f32> = (0..48000)
        .map(|i| (0.2 * (std::f64::consts::TAU * 20.0 * i as f64 / RATE as f64).sin()) as f32)
        .collect();
    let mut expected = input.clone();
    rust_low_latency_voice_changer::rumble::RumbleFilter::new(true).process_in_place(&mut expected);
    let mut processor =
        AudioProcessor::<f32>::new(RATE, 512, settings(&["--rumble-filter", "on"])).unwrap();
    let mut actual = vec![0.0; input.len()];
    for (source, dest) in input.chunks(511).zip(actual.chunks_mut(511)) {
        processor.process_block(source, dest).unwrap();
    }
    assert_eq!(actual, expected);
    assert!(actual[24000..].iter().all(|x| x.abs() < 0.013));
}

#[test]
fn nonzero_input_count_survives_integer_dc_removal() {
    fn check<T: Sample>() {
        for gate in ["off", "on"] {
            let mut processor = AudioProcessor::<T>::new(
                RATE,
                512,
                settings(&["--rumble-filter", "on", "--gate", gate]),
            )
            .unwrap();
            let dc = T::from_normalized(0.1);
            let mut output = [T::default(); 512];
            // DC settles below one LSB, but every input sample remains nonzero.
            for frames in [1, 63, 511, 512].into_iter().cycle().take(200) {
                assert_eq!(
                    processor
                        .process_block(&[dc; 512][..frames], &mut output[..frames])
                        .unwrap(),
                    frames as u64
                );
            }
            assert!(output.iter().all(|x| *x == T::default()));
            // Filter ringing is output activity, not device input activity.
            assert_eq!(
                processor
                    .process_block(&[T::default(); 512], &mut output)
                    .unwrap(),
                0
            );
            assert!(output.iter().any(|x| *x != T::default()));
        }
    }
    check::<i16>();
    check::<i32>();
}

#[test]
fn deesser_only_attenuates_highs_through_shared_backend_pipeline() {
    let input: Vec<f32> = (0..48000)
        .map(|i| (0.4 * (std::f64::consts::TAU * 8000.0 * i as f64 / RATE as f64).sin()) as f32)
        .collect();
    let mut processor =
        AudioProcessor::<f32>::new(RATE, 512, settings(&["--deesser", "on"])).unwrap();
    let mut actual = vec![0.0; input.len()];
    for (source, dest) in input.chunks(63).zip(actual.chunks_mut(63)) {
        assert_eq!(
            processor.process_block(source, dest).unwrap(),
            source.iter().filter(|x| **x != 0.0).count() as u64
        );
    }
    let power = |x: &[f32]| x.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
    assert!(power(&actual[24000..]) < power(&input[24000..]) * 0.5);
}

#[test]
fn deesser_mode_only_changes_presentation_and_custom_parameters_reach_audio() {
    let common = [
        "--deesser",
        "on",
        "--deesser-frequency",
        "6500",
        "--deesser-threshold",
        "-30",
        "--deesser-range",
        "9",
    ];
    let input: Vec<f32> = (0..48000)
        .map(|i| (0.4 * (std::f64::consts::TAU * 8000.0 * i as f64 / RATE as f64).sin()) as f32)
        .collect();
    let mut reference = input.clone();
    let parameters = settings(&common).deesser_parameters;
    rust_low_latency_voice_changer::deesser::DeEsser::with_parameters(true, parameters)
        .unwrap()
        .process_in_place(&mut reference);
    for mode in ["simple", "pro"] {
        let mut args = common.to_vec();
        args.extend(["--deesser-mode", mode]);
        let mut processor = AudioProcessor::<f32>::new(RATE, 512, settings(&args)).unwrap();
        let mut output = vec![0.0; input.len()];
        for (source, dest) in input.chunks(127).zip(output.chunks_mut(127)) {
            processor.process_block(source, dest).unwrap();
        }
        assert_eq!(output, reference);
    }
}
