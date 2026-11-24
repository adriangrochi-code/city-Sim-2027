// Descarte en la GPU: una invocación por columna. Prueba la caja de la columna contra el
// volumen de visión (y, en la vista principal, qué direcciones de cara miran a la cámara) y
// escribe los argumentos de dibujo indirecto de sus 6 grupos de caras. Las que no se ven
// quedan con 0 instancias. Es el ítem V4 pendiente de Tisma engine (PLAN.md): llenar
// `draw_indirect` desde un compute shader.
//
// Descarte de lo tapado (Hi-Z) en dos pasadas, sin errores al mover la cámara:
// - phase 1: se dibuja lo que se vio en el cuadro anterior (marca `visible`) y está en el
//   volumen de visión. Con esa profundidad se arma la pirámide (hiz.wgsl).
// - phase 2: se prueba TODO lo que está en el volumen contra la pirámide del cuadro actual;
//   se dibuja lo visible que no se dibujó en la pasada 1 y se actualizan las marcas.
// Como la pirámide sale de una parte de lo que se dibuja en este mismo cuadro, la prueba es
// conservadora: nada visible se descarta, ni siquiera un cuadro (no hay "pop-in").
// phase 0: solo volumen de visión (sin Hi-Z, o el pase de sombras).
struct Col {
    lo: vec4<f32>,   // w: nivel de detalle
    hi: vec4<f32>,   // w: 1 si la columna está activa
    start: u32,      // primera cara en el megabúfer
    g: array<u32, 7>, // inicio de cada grupo de caras (relativo a start)
};
struct Args {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};
struct Params {
    planes: array<vec4<f32>, 6>,
    eye: vec4<f32>,
    count: u32,
    shadow: u32,    // 1 = pase de sombras (todas las direcciones, solo niveles <= max_lod)
    max_lod: u32,
    phase: u32,
    view_proj: mat4x4<f32>,
    screen: vec4<f32>, // ancho, alto (píxeles), niveles de la pirámide, plano cercano
};
@group(0) @binding(0) var<storage, read> cols: array<Col>;
@group(0) @binding(1) var<storage, read_write> args: array<Args>;
@group(0) @binding(2) var<uniform> p: Params;
@group(0) @binding(3) var<storage, read_write> visible: array<u32>;
@group(0) @binding(4) var hiz: texture_2d<f32>;

fn box_visible(lo: vec3<f32>, hi: vec3<f32>) -> bool {
    for (var i = 0; i < 6; i++) {
        let pl = p.planes[i];
        let v = select(lo, hi, pl.xyz >= vec3(0.0));
        if (dot(pl.xyz, v) + pl.w < 0.0) {
            return false;
        }
    }
    return true;
}

// ¿La caja queda detrás de lo ya dibujado? Misma lógica que render/hiz.rs (`occluded`).
fn occluded(lo: vec3<f32>, hi: vec3<f32>) -> bool {
    var mn = vec2(1e9);
    var mx = vec2(-1e9);
    var zmax = 0.0;
    for (var k = 0u; k < 8u; k++) {
        let c = vec3(select(lo.x, hi.x, (k & 1u) != 0u), select(lo.y, hi.y, (k & 2u) != 0u), select(lo.z, hi.z, (k & 4u) != 0u));
        let q = p.view_proj * vec4(c, 1.0);
        // Una esquina detrás del plano cercano: no se puede proyectar, se dibuja.
        if (q.w <= p.screen.w) {
            return false;
        }
        let n = q.xyz / q.w;
        mn = min(mn, n.xy);
        mx = max(mx, n.xy);
        zmax = max(zmax, n.z);
    }
    let w = p.screen.x;
    let h = p.screen.y;
    // Rectángulo en píxeles (la y de la pantalla va hacia abajo), con un píxel de margen.
    let x0 = u32(clamp(floor((mn.x * 0.5 + 0.5) * w) - 1.0, 0.0, w - 1.0));
    let x1 = u32(clamp(ceil((mx.x * 0.5 + 0.5) * w) + 1.0, 0.0, w - 1.0));
    let y0 = u32(clamp(floor((0.5 - mx.y * 0.5) * h) - 1.0, 0.0, h - 1.0));
    let y1 = u32(clamp(ceil((0.5 - mn.y * 0.5) * h) + 1.0, 0.0, h - 1.0));
    // El píxel p cae en el texel min(p >> (l + 1), tamaño − 1) del nivel l (ver render/hiz.rs).
    let n = u32(p.screen.z);
    var l = 0u;
    var t0 = vec2<u32>(0u);
    var t1 = vec2<u32>(0u);
    loop {
        let dim = textureDimensions(hiz, i32(l)) - vec2(1u);
        t0 = min(vec2(x0, y0) >> vec2(l + 1u), dim);
        t1 = min(vec2(x1, y1) >> vec2(l + 1u), dim);
        if (l + 1u >= n || (t1.x - t0.x <= 1u && t1.y - t0.y <= 1u)) {
            break;
        }
        l += 1u;
    }
    let li = i32(l);
    let a = textureLoad(hiz, vec2<i32>(t0), li).r;
    let b = textureLoad(hiz, vec2<i32>(i32(t1.x), i32(t0.y)), li).r;
    let c = textureLoad(hiz, vec2<i32>(i32(t0.x), i32(t1.y)), li).r;
    let d = textureLoad(hiz, vec2<i32>(t1), li).r;
    return zmax < min(min(a, b), min(c, d));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= p.count) {
        return;
    }
    let c = cols[i];
    var lo = c.lo.xyz;
    if (p.shadow == 0u) {
        // Curvatura de la Tierra: la geometría se dibuja hasta d²/2R más abajo (ver curved_box
        // en render/mod.rs); el pase de sombras no la aplica.
        let dx = max(abs(p.eye.x - c.lo.x), abs(p.eye.x - c.hi.x));
        let dz = max(abs(p.eye.z - c.lo.z), abs(p.eye.z - c.hi.z));
        lo.y -= (dx * dx + dz * dz) / 12742000.0;
    }
    // hi.w: 0 = libre, 1 = estable, 2 = entrando, 3 = saliendo (fundido entre niveles, que se
    // dibuja aparte). La vista principal y el Hi-Z, solo las estables; las sombras, también las
    // que entran (las que salen ya no tiran sombra).
    let estado = c.hi.w;
    var vis = estado > 0.5 && estado < select(1.5, 2.5, p.shadow == 1u) && box_visible(lo, c.hi.xyz);
    if (p.shadow == 1u && u32(c.lo.w) > p.max_lod) {
        vis = false;
    }
    if (p.phase == 1u) {
        vis = vis && visible[i] != 0u;
    } else if (p.phase == 2u) {
        let was = visible[i] != 0u;
        let now = vis && !occluded(lo, c.hi.xyz);
        visible[i] = select(0u, 1u, now);
        vis = now && !was;
    }
    let e = p.eye.xyz;
    // Mismo orden que mesher::DIRS: +y, +x, -x, +z, -z, -y. Vista principal: caras que miran a
    // la cámara; sombras: caras que miran al sol (eye = dirección del sol).
    var facing = array<bool, 6>(e.y > c.lo.y, e.x > c.lo.x, e.x < c.hi.x, e.z > c.lo.z, e.z < c.hi.z, e.y < c.hi.y);
    if (p.shadow == 1u) {
        facing = array<bool, 6>(e.y > 0.0, e.x > 0.0, e.x < 0.0, e.z > 0.0, e.z < 0.0, e.y < 0.0);
    }
    var g = c.g;
    var f = facing;
    for (var d = 0u; d < 6u; d++) {
        let n = g[d + 1u] - g[d];
        let show = vis && n > 0u && f[d];
        args[i * 6u + d] = Args(6u, select(0u, n, show), 0u, 0i, c.start + g[d]);
    }
}
