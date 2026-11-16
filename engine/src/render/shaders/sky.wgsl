// Cielo: degradado del horizonte al cenit, con el disco y el halo del sol.
struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    params: vec4<f32>, // x: densidad de niebla, y: noche (0..1), z: tiempo, w: luz ambiente
};
@group(0) @binding(0) var<uniform> g: Globals;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VOut {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: VOut;
    o.pos = vec4(p[i], 0.0, 1.0);
    o.ndc = p[i];
    return o;
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let far = g.inv_view_proj * vec4(v.ndc, 0.0001, 1.0);
    let dir = normalize(far.xyz / far.w - g.cam_pos.xyz);
    let up = clamp(dir.y, 0.0, 1.0);
    var col = mix(g.sky_horizon.rgb, g.sky_top.rgb, pow(up, 0.55));
    if (dir.y < 0.0) {
        col = g.sky_horizon.rgb * 0.9;
    }
    let s = max(dot(dir, g.sun_dir.xyz), 0.0);
    col += g.sun_color.rgb * (pow(s, 600.0) * 6.0 + pow(s, 12.0) * 0.25) * (1.0 - g.params.y);
    return vec4(col, 1.0);
}
