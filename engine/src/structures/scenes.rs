//! Escenas de muestra: la ciudad colonial y el catálogo de estructuras.

use super::colonial::*;
use super::*;
use crate::buildings::{plaza_block, vegetation, vox};
use glam::Vec2;

/// Resultado de una escena: ediciones y una cámara sugerida (x, z, distancia, giro, inclinación).
pub struct SceneOut {
    pub edits: Edits,
    pub cam: [f32; 5],
    pub center: Vec2,
}

fn land(t: &TerrainGen, vx: i32, vz: i32) -> bool {
    t.height((vx as f32 + 0.5) * BASE_VOXEL, (vz as f32 + 0.5) * BASE_VOXEL) > 1.0
}

use crate::lod::BASE_VOXEL;

/// Crea un marco apoyado en el terreno: la base queda sobre el punto más alto del contorno.
pub fn place(t: &TerrainGen, origin: IVec2, du: IVec2, dw: IVec2, width: i32, depth: i32, seed: u64) -> Frame {
    let mut base = i32::MIN;
    let step = 6;
    let mut w = 0;
    while w <= depth.min(48) {
        let mut u = 0;
        while u <= width {
            let p = origin + du * u.min(width - 1) + dw * w.min(depth - 1);
            base = base.max(top_at(t, p.x, p.y) + 1);
            u += step;
        }
        w += step;
    }
    Frame { origin, du, dw, base, width, depth, seed }
}

fn cobble_line(t: &TerrainGen, e: &mut Edits, a: IVec2, b: IVec2, half: i32, m: Mat) {
    let (x0, x1) = (a.x.min(b.x) - half, a.x.max(b.x) + half);
    let (z0, z1) = (a.y.min(b.y) - half, a.y.max(b.y) + half);
    for z in z0..z1 {
        for x in x0..x1 {
            if land(t, x, z) {
                e.surface.insert(IVec2::new(x, z), m);
            }
        }
    }
}

/// Ciudad colonial: damero de cuadras de 84 m alrededor de la plaza, iglesia matriz, cabildo,
/// pulperías en las esquinas, casas de los tres niveles, muralla con portón al norte y muelle
/// con barraca en la costa oeste.
pub fn ciudad(t: &TerrainGen, plaza: Vec2, blocks: i32, seed: u64) -> SceneOut {
    let mut e = Edits::default();
    let mut r = Rng::new(seed);
    let block = 84.0;
    let road = 8.0;
    let half = block * (blocks as f32 + 0.5);
    let x0 = plaza.x - half;
    let z0 = plaza.y - half;
    let n_lines = blocks * 2 + 2;
    // Calles empedradas.
    for k in 0..n_lines {
        let line = -half + block * k as f32;
        for a in vox(-half - road * 0.5)..vox(half + road * 0.5) {
            for b in vox(line - road * 0.5)..vox(line + road * 0.5) {
                for (px, pz) in [(vox(plaza.x) + a, vox(plaza.y) + b), (vox(plaza.x) + b, vox(plaza.y) + a)] {
                    if t.height((px as f32 + 0.5) * BASE_VOXEL, (pz as f32 + 0.5) * BASE_VOXEL) > 0.6 {
                        e.surface.insert(IVec2::new(px, pz), COBBLE);
                    }
                }
            }
        }
    }
    let inner = block - road;
    let side = vox(inner);
    let mid = blocks;
    for bz in 0..(n_lines - 1) {
        for bx in 0..(n_lines - 1) {
            let mx = vox(x0 + block * bx as f32 + road * 0.5);
            let mz = vox(z0 + block * bz as f32 + road * 0.5);
            let center = Vec2::new((mx + side / 2) as f32 * BASE_VOXEL, (mz + side / 2) as f32 * BASE_VOXEL);
            if bx == mid && bz == mid {
                plaza_block(t, &mut e, mx, mz, side);
                continue;
            }
            if bx == mid && bz == mid - 1 {
                let f = place(t, IVec2::new(mx + side / 2 - 15, mz + side - 1), IVec2::X, IVec2::NEG_Y, 30, 84, r.next());
                iglesia(t, &mut e, &f, 5);
                continue;
            }
            let dist = center.distance(plaza);
            // Solares: (origen, du, dw, ¿esquina a la izquierda?, ¿esquina a la derecha?)
            let mut lots = Vec::new();
            let per = side / LOT_W;
            let cabildo_block = bx == mid && bz == mid + 1;
            if cabildo_block {
                let f = place(t, IVec2::new(mx + side / 2 - 30, mz), IVec2::X, IVec2::Y, 60, 36, r.next());
                cabildo(t, &mut e, &f);
            }
            for k in 0..per {
                if !cabildo_block {
                    lots.push((IVec2::new(mx + k * LOT_W, mz), IVec2::X, IVec2::Y, k == 0, k == per - 1));
                }
                lots.push((IVec2::new(mx + (k + 1) * LOT_W - 1, mz + side - 1), IVec2::NEG_X, IVec2::NEG_Y, k == per - 1, k == 0));
            }
            for k in 0..2 {
                lots.push((IVec2::new(mx, mz + LOT_D + 4 + (k + 1) * LOT_W - 1 - 4), IVec2::NEG_Y, IVec2::X, false, false));
                lots.push((IVec2::new(mx + side - 1, mz + LOT_D + k * LOT_W), IVec2::Y, IVec2::NEG_X, false, false));
            }
            for (origin, du, dw, corner_l, corner_r) in lots {
                let corners = [origin, origin + du * (LOT_W - 1), origin + dw * (LOT_D - 1), origin + du * (LOT_W - 1) + dw * (LOT_D - 1)];
                if !corners.iter().all(|p| land(t, p.x, p.y)) {
                    continue;
                }
                let f = place(t, origin, du, dw, LOT_W, LOT_D, r.next());
                let x = r.f();
                if (corner_l || corner_r) && dist < 330.0 && x < 0.45 {
                    pulperia(t, &mut e, &f, corner_l);
                    continue;
                }
                let kind = if dist < 220.0 {
                    if x < 0.35 { 3 } else if x < 0.85 { 2 } else { 1 }
                } else if dist < 360.0 {
                    if x < 0.08 { 3 } else if x < 0.55 { 2 } else if x < 0.92 { 1 } else { 0 }
                } else if x < 0.15 { 2 } else if x < 0.7 { 1 } else { 0 };
                match kind {
                    3 => casa_altos(t, &mut e, &f),
                    2 => casa_azotea(t, &mut e, &f),
                    1 => rancho(t, &mut e, &f),
                    _ => {}
                }
            }
        }
    }
    // Calle hacia el norte, muralla de costa a costa y portón.
    let north = vox(plaza.y - half - road * 0.5);
    let wall_z = north - vox(36.0);
    let cx = vox(plaza.x);
    cobble_line(t, &mut e, IVec2::new(cx, north), IVec2::new(cx, wall_z - vox(120.0)), vox(road * 0.5), DIRT_ROAD);
    let seg = 60;
    let gate = place(t, IVec2::new(cx - seg / 2, wall_z), IVec2::X, IVec2::NEG_Y, seg, 12, r.next());
    muralla(t, &mut e, &gate, true, false);
    for dir in [1, -1] {
        let mut k = 1;
        loop {
            let u0 = if dir > 0 { cx + seg / 2 + (k - 1) * seg } else { cx - seg / 2 - k * seg };
            let ends_on_land = land(t, u0, wall_z - 4) && land(t, u0 + seg - 1, wall_z - 4);
            let f = place(t, IVec2::new(u0, wall_z), IVec2::X, IVec2::NEG_Y, seg, 12, r.next());
            if !ends_on_land {
                break;
            }
            let last = !land(t, u0 + dir * seg, wall_z - 4) || !land(t, u0 + seg - 1 + dir * seg, wall_z - 4);
            muralla(t, &mut e, &f, false, last && dir < 0);
            k += 1;
            if k > 20 {
                break;
            }
        }
    }
    // Muelle y barraca en la costa oeste, a la altura de la plaza.
    let mut sx = vox(plaza.x - half);
    while land(t, sx - 1, vox(plaza.y)) && sx > vox(plaza.x - half - 600.0) {
        sx -= 1;
    }
    let pier = Frame { origin: IVec2::new(sx + 6, vox(plaza.y) - 6), du: IVec2::Y, dw: IVec2::NEG_X, base: 0, width: 12, depth: 90, seed: r.next() };
    muelle(t, &mut e, &pier);
    let bar = place(t, IVec2::new(sx + 14, vox(plaza.y) - 16), IVec2::Y, IVec2::X, 32, 20, r.next());
    barraca(t, &mut e, &bar);
    cobble_line(t, &mut e, IVec2::new(sx + 6, vox(plaza.y)), IVec2::new(vox(plaza.x - half), vox(plaza.y)), 6, DIRT_ROAD);
    vegetation(t, &mut e, plaza, 1800.0, half + 20.0, 11);
    SceneOut { edits: e, cam: [plaza.x, plaza.y, 420.0, 200.0, 38.0], center: plaza }
}

/// Catálogo: todas las estructuras coloniales en una exhibición junto a una calle de muestra,
/// en tierra firme al este de la bahía.
pub fn catalogo(t: &TerrainGen) -> SceneOut {
    let mut e = Edits::default();
    let street_z = vox(15000.0);
    let x0 = vox(13400.0);
    cobble_line(t, &mut e, IVec2::new(x0 - 10, street_z), IVec2::new(x0 + 760, street_z), 8, COBBLE);
    // Fila norte (casas, frente a la calle): el fondo va hacia el norte.
    let row_a = street_z - 10;
    let mut x = x0;
    let mut seed = 100u64;
    let mut next = |w: i32, x: &mut i32| {
        let o = *x;
        *x += w + 4;
        seed += 1;
        (o, seed)
    };
    let (o, s) = next(LOT_W, &mut x);
    rancho(t, &mut e, &place(t, IVec2::new(o, row_a), IVec2::X, IVec2::NEG_Y, LOT_W, LOT_D, s));
    for _ in 0..2 {
        let (o, s) = next(LOT_W, &mut x);
        casa_azotea(t, &mut e, &place(t, IVec2::new(o, row_a), IVec2::X, IVec2::NEG_Y, LOT_W, LOT_D, s));
    }
    for _ in 0..2 {
        let (o, s) = next(LOT_W, &mut x);
        casa_altos(t, &mut e, &place(t, IVec2::new(o, row_a), IVec2::X, IVec2::NEG_Y, LOT_W, LOT_D, s));
    }
    let (o, s) = next(LOT_W, &mut x);
    pulperia(t, &mut e, &place(t, IVec2::new(o, row_a), IVec2::X, IVec2::NEG_Y, LOT_W, LOT_D, s), true);
    let (o, s) = next(LOT_W, &mut x);
    pulperia(t, &mut e, &place(t, IVec2::new(o, row_a), IVec2::X, IVec2::NEG_Y, LOT_W, LOT_D, s), false);
    let (o, s) = next(60, &mut x);
    cabildo(t, &mut e, &place(t, IVec2::new(o, row_a), IVec2::X, IVec2::NEG_Y, 60, 36, s));
    // Misma fila: la iglesia en sus cinco fases de obra y la muralla con portón y garita.
    for phase in 1..=5u32 {
        let (o, _) = next(46, &mut x);
        iglesia(t, &mut e, &place(t, IVec2::new(o + 6, row_a), IVec2::X, IVec2::NEG_Y, 30, 84, 500 + phase as u64), phase);
    }
    let (mw, _) = next(120, &mut x);
    muralla(t, &mut e, &place(t, IVec2::new(mw, row_a), IVec2::X, IVec2::NEG_Y, 120, 12, 7), true, true);
    let row_b = street_z + 10;
    // Más al norte: campo y producción.
    let row_c = row_a - 84 - 24;
    saladero(t, &mut e, &place(t, IVec2::new(x0, row_c - 90), IVec2::X, IVec2::Y, 120, 90, 31));
    estancia(t, &mut e, &place(t, IVec2::new(x0 + 130, row_c - 90), IVec2::X, IVec2::Y, 100, 90, 32));
    atahona(t, &mut e, &place(t, IVec2::new(x0 + 240, row_c - 50), IVec2::X, IVec2::Y, 40, 40, 33));
    molino(t, &mut e, &place(t, IVec2::new(x0 + 290, row_c - 40), IVec2::X, IVec2::Y, 22, 24, 34));
    // Muelle y barraca en la costa sur (el río), con su propio camino corto.
    let px = x0 + 300;
    let mut pz = row_b;
    while land(t, px, pz + 1) && pz < row_b + 3000 {
        pz += 1;
    }
    let pier = Frame { origin: IVec2::new(px, pz - 8), du: IVec2::X, dw: IVec2::Y, base: 0, width: 12, depth: 90, seed: 41 };
    muelle(t, &mut e, &pier);
    barraca(t, &mut e, &place(t, IVec2::new(px - 10, pz - 22), IVec2::X, IVec2::NEG_Y, 32, 20, 42));
    cobble_line(t, &mut e, IVec2::new(px - 12, pz - 16), IVec2::new(px + 24, pz - 12), 3, DIRT_ROAD);
    let center = Vec2::new((x0 + 370) as f32 * BASE_VOXEL, street_z as f32 * BASE_VOXEL);
    vegetation(t, &mut e, center, 900.0, 260.0, 12);
    SceneOut { edits: e, cam: [center.x, center.y - 40.0, 330.0, 15.0, 36.0], center }
}
