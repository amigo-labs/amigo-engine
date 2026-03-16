//! egui integration layer for the Amigo Engine renderer.
//!
//! Wraps `egui`, `egui-winit`, and `egui-wgpu` into a single `EguiRenderer`
//! that renders egui UI on top of the sprite pipeline. Gated behind the
//! `editor` feature flag.

pub use egui;
pub use egui_wgpu;
use egui_wgpu::ScreenDescriptor;
use tracing::info;

/// Manages egui state, winit event translation, and wgpu rendering.
pub struct EguiRenderer {
    ctx: egui::Context,
    winit_state: egui_winit::State,
    wgpu_renderer: egui_wgpu::Renderer,
}

impl EguiRenderer {
    /// Create a new EguiRenderer.
    ///
    /// Call this once during engine initialization, after creating the wgpu
    /// device and window.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        window: &winit::window::Window,
    ) -> Self {
        let ctx = egui::Context::default();

        let winit_state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let wgpu_renderer = egui_wgpu::Renderer::new(device, surface_format, None, 1, false);

        info!("egui editor overlay initialized");

        Self {
            ctx,
            winit_state,
            wgpu_renderer,
        }
    }

    /// The egui context. Pass this to your UI drawing code.
    pub fn context(&self) -> &egui::Context {
        &self.ctx
    }

    /// Forward a winit `WindowEvent` to egui. Returns `true` if egui consumed
    /// the event (i.e. the game should ignore it — e.g. mouse over an egui panel).
    pub fn handle_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        let response = self.winit_state.on_window_event(window, event);
        response.consumed
    }

    /// Returns `true` if the mouse pointer is over any egui area.
    pub fn wants_pointer_input(&self) -> bool {
        self.ctx.wants_pointer_input()
    }

    /// Returns `true` if egui wants keyboard input (e.g. a text field is focused).
    pub fn wants_keyboard_input(&self) -> bool {
        self.ctx.wants_keyboard_input()
    }

    /// Run an egui frame and render it on top of the given surface view.
    ///
    /// `run_ui` receives the `egui::Context` and should draw all egui
    /// windows/panels. A separate command encoder is created and submitted
    /// for the egui overlay pass.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        window: &winit::window::Window,
        surface_view: &wgpu::TextureView,
        screen_descriptor: ScreenDescriptor,
        mut run_ui: impl FnMut(&egui::Context),
    ) {
        let raw_input = self.winit_state.take_egui_input(window);
        let full_output = self.ctx.run(raw_input, &mut run_ui);

        self.winit_state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        for (id, image_delta) in &full_output.textures_delta.set {
            self.wgpu_renderer
                .update_texture(device, queue, *id, image_delta);
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui_encoder"),
        });

        self.wgpu_renderer
            .update_buffers(device, queue, &mut encoder, &tris, &screen_descriptor);

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve sprite output underneath
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // egui-wgpu requires RenderPass<'static>; wgpu 24 provides
            // forget_lifetime() to convert from the encoder-borrowing lifetime.
            let mut render_pass = render_pass.forget_lifetime();

            self.wgpu_renderer
                .render(&mut render_pass, &tris, &screen_descriptor);
        }

        queue.submit(std::iter::once(encoder.finish()));

        for id in &full_output.textures_delta.free {
            self.wgpu_renderer.free_texture(id);
        }
    }
}
