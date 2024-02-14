use crate::puzzle::*;
use excali_io::tokio::sync::oneshot;
use excali_io::{load_from_toml, receive_oneshot_rx, save_to_toml, tokio, OneShotStatus};
use excali_sprite::{Color, Sprite, SpriteBatch, SpriteTexture, Transform};
use excali_ui::egui_winit::egui::{self, Context};
use excali_ui::Mode;
use log::error;

#[derive(Eq, PartialEq)]
enum LevelEditorMode {
    Clear,
    Cursor,
    Place,
    Lines,
}

impl std::fmt::Display for LevelEditorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Clear => "Clear".to_string(),
                Self::Cursor => "Cursor".to_string(),
                Self::Place => "Place".to_string(),
                Self::Lines => "Lines".to_string(),
            }
        )
    }
}

impl Mode for LevelEditorMode {
    fn change(&self) -> Self {
        match *self {
            Self::Clear => Self::Cursor,
            Self::Cursor => Self::Place,
            Self::Place => Self::Lines,
            Self::Lines => Self::Clear,
        }
    }
}

async fn load_puzzle(name: String) -> Result<Puzzle, String> {
    match load_from_toml::<SerialablePuzzle>(format!("{}{}", LEVELS_PATH, name)).await {
        Ok(serialable_puzzle) => match Puzzle::try_from(serialable_puzzle) {
            Ok(puzzle) => Ok(puzzle),
            Err(err) => Err(format!("{err:?}")),
        },
        Err(err) => Err(err),
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
    delete_rx: Option<oneshot::Receiver<Result<(), String>>>,
    load_rx: Option<oneshot::Receiver<Result<Puzzle, String>>>,
    line_start: Option<SigilCoordinate>,
    rune: Sigil,
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
            delete_rx: None,
            line_start: None,
            rune: Sigil {
                rune: Rune::Alpha,
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

    pub fn ui(&mut self, ctx: &Context, puzzle: &mut ActivePuzzle) {
        if let Some(rx) = self.delete_rx.as_mut() {
            if let Ok(val) = rx.try_recv() {
                match val {
                    Ok(_) => {
                        self.load_levels();
                    }
                    Err(err) => error!("{err}"),
                };
            }
        }

        egui::Window::new("levels").show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.file_name));

            ui.horizontal(|ui| {
                // saving
                match receive_oneshot_rx(&mut self.save_rx) {
                    OneShotStatus::Value(..) => self.load_levels(),
                    OneShotStatus::None => {
                        if ui.button("Save").clicked() {
                            self.save_level();
                        }
                    }
                    OneShotStatus::Empty => {
                        ui.label("Saving");
                    }
                    OneShotStatus::Closed => error!("Save channel unexpectantly closed"),
                }

                match receive_oneshot_rx(&mut self.load_rx) {
                    OneShotStatus::Closed => error!("Puzzle load channel closed unexpectantly"),
                    OneShotStatus::Value(new_puzzle) => {
                        match new_puzzle {
                            Ok(new_puzzle) => {
                                self.loaded_puzzle = new_puzzle.clone();
                                puzzle.load_puzzle(new_puzzle);
                            }
                            Err(err) => error!("{err}"),
                        };
                    }
                    OneShotStatus::None => {
                        if ui.button("Load").clicked() {
                            self.load_level();
                        }
                    }
                    OneShotStatus::Empty => {
                        ui.label("Loading level");
                    }
                }
            });

            // levels
            ui.label("Levels");
            match receive_oneshot_rx(&mut self.levels_rx) {
                OneShotStatus::Empty => {
                    ui.label("Loading levels");
                }
                OneShotStatus::None => {
                    for level in self.levels.clone().drain(..) {
                        ui.horizontal(|ui| {
                            ui.menu_button("edit", |ui| {
                                if ui.button("Delete").clicked() {
                                    self.delete_level(level.clone());
                                }
                            });
                            if ui.button(level.clone()).clicked() {
                                self.file_name = level;
                                self.load_level();
                            }
                        });
                    }
                }
                OneShotStatus::Value(levels) => {
                    match levels {
                        Ok(levels) => {
                            self.levels = levels;
                        }
                        Err(err) => error!("{err}"),
                    };
                }
                OneShotStatus::Closed => error!("Levels channel closed unexpectantly"),
            }
        });

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

            self.mode.ui(ui, "Mode");

            if self.mode == LevelEditorMode::Place {
                self.rune.rune.ui(ui, "Sigil");
                self.rune.orb.ui(ui, "Orb");
            }
        });
    }

    fn delete_level(&mut self, level: String) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.delete_rx = Some(rx);

        tokio::spawn(async move {
            tx.send(
                match tokio::fs::remove_file(format!("{}{}", LEVELS_PATH, level)).await {
                    Ok(_) => Ok(()),
                    Err(err) => Err(err.to_string()),
                },
            )
            .unwrap();
        });
    }

    fn save_level(&mut self) {
        let file_name = self.file_name.clone();
        self.save_rx = Some(save_to_toml(
            &SerialablePuzzle::from(self.loaded_puzzle.clone()),
            format!("{}{}", LEVELS_PATH, file_name),
        ));
    }

    fn load_level(&mut self) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.load_rx = Some(rx);

        let file_name = self.file_name.clone();
        tokio::spawn(async move {
            tx.send(load_puzzle(file_name).await).unwrap();
        });
    }

    pub fn input(&mut self, coordinate: SigilCoordinate, puzzle: &mut ActivePuzzle) {
        if !self.enabled {
            return;
        }
        match self.mode {
            LevelEditorMode::Cursor => {
                self.loaded_puzzle.cursor = coordinate;
            }
            LevelEditorMode::Clear => {
                self.loaded_puzzle.sigils.remove(&coordinate);
                // crappy ui ik but don't care rn
                let mut new_lines = Vec::<Line>::new();
                for line in self.loaded_puzzle.lines.iter() {
                    if line.start != coordinate && line.end != coordinate {
                        new_lines.push(*line);
                    }
                }
                self.loaded_puzzle.lines = new_lines;
            }
            LevelEditorMode::Place => {
                self.loaded_puzzle.sigils.insert(coordinate, self.rune);
            }
            LevelEditorMode::Lines => match self.line_start {
                Some(start) => {
                    if coordinate != start {
                        self.line_start = None;
                        self.loaded_puzzle.lines.push(Line {
                            start,
                            end: coordinate,
                        });
                    }
                }
                None => {
                    self.line_start = Some(coordinate);
                }
            },
        };
        puzzle.load_puzzle(self.loaded_puzzle.clone());
    }

    fn toggle(&mut self, puzzle: &mut ActivePuzzle) {
        self.enabled = !self.enabled;
        puzzle.load_puzzle(self.loaded_puzzle.clone());
    }

    pub fn sprite_batches<'a>(
        &'a self,
        camera: &Transform,
        mouse_coordinate: SigilCoordinate,
        cursor_texture: &'a SpriteTexture,
        sigils_texture: &'a SpriteTexture,
        orbs_texture: &'a SpriteTexture,
        border_texture: &'a SpriteTexture,
    ) -> Option<Vec<SpriteBatch>> {
        if !self.enabled {
            return None;
        }

        match self.mode {
            LevelEditorMode::Place => {
                let transform = Transform::from_sigil_coordinate(mouse_coordinate, camera);
                Some(vec![
                    SpriteBatch {
                        sprites: vec![Sprite {
                            transform,
                            texture_coordinate: self.rune.orb.texture_coordinate(false),
                            color: Color::new(1.0, 1.0, 1.0, 0.8),
                        }],
                        texture: orbs_texture,
                    },
                    SpriteBatch {
                        sprites: vec![Sprite {
                            transform,
                            texture_coordinate: self.rune.rune.texture_coordinate(),
                            color: Color::new(1.0, 1.0, 1.0, 0.8),
                        }],
                        texture: sigils_texture,
                    },
                ])
            }
            LevelEditorMode::Clear => {
                let transform = Transform::from_sigil_coordinate(mouse_coordinate, camera);
                Some(vec![SpriteBatch {
                    sprites: vec![Sprite {
                        transform,
                        color: Color::new(1.0, 0.0, 0.0, 1.0),
                        ..Default::default()
                    }],
                    texture: border_texture,
                }])
            }
            LevelEditorMode::Cursor => {
                let transform = Transform {
                    position: mouse_coordinate.position(),
                    rotation: 0.0,
                    scale: camera.scale,
                };
                Some(vec![SpriteBatch {
                    sprites: vec![Sprite {
                        transform,
                        color: Color::new(1.0, 1.0, 1.0, 0.6),
                        ..Default::default()
                    }],
                    texture: cursor_texture,
                }])
            }
            LevelEditorMode::Lines => {
                let transform = Transform {
                    position: mouse_coordinate.position(),
                    rotation: 0.0,
                    scale: camera.scale,
                };
                let mut sprites = vec![Sprite {
                    transform,
                    color: Color::new(1.0, 1.0, 1.0, 0.6),
                    ..Default::default()
                }];

                if let Some(start) = self.line_start {
                    sprites.push(Sprite {
                        transform: Transform {
                            position: start.position(),
                            rotation: 0.0,
                            scale: camera.scale,
                        },
                        color: Color::new(1.0, 1.0, 1.0, 0.8),
                        ..Default::default()
                    });
                }

                Some(vec![SpriteBatch {
                    sprites,
                    texture: cursor_texture,
                }])
            }
        }
    }
}
