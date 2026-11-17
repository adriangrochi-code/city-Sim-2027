//! Niveles de detalle tipo *clipmap*: anillos concéntricos alrededor de la cámara.
//! El nivel 0 usa vóxeles de 0,25 m; cada nivel siguiente duplica el tamaño del vóxel
//! (0,5, 1, 2, 4 m…) y cubre el doble de distancia. Con 9 niveles se cubren los 24 km del
//! mapa (el noveno, de 64 m por vóxel, tiene un anillo de 49 km); con los 11 de [`LEVELS`]
//! (el último, de 256 m por vóxel, con un anillo de 197 km) se ve el entorno amplio hasta el
//! horizonte: Colonia, Florida, Maldonado y la costa argentina (ver `terrain`, `amplio.bin`).
//!
//! Los edificios se diseñan en una grilla más gruesa, de [`DESIGN_VOXEL`] (0,5 m: puerta de
//! 3 × 6 vóxeles, solar de 24 × 48): cada vóxel de diseño ocupa un bloque de
//! [`DESIGN_SCALE`]³ vóxeles base (ver `buildings::Edits::design`).

use glam::{IVec2, Vec2};

pub const CHUNK: i32 = 32;
pub const BASE_VOXEL: f32 = 0.25;
/// Niveles de detalle por omisión (distancia de dibujo "larga": el entorno amplio, ~90 km
/// alrededor de la cámara).
pub const LEVELS: u32 = 11;
/// Niveles que bastan para ver todo el mapa jugable (24 km): la distancia "corta".
pub const MAP_LEVELS: u32 = 9;
/// Desde este nivel (vóxel de 64 m) la malla del terreno se baja casi un vóxel entero (ver
/// [`far_drop`]).
pub const DROP_LOD: u32 = 8;

/// Cuánto se baja la malla de un nivel grueso (m). Con vóxeles de 64 a 256 m, la regla de que
/// la tierra firme quede un vóxel entero sobre el agua (`stream::top_from_height`) convertía
/// cada costa en un acantilado de 64 a 256 m. Bajando la malla `vóxel − 3 m`, el primer vóxel
/// de tierra queda 3 m sobre el agua (por encima de la marea) y el fondo, bajo ella; las
/// alturas se redondean al vóxel más cercano (`stream::top_from_height_far`).
pub fn far_drop(lod: u32) -> f32 {
    if lod >= DROP_LOD {
        BASE_VOXEL * (1u32 << lod) as f32 - 3.0
    } else {
        0.0
    }
}
/// Vóxeles base por lado de un vóxel de diseño, en potencia de dos (1 → 2 × 2 × 2).
pub const DESIGN_SHIFT: u32 = 1;
pub const DESIGN_SCALE: i32 = 1 << DESIGN_SHIFT;
/// Lado del vóxel de diseño de los edificios (m).
pub const DESIGN_VOXEL: f32 = BASE_VOXEL * DESIGN_SCALE as f32;
// El almacén de ediciones expande el diseño solo en el nivel 0 (ver `store`).
const _: () = assert!(DESIGN_SHIFT <= 1);
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
        // De cerca se usa el nivel 0; a 4 km de altura, un vóxel de 0,25 m es una fracción de píxel.
        assert_eq!(choose_first_level(0, 80.0, 50.0, 720.0, LEVELS), 0);
        let far = choose_first_level(0, 4500.0, 50.0, 720.0, LEVELS);
        assert!(voxel_pixels(far - 1, 4500.0, 50.0, 720.0) < 0.45, "desde 4,5 km el nivel {far} es demasiado fino");
        assert!(far >= 4, "desde 4,5 km el nivel más fino debería ser al menos 4 (vóxel de 4 m; es {far})");
        assert!(voxel_pixels(far, 4500.0, 50.0, 720.0) >= 0.45);
        // Histéresis: justo en el borde no cambia de nivel en ninguno de los dos sentidos.
        let edge = BASE_VOXEL * 720.0 / (2.0 * 0.5 * (25f32).to_radians().tan()); // altura donde el nivel 0 mide 0,5 px
        assert_eq!(choose_first_level(0, edge * 1.05, 50.0, 720.0, LEVELS), 0);
        assert_eq!(choose_first_level(1, edge * 0.95, 50.0, 720.0, LEVELS), 1);
        assert_eq!(choose_first_level(1, edge * 0.8, 50.0, 720.0, LEVELS), 0);
    }

    #[test]
    fn el_ultimo_anillo_cubre_el_entorno_amplio() {
        // Desde cualquier punto del mapa (la cámara no sale de él), el anillo más grueso llega a
        // por lo menos 85 km en todas las direcciones, y no pasa del borde de la grilla amplia
        // (150 km alrededor del centro del mapa), así no se estira el borde de los datos.
        let levels: Vec<LevelSpec> = (0..LEVELS).map(|lod| LevelSpec { lod }).collect();
        let last = levels[LEVELS as usize - 1];
        for (cx, cz) in [(0.0, 0.0), (24000.0, 0.0), (0.0, 24000.0), (24000.0, 24000.0), (12000.0, 12000.0), (3100.0, 20500.0)] {
            let c = Vec2::new(cx, cz);
            let (min, max) = last.region(c);
            let (lo, hi) = (min.as_vec2() * last.chunk_m(), max.as_vec2() * last.chunk_m());
            let near = (c.x - lo.x).min(c.y - lo.y).min(hi.x - c.x).min(hi.y - c.y);
            assert!(near >= 85000.0, "desde {c:?} el último anillo llega a {near} m");
            let far = (12000.0 - lo.x).max(12000.0 - lo.y).max(hi.x - 12000.0).max(hi.y - 12000.0);
            assert!(far <= 150000.0, "desde {c:?} el último anillo sale de la grilla amplia ({far} m)");
            // Todas las columnas de todos los niveles: un punto en exactamente un nivel.
            let cols = level_columns(&levels, c);
            let total: usize = cols.iter().map(|l| l.len()).sum();
            assert!(total < 6000, "{total} columnas");
            for k in 0..200 {
                let a = k as f32 * 0.7;
                let r = 300.0 + k as f32 * 450.0;
                let p = c + Vec2::new(a.cos(), a.sin()) * r;
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
    fn la_malla_de_los_niveles_gruesos_baja_casi_un_voxel() {
        assert_eq!(far_drop(0), 0.0);
        assert_eq!(far_drop(DROP_LOD - 1), 0.0);
        for lod in DROP_LOD..LEVELS {
            let vs = BASE_VOXEL * (1u32 << lod) as f32;
            assert_eq!(far_drop(lod), vs - 3.0);
        }
    }

    #[test]
    fn empezando_en_otro_nivel_se_cubre_todo_igual() {
        let levels: Vec<LevelSpec> = (0..LEVELS).map(|lod| LevelSpec { lod }).collect();
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
        let levels: Vec<LevelSpec> = (0..LEVELS).map(|lod| LevelSpec { lod }).collect();
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
