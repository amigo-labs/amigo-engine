---
status: done
crate: amigo_debug
depends_on: []
last_updated: 2026-03-20
---

# Built-in Frame Timeline Profiler

## Purpose

Provide a self-contained frame timeline profiler inside the debug overlay so developers can diagnose performance problems without external tools (Tracy, Chrome tracing). The profiler records hierarchical system timing spans per frame, retains a configurable history ring buffer, and renders them as a horizontal timeline inside the existing `DebugOverlay` (toggled via a new F-key binding). It replaces the text-only `show_systems` table in `crates/amigo_debug/src/lib.rs` (lines 202-223) with a visual, zoomable timeline when the `builtin_profiler` feature flag is enabled, while keeping the text table as a fallback when the feature is off.

## Public API

```rust
// ── Feature gate: cfg(feature = "builtin_profiler") ──

/// Unique identifier for a profiler span within a single frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpanId(u32);

/// A completed timing span recorded during a frame.
#[derive(Clone, Debug)]
pub struct ProfileSpan {
    pub id: SpanId,
    pub parent: Option<SpanId>,
    pub name: &'static str,
    /// Offset from frame start, in microseconds.
    pub start_us: u64,
    /// Duration in microseconds.
    pub duration_us: u64,
    /// Depth in the span tree (0 = top-level system).
    pub depth: u16,
    /// Optional color tag for the timeline bar (e.g. hash of name).
    pub color: u32,
}

/// One frame's worth of recorded profiling data.
#[derive(Clone, Debug)]
pub struct FrameProfile {
    pub frame_number: u64,
    /// Total frame duration in microseconds.
    pub total_us: u64,
    /// Ordered list of spans (sorted by start_us).
    pub spans: Vec<ProfileSpan>,
}

/// Configuration for the profiler.
#[derive(Clone, Debug)]
pub struct ProfilerConfig {
    /// Number of frames retained in the ring buffer. Default: 300 (5 seconds at 60 fps).
    pub history_size: usize,
    /// If true, profiler records spans. If false, all begin/end calls are no-ops.
    pub enabled: bool,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            history_size: 300,
            enabled: true,
        }
    }
}

/// The frame timeline profiler. Owned by the engine, accessible via `DebugOverlay`.
pub struct FrameProfiler {
    config: ProfilerConfig,
    // ...internal state
}

impl FrameProfiler {
    pub fn new(config: ProfilerConfig) -> Self;

    /// Call at the very beginning of a frame. Records the frame-start timestamp
    /// and resets the per-frame span list.
    pub fn begin_frame(&mut self, frame_number: u64);

    /// Open a new span. Returns a `SpanId` that must be passed to `end_span`.
    /// Nests under the current open span (if any).
    /// `name` must be a `&'static str` to avoid allocation per frame.
    pub fn begin_span(&mut self, name: &'static str) -> SpanId;

    /// Close a previously opened span.
    pub fn end_span(&mut self, id: SpanId);

    /// Call at the end of a frame. Finalises the `FrameProfile`, pushes it
    /// into the ring buffer, and evicts the oldest frame if necessary.
    pub fn end_frame(&mut self);

    /// Convenience: time a closure as a named span.
    pub fn span<R>(&mut self, name: &'static str, f: impl FnOnce() -> R) -> R;

    /// Access the ring buffer of recorded frames. Index 0 is the oldest.
    pub fn history(&self) -> &[FrameProfile];

    /// Get the most recently completed frame profile.
    pub fn last_frame(&self) -> Option<&FrameProfile>;

    /// Reconfigure the profiler (e.g. resize history).
    pub fn set_config(&mut self, config: ProfilerConfig);

    /// Returns true if profiling is enabled.
    pub fn is_enabled(&self) -> bool;

    /// Clear all recorded history.
    pub fn clear(&mut self);
}

/// RAII span guard. Calls `end_span` on drop.
/// Obtained via `FrameProfiler::begin_span_guard`.
pub struct SpanGuard<'a> {
    profiler: &'a mut FrameProfiler,
    id: SpanId,
}

impl<'a> Drop for SpanGuard<'a> {
    fn drop(&mut self);
}

impl FrameProfiler {
    /// Open a span and return an RAII guard that closes it on drop.
    pub fn begin_span_guard(&mut self, name: &'static str) -> SpanGuard<'_>;
}

// ── Timeline renderer (for the debug overlay) ──

/// Rendering parameters for the timeline overlay.
#[derive(Clone, Debug)]
pub struct TimelineView {
    /// Which frame in history to center on (offset from latest). 0 = current.
    pub selected_frame_offset: usize,
    /// Horizontal zoom level. 1.0 = one frame fills the timeline width.
    pub zoom: f32,
    /// Vertical row height in pixels.
    pub row_height: f32,
    /// Whether the timeline is paused (freezes on the selected frame).
    pub paused: bool,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            selected_frame_offset: 0,
            zoom: 1.0,
            row_height: 18.0,
            paused: false,
        }
    }
}

/// Generates draw commands for the timeline overlay.
/// Returns a list of colored rectangles and text labels to be rendered
/// by the debug overlay's existing text/rect drawing facilities.
pub struct TimelineRenderer;

impl TimelineRenderer {
    /// Produce overlay draw data for the given frame profile and view settings.
    /// `viewport_width` and `viewport_height` are the overlay area dimensions in pixels.
    pub fn render(
        profile: &FrameProfile,
        view: &TimelineView,
        viewport_width: f32,
        viewport_height: f32,
    ) -> TimelineDrawData;
}

/// Output of the timeline renderer: rectangles and labels.
pub struct TimelineDrawData {
    /// Colored bars representing spans.
    pub bars: Vec<TimelineBar>,
    /// Text labels positioned on or next to bars.
    pub labels: Vec<TimelineLabel>,
    /// 16.66ms frame budget guideline position (x coordinate in pixels).
    pub budget_line_x: f32,
}

pub struct TimelineBar {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: amigo_core::Color,
}

pub struct TimelineLabel {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: amigo_core::Color,
}
```

## Behavior

### Normal Flow

1. Engine calls `profiler.begin_frame(frame_number)` at the top of the frame loop (in `crates/amigo_engine/src/engine.rs`, before the update phase).
2. Each system call is wrapped: `profiler.begin_span("physics")` ... `profiler.end_span(id)`. The existing `DebugOverlay::begin_system` / `end_system` calls (lib.rs lines 140-153) delegate to the `FrameProfiler` when the feature is enabled, preserving backward compatibility.
3. Spans nest automatically. If `begin_span("render")` is called while `begin_span("frame")` is still open, the render span becomes a child of the frame span.
4. `profiler.end_frame()` is called after `queue.submit()`. This finalizes the `FrameProfile` and pushes it into the ring buffer.
5. When the user presses the designated F-key (F9, added alongside the F1-F8 block at engine.rs lines 578-591), `DebugOverlay` activates the timeline view. The `TimelineRenderer` is called during overlay rendering with the selected `FrameProfile` from `history()`.

### Edge Cases

- If `end_span` is never called for an open span (e.g. a panic path), `end_frame` force-closes all open spans with the frame-end timestamp and logs a warning.
- If `begin_span` is called outside of a `begin_frame`/`end_frame` bracket, the call is a no-op.
- If the ring buffer is full, the oldest `FrameProfile` is silently overwritten.
- When `config.enabled` is `false`, all recording methods are no-ops (zero overhead beyond a branch).

### Ordering Guarantees

- Spans within a single `FrameProfile` are ordered by `start_us`.
- The `history()` slice is oldest-first.
- `SpanId` values are only valid within the frame that created them.

### Integration with Existing System Timings

When `builtin_profiler` is enabled, `DebugOverlay::begin_system(name)` internally calls `profiler.begin_span(name)` and `end_system()` calls `profiler.end_span(id)`. The `SystemTiming` struct (lib.rs lines 40-45) is still updated in parallel so the text overlay (`show_systems`) remains functional. Both views coexist.

## Internal Design

### Data Structures

- **Ring buffer**: `VecDeque<FrameProfile>` capped at `history_size`. Each `FrameProfile` owns a `Vec<ProfileSpan>` (typical frame has 10-30 spans, so allocation is small).
- **Open span stack**: A `Vec<(SpanId, &'static str, Instant)>` tracks the currently open spans for nesting. Cleared on `end_frame`.
- **Timestamps**: Use `std::time::Instant` for measurement. Store offsets as `u64` microseconds relative to the frame start `Instant` to keep `ProfileSpan` small (40 bytes).
- **Color hashing**: Span colors are derived from `name.as_ptr() as u32` for deterministic per-name coloring without a lookup table.

### Performance Considerations

- `begin_span` / `end_span` are the hottest calls. They must not allocate. The span `Vec` is pre-allocated to 64 entries per frame and reused.
- `name: &'static str` avoids per-frame string allocation. All system names in Amigo Engine are string literals.
- When `enabled` is `false`, the methods early-return after checking a single `bool` field (no `Instant::now()` call).
- The `TimelineRenderer` is only called when the overlay is visible, so it has no cost during normal gameplay.

### Timeline Rendering

The `TimelineRenderer` maps `FrameProfile` data to pixel coordinates:
- X axis: time within the frame. `start_us` maps to `x`, `duration_us` maps to `width`. A vertical guideline at 16,666 us marks the 60 fps budget.
- Y axis: span depth. Each depth level gets `row_height` pixels.
- Zoom and pan are controlled by `TimelineView` fields, adjustable via mouse wheel when the overlay is active.
- The renderer outputs abstract draw data (`TimelineDrawData`) rather than issuing GPU commands directly. The existing debug overlay rendering path in `amigo_render` consumes this data.

## Non-Goals

- **Replacing Tracy**: This profiler is for quick in-game diagnostics. Tracy remains the tool for deep analysis (memory allocations, lock contention, GPU timing). The `tracy` feature flag is independent.
- **GPU timing**: This profiler measures CPU-side spans only. GPU profiling (render pass timing via wgpu timestamp queries) is a separate future effort.
- **Network profiling**: The profiler does not capture network round-trip times or packet statistics. The F8 network debug overlay remains separate.
- **Persistent recording**: Frame profiles are not saved to disk. Export to Chrome trace JSON (`chrome://tracing`) is a possible future extension but not in scope.
- **Multi-threaded span tracking**: The profiler assumes single-threaded system execution (matching the current engine architecture). Thread-safe span recording is not a goal.

## Open Questions

- Should the timeline F-key be F9, or should we repurpose F4 (currently `show_paths`)? F9 keeps backward compatibility but is further from the existing F1-F8 block.
- Should `FrameProfile` include a snapshot of `DebugOverlay` stats (FPS, entity count, draw calls) for correlation, or should those be displayed separately alongside the timeline?
- Should the `span` convenience method accept a mutable reference to self and return `R`, or should we provide a macro `profile_span!("name", { ... })` that handles the borrow more ergonomically?

## Acceptance Criteria

### API Completeness

#### Core Types
- [ ] `SpanId(u32)` struct exists and derives `Clone, Copy, Debug, PartialEq, Eq, Hash`
- [ ] `ProfileSpan` struct exists with fields: `id: SpanId`, `parent: Option<SpanId>`, `name: &'static str`, `start_us: u64`, `duration_us: u64`, `depth: u16`, `color: u32`
- [ ] `ProfileSpan` derives `Clone, Debug`
- [ ] `FrameProfile` struct exists with fields: `frame_number: u64`, `total_us: u64`, `spans: Vec<ProfileSpan>`
- [ ] `FrameProfile` derives `Clone, Debug`
- [ ] `ProfilerConfig` struct exists with fields: `history_size: usize`, `enabled: bool`
- [ ] `ProfilerConfig` derives `Clone, Debug`
- [ ] `ProfilerConfig::default()` returns `history_size: 300, enabled: true`

#### FrameProfiler
- [ ] `FrameProfiler::new(config: ProfilerConfig) -> Self`
- [ ] `FrameProfiler::begin_frame(&mut self, frame_number: u64)`
- [ ] `FrameProfiler::begin_span(&mut self, name: &'static str) -> SpanId`
- [ ] `FrameProfiler::end_span(&mut self, id: SpanId)`
- [ ] `FrameProfiler::end_frame(&mut self)`
- [ ] `FrameProfiler::span<R>(&mut self, name: &'static str, f: impl FnOnce() -> R) -> R`
- [ ] `FrameProfiler::history(&self) -> &[FrameProfile]`
- [ ] `FrameProfiler::last_frame(&self) -> Option<&FrameProfile>`
- [ ] `FrameProfiler::set_config(&mut self, config: ProfilerConfig)`
- [ ] `FrameProfiler::is_enabled(&self) -> bool`
- [ ] `FrameProfiler::clear(&mut self)`
- [ ] `FrameProfiler::begin_span_guard(&mut self, name: &'static str) -> SpanGuard<'_>`

#### SpanGuard
- [ ] `SpanGuard<'a>` struct exists with fields: `profiler: &'a mut FrameProfiler`, `id: SpanId`
- [ ] `SpanGuard` implements `Drop` which calls `end_span`

#### Timeline Renderer Types
- [ ] `TimelineView` struct exists with fields: `selected_frame_offset: usize`, `zoom: f32`, `row_height: f32`, `paused: bool`
- [ ] `TimelineView::default()` returns `selected_frame_offset: 0, zoom: 1.0, row_height: 18.0, paused: false`
- [ ] `TimelineRenderer` struct exists
- [ ] `TimelineRenderer::render(profile: &FrameProfile, view: &TimelineView, viewport_width: f32, viewport_height: f32) -> TimelineDrawData`
- [ ] `TimelineDrawData` struct exists with fields: `bars: Vec<TimelineBar>`, `labels: Vec<TimelineLabel>`, `budget_line_x: f32`
- [ ] `TimelineBar` struct exists with fields: `x: f32`, `y: f32`, `width: f32`, `height: f32`, `color: Color`
- [ ] `TimelineLabel` struct exists with fields: `x: f32`, `y: f32`, `text: String`, `color: Color`

### Behavior

#### Normal Flow
- [ ] `begin_frame` records the frame-start timestamp and resets the per-frame span list
- [ ] `begin_span` opens a new span that nests under the current open span (if any)
- [ ] `begin_span` returns a `SpanId` for use with `end_span`
- [ ] `end_span` closes the span identified by `SpanId` and records its duration
- [ ] `end_frame` finalizes the `FrameProfile` and pushes it into the ring buffer
- [ ] `end_frame` evicts the oldest frame when the ring buffer is full
- [ ] `span()` convenience method times a closure as a named span and returns its result
- [ ] `begin_span_guard` returns an RAII guard that calls `end_span` on drop
- [ ] Spans nest automatically: a span opened while another is open becomes a child
- [ ] `history()` returns frames oldest-first (index 0 = oldest)
- [ ] `last_frame()` returns the most recently completed frame profile

#### Edge Cases
- [ ] If `end_span` is never called for an open span, `end_frame` force-closes all open spans with the frame-end timestamp and logs a warning
- [ ] If `begin_span` is called outside of a `begin_frame`/`end_frame` bracket, the call is a no-op
- [ ] If the ring buffer is full, the oldest `FrameProfile` is silently overwritten (no panic, no error)
- [ ] When `config.enabled` is `false`, all recording methods (`begin_frame`, `begin_span`, `end_span`, `end_frame`) are no-ops
- [ ] When `config.enabled` is `false`, no `Instant::now()` calls are made (zero overhead beyond a bool check)

#### Ordering Guarantees
- [ ] Spans within a single `FrameProfile` are sorted by `start_us`
- [ ] `SpanId` values are only valid within the frame that created them

#### Integration with DebugOverlay
- [ ] When `builtin_profiler` feature is enabled, `DebugOverlay::begin_system(name)` delegates to `profiler.begin_span(name)`
- [ ] When `builtin_profiler` feature is enabled, `DebugOverlay::end_system()` delegates to `profiler.end_span(id)`
- [ ] `SystemTiming` struct is still updated in parallel so the text overlay remains functional
- [ ] Both text and timeline views coexist

#### Timeline Rendering
- [ ] X axis maps `start_us` to x position and `duration_us` to width
- [ ] Y axis maps span `depth` to vertical row position using `row_height`
- [ ] A vertical guideline at 16,666 us marks the 60 fps budget (`budget_line_x`)
- [ ] Zoom and pan are controlled by `TimelineView` fields
- [ ] Renderer outputs abstract `TimelineDrawData`, not GPU commands directly

#### Performance
- [ ] `begin_span` / `end_span` do not allocate (span `Vec` is pre-allocated and reused)
- [ ] `name: &'static str` avoids per-frame string allocation
- [ ] `TimelineRenderer` is only called when the overlay is visible

#### Data Structures
- [ ] Ring buffer uses `VecDeque<FrameProfile>` capped at `history_size`
- [ ] Timestamps use `std::time::Instant`; offsets stored as `u64` microseconds relative to frame start
- [ ] Span colors derived from `name.as_ptr() as u32` for deterministic per-name coloring

### Quality Gates
- [ ] `cargo check --workspace` compiles without errors
- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] `cargo fmt --all --check` — correctly formatted
- [ ] New public API has at least one test per method
- [ ] No `unwrap()` in library code
- [ ] No `todo!()` or `unimplemented!()` in committed code

### Convention Compliance
- [ ] Crate is `amigo_debug` (amigo_ prefix, snake_case)
- [ ] All functionality gated behind `builtin_profiler` feature flag
- [ ] Logging uses `tracing` crate (e.g., warning when `end_frame` force-closes open spans)
- [ ] Error handling uses `thiserror` for any error types
- [ ] No `unwrap()` in library code; graceful handling of edge cases
- [ ] Traits use PascalCase; structs use PascalCase
- [ ] F-key toggle follows existing F1-F8 pattern in engine.rs
