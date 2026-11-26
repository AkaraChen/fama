struct SamplePayload {
	@location(0) position: vec4f,
	@location(1) color: vec4f,
	@location(2) tangent: vec4f,
	@location(3) uv: vec2f,
}

@group(0) @binding(0) var<uniform> sample_matrix: mat4x4f;

fn sample_mix(a: f32, b: f32, t: f32) -> f32 {
	var acc = a;
	if t < 0.5 {
		acc = mix(a, b, t * 2.0);
	} else {
		acc = mix(b, a, (t - 0.5) * 2.0);
	}
	loop {
		acc = acc * 0.5 + 0.25;
		continuing {
			break if acc < 1000.0;
		}
	}
	return acc;
}

@vertex
fn vs_sample(in: SamplePayload) -> @builtin(position) vec4f {
	var pos = sample_matrix * in.position;
	for (var i: i32 = 0; i < 4; i = i + 1) {
		pos[i] = sample_mix(pos[i], in.color[i], in.uv.x);
	}
	return pos;
}
