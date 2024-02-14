use crate::puzzle::*;
use excali_sprite::{Color, Sprite, SpriteBatch, Transform};
use excali_ui::egui_winit::egui::{self, Context};
use nalgebra::Vector2;

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

pub struct LevelEditor {
    pub enabled: bool,
    mode: LevelEditorMode,
    rune: Rune,
}

impl Default for LevelEditor {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: LevelEditorMode::Place,
            rune: Rune {
                sigil: Sigil::Alpha,
                orb: Orb::Circle,
            },
        }
    }
}

impl LevelEditor {
    pub fn ui(&mut self, ctx: &Context, puzzle: &mut Puzzle, loaded_puzzle: &mut Puzzle) {
        egui::Window::new("level editor").show(ctx, |ui| {
            if ui
                .button(if self.enabled { "Play" } else { "Edit" })
                .clicked()
            {
                self.toggle(puzzle, loaded_puzzle);
            }
            if self.enabled {
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
            }
        });
    }

    pub fn input(&self, coordinate: SigilCoordinate, puzzle: &mut Puzzle, loaded_puzzle: &mut Puzzle ) {
        if !self.enabled {
            return;
        }
        match self.mode {
            LevelEditorMode::Cursor => {loaded_puzzle.cursor = coordinate;},
            LevelEditorMode::Clear => {loaded_puzzle.runes.remove(&coordinate);},
            LevelEditorMode::Place => {loaded_puzzle.runes.insert(coordinate, self.rune);},
        };
        *puzzle = loaded_puzzle.clone();
    }

    fn toggle(&mut self, puzzle: &mut Puzzle, loaded_puzzle: &mut Puzzle) {
        self.enabled = !self.enabled;
        *puzzle = loaded_puzzle.clone();
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
