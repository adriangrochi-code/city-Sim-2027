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
    extra: vec4<f32>, // x: suavizado de escalones del terreno (1 = activo)
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

// Cara voraz compacta (20 bytes, ver mesher::Quad), una instancia por cara. El vertex shader
// arma sus 2 triángulos con `vertex_index` (*vertex pulling*, idea de Tisma engine:
// preview.vert arma el prisma de cada extrusión a partir de una instancia mínima).
// El origen de cada columna (xyz en metros, w = tamaño del vóxel) está en un búfer de
// almacenamiento indexado por `slot`, y el color del material en una paleta uniforme.
struct VIn {
    @location(0) qa: u32,
    @location(1) qb: u32,
    @location(2) smooth: vec2<u32>,
    @location(3) slot: u32,
    @builtin(vertex_index) vi: u32,
};
@group(0) @binding(3) var<storage, read> origins: array<vec4<f32>>;

const DIRS = array<vec3<f32>, 6>(
    vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0),
    vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, -1.0), vec3(0.0, -1.0, 0.0),
);
// Ejes u y v de cada dirección (u = eje + 1, v = eje + 2, módulo 3).
const UAX = array<vec3<u32>, 6>(
    vec3(0u, 0u, 1u), vec3(0u, 1u, 0u), vec3(0u, 1u, 0u),
    vec3(1u, 0u, 0u), vec3(1u, 0u, 0u), vec3(0u, 0u, 1u),
);
const VAX = array<vec3<u32>, 6>(
    vec3(1u, 0u, 0u), vec3(0u, 0u, 1u), vec3(0u, 0u, 1u),
    vec3(0u, 1u, 0u), vec3(0u, 1u, 0u), vec3(1u, 0u, 0u),
);
const POSITIVE = array<bool, 6>(true, true, false, true, false, false);
const TRIS = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
const TRIS_FLIP = array<u32, 6>(1u, 2u, 3u, 1u, 3u, 0u);
const NEG = array<u32, 4>(0u, 3u, 2u, 1u);

struct Corner {
    pos: vec3<f32>,
    dir: u32,
    k: u32,
    ao: u32,
    mat: u32,
};

fn corner(v: VIn) -> Corner {
    var c: Corner;
    c.dir = (v.qa >> 23u) & 7u;
    let base = vec3<u32>(v.qa & 63u, (v.qa >> 12u) & 2047u, (v.qa >> 6u) & 63u);
    let w = (v.qb & 31u) + 1u;
    let h = ((v.qb >> 5u) & 31u) + 1u;
    let ao = (v.qb >> 10u) & 255u;
    c.mat = (v.qb >> 18u) & 255u;
    let a0 = ao & 3u;
    let a1 = (ao >> 2u) & 3u;
    let a2 = (ao >> 4u) & 3u;
    let a3 = (ao >> 6u) & 3u;
    var tris = TRIS;
    var flip = TRIS_FLIP;
    var t = tris[v.vi];
    if (a0 + a2 < a1 + a3) {
        t = flip[v.vi];
    }
    var pos_tab = POSITIVE;
    var neg = NEG;
    if (!pos_tab[c.dir]) {
        t = neg[t];
    }
    c.k = t;
    var aos = array<u32, 4>(a0, a1, a2, a3);
    c.ao = aos[t];
    let du = select(0u, w, t == 1u || t == 2u);
    let dv = select(0u, h, t == 2u || t == 3u);
    var ua = UAX;
    var va = VAX;
    let o = origins[v.slot];
    c.pos = o.xyz + vec3<f32>(base + ua[c.dir] * du + va[c.dir] * dv) * o.w;
    return c;
}

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) ao: f32,
    @location(4) voxel: f32,
    @location(5) smooth_n: vec3<f32>,
};

@group(0) @binding(4) var<uniform> palette: array<vec4<f32>, 64>;

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let c = corner(v);
    o.clip = g.view_proj * vec4(c.pos, 1.0);
    o.world = c.pos;
    o.color = vec4(palette[c.mat].rgb, f32(c.mat) / 255.0);
    var dirs = DIRS; // copia local: naga solo permite índices variables sobre variables
    o.normal = dirs[c.dir];
    o.ao = f32(c.ao);
    o.voxel = origins[v.slot].w;
    let sw = (v.smooth[c.k / 2u] >> ((c.k % 2u) * 16u)) & 0xffffu;
    let sx = (f32(sw & 255u) - 128.0) / 127.0;
    let sz = (f32(sw >> 8u) - 128.0) / 127.0;
    o.smooth_n = vec3(sx, sqrt(max(1.0 - sx * sx - sz * sz, 0.0)), sz);
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    return g.light_view_proj * vec4(corner(v).pos, 1.0);
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

// Números de material (deben coincidir con palette.rs).
const M_WINDOW: u32 = 14u;
const M_TILE: u32 = 8u;
const M_THATCH: u32 = 9u;
const M_TILE_DARK: u32 = 30u;
const M_TILE_LIGHT: u32 = 31u;
const M_THATCH_DARK: u32 = 32u;
const M_REJA: u32 = 35u;
const M_STONE: u32 = 36u;
const M_STONE_DARK: u32 = 37u;
const M_HIDE: u32 = 38u;
const M_DOME: u32 = 41u;

// Materiales del terreno (deben coincidir con palette.rs).
fn is_terrain(m: u32) -> bool {
    return m == 1u || m == 2u || m == 3u || m == 4u || m == 5u || m == 12u || m == 13u || m == 17u || m == 20u || m == 25u;
}

// Coordenada horizontal "a lo largo" de la cara (para dibujos verticales).
fn along(world: vec3<f32>, n: vec3<f32>) -> f32 {
    if (abs(n.x) > 0.5) {
        return world.z;
    }
    return world.x;
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let n = normalize(v.normal);
    let mat = u32(v.color.a * 255.0 + 0.5);
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
    // Escalones del terreno: el terreno es una ladera suave cortada en vóxeles. Se ilumina
    // con la normal suave de la ladera (calculada al mallar), así el relieve se lee como una
    // colina y los escalones de un vóxel no forman "curvas de nivel" oscuras a lo lejos.
    let terr = is_terrain(mat) && g.extra.x > 0.5;
    var ln = n;
    if (terr) {
        ln = normalize(v.smooth_n);
        // De cerca se nota algo el escalón (estética de vóxeles); de lejos, nada.
        if (abs(n.y) < 0.5) {
            let far = smoothstep(0.08, 0.35, fw);
            ln = normalize(mix(normalize(n * 0.35 + ln), ln, far));
        }
    }
    albedo *= 1.0 - select(0.18, 0.05, terr) * line;

    // Detalle más fino que el vóxel, dibujado por material. Se apaga a lo lejos (fwidth)
    // para que no haga ruido: de lejos manda el color del material.
    let near = 1.0 - smoothstep(0.25, 0.7, length(fwidth(v.world * 4.0)));
    var glass = 0.0;
    if (mat == M_REJA) {
        // Reja de hierro: barrotes verticales cada 12,5 cm y un travesaño arriba.
        let h = fract(along(v.world, n) * 8.0);
        let ty = fract(v.world.y * 2.0);
        let bar = max(step(h, 0.3), step(0.86, ty));
        // Vidrio que refleja el cielo detrás de barrotes casi negros: así la reja se lee.
        let pane = srgb_to_linear(vec3(0.24, 0.29, 0.34)) * (0.85 + 0.3 * fract(v.world.y * 0.7));
        let iron = srgb_to_linear(vec3(0.05, 0.05, 0.05));
        albedo = mix(pane, iron, bar * near + (1.0 - near) * 0.5);
        glass = 1.0 - bar * near;
    } else if (mat == M_WINDOW) {
        glass = 1.0;
        albedo = mix(albedo, srgb_to_linear(vec3(0.20, 0.24, 0.28)), 0.6);
        // Postigos: marco claro alrededor de cada hoja de 0,5 m.
        let h = fract(along(v.world, n) * 2.0);
        let frame = 1.0 - step(0.12, h) * step(h, 0.88);
        albedo = mix(albedo, albedo * 2.2, frame * near * 0.5);
    } else if (mat == M_TILE || mat == M_TILE_DARK || mat == M_TILE_LIGHT) {
        // Tejas: canales cada 25 cm y filas de 25 cm, en tresbolillo.
        let a = along(v.world, n) * 4.0;
        let row = select(floor(v.world.y * 4.0), floor(v.world.z * 4.0), n.y > 0.5);
        let canal = 0.5 + 0.5 * sin((a + row * 0.5) * 6.2832);
        albedo *= mix(1.0, 0.82 + 0.22 * canal, near);
    } else if (mat == M_THATCH || mat == M_THATCH_DARK) {
        // Paja: hebras verticales irregulares.
        let a = along(v.world, n) * 10.0;
        let s = hash3(vec3(floor(a), floor(v.world.y * 3.0), 0.0));
        albedo *= mix(1.0, 0.82 + 0.3 * s, near);
    } else if (mat == M_STONE || mat == M_STONE_DARK) {
        // Sillería: juntas de mortero que dividen el vóxel en dos sillares en tresbolillo.
        let a = fract(along(v.world, n) * 2.0 + floor(v.world.y * 2.0) * 0.5);
        let joint = max(1.0 - step(0.06, fract(v.world.y * 2.0)), 1.0 - step(0.05, a));
        albedo *= 1.0 - 0.25 * joint * near;
    } else if (mat == M_HIDE) {
        // Cueros estaqueados: borde más oscuro.
        let a = fract(along(v.world, n) * 2.0);
        albedo *= mix(1.0, 0.75 + 0.25 * smoothstep(0.0, 0.2, min(a, 1.0 - a)), near);
    } else if (mat == M_DOME) {
        // Azulejos de la cúpula: damero fino.
        let c = floor(along(v.world, n) * 6.0) + floor(v.world.y * 6.0);
        albedo *= mix(1.0, 0.9 + 0.12 * (c - 2.0 * floor(c * 0.5)), near);
    }

    let ao = mix(select(0.42, 0.8, terr), 1.0, clamp(v.ao / 3.0, 0.0, 1.0));
    let ndl = max(dot(ln, g.sun_dir.xyz), 0.0);
    // Las caras que no miran al sol no reciben su luz: no hace falta consultar la sombra
    // (9 lecturas del mapa por fragmento). El resultado es idéntico.
    var sh = 1.0;
    if (ndl > 0.0) {
        sh = shadow(v.world, n);
    }
    let sky = mix(g.sky_horizon.rgb, g.sky_top.rgb, ln.y * 0.5 + 0.5);
    let bounce = vec3(0.35, 0.30, 0.22) * max(-ln.y, 0.0);
    var light = g.sun_color.rgb * ndl * sh + (sky * 0.55 + bounce * 0.2) * g.params.w * ao;
    var col = albedo * light;
    // Ventanas iluminadas por velas de noche.
    if (glass > 0.0) {
        // Toda la ventana se enciende o no (celda de ~2 m × 3 m), no vóxel por vóxel.
        let lit = step(0.5, hash3(floor(v.world / vec3(2.0, 3.0, 2.0))));
        col += vec3(1.0, 0.55, 0.2) * 1.6 * g.params.y * lit * glass;
    }
    // Niebla de distancia hacia el color del horizonte.
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x);
    col = mix(col, g.sky_horizon.rgb, clamp(fog, 0.0, 0.92));
    return vec4(col, 1.0);
}
