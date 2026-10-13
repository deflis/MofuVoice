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
fn pipeline_matches_rumble_gate_pitch_level_order_across_variable_blocks() {
    let args = [
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
    ];
    let dsp = settings(&args);
    let mut rumble = rust_low_latency_voice_changer::rumble::RumbleFilter::new(dsp.rumble_filter);
    let mut gate: NoiseGate = dsp.gate.build().unwrap();
    let mut pitch =
        PitchShifter::new(dsp.semitones, dsp.preserve_formants, dsp.formant_shift).unwrap();
    let mut level = dsp.level.build().unwrap();
    let mut processor = AudioProcessor::<f32>::new(RATE, 512, dsp).unwrap();
    let input: Vec<_> = (0..12000).map(|i| (i as f32 * 0.03).sin() * 0.1).collect();
    let mut gated = vec![0.0; input.len()];
    let mut expected = vec![0.0; input.len()];
    let mut filtered = input.clone();
    rumble.process_in_place(&mut filtered);
    gate.process_block(&filtered, &mut gated);
    pitch.process_block(&gated, &mut expected);
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
