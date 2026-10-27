use mofu_voice::{
    audio::{AudioProcessor, DspSettings},
    cli::Options,
    dsp::Sample,
    gate::NoiseGate,
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
fn level_and_compressor_switches_reach_the_audio_pipeline_in_all_formats() {
    fn check<T: Sample + std::fmt::Debug>() {
        let input: Vec<T> = (0..RATE as usize)
            .map(|i| T::from_normalized(0.5 * (i as f64 * 0.03).sin()))
            .collect();
        for level in ["off", "on"] {
            for compressor in ["off", "on"] {
                let args = ["--auto-level", level, "--compressor", compressor];
                let mut expected = input.clone();
                settings(&args)
                    .level
                    .build()
                    .unwrap()
                    .process_in_place(&mut expected);
                let mut processor = AudioProcessor::<T>::new(RATE, 63, settings(&args)).unwrap();
                let mut actual = vec![T::default(); input.len()];
                for (source, dest) in input.chunks(63).zip(actual.chunks_mut(63)) {
                    processor.process_block(source, dest).unwrap();
                }
                assert_eq!(actual, expected, "level={level}, compressor={compressor}");
            }
        }
    }
    check::<f32>();
    check::<i16>();
    check::<i32>();
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
fn pipeline_matches_rumble_gate_deesser_pitch_eq_level_order_across_variable_blocks() {
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
    let mut rumble = mofu_voice::rumble::RumbleFilter::new(dsp.rumble_filter);
    let mut gate: NoiseGate = dsp.gate.build().unwrap();
    let mut pitch = dsp.pitch.build().unwrap();
    let mut eq = dsp.eq.build().unwrap();
    let mut level = dsp.level.build().unwrap();
    let mut deesser = mofu_voice::deesser::DeEsser::new(dsp.deesser);
    let mut processor = AudioProcessor::<f32>::new(RATE, 512, dsp).unwrap();
    let input: Vec<_> = (0..12000).map(|i| (i as f32 * 0.03).sin() * 0.1).collect();
    let mut gated = vec![0.0_f64; input.len()];
    let mut expected = vec![0.0; input.len()];
    let mut filtered: Vec<f64> = input.iter().map(|s| *s as f64).collect();
    rumble.process_in_place(&mut filtered);
    gate.process_block(&filtered, &mut gated);
    deesser.process_in_place(&mut gated);
    pitch.process_block(&gated, &mut expected);
    eq.process_in_place(&mut expected);
    level.process_in_place(&mut expected);
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
        expected
            .iter()
            .map(|s| (*s as f32).to_bits())
            .collect::<Vec<_>>(),
        actual.iter().map(|s| s.to_bits()).collect::<Vec<_>>()
    );
}

#[test]
fn invalid_configuration_and_buffers_fail_without_advancing_state() {
    for (rate, frames) in [(32000, 512), (0, 512), (RATE, 0), (RATE, 8193)] {
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
    mofu_voice::rumble::RumbleFilter::new(true).process_in_place(&mut expected);
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
    mofu_voice::deesser::DeEsser::with_parameters(true, parameters)
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

#[test]
fn selected_rates_preserve_neutral_bits_and_wet_block_continuity() {
    for rate in mofu_voice::SAMPLE_RATES {
        let input = [f32::from_bits(0x7fc01234), -0.0, f32::from_bits(1), 0.25];
        let mut output = [0.0; 4];
        let mut processor = AudioProcessor::new(rate, 4, settings(&[])).unwrap();
        processor.process_block(&input, &mut output).unwrap();
        assert_eq!(input.map(f32::to_bits), output.map(f32::to_bits));
        let mut integers = AudioProcessor::<i32>::new(rate, 4, settings(&[])).unwrap();
        let input = [i32::MIN, -1, 0, i32::MAX];
        let mut output = [0; 4];
        integers.process_block(&input, &mut output).unwrap();
        assert_eq!(input, output);
        let mut shorts = AudioProcessor::<i16>::new(rate, 4, settings(&[])).unwrap();
        let input = [i16::MIN, -1, 0, i16::MAX];
        let mut output = [0; 4];
        shorts.process_block(&input, &mut output).unwrap();
        assert_eq!(input, output);

        let args = [
            "--pitch",
            "3",
            "--formant-shift",
            "2",
            "--rumble-filter",
            "on",
            "--gate",
            "on",
            "--eq",
            "on",
            "--eq-band2",
            "3000,3,0.8",
            "--auto-level",
            "on",
            "--deesser",
            "on",
        ];
        let input: Vec<f32> = (0..8192)
            .map(|i| (0.2 * (std::f64::consts::TAU * 300.0 * i as f64 / rate as f64).sin()) as f32)
            .collect();
        let mut whole = AudioProcessor::new(rate, input.len(), settings(&args)).unwrap();
        let mut split = AudioProcessor::new(rate, 63, settings(&args)).unwrap();
        let mut expected = vec![0.0; input.len()];
        let mut actual = expected.clone();
        whole.process_block(&input, &mut expected).unwrap();
        for (input, output) in input.chunks(63).zip(actual.chunks_mut(63)) {
            split.process_block(input, output).unwrap();
        }
        assert_eq!(expected, actual, "rate={rate}");
        assert!(actual.iter().all(|s| s.is_finite()));
        assert!(actual[4096..].iter().any(|s| s.abs() > 0.001));
    }
}

#[test]
fn selected_rates_keep_filter_frequencies_and_envelope_times() {
    fn render(rate: u32, input: &[f32], args: &[&str]) -> Vec<f32> {
        let mut processor = AudioProcessor::new(rate, 512, settings(args)).unwrap();
        let mut output = vec![0.0; input.len()];
        for (input, output) in input.chunks(512).zip(output.chunks_mut(512)) {
            processor.process_block(input, output).unwrap();
        }
        output
    }
    fn gain_db(input: &[f32], output: &[f32]) -> f64 {
        let power = |values: &[f32]| {
            values[values.len() / 2..]
                .iter()
                .map(|s| (*s as f64).powi(2))
                .sum::<f64>()
        };
        10.0 * (power(output) / power(input)).log10()
    }
    let mut gate_levels = Vec::new();
    let mut auto_levels = Vec::new();
    for rate in mofu_voice::SAMPLE_RATES {
        let tone = |hz: f64| -> Vec<f32> {
            (0..rate)
                .map(|i| (0.1 * (std::f64::consts::TAU * hz * i as f64 / rate as f64).sin()) as f32)
                .collect()
        };
        let input = tone(80.0);
        let output = render(rate, &input, &["--rumble-filter", "on"]);
        assert!(
            (gain_db(&input, &output) + 3.0103).abs() < 0.02,
            "rumble at {rate}"
        );
        let input = tone(3000.0);
        let output = render(rate, &input, &["--eq", "on", "--eq-band2", "3000,-6,1"]);
        assert!(
            (gain_db(&input, &output) + 6.0).abs() < 0.02,
            "EQ at {rate}"
        );
        let input = tone(8000.0);
        let output = render(
            rate,
            &input,
            &["--deesser", "on", "--deesser-threshold", "-36"],
        );
        assert!(gain_db(&input, &output) < -2.0, "de-esser at {rate}");
        // Compare at equal elapsed time, not at equal sample indices.
        let input = vec![0.01; (rate / 10) as usize];
        let output = render(
            rate,
            &input,
            &[
                "--gate",
                "on",
                "--gate-threshold",
                "-20",
                "--gate-release",
                "80",
            ],
        );
        gate_levels.push(*output.last().unwrap());
        let output = render(
            rate,
            &input,
            &["--auto-level", "on", "--level-response", "100"],
        );
        auto_levels.push(*output.last().unwrap());
    }
    for values in [gate_levels, auto_levels] {
        let min = values.iter().copied().fold(f32::INFINITY, f32::min);
        let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max - min < 0.0001,
            "envelope changes across rates: {values:?}"
        );
    }
}

#[test]
fn mixed_formats_quantize_only_after_the_complete_chain() {
    let args = [
        "--rumble-filter",
        "on",
        "--eq",
        "on",
        "--eq-band2",
        "3000,3,0.8",
        "--pitch",
        "3",
    ];
    let input: Vec<i32> = (0..8192)
        .map(|i| ((i as f64 * 0.07).sin() * 123456789.0) as i32)
        .collect();
    let normalized: Vec<f64> = input.iter().map(|s| s.normalized()).collect();
    let mut reference = AudioProcessor::<f64>::new(RATE, 8192, settings(&args)).unwrap();
    let mut expected = vec![0.0_f64; input.len()];
    reference.process_block(&normalized, &mut expected).unwrap();
    let mut processor = AudioProcessor::<i32>::new(RATE, 127, settings(&args)).unwrap();
    let mut actual = vec![0_i16; input.len()];
    for (source, dest) in input.chunks(127).zip(actual.chunks_mut(127)) {
        processor.process_into(source, dest).unwrap();
    }
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|s| i16::from_normalized(*s))
            .collect::<Vec<_>>()
    );
    let mut processor = AudioProcessor::<i32>::new(RATE, 127, settings(&args)).unwrap();
    let mut actual = vec![0.0_f64; input.len()];
    for (source, dest) in input.chunks(127).zip(actual.chunks_mut(127)) {
        processor.process_into(source, dest).unwrap();
    }
    assert_eq!(actual, expected);
}

#[test]
fn f64_bypass_preserves_all_bits_and_mixed_invalid_blocks_leave_state_unchanged() {
    let input = [
        f64::from_bits(0x7ff0000000000123),
        -0.0,
        f64::from_bits(1),
        f64::MAX,
    ];
    let mut p = AudioProcessor::<f64>::new(RATE, 4, settings(&[])).unwrap();
    let mut out = [0.0_f64; 4];
    p.process_block(&input, &mut out).unwrap();
    assert_eq!(input.map(f64::to_bits), out.map(f64::to_bits));
    let mut wrong = [1_i16; 5];
    assert!(p.process_into(&input, &mut wrong).is_err());
    assert_eq!(wrong, [0; 5]);
    p.process_block(&input, &mut out).unwrap();
    assert_eq!(input.map(f64::to_bits), out.map(f64::to_bits));
}
