use super::*;
impl SettingsView {
    pub(super) fn band_count(
        &mut self,
        root: &str,
        add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.read(cx).locked() {
            return;
        }
        match self.options() {
            Ok(mut options) => {
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
                }
                self.base = Settings::from_options(&options);
                self.build_controls(window, cx);
            }
            Err(e) => self.error = e,
        };
        window.refresh();
        cx.notify();
    }
}
