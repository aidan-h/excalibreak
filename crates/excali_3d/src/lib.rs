use excali_render::wgpu::util::DeviceExt;
use excali_render::wgpu::*;
use excali_render::{wgpu, Renderer};
use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};

pub struct Model {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub indices: u32,
}

impl Model {
    pub fn new(device: &Device, vertices: Vec<Vertex>, indices: Vec<u16>, name: String) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some((name.clone() + " Vertex Buffer").as_str()),
            contents: bytemuck::cast_slice(vertices.as_slice()),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some((name + " Index Buffer").as_str()),
            contents: bytemuck::cast_slice(indices.as_slice()),
            usage: wgpu::BufferUsages::INDEX,
        });
        let indices = indices.len() as u32;

        Self {
            vertex_buffer,
            index_buffer,
            indices,
        }
    }

    /// creates a 3D cube model with a center origin
    pub fn cube(device: &Device, name: String, mut size: Vector3<f32>, color: [f32; 3]) -> Self {
        size /= 2.0;

        let vertices = vec![
            Vertex::new([-size.x, -size.y, -size.z], color),
            Vertex::new([size.x, -size.y, -size.z], color),
            Vertex::new([-size.x, size.y, -size.z], color),
            Vertex::new([-size.x, -size.y, size.z], color),
            Vertex::new([size.x, size.y, -size.z], color),
            Vertex::new([-size.x, size.y, size.z], color),
            Vertex::new([size.x, -size.y, size.z], color),
            Vertex::new([size.x, size.y, size.z], color),
        ];

        let indices = vec![
            0, 2, 1, 1, 2, 4, // Back
            3, 6, 5, 6, 7, 5, // Front
            0, 1, 3, 1, 6, 3, // Bottom
            0, 3, 5, 0, 5, 2, // Left
            1, 4, 7, 1, 7, 6, // Right
            5, 7, 4, 5, 4, 2, // Top
        ];
        Self::new(device, vertices, indices, name)
    }
}

struct DepthTexture {
    view: wgpu::TextureView,
    size: [u32; 2],
}

impl DepthTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, label: &str) -> Self {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            view_formats: &[Self::DEPTH_FORMAT],
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            view,
            size: [config.width, config.height],
        }
    }
}

pub struct Renderer3D {
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instances: usize,
    debug_render_pipeline: wgpu::RenderPipeline,
    depth_texture: DepthTexture,
}

pub struct ModelBatch<'a> {
    pub model: &'a Model,
    pub matrices: Vec<Matrix4<f32>>,
}

pub struct Transform {
    pub position: Vector3<f32>,
    pub scale: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
}

impl Transform {
    pub fn matrix(&self) -> Matrix4<f32> {
        Matrix4::new_translation(&self.position)
            * (self.rotation.to_homogeneous() * Matrix4::new_nonuniform_scaling(&self.scale))
    }

    /// Creates a transfrom between two points, forming a line across the z-axis
    pub fn line(a: &Vector3<f32>, b: &Vector3<f32>, thickness: f32) -> Self {
        let direction = b - a;
        let middle = (a + b) / 2.0;
        let length = direction.magnitude();
        Self {
            position: middle,
            scale: Vector3::new(thickness, thickness, length),
            rotation: UnitQuaternion::face_towards(&direction, &Vector3::y()),
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Renderer3D {
    pub fn draw(
        &mut self,
        renderer: &Renderer,
        view: &TextureView,
        batches: &[ModelBatch],
        camera: &Camera,
        debug: bool,
    ) -> CommandBuffer {
        let mut encoder = renderer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("3D Command Encoder"),
            });

        renderer.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(camera.projection_matrix().as_slice()),
        );
        if [renderer.config.width, renderer.config.height] != self.depth_texture.size {
            self.depth_texture =
                DepthTexture::new(&renderer.device, &renderer.config, "3D Depth Texture");
        }

        let mut instances = Vec::<InstanceRaw>::new();
        for batch in batches {
            for matrix in batch.matrices.iter() {
                instances.push((*matrix).into());
            }
        }

        if instances.len() > self.instances {
            self.instances = instances.len();
            self.instance_buffer = create_instance_buffer(&renderer.device, instances);
        } else {
            renderer
                .queue
                .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Map Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: if debug {
                None
            } else {
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                })
            },
        });

        render_pass.set_pipeline(match debug {
            false => &self.render_pipeline,
            true => &self.debug_render_pipeline,
        });
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

        let mut instance_index = 0u32;
        for batch in batches {
            let end_instances = batch.matrices.len() as u32 + instance_index;
            if end_instances == instance_index {
                continue;
            }

            render_pass.set_vertex_buffer(0, batch.model.vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                batch.model.index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );
            render_pass.draw_indexed(0..batch.model.indices, 0, instance_index..end_instances);
            instance_index = end_instances;
        }

        drop(render_pass);
        encoder.finish()
    }

    /// instances must be greater than 0
    pub fn new(config: &SurfaceConfiguration, device: &Device, instances: usize) -> Self {
        let mut instance_data = Vec::<InstanceRaw>::new();
        for _ in 0..instances {
            instance_data.push(InstanceRaw {
                model: [[0.0; 4]; 4],
            });
        }

        let instance_buffer = create_instance_buffer(device, instance_data);

        let camera = Camera {
            eye: Point3::new(2.0, 3.0, -1.0),
            target: Point3::new(2.0, 1.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            aspect: 1.0,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };

        let camera_uniform = CameraUniform {
            matrix: camera.projection_matrix().into(),
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("3D Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
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
                label: Some("map_camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("web_camera_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("3D Render Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

        let vertex = VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[Vertex::descriptor(), InstanceRaw::desc()],
        };
        let layout = Some(&pipeline_layout);
        let targets = [Some(ColorTargetState {
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
        })];
        let fragment = Some(FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &targets,
        });
        let debug_render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("3D Debug Render Pipeline"),
            layout,
            vertex: vertex.clone(),
            fragment: fragment.clone(),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Line,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("3D Render Pipeline"),
            layout,
            vertex,
            fragment,
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview: None,
        });
        let depth_texture = DepthTexture::new(device, config, "3D Depth Texture");

        Self {
            render_pipeline,
            debug_render_pipeline,
            instances,
            instance_buffer,
            camera_buffer,
            camera_bind_group,
            depth_texture,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    matrix: [[f32; 4]; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            matrix: Matrix4::identity().into(),
        }
    }
}

pub struct Camera {
    pub eye: Point3<f32>,
    pub target: Point3<f32>,
    pub up: Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn projection_matrix(&self) -> Matrix4<f32> {
        Matrix4::<f32>::new_perspective(self.aspect, self.fovy, self.znear, self.zfar) * self.view()
    }

    pub fn view(&self) -> Matrix4<f32> {
        Matrix4::<f32>::look_at_rh(&self.eye, &self.target, &self.up)
    }

    /// process input as a fly camera
    pub fn input_fly(&mut self, mut direction: Vector3<f32>, distance: f32) {
        direction *= distance;
        self.eye += direction;
        self.target += direction;
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    // vec3 has an alignment of 16 bytes
    _padding: u32,
    pub color: [f32; 3],
    // vec3 has an alignment of 16 bytes
    _padding2: u32,
}

impl Vertex {
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self {
            position,
            _padding: 0,
            color,
            _padding2: 0,
        }
    }
    pub fn descriptor<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: (std::mem::size_of::<f32>() * 4) as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
}

impl From<Matrix4<f32>> for InstanceRaw {
    fn from(value: Matrix4<f32>) -> Self {
        Self {
            model: value.data.0,
        }
    }
}

impl InstanceRaw {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We'll have to reassemble the mat4 in
                // the shader.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

fn create_instance_buffer(device: &Device, instances: Vec<InstanceRaw>) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("3D Instance Buffer"),
        contents: bytemuck::cast_slice(&instances),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    })
}
