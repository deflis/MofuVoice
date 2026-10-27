use mofu_voice::{
    audio::{AudioProcessor, DspSettings},
    cli::Options,
    dsp::Sample,
    eq::EqSettings,
    RATE,
};

fn run<T: Sample>(input: &[T], args: &[&str], block: usize) -> Vec<T> {
    let options = Options::parse(args.iter().map(|arg| (*arg).into())).unwrap();
    let mut processor = AudioProcessor::new(RATE, block, DspSettings::from(&options)).unwrap();
    let mut output = vec![T::default(); input.len()];
    for (source, dest) in input.chunks(block).zip(output.chunks_mut(block)) {
        processor.process_block(source, dest).unwrap();
    }
    output
}

#[test]
fn eq_chain_is_continuous_across_variable_blocks_and_modes() {
    let input: Vec<f32> = (0..12000)
        .map(|i| (0.2 * (i as f64 * 0.027).sin()) as f32)
        .collect();
    for pitch in ["-12", "0", "3", "7", "12"] {
        let args = [
            "--pitch",
            pitch,
            "--formant-shift",
            "2",
            "--eq",
            "on",
            "--eq-band1",
            "250,-4,0.7",
            "--eq-band2",
            "3000,3,0.8",
            "--eq-band3",
            "8000,2,0.7",
            "--auto-level",
            "on",
        ];
        let expected = run(&input, &args, 512);
        for block in [1, 63, 128, 777] {
            assert_eq!(run(&input, &args, block), expected);
        }
        for mode in ["simple", "pro"] {
            let mut mode_args = args.to_vec();
            mode_args.extend(["--eq-mode", mode]);
            assert_eq!(run(&input, &mode_args, 512), expected);
        }
    }
}

#[test]
fn eq_converts_integer_formats_once_and_has_no_buffered_delay() {
    let options =
        Options::parse(["--eq", "on", "--eq-band1", "250,-4,0.7"].map(String::from)).unwrap();
    let input: Vec<f32> = (0..2000)
        .map(|i| (0.2 * (i as f64 * 0.1).sin()) as f32)
        .collect();
    let mut expected: Vec<f64> = input.iter().map(|s| *s as f64).collect();
    options.eq.build().unwrap().process_in_place(&mut expected);
    assert_eq!(
        run(&input, &["--eq", "on", "--eq-band1", "250,-4,0.7"], 63),
        expected.iter().map(|s| *s as f32).collect::<Vec<_>>()
    );
    let i16_input: Vec<_> = input
        .iter()
        .map(|x| i16::from_normalized(*x as f64))
        .collect();
    let i32_input: Vec<_> = input
        .iter()
        .map(|x| i32::from_normalized(*x as f64))
        .collect();
    let i16_output = run(&i16_input, &["--eq", "on", "--eq-band1", "250,-4,0.7"], 128);
    let i32_output = run(&i32_input, &["--eq", "on", "--eq-band1", "250,-4,0.7"], 128);
    for (i, sample) in expected.iter().enumerate() {
        assert!((i16_output[i].normalized() - *sample).abs() < 0.0001);
        assert!((i32_output[i].normalized() - *sample).abs() < 0.000001);
    }
    let mut impulse = [1.0_f64, 0.0, 0.0];
    options.eq.build().unwrap().process_in_place(&mut impulse);
    assert_ne!(impulse[0], 0.0);
    for enabled in ["off", "on"] {
        assert_eq!(run(&i16_input, &["--eq", enabled], 63), i16_input);
        assert_eq!(run(&i32_input, &["--eq", enabled], 63), i32_input);
    }
}

#[test]
fn eq_arguments_validate_and_modes_preserve_parameters() {
    for args in [
        vec!["--eq", "yes"],
        vec!["--eq-mode", "advanced"],
        vec!["--eq-band1", "250,NaN,1"],
        vec!["--eq-output", "1"],
        vec!["--eq", "on", "--eq", "off"],
        vec!["--eq-mode", "pro", "--inspect"],
    ] {
        assert!(Options::parse(args.into_iter().map(String::from)).is_err());
    }
    let options = Options::parse(
        [
            "--eq",
            "on",
            "--eq-band2",
            "2200,4,1.2",
            "--eq-output",
            "-2",
        ]
        .map(String::from),
    )
    .unwrap();
    assert_eq!(options.eq.bands[1].frequency, 2200.0);
    assert_eq!(options.eq.headroom_db(), 4.0);
    assert_eq!(
        Options::parse(Vec::<String>::new()).unwrap().eq,
        EqSettings::default()
    );
}

#[test]
fn added_bands_are_processed_and_contiguous_across_blocks() {
    let args = [
        "--eq",
        "on",
        "--eq-band5",
        "6000,2,1",
        "--eq-band4",
        "1200,-6,1",
    ];
    let options = Options::parse(args.map(String::from)).unwrap();
    assert_eq!(options.eq.bands.len(), 5);
    let input: Vec<f32> = (0..4096).map(|i| 0.1 * (i as f32 * 0.157).sin()).collect();
    let expected = run(&input, &args, 512);
    assert_ne!(expected, run(&input, &["--eq", "on"], 512));
    for block in [1, 31, 777] {
        assert_eq!(run(&input, &args, block), expected);
    }
    for flag in ["--eq-band0", "--eq-band04", "--eq-band5"] {
        assert!(Options::parse([flag, "1000,0,1"].map(String::from)).is_err());
    }
}
