//! Almacén de ediciones por nivel de detalle.
//!
//! El terreno es una función (no se guarda): solo se guardan las *ediciones* (edificios,
//! calles, excavaciones), ya llevadas a la resolución de cada nivel. Así el mundo en streaming
//! puede generar cualquier chunk cuando lo necesita y la memoria no crece con el mapa.
//!
//! Regla al reducir resolución: cada vóxel grueso toma el material del vóxel fino editado
//! más alto que contiene (desde arriba se ve el techo, no un moteado). A igual altura gana la
//! edición más reciente, así excavar o construir encima reemplaza lo anterior.

use crate::buildings::Edits;
use crate::lod::CHUNK;
use crate::palette::Mat;
use glam::{IVec2, IVec3};
use std::collections::{HashMap, HashSet};

/// Índice local dentro de un chunk de 32³.
#[inline]
pub fn local_index(v: IVec3) -> u16 {
    ((v.x & 31) + (v.z & 31) * 32 + (v.y & 31) * 1024) as u16
}

#[inline]
pub fn chunk_of(v: IVec3) -> IVec3 {
    IVec3::new(v.x >> 5, v.y >> 5, v.z >> 5)
}

#[derive(Default)]
pub struct LevelStore {
    pub lod: u32,
    /// Por chunk: índice local → (altura del vóxel fino que decidió, material).
    pub voxels: HashMap<IVec3, HashMap<u16, (i32, Mat)>>,
    /// Material de la capa superior del terreno, por columna de vóxeles del nivel.
    pub surface: HashMap<IVec2, Mat>,
    /// Altura máxima (en vóxeles del nivel) de lo editado en cada columna de chunks.
    pub max_y: HashMap<IVec2, i32>,
}

impl LevelStore {
    pub fn get(&self, v: IVec3) -> Option<Mat> {
        self.voxels.get(&chunk_of(v)).and_then(|m| m.get(&local_index(v))).map(|e| e.1)
    }

    fn apply(&mut self, edits: &Edits, touched: Option<&mut HashSet<IVec2>>) {
        let lod = self.lod as i32;
        let mut cols: Vec<IVec2> = Vec::new();
        for (&v, &m) in &edits.voxels {
            let k = IVec3::new(v.x >> lod, v.y >> lod, v.z >> lod);
            let ck = chunk_of(k);
            let map = self.voxels.entry(ck).or_default();
            let e = map.entry(local_index(k)).or_insert((i32::MIN, m));
            if v.y >= e.0 {
                *e = (v.y, m);
            }
            let col = IVec2::new(ck.x, ck.z);
            let my = self.max_y.entry(col).or_insert(i32::MIN);
            *my = (*my).max(k.y);
            if touched.is_some() {
                cols.push(IVec2::new(k.x, k.z));
            }
        }
        for (&c, &m) in &edits.surface {
            let k = IVec2::new(c.x >> lod, c.y >> lod);
            self.surface.insert(k, m);
            if touched.is_some() {
                cols.push(k);
            }
        }
        if let Some(t) = touched {
            // Una edición afecta a su columna de chunks y, si está en el borde, a la vecina
            // (por el borde de un vóxel que se usa al mallar y por la oclusión ambiental).
            for c in cols {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let n = IVec2::new((c.x + dx) >> 5, (c.y + dz) >> 5);
                        t.insert(n);
                    }
                }
            }
        }
    }
}

pub struct EditStore {
    pub levels: Vec<LevelStore>,
}

impl EditStore {
    pub fn new(levels: u32) -> Self {
        EditStore { levels: (0..levels).map(|lod| LevelStore { lod, ..Default::default() }).collect() }
    }

    /// Aplica ediciones en todos los niveles (en paralelo, un hilo por nivel).
    /// Si `track` es verdadero, devuelve las columnas de chunks tocadas por nivel.
    pub fn apply(&mut self, edits: &Edits, track: bool) -> Vec<HashSet<IVec2>> {
        let mut out: Vec<HashSet<IVec2>> = vec![HashSet::new(); self.levels.len()];
        std::thread::scope(|s| {
            for (level, t) in self.levels.iter_mut().zip(out.iter_mut()) {
                s.spawn(move || level.apply(edits, if track { Some(t) } else { None }));
            }
        });
        out
    }
}

#[allow(dead_code)]
const _: () = assert!(CHUNK == 32);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::*;

    #[test]
    fn a_igual_altura_gana_la_edicion_mas_reciente() {
        let mut s = EditStore::new(3);
        let mut a = Edits::default();
        a.set(IVec3::new(10, 5, 10), CAL);
        s.apply(&a, false);
        let mut b = Edits::default();
        b.set(IVec3::new(10, 5, 10), AIR);
        let touched = s.apply(&b, true);
        assert_eq!(s.levels[0].get(IVec3::new(10, 5, 10)), Some(AIR));
        assert_eq!(s.levels[2].get(IVec3::new(2, 1, 2)), Some(AIR));
        // Solo se tocan las columnas cercanas, no todo el mundo.
        assert!(touched[0].len() <= 4 && touched[0].contains(&IVec2::new(0, 0)));
    }
}
