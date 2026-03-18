//! Visual script editor — node-based logic graphs for AI behaviors, triggers,
//! and game events. Provides data structures and evaluation; the UI rendering
//! is handled by the main editor UI module.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Node graph primitives
// ---------------------------------------------------------------------------

/// Unique node identifier within a script graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// Unique pin identifier (node + pin index).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinId {
    pub node: NodeId,
    pub index: u8,
    pub kind: PinKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinKind {
    Input,
    Output,
}

/// A connection between two pins.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Connection {
    pub from: PinId,
    pub to: PinId,
}

/// Data that flows through pins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PinValue {
    None,
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
    Entity(u32),
    Vec2(f32, f32),
}

impl Default for PinValue {
    fn default() -> Self {
        PinValue::None
    }
}

/// Type of a pin (for validation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinType {
    Flow,
    Bool,
    Int,
    Float,
    String,
    Entity,
    Vec2,
    Any,
}

/// A pin descriptor on a node template.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinDef {
    pub name: String,
    pub pin_type: PinType,
}

// ---------------------------------------------------------------------------
// Node types (built-in)
// ---------------------------------------------------------------------------

/// The kind of logic a node performs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeKind {
    // Events
    OnStart,
    OnUpdate,
    OnCollision,
    OnTriggerEnter,
    OnTriggerExit,
    OnCustomEvent(String),

    // Flow control
    Branch,
    Sequence(u8),
    ForLoop { count_pin: u8 },
    Delay { seconds: f32 },
    Gate { open: bool },

    // Math
    Add,
    Subtract,
    Multiply,
    Divide,
    Clamp,
    RandomRange,
    Abs,
    Min,
    Max,

    // Comparison
    Equal,
    NotEqual,
    Greater,
    Less,
    And,
    Or,
    Not,

    // Actions
    SetVariable(String),
    GetVariable(String),
    Print,
    PlaySound(String),
    SpawnEntity(String),
    DestroyEntity,
    SetPosition,
    GetPosition,
    SetVelocity,
    ApplyForce,

    // AI
    MoveTo,
    LookAt,
    Patrol { speed: f32 },
    Chase { speed: f32, radius: f32 },
    Flee { speed: f32, radius: f32 },
    Wait { seconds: f32 },
    ChooseRandom,

    // Game-specific
    DealDamage,
    Heal,
    GiveItem { item_id: u32, count: u32 },
    CheckFlag(String),
    SetFlag(String),

    // Custom (user-defined via name + arbitrary data)
    Custom { name: String, data: String },
}

// ---------------------------------------------------------------------------
// Visual Script Node
// ---------------------------------------------------------------------------

/// A node instance in the script graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptNode {
    pub id: NodeId,
    pub kind: NodeKind,
    /// Display position in the editor canvas (x, y).
    pub position: (f32, f32),
    /// Human-readable comment / label.
    pub comment: String,
    /// Input pin definitions.
    pub inputs: Vec<PinDef>,
    /// Output pin definitions.
    pub outputs: Vec<PinDef>,
    /// Constant values set directly on input pins (pin_index → value).
    pub constants: HashMap<u8, PinValue>,
}

impl ScriptNode {
    /// Create a new node with auto-generated pin definitions.
    pub fn new(id: NodeId, kind: NodeKind, position: (f32, f32)) -> Self {
        let (inputs, outputs) = default_pins(&kind);
        Self {
            id,
            kind,
            position,
            comment: String::new(),
            inputs,
            outputs,
            constants: HashMap::new(),
        }
    }

    pub fn input_pin(&self, index: u8) -> PinId {
        PinId {
            node: self.id,
            index,
            kind: PinKind::Input,
        }
    }

    pub fn output_pin(&self, index: u8) -> PinId {
        PinId {
            node: self.id,
            index,
            kind: PinKind::Output,
        }
    }
}

// ---------------------------------------------------------------------------
// Script graph
// ---------------------------------------------------------------------------

/// A complete visual script graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptGraph {
    pub name: String,
    pub nodes: Vec<ScriptNode>,
    pub connections: Vec<Connection>,
    next_id: u32,
}

impl ScriptGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            connections: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a node and return its id.
    pub fn add_node(&mut self, kind: NodeKind, position: (f32, f32)) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(ScriptNode::new(id, kind, position));
        id
    }

    /// Remove a node and all connections involving it.
    pub fn remove_node(&mut self, id: NodeId) {
        self.connections
            .retain(|c| c.from.node != id && c.to.node != id);
        self.nodes.retain(|n| n.id != id);
    }

    /// Connect two pins. Returns false if types are incompatible.
    pub fn connect(&mut self, from: PinId, to: PinId) -> bool {
        if from.kind != PinKind::Output || to.kind != PinKind::Input {
            return false;
        }

        // Check type compatibility
        let from_type = self.pin_type(&from);
        let to_type = self.pin_type(&to);
        if !types_compatible(from_type, to_type) {
            return false;
        }

        // Remove existing connection to the target input (inputs accept one connection)
        self.connections.retain(|c| c.to != to);

        self.connections.push(Connection { from, to });
        true
    }

    /// Disconnect a specific connection.
    pub fn disconnect(&mut self, from: PinId, to: PinId) {
        self.connections.retain(|c| !(c.from == from && c.to == to));
    }

    /// Get a node by id.
    pub fn get_node(&self, id: NodeId) -> Option<&ScriptNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get a mutable node by id.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut ScriptNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Find all connections originating from a given pin.
    pub fn outgoing(&self, from: PinId) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.from == from).collect()
    }

    /// Find the connection targeting a given input pin.
    pub fn incoming(&self, to: PinId) -> Option<&Connection> {
        self.connections.iter().find(|c| c.to == to)
    }

    /// Get the type of a pin.
    fn pin_type(&self, pin: &PinId) -> PinType {
        if let Some(node) = self.get_node(pin.node) {
            let defs = match pin.kind {
                PinKind::Input => &node.inputs,
                PinKind::Output => &node.outputs,
            };
            defs.get(pin.index as usize)
                .map(|d| d.pin_type)
                .unwrap_or(PinType::Any)
        } else {
            PinType::Any
        }
    }

    /// Validate the graph: check for cycles in flow connections.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Check for dangling connections
        for conn in &self.connections {
            if self.get_node(conn.from.node).is_none() {
                errors.push(format!("Connection from deleted node {:?}", conn.from.node));
            }
            if self.get_node(conn.to.node).is_none() {
                errors.push(format!("Connection to deleted node {:?}", conn.to.node));
            }
        }

        // Check for orphan nodes (no connections at all)
        for node in &self.nodes {
            let has_any = self
                .connections
                .iter()
                .any(|c| c.from.node == node.id || c.to.node == node.id);
            if !has_any
                && !matches!(
                    node.kind,
                    NodeKind::OnStart
                        | NodeKind::OnUpdate
                        | NodeKind::OnCollision
                        | NodeKind::OnTriggerEnter
                        | NodeKind::OnTriggerExit
                        | NodeKind::OnCustomEvent(_)
                )
            {
                errors.push(format!(
                    "Node {:?} is disconnected ({})",
                    node.id, node.comment
                ));
            }
        }

        errors
    }
}

// ---------------------------------------------------------------------------
// Pin definitions for built-in node types
// ---------------------------------------------------------------------------

fn pin(name: &str, t: PinType) -> PinDef {
    PinDef {
        name: name.to_string(),
        pin_type: t,
    }
}

fn default_pins(kind: &NodeKind) -> (Vec<PinDef>, Vec<PinDef>) {
    match kind {
        // Events → output flow only
        NodeKind::OnStart | NodeKind::OnUpdate => (vec![], vec![pin("Exec", PinType::Flow)]),
        NodeKind::OnCollision | NodeKind::OnTriggerEnter | NodeKind::OnTriggerExit => (
            vec![],
            vec![pin("Exec", PinType::Flow), pin("Other", PinType::Entity)],
        ),
        NodeKind::OnCustomEvent(_) => (vec![], vec![pin("Exec", PinType::Flow)]),

        // Flow
        NodeKind::Branch => (
            vec![pin("Exec", PinType::Flow), pin("Condition", PinType::Bool)],
            vec![pin("True", PinType::Flow), pin("False", PinType::Flow)],
        ),
        NodeKind::Sequence(n) => {
            let inputs = vec![pin("Exec", PinType::Flow)];
            let outputs = (0..*n)
                .map(|i| pin(&format!("Then {i}"), PinType::Flow))
                .collect();
            (inputs, outputs)
        }
        NodeKind::ForLoop { .. } => (
            vec![pin("Exec", PinType::Flow), pin("Count", PinType::Int)],
            vec![
                pin("Body", PinType::Flow),
                pin("Index", PinType::Int),
                pin("Done", PinType::Flow),
            ],
        ),
        NodeKind::Delay { .. } => (
            vec![pin("Exec", PinType::Flow)],
            vec![pin("Done", PinType::Flow)],
        ),
        NodeKind::Gate { .. } => (
            vec![pin("Exec", PinType::Flow), pin("Open", PinType::Bool)],
            vec![pin("Exec", PinType::Flow)],
        ),

        // Math
        NodeKind::Add | NodeKind::Subtract | NodeKind::Multiply | NodeKind::Divide => (
            vec![pin("A", PinType::Float), pin("B", PinType::Float)],
            vec![pin("Result", PinType::Float)],
        ),
        NodeKind::Clamp => (
            vec![
                pin("Value", PinType::Float),
                pin("Min", PinType::Float),
                pin("Max", PinType::Float),
            ],
            vec![pin("Result", PinType::Float)],
        ),
        NodeKind::RandomRange => (
            vec![pin("Min", PinType::Float), pin("Max", PinType::Float)],
            vec![pin("Result", PinType::Float)],
        ),
        NodeKind::Abs | NodeKind::Not => (
            vec![pin("Value", PinType::Float)],
            vec![pin("Result", PinType::Float)],
        ),
        NodeKind::Min | NodeKind::Max => (
            vec![pin("A", PinType::Float), pin("B", PinType::Float)],
            vec![pin("Result", PinType::Float)],
        ),

        // Comparison
        NodeKind::Equal | NodeKind::NotEqual | NodeKind::Greater | NodeKind::Less => (
            vec![pin("A", PinType::Float), pin("B", PinType::Float)],
            vec![pin("Result", PinType::Bool)],
        ),
        NodeKind::And | NodeKind::Or => (
            vec![pin("A", PinType::Bool), pin("B", PinType::Bool)],
            vec![pin("Result", PinType::Bool)],
        ),

        // Variables
        NodeKind::SetVariable(_) => (
            vec![pin("Exec", PinType::Flow), pin("Value", PinType::Any)],
            vec![pin("Exec", PinType::Flow)],
        ),
        NodeKind::GetVariable(_) => (vec![], vec![pin("Value", PinType::Any)]),
        NodeKind::Print => (
            vec![pin("Exec", PinType::Flow), pin("Text", PinType::String)],
            vec![pin("Exec", PinType::Flow)],
        ),

        // Actions
        NodeKind::PlaySound(_) => (
            vec![pin("Exec", PinType::Flow)],
            vec![pin("Exec", PinType::Flow)],
        ),
        NodeKind::SpawnEntity(_) => (
            vec![pin("Exec", PinType::Flow), pin("Position", PinType::Vec2)],
            vec![pin("Exec", PinType::Flow), pin("Spawned", PinType::Entity)],
        ),
        NodeKind::DestroyEntity => (
            vec![pin("Exec", PinType::Flow), pin("Entity", PinType::Entity)],
            vec![pin("Exec", PinType::Flow)],
        ),
        NodeKind::SetPosition | NodeKind::SetVelocity | NodeKind::ApplyForce => (
            vec![
                pin("Exec", PinType::Flow),
                pin("Entity", PinType::Entity),
                pin("Value", PinType::Vec2),
            ],
            vec![pin("Exec", PinType::Flow)],
        ),
        NodeKind::GetPosition => (
            vec![pin("Entity", PinType::Entity)],
            vec![pin("Position", PinType::Vec2)],
        ),

        // AI
        NodeKind::MoveTo | NodeKind::LookAt => (
            vec![pin("Exec", PinType::Flow), pin("Target", PinType::Vec2)],
            vec![pin("Done", PinType::Flow)],
        ),
        NodeKind::Patrol { .. } | NodeKind::Chase { .. } | NodeKind::Flee { .. } => (
            vec![pin("Exec", PinType::Flow), pin("Target", PinType::Entity)],
            vec![pin("Done", PinType::Flow)],
        ),
        NodeKind::Wait { .. } => (
            vec![pin("Exec", PinType::Flow)],
            vec![pin("Done", PinType::Flow)],
        ),
        NodeKind::ChooseRandom => (
            vec![pin("Exec", PinType::Flow)],
            vec![pin("A", PinType::Flow), pin("B", PinType::Flow)],
        ),

        // Game
        NodeKind::DealDamage | NodeKind::Heal => (
            vec![
                pin("Exec", PinType::Flow),
                pin("Target", PinType::Entity),
                pin("Amount", PinType::Float),
            ],
            vec![pin("Exec", PinType::Flow)],
        ),
        NodeKind::GiveItem { .. } => (
            vec![pin("Exec", PinType::Flow), pin("Target", PinType::Entity)],
            vec![pin("Exec", PinType::Flow)],
        ),
        NodeKind::CheckFlag(_) => (vec![], vec![pin("Result", PinType::Bool)]),
        NodeKind::SetFlag(_) => (
            vec![pin("Exec", PinType::Flow)],
            vec![pin("Exec", PinType::Flow)],
        ),

        NodeKind::Custom { .. } => (
            vec![pin("Exec", PinType::Flow), pin("In", PinType::Any)],
            vec![pin("Exec", PinType::Flow), pin("Out", PinType::Any)],
        ),
    }
}

fn types_compatible(from: PinType, to: PinType) -> bool {
    if from == to {
        return true;
    }
    if from == PinType::Any || to == PinType::Any {
        return true;
    }
    // Allow int → float coercion
    if from == PinType::Int && to == PinType::Float {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Simple graph interpreter (for preview / debugging in editor)
// ---------------------------------------------------------------------------

/// Execution context for stepping through a script in the editor preview.
pub struct ScriptDebugger {
    /// Currently highlighted node (for step-through debugging).
    pub current_node: Option<NodeId>,
    /// Variable store.
    pub variables: HashMap<String, PinValue>,
    /// Log of execution steps.
    pub log: Vec<String>,
    /// Max steps before halting (prevents infinite loops).
    pub max_steps: u32,
    step_count: u32,
}

impl ScriptDebugger {
    pub fn new() -> Self {
        Self {
            current_node: None,
            variables: HashMap::new(),
            log: Vec::new(),
            max_steps: 1000,
            step_count: 0,
        }
    }

    /// Reset the debugger state.
    pub fn reset(&mut self) {
        self.current_node = None;
        self.variables.clear();
        self.log.clear();
        self.step_count = 0;
    }

    /// Step to the next node following flow connections.
    pub fn step(&mut self, graph: &ScriptGraph) -> bool {
        self.step_count += 1;
        if self.step_count > self.max_steps {
            self.log
                .push(format!("Halted: exceeded {} steps", self.max_steps));
            return false;
        }

        let current = match self.current_node {
            Some(id) => id,
            None => {
                // Find OnStart node
                if let Some(node) = graph
                    .nodes
                    .iter()
                    .find(|n| matches!(n.kind, NodeKind::OnStart))
                {
                    self.current_node = Some(node.id);
                    self.log.push(format!("Started at node {:?}", node.id));
                    return true;
                }
                self.log.push("No OnStart node found".to_string());
                return false;
            }
        };

        let node = match graph.get_node(current) {
            Some(n) => n,
            None => {
                self.log.push(format!("Node {:?} not found", current));
                return false;
            }
        };

        self.log.push(format!(
            "Executing {:?} ({:?})",
            node.id,
            std::mem::discriminant(&node.kind)
        ));

        // Find the first flow output connection
        let flow_output = PinId {
            node: current,
            index: 0,
            kind: PinKind::Output,
        };

        if let Some(conn) = graph.connections.iter().find(|c| c.from == flow_output) {
            self.current_node = Some(conn.to.node);
            true
        } else {
            self.log.push("End of flow".to_string());
            self.current_node = None;
            false
        }
    }
}

impl Default for ScriptDebugger {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Graph construction ───────────────────────────────────────

    #[test]
    fn create_graph_and_add_nodes() {
        let mut graph = ScriptGraph::new("test");
        let on_start = graph.add_node(NodeKind::OnStart, (0.0, 0.0));
        let print = graph.add_node(NodeKind::Print, (200.0, 0.0));

        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.get_node(on_start).is_some());
        assert!(graph.get_node(print).is_some());
    }

    #[test]
    fn connect_nodes() {
        let mut graph = ScriptGraph::new("test");
        let on_start = graph.add_node(NodeKind::OnStart, (0.0, 0.0));
        let print = graph.add_node(NodeKind::Print, (200.0, 0.0));

        let start_node = graph.get_node(on_start).unwrap();
        let print_node = graph.get_node(print).unwrap();
        let from = start_node.output_pin(0); // Exec out
        let to = print_node.input_pin(0); // Exec in

        assert!(graph.connect(from, to));
        assert_eq!(graph.connections.len(), 1);
    }

    #[test]
    fn reject_incompatible_types() {
        let mut graph = ScriptGraph::new("test");
        let add = graph.add_node(NodeKind::Add, (0.0, 0.0));
        let branch = graph.add_node(NodeKind::Branch, (200.0, 0.0));

        let add_node = graph.get_node(add).unwrap();
        let branch_node = graph.get_node(branch).unwrap();
        // Add outputs Float, Branch.Condition expects Bool
        let from = add_node.output_pin(0); // Float result
        let to = branch_node.input_pin(1); // Bool condition

        assert!(!graph.connect(from, to));
    }

    #[test]
    fn remove_node_cleans_connections() {
        let mut graph = ScriptGraph::new("test");
        let a = graph.add_node(NodeKind::OnStart, (0.0, 0.0));
        let b = graph.add_node(NodeKind::Print, (200.0, 0.0));

        let from = graph.get_node(a).unwrap().output_pin(0);
        let to = graph.get_node(b).unwrap().input_pin(0);
        graph.connect(from, to);
        assert_eq!(graph.connections.len(), 1);

        graph.remove_node(a);
        assert_eq!(graph.connections.len(), 0);
        assert_eq!(graph.nodes.len(), 1);
    }

    #[test]
    fn disconnect() {
        let mut graph = ScriptGraph::new("test");
        let a = graph.add_node(NodeKind::OnStart, (0.0, 0.0));
        let b = graph.add_node(NodeKind::Print, (200.0, 0.0));

        let from = graph.get_node(a).unwrap().output_pin(0);
        let to = graph.get_node(b).unwrap().input_pin(0);
        graph.connect(from, to);
        graph.disconnect(from, to);
        assert_eq!(graph.connections.len(), 0);
    }

    // ── Debugger ─────────────────────────────────────────────────

    #[test]
    fn debugger_step_through() {
        let mut graph = ScriptGraph::new("test");
        let start = graph.add_node(NodeKind::OnStart, (0.0, 0.0));
        let print = graph.add_node(NodeKind::Print, (200.0, 0.0));

        let from = graph.get_node(start).unwrap().output_pin(0);
        let to = graph.get_node(print).unwrap().input_pin(0);
        graph.connect(from, to);

        let mut debugger = ScriptDebugger::new();

        // Step 1: find OnStart
        assert!(debugger.step(&graph));
        assert_eq!(debugger.current_node, Some(start));

        // Step 2: follow flow to print
        assert!(debugger.step(&graph));
        assert_eq!(debugger.current_node, Some(print));

        // Step 3: no more flow → end
        assert!(!debugger.step(&graph));
        assert_eq!(debugger.current_node, None);
    }

    #[test]
    fn debugger_max_steps() {
        let graph = ScriptGraph::new("empty");
        let mut debugger = ScriptDebugger::new();
        debugger.max_steps = 2;

        // No OnStart → returns false immediately
        assert!(!debugger.step(&graph));
    }

    // ── Validation & serialization ────────────────────────────

    #[test]
    fn validate_detects_orphans() {
        let mut graph = ScriptGraph::new("test");
        graph.add_node(NodeKind::OnStart, (0.0, 0.0));
        graph.add_node(NodeKind::Print, (200.0, 0.0)); // disconnected

        let errors = graph.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("disconnected"));
    }

    #[test]
    fn script_graph_serialization() {
        let mut graph = ScriptGraph::new("test_serial");
        let a = graph.add_node(NodeKind::OnStart, (10.0, 20.0));
        let b = graph.add_node(NodeKind::Branch, (100.0, 20.0));
        let from = graph.get_node(a).unwrap().output_pin(0);
        let to = graph.get_node(b).unwrap().input_pin(0);
        graph.connect(from, to);

        let json = serde_json::to_string(&graph).unwrap();
        let restored: ScriptGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "test_serial");
        assert_eq!(restored.nodes.len(), 2);
        assert_eq!(restored.connections.len(), 1);
    }
}
