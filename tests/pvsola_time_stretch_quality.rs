use rust_low_latency_voice_changer::pitch::{pitch_ratio, PitchAlgorithm, PitchShifter};
use rust_low_latency_voice_changer::RATE;

fn peak_frequency(samples: &[f32], low: f64, high: f64) -> f64 {
    let mut best = (0.0, 0.0);
    let count = samples.len();
    for step in 0..=400 {
        let frequency = low + (high - low) * step as f64 / 400.0;
        let (mut re, mut im) = (0.0, 0.0);
        for (index, &sample) in samples.iter().enumerate() {
            let window = 0.5 - 0.5 * (std::f64::consts::TAU * index as f64 / count as f64).cos();
            let phase = std::f64::consts::TAU * frequency * index as f64 / RATE as f64;
            re += sample as f64 * window * phase.cos();
            im -= sample as f64 * window * phase.sin();
        }
        let magnitude = re.hypot(im);
        if magnitude > best.0 {
            best = (magnitude, frequency);
        }
    }
    best.1
}

#[test]
fn paper_pvsola_time_stretch_then_resampling_moves_pitch() {
    let pitch = 7.019550008653875;
    let ratio = pitch_ratio(pitch).unwrap();
    assert!((ratio - 1.5).abs() < 1e-12);
    let input: Vec<f32> = (0..RATE as usize)
        .map(|i| (0.15 * (std::f64::consts::TAU * 440.0 * i as f64 / RATE as f64).sin()) as f32)
        .collect();
    let mut output = vec![0.0; input.len()];
    let mut shifter = PitchShifter::new(
        pitch,
        false,
        0.0,
        PitchAlgorithm::Pvsola,
        rust_low_latency_voice_changer::pitch::SpectralWindows::default(),
    )
    .unwrap();
    let mut position = 0;
    let blocks = [1, 127, 512, 64, 239];
    let mut block_index = 0;
    while position < input.len() {
        let end = (position + blocks[block_index % blocks.len()]).min(input.len());
        shifter.process_block(&input[position..end], &mut output[position..end]);
        position = end;
        block_index += 1;
    }
    let settled = &output[16_384..24_576];
    let frequency = peak_frequency(settled, 620.0, 700.0);
    assert!((frequency - 660.0).abs() < 2.0, "measured {frequency} Hz");
    assert!(output
        .iter()
        .all(|sample| sample.is_finite() && sample.abs() < 1.0));
}

#[test]
fn formant_preservation_keeps_the_first_resonance_after_pitch_resampling() {
    let pitch = 7.019550008653875;
    let mut input = Vec::with_capacity(RATE as usize * 2);
    let mut phases = [0.0_f64; 80];
    for _ in 0..RATE as usize * 2 {
        let mut sample = 0.0;
        for (index, phase) in phases.iter_mut().enumerate() {
            let harmonic = index + 1;
            let frequency = 120.0 * harmonic as f64;
            *phase += std::f64::consts::TAU * frequency / RATE as f64;
            let first = (-0.5 * ((frequency - 900.0) / 150.0).powi(2)).exp();
            let second = 0.7 * (-0.5 * ((frequency - 2100.0) / 230.0).powi(2)).exp();
            sample += 0.003 * (first + second + 0.015) * phase.cos();
        }
        input.push(sample as f32);
    }
    let mut output = vec![0.0; input.len()];
    PitchShifter::new(
        pitch,
        true,
        0.0,
        PitchAlgorithm::Pvsola,
        rust_low_latency_voice_changer::pitch::SpectralWindows::default(),
    )
    .unwrap()
    .process_block(&input, &mut output);
    let peak = peak_frequency(&output[24_576..32_768], 700.0, 1500.0);
    assert!(
        (peak - 900.0).abs() < 40.0,
        "preserved formant peak={peak} Hz"
    );
}

#[test]
fn paper_pvsola_survives_pitch_transitions_and_reports_its_delay() {
    let mut shifter = PitchShifter::new(
        0.0,
        true,
        0.0,
        PitchAlgorithm::Pvsola,
        rust_low_latency_voice_changer::pitch::SpectralWindows::default(),
    )
    .unwrap();
    assert_eq!(PitchAlgorithm::Pvsola.latency(), 4608);
    let input: Vec<f32> = (0..RATE as usize)
        .map(|i| (0.1 * (std::f64::consts::TAU * 170.0 * i as f64 / RATE as f64).sin()) as f32)
        .collect();
    let mut output = vec![0.0; input.len()];
    for (chunk_index, (input_chunk, output_chunk)) in
        input.chunks(384).zip(output.chunks_mut(384)).enumerate()
    {
        if matches!(chunk_index, 24 | 48 | 72) {
            let semitones = match chunk_index {
                24 => 7.0,
                48 => -4.0,
                _ => 0.0,
            };
            shifter.set_semitones(semitones).unwrap();
        }
        shifter.process_block(input_chunk, output_chunk);
    }
    assert!(
        output.iter().all(|sample| sample.is_finite()),
        "non-finite PVSOLA transition output"
    );
}

#[test]
fn neutral_interval_does_not_replay_old_pvsola_audio() {
    let mut shifter = PitchShifter::new(
        7.0,
        false,
        0.0,
        PitchAlgorithm::Pvsola,
        rust_low_latency_voice_changer::pitch::SpectralWindows::default(),
    )
    .unwrap();
    let mut output = vec![0.0; 24000];
    shifter.process_block(&vec![0.1_f32; output.len()], &mut output);
    shifter.set_semitones(0.0).unwrap();
    shifter.process_block(&vec![0.0_f32; output.len()], &mut output);
    shifter.set_semitones(-3.0).unwrap();
    shifter.process_block(&vec![0.0_f32; output.len()], &mut output);
    assert!(
        output.iter().all(|x| x.abs() < 1e-8),
        "old audio replayed after neutral bypass: peak={}",
        output.iter().map(|x| x.abs()).fold(0.0_f32, f32::max)
    );
}

#[test]
fn neutral_to_pitched_keeps_live_audio_until_the_delayed_path_is_ready() {
    let input = vec![0.1_f32; 12000];
    let mut output = vec![0.0; input.len()];
    let mut dsp = PitchShifter::new(
        0.0,
        false,
        0.0,
        PitchAlgorithm::Pvsola,
        rust_low_latency_voice_changer::pitch::SpectralWindows::default(),
    )
    .unwrap();
    dsp.process_block(&input, &mut output);
    for pitch in [7.0, -3.0] {
        dsp.set_semitones(pitch).unwrap();
        dsp.process_block(&input, &mut output);
        for block in output[..4608].chunks(128) {
            assert!(
                block.iter().any(|x| x.abs() > 0.03),
                "live signal faded into silence"
            );
        }
        dsp.set_semitones(0.0).unwrap();
        dsp.process_block(&input, &mut output);
    }
}
