use super::SettingsView;
use crate::gui::draft::NumberField;

impl SettingsView {
    pub(super) fn deesser_custom(&self) -> bool {
        [
            (NumberField::DeesserFrequencyHz, 5000.),
            (NumberField::DeesserThresholdDb, -24.),
            (NumberField::DeesserAttackMs, 0.5),
            (NumberField::DeesserReleaseMs, 60.),
        ]
        .into_iter()
        .any(|(field, default)| self.draft.number_text(field).parse::<f64>().ok() != Some(default))
    }
}
