use excali_render::*;
//use excali_sprite::*;
use winit::event_loop::EventLoop;

const STACK_SIZE: usize = 10_000_000;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(STACK_SIZE)
        .build()
        .unwrap();
    rt.block_on(game());
}

async fn game() {
    let mut event_loop = EventLoop::new();
    let mut renderer = Renderer::new(&mut event_loop).await;
    event_loop.run(move |event, _, control_flow| {
        if let Err(err) = renderer.handle_event(&event, control_flow, |renderer, view| {
            let commands = vec![renderer.clear(
                view,
                Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            )];
            commands
        }) {
            println!("{err}");
        }
    });
}
