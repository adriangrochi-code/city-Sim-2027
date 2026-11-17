// Pirámide de profundidad (Hi-Z) con Z invertida: cada texel guarda la profundidad MÁS LEJANA
// (el mínimo) de lo que cubre. Ver render/hiz.rs (referencia en la CPU y pruebas).
//
// `SRC_TYPE` y `SAMPLES` los reemplaza render/mod.rs: con MSAA se lee la profundidad
// multimuestreada (mínimo de todas las muestras, así un borde de un objeto que cubre solo
// algunas muestras no "tapa" lo de atrás); sin MSAA, la profundidad común.
@group(0) @binding(0) var src: SRC_TYPE;
@group(0) @binding(1) var dst: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var prev: texture_2d<f32>;

// Nivel 0: mitad de la pantalla, mínimo de 2×2 píxeles × muestras.
@compute @workgroup_size(8, 8)
fn reduce(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = textureDimensions(dst);
    if (id.x >= d.x || id.y >= d.y) {
        return;
    }
    let s = textureDimensions(src);
    let ns = SAMPLES;
    var m = 1.0;
    for (var y = id.y * 2u; y < min(id.y * 2u + 2u, s.y); y++) {
        for (var x = id.x * 2u; x < min(id.x * 2u + 2u, s.x); x++) {
            for (var k = 0u; k < ns; k++) {
                m = min(m, textureLoad(src, vec2<i32>(i32(x), i32(y)), i32(k)));
            }
        }
    }
    textureStore(dst, id.xy, vec4(m, 0.0, 0.0, 0.0));
}

// Niveles siguientes (tamaños de wgpu: ⌊anterior/2⌋): mínimo de 2×2 texels del nivel anterior;
// si el anterior es impar, el último texel lee también el que sobra.
@compute @workgroup_size(8, 8)
fn down(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = textureDimensions(dst);
    if (id.x >= d.x || id.y >= d.y) {
        return;
    }
    let s = textureDimensions(prev);
    let ex = select(min(id.x * 2u + 2u, s.x), s.x, id.x + 1u == d.x);
    let ey = select(min(id.y * 2u + 2u, s.y), s.y, id.y + 1u == d.y);
    var m = 1.0;
    for (var y = id.y * 2u; y < ey; y++) {
        for (var x = id.x * 2u; x < ex; x++) {
            m = min(m, textureLoad(prev, vec2<i32>(i32(x), i32(y)), 0).r);
        }
    }
    textureStore(dst, id.xy, vec4(m, 0.0, 0.0, 0.0));
}
