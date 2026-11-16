//! Niveles de detalle tipo *clipmap*: anillos concéntricos alrededor de la cámara.
//! El nivel 0 usa vóxeles de 0,5 m; cada nivel siguiente duplica el tamaño del vóxel
//! (1, 2, 4, 8 m…) y cubre el doble de distancia. Con 8 niveles se cubren los 24 km del mapa.

use glam::{IVec2, Vec2};

pub const CHUNK: i32 = 32;
pub const BASE_VOXEL: f32 = 0.5;
/// Lado de cada anillo, en chunks (par y con mitad par para alinear con el nivel siguiente).
pub const RING_CHUNKS: i32 = 24;

#[derive(Clone, Copy, Debug)]
pub struct LevelSpec {
    pub lod: u32,
}

impl LevelSpec {
    pub fn voxel(&self) -> f32 {
        BASE_VOXEL * (1u32 << self.lod) as f32
    }
    pub fn chunk_m(&self) -> f32 {
        self.voxel() * CHUNK as f32
    }
    /// Región del anillo en coordenadas de chunk de este nivel: [min, max).
    pub fn region(&self, center: Vec2) -> (IVec2, IVec2) {
        let c2 = self.chunk_m() * 2.0;
        let snapped = IVec2::new((center.x / c2).round() as i32 * 2, (center.y / c2).round() as i32 * 2);
        let min = snapped - IVec2::splat(RING_CHUNKS / 2);
        (min, min + IVec2::splat(RING_CHUNKS))
    }
}

/// Columnas de chunks que le tocan a cada nivel: dentro de su anillo y fuera del anillo
/// del nivel más fino.
pub fn level_columns(levels: &[LevelSpec], center: Vec2) -> Vec<Vec<IVec2>> {
    let mut out = Vec::new();
    for (i, spec) in levels.iter().enumerate() {
        let (min, max) = spec.region(center);
        let inner = if i > 0 {
            let (imin, imax) = levels[i - 1].region(center);
            Some((imin / 2, imax / 2))
        } else {
            None
        };
        let mut cols = Vec::new();
        for z in min.y..max.y {
            for x in min.x..max.x {
                if let Some((imin, imax)) = inner {
                    if x >= imin.x && x < imax.x && z >= imin.y && z < imax.y {
                        continue;
                    }
                }
                cols.push(IVec2::new(x, z));
            }
        }
        out.push(cols);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_anillos_no_se_superponen_y_cubren_el_mapa() {
        let levels: Vec<LevelSpec> = (0..8).map(|lod| LevelSpec { lod }).collect();
        let center = Vec2::new(13500.0, 16300.0);
        let cols = level_columns(&levels, center);
        // Cada punto de una grilla de prueba debe pertenecer a exactamente un nivel.
        for gz in (0..24000).step_by(397) {
            for gx in (0..24000).step_by(397) {
                let p = Vec2::new(gx as f32, gz as f32);
                let mut hits = 0;
                for (i, spec) in levels.iter().enumerate() {
                    let c = IVec2::new((p.x / spec.chunk_m()).floor() as i32, (p.y / spec.chunk_m()).floor() as i32);
                    if cols[i].contains(&c) {
                        hits += 1;
                    }
                }
                assert_eq!(hits, 1, "el punto {p:?} está en {hits} niveles");
            }
        }
    }
}
