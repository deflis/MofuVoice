use mofu_voice::{limiter::LimiterSettings, RATE};
use std::{hint::black_box, time::Instant};

fn ceiling(db: f64) -> f64 {
    (db * (std::f64::consts::LOG2_10 / 20.0)).exp2()
}

fn release(ms: f64) -> f64 {
    (-1.0 / (ms * 0.001 * RATE as f64)).exp()
}

fn process_reference(samples: &mut [f64], ceiling: f64, release: f64, gain: &mut f64) {
    for sample in samples {
        let input = *sample;
        if !input.is_finite() {
            *sample = 0.0;
            *gain = 1.0;
            continue;
        }
        let peak = input.abs();
        let target = if peak > ceiling { ceiling / peak } else { 1.0 };
        let released = 1.0 - (1.0 - *gain) * release;
        *gain = target.min(released);
        if *gain < 1.0 {
            *sample = (input * *gain).clamp(-ceiling, ceiling);
        }
    }
}

fn median_reference(input: &[f64], settings: LimiterSettings) -> f64 {
    let ceiling = ceiling(settings.ceiling_db);
    let release = release(settings.release_ms);
    let mut times = Vec::with_capacity(31);
    for _ in 0..31 {
        let mut gain = 1.0;
        let mut output = [0.0_f64; 64];
        let start = Instant::now();
        for chunk in input.chunks(64) {
            output[..chunk.len()].copy_from_slice(black_box(chunk));
            process_reference(&mut output[..chunk.len()], ceiling, release, &mut gain);
            black_box(&output);
        }
        times.push(start.elapsed().as_secs_f64() * 1e9 / input.len() as f64);
    }
    times.sort_by(f64::total_cmp);
    times[15]
}

fn median_limiter(input: &[f64], settings: LimiterSettings) -> f64 {
    let mut times = Vec::with_capacity(31);
    for _ in 0..31 {
        let mut limiter = settings.build().unwrap();
        let mut output = [0.0_f64; 64];
        let start = Instant::now();
        for chunk in input.chunks(64) {
            output[..chunk.len()].copy_from_slice(black_box(chunk));
            limiter.process_in_place(black_box(&mut output[..chunk.len()]));
            black_box(&output);
        }
        times.push(start.elapsed().as_secs_f64() * 1e9 / input.len() as f64);
    }
    times.sort_by(f64::total_cmp);
    times[15]
}

fn output_reference(input: &[f64], settings: LimiterSettings) -> Vec<f64> {
    let mut gain = 1.0;
    let ceiling = ceiling(settings.ceiling_db);
    let release = release(settings.release_ms);
    let mut output = input.to_vec();
    process_reference(&mut output, ceiling, release, &mut gain);
    output
}

#[test]
#[ignore = "manual local benchmark; run with cargo test --bench limiter -- --ignored --nocapture"]
fn limiter_below_ceiling_and_fallback_benchmark() {
    let settings = LimiterSettings {
        enabled: true,
        ..Default::default()
    };
    for (name, amplitude) in [("below-ceiling", 0.2), ("fallback-clipping", 0.95)] {
        let input: Vec<f64> = (0..RATE as usize * 4)
            .map(|i| amplitude * (std::f64::consts::TAU * 173.0 * i as f64 / RATE as f64).sin())
            .collect();
        let expected = output_reference(&input, settings);
        let mut actual = input.clone();
        settings.build().unwrap().process_in_place(&mut actual);
        assert_eq!(actual, expected, "{name} output diverged from reference");

        let baseline = median_reference(&input, settings);
        let optimized = median_limiter(&input, settings);
        println!(
            "{name}: reference={baseline:.2} ns/sample, limiter={optimized:.2} ns/sample, speedup={:.2}x (median of 31, 64-frame blocks)",
            baseline / optimized
        );
    }
}
