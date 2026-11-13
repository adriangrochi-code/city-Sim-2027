shader_type spatial;
render_mode diffuse_burley;

// Edificios con color por vértice. Alfa 0 = ventana: de noche algunas se iluminan con
// luz de vela que titila.
uniform float night_factor = 0.0;

varying vec3 wpos;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

void vertex() {
	wpos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	float is_win = 1.0 - step(0.5, COLOR.a);
	// Textura sutil de revoque para que las paredes no se vean planas.
	float grain = hash(floor(wpos.xz * 4.0) + floor(wpos.y * 4.0));
	vec3 col = COLOR.rgb * (0.95 + 0.05 * grain);
	// Suciedad y humedad cerca del suelo.
	ALBEDO = col;
	ROUGHNESS = mix(0.9, 0.35, is_win);
	vec2 cell = floor(wpos.xz * 0.7) + floor(wpos.y * 0.4) * 13.0;
	float lit = step(0.45, hash(cell));
	float flicker = 0.8 + 0.2 * sin(TIME * (5.0 + 3.0 * hash(cell + 3.1)) + hash(cell) * 40.0);
	EMISSION = vec3(1.0, 0.58, 0.22) * 2.2 * night_factor * lit * flicker * is_win;
}
