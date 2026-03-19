use crate::texture::TextureId;
use crate::vertex::Vertex;
use amigo_core::Color;

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
#[derive(Clone, Debug)]
pub struct SpriteInstance {
    pub texture_id: TextureId,
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
    /// Optional per-sprite shader effects (applied in order).
    pub shaders: Vec<SpriteShader>,
}

/// Collects sprites per frame, sorts by texture, and generates vertex data.
pub struct SpriteBatcher {
    sprites: Vec<SpriteInstance>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

/// A batch of sprites sharing the same texture.
pub struct SpriteBatch {
    pub texture_id: TextureId,
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
        }
    }

    pub fn clear(&mut self) {
        self.sprites.clear();
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn push(&mut self, sprite: SpriteInstance) {
        self.sprites.push(sprite);
    }

    /// Sort sprites and generate vertex/index data. Returns batches grouped by texture.
    pub fn build(&mut self) -> Vec<SpriteBatch> {
        // Sort by z_order, then by texture to minimize draw calls
        self.sprites.sort_by(|a, b| {
            a.z_order
                .cmp(&b.z_order)
                .then(a.texture_id.0.cmp(&b.texture_id.0))
        });

        self.vertices.clear();
        self.indices.clear();

        let mut batches = Vec::new();
        let mut current_texture: Option<TextureId> = None;
        let mut batch_index_start = 0u32;

        for sprite in &self.sprites {
            // Start new batch if texture changed
            if current_texture != Some(sprite.texture_id) {
                if let Some(tex_id) = current_texture {
                    let index_count = self.indices.len() as u32 - batch_index_start;
                    if index_count > 0 {
                        batches.push(SpriteBatch {
                            texture_id: tex_id,
                            vertex_offset: 0,
                            index_offset: batch_index_start,
                            index_count,
                        });
                    }
                }
                current_texture = Some(sprite.texture_id);
                batch_index_start = self.indices.len() as u32;
            }

            let base_vertex = self.vertices.len() as u32;

            // UV coordinates with flip support
            let (u0, u1) = if sprite.flip_x {
                (sprite.uv_x + sprite.uv_w, sprite.uv_x)
            } else {
                (sprite.uv_x, sprite.uv_x + sprite.uv_w)
            };
            let (v0, v1) = if sprite.flip_y {
                (sprite.uv_y + sprite.uv_h, sprite.uv_y)
            } else {
                (sprite.uv_y, sprite.uv_y + sprite.uv_h)
            };

            let color = sprite.tint.to_array();

            // Top-left, top-right, bottom-right, bottom-left
            self.vertices.push(Vertex {
                position: [sprite.x, sprite.y],
                uv: [u0, v0],
                color,
            });
            self.vertices.push(Vertex {
                position: [sprite.x + sprite.width, sprite.y],
                uv: [u1, v0],
                color,
            });
            self.vertices.push(Vertex {
                position: [sprite.x + sprite.width, sprite.y + sprite.height],
                uv: [u1, v1],
                color,
            });
            self.vertices.push(Vertex {
                position: [sprite.x, sprite.y + sprite.height],
                uv: [u0, v1],
                color,
            });

            // Two triangles per quad
            self.indices.push(base_vertex);
            self.indices.push(base_vertex + 1);
            self.indices.push(base_vertex + 2);
            self.indices.push(base_vertex);
            self.indices.push(base_vertex + 2);
            self.indices.push(base_vertex + 3);
        }

        // Finalize last batch
        if let Some(tex_id) = current_texture {
            let index_count = self.indices.len() as u32 - batch_index_start;
            if index_count > 0 {
                batches.push(SpriteBatch {
                    texture_id: tex_id,
                    vertex_offset: 0,
                    index_offset: batch_index_start,
                    index_count,
                });
            }
        }

        batches
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
