use excali_input::*;
use excali_render::*;
use excali_sprite::*;
use nalgebra::Vector2;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};
use winit::event_loop::EventLoop;

const STACK_SIZE: usize = 10_000_000;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(STACK_SIZE)
        .build()
        .unwrap();
    rt.block_on(game());
}

const PUZZLE_SIZE: usize = 7;
const SIGIL_SIZE: f32 = 50.0;
const SIGIL_SCALE: Vector2<f32> = Vector2::new(SIGIL_SIZE, SIGIL_SIZE);
const CURSOR_SIZE: f32 = 70.0;

const SIGIL_DISTANCE: f32 = SIGIL_SIZE * 1.5;
const SIGIL_OFFSET: f32 = SIGIL_DISTANCE * PUZZLE_SIZE as f32 / 2.0 - SIGIL_DISTANCE / 2.0;

#[derive(Copy, Clone)]
enum Sigil {
    Alpha,
}

impl Sigil {
    fn sprite(&self, coordinate: SigilCoordinate, lines: &[Line]) -> Sprite {
        Sprite {
            transform: Transform {
                position: coordinate.position(),
                rotation: 0.0,
                scale: SIGIL_SCALE,
            },
            texture_coordinate: self.texture_coordinate(coordinate, lines),
        }
    }

    fn active(&self, coordinate: SigilCoordinate, lines: &[Line]) -> bool {
        for line in lines.iter() {
            if line.start == coordinate || line.end == coordinate {
                return true;
            }
        }
        false
    }

    fn texture_coordinate(&self, coordinate: SigilCoordinate, lines: &[Line]) -> TextureCoordinate {
        if self.active(coordinate, lines) {
            return TextureCoordinate {
                width: 0.5,
                height: 1.0,
                y: 0.0,
                x: 0.5,
            };
        }
        TextureCoordinate {
            width: 0.5,
            height: 1.0,
            y: 0.0,
            x: 0.0,
        }
    }
}

#[derive(PartialEq, Eq, Copy, Clone)]
struct SigilCoordinate(Vector2<usize>);

impl SigilCoordinate {
    fn valid(&self) -> bool {
        self.0.x < PUZZLE_SIZE || self.0.y < PUZZLE_SIZE
    }

    fn position(&self) -> Vector2<f32> {
        Vector2::new(
            self.0.x as f32 * SIGIL_DISTANCE - SIGIL_OFFSET,
            self.0.y as f32 * SIGIL_DISTANCE - SIGIL_OFFSET,
        )
    }

    fn from_position(position: Vector2<f32>) -> Self {
        SigilCoordinate(Vector2::new(
            ((position.x + SIGIL_OFFSET) / SIGIL_DISTANCE + 0.5).floor() as usize,
            ((position.y + SIGIL_OFFSET) / SIGIL_DISTANCE + 0.5).floor() as usize,
        ))
    }
}

fn line_between(start: Vector2<f32>, end: Vector2<f32>, thickness: f32) -> Transform {
    let position = (start + end) / 2.0;
    let direction = end - start;
    let magnitude = direction.magnitude();

    let rotation = if direction.x < 0.0 {
        (direction.y / magnitude).asin()
    } else {
        (-direction.y / magnitude).asin()
    };

    Transform {
        position,
        rotation,
        scale: Vector2::new(magnitude, thickness),
    }
}

struct Line {
    start: SigilCoordinate,
    end: SigilCoordinate,
}

impl Line {
    fn sprite(&self) -> Sprite {
        Sprite {
            transform: line_between(self.start.position(), self.end.position(), 40.0),
            texture_coordinate: Default::default(),
        }
    }
}

struct Puzzle {
    sigils: [[Option<Sigil>; PUZZLE_SIZE]; PUZZLE_SIZE],
    lines: Vec<Line>,
    cursor: SigilCoordinate,
}

impl Default for Puzzle {
    fn default() -> Self {
        Self {
            sigils: [[Some(Sigil::Alpha); PUZZLE_SIZE]; PUZZLE_SIZE],
            lines: Vec::new(),
            cursor: SigilCoordinate(Vector2::zeros()),
        }
    }
}

impl Puzzle {
    fn get_sigil_mut(&mut self, coordinate: &SigilCoordinate) -> Option<&mut Option<Sigil>> {
        if !coordinate.valid() {
            return None;
        }

        Some(&mut self.sigils[coordinate.0.y][coordinate.0.x])
    }

    fn input(&mut self, coordinate: &SigilCoordinate) {
        if *coordinate == self.cursor || !coordinate.valid() {
            return;
        }
        if let Some(_sigil) = self.sigils[coordinate.0.y][coordinate.0.x] {
            self.lines.push(Line {
                start: self.cursor,
                end: *coordinate,
            });
            self.cursor = *coordinate;
        }
    }

    fn sprite_batches<'a>(
        &'a self,
        cursor_texture: &'a wgpu::BindGroup,
        sigils_texture: &'a wgpu::BindGroup,
    ) -> Vec<SpriteBatch> {
        let mut sprites = Vec::<Sprite>::new();
        for (y, row) in self.sigils.iter().enumerate() {
            for (x, slot) in row.iter().enumerate() {
                if let Some(sigil) = slot {
                    sprites.push(sigil.sprite(SigilCoordinate(Vector2::new(x, y)), &self.lines));
                }
            }
        }

        let mut circle_sprites: Vec<Sprite> = self.lines.iter().map(|line| line.sprite()).collect();
        circle_sprites.push(Sprite {
            transform: Transform {
                scale: Vector2::new(CURSOR_SIZE, CURSOR_SIZE),
                position: self.cursor.position(),
                rotation: 0.0,
            },
            texture_coordinate: Default::default(),
        });

        let cursor = SpriteBatch {
            sprites: circle_sprites,
            texture_bind_group: cursor_texture,
        };

        vec![
            cursor,
            SpriteBatch {
                sprites,
                texture_bind_group: sigils_texture,
            },
        ]
    }
}

// TODO move to excali_renderer under feature
async fn load_texture_from_file(path: &str, renderer: &Renderer) -> io::Result<wgpu::TextureView> {
    let mut file = File::open(path).await?;

    let mut bytes = vec![];
    file.read_to_end(&mut bytes).await?;

    Ok(renderer.load_texture(&bytes, Some(path)))
}

async fn game() {
    let mut event_loop = EventLoop::new();
    let mut renderer = Renderer::new(&mut event_loop).await;
    let mut sprite_renderer = SpriteRenderer::new(
        &renderer.config,
        &renderer.device,
        renderer.size.width as f32,
        renderer.size.height as f32,
    );
    let mut puzzle = Puzzle::default();
    let mut input = Input::new(renderer.window.id());
    puzzle.sigils[PUZZLE_SIZE - 1][PUZZLE_SIZE - 1] = Some(Sigil::Alpha);

    let sampler = renderer.pixel_art_sampler();
    let sigils_texture = sprite_renderer.create_texture_bind_group(
        &renderer.device,
        &sampler,
        &load_texture_from_file("assets/sigils.png", &renderer)
            .await
            .unwrap(),
    );

    let cursor_texture = sprite_renderer.create_texture_bind_group(
        &renderer.device,
        &sampler,
        &load_texture_from_file("assets/cursor.png", &renderer)
            .await
            .unwrap(),
    );

    event_loop.run(move |event, _, control_flow| {
        input.handle_event(&event);

        if let Err(err) = renderer.handle_event(&event, control_flow, |renderer, view| {
            if input.left_mouse_click == InputState::JustPressed {
                if let Some(mouse_position) = input.mouse_position {
                    puzzle.input(&SigilCoordinate::from_position(
                        mouse_position.world_position(&renderer.size).into(),
                    ));
                }
            }

            let batches = puzzle.sprite_batches(&cursor_texture, &sigils_texture);

            let commands = vec![
                renderer.clear(
                    view,
                    Color {
                        r: 0.4,
                        g: 0.4,
                        b: 0.4,
                        a: 1.0,
                    },
                ),
                sprite_renderer.draw(
                    &batches,
                    &renderer.device,
                    &renderer.queue,
                    view,
                    [renderer.size.width as f32, renderer.size.height as f32],
                ),
            ];

            input.clear();
            commands
        }) {
            println!("{err}");
        }
    });
}
