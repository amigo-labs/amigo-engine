//! Behavior Trees for hierarchical, reactive AI logic.
//!
//! Complements the Utility AI system (agents.rs) — Utility AI decides *what* to do,
//! Behavior Trees control *how* to do it.

use crate::ecs::EntityId;
use crate::math::SimVec2;
use rustc_hash::FxHashMap;

// ---------------------------------------------------------------------------
// Node Status
// ---------------------------------------------------------------------------

/// Result of a node tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStatus {
    Success,
    Failure,
    Running,
}

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

/// Identifies a registered condition function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionId(pub u32);

/// Identifies a registered action function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub u32);

// ---------------------------------------------------------------------------
// Parallel Policy
// ---------------------------------------------------------------------------

/// When a Parallel node reports Success/Failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParallelPolicy {
    /// Success when all children succeed. Failure when any child fails.
    RequireAll,
    /// Success when any child succeeds. Failure when all children fail.
    RequireOne,
}

// ---------------------------------------------------------------------------
// Repeat Count (re-use from tween if desired, but keep independent)
// ---------------------------------------------------------------------------

/// How many times a Repeat decorator runs its child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BtRepeatCount {
    Times(u32),
    Forever,
}

// ---------------------------------------------------------------------------
// BtNode
// ---------------------------------------------------------------------------

/// A node in the behavior tree.
pub enum BtNode {
    // Composite Nodes
    Sequence(Vec<BtNode>),
    Selector(Vec<BtNode>),
    Parallel {
        children: Vec<BtNode>,
        policy: ParallelPolicy,
    },

    // Decorator Nodes
    Inverter(Box<BtNode>),
    Repeat {
        child: Box<BtNode>,
        count: BtRepeatCount,
        current: u32,
    },
    Timeout {
        child: Box<BtNode>,
        max_ticks: u32,
        elapsed: u32,
    },
    AlwaysSucceed(Box<BtNode>),
    AlwaysFail(Box<BtNode>),

    // Leaf Nodes
    Condition(ConditionId),
    Action(ActionId),
}

// ---------------------------------------------------------------------------
// Blackboard
// ---------------------------------------------------------------------------

/// Value types stored in the blackboard.
#[derive(Clone, Debug)]
pub enum BlackboardValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    Vec2(SimVec2),
    Entity(EntityId),
}

/// Typed key-value store per tree instance for inter-node communication.
pub struct Blackboard {
    values: FxHashMap<String, BlackboardValue>,
}

impl Blackboard {
    pub fn new() -> Self {
        Self {
            values: FxHashMap::default(),
        }
    }

    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        self.values.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        self.values.get(key)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.values.get(key) {
            Some(BlackboardValue::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_int(&self, key: &str) -> Option<i32> {
        match self.values.get(key) {
            Some(BlackboardValue::Int(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_float(&self, key: &str) -> Option<f32> {
        match self.values.get(key) {
            Some(BlackboardValue::Float(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_vec2(&self, key: &str) -> Option<SimVec2> {
        match self.values.get(key) {
            Some(BlackboardValue::Vec2(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_entity(&self, key: &str) -> Option<EntityId> {
        match self.values.get(key) {
            Some(BlackboardValue::Entity(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

impl Default for Blackboard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BtContext
// ---------------------------------------------------------------------------

/// Context available to every node during a tick.
pub struct BtContext {
    pub entity: EntityId,
    pub position: SimVec2,
    pub target_pos: Option<SimVec2>,
    pub target_entity: Option<EntityId>,
    pub health_fraction: f32,
    pub dt: f32,
}

// ---------------------------------------------------------------------------
// BtRegistry
// ---------------------------------------------------------------------------

/// Registry for condition and action functions. One per game (global).
#[allow(clippy::type_complexity)]
pub struct BtRegistry {
    conditions: FxHashMap<ConditionId, Box<dyn Fn(&BtContext, &Blackboard) -> bool + Send>>,
    actions: FxHashMap<ActionId, Box<dyn Fn(&BtContext, &mut Blackboard) -> NodeStatus + Send>>,
}

impl BtRegistry {
    pub fn new() -> Self {
        Self {
            conditions: FxHashMap::default(),
            actions: FxHashMap::default(),
        }
    }

    pub fn register_condition(
        &mut self,
        id: ConditionId,
        f: impl Fn(&BtContext, &Blackboard) -> bool + Send + 'static,
    ) {
        self.conditions.insert(id, Box::new(f));
    }

    pub fn register_action(
        &mut self,
        id: ActionId,
        f: impl Fn(&BtContext, &mut Blackboard) -> NodeStatus + Send + 'static,
    ) {
        self.actions.insert(id, Box::new(f));
    }
}

impl Default for BtRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BehaviorTree
// ---------------------------------------------------------------------------

/// An instantiated behavior tree with its own blackboard.
pub struct BehaviorTree {
    root: BtNode,
    blackboard: Blackboard,
}

impl BehaviorTree {
    pub fn new(root: BtNode) -> Self {
        Self {
            root,
            blackboard: Blackboard::new(),
        }
    }

    pub fn blackboard(&self) -> &Blackboard {
        &self.blackboard
    }

    pub fn blackboard_mut(&mut self) -> &mut Blackboard {
        &mut self.blackboard
    }

    /// Execute one tick of the behavior tree.
    pub fn tick(&mut self, ctx: &BtContext, registry: &BtRegistry) -> NodeStatus {
        tick_node(&mut self.root, ctx, &mut self.blackboard, registry)
    }

    /// Reset all running states in the tree.
    pub fn reset(&mut self) {
        reset_node(&mut self.root);
    }
}

// ---------------------------------------------------------------------------
// Tree traversal
// ---------------------------------------------------------------------------

fn tick_node(
    node: &mut BtNode,
    ctx: &BtContext,
    bb: &mut Blackboard,
    reg: &BtRegistry,
) -> NodeStatus {
    match node {
        BtNode::Sequence(children) => {
            for child in children.iter_mut() {
                let status = tick_node(child, ctx, bb, reg);
                match status {
                    NodeStatus::Failure => return NodeStatus::Failure,
                    NodeStatus::Running => return NodeStatus::Running,
                    NodeStatus::Success => continue,
                }
            }
            NodeStatus::Success
        }

        BtNode::Selector(children) => {
            for child in children.iter_mut() {
                let status = tick_node(child, ctx, bb, reg);
                match status {
                    NodeStatus::Success => return NodeStatus::Success,
                    NodeStatus::Running => return NodeStatus::Running,
                    NodeStatus::Failure => continue,
                }
            }
            NodeStatus::Failure
        }

        BtNode::Parallel { children, policy } => {
            let mut success_count = 0;
            let mut failure_count = 0;
            let mut running = false;
            for child in children.iter_mut() {
                match tick_node(child, ctx, bb, reg) {
                    NodeStatus::Success => success_count += 1,
                    NodeStatus::Failure => failure_count += 1,
                    NodeStatus::Running => running = true,
                }
            }
            let total = children.len();
            match policy {
                ParallelPolicy::RequireAll => {
                    if failure_count > 0 {
                        NodeStatus::Failure
                    } else if success_count == total {
                        NodeStatus::Success
                    } else {
                        NodeStatus::Running
                    }
                }
                ParallelPolicy::RequireOne => {
                    if success_count > 0 {
                        NodeStatus::Success
                    } else if failure_count == total {
                        NodeStatus::Failure
                    } else if running {
                        NodeStatus::Running
                    } else {
                        NodeStatus::Failure
                    }
                }
            }
        }

        BtNode::Inverter(child) => match tick_node(child, ctx, bb, reg) {
            NodeStatus::Success => NodeStatus::Failure,
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Running => NodeStatus::Running,
        },

        BtNode::Repeat {
            child,
            count,
            current,
        } => {
            let status = tick_node(child, ctx, bb, reg);
            match status {
                NodeStatus::Running => NodeStatus::Running,
                NodeStatus::Failure => NodeStatus::Failure,
                NodeStatus::Success => {
                    *current += 1;
                    match count {
                        BtRepeatCount::Forever => {
                            reset_node(child);
                            NodeStatus::Running
                        }
                        BtRepeatCount::Times(n) => {
                            if *current >= *n {
                                NodeStatus::Success
                            } else {
                                reset_node(child);
                                NodeStatus::Running
                            }
                        }
                    }
                }
            }
        }

        BtNode::Timeout {
            child,
            max_ticks,
            elapsed,
        } => {
            *elapsed += 1;
            if *elapsed > *max_ticks {
                return NodeStatus::Failure;
            }
            tick_node(child, ctx, bb, reg)
        }

        BtNode::AlwaysSucceed(child) => {
            tick_node(child, ctx, bb, reg);
            NodeStatus::Success
        }

        BtNode::AlwaysFail(child) => {
            tick_node(child, ctx, bb, reg);
            NodeStatus::Failure
        }

        BtNode::Condition(id) => {
            if let Some(f) = reg.conditions.get(id) {
                if f(ctx, bb) {
                    NodeStatus::Success
                } else {
                    NodeStatus::Failure
                }
            } else {
                NodeStatus::Failure
            }
        }

        BtNode::Action(id) => {
            if let Some(f) = reg.actions.get(id) {
                f(ctx, bb)
            } else {
                NodeStatus::Failure
            }
        }
    }
}

fn reset_node(node: &mut BtNode) {
    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) => {
            for child in children {
                reset_node(child);
            }
        }
        BtNode::Parallel { children, .. } => {
            for child in children {
                reset_node(child);
            }
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) | BtNode::AlwaysFail(child) => {
            reset_node(child);
        }
        BtNode::Repeat { child, current, .. } => {
            *current = 0;
            reset_node(child);
        }
        BtNode::Timeout { child, elapsed, .. } => {
            *elapsed = 0;
            reset_node(child);
        }
        BtNode::Condition(_) | BtNode::Action(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Fix;

    fn test_ctx() -> BtContext {
        BtContext {
            entity: EntityId::from_raw(1, 0),
            position: SimVec2::ZERO,
            target_pos: None,
            target_entity: None,
            health_fraction: 1.0,
            dt: 1.0 / 60.0,
        }
    }

    const COND_TRUE: ConditionId = ConditionId(1);
    const COND_FALSE: ConditionId = ConditionId(2);
    const COND_HP_LOW: ConditionId = ConditionId(3);
    const ACT_SUCCESS: ActionId = ActionId(10);
    const ACT_RUNNING: ActionId = ActionId(11);
    const ACT_FAIL: ActionId = ActionId(12);
    const ACT_INCREMENT: ActionId = ActionId(13);

    fn test_registry() -> BtRegistry {
        let mut reg = BtRegistry::new();
        reg.register_condition(COND_TRUE, |_, _| true);
        reg.register_condition(COND_FALSE, |_, _| false);
        reg.register_condition(COND_HP_LOW, |ctx, _| ctx.health_fraction < 0.3);
        reg.register_action(ACT_SUCCESS, |_, _| NodeStatus::Success);
        reg.register_action(ACT_RUNNING, |_, _| NodeStatus::Running);
        reg.register_action(ACT_FAIL, |_, _| NodeStatus::Failure);
        reg.register_action(ACT_INCREMENT, |_, bb| {
            let count = bb.get_int("count").unwrap_or(0);
            bb.set("count", BlackboardValue::Int(count + 1));
            NodeStatus::Success
        });
        reg
    }

    #[test]
    fn sequence_all_success() {
        let mut bt = BehaviorTree::new(BtNode::Sequence(vec![
            BtNode::Action(ACT_SUCCESS),
            BtNode::Action(ACT_SUCCESS),
        ]));
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Success);
    }

    #[test]
    fn sequence_stops_on_failure() {
        let mut bt = BehaviorTree::new(BtNode::Sequence(vec![
            BtNode::Action(ACT_SUCCESS),
            BtNode::Action(ACT_FAIL),
            BtNode::Action(ACT_SUCCESS),
        ]));
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Failure);
    }

    #[test]
    fn selector_stops_on_success() {
        let mut bt = BehaviorTree::new(BtNode::Selector(vec![
            BtNode::Action(ACT_FAIL),
            BtNode::Action(ACT_SUCCESS),
            BtNode::Action(ACT_FAIL),
        ]));
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Success);
    }

    #[test]
    fn selector_all_fail() {
        let mut bt = BehaviorTree::new(BtNode::Selector(vec![
            BtNode::Action(ACT_FAIL),
            BtNode::Action(ACT_FAIL),
        ]));
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Failure);
    }

    #[test]
    fn inverter() {
        let mut bt = BehaviorTree::new(BtNode::Inverter(Box::new(BtNode::Action(ACT_SUCCESS))));
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Failure);
    }

    #[test]
    fn condition_node() {
        let mut bt = BehaviorTree::new(BtNode::Sequence(vec![
            BtNode::Condition(COND_TRUE),
            BtNode::Action(ACT_SUCCESS),
        ]));
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Success);
    }

    #[test]
    fn condition_hp_low() {
        let mut bt = BehaviorTree::new(BtNode::Condition(COND_HP_LOW));
        let reg = test_registry();
        let mut ctx = test_ctx();

        ctx.health_fraction = 0.5;
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Failure);

        ctx.health_fraction = 0.2;
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Success);
    }

    #[test]
    fn blackboard_communication() {
        let mut bt = BehaviorTree::new(BtNode::Sequence(vec![
            BtNode::Action(ACT_INCREMENT),
            BtNode::Action(ACT_INCREMENT),
            BtNode::Action(ACT_INCREMENT),
        ]));
        let reg = test_registry();
        bt.tick(&test_ctx(), &reg);
        assert_eq!(bt.blackboard().get_int("count"), Some(3));
    }

    #[test]
    fn timeout_expires() {
        let mut bt = BehaviorTree::new(BtNode::Timeout {
            child: Box::new(BtNode::Action(ACT_RUNNING)),
            max_ticks: 3,
            elapsed: 0,
        });
        let reg = test_registry();
        let ctx = test_ctx();
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Running);
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Running);
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Running);
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Failure); // Timed out
    }

    #[test]
    fn parallel_require_all() {
        let mut bt = BehaviorTree::new(BtNode::Parallel {
            children: vec![BtNode::Action(ACT_SUCCESS), BtNode::Action(ACT_SUCCESS)],
            policy: ParallelPolicy::RequireAll,
        });
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Success);
    }

    #[test]
    fn parallel_require_one() {
        let mut bt = BehaviorTree::new(BtNode::Parallel {
            children: vec![BtNode::Action(ACT_FAIL), BtNode::Action(ACT_SUCCESS)],
            policy: ParallelPolicy::RequireOne,
        });
        let reg = test_registry();
        assert_eq!(bt.tick(&test_ctx(), &reg), NodeStatus::Success);
    }

    #[test]
    fn repeat_times() {
        let mut bt = BehaviorTree::new(BtNode::Repeat {
            child: Box::new(BtNode::Action(ACT_INCREMENT)),
            count: BtRepeatCount::Times(3),
            current: 0,
        });
        let reg = test_registry();
        let ctx = test_ctx();
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Running); // 1/3
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Running); // 2/3
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Success); // 3/3
        assert_eq!(bt.blackboard().get_int("count"), Some(3));
    }

    #[test]
    fn reset_clears_state() {
        let mut bt = BehaviorTree::new(BtNode::Timeout {
            child: Box::new(BtNode::Action(ACT_RUNNING)),
            max_ticks: 5,
            elapsed: 0,
        });
        let reg = test_registry();
        let ctx = test_ctx();
        bt.tick(&ctx, &reg);
        bt.tick(&ctx, &reg);
        bt.reset();
        // After reset, elapsed should be 0 again
        assert_eq!(bt.tick(&ctx, &reg), NodeStatus::Running);
    }
}
