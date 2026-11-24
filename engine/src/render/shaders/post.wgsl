// [render] Pasadas de pantalla de las técnicas del mod LOD (docs/sistemas/tecnicas-minecraft-lod.md):
// - SSAO lejano (§2.13, `lod_ssao.fsh` + `lod_acabado.fsh` del mod): 8 muestras en espiral de
//   ángulo áureo, radio que crece con la distancia (máx(2 m, 0,025·d)), a media resolución; se
//   aplica con un realce bilateral (respeta los bordes de profundidad) multiplicando el color ya
//   dibujado. Solo lejos (desde ~150 m; de cerca manda la oclusión por vértice) y menos con la
//   niebla.
// - Escala de render (§2.16, `EscaladoFsr` del mod): realce bicúbico (Catmull-Rom con 9 lecturas
//   bilineales) de la imagen chica a la ventana y nitidez adaptativa RCAS de AMD FidelityFX FSR 1
//   (licencia MIT, AMD); el realce EASU de FSR 1 queda pendiente.
// `DEPTH_TEX` se reemplaza por `texture_depth_2d` o `texture_depth_multisampled_2d` (MSAA).
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
// screen: ancho y alto de la imagen de la escena (px), píxeles por unidad de tangente (foco), 0.
struct Post {
    screen: vec4<f32>,
};

struct FOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> FOut {
    var o: FOut;
    let uv = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    o.pos = vec4(uv * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

// --- SSAO --------------------------------------------------------------------------------------

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<uniform> post: Post;
@group(0) @binding(2) var depth_tex: DEPTH_TEX;
@group(0) @binding(3) var ao_tex: texture_2d<f32>;

fn depth_at(p: vec2<i32>) -> f32 {
    let s = vec2<i32>(post.screen.xy) - vec2(1);
    return textureLoad(depth_tex, clamp(p, vec2(0), s), 0);
}

// Punto del mundo (con la curvatura ya aplicada, como se dibujó) de un píxel y su profundidad.
fn world_at(p: vec2<f32>, d: f32) -> vec3<f32> {
    let ndc = vec2((p.x + 0.5) / post.screen.x * 2.0 - 1.0, 1.0 - (p.y + 0.5) / post.screen.y * 2.0);
    let w = g.inv_view_proj * vec4(ndc, d, 1.0);
    return w.xyz / w.w;
}

fn hash2(p: vec2<u32>) -> f32 {
    var h = p.x * 0x8da6b343u ^ p.y * 0xd8163841u;
    h = (h ^ (h >> 15u)) * 0x2c1b3c6du;
    h = h ^ (h >> 12u);
    return f32(h & 0xffffu) / 65535.0;
}

// Oclusión de un píxel de media resolución: r = oclusión (1 = nada), g = distancia a la cámara.
@fragment
fn fs_ssao(f: FOut) -> @location(0) vec4<f32> {
    let hp = vec2<i32>(f.pos.xy);
    let pc = hp * 2;
    let d0 = depth_at(pc);
    if (d0 <= 0.0) {
        return vec4(1.0, 1.0e9, 0.0, 1.0);
    }
    let pf = vec2<f32>(pc);
    let p = world_at(pf, d0);
    let dist = distance(p, g.cam_pos.xyz);
    // De cerca manda la oclusión por vértice; lo lejano se funde con la niebla.
    let lejos = smoothstep(100.0, 200.0, dist);
    let fog = clamp(1.0 - exp(-dist * g.params.x), 0.0, 0.88);
    let peso = lejos * (1.0 - fog);
    if (peso < 0.01) {
        return vec4(1.0, dist, 0.0, 1.0);
    }
    // Normal con los vecinos del lado más parecido (sin cruzar bordes).
    let dx1 = world_at(pf + vec2(2.0, 0.0), depth_at(pc + vec2(2, 0))) - p;
    let dx0 = p - world_at(pf - vec2(2.0, 0.0), depth_at(pc - vec2(2, 0)));
    let dy1 = world_at(pf + vec2(0.0, 2.0), depth_at(pc + vec2(0, 2))) - p;
    let dy0 = p - world_at(pf - vec2(0.0, 2.0), depth_at(pc - vec2(0, 2)));
    let dx = select(dx0, dx1, dot(dx1, dx1) < dot(dx0, dx0));
    let dy = select(dy0, dy1, dot(dy1, dy1) < dot(dy0, dy0));
    var n = normalize(cross(dy, dx));
    if (dot(n, g.cam_pos.xyz - p) < 0.0) {
        n = -n;
    }
    let r = max(2.0, 0.025 * dist);
    let r_px = clamp(r / dist * post.screen.z, 2.0, 64.0);
    let giro = hash2(vec2<u32>(hp)) * 6.2831853;
    var occ = 0.0;
    for (var i = 0; i < 8; i++) {
        let a = f32(i) * 2.3999632 + giro;
        let rad = sqrt((f32(i) + 0.5) / 8.0) * r_px;
        let q = pf + vec2(cos(a), sin(a)) * rad;
        let ds = depth_at(vec2<i32>(q));
        if (ds <= 0.0) {
            continue;
        }
        let v = world_at(floor(q), ds) - p;
        let l = length(v);
        if (l < 1.0e-3) {
            continue;
        }
        // Lo muy cercano no cuenta (los escalones de un vóxel dibujaban curvas de nivel): solo
        // tapa lo que está a más de un tercio del radio, por encima de unos 20° del plano.
        occ += max(dot(n, v) / l - 0.35, 0.0) * smoothstep(0.2 * r, 0.4 * r, l) * (1.0 - smoothstep(r, 2.0 * r, l));
    }
    let ao = clamp(1.0 - 1.8 * occ / 8.0, 0.0, 1.0);
    return vec4(mix(1.0, ao, peso), dist, 0.0, 1.0);
}

// Aplicación a resolución completa: las 4 muestras de media resolución más cercanas, pesadas por
// cercanía y por parecido de la distancia (bilateral). Sale el factor (mezcla multiplicativa).
@fragment
fn fs_ssao_apply(f: FOut) -> @location(0) vec4<f32> {
    let pc = vec2<i32>(f.pos.xy);
    let d0 = depth_at(pc);
    if (d0 <= 0.0) {
        return vec4(1.0);
    }
    let dist = distance(world_at(vec2<f32>(pc), d0), g.cam_pos.xyz);
    let hs = vec2<i32>(textureDimensions(ao_tex)) - vec2(1);
    let hf = (vec2<f32>(pc) + 0.5) * 0.5 - 0.5;
    let b = vec2<i32>(floor(hf));
    let t = hf - floor(hf);
    var acc = 0.0;
    var wsum = 0.0;
    for (var k = 0; k < 4; k++) {
        let o = vec2(k & 1, k >> 1);
        let s = textureLoad(ao_tex, clamp(b + o, vec2(0), hs), 0);
        let wb = select(1.0 - t.x, t.x, o.x == 1) * select(1.0 - t.y, t.y, o.y == 1);
        let wd = 1.0 / (0.02 + abs(s.g - dist) / max(dist, 1.0));
        let w = wb * wd + 1.0e-4;
        acc += s.r * w;
        wsum += w;
    }
    let ao = acc / wsum;
    return vec4(ao, ao, ao, 1.0);
}

// --- Escala de render ---------------------------------------------------------------------------

@group(0) @binding(4) var src_tex: texture_2d<f32>;
@group(0) @binding(5) var src_samp: sampler;

// Catmull-Rom con 9 lecturas bilineales (truco de las lecturas combinadas).
@fragment
fn fs_upscale(f: FOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(src_tex));
    let uv = f.pos.xy / post.screen.xy;
    let sp = uv * size;
    let tc = floor(sp - 0.5) + 0.5;
    let t = sp - tc;
    let w0 = t * (-0.5 + t * (1.0 - 0.5 * t));
    let w1 = 1.0 + t * t * (-2.5 + 1.5 * t);
    let w2 = t * (0.5 + t * (2.0 - 1.5 * t));
    let w3 = t * t * (-0.5 + 0.5 * t);
    let w12 = w1 + w2;
    let o12 = w2 / w12;
    let p0 = (tc - 1.0) / size;
    let p3 = (tc + 2.0) / size;
    let p12 = (tc + o12) / size;
    var c = vec3(0.0);
    c += textureSampleLevel(src_tex, src_samp, vec2(p0.x, p0.y), 0.0).rgb * w0.x * w0.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p12.x, p0.y), 0.0).rgb * w12.x * w0.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p3.x, p0.y), 0.0).rgb * w3.x * w0.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p0.x, p12.y), 0.0).rgb * w0.x * w12.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p12.x, p12.y), 0.0).rgb * w12.x * w12.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p3.x, p12.y), 0.0).rgb * w3.x * w12.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p0.x, p3.y), 0.0).rgb * w0.x * w3.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p12.x, p3.y), 0.0).rgb * w12.x * w3.y;
    c += textureSampleLevel(src_tex, src_samp, vec2(p3.x, p3.y), 0.0).rgb * w3.x * w3.y;
    return vec4(max(c, vec3(0.0)), 1.0);
}

// RCAS (Robust Contrast Adaptive Sharpening) de FSR 1: nitidez limitada por el contraste local
// para no sacar halos. Nitidez 0,2 "stops" (como el mod).
@fragment
fn fs_rcas(f: FOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(f.pos.xy);
    let s = vec2<i32>(textureDimensions(src_tex)) - vec2(1);
    let e = textureLoad(src_tex, p, 0).rgb;
    let b = textureLoad(src_tex, clamp(p + vec2(0, -1), vec2(0), s), 0).rgb;
    let d = textureLoad(src_tex, clamp(p + vec2(-1, 0), vec2(0), s), 0).rgb;
    let fr = textureLoad(src_tex, clamp(p + vec2(1, 0), vec2(0), s), 0).rgb;
    let h = textureLoad(src_tex, clamp(p + vec2(0, 1), vec2(0), s), 0).rgb;
    let mn4 = min(min(b, d), min(fr, h));
    let mx4 = max(max(b, d), max(fr, h));
    let hit_min = mn4 / (4.0 * mx4 + 1.0e-5);
    let hit_max = (vec3(1.0) - mx4) / (4.0 * mn4 - 4.0 - 1.0e-5);
    let lobe_rgb = max(-hit_min, hit_max);
    let lobe = max(-0.1875, min(max(lobe_rgb.r, max(lobe_rgb.g, lobe_rgb.b)), 0.0)) * exp2(-0.2);
    let c = (lobe * (b + d + fr + h) + e) / (4.0 * lobe + 1.0);
    return vec4(clamp(c, vec3(0.0), vec3(1.0)), 1.0);
}
