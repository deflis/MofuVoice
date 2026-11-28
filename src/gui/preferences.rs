use crate::{audio::FilterStage, gui_settings::UiPreferences};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Ja,
    En,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::Ja => "ja",
            Self::En => "en",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ja" => Some(Self::Ja),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Ja => Self::En,
            Self::En => Self::Ja,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Overview,
    Devices,
    Voice,
    InputEq,
    Limiter,
    NoiseGate,
    Dereverb,
    Deesser,
    Compressor,
    PreConversionEq,
    PostConversionEq,
    AutoLevel,
    PostCompressor,
    Reverb,
}

impl Section {
    pub const ALL: [Self; 14] = [
        Self::Overview,
        Self::Devices,
        Self::Voice,
        Self::InputEq,
        Self::Limiter,
        Self::NoiseGate,
        Self::Dereverb,
        Self::Deesser,
        Self::Compressor,
        Self::PreConversionEq,
        Self::PostConversionEq,
        Self::AutoLevel,
        Self::PostCompressor,
        Self::Reverb,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Devices => "devices",
            Self::Voice => "voice",
            Self::InputEq => "inputEq",
            Self::Limiter => "limiter",
            Self::NoiseGate => "noiseGate",
            Self::Dereverb => "dereverb",
            Self::Deesser => "deesser",
            Self::Compressor => "compressor",
            Self::PreConversionEq => "preConversionEq",
            Self::PostConversionEq => "postConversionEq",
            Self::AutoLevel => "autoLevel",
            Self::PostCompressor => "postCompressor",
            Self::Reverb => "reverb",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "フィルタ構成",
            Self::Devices => "音声デバイス",
            Self::Voice => "声の変換",
            Self::InputEq => "入力EQ",
            Self::Limiter => "ピーク保護",
            Self::NoiseGate => "ノイズゲート",
            Self::Dereverb => "反響低減",
            Self::Deesser => "ディエッサー",
            Self::Compressor => "コンプレッサー",
            Self::PreConversionEq => "変換前EQ",
            Self::PostConversionEq => "変換後EQ",
            Self::AutoLevel => "自動音量調整",
            Self::PostCompressor => "変換後コンプレッサー",
            Self::Reverb => "リバーブ",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.id() == value)
    }

    pub fn from_stage(stage: FilterStage) -> Self {
        match stage {
            FilterStage::InputEq => Self::InputEq,
            FilterStage::Limiter => Self::Limiter,
            FilterStage::NoiseGate => Self::NoiseGate,
            FilterStage::Dereverb => Self::Dereverb,
            FilterStage::Deesser => Self::Deesser,
            FilterStage::Compressor => Self::Compressor,
            FilterStage::PreConversionEq => Self::PreConversionEq,
            FilterStage::Voice => Self::Voice,
            FilterStage::PostConversionEq => Self::PostConversionEq,
            FilterStage::Level => Self::AutoLevel,
            FilterStage::Reverb => Self::Reverb,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub language: Language,
    pub section: Section,
}

impl Preferences {
    pub fn system() -> Self {
        Self {
            language: sys_locale::get_locales()
                .find_map(|locale| {
                    let base = locale.split(['-', '_']).next()?.to_ascii_lowercase();
                    Language::parse(&base)
                })
                .unwrap_or(Language::En),
            section: Section::Voice,
        }
    }

    pub fn from_saved(ui: UiPreferences) -> Self {
        let mut preferences = Self::system();
        if let Some(language) = ui.language.as_deref().and_then(Language::parse) {
            preferences.language = language;
        }
        if let Some(section) = ui.section.as_deref().and_then(Section::parse) {
            preferences.section = section;
        }
        preferences
    }

    pub fn stored(&self) -> UiPreferences {
        UiPreferences {
            language: Some(self.language.code().into()),
            section: Some(self.section.id().into()),
        }
    }

    pub fn text(&self, key: &str) -> String {
        if let Some((prefix, label)) = key.split_once(" · ") {
            return format!("{prefix} · {}", self.text(label));
        }
        static EN: OnceLock<BTreeMap<String, String>> = OnceLock::new();
        if self.language == Language::Ja {
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
        assert_eq!(preferences.section, Section::Devices);
        let preferences = Preferences::from_saved(UiPreferences {
            language: Some("ja".into()),
            section: Some("unknown".into()),
        });
        assert_eq!(preferences.language, Language::Ja);
        assert_eq!(preferences.section, defaults.section);
    }
}
