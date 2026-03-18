---
status: draft
last_updated: 2026-03-18
---

# Engine Tricks & Optimierungen -- Verweise

Zentrale Übersicht aller Optimierungen und Tricks der Engine. Kein doppelter Content -- jeder Eintrag verweist auf die Spec, in der das Thema beschrieben ist.

## Memory & Allocation

| Trick | Spec |
| --- | --- |
| Structure of Arrays (SoA) statt AoS für Cache-Effizienz | [memory-performance](memory-performance.md) → "Data-Oriented Design" |
| Arena Allocator (`bumpalo`) für per-Frame Temp-Daten | [memory-performance](memory-performance.md) → "Allocator Strategy" |
| Object Pool für Entities (Enemies, Projectiles, Particles) | [memory-performance](memory-performance.md) → "Allocator Strategy" |
| Standard Heap nur für langlebige Daten (Assets, Tilemap, Config) | [memory-performance](memory-performance.md) → "Allocator Strategy" |
| Capacity Hints per SparseSet, kein Realloc im Hot Path | [memory-performance](memory-performance.md) → "Pre-Allocation" |

## ECS

| Trick | Spec |
| --- | --- |
| SparseSet Storage: Dense Array (Cache) + Sparse Lookup (O(1)) | [core](core.md) → "ECS (SparseSet + Change Tracking)" |
| BitSet Change Tracking für Mutations/Insertions/Removals | [core](core.md) → "ECS (SparseSet + Change Tracking)" |
| Fixed-Point Q16.16 (I16F16) für deterministische Simulation | [core](core.md) → "Fixed-Point Arithmetic" |

## Rendering

| Trick | Spec |
| --- | --- |
| Chunk-basiertes Tilemap-Rendering (16x16) mit Render-Texture-Cache | [rendering](rendering.md) → "Tilemap Rendering" |
| Dirty Chunk Tracking -- nur geänderte Chunks neu rendern | [rendering](rendering.md) → "Tilemap Rendering" |
| Frustum Culling -- Chunks außerhalb der Kamera werden übersprungen | [rendering](rendering.md) → "Tilemap Rendering" |
| Sprite Batcher: Sortierung nach Atlas, ein Draw Call pro Atlas | [rendering](rendering.md) → "Sprite Batcher" |
| LOD nach Zoom-Level (Full → Reduced → Simplified → Icon) | [rendering](rendering.md) → "LOD for Zoom" |
| Texture Atlas Bin-Packing (ein Atlas = ein Draw Call) | [atlas](../assets/atlas.md) → "Atlas Pipeline" |

## Chunks & Streaming

| Trick | Spec |
| --- | --- |
| 32x32 Chunk Streaming nach Kamera-Nähe mit Hysterese-Band | [chunks](chunks.md) → "Chunk Streaming" |
| Spatial Hash (`FxHashMap<ChunkCoord, Chunk>`) für O(1) Chunk-Lookup | [chunks](chunks.md) → "Chunk Streaming" |
| Dirty Flag per Chunk für inkrementelle Serialisierung | [chunks](chunks.md) → "Chunk / Behavior" |

## Simulation & Scheduling

| Trick | Spec |
| --- | --- |
| Fixed Timestep (1/60s) für deterministische Simulation | [simulation](simulation.md) → "Fixed Timestep Game Loop" |
| Per-System Tick Intervals (teure Systeme laufen nur alle N Ticks) | [simulation](simulation.md) → "SimSystem / Tick Intervals" |
| Priority-basierte System-Ausführung | [simulation](simulation.md) → "Priority Ordering" |
| Spiral-of-Death Cap (`max_ticks_per_frame`) | [simulation](simulation.md) → "Spiral-of-Death Cap" |
| Double-Buffer Events (leben genau einen Tick) | [core](core.md) → "Event System" |

## Pathfinding

| Trick | Spec |
| --- | --- |
| Flow Fields: vorberechnetes Dijkstra-Feld, O(1) pro Entity | [pathfinding](pathfinding.md) → "Flow Fields" |
| Ein Flow Field für hunderte Agents zum selben Ziel | [pathfinding](pathfinding.md) → "Flow Fields for Mass Navigation" |

## Liquids

| Trick | Spec |
| --- | --- |
| Settled-Cell Tracking -- ruhende Zellen werden übersprungen | [liquids](liquids.md) → "Settled-Cell Optimization" |
| Wake Neighbors -- nur Nachbarn von geänderten Zellen aufwecken | [liquids](liquids.md) → "Wake Neighbors" |
| Gravity-First Flow (runter vor seitlich) | [liquids](liquids.md) → "Flow Priority" |
| Bottom-to-Top Processing (Schwerkraft in einem Pass korrekt) | [liquids](liquids.md) → "Processing Order" |

## Lighting

| Trick | Spec |
| --- | --- |
| BFS Flood-Fill -- nur Nachbarn enqueuen die heller würden | [lighting](lighting.md) → "Flood-Fill Propagation" |
| Max-per-Channel Blending statt Summe (kein Overflow) | [lighting](lighting.md) → "Additive Blending" |
| Dirty-Region inkrementelle Neuberechnung | [lighting](lighting.md) → "Incremental Recalculation" |
| Bilineare Interpolation zwischen Tiles (smooth ohne per-Pixel) | [lighting](lighting.md) → "Smooth Interpolation" |

## Particles

| Trick | Spec |
| --- | --- |
| Fixed-Capacity Object Pool pro Emitter (kein Alloc) | [particles](particles.md) → "Object Pool" |
| XorShift64 PRNG pro Emitter (kein System-Random) | [particles](particles.md) → "XorShift64 PRNG" |
| Lineare Interpolation über Lifetime statt per-Frame Berechnung | [particles](particles.md) → "Color/Size Interpolation" |

## Collision

| Trick | Spec |
| --- | --- |
| Tile-basiertes O(1) Lookup | [memory-performance](memory-performance.md) → "Collision Detection" |
| Spatial Hash Broad-Phase: O(n²) → O(n) | [memory-performance](memory-performance.md) → "Collision Detection" |
| AABB Collision Primitives | [memory-performance](memory-performance.md) → "Collision Detection" |

## Dynamic Tilemap

| Trick | Spec |
| --- | --- |
| Per-Chunk per-Layer Dirty Set (HashSet-Deduplizierung) | [dynamic-tilemap](dynamic-tilemap.md) → "Dirty Tracking" |
| Gravity Bottom-to-Top in einem Pass | [dynamic-tilemap](dynamic-tilemap.md) → "Gravity Step" |
| O(1) Tile-Property-Lookup via Dense Vec statt HashMap | [dynamic-tilemap](dynamic-tilemap.md) → "Tile Registry" |

## Save/Load

| Trick | Spec |
| --- | --- |
| CRC32 mit Compile-Time Lookup-Table (256 Einträge) | [save-load](save-load.md) → "CRC32 Integrity" |
| Rotierende Autosave-Slots (Modulo-Arithmetik) | [save-load](save-load.md) → "Autosave Rotation" |
| Safe Write: Daten zuerst, dann Metadaten (Crash-sicher) | [save-load](save-load.md) → "Safe Write Pattern" |
| Lazy Slot Scanning -- nur Metadaten laden | [save-load](save-load.md) → "Slot Listing" |

## Networking & Determinismus

| Trick | Spec |
| --- | --- |
| Serializable Commands → deterministische Replays | [networking](networking.md) → "Commands" |
| Voll serialisierbarer GameState (Save/Load/Replay gratis) | [networking](networking.md) → "GameState Serialization" |

## Camera

| Trick | Spec |
| --- | --- |
| Zoom-Level steuert LOD-Entscheidungen | [camera](camera.md) → "LOD Hint" |
| Zweites Viewport für Minimap (kein Duplikat) | [camera](camera.md) → "Minimap Camera" |

## Profiling

| Trick | Spec |
| --- | --- |
| Tracy Integration ab Tag 1 | [memory-performance](memory-performance.md) → "Profiling" |
| In-Game Debug Overlay (FPS, Entities, Draw Calls, Memory) | [memory-performance](memory-performance.md) → "Profiling" |
