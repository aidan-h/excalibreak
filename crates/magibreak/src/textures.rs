use excali_render::Renderer;
use excali_sprite::{SpriteRenderer, SpriteTexture};

use crate::load_sprite_texture;

pub struct Textures {
    pub orbs: SpriteTexture,
    pub border: SpriteTexture,
    pub sigils: SpriteTexture,
    pub cursor: SpriteTexture,
    pub line: SpriteTexture,
}

impl Textures {
    pub async fn new(
        sprite_renderer: &SpriteRenderer,
        renderer: &Renderer,
        sampler: &wgpu::Sampler,
        line_sampler: &wgpu::Sampler,
    ) -> Self {
        Self {
            orbs: load_sprite_texture("assets/orbs.png", sprite_renderer, renderer, sampler).await,
            border: load_sprite_texture("assets/border.png", sprite_renderer, renderer, sampler)
                .await,
            sigils: load_sprite_texture("assets/sigils.png", sprite_renderer, renderer, sampler)
                .await,
            cursor: load_sprite_texture("assets/cursor.png", sprite_renderer, renderer, sampler)
                .await,
            line: load_sprite_texture("assets/line.png", sprite_renderer, renderer, line_sampler)
                .await,
        }
    }
}
