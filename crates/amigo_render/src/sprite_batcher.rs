use crate::blend::BlendMode;
use crate::instancing::{InstanceData, InstancedBatch};
use crate::texture::TextureId;
use crate::vertex::Vertex;
use amigo_core::{Color, Rect};

// ---------------------------------------------------------------------------
// Per-Sprite Shaders (RS-03)
// ---------------------------------------------------------------------------

/// Visual shader effects that can be applied to individual sprites.
///
/// Multiple shaders can be stacked on a single sprite. The renderer
/// applies them in the order given.
#[derive(Clone, Debug)]
pub enum SpriteShader {
    /// Flash the sprite a solid color (hit feedback).
    Flash {
        color: Color,
        /// Progress 0.0 (full flash) to 1.0 (normal).
        progress: f32,
    },
    /// Draw a colored pixel outline around the sprite.
    Outline { color: Color, width: u8 },
    /// Dissolve the sprite into pixels.
    Dissolve {
        /// 0.0 = fully visible, 1.0 = fully dissolved.
        progress: f32,
        seed: u32,
    },
    /// Swap the sprite's color palette.
    PaletteSwap {
        source_palette: Vec<Color>,
        target_palette: Vec<Color>,
    },
    /// Render the sprite as a solid-color silhouette.
    Silhouette { color: Color },
    /// Apply a sine-wave distortion.
    Wave {
        amplitude: f32,
        frequency: f32,
        speed: f32,
    },
}

/// A single sprite to be rendered.
///
/// Build one with [`SpriteInstance::new`] and set the fields you need, or use
/// struct update syntax (`..SpriteInstance::new(..)`): the struct grows fields
/// over time.
#[derive(Clone, Debug)]
pub struct SpriteInstance {
    pub texture_id: TextureId,
    /// World (or screen, for the UI pass) position of the pivot.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    pub tint: Color,
    pub flip_x: bool,
    pub flip_y: bool,
    pub z_order: i32,
    /// Optional per-sprite shader effects (applied in order). Not rendered yet.
    pub shaders: Vec<SpriteShader>,
    /// Pivot, measured from the unrotated quad's top-left corner, in the same
    /// units as `width`/`height`. `[0.0, 0.0]` (the default) makes `x`/`y` the
    /// top-left corner.
    pub origin: [f32; 2],
    /// Rotation about `origin` in radians. Positive turns clockwise on screen
    /// (y points down).
    pub rotation: f32,
    /// How the sprite is composited.
    pub blend: BlendMode,
    /// Explicit corner geometry. When `Some`, the batcher uses these corners
    /// and colours and ignores `x`, `y`, `width`, `height`, `origin`,
    /// `rotation`, `flip_x` and `flip_y`.
    pub geometry: Option<QuadGeometry>,
}

/// Four corners in draw order top-left, top-right, bottom-right, bottom-left,
/// each with its own colour (multiplied by `tint`). A triangle repeats its
/// third corner as the fourth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadGeometry {
    pub corners: [[f32; 2]; 4],
    pub colors: [Color; 4],
}

impl QuadGeometry {
    /// A triangle: the third corner repeated as the fourth.
    pub fn triangle(points: [[f32; 2]; 3], colors: [Color; 3]) -> Self {
        Self {
            corners: [points[0], points[1], points[2], points[2]],
            colors: [colors[0], colors[1], colors[2], colors[2]],
        }
    }
}

/// `v` when finite, 0 otherwise: the frame path never drops a sprite for a bad
/// transform.
fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

impl SpriteInstance {
    /// A white-tinted, unrotated, unflipped sprite covering the whole texture,
    /// pivot at the top-left, z 0, [`BlendMode::Normal`], no geometry.
    pub fn new(texture_id: TextureId, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            texture_id,
            x,
            y,
            width,
            height,
            uv_x: 0.0,
            uv_y: 0.0,
            uv_w: 1.0,
            uv_h: 1.0,
            tint: Color::WHITE,
            flip_x: false,
            flip_y: false,
            z_order: 0,
            shaders: Vec::new(),
            origin: [0.0, 0.0],
            rotation: 0.0,
            blend: BlendMode::Normal,
            geometry: None,
        }
    }

    /// Scale `width`, `height` and `origin` together, so the pivot stays on
    /// the same point of the art. A negative factor mirrors the sprite about
    /// its pivot: the size stays positive, `flip_x`/`flip_y` toggles, and
    /// `origin` becomes `size - origin` on that axis.
    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.width *= sx.abs();
        self.origin[0] *= sx.abs();
        if sx < 0.0 {
            self.flip_x = !self.flip_x;
            self.origin[0] = self.width - self.origin[0];
        }
        self.height *= sy.abs();
        self.origin[1] *= sy.abs();
        if sy < 0.0 {
            self.flip_y = !self.flip_y;
            self.origin[1] = self.height - self.origin[1];
        }
    }

    /// Set `origin` as a fraction of the current size: `(0.5, 0.5)` is the
    /// centre, `(0.5, 1.0)` the bottom centre.
    pub fn set_origin_normalized(&mut self, nx: f32, ny: f32) {
        self.origin = [self.width * nx, self.height * ny];
    }

    /// World corners after origin and rotation (or `geometry` when set), in
    /// the order top-left, top-right, bottom-right, bottom-left.
    pub fn corners(&self) -> [[f32; 2]; 4] {
        if let Some(geometry) = &self.geometry {
            return geometry.corners;
        }
        let (w, h) = (self.width, self.height);
        let [ox, oy] = [
            finite_or_zero(self.origin[0]),
            finite_or_zero(self.origin[1]),
        ];
        let rotation = finite_or_zero(self.rotation);
        let local = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
        if rotation == 0.0 {
            if ox == 0.0 && oy == 0.0 {
                // Exactly the arithmetic the batcher used before pivots
                // existed, so unrotated sprites keep bit-identical vertices.
                return local.map(|[cx, cy]| [self.x + cx, self.y + cy]);
            }
            return local.map(|[cx, cy]| [self.x + (cx - ox), self.y + (cy - oy)]);
        }
        let (sin, cos) = rotation.sin_cos();
        local.map(|[cx, cy]| {
            let (dx, dy) = (cx - ox, cy - oy);
            // y points down, so this turns clockwise on screen.
            [self.x + dx * cos - dy * sin, self.y + dx * sin + dy * cos]
        })
    }

    /// Axis-aligned bounding box of [`corners`](Self::corners).
    pub fn bounds(&self) -> Rect {
        let corners = self.corners();
        let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
        let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for [x, y] in corners {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// The UVs of the four corners, flips applied (geometry ignores flips).
    fn corner_uvs(&self) -> [[f32; 2]; 4] {
        let flip_x = self.flip_x && self.geometry.is_none();
        let flip_y = self.flip_y && self.geometry.is_none();
        let (u0, u1) = if flip_x {
            (self.uv_x + self.uv_w, self.uv_x)
        } else {
            (self.uv_x, self.uv_x + self.uv_w)
        };
        let (v0, v1) = if flip_y {
            (self.uv_y + self.uv_h, self.uv_y)
        } else {
            (self.uv_y, self.uv_y + self.uv_h)
        };
        [[u0, v0], [u1, v0], [u1, v1], [u0, v1]]
    }

    /// The vertices the batcher writes for this sprite.
    pub fn vertices(&self) -> [Vertex; 4] {
        let corners = self.corners();
        let uvs = self.corner_uvs();
        let tint = self.tint;
        let colors = match &self.geometry {
            Some(g) => g
                .colors
                .map(|c| [c.r * tint.r, c.g * tint.g, c.b * tint.b, c.a * tint.a]),
            None => [tint.to_array(); 4],
        };
        [0, 1, 2, 3].map(|i| Vertex {
            position: corners[i],
            uv: uvs[i],
            color: colors[i],
        })
    }
}

/// Collects sprites per frame, sorts by texture, and generates vertex data.
pub struct SpriteBatcher {
    sprites: Vec<SpriteInstance>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    instances: Vec<InstanceData>,
}

/// One draw of a frame, in painter's order: indexed quads or an instanced run.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawBatch {
    Indexed(SpriteBatch),
    Instanced(InstancedBatch),
}

/// A batch of sprites sharing the same texture.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteBatch {
    pub texture_id: TextureId,
    /// Every sprite in the batch blends this way.
    pub blend: BlendMode,
    pub vertex_offset: u32,
    pub index_offset: u32,
    pub index_count: u32,
}

impl SpriteBatcher {
    pub fn new() -> Self {
        Self {
            sprites: Vec::with_capacity(1024),
            vertices: Vec::with_capacity(4096),
            indices: Vec::with_capacity(6144),
            instances: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.sprites.clear();
        self.vertices.clear();
        self.indices.clear();
        self.instances.clear();
    }

    pub fn push(&mut self, sprite: SpriteInstance) {
        self.sprites.push(sprite);
    }

    /// Sort sprites and generate vertex/index data. Returns one batch per run
    /// of consecutive sprites that share a texture and blend mode.
    pub fn build(&mut self) -> Vec<SpriteBatch> {
        self.build_hybrid(u32::MAX)
            .into_iter()
            .filter_map(|batch| match batch {
                DrawBatch::Indexed(batch) => Some(batch),
                DrawBatch::Instanced(_) => None,
            })
            .collect()
    }

    /// Sort sprites and build the frame's draws. A run of consecutive sprites
    /// sharing a texture and blend mode becomes one instanced draw when it
    /// has at least `threshold` sprites and every one of them can be an
    /// instance ([`InstanceData::from_sprite`]); otherwise it is indexed.
    /// Painter's order is kept across both kinds.
    pub fn build_hybrid(&mut self, threshold: u32) -> Vec<DrawBatch> {
        // Sort by z_order only. The sort is stable, so sprites on the same z
        // keep their submission order: painter's order is what callers mean.
        // Sorting by texture within a z (to save draw calls) layered same-z
        // sprites by texture-load order instead — a full-screen fade drawn
        // with the white texture (id 0) ended up under every sprite, health
        // bars under enemies, and a hot reload reordered layers.
        self.sprites.sort_by_key(|s| s.z_order);

        self.vertices.clear();
        self.indices.clear();
        self.instances.clear();

        let mut batches = Vec::new();
        let mut start = 0;
        while start < self.sprites.len() {
            // A run breaks when the texture or the blend mode changes, so
            // painter's order within a z holds across blend modes.
            let key = (self.sprites[start].texture_id, self.sprites[start].blend);
            let mut end = start + 1;
            while end < self.sprites.len()
                && (self.sprites[end].texture_id, self.sprites[end].blend) == key
            {
                end += 1;
            }
            let run = &self.sprites[start..end];

            let instances: Option<Vec<InstanceData>> = if run.len() as u64 >= threshold as u64 {
                run.iter().map(InstanceData::from_sprite).collect()
            } else {
                None
            };
            match instances {
                Some(instances) => {
                    batches.push(DrawBatch::Instanced(InstancedBatch {
                        texture_id: key.0,
                        blend: key.1,
                        instance_offset: self.instances.len() as u32,
                        instance_count: instances.len() as u32,
                    }));
                    self.instances.extend(instances);
                }
                None => {
                    let index_offset = self.indices.len() as u32;
                    for sprite in run {
                        let base_vertex = self.vertices.len() as u32;
                        self.vertices.extend_from_slice(&sprite.vertices());
                        // Two triangles per quad
                        self.indices.extend_from_slice(&[
                            base_vertex,
                            base_vertex + 1,
                            base_vertex + 2,
                            base_vertex,
                            base_vertex + 2,
                            base_vertex + 3,
                        ]);
                    }
                    batches.push(DrawBatch::Indexed(SpriteBatch {
                        texture_id: key.0,
                        blend: key.1,
                        vertex_offset: 0,
                        index_offset,
                        index_count: self.indices.len() as u32 - index_offset,
                    }));
                }
            }
            start = end;
        }
        batches
    }

    /// Instance data of the instanced draws of the last build.
    pub fn instances(&self) -> &[InstanceData] {
        &self.instances
    }

    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn sprite_count(&self) -> usize {
        self.sprites.len()
    }
}

impl Default for SpriteBatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite(texture: u32, z_order: i32) -> SpriteInstance {
        SpriteInstance {
            z_order,
            ..SpriteInstance::new(TextureId(texture), 0.0, 0.0, 1.0, 1.0)
        }
    }

    #[test]
    fn same_z_sprites_draw_in_submission_order() {
        let mut batcher = SpriteBatcher::new();
        batcher.push(sprite(5, 0)); // a sprite
        batcher.push(sprite(0, 0)); // then a fade rect over it (white texture)
        batcher.push(sprite(3, -1)); // background, lower z
        let order: Vec<u32> = batcher.build().iter().map(|b| b.texture_id.0).collect();
        assert_eq!(order, vec![3, 5, 0]);
    }

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4
    }

    #[test]
    fn new_has_the_documented_defaults() {
        let s = SpriteInstance::new(TextureId(7), 1.0, 2.0, 3.0, 4.0);
        assert_eq!(s.tint, Color::WHITE);
        assert_eq!(s.z_order, 0);
        assert!(!s.flip_x && !s.flip_y);
        assert_eq!(s.origin, [0.0, 0.0]);
        assert_eq!(s.rotation, 0.0);
        assert_eq!(s.blend, BlendMode::Normal);
        assert!(s.geometry.is_none());
        assert_eq!((s.uv_x, s.uv_y, s.uv_w, s.uv_h), (0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn an_unrotated_sprite_writes_the_same_vertices_as_before() {
        let mut s = SpriteInstance::new(TextureId(1), 3.25, -7.5, 16.0, 9.0);
        s.uv_x = 0.25;
        s.uv_w = 0.5;
        s.flip_x = true;
        s.tint = Color::new(0.5, 0.25, 1.0, 0.75);
        let v = s.vertices();
        // The pre-pivot arithmetic: x, x + w, y, y + h; flip swaps u0/u1.
        let positions = [
            [3.25, -7.5],
            [3.25 + 16.0, -7.5],
            [3.25 + 16.0, -7.5 + 9.0],
            [3.25, -7.5 + 9.0],
        ];
        let uvs = [[0.75, 0.0], [0.25, 0.0], [0.25, 1.0], [0.75, 1.0]];
        for i in 0..4 {
            assert_eq!(v[i].position, positions[i]);
            assert_eq!(v[i].uv, uvs[i]);
            assert_eq!(v[i].color, [0.5, 0.25, 1.0, 0.75]);
        }
    }

    #[test]
    fn rotation_turns_clockwise_about_the_origin() {
        let mut s = SpriteInstance::new(TextureId(1), 10.0, 10.0, 2.0, 4.0);
        s.set_origin_normalized(0.5, 0.5);
        s.rotation = std::f32::consts::FRAC_PI_2;
        let c = s.corners();
        assert!(close(c[0], [12.0, 9.0]), "{c:?}");
        assert!(close(c[1], [12.0, 11.0]), "{c:?}");
        assert!(close(c[2], [8.0, 11.0]), "{c:?}");
        assert!(close(c[3], [8.0, 9.0]), "{c:?}");
    }

    #[test]
    fn scale_keeps_the_pivot_and_mirrors_on_negative_factors() {
        let mut s = SpriteInstance::new(TextureId(1), 0.0, 0.0, 10.0, 6.0);
        s.origin = [2.0, 3.0];
        s.scale(2.0, 2.0);
        assert_eq!((s.width, s.height, s.origin), (20.0, 12.0, [4.0, 6.0]));
        s.scale(-1.0, 1.0);
        assert!(s.flip_x);
        assert_eq!(s.width, 20.0);
        assert_eq!(s.origin, [16.0, 6.0]);
    }

    #[test]
    fn origin_normalized_is_a_fraction_of_the_size() {
        let mut s = SpriteInstance::new(TextureId(1), 0.0, 0.0, 8.0, 6.0);
        s.set_origin_normalized(0.5, 1.0);
        assert_eq!(s.origin, [4.0, 6.0]);
    }

    #[test]
    fn corners_and_bounds_match_the_batched_vertices() {
        let mut s = SpriteInstance::new(TextureId(1), 5.0, 5.0, 4.0, 2.0);
        s.origin = [1.0, 1.0];
        s.rotation = 0.3;
        let mut batcher = SpriteBatcher::new();
        batcher.push(s.clone());
        batcher.build();
        let corners = s.corners();
        for (v, c) in batcher.vertices().iter().zip(corners) {
            assert!(close(v.position, c));
        }
        let b = s.bounds();
        for v in batcher.vertices() {
            assert!(v.position[0] >= b.x - 1e-4 && v.position[0] <= b.x + b.w + 1e-4);
            assert!(v.position[1] >= b.y - 1e-4 && v.position[1] <= b.y + b.h + 1e-4);
        }
    }

    #[test]
    fn non_finite_transforms_render_as_zero() {
        let mut s = SpriteInstance::new(TextureId(1), 1.0, 1.0, 2.0, 2.0);
        s.rotation = f32::NAN;
        s.origin = [f32::INFINITY, f32::NAN];
        let plain = SpriteInstance::new(TextureId(1), 1.0, 1.0, 2.0, 2.0);
        assert_eq!(s.corners(), plain.corners());
        let mut batcher = SpriteBatcher::new();
        batcher.push(s);
        assert_eq!(batcher.build().len(), 1);
        assert_eq!(batcher.vertices().len(), 4);
    }

    #[test]
    fn geometry_overrides_the_quad_and_multiplies_colours() {
        let mut s = SpriteInstance::new(TextureId(0), 100.0, 100.0, 1.0, 1.0);
        s.tint = Color::new(1.0, 1.0, 1.0, 0.5);
        s.flip_x = true;
        s.geometry = Some(QuadGeometry::triangle(
            [[0.0, 0.0], [4.0, 0.0], [0.0, 3.0]],
            [Color::RED, Color::GREEN, Color::BLUE],
        ));
        let v = s.vertices();
        assert_eq!(v[3].position, [0.0, 3.0]);
        assert_eq!(v[0].uv, [0.0, 0.0]);
        assert_eq!(v[0].color, [1.0, 0.0, 0.0, 0.5]);
    }

    #[test]
    fn long_plain_runs_are_instanced_in_painters_order() {
        let mut batcher = SpriteBatcher::new();
        batcher.push(sprite(1, 0)); // short run: indexed
        for _ in 0..100 {
            batcher.push(sprite(2, 1)); // long plain run: instanced
        }
        for i in 0..80 {
            let mut s = sprite(3, 2); // long run with one rotated sprite: indexed
            if i == 40 {
                s.rotation = 1.0;
            }
            batcher.push(s);
        }
        batcher.push(sprite(2, 3)); // same texture as the instanced run, later
        let batches = batcher.build_hybrid(64);
        assert_eq!(batches.len(), 4);
        assert!(matches!(&batches[0], DrawBatch::Indexed(b) if b.texture_id == TextureId(1)));
        assert_eq!(
            batches[1],
            DrawBatch::Instanced(InstancedBatch {
                texture_id: TextureId(2),
                blend: BlendMode::Normal,
                instance_offset: 0,
                instance_count: 100,
            })
        );
        assert!(matches!(&batches[2], DrawBatch::Indexed(b) if b.index_count == 80 * 6));
        assert!(matches!(&batches[3], DrawBatch::Indexed(b) if b.texture_id == TextureId(2)));
        assert_eq!(batcher.instances().len(), 100);
        // The indexed batches index into one shared vertex buffer.
        assert_eq!(batcher.vertices().len(), (1 + 80 + 1) * 4);
    }

    #[test]
    fn a_threshold_out_of_reach_keeps_everything_indexed() {
        let mut batcher = SpriteBatcher::new();
        for _ in 0..200 {
            batcher.push(sprite(2, 0));
        }
        let batches = batcher.build_hybrid(u32::MAX);
        assert!(batches.iter().all(|b| matches!(b, DrawBatch::Indexed(_))));
        assert!(batcher.instances().is_empty());
    }

    #[test]
    fn batches_break_on_blend_changes_and_keep_submission_order() {
        let mut batcher = SpriteBatcher::new();
        batcher.push(sprite(1, 0));
        let mut glow = sprite(1, 0);
        glow.blend = BlendMode::Additive;
        batcher.push(glow);
        batcher.push(sprite(1, 0));
        batcher.push(sprite(1, -1));
        let batches = batcher.build();
        let order: Vec<(u32, BlendMode, u32)> = batches
            .iter()
            .map(|b| (b.texture_id.0, b.blend, b.index_count / 6))
            .collect();
        assert_eq!(
            order,
            vec![
                (1, BlendMode::Normal, 2),
                (1, BlendMode::Additive, 1),
                (1, BlendMode::Normal, 1),
            ]
        );
    }
}
