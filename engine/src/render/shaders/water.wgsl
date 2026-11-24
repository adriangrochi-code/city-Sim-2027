// El Río de la Plata y los arroyos: agua color "león" que refleja el cielo y el sol.
//
// [agua en vóxeles] El agua son caras de vóxeles (ver `agua.rs`): las del río suben y bajan
// con la marea (este shader las pone al nivel del agua, redondeado a la grilla de 0,25 m, y
// las esconde donde el suelo queda arriba); las de los arroyos tienen su nivel fijo y corren.
// El plano enorme de antes queda solo como respaldo más allá del último anillo de detalle.
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
    extra: vec4<f32>, // x: suavizado de escalones del terreno (1 = activo), y: nivel del río
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(3) var<storage, read> origins: array<vec4<f32>>;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Corriente (m/s): cero en el río, la del cauce en los arroyos.
    @location(1) flow: vec2<f32>,
    // x: fondo bajo el agua (m), y: vóxel (m), z: tipo (0 plano, 1 río, 2 arroyo), w: cara.
    @location(2) @interpolate(flat) info: vec4<f32>,
};

// Curvatura de la Tierra (radio 6371 km): lo lejano baja d²/2R respecto de la cámara. A 10 km
// son 8 m; a 24 km, 45 m: el horizonte se curva y el río se pierde detrás de él.
fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

// Nivel del río redondeado a la grilla de 0,25 m (igual que `agua::nivel_en_grilla`).
fn nivel_rio() -> f32 {
    return floor(g.extra.y / 0.25 + 0.5) * 0.25;
}

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) flow: vec2<f32>) -> VOut {
    var o: VOut;
    o.flow = flow;
    var p = vec3(pos.x, pos.y + nivel_rio(), pos.z);
    // [entorno amplio] El plano (400 km, centrado en el mapa) sigue al centro de los anillos
    // de detalle, de a baldosas enteras de 2 km: siempre llega más allá del terreno dibujado.
    if (g.sun_color.w > 0.0) {
        p.x += round((g.sky_top.w - 12000.0) / 2000.0) * 2000.0;
        p.z += round((g.sky_horizon.w - 12000.0) / 2000.0) * 2000.0;
    }
    o.clip = g.view_proj * vec4(curved(p), 1.0);
    o.world = p;
    o.info = vec4(-1000.0, 0.25, 0.0, 0.0);
    return o;
}

// [agua en vóxeles] Caras del agua (mismo formato que las del terreno, ver mesher::Quad):
// campo de material = tipo de agua; `suave.x` = corriente (mm/s, x y z de 16 bits con signo);
// `suave.y` = fondo (m, en bits de f32).
struct VIn {
    @location(0) qa: u32,
    @location(1) qb: u32,
    @location(2) suave: vec2<u32>,
    @location(3) slot: u32,
    @builtin(vertex_index) vi: u32,
};

fn dir_axis(d: u32) -> u32 {
    return select(select(2u, 0u, d == 1u || d == 2u), 1u, d == 0u || d == 5u);
}
fn unit(ax: u32) -> vec3<u32> {
    return vec3(select(0u, 1u, ax == 0u), select(0u, 1u, ax == 1u), select(0u, 1u, ax == 2u));
}
fn s16(v: u32) -> f32 {
    return f32(bitcast<i32>(v << 16u) >> 16u);
}

@vertex
fn vs_quad(v: VIn) -> VOut {
    var o: VOut;
    let dir = (v.qa >> 23u) & 7u;
    let base = vec3<u32>(v.qa & 63u, (v.qa >> 12u) & 2047u, (v.qa >> 6u) & 63u);
    let w = (v.qb & 31u) + 1u;
    let h = ((v.qb >> 5u) & 31u) + 1u;
    let tipo = (v.qb >> 18u) & 255u;
    var t = v.vi & 3u;
    let positive = dir == 0u || dir == 1u || dir == 3u;
    t = select((4u - t) & 3u, t, positive);
    let du = select(0u, w, t == 1u || t == 2u);
    let dv = select(0u, h, t == 2u || t == 3u);
    let ax = dir_axis(dir);
    let org = origins[v.slot];
    var p = org.xyz + vec3<f32>(base + unit((ax + 1u) % 3u) * du + unit((ax + 2u) % 3u) * dv) * org.w;
    let fondo = bitcast<f32>(v.suave.y);
    o.flow = vec2(s16(v.suave.x), s16(v.suave.x >> 16u)) / 1000.0;
    if (tipo == 1u) {
        // Río: la cara de arriba va al nivel del agua de este momento. En los niveles más
        // gruesos (vóxel de 64 m o más, malla bajada) no pasa de 2 m: la tierra firme está a 3 m.
        var lvl = nivel_rio();
        if (org.w >= 63.0) {
            lvl = min(lvl, 2.0);
        }
        if (lvl <= fondo + 0.001) {
            // El suelo de estas columnas queda sobre el agua: la cara no se dibuja.
            o.clip = vec4(2.0, 2.0, 2.0, 1.0);
            o.world = p;
            o.info = vec4(fondo, org.w, 1.0, 0.0);
            return o;
        }
        p.y = lvl;
    }
    o.clip = g.view_proj * vec4(curved(p), 1.0);
    o.world = p;
    o.info = vec4(fondo, org.w, f32(tipo), f32(dir));
    return o;
}

// [render] Niebla de lo que falta (técnicas del mod LOD §2.12): por sector alrededor del centro
// de los anillos, la distancia a la primera zona sin nada dibujado (`Streamer::sectores_faltan`).
// Se funde desde un poco antes de esa distancia; entre dos sectores manda el más cercano.
@group(0) @binding(8) var<storage, read> faltan: array<f32, 64>;
fn hole_fog(world: vec3<f32>) -> f32 {
    let d = world.xz - vec2(g.sky_top.w, g.sky_horizon.w);
    let f = (atan2(d.y, d.x) / 6.2831853 + 0.5) * 64.0 - 0.5;
    let i0 = i32(floor(f));
    let a = faltan[u32(((i0 % 64) + 64) % 64)];
    let b = faltan[u32((((i0 + 1) % 64) + 64) % 64)];
    let lim = min(a, b);
    if (lim > 1.0e8) {
        return 0.0;
    }
    return smoothstep(lim - max(40.0, lim * 0.2), lim, length(d));
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

// Agua en vóxeles: la superficie se divide en celdas cuadradas del tamaño del vóxel (0,25 m de
// cerca, más grandes a lo lejos, como los niveles de detalle). Cada celda tiene su tinte y su
// propio brillo que sube y baja (la ola es de luz, no de geometría: la cara sigue plana), con
// el borde marcado de cerca: el agua se lee hecha de bloques, como el terreno. La luz del sol
// se refleja por celda, así que a lo lejos no forma el tramado brillante de las olas finas.
// Devuelve (inclinación x, z; tinte; distancia al borde; brillo de la ola).
struct Celda {
    tilt: vec2<f32>,
    h: f32,
    e: f32,
    ola: f32,
};

fn cell_shade(p: vec2<f32>, size: f32, t: f32) -> Celda {
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
    // Brillo de la ola en la celda: la cresta del tren de olas y un titileo lento propio.
    let ola = swell * 0.05 * min(amp, 1.2) + sin(t * (0.8 + h2) + h * 6.28) * 0.025;
    return Celda(tilt, h, e, ola);
}

// [render] El plano de respaldo (pipeline sin la niebla de lo que falta: está más allá del
// último anillo) y las caras del agua de las columnas.
@fragment
fn fs_plano(v: VOut) -> @location(0) vec4<f32> {
    return shade(v, 0.0);
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    return shade(v, hole_fog(v.world));
}

fn shade(v: VOut, hole: f32) -> vec4<f32> {
    let tipo = u32(v.info.z + 0.5);
    // El plano de respaldo solo se ve más allá del último anillo de detalle (ahí ya es niebla).
    if (tipo == 0u && g.sun_color.w > 0.0 && distance(v.world.xz, vec2(g.sky_top.w, g.sky_horizon.w)) < g.sun_color.w * 0.97) {
        discard;
    }
    let t = g.params.z;
    let dir = u32(v.info.w + 0.5);
    let side = dir != 0u;
    // En los arroyos el dibujo de las celdas viaja con la corriente: el agua se ve correr. En
    // los costados (cascadas) cae.
    let flowing = dot(v.flow, v.flow) > 1e-6;
    var p = select(v.world.xz, v.world.xz - v.flow * t, flowing);
    if (side) {
        let a = select(v.world.x, v.world.z, dir == 1u || dir == 2u);
        p = vec2(a, v.world.y + t * 1.2);
    }
    // Tamaño de celda: el vóxel de la columna, o más grande si mediría menos de ~6 píxeles
    // (mezclando dos tamaños para que no se vean saltos).
    let vox = max(v.info.y, 0.25);
    let mpp = max(length(fwidth(p)), 1e-4);
    let lv = max(log2(mpp * 6.0 / vox), 0.0);
    let k = floor(lv);
    let a = cell_shade(p, vox * exp2(k), t);
    let b = cell_shade(p, vox * exp2(k + 1.0), t);
    let w = smoothstep(0.6, 1.0, lv - k);
    let tilt = mix(a.tilt, b.tilt, w);
    let ch = mix(a.h, b.h, w);
    let ce = mix(a.e, b.e, w);
    let ola = mix(a.ola, b.ola, w);
    var n = normalize(vec3(tilt.x, 1.0, tilt.y));
    if (side) {
        let s = select(-1.0, 1.0, dir == 1u || dir == 3u);
        n = select(vec3(0.0, 0.0, s), vec3(s, 0.0, 0.0), dir == 1u || dir == 2u);
    }
    let view = normalize(g.cam_pos.xyz - v.world);
    let fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
    // El río es color león; el agua de los arroyos, más clara y verdosa (agua dulce y baja).
    let dulce = tipo == 2u;
    let tint = select(vec3(0.43, 0.34, 0.23), vec3(0.33, 0.38, 0.32), dulce);
    let base = pow(tint, vec3(2.2)) * (0.92 + 0.16 * ch) * (1.0 + ola);
    let sky = mix(g.sky_horizon.rgb, g.sky_top.rgb, 0.3);
    let moon = vec3(0.45, 0.5, 0.65) * 0.12 * g.extra.z * g.params.y;
    let lit = base * (g.sun_color.rgb * max(g.sun_dir.y, 0.0) * 0.8 + sky * 0.5 * g.params.w + moon);
    var col = mix(lit, sky, fres * select(0.6, 0.3, side));
    let hv = normalize(g.sun_dir.xyz + view);
    // Brillo por celda: una celda entera destella (vóxel), no un punto de ola fina.
    // Solo destellan algunas celdas (las que justo apuntan al sol), y menos a lo lejos.
    let glint = pow(max(dot(n, hv), 0.0), 160.0) * step(0.55, ch) * select(1.0, 0.0, side);
    col += g.sun_color.rgb * glint * 0.45 / (1.0 + lv * 0.5);
    // Espuma clara en las cascadas.
    if (side) {
        col = mix(col, vec3(0.75, 0.75, 0.7) * (g.sun_color.rgb * 0.5 + sky * 0.5), 0.25 + 0.2 * ch);
    }
    // Borde de cada celda, como el de los vóxeles del terreno; se apaga a lo lejos.
    let edge = (1.0 - smoothstep(0.0, 0.06, ce)) * (1.0 - smoothstep(0.3, 1.0, w + lv * 0.25));
    col *= 1.0 - 0.12 * edge;
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x * niebla_altura(g.cam_pos.y, v.world.y));
    let vdir_f = normalize(v.world - g.cam_pos.xyz);
    let cs = max(dot(vdir_f, g.sun_dir.xyz), 0.0);
    // [render] Igual que `fog_color` en voxel.wgsl (halo cercano y dispersión ancha).
    let sun_f = (pow(cs, 6.0) * 0.6 + pow(cs, 2.0) * 0.12 * (0.6 + 0.4 * (1.0 - max(g.sun_dir.y, 0.0)))) * (1.0 - g.params.y);
    let fog_c = mix(g.sky_horizon.rgb, g.sky_horizon.rgb * 0.55 + g.sun_color.rgb * 0.45, sun_f);
    let fogk = max(max(clamp(fog, 0.0, 0.88), reach_fog(v.world)), hole);
    col = mix(col, fog_c, fogk);
    // El río es turbio (agua color león): casi no deja ver el fondo, salvo en la orilla, donde
    // el agua es baja y se ve la arena (la profundidad sale del fondo de cada columna).
    let hondo = v.world.y - v.info.x;
    var alpha = select(0.97, 0.88, dulce);
    alpha *= mix(select(0.5, 0.45, dulce), 1.0, smoothstep(0.0, select(1.2, 0.8, dulce), hondo));
    // Lejos (niebla) el agua es opaca: no se nota el escalón del fondo entre niveles.
    alpha = mix(alpha, 1.0, fogk);
    return vec4(curva_tonal(col), alpha);
}

// [render] Niebla más densa abajo (el aire húmedo de la costa y los bañados) y más rala en los
// cerros: densidad proporcional a exp(−y/H) promediada a lo largo del rayo, relativa a la de
// 120 m de altura y suavizada (entre ×0,8 y ×1,2 de la de antes).
fn niebla_altura(ya: f32, yb: f32) -> f32 {
    let hh = 450.0;
    let a = max(ya, 0.0);
    let b = max(yb, 0.0);
    let dy = b - a;
    var m = exp(-a / hh);
    if (abs(dy) > 1.0) {
        m = hh * (exp(-a / hh) - exp(-b / hh)) / dy;
    }
    return clamp(mix(1.0, m / 0.766, 0.5), 0.8, 1.2);
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
