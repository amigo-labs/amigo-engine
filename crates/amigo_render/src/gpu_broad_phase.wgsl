// GPU Broad-Phase Collision Detection — N×N Parallel Overlap Check
//
// Each thread maps to a unique (i, j) pair from the upper triangle of the
// N×N body matrix. If the two AABBs overlap, the pair is written to the
// output buffer via an atomic counter.

struct GpuBody {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
    entity_index: u32,
    entity_gen: u32,
    _pad0: u32,
    _pad1: u32,
};

struct Params {
    body_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> bodies: array<GpuBody>;
// output[0] = atomic pair count, followed by packed GpuPair data (4 u32s each)
@group(0) @binding(2) var<storage, read_write> output: array<atomic<u32>>;

@compute @workgroup_size(256)
fn cs_overlap(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = params.body_count;
    if (n < 2u) {
        return;
    }
    let total_pairs = n * (n - 1u) / 2u;
    let idx = gid.x;
    if (idx >= total_pairs) {
        return;
    }

    // Map linear index to unique pair (i, j) where i < j.
    // Derivation: idx = j*(j-1)/2 + i, solve for j then i.
    let jf = floor(0.5 + sqrt(0.25 + 2.0 * f32(idx)));
    let j = u32(jf);
    let i = idx - j * (j - 1u) / 2u;

    let a = bodies[i];
    let b = bodies[j];

    // AABB overlap test: overlap on both X and Y axes.
    if (a.min_x < b.max_x && a.max_x > b.min_x &&
        a.min_y < b.max_y && a.max_y > b.min_y) {

        // Canonical ordering: smaller EntityId first (compare index, then generation).
        var a_idx = a.entity_index;
        var a_gen = a.entity_gen;
        var b_idx = b.entity_index;
        var b_gen = b.entity_gen;

        let swap = (a_idx > b_idx) || (a_idx == b_idx && a_gen > b_gen);
        if (swap) {
            let tmp_idx = a_idx;
            let tmp_gen = a_gen;
            a_idx = b_idx;
            a_gen = b_gen;
            b_idx = tmp_idx;
            b_gen = tmp_gen;
        }

        // Atomically allocate a slot in the output buffer.
        let slot = atomicAdd(&output[0], 1u);
        // Each pair occupies 4 u32 values starting at offset 1 (after the counter).
        let base = 1u + slot * 4u;
        atomicStore(&output[base + 0u], a_idx);
        atomicStore(&output[base + 1u], a_gen);
        atomicStore(&output[base + 2u], b_idx);
        atomicStore(&output[base + 3u], b_gen);
    }
}
