use crate::puzzle::*;
use excali_sprite::{Color, Sprite, SpriteBatch, Transform};
use excali_ui::egui_winit::egui::{self, Context};
use nalgebra::Vector2;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

#[derive(Eq, PartialEq)]
enum LevelEditorMode {
    Clear,
    Cursor,
    Place,
}

impl ToString for LevelEditorMode {
    fn to_string(&self) -> String {
        match self {
            Self::Clear => "Clear".to_string(),
            Self::Cursor => "Cursor".to_string(),
            Self::Place => "Place".to_string(),
        }
    }
}

//TODO remove unwraps
async fn load_puzzle(name: String) -> Puzzle {
    let mut file = File::open(format!("{}{}.toml", LEVELS_PATH, name))
        .await
        .unwrap();

    let mut contents = String::new();
    file.read_to_string(&mut contents).await.unwrap();
    let serialable_puzzle: SerialablePuzzle = toml::from_str(contents.as_str()).unwrap();
    Puzzle::try_from(serialable_puzzle).unwrap()
}

pub struct LevelEditor {
    pub enabled: bool,
    // puzzle's original state
    pub loaded_puzzle: Puzzle,
    file_name: String,
    mode: LevelEditorMode,
    save_future: Option<JoinHandle<()>>,
    rune: Rune,
}

impl LevelEditor {
    pub async fn new(file_name: String) -> Self {
        Self {
            enabled: false,
            loaded_puzzle: load_puzzle(file_name.clone()).await,
            file_name,
            mode: LevelEditorMode::Place,
            save_future: None,
            rune: Rune {
                sigil: Sigil::Alpha,
                orb: Orb::Circle,
            },
        }
    }
}

const LEVELS_PATH: &str = "./assets/levels/";
impl LevelEditor {
    pub fn ui(&mut self, ctx: &Context, puzzle: &mut Puzzle) {
        egui::Window::new(format!("level editor - {}.toml", self.file_name)).show(ctx, |ui| {
            if ui
                .button(if self.enabled { "Play" } else { "Edit" })
                .clicked()
            {
                self.toggle(puzzle);
            }
            if !self.enabled {
                return;
            }

            ui.label("Mode");
            if ui.button(self.mode.to_string()).clicked() {
                self.change_mode();
            }

            if self.mode == LevelEditorMode::Place {
                ui.label("Sigil");
                if ui.button(self.rune.sigil.to_string()).clicked() {
                    self.change_sigil();
                }

                ui.label("Orb");
                if ui.button(self.rune.orb.to_string()).clicked() {
                    self.change_orb();
                }
            }

            if let Some(save_future) = &self.save_future {
                if save_future.is_finished() {
                    self.save_future = None;
                } else {
                    ui.label("Saving");
                }
            } else if ui.button("Save").clicked() {
                let string: String =
                    toml::to_string(&SerialablePuzzle::from(self.loaded_puzzle.clone())).unwrap();
                let file_name = self.file_name.clone();

                self.save_future = Some(tokio::spawn(async move {
                    let mut file = File::create(format!("{}{}.toml", LEVELS_PATH, file_name))
                        .await
                        .unwrap();
                    file.write_all(string.as_bytes()).await.unwrap();
                }));
            }
        });
    }

    pub fn input(&mut self, coordinate: SigilCoordinate, puzzle: &mut Puzzle) {
        if !self.enabled {
            return;
        }
        match self.mode {
            LevelEditorMode::Cursor => {
                self.loaded_puzzle.cursor = coordinate;
            }
            LevelEditorMode::Clear => {
                self.loaded_puzzle.runes.remove(&coordinate);
            }
            LevelEditorMode::Place => {
                self.loaded_puzzle.runes.insert(coordinate, self.rune);
            }
        };
        *puzzle = self.loaded_puzzle.clone();
    }

    fn toggle(&mut self, puzzle: &mut Puzzle) {
        self.enabled = !self.enabled;
        *puzzle = self.loaded_puzzle.clone();
    }

    fn change_mode(&mut self) {
        self.mode = match self.mode {
            LevelEditorMode::Clear => LevelEditorMode::Cursor,
            LevelEditorMode::Cursor => LevelEditorMode::Place,
            LevelEditorMode::Place => LevelEditorMode::Clear,
        };
    }

    fn change_orb(&mut self) {
        self.rune.orb = match self.rune.orb {
            Orb::Circle => Orb::Diamond,
            Orb::Diamond => Orb::Octogon,
            Orb::Octogon => Orb::Circle,
        };
    }

    fn change_sigil(&mut self) {
        self.rune.sigil = match self.rune.sigil {
            Sigil::Alpha => Sigil::Sigma,
            Sigil::Sigma => Sigil::Alpha,
        };
    }

    pub fn sprite_batches<'a>(
        &'a self,
        mouse_coordinate: SigilCoordinate,
        cursor_texture: &'a wgpu::BindGroup,
        sigils_texture: &'a wgpu::BindGroup,
        orbs_texture: &'a wgpu::BindGroup,
        border_texture: &'a wgpu::BindGroup,
    ) -> Option<Vec<SpriteBatch>> {
        if !self.enabled {
            return None;
        }

        match self.mode {
            LevelEditorMode::Place => {
                let transform = Transform::from_sigil_coordinate(mouse_coordinate);
                Some(vec![
                    SpriteBatch {
                        sprites: vec![Sprite {
                            transform,
                            texture_coordinate: self.rune.orb.texture_coordinate(false),
                            color: Color::new(1.0, 1.0, 1.0, 0.8),
                        }],
                        texture_bind_group: orbs_texture,
                    },
                    SpriteBatch {
                        sprites: vec![Sprite {
                            transform,
                            texture_coordinate: self.rune.sigil.texture_coordinate(),
                            color: Color::new(1.0, 1.0, 1.0, 0.8),
                        }],
                        texture_bind_group: sigils_texture,
                    },
                ])
            }
            LevelEditorMode::Clear => {
                let transform = Transform::from_sigil_coordinate(mouse_coordinate);
                Some(vec![SpriteBatch {
                    sprites: vec![Sprite {
                        transform,
                        color: Color::new(1.0, 0.0, 0.0, 1.0),
                        ..Default::default()
                    }],
                    texture_bind_group: border_texture,
                }])
            }
            LevelEditorMode::Cursor => {
                let transform = Transform {
                    position: mouse_coordinate.position(),
                    rotation: 0.0,
                    scale: Vector2::new(CURSOR_SIZE, CURSOR_SIZE),
                };
                Some(vec![SpriteBatch {
                    sprites: vec![Sprite {
                        transform,
                        color: Color::new(1.0, 1.0, 1.0, 0.6),
                        ..Default::default()
                    }],
                    texture_bind_group: cursor_texture,
                }])
            }
        }
    }
}
