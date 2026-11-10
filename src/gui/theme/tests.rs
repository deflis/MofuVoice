use super::{apply_appearance, init, palette};
use gpui_kit::{
    base,
    component::{Theme, ThemeMode},
    rgb, TestAppContext, WindowAppearance,
};

#[gpui_kit::test]
fn light_dark_and_vibrant_appearances_keep_styled_and_base_tokens_in_sync(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        init(cx);
    });
    for (appearance, dark, bg, text, accent, danger) in [
        (
            WindowAppearance::Light,
            false,
            0xf7f1ea,
            0x2a1f18,
            0xd4652a,
            0xa80000,
        ),
        (
            WindowAppearance::Dark,
            true,
            0x1a1512,
            0xf3e8dc,
            0xe8783a,
            0xff9090,
        ),
        (
            WindowAppearance::VibrantDark,
            true,
            0x1a1512,
            0xf3e8dc,
            0xe8783a,
            0xff9090,
        ),
        (
            WindowAppearance::VibrantLight,
            false,
            0xf7f1ea,
            0x2a1f18,
            0xd4652a,
            0xa80000,
        ),
    ] {
        cx.update(|cx| {
            apply_appearance(appearance, cx);
            let p = palette(cx);
            let theme = Theme::global(cx);
            let base = base::Theme::global(cx);
            assert_eq!(
                theme.mode,
                if dark {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                }
            );
            assert_eq!(p.background, rgb(bg).into());
            assert_eq!(p.foreground, rgb(text).into());
            assert_eq!(p.accent, rgb(accent).into());
            assert_eq!(p.danger, rgb(danger).into());
            assert_eq!(theme.background, p.background);
            assert_eq!(theme.popover, p.surface);
            assert_eq!(theme.button, p.control);
            assert_eq!(theme.slider_bar, p.accent);
            assert_eq!(theme.primary_foreground, p.on_accent);
            assert_eq!(theme.tokens.background.color, p.background);
            assert_eq!(theme.tokens.primary.color, p.accent);
            assert_eq!(base.tokens.colors.background, p.background);
            assert_eq!(base.tokens.colors.surface, p.surface);
            assert_eq!(base.tokens.colors.primary, p.accent);
            assert_eq!(
                base.appearance,
                if dark {
                    base::ThemeAppearance::Dark
                } else {
                    base::ThemeAppearance::Light
                }
            );
        });
    }
}
