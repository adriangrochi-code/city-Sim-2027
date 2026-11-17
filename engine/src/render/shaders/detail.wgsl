// Flora baja en micro-vóxeles de 0,125 m (ver trees::Micro): una instancia por caja; el
// vertex shader arma sus caras con `vertex_index`. Luz, sombra recibida y niebla como el
// terreno; las puntas se mecen con el viento y todo se achica hasta desaparecer en el borde
// del nivel más fino (más lejos la pradera se ve por el color del suelo).
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
    extra: vec4<f32>,  // w: viento (dirección de donde viene, en grados, × 100 + velocidad m/s)
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;
@group(0) @binding(4) var<uniform> palette: array<vec4<f32>, 96>;

const MICRO: f32 = 0.125;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) size: vec4<u32>,
    @location(2) info: vec4<u32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) h: f32,
};

fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3(2.2));
}

// Esquinas de las 5 caras visibles (la de abajo apoya en el suelo): 4 por cara.
const CORNERS = array<vec3<f32>, 20>(
    vec3(1.0, 0.0, 0.0), vec3(1.0, 1.0, 0.0), vec3(1.0, 1.0, 1.0), vec3(1.0, 0.0, 1.0), // +x
    vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 1.0), vec3(0.0, 1.0, 0.0), // -x
    vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 1.0), vec3(1.0, 1.0, 1.0), vec3(1.0, 1.0, 0.0), // +y
    vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 1.0), vec3(1.0, 1.0, 1.0), vec3(0.0, 1.0, 1.0), // +z
    vec3(0.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(1.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), // -z
);
const NORMALS = array<vec3<f32>, 5>(vec3(1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, -1.0));
const TRI = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);

fn hash_u(x: u32) -> u32 {
    var h = x;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return h;
}

@vertex
fn vs_main(v: VIn, @builtin(vertex_index) vi: u32) -> VOut {
    let face = vi / 6u;
    var corners = CORNERS;
    var tri = TRI;
    var normals = NORMALS;
    let c = corners[face * 4u + tri[vi % 6u]];
    var size = vec3<f32>(v.size.xyz) * MICRO;
    // Se achica a lo lejos (desde 70 m) y desaparece antes del borde del nivel fino.
    let dist = distance(v.pos.xz, g.cam_pos.xz);
    let fade = 1.0 - smoothstep(70.0, 92.0, dist);
    // Suelo de la planta: la base de la caja menos su altura sobre el suelo.
    let ground = v.pos.y - f32(v.info.z) * MICRO;
    let hoja = v.info.w == 1u;
    var p = v.pos + c * size;
    if (hoja) {
        // Hoja de una copa: se achica en su lugar (no baja hacia el suelo).
        p = v.pos + size * 0.5 + (c - vec3(0.5)) * size * fade;
    } else {
        p.y = ground + (p.y - ground) * fade;
    }
    // Viento: las puntas se corren en la dirección a donde sopla, más cuanto más alto el
    // tallo y más fuerte el viento; ráfagas que recorren el campo.
    let w = g.extra.w;
    let wdir = floor(w / 100.0);
    let wspd = w - wdir * 100.0;
    let a = radians(wdir + 180.0);
    let to = vec2(sin(a), -cos(a));
    let gust = 0.6 + 0.4 * sin(g.params.z * 1.7 + dot(v.pos.xz, to) * 0.35);
    // Altura del punto sobre el suelo.
    let hh = max(p.y - ground, 0.0);
    var sway = min(wspd, 15.0) * 0.012 * hh * hh * gust;
    if (hoja) {
        // Las hojas de las copas tiemblan unos centímetros, desfasadas entre sí.
        sway = min(wspd, 15.0) * 0.006 * (0.5 + 0.5 * sin(g.params.z * 3.1 + f32(v.info.y) * 0.4));
    }
    p = vec3(p.x + to.x * sway, p.y, p.z + to.y * sway);
    var o: VOut;
    o.clip = g.view_proj * vec4(curved(p), 1.0);
    o.world = p;
    let shade = 0.82 + 0.36 * f32(v.info.y) / 255.0;
    o.color = srgb_to_linear(palette[v.info.x].rgb) * shade;
    o.normal = normals[face];
    o.h = hh;
    return o;
}

fn shadow(world: vec3<f32>, n: vec3<f32>) -> f32 {
    let lp = g.light_view_proj * vec4(world + n * 0.1, 1.0);
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

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let n = v.normal;
    // La base de la mata queda en la sombra de sus propias hojas.
    let ao = mix(0.55, 1.0, smoothstep(0.0, 0.5, v.h));
    let ndl = max(dot(n, g.sun_dir.xyz), 0.0);
    // Las hojas dejan pasar algo de luz: la cara a contraluz no queda negra.
    let trans = max(dot(-n, g.sun_dir.xyz), 0.0) * 0.25;
    var sh = 1.0;
    if (ndl + trans > 0.0) {
        sh = shadow(v.world, n);
    }
    let sky = mix(g.sky_horizon.rgb, g.sky_top.rgb, n.y * 0.5 + 0.5);
    let moon = vec3(0.45, 0.5, 0.65) * 0.14 * g.extra.z * g.params.y * ao;
    let light = g.sun_color.rgb * (ndl + trans) * sh + sky * 0.55 * g.params.w * ao + moon;
    var col = v.color * light;
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x);
    let vdir = normalize(v.world - g.cam_pos.xyz);
    let s = pow(max(dot(vdir, g.sun_dir.xyz), 0.0), 6.0) * (1.0 - g.params.y);
    let fog_c = mix(g.sky_horizon.rgb, g.sky_horizon.rgb * 0.55 + g.sun_color.rgb * 0.45, s * 0.6);
    col = mix(col, fog_c, clamp(fog, 0.0, 0.88));
    return vec4(col, 1.0);
}
