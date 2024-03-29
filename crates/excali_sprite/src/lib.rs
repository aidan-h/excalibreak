use excali_render::wgpu::util::DeviceExt;
use excali_render::wgpu::*;
use nalgebra::{Vector2, Vector4};

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
    color: [f32; 4],
}

impl Vertex {
    fn descriptor<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x4,
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

#[derive(Debug, Clone, Copy)]
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

impl std::ops::Mul for &Transform {
    type Output = Transform;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn mul(self, rhs: Self) -> Self::Output {
        let position = rhs.position.component_mul(&self.scale);
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        Self::Output {
            scale: self.scale.component_mul(&rhs.scale),
            rotation: self.rotation + rhs.rotation,
            position: self.position
                + Vector2::new(
                    position.x * cos + position.y * -sin,
                    position.x * sin + position.y * cos,
                ),
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vector2::zeros(),
            rotation: 0.0,
            scale: Vector2::new(1.0, 1.0),
        }
    }
}

impl Transform {
    pub fn to_object_space(&self, rhs: &Self) -> Self {
        let position = (rhs.position - self.position).component_div(&self.scale);
        let rotation = -self.rotation;
        let sin = rotation.sin();
        let cos = rotation.cos();

        Self {
            rotation: rhs.rotation + rotation,
            scale: rhs.scale.component_div(&self.scale),
            position: Vector2::new(
                position.x * cos + position.y * -sin,
                position.x * sin + position.y * cos,
            ),
        }
    }

    pub fn from_scale(scale: Vector2<f32>) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }

    pub fn from_position(position: Vector2<f32>) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    pub fn from_rotation(rotation: f32) -> Self {
        Self {
            rotation,
            ..Default::default()
        }
    }
}

pub type Color = Vector4<f32>;

#[derive(Clone, Copy)]
pub struct Sprite {
    pub transform: Transform,
    pub color: Color,
    pub texture_coordinate: TextureCoordinate,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            texture_coordinate: TextureCoordinate::default(),
            color: Color::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

impl Sprite {
    fn vertices(&self, width: u32, height: u32) -> [Vertex; 4] {
        let position = self.transform.position;
        let rotation = self.transform.rotation;
        let scale = self.transform.scale.component_mul(&Vector2::new(
            width as f32 * self.texture_coordinate.width.abs(),
            height as f32 * self.texture_coordinate.height.abs(),
        )) / 2.0;
        let sin = rotation.sin();
        let cos = rotation.cos();

        let mut top_right =
            Vector2::new(scale.x * cos + scale.y * sin, scale.y * cos - scale.x * sin);
        let bottom_left = position - top_right;
        top_right += position;

        let mut top_left = Vector2::new(
            -scale.x * cos + scale.y * sin,
            scale.y * cos + scale.x * sin,
        );
        let bottom_right = position - top_left;
        top_left += position;

        [
            Vertex {
                position: [bottom_left.x, bottom_left.y],
                color: [self.color.x, self.color.y, self.color.z, self.color.w],
                tex_coords: self.texture_coordinate.bottom_left(),
            },
            Vertex {
                position: [bottom_right.x, bottom_right.y],
                color: [self.color.x, self.color.y, self.color.z, self.color.w],
                tex_coords: self.texture_coordinate.bottom_right(),
            },
            Vertex {
                position: [top_right.x, top_right.y],
                color: [self.color.x, self.color.y, self.color.z, self.color.w],
                tex_coords: self.texture_coordinate.top_right(),
            },
            Vertex {
                position: [top_left.x, top_left.y],
                color: [self.color.x, self.color.y, self.color.z, self.color.w],
                tex_coords: self.texture_coordinate.top_left(),
            },
        ]
    }
}

pub struct SpriteTexture {
    pub bind_group: BindGroup,
    pub data: excali_render::Texture,
}

#[derive(Clone)]
pub struct SpriteBatch<'a> {
    pub sprites: Vec<Sprite>,
    pub texture: &'a SpriteTexture,
}

fn create_vertex_buffer(sprite_count: u16, device: &Device) -> Buffer {
    device.create_buffer_init(&util::BufferInitDescriptor {
        label: Some("Sprite Vertex Buffer"),
        contents: &vec![0u8; std::mem::size_of::<Vertex>() * (sprite_count * 4) as usize],
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
    })
}

fn create_index_buffer(sprite_count: u16, device: &Device) -> Buffer {
    let index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
        label: Some("Sprite Index Buffer"),
        contents: bytemuck::cast_slice(&indices(sprite_count)),
        usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
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
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            multisampled: false,
                            view_dimension: TextureViewDimension::D2,
                            sample_type: TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("sprite_texture_bind_group_layout"),
            });

        let window_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
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

        let window_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Window Buffer"),
            contents: bytemuck::cast_slice(&[window_uniform]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let window_bind_group = device.create_bind_group(&BindGroupDescriptor {
            layout: &window_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: window_buffer.as_entire_binding(),
            }],
            label: Some("window_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &window_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::descriptor()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
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

    pub fn create_bind_group(
        &self,
        device: &Device,
        sampler: &Sampler,
        texture: &excali_render::Texture,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture.view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
            label: Some(&format!("{}_bind_group", texture.name)),
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
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Sprite Command Encoder"),
        });

        // this doesn't need to write every frame, but I don't want to overcomplicate things
        queue.write_buffer(
            &self.window_buffer,
            0,
            bytemuck::cast_slice(&[WindowUnifrom { size: window_size }]),
        );

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Sprite Render Pass"),
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

        let mut vertices = Vec::<Vertex>::new();

        for batch in sprite_batches.iter() {
            for sprite in batch.sprites.iter() {
                let sprite_vertices =
                    sprite.vertices(batch.texture.data.width, batch.texture.data.height);
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
        render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);

        let mut indices_offset = 0;
        for batch in sprite_batches.iter() {
            let sprite_indices = batch.sprites.len() as u32 * 6;

            render_pass.set_bind_group(0, &batch.texture.bind_group, &[]);
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
