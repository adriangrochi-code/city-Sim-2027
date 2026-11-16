// Vóxeles: color por material con una leve variación por vóxel, bordes marcados,
// oclusión ambiental, sol con sombras, cielo como luz ambiente y niebla de distancia.
struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec4<f32>,
    @location(3) voxel: f32,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) ao: f32,
    @location(4) voxel: f32,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4(v.pos, 1.0);
    o.world = v.pos;
    o.color = v.color;
    o.normal = v.normal.xyz;
    o.ao = v.normal.w * 127.0 / 42.0;
    o.voxel = v.voxel;
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    return g.light_view_proj * vec4(v.pos, 1.0);
}

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3(2.2));
}

fn shadow(world: vec3<f32>, n: vec3<f32>) -> f32 {
    let lp = g.light_view_proj * vec4(world + n * 0.15, 1.0);
    let uv = vec2(lp.x * 0.5 + 0.5, 0.5 - lp.y * 0.5);
    if (uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0 || lp.z >= 1.0) {
        return 1.0;
    }
    let texel = 1.0 / 4096.0;
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            sum += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2(f32(x), f32(y)) * texel, lp.z - 0.0008);
        }
    }
    return sum / 9.0;
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let n = normalize(v.normal);
    // Variación de tono por vóxel: se ve cada bloque aunque las caras estén unidas.
    let cell = floor((v.world - n * 0.01 * v.voxel) / v.voxel);
    let tint = 0.9 + 0.16 * hash3(cell);
    var albedo = srgb_to_linear(v.color.rgb) * tint;
    // Bordes de cada vóxel, más suaves a lo lejos.
    let f = fract(v.world / v.voxel);
    let e3 = min(f, 1.0 - f) + abs(n) * 10.0;
    let edge = min(min(e3.x, e3.y), e3.z);
    let fw = length(fwidth(v.world / v.voxel));
    let line = (1.0 - smoothstep(0.0, 0.07, edge)) * (1.0 - smoothstep(0.15, 0.6, fw));
    albedo *= 1.0 - 0.18 * line;

    let ao = mix(0.42, 1.0, clamp(v.ao / 3.0, 0.0, 1.0));
    let ndl = max(dot(n, g.sun_dir.xyz), 0.0);
    let sh = shadow(v.world, n);
    let sky = mix(g.sky_horizon.rgb, g.sky_top.rgb, n.y * 0.5 + 0.5);
    let bounce = vec3(0.35, 0.30, 0.22) * max(-n.y, 0.0);
    var light = g.sun_color.rgb * ndl * sh + (sky * 0.55 + bounce * 0.2) * g.params.w * ao;
    var col = albedo * light;
    // Ventanas iluminadas por velas de noche.
    if (v.color.a > 0.5) {
        // Toda la ventana se enciende o no (celda de ~2 m × 3 m), no vóxel por vóxel.
        let lit = step(0.5, hash3(floor(v.world / vec3(2.0, 3.0, 2.0))));
        col += vec3(1.0, 0.55, 0.2) * 1.6 * g.params.y * lit;
    }
    // Niebla de distancia hacia el color del horizonte.
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x);
    col = mix(col, g.sky_horizon.rgb, clamp(fog, 0.0, 0.92));
    return vec4(col, 1.0);
}
