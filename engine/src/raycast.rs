//! Selección con el ratón: un rayo desde la cámara contra los vóxeles del nivel 0.
//!
//! Primero se avanza a pasos largos sobre el campo de alturas del terreno (barato) hasta
//! quedar a pocos metros por encima de lo más alto que podría haber (edificios); desde ahí se
//! recorre vóxel a vóxel con DDA (Amanatides–Woo), consultando las ediciones y el terreno.

use crate::lod::BASE_VOXEL;
use crate::palette::AIR;
use crate::store::LevelStore;
use crate::stream::terrain_top_l0;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec3};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub voxel: IVec3,
    /// Normal de la cara tocada (hacia afuera).
    pub normal: IVec3,
    pub point: Vec3,
}

/// Altura máxima de lo construido sobre el terreno que se tiene en cuenta (m).
const HEADROOM: f32 = 45.0;

pub fn solid(t: &TerrainGen, l0: &LevelStore, v: IVec3, cache: &mut HashMap<IVec2, i32>) -> bool {
    if let Some(m) = l0.get(v) {
        return m != AIR;
    }
    let top = *cache.entry(IVec2::new(v.x, v.z)).or_insert_with(|| terrain_top_l0(t, v.x, v.z));
    v.y <= top
}

pub fn raycast(t: &TerrainGen, l0: &LevelStore, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<Hit> {
    let dir = dir.normalize();
    // 1. Pasos largos sobre el campo de alturas.
    let mut s = 0.0f32;
    loop {
        let p = origin + dir * s;
        let ground = t.height(p.x, p.z).max(0.0);
        let clearance = p.y - ground - HEADROOM;
        if clearance <= 0.0 {
            break;
        }
        s += (clearance * 0.5).max(1.0);
        if s > max_dist {
            return None;
        }
    }
    let start = (s - 2.0).max(0.0);
    dda(t, l0, origin + dir * start, dir, (max_dist - start).min(900.0), start, origin)
}

#[allow(clippy::too_many_arguments)]
fn dda(t: &TerrainGen, l0: &LevelStore, from: Vec3, dir: Vec3, max_len: f32, offset: f32, origin: Vec3) -> Option<Hit> {
    let vs = BASE_VOXEL;
    let p = from / vs;
    let mut v = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let step = IVec3::new(dir.x.signum() as i32, dir.y.signum() as i32, dir.z.signum() as i32);
    let inv = Vec3::new(1.0 / dir.x.abs().max(1e-9), 1.0 / dir.y.abs().max(1e-9), 1.0 / dir.z.abs().max(1e-9));
    let frac = |pc: f32, vc: i32, st: i32| if st > 0 { (vc as f32 + 1.0) - pc } else { pc - vc as f32 };
    let mut tmax = Vec3::new(frac(p.x, v.x, step.x) * inv.x, frac(p.y, v.y, step.y) * inv.y, frac(p.z, v.z, step.z) * inv.z);
    let tdelta = inv;
    let mut normal = IVec3::ZERO;
    let mut cache = HashMap::new();
    let mut tcur = 0.0f32;
    let max_t = max_len / vs;
    while tcur <= max_t {
        if solid(t, l0, v, &mut cache) {
            let point = origin + dir * (offset + tcur * vs);
            return Some(Hit { voxel: v, normal, point });
        }
        // Debajo del agua y del lecho: no hay nada que tocar.
        if (v.y as f32) * vs < -30.0 {
            return None;
        }
        if tmax.x < tmax.y && tmax.x < tmax.z {
            v.x += step.x;
            tcur = tmax.x;
            tmax.x += tdelta.x;
            normal = IVec3::new(-step.x, 0, 0);
        } else if tmax.y < tmax.z {
            v.y += step.y;
            tcur = tmax.y;
            tmax.y += tdelta.y;
            normal = IVec3::new(0, -step.y, 0);
        } else {
            v.z += step.z;
            tcur = tmax.z;
            tmax.z += tdelta.z;
            normal = IVec3::new(0, 0, -step.z);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildings::Edits;
    use crate::palette::CAL;
    use crate::store::EditStore;

    #[test]
    fn el_rayo_vertical_toca_el_terreno_y_los_edificios() {
        let t = TerrainGen::new(1726);
        let mut s = EditStore::new(1);
        let (x, z) = (27000, 32600);
        let top = terrain_top_l0(&t, x, z);
        let o = Vec3::new((x as f32 + 0.5) * 0.5, 400.0, (z as f32 + 0.5) * 0.5);
        let hit = raycast(&t, &s.levels[0], o, Vec3::NEG_Y, 2000.0).expect("debe tocar el suelo");
        assert_eq!(hit.voxel, IVec3::new(x, top, z));
        assert_eq!(hit.normal, IVec3::Y);
        // Un bloque de 10 vóxeles de alto encima: ahora toca el techo del bloque.
        let mut e = Edits::default();
        e.fill(IVec3::new(x - 2, top + 1, z - 2), IVec3::new(x + 3, top + 11, z + 3), CAL);
        s.apply(&e, false);
        let hit = raycast(&t, &s.levels[0], o, Vec3::NEG_Y, 2000.0).unwrap();
        assert_eq!(hit.voxel, IVec3::new(x, top + 10, z));
        // Un rayo inclinado contra la pared del bloque toca su cara lateral.
        let o2 = Vec3::new((x as f32 - 20.0) * 0.5, (top as f32 + 5.5) * 0.5, (z as f32 + 0.5) * 0.5);
        let hit = raycast(&t, &s.levels[0], o2, Vec3::X, 100.0).unwrap();
        assert_eq!(hit.voxel, IVec3::new(x - 2, top + 5, z));
        assert_eq!(hit.normal, IVec3::NEG_X);
    }
}
