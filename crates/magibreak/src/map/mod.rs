use crate::input::Actions;

pub use self::grid::SerializableGrid;
use self::grid::*;
use self::model::*;
use excali_3d::*;
use excali_input::{InputState, MousePosition};
use excali_io::tokio::sync::oneshot;
use excali_io::OneShotStatus;
use excali_io::{receive_oneshot_rx, save_to_toml};
use excali_render::Renderer;
use excali_ui::egui_winit::egui;
use excali_ui::egui_winit::egui::Context;
use excali_ui::Mode;
use log::error;
use nalgebra::Matrix4;
use nalgebra::{Perspective3, Point3, Unit, Vector2, Vector3, Vector4};
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
    /// pins are level locations
    pin_model: Model,
    renderer: Renderer3D,
    mouse_coordinate: Option<Vector2<u16>>,
    saving_rx: Option<oneshot::Receiver<Result<(), String>>>,
    mode: EditorMode,
    brush_mode: BrushMode,
}

enum BrushMode {
    Grow,
    Remove,
}

impl ToString for BrushMode {
    fn to_string(&self) -> String {
        match *self {
            Self::Grow => "Grow",
            Self::Remove => "Remove",
        }
        .to_string()
    }
}

impl Mode for BrushMode {
    fn change(&self) -> Self {
        match *self {
            Self::Grow => Self::Remove,
            Self::Remove => Self::Grow,
        }
    }
}

#[derive(PartialEq)]
enum EditorMode {
    Edit,
    Play,
}

impl ToString for EditorMode {
    fn to_string(&self) -> String {
        match *self {
            Self::Edit => "Edit",
            Self::Play => "Play",
        }
        .to_string()
    }
}

impl Mode for EditorMode {
    fn change(&self) -> Self {
        match *self {
            Self::Edit => Self::Play,
            Self::Play => Self::Edit,
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
    pub fn new(mut grid: Grid, config: &SurfaceConfiguration, device: &Device) -> Self {
        grid.zones.insert(
            Vector2::new(18, 25),
            Zone {
                level_name: "alpha".to_string(),
            },
        );

        let model = from_marching_squares(device, &grid);
        let renderer = Renderer3D::new(config, device, 4);

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
            brush_mode: BrushMode::Grow,
            pin_model: Model::cube(device, "Pin Model".to_string(), ZONE_SIZE, [1.0, 1.0, 0.0]),
            saving_rx: None,
            mouse_coordinate: None,
            mode: EditorMode::Play,
            cursor_model: None,
            grid,
            camera,
            model,
            renderer,
        }
    }

    /// returns a zone coordinate if clicked
    pub fn input(
        &mut self,
        input: &excali_input::Input<Actions>,
        renderer: &Renderer,
        delta: f64,
    ) -> Option<Vector2<u16>> {
        let mut direction = Vector3::<f32>::zeros();

        if input.input_map.camera_forward.button.state.pressed() {
            direction += Vector3::new(0.0, 0.0, 1.0);
        }

        if input.input_map.camera_backward.button.state.pressed() {
            direction -= Vector3::new(0.0, 0.0, 1.0);
        }

        if input.input_map.camera_left.button.state.pressed() {
            direction += Vector3::new(1.0, 0.0, 0.0);
        }

        if input.input_map.camera_right.button.state.pressed() {
            direction -= Vector3::new(1.0, 0.0, 0.0);
        }

        if input.input_map.camera_up.button.state.pressed() {
            direction += Vector3::new(0.0, 1.0, 0.0);
        }

        if input.input_map.camera_down.button.state.pressed() {
            direction -= Vector3::new(0.0, 1.0, 0.0);
        }
        const CAMERA_SPEED: f32 = 5.0;

        if direction.magnitude_squared() >= 1.0 {
            self.camera
                .input_fly(direction, delta as f32 * CAMERA_SPEED);
        }

        if let Some(mouse_position) = input.mouse_position {
            let ray = self.camera.get_ray(&mouse_position, &renderer.size);
            match self.mode {
                EditorMode::Edit => {
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
                            match self.brush_mode {
                                BrushMode::Grow => {
                                    *cell += 1;
                                }
                                BrushMode::Remove => {
                                    // prevent buffer overflow
                                    if *cell > 0 {
                                        *cell -= 1;
                                    }
                                }
                            }
                            self.model = from_marching_squares(&renderer.device, &self.grid);
                        }
                        return None;
                    }
                }
                EditorMode::Play => {
                    if input.left_mouse_click.state == InputState::JustPressed {
                        let mut clicked_coordinate: Option<Vector2<u16>> = None;
                        let mut distance = f32::INFINITY;

                        for coordinate in self.grid.zones.keys() {
                            if let Some(result) = self.grid.zone_aabb(coordinate).clip_ray(&ray) {
                                let length = result.length();
                                if clicked_coordinate.is_none() || length < distance {
                                    distance = result.length();
                                    clicked_coordinate = Some(*coordinate);
                                }
                            }
                        }

                        if let Some(coordinate) = clicked_coordinate {
                            println!("Clicked {coordinate}");
                            self.mouse_coordinate = None;
                            self.cursor_model = None;
                            return Some(coordinate);
                        }
                    }
                }
            }
        }
        self.mouse_coordinate = None;
        self.cursor_model = None;
        None
    }

    pub fn draw(
        &mut self,
        renderer: &Renderer,
        view: &TextureView,
        debug: bool,
    ) -> Vec<CommandBuffer> {
        self.camera.aspect = renderer.config.width as f32 / renderer.config.height as f32;
        let mut buffers = vec![self.renderer.draw(
            renderer,
            view,
            &[
                ModelBatch {
                    model: &self.model,
                    matrices: vec![Transform::default().matrix()],
                },
                ModelBatch {
                    model: &self.pin_model,
                    matrices: self
                        .grid
                        .zones
                        .keys()
                        .map(|coordinate| {
                            Matrix4::new_translation(&self.grid.zone_world_position(coordinate))
                        })
                        .collect(),
                },
            ],
            &self.camera,
            debug,
        )];
        if let Some(model) = &self.cursor_model {
            buffers.push(self.renderer.draw(
                renderer,
                view,
                &[ModelBatch {
                    model,
                    matrices: vec![Transform::default().matrix()],
                }],
                &self.camera,
                true,
            ));
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
                        ui.label(format!("({}, {}, {height})", coordinate.x, coordinate.y));
                    }
                };
            });
            self.mode.ui(ui, "Mode");
            if self.mode == EditorMode::Edit {
                self.brush_mode.ui(ui, "Brush");
            }

            match receive_oneshot_rx(&mut self.saving_rx) {
                OneShotStatus::None => {
                    if ui.button("Save").clicked() {
                        self.saving_rx = Some(save_to_toml::<SerializableGrid>(
                            &(&self.grid).into(),
                            grid::MAP_FILE_PATH.to_string(),
                        ));
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
