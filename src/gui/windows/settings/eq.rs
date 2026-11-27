use super::*;

impl SettingsView {
    pub(super) fn band_count(
        &mut self,
        kind: EqKind,
        add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.read(cx).locked() || self.pending() {
            return;
        }
        if add {
            let Some(index) = self.draft.add_eq_band(kind) else {
                return;
            };
            for field in EqBandField::ALL {
                let id = NumberField::EqBand(kind, index, field);
                let control = self.create_number_control(id, window, cx);
                self.numbers.insert(id, control);
            }
        } else {
            let Some(removed) = self.draft.remove_eq_band(kind) else {
                return;
            };
            for field in EqBandField::ALL {
                self.numbers
                    .remove(&NumberField::EqBand(kind, removed, field));
            }
        }
        self.error = None;
        self.notice.clear();
        self.save_failed = false;
        window.refresh();
        self.notify_host(cx);
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::super::tests::{cleanup, directory, open, set_number};
    use super::super::CloseTarget;
    use crate::gui::draft::{EqBandField, EqKind, ErrorField, NumberField};
    use gpui_kit::TestAppContext;

    #[gpui_kit::test]
    fn eq_band_edits_can_remove_an_invalid_band_and_preserve_other_invalid_fields(
        cx: &mut TestAppContext,
    ) {
        let dir = directory();
        let (model, window, view) = open(cx, &dir);
        set_number(&view, window, NumberField::Pitch, "", cx);
        let initial_count =
            view.read_with(cx, |v, _| v.draft.eq(EqKind::PostConversion).bands.len());
        let band_q = NumberField::EqBand(EqKind::PostConversion, initial_count, EqBandField::Q);

        window
            .update(cx, |_, w, cx| {
                view.update(cx, |v, cx| {
                    v.band_count(EqKind::PostConversion, true, w, cx)
                })
            })
            .unwrap();
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.eq(EqKind::PostConversion).bands.len()),
            initial_count + 1
        );
        set_number(&view, window, band_q, "", cx);
        view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
        assert_eq!(
            view.read_with(cx, |v, _| v.error.as_ref().and_then(|e| e.field)),
            Some(ErrorField::Number(band_q))
        );
        window
            .update(cx, |_, w, cx| {
                view.update(cx, |v, cx| {
                    v.band_count(EqKind::PostConversion, false, w, cx)
                })
            })
            .unwrap();

        view.read_with(cx, |v, _| {
            assert_eq!(
                v.draft.eq(EqKind::PostConversion).bands.len(),
                initial_count
            );
            assert!(!v.numbers.contains_key(&band_q));
            assert_eq!(v.draft.number_text(NumberField::Pitch), "");
            assert!(v.error.is_none());
        });
        view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
        assert_eq!(
            view.read_with(cx, |v, _| v.error.as_ref().and_then(|e| e.field)),
            Some(ErrorField::Number(NumberField::Pitch))
        );
        cleanup(model, window, cx);
    }
}
