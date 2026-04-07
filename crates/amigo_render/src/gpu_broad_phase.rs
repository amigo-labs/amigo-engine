//! GPU-accelerated broad-phase collision detection via wgpu compute shader.
//!
//! Implements the [`BroadPhase`] trait from `amigo_core` using an N×N parallel
//! overlap check on the GPU. Each thread tests one unique (i, j) body pair for
//! AABB overlap. Matching pairs are written via atomic counter to a storage
//! buffer and read back to the CPU.
//!
//! For small body counts (< 32), falls back to [`CpuBroadPhase`] to avoid
//! GPU dispatch overhead.
//!
//! Gated behind `cfg(feature = "gpu_physics")`.

use std::sync::Arc;

use amigo_core::broad_phase::{BroadPhase, CollisionPair, CpuBroadPhase};
use amigo_core::ecs::EntityId;
use amigo_core::rect::Rect;
use bytemuck::{Pod, Zeroable};
use tracing::debug;

// ---------------------------------------------------------------------------
// GPU-side data types (must match gpu_broad_phase.wgsl layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuBody {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
    entity_index: u32,
    entity_gen: u32,
    _pad: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Params {
    body_count: u32,
    _pad: [u32; 3],
}

/// Minimum body count before switching from CPU to GPU.
const GPU_THRESHOLD: usize = 32;

/// Maximum supported bodies (limits buffer allocation).
const MAX_BODIES: usize = 8192;

// ---------------------------------------------------------------------------
// GpuBroadPhase
// ---------------------------------------------------------------------------

/// GPU-accelerated broad-phase using a wgpu compute shader.
///
/// Falls back to [`CpuBroadPhase`] when body count is below [`GPU_THRESHOLD`].
pub struct GpuBroadPhase {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Current buffer capacity in number of bodies.
    capacity: usize,
    param_buffer: wgpu::Buffer,
    body_buffer: wgpu::Buffer,
    output_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    fallback: CpuBroadPhase,
}

impl GpuBroadPhase {
    /// Create a new GPU broad-phase with the given wgpu device and queue.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_broad_phase"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_broad_phase.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_broad_phase_layout"),
            entries: &[
                // binding 0: Params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: Bodies storage (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: Output storage (read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_broad_phase_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gpu_broad_phase_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_overlap"),
            compilation_options: Default::default(),
            cache: None,
        });

        let initial_capacity = 256;
        let (param_buffer, body_buffer, output_buffer, staging_buffer) =
            Self::create_buffers(&device, initial_capacity);

        Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            capacity: initial_capacity,
            param_buffer,
            body_buffer,
            output_buffer,
            staging_buffer,
            fallback: CpuBroadPhase::new(),
        }
    }

    fn create_buffers(
        device: &wgpu::Device,
        capacity: usize,
    ) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer, wgpu::Buffer) {
        let body_size = (capacity * std::mem::size_of::<GpuBody>()) as u64;
        // Max pairs = N*(N-1)/2, each pair = 4 u32s = 16 bytes, plus 4 bytes for counter.
        let max_pairs = capacity * (capacity.saturating_sub(1)) / 2;
        let output_size = (4 + max_pairs * 16) as u64;

        let param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bp_params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let body_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bp_bodies"),
            size: body_size.max(32), // wgpu requires non-zero
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bp_output"),
            size: output_size.max(32),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bp_staging"),
            size: output_size.max(32),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        (param_buffer, body_buffer, output_buffer, staging_buffer)
    }

    /// Ensure buffers are large enough for `n` bodies, reallocating if needed.
    fn ensure_capacity(&mut self, n: usize) {
        if n <= self.capacity {
            return;
        }
        let new_capacity = n.next_power_of_two().min(MAX_BODIES);
        debug!(
            "GPU broad-phase: growing buffers {} → {}",
            self.capacity, new_capacity
        );
        let (param, body, output, staging) = Self::create_buffers(&self.device, new_capacity);
        self.param_buffer = param;
        self.body_buffer = body;
        self.output_buffer = output;
        self.staging_buffer = staging;
        self.capacity = new_capacity;
    }

    /// Run the GPU broad-phase. Falls back to CPU for small inputs.
    fn find_candidates_gpu(&mut self, bodies: &[(EntityId, Rect)]) -> Vec<CollisionPair> {
        let n = bodies.len();

        // CPU fallback for small counts or exceeding max.
        if !(GPU_THRESHOLD..=MAX_BODIES).contains(&n) {
            return self.fallback.find_candidates(bodies);
        }

        self.ensure_capacity(n);

        // Convert to GPU format.
        let gpu_bodies: Vec<GpuBody> = bodies
            .iter()
            .map(|(id, r)| GpuBody {
                min_x: r.x,
                min_y: r.y,
                max_x: r.x + r.w,
                max_y: r.y + r.h,
                entity_index: id.index(),
                entity_gen: id.generation(),
                _pad: [0; 2],
            })
            .collect();

        // Upload.
        let params = Params {
            body_count: n as u32,
            _pad: [0; 3],
        };
        self.queue
            .write_buffer(&self.param_buffer, 0, bytemuck::bytes_of(&params));
        self.queue
            .write_buffer(&self.body_buffer, 0, bytemuck::cast_slice(&gpu_bodies));

        // Clear the atomic counter (first 4 bytes of output).
        self.queue.write_buffer(&self.output_buffer, 0, &[0u8; 4]);

        // Dispatch compute.
        let total_pairs = (n * (n - 1) / 2) as u32;
        let workgroups = total_pairs.div_ceil(256);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bp_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.body_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.output_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bp_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("bp_compute"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy output to staging for readback.
        let output_size = self.output_buffer.size();
        encoder.copy_buffer_to_buffer(&self.output_buffer, 0, &self.staging_buffer, 0, output_size);

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map staging buffer and read back.
        let slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });
        self.device.poll(wgpu::Maintain::Wait);

        if rx.recv().map(|r| r.is_err()).unwrap_or(true) {
            // GPU readback failed — fall back to CPU.
            return self.fallback.find_candidates(bodies);
        }

        let data = slice.get_mapped_range();
        let u32_data: &[u32] = bytemuck::cast_slice(&data);

        let pair_count = u32_data[0] as usize;
        let mut pairs = Vec::with_capacity(pair_count);

        for i in 0..pair_count {
            let base = 1 + i * 4;
            if base + 3 >= u32_data.len() {
                break;
            }
            let a = EntityId::from_raw(u32_data[base], u32_data[base + 1]);
            let b = EntityId::from_raw(u32_data[base + 2], u32_data[base + 3]);
            pairs.push(CollisionPair::new(a, b));
        }

        drop(data);
        self.staging_buffer.unmap();

        pairs
    }
}

impl BroadPhase for GpuBroadPhase {
    fn find_candidates(&mut self, bodies: &[(EntityId, Rect)]) -> Vec<CollisionPair> {
        self.find_candidates_gpu(bodies)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // GPU tests require a real wgpu adapter. If unavailable, tests are skipped.

    fn try_create_gpu() -> Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("test"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;
        Some((Arc::new(device), Arc::new(queue)))
    }

    fn id(n: u32) -> EntityId {
        EntityId::from_raw(n, 0)
    }

    #[test]
    fn gpu_fallback_for_small_input() {
        // Below threshold: should use CPU fallback and produce correct results.
        let (device, queue) = match try_create_gpu() {
            Some(dq) => dq,
            None => return, // skip if no GPU
        };
        let mut bp = GpuBroadPhase::new(device, queue);

        // Two overlapping bodies (below GPU_THRESHOLD=32, uses CPU fallback).
        let bodies = vec![
            (
                id(1),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                },
            ),
            (
                id(2),
                Rect {
                    x: 5.0,
                    y: 5.0,
                    w: 10.0,
                    h: 10.0,
                },
            ),
        ];
        let pairs = bp.find_candidates(&bodies);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], CollisionPair::new(id(1), id(2)));
    }

    #[test]
    fn gpu_matches_cpu_many_bodies() {
        let (device, queue) = match try_create_gpu() {
            Some(dq) => dq,
            None => return,
        };
        let mut gpu = GpuBroadPhase::new(device, queue);
        let mut cpu = CpuBroadPhase::new();

        // Create 64 bodies in a grid — some will overlap.
        let mut bodies = Vec::new();
        for i in 0..8 {
            for j in 0..8 {
                let eid = id(i * 8 + j);
                let rect = Rect {
                    x: (j as f32) * 8.0,
                    y: (i as f32) * 8.0,
                    w: 10.0,
                    h: 10.0,
                };
                bodies.push((eid, rect));
            }
        }

        let mut gpu_pairs = gpu.find_candidates(&bodies);
        let mut cpu_pairs = cpu.find_candidates(&bodies);

        // Sort both for comparison.
        gpu_pairs.sort_by_key(|p| (p.a.index(), p.a.generation(), p.b.index(), p.b.generation()));
        cpu_pairs.sort_by_key(|p| (p.a.index(), p.a.generation(), p.b.index(), p.b.generation()));

        assert_eq!(
            gpu_pairs.len(),
            cpu_pairs.len(),
            "GPU found {} pairs, CPU found {}",
            gpu_pairs.len(),
            cpu_pairs.len()
        );
        assert_eq!(gpu_pairs, cpu_pairs);
    }

    #[test]
    fn gpu_no_overlaps() {
        let (device, queue) = match try_create_gpu() {
            Some(dq) => dq,
            None => return,
        };
        let mut bp = GpuBroadPhase::new(device, queue);

        // 64 bodies far apart — no overlaps.
        let bodies: Vec<_> = (0..64)
            .map(|i| {
                (
                    id(i),
                    Rect {
                        x: (i as f32) * 100.0,
                        y: 0.0,
                        w: 5.0,
                        h: 5.0,
                    },
                )
            })
            .collect();

        let pairs = bp.find_candidates(&bodies);
        assert!(pairs.is_empty());
    }

    #[test]
    fn gpu_empty_and_single() {
        let (device, queue) = match try_create_gpu() {
            Some(dq) => dq,
            None => return,
        };
        let mut bp = GpuBroadPhase::new(device, queue);

        assert!(bp.find_candidates(&[]).is_empty());
        assert!(bp
            .find_candidates(&[(
                id(1),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1.0,
                    h: 1.0
                }
            )])
            .is_empty());
    }
}
