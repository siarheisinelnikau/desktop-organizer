//! Complete UI catalogs with an English fallback; user file and group names are never keys.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicU8, Ordering},
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    English,
    Russian,
    French,
    German,
    Spanish,
    Italian,
}
impl Language {
    pub const ALL: [Self; 6] = [
        Self::English,
        Self::Russian,
        Self::French,
        Self::German,
        Self::Spanish,
        Self::Italian,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Russian => "Русский",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::Italian => "Italiano",
        }
    }
    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

static LANGUAGE: AtomicU8 = AtomicU8::new(0);
static DARK: AtomicU8 = AtomicU8::new(0);
static CATALOG: LazyLock<HashMap<String, [String; 5]>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("locales.json")).expect("embedded locale catalog is valid")
});
pub fn configure(language: Language, theme: Theme) {
    LANGUAGE.store(language as u8, Ordering::Relaxed);
    DARK.store(u8::from(theme == Theme::Dark), Ordering::Relaxed);
}
pub fn dark() -> bool {
    DARK.load(Ordering::Relaxed) != 0
}
pub fn is_message(key: &str) -> bool {
    CATALOG.contains_key(key)
}
pub fn translate(language: Language, key: &str) -> &str {
    if language == Language::English {
        return key;
    }
    CATALOG
        .get(key)
        .map(|row| row[language as usize - 1].as_str())
        .unwrap_or(key)
}
pub fn tr(key: &str) -> &str {
    translate(
        Language::from_index(LANGUAGE.load(Ordering::Relaxed) as usize),
        key,
    )
}
pub fn message(key: &str, values: &[String]) -> String {
    let language = Language::from_index(LANGUAGE.load(Ordering::Relaxed) as usize);
    render(language, key, values)
}
pub fn render(language: Language, key: &str, values: &[String]) -> String {
    let template = translate(language, key);
    let mut output = String::new();
    let mut rest = template;
    for value in values {
        let Some((before, after)) = rest.split_once("{}") else {
            break;
        };
        output.push_str(before);
        output.push_str(value);
        rest = after;
    }
    output.push_str(rest);
    output
}
#[macro_export]
macro_rules! trf { ($key:literal $(, $value:expr)* $(,)?) => { $crate::i18n::message($key, &[$(format!("{}", $value)),*]) }; }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_translation_preserves_placeholders_and_has_text() {
        assert!(CATALOG.len() > 120);
        for (key, row) in CATALOG.iter() {
            for (index, value) in row.iter().enumerate() {
                assert!(
                    !value.trim().is_empty(),
                    "empty translation: {key} / {index}"
                );
                assert_eq!(
                    key.matches("{}").count(),
                    value.matches("{}").count(),
                    "placeholder mismatch: {key} / {index}"
                );
            }
        }
    }
    #[test]
    fn user_data_and_unknown_text_are_preserved() {
        for language in Language::ALL {
            assert_eq!(
                translate(language, "My личные notes.txt"),
                "My личные notes.txt"
            );
        }
        assert_eq!(translate(Language::French, "Settings"), "Paramètres");
    }
    #[test]
    fn translated_parameters_preserve_paths_braces_and_counts() {
        for language in Language::ALL {
            let text = render(language, "Error: {}", &["C:/Private/{name}.txt".into()]);
            assert!(text.ends_with("C:/Private/{name}.txt"));
            let text = render(
                language,
                "Downloading update: {}% ({} / {} bytes)",
                &["25".into(), "1024".into(), "4096".into()],
            );
            assert!(text.contains("25") && text.contains("1024") && text.contains("4096"));
            assert!(!text.contains("{}"));
        }
    }
    #[test]
    fn old_settings_keep_english_light_and_existing_options() {
        let settings: crate::model::Settings =
            serde_json::from_str(r#"{"opacity":220,"startup_update_check":false}"#).unwrap();
        assert_eq!(settings.language, Language::English);
        assert_eq!(settings.theme, Theme::Light);
        assert!(!settings.startup_update_check);
        let mut settings = settings;
        settings.language = Language::Italian;
        settings.theme = Theme::Dark;
        let restored: crate::model::Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.language, Language::Italian);
        assert_eq!(restored.theme, Theme::Dark);
        assert_eq!(restored.opacity, 220);
    }
}
