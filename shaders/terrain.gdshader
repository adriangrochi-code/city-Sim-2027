shader_type spatial;
render_mode cull_disabled, diffuse_burley;

// Terreno rioplatense: pastizal, arena en la costa, roca en las pendientes.
uniform sampler2D owned_mask : filter_nearest, repeat_disable;
uniform float map_size = 24000.0;
uniform float tile_size = 3000.0;
uniform float overlay = 0.0;
uniform vec2 hover_tile = vec2(-10.0, -10.0);

varying vec3 wpos;
varying vec3 wnorm;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

void vertex() {
	wpos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	wnorm = (MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz;
}

void fragment() {
	vec2 p = wpos.xz;
	float n = vnoise(p * 0.004) * 0.5 + vnoise(p * 0.03) * 0.3 + vnoise(p * 0.25) * 0.2;
	vec3 grass = mix(vec3(0.33, 0.40, 0.18), vec3(0.48, 0.50, 0.26), n);
	grass = mix(grass, vec3(0.55, 0.52, 0.30), smoothstep(0.55, 0.9, vnoise(p * 0.0015)) * 0.5);
	vec3 sand = vec3(0.70, 0.62, 0.46);
	vec3 rock = vec3(0.47, 0.45, 0.41);
	vec3 mud = vec3(0.36, 0.31, 0.23);
	float slope = 1.0 - normalize(wnorm).y;

	vec3 col = mix(sand, grass, smoothstep(0.7, 2.4, wpos.y + n * 0.8));
	col = mix(col, rock, smoothstep(0.22, 0.42, slope + n * 0.08));
	col = mix(mud, col, smoothstep(-0.8, 0.3, wpos.y));

	// Parcelas: lo que no es tuyo se ve algo más apagado.
	vec2 tuv = wpos.xz / map_size;
	float owned = 0.0;
	if (tuv.x >= 0.0 && tuv.y >= 0.0 && tuv.x < 1.0 && tuv.y < 1.0) {
		owned = texture(owned_mask, tuv).r;
	}
	float grey = dot(col, vec3(0.333));
	vec3 dim = mix(vec3(grey), col, 0.6) * 0.92;
	col = mix(dim, col, max(owned, 0.55));
	if (overlay > 0.5) {
		col = mix(dim * 0.85, col, owned);
		vec2 g = abs(fract(p / tile_size + 0.5) - 0.5) * tile_size;
		float line = 1.0 - smoothstep(6.0, 22.0, min(g.x, g.y));
		col = mix(col, vec3(0.95, 0.85, 0.6), line * 0.85);
		vec2 tile = floor(p / tile_size);
		if (tile.x == hover_tile.x && tile.y == hover_tile.y) {
			col = mix(col, vec3(1.0, 0.85, 0.45), 0.25);
		}
	}
	ALBEDO = col;
	ROUGHNESS = 0.96;
}
