use std::{collections::BTreeMap, fs, io::ErrorKind, path::Path, sync::OnceLock};

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
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::system()),
            Err(e) => Err(format!("Cannot read {}: {e}", path.display())),
        }
    }
    fn parse(text: &str) -> Result<Self, String> {
        let mut values = BTreeMap::new();
        for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
            let (key, value) = line.split_once('=').ok_or("Invalid ui.ini entry")?;
            if !["language", "section"].contains(&key) || values.insert(key, value).is_some() {
                return Err(format!("Unknown or repeated ui.ini key: {key}"));
            }
        }
        let language = *values.get("language").ok_or("Missing ui.ini language")?;
        let section = *values.get("section").ok_or("Missing ui.ini section")?;
        if !["ja", "en"].contains(&language) || !SECTIONS.iter().any(|(id, _)| *id == section) {
            return Err("Invalid ui.ini language or section".into());
        }
        Ok(Self {
            language: language.into(),
            section: section.into(),
        })
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        fs::write(
            path,
            format!("language={}\nsection={}\n", self.language, self.section),
        )
        .map_err(|e| format!("Cannot save {}: {e}", path.display()))
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
    fn preferences_are_strict_and_round_trip() {
        let p = Preferences::parse("language=ja\nsection=devices\n").unwrap();
        assert_eq!(p.language, "ja");
        for text in [
            "language=xx\nsection=voice",
            "language=ja\nsection=missing",
            "language=ja\nlanguage=en\nsection=voice",
            "language=ja",
        ] {
            assert!(Preferences::parse(text).is_err());
        }
    }
}
