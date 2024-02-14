use self::grid::*;
use self::model::*;
use excali_3d::*;
use excali_render::Renderer;
use nalgebra::{Point3, Vector3};
use wgpu::{CommandBuffer, Device, Queue, SurfaceConfiguration, TextureView};
pub mod grid;
mod model;

pub struct Map {
    pub grid: Grid,
    pub camera: Camera,
    model: Model,
    renderer: Renderer3D,
}

impl Map {
    pub fn new(grid: Grid, config: &SurfaceConfiguration, device: &Device) -> Self {
        let model = from_marching_squares(device, &grid);
        let renderer = Renderer3D::new(config, device);

        let camera = Camera {
            eye: Point3::new(2.0, 3.0, -1.0),
            target: Point3::new(2.0, 1.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            aspect: 1.0,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };

        Self {
            grid,
            camera,
            model,
            renderer,
        }
    }

    pub fn draw(&mut self, renderer: &Renderer, view: &TextureView, debug: bool) -> CommandBuffer {
        self.camera.aspect = renderer.config.width as f32 / renderer.config.height as f32;
        self.renderer
            .draw(renderer, view, &[&self.model], &self.camera, debug)
    }
}
