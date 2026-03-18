# Visual Novel

> **Status:** draft
> **Priorität:** nice-to-have

## Überblick

Visual Novel Engine auf Amigo: Charakter-Sprites mit Layer-System, Hintergrund-Wechsel,
Typewriter-Texteffekt, Entscheidungen und Branching-Narrativ. Baut stark auf
[dialogue](../engine/dialogue.md) auf.

## Scope (tbd)

- [ ] **Charakter-Sprite-Layer**: Körper, Gesicht, Outfit als separate Layer (blendbar)
- [ ] **Sprite-Emotionen**: Sprite-Varianten für Emotionen (Happy, Sad, Angry etc.)
- [ ] **Hintergrund-Wechsel**: Fade/Cut/Slide-Transitions zwischen Backgrounds
- [ ] **Typewriter-Effekt**: Character-by-Character-Text mit konfigurierbarer Geschwindigkeit
- [ ] **Textbox-Styling**: Name, Portrait, Textbox-Hintergrund konfigurierbar
- [ ] **Entscheidungen**: Multiple-Choice-Dialoge (via [dialogue](../engine/dialogue.md))
- [ ] **Musik & Ambience**: Szenen-basierte Audio-Wechsel ([engine/audio](../engine/audio.md))
- [ ] **ADV vs. NVL-Modus**: Adventure (Textbox unten) vs. Novel (Vollbild-Text)
- [ ] **Skip / Auto-Read**: Schnelles Durchlesen für bekannten Content
- [ ] **Save-Points**: Speichern an beliebigen Dialogue-Nodes
- [ ] Integration mit [dialogue](../engine/dialogue.md) als Story-Engine
- [ ] Offene Fragen: Ren'Py-kompatibles Skriptformat?

## Referenzen

- Ren'Py: De-facto VN-Engine als API-Referenz
- [engine/dialogue](../engine/dialogue.md) → Branching-Narrative
- [engine/tween](../engine/tween.md) → Sprite-Ein/Ausblendungen
