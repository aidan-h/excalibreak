use self::grid::*;
use self::model::*;
use excali_3d::*;
use excali_input::InputMap;
use excali_input::InputState;
use excali_input::MousePosition;
use excali_io::receive_oneshot_rx;
use excali_io::save_to_toml;
use excali_io::tokio::sync::oneshot;
use excali_io::OneShotStatus;
use excali_render::Renderer;
use excali_ui::egui_winit::egui;
use excali_ui::egui_winit::egui::Context;
use excali_ui::Mode;
use log::error;
use nalgebra::Perspective3;
use nalgebra::Unit;
use nalgebra::Vector2;
use nalgebra::Vector4;
use nalgebra::{Point3, Vector3};
use parry3d::bounding_volume::Aabb;
use parry3d::query::Ray;
use wgpu::{CommandBuffer, Device, SurfaceConfiguration, TextureView};
use winit::dpi::PhysicalSize;
pub mod grid;
mod model;

pub struct Map {
    pub grid: Grid,
    pub camera: Camera,
    model: Model,
    cursor_model: Option<Model>,
    renderer: Renderer3D,
    mouse_coordinate: Option<Vector2<u16>>,
    saving_rx: Option<oneshot::Receiver<Result<(), String>>>,
    mode: EditorMode,
}

enum EditorMode {
    Grow,
    Remove,
}

impl ToString for EditorMode {
    fn to_string(&self) -> String {
        match *self {
            Self::Grow => "Grow",
            Self::Remove => "Remove",
        }
        .to_string()
    }
}

impl Mode for EditorMode {
    fn change(&self) -> Self {
        match *self {
            Self::Grow => Self::Remove,
            Self::Remove => Self::Grow,
        }
    }
}

trait GetRay {
    fn get_ray(&self, mouse_position: &MousePosition, window_size: &PhysicalSize<u32>) -> Ray;
}

impl GetRay for Camera {
    fn get_ray(&self, mouse_position: &MousePosition, window_size: &PhysicalSize<u32>) -> Ray {
        let point = mouse_position.clip_space(window_size);
        //println!("({}, {})", point[0], point[1]);
        let projection = Perspective3::new(self.aspect, self.fovy, self.znear, self.zfar);

        // Compute two points in clip-space.
        // "ndc" = normalized device coordinates.
        let near_ndc_point = Point3::new(point[0], point[1], -1.0);
        let far_ndc_point = Point3::new(point[0], point[1], 1.0);

        // Unproject them to view-space.
        let near_view_point = projection.unproject_point(&near_ndc_point);
        let far_view_point = projection.unproject_point(&far_ndc_point);

        // Compute the view-space line parameters.
        let inverse = self.view().try_inverse().unwrap();
        let start = (inverse
            * Vector4::new(near_view_point.x, near_view_point.y, near_view_point.z, 1.0))
        .xyz();
        let line_direction = Unit::new_normalize(far_view_point - near_view_point).xyz();
        let direction = (inverse
            * Vector4::new(line_direction.x, line_direction.y, line_direction.z, 1.0))
        .xyz()
            - start;

        Ray::new(start.into(), direction)
    }
}

trait DebugModel {
    fn debug_model(&self, device: &Device, color: &[f32; 3], name: String) -> Model;
}

impl DebugModel for Aabb {
    fn debug_model(&self, device: &Device, color: &[f32; 3], name: String) -> Model {
        let mut vertices = Vec::<Vertex>::new();
        for vertex in self.vertices() {
            vertices.push(Vertex::new([vertex.x, vertex.y, vertex.z], *color));
        }
        let mut indices = Vec::<u16>::new();
        for (a, b, c, d) in Self::FACES_VERTEX_IDS.iter() {
            indices.push(*a as u16);
            indices.push(*b as u16);
            indices.push(*c as u16);
            indices.push(*c as u16);
            indices.push(*d as u16);
            indices.push(*a as u16);
        }

        Model::new(device, vertices, indices, name)
    }
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
            saving_rx: None,
            mouse_coordinate: None,
            mode: EditorMode::Grow,
            cursor_model: None,
            grid,
            camera,
            model,
            renderer,
        }
    }

    pub fn input<T: InputMap>(&mut self, input: &excali_input::Input<T>, renderer: &Renderer) {
        if let Some(mouse_position) = input.mouse_position {
            let ray = self.camera.get_ray(&mouse_position, &renderer.size);
            let aabb = Aabb::new(
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(CHUNK_SIZE as f32, 1.9, CHUNK_SIZE as f32),
            );
            if let Some(result) = aabb.clip_ray(&ray) {
                let x = result.a.x.floor() as u16;
                let y = result.a.z.floor() as u16;
                self.mouse_coordinate = Some(Vector2::new(x, y));
                let mut row = self.grid.height_map.row_mut(y as usize);
                let cell = row.get_mut(x as usize).unwrap();

                self.cursor_model = Some(
                    Aabb::new(
                        Point3::new(x as f32 - 0.5, *cell as f32 - 1.0, y as f32 - 0.5),
                        Point3::new(x as f32 + 0.5, *cell as f32, y as f32 + 0.5),
                    )
                    .debug_model(
                        &renderer.device,
                        &[1.0, 0.0, 0.0],
                        "Cursor Debug".to_string(),
                    ),
                );
                if input.left_mouse_click.state == InputState::JustPressed {
                    match self.mode {
                        EditorMode::Grow => {
                            *cell += 1;
                        }
                        EditorMode::Remove => {
                            // prevent buffer overflow
                            if *cell > 0 {
                                *cell -= 1;
                            }
                        }
                    }
                    self.model = from_marching_squares(&renderer.device, &self.grid);
                }
                return;
            }
        }
        self.mouse_coordinate = None;
        self.cursor_model = None;
    }

    pub fn draw(
        &mut self,
        renderer: &Renderer,
        view: &TextureView,
        debug: bool,
    ) -> Vec<CommandBuffer> {
        self.camera.aspect = renderer.config.width as f32 / renderer.config.height as f32;
        let mut buffers =
            vec![self
                .renderer
                .draw(renderer, view, &[&self.model], &self.camera, debug)];
        if let Some(model) = &self.cursor_model {
            buffers.push(
                self.renderer
                    .draw(renderer, view, &[model], &self.camera, true),
            );
        }
        buffers
    }

    pub fn ui(&mut self, ctx: &Context) {
        egui::Window::new("Map Editor").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Coordinate");
                if let Some(coordinate) = self.mouse_coordinate {
                    if let Some(height) = self
                        .grid
                        .height_map
                        .row(coordinate.y as usize)
                        .get(coordinate.x as usize)
                    {
                        ui.label(format!("({}, {}, {})", coordinate.x, coordinate.y, height));
                    }
                };
            });
            self.mode.ui(ui, "Mode");
            match receive_oneshot_rx(&mut self.saving_rx) {
                OneShotStatus::None => {
                    if ui.button("Save").clicked() {
                        self.saving_rx =
                            Some(save_to_toml(&self.grid, grid::MAP_FILE_PATH.to_string()));
                    }
                }
                OneShotStatus::Closed => error!("Saving map.toml channel closed"),
                OneShotStatus::Value(result) => {
                    if let Err(err) = result {
                        error!("{err}");
                    }
                }
                OneShotStatus::Empty => {
                    ui.label("Saving map");
                }
            }
        });
    }
}
