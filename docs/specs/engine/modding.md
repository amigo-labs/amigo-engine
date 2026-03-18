# Modding Support

> **Status:** draft
> **Crate:** `amigo_assets` (tbd)
> **Priorität:** fehlt komplett

## Überblick

Externes Modding ermöglicht Community-Inhalte ohne Neukompilierung: Asset-Packs,
Data-Overrides, neue Level und Game-Parameter. Mods werden als Verzeichnisse
mit überschreibenden Assets und RON-Dateien verteilt.

## Scope (tbd)

- [ ] **Mod-Discovery**: Mods-Verzeichnis scannen, Prioritäts-Reihenfolge
- [ ] **Asset-Override**: Mod-Assets überschreiben Base-Game-Assets per Pfad
- [ ] **Data-Override**: RON-Dateien aus Mods überschreiben/erweiterern Base-Data
- [ ] Mod-Manifest: `mod.toml` mit Name, Version, Author, Kompatibilitäts-Version
- [ ] **Aktivierungs-UI**: Mod-Manager im Hauptmenü
- [ ] Sandboxing: Mods dürfen nur Assets und Data, kein Rust-Code (kein Scripting-Layer!)
- [ ] Integration mit [engine/assets](../assets/pipeline.md) Asset-Loader
- [ ] Offene Fragen: Scripting-Layer für fortgeschrittene Mods? Workshop-Integration?

## Referenzen

- [engine/assets](../assets/pipeline.md) → Asset-Loading-Pipeline
- [config/amigo-toml](../config/amigo-toml.md) → Konfigurations-Schicht
- Stardew Valley Modding / SMAPI als Referenz-Modding-Framework
