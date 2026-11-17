// Vecinos y vehículos (ver city::agentes::dibujo::Caja): una instancia por caja, girada
// alrededor de y; el vertex shader arma sus 5 caras visibles con `vertex_index` (la de abajo
// apoya en el suelo). Luz, sombra recibida, niebla y curvatura como la flora baja.
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
    @location(2) tam: vec3<f32>,
    @location(3) color: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
};

fn curved(p: vec3<f32>) -> vec3<f32> {
    let d = p.xz - g.cam_pos.xz;
    return vec3(p.x, p.y - dot(d, d) / 12742000.0, p.z);
}

const CORNERS = array<vec3<f32>, 20>(
    vec3(1.0, 0.0, 0.0), vec3(1.0, 1.0, 0.0), vec3(1.0, 1.0, 1.0), vec3(1.0, 0.0, 1.0), // +x
    vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 1.0), vec3(0.0, 1.0, 0.0), // -x
    vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 1.0), vec3(1.0, 1.0, 1.0), vec3(1.0, 1.0, 0.0), // +y
    vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 1.0), vec3(1.0, 1.0, 1.0), vec3(0.0, 1.0, 1.0), // +z
    vec3(0.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(1.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), // -z
);
const NORMALS = array<vec3<f32>, 5>(vec3(1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, -1.0));
const TRI = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);

// Gira alrededor de y: (0, 0, 1) va a (sen, 0, cos).
fn girar(v: vec3<f32>, s: f32, c: f32) -> vec3<f32> {
    return vec3(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

@vertex
fn vs_main(v: VIn, @builtin(vertex_index) vi: u32) -> VOut {
    let face = vi / 6u;
    var corners = CORNERS;
    var tri = TRI;
    var normals = NORMALS;
    let c = corners[face * 4u + tri[vi % 6u]];
    let local = vec3((c.x - 0.5) * v.tam.x, c.y * v.tam.y, (c.z - 0.5) * v.tam.z);
    let s = sin(v.giro);
    let co = cos(v.giro);
    let p = v.pos + girar(local, s, co);
    var o: VOut;
    o.clip = g.view_proj * vec4(curved(p), 1.0);
    o.world = p;
    o.color = pow(v.color.rgb, vec3(2.2));
    o.normal = girar(normals[face], s, co);
    return o;
}


@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let n = normalize(v.normal);
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
    var col = v.color * light;
    let d = distance(v.world, g.cam_pos.xyz);
    let fog = 1.0 - exp(-d * g.params.x);
    let vdir = normalize(v.world - g.cam_pos.xyz);
    let s = pow(max(dot(vdir, g.sun_dir.xyz), 0.0), 6.0) * (1.0 - g.params.y);
    let fog_c = mix(g.sky_horizon.rgb, g.sky_horizon.rgb * 0.55 + g.sun_color.rgb * 0.45, s * 0.6);
    col = mix(col, fog_c, clamp(fog, 0.0, 0.88));
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
