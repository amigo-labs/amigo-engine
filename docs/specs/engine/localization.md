---
status: spec
crate: amigo_assets
depends_on: ["assets/format"]
last_updated: 2026-03-18
---

# Localization (i18n)

## Purpose

Internationalization system for all player-facing text. Provides key-based
string lookup from TOML language files, variable interpolation, plural form
handling with language-specific rules, and a fallback chain so that missing
translations never result in blank text. Integrates with the dialogue system
for localized conversations and with the UI system for localized labels. Supports
hot-reload in dev builds so translators can iterate without restarting the game.

## Public API

### LocaleId

```rust
/// BCP-47 style locale identifier stored as a compact string.
/// Examples: "en", "de", "ja", "pt-BR".
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocaleId(String);

impl LocaleId {
    pub fn new(code: &str) -> Self;
    pub fn as_str(&self) -> &str;
}
```

### LocaleManager

```rust
/// Central localization manager. Holds all loaded language tables and
/// resolves string lookups with fallback.
pub struct LocaleManager {
    /// Currently active locale.
    active: LocaleId,
    /// Default fallback locale (typically "en").
    fallback: LocaleId,
    /// Loaded string tables: locale -> (key -> template string).
    tables: FxHashMap<LocaleId, FxHashMap<String, StringEntry>>,
    /// Plural rule function per locale.
    plural_rules: FxHashMap<LocaleId, PluralRuleFn>,
    /// Watcher handle for hot-reload (debug builds only).
    #[cfg(debug_assertions)]
    watcher: Option<FileWatcher>,
}

/// A single localized string entry, supporting optional plural forms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StringEntry {
    /// Simple string template: "Hello, {name}!"
    Simple(String),
    /// Plural forms keyed by plural category.
    Plural {
        zero: Option<String>,
        one: String,
        two: Option<String>,
        few: Option<String>,
        many: Option<String>,
        other: String,
    },
}

impl LocaleManager {
    /// Create a new manager with the given default/fallback locale.
    pub fn new(fallback: LocaleId) -> Self;

    /// Load a TOML language file for a locale.
    /// File format: flat key-value pairs or tables for plural forms.
    /// Path example: "assets/i18n/de.toml"
    pub fn load_locale(&mut self, locale: LocaleId, path: &Path) -> Result<(), LocaleError>;

    /// Load all .toml files from a directory, inferring locale from filename.
    /// e.g., "assets/i18n/en.toml" -> LocaleId("en")
    pub fn load_directory(&mut self, dir: &Path) -> Result<Vec<LocaleId>, LocaleError>;

    /// Set the active locale. Returns error if locale is not loaded.
    pub fn set_active(&mut self, locale: LocaleId) -> Result<(), LocaleError>;

    /// Get the currently active locale.
    pub fn active(&self) -> &LocaleId;

    /// List all loaded locales.
    pub fn available_locales(&self) -> Vec<&LocaleId>;

    /// Look up a localized string by key.
    /// Fallback chain: active locale -> fallback locale -> raw key.
    pub fn t(&self, key: &str) -> String;

    /// Look up a localized string with variable interpolation.
    /// Variables in the template are denoted by `{name}`.
    /// `vars` is a slice of (variable_name, value) pairs.
    pub fn t_with(&self, key: &str, vars: &[(&str, &str)]) -> String;

    /// Look up a plural-aware localized string.
    /// Selects the correct plural form based on `count` and the active
    /// locale's plural rules, then interpolates `{count}` automatically.
    pub fn t_plural(&self, key: &str, count: u32) -> String;

    /// Plural lookup with additional variables beyond `count`.
    pub fn t_plural_with(&self, key: &str, count: u32, vars: &[(&str, &str)]) -> String;

    /// Register a custom plural rule function for a locale.
    /// If not registered, the built-in CLDR-based rules are used.
    pub fn set_plural_rule(&mut self, locale: LocaleId, rule: PluralRuleFn);

    /// Check if a key exists in the active locale (ignoring fallback).
    pub fn has_key(&self, key: &str) -> bool;

    /// Reload all language files from disk (hot-reload in dev mode).
    #[cfg(debug_assertions)]
    pub fn reload(&mut self) -> Result<(), LocaleError>;

    /// Enable file watching for automatic hot-reload.
    #[cfg(debug_assertions)]
    pub fn enable_hot_reload(&mut self, dir: &Path);

    /// Poll for file changes and reload if needed.
    /// Call once per frame in dev builds.
    #[cfg(debug_assertions)]
    pub fn poll_hot_reload(&mut self);
}
```

### Plural Rules

```rust
/// Plural category as defined by Unicode CLDR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

/// Function that maps a count to a plural category for a given locale.
pub type PluralRuleFn = fn(count: u32) -> PluralCategory;

/// Built-in plural rules for common languages.
pub mod plural_rules {
    /// English, German, Dutch, etc.: 1 -> One, else Other.
    pub fn germanic(count: u32) -> PluralCategory;

    /// French, Portuguese (BR): 0-1 -> One, else Other.
    pub fn romance(count: u32) -> PluralCategory;

    /// Polish: complex few/many rules based on last digits.
    pub fn polish(count: u32) -> PluralCategory;

    /// Russian, Ukrainian: complex one/few/many rules.
    pub fn slavic(count: u32) -> PluralCategory;

    /// Arabic: zero/one/two/few/many/other (6 forms).
    pub fn arabic(count: u32) -> PluralCategory;

    /// Japanese, Chinese, Korean, Turkish: no plural distinction.
    pub fn no_plural(count: u32) -> PluralCategory;
}
```

### Errors

```rust
#[derive(Debug, Error)]
pub enum LocaleError {
    #[error("IO error loading locale file: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Parse(String),
    #[error("Locale not loaded: {0}")]
    NotLoaded(String),
    #[error("Missing key '{key}' in locale '{locale}'")]
    MissingKey { key: String, locale: String },
}
```

### Convenience Macro

```rust
/// Shorthand for `locale_manager.t_with(key, vars)`.
/// Usage: `t!(mgr, "greeting", "name" => "Player", "title" => "Knight")`
#[macro_export]
macro_rules! t {
    ($mgr:expr, $key:expr) => {
        $mgr.t($key)
    };
    ($mgr:expr, $key:expr, $($var:expr => $val:expr),+ $(,)?) => {
        $mgr.t_with($key, &[$(($var, $val)),+])
    };
}
```

## Behavior

- **TOML file format**: Each language file is a flat TOML document. Simple
  strings are plain key-value pairs. Plural forms use a sub-table:

  ```toml
  # en.toml
  greeting = "Hello, {name}!"
  menu_start = "Start Game"
  menu_quit = "Quit"

  [enemy]
  one = "{count} enemy"
  other = "{count} enemies"

  [coin]
  one = "{count} coin"
  other = "{count} coins"
  ```

- **Fallback chain**: When `t("key")` is called:
  1. Look up `key` in the active locale's table.
  2. If not found, look up in the fallback locale's table.
  3. If still not found, return the raw key string itself (e.g., `"menu_start"`).
  This ensures the game never displays blank text.

- **Variable interpolation**: Template strings contain `{variable_name}`
  placeholders. `t_with("greeting", &[("name", "Player")])` replaces `{name}`
  with `"Player"`. Unresolved placeholders are left as-is (visible in debug).

- **Plural resolution**: `t_plural("enemy", 3)` selects the plural form by:
  1. Looking up the plural rule function for the active locale.
  2. Calling `rule(3)` to get a `PluralCategory` (e.g., `Other` for English).
  3. Selecting the matching form from the `StringEntry::Plural` variants.
  4. Automatically injecting `{count}` as "3".

- **Hot-reload (debug only)**: When `enable_hot_reload()` is active,
  `poll_hot_reload()` checks a file watcher for changes to .toml files and
  reloads them. This is compiled out in release builds via `#[cfg(debug_assertions)]`.

- **Dialogue integration**: The dialogue system stores string keys instead of
  raw text. At display time, dialogue nodes call `locale_manager.t(key)` to
  resolve the localized text.

## Internal Design

- String tables are stored as `FxHashMap<String, StringEntry>` per locale.
  Typical game has 500-2000 keys -- lookup is O(1) via hash.
- TOML parsing uses the `toml` crate. Nested tables (for plurals) are detected
  by checking if a value is a table with known plural keys (one, other, etc.).
- Plural rules default to the `germanic` rule (covers English and German). Games
  register additional rules for their target languages at startup.
- Variable interpolation is a simple linear scan of the template string,
  replacing `{...}` sequences. No regex, no allocator pressure beyond the
  output String.
- The `FileWatcher` in debug mode uses `notify` crate (or simple polling with
  file modification timestamps if `notify` is not desired as a dependency).

## Non-Goals

- **Runtime translation / machine translation.** All strings are pre-authored
  by humans. No AI or MT at runtime.
- **Right-to-left layout.** RTL text rendering (Arabic, Hebrew) requires
  bidi algorithm support in the font renderer. This spec notes it as future
  work but does not design the layout system.
- **ICU MessageFormat.** The interpolation syntax is deliberately simple
  (`{name}` replacement). Full ICU MessageFormat with select/ordinal is
  over-engineered for a 2D game engine.
- **Compile-time key validation.** Keys are runtime strings. A linting tool
  could check for unused/missing keys, but that is a tooling concern, not
  engine API.
- **Font switching per locale.** Some languages (CJK) need different font
  files. Font selection is handled by the font rendering system, not the
  localization manager. The locale manager exposes `active()` so the font
  system can pick the right font.

## Open Questions

- Should the TOML format support nested namespaces (e.g., `[ui.menu]`) or
  stay flat with dot-separated keys (e.g., `ui.menu.start = "Start"`)?
- Is the `toml` crate an acceptable dependency, or should we use a simpler
  custom parser to reduce compile times?
- Should `t()` return `Cow<str>` instead of `String` to avoid allocation when
  no interpolation is needed?
- How should dialogue branching keys work? e.g., `dialogue.npc_01.line_03` --
  is there a convention the dialogue system should enforce?
- Should there be a `t_fmt` variant that returns a pre-allocated `PixelText`
  for direct UI rendering?

## Referenzen

- [engine/assets](../assets/pipeline.md) -- Asset loading pipeline
- [engine/dialogue](dialogue.md) -- Dialogue trees consume localized strings
- [engine/ui](ui.md) -- `pixel_text` renders localized text
- [engine/font-rendering](font-rendering.md) -- Font system, glyph atlas
- Unicode CLDR Plural Rules (https://cldr.unicode.org/index/cldr-spec/plural-rules)
- fluent-rs (Mozilla) as design reference (not a dependency)
