# Architecture Decision Records

One file per decision, numbered in the order they were accepted. An ADR records
why a choice was made, not how the code works — the specs in `docs/specs/` and the
code's own docs cover that.

| ADR | Decision |
|-----|----------|
| [0001](0001-fixed-point-simulation.md) | Fixed-point math for the deterministic simulation layer |
| [0002](0002-sparse-set-ecs.md) | SparseSet ECS with dynamic components; also covers change detection and the optional `system_graph` parallel dispatch |
| [0003](0003-rollback-netcode.md) | Rollback netcode over deterministic lockstep state |
| [0009](0009-audio-bus-graph.md) | Node-based audio bus graph on Kira tracks |
| [0010](0010-reflection-based-inspector.md) | Reflection-based editor inspector |
| [0011](0011-retained-ui-layout.md) | Retained-mode layout over the immediate-mode UI |
| [0012](0012-hierarchical-scenes.md) | Hierarchical scene composition as an alternative to the flat stack |
| [0013](0013-ai-native-interfaces.md) | AI-native interfaces: stable surface, shared world context, measurable runs |

## Gaps in the numbering

0004–0008 were never written. The numbers were taken by decisions that ended up
recorded as specs under `docs/specs/` instead, and the gap is left rather than
renumbered so that references from code stay valid.

0009–0013 were written after the fact, from the implementations that already
referenced them: the code cited these numbers while the files did not exist. They
describe the decisions as the code makes them, which is why their dates are later
than the code they document.

## Referencing an ADR from code

Cite the number in a module doc comment, e.g.

```rust
//! Incremental, tick-based change detection for the ECS (ADR-0002).
```

Check the number actually matches the ADR's subject. Three references pointed at
ADR-0003 (rollback netcode) for change detection, which ADR-0002 documents.
