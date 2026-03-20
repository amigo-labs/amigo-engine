---
number: "0013"
title: AI-Native Engine Vision & Public API Strategy
status: proposed
date: 2026-03-20
---

# ADR-0013: AI-Native Engine Vision & Public API Strategy

## Status

proposed

## Context

Amigo Engine is a pixel-art game engine built in Pure Rust with 62 implemented specs covering ECS, rendering, audio, networking, 8 game-type presets, AI asset pipelines (ComfyUI, ACE-Step), and MCP-based Claude Code integration. The engine already declares "AI-native development" as a core principle (`docs/specs/index.md`).

However, the current specs treat AI integration as a tooling concern — MCP tools that observe and control the engine externally. As AI capabilities grow (code generation, asset creation, music composition, story writing), the engine's role shifts from "developer tool" to "AI-native creative platform." This ADR captures the expanded vision and the architectural consequences.

Additionally, the engine is intended to be a **public product** — other developers (not just amigo-labs) should be able to build games with it. This requires stable APIs, semantic versioning, documentation, and a clean engine/game separation.

### Current State

**What exists:**
- 16 engine crates + 4 tool crates in a Cargo workspace (`Cargo.toml`)
- `amigo_api` (`crates/amigo_api/`): JSON-RPC IPC interface for AI agents
- `amigo_mcp` (`tools/amigo_mcp/`): MCP server wrapping `amigo_api` for Claude Code
- `amigo_artgen` (`tools/amigo_artgen/`): ComfyUI art generation via MCP
- `amigo_audiogen` (`tools/amigo_audiogen/`): ACE-Step/AudioGen music + SFX via MCP
- Headless mode for simulation without window
- Screenshot-based visual feedback
- Command-based architecture enabling multiplayer, replays, and AI control through the same interface
- `amigo_net` (`crates/amigo_net/`): Lockstep over UDP, lobby system, replay, desync detection

**What's missing:**
- No formal "public engine" strategy (versioning, API stability guarantees, onboarding)
- No AI evaluation loop (Claude generates but cannot assess quality)
- No cross-modal coherence system (art, music, story generated independently)
- No preview workflow (Claude executes changes directly, no "show me first")
- No intent-based generation interface ("make it tense" → concrete parameters)
- No parallel simulation for AI-driven balance testing
- Multiplayer assumes dedicated or self-hosted server; no explicit peer-to-peer host model documented

## Decision

We extend the engine vision along two axes: **Public Engine** and **AI-Native Creative Platform**. These are not separate initiatives — a public engine that works well with AI is the product.

### 1. Public Engine Strategy

The engine becomes a product that external developers use via `cargo add amigo_engine`.

**Stable API Surface:**
- `amigo_engine` re-exports a curated public API (already exists, needs stabilization)
- Semantic versioning: breaking changes require major version bumps
- Public types and functions documented with rustdoc
- Internal implementation details behind `pub(crate)` or `#[doc(hidden)]`

**Modular Opt-In:**
- Core engine (`amigo_engine`) provides ECS, rendering, input, assets, scene management
- Genre systems are opt-in features or separate crates:
  ```toml
  [dependencies]
  amigo_engine = { version = "0.1", features = ["platformer", "audio", "editor"] }
  ```
- Not every game needs physics, networking, dialogue, or particles
- Feature flags already exist for `editor` and `api`; extend pattern to genre modules

**Onboarding:**
- `cargo install amigo-cli` → `amigo new my-game` → `amigo run`
- Starter template generates a minimal working game
- Getting Started guide already exists (`docs/wiki/Getting-Started.md`), needs expansion

**Engine/Game Separation:**
- Engine code lives in `amigo-engine` repository
- Game code lives in separate repositories, depending on `amigo_engine` as a crate
- The `examples/` directory serves as reference implementations
- No game-specific types (TowerData, WaveConfig) in engine crates

### 2. AI-Assisted Development Workflow

Claude operates as **advisor**: it researches, proposes, and implements — but the developer decides.

**Workflow Pattern:**
```
Developer: "Create a level for World 2"
Claude:    "I suggest an ice-themed level with 5 rooms:
            - Linear layout with optional secret room
            - New enemy: Snowball Roller (patrol, medium)
            - Platforming challenge using sliding ice physics
            - Color palette: #1a1a2e, #4a6fa5, #c4d7e0
            Want me to proceed or adjust?"
Developer: "No ice puzzle, make it a combat arena instead"
Claude:    *implements adjusted version*
```

This is a workflow convention, not an engine feature. The MCP tools already support this — Claude chooses to preview before executing.

**Preview-Capable MCP Tools:**
- `preview_level(config)` → returns a screenshot of the proposed level without committing it
- `preview_palette(colors)` → renders sample sprites with the proposed palette
- `diff_levels(a, b)` → side-by-side comparison of two level versions
- These are extensions to `amigo_mcp`, not new crates

### 3. AI Evaluation System

Claude can objectively evaluate certain aspects of a game. The engine provides the data; Claude interprets it.

**What Claude Evaluates (Objective):**
- **Visual readability**: Screenshot analysis — "player sprite has insufficient contrast against background"
- **Level reachability**: Pathfinding check — "tile at (12, 8) is unreachable from spawn"
- **Balance metrics**: Simulation output — "73% death rate at Boss 2 is above target 40%"
- **Consistency**: Cross-reference checks — "enemy speed 5.0 in World 1 but 2.0 in World 3 without justification"
- **Code quality**: Clippy, tests, coverage

**What Humans Evaluate (Subjective):**
- Game feel — does the jump feel "tight"?
- Emotional impact — is this moment tense, funny, sad?
- Fun — is this actually enjoyable to play?
- Creative direction — does this match the vision?

**Engine Support for Evaluation:**
- `amigo_api` already provides screenshot export and event streaming
- Add: metrics collection during headless simulation (death locations, completion times, resource usage per tick)
- Add: reachability analysis tool (pathfind from spawn to every tile/entity, report unreachable)
- Add: parallel headless runs with different behavioral profiles (cautious player, speedrunner, explorer)

### 4. Cross-Modal Coherence (World Context System)

When Claude generates art, music, and story independently, they may not match. A world context system provides shared constraints.

**World Context Definition (RON):**
```ron
WorldContext(
    name: "Frozen Peaks",
    biome: Ice,
    mood: Tense,
    era: Medieval,
    color_palette: Palette(
        primary: "#1a1a2e",
        secondary: "#4a6fa5",
        accent: "#c4d7e0",
        danger: "#ff4444",
    ),
    music_style: MusicStyle(
        genre: Orchestral,
        tempo_range: (80, 110),
        key: Minor,
        instruments: ["strings", "choir", "timpani"],
    ),
    visual_style: VisualStyle(
        art_type: PixelArt,
        tile_size: 16,
        lighting: Dim,
        weather: Snow,
    ),
)
```

- Artgen receives `color_palette` and `visual_style` as constraints
- Audiogen receives `music_style` as constraints
- Dialogue generation receives `mood` and `era` as tone guides
- Claude checks coherence: "Generated sprite uses colors outside world palette"

This is a **data format** living in game repositories, not engine code. The engine provides the `WorldContext` struct and passes it to generation tools.

### 5. Multiplayer: Peer-to-Peer with Host

Clarifies and extends the existing `amigo_net` architecture for the target use case: cooperative/turn-based multiplayer without dedicated servers.

**Model:**
```
Player 1 (Host)          Player 2            Player 3
┌──────────────┐       ┌──────────┐       ┌──────────┐
│ Server+Client │◄─────│  Client  │       │  Client  │
│               │◄─────│          │       │          │
│               │─────►│          │       │          │
└──────────────┘       └──────────┘       └──────────┘
```

- Player 1 creates lobby → becomes **Host** (runs server + client in same process)
- Other players connect via lobby code or direct IP
- Host is authoritative — validates all commands, broadcasts state
- No dedicated server infrastructure required
- Lockstep protocol is sufficient (no shooter-like latency requirements)
- ~15-30 Hz sync rate, latency-tolerant (50-100ms imperceptible for target genres)
- If Host disconnects → game ends (host migration is a future enhancement)
- NAT traversal via UDP hole-punching; relay server as optional fallback

This aligns with the existing `amigo_net` implementation (`LobbyManager`, `Transport` trait, lockstep protocol). The key addition is making the "host as server" pattern explicit and documenting that no dedicated server is needed.

### 6. Scalable Quality

The engine does not enforce a quality bar. Quality depends on how much time and effort the developer (human + AI) invests.

**Minimum Viable Game:**
- Sprites can be placeholder rectangles
- Audio can be absent (feature-gated)
- Single level, no story
- `amigo new --minimal` generates the smallest possible game

**Polished Game:**
- AI-generated pixel art with coherent palettes
- Adaptive music with vertical layering
- Multiple worlds with consistent themes
- Dialogue, cutscenes, achievements
- Multiplayer

The engine supports both by making every system optional and providing sensible defaults.

### Alternatives Considered

**1. Scripting Layer (Lua/Rhai) instead of Rust-only:**
Rejected. A scripting layer adds a second language, a second error surface, and a second debugging experience. Claude writes Rust directly — the compiler is the feedback loop. Hot-reload of RON data covers the iteration speed need for game configuration. Behavior trees as RON data cover simple AI patterns.

**2. Runtime Generation (procedural content during gameplay):**
Rejected for now. Build-time generation is simpler, more controllable, and doesn't require generative AI in the shipping game binary. The engine provides procedural generation tools (`engine/procedural` spec) for developers who want it, but AI-generated content is created at dev-time via MCP tools.

**3. Dedicated Server Model for Multiplayer:**
Rejected. Target genres (co-op platformer, RPG, puzzle, tower defense) don't need dedicated servers. Peer-to-peer with host eliminates server costs and infrastructure complexity. If competitive multiplayer becomes a goal, dedicated servers can be added as an optional deployment mode.

## Migration Path

This ADR covers vision and architecture direction. Implementation is broken into phases that extend existing specs.

1. **Document World Context format** — Add `WorldContext` struct definition to `config/data-formats` spec. Add example RON files. Verify: RON parses correctly in a test.
2. **Extend MCP tools with preview capabilities** — Add `preview_level`, `preview_palette`, `diff_levels` to `amigo_mcp`. Verify: Claude can request a preview and receive a screenshot without committing changes.
3. **Add metrics collection to headless mode** — Extend `amigo_api` to collect death positions, completion time, resource usage during simulation runs. Verify: headless run produces metrics JSON.
4. **Add reachability analysis tool** — Use existing pathfinding (`amigo_core` A*) to verify all entities/exits are reachable from spawn. Verify: detects intentionally unreachable tile in test map.
5. (rough) Stabilize public API surface — audit `amigo_engine` re-exports, add `#[doc(hidden)]` to internals, generate rustdoc
6. (rough) Implement feature-flag modularity for genre systems
7. (rough) Add parallel headless simulation (multiple instances with different player profiles)
8. (rough) Expand onboarding (`amigo new` templates for each genre)
9. (rough) Document peer-to-peer host model explicitly in networking spec
10. (rough) Build cross-modal coherence validation (palette check, music style check)

## Abort Criteria

- If stabilizing the public API requires breaking more than 30% of existing specs, the scope is too large — split into per-module ADRs instead.
- If preview MCP tools add more than 500ms latency to the feedback loop, the approach needs rethinking (pre-render cache, lower resolution previews).
- If parallel headless simulation reveals non-determinism in the engine (different results per run), fix determinism first (see ADR-0006) before proceeding.

## Consequences

### Positive

- **Clear product vision**: Engine has a defined audience (public developers + AI-assisted workflows)
- **AI generates better results**: Evaluation loop catches problems before human review
- **Coherent games**: World context prevents art/music/story mismatch
- **Lower barrier to entry**: Modular opt-in means simple games stay simple
- **No server costs**: Peer-to-peer host model eliminates infrastructure requirements
- **Scalable quality**: Same engine serves game jams and polished releases

### Negative / Trade-offs

- **API stability burden**: Public API means slower iteration on engine internals
- **World context is opinionated**: Developers who don't want AI assistance still carry the WorldContext struct (mitigated: it's optional, lives in game code)
- **Evaluation is approximate**: Claude's objective evaluation catches some issues but misses subjective quality — developers may over-rely on it
- **Preview MCP tools add scope**: Each preview tool needs rendering, caching, and cleanup logic
- **Host model has limitations**: No host migration means host disconnect kills the session

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
