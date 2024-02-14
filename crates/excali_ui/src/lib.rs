use egui::Context;
use egui_wgpu::renderer::Renderer;
use egui_wgpu::wgpu::{self, CommandBuffer, Device, Queue, TextureView};
pub use egui_winit;
use egui_winit::winit::event::Event;
use egui_winit::winit::window::{Window, WindowId};

pub struct UI {
    renderer: Renderer,
    winit_state: egui_winit::State,
    context: Context,
}

impl UI {
    pub fn new<T>(
        device: &wgpu::Device,
        event_loop_window_target: &egui_winit::winit::event_loop::EventLoopWindowTarget<T>,
    ) -> Self {
        let renderer = Renderer::new(device, wgpu::TextureFormat::Bgra8UnormSrgb, None, 1);
        let winit_state = egui_winit::State::new(event_loop_window_target);
        let context = egui::Context::default();
        Self {
            renderer,
            context,
            winit_state,
        }
    }

    pub fn handle_event(&mut self, event: &Event<()>, id: WindowId) -> bool {
        match event {
            Event::WindowEvent { window_id, event } => {
                if *window_id != id {
                    false
                } else {
                    self.winit_state.on_event(&self.context, event).consumed
                }
            }
            _ => false,
        }
    }

    pub fn update(
        &mut self,
        run_ui: impl FnOnce(&Context),
        device: &Device,
        queue: &Queue,
        view: &TextureView,
        window: &Window,
        window_size: [u32; 2],
    ) -> CommandBuffer {
        let input = self.winit_state.take_egui_input(window);
        let output = self.context.run(input, run_ui);
        for (id, image_delta) in output.textures_delta.set.iter() {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        for id in output.textures_delta.free.iter() {
            self.renderer.free_texture(id);
        }

        let triangles = self.context.tessellate(output.shapes);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui Command Encoder"),
        });

        let descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: window_size,
            pixels_per_point: 1.0,
        };
        self.renderer
            .update_buffers(device, queue, &mut encoder, &triangles, &descriptor);
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        self.renderer
            .render(&mut render_pass, &triangles, &descriptor);
        drop(render_pass);
        encoder.finish()
    }
}

pub trait Mode: ToString + std::marker::Sized {
    fn change(&self) -> Self;
    fn ui(&mut self, ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
        ui.horizontal(|ui| {
            ui.label(text);
            if ui.button(self.to_string()).clicked() {
                *self = self.change();
            }
        });
    }
}
