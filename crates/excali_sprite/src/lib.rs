use nalgebra::Vector2;
use wgpu::util::DeviceExt;
use wgpu::*;

const STARTING_LENGTH: u16 = 16;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WindowUnifrom {
    pub size: [f32; 2],
}

type VertexTextureCoordinate = [f32; 2];

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: VertexTextureCoordinate,
}

impl Vertex {
    fn descriptor<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

fn indices(sprites: u16) -> Vec<u16> {
    let mut indicies = Vec::<u16>::new();

    for i in 0..sprites {
        let offset = i * 4;
        indicies.push(offset);
        indicies.push(1 + offset);
        indicies.push(2 + offset);
        indicies.push(2 + offset);
        indicies.push(3 + offset);
        indicies.push(offset);
    }

    indicies
}

pub struct SpriteRenderer {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    window_buffer: Buffer,
    pipeline: RenderPipeline,
    window_bind_group: BindGroup,
    texture_bind_group_layout: BindGroupLayout,
    length: u16,
}

#[derive(Debug)]
pub struct TextureCoordinate {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}

impl Default for TextureCoordinate {
    fn default() -> Self {
        Self {
            width: 1.0,
            height: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }
}
impl TextureCoordinate {
    fn bottom_left(&self) -> VertexTextureCoordinate {
        [self.x, self.y]
    }

    fn bottom_right(&self) -> VertexTextureCoordinate {
        [self.x + self.width, self.y]
    }

    fn top_left(&self) -> VertexTextureCoordinate {
        [self.x, self.y + self.height]
    }

    fn top_right(&self) -> VertexTextureCoordinate {
        [self.x + self.width, self.y + self.height]
    }
}

#[derive(Copy, Clone)]
pub struct Transform {
    pub position: Vector2<f32>,
    pub rotation: f32,
    pub scale: Vector2<f32>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vector2::zeros(),
            rotation: 0.0,
            scale: Vector2::zeros(),
        }
    }
}

pub struct Sprite {
    pub transform: Transform,
    pub texture_coordinate: TextureCoordinate,
}

impl Sprite {
    fn vertices(&self) -> [Vertex; 4] {
        let position = self.transform.position;
        let rotation = self.transform.rotation;
        let scale = self.transform.scale / 2.0;
        let sin = rotation.sin();
        let cos = rotation.cos();

        let mut top_right =Vector2::new(scale.x * cos + scale.y * sin, scale.y * cos - scale.x * sin);
        let bottom_left = position - top_right;
        top_right += position;

        let mut top_left =Vector2::new(-scale.x * cos + scale.y * sin, scale.y * cos + scale.x * sin);
        let bottom_right= position - top_left;
        top_left += position;

        [
            Vertex {
                position: [bottom_left.x, bottom_left.y],
                tex_coords: self.texture_coordinate.bottom_left(),
            },
            Vertex {
                position: [bottom_right.x, bottom_right.y],
                tex_coords: self.texture_coordinate.bottom_right(),
            },
            Vertex {
                position: [top_right.x, top_right.y],
                tex_coords: self.texture_coordinate.top_right(),
            },
            Vertex {
                position: [top_left.x, top_left.y],
                tex_coords: self.texture_coordinate.top_left(),
            },
        ]
    }
}

pub struct SpriteBatch<'a> {
    pub sprites: Vec<Sprite>,
    pub texture_bind_group: &'a BindGroup,
}

fn create_vertex_buffer(sprite_count: u16, device: &Device) -> wgpu::Buffer {
    device.create_buffer_init(&util::BufferInitDescriptor {
        label: Some("Sprite Vertex Buffer"),
        contents: &vec![0u8; std::mem::size_of::<Vertex>() * (sprite_count * 4) as usize],
        usage: wgpu::BufferUsages::VERTEX | BufferUsages::COPY_DST,
    })
}

fn create_index_buffer(sprite_count: u16, device: &Device) -> wgpu::Buffer {
    let index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
        label: Some("Sprite Index Buffer"),
        contents: bytemuck::cast_slice(&indices(sprite_count)),
        usage: wgpu::BufferUsages::INDEX | BufferUsages::COPY_DST,
    });

    index_buffer
}

impl SpriteRenderer {
    pub fn new(
        config: &SurfaceConfiguration,
        device: &Device,
        window_width: f32,
        window_height: f32,
    ) -> Self {
        let shader = device.create_shader_module(include_wgsl!("sprite.wgsl"));

        let vertex_buffer = create_vertex_buffer(STARTING_LENGTH, device);
        let index_buffer = create_index_buffer(STARTING_LENGTH, device);

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let window_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("window_bind_group_layout"),
            });

        // window size
        let window_uniform = WindowUnifrom {
            size: [window_width, window_height],
        };

        let window_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Window Buffer"),
            contents: bytemuck::cast_slice(&[window_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let window_bind_group = device.create_bind_group(&BindGroupDescriptor {
            layout: &window_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: window_buffer.as_entire_binding(),
            }],
            label: Some("window_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &window_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            texture_bind_group_layout,
            length: STARTING_LENGTH,
            vertex_buffer,
            index_buffer,
            pipeline,
            window_bind_group,
            window_buffer,
        }
    }

    pub fn create_texture_bind_group(
        &self,
        device: &Device,
        sampler: &Sampler,
        view: &TextureView,
    ) -> BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
            label: Some("world_bind_group"),
        })
    }

    pub fn draw(
        &mut self,
        sprite_batches: &[SpriteBatch],
        device: &Device,
        queue: &Queue,
        view: &TextureView,
        window_size: [f32; 2],
    ) -> CommandBuffer {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sprite Command Encoder"),
        });

        // this doesn't need to write every frame, but I don't want to overcomplicate things
        queue.write_buffer(
            &self.window_buffer,
            0,
            bytemuck::cast_slice(&[WindowUnifrom { size: window_size }]),
        );

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sprite Render Pass"),
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

        let mut vertices = Vec::<Vertex>::new();

        for batch in sprite_batches.iter() {
            for sprite in batch.sprites.iter() {
                let sprite_vertices = sprite.vertices();
                vertices.push(sprite_vertices[0]);
                vertices.push(sprite_vertices[1]);
                vertices.push(sprite_vertices[2]);
                vertices.push(sprite_vertices[3]);
            }
        }
        let sprite_count = vertices.len() as u16 / 4;

        if self.length < sprite_count {
            self.resize(sprite_count, device);
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(1, &self.window_bind_group, &[]);

        // can only write to buffer once a frame
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        let mut indices_offset = 0;
        for batch in sprite_batches.iter() {
            let sprite_indices = batch.sprites.len() as u32 * 6;

            render_pass.set_bind_group(0, batch.texture_bind_group, &[]);
            render_pass.draw_indexed(indices_offset..indices_offset + sprite_indices, 0, 0..1);

            indices_offset += sprite_indices;
        }

        drop(render_pass);
        encoder.finish()
    }

    pub fn resize(&mut self, sprite_count: u16, device: &Device) {
        if sprite_count == 0 {
            return;
        }
        self.vertex_buffer = create_vertex_buffer(sprite_count, device);
        self.index_buffer = create_index_buffer(sprite_count, device);
        self.length = sprite_count;
    }
}
