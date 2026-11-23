use crate::gui_settings::UiPreferences;
use std::{collections::BTreeMap, sync::OnceLock};

pub const SECTIONS: &[(&str, &str)] = &[
    ("overview", "フィルタ構成"),
    ("devices", "音声デバイス"),
    ("voice", "声の変換"),
    ("inputEq", "入力EQ"),
    ("limiter", "ピーク保護"),
    ("noiseGate", "ノイズゲート"),
    ("dereverb", "反響低減"),
    ("deesser", "ディエッサー"),
    ("compressor", "コンプレッサー"),
    ("preConversionEq", "変換前EQ"),
    ("postConversionEq", "変換後EQ"),
    ("autoLevel", "自動音量調整"),
    ("postCompressor", "変換後コンプレッサー"),
    ("reverb", "リバーブ"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub language: String,
    pub section: String,
}
impl Preferences {
    pub fn system() -> Self {
        Self {
            language: sys_locale::get_locales()
                .find_map(|locale| {
                    let base = locale.split(['-', '_']).next()?.to_ascii_lowercase();
                    ["ja", "en"].contains(&base.as_str()).then_some(base)
                })
                .unwrap_or_else(|| "en".into()),
            section: "voice".into(),
        }
    }
    pub fn from_saved(ui: UiPreferences) -> Self {
        let mut preferences = Self::system();
        if let Some(language) = ui
            .language
            .filter(|language| matches!(language.as_str(), "ja" | "en"))
        {
            preferences.language = language;
        }
        if let Some(section) = ui
            .section
            .filter(|section| SECTIONS.iter().any(|(id, _)| id == section))
        {
            preferences.section = section;
        }
        preferences
    }

    pub fn stored(&self) -> UiPreferences {
        UiPreferences {
            language: Some(self.language.clone()),
            section: Some(self.section.clone()),
        }
    }
    pub fn text(&self, key: &str) -> String {
        if let Some((prefix, label)) = key.split_once(" · ") {
            return format!("{prefix} · {}", self.text(label));
        }
        static EN: OnceLock<BTreeMap<String, String>> = OnceLock::new();
        if self.language == "ja" {
            return key.into();
        }
        EN.get_or_init(|| {
            serde_json::from_str(include_str!("locales/en.json"))
                .expect("embedded English translations")
        })
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_invalid_preferences_default_independently() {
        let defaults = Preferences::system();
        assert_eq!(Preferences::from_saved(UiPreferences::default()), defaults);
        let preferences = Preferences::from_saved(UiPreferences {
            language: Some("unknown".into()),
            section: Some("devices".into()),
        });
        assert_eq!(preferences.language, defaults.language);
        assert_eq!(preferences.section, "devices");
        let preferences = Preferences::from_saved(UiPreferences {
            language: Some("ja".into()),
            section: Some("unknown".into()),
        });
        assert_eq!(preferences.language, "ja");
        assert_eq!(preferences.section, defaults.section);
    }
}
