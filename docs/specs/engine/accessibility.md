# Accessibility

> **Status:** draft
> **Crate:** `amigo_render` + `amigo_ui` (tbd)
> **Priorität:** fehlt komplett

## Überblick

Barrierefreiheits-Features für Spieler mit Sehbehinderungen, motorischen
Einschränkungen oder kognitiven Unterschieden. Kein Pendant in aktuellen Specs.

## Scope (tbd)

- [ ] **Farbenblind-Modi**: Deuteranopie, Protanopie, Tritanopie — Post-Processing-Filter
- [ ] **Hoher Kontrast**: Konfigurierbare High-Contrast-UI-Themes
- [ ] **Text-Skalierung**: Skalierbare UI-Schriftgröße ([font-rendering](font-rendering.md))
- [ ] **Gamepad-Remapping**: Alle Aktionen neu belegbar ([engine/input](input.md))
- [ ] **Eingabe-Hilfen**: Sticky Keys, Hold-to-Activate, Toggle statt Hold
- [ ] **Screen Shake Reduktion**: Optionales Deaktivieren von Kamera-Shake
- [ ] **Subtitle-System**: Für Soundeffekte und Musik (optional)
- [ ] Accessibility-Einstellungen in `amigo.toml`
- [ ] Offene Fragen: Screenreader-Support? Welche Plattform-Standards (WCAG)?

## Referenzen

- [engine/rendering](rendering.md) → Post-Processing für Farbenblind-Shader
- [engine/input](input.md) → Input-Remapping
- [engine/ui](ui.md) → High-Contrast-Themes
- Game Accessibility Guidelines (gameaccessibilityguidelines.com)
