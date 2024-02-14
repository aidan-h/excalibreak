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
const CURSOR_SIZE: f32 = 70.0;

const SIGIL_DISTANCE: f32 = SIGIL_SIZE * 1.5;
const SIGIL_OFFSET: f32 = SIGIL_DISTANCE * PUZZLE_SIZE as f32 / 2.0 - SIGIL_DISTANCE / 2.0;

#[derive(Copy, Clone)]
struct Sigil {
    active: bool,
}

impl Sigil {
    fn sprite(&self, coordinate: SigilCoordinate) -> Sprite {
        Sprite {
            position: coordinate.position().data.0[0],
            size: [SIGIL_SIZE, SIGIL_SIZE],
            texture_coordinate: self.texture_coordinate(),
        }
    }

    fn texture_coordinate(&self) -> TextureCoordinate {
        if self.active {
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

struct SigilCoordinate(Vector2<usize>);

impl SigilCoordinate {
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

struct Puzzle {
    sigils: [[Option<Sigil>; PUZZLE_SIZE]; PUZZLE_SIZE],
    cursor: SigilCoordinate,
}

impl Default for Puzzle {
    fn default() -> Self {
        Self {
            sigils: [[Some(Sigil { active: false }); PUZZLE_SIZE]; PUZZLE_SIZE],
            cursor: SigilCoordinate(Vector2::zeros()),
        }
    }
}

impl Puzzle {
    fn get_sigil_mut(&mut self, coordinate: &SigilCoordinate) -> Option<&mut Option<Sigil>> {
        if coordinate.0.x >= PUZZLE_SIZE || coordinate.0.y >= PUZZLE_SIZE {
            return None;
        }

        Some(&mut self.sigils[coordinate.0.y][coordinate.0.x])
    }

    fn sprite_batches<'a>(
        &'a self,
        cursor_texture: &'a wgpu::BindGroup,
        sigils_texture: &'a wgpu::BindGroup,
    ) -> [SpriteBatch; 2] {
        let mut sprites = Vec::<Sprite>::new();
        for (y, row) in self.sigils.iter().enumerate() {
            for (x, slot) in row.iter().enumerate() {
                if let Some(sigil) = slot {
                    sprites.push(sigil.sprite(SigilCoordinate(Vector2::new(x, y))));
                }
            }
        }
        let cursor = SpriteBatch {
            sprites: vec![Sprite {
                position: self.cursor.position().data.0[0],
                size: [CURSOR_SIZE, CURSOR_SIZE],
                texture_coordinate: Default::default(),
            }],
            texture_bind_group: cursor_texture,
        };

        [
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
    puzzle.sigils[0][0] = Some(Sigil { active: true });

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
            if input.left_mouse_click {
                if let Some(mouse_position) = input.mouse_position {
                    if let Some(sigil) = puzzle.get_sigil_mut(&SigilCoordinate::from_position(
                        mouse_position.world_position(&renderer.size).into(),
                    )) {
                        *sigil = Some(Sigil { active: true });
                    }
                }
            }

            let batches =
                Vec::<SpriteBatch>::from(puzzle.sprite_batches(&cursor_texture, &sigils_texture));

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
