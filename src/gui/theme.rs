//! The original UI palette, following the operating system's color scheme.
use gpui_kit::{
    component::{Theme, ThemeMode},
    px, rgb, rgba, App, Context, Hsla, Subscription, Window, WindowAppearance,
};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub control: Hsla,
    pub hover: Hsla,
    pub foreground: Hsla,
    pub secondary: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub strong_border: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub soft_accent: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,
    pub plot_label: Hsla,
    pub plot_axis: Hsla,
    pub overlay: Hsla,
}
fn colors(dark: bool) -> Palette {
    let (
        background,
        surface,
        raised,
        control,
        hover,
        foreground,
        secondary,
        muted,
        border,
        strong_border,
        accent,
        on_accent,
        soft_accent,
        danger,
        warning,
        plot_label,
        plot_axis,
        overlay,
    ) = if dark {
        (
            0x1a1512, 0x241e1a, 0x2c241e, 0x3a3028, 0x4a3c32, 0xf3e8dc, 0xb8a898, 0x9a8a78,
            0x43362c, 0x5a4a3c, 0xe8783a, 0x1a120c, 0xf0a070, 0xff9090, 0xffd092, 0xc4b4a0,
            0x6a5848, 0x00000099,
        )
    } else {
        (
            0xf7f1ea, 0xfffaf5, 0xf3ebe3, 0xebe0d4, 0xdfd1c2, 0x2a1f18, 0x6b5848, 0x8a7464,
            0xd4c4b4, 0xb8a090, 0xd4652a, 0xfff8f0, 0xb85420, 0xa80000, 0x9a6a20, 0x6b5848,
            0xc4b4a4, 0x00000066,
        )
    };
    Palette {
        background: rgb(background).into(),
        surface: rgb(surface).into(),
        raised: rgb(raised).into(),
        control: rgb(control).into(),
        hover: rgb(hover).into(),
        foreground: rgb(foreground).into(),
        secondary: rgb(secondary).into(),
        muted: rgb(muted).into(),
        border: rgb(border).into(),
        strong_border: rgb(strong_border).into(),
        accent: rgb(accent).into(),
        on_accent: rgb(on_accent).into(),
        soft_accent: rgb(soft_accent).into(),
        danger: rgb(danger).into(),
        warning: rgb(warning).into(),
        plot_label: rgb(plot_label).into(),
        plot_axis: rgb(plot_axis).into(),
        overlay: rgba(overlay).into(),
    }
}
pub fn palette(cx: &App) -> Palette {
    colors(Theme::global(cx).is_dark())
}

pub fn init(cx: &mut App) {
    apply_appearance(cx.window_appearance(), cx);
}

pub fn apply_appearance(appearance: WindowAppearance, cx: &mut App) {
    let dark = matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    let mode = if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    // A mode change loads Kit's defaults. Apply our palette afterward through
    // Theme::update so styled tokens and the Base projection change together.
    if Theme::global(cx).mode != mode {
        Theme::change(mode, None, cx);
    }
    let p = colors(dark);
    Theme::update(cx, |theme| {
        theme.font_size = px(13.);
        let c = &mut theme.colors;
        c.background = p.background;
        c.foreground = p.foreground;
        c.popover = p.surface;
        c.popover_foreground = p.foreground;
        c.button = p.control;
        c.button_hover = p.hover;
        c.button_active = p.strong_border;
        c.button_foreground = p.foreground;
        c.primary = p.accent;
        c.primary_hover = p.soft_accent;
        c.primary_active = p.soft_accent;
        c.primary_foreground = p.on_accent;
        c.secondary = p.control;
        c.secondary_hover = p.hover;
        c.secondary_active = p.strong_border;
        c.secondary_foreground = p.foreground;
        c.button_secondary = p.control;
        c.button_secondary_hover = p.hover;
        c.button_secondary_active = p.strong_border;
        c.button_secondary_foreground = p.foreground;
        c.accent = p.hover;
        c.accent_foreground = p.foreground;
        c.border = p.border;
        c.input = p.border;
        c.ring = p.accent;
        c.caret = p.accent;
        c.description_list_label = p.raised;
        c.description_list_label_foreground = p.secondary;
        c.muted = p.raised;
        c.muted_foreground = p.muted;
        c.list = p.surface;
        c.list_even = p.raised;
        c.list_head = p.raised;
        c.list_hover = p.hover;
        c.list_active = p.control;
        c.list_active_border = p.accent;
        c.selection = p.accent.opacity(0.3);
        c.sidebar = p.background;
        c.sidebar_foreground = p.foreground;
        c.sidebar_border = p.border;
        c.sidebar_accent = p.hover;
        c.sidebar_accent_foreground = p.foreground;
        c.sidebar_primary = p.accent;
        c.sidebar_primary_foreground = p.on_accent;
        c.slider_bar = p.accent;
        c.slider_thumb = p.accent;
        c.progress_bar = p.accent;
        c.switch = p.control;
        c.switch_thumb = p.on_accent;
        c.danger = p.danger;
        c.danger_foreground = p.on_accent;
        c.danger_hover = p.danger;
        c.danger_active = p.danger;
        c.button_danger = p.danger;
        c.button_danger_foreground = p.on_accent;
        c.button_danger_hover = p.danger;
        c.button_danger_active = p.danger;
        c.warning = p.warning;
        c.warning_foreground = p.background;
        c.warning_hover = p.warning;
        c.warning_active = p.warning;
        c.overlay = p.overlay;
        c.window_border = p.border;
        c.title_bar = p.surface;
        c.title_bar_border = p.border;
        c.status_bar = p.surface;
        c.status_bar_border = p.border;
        c.chart_1 = p.accent;
        c.chart_grid = p.plot_axis;
        c.scrollbar = p.background;
        c.scrollbar_thumb = p.strong_border;
        c.scrollbar_thumb_hover = p.plot_axis;
        c.group_box = p.surface;
        c.group_box_foreground = p.foreground;
    });
}

/// The main window lives for the whole application; its appearance notification
/// updates every window, including already-open modal layers and future windows.
pub fn observe<T: 'static>(window: &mut Window, cx: &mut Context<T>) -> Subscription {
    apply_appearance(window.appearance(), cx);
    cx.observe_window_appearance(window, |_, window, cx| {
        apply_appearance(window.appearance(), cx)
    })
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests;
