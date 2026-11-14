use crate::eq::EqSettings;
mod diagram;
pub use diagram::{render, Diagram};
use std::f64::consts::PI;

/// Small-signal response, including the DSP's reserved positive-gain headroom.
pub fn eq_response(settings: &EqSettings, rate: u32, frequency: f64) -> f64 {
    let phase = 2. * PI * frequency / rate as f64;
    let (s1, c1) = phase.sin_cos();
    let (s2, c2) = (2. * phase).sin_cos();
    let mut db = settings.output_db;
    for band in &settings.bands {
        db -= band.gain_db.max(0.);
        if band.gain_db == 0. {
            continue;
        }
        let a = 10f64.powf(band.gain_db / 40.);
        let omega = 2. * PI * band.frequency / rate as f64;
        let alpha = omega.sin() / (2. * band.q);
        let norm = 1. / (1. + alpha / a);
        let b0 = (1. + alpha * a) * norm;
        let b1 = -2. * omega.cos() * norm;
        let b2 = (1. - alpha * a) * norm;
        let a2 = (1. - alpha / a) * norm;
        let numerator = (b0 + b1 * c1 + b2 * c2).powi(2) + (b1 * s1 + b2 * s2).powi(2);
        let denominator = (1. + b1 * c1 + a2 * c2).powi(2) + (b1 * s1 + a2 * s2).powi(2);
        db += 10. * (numerator / denominator).log10();
    }
    db
}
/// Bilinear-transform response of the existing 80 Hz Butterworth HPF.
pub fn rumble_response(rate: u32, frequency: f64) -> f64 {
    let ratio = (PI * frequency / rate as f64).tan() / (PI * 80. / rate as f64).tan();
    -10. * (1. + ratio.powi(-4)).log10()
}
pub fn active_eq_response(settings: &EqSettings, rate: u32, frequency: f64, rumble: bool) -> f64 {
    (if settings.enabled {
        eq_response(settings, rate, frequency)
    } else {
        0.
    }) + if rumble {
        rumble_response(rate, frequency)
    } else {
        0.
    }
}
pub fn eq_diagram(settings: &EqSettings, rate: u32, rumble: bool, description: String) -> Diagram {
    let values: Vec<_> = (0..=120)
        .map(|i| {
            let frequency = 20. * 1000f64.powf(i as f64 / 120.);
            (
                frequency,
                active_eq_response(settings, rate, frequency, rumble),
            )
        })
        .collect();
    let min = (values.iter().map(|(_, db)| *db).fold(-12., f64::min) / 6.).floor() * 6.;
    let max = (values.iter().map(|(_, db)| *db).fold(6., f64::max) / 6.).ceil() * 6.;
    let x = |f: f64| 36. + (f / 20.).log10() / 3. * 272.;
    let y = |db: f64| 86. - (db - min) / (max - min) * 64.;
    let mut graph = Diagram::new(
        description,
        "20 Hz → 20 kHz (log)".into(),
        format!("{min} … {max} dB"),
    );
    graph.references.push(vec![(36., y(0.)), (308., y(0.))]);
    graph
        .paths
        .push(values.iter().map(|(f, db)| (x(*f), y(*db))).collect());
    if settings.enabled {
        graph.markers = settings
            .bands
            .iter()
            .map(|b| {
                (
                    x(b.frequency),
                    y(active_eq_response(settings, rate, b.frequency, rumble)),
                )
            })
            .collect();
    }
    graph
}
#[cfg(test)]
mod tests {
    use super::{eq_response, EqSettings};
    #[test]
    fn input_curve_combines_only_active_eq_and_high_pass() {
        use super::{active_eq_response, rumble_response};
        let mut eq = EqSettings::default();
        eq.bands[0].gain_db = 6.;
        for rate in [44100, 48000] {
            eq.enabled = false;
            assert_eq!(active_eq_response(&eq, rate, 80., false), 0.);
            assert!((active_eq_response(&eq, rate, 80., true) + 3.01029995664).abs() < 1e-9);
            assert!(rumble_response(rate, 20.) < -24.);
            assert!(rumble_response(rate, 10000.).abs() < 1e-6);
            eq.enabled = true;
            for frequency in [20., 80., 250., 1000., 16000.] {
                assert!(
                    (active_eq_response(&eq, rate, frequency, true)
                        - eq_response(&eq, rate, frequency)
                        - rumble_response(rate, frequency))
                    .abs()
                        < 1e-9
                );
            }
        }
    }
    #[test]
    fn response_includes_trim_and_headroom() {
        let mut settings = EqSettings {
            output_db: -3.,
            ..Default::default()
        };
        for rate in [44100, 48000] {
            for f in [20., 250., 1000., 16000.] {
                assert!((eq_response(&settings, rate, f) + 3.).abs() < 1e-9);
            }
        }
        settings.bands[0].gain_db = 6.;
        assert!((eq_response(&settings, 48000, 250.) + 3.).abs() < 1e-8);
        assert!(eq_response(&settings, 48000, 16000.) < -8.9);
    }
}
