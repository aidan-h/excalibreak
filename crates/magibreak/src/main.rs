use std::collections::HashMap;

use excali_input::*;
use excali_render::*;
use excali_sprite::*;
use gcd::Gcd;
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

const SIGIL_SIZE: f32 = 50.0;
const SIGIL_SCALE: Vector2<f32> = Vector2::new(SIGIL_SIZE, SIGIL_SIZE);
const CURSOR_SIZE: f32 = 70.0;

const SIGIL_DISTANCE: f32 = SIGIL_SIZE * 1.5;

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

    fn allow_intersections(&self) -> bool {
        false
    }

    fn active(&self, coordinate: SigilCoordinate, lines: &[Line]) -> bool {
        for line in lines.iter() {
            for touching_coordinate in line.coordinates().iter() {
                if *touching_coordinate == coordinate {
                    return true;
                }
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

type CoordinateScalar = i32;
type Position = Vector2<f32>;
type SigilCoordinate = Vector2<CoordinateScalar>;

trait Coordinate {
    fn position(&self) -> Position;
    fn from_position(position: Position) -> Self;
}

impl Coordinate for SigilCoordinate {
    fn position(&self) -> Vector2<f32> {
        Vector2::new(
            self.x as f32 * SIGIL_DISTANCE,
            self.y as f32 * SIGIL_DISTANCE,
        )
    }

    fn from_position(position: Vector2<f32>) -> Self {
        Vector2::new(
            (position.x / SIGIL_DISTANCE + 0.5).floor() as CoordinateScalar,
            (position.y / SIGIL_DISTANCE + 0.5).floor() as CoordinateScalar,
        )
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

#[derive(Debug)]
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

    fn cross(a: &Position, b: &Position) -> f32 {
        a.x * b.y - a.y * b.x
    }

    fn intersects(&self, other: &Self) -> bool {
        // endpoints are not considered
        if (self.start == other.start && self.end != other.end)
            || (self.start == other.end && self.end != other.start)
            || (self.end == other.start && self.start != other.end)
            || (self.end == other.end && self.start != other.start)
        {
            return false;
        }

        // see https://stackoverflow.com/a/565282 & ucarion/line_intersection
        let p = Position::new(self.start.x as f32, self.start.y as f32);
        let q = Position::new(other.start.x as f32, other.start.y as f32);
        let r = Position::new(self.end.x as f32, self.end.y as f32) - p;
        let s = Position::new(other.end.x as f32, other.end.y as f32) - q;

        let r_cross_s = Self::cross(&r, &s);
        let q_minus_p = q - p;
        let q_minus_p_cross_r = Self::cross(&q_minus_p, &r);

        // are the lines are parallel?
        if r_cross_s == 0.0 {
            // are the lines collinear?
            q_minus_p_cross_r == 0.0
        } else {
            // the lines are not parallel
            let t = Self::cross(&q_minus_p, &(s / r_cross_s));
            let u = Self::cross(&q_minus_p, &(r / r_cross_s));

            // are the intersection coordinates both in range?
            let t_in_range = (0.0..=1.0).contains(&t);
            let u_in_range = (0.0..=1.0).contains(&u);

            t_in_range && u_in_range
        }
    }

    /// retrieves all touching coordinates
    fn coordinates(&self) -> Vec<SigilCoordinate> {
        if self.end == self.start {
            return vec![self.start];
        }
        let mut slope = self.end - self.start;
        let cardinal_direction = SigilCoordinate::new(
            if slope.x < 0 { -1 } else { 1 },
            if slope.y < 0 { -1 } else { 1 },
        );
        // for gcd
        slope.component_mul_assign(&cardinal_direction);

        // number of touching coordinates after start
        let gcd = (slope.x as u32).gcd(slope.y as u32) as i32;
        slope.component_mul_assign(&cardinal_direction);
        slope /= gcd;
        let mut coordinates = vec![self.start];
        for i in 1..=gcd {
            coordinates.push(slope * i + self.start);
        }
        coordinates
    }
}

struct Puzzle {
    sigils: HashMap<SigilCoordinate, Sigil>,
    lines: Vec<Line>,
    cursor: SigilCoordinate,
}

impl Default for Puzzle {
    fn default() -> Self {
        Self {
            sigils: HashMap::new(),
            lines: Vec::new(),
            cursor: Vector2::zeros(),
        }
    }
}

impl Puzzle {
    fn input(&mut self, coordinate: &SigilCoordinate) {
        if *coordinate == self.cursor {
            return;
        }
        if let Some(cursor_sigil) = self.sigils.get(&self.cursor) {
            let line = Line {
                start: self.cursor,
                end: *coordinate,
            };

            if !cursor_sigil.allow_intersections() && self.intersects_lines(&line) {
                return;
            }

            if let Some(_sigil) = self.sigils.get(coordinate) {
                self.lines.push(line);
                self.cursor = *coordinate;
            }
        }
    }

    fn intersects_lines(&self, line: &Line) -> bool {
        for other_line in self.lines.iter() {
            if other_line.intersects(line) {
                return true;
            }
        }
        false
    }

    fn sprite_batches<'a>(
        &'a self,
        cursor_texture: &'a wgpu::BindGroup,
        sigils_texture: &'a wgpu::BindGroup,
    ) -> Vec<SpriteBatch> {
        let mut sprites = Vec::<Sprite>::new();
        for (coordinate, sigil) in self.sigils.iter() {
            sprites.push(sigil.sprite(*coordinate, &self.lines));
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
    puzzle.sigils.insert(Vector2::zeros(), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(1, 0), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(0, 1), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(1, 1), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(0, 2), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(2, 0), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(1, 2), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(2, 1), Sigil::Alpha);
    puzzle.sigils.insert(Vector2::new(2, 2), Sigil::Alpha);

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
