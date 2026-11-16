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

/// Ediciones de un chunk, compactas: índices locales ordenados y, en paralelo, la altura del
/// vóxel fino que decidió cada uno y su material (5 bytes por vóxel; un HashMap por vóxel
/// ocupaba unas cuatro veces más).
#[derive(Default, Clone)]
pub struct ChunkEdits {
    idx: Vec<u16>,
    h: Vec<i16>,
    m: Vec<Mat>,
}

impl ChunkEdits {
    #[inline]
    pub fn get(&self, i: u16) -> Option<Mat> {
        self.idx.binary_search(&i).ok().map(|k| self.m[k])
    }

    pub fn len(&self) -> usize {
        self.idx.len()
    }

    /// Recorre (índice local, material).
    pub fn iter(&self) -> impl Iterator<Item = (u16, Mat)> + '_ {
        self.idx.iter().copied().zip(self.m.iter().copied())
    }

    fn bytes(&self) -> usize {
        self.idx.capacity() * 2 + self.h.capacity() * 2 + self.m.capacity() + std::mem::size_of::<Self>()
    }

    /// Une entradas nuevas (ordenadas por índice y sin repetir). Gana la nueva si su vóxel
    /// fino está a igual o mayor altura (la edición más reciente gana los empates).
    fn merge(&mut self, new: &[(u16, i16, Mat)]) {
        let (mut idx, mut h, mut m) = (Vec::with_capacity(self.idx.len() + new.len()), Vec::new(), Vec::new());
        h.reserve(self.idx.len() + new.len());
        m.reserve(self.idx.len() + new.len());
        let (mut a, mut b) = (0usize, 0usize);
        while a < self.idx.len() || b < new.len() {
            let take_old = b >= new.len() || (a < self.idx.len() && self.idx[a] < new[b].0);
            let take_new = a >= self.idx.len() || (b < new.len() && new[b].0 < self.idx[a]);
            if take_old {
                idx.push(self.idx[a]);
                h.push(self.h[a]);
                m.push(self.m[a]);
                a += 1;
            } else if take_new {
                idx.push(new[b].0);
                h.push(new[b].1);
                m.push(new[b].2);
                b += 1;
            } else {
                // Mismo vóxel: decide la altura; a igual altura, la edición nueva.
                let (nh, nm) = (new[b].1, new[b].2);
                idx.push(new[b].0);
                if nh >= self.h[a] {
                    h.push(nh);
                    m.push(nm);
                } else {
                    h.push(self.h[a]);
                    m.push(self.m[a]);
                }
                a += 1;
                b += 1;
            }
        }
        idx.shrink_to_fit();
        h.shrink_to_fit();
        m.shrink_to_fit();
        *self = ChunkEdits { idx, h, m };
    }
}

#[derive(Default)]
pub struct LevelStore {
    pub lod: u32,
    /// Ediciones por chunk.
    pub voxels: HashMap<IVec3, ChunkEdits>,
    /// Material de la capa superior del terreno, por columna de vóxeles del nivel.
    pub surface: HashMap<IVec2, Mat>,
    /// Altura máxima (en vóxeles del nivel) de lo editado en cada columna de chunks.
    pub max_y: HashMap<IVec2, i32>,
}

/// Clave de desempate determinista a partir de la posición fina (x, z).
#[inline]
fn tiebreak(x: i32, z: i32) -> u64 {
    ((x as u32 as u64) << 32) | z as u32 as u64
}

impl LevelStore {
    pub fn get(&self, v: IVec3) -> Option<Mat> {
        self.voxels.get(&chunk_of(v)).and_then(|c| c.get(local_index(v)))
    }

    fn apply(&mut self, edits: &Edits, touched: Option<&mut HashSet<IVec2>>) {
        let lod = self.lod as i32;
        // 1. Ganador de cada vóxel del nivel dentro de esta carga: el vóxel fino más alto; a
        //    igual altura, el de mayor posición (x, z). Así el resultado no depende del orden
        //    en que se recorre el HashMap de ediciones.
        let mut staged: HashMap<IVec3, HashMap<u16, (i32, u64, Mat)>> = HashMap::new();
        let mut cols: Vec<IVec2> = Vec::new();
        for (&v, &m) in &edits.voxels {
            let k = IVec3::new(v.x >> lod, v.y >> lod, v.z >> lod);
            let ck = chunk_of(k);
            let tb = tiebreak(v.x, v.z);
            let e = staged.entry(ck).or_default().entry(local_index(k)).or_insert((i32::MIN, 0, m));
            if (v.y, tb) >= (e.0, e.1) {
                *e = (v.y, tb, m);
            }
            let col = IVec2::new(ck.x, ck.z);
            let my = self.max_y.entry(col).or_insert(i32::MIN);
            *my = (*my).max(k.y);
            if touched.is_some() {
                cols.push(IVec2::new(k.x, k.z));
            }
        }
        // 2. Se compacta y se une con lo que ya había (la carga nueva gana los empates).
        for (ck, map) in staged {
            let mut list: Vec<(u16, i16, Mat)> = map.into_iter().map(|(i, (h, _, m))| (i, h.clamp(i16::MIN as i32, i16::MAX as i32) as i16, m)).collect();
            list.sort_unstable_by_key(|e| e.0);
            self.voxels.entry(ck).or_default().merge(&list);
        }
        // Superficie: a igual celda gruesa, gana la celda fina de mayor posición.
        let mut surf: HashMap<IVec2, (u64, Mat)> = HashMap::new();
        for (&c, &m) in &edits.surface {
            let k = IVec2::new(c.x >> lod, c.y >> lod);
            let tb = tiebreak(c.x, c.y);
            let e = surf.entry(k).or_insert((0, m));
            if tb >= e.0 {
                *e = (tb, m);
            }
            if touched.is_some() {
                cols.push(k);
            }
        }
        for (k, (_, m)) in surf {
            self.surface.insert(k, m);
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

impl LevelStore {
    /// Memoria aproximada de las ediciones de este nivel, en bytes.
    pub fn approx_bytes(&self) -> usize {
        let mut b: usize = self.voxels.values().map(|c| c.bytes()).sum();
        b += self.voxels.capacity() * (std::mem::size_of::<(IVec3, ChunkEdits)>() + 1);
        b += self.surface.capacity() * (std::mem::size_of::<(IVec2, Mat)>() + 1);
        b += self.max_y.capacity() * (std::mem::size_of::<(IVec2, i32)>() + 1);
        b
    }

    /// Número de vóxeles editados guardados en este nivel.
    pub fn voxel_count(&self) -> usize {
        self.voxels.values().map(|c| c.len()).sum()
    }
}

pub struct EditStore {
    pub levels: Vec<LevelStore>,
}

impl EditStore {
    pub fn new(levels: u32) -> Self {
        EditStore { levels: (0..levels).map(|lod| LevelStore { lod, ..Default::default() }).collect() }
    }

    pub fn approx_bytes(&self) -> usize {
        self.levels.iter().map(|l| l.approx_bytes()).sum()
    }

    pub fn voxel_count(&self) -> usize {
        self.levels.iter().map(|l| l.voxel_count()).sum()
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
    fn el_almacen_compacto_es_determinista_y_ocupa_poco() {
        // Dos vóxeles finos a la misma altura caen en el mismo vóxel grueso: el resultado no
        // debe depender del orden de inserción.
        let mut a = Edits::default();
        a.set(IVec3::new(0, 4, 0), CAL);
        a.set(IVec3::new(1, 4, 1), TILE);
        let mut b = Edits::default();
        b.set(IVec3::new(1, 4, 1), TILE);
        b.set(IVec3::new(0, 4, 0), CAL);
        let (mut sa, mut sb) = (EditStore::new(2), EditStore::new(2));
        sa.apply(&a, false);
        sb.apply(&b, false);
        assert_eq!(sa.levels[1].get(IVec3::new(0, 2, 0)), sb.levels[1].get(IVec3::new(0, 2, 0)));
        // Un bloque de 16³ ocupa ~5 bytes por vóxel en el nivel 0.
        let mut e = Edits::default();
        e.fill(IVec3::ZERO, IVec3::splat(16), CAL);
        let mut s = EditStore::new(1);
        s.apply(&e, false);
        assert_eq!(s.levels[0].voxel_count(), 4096);
        let per = s.levels[0].voxels.values().map(|c| c.bytes()).sum::<usize>() as f64 / 4096.0;
        assert!(per < 6.0, "{per} bytes por vóxel");
        // Y se siguen leyendo bien después de unir ediciones nuevas.
        let mut q = Edits::default();
        q.set(IVec3::new(3, 3, 3), AIR);
        s.apply(&q, false);
        assert_eq!(s.levels[0].get(IVec3::new(3, 3, 3)), Some(AIR));
        assert_eq!(s.levels[0].get(IVec3::new(4, 3, 3)), Some(CAL));
        assert_eq!(s.levels[0].get(IVec3::new(20, 3, 3)), None);
    }

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
