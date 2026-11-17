// El Río de la Plata: agua color "león" que refleja el cielo y el sol.
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
    extra: vec4<f32>, // x: suavizado de escalones del terreno (1 = activo)
};
@group(0) @binding(0) var<uniform> g: Globals;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4(pos, 1.0);
    o.world = pos;
    return o;
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let t = g.params.z;
    let p = v.world.xz;
    let wave = vec2(sin(p.x * 0.05 + t * 0.9) + sin(p.y * 0.083 - t * 1.1) * 0.6,
                    cos(p.y * 0.06 + t * 0.7) + cos(p.x * 0.071 + t) * 0.6) * 0.035;
    let n = normalize(vec3(wave.x, 1.0, wave.y));
    let view = normalize(g.cam_pos.xyz - v.world);
    let fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
    let base = pow(vec3(0.43, 0.34, 0.23), vec3(2.2));
    let sky = mix(g.sky_horizon.rgb, g.sky_top.rgb, 0.3);
    let lit = base * (g.sun_color.rgb * max(g.sun_dir.y, 0.0) * 0.8 + sky * 0.5 * g.params.w);
    var col = mix(lit, sky, fres * 0.6);
    let h = normalize(g.sun_dir.xyz + view);
    col += g.sun_color.rgb * pow(max(dot(n, h), 0.0), 220.0) * 1.5;
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x);
    col = mix(col, g.sky_horizon.rgb, clamp(fog, 0.0, 0.92));
    // El río es turbio (agua color león): casi no deja ver el fondo. Así tampoco se notan
    // los escalones del fondo entre niveles de detalle.
    return vec4(col, 0.97);
}
