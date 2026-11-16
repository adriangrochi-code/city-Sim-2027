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
    level_columns_from(levels, center, 0)
}

/// Igual, pero los niveles más finos que `first` no se usan: el nivel `first` cubre también
/// el centro (LOD por tamaño en pantalla, ver [`choose_first_level`]).
pub fn level_columns_from(levels: &[LevelSpec], center: Vec2, first: u32) -> Vec<Vec<IVec2>> {
    let mut out = Vec::new();
    for (i, spec) in levels.iter().enumerate() {
        if (i as u32) < first {
            out.push(Vec::new());
            continue;
        }
        let (min, max) = spec.region(center);
        let inner = if i as u32 > first {
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

/// Tamaño en píxeles de un vóxel del nivel `lod` visto desde `eye_height` metros (el punto
/// del terreno más cercano a la cámara), con un campo de visión vertical `fov_y` (grados) y
/// una imagen de `view_h` píxeles de alto.
pub fn voxel_pixels(lod: u32, eye_height: f32, fov_y: f32, view_h: f32) -> f32 {
    let v = BASE_VOXEL * (1u32 << lod) as f32;
    v * view_h / (2.0 * eye_height.max(1.0) * (fov_y.to_radians() * 0.5).tan())
}

/// Elige el nivel más fino que vale la pena generar. Criterio de Tisma engine
/// (`far_group()` / `merged_layers()` en src/ui/scene.cpp): no hace falta un detalle que
/// mida menos de medio píxel. Con histéresis, para no ir y volver al acercar y alejar:
/// se engrosa solo si el nivel actual baja de 0,45 px y se afina si el más fino pasa de 0,55 px.
pub fn choose_first_level(current: u32, eye_height: f32, fov_y: f32, view_h: f32, levels: u32) -> u32 {
    let px = |l: u32| voxel_pixels(l, eye_height, fov_y, view_h);
    let mut first = current.min(levels.saturating_sub(1));
    while first + 1 < levels && px(first) < 0.45 {
        first += 1;
    }
    while first > 0 && px(first - 1) >= 0.55 {
        first -= 1;
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nivel_mas_fino_depende_de_la_altura_con_histeresis() {
        // De cerca se usa el nivel 0; a 4 km de altura, un vóxel de 0,5 m es una fracción de píxel.
        assert_eq!(choose_first_level(0, 80.0, 50.0, 720.0, 8), 0);
        let far = choose_first_level(0, 4500.0, 50.0, 720.0, 8);
        assert!(far >= 3, "desde 4,5 km el nivel más fino debería ser al menos 3 (es {far})");
        assert!(voxel_pixels(far, 4500.0, 50.0, 720.0) >= 0.45);
        // Histéresis: justo en el borde no cambia de nivel en ninguno de los dos sentidos.
        let edge = 0.5 * 720.0 / (2.0 * 0.5 * (25f32).to_radians().tan()); // altura donde el nivel 0 mide 0,5 px
        assert_eq!(choose_first_level(0, edge * 1.05, 50.0, 720.0, 8), 0);
        assert_eq!(choose_first_level(1, edge * 0.95, 50.0, 720.0, 8), 1);
        assert_eq!(choose_first_level(1, edge * 0.8, 50.0, 720.0, 8), 0);
    }

    #[test]
    fn empezando_en_otro_nivel_se_cubre_todo_igual() {
        let levels: Vec<LevelSpec> = (0..8).map(|lod| LevelSpec { lod }).collect();
        let center = Vec2::new(13500.0, 16300.0);
        let cols = level_columns_from(&levels, center, 3);
        assert!(cols[0].is_empty() && cols[1].is_empty() && cols[2].is_empty());
        for gz in (0..24000).step_by(397) {
            for gx in (0..24000).step_by(397) {
                let p = Vec2::new(gx as f32, gz as f32);
                let hits = levels
                    .iter()
                    .enumerate()
                    .filter(|(i, spec)| cols[*i].contains(&IVec2::new((p.x / spec.chunk_m()).floor() as i32, (p.y / spec.chunk_m()).floor() as i32)))
                    .count();
                assert_eq!(hits, 1, "el punto {p:?} está en {hits} niveles");
            }
        }
    }

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
