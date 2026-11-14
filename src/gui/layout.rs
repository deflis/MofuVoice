use gpui_kit::*;
pub fn window_options(
    title: &str,
    width: f32,
    height: f32,
    minimum_width: f32,
    minimum_height: f32,
) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(100.), px(100.)),
            size(px(width), px(height)),
        ))),
        window_min_size: Some(size(px(minimum_width), px(minimum_height))),
        ..Default::default()
    }
}
pub fn surface(cx: &App) -> Div {
    let palette = super::theme::palette(cx);
    div()
        .size_full()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .bg(palette.background)
        .text_color(palette.foreground)
        .text_size(px(13.))
}
