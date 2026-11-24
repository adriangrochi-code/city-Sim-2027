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
    extra: vec4<f32>, // x: suavizado de escalones del terreno (1 = activo)
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
    return vec4(curva_tonal(col), 1.0);
}

// Curva tonal suave: hasta 0,75 de luminancia, un contraste leve en los medios ([render]: las
// sombras ~5 % más hondas, los medios ~3 % más claros; continua en 0 y en 0,75); más arriba
// comprime de a poco (sin cambiar el tono), así los claros al sol no se queman en amarillo
// saturado. La misma curva en el cielo, el agua, la flora y los agentes.
fn curva_tonal(c: vec3<f32>) -> vec3<f32> {
    let l = dot(c, vec3(0.2126, 0.7152, 0.0722));
    if (l <= 0.0) {
        return c;
    }
    if (l <= 0.75) {
        let l2 = l + 0.6 * l * (0.75 - l) * (l - 0.3);
        return c * (l2 / l);
    }
    let x = l - 0.75;
    let l2 = 0.75 + x / (1.0 + x * 2.5);
    return c * (l2 / l);
}
