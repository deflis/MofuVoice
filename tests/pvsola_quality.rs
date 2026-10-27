use mofu_voice::{
    pitch::{pitch_ratio, PitchAlgorithm, PitchShifter, LATENCY},
    RATE,
};

#[test]
fn voiced_glides_keep_the_fundamental_phase_continuous() {
    let mut phase = 0.0;
    let phases: Vec<_> = (0..RATE as usize * 2)
        .map(|i| {
            let frequency = 100.0 + 100.0 * i as f64 / (RATE as f64 * 2.0);
            phase += std::f64::consts::TAU * frequency / RATE as f64;
            phase
        })
        .collect();
    let input: Vec<f64> = phases
        .iter()
        .map(|&phase| {
            (1..=8)
                .map(|h| 0.025 * (phase * h as f64).cos() / (h as f64).sqrt())
                .sum::<f64>()
        })
        .collect();
    for pitch in [3.0, 7.0, 12.0] {
        let ratio = pitch_ratio(pitch).unwrap();
        for algorithm in PitchAlgorithm::SPECTRAL {
            let mut output = vec![0.0; input.len()];
            PitchShifter::new(
                pitch,
                false,
                0.0,
                algorithm,
                mofu_voice::pitch::SpectralWindows::default(),
                Default::default(),
            )
            .unwrap()
            .process_block(&input, &mut output);
            let residual: Vec<_> = (8192..input.len() - LATENCY - 1024)
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
                    let delta = p[1] - p[0];
                    delta.sin().atan2(delta.cos()).abs()
                })
                .fold(0.0_f64, f64::max);
            eprintln!(
                "{}, pitch={pitch}, voiced-glide phase jump={maximum}",
                algorithm.name()
            );
            assert!(
                maximum < 0.5,
                "{}, pitch={pitch}, phase jump={maximum}",
                algorithm.name()
            );
        }
    }
}

fn harmonic_phase(signal: &[f64], frequency: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &sample) in signal.iter().enumerate() {
        let angle = std::f64::consts::TAU * frequency * i as f64 / RATE as f64;
        let window = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / signal.len() as f64).cos();
        re += sample * window * angle.cos();
        im -= sample * window * angle.sin();
    }
    im.atan2(re)
}

fn phase_curvature(signal: &[f64], fundamental: f64, shape: f64) -> f64 {
    let phases: Vec<_> = (1..=8)
        .map(|h| harmonic_phase(signal, fundamental * h as f64))
        .collect();
    phases
        .windows(3)
        .map(|p| {
            let value = p[0] - 2.0 * p[1] + p[2] - 2.0 * shape;
            value.sin().powi(2) + (1.0 - value.cos()).powi(2)
        })
        .sum::<f64>()
        / 6.0
}

#[test]
fn voiced_harmonics_keep_their_relative_phase_shape() {
    // Curvature removes arbitrary common time shifts while retaining the
    // input pulse shape. A quadratic harmonic phase tests a non-impulse shape too.
    for shape in [0.0, 0.17] {
        for preserve in [false, true] {
            for frequency in [80.0, 100.0, 137.0, 203.0] {
                for pitch in [-12.0, -3.0, 3.0, 7.0, 12.0] {
                    let input: Vec<f64> = (0..RATE as usize * 2)
                        .map(|i| {
                            let phase = std::f64::consts::TAU * frequency * i as f64 / RATE as f64;
                            (1..=8)
                                .map(|h| 0.02 * (phase * h as f64 + shape * (h * h) as f64).cos())
                                .sum::<f64>()
                        })
                        .collect();
                    let mut errors = [0.0; 2];
                    for (algorithm, error) in PitchAlgorithm::SPECTRAL.into_iter().zip(&mut errors)
                    {
                        let mut output = vec![0.0; input.len()];
                        PitchShifter::new(
                            pitch,
                            preserve,
                            0.0,
                            algorithm,
                            mofu_voice::pitch::SpectralWindows::default(),
                            Default::default(),
                        )
                        .unwrap()
                        .process_block(&input, &mut output);
                        let frames: Vec<_> = output[RATE as usize..]
                            .as_chunks::<8192>()
                            .0
                            .iter()
                            .map(|chunk| {
                                phase_curvature(
                                    chunk,
                                    frequency * pitch_ratio(pitch).unwrap(),
                                    shape,
                                )
                            })
                            .collect();
                        *error = frames.iter().sum::<f64>() / frames.len() as f64;
                        assert!(output.iter().all(|x| x.is_finite() && x.abs() < 1.0));
                    }
                    eprintln!(
                    "shape={shape}, preserve={preserve}, f={frequency}, pitch={pitch}: phase-locked={}, pvsola-sync={}",
                    errors[0], errors[1]
                );
                    // At 80 Hz, eight nonlinearly phased harmonics are less well
                    // resolved by the fixed analysis window. Require improvement
                    // there too, without claiming the same reconstruction accuracy.
                    let (absolute_bound, relative_bound) = if frequency == 80.0 && shape != 0.0 {
                        (1.6, 0.95)
                    } else if frequency == 80.0 || pitch == -12.0 {
                        (0.8, 0.5)
                    } else {
                        (0.15, 0.1)
                    };
                    assert!(
                    errors[1] < absolute_bound,
                    "shape={shape}, preserve={preserve}, f={frequency}, pitch={pitch}, phase curvature={}",
                    errors[1]
                );
                    assert!(errors[1] < errors[0] * relative_bound, "shape={shape}, preserve={preserve}, f={frequency}, pitch={pitch}: harmonic phase curvature did not improve sufficiently");
                }
            }
        }
    }
}
