use excali_render::*;
use excali_sprite::*;
use winit::event_loop::EventLoop;

const STACK_SIZE: usize = 10_000_000;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(STACK_SIZE)
        .build()
        .unwrap();
    rt.block_on(game());
}

const PUZZLE_SIZE: usize = 10;
const SIGIL_SIZE: f32 = 50.0;
const SIGIL_DISTANCE: f32 = SIGIL_SIZE * 1.5;
const SIGIL_OFFSET: f32 = SIGIL_DISTANCE * PUZZLE_SIZE as f32 / 2.0 - SIGIL_DISTANCE / 2.0;

#[derive(Copy, Clone)]
struct Sigil {
    active: bool,
}

impl Sigil {
    fn sprite(&self, x: usize, y: usize) -> Sprite {
        Sprite {
            position: [
                x as f32 * SIGIL_DISTANCE - SIGIL_OFFSET,
                y as f32 * SIGIL_DISTANCE - SIGIL_OFFSET,
            ],
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

struct Puzzle {
    sigils: [[Option<Sigil>; PUZZLE_SIZE]; PUZZLE_SIZE],
}

impl Default for Puzzle {
    fn default() -> Self {
        Self {
            sigils: [[Some(Sigil { active: false }); PUZZLE_SIZE]; PUZZLE_SIZE],
        }
    }
}

impl Puzzle {
    fn sprites(&self) -> Vec<Sprite> {
        let mut sprites = Vec::<Sprite>::new();
        for (y, row) in self.sigils.iter().enumerate() {
            for (x, slot) in row.iter().enumerate() {
                if let Some(sigil) = slot {
                    sprites.push(sigil.sprite(x, y));
                }
            }
        }
        sprites
    }
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
    puzzle.sigils[0][0] = Some(Sigil { active: true });

    let sampler = renderer.pixel_art_sampler();
    let sigils_texture = sprite_renderer.create_texture_bind_group(
        &renderer.device,
        &sampler,
        &renderer.load_texture(
            include_bytes!("../assets/sigils.png"),
            Some("Sigils Texture"),
        ),
    );

    event_loop.run(move |event, _, control_flow| {
        if let Err(err) = renderer.handle_event(&event, control_flow, |renderer, view| {
            let sprite_batch = SpriteBatch {
                sprites: puzzle.sprites(),
                texture_bind_group: &sigils_texture,
            };

            let commands = vec![
                renderer.clear(
                    view,
                    Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    },
                ),
                sprite_renderer.draw(
                    &[sprite_batch],
                    &renderer.device,
                    &renderer.queue,
                    view,
                    [renderer.size.width as f32, renderer.size.height as f32],
                ),
            ];
            commands
        }) {
            println!("{err}");
        }
    });
}
