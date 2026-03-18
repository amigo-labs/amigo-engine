# Font Rendering (TTF/OTF)

> **Status:** draft
> **Crate:** `amigo_render`
> **Priorität:** fehlt komplett

## Überblick

Vollständiges TTF/OTF-Font-Rendering für Unicode-Text, skalierbare Schriften und
moderne Typografie. Ergänzt die bestehende Bitmap-Font-Unterstützung um
vektorbasierte Schriften mit Sub-Pixel-Rendering und Hinting.

## Scope (tbd)

- [ ] `fontdue`-Integration (bereits als Dependency gelistet, aber kein Spec)
- [ ] Font-Asset-Loading (TTF/OTF via `amigo_assets`)
- [ ] **Text-Layout**: Zeilenumbruch, Textausrichtung (L/C/R), Kerning
- [ ] **Rich Text**: Fett, Kursiv, Farb-Inline-Markup (tbd)
- [ ] Glyph-Atlas-Caching (Rasterisierte Glyphen im Atlas-System)
- [ ] Pixel-Font-Kompatibilität: Integer-Skalierung für Pixel-Art-Fonts
- [ ] SDF-Rendering (Signed Distance Field) für skalierbare Qualität (optional)
- [ ] Integration mit [engine/ui](ui.md) und [dialogue](dialogue.md)
- [ ] Offene Fragen: Soll `fontdue` durch `rustybuzz` + eigenes Rasterizer ersetzt werden?

## Referenzen

- fontdue (pure Rust TTF rasterizer) — bereits Dependency
- [engine/ui](ui.md) → Text-Rendering-Abnehmer
- [engine/assets](../assets/pipeline.md) → Font-Loading
