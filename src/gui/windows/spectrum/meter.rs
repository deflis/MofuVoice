use crate::{gui::theme, spectrum::MeterLevels};
use gpui_kit::{base::TestSupportExt as _, *};

fn height(db: f64) -> f32 {
    ((db.clamp(-60., 0.) + 60.) / 60.) as f32
}
fn clipped(levels: MeterLevels) -> bool {
    levels.hold_db >= 0.
}

pub fn render(levels: MeterLevels, cx: &App) -> Div {
    let p = theme::palette(cx);
    let clip_color: Hsla = rgb(0xeb4b4b).into();
    div()
        .w(px(148.))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap_2()
        .child("Output 1 / dBFS")
        .child(div().flex().gap_3().child("RMS").child("Peak"))
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    div().id("spectrum-meter-bars").test_support().child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let y = |db| bounds.bottom() - bounds.size.height * height(db);
                                for (left, db) in [(12., levels.rms_db), (48., levels.peak_db)] {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(bounds.left() + px(left), bounds.top()),
                                            size(px(26.), bounds.size.height),
                                        ),
                                        p.control,
                                    ));
                                    for (low, high, color) in [
                                        (-60., -12., p.accent),
                                        (-12., -3., rgb(0xebb93c).into()),
                                        (-3., 0., clip_color),
                                    ] {
                                        if db > low {
                                            window.paint_quad(fill(
                                                Bounds::new(
                                                    point(
                                                        bounds.left() + px(left),
                                                        y(db.min(high)),
                                                    ),
                                                    size(px(26.), y(low) - y(db.min(high))),
                                                ),
                                                color,
                                            ));
                                        }
                                    }
                                }
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(bounds.left() + px(46.), y(levels.hold_db)),
                                        size(px(30.), px(2.)),
                                    ),
                                    p.foreground,
                                ));
                            },
                        )
                        .w(px(80.))
                        .h(px(252.)),
                    ),
                )
                .child(
                    div()
                        .h(px(252.))
                        .flex()
                        .flex_col()
                        .justify_between()
                        .text_color(p.muted)
                        .children(
                            [0, -12, -24, -36, -48, -60]
                                .into_iter()
                                .map(|db| db.to_string()),
                        ),
                ),
        )
        .child(format!("RMS {:.1}", levels.rms_db))
        .child(format!("Peak {:.1}", levels.peak_db))
        .child(
            div()
                .id("spectrum-meter-hold")
                .test_support()
                .text_color(if clipped(levels) { clip_color } else { p.muted })
                .child(format!("Hold {:.1}", levels.hold_db)),
        )
}

#[cfg(test)]
mod tests {
    use super::{clipped, height};
    use crate::spectrum::MeterLevels;
    #[test]
    fn scale_clamps_silence_and_clipping_and_places_hold_on_the_same_scale() {
        assert_eq!(height(-120.), 0.);
        assert_eq!(height(-60.), 0.);
        assert!((height(-12.) - 0.8).abs() < 1e-6);
        assert!((height(-3.) - 0.95).abs() < 1e-6);
        assert_eq!(height(0.), 1.);
        assert_eq!(height(6.), 1.);
        let mut levels = MeterLevels {
            rms_db: -18.,
            peak_db: -3.,
            hold_db: 0.,
        };
        assert!(clipped(levels));
        levels.hold_db = -0.01;
        assert!(!clipped(levels));
        assert!(height(levels.rms_db) < height(levels.peak_db));
        assert!(height(levels.peak_db) < height(levels.hold_db));
    }

    #[cfg(feature = "desktop-tests")]
    #[gpui_kit::test]
    fn bars_and_peak_hold_render_and_update_in_both_themes(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::{test::TestWindowExt as _, AppContext as _, IntoElement, Render};
        struct MeterTest(MeterLevels);
        impl Render for MeterTest {
            fn render(
                &mut self,
                _: &mut gpui_kit::Window,
                cx: &mut gpui_kit::Context<Self>,
            ) -> impl IntoElement {
                super::render(self.0, cx)
            }
        }
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::gui::theme::init(cx);
        });
        let (window, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    crate::gui::layout::window_options("Meter test", 220., 450., 180., 400.),
                    cx,
                    |_, cx| cx.new(|_| MeterTest(MeterLevels::default())),
                )
            })
            .unwrap();
        for appearance in [
            gpui_kit::WindowAppearance::Light,
            gpui_kit::WindowAppearance::Dark,
        ] {
            cx.update(|cx| crate::gui::theme::apply_appearance(appearance, cx));
            for levels in [
                MeterLevels::default(),
                MeterLevels {
                    rms_db: -9.,
                    peak_db: 0.,
                    hold_db: 1.,
                },
            ] {
                view.update(cx, |v, cx| {
                    v.0 = levels;
                    cx.notify();
                });
                window
                    .update(cx, |_, w, cx| {
                        w.render_frame(cx);
                        let bars = w.find("spectrum-meter-bars").bounds();
                        assert!(
                            bars.size.width > gpui_kit::px(0.)
                                && bars.size.height > gpui_kit::px(0.)
                        );
                        assert!(w.try_find("spectrum-meter-hold").is_some());
                    })
                    .unwrap();
            }
        }
        window.update(cx, |_, w, _| w.remove_window()).unwrap();
    }
}
