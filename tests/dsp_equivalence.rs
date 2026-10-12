mod reference;
use rust_low_latency_voice_changer::{pitch::PitchShifter, RATE};

#[test]
fn optimized_dsp_matches_original_bits_including_transitions_and_formats() {
    let mut input = reference::signal(16_384);
    input[8192..8199].copy_from_slice(&[
        -0.0,
        f32::from_bits(1),
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        -f32::MAX,
    ]);
    input[12_288..].fill(0.0);
    for preserve in [false, true] {
        for size in [1, 7, 64, 512] {
            let mut original = reference::pitch::PitchShifter::new(3.0, preserve).unwrap();
            let mut optimized = PitchShifter::new(3.0, preserve).unwrap();
            for pitch in [3.0, -3.0, 12.0, -12.0, 0.0, 3.0] {
                original.set_semitones(pitch).unwrap();
                optimized.set_semitones(pitch).unwrap();
                for chunk in input.chunks(size) {
                    let mut expected = vec![0.0; chunk.len()];
                    let mut actual = vec![0.0; chunk.len()];
                    assert_eq!(
                        original.process_block(chunk, &mut expected),
                        optimized.process_block(chunk, &mut actual)
                    );
                    assert_eq!(
                        expected.iter().map(|s| s.to_bits()).collect::<Vec<_>>(),
                        actual.iter().map(|s| s.to_bits()).collect::<Vec<_>>(),
                        "preserve={preserve}, size={size}, pitch={pitch}"
                    );
                }
            }
            let integers: Vec<i32> = input
                .iter()
                .map(|&x| (x as f64 * 2147483648.0) as i32)
                .collect();
            let shorts: Vec<i16> = input.iter().map(|&x| (x as f64 * 32768.0) as i16).collect();
            let mut expected = vec![0; input.len()];
            let mut actual = vec![0; input.len()];
            original.process_block(&integers, &mut expected);
            optimized.process_block(&integers, &mut actual);
            assert_eq!(expected, actual);
            let mut expected = vec![0; input.len()];
            let mut actual = vec![0; input.len()];
            original.process_block(&shorts, &mut expected);
            optimized.process_block(&shorts, &mut actual);
            assert_eq!(expected, actual);
        }
    }
}
