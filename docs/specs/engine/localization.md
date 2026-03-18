# Localization (i18n)

> **Status:** draft
> **Crate:** `amigo_assets` (tbd)
> **Priorität:** nice-to-have

## Überblick

Internationalisierungs-System für Spieltexte. Schlüssel-basierte Texte in
TOML-Sprachdateien ermöglichen mehrsprachige Releases ohne Code-Änderungen.

## Scope (tbd)

- [ ] TOML-Sprachdateien (`assets/i18n/de.toml`, `en.toml`, etc.)
- [ ] `t!("key")` Makro oder `l10n.get("key")` API
- [ ] Variablen-Interpolation: `"Du hast {count} Münzen"` → `t!("coins", count = 5)`
- [ ] Plural-Formen: `t_plural!("enemy", count)` → "1 Feind" / "3 Feinde"
- [ ] Hot-Reload von Sprachdateien im Dev-Modus
- [ ] Fallback-Sprache bei fehlendem Key
- [ ] Integration mit [dialogue](dialogue.md) und [engine/ui](ui.md)
- [ ] Offene Fragen: RTL-Support (Arabisch, Hebräisch)? Schriftart-Wechsel per Sprache?

## Referenzen

- fluent-rs (Mozilla's Fluent localization system)
- [engine/assets](../assets/pipeline.md) → Asset-Loading für Sprachdateien
- [engine/ui](ui.md) → Text-Rendering als Abnehmer
