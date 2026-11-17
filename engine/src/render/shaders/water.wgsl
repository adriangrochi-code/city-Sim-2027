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
    // Corriente (m/s): cero en el río, la del cauce en los arroyos.
    @location(1) flow: vec2<f32>,
};

// Curvatura de la Tierra (radio 6371 km): lo lejano baja d²/2R respecto de la cámara. A 10 km
// son 8 m; a 24 km, 45 m: el horizonte se curva y el río se pierde detrás de él.
fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) flow: vec2<f32>) -> VOut {
    var o: VOut;
    o.flow = flow;
    // El plano del río sube y baja con la marea y los temporales (extra.y); los arroyos no.
    let is_river = dot(flow, flow) < 1e-6;
    var p = vec3(pos.x, pos.y + select(0.0, g.extra.y, is_river), pos.z);
    // [entorno amplio] El plano (400 km, centrado en el mapa) sigue al centro de los anillos
    // de detalle, de a baldosas enteras de 2 km: siempre llega más allá del terreno dibujado.
    if (is_river && g.sun_color.w > 0.0) {
        p.x += round((g.sky_top.w - 12000.0) / 2000.0) * 2000.0;
        p.z += round((g.sky_horizon.w - 12000.0) / 2000.0) * 2000.0;
    }
    o.clip = g.view_proj * vec4(curved(p), 1.0);
    o.world = p;
    return o;
}

// [entorno amplio] Fin del terreno dibujado: el círculo de radio sun_color.w alrededor del
// centro de los anillos de detalle (sky_top.w, sky_horizon.w). Desde el 60 % de ese radio
// todo se funde con la niebla, así el último anillo (y el agua más allá) no se ve como un
// borde recto. 0 = sin límite.
fn reach_fog(world: vec3<f32>) -> f32 {
    let r = g.sun_color.w;
    if (r <= 0.0) {
        return 0.0;
    }
    let d = distance(world.xz, vec2(g.sky_top.w, g.sky_horizon.w));
    return smoothstep(r * 0.6, r * 0.97, d);
}

// Azar con aritmética entera (no con sin(), que en las placas NVIDIA pierde precisión con
// coordenadas de decenas de miles de metros y dibuja rayas en abanico en lugar de ruido).
fn hash_u(x: u32) -> u32 {
    var h = x;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return h;
}

fn hash2(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let h = hash_u(bitcast<u32>(i.x) ^ hash_u(bitcast<u32>(i.y) + 0x9e3779b9u));
    return f32(h & 0xffffffu) / 16777215.0;
}

// Agua en vóxeles: la superficie se divide en celdas cuadradas del tamaño del vóxel del
// terreno a esa distancia (0,25 m de cerca, más grandes a lo lejos, como los niveles de
// detalle). Cada celda tiene su tinte y su propia ondulación (se inclina y sube un poco), con
// el borde marcado de cerca: el río se lee hecho de bloques, como las casas. La luz del sol se
// refleja por celda, así que a lo lejos no forma el tramado brillante de las olas finas.
fn cell_shade(p: vec2<f32>, size: f32, t: f32) -> vec4<f32> {
    let c = floor(p / size);
    let h = hash2(c);
    let h2 = hash2(c + vec2(17.0, 31.0));
    // Oleaje: un tren de olas grande (que cruza las celdas) más el titileo propio de cada una.
    let ctr = (c + 0.5) * size;
    // Viento (extra.w = grados·100 + m/s): las olas avanzan hacia donde sopla y crecen con él.
    let wdeg = floor(g.extra.w / 100.0);
    let wspd = g.extra.w - wdeg * 100.0;
    let wa = radians(wdeg);
    let to = -vec2(sin(wa), -cos(wa)); // hacia dónde va (x este, z sur)
    let amp = clamp(wspd / 8.0, 0.15, 2.0);
    let along = dot(ctr, to);
    // Fases reducidas a una vuelta antes del seno (precisión en coordenadas grandes).
    let f1 = fract(along * 0.06 / 6.2832 - t * (0.6 + wspd * 0.08) / 6.2832);
    let f2 = fract(ctr.y * 0.07 / 6.2832 - t * 1.1 / 6.2832 + h);
    let swell = sin(f1 * 6.2832) * 0.7 + sin(f2 * 6.2832) * 0.3;
    let tilt = vec2(cos(t * (0.6 + h) + h2 * 6.28), sin(t * (0.5 + h2) + h * 6.28)) * 0.04 * amp + to * swell * 0.05 * amp;
    let f = fract(p / size);
    let e = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
    return vec4(tilt, h, e);
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let t = g.params.z;
    // En los arroyos el dibujo de las celdas viaja con la corriente: el agua se ve correr.
    let flowing = dot(v.flow, v.flow) > 1e-6;
    let p = select(v.world.xz, v.world.xz - v.flow * t, flowing);
    // Metros por píxel: elige el tamaño de celda (≥ ~6 píxeles), mezclando dos tamaños para que
    // no se vean saltos.
    let mpp = max(length(fwidth(p)), 1e-4);
    let lv = max(log2(mpp * 6.0 / 0.25), 0.0);
    let k = floor(lv);
    let a = cell_shade(p, 0.25 * exp2(k), t);
    let b = cell_shade(p, 0.25 * exp2(k + 1.0), t);
    let w = smoothstep(0.6, 1.0, lv - k);
    let cs = mix(a, b, w);
    let n = normalize(vec3(cs.x, 1.0, cs.y));
    let view = normalize(g.cam_pos.xyz - v.world);
    let fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
    // El río es color león; el agua de los arroyos, más clara y verdosa (agua dulce y baja).
    let tint = select(vec3(0.43, 0.34, 0.23), vec3(0.33, 0.38, 0.32), flowing);
    let base = pow(tint, vec3(2.2)) * (0.92 + 0.16 * cs.z);
    let sky = mix(g.sky_horizon.rgb, g.sky_top.rgb, 0.3);
    let moon = vec3(0.45, 0.5, 0.65) * 0.12 * g.extra.z * g.params.y;
    let lit = base * (g.sun_color.rgb * max(g.sun_dir.y, 0.0) * 0.8 + sky * 0.5 * g.params.w + moon);
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
    let vdir_f = normalize(v.world - g.cam_pos.xyz);
    let sun_f = pow(max(dot(vdir_f, g.sun_dir.xyz), 0.0), 6.0) * (1.0 - g.params.y);
    let fog_c = mix(g.sky_horizon.rgb, g.sky_horizon.rgb * 0.55 + g.sun_color.rgb * 0.45, sun_f * 0.6);
    col = mix(col, fog_c, max(clamp(fog, 0.0, 0.88), reach_fog(v.world)));
    // El río es turbio (agua color león): casi no deja ver el fondo. Así tampoco se notan
    // los escalones del fondo entre niveles de detalle.
    return vec4(col, select(0.97, 0.88, flowing));
}
