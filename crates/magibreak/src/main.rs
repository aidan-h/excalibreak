use excali_io::{load_file, tokio};
use nalgebra::Vector2;
use std::time::Instant;

use crate::level_editor::*;
use crate::puzzle::*;
use excali_input::*;
use excali_render::*;
use excali_sprite::*;
use excali_ui::*;
use winit::event_loop::EventLoop;

mod level_editor;
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
}

impl Default for Actions {
    fn default() -> Self {
        Self {
            undo: Action::new(winit::event::VirtualKeyCode::U),
        }
    }
}

impl InputMap for Actions {
    fn actions(&mut self) -> Vec<&mut Action> {
        vec![&mut self.undo]
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

async fn game() {
    env_logger::init();

    let mut event_loop = EventLoop::new();
    let mut renderer = Renderer::new(&mut event_loop).await;
    let mut sprite_renderer = SpriteRenderer::new(
        &renderer.config,
        &renderer.device,
        renderer.size.width as f32,
        renderer.size.height as f32,
    );

    let mut level_editor = LevelEditor::new("draft.toml".to_string()).await;
    let mut puzzle = ActivePuzzle::new(level_editor.loaded_puzzle.clone());

    let mut input = Input::new(renderer.window.id(), Actions::default());
    let mut ui = UI::new(&renderer.device, &event_loop);

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

    event_loop.run(move |event, _, control_flow| {
        input.handle_event(&event, ui.handle_event(&event, renderer.window.id()));
        if let Err(err) = renderer.handle_event(&event, control_flow, |renderer, view| {
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

            if input.input_map.undo.button.state == InputState::JustPressed {
                puzzle.undo();
            }

            if !input.left_mouse_click.consumed
                && input.left_mouse_click.state == InputState::JustPressed
            {
                if let Some(coordinate) = mouse_coordinate {
                    if !level_editor.enabled {
                        puzzle.input(&coordinate);
                    }
                    level_editor.input(coordinate, &mut puzzle);
                }
            }

            let ui_output = ui.update(
                |ctx| {
                    level_editor.ui(ctx, &mut puzzle);
                },
                &renderer.device,
                &renderer.queue,
                view,
                &renderer.window,
                [renderer.size.width, renderer.size.height],
            );

            let time = renderer
                .last_frame
                .duration_since(start_instant)
                .as_secs_f32();

            let mut batches = puzzle.sprite_batches(
                time,
                &camera,
                &cursor_texture,
                &sigils_texture,
                &orbs_texture,
                &line_texture,
            );
            if let Some(coordinate) = mouse_coordinate {
                if let Some(mut editor_batches) = level_editor.sprite_batches(
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

            let commands = vec![
                renderer.clear(
                    view,
                    wgpu::Color {
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
                ui_output,
            ];

            input.clear();
            commands
        }) {
            println!("{err}");
        }
    });
}
