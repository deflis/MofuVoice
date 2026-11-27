use super::*;
impl SettingsView {
    pub(super) fn band_count(
        &mut self,
        root: &str,
        add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.read(cx).locked() || self.pending() {
            return;
        }
        let mut options = self.base.options().expect("validated settings");
        let eq = match root {
            "inputEq" => &mut options.input_eq,
            "preConversionEq" => &mut options.pre_conversion_eq,
            _ => &mut options.eq,
        };
        if add && eq.bands.len() < crate::eq::MAX_BANDS {
            eq.bands.push(crate::eq::Band {
                frequency: 1000.,
                gain_db: 0.,
                q: 0.7,
            });
        } else if !add && eq.bands.len() > 1 {
            eq.bands.pop();
        } else {
            return;
        }
        self.base = Settings::from_options(&options);
        self.build_controls(window, cx);
        self.error.clear();
        window.refresh();
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::super::tests::{cleanup, directory, open, set};
    use super::super::CloseTarget;
    use gpui_kit::TestAppContext;

    #[gpui_kit::test]
    fn eq_band_edits_can_remove_an_invalid_band_and_preserve_other_invalid_fields(
        cx: &mut TestAppContext,
    ) {
        let dir = directory();
        let (model, window, view) = open(cx, &dir);
        set(&view, window, "/voice/pitch", "", cx);
        let initial_count = view.read_with(cx, |v, _| v.base.options().unwrap().eq.bands.len());
        let band_q = format!("/eq/bands/{initial_count}/q");

        window
            .update(cx, |_, w, cx| {
                view.update(cx, |v, cx| v.band_count("eq", true, w, cx))
            })
            .unwrap();
        assert_eq!(
            view.read_with(cx, |v, _| v.base.options().unwrap().eq.bands.len()),
            initial_count + 1
        );
        set(&view, window, &band_q, "", cx);
        view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
        assert!(view.read_with(cx, |v, _| v.error.starts_with(&band_q)));
        window
            .update(cx, |_, w, cx| {
                view.update(cx, |v, cx| v.band_count("eq", false, w, cx))
            })
            .unwrap();

        view.read_with(cx, |v, _| {
            assert_eq!(v.base.options().unwrap().eq.bands.len(), initial_count);
            assert!(!v.edits().contains_key(&band_q));
            assert_eq!(v.value("/voice/pitch"), "");
            assert!(v.error.is_empty());
        });
        view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
        assert!(view.read_with(cx, |v, _| v.error.starts_with("/voice/pitch")));
        cleanup(model, window, cx);
    }
}
