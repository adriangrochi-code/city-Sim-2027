shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_disabled, diffuse_burley, specular_schlick_ggx;

// El Río de la Plata: agua color "león", marrón dorado, con oleaje suave.
uniform vec3 water_color : source_color = vec3(0.40, 0.31, 0.20);
uniform vec3 far_color : source_color = vec3(0.36, 0.30, 0.22);
uniform float specular_amount = 0.55;
// Arroyos: acerca el agua a la cámara (sin moverla en pantalla) para que el terreno lejano,
// más grueso que el cauce, no la tape. 0 para el río.
uniform float view_bias = 0.0;


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
	if (view_bias > 0.0) {
		vec3 to_cam = CAMERA_POSITION_WORLD - wpos;
		float d = length(to_cam);
		vec3 moved = wpos + to_cam / max(d, 0.001) * min(d * view_bias, 40.0);
		VERTEX = (inverse(MODEL_MATRIX) * vec4(moved, 1.0)).xyz;
	}
}

void fragment() {
	vec2 p = wpos.xz;
	float t = TIME * 0.6;
	float h1 = vnoise(p * 0.05 + vec2(t, t * 0.4));
	float h2 = vnoise(p * 0.013 - vec2(t * 0.2, t * 0.3));
	float hx = vnoise(p * 0.05 + vec2(t + 0.15, t * 0.4)) - h1;
	float hz = vnoise(p * 0.05 + vec2(t, t * 0.4 + 0.15)) - h1;
	NORMAL_MAP = normalize(vec3(hx * 1.2, hz * 1.2, 1.0)) * 0.5 + 0.5;
	NORMAL_MAP_DEPTH = 0.35;
	// Las ondas finas se desvanecen a lo lejos para evitar el "ruido".
	float dist_fade = clamp(length(VERTEX) / 1500.0, 0.0, 1.0);
	NORMAL_MAP = mix(NORMAL_MAP, vec3(0.5, 0.5, 1.0), dist_fade);
	float far = clamp(length(wpos.xz - vec2(12000.0, 12000.0)) / 30000.0, 0.0, 1.0);
	ALBEDO = mix(water_color, far_color, far) * (0.92 + 0.08 * h2);
	ROUGHNESS = 0.12;
	SPECULAR = specular_amount;
	ALPHA = 0.9;
}
