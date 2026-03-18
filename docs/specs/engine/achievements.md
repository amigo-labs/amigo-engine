# Achievement System

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** nice-to-have

## Überblick

In-Game Achievement System mit optionaler Platform-Integration (Steam, itch.io).
Achievements werden über Event-basierte Trigger freigeschaltet und im Save-System
persistiert.

## Scope (tbd)

- [ ] Achievement-Definition in RON: ID, Name, Beschreibung, Icon, Bedingung
- [ ] **Trigger-System**: Event-basiert — Achievements registrieren Listener auf ECS-Events
- [ ] **Fortschritts-Achievements**: `progress(current, total)` für zählbare Ziele
- [ ] Persistenz via [save-load](save-load.md) (freigeschaltete Achievement-IDs)
- [ ] In-Game Popup (Toast-Notification via [engine/ui](ui.md))
- [ ] **Steam-Integration**: `steamworks-rs` Achievement-Unlock API (feature-flag)
- [ ] **itch.io-Integration**: Butler API oder Web-Trophy-System (tbd)
- [ ] Debug-Modus: Alle Achievements anzeigen / forciert freischalten
- [ ] Offene Fragen: Offline-First oder Platform-Sync beim Start?

## Referenzen

- [engine/save-load](save-load.md) → Achievement-Persistenz
- [engine/ui](ui.md) → Popup-Notifications
- steamworks-rs als Steam-Integration-Crate
