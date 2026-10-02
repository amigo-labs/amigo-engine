# Audit backlog, 2026-10

Findings from the October 2026 audit that are **not** fixed yet, grouped by area
and sorted by severity. Each one was backed by a concrete failure scenario when
it was reported; re-check the code before fixing, since line numbers drift.

The findings fixed in the same pass are listed in `CHANGELOG.md` under
`[Unreleased]` and are omitted here.

Open: 208 (0 critical, 15 high, 107 medium, 86 low).


## Core: ECS, math, save, rewind, tasks, reflection

| ID | Severity | Finding | Location |
|---|---|---|---|
| core-ecs-6 | medium | Saving over an existing slot is not crash-safe and can destroy the previous save | `crates/amigo_core/src/save.rs:312-315` |
| core-ecs-11 | low | Legacy `events::Events::swap_all` never clears, so every event is re-delivered every other tick | `crates/amigo_core/src/events.rs:117-131` |
| core-ecs-12 | low | Cross-stage `after`/`before` constraints are silently ignored when they point the wrong way | `crates/amigo_core/src/ecs/schedule.rs:162-186` |
| core-ecs-14 | low | `SimulationRunner` stalls permanently on a NaN or negative custom speed | `crates/amigo_core/src/simulation.rs:137-178` |
| core-ecs-15 | low | `task_system`: `repeatable` is ignored, task order follows FxHashMap layout, and progress is stored as f32 | `crates/amigo_core/src/task_system.rs:44, 148-158, 168-172, 224-256` |
| core-ecs-16 | low | Reflect `type_path` is just the short name; `TypeRegistry::by_name` silently overwrites on collision | `crates/amigo_reflect_derive/src/lib.rs:316` |
| core-ecs-17 | low | `SimVec2::from_f32` panics on NaN or infinity (and on out-of-range values in debug) | `crates/amigo_core/src/math.rs:25-30` |
| core-ecs-19 | low | Save paths and features don't match A.7: macOS writes to `./saves`, LZ4 is missing, the autosave rotation index resets on restart | `crates/amigo_core/src/save.rs:129-151, 119, 290` |
| core-ecs-20 | low | The `change_detection` feature only adds types; nothing in the engine uses them | `crates/amigo_core/src/ecs/change_detection.rs` |
| core-ecs-21 | low | `FrameArena::alloc` / `alloc_slice_clone` leak the heap memory of `Drop` types every frame | `crates/amigo_core/src/frame_arena.rs:46-53` |
| core-ecs-22 | low | Avoidable `unsafe` and missing `// SAFETY:` comments *(partly fixed: SAFETY comments added in schedule.rs; world.rs:457 and the safe rewrites still open)* | `crates/amigo_core/src/ecs/schedule.rs:267, 315-316` |

## Core gameplay A: physics, collision, pathfinding, genres

| ID | Severity | Finding | Location |
|---|---|---|---|
| gpa-37 | high | libm transcendentals decide gameplay outcomes (bullet patterns, AoE cones, capsules, Fix tweens) | `crates/amigo_core/src/bullet_pattern.rs:211-297` |
| gpa-2 | medium | swept_aabb reports hits when the boxes never overlap on a zero-velocity axis | `crates/amigo_core/src/collision.rs:470-492` |
| gpa-4 | medium | raycast_tiles reports the wrong hit normal: it uses the *next* boundary comparison, not the axis just stepped | `crates/amigo_core/src/raycast.rs:126-133` |
| gpa-5 | medium | Slope tiles in raycast_tiles are sampled only at the ray's entry point; rays that enter above the surface pass through | `crates/amigo_core/src/raycast.rs:149-172` |
| gpa-6 | medium | FlowField direction pass ignores the corner-cutting rule that Dijkstra enforced; agents are steered diagonally around/into wall corners | `crates/amigo_core/src/pathfinding.rs:310-328` |
| gpa-7 | medium | NavAgent::distance_to_goal / update square raw Fix deltas -> overflow panic (debug) or wrapped garbage (release) | `crates/amigo_core/src/navigation.rs:242-251` |
| gpa-9 | medium | PathFollower speed is "segment fractions per call": speed depends on segment length and ignores dt / game-speed multiplier | `crates/amigo_core/src/pathfinding.rs:213-240` |
| gpa-10 | medium | AoE cone test returns false for targets exactly on the cone axis (acos of 1.0000001 = NaN) | `crates/amigo_core/src/combat.rs:404-417` |
| gpa-12 | medium | Shmup graze dedup keys on (pool index, kind) forever, so recycled bullet slots never graze again | `crates/amigo_core/src/shmup.rs:118-140` |
| gpa-17 | medium | Platformer dash with refresh_on_ground is never refreshed while staying on the ground | `crates/amigo_core/src/platformer.rs:265-276, 315-337` |
| gpa-18 | medium | Platformer max_jumps=1: walking off a ledge leaves a mid-air jump available | `crates/amigo_core/src/platformer.rs:255-266, 377-398` |
| gpa-20 | medium | Metroidvania: BossDefeated/HasItem gates are always "unsatisfied" in reachable_rooms and backtrack pins | `crates/amigo_core/src/metroidvania.rs:85-93` |
| gpa-21 | medium | A zero-size AABB still collides; BossRoomSystem "unseals" doors with Rect(0,0,0,0) and cannot re-seal them | `crates/amigo_core/src/collision.rs:20-24` |
| gpa-22 | medium | Fighting ComboTracker keeps the decayed damage scaling after a combo drops | `crates/amigo_core/src/fighting.rs:410-440` |
| gpa-23 | medium | Fighter stuck forever in HitStun/BlockStun when stun frames are 0 | `crates/amigo_core/src/fighting.rs:549-571` |
| gpa-28 | medium | TweenSequence: yoyo never reverses, and leftover dt is dropped at every step boundary | `crates/amigo_core/src/tween.rs:444-450 & 498-501` |
| gpa-29 | medium | Fixed-point splines overflow far inside the ADR coordinate envelope | `crates/amigo_core/src/spline.rs:117-127` |
| gpa-30 | medium | Timeline discrete events at time 0.0 never fire | `crates/amigo_core/src/timeline.rs:282-286, 298-302, 327-331` |
| gpa-31 | medium | Timeline skip()/seek() forward drop state-changing events (spawns/despawns), leaving cutscene actors in the world | `crates/amigo_core/src/timeline.rs:211-219` |
| gpa-34 | medium | Behavior tree Timeout/Repeat never reset after finishing, so their budget is consumed across all future executions | `crates/amigo_core/src/behavior_tree.rs:327-361` |
| gpa-35 | medium | amigo_steering cohesion/alignment sum absolute positions in Fix -> overflow with a handful of neighbours | `crates/amigo_steering/src/behaviors.rs:166-178` |
| gpa-38 | medium | The prelude collision/physics stack and the genre modules keep simulation state in f32 (ADR-0001 violation); physics resolution order depends on FxHashMap layout | `crates/amigo_core/src/collision.rs:7-18` |
| gpa-3 | low | PhysicsWorld::set_ccd_threshold is a no-op; swept tests are never used | `crates/amigo_core/src/physics.rs:110-113, 463-467` |
| gpa-13 | low | ExtendState::check_score hangs on equal last two thresholds and panics on unsorted ones | `crates/amigo_core/src/shmup.rs:497-525` |
| gpa-14 | low | Shmup chain score loses points through f32 multiplier truncation | `crates/amigo_core/src/shmup.rs:564-575` |
| gpa-15 | low | BulletPattern Aimed with count 0 underflows; seed 0 freezes Random patterns | `crates/amigo_core/src/bullet_pattern.rs:236-251, 290-297, 380-383` |
| gpa-19 | low | Platformer small edge cases: dash_duration 0 underflow, coyote/buffer off by one | `crates/amigo_core/src/platformer.rs:340-347, 262-283` |
| gpa-24 | low | Fighting hit detection ignores invincible/hurtbox_override/super_armor and uses the defender's facing for knockback | `crates/amigo_core/src/fighting.rs:597-611` |
| gpa-25 | low | InputBuffer::new(0) panics on the first push | `crates/amigo_core/src/fighting.rs:296-301` |
| gpa-27 | low | Turn combat: "immune" matchups still deal 1 damage; Battle RNG is hard-wired to seed 12345 | `crates/amigo_core/src/turn_combat.rs:96 & 109` |
| gpa-32 | low | monster_ai: a killed monster attacks and flees before it dies (DEAD transitions registered last, first match wins) | `crates/amigo_core/src/ai.rs:70-84` |
| gpa-33 | low | agents::Needs::most_urgent breaks ties by RandomState iteration order (differs per process) | `crates/amigo_core/src/agents.rs:63-66` |
| gpa-36 | low | Separation ignores coincident neighbours, so agents spawned on the same point never separate | `crates/amigo_steering/src/behaviors.rs:150-160` |
| gpa-39 | low | has_line_of_sight is asymmetric (Bresenham), so A can see B while B cannot see A | `crates/amigo_core/src/vision_ray.rs:30-69` |
| gpa-40 | low | Direction::from_delta octants are offset by 22.5 degrees | `crates/amigo_core/src/navigation.rs:46-63` |
| gpa-41 | low | find_path: SipHash HashMaps and stale heap entries that burn max_search | `crates/amigo_core/src/pathfinding.rs:121-174` |
| gpa-42 | low | Small API/edge notes in this area | `various` |

## Core gameplay B: TD, economy, inventory, dialog, genres

| ID | Severity | Finding | Location |
|---|---|---|---|
| gpb-10 | high | RTS formations use libm `f32::sin/cos` in simulation (cross-platform desync) | `crates/amigo_core/src/rts.rs:507-516` |
| gpb-11 | medium | `RunManager::new` seeds the simulation RNG from `SystemTime` when `seed` is None | `crates/amigo_core/src/roguelike.rs:1240-1252, 1212-1213` |
| gpb-12 | medium | `RunManager::floor_rng` depends on how many items were rolled before, contrary to its doc | `crates/amigo_core/src/roguelike.rs:1353-1358, 662-664` |
| gpb-13 | medium | `FloorEscalation` uses `f32::powi`, which is documented as non-deterministic | `crates/amigo_core/src/roguelike.rs:1151-1164` |
| gpb-14 | medium | Idle production rate depends on FxHashMap iteration order of upgrades | `crates/amigo_core/src/idle.rs:213-227` |
| gpb-15 | medium | `AchievementTracker::load_state` makes achievements added later unachievable | `crates/amigo_core/src/achievements.rs:323-328, 390-393` |
| gpb-16 | medium | Social deduction: once a non-critical sabotage expires, no further sabotage is possible; critical sabotage cannot be repaired | `crates/amigo_core/src/social_deduction.rs:277-303, 517-519` |
| gpb-17 | medium | City-builder resource-flow throughput is wrong in three ways | `crates/amigo_core/src/city_builder.rs:146-156, 246-262` |
| gpb-18 | medium | City-builder producers that need an input never receive it | `crates/amigo_core/src/city_builder.rs:182-207, 228-244` |
| gpb-19 | medium | Employed citizens never go home | `crates/amigo_core/src/city_builder.rs:1482-1508` |
| gpb-20 | medium | `PopulationSim::tick` recomputes global happiness for every citizen, then discards it | `crates/amigo_core/src/city_builder.rs:1461-1464, 850-860` |
| gpb-21 | medium | RTS slot assignment degenerates once distances exceed ~181 units | `crates/amigo_core/src/rts.rs:476-485` |
| gpb-22 | medium | WFC: pinned cells constrain nothing, and a malformed ruleset panics | `crates/amigo_core/src/procgen.rs:717-723, 762, 806-808, 817` |
| gpb-23 | medium | Colour-blind "correction" applies simulation matrices | `crates/amigo_core/src/accessibility.rs:78-107` |
| gpb-24 | medium | TD speed control: `SetSpeed(NaN)` freezes the game, and speed changes outcomes | `crates/amigo_core/src/game_state.rs:266-269, 279` |
| gpb-25 | medium | Tower data with empty `tiers` panics; a huge `cost` grants gold | `crates/amigo_core/src/game_state.rs:149-150` |
| gpb-26 | medium | `PlaceTower` takes gold without checking or occupying the tile | `crates/amigo_core/src/game_state.rs:142-169` |
| gpb-27 | medium | `MetaManager::load` silently discards a corrupt or outdated save, and the next save overwrites it | `crates/amigo_core/src/roguelike.rs:1396-1404` |
| gpb-28 | medium | Puzzle `LevelLoader` validation overflows; a crafted level passes and then panics | `crates/amigo_core/src/puzzle.rs:1449, 1467, 1482-1489` |
| gpb-29 | medium | Puzzle constraint types do nothing | `crates/amigo_core/src/puzzle.rs:1114-1163` |
| gpb-30 | medium | `BacklogSystem::new(0)` panics on the first push | `crates/amigo_core/src/visual_novel.rs:677-682` |
| gpb-31 | medium | `StatusEffects::apply` combines the strongest magnitude with the longest duration from different sources | `crates/amigo_core/src/status_effect.rs:87-109` |
| gpb-32 | medium | Cost checks miss duplicate entries for the same resource | `crates/amigo_core/src/crafting.rs:181-186` |
| gpb-34 | medium | `fog_of_war::update_visibility` supports a single observer only | `crates/amigo_core/src/fog_of_war.rs:133-159` |
| gpb-33 | low | `craft()` enforces neither station nor unlock, and reports InventoryFull wrongly | `crates/amigo_core/src/crafting.rs:189-205` |
| gpb-35 | low | VN `BranchingSystem` counters are invisible to conditions; `ChoiceMenu::can_confirm` rejects conditions that are met | `crates/amigo_core/src/visual_novel.rs:522-550, 616-621` |
| gpb-36 | low | Accessibility: shake cap is one-sided; subtitles pile up while disabled; config partly ignored | `crates/amigo_core/src/accessibility.rs:498-503, 220-225, 723-727, 6...` |
| gpb-37 | low | Voting: early close emits no events; skip ties go to the top choice; one-element "Tie" | `crates/amigo_core/src/voting.rs:178-183, 247-262, 279-289` |
| gpb-38 | low | Social deduction does not validate actions | `crates/amigo_core/src/social_deduction.rs:472-505, 402-469` |
| gpb-39 | low | Idle `prestige` wipes other prestige layers and meta upgrades | `crates/amigo_core/src/idle.rs:357-365` |
| gpb-40 | low | Loot and item-pool roll edge cases | `crates/amigo_core/src/loot.rs:246-258, 122, 212-217` |
| gpb-41 | low | Several hand-rolled xorshift RNGs never advance from seed 0 | `crates/amigo_core/src/puzzle.rs:484-492, 542-549` |
| gpb-42 | low | Achievements: counter overflow, LIFO toast queue, wall-clock unlock time | `crates/amigo_core/src/achievements.rs:210, 689, 421-426` |
| gpb-43 | low | Door auto-lock with duration 0 never unlocks | `crates/amigo_core/src/door.rs:155-158, 203` |
| gpb-44 | low | City-builder smaller issues: radius-0 NaN, `try_grow` repeats the same tile, disabled disasters keep damaging | `crates/amigo_core/src/city_builder.rs:928, 471, 1176-1212` |
| gpb-45 | low | `Economy.history` grows without bound | `crates/amigo_core/src/economy.rs:55, 86, 99` |
| gpb-46 | low | Deckbuilder `play_card` makes unknown cards free and allows play outside combat | `crates/amigo_core/src/deckbuilder.rs:341, 331-334` |
| gpb-47 | low | Tower targeting ties resolve inconsistently | `crates/amigo_core/src/tower.rs:48-55` |
| gpb-48 | low | RTS `ResourceStockpile::add` destroys resources when the stock is already above capacity | `crates/amigo_core/src/rts.rs:559-569` |
| gpb-49 | low | `RandomState` HashMaps produce non-reproducible serialized output | `crates/amigo_core/src/game_preset.rs:247, 326` |
| gpb-50 | low | Duplicated RNG, enum and generator implementations | `roguelike.rs:143-167, 621-671` |
| gpb-51 | low | `ScenePreset::default_systems` omits the genre modules | `crates/amigo_core/src/game_preset.rs:120-175` |
| gpb-52 | low | German comments and docs in fog_of_war.rs | `crates/amigo_core/src/fog_of_war.rs:1-18, 26-43, 62-132, 134-192` |

## Rendering and shaders

| ID | Severity | Finding | Location |
|---|---|---|---|
| render-5 | high | `capture_screenshot` panics on typical desktops because of a pipeline/attachment format mismatch | `crates/amigo_render/src/renderer.rs:636, 707` |
| render-6 | high | GPU broad phase panics for more than 4096 bodies (buffer and dispatch limits) | `crates/amigo_render/src/gpu_broad_phase.rs:48, 157-158, 198, 251` |
| render-8 | medium | Colour space: authored sRGB colours are written as linear into an sRGB pipeline | `crates/amigo_render/src/renderer.rs:139-144` |
| render-9 | medium | No virtual-resolution target: no pixel-perfect or integer scaling, no letterbox, aspect is stretched | `crates/amigo_render/src/camera.rs:492-509` |
| render-10 | medium | Texture lifecycle: no update or remove API, so font re-uploads and hot reloads leak GPU textures | `crates/amigo_render/src/renderer.rs:336-360` |
| render-11 | medium | Text drops every non-ASCII character | `crates/amigo_render/src/font.rs:84, 190` |
| render-12 | medium | GPU broad phase returns pairs in non-deterministic order (breaks ADR-0001) | `crates/amigo_render/src/gpu_broad_phase.wgsl:72` |
| render-13 | medium | GPU broad phase reads back the whole worst-case buffer every call (up to 128 MiB per tick) | `crates/amigo_render/src/gpu_broad_phase.rs:289-300` |
| render-14 | medium | Per-frame GPU buffer creation for sprite and UI geometry; per-frame bind groups and allocations | `crates/amigo_render/src/renderer.rs:396-420, 555-568` |
| render-15 | medium | Init: unhelpful panics, no software fallback, and `Limits::default()` rejects downlevel adapters | `crates/amigo_render/src/renderer.rs:112, 114-121, 125-136, 144, 152` |
| render-16 | medium | `CinematicPan` never ends: `is_panning()` stays true forever | `crates/amigo_render/src/camera.rs:404-415, 177-179` |
| render-17 | medium | `EdgePan` drift compounds every frame; smoothing and zoom overshoot (zoom can go negative) | `crates/amigo_render/src/camera.rs:316, 352-353, 421, 234, 242, 276` |
| render-18 | medium | Per-sprite shaders (`SpriteShader`) and additive particles are silently ignored | `crates/amigo_render/src/sprite_batcher.rs:9-12, 61` |
| render-19 | medium | Colourblind "correction" is actually a simulation, and the matrices are not Machado | `crates/amigo_render/src/post_process.rs:132-181, 285-291` |
| render-20 | medium | Texture creation panics on oversize (or zero-size) images instead of degrading | `crates/amigo_render/src/texture.rs:56-81` |
| render-21 | low | Lights beyond 64 are dropped in insertion order, without culling against the view | `crates/amigo_render/src/lighting_pipeline.rs:309-317` |
| render-22 | low | GPU broad-phase pair index evaluates `floor` exactly at row boundaries | `crates/amigo_render/src/gpu_broad_phase.wgsl:44-46` |
| render-23 | low | `collect_instance_data` picks the wrong sprites when a texture appears in several runs | `crates/amigo_render/src/instancing.rs:130-151` |
| render-24 | low | Dynamic atlas never reclaims space, and a same-name insert ignores the new size | `crates/amigo_render/src/dynamic_atlas.rs:124-127, 160-163` |
| render-25 | low | Post-processing applies effects in a fixed order regardless of the `Vec` order; vignette uses reversed `smoothstep` edges | `crates/amigo_render/src/post_process.rs:426-427, 592-599` |
| render-26 | low | (cross-area, A.6) The debug overlay is drawn in the world batch, so lighting and post-processing are applied to it | `crates/amigo_engine/src/engine.rs:1007-1053` |
| render-27 | low | (cross-area) The `art_style` config is never applied; `set_art_style` writes to the swapped-out camera | `crates/amigo_render/src/renderer.rs:363-366` |
| render-28 | low | Small items | |

## Audio, input, assets, tilemap, animation, scene, UI

| ID | Severity | Finding | Location |
|---|---|---|---|
| sub-17 | high | DynamicTileWorld::step_gravity deletes tiles at unloaded chunk borders and depends on HashMap iteration order | `crates/amigo_tilemap/src/dynamic.rs:268-307` |
| sub-3 | medium | AnimPlayer drops events on the final tick of every loop / Once playthrough | `crates/amigo_animation/src/lib.rs:101-117` |
| sub-4 | medium | Looping skeletal clip with duration 0 turns playback time into NaN permanently | `crates/amigo_animation/src/lib.rs:493-496` |
| sub-8 | medium | ModManager::compute_load_order sorts away the topological order (dependencies can load after dependents) | `crates/amigo_assets/src/modding.rs:339-379` |
| sub-11 | medium | parse_mml panics on `c0`/`l0` (division by zero) and on large octave values; produces invalid MIDI notes | `crates/amigo_assets/src/import.rs:527-530, 541-542, 549-553, 563-567` |
| sub-18 | medium | DynamicTileWorld::place_tile reports success for tiles placed in unloaded chunks | `crates/amigo_tilemap/src/dynamic.rs:172-195` |
| sub-19 | medium | LiquidMap leaks liquid into the void at its boundary; sideways flow is biased to the right | `crates/amigo_tilemap/src/liquid.rs:216-265, 274-342` |
| sub-20 | medium | 8-neighbor autotile mask includes diagonals unconditionally, so standard 47-tile blob rule sets miss | `crates/amigo_tilemap/src/auto_tile.rs:140-192` |
| sub-21 | medium | TileLayer/CollisionLayer/Chunk index into Vecs without validating length (panic on deserialized or edited data) | `crates/amigo_tilemap/src/lib.rs:72, 81-92, 114, 120-131` |
| sub-22 | medium | DrawContext::draw_tilemap_sprite divides by `columns` (panic when 0) and has no view culling | `crates/amigo_engine/src/context.rs:424-445` |
| sub-24 | medium | Keys/buttons get stuck down: no focus-loss handling, and egui can swallow the release | `crates/amigo_engine/src/engine.rs:716-722, 768-775` |
| sub-25 | medium | Gamepad support is not wired into the engine; analog triggers always read 0 on mapped controllers | `crates/amigo_input/src/gamepad.rs:204-212, 337-342` |
| sub-29 | medium | UiContext::text_input mixes byte and char indices (panics on non-ASCII), and the engine never feeds typed text at all | `crates/amigo_ui/src/lib.rs:403-420, 438` |
| sub-31 | medium | Retained layout: `Size::Auto` ("determined by children / content") resolves to 0 on the main axis, so auto-sized siblings overlap | `crates/amigo_ui/src/layout.rs:333-337, 273-285, 401-435` |
| sub-33 | medium | Volume settings are never applied to playback; amigo.toml [audio] volumes are ignored | `crates/amigo_audio/src/lib.rs:350-371, 378-427, 430-448` |
| sub-38 | medium | Tidal evaluator hangs on repeated empty groups (event cap never reached); parser has unbounded recursion | `crates/amigo_tidal_parser/src/eval.rs:183-207` |
| sub-1 | low | Pak flags (COMPRESSED/ENCRYPTED) are parsed but ignored; integrity never checked on load | `crates/amigo_assets/src/pak.rs:394-398, 442-461` |
| sub-2 | low | PakWriter silently writes a corrupt TOC for names > 65535 bytes / > u32::MAX entries | `crates/amigo_assets/src/pak.rs:172, 188-189` |
| sub-5 | low | AnimPlayer::update hangs forever for speed >= 2^24 or +inf | `crates/amigo_animation/src/lib.rs:105-109` |
| sub-6 | low | AnimStateMachine freezes the outgoing pose during a crossfade | `crates/amigo_animation/src/lib.rs:1044-1066, 1152-1159, 1181-1184` |
| sub-7 | low | load_aseprite ignores tag direction/repeat and accepts out-of-range tag frames | `crates/amigo_assets/src/aseprite.rs:65-93` |
| sub-9 | low | ModManager::deactivate cascades only one level; version ranges accept unknown syntax | `crates/amigo_assets/src/modding.rs:282-310, 414-446` |
| sub-10 | low | AIT format loses the real atlas stride; decode doesn't cross-check dimensions | `crates/amigo_assets/src/formats.rs:78-79, 106-135, 157-164` |
| sub-12 | low | LDTK import divides by a zero grid size from Entities layers / defaultGridSize; auto-tile px math overflows | `crates/amigo_assets/src/import.rs:307-312, 339-342, 381-382` |
| sub-13 | low | Tiled import keeps raw GIDs (flip flags, firstgid) and silently drops infinite/encoded layers | `crates/amigo_assets/src/import.rs:125-131` |
| sub-14 | low | FormatRegistry::load_directory: one bad file aborts the walk (order-dependent partial state); symlink loops recurse forever | `crates/amigo_assets/src/registry.rs:333-335, 462-497` |
| sub-15 | low | Hot reload: no event coalescing, so each save re-decodes and re-uploads the texture several times (leaking GPU textures) | `crates/amigo_assets/src/hot_reload.rs:46-53, 64-68` |
| sub-26 | low | ActionState computes edges per binding, not per action; RON input maps silently drop most key names | `crates/amigo_input/src/action_map.rs:238-303, 26-92, 125-131, 178-196` |
| sub-27 | low | HierarchicalSceneManager pops the wrong overlay when a non-last overlay returns Pop | `crates/amigo_scene/src/hierarchical.rs:271-293` |
| sub-30 | low | Overlapping immediate-mode widgets both receive the same click (no hot/active id) | `crates/amigo_ui/src/lib.rs:9-14` |
| sub-34 | low | BarClock/MusicConfig accept bpm<=0 and beats_per_bar=0 -> transitions never complete or `bar_number()+1` overflows | `crates/amigo_audio/src/lib.rs:533-572, 1082, 1094, 1158, 1427` |
| sub-35 | low | AdaptiveMusicEngine::load_from_ron drops all audio paths and stingers; SFX "random" variant/pitch is not random | `crates/amigo_audio/src/lib.rs:1423-1455, 1144-1148` |
| sub-36 | low | audio_graph buses receive no audio; bus volumes unclamped; Filter/Ducking/Crossfade nodes can't be built | `crates/amigo_audio/src/graph.rs:104-120, 167-177` |
| sub-37 | low | SpatialListener::from_camera_params panics on NaN or off-range camera coordinates | `crates/amigo_audio/src/spatial.rs:166-171` |
| sub-39 | low | format_amigo_tidal/save doesn't round-trip: drops fast/rev, applies the global slow to every stem, and writes `n "stack [...]"` which the parser rejects | `crates/amigo_tidal_parser/src/file.rs:165-187, 213-219` |
| sub-41 | low | PostProcessor::merge_short_rests truncates overlapping same-pitch notes and can underflow on unsorted input | `crates/amigo_audio_pipeline/src/postprocess.rs:56-77` |

## Engine loop, networking, API server, editor

| ID | Severity | Finding | Location |
|---|---|---|---|
| eng-2 | high | Unauthenticated API tolerates non-JSON lines -> any web page can drive it (cross-protocol) and write arbitrary files *(partly fixed: path confinement and connection close done; session token still open)* | `crates/amigo_api/src/server.rs:244-253` |
| eng-6 | high | UDP transport silently drops any message over ~300 bytes of command JSON (JSON-in-JSON, no size check, 1200-byte recv buffer) | `crates/amigo_net/src/protocol.rs:35-65` |
| eng-8 | high | Rollback: a late input older than the window is silently half-applied (permanent desync), no prediction stall | `crates/amigo_net/src/rollback.rs:187-209` |
| eng-3 | medium | API tick/step requests: silently truncated to 32 in windowed mode, unbounded (unkillable) in headless, overflow panic in debug | `crates/amigo_engine/src/engine.rs:861-864` |
| eng-4 | medium | `init_logging()` panics if any global tracing subscriber already exists; bad AMIGO_LOG silently ignored | `crates/amigo_debug/src/lib.rs:387-404` |
| eng-5 | medium | Headless tick is a different (hand-copied) tick: no Plugin::update, no ui.begin(), no font, real audio device | `crates/amigo_engine/src/engine.rs:326-351` |
| eng-7 | medium | UDP server: slots never expire and any source can claim them; client accepts packets from any address | `crates/amigo_net/src/udp.rs:20-23` |
| eng-9 | medium | Rollback: desync checksums are write-only and stale after resimulation; remote inputs are not validated | `crates/amigo_net/src/rollback.rs:96,219-222` |
| eng-10 | medium | Lobby state machine: failed join kicks you out of your room, create_room leaves ghost rooms, unready during countdown is ignored | `crates/amigo_net/src/lobby.rs:253-270` |
| eng-11 | medium | The in-engine editor (`--features editor`) is a non-functional shell: Save/Load/New do nothing, Undo/Redo drop the command | `crates/amigo_editor/src/egui_ui.rs:7` |
| eng-13 | medium | Init failures either panic or exit with status 0; proposed `EngineError` | `crates/amigo_engine/src/engine.rs:186-223` |
| eng-15 | medium | `PositionDeltaEncoder` assumes lossless in-order delivery: one lost packet permanently offsets or hides an entity | `crates/amigo_net/src/sync.rs:84-133` |
| eng-16 | medium | Most of the API's observation and query surface returns empty or `{"ok":true}` with no data | `crates/amigo_api/src/handler.rs:129` |
| eng-18 | medium | Config: a partial amigo.toml is discarded wholesale, half the documented keys do nothing, and `.config()` drops env overrides | `crates/amigo_engine/src/config.rs:10-62` |
| eng-20 | medium | `mouse_world_pos` only updates on CursorMoved, so it goes stale whenever the camera moves under a still mouse | `crates/amigo_engine/src/engine.rs:756-764` |
| eng-21 | medium | Gamepads are never polled by the engine, and keys stay "held" after focus loss | `crates/amigo_engine/src/context.rs:22-78` |
| eng-12 | low | `save_level` is not atomic and its HashMap fields make every save reorder the file | `crates/amigo_editor/src/lib.rs:372-376` |
| eng-14 | low | Shutdown skips `Game::on_exit`, and the API join can hang the process after the window closes | `crates/amigo_engine/src/stack.rs:117` |
| eng-17 | low | Unbounded API queues; each queued screenshot blocks the frame on a GPU readback | `crates/amigo_api/src/handler.rs:162-169` |

## CLI, MCP servers, ComfyUI tools, examples

| ID | Severity | Finding | Location |
|---|---|---|---|
| tools-1 | high | `amigo publish itch\|steam` uploads the whole cargo build dir and omits the assets | `tools/amigo_cli/src/main.rs:1784-1795` |
| tools-2 | high | `amigo connect --global` writes a file Claude Code never reads, and replaces all existing servers | `tools/amigo_cli/src/main.rs:1199-1240` |
| tools-4 | high | `amigo new` performs no name or target-directory validation: `amigo new --help`, `amigo new .`, existing dirs and invalid package names | `tools/amigo_cli/src/main.rs:375-400, 432-455, 457-468` |
| tools-8 | high | amigo-artgen generation tools are stubs that report success with fabricated paths | `tools/amigo_artgen/src/tools.rs:347-505` |
| tools-10 | high | Auto-starting ComfyUI cannot work, and if it did it would deadlock on a full stderr pipe | `tools/amigo_comfyui/src/lib.rs:476-492` |
| tools-3 | medium | `amigo connect` (project mode) clobbers `.mcp.json` instead of merging, and "Aborted" exits 0 | `tools/amigo_cli/src/main.rs:1246-1263` |
| tools-5 | medium | Hand-rolled flag parsing silently ignores unknown flags, `--flag=value` and missing values | `tools/amigo_cli/src/main.rs:1804-1808` |
| tools-6 | medium | `AMIGO_ENGINE_REV` goes stale and can pin new projects to a commit GitHub does not have | `tools/amigo_cli/build.rs:6-14` |
| tools-7 | medium | MCP server answers JSON-RPC notifications, uses the wrong `initialized` method name, and omits `id` in error responses | `tools/amigo_mcp/src/protocol.rs:12-23` |
| tools-9 | medium | amigo-artgen ignores `--server` and never reads project defaults | `tools/amigo_artgen/src/main.rs:43-68, 147` |
| tools-12 | medium | audiogen processing tools are stubs that report a fabricated `output` and no error | `tools/amigo_audiogen/src/tools.rs:1198-1324` |
| tools-15 | medium | `.mcp.json` launches three binaries that a fresh checkout does not have | `.mcp.json:1-16` |
| tools-16 | medium | `amigo pipeline batch` exits 0 when every file failed | `tools/amigo_cli/src/pipeline_cmd.rs:271-296` |
| tools-17 | medium | `amigo setup` is not isolated in `~/.amigo`, `--update` updates nothing, and `--check` misreports the GPU *(partly fixed: UV_NO_MODIFY_PATH set; the other setup issues remain)* | `tools/amigo_cli/src/setup.rs:183-193` |
| tools-13 | low | ComfyUI URL parsing breaks on a trailing slash, https or IPv6 | `tools/amigo_artgen/src/main.rs:190-204` |
| tools-14 | low | ComfyUI client: unbounded downloads and JSON bodies, and the completion check depends on non-empty outputs | `tools/amigo_comfyui/src/lib.rs:310-316 / 332-338` |
| tools-18 | low | `amigo dev` treats a JSON-RPC error reply to `dev.save_snapshot` as success | `tools/amigo_cli/src/main.rs:1454-1463, 1559-1588` |
| tools-19 | low | Help output goes to stderr, there is no `--version` or per-command help, and the help text has drifted from the docs | `tools/amigo_cli/src/main.rs:15-58` |
| tools-20 | low | Example headers: 8 of 9 have none, and `basic_game`'s header describes the template as it used to be | `examples/basic_game/src/main.rs:1-5` |
| tools-21 | low | `amigo pack` skips `.PNG` sprites without a word, and its output depends on directory order | `tools/amigo_cli/src/main.rs:955` |

## Documentation, specs, public API

| ID | Severity | Finding | Location |
|---|---|---|---|
| docs-4 | high | CONTRIBUTING "Running with feature flags" commands do not work in this workspace | `CONTRIBUTING.md:86-98` |
| docs-7 | high | Prelude glob re-exports leak whole crates (incl. 5 modules) into the "stable" API | `crates/amigo_engine/src/lib.rs:179, :209` |
| docs-1 | medium | Spec index status table omits 4 specs; "counts" comment refers to counts that do not exist | `docs/specs/index.md:5, docs/specs/index.md:245-324` |
| docs-3 | medium | index.md architecture/tech-stack sections contradict the code | `docs/specs/index.md:39, :83, :99-121, :383, :410` |
| docs-5 | medium | CONTRIBUTING layout, feature table and workflow are stale | `CONTRIBUTING.md:28-63, :65-75, :79-84, :24` |
| docs-6 | medium | 15 of 19 library crates have no crate-level docs; two put the crate doc on the wrong item | `crates/amigo_tidal_parser/src/lib.rs:1-20` |
| docs-8 | medium | Facade types are thinly documented; PluginContext's doc is attached to a private type alias | `crates/amigo_engine/src/engine.rs:31-35, :78-186` |
| docs-9 | medium | Stable-API hygiene: no `#[non_exhaustive]`, third-party types in the stable surface, `Component` is an implementable trait users cannot implement | `crates/amigo_engine/src/config.rs:11-21` |
| docs-11 | medium | False `done`: engine/builtin-profiler — the engine never feeds or shows the profiler | `docs/specs/engine/builtin-profiler.md:2` |
| docs-12 | medium | False `done`: engine/reflection — the inspector it exists for is unreachable | `docs/specs/engine/reflection.md:2, :12-14, :218-235` |
| docs-13 | medium | Specs marked `done` whose core promise other audits found broken (status should be `partial`) | `docs/specs/engine/input.md, engine/audio.md, engine/ui.md, engine/f...` |
| docs-14 | medium | tooling/cli spec contradicts the CLI it documents | `docs/specs/tooling/cli.md:14, :20-41, :315, :341` |
| docs-15 | medium | docs/tricks.md: every link is broken, and the page is German | `docs/tricks.md:8-157` |
| docs-17 | medium | Project `spec`/`develop` skills are the source of the drift: wrong paths, wrong status vocabulary, "write in German", no status/index update step | `.claude/skills/spec/SKILL.md:5, :36, :42, :56-59, :69, :79` |
| docs-2 | low | Wiki status legend omits `partial`; lists unused `spec` | `docs/wiki/Specifications.md:8` |
| docs-10 | low | Candidates for `#[diagnostic::on_unimplemented]` (none used today) | `crates/amigo_scene/src/lib.rs:31` |
| docs-16 | low | German text in a repo whose language is English | `see list` |
| docs-18 | low | README / CLAUDE.md small inaccuracies | `README.md:12, :24, :28-44` |
| docs-19 | low | Doc tests: the 7 compiled blocks look sound, but coverage is tiny and `ignore` hides runnable examples | `crates/amigo_core/src/scheduler.rs:23` |
