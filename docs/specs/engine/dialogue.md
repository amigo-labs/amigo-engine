# Dialogue System

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** nice-to-have

## Überblick

Branching-Dialogue-System für Tutorials, Story-Elemente und NPC-Interaktionen.
Unterstützt bedingte Zweige, Variablen-Checks und Aktions-Trigger während
Dialogen (z.B. Item-Vergabe, Quest-Fortschritt).

## Scope (tbd)

- [ ] Dialogue-Graph in RON: Nodes mit Text, Antwort-Optionen, Bedingungen
- [ ] **Ink-kompatibles Format** oder eigenes RON-Format (tbd)
- [ ] Variablen-Binding: Dialogue-Bedingungen prüfen Game-State
- [ ] Aktions-Trigger: Dialogue-Nodes können ECS-Events feuern
- [ ] Typewriter-Effekt (Character-by-Character-Rendering)
- [ ] Portrait-System (Charakter-Bild neben Text)
- [ ] Integration mit [engine/ui](ui.md) für Dialogue-Box-Rendering
- [ ] Lokalisierungs-Hook für [localization](localization.md)
- [ ] Offene Fragen: Ink-Kompatibilität oder eigenes Format?

## Referenzen

- Inkle's Ink Scripting Language als Branching-Referenz
- [engine/ui](ui.md) → Dialogue-Box als UI-Element
- [gametypes/visual-novel](../gametypes/visual-novel.md) → Erweiterter Use Case
