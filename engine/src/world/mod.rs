//! Mundo de vóxeles: un almacén disperso de chunks por nivel de detalle.
//! Solo se guardan los chunks que contienen la superficie; lo que queda por debajo es
//! roca maciza implícita y lo que queda por encima, aire.

pub mod chunk;

use crate::buildings::Edits;
use crate::lod::{level_columns, LevelSpec, CHUNK};
use crate::palette::*;
use crate::terrain::TerrainGen;
use chunk::Chunk;
use glam::{IVec2, IVec3, Vec2};
use std::collections::HashMap;

const N: i32 = CHUNK;

/// Alturas de una columna de chunks: índice del vóxel más alto de terreno por (x, z).
pub struct Column {
    pub top: Vec<i32>,
}

pub struct Level {
    pub spec: LevelSpec,
    pub columns: HashMap<IVec2, Column>,
    pub chunks: HashMap<IVec3, Chunk>,
}

impl Level {
    /// Lee un vóxel en coordenadas del nivel. Si el chunk no está guardado, decide por la
    /// altura de su columna (macizo debajo, aire encima). Fuera del anillo devuelve `None`.
    pub fn sample(&self, v: IVec3) -> Option<Mat> {
        let ck = IVec3::new(v.x >> 5, v.y >> 5, v.z >> 5);
        if let Some(c) = self.chunks.get(&ck) {
            return Some(c.get((v.x & 31) as usize, (v.y & 31) as usize, (v.z & 31) as usize));
        }
        let col = self.columns.get(&IVec2::new(ck.x, ck.z))?;
        let top = col.top[((v.x & 31) + (v.z & 31) * N) as usize];
        Some(if v.y <= top { ROCK } else { AIR })
    }
}

pub struct World {
    pub terrain: TerrainGen,
    pub levels: Vec<Level>,
}

/// Ediciones (edificios, calles) llevadas a la resolución de un nivel y agrupadas por chunk.
struct LevelEdits {
    voxels: HashMap<IVec3, Vec<(u16, u8, u16, Mat)>>,
    surface: HashMap<IVec2, Vec<(u8, u8, Mat)>>,
    max_y: HashMap<IVec2, i32>,
}

fn level_edits(edits: &Edits, lod: u32) -> LevelEdits {
    // Al reducir la resolución, cada vóxel grueso toma el material del vóxel fino más alto
    // que contiene (así desde arriba se ve el techo y no un moteado de materiales).
    // A igual altura decide la posición fina (x, z): el resultado no depende del orden del HashMap.
    let mut best: HashMap<IVec3, (i32, i64, Mat)> = HashMap::new();
    for (&v, &m) in &edits.voxels {
        let k = IVec3::new(v.x >> lod, v.y >> lod, v.z >> lod);
        let tb = ((v.x as i64) << 32) | (v.z as u32 as i64);
        let e = best.entry(k).or_insert((i32::MIN, i64::MIN, m));
        if (v.y, tb) > (e.0, e.1) {
            *e = (v.y, tb, m);
        }
    }
    let vox: HashMap<IVec3, Mat> = best.into_iter().map(|(k, (_, _, m))| (k, m)).collect();
    let mut out = LevelEdits { voxels: HashMap::new(), surface: HashMap::new(), max_y: HashMap::new() };
    for (v, m) in vox {
        let ck = IVec3::new(v.x >> 5, v.y >> 5, v.z >> 5);
        out.voxels.entry(ck).or_default().push(((v.x & 31) as u16, (v.y & 31) as u8, (v.z & 31) as u16, m));
        let col = IVec2::new(ck.x, ck.z);
        let e = out.max_y.entry(col).or_insert(i32::MIN);
        *e = (*e).max(v.y);
    }
    let mut surf: HashMap<IVec2, (i64, Mat)> = HashMap::new();
    for (&c, &m) in &edits.surface {
        let tb = ((c.x as i64) << 32) | (c.y as u32 as i64);
        let e = surf.entry(IVec2::new(c.x >> lod, c.y >> lod)).or_insert((i64::MIN, m));
        if tb > e.0 {
            *e = (tb, m);
        }
    }
    for (c, (_, m)) in surf {
        let ck = IVec2::new(c.x >> 5, c.y >> 5);
        out.surface.entry(ck).or_default().push(((c.x & 31) as u8, (c.y & 31) as u8, m));
    }
    out
}

impl World {
    /// Genera todos los niveles alrededor de `center` (en metros, plano x/z).
    pub fn generate(terrain: TerrainGen, edits: &Edits, levels: &[LevelSpec], center: Vec2) -> World {
        // Captura de una vez (sin streaming): el diseño se expande a la grilla base.
        let base = edits.to_base();
        let edits = &base;
        let cols = level_columns(levels, center);
        let mut out = Vec::new();
        for (i, spec) in levels.iter().enumerate() {
            let le = level_edits(edits, spec.lod);
            let list = &cols[i];
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
            let per = list.len().div_ceil(threads).max(1);
            let results: Vec<Vec<(IVec2, Column, Vec<(IVec3, Chunk)>)>> = std::thread::scope(|s| {
                let handles: Vec<_> = list
                    .chunks(per)
                    .map(|part| {
                        let terrain = &terrain;
                        let le = &le;
                        let spec = *spec;
                        s.spawn(move || part.iter().map(|&c| gen_column(terrain, le, spec, c)).collect::<Vec<_>>())
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            let mut level = Level { spec: *spec, columns: HashMap::new(), chunks: HashMap::new() };
            for part in results {
                for (c, col, chunks) in part {
                    level.columns.insert(c, col);
                    for (k, ch) in chunks {
                        level.chunks.insert(k, ch);
                    }
                }
            }
            out.push(level);
        }
        World { terrain, levels: out }
    }

    pub fn chunk_count(&self) -> usize {
        self.levels.iter().map(|l| l.chunks.len()).sum()
    }
}

/// Genera una columna de chunks: terreno por capas + ediciones del nivel.
fn gen_column(t: &TerrainGen, le: &LevelEdits, spec: LevelSpec, c: IVec2) -> (IVec2, Column, Vec<(IVec3, Chunk)>) {
    let vs = spec.voxel();
    let p = (N + 2) as usize;
    // Alturas con un borde de un vóxel para calcular la pendiente.
    let mut hs = vec![0f32; p * p];
    let coast: Vec<f32> = (0..p).map(|i| t.coast_z(((c.x * N + i as i32 - 1) as f32 + 0.5) * vs)).collect();
    for j in 0..p {
        for i in 0..p {
            let x = ((c.x * N + i as i32 - 1) as f32 + 0.5) * vs;
            let z = ((c.y * N + j as i32 - 1) as f32 + 0.5) * vs;
            hs[i + j * p] = t.height_with_coast(x, z, coast[i]);
        }
    }
    let mut top = vec![0i32; (N * N) as usize];
    let mut surf = vec![AIR; (N * N) as usize];
    let (mut tmin, mut tmax) = (i32::MAX, i32::MIN);
    for lz in 0..N as usize {
        for lx in 0..N as usize {
            let h = hs[(lx + 1) + (lz + 1) * p];
            let dx = hs[(lx + 2) + (lz + 1) * p] - hs[lx + (lz + 1) * p];
            let dz = hs[(lx + 1) + (lz + 2) * p] - hs[(lx + 1) + lz * p];
            let slope = (dx * dx + dz * dz).sqrt() / (2.0 * vs);
            // Bajo el agua, la cara superior del vóxel no debe asomar sobre el nivel del río.
            let ty = if h < 0.0 { (h / vs - 1.0).floor() as i32 } else { (h / vs - 0.5).floor() as i32 };
            let x = ((c.x * N + lx as i32) as f32 + 0.5) * vs;
            let z = ((c.y * N + lz as i32) as f32 + 0.5) * vs;
            top[lx + lz * N as usize] = ty;
            surf[lx + lz * N as usize] = t.surface_material(x, z, h, slope);
            tmin = tmin.min(ty);
            tmax = tmax.max(ty);
        }
    }
    if let Some(list) = le.surface.get(&c) {
        for &(lx, lz, m) in list {
            surf[lx as usize + lz as usize * N as usize] = m;
        }
    }
    // Capas: superficie, tierra (o arena) y roca.
    let soil = ((2.0 / vs).ceil() as i32).max(1);
    let edit_top = le.max_y.get(&c).copied().unwrap_or(i32::MIN);
    let y0 = (tmin - 1).div_euclid(N);
    let y1 = tmax.max(edit_top).div_euclid(N);
    let mut chunks = Vec::new();
    for cy in y0..=y1 {
        let mut ch = Chunk::new();
        for ly in 0..N {
            let gy = cy * N + ly;
            for lz in 0..N as usize {
                for lx in 0..N as usize {
                    let i = lx + lz * N as usize;
                    let ty = top[i];
                    if gy > ty {
                        continue;
                    }
                    let depth = ty - gy;
                    let s = surf[i];
                    let m = if depth == 0 {
                        s
                    } else if depth <= soil {
                        match s {
                            SAND | MUD => s,
                            SAND_WET | DUNE => SAND,
                            STREAM | MARSH => MUD,
                            ROCK | COAST_ROCK => ROCK,
                            _ => DIRT,
                        }
                    } else {
                        ROCK
                    };
                    ch.set(lx, ly as usize, lz, m);
                }
            }
        }
        let key = IVec3::new(c.x, cy, c.y);
        if let Some(list) = le.voxels.get(&key) {
            for &(x, y, z, m) in list {
                ch.set(x as usize, y as usize, z as usize, m);
            }
        }
        if !ch.is_empty() {
            chunks.push((key, ch));
        }
    }
    (c, Column { top }, chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildings::Edits;

    #[test]
    fn cada_columna_tiene_su_superficie_guardada() {
        let t = TerrainGen::new(1726);
        let levels = [LevelSpec::new(0), LevelSpec::new(1)]; // [lod]
        let w = World::generate(t, &Edits::default(), &levels, Vec2::new(13500.0, 16300.0));
        for level in &w.levels {
            for (c, col) in &level.columns {
                for (i, &top) in col.top.iter().enumerate() {
                    let v = IVec3::new(c.x * N + (i as i32 % N), top, c.y * N + (i as i32 / N));
                    let k = IVec3::new(v.x >> 5, v.y >> 5, v.z >> 5);
                    assert!(level.chunks.contains_key(&k), "falta el chunk de superficie {k:?} (nivel {})", level.spec.lod);
                    assert_ne!(level.sample(v), Some(AIR));
                    assert_eq!(level.sample(v + IVec3::Y), Some(AIR));
                }
            }
        }
    }

    #[test]
    fn los_edificios_se_ven_en_los_niveles_gruesos() {
        let mut e = Edits::default();
        let base = IVec3::new(27000, 40, 32600);
        e.fill(base, base + IVec3::new(8, 10, 8), CAL);
        e.fill(base + IVec3::new(0, 10, 0), base + IVec3::new(8, 11, 8), AZOTEA);
        let le = level_edits(&e, 2);
        // Capa más alta a resolución de 1 m (nivel 2): debe ser azotea, no cal.
        let top_y = (base.y + 10) >> 2;
        let mut tops = Vec::new();
        for (ck, list) in &le.voxels {
            for &(_, y, _, m) in list {
                if ck.y * N + y as i32 == top_y {
                    tops.push(m);
                }
            }
        }
        assert!(!tops.is_empty());
        assert!(tops.iter().all(|&m| m == AZOTEA), "desde arriba debe verse la azotea");
    }
}
