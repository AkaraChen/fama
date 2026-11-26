struct VertexInput {
	@location(0) position: vec4f,
	@location(1) normal: vec3f,
	@location(2) uv: vec2f,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4f,
	@location(0) world_normal: vec3f,
	@location(1) tex_coords: vec2f,
}

@group(0) @binding(0) var<uniform> model_view_proj: mat4x4f;
@group(0) @binding(1) var<uniform> light_dir: vec3f;
@group(1) @binding(0) var<uniform> albedo: vec4f;
@group(1) @binding(1) var albedo_texture: texture_2d<f32>;
@group(1) @binding(2) var albedo_sampler: sampler;

fn saturate3(v: vec3f) -> vec3f {
	return clamp(v, vec3f(0.0), vec3f(1.0));
}

fn lambert(normal: vec3f, light: vec3f) -> f32 {
	return max(dot(normalize(normal), normalize(light)), 0.0);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.clip_position = model_view_proj * vec4f(in.position.xyz, 1.0);
	out.world_normal = in.normal;
	out.tex_coords = in.uv;
	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
	let texel = textureSample(albedo_texture, albedo_sampler, in.tex_coords);
	let ndl = lambert(in.world_normal, light_dir);
	var color = albedo.rgb * texel.rgb * (0.1 + 0.9 * ndl);
	for (var i: i32 = 0; i < 3; i = i + 1) {
		color[i] = saturate3(color)[i];
	}
	return vec4f(color, texel.a);
}
