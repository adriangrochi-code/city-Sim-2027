// Vecinos, vehículos, animales y modelos de detalle (ver detalle::Caja): una instancia por
// caja; el vertex shader arma sus 6 caras con `vertex_index`, la inclina alrededor de x por el
// pivote de su parte (piernas, cabeza, ruedas) y la gira alrededor de y. Las de micro-vóxeles
// (color.a > 0) llevan el matiz y el borde de cada vóxel de 1/16 m. Luz, sombra recibida,
// niebla, curvatura y curva tonal como la flora baja. Las instancias de alto 0 son sombras de
// contacto (vs_sombra / fs_sombra, mezcla que oscurece).
struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    params: vec4<f32>, // x: niebla, y: noche, z: tiempo, w: luz ambiente
    extra: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
// Mapa de sombras en cascadas y `shadow()`: sombras.wgsl (se antepone a este archivo).

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) giro: f32,
    @location(2) desde: vec3<f32>,
    @location(3) inclina: f32,
    @location(4) tam: vec3<f32>,
    @location(5) color: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
    // Coordenadas en micro-vóxeles desde la esquina de la caja (xyz) y semilla (w; 0 = lisa).
    @location(3) celda: vec4<f32>,
};

fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

const CORNERS = array<vec3<f32>, 24>(
    vec3(1.0, 0.0, 0.0), vec3(1.0, 1.0, 0.0), vec3(1.0, 1.0, 1.0), vec3(1.0, 0.0, 1.0), // +x
    vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 1.0), vec3(0.0, 1.0, 0.0), // -x
    vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 1.0), vec3(1.0, 1.0, 1.0), vec3(1.0, 1.0, 0.0), // +y
    vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 1.0), vec3(1.0, 1.0, 1.0), vec3(0.0, 1.0, 1.0), // +z
    vec3(0.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(1.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), // -z
    vec3(0.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 1.0), vec3(0.0, 0.0, 1.0), // -y
);
const NORMALS = array<vec3<f32>, 6>(vec3(1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, -1.0), vec3(0.0, -1.0, 0.0));
const TRI = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
// Lado del micro-vóxel de los modelos de detalle (detalle::VOX).
const VOX: f32 = 0.0625;

// Gira alrededor de y: (0, 0, 1) va a (sen, 0, cos).
fn girar(v: vec3<f32>, s: f32, c: f32) -> vec3<f32> {
    return vec3(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

// Gira alrededor de x (inclinación de una parte: piernas, cabeza, ruedas).
fn inclinar(v: vec3<f32>, s: f32, c: f32) -> vec3<f32> {
    return vec3(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
}

@vertex
fn vs_main(v: VIn, @builtin(vertex_index) vi: u32) -> VOut {
    var o: VOut;
    // Las sombras de contacto (alto 0) las dibuja vs_sombra.
    if (v.tam.y <= 0.0) {
        o.clip = vec4(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    let face = vi / 6u;
    var corners = CORNERS;
    var tri = TRI;
    var normals = NORMALS;
    let c = corners[face * 4u + tri[vi % 6u]];
    let local = vec3((c.x - 0.5) * v.tam.x, c.y * v.tam.y, (c.z - 0.5) * v.tam.z) + v.desde;
    let si = sin(v.inclina);
    let ci = cos(v.inclina);
    let s = sin(v.giro);
    let co = cos(v.giro);
    let p = v.pos + girar(inclinar(local, si, ci), s, co);
    o.clip = g.view_proj * vec4(curved(p), 1.0);
    o.world = p;
    o.color = pow(v.color.rgb, vec3(2.2));
    o.normal = girar(inclinar(normals[face], si, ci), s, co);
    o.celda = vec4(c * v.tam / VOX, round(v.color.a * 255.0));
    return o;
}

// Azar entero (como voxel.wgsl).
fn hash_u(x: u32) -> u32 {
    var h = x;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return h;
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let n = normalize(v.normal);
    var base = v.color;
    let dcam = distance(v.world, g.cam_pos.xyz);
    if (v.celda.w > 0.5) {
        // Micro-vóxeles: cada vóxel con su matiz (±5 %) y un borde apenas más oscuro, que se
        // apagan con la distancia (para que no titilen).
        let ln = abs(n);
        let q = v.celda.xyz;
        let k = vec3<i32>(floor(q - 0.01 * sign(q - floor(q) - 0.5)));
        let h = hash_u(bitcast<u32>(k.x) ^ hash_u(bitcast<u32>(k.y) ^ hash_u(bitcast<u32>(k.z) + u32(v.celda.w) * 0x9e3779b9u)));
        let mat = (f32(h & 0xffffu) / 65535.0 - 0.5) * 0.1;
        let f = fract(q);
        let e = min(min(f, 1.0 - f), vec3(1.0));
        // El borde en las dos direcciones de la cara (no en la de su normal).
        let ee = select(e, vec3(1.0), ln > vec3(0.5));
        let borde = 1.0 - 0.07 * (1.0 - smoothstep(0.0, 0.12, min(min(ee.x, ee.y), ee.z)));
        let fade = 1.0 - smoothstep(25.0, 70.0, dcam);
        base = base * mix(1.0, (1.0 + mat) * borde, fade);
    }
    let ndl = max(dot(n, g.sun_dir.xyz), 0.0);
    var sh = 1.0;
    if (ndl > 0.0) {
        sh = shadow(v.world, n);
    }
    // Luz del cielo en la sombra, algo desaturada: si no, al atardecer las sombras se ponían
    // violetas y azules.
    let sky_c = mix(g.sky_horizon.rgb, g.sky_top.rgb, n.y * 0.5 + 0.5);
    let sky = mix(vec3(dot(sky_c, vec3(0.299, 0.587, 0.114))), sky_c, 0.55);
    let moon = vec3(0.45, 0.5, 0.65) * 0.14 * g.extra.z * g.params.y;
    let light = g.sun_color.rgb * ndl * sh + sky * 0.55 * g.params.w + moon;
    var col = base * light;
    let d = distance(v.world, g.cam_pos.xyz);
    // [render] Niebla por altura y dispersión ancha del sol, como en voxel.wgsl.
    let fog = 1.0 - exp(-d * g.params.x * niebla_altura(g.cam_pos.y, v.world.y));
    let vdir = normalize(v.world - g.cam_pos.xyz);
    let cs = max(dot(vdir, g.sun_dir.xyz), 0.0);
    let s = (pow(cs, 6.0) * 0.6 + pow(cs, 2.0) * 0.12 * (0.6 + 0.4 * (1.0 - max(g.sun_dir.y, 0.0)))) * (1.0 - g.params.y);
    let fog_c = mix(g.sky_horizon.rgb, g.sky_horizon.rgb * 0.55 + g.sun_color.rgb * 0.45, s);
    col = mix(col, fog_c, clamp(fog, 0.0, 0.88));
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

// --- Sombras de contacto ---------------------------------------------------------------------
struct SOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fuerza: f32,
};

@vertex
fn vs_sombra(v: VIn, @builtin(vertex_index) vi: u32) -> SOut {
    var o: SOut;
    if (v.tam.y > 0.0) {
        o.clip = vec4(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    var tri = TRI;
    let k = tri[vi % 6u];
    let uv = vec2(select(-1.0, 1.0, k == 1u || k == 2u), select(-1.0, 1.0, k >= 2u));
    // Un poco más grande y corrida hacia donde no da el sol.
    let local = vec3(uv.x * v.tam.x * 1.25, 0.0, uv.y * v.tam.z * 1.15);
    let p0 = v.pos + girar(local, sin(v.giro), cos(v.giro));
    let sol = g.sun_dir.xyz;
    let corr = -sol.xz / max(sol.y, 0.35) * 0.25;
    let p = vec3(p0.x + corr.x, p0.y, p0.z + corr.y);
    o.clip = g.view_proj * vec4(curved(p), 1.0);
    o.uv = uv;
    // Más tenue de noche y con el sol bajo.
    o.fuerza = v.color.a * (1.0 - g.params.y * 0.7) * smoothstep(0.0, 0.25, sol.y + 0.1);
    return o;
}

@fragment
fn fs_sombra(v: SOut) -> @location(0) vec4<f32> {
    let r = dot(v.uv, v.uv);
    let a = (1.0 - smoothstep(0.25, 1.0, r)) * v.fuerza * 0.6;
    return vec4(0.0, 0.0, 0.0, a);
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
