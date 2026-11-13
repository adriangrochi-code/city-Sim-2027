shader_type spatial;
render_mode cull_disabled, diffuse_burley;

// Calles coloniales: 0 = camino de tierra con huellas, 1 = empedrado de granito.
uniform int kind = 0;
uniform vec3 base_color : source_color = vec3(0.5, 0.4, 0.28);

varying vec3 wpos;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

void vertex() {
	wpos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec2 p = wpos.xz;
	vec3 col;
	float rough;
	if (kind == 1) {
		vec2 g = p * 2.6;
		g.x += floor(g.y) * 0.5;
		vec2 cell = floor(g);
		vec2 f = fract(g);
		float edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
		float grout = smoothstep(0.03, 0.14, edge);
		col = base_color * (0.72 + 0.4 * hash(cell));
		col = mix(base_color * 0.42, col, grout);
		float aa = clamp(length(fwidth(g)) * 1.2, 0.0, 1.0);
		col = mix(col, base_color * 0.82, aa);
		rough = 0.82;
	} else {
		float n = vnoise(p * 0.5) * 0.55 + vnoise(p * 2.3) * 0.3 + vnoise(p * 9.0) * 0.15;
		col = base_color * (0.78 + 0.38 * n);
		rough = 0.97;
	}
	ALBEDO = col;
	ROUGHNESS = rough;
}
