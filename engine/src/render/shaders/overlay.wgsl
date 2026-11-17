// Resaltado del vóxel bajo el cursor y contorno de la estructura a colocar: cajas
// translúcidas que se ven siempre (aunque estén detrás de otra cosa).
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
    extra: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Curvatura de la Tierra (radio 6371 km): lo lejano baja d²/2R respecto de la cámara. A 10 km
// son 8 m; a 24 km, 45 m: el horizonte se curva y el río se pierde detrás de él.
fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) color: vec4<f32>) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4(curved(pos), 1.0);
    o.color = color;
    return o;
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    return v.color;
}
