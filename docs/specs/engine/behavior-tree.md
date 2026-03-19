---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/agents"]
last_updated: 2026-03-18
---

# Behavior Trees

## Purpose

Formale Verhaltensbäume für KI-Entitäten als Ergänzung zum Utility-AI-System ([agents](agents.md)). Behavior Trees eignen sich für hierarchisch strukturierte, reaktive KI-Logik mit klarer Debuggbarkeit. Utility AI entscheidet *was* ein Agent tut (höchste Dringlichkeit), BT steuert *wie* er es tut (Sequenz von Schritten).

## Existierende Bausteine

- `Agent` + `Needs` + `AgentMemory` in `crates/amigo_core/src/agents.rs` — Utility AI bestimmt Ziel-Action
- `StateMachine` + `AiContext` in `crates/amigo_core/src/ai.rs` — FSM für einfache Zustandswechsel
- `SteeringBehavior` in `crates/amigo_steering/src/behaviors.rs` — Bewegung als Leaf-Node nutzbar
- `DialogTree` in `crates/amigo_core/src/dialog.rs` — Tree-Navigation als Pattern-Referenz

## Public API

### Node-Typen

```rust
/// Ergebnis eines Node-Ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStatus {
    Success,
    Failure,
    Running,  // Node braucht weitere Ticks
}

/// Ein Node im Behavior Tree.
pub enum BtNode {
    // ── Composite Nodes ──────────────────────────────────────

    /// Führt Kinder sequentiell aus. Stoppt beim ersten Failure.
    /// Success nur wenn alle Kinder Success.
    Sequence(Vec<BtNode>),

    /// Führt Kinder sequentiell aus. Stoppt beim ersten Success.
    /// Failure nur wenn alle Kinder Failure.
    Selector(Vec<BtNode>),

    /// Führt alle Kinder parallel aus.
    /// Policy bestimmt wann Success/Failure gemeldet wird.
    Parallel {
        children: Vec<BtNode>,
        policy: ParallelPolicy,
    },

    // ── Decorator Nodes ──────────────────────────────────────

    /// Invertiert das Ergebnis des Kindes (Success ↔ Failure, Running bleibt).
    Inverter(Box<BtNode>),

    /// Wiederholt das Kind N Mal oder Forever.
    Repeat {
        child: Box<BtNode>,
        count: RepeatCount,
    },

    /// Gibt Failure zurück wenn das Kind nach `max_ticks` noch Running ist.
    Timeout {
        child: Box<BtNode>,
        max_ticks: u32,
        elapsed: u32,
    },

    /// Gibt immer Success zurück, egal was das Kind meldet.
    AlwaysSucceed(Box<BtNode>),

    /// Gibt immer Failure zurück, egal was das Kind meldet.
    AlwaysFail(Box<BtNode>),

    // ── Leaf Nodes ───────────────────────────────────────────

    /// Prüft eine Bedingung. Gibt Success oder Failure zurück, nie Running.
    Condition(ConditionId),

    /// Führt eine Aktion aus. Kann Running zurückgeben.
    Action(ActionId),
}

/// Wann ein Parallel-Node Success/Failure meldet.
#[derive(Clone, Copy, Debug)]
pub enum ParallelPolicy {
    /// Success wenn alle Kinder Success. Failure wenn eines Failure.
    RequireAll,
    /// Success wenn eines Success. Failure wenn alle Failure.
    RequireOne,
}
```

### Blackboard

```rust
/// Typisierter Key-Value Store pro Tree-Instanz.
/// Dient zur Kommunikation zwischen Nodes.
pub struct Blackboard {
    values: FxHashMap<String, BlackboardValue>,
}

#[derive(Clone, Debug)]
pub enum BlackboardValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    Vec2(SimVec2),
    Entity(EntityId),
}

impl Blackboard {
    pub fn new() -> Self;
    pub fn set(&mut self, key: &str, value: BlackboardValue);
    pub fn get(&self, key: &str) -> Option<&BlackboardValue>;
    pub fn get_bool(&self, key: &str) -> Option<bool>;
    pub fn get_int(&self, key: &str) -> Option<i32>;
    pub fn get_float(&self, key: &str) -> Option<f32>;
    pub fn get_vec2(&self, key: &str) -> Option<SimVec2>;
    pub fn get_entity(&self, key: &str) -> Option<EntityId>;
    pub fn remove(&mut self, key: &str);
    pub fn clear(&mut self);
}
```

### ConditionId & ActionId

```rust
/// Identifiziert eine registrierte Condition-Funktion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionId(pub u32);

/// Identifiziert eine registrierte Action-Funktion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub u32);
```

### BehaviorTree

```rust
/// Eine instanziierte Behavior Tree mit eigenem Blackboard.
pub struct BehaviorTree {
    root: BtNode,
    blackboard: Blackboard,
}

impl BehaviorTree {
    pub fn new(root: BtNode) -> Self;
    pub fn blackboard(&self) -> &Blackboard;
    pub fn blackboard_mut(&mut self) -> &mut Blackboard;

    /// Führt einen Tick aus. Conditions und Actions werden via Registry aufgelöst.
    pub fn tick(&mut self, ctx: &BtContext, registry: &BtRegistry) -> NodeStatus;

    /// Setzt den Tree zurück (alle Running-States gelöscht).
    pub fn reset(&mut self);
}
```

### BtContext & BtRegistry

```rust
/// Kontext, der jedem Node zur Verfügung steht.
pub struct BtContext {
    pub entity: EntityId,
    pub position: SimVec2,
    pub target_pos: Option<SimVec2>,
    pub target_entity: Option<EntityId>,
    pub health_fraction: f32,
    pub dt: f32,
}

/// Registry für Condition- und Action-Funktionen.
pub struct BtRegistry {
    conditions: FxHashMap<ConditionId, Box<dyn Fn(&BtContext, &Blackboard) -> bool + Send>>,
    actions: FxHashMap<ActionId, Box<dyn Fn(&BtContext, &mut Blackboard) -> NodeStatus + Send>>,
}

impl BtRegistry {
    pub fn new() -> Self;

    pub fn register_condition(
        &mut self,
        id: ConditionId,
        f: impl Fn(&BtContext, &Blackboard) -> bool + Send + 'static,
    );

    pub fn register_action(
        &mut self,
        id: ActionId,
        f: impl Fn(&BtContext, &mut Blackboard) -> NodeStatus + Send + 'static,
    );
}
```

### RON-Definition

```ron
// Beispiel: Wach-KI für einen Tower-Defense-Gegner
BehaviorTree(
    root: Selector([
        // Priorität 1: Fliehen wenn HP niedrig
        Sequence([
            Condition("hp_below_30"),
            Action("flee_to_spawn"),
        ]),
        // Priorität 2: Angriff wenn Ziel in Reichweite
        Sequence([
            Condition("target_in_range"),
            Action("attack_target"),
        ]),
        // Priorität 3: Zum Ziel bewegen
        Action("move_to_waypoint"),
    ]),
)
```

RON-Dateien verwenden String-IDs (`"hp_below_30"`) die beim Laden in `ConditionId`/`ActionId` aufgelöst werden. Ein `BtLoader` konvertiert RON → `BtNode`-Baum.

## Behavior

- **Tick-Modell**: `BehaviorTree::tick()` traversiert den Baum von der Root. Jeder Node gibt `Success`, `Failure` oder `Running` zurück. `Running` bedeutet: der Node braucht weitere Ticks (z.B. eine laufende Bewegung).
- **Sequence**: Führt Kinder von links nach rechts aus. Überspringt bereits abgeschlossene Kinder. Stoppt bei `Failure` oder `Running`. Gibt `Success` nur wenn alle Kinder `Success`.
- **Selector**: Probiert Kinder von links nach rechts. Stoppt bei `Success` oder `Running`. Gibt `Failure` nur wenn alle Kinder `Failure`. Implementiert Priority-Fallback.
- **Parallel**: Tickt alle Kinder pro Tick. `RequireAll` gibt `Success` wenn alle `Success`, `Failure` sobald eines `Failure`. `RequireOne` gibt `Success` sobald eines `Success`.
- **Blackboard**: Nodes kommunizieren über den Blackboard. Z.B. eine Condition schreibt `"nearest_enemy"` als EntityId, eine Action liest es. Der Blackboard gehört zur Tree-Instanz, nicht global.
- **Integration mit Utility AI**: `Agent::evaluate_actions()` bestimmt die übergeordnete Aktion (Eat, Fight, Flee). Pro Aktion existiert ein BehaviorTree der die Details steuert (Fight-BT: Ziel auswählen → nähern → angreifen → ausweichen).
- **Fixed-Timestep**: `tick()` wird einmal pro Simulation-Tick aufgerufen (nicht pro Frame). `BtContext.dt` ist konstant.

## Internal Design

- `BtNode` ist ein owned enum-tree (keine Heap-Indirektion für Composites, nur für Decorators via `Box`).
- Running-State wird implizit durch den Tree-Traversal gehalten: Sequence/Selector merken sich den zuletzt `Running` Kindindex.
- `BtRegistry` ist pro-Spiel global (ein Satz Conditions/Actions). `BehaviorTree`-Instanzen sind pro-Entity.
- RON-Loader parst String-IDs und mappt sie auf `ConditionId`/`ActionId` via Name→ID Lookup-Tabelle.

## Non-Goals

- **Externe Dependency (bonsai-bt).** Eigene Implementierung — einfacher, keine fremde API, konsistent mit dem Rest der Engine.
- **Visueller Editor.** Tree-Definition erfolgt in RON. Ein visueller BT-Editor wäre ein Editor-Plugin, nicht Teil des Runtime-Systems.
- **Async-Nodes.** Kein `async`/`await`. Langlebige Actions geben `Running` zurück und setzen Zustand im Blackboard.
- **Hot-Reload von Trees.** RON-Dateien werden bei Szenen-Start geladen. Live-Editing von Trees ist ein Future-Feature.
- **Ersetzen von Utility AI.** BT ergänzt das Agent-System, ersetzt es nicht. Utility AI für "was tun?", BT für "wie tun?".

## Open Questions

- Soll der Debug-Overlay den aktiven Pfad im Tree visualisieren (farbige Nodes)?
- Braucht es einen `RandomSelector` der Kinder in zufälliger Reihenfolge probiert?
- Sollen Blackboard-Werte serialisierbar sein (für Save/Load von laufenden BTs)?
- Soll es ein `SubTree`-Node geben der einen anderen BT referenziert (Komposition)?

## Referenzen

- [engine/agents](agents.md) → Utility AI als übergeordnetes Entscheidungssystem
- [engine/steering](steering.md) → Steering Behaviors als Action-Implementierung
- [engine/simulation](simulation.md) → Fixed-Timestep Tick-Modell
- Halo 2 GDC Talk als Referenz-Architektur für BT-KI
