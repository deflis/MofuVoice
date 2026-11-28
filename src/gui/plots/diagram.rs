use gpui_kit::*;

/// Coordinates use the original overview's 320 × 112 drawing space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagram {
    pub description: String,
    pub x_label: String,
    pub y_label: String,
    pub paths: Vec<Vec<(f64, f64)>>,
    pub references: Vec<Vec<(f64, f64)>>,
    pub areas: Vec<[f64; 4]>,
    pub markers: Vec<(f64, f64)>,
}
impl Diagram {
    pub fn new(description: String, x_label: String, y_label: String) -> Self {
        Self {
            description,
            x_label,
            y_label,
            ..Self::default()
        }
    }
}

pub fn render(graph: Diagram, cx: &App) -> Div {
    let p = super::super::theme::palette(cx);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_color(p.plot_label)
        .child(graph.description.clone())
        .child(graph.y_label.clone())
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let position = |(x, y): (f64, f64)| {
                        point(
                            bounds.left() + bounds.size.width * (x as f32 / 320.),
                            bounds.top() + bounds.size.height * (y as f32 / 112.),
                        )
                    };
                    for [x, y, w, h] in &graph.areas {
                        window.paint_quad(fill(
                            Bounds::new(
                                position((*x, *y)),
                                size(
                                    bounds.size.width * (*w as f32 / 320.),
                                    bounds.size.height * (*h as f32 / 112.),
                                ),
                            ),
                            p.accent.opacity(0.18),
                        ));
                    }
                    let axes = vec![(36., 12.), (36., 86.), (308., 86.)];
                    for (paths, color, width) in [
                        (std::slice::from_ref(&axes), p.plot_axis, 1.),
                        (graph.references.as_slice(), p.plot_axis, 1.),
                        (graph.paths.as_slice(), p.accent, 2.),
                    ] {
                        for points in paths {
                            let mut path = PathBuilder::stroke(px(width));
                            for (i, value) in points.iter().enumerate() {
                                if i == 0 {
                                    path.move_to(position(*value));
                                } else {
                                    path.line_to(position(*value));
                                }
                            }
                            window.paint_path(path.build().expect("finite overview path"), color);
                        }
                    }
                    for marker in &graph.markers {
                        let mut path = PathBuilder::fill();
                        for i in 0..=16 {
                            let angle = i as f64 * std::f64::consts::TAU / 16.;
                            let point = position((
                                marker.0 + 3. * angle.cos(),
                                marker.1 + 3. * angle.sin(),
                            ));
                            if i == 0 {
                                path.move_to(point);
                            } else {
                                path.line_to(point);
                            }
                        }
                        path.close();
                        window.paint_path(path.build().expect("finite overview marker"), p.accent);
                    }
                },
            )
            .w_full()
            .h(px(140.))
            .bg(p.surface),
        )
        .child(graph.x_label)
}
