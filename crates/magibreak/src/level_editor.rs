use crate::puzzle::*;
use excali_sprite::{Color, Sprite, SpriteBatch, Transform};
use excali_ui::egui_winit::egui::{self, Context};
use log::error;
use nalgebra::Vector2;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

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
async fn load_puzzle(name: String) -> Result<Puzzle, String> {
    match File::open(format!("{}{}", LEVELS_PATH, name)).await {
        Ok(mut file) => {
            let mut contents = String::new();
            file.read_to_string(&mut contents).await.unwrap();
            let result: Result<SerialablePuzzle, toml::de::Error> =
                toml::from_str(contents.as_str());
            match result {
                Ok(serialable_puzzle) => match Puzzle::try_from(serialable_puzzle) {
                    Ok(puzzle) => Ok(puzzle),
                    Err(err) => Err(format!("{err:?}")),
                },
                Err(err) => Err(err.message().to_string()),
            }
        }
        Err(err) => Err(format!("{err}")),
    }
}

pub struct LevelEditor {
    pub enabled: bool,
    // puzzle's original state
    pub loaded_puzzle: Puzzle,
    file_name: String,
    mode: LevelEditorMode,
    levels: Vec<String>,
    levels_rx: Option<oneshot::Receiver<Result<Vec<String>, String>>>,
    save_rx: Option<oneshot::Receiver<Result<(), String>>>,
    load_rx: Option<oneshot::Receiver<Result<Puzzle, String>>>,
    rune: Rune,
}

const LEVELS_PATH: &str = "./assets/levels/";
impl LevelEditor {
    pub async fn new(file_name: String) -> Self {
        let mut editor = Self {
            levels: Vec::new(),
            enabled: false,
            loaded_puzzle: load_puzzle(file_name.clone()).await.unwrap(),
            file_name,
            mode: LevelEditorMode::Place,
            save_rx: None,
            load_rx: None,
            levels_rx: None,
            rune: Rune {
                sigil: Sigil::Alpha,
                orb: Orb::Circle,
            },
        };
        editor.load_levels();
        editor
    }

    fn load_levels(&mut self) {
        if self.levels_rx.is_some() {
            return;
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.levels_rx = Some(rx);

        tokio::spawn(async move {
            tx.send(match tokio::fs::read_dir(LEVELS_PATH).await {
                Err(err) => Err(err.to_string()),
                Ok(mut dir) => {
                    let mut entries = Vec::<String>::new();
                    loop {
                        match dir.next_entry().await {
                            Err(err) => {
                                return Err(err.to_string());
                            }
                            Ok(entry) => match entry {
                                Some(entry) => {
                                    entries.push(entry.file_name().to_str().unwrap().to_string());
                                }
                                None => {
                                    break;
                                }
                            },
                        }
                    }
                    Ok(entries)
                }
            })
            .unwrap();
            // no idea what I'm doing here
            Ok(())
        });
    }

    pub fn ui(&mut self, ctx: &Context, puzzle: &mut Puzzle) {
        egui::Window::new("level editor").show(ctx, |ui| {
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

            ui.add(egui::TextEdit::singleline(&mut self.file_name));

            // saving
            if let Some(save_rx) = self.save_rx.as_mut() {
                match save_rx.try_recv() {
                    Ok(..) => {
                        self.save_rx = None;
                        // refresh levels list
                        self.load_levels();
                    }
                    Err(err) => match err {
                        oneshot::error::TryRecvError::Empty => {
                            ui.label("Saving");
                        }
                        oneshot::error::TryRecvError::Closed => {
                            self.save_rx = None;
                            error!("Save channel unexpectantly closed");
                        }
                    },
                };
            } else if ui.button("Save").clicked() {
                self.save_level();
            }

            // loading
            if let Some(load_rx) = self.load_rx.as_mut() {
                match load_rx.try_recv() {
                    Ok(new_puzzle) => {
                        match new_puzzle {
                            Ok(new_puzzle) => {
                                self.loaded_puzzle = new_puzzle.clone();
                                *puzzle = new_puzzle;
                            }
                            Err(err) => error!("{err}"),
                        };
                        self.load_rx = None;
                    }
                    Err(err) => match err {
                        oneshot::error::TryRecvError::Closed => {
                            self.load_rx = None;
                            error!("Puzzle load channel closed unexpectantly")
                        }
                        oneshot::error::TryRecvError::Empty => {}
                    },
                }
            } else if ui.button("Load").clicked() {
                self.load_level();
            }

            // levels
            ui.label("Levels");
            match self.levels_rx.as_mut() {
                Some(levels_rx) => match levels_rx.try_recv() {
                    Ok(levels) => {
                        match levels {
                            Ok(levels) => {
                                self.levels = levels;
                            }
                            Err(err) => error!("{err}"),
                        };
                        self.levels_rx = None;
                    }
                    Err(err) => match err {
                        oneshot::error::TryRecvError::Closed => {
                            self.levels_rx = None;
                            error!("Levels channel closed unexpectantly")
                        }
                        oneshot::error::TryRecvError::Empty => {}
                    },
                },
                None => {
                    for level in self.levels.iter() {
                        if ui.button(level).clicked() {
                            self.file_name = level.to_string();
                            self.load_level();
                            break;
                        }
                    }
                }
            }
        });
    }

    fn save_level(&mut self) {
        let string: String =
            toml::to_string(&SerialablePuzzle::from(self.loaded_puzzle.clone())).unwrap();
        let file_name = self.file_name.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.save_rx = Some(rx);

        tokio::spawn(async move {
            match File::create(format!("{}{}", LEVELS_PATH, file_name)).await {
                Ok(mut file) => {
                    match file.write_all(string.as_bytes()).await {
                        Ok(()) => {
                            tx.send(Ok(())).unwrap();
                        }
                        Err(err) => {
                            tx.send(Err(format!("{err}"))).unwrap();
                        }
                    };
                }
                Err(err) => {
                    tx.send(Err(format!("{err}"))).unwrap();
                }
            }
        });
    }

    fn load_level(&mut self) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.load_rx = Some(rx);

        let file_name = self.file_name.clone();
        tokio::spawn(async move {
            tx.send(load_puzzle(file_name).await).unwrap();
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
