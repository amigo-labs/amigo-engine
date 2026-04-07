//! Broad-phase collision detection abstractions.
//!
//! Provides a [`BroadPhase`] trait that decouples the physics world from any
//! specific spatial acceleration structure.  Two implementations ship today:
//!
//! * [`CpuBroadPhase`] -- sort-and-sweep on the X axis (always available).
//! * [`GpuBroadPhase`] -- placeholder for a future wgpu compute-shader path
//!   (radix sort + sweep on the GPU).  Currently delegates to `CpuBroadPhase`.

use crate::ecs::EntityId;
use crate::rect::Rect;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box used by the broad phase.
///
/// Unlike [`Rect`], this stores min/max coordinates directly which is the
/// natural representation for sort-and-sweep algorithms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Aabb {
    pub fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    /// Convert from an engine [`Rect`] (x, y, w, h) to an [`Aabb`].
    pub fn from_rect(r: &Rect) -> Self {
        Self {
            min_x: r.x,
            min_y: r.y,
            max_x: r.x + r.w,
            max_y: r.y + r.h,
        }
    }

    /// Check overlap with another AABB.
    #[inline]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.min_x < other.max_x
            && self.max_x > other.min_x
            && self.min_y < other.max_y
            && self.max_y > other.min_y
    }
}

/// An unordered pair of entities whose AABBs overlap in the broad phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CollisionPair {
    /// The entity with the smaller id (canonical ordering).
    pub a: EntityId,
    /// The entity with the larger id.
    pub b: EntityId,
}

impl CollisionPair {
    /// Create a canonical pair (smaller id first).
    pub fn new(a: EntityId, b: EntityId) -> Self {
        if a < b {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Broad-phase collision detection strategy.
///
/// Given a list of `(EntityId, Rect)` bodies, returns all pairs whose AABBs
/// overlap. The returned pairs are canonically ordered (smaller id first) and
/// deduplicated.
pub trait BroadPhase: Send {
    /// Compute candidate collision pairs from the given body AABBs.
    fn find_candidates(&mut self, bodies: &[(EntityId, Rect)]) -> Vec<CollisionPair>;
}

// ---------------------------------------------------------------------------
// CPU Sort-and-Sweep
// ---------------------------------------------------------------------------

/// CPU broad-phase using a sort-and-sweep (sweep-and-prune) algorithm on the
/// X axis.
///
/// Complexity: O(N log N) for the sort, plus O(N + K) for the sweep where K is
/// the number of overlapping pairs.  This beats spatial-hash for uniformly
/// distributed bodies and avoids hash-map overhead entirely.
pub struct CpuBroadPhase {
    /// Scratch buffer reused across frames to avoid allocation.
    sorted: Vec<(EntityId, Aabb)>,
}

impl CpuBroadPhase {
    pub fn new() -> Self {
        Self { sorted: Vec::new() }
    }
}

impl Default for CpuBroadPhase {
    fn default() -> Self {
        Self::new()
    }
}

impl BroadPhase for CpuBroadPhase {
    fn find_candidates(&mut self, bodies: &[(EntityId, Rect)]) -> Vec<CollisionPair> {
        // 1. Convert to Aabb and collect into scratch buffer.
        self.sorted.clear();
        self.sorted.reserve(bodies.len());
        for &(id, ref rect) in bodies {
            self.sorted.push((id, Aabb::from_rect(rect)));
        }

        // 2. Sort by min_x (sweep axis).
        self.sorted
            .sort_unstable_by(|a, b| a.1.min_x.partial_cmp(&b.1.min_x).unwrap());

        // 3. Sweep: for each body, walk forward while the next body's min_x is
        //    less than this body's max_x.  Check Y overlap for each candidate.
        let mut pairs = Vec::new();
        let len = self.sorted.len();
        for i in 0..len {
            let (id_a, ref aabb_a) = self.sorted[i];
            for j in (i + 1)..len {
                let (id_b, ref aabb_b) = self.sorted[j];
                // Early exit: no more overlaps on X axis.
                if aabb_b.min_x >= aabb_a.max_x {
                    break;
                }
                // Check Y overlap.
                if aabb_a.min_y < aabb_b.max_y && aabb_a.max_y > aabb_b.min_y {
                    pairs.push(CollisionPair::new(id_a, id_b));
                }
            }
        }

        pairs
    }
}

// ---------------------------------------------------------------------------
// GPU Broad-Phase (stub)
// ---------------------------------------------------------------------------

/// CPU-only GPU broad-phase fallback.
///
/// This stub delegates to [`CpuBroadPhase`] and exists for use when
/// the `gpu_physics` feature is not enabled. For the real GPU compute
/// shader implementation, see `amigo_render::gpu_broad_phase::GpuBroadPhase`
/// (enabled via the `gpu_physics` feature flag on the `amigo_render` crate).
pub struct GpuBroadPhase {
    /// Fallback used until the compute shader is implemented.
    fallback: CpuBroadPhase,
}

impl GpuBroadPhase {
    /// Create a new GPU broad-phase.
    ///
    /// In the future this will accept wgpu device/queue references and create
    /// the compute pipeline.  For now it simply wraps a [`CpuBroadPhase`].
    pub fn new() -> Self {
        Self {
            fallback: CpuBroadPhase::new(),
        }
    }
}

impl Default for GpuBroadPhase {
    fn default() -> Self {
        Self::new()
    }
}

impl BroadPhase for GpuBroadPhase {
    fn find_candidates(&mut self, bodies: &[(EntityId, Rect)]) -> Vec<CollisionPair> {
        // TODO: dispatch wgpu compute shader (radix sort + sweep).
        // For now, fall back to the CPU implementation.
        self.fallback.find_candidates(bodies)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    fn id(n: u32) -> EntityId {
        EntityId::from_raw(n, 0)
    }

    // -- Aabb -----------------------------------------------------------------

    #[test]
    fn aabb_overlaps() {
        let a = Aabb::new(0.0, 0.0, 10.0, 10.0);
        let b = Aabb::new(5.0, 5.0, 15.0, 15.0);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));

        let c = Aabb::new(20.0, 20.0, 30.0, 30.0);
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn aabb_touching_edges_do_not_overlap() {
        let a = Aabb::new(0.0, 0.0, 10.0, 10.0);
        let b = Aabb::new(10.0, 0.0, 20.0, 10.0);
        assert!(!a.overlaps(&b));
    }

    // -- CollisionPair --------------------------------------------------------

    // -- CpuBroadPhase --------------------------------------------------------

    #[test]
    fn cpu_finds_overlapping_pair() {
        let mut bp = CpuBroadPhase::new();
        let bodies = vec![
            (id(1), Rect::new(0.0, 0.0, 10.0, 10.0)),
            (id(2), Rect::new(5.0, 5.0, 10.0, 10.0)),
        ];
        let pairs = bp.find_candidates(&bodies);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], CollisionPair::new(id(1), id(2)));
    }

    #[test]
    fn cpu_no_overlap() {
        let mut bp = CpuBroadPhase::new();
        let bodies = vec![
            (id(1), Rect::new(0.0, 0.0, 10.0, 10.0)),
            (id(2), Rect::new(100.0, 100.0, 10.0, 10.0)),
        ];
        let pairs = bp.find_candidates(&bodies);
        assert!(pairs.is_empty());
    }

    #[test]
    fn cpu_x_overlap_but_not_y() {
        let mut bp = CpuBroadPhase::new();
        let bodies = vec![
            (id(1), Rect::new(0.0, 0.0, 10.0, 10.0)),
            (id(2), Rect::new(5.0, 50.0, 10.0, 10.0)),
        ];
        let pairs = bp.find_candidates(&bodies);
        assert!(pairs.is_empty());
    }

    #[test]
    fn cpu_multiple_bodies() {
        let mut bp = CpuBroadPhase::new();
        let bodies = vec![
            (id(1), Rect::new(0.0, 0.0, 20.0, 20.0)),
            (id(2), Rect::new(10.0, 10.0, 20.0, 20.0)),
            (id(3), Rect::new(15.0, 15.0, 20.0, 20.0)),
            (id(4), Rect::new(100.0, 100.0, 10.0, 10.0)),
        ];
        let pairs = bp.find_candidates(&bodies);
        // 1-2 overlap, 1-3 overlap, 2-3 overlap.  4 is isolated.
        assert_eq!(pairs.len(), 3);
        assert!(pairs.contains(&CollisionPair::new(id(1), id(2))));
        assert!(pairs.contains(&CollisionPair::new(id(1), id(3))));
        assert!(pairs.contains(&CollisionPair::new(id(2), id(3))));
    }

    // -- GpuBroadPhase (stub delegates to CPU) --------------------------------

    #[test]
    fn gpu_stub_finds_same_pairs_as_cpu() {
        let mut cpu = CpuBroadPhase::new();
        let mut gpu = GpuBroadPhase::new();

        // 4 bodies with known overlaps:
        //   A (0,0)-(20,20) overlaps B (10,10)-(30,30) and C (15,0)-(25,15)
        //   B overlaps C as well
        //   D (100,100)-(110,110) is isolated
        let bodies = vec![
            (id(1), Rect::new(0.0, 0.0, 20.0, 20.0)),     // A
            (id(2), Rect::new(10.0, 10.0, 20.0, 20.0)),   // B
            (id(3), Rect::new(15.0, 0.0, 10.0, 15.0)),    // C
            (id(4), Rect::new(100.0, 100.0, 10.0, 10.0)), // D (isolated)
        ];

        let mut cpu_pairs = cpu.find_candidates(&bodies);
        let mut gpu_pairs = gpu.find_candidates(&bodies);

        // Sort for deterministic comparison
        cpu_pairs.sort_by_key(|p| (p.a, p.b));
        gpu_pairs.sort_by_key(|p| (p.a, p.b));

        // Both should return the exact same pairs
        assert_eq!(
            cpu_pairs, gpu_pairs,
            "CPU and GPU broad phase must return identical pairs"
        );

        // Verify the expected pair count: A-B, A-C, B-C = 3 pairs
        assert_eq!(
            cpu_pairs.len(),
            3,
            "Expected 3 overlapping pairs (A-B, A-C, B-C), got {}",
            cpu_pairs.len()
        );

        // Verify specific expected pairs exist
        assert!(
            cpu_pairs.contains(&CollisionPair::new(id(1), id(2))),
            "A-B should overlap"
        );
        assert!(
            cpu_pairs.contains(&CollisionPair::new(id(1), id(3))),
            "A-C should overlap"
        );
        assert!(
            cpu_pairs.contains(&CollisionPair::new(id(2), id(3))),
            "B-C should overlap"
        );

        // D should not appear in any pair
        assert!(
            !cpu_pairs.iter().any(|p| p.a == id(4) || p.b == id(4)),
            "Isolated body D should not appear in any collision pair"
        );
    }
}
