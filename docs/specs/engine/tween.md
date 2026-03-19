---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Tween System

## Purpose

Interpoliert Werte über Zeit mit konfigurierbaren Easing-Funktionen. Unverzichtbar für UI-Animationen (Menü-Einblendungen), Tower-Effekte (Bounce beim Platzieren), Kamera-Bewegungen, und jede "polished" Spielgefühl-Verbesserung. Ergänzt das Spline-System ([spline](spline.md)) — Splines interpolieren entlang Pfaden, Tweens interpolieren einzelne Werte über Zeit.

## Existierende Bausteine

- `Easing` enum in `crates/amigo_render/src/camera.rs` (Linear, EaseIn, EaseOut, EaseInOut) — wird erweitert und nach `amigo_core` verschoben
- `RenderVec2::lerp()` in `crates/amigo_core/src/math.rs`
- `CatmullRomSpline` / `CubicBezier` in `crates/amigo_core/src/spline.rs` für Pfad-Tweens

## Public API

### Tweenable Trait

```rust
/// Trait für Typen, die interpoliert werden können.
pub trait Tweenable: Clone {
    fn lerp(&self, target: &Self, t: f32) -> Self;
}

// Built-in Implementierungen:
impl Tweenable for f32 { .. }          // Lineare Interpolation
impl Tweenable for RenderVec2 { .. }   // Komponenten-weise Interpolation
impl Tweenable for Color { .. }        // RGBA-Interpolation
impl Tweenable for Fix { .. }          // Fixed-Point Interpolation
impl Tweenable for SimVec2 { .. }      // Simulation-Space Interpolation
```

### EasingFn

```rust
/// Vollständige Easing-Bibliothek (Penner-Kurven).
#[derive(Clone, Copy, Debug)]
pub enum EasingFn {
    Linear,
    // Quadratic
    QuadIn, QuadOut, QuadInOut,
    // Cubic
    CubicIn, CubicOut, CubicInOut,
    // Quartic
    QuartIn, QuartOut, QuartInOut,
    // Quintic
    QuintIn, QuintOut, QuintInOut,
    // Sine
    SineIn, SineOut, SineInOut,
    // Exponential
    ExpoIn, ExpoOut, ExpoInOut,
    // Circular
    CircIn, CircOut, CircInOut,
    // Elastic (spring-like overshoot)
    ElasticIn, ElasticOut, ElasticInOut,
    // Back (overshoot and return)
    BackIn, BackOut, BackInOut,
    // Bounce (ball-drop effect)
    BounceIn, BounceOut, BounceInOut,
}

impl EasingFn {
    /// Berechnet den eased t-Wert. Input und Output in [0.0, 1.0]
    /// (Elastic/Back können kurzzeitig außerhalb liegen).
    pub fn apply(self, t: f32) -> f32;
}
```

Die bestehende `Easing` enum in `amigo_render/src/camera.rs` wird durch einen Re-Export von `EasingFn` ersetzt (Linear→Linear, EaseIn→QuadIn, EaseOut→QuadOut, EaseInOut→QuadInOut).

### Tween

```rust
/// Ein aktiver Tween, der einen Wert von `from` nach `to` über `duration` Ticks interpoliert.
pub struct Tween<T: Tweenable> {
    from: T,
    to: T,
    easing: EasingFn,
    elapsed: f32,
    duration: f32,
    state: TweenState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TweenState {
    Running,
    Paused,
    Completed,
}

impl<T: Tweenable> Tween<T> {
    pub fn new(from: T, to: T, duration: f32, easing: EasingFn) -> Self;
    pub fn current(&self) -> T;
    pub fn progress(&self) -> f32;  // 0.0..1.0
    pub fn is_complete(&self) -> bool;
    pub fn update(&mut self, dt: f32);
    pub fn pause(&mut self);
    pub fn resume(&mut self);
    pub fn reset(&mut self);
}
```

### TweenSequence

```rust
/// Sequenz von Tweens mit Verkettung, Delay, Repeat und Yoyo.
pub struct TweenSequence<T: Tweenable> {
    steps: Vec<TweenStep<T>>,
    current_step: usize,
    repeat_count: RepeatCount,
    yoyo: bool,
    direction_forward: bool,
}

enum TweenStep<T: Tweenable> {
    Animate { tween: Tween<T> },
    Delay { remaining: f32 },
}

#[derive(Clone, Copy, Debug)]
pub enum RepeatCount {
    Once,
    Times(u32),
    Forever,
}

impl<T: Tweenable> TweenSequence<T> {
    pub fn new(from: T, to: T, duration: f32, easing: EasingFn) -> Self;
    pub fn then(self, to: T, duration: f32, easing: EasingFn) -> Self;
    pub fn delay(self, duration: f32) -> Self;
    pub fn repeat(self, count: RepeatCount) -> Self;
    pub fn yoyo(self) -> Self;  // Setzt repeat + direction reversal
    pub fn current(&self) -> T;
    pub fn is_complete(&self) -> bool;
    pub fn update(&mut self, dt: f32);
}
```

### TweenHandle & TweenManager

```rust
/// Opakes Handle für Steuerung eines registrierten Tweens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TweenHandle(u32);

/// Zentraler Manager, der alle aktiven Tweens pro Tick aktualisiert.
pub struct TweenManager {
    next_id: u32,
    // Type-erased storage (Box<dyn ErasedTween>)
}

impl TweenManager {
    pub fn new() -> Self;

    /// Registriert einen Tween und gibt ein Handle zurück.
    pub fn start<T: Tweenable + 'static>(
        &mut self,
        tween: Tween<T>,
        on_update: impl Fn(&T) + Send + 'static,
    ) -> TweenHandle;

    /// Registriert eine TweenSequence.
    pub fn start_sequence<T: Tweenable + 'static>(
        &mut self,
        seq: TweenSequence<T>,
        on_update: impl Fn(&T) + Send + 'static,
    ) -> TweenHandle;

    /// Aktualisiert alle aktiven Tweens. Entfernt abgeschlossene.
    pub fn update(&mut self, dt: f32);

    pub fn pause(&mut self, handle: TweenHandle);
    pub fn resume(&mut self, handle: TweenHandle);
    pub fn cancel(&mut self, handle: TweenHandle);
    pub fn is_active(&self, handle: TweenHandle) -> bool;
    pub fn active_count(&self) -> usize;
}
```

## Behavior

- **Update-Zyklus**: `TweenManager::update(dt)` wird einmal pro Frame aufgerufen. Iteriert über alle aktiven Tweens, ruft `tween.update(dt)` auf, feuert `on_update` Callback mit dem interpolierten Wert, entfernt abgeschlossene Tweens.
- **Easing**: Input `t` wird durch `EasingFn::apply()` transformiert, dann als Parameter für `Tweenable::lerp()` genutzt. Elastic/Back-Kurven können Werte außerhalb [0, 1] erzeugen — `lerp` muss dies korrekt handhaben (Extrapolation).
- **Sequencing**: `then()` hängt einen neuen Tween an, dessen `from` automatisch der `to` des vorherigen ist. `delay()` fügt eine Pause ein. `yoyo()` setzt `repeat(RepeatCount::Times(2))` und kehrt die Richtung um.
- **Cancellation**: `cancel()` entfernt den Tween sofort. Kein finaler Callback.
- **Pausierung**: Pausierte Tweens werden von `update()` übersprungen, bleiben aber registriert.

## Internal Design

- Type Erasure via `Box<dyn ErasedTween>` trait mit `update(dt)`, `is_complete()`, `pause()`, `resume()` Methoden. Der `on_update` Callback ist im Trait-Objekt eingeschlossen.
- `FxHashMap<TweenHandle, Box<dyn ErasedTween>>` für O(1) Zugriff via Handle.
- Kein Allocator-Druck: Tweens sind kurzlebig (typisch 0.2-2s), abgeschlossene werden in `update()` entfernt.

## Non-Goals

- **Serialisierung.** Tweens sind flüchtige visuelle Effekte, nicht teil des Spielzustands. Kein Save/Load Support.
- **ECS-Integration als Component.** Tweens werden über Callbacks gesteuert, nicht als ECS-Komponenten. Ein `TweenComponent` würde den ECS-Storage mit kurzlebigen Daten belasten.
- **Pfad-Tweens.** Interpolation entlang komplexer Pfade wird vom [Spline-System](spline.md) übernommen. Tween deckt nur A→B Übergänge ab.
- **Async/Await.** Kein async Runtime. Tweens sind synchron, tickbasiert.

## Open Questions

- Soll `TweenManager` nach Entity gruppieren können, um alle Tweens einer Entity auf einmal zu cancellen?
- Braucht es ein `on_complete` Callback zusätzlich zu `on_update`?
- Soll die bestehende `Easing` enum in `amigo_render` komplett durch `EasingFn` ersetzt werden, oder bleibt sie als vereinfachtes Subset?

## Referenzen

- DOTween (Unity) als API-Vorbild
- [engine/spline](spline.md) → CatmullRom/Bezier für Pfad-Interpolation
- [engine/camera](camera.md) → Kamera-Shake und CinematicPan nutzen Easing
- [engine/animation](animation.md) → Sprite-Animation als übergeordnetes System
- [engine/timeline](timeline.md) → Timeline baut auf Tween-Sequenzen auf
