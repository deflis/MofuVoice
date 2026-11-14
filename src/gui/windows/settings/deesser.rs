use super::SettingsView;
impl SettingsView {
    pub(super) fn deesser_custom(&self) -> bool {
        [
            ("frequencyHz", 5000.),
            ("thresholdDb", -24.),
            ("attackMs", 0.5),
            ("releaseMs", 60.),
        ]
        .into_iter()
        .any(|(key, default)| {
            self.value(&format!("/deesser/{key}")).parse::<f64>().ok() != Some(default)
        })
    }
}
