# GPU Instancing

> **Status:** draft
> **Crate:** `amigo_render`
> **Priorität:** fehlt komplett

## Überblick

Instanced Draw Calls für das Rendering großer Mengen gleichartiger Objekte
(Gras, Partikel-Sprites, Tilemap-Tiles, Feinde in Massen) mit minimalem
CPU-Overhead. Kein Spec vorhanden, obwohl render-relevant.

## Scope (tbd)

- [ ] `InstanceBuffer` für per-Instance-Daten (Transform, UV-Offset, Color-Tint)
- [ ] Automatisches Batching: Gleicher Atlas + Shader → ein instanced Draw Call
- [ ] Max-Instance-Count pro Batch (GPU-Buffer-Limit, konfigurierbar)
- [ ] **Sprite Batcher**: Migration von indexed zu instanced Rendering
- [ ] **Tilemap-Rendering**: Instanced Tiles statt einzelne Draw Calls
- [ ] Breakeven-Analyse: Ab wann lohnt Instancing vs. klassisches Batching?
- [ ] Integration mit [gpu-instancing tricks](tricks.md) Breakeven-Referenz
- [ ] wgpu `draw_indexed_indirect` vs. `draw_indirect` Tradeoffs
- [ ] Offene Fragen: Compute-Shader für Culling? Dynamic vs. Static Instance Buffer?

## Referenzen

- [engine/rendering](rendering.md) → Sprite Batcher als Basis
- [engine/particles](particles.md) → Partikel als Instancing-Kandidat
- wgpu Instancing Guide / Examples
