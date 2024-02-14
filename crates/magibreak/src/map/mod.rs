use crate::input::Actions;

pub use self::grid::SerializableGrid;
use self::grid::*;
use self::model::*;
use excali_3d::*;
use excali_input::{InputState, MousePosition};
use excali_io::load_from_toml;
use excali_io::tokio;
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
    solved_pin_model: Model,
    locked_pin_model: Model,
    renderer: Renderer3D,
    mouse_coordinate: Option<MapCoordinate>,
    selected_zone_coordinate: Option<MapCoordinate>,
    saving_rx: Option<oneshot::Receiver<Result<(), String>>>,
    loading_rx: Option<oneshot::Receiver<Grid>>,
    brush_mode: BrushMode,
}

enum BrushMode {
    Grow,
    Remove,
    AddZone,
    EditZone,
}

impl ToString for BrushMode {
    fn to_string(&self) -> String {
        match *self {
            Self::Grow => "Grow",
            Self::Remove => "Remove",
            Self::AddZone => "Add Zone",
            Self::EditZone => "Edit Zone",
        }
        .to_string()
    }
}

impl Mode for BrushMode {
    fn change(&self) -> Self {
        match *self {
            Self::Grow => Self::Remove,
            Self::Remove => Self::AddZone,
            Self::AddZone => Self::EditZone,
            Self::EditZone => Self::Grow,
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
            solved_pin_model: Model::cube(
                device,
                "Solved Pin Model".to_string(),
                ZONE_SIZE,
                [1.0, 1.0, 1.0],
            ),
            locked_pin_model: Model::cube(
                device,
                "Locked Pin Model".to_string(),
                ZONE_SIZE,
                [0.0, 0.0, 0.0],
            ),
            selected_zone_coordinate: None,
            saving_rx: None,
            loading_rx: None,
            mouse_coordinate: None,
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
        edit: bool,
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
            if edit {
                let aabb = Aabb::new(
                    Point3::new(0.0, 0.0, 0.0),
                    Point3::new(CHUNK_SIZE as f32, 1.9, CHUNK_SIZE as f32),
                );
                if let Some(result) = aabb.clip_ray(&ray) {
                    let x = result.a.x.floor() as u16;
                    let y = result.a.z.floor() as u16;
                    let coordinate = Vector2::new(x, y);
                    self.mouse_coordinate = Some(coordinate);
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
                    let input_state = &input.left_mouse_click;
                    if !input_state.consumed && input_state.state == InputState::JustPressed {
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
                            BrushMode::AddZone => {
                                self.grid.add_zone(coordinate);
                            }
                            BrushMode::EditZone => {
                                self.selected_zone_coordinate = Some(coordinate);
                            }
                        }
                        self.model = from_marching_squares(&renderer.device, &self.grid);
                    }
                    return None;
                }
            } else if input.left_mouse_click.state == InputState::JustPressed {
                let mut clicked_coordinate: Option<Vector2<u16>> = None;
                let mut distance = f32::INFINITY;

                for (coordinate, active_zone) in self.grid.zones().iter() {
                    if !active_zone.state.selectable() {
                        continue;
                    }
                    if let Some(result) = self.grid.zone_aabb(coordinate).clip_ray(&ray) {
                        let length = result.length();
                        if clicked_coordinate.is_none() || length < distance {
                            distance = result.length();
                            clicked_coordinate = Some(*coordinate);
                        }
                    }
                }

                if let Some(coordinate) = clicked_coordinate {
                    self.mouse_coordinate = None;
                    self.cursor_model = None;
                    return Some(coordinate);
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

        let mut line_matrices = Vec::<Matrix4<f32>>::new();
        for (coordinate, zone) in self.grid.zones() {
            let a = self.grid.zone_world_position(coordinate);
            for next_coordinate in zone.zone.next_zones.iter() {
                line_matrices.push(
                    Transform::line(&a, &self.grid.zone_world_position(next_coordinate), 0.2)
                        .matrix(),
                );
            }
        }

        let mut buffers = vec![self.renderer.draw(
            renderer,
            view,
            &[
                ModelBatch {
                    model: &self.model,
                    matrices: vec![Transform::default().matrix()],
                },
                // TODO clean this shit
                ModelBatch {
                    model: &self.pin_model,
                    matrices: self
                        .grid
                        .zones()
                        .iter()
                        .filter_map(|(coordinate, active_zone)| {
                            if active_zone.state == ZoneState::Unlocked {
                                return Some(Matrix4::new_translation(
                                    &self.grid.zone_world_position(coordinate),
                                ));
                            }
                            None
                        })
                        .collect(),
                },
                ModelBatch {
                    model: &self.solved_pin_model,
                    matrices: self
                        .grid
                        .zones()
                        .iter()
                        .filter_map(|(coordinate, active_zone)| {
                            if active_zone.state == ZoneState::Solved {
                                return Some(Matrix4::new_translation(
                                    &self.grid.zone_world_position(coordinate),
                                ));
                            }
                            None
                        })
                        .collect(),
                },
                ModelBatch {
                    model: &self.locked_pin_model,
                    matrices: self
                        .grid
                        .zones()
                        .iter()
                        .filter_map(|(coordinate, active_zone)| {
                            if active_zone.state == ZoneState::Locked {
                                return Some(Matrix4::new_translation(
                                    &self.grid.zone_world_position(coordinate),
                                ));
                            }
                            None
                        })
                        .collect(),
                },
                // lines
                ModelBatch {
                    model: &self.locked_pin_model,
                    matrices: line_matrices,
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

    pub fn edit_ui(&mut self, ctx: &Context) {
        if let Some(coordinate) = self.selected_zone_coordinate {
            egui::Window::new("Zone Editor").show(ctx, |ui| {
                if let Some(zone) = self.grid.zone_mut(&coordinate) {
                    ui.label(format!("Zone: {} {}", coordinate.x, coordinate.y));
                    ui.horizontal(|ui| {
                        ui.label("Level");
                        ui.text_edit_singleline(&mut zone.zone.level_name);
                    });
                    for next_zone in zone.zone.next_zones.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} {}", next_zone.x, next_zone.y));
                        });
                    }
                    if ui.button("DELETE").clicked() {
                        self.grid.delete_zone(&coordinate);
                    }
                }
            });
        }

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
            self.brush_mode.ui(ui, "Brush");

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

            match receive_oneshot_rx(&mut self.loading_rx) {
                OneShotStatus::None => {
                    if ui.button("Load").clicked() {
                        let (tx, rx) = oneshot::channel();
                        tokio::spawn(async move {
                            let grid: Grid =
                                load_from_toml::<SerializableGrid>(grid::MAP_FILE_PATH.to_string())
                                    .await
                                    .unwrap()
                                    .try_into()
                                    .unwrap();
                            tx.send(grid).unwrap();
                        });
                        self.loading_rx = Some(rx);
                    }
                }
                OneShotStatus::Closed => error!("Loading map.toml channel closed"),
                OneShotStatus::Value(result) => {
                    self.grid = result;
                }
                OneShotStatus::Empty => {
                    ui.label("Loading map");
                }
            }
        });
    }
}
