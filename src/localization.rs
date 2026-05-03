use std::collections::HashMap;
use std::env;
use std::sync::{OnceLock, RwLock};

use crate::settings::load_preferred_language_value;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLocale {
    pub code: &'static str,
    pub display_name: &'static str,
}

const SUPPORTED_LOCALES: [AppLocale; 12] = [
    AppLocale {
        code: "en",
        display_name: "English",
    },
    AppLocale {
        code: "zh-CN",
        display_name: "简体中文",
    },
    AppLocale {
        code: "ja",
        display_name: "日本語",
    },
    AppLocale {
        code: "ko",
        display_name: "한국어",
    },
    AppLocale {
        code: "fr",
        display_name: "Français",
    },
    AppLocale {
        code: "de",
        display_name: "Deutsch",
    },
    AppLocale {
        code: "es",
        display_name: "Español",
    },
    AppLocale {
        code: "it",
        display_name: "Italiano",
    },
    AppLocale {
        code: "pt-BR",
        display_name: "Português (Brasil)",
    },
    AppLocale {
        code: "ru",
        display_name: "Русский",
    },
    AppLocale {
        code: "ar",
        display_name: "العربية",
    },
    AppLocale {
        code: "tr",
        display_name: "Türkçe",
    },
];

static CURRENT_LOCALE: OnceLock<RwLock<&'static AppLocale>> = OnceLock::new();

pub fn supported_locales() -> &'static [AppLocale] {
    &SUPPORTED_LOCALES
}

pub fn english_locale() -> &'static AppLocale {
    &SUPPORTED_LOCALES[0]
}

pub fn chinese_locale() -> &'static AppLocale {
    &SUPPORTED_LOCALES[1]
}

pub fn locale_is_chinese(locale: &AppLocale) -> bool {
    locale.code.eq_ignore_ascii_case("zh-CN")
}

pub fn locale_for_code(value: &str) -> Option<&'static AppLocale> {
    let normalized = normalize_locale_code(value);
    if normalized.is_empty() {
        return None;
    }

    match normalized.as_str() {
        "en" | "en-us" | "en-gb" => Some(&SUPPORTED_LOCALES[0]),
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" | "zh-hans-cn" => Some(&SUPPORTED_LOCALES[1]),
        "ja" | "ja-jp" => Some(&SUPPORTED_LOCALES[2]),
        "ko" | "ko-kr" => Some(&SUPPORTED_LOCALES[3]),
        "fr" | "fr-fr" => Some(&SUPPORTED_LOCALES[4]),
        "de" | "de-de" => Some(&SUPPORTED_LOCALES[5]),
        "es" | "es-es" | "es-419" | "es-mx" => Some(&SUPPORTED_LOCALES[6]),
        "it" | "it-it" => Some(&SUPPORTED_LOCALES[7]),
        "pt" | "pt-br" => Some(&SUPPORTED_LOCALES[8]),
        "ru" | "ru-ru" => Some(&SUPPORTED_LOCALES[9]),
        "ar" | "ar-sa" | "ar-eg" => Some(&SUPPORTED_LOCALES[10]),
        "tr" | "tr-tr" => Some(&SUPPORTED_LOCALES[11]),
        _ => None,
    }
}

pub fn detect_app_locale() -> &'static AppLocale {
    if let Ok(value) = env::var("FASTZIP_LANG") {
        if let Some(locale) = locale_for_code(&value) {
            return locale;
        }
    }

    if let Some(value) = load_preferred_language_value() {
        if let Some(locale) = locale_for_code(&value) {
            return locale;
        }
    }

    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = env::var(key) {
            if let Some(locale) = locale_for_code(&value) {
                return locale;
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let primary_language = unsafe { GetUserDefaultUILanguage() } & 0x03ff;
        return match primary_language {
            0x0001 => &SUPPORTED_LOCALES[10],
            0x0004 => &SUPPORTED_LOCALES[1],
            0x0007 => &SUPPORTED_LOCALES[5],
            0x0009 => &SUPPORTED_LOCALES[0],
            0x000A => &SUPPORTED_LOCALES[6],
            0x000C => &SUPPORTED_LOCALES[4],
            0x0010 => &SUPPORTED_LOCALES[7],
            0x0011 => &SUPPORTED_LOCALES[2],
            0x0012 => &SUPPORTED_LOCALES[3],
            0x0016 => &SUPPORTED_LOCALES[8],
            0x0019 => &SUPPORTED_LOCALES[9],
            0x001F => &SUPPORTED_LOCALES[11],
            _ => english_locale(),
        };
    }

    #[allow(unreachable_code)]
    english_locale()
}

pub fn current_locale() -> &'static AppLocale {
    let lock = CURRENT_LOCALE.get_or_init(|| RwLock::new(detect_app_locale()));
    *lock.read().expect("FastZIP locale lock should be readable")
}

pub fn set_current_locale(locale: &'static AppLocale) {
    let lock = CURRENT_LOCALE.get_or_init(|| RwLock::new(locale));
    *lock
        .write()
        .expect("FastZIP locale lock should be writable") = locale;
}

pub fn set_current_locale_by_code(value: &str) -> &'static AppLocale {
    let locale = locale_for_code(value).unwrap_or_else(english_locale);
    set_current_locale(locale);
    locale
}

pub fn localize_message(english: &'static str, chinese: &'static str) -> &'static str {
    let locale = current_locale();
    if locale_is_chinese(locale) {
        return chinese;
    }
    if locale.code.eq_ignore_ascii_case("en") {
        return english;
    }

    lookup_translation(locale.code, english).unwrap_or(english)
}

fn normalize_locale_code(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

fn lookup_translation(locale_code: &str, english: &str) -> Option<&'static str> {
    catalog_for(locale_code).and_then(|catalog| catalog.get(english).copied())
}

fn catalog_for(locale_code: &str) -> Option<&'static HashMap<&'static str, &'static str>> {
    match normalize_locale_code(locale_code).as_str() {
        "ja" => Some(JA_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/ja.tsv")))),
        "ko" => Some(KO_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/ko.tsv")))),
        "fr" => Some(FR_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/fr.tsv")))),
        "de" => Some(DE_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/de.tsv")))),
        "es" => Some(ES_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/es.tsv")))),
        "it" => Some(IT_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/it.tsv")))),
        "pt-br" => {
            Some(PT_BR_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/pt-BR.tsv"))))
        }
        "ru" => Some(RU_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/ru.tsv")))),
        "ar" => Some(AR_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/ar.tsv")))),
        "tr" => Some(TR_CATALOG.get_or_init(|| parse_catalog(include_str!("../locales/tr.tsv")))),
        _ => None,
    }
}

fn parse_catalog(contents: &'static str) -> HashMap<&'static str, &'static str> {
    let mut catalog = HashMap::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((msgid, translation)) = line.split_once('\t') else {
            continue;
        };
        catalog.insert(msgid, translation);
    }

    catalog
}

static JA_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static KO_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static FR_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static DE_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static ES_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static IT_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static PT_BR_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static RU_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static AR_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static TR_CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
