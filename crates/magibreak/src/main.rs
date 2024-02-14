use crate::level_editor::*;
use crate::puzzle::*;
use excali_input::*;
use excali_render::*;
use excali_sprite::*;
use excali_ui::*;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};
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

// TODO move to excali_renderer under feature
async fn load_texture_from_file(path: &str, renderer: &Renderer) -> io::Result<wgpu::TextureView> {
    let mut file = File::open(path).await?;

    let mut bytes = vec![];
    file.read_to_end(&mut bytes).await?;

    Ok(renderer.load_texture(&bytes, Some(path)))
}

async fn load_texture(
    path: &str,
    sprite_renderer: &SpriteRenderer,
    renderer: &Renderer,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    sprite_renderer.create_texture_bind_group(
        &renderer.device,
        sampler,
        &load_texture_from_file(path, renderer).await.unwrap(),
    )
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

    let mut level_editor = LevelEditor::new("draft".to_string()).await;
    let mut puzzle = level_editor.loaded_puzzle.clone();

    let mut input = Input::new(renderer.window.id());
    let mut ui = UI::new(&renderer.device, &event_loop);

    let sampler = renderer.pixel_art_sampler();

    let orbs_texture = load_texture("assets/orbs.png", &sprite_renderer, &renderer, &sampler).await;
    let border_texture =
        load_texture("assets/border.png", &sprite_renderer, &renderer, &sampler).await;
    let sigils_texture =
        load_texture("assets/sigils.png", &sprite_renderer, &renderer, &sampler).await;
    let cursor_texture =
        load_texture("assets/cursor.png", &sprite_renderer, &renderer, &sampler).await;

    event_loop.run(move |event, _, control_flow| {
        input.handle_event(&event, ui.handle_event(&event, renderer.window.id()));

        if let Err(err) = renderer.handle_event(&event, control_flow, |renderer, view| {
            let mouse_coordinate = if let Some(mouse_position) = input.mouse_position {
                Some(SigilCoordinate::from_position(
                    mouse_position.world_position(&renderer.size).into(),
                ))
            } else {
                None
            };

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

            let mut batches =
                puzzle.sprite_batches(&cursor_texture, &sigils_texture, &orbs_texture);
            if let Some(coordinate) = mouse_coordinate {
                if let Some(mut editor_batches) = level_editor.sprite_batches(
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
