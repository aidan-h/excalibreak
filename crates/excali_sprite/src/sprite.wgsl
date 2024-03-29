// Vertex shader
struct WindowUniform {
    size: vec2<f32>,
};

@group(1) @binding(0)
var<uniform> window: WindowUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
	@location(1) tex_coords: vec2<f32>,
	@location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
	model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>((model.position + window.size / 2.0) / window.size * 2.0 - 1.0, 0.5, 1.0);
	out.tex_coords = model.tex_coords;
	out.color = model.color;
    return out;
}

// Fragment shader

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0)@binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords) * in.color;
}

