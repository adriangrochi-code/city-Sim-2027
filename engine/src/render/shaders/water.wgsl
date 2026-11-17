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

// Curvatura de la Tierra (radio 6371 km): lo lejano baja d²/2R respecto de la cámara. A 10 km
// son 8 m; a 24 km, 45 m: el horizonte se curva y el río se pierde detrás de él.
fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4(curved(pos), 1.0);
    o.world = pos;
    return o;
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// Agua en vóxeles: la superficie se divide en celdas cuadradas del tamaño del vóxel del
// terreno a esa distancia (0,5 m de cerca, más grandes a lo lejos, como los niveles de
// detalle). Cada celda tiene su tinte y su propia ondulación (se inclina y sube un poco), con
// el borde marcado de cerca: el río se lee hecho de bloques, como las casas. La luz del sol se
// refleja por celda, así que a lo lejos no forma el tramado brillante de las olas finas.
fn cell_shade(p: vec2<f32>, size: f32, t: f32) -> vec4<f32> {
    let c = floor(p / size);
    let h = hash2(c);
    let h2 = hash2(c + vec2(17.0, 31.0));
    // Oleaje: un tren de olas grande (que cruza las celdas) más el titileo propio de cada una.
    let ctr = (c + 0.5) * size;
    let swell = sin(ctr.x * 0.045 + ctr.y * 0.02 + t * 0.8) * 0.5 + sin(ctr.y * 0.07 - t * 1.1 + h * 6.28) * 0.5;
    let tilt = vec2(cos(t * (0.6 + h) + h2 * 6.28), sin(t * (0.5 + h2) + h * 6.28)) * 0.06 + vec2(swell * 0.05, 0.0);
    let f = fract(p / size);
    let e = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
    return vec4(tilt, h, e);
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let t = g.params.z;
    let p = v.world.xz;
    // Metros por píxel: elige el tamaño de celda (≥ ~6 píxeles), mezclando dos tamaños para que
    // no se vean saltos.
    let mpp = max(length(fwidth(p)), 1e-4);
    let lv = max(log2(mpp * 6.0 / 0.5), 0.0);
    let k = floor(lv);
    let a = cell_shade(p, 0.5 * exp2(k), t);
    let b = cell_shade(p, 0.5 * exp2(k + 1.0), t);
    let w = smoothstep(0.6, 1.0, lv - k);
    let cs = mix(a, b, w);
    let n = normalize(vec3(cs.x, 1.0, cs.y));
    let view = normalize(g.cam_pos.xyz - v.world);
    let fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
    let base = pow(vec3(0.43, 0.34, 0.23), vec3(2.2)) * (0.92 + 0.16 * cs.z);
    let sky = mix(g.sky_horizon.rgb, g.sky_top.rgb, 0.3);
    let lit = base * (g.sun_color.rgb * max(g.sun_dir.y, 0.0) * 0.8 + sky * 0.5 * g.params.w);
    var col = mix(lit, sky, fres * 0.6);
    let hv = normalize(g.sun_dir.xyz + view);
    // Brillo por celda: una celda entera destella (vóxel), no un punto de ola fina.
    // Solo destellan algunas celdas (las que justo apuntan al sol), y menos a lo lejos.
    let glint = pow(max(dot(n, hv), 0.0), 160.0) * step(0.55, cs.z);
    col += g.sun_color.rgb * glint * 0.45 / (1.0 + lv * 0.5);
    // Borde de cada celda, como el de los vóxeles del terreno; se apaga a lo lejos.
    let edge = (1.0 - smoothstep(0.0, 0.06, cs.w)) * (1.0 - smoothstep(0.3, 1.0, w + lv * 0.25));
    col *= 1.0 - 0.12 * edge;
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x);
    col = mix(col, g.sky_horizon.rgb, clamp(fog, 0.0, 0.88));
    // El río es turbio (agua color león): casi no deja ver el fondo. Así tampoco se notan
    // los escalones del fondo entre niveles de detalle.
    return vec4(col, 0.97);
}
