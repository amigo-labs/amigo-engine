# Plugin System Guide

Plugins extend the engine with custom events, resources, and initialization
logic — without modifying engine code.

## The Plugin trait

```rust
use amigo_engine::prelude::*;

pub trait Plugin: 'static {
    /// Called during EngineBuilder::add_plugin() to register events and resources.
    fn build(&self, ctx: &mut PluginContext);

    /// Called once after the window and renderer are ready.
    fn init(&self, _ctx: &mut GameContext) {}
}
```

## Writing a plugin

### Step 1: Define your types

```rust
// Events
#[derive(Clone, Debug)]
struct DamageEvent {
    target: EntityId,
    amount: i32,
}

#[derive(Clone, Debug)]
struct HealEvent {
    target: EntityId,
    amount: i32,
}

// Resources (singletons)
struct CombatLog {
    entries: Vec<String>,
}
```

### Step 2: Implement the Plugin trait

```rust
struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        // Register event types so they can be emitted and read
        ctx.register_event::<DamageEvent>();
        ctx.register_event::<HealEvent>();

        // Insert initial resources
        ctx.insert_resource(CombatLog { entries: Vec::new() });
    }

    fn init(&self, ctx: &mut GameContext) {
        // Runs after window/renderer are ready.
        // Good for loading assets, spawning initial entities, etc.
    }
}
```

### Step 3: Register with the engine

```rust
fn main() {
    EngineBuilder::new()
        .title("My Game")
        .add_plugin(CombatPlugin)
        .build()
        .run(MyGame);
}
```

## Using events

Events use a **double-buffered** system. Events emitted this tick are readable
next tick. This ensures deterministic ordering.

```rust
fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
    // Emit an event (written to write buffer)
    ctx.events.emit(DamageEvent { target: enemy_id, amount: 25 });

    // Read events from the *previous* tick
    for event in ctx.events.read::<HealEvent>() {
        // process healing...
    }

    SceneAction::Continue
}
```

The engine calls `events.flush()` at the end of each tick automatically.

### Event lifecycle

```
Tick N:   emit(A)  emit(B)   → write buffer: [A, B]
                              → read buffer:  [] (empty)
--- flush ---
Tick N+1: read() returns [A, B]
          emit(C)             → write buffer: [C]
--- flush ---
Tick N+2: read() returns [C]
                              → [A, B] are gone
```

## Using resources

Resources are typed singletons — one instance per type, accessible from
`GameContext.resources`.

```rust
// Insert (usually in Plugin::build or Game::init)
ctx.resources.insert(PlayerStats { health: 100, gold: 0 });

// Read
if let Some(stats) = ctx.resources.get::<PlayerStats>() {
    println!("Health: {}", stats.health);
}

// Write
if let Some(stats) = ctx.resources.get_mut::<PlayerStats>() {
    stats.gold += 10;
}

// Get or create with default
let score = ctx.resources.get_or_default::<Score>();

// Remove
let old_stats = ctx.resources.remove::<PlayerStats>();
```

## PluginContext API

| Method | Purpose |
|--------|---------|
| `register_event::<T>()` | Register a new event type for `emit`/`read` |
| `insert_resource(value)` | Pre-insert a resource before game starts |

Registrations are deferred — they're applied when `GameContext` is created,
before `Plugin::init()` and `Game::init()` run.

## Multiple plugins

Plugins are initialized in registration order:

```rust
EngineBuilder::new()
    .add_plugin(InputMappingPlugin)   // build() called first
    .add_plugin(CombatPlugin)         // build() called second
    .add_plugin(AudioPlugin)          // build() called third
    .build()
    .run(MyGame);

// init() is also called in the same order, after the window is created.
```

## Example: Score tracking plugin

```rust
#[derive(Default)]
struct ScoreBoard {
    score: u64,
    high_score: u64,
    combo: u32,
}

#[derive(Clone, Debug)]
struct ScoreEvent {
    points: u64,
}

struct ScorePlugin;

impl Plugin for ScorePlugin {
    fn build(&self, ctx: &mut PluginContext) {
        ctx.register_event::<ScoreEvent>();
        ctx.insert_resource(ScoreBoard::default());
    }
}

// In Game::update():
fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
    // Other systems emit score events
    ctx.events.emit(ScoreEvent { points: 100 });

    // Score system reads them
    let mut total = 0u64;
    for event in ctx.events.read::<ScoreEvent>() {
        total += event.points;
    }

    if total > 0 {
        let board = ctx.resources.get_mut::<ScoreBoard>().unwrap();
        board.score += total;
        board.combo += 1;
        if board.score > board.high_score {
            board.high_score = board.score;
        }
    }

    SceneAction::Continue
}
```

## Example: Editor integration plugin

The built-in editor uses the same plugin pattern:

```rust
use amigo_editor::{EditorRuntime, AmigoLevel};

struct EditorPlugin {
    level: AmigoLevel,
}

impl Plugin for EditorPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        ctx.insert_resource(EditorRuntime::new(self.level.clone()));
    }

    fn init(&self, ctx: &mut GameContext) {
        // Editor is now accessible via:
        // ctx.resources.get::<EditorRuntime>()
    }
}
```

## Tips

- **Keep plugins focused.** One plugin per system (combat, audio, scoring).
- **Use events for cross-system communication.** Don't reach into other
  plugins' resources — emit an event and let the other system handle it.
- **Resources are for state, events are for notifications.** If something
  needs to persist across ticks, use a resource. If it's a one-time signal,
  use an event.
- **Plugin::build() runs before the window exists.** Don't access GPU or
  windowing resources there. Use `Plugin::init()` for that.
