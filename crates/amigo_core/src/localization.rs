//! Localization (i18n) system for the Amigo Engine.
//!
//! Provides key-based string lookup from RON language files, variable
//! interpolation, plural form handling with language-specific rules, and a
//! fallback chain so that missing translations never result in blank text.

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// LocaleId
// ---------------------------------------------------------------------------

/// BCP-47 style locale identifier stored as a compact string.
/// Examples: `"en"`, `"de"`, `"ja"`, `"pt-BR"`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocaleId(String);

impl LocaleId {
    /// Create a new locale identifier from a BCP-47 code.
    pub fn new(code: &str) -> Self {
        Self(code.to_owned())
    }

    /// Return the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// PluralCategory & rules
// ---------------------------------------------------------------------------

/// Plural category as defined by Unicode CLDR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralCategory {
    /// The `zero` form (Arabic, Latvian, ...).
    Zero,
    /// The `one` (singular) form.
    One,
    /// The `two` (dual) form (Arabic, Welsh, ...).
    Two,
    /// The `few` form (Polish, Czech, ...).
    Few,
    /// The `many` form (Polish, Russian, ...).
    Many,
    /// The `other` (general plural) form -- always required.
    Other,
}

/// Function that maps a count to a [`PluralCategory`] for a given locale.
pub type PluralRuleFn = fn(count: u32) -> PluralCategory;

/// Built-in plural rules for common language families.
pub mod plural_rules {
    use super::PluralCategory;

    /// English, German, Dutch, etc.: 1 -> One, else Other.
    pub fn germanic(count: u32) -> PluralCategory {
        if count == 1 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        }
    }

    /// French, Portuguese (BR): 0-1 -> One, else Other.
    pub fn romance(count: u32) -> PluralCategory {
        if count <= 1 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        }
    }

    /// Polish: complex few/many rules based on last digits.
    pub fn polish(count: u32) -> PluralCategory {
        if count == 1 {
            return PluralCategory::One;
        }
        let last2 = count % 100;
        let last1 = count % 10;
        if (2..=4).contains(&last1) && !(12..=14).contains(&last2) {
            PluralCategory::Few
        } else {
            PluralCategory::Many
        }
    }

    /// Russian, Ukrainian: complex one/few/many rules.
    pub fn slavic(count: u32) -> PluralCategory {
        let last2 = count % 100;
        let last1 = count % 10;
        if last1 == 1 && last2 != 11 {
            PluralCategory::One
        } else if (2..=4).contains(&last1) && !(12..=14).contains(&last2) {
            PluralCategory::Few
        } else {
            PluralCategory::Many
        }
    }

    /// Arabic: zero/one/two/few/many/other (6 forms).
    pub fn arabic(count: u32) -> PluralCategory {
        let last2 = count % 100;
        match count {
            0 => PluralCategory::Zero,
            1 => PluralCategory::One,
            2 => PluralCategory::Two,
            _ if (3..=10).contains(&last2) => PluralCategory::Few,
            _ if (11..=99).contains(&last2) => PluralCategory::Many,
            _ => PluralCategory::Other,
        }
    }

    /// Japanese, Chinese, Korean, Turkish: no plural distinction.
    pub fn no_plural(_count: u32) -> PluralCategory {
        PluralCategory::Other
    }
}

// ---------------------------------------------------------------------------
// StringEntry
// ---------------------------------------------------------------------------

/// A single localized string entry, supporting optional plural forms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StringEntry {
    /// Simple string template, e.g. `"Hello, {name}!"`.
    Simple(String),
    /// Plural forms keyed by plural category.
    Plural {
        /// The `zero` form (optional).
        zero: Option<String>,
        /// The `one` (singular) form.
        one: String,
        /// The `two` (dual) form (optional).
        two: Option<String>,
        /// The `few` form (optional).
        few: Option<String>,
        /// The `many` form (optional).
        many: Option<String>,
        /// The `other` (general plural) form -- always required.
        other: String,
    },
}

// ---------------------------------------------------------------------------
// LocaleError
// ---------------------------------------------------------------------------

/// Errors that can occur during locale loading or lookup.
#[derive(Debug, thiserror::Error)]
pub enum LocaleError {
    /// An I/O error occurred while reading a locale file.
    #[error("IO error loading locale file: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse a RON locale file.
    #[error("RON parse error: {0}")]
    Parse(String),

    /// The requested locale has not been loaded.
    #[error("Locale not loaded: {0}")]
    NotLoaded(String),

    /// A key was not found in the given locale.
    #[error("Missing key '{key}' in locale '{locale}'")]
    MissingKey {
        /// The missing key.
        key: String,
        /// The locale that was searched.
        locale: String,
    },
}

// ---------------------------------------------------------------------------
// LocaleManager
// ---------------------------------------------------------------------------

/// Central localization manager.
///
/// Holds all loaded language tables and resolves string lookups with fallback.
pub struct LocaleManager {
    /// Currently active locale.
    current_locale: LocaleId,
    /// Default fallback locale (typically `"en"`).
    fallback: LocaleId,
    /// Loaded string tables: locale -> (key -> entry).
    tables: FxHashMap<LocaleId, FxHashMap<String, StringEntry>>,
    /// Plural rule function per locale.
    plural_rules: FxHashMap<LocaleId, PluralRuleFn>,
}

impl LocaleManager {
    /// Create a new manager with the given default/fallback locale.
    ///
    /// The fallback locale is also set as the initially active locale.
    pub fn new(fallback: LocaleId) -> Self {
        Self {
            current_locale: fallback.clone(),
            fallback,
            tables: FxHashMap::default(),
            plural_rules: FxHashMap::default(),
        }
    }

    // -- locale management --------------------------------------------------

    /// Set the active locale. Returns an error if the locale is not loaded.
    pub fn set_locale(&mut self, locale: LocaleId) -> Result<(), LocaleError> {
        if self.tables.contains_key(&locale) {
            self.current_locale = locale;
            Ok(())
        } else {
            Err(LocaleError::NotLoaded(locale.as_str().to_owned()))
        }
    }

    /// Get the currently active locale.
    pub fn get_locale(&self) -> &LocaleId {
        &self.current_locale
    }

    /// List all loaded locales.
    pub fn available_locales(&self) -> Vec<&LocaleId> {
        self.tables.keys().collect()
    }

    /// Check if a key exists in the active locale (ignoring fallback).
    pub fn has_key(&self, key: &str) -> bool {
        self.tables
            .get(&self.current_locale)
            .is_some_and(|t| t.contains_key(key))
    }

    // -- loading ------------------------------------------------------------

    /// Load a RON string table for a locale from a file on disk.
    ///
    /// The file should contain a RON map of `String -> StringEntry`, e.g.:
    ///
    /// ```ron
    /// {
    ///     "greeting": Simple("Hello, {name}!"),
    ///     "enemy": Plural(one: "{count} enemy", other: "{count} enemies",
    ///                      zero: None, two: None, few: None, many: None),
    /// }
    /// ```
    pub fn load_locale(&mut self, locale: LocaleId, path: &Path) -> Result<(), LocaleError> {
        let content = std::fs::read_to_string(path)?;
        let table: FxHashMap<String, StringEntry> =
            ron::from_str(&content).map_err(|e| LocaleError::Parse(e.to_string()))?;
        self.tables.insert(locale, table);
        Ok(())
    }

    /// Insert a string table directly (useful for tests and embedded data).
    pub fn insert_table(&mut self, locale: LocaleId, table: FxHashMap<String, StringEntry>) {
        self.tables.insert(locale, table);
    }

    // -- plural rules -------------------------------------------------------

    /// Register a custom plural rule function for a locale.
    ///
    /// If not registered, the built-in `germanic` rule is used as default.
    pub fn set_plural_rule(&mut self, locale: LocaleId, rule: PluralRuleFn) {
        self.plural_rules.insert(locale, rule);
    }

    // -- lookup -------------------------------------------------------------

    /// Look up a localized string by key.
    ///
    /// Fallback chain: active locale -> fallback locale -> raw key.
    pub fn t(&self, key: &str) -> String {
        if let Some(entry) = self.lookup(key) {
            match entry {
                StringEntry::Simple(s) => s.clone(),
                // For plural entries accessed via t(), return the `other` form.
                StringEntry::Plural { other, .. } => other.clone(),
            }
        } else {
            key.to_owned()
        }
    }

    /// Look up a localized string with variable interpolation.
    ///
    /// Variables in the template are denoted by `{name}`.
    /// `vars` is a slice of `(variable_name, value)` pairs.
    pub fn t_fmt(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let raw = self.t(key);
        Self::interpolate(&raw, vars)
    }

    /// Look up a plural-aware localized string.
    ///
    /// Selects the correct plural form based on `count` and the active
    /// locale's plural rules, then interpolates `{count}` automatically.
    pub fn t_plural(&self, key: &str, count: u32) -> String {
        self.t_plural_with(key, count, &[])
    }

    /// Plural lookup with additional variables beyond `count`.
    pub fn t_plural_with(&self, key: &str, count: u32, vars: &[(&str, &str)]) -> String {
        let template = if let Some(entry) = self.lookup(key) {
            match entry {
                StringEntry::Plural {
                    zero,
                    one,
                    two,
                    few,
                    many,
                    other,
                } => {
                    let category = self.plural_category(count);
                    match category {
                        PluralCategory::Zero => {
                            zero.as_deref().unwrap_or(other.as_str()).to_owned()
                        }
                        PluralCategory::One => one.clone(),
                        PluralCategory::Two => two.as_deref().unwrap_or(other.as_str()).to_owned(),
                        PluralCategory::Few => few.as_deref().unwrap_or(other.as_str()).to_owned(),
                        PluralCategory::Many => {
                            many.as_deref().unwrap_or(other.as_str()).to_owned()
                        }
                        PluralCategory::Other => other.clone(),
                    }
                }
                StringEntry::Simple(s) => s.clone(),
            }
        } else {
            return key.to_owned();
        };

        let count_str = count.to_string();
        // Build combined variable list: user vars + {count}.
        let mut all_vars: Vec<(&str, &str)> = vars.to_vec();
        all_vars.push(("count", &count_str));
        Self::interpolate(&template, &all_vars)
    }

    // -- internals ----------------------------------------------------------

    /// Look up a key in the active locale, falling back to the fallback locale.
    fn lookup(&self, key: &str) -> Option<&StringEntry> {
        self.tables
            .get(&self.current_locale)
            .and_then(|t| t.get(key))
            .or_else(|| self.tables.get(&self.fallback).and_then(|t| t.get(key)))
    }

    /// Determine the plural category for the active locale and count.
    fn plural_category(&self, count: u32) -> PluralCategory {
        let rule = self
            .plural_rules
            .get(&self.current_locale)
            .copied()
            .unwrap_or(plural_rules::germanic);
        rule(count)
    }

    /// Replace `{name}` placeholders in `template` using the given vars.
    ///
    /// Unresolved placeholders are left as-is.
    fn interpolate(template: &str, vars: &[(&str, &str)]) -> String {
        let mut result = String::with_capacity(template.len());
        let bytes = template.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if bytes[i] == b'{' {
                // Find closing brace.
                if let Some(end) = template[i + 1..].find('}') {
                    let name = &template[i + 1..i + 1 + end];
                    if let Some((_k, v)) = vars.iter().find(|(k, _)| *k == name) {
                        result.push_str(v);
                    } else {
                        // Leave unresolved placeholder as-is.
                        result.push('{');
                        result.push_str(name);
                        result.push('}');
                    }
                    i = i + 1 + end + 1; // skip past '}'
                } else {
                    result.push('{');
                    i += 1;
                }
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }

        result
    }
}

/// Shorthand macro for localized string lookup.
///
/// ```ignore
/// t!(mgr, "greeting");
/// t!(mgr, "greeting", "name" => "Player", "title" => "Knight");
/// ```
#[macro_export]
macro_rules! t {
    ($mgr:expr, $key:expr) => {
        $mgr.t($key)
    };
    ($mgr:expr, $key:expr, $($var:expr => $val:expr),+ $(,)?) => {
        $mgr.t_fmt($key, &[$(($var, $val)),+])
    };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a manager with English and German tables loaded in memory.
    fn test_manager() -> LocaleManager {
        let en = LocaleId::new("en");
        let de = LocaleId::new("de");
        let mut mgr = LocaleManager::new(en.clone());

        let mut en_table: FxHashMap<String, StringEntry> = FxHashMap::default();
        en_table.insert(
            "greeting".into(),
            StringEntry::Simple("Hello, {name}!".into()),
        );
        en_table.insert(
            "menu_start".into(),
            StringEntry::Simple("Start Game".into()),
        );
        en_table.insert(
            "enemy".into(),
            StringEntry::Plural {
                zero: None,
                one: "{count} enemy".into(),
                two: None,
                few: None,
                many: None,
                other: "{count} enemies".into(),
            },
        );
        en_table.insert(
            "coin".into(),
            StringEntry::Plural {
                zero: Some("no coins".into()),
                one: "{count} coin".into(),
                two: None,
                few: None,
                many: None,
                other: "{count} coins".into(),
            },
        );
        mgr.insert_table(en.clone(), en_table);

        let mut de_table: FxHashMap<String, StringEntry> = FxHashMap::default();
        de_table.insert(
            "greeting".into(),
            StringEntry::Simple("Hallo, {name}!".into()),
        );
        de_table.insert(
            "menu_start".into(),
            StringEntry::Simple("Spiel starten".into()),
        );
        de_table.insert(
            "enemy".into(),
            StringEntry::Plural {
                zero: None,
                one: "{count} Gegner".into(),
                two: None,
                few: None,
                many: None,
                other: "{count} Gegner".into(),
            },
        );
        mgr.insert_table(de.clone(), de_table);

        mgr.set_plural_rule(en, plural_rules::germanic);
        mgr.set_plural_rule(de.clone(), plural_rules::germanic);
        mgr
    }

    #[test]
    fn simple_lookup_returns_translated_string() {
        let mgr = test_manager();
        assert_eq!(mgr.t("menu_start"), "Start Game");
    }

    #[test]
    fn missing_key_returns_key_itself() {
        let mgr = test_manager();
        assert_eq!(mgr.t("nonexistent_key"), "nonexistent_key");
    }

    #[test]
    fn fallback_to_default_locale_when_key_missing() {
        let mut mgr = test_manager();
        mgr.set_locale(LocaleId::new("de")).unwrap();
        // "coin" only exists in English -- should fall back.
        assert_eq!(mgr.t_plural("coin", 5), "5 coins");
    }

    #[test]
    fn set_locale_switches_language() {
        let mut mgr = test_manager();
        mgr.set_locale(LocaleId::new("de")).unwrap();
        assert_eq!(mgr.t("menu_start"), "Spiel starten");
    }

    #[test]
    fn set_locale_rejects_unloaded_locale() {
        let mut mgr = test_manager();
        let result = mgr.set_locale(LocaleId::new("ja"));
        assert!(result.is_err());
    }

    #[test]
    fn interpolation_replaces_variables() {
        let mgr = test_manager();
        let result = mgr.t_fmt("greeting", &[("name", "Player")]);
        assert_eq!(result, "Hello, Player!");
    }

    #[test]
    fn interpolation_with_locale_switch() {
        let mut mgr = test_manager();
        mgr.set_locale(LocaleId::new("de")).unwrap();
        let result = mgr.t_fmt("greeting", &[("name", "Spieler")]);
        assert_eq!(result, "Hallo, Spieler!");
    }

    #[test]
    fn plural_singular_form() {
        let mgr = test_manager();
        assert_eq!(mgr.t_plural("enemy", 1), "1 enemy");
    }

    #[test]
    fn plural_other_form() {
        let mgr = test_manager();
        assert_eq!(mgr.t_plural("enemy", 5), "5 enemies");
    }

    #[test]
    fn plural_zero_form_when_available() {
        let mgr = test_manager();
        // "coin" has an explicit zero form.
        // germanic rule: 0 -> Other, so zero form is only used when the
        // plural rule returns Zero. For English (germanic), 0 maps to Other.
        assert_eq!(mgr.t_plural("coin", 0), "0 coins");
    }

    #[test]
    fn plural_with_arabic_zero() {
        let mut mgr = test_manager();
        let ar = LocaleId::new("ar");
        let mut ar_table: FxHashMap<String, StringEntry> = FxHashMap::default();
        ar_table.insert(
            "item".into(),
            StringEntry::Plural {
                zero: Some("no items".into()),
                one: "{count} item".into(),
                two: Some("{count} items (dual)".into()),
                few: Some("{count} items (few)".into()),
                many: Some("{count} items (many)".into()),
                other: "{count} items".into(),
            },
        );
        mgr.insert_table(ar.clone(), ar_table);
        mgr.set_plural_rule(ar.clone(), plural_rules::arabic);
        mgr.set_locale(ar).unwrap();

        assert_eq!(mgr.t_plural("item", 0), "no items");
        assert_eq!(mgr.t_plural("item", 2), "2 items (dual)");
        assert_eq!(mgr.t_plural("item", 5), "5 items (few)");
        assert_eq!(mgr.t_plural("item", 11), "11 items (many)");
    }

    #[test]
    fn t_macro_simple() {
        let mgr = test_manager();
        assert_eq!(t!(mgr, "menu_start"), "Start Game");
    }

    #[test]
    fn t_macro_with_vars() {
        let mgr = test_manager();
        assert_eq!(t!(mgr, "greeting", "name" => "Hero"), "Hello, Hero!");
    }

    #[test]
    fn has_key_checks_active_locale_only() {
        let mut mgr = test_manager();
        mgr.set_locale(LocaleId::new("de")).unwrap();
        assert!(mgr.has_key("greeting"));
        // "coin" only exists in English.
        assert!(!mgr.has_key("coin"));
    }

    #[test]
    fn get_locale_returns_current() {
        let mut mgr = test_manager();
        assert_eq!(mgr.get_locale().as_str(), "en");
        mgr.set_locale(LocaleId::new("de")).unwrap();
        assert_eq!(mgr.get_locale().as_str(), "de");
    }

    #[test]
    fn unresolved_placeholder_left_as_is() {
        let mgr = test_manager();
        // "greeting" has {name} but we pass no vars.
        let result = mgr.t("greeting");
        assert_eq!(result, "Hello, {name}!");
    }
}
