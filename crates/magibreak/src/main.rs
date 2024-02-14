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

async fn load_level_file(name: &str) -> io::Result<String> {
    const LEVELS_PATH: &str = "./assets/levels/";
    let mut file = File::open(format!("{}{}.toml", LEVELS_PATH, name)).await?;

    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;

    Ok(contents)
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

    let seriable_puzzle: SerialablePuzzle =
        toml::from_str(load_level_file("draft").await.unwrap().as_str()).unwrap();
    let mut puzzle = Puzzle::try_from(seriable_puzzle).unwrap();
    let mut level_editor = LevelEditor::default();

    let mut input = Input::new(renderer.window.id());
    let mut ui = UI::new(&renderer.device, &event_loop);

    let sampler = renderer.pixel_art_sampler();
    let orbs_texture = sprite_renderer.create_texture_bind_group(
        &renderer.device,
        &sampler,
        &load_texture_from_file("assets/orbs.png", &renderer)
            .await
            .unwrap(),
    );

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

    let border_texture = sprite_renderer.create_texture_bind_group(
        &renderer.device,
        &sampler,
        &load_texture_from_file("assets/border.png", &renderer)
            .await
            .unwrap(),
    );

    event_loop.run(move |event, _, control_flow| {
        input.handle_event(&event);
        ui.handle_event(&event, renderer.window.id());

        if let Err(err) = renderer.handle_event(&event, control_flow, |renderer, view| {
            let mouse_coordinate = if let Some(mouse_position) = input.mouse_position {
                Some(SigilCoordinate::from_position(
                    mouse_position.world_position(&renderer.size).into(),
                ))
            } else {
                None
            };

            if input.left_mouse_click == InputState::JustPressed {
                if let Some(coordinate) = mouse_coordinate {
                    if !level_editor.enabled {
                        puzzle.input(&coordinate);
                    }
                    level_editor.input(coordinate, &mut puzzle);
                }
            }

            let ui_output = ui.update(
                |ctx| {
                    level_editor.ui(ctx);
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
