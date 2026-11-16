// Descarte en la GPU: una invocación por columna. Prueba la caja de la columna contra el
// volumen de visión (y, en la vista principal, qué direcciones de cara miran a la cámara) y
// escribe los argumentos de dibujo indirecto de sus 6 grupos de caras. Las que no se ven
// quedan con 0 instancias. Es el ítem V4 pendiente de Tisma engine (PLAN.md): llenar
// `draw_indirect` desde un compute shader.
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
    _pad: u32,
};
@group(0) @binding(0) var<storage, read> cols: array<Col>;
@group(0) @binding(1) var<storage, read_write> args: array<Args>;
@group(0) @binding(2) var<uniform> p: Params;

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

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= p.count) {
        return;
    }
    let c = cols[i];
    var vis = c.hi.w > 0.5 && box_visible(c.lo.xyz, c.hi.xyz);
    if (p.shadow == 1u && u32(c.lo.w) > p.max_lod) {
        vis = false;
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
