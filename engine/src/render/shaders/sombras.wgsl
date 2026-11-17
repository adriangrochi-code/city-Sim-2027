// Sombras del sol en cascadas (ver render/cascades.rs). Este archivo se antepone a voxel.wgsl,
// detail.wgsl y agentes.wgsl (render/mod.rs), que declaran `g` (globales, con `sun_dir`).
//
// Cada cascada es una capa del arreglo de mapas. Se usa la más fina que contiene el punto; en
// el borde (último 10 % de su lado) se mezcla con la siguiente para que no se vea el salto, y
// la última se desvanece hacia "sin sombra" en vez de cortar de golpe.
// Sesgo en metros, proporcional al texel de cada cascada: corrimiento a lo largo de la normal
// y sesgo de profundidad que crece con lo rasante de la luz (más texels de pendiente).
struct Sombras {
    vp: array<mat4x4<f32>, 4>,
    // x: lado del texel (m), y: 1 / rango de profundidad (por m), z: 1 si la cascada está activa
    info: array<vec4<f32>, 4>,
    // x: cantidad de cascadas, y: 1 / resolución, z: 1 = teñir cada cascada (depuración)
    misc: vec4<f32>,
};
@group(0) @binding(1) var shadow_map: texture_depth_2d_array;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;
@group(0) @binding(5) var<uniform> sombras: Sombras;

// PCF de 4 lecturas a ±½ texel: con el muestreador de comparación lineal (2×2 cada una)
// cubre 3×3 texels con pesos en carpa, como las 9 lecturas de antes pero con menos de la mitad.
fn shadow_pcf(i: u32, uv: vec2<f32>, z: f32) -> f32 {
    let h = sombras.misc.y * 0.5;
    var sum = textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2(-h, -h), i, z);
    sum += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2(h, -h), i, z);
    sum += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2(-h, h), i, z);
    sum += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2(h, h), i, z);
    return sum * 0.25;
}

// Sombra de la cascada i en `world` (normal `n`): x = luz (0..1), y = distancia al borde
// (0 en el centro, 1 en el borde), z = 1 si el punto cae adentro.
fn shadow_in(i: u32, world: vec3<f32>, n: vec3<f32>, slope: f32) -> vec3<f32> {
    let info = sombras.info[i];
    if (info.z < 0.5) {
        return vec3(1.0, 1.0, 0.0);
    }
    let lp = sombras.vp[i] * vec4(world + n * info.x * 1.5, 1.0);
    let e = max(abs(lp.x), abs(lp.y));
    let margin = 1.0 - 4.0 * sombras.misc.y;
    if (e >= margin || lp.z <= 0.0 || lp.z >= 1.0) {
        return vec3(1.0, 1.0, 0.0);
    }
    let uv = vec2(lp.x * 0.5 + 0.5, 0.5 - lp.y * 0.5);
    let bias = info.x * slope * info.y;
    return vec3(shadow_pcf(i, uv, lp.z - bias), e / margin, 1.0);
}

fn shadow(world: vec3<f32>, n: vec3<f32>) -> f32 {
    let ndl = clamp(dot(n, g.sun_dir.xyz), 0.05, 1.0);
    // Texels de sesgo: 1 de frente al sol, hasta 6 con luz rasante (tan del ángulo).
    let slope = min(1.0 + 2.0 * sqrt(1.0 - ndl * ndl) / ndl, 6.0);
    let count = u32(sombras.misc.x);
    for (var i = 0u; i < count; i++) {
        let a = shadow_in(i, world, n, slope);
        if (a.z < 0.5) {
            continue;
        }
        let blend = smoothstep(0.9, 1.0, a.y);
        if (blend <= 0.0) {
            return a.x;
        }
        // Borde de la cascada: se mezcla con la siguiente (o, en la última, con "sin sombra").
        var b = 1.0;
        if (i + 1u < count) {
            let nb = shadow_in(i + 1u, world, n, slope);
            b = select(a.x, nb.x, nb.z > 0.5);
        }
        return mix(a.x, b, blend);
    }
    return 1.0;
}

// Depuración (`MOTOR_DEPURAR_SOMBRAS=1`): tinte según la cascada que cubre el punto (roja,
// verde, azul, amarilla; magenta fuera de todas).
fn shadow_debug_tint(world: vec3<f32>) -> vec3<f32> {
    if (sombras.misc.z < 0.5) {
        return vec3(1.0);
    }
    let colors = array<vec3<f32>, 4>(vec3(1.0, 0.6, 0.6), vec3(0.6, 1.0, 0.6), vec3(0.6, 0.6, 1.0), vec3(1.0, 1.0, 0.5));
    for (var i = 0u; i < u32(sombras.misc.x); i++) {
        let lp = sombras.vp[i] * vec4(world, 1.0);
        if (sombras.info[i].z > 0.5 && max(abs(lp.x), abs(lp.y)) < 1.0 && lp.z > 0.0 && lp.z < 1.0) {
            var c = colors;
            return c[i];
        }
    }
    return vec3(1.0, 0.6, 1.0);
}
