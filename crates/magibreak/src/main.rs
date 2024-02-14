use excali_io::{load_file, tokio};
use nalgebra::Vector2;
use nalgebra::Vector3;
use std::time::Instant;
use winit::event::VirtualKeyCode;

use crate::level_editor::*;
use crate::puzzle::*;
use excali_input::*;
use excali_render::*;
use excali_sprite::*;
use excali_ui::*;
use winit::event_loop::EventLoop;

use self::map::grid::Grid;
use self::map::Map;

mod level_editor;
mod map;
mod puzzle;

const STACK_SIZE: usize = 10_000_000;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(STACK_SIZE)
        .build()
        .unwrap();
    rt.block_on(game());
}

struct Actions {
    undo: Action,
    debug: Action,
    camera_forward: Action,
    camera_backward: Action,
    camera_left: Action,
    camera_right: Action,
    camera_up: Action,
    camera_down: Action,
}

impl Default for Actions {
    fn default() -> Self {
        Self {
            undo: Action::new(VirtualKeyCode::U),
            debug: Action::new(VirtualKeyCode::F2),
            camera_forward: Action::new(VirtualKeyCode::W),
            camera_backward: Action::new(VirtualKeyCode::S),
            camera_left: Action::new(VirtualKeyCode::A),
            camera_right: Action::new(VirtualKeyCode::D),
            camera_up: Action::new(VirtualKeyCode::Space),
            camera_down: Action::new(VirtualKeyCode::LShift),
        }
    }
}

impl InputMap for Actions {
    fn actions(&mut self) -> Vec<&mut Action> {
        vec![
            &mut self.undo,
            &mut self.camera_forward,
            &mut self.camera_up,
            &mut self.camera_down,
            &mut self.camera_right,
            &mut self.camera_left,
            &mut self.camera_backward,
            &mut self.debug,
        ]
    }
}
async fn load_sprite_texture(
    path: &str,
    sprite_renderer: &SpriteRenderer,
    renderer: &Renderer,
    sampler: &wgpu::Sampler,
) -> SpriteTexture {
    let texture = renderer.load_texture(&load_file(path).await.unwrap(), path.to_string());
    let bind_group = sprite_renderer.create_bind_group(&renderer.device, sampler, &texture);
    SpriteTexture {
        data: texture,
        bind_group,
    }
}

/// The user facing puzzle
struct PuzzlePlayer {
    puzzle: ActivePuzzle,
    editor: LevelEditor,
}

impl PuzzlePlayer {
    async fn new() -> Self {
        let editor = LevelEditor::new("draft.toml".to_string()).await;
        let puzzle = ActivePuzzle::new(editor.loaded_puzzle.clone());
        Self { editor, puzzle }
    }
}

async fn game() {
    env_logger::init();

    let mut event_loop = EventLoop::new();
    let mut renderer = Renderer::new(&mut event_loop, wgpu::Features::POLYGON_MODE_LINE).await;
    let mut sprite_renderer = SpriteRenderer::new(
        &renderer.config,
        &renderer.device,
        renderer.size.width as f32,
        renderer.size.height as f32,
    );

    let mut puzzle_player: Option<PuzzlePlayer> = None;

    let mut input = Input::new(renderer.window.id(), Actions::default());
    let mut ui = UI::new(&renderer.device, &event_loop);
    let mut map = Map::new(Grid::from_noise(), &renderer.config, &renderer.device);

    let sampler = renderer.pixel_art_sampler();
    let line_sampler = renderer.pixel_art_wrap_sampler();

    let orbs_texture =
        load_sprite_texture("assets/orbs.png", &sprite_renderer, &renderer, &sampler).await;
    let border_texture =
        load_sprite_texture("assets/border.png", &sprite_renderer, &renderer, &sampler).await;
    let sigils_texture =
        load_sprite_texture("assets/sigils.png", &sprite_renderer, &renderer, &sampler).await;
    let cursor_texture =
        load_sprite_texture("assets/cursor.png", &sprite_renderer, &renderer, &sampler).await;
    let line_texture = load_sprite_texture(
        "assets/line.png",
        &sprite_renderer,
        &renderer,
        &line_sampler,
    )
    .await;
    let start_instant = Instant::now();
    let camera = Transform::from_scale(Vector2::new(2.0, 2.0));
    let mut debug = false;

    event_loop.run(move |event, _, control_flow| {
        input.handle_event(&event, ui.handle_event(&event, renderer.window.id()));
        if let Err(err) = renderer.handle_event(&event, control_flow, |renderer, view, delta| {
            let mouse_coordinate = if let Some(mouse_position) = input.mouse_position {
                Some(SigilCoordinate::from_position(
                    camera
                        .to_object_space(&Transform::from_position(
                            mouse_position.world_position(&renderer.size).into(),
                        ))
                        .position,
                ))
            } else {
                None
            };

            if input.input_map.debug.button.state == InputState::JustPressed {
                debug = !debug;
            }

            {
                let mut direction = Vector3::<f32>::zeros();

                if input.input_map.camera_forward.button.state.pressed() {
                    direction += Vector3::new(0.0, 0.0, 1.0);
                }

                if input.input_map.camera_backward.button.state.pressed() {
                    direction -= Vector3::new(0.0, 0.0, 1.0);
                }

                if input.input_map.camera_left.button.state.pressed() {
                    direction -= Vector3::new(1.0, 0.0, 0.0);
                }

                if input.input_map.camera_right.button.state.pressed() {
                    direction += Vector3::new(1.0, 0.0, 0.0);
                }

                if input.input_map.camera_up.button.state.pressed() {
                    direction += Vector3::new(0.0, 1.0, 0.0);
                }

                if input.input_map.camera_down.button.state.pressed() {
                    direction -= Vector3::new(0.0, 1.0, 0.0);
                }
                const CAMERA_SPEED: f32 = 5.0;

                if direction.magnitude_squared() >= 1.0 {
                    map.camera.input_fly(direction, delta as f32 * CAMERA_SPEED);
                }
            }

            let time = renderer
                .last_frame
                .duration_since(start_instant)
                .as_secs_f32();

            let mut batches = Vec::<SpriteBatch>::new();
            let mut ui_output: Option<wgpu::CommandBuffer> = None;

            if let Some(player) = puzzle_player.as_mut() {
                if input.input_map.undo.button.state == InputState::JustPressed {
                    player.puzzle.undo();
                }
                if !input.left_mouse_click.consumed
                    && input.left_mouse_click.state == InputState::JustPressed
                {
                    if let Some(coordinate) = mouse_coordinate {
                        if !player.editor.enabled {
                            player.puzzle.input(&coordinate);
                        }
                        player.editor.input(coordinate, &mut player.puzzle);
                    }
                }
                ui_output = Some(ui.update(
                    |ctx| {
                        player.editor.ui(ctx, &mut player.puzzle);
                    },
                    &renderer.device,
                    &renderer.queue,
                    view,
                    &renderer.window,
                    [renderer.size.width, renderer.size.height],
                ));
                for batch in player
                    .puzzle
                    .sprite_batches(
                        time,
                        &camera,
                        &cursor_texture,
                        &sigils_texture,
                        &orbs_texture,
                        &line_texture,
                    )
                    .drain(..)
                {
                    batches.push(batch);
                }
                if let Some(coordinate) = mouse_coordinate {
                    if let Some(mut editor_batches) = player.editor.sprite_batches(
                        &camera,
                        coordinate,
                        &cursor_texture,
                        &sigils_texture,
                        &orbs_texture,
                        &border_texture,
                    ) {
                        for batch in editor_batches.drain(..) {
                            batches.push(batch);
                        }
                    }
                }
            }

            let mut commands = vec![
                renderer.clear(
                    view,
                    wgpu::Color {
                        r: 0.4,
                        g: 0.4,
                        b: 0.4,
                        a: 1.0,
                    },
                ),
                map.draw(renderer, view, debug),
                sprite_renderer.draw(
                    &batches,
                    &renderer.device,
                    &renderer.queue,
                    view,
                    [renderer.size.width as f32, renderer.size.height as f32],
                ),
            ];

            if let Some(output) = ui_output {
                commands.push(output);
            }

            input.clear();
            commands
        }) {
            println!("{err}");
        }
    });
}
