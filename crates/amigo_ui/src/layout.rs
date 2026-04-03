//! Retained-mode layout system (ADR-0011).
//!
//! Provides a declarative node tree with Flexbox-like layout that resolves to
//! absolute positions and emits [`UiDrawCommand`]s into the existing
//! [`UiContext`].
//!
//! Gated behind the `retained_layout` feature flag.

use amigo_core::{Color, Rect};

use crate::UiContext;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Opaque handle to a node inside a [`UiTree`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) usize);

/// How a dimension is specified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Fixed pixel value.
    Fixed(f32),
    /// Percentage of the parent's corresponding dimension (0.0 .. 1.0).
    Percent(f32),
    /// Determined by children / content.
    Auto,
}

impl Default for Size {
    fn default() -> Self {
        Size::Auto
    }
}

/// Direction of the main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    #[default]
    Column,
    Row,
}

/// How children are distributed along the main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

/// How children are aligned along the cross axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignItems {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// Spacing on each side of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Layout style for a [`UiNode`].
#[derive(Clone, Debug)]
pub struct Style {
    pub width: Size,
    pub height: Size,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub padding: Edges,
    pub margin: Edges,
    pub gap: f32,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub background: Option<Color>,
    pub border: Option<(Color, f32)>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            width: Size::Auto,
            height: Size::Auto,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: Edges::ZERO,
            margin: Edges::ZERO,
            gap: 0.0,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Start,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            background: None,
            border: None,
        }
    }
}

/// The visual content of a node.
#[derive(Clone, Debug, Default)]
pub enum NodeKind {
    /// A layout-only container (no visual content of its own).
    #[default]
    Container,
    /// A text label.
    Text {
        text: String,
        color: Color,
        scale: f32,
    },
    /// A clickable button with a label.
    Button {
        label: String,
        color: Color,
        bg: Color,
    },
    /// A sprite reference.
    Image { sprite_name: String },
}

// ---------------------------------------------------------------------------
// Node & Tree
// ---------------------------------------------------------------------------

/// A single node in the retained-mode UI tree.
#[derive(Clone, Debug)]
pub struct UiNode {
    pub style: Style,
    pub kind: NodeKind,
    parent: Option<usize>,
    children: Vec<usize>,
    /// Computed layout rect (set by [`UiTree::layout`]).
    pub(crate) computed_rect: Rect,
}

/// Arena-backed tree of [`UiNode`]s.
pub struct UiTree {
    nodes: Vec<UiNode>,
}

impl UiTree {
    /// Create an empty tree.
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add a root node (no parent).
    pub fn add_root(&mut self, style: Style, kind: NodeKind) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(UiNode {
            style,
            kind,
            parent: None,
            children: Vec::new(),
            computed_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
        });
        NodeId(id)
    }

    /// Add a child node under `parent`.
    pub fn add_node(&mut self, parent: NodeId, style: Style, kind: NodeKind) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(UiNode {
            style,
            kind,
            parent: Some(parent.0),
            children: Vec::new(),
            computed_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
        });
        self.nodes[parent.0].children.push(id);
        NodeId(id)
    }

    /// Access a node by id.
    pub fn node(&self, id: NodeId) -> &UiNode {
        &self.nodes[id.0]
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get the computed rect after layout.
    pub fn rect(&self, id: NodeId) -> Rect {
        self.nodes[id.0].computed_rect
    }

    // -----------------------------------------------------------------------
    // Layout
    // -----------------------------------------------------------------------

    /// Run the flexbox layout algorithm. Call once (or when the tree changes).
    pub fn layout(&mut self, available_width: f32, available_height: f32) {
        if self.nodes.is_empty() {
            return;
        }

        // Find root nodes (those without parents) and lay them out.
        let roots: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(i, _)| i)
            .collect();

        for root in roots {
            self.layout_node(root, 0.0, 0.0, available_width, available_height);
        }
    }

    fn layout_node(&mut self, idx: usize, x: f32, y: f32, available_w: f32, available_h: f32) {
        // 1. Resolve own size.
        let style = self.nodes[idx].style.clone();

        let content_w = self.resolve_size(style.width, available_w);
        let content_h = self.resolve_size(style.height, available_h);

        let node_w = clamp_size(
            content_w.unwrap_or(available_w),
            style.min_width,
            style.max_width,
        );
        let node_h = clamp_size(
            content_h.unwrap_or(available_h),
            style.min_height,
            style.max_height,
        );

        // Apply margin offset.
        let mx = x + style.margin.left;
        let my = y + style.margin.top;

        self.nodes[idx].computed_rect = Rect::new(mx, my, node_w, node_h);

        // 2. Layout children inside the padded content box.
        let children: Vec<usize> = self.nodes[idx].children.clone();
        if children.is_empty() {
            return;
        }

        let inner_x = mx + style.padding.left;
        let inner_y = my + style.padding.top;
        let inner_w = (node_w - style.padding.horizontal()).max(0.0);
        let inner_h = (node_h - style.padding.vertical()).max(0.0);

        let is_row = style.flex_direction == FlexDirection::Row;
        let main_size = if is_row { inner_w } else { inner_h };
        let cross_size = if is_row { inner_h } else { inner_w };
        let gap = style.gap;

        // Measure children's base sizes along the main axis.
        let child_count = children.len();
        let total_gaps = if child_count > 1 {
            gap * (child_count - 1) as f32
        } else {
            0.0
        };

        let mut child_main_sizes: Vec<f32> = Vec::with_capacity(child_count);
        let mut child_cross_sizes: Vec<f32> = Vec::with_capacity(child_count);
        let mut total_grow: f32 = 0.0;
        let mut total_shrink: f32 = 0.0;
        let mut base_main_total: f32 = 0.0;

        for &ci in &children {
            let cs = &self.nodes[ci].style;
            let cm = cs.margin;

            let (main_avail, cross_avail) = if is_row {
                (inner_w, inner_h)
            } else {
                (inner_h, inner_w)
            };

            let child_main = if is_row {
                self.resolve_size(cs.width, main_avail).unwrap_or(0.0) + cm.horizontal()
            } else {
                self.resolve_size(cs.height, main_avail).unwrap_or(0.0) + cm.vertical()
            };

            let child_cross = if is_row {
                self.resolve_size(cs.height, cross_avail)
                    .unwrap_or(cross_avail)
                    + cm.vertical()
            } else {
                self.resolve_size(cs.width, cross_avail)
                    .unwrap_or(cross_avail)
                    + cm.horizontal()
            };

            child_main_sizes.push(child_main);
            child_cross_sizes.push(child_cross);
            base_main_total += child_main;
            total_grow += cs.flex_grow;
            total_shrink += cs.flex_shrink;
        }

        // Distribute flex space.
        let free_space = main_size - base_main_total - total_gaps;

        if free_space > 0.0 && total_grow > 0.0 {
            // Grow
            for (i, &ci) in children.iter().enumerate() {
                let grow = self.nodes[ci].style.flex_grow;
                if grow > 0.0 {
                    child_main_sizes[i] += free_space * (grow / total_grow);
                }
            }
        } else if free_space < 0.0 && total_shrink > 0.0 {
            // Shrink
            let shrink_amount = -free_space;
            for (i, &ci) in children.iter().enumerate() {
                let shrink = self.nodes[ci].style.flex_shrink;
                if shrink > 0.0 {
                    child_main_sizes[i] =
                        (child_main_sizes[i] - shrink_amount * (shrink / total_shrink)).max(0.0);
                }
            }
        }

        // Compute starting offset for justify_content.
        let used_main: f32 = child_main_sizes.iter().sum::<f32>() + total_gaps;
        let remaining = (main_size - used_main).max(0.0);

        let (mut main_cursor, extra_gap) = match style.justify_content {
            JustifyContent::Start => (0.0, 0.0),
            JustifyContent::End => (remaining, 0.0),
            JustifyContent::Center => (remaining / 2.0, 0.0),
            JustifyContent::SpaceBetween => {
                if child_count > 1 {
                    (0.0, remaining / (child_count - 1) as f32)
                } else {
                    (0.0, 0.0)
                }
            }
            JustifyContent::SpaceAround => {
                let each = remaining / child_count as f32;
                (each / 2.0, each)
            }
        };

        // Position each child.
        for i in 0..child_count {
            let ci = children[i];
            let cm = child_main_sizes[i];
            let cc = child_cross_sizes[i];

            // Cross-axis alignment.
            let cross_offset = match style.align_items {
                AlignItems::Start => 0.0,
                AlignItems::End => cross_size - cc,
                AlignItems::Center => (cross_size - cc) / 2.0,
                AlignItems::Stretch => 0.0,
            };

            let (child_x, child_y, child_w, child_h) = if is_row {
                let cw = cm;
                let ch = if style.align_items == AlignItems::Stretch {
                    cross_size
                } else {
                    cc
                };
                (inner_x + main_cursor, inner_y + cross_offset, cw, ch)
            } else {
                let ch = cm;
                let cw = if style.align_items == AlignItems::Stretch {
                    cross_size
                } else {
                    cc
                };
                (inner_x + cross_offset, inner_y + main_cursor, cw, ch)
            };

            // Recurse into the child.
            self.layout_node(ci, child_x, child_y, child_w, child_h);

            main_cursor += cm + gap + extra_gap;
        }
    }

    fn resolve_size(&self, size: Size, available: f32) -> Option<f32> {
        match size {
            Size::Fixed(v) => Some(v),
            Size::Percent(p) => Some(available * p),
            Size::Auto => None,
        }
    }

    // -----------------------------------------------------------------------
    // Render
    // -----------------------------------------------------------------------

    /// Walk the tree depth-first and emit [`UiDrawCommand`]s into `ctx`.
    pub fn render(&self, ctx: &mut UiContext) {
        for (i, node) in self.nodes.iter().enumerate() {
            if node.parent.is_none() {
                self.render_node(i, ctx);
            }
        }
    }

    fn render_node(&self, idx: usize, ctx: &mut UiContext) {
        let node = &self.nodes[idx];
        let r = node.computed_rect;

        // Background
        if let Some(bg) = node.style.background {
            ctx.filled_rect(r, bg);
        }

        // Border
        if let Some((color, _width)) = node.style.border {
            ctx.rect_outline(r, color);
        }

        // Content
        let px = r.x + node.style.padding.left;
        let py = r.y + node.style.padding.top;

        match &node.kind {
            NodeKind::Container => {}
            NodeKind::Text { text, color, scale } => {
                if (*scale - 1.0).abs() < f32::EPSILON {
                    ctx.pixel_text(text, px, py, *color);
                } else {
                    ctx.pixel_text_scaled(text, px, py, *color, *scale);
                }
            }
            NodeKind::Button { label, color, bg } => {
                ctx.filled_rect(r, *bg);
                ctx.rect_outline(r, Color::new(0.6, 0.6, 0.6, 1.0));
                ctx.pixel_text(label, px, py, *color);
            }
            NodeKind::Image { sprite_name } => {
                ctx.sprite(sprite_name, px, py);
            }
        }

        // Recurse into children.
        for &ci in &node.children {
            self.render_node(ci, ctx);
        }
    }
}

impl Default for UiTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Builder API
// ---------------------------------------------------------------------------

/// Declarative builder for constructing a [`UiTree`].
///
/// # Example
/// ```ignore
/// let tree = UiTreeBuilder::new(320.0, 240.0)
///     .column(Style::default(), |b| {
///         b.text("Hello", Color::WHITE);
///         b.row(Style { gap: 4.0, ..Default::default() }, |b| {
///             b.button("OK", Color::WHITE, Color::new(0.3, 0.5, 0.3, 1.0));
///             b.button("Cancel", Color::WHITE, Color::new(0.5, 0.3, 0.3, 1.0));
///         });
///     })
///     .build();
/// ```
pub struct UiTreeBuilder {
    tree: UiTree,
    /// Stack of parent node indices for nesting.
    parent_stack: Vec<NodeId>,
    available_w: f32,
    available_h: f32,
}

impl UiTreeBuilder {
    pub fn new(available_w: f32, available_h: f32) -> Self {
        Self {
            tree: UiTree::new(),
            parent_stack: Vec::new(),
            available_w,
            available_h,
        }
    }

    /// Add a column container at the current level.
    pub fn column<F: FnOnce(&mut Self)>(mut self, style: Style, f: F) -> Self {
        let s = Style {
            flex_direction: FlexDirection::Column,
            ..style
        };
        let id = self.add_node_internal(s, NodeKind::Container);
        self.parent_stack.push(id);
        f(&mut self);
        self.parent_stack.pop();
        self
    }

    /// Add a row container at the current level.
    pub fn row<F: FnOnce(&mut Self)>(mut self, style: Style, f: F) -> Self {
        let s = Style {
            flex_direction: FlexDirection::Row,
            ..style
        };
        let id = self.add_node_internal(s, NodeKind::Container);
        self.parent_stack.push(id);
        f(&mut self);
        self.parent_stack.pop();
        self
    }

    /// Add a nested column inside a closure callback.
    pub fn nest_column<F: FnOnce(&mut Self)>(&mut self, style: Style, f: F) {
        let s = Style {
            flex_direction: FlexDirection::Column,
            ..style
        };
        let id = self.add_node_internal(s, NodeKind::Container);
        self.parent_stack.push(id);
        f(self);
        self.parent_stack.pop();
    }

    /// Add a nested row inside a closure callback.
    pub fn nest_row<F: FnOnce(&mut Self)>(&mut self, style: Style, f: F) {
        let s = Style {
            flex_direction: FlexDirection::Row,
            ..style
        };
        let id = self.add_node_internal(s, NodeKind::Container);
        self.parent_stack.push(id);
        f(self);
        self.parent_stack.pop();
    }

    /// Add a text node.
    pub fn text(&mut self, text: &str, color: Color) -> NodeId {
        self.add_node_internal(
            Style::default(),
            NodeKind::Text {
                text: text.to_string(),
                color,
                scale: 1.0,
            },
        )
    }

    /// Add a text node with custom style.
    pub fn text_styled(&mut self, text: &str, color: Color, scale: f32, style: Style) -> NodeId {
        self.add_node_internal(
            style,
            NodeKind::Text {
                text: text.to_string(),
                color,
                scale,
            },
        )
    }

    /// Add a button node.
    pub fn button(&mut self, label: &str, color: Color, bg: Color) -> NodeId {
        self.add_node_internal(
            Style {
                padding: Edges::symmetric(4.0, 6.0),
                ..Default::default()
            },
            NodeKind::Button {
                label: label.to_string(),
                color,
                bg,
            },
        )
    }

    /// Add an image / sprite node.
    pub fn image(&mut self, sprite_name: &str, style: Style) -> NodeId {
        self.add_node_internal(
            style,
            NodeKind::Image {
                sprite_name: sprite_name.to_string(),
            },
        )
    }

    /// Consume the builder and return a fully laid-out [`UiTree`].
    pub fn build(mut self) -> UiTree {
        self.tree.layout(self.available_w, self.available_h);
        self.tree
    }

    /// Consume the builder and return the tree *without* running layout.
    pub fn build_raw(self) -> UiTree {
        self.tree
    }

    // -- internal --

    fn add_node_internal(&mut self, style: Style, kind: NodeKind) -> NodeId {
        if let Some(&parent) = self.parent_stack.last() {
            self.tree.add_node(parent, style, kind)
        } else {
            self.tree.add_root(style, kind)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn clamp_size(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let v = if let Some(mn) = min {
        value.max(mn)
    } else {
        value
    };
    if let Some(mx) = max {
        v.min(mx)
    } else {
        v
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use amigo_core::Color;

    #[test]
    fn basic_tree_layout() {
        let mut tree = UiTree::new();
        let root = tree.add_root(
            Style {
                width: Size::Fixed(320.0),
                height: Size::Fixed(240.0),
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            NodeKind::Container,
        );

        let c1 = tree.add_node(
            root,
            Style {
                height: Size::Fixed(80.0),
                ..Default::default()
            },
            NodeKind::Container,
        );
        let c2 = tree.add_node(
            root,
            Style {
                height: Size::Fixed(80.0),
                ..Default::default()
            },
            NodeKind::Container,
        );
        let c3 = tree.add_node(
            root,
            Style {
                height: Size::Fixed(80.0),
                ..Default::default()
            },
            NodeKind::Container,
        );

        tree.layout(320.0, 240.0);

        // Root should be at origin with specified size.
        let rr = tree.rect(root);
        assert_eq!(rr.x, 0.0);
        assert_eq!(rr.y, 0.0);
        assert_eq!(rr.w, 320.0);
        assert_eq!(rr.h, 240.0);

        // Children should be within root and non-overlapping.
        let r1 = tree.rect(c1);
        let r2 = tree.rect(c2);
        let r3 = tree.rect(c3);

        assert!(r1.y + r1.h <= r2.y + 0.01, "c1 should be above c2");
        assert!(r2.y + r2.h <= r3.y + 0.01, "c2 should be above c3");
        assert!(r3.y + r3.h <= rr.y + rr.h + 0.01, "c3 within root");
    }

    #[test]
    fn flex_grow_distributes_space() {
        let mut tree = UiTree::new();
        let root = tree.add_root(
            Style {
                width: Size::Fixed(300.0),
                height: Size::Fixed(100.0),
                flex_direction: FlexDirection::Row,
                ..Default::default()
            },
            NodeKind::Container,
        );
        let a = tree.add_node(
            root,
            Style {
                width: Size::Fixed(50.0),
                flex_grow: 0.0,
                ..Default::default()
            },
            NodeKind::Container,
        );
        let b = tree.add_node(
            root,
            Style {
                flex_grow: 1.0,
                ..Default::default()
            },
            NodeKind::Container,
        );

        tree.layout(300.0, 100.0);

        let ra = tree.rect(a);
        let rb = tree.rect(b);
        assert!((ra.w - 50.0).abs() < 0.01);
        // b should get the remaining 250.
        assert!((rb.w - 250.0).abs() < 0.01, "b.w = {}", rb.w);
    }

    #[test]
    fn builder_api() {
        let tree = UiTreeBuilder::new(320.0, 240.0)
            .column(
                Style {
                    width: Size::Fixed(320.0),
                    height: Size::Fixed(240.0),
                    gap: 4.0,
                    ..Default::default()
                },
                |b| {
                    b.text("Hello", Color::WHITE);
                    b.nest_row(
                        Style {
                            gap: 4.0,
                            ..Default::default()
                        },
                        |b| {
                            b.button("OK", Color::WHITE, Color::new(0.3, 0.5, 0.3, 1.0));
                            b.button("Cancel", Color::WHITE, Color::new(0.5, 0.3, 0.3, 1.0));
                        },
                    );
                },
            )
            .build();

        // Root + text + row + 2 buttons = 5 nodes.
        assert_eq!(tree.len(), 5);
    }

    #[test]
    fn render_emits_commands() {
        let tree = UiTreeBuilder::new(320.0, 240.0)
            .column(
                Style {
                    width: Size::Fixed(320.0),
                    height: Size::Fixed(240.0),
                    background: Some(Color::new(0.0, 0.0, 0.0, 0.8)),
                    ..Default::default()
                },
                |b| {
                    b.text("Test", Color::WHITE);
                },
            )
            .build();

        let mut ctx = UiContext::new();
        ctx.begin();
        tree.render(&mut ctx);
        let cmds = ctx.end();

        // Should have at least a filled rect (bg) + text.
        assert!(
            cmds.len() >= 2,
            "expected >= 2 commands, got {}",
            cmds.len()
        );
    }

    #[test]
    fn justify_center() {
        let mut tree = UiTree::new();
        let root = tree.add_root(
            Style {
                width: Size::Fixed(200.0),
                height: Size::Fixed(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            NodeKind::Container,
        );
        let child = tree.add_node(
            root,
            Style {
                width: Size::Fixed(50.0),
                ..Default::default()
            },
            NodeKind::Container,
        );

        tree.layout(200.0, 100.0);

        let rc = tree.rect(child);
        // Child should be centered: (200 - 50) / 2 = 75.
        assert!((rc.x - 75.0).abs() < 0.01, "child.x = {}", rc.x);
    }
}
