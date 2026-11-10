use crate::eq::EqSettings;
use gpui_kit::*;
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
pub fn eq_plot(settings: &EqSettings, rate: u32, cx: &App) -> impl IntoElement {
    let palette = super::theme::palette(cx);
    let values: Vec<f32> = (0..256)
        .map(|i| eq_response(settings, rate, 20. * 1000f64.powf(i as f64 / 255.)) as f32)
        .collect();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_color(palette.plot_label)
        .child("dB · +12 / 0 / −36")
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let mut path = PathBuilder::stroke(px(2.));
                    for (i, value) in values.iter().enumerate() {
                        let p = point(
                            bounds.left() + bounds.size.width * (i as f32 / 255.),
                            bounds.top()
                                + bounds.size.height * ((12. - value.clamp(-36., 12.)) / 48.),
                        );
                        if i == 0 {
                            path.move_to(p);
                        } else {
                            path.line_to(p);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, palette.accent);
                    }
                },
            )
            .w_full()
            .h(px(140.))
            .bg(palette.surface),
        )
        .child("20 Hz                                 1 kHz                                 20 kHz")
}
#[cfg(test)]
mod tests {
    use super::{eq_response, EqSettings};
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
