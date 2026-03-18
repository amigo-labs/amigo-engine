# Behavior Trees

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** nice-to-have

## Überblick

Formale Verhaltensbäume für KI-Entitäten als Alternative zu oder Ergänzung von
Utility-AI (bereits in [agents](agents.md)). Behavior Trees eignen sich besonders
für hierarchisch strukturierte, reaktive KI-Logik mit klarer Debuggbarkeit.

## Scope (tbd)

- [ ] **Node-Typen**: Sequence, Selector, Parallel, Decorator (Invert, Repeat, Timeout)
- [ ] **Leaf-Nodes**: Action (führt Code aus), Condition (prüft Zustand)
- [ ] Blackboard: Shared Memory zwischen Nodes einer Tree-Instanz
- [ ] RON-basierte Tree-Definition für Designer-Zugriff
- [ ] Async-kompatibel mit Fixed-Timestep (keine echten Async-Tasks)
- [ ] Debug-Overlay: Aktiver Pfad im Tree visualisiert
- [ ] Integration mit [agents](agents.md) Utility-AI (hybrid approach)
- [ ] Offene Fragen: Soll BT [agents](agents.md) ersetzen oder ergänzen?

## Referenzen

- [engine/agents](agents.md) → Utility-AI als aktueller Ansatz
- bonsai-bt (Rust BT library) als potentielle Dependency
- Halo / Halo 2 als Referenz für Behavior Tree KI
