//! Localisation using Fluent (`.ftl`) catalogs shared with the Svelte UI.
//!
//! The English catalog is the source of truth. Missing messages fall back to
//! English; if both fail, the key itself is returned and recorded for
//! telemetry (feature P0-36).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use hr_core::{AppError, AppResult};
use serde::Serialize;
use unic_langid::LanguageIdentifier;

pub const FALLBACK_LOCALE: &str = "en-US";
/// Sentinel used by the settings model for "follow the operating system".
pub const SYSTEM_LOCALE: &str = "system";

/// A shipped locale. `id` matches the `.ftl` filename.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleInfo {
    pub id: &'static str,
    pub english_name: &'static str,
    pub native_name: &'static str,
    pub complete: bool,
}

struct CatalogSource {
    id: &'static str,
    english_name: &'static str,
    native_name: &'static str,
    source: &'static str,
    complete: bool,
}

const CATALOGS: &[CatalogSource] = &[
    CatalogSource {
        id: "en-US",
        english_name: "English (United States)",
        native_name: "English (United States)",
        source: include_str!("../../../locales/en-US.ftl"),
        complete: true,
    },
    CatalogSource {
        id: "zh-CN",
        english_name: "Chinese (Simplified)",
        native_name: "简体中文",
        source: include_str!("../../../locales/zh-CN.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "zh-TW",
        english_name: "Chinese (Traditional)",
        native_name: "繁體中文",
        source: include_str!("../../../locales/zh-TW.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "ja-JP",
        english_name: "Japanese",
        native_name: "日本語",
        source: include_str!("../../../locales/ja-JP.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "ru-RU",
        english_name: "Russian",
        native_name: "Русский",
        source: include_str!("../../../locales/ru-RU.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "de-DE",
        english_name: "German",
        native_name: "Deutsch",
        source: include_str!("../../../locales/de-DE.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "fr-FR",
        english_name: "French",
        native_name: "Français",
        source: include_str!("../../../locales/fr-FR.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "it-IT",
        english_name: "Italian",
        native_name: "Italiano",
        source: include_str!("../../../locales/it-IT.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "es-AR",
        english_name: "Spanish (Argentina)",
        native_name: "Español (Argentina)",
        source: include_str!("../../../locales/es-AR.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "gl-ES",
        english_name: "Galician",
        native_name: "Galego",
        source: include_str!("../../../locales/gl-ES.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "pl-PL",
        english_name: "Polish",
        native_name: "Polski",
        source: include_str!("../../../locales/pl-PL.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "pt-BR",
        english_name: "Portuguese (Brazil)",
        native_name: "Português (Brasil)",
        source: include_str!("../../../locales/pt-BR.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "pt-PT",
        english_name: "Portuguese (Portugal)",
        native_name: "Português (Portugal)",
        source: include_str!("../../../locales/pt-PT.ftl"),
        complete: false,
    },
    CatalogSource {
        id: "cs-CZ",
        english_name: "Czech",
        native_name: "Čeština",
        source: include_str!("../../../locales/cs-CZ.ftl"),
        complete: false,
    },
];

pub fn supported_locales() -> Vec<LocaleInfo> {
    CATALOGS
        .iter()
        .map(|catalog| LocaleInfo {
            id: catalog.id,
            english_name: catalog.english_name,
            native_name: catalog.native_name,
            complete: catalog.complete,
        })
        .collect()
}

pub fn canonical_locale(id: &str) -> Option<&'static str> {
    CATALOGS
        .iter()
        .find(|catalog| catalog.id.eq_ignore_ascii_case(id.trim()))
        .map(|catalog| catalog.id)
}

/// Picks the best supported locale for an explicit request and the system
/// locale. Explicit requests win; unsupported requests degrade by language
/// (and Chinese script), then to English.
pub fn negotiate(requested: Option<&str>, system: Option<&str>) -> String {
    if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        if !requested.eq_ignore_ascii_case(SYSTEM_LOCALE) {
            if let Some(canonical) = canonical_locale(requested) {
                return canonical.to_string();
            }
            if let Some(language_match) = match_language(requested) {
                return language_match.to_string();
            }
        }
    }
    if let Some(system) = system {
        if let Some(canonical) = canonical_locale(system) {
            return canonical.to_string();
        }
        if let Some(language_match) = match_language(system) {
            return language_match.to_string();
        }
    }
    FALLBACK_LOCALE.to_string()
}

fn match_language(tag: &str) -> Option<&'static str> {
    let identifier: LanguageIdentifier = tag.parse().ok()?;
    let language = identifier.language.as_str();
    if language == "zh" {
        let traditional = identifier
            .script
            .as_ref()
            .is_some_and(|script| script.as_str().eq_ignore_ascii_case("Hant"))
            || tag.to_ascii_lowercase().contains("tw")
            || tag.to_ascii_lowercase().contains("hk");
        return Some(if traditional { "zh-TW" } else { "zh-CN" });
    }
    CATALOGS
        .iter()
        .find(|catalog| {
            catalog
                .id
                .split('-')
                .next()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(language))
        })
        .map(|catalog| catalog.id)
}

pub struct I18n {
    bundles: HashMap<&'static str, FluentBundle<FluentResource>>,
    current: RwLock<&'static str>,
    system_locale: String,
    missing: Mutex<BTreeSet<String>>,
}

impl I18n {
    /// Loads every catalog. Construction fails only if a catalog cannot be
    /// parsed, which is a build/translation defect, not a runtime condition.
    pub fn new(requested: Option<&str>) -> AppResult<Self> {
        let system_locale = sys_locale::get_locale().unwrap_or_else(|| FALLBACK_LOCALE.to_string());
        Self::new_with_system_locale(requested, Some(&system_locale))
    }

    pub fn new_with_system_locale(
        requested: Option<&str>,
        system: Option<&str>,
    ) -> AppResult<Self> {
        let mut bundles = HashMap::with_capacity(CATALOGS.len());
        for catalog in CATALOGS {
            let language: LanguageIdentifier = catalog.id.parse().map_err(|error| {
                AppError::config(format!("invalid locale id {}: {error}", catalog.id))
            })?;
            let resource =
                FluentResource::try_new(catalog.source.to_string()).map_err(|(_, errors)| {
                    AppError::config(format!(
                        "catalog {} has {} Fluent parse error(s): {:?}",
                        catalog.id,
                        errors.len(),
                        errors
                    ))
                })?;
            let mut bundle: FluentBundle<FluentResource> =
                FluentBundle::new_concurrent(vec![language]);
            bundle.set_use_isolating(false);
            bundle.add_resource(resource).map_err(|errors| {
                AppError::config(format!(
                    "catalog {} could not be loaded: {errors:?}",
                    catalog.id
                ))
            })?;
            bundles.insert(catalog.id, bundle);
        }

        let current = negotiate(requested, system);
        let canonical = canonical_locale(&current)
            .ok_or_else(|| AppError::config(format!("unsupported locale {current}")))?;
        Ok(Self {
            bundles,
            current: RwLock::new(canonical),
            system_locale: system.unwrap_or(FALLBACK_LOCALE).to_string(),
            missing: Mutex::new(BTreeSet::new()),
        })
    }

    pub fn locale(&self) -> String {
        (*self.current.read().expect("i18n lock poisoned")).to_string()
    }

    pub fn system_locale(&self) -> &str {
        &self.system_locale
    }

    /// Switches the active catalog at runtime (P0-34). Returns the canonical id.
    pub fn set_locale(&self, locale: &str) -> AppResult<String> {
        let canonical = match canonical_locale(locale) {
            Some(canonical) => canonical,
            None => canonical_locale(&negotiate(Some(locale), Some(&self.system_locale)))
                .ok_or_else(|| AppError::config(format!("unsupported locale {locale}")))?,
        };
        *self.current.write().expect("i18n lock poisoned") = canonical;
        Ok(canonical.to_string())
    }

    /// Resolves `key` in the active locale, then English. Never panics.
    pub fn t(&self, key: &str) -> String {
        self.format(key, None)
    }

    /// Resolves `key` with string arguments.
    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, *value);
        }
        self.format(key, Some(&fluent_args))
    }

    fn format(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let current = *self.current.read().expect("i18n lock poisoned");
        if let Some(value) = self.format_in(current, key, args) {
            return value;
        }
        if current != FALLBACK_LOCALE
            && let Some(value) = self.format_in(FALLBACK_LOCALE, key, args)
        {
            return value;
        }
        self.missing
            .lock()
            .expect("i18n lock poisoned")
            .insert(key.to_string());
        tracing::warn!(key, locale = current, "missing translation key");
        key.to_string()
    }

    fn format_in(&self, locale: &str, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let bundle = self.bundles.get(locale)?;
        let message = bundle.get_message(&fluent_id(key))?;
        let pattern = message.value()?;
        let mut errors = Vec::new();
        let formatted = bundle.format_pattern(pattern, args, &mut errors);
        if !errors.is_empty() {
            tracing::warn!(key, locale, errors = ?errors, "translation formatting failed");
        }
        Some(formatted.into_owned())
    }

    /// Whether a locale defines the key itself (no fallback).
    pub fn is_translated(&self, locale: &str, key: &str) -> bool {
        canonical_locale(locale)
            .and_then(|canonical| self.bundles.get(canonical))
            .and_then(|bundle| bundle.get_message(&fluent_id(key)))
            .and_then(|message| message.value())
            .is_some()
    }

    /// Keys that were requested but exist in no catalog. Used by tests and the
    /// missing-key telemetry hook.
    pub fn missing_keys(&self) -> Vec<String> {
        self.missing
            .lock()
            .expect("i18n lock poisoned")
            .iter()
            .cloned()
            .collect()
    }
}

/// Fluent identifiers cannot contain dots; H-Remote keeps dotted keys in code.
fn fluent_id(key: &str) -> String {
    key.replace('.', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ships_fourteen_locales_including_english() {
        let locales = supported_locales();
        assert_eq!(locales.len(), 14);
        assert_eq!(locales[0].id, "en-US");
        assert!(locales.iter().any(|locale| locale.id == "pt-BR"));
        assert!(locales.iter().any(|locale| locale.id == "cs-CZ"));
        assert_eq!(locales.iter().filter(|locale| locale.complete).count(), 1);
    }

    #[test]
    fn negotiation_prefers_exact_then_language_then_english() {
        assert_eq!(negotiate(Some("fr-fr"), None), "fr-FR");
        assert_eq!(negotiate(Some("de-AT"), None), "de-DE");
        assert_eq!(negotiate(Some("pt"), Some("pt-BR")), "pt-BR");
        assert_eq!(negotiate(Some("en-GB"), None), "en-US");
        assert_eq!(negotiate(Some("xx-YY"), Some("ja-JP")), "ja-JP");
        assert_eq!(negotiate(Some("zh-Hans-CN"), None), "zh-CN");
        assert_eq!(negotiate(Some("zh-Hant-HK"), None), "zh-TW");
        assert_eq!(negotiate(Some("zh"), Some("zh-TW")), "zh-CN");
        assert_eq!(negotiate(Some(SYSTEM_LOCALE), Some("it-IT")), "it-IT");
        assert_eq!(negotiate(None, None), FALLBACK_LOCALE);
    }

    #[test]
    fn every_catalog_parses_and_is_usable() {
        let i18n = I18n::new_with_system_locale(None, Some(FALLBACK_LOCALE)).unwrap();
        for locale in supported_locales() {
            i18n.set_locale(locale.id).unwrap();
            assert_eq!(i18n.t("app.name"), "H-Remote", "locale {}", locale.id);
        }
    }

    #[test]
    fn scaffolded_locale_falls_back_to_english() {
        let i18n = I18n::new_with_system_locale(Some("de-DE"), None).unwrap();
        assert_eq!(i18n.locale(), "de-DE");
        assert_eq!(i18n.t("common.cancel"), "Cancel");
        assert!(!i18n.is_translated("de-DE", "common.cancel"));
        assert!(i18n.is_translated("en-US", "common.cancel"));
        assert!(i18n.missing_keys().is_empty());
    }

    #[test]
    fn missing_keys_are_returned_and_recorded() {
        let i18n = I18n::new_with_system_locale(None, None).unwrap();
        assert_eq!(i18n.t("does.not.exist"), "does.not.exist");
        assert_eq!(i18n.missing_keys(), vec!["does.not.exist".to_string()]);
    }

    #[test]
    fn interpolation_works_and_arguments_are_escaped_by_fluent() {
        let i18n = I18n::new_with_system_locale(None, None).unwrap();
        assert_eq!(i18n.t_args("common.seconds", &[("count", "5")]), "5 s");
        assert_eq!(
            i18n.t_args(
                "profile.notWritable",
                &[("path", "/x"), ("message", "denied")]
            ),
            "/x is not writable: denied"
        );
    }

    #[test]
    fn runtime_language_switch_does_not_require_reload() {
        let i18n = I18n::new_with_system_locale(None, None).unwrap();
        assert_eq!(i18n.t("common.cancel"), "Cancel");
        assert_eq!(i18n.set_locale("fr-FR").unwrap(), "fr-FR");
        assert_eq!(i18n.locale(), "fr-FR");
        assert_eq!(i18n.set_locale("not-a-locale").unwrap(), "en-US");
    }

    #[test]
    fn english_catalog_contains_phase_zero_surface_keys() {
        let i18n = I18n::new_with_system_locale(None, None).unwrap();
        for key in [
            "guidance.title",
            "settings.general",
            "general.startMinimized",
            "error.title",
            "capability.osKeyring",
            "logs.level.warn",
            "catalog.emptyHint",
        ] {
            assert!(i18n.is_translated("en-US", key), "missing {key}");
        }
    }
}
