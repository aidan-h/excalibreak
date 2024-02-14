use excali_render::wgpu::util::DeviceExt;
use excali_render::wgpu::{
    include_wgsl, CommandEncoderDescriptor, FragmentState, FrontFace, LoadOp, MultisampleState,
    Operations, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, TextureView, VertexState,
};
use excali_render::{wgpu, Renderer};

use crate::renderer::{Renderer3D, Vertex};

pub struct LineRenderer {
    render_pipeline: wgpu::RenderPipeline,
    vertices: usize,
    vertex_buffer: wgpu::Buffer,
}

fn create_vertex_buffer(vertices: &[Vertex], renderer: &Renderer) -> wgpu::Buffer {
    renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Renderer Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        })
}

impl LineRenderer {
    /// lines must be greater than 0
    pub fn new(renderer: &Renderer, renderer_3d: &Renderer3D, lines: usize) -> Self {
        let mut vertices = Vec::<Vertex>::new();
        for _ in 0..lines * 2 {
            vertices.push(Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]));
            vertices.push(Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]));
        }

        let shader = renderer
            .device
            .create_shader_module(include_wgsl!("line.wgsl"));

        let render_pipeline = renderer
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Line Render Pipeline"),
                layout: Some(&renderer.device.create_pipeline_layout(
                    &wgpu::PipelineLayoutDescriptor {
                        label: Some("Line Render Pipeline Layout"),
                        bind_group_layouts: &[&renderer_3d.camera_bind_group_layout],
                        push_constant_ranges: &[],
                    },
                )),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::descriptor()],
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &renderer_3d.targets,
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::LineList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: PolygonMode::Line,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview: None,
            });
        Self {
            render_pipeline,
            vertices: lines * 2,
            vertex_buffer: create_vertex_buffer(&vertices, renderer),
        }
    }

    pub fn draw(
        &mut self,
        vertices: Vec<Vertex>,
        renderer: &Renderer,
        renderer_3d: &Renderer3D,
        view: &TextureView,
    ) -> wgpu::CommandBuffer {
        let mut encoder = renderer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Line Command Encoder"),
            });
        if vertices.len() > self.vertices {
            self.vertex_buffer = create_vertex_buffer(&vertices, renderer);
        } else {
            renderer
                .queue
                .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Line Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &renderer_3d.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);
        drop(render_pass);
        encoder.finish()
    }
}
