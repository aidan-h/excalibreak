use excali_io::tokio::sync::oneshot;
use excali_io::{load_file, tokio};
use excali_io::{load_from_toml, receive_oneshot_rx, OneShotStatus};
use log::error;
use nalgebra::Vector2;
use std::time::Instant;

use crate::level_editor::*;
use crate::puzzle::*;
use excali_input::*;
use excali_render::*;
use excali_sprite::*;
use excali_ui::*;
use winit::event_loop::EventLoop;

use self::input::*;
use self::map::Map;

mod input;
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
#[derive(Debug)]
struct PuzzlePlayer {
    puzzle: ActivePuzzle,
    editor: LevelEditor,
}

impl PuzzlePlayer {
    async fn new(level: String) -> Self {
        let editor = LevelEditor::new(level + ".toml").await;
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
    let mut load_puzzle_rx: Option<oneshot::Receiver<PuzzlePlayer>> = None;

    let mut input = Input::new(renderer.window.id(), Actions::default());
    let mut ui = UI::new(&renderer.device, &event_loop);
    let mut map = Map::new(
        load_from_toml::<map::SerializableGrid>(map::grid::MAP_FILE_PATH)
            .await
            .unwrap()
            .try_into()
            .unwrap(),
        &renderer.config,
        &renderer.device,
    );

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

            let time = renderer
                .last_frame
                .duration_since(start_instant)
                .as_secs_f32();

            let mut batches = Vec::<SpriteBatch>::new();

            let ui_output = ui.update(
                |ctx| {
                    if let Some(player) = puzzle_player.as_mut() {
                        player.editor.ui(ctx, &mut player.puzzle);
                    } else {
                        map.ui(ctx);
                    }
                },
                &renderer.device,
                &renderer.queue,
                view,
                &renderer.window,
                [renderer.size.width, renderer.size.height],
            );

            // TODO temporary
            if input.input_map.escape.button.state == InputState::JustPressed {
                puzzle_player = None;
            }

            // TODO borrow checker is stupid here, why can't I have a mut ref of a variable used in
            // if condition?!
            if puzzle_player.is_none() {
                match receive_oneshot_rx(&mut load_puzzle_rx) {
                    OneShotStatus::Closed => error!("Load level channel closed"),
                    OneShotStatus::Value(player) => puzzle_player = Some(player),
                    OneShotStatus::None => {
                        if let Some(zone_coordinate) = map.input(&input, renderer, delta) {
                            // TODO this is shit
                            let zone_name = map
                                .grid
                                .zones
                                .get(&zone_coordinate)
                                .unwrap()
                                .level_name
                                .clone();
                            let (tx, rx) = oneshot::channel();
                            tokio::spawn(async move {
                                tx.send(PuzzlePlayer::new(zone_name).await).unwrap();
                            });
                            load_puzzle_rx = Some(rx);
                        }
                    }
                    OneShotStatus::Empty => (),
                }
            }
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
            // process map input if there isn't a level loaded

            let mut map_commands = map.draw(renderer, view, debug);
            let mut commands = vec![renderer.clear(
                view,
                wgpu::Color {
                    r: 0.4,
                    g: 0.4,
                    b: 0.4,
                    a: 1.0,
                },
            )];
            for command in map_commands.drain(..) {
                commands.push(command);
            }
            commands.push(sprite_renderer.draw(
                &batches,
                &renderer.device,
                &renderer.queue,
                view,
                [renderer.size.width as f32, renderer.size.height as f32],
            ));
            commands.push(ui_output);

            input.clear();
            commands
        }) {
            println!("{err}");
        }
    });
}
