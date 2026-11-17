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
    @location(2) suave: vec2<u32>,
    @location(3) slot: u32,
    @builtin(vertex_index) vi: u32,
};
@group(0) @binding(3) var<storage, read> origins: array<vec4<f32>>;

// Dirección de cara (orden de mesher::DIRS: +y, +x, -x, +z, -z, -y) sin tablas: en el render
// por CPU (llvmpipe) los índices dinámicos sobre arreglos locales cuestan accesos a memoria.
fn dir_axis(d: u32) -> u32 {
    // 0 = x, 1 = y, 2 = z
    return select(select(2u, 0u, d == 1u || d == 2u), 1u, d == 0u || d == 5u);
}
fn dir_normal(d: u32) -> vec3<f32> {
    let s = select(-1.0, 1.0, d == 0u || d == 1u || d == 3u);
    let ax = dir_axis(d);
    return vec3(select(0.0, s, ax == 0u), select(0.0, s, ax == 1u), select(0.0, s, ax == 2u));
}
fn unit(ax: u32) -> vec3<u32> {
    return vec3(select(0u, 1u, ax == 0u), select(0u, 1u, ax == 1u), select(0u, 1u, ax == 2u));
}

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
    // Índices compartidos 0,1,2,0,2,3 sobre 4 vértices por cara (así se reutilizan los
    // vértices): la diagonal y el sentido se eligen rotando y reflejando las esquinas.
    let flip = (ao & 3u) + ((ao >> 4u) & 3u) < ((ao >> 2u) & 3u) + ((ao >> 6u) & 3u);
    var t = (v.vi + select(0u, 1u, flip)) & 3u;
    let positive = c.dir == 0u || c.dir == 1u || c.dir == 3u;
    t = select((4u - t) & 3u, t, positive);
    c.k = t;
    c.ao = (ao >> (t * 2u)) & 3u;
    let du = select(0u, w, t == 1u || t == 2u);
    let dv = select(0u, h, t == 2u || t == 3u);
    let ax = dir_axis(c.dir);
    let o = origins[v.slot];
    c.pos = o.xyz + vec3<f32>(base + unit((ax + 1u) % 3u) * du + unit((ax + 2u) % 3u) * dv) * o.w;
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

@group(0) @binding(4) var<uniform> palette: array<vec4<f32>, 96>;

// Curvatura de la Tierra (radio 6371 km): lo lejano baja d²/2R respecto de la cámara. A 10 km
// son 8 m; a 24 km, 45 m: el horizonte se curva y el río se pierde detrás de él.
fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let c = corner(v);
    o.clip = g.view_proj * vec4(curved(c.pos), 1.0);
    o.world = c.pos;
    o.color = vec4(palette[c.mat].rgb, f32(c.mat) / 255.0);
    o.normal = dir_normal(c.dir);
    o.ao = f32(c.ao);
    o.voxel = origins[v.slot].w;
    let sw = (v.suave[c.k / 2u] >> ((c.k % 2u) * 16u)) & 0xffffu;
    let sx = (f32(sw & 255u) - 128.0) / 127.0;
    let sz = (f32(sw >> 8u) - 128.0) / 127.0;
    o.smooth_n = vec3(sx, sqrt(max(1.0 - sx * sx - sz * sz, 0.0)), sz);
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    return g.light_view_proj * vec4(corner(v).pos, 1.0);
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

// `p` son coordenadas ya redondeadas (celdas enteras).
fn hash3(p: vec3<f32>) -> f32 {
    let i = vec3<i32>(floor(p));
    let h = hash_u(bitcast<u32>(i.x) ^ hash_u(bitcast<u32>(i.y) ^ hash_u(bitcast<u32>(i.z) + 0x9e3779b9u)));
    return f32(h & 0xffffffu) / 16777215.0;
}

// Ruido de valor 2D (0..1), suave, para variaciones de color a gran escala.
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash3(vec3(i, 0.0));
    let b = hash3(vec3(i + vec2(1.0, 0.0), 0.0));
    let c = hash3(vec3(i + vec2(0.0, 1.0), 0.0));
    let d = hash3(vec3(i + vec2(1.0, 1.0), 0.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Color de la niebla: el del horizonte, más cálido y claro mirando hacia el sol (la luz
// que dispersa el aire), y nada de eso de noche.
fn fog_color(world: vec3<f32>) -> vec3<f32> {
    let vdir = normalize(world - g.cam_pos.xyz);
    let s = pow(max(dot(vdir, g.sun_dir.xyz), 0.0), 6.0) * (1.0 - g.params.y);
    return mix(g.sky_horizon.rgb, g.sky_horizon.rgb * 0.55 + g.sun_color.rgb * 0.45, s * 0.6);
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
const M_STREAM: u32 = 52u;
const M_MARSH: u32 = 48u;
const M_TABLAS: u32 = 64u;
const M_TABLAS_GRIS: u32 = 65u;
const M_QUINCHA: u32 = 66u;
const M_QUINCHA_DARK: u32 = 67u;
const M_MAMPOSTERIA: u32 = 68u;
const M_MAMPOSTERIA_DARK: u32 = 69u;
const M_SHUTTER: u32 = 16u;
const M_CARP_ROJIZA: u32 = 78u;
const M_CARP_OCRE: u32 = 80u;
const M_TILE_LIQUEN: u32 = 81u;

// Materiales del terreno (deben coincidir con palette.rs).
fn is_terrain(m: u32) -> bool {
    return m == 1u || m == 2u || m == 3u || m == 4u || m == 5u || m == 12u || m == 13u || m == 17u || m == 20u || m == 25u || (m >= 47u && m <= 53u);
}

// Materiales de obra (revoques, tejas, paja, madera, ladrillo, piedra, aberturas): los
// edificios se diseñan en vóxeles de 0,5 m (`lod::DESIGN_VOXEL`), cada uno hecho de 2 × 2 × 2
// vóxeles del mundo. Sus bordes y su tinte van por vóxel de diseño, no por vóxel del mundo.
const DESIGN_VOXEL: f32 = 0.5;
fn is_built(m: u32) -> bool {
    return (m >= 6u && m <= 11u) || (m >= 14u && m <= 16u) || (m >= 21u && m <= 23u) || (m >= 26u && m <= 46u && m != 44u) || (m >= 64u && m <= 81u);
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
    // Variación de tono por vóxel: se ve cada bloque aunque las caras estén unidas (en la
    // obra, por vóxel de diseño).
    let cv = select(v.voxel, max(v.voxel, DESIGN_VOXEL), is_built(mat));
    let cell = floor((v.world - n * 0.01 * cv) / cv);
    let tint = 0.9 + 0.16 * hash3(cell);
    var albedo = srgb_to_linear(v.color.rgb) * tint;
    // Bordes de cada vóxel, más suaves a lo lejos.
    let f = fract(v.world / cv);
    let e3 = min(f, 1.0 - f) + abs(n) * 10.0;
    let edge = min(min(e3.x, e3.y), e3.z);
    let fw = length(fwidth(v.world / cv));
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
            ln = normalize(mix(normalize(n * 0.15 + ln), ln, far));
        }
    }
    albedo *= 1.0 - select(0.18, 0.05, terr) * line;
    // Monte visto de lejos: en los niveles sin árboles (vóxel de más de 4 m) el suelo del
    // monte toma el color de las copas, y a media distancia se mezcla con él (desde lejos,
    // entre árbol y árbol se ven más copas que hojarasca).
    if (mat == 53u) {
        let m = vnoise(v.world.xz / 23.0);
        let canopy = srgb_to_linear(mix(vec3(0.17, 0.27, 0.12), vec3(0.27, 0.36, 0.16), m));
        let k = max(smoothstep(3.0, 6.0, v.voxel), smoothstep(0.1, 0.5, fw) * 0.7);
        albedo = mix(albedo, canopy, k);
    }
    // Follaje: cada racimo de hojas (~0,8 m) con su tono, y las hojas de adentro y de abajo
    // de la copa más oscuras (la oclusión de los vóxeles no alcanza a verse de lejos).
    if (mat == 18u || mat == 24u || mat == 44u || mat == 54u || mat == 55u || mat == 57u) {
        let q = v.world / 0.8;
        let r = hash3(floor(q)) * 0.6 + hash3(floor(q * 2.0) + 77.0) * 0.4;
        albedo *= mix(0.68, 1.15, r) * mix(0.72, 1.0, smoothstep(-0.6, 0.6, ln.y));
    }
    // Pastos y arenas: manchas de color de decenas de metros (más verde azulado en unas,
    // más pajizo en otras), para que de lejos el campo no sea un verde parejo.
    if (mat == 1u || mat == 20u || mat == 49u || mat == 3u || mat == 51u) {
        let m = vnoise(v.world.xz / 70.0) * 0.6 + vnoise(v.world.xz / 19.0 + 13.0) * 0.4;
        let k = select(0.5, 1.0, mat == 1u || mat == 20u || mat == 49u);
        albedo *= mix(vec3(1.0), mix(vec3(0.86, 0.91, 0.88), vec3(1.09, 1.05, 0.9), m), k);
    }

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
    } else if (mat == M_SHUTTER || (mat >= M_CARP_ROJIZA && mat <= M_CARP_OCRE)) {
        // Carpintería pintada (puertas y postigos): tableros con el marco claro y la junta
        // entre las dos hojas cada 0,5 m.
        let h = fract(along(v.world, n) * 2.0);
        let ty = fract(v.world.y * 1.4);
        let rail = 1.0 - step(0.1, ty) * step(ty, 0.9);
        let stile = 1.0 - step(0.08, h) * step(h, 0.92);
        albedo *= mix(1.0, 1.0 + 0.18 * max(rail, stile) - 0.25 * (1.0 - step(0.03, h)), near);
    } else if (mat == M_TILE || mat == M_TILE_DARK || mat == M_TILE_LIGHT || mat == M_TILE_LIQUEN) {
        // Tejas: canales cada 25 cm y filas de 25 cm, en tresbolillo.
        let a = along(v.world, n) * 4.0;
        let row = select(floor(v.world.y * 4.0), floor(v.world.z * 4.0), n.y > 0.5);
        let canal = 0.5 + 0.5 * sin(fract(a + row * 0.5) * 6.2832);
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
    } else if (mat == M_MARSH) {
        // Bañado: matas de junco más oscuras y espejos de agua entre ellas.
        let c = hash3(floor(v.world * vec3(1.5, 1.0, 1.5)));
        albedo *= mix(1.0, 0.75 + 0.4 * c, near * 0.8 + 0.2);
    } else if (mat == M_TABLAS || mat == M_TABLAS_GRIS) {
        // Tablas verticales de 20 cm con la junta oscura y la veta.
        let a = fract(along(v.world, n) * 5.0);
        let joint = 1.0 - step(0.08, a);
        let grain = 0.94 + 0.08 * sin(v.world.y * 23.0 + floor(along(v.world, n) * 5.0) * 1.7);
        albedo *= mix(1.0, (1.0 - 0.35 * joint) * grain, near);
    } else if (mat == M_QUINCHA || mat == M_QUINCHA_DARK) {
        // Barro con paja: pajitas claras sueltas en la masa.
        let s = hash3(floor(v.world * 12.0));
        albedo *= mix(1.0, select(0.92, 1.25, s > 0.86), near);
    } else if (mat == M_MAMPOSTERIA || mat == M_MAMPOSTERIA_DARK) {
        // Piedras irregulares: celdas de ~25 cm con juntas de barro.
        let q = v.world * 4.0 + vec3(0.0, 0.0, floor(v.world.y * 4.0) * 0.37);
        let c = fract(q);
        let j = min(min(c.x, 1.0 - c.x), min(c.y, 1.0 - c.y));
        let tone = 0.85 + 0.25 * hash3(floor(q));
        albedo *= mix(1.0, tone * (1.0 - 0.3 * (1.0 - smoothstep(0.0, 0.1, j))), near);
    } else if (mat == M_DOME) {
        // Azulejos de la cúpula: damero fino.
        let c = floor(along(v.world, n) * 6.0) + floor(v.world.y * 6.0);
        albedo *= mix(1.0, 0.9 + 0.12 * (c - 2.0 * floor(c * 0.5)), near);
    }

    // De lejos, el terreno no se oscurece en los escalones: si no, cada escalón dibuja una
    // curva de nivel y la ladera se ve rayada.
    let far_t = smoothstep(0.08, 0.35, fw);
    let ao_min = select(0.42, mix(0.9, 1.0, far_t), terr);
    let ao = mix(ao_min, 1.0, clamp(v.ao / 3.0, 0.0, 1.0));
    let ndl = max(dot(ln, g.sun_dir.xyz), 0.0);
    // Las caras que no miran al sol no reciben su luz: no hace falta consultar la sombra
    // (9 lecturas del mapa por fragmento). El resultado es idéntico.
    var sh = 1.0;
    if (ndl > 0.0) {
        // El terreno es una ladera suave hecha de escalones de un vóxel: con el sol bajo, cada
        // escalón tiraba una sombra larga sobre el de al lado (rayas en las laderas). Se consulta
        // la sombra unos vóxeles hacia el sol: los escalones vecinos ya no la tapan, y los árboles,
        // las casas y los cerros siguen dando sombra.
        // Con el sol bajo la sombra de un escalón es más larga: el corrimiento crece con 1/alto.
        let hacia_sol = select(0.0, (v.voxel * 1.5 + 0.3) / max(g.sun_dir.y, 0.12), terr);
        sh = shadow(v.world + g.sun_dir.xyz * hacia_sol, n);
    }
    // Luz del cielo en la sombra, algo desaturada: si no, al atardecer las sombras se ponían
    // violetas y azules.
    let sky_c = mix(g.sky_horizon.rgb, g.sky_top.rgb, ln.y * 0.5 + 0.5);
    let sky = mix(vec3(dot(sky_c, vec3(0.299, 0.587, 0.114))), sky_c, 0.55);
    let bounce = vec3(0.35, 0.30, 0.22) * max(-ln.y, 0.0);
    // Luz de luna: de noche, más clara en luna llena (extra.z), fría y desde arriba.
    let moon = vec3(0.45, 0.5, 0.65) * 0.14 * g.extra.z * g.params.y * (0.4 + 0.6 * max(ln.y, 0.0)) * ao;
    var light = g.sun_color.rgb * ndl * sh + (sky * 0.55 + bounce * 0.2) * g.params.w * ao + moon;
    var col = albedo * light;
    // Agua de los arroyos: refleja el cielo, más cuanto más rasante se mira.
    if (mat == M_STREAM) {
        let vdir = normalize(g.cam_pos.xyz - v.world);
        let fres = 0.25 + 0.6 * pow(1.0 - max(vdir.y, 0.0), 3.0);
        col = mix(col, g.sky_horizon.rgb * 0.9, fres);
    }
    // Ventanas iluminadas por velas de noche.
    if (glass > 0.0) {
        // Toda la ventana se enciende o no (celda de ~2 m × 3 m), no vóxel por vóxel.
        let lit = step(0.5, hash3(floor(v.world / vec3(2.0, 3.0, 2.0))));
        col += vec3(1.0, 0.55, 0.2) * 0.9 * g.params.y * lit * glass;
    }
    // Niebla de distancia hacia el color del horizonte.
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x);
    col = mix(col, fog_color(v.world), max(clamp(fog, 0.0, 0.88), reach_fog(v.world)));
    return vec4(curva_tonal(col), 1.0);
}

// Curva tonal suave: hasta 0,75 de luminancia no toca nada; más arriba comprime de a poco (sin
// cambiar el tono), así los claros al sol no se queman en amarillo saturado.
fn curva_tonal(c: vec3<f32>) -> vec3<f32> {
    let l = dot(c, vec3(0.2126, 0.7152, 0.0722));
    if (l <= 0.75) {
        return c;
    }
    let x = l - 0.75;
    let l2 = 0.75 + x / (1.0 + x * 2.5);
    return c * (l2 / l);
}
