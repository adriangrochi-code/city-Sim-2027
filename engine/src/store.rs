//! Almacén de ediciones por nivel de detalle.
//!
//! El terreno es una función (no se guarda): solo se guardan las *ediciones* (edificios,
//! calles, excavaciones), ya llevadas a la resolución de cada nivel. Así el mundo en streaming
//! puede generar cualquier chunk cuando lo necesita y la memoria no crece con el mapa.
//!
//! Regla al reducir resolución: cada vóxel grueso toma el material del vóxel fino macizo
//! más alto que contiene (desde arriba se ve el techo, no un moteado); solo si todo lo que
//! trae la carga en ese vóxel es aire, queda aire. Cada carga nueva reemplaza a la anterior
//! en los vóxeles que toca, así excavar, demoler o reemplazar un edificio funciona en todos
//! los niveles.

use crate::buildings::Edits;
use crate::lod::{CHUNK, DESIGN_SCALE, DESIGN_SHIFT};
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

    /// Quita los índices dados (ordenados).
    fn remove_sorted(&mut self, gone: &[u16]) {
        let mut k = 0;
        let (mut idx, mut h, mut m) = (Vec::new(), Vec::new(), Vec::new());
        for j in 0..self.idx.len() {
            let i = self.idx[j];
            while k < gone.len() && gone[k] < i {
                k += 1;
            }
            if k < gone.len() && gone[k] == i {
                continue;
            }
            idx.push(i);
            h.push(self.h[j]);
            m.push(self.m[j]);
        }
        *self = ChunkEdits { idx, h, m };
    }

    /// Recorre (índice local, material).
    pub fn iter(&self) -> impl Iterator<Item = (u16, Mat)> + '_ {
        self.idx.iter().copied().zip(self.m.iter().copied())
    }

    fn bytes(&self) -> usize {
        self.idx.capacity() * 2 + self.h.capacity() * 2 + self.m.capacity() + std::mem::size_of::<Self>()
    }

    /// Une entradas nuevas (ordenadas por índice y sin repetir). La carga nueva reemplaza a la
    /// vieja en cada vóxel que toca: así se puede demoler o reemplazar un edificio (la carga
    /// trae aire donde estaba el viejo) también en los niveles gruesos.
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
                // Mismo vóxel: gana la carga nueva.
                idx.push(new[b].0);
                h.push(new[b].1);
                m.push(new[b].2);
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
    /// Suelo nivelado por obras, en vóxeles de este nivel (ver `Edits::ground`).
    pub ground: HashMap<IVec2, i32>,
    /// Solo en el nivel 0: ediciones de diseño (vóxeles de 0,5 m) por chunk de diseño, sin
    /// expandir a la grilla base. Lo que hay en `voxels` les gana.
    pub design: HashMap<IVec3, ChunkEdits>,
}

/// Clave de desempate determinista a partir de la posición fina (x, z).
#[inline]
fn tiebreak(x: i32, z: i32) -> u64 {
    ((x as u32 as u64) << 32) | z as u32 as u64
}

impl LevelStore {
    /// Material editado de un vóxel del nivel (base primero; en el nivel 0, después diseño).
    pub fn get(&self, v: IVec3) -> Option<Mat> {
        self.voxels.get(&chunk_of(v)).and_then(|c| c.get(local_index(v))).or_else(|| {
            let d = IVec3::new(v.x >> DESIGN_SHIFT, v.y >> DESIGN_SHIFT, v.z >> DESIGN_SHIFT);
            self.design.get(&chunk_of(d)).and_then(|c| c.get(local_index(d)))
        })
    }

    /// ¿Hay ediciones en el chunk `ck` (coordenadas de chunk del nivel)?
    pub fn has_chunk(&self, ck: IVec3) -> bool {
        self.voxels.contains_key(&ck) || self.design.contains_key(&IVec3::new(ck.x >> DESIGN_SHIFT, ck.y >> DESIGN_SHIFT, ck.z >> DESIGN_SHIFT))
    }

    /// Recorre las ediciones del chunk `ck` como (índice local, material): primero los
    /// bloques de diseño que caen en él y después los vóxeles base (que deben ganar: quien
    /// escribe en orden se queda con lo último).
    pub fn for_each_in_chunk(&self, ck: IVec3, mut f: impl FnMut(u16, Mat)) {
        let s = DESIGN_SCALE;
        let dk = IVec3::new(ck.x >> DESIGN_SHIFT, ck.y >> DESIGN_SHIFT, ck.z >> DESIGN_SHIFT);
        if let Some(dc) = self.design.get(&dk) {
            // Parte del chunk de diseño que cubre este chunk base (en vóxeles de diseño locales).
            let side = CHUNK / s;
            let o = (ck - dk * s) * side;
            for (i, m) in dc.iter() {
                let l = IVec3::new((i & 31) as i32, (i >> 10) as i32, ((i >> 5) & 31) as i32) - o;
                if l.x < 0 || l.y < 0 || l.z < 0 || l.x >= side || l.y >= side || l.z >= side {
                    continue;
                }
                let b = l * s;
                for dy in 0..s {
                    for dz in 0..s {
                        for dx in 0..s {
                            f(((b.x + dx) + (b.z + dz) * CHUNK + (b.y + dy) * CHUNK * CHUNK) as u16, m);
                        }
                    }
                }
            }
        }
        if let Some(c) = self.voxels.get(&ck) {
            for (i, m) in c.iter() {
                f(i, m);
            }
        }
    }

    /// Columnas de chunks del nivel que tienen ediciones de vóxeles.
    pub fn edited_columns(&self) -> HashSet<IVec2> {
        let s = DESIGN_SCALE;
        let mut out: HashSet<IVec2> = self.voxels.keys().map(|k| IVec2::new(k.x, k.z)).collect();
        for k in self.design.keys() {
            for dz in 0..s {
                for dx in 0..s {
                    out.insert(IVec2::new(k.x * s + dx, k.z * s + dz));
                }
            }
        }
        out
    }

    fn apply(&mut self, edits: &Edits, touched: Option<&mut HashSet<IVec2>>) {
        let lod = self.lod as i32;
        let track = touched.is_some();
        let s = DESIGN_SCALE;
        let mut cols: Vec<IVec2> = Vec::new();
        if lod == 0 {
            self.apply_fine(edits, track, &mut cols);
        } else {
            self.apply_coarse(edits, track, &mut cols);
        }
        // Superficie: a igual celda gruesa, gana la celda fina de mayor posición. Una columna
        // de diseño cuenta como su columna base de más arriba a la derecha; en el nivel 0 se
        // expande y lo base gana donde se pisan.
        if lod == 0 {
            for (&c, &m) in &edits.design_surface {
                for dz in 0..s {
                    for dx in 0..s {
                        let k = c * s + IVec2::new(dx, dz);
                        self.surface.insert(k, m);
                        if track {
                            cols.push(k);
                        }
                    }
                }
            }
            for (&c, &m) in &edits.surface {
                self.surface.insert(c, m);
                if track {
                    cols.push(c);
                }
            }
        } else {
            let mut surf: HashMap<IVec2, (u64, Mat)> = HashMap::new();
            let design = edits.design_surface.iter().map(|(&c, &m)| (c * s + IVec2::splat(s - 1), m));
            for (c, m) in edits.surface.iter().map(|(&c, &m)| (c, m)).chain(design) {
                let k = IVec2::new(c.x >> lod, c.y >> lod);
                let tb = tiebreak(c.x, c.y);
                let e = surf.entry(k).or_insert((0, m));
                if tb >= e.0 {
                    *e = (tb, m);
                }
                if track {
                    cols.push(k);
                }
            }
            for (k, (_, m)) in surf {
                self.surface.insert(k, m);
            }
        }
        // Suelo nivelado: promedio de la altura de la superficie de las columnas finas que caen
        // en cada columna gruesa, convertido a vóxeles del nivel.
        let mut gr: HashMap<IVec2, (f64, u32)> = HashMap::new();
        for (&c, &top) in &edits.ground {
            let k = IVec2::new(c.x >> lod, c.y >> lod);
            let e = gr.entry(k).or_insert((0.0, 0));
            e.0 += (top + 1) as f64; // altura de la cara de arriba, en vóxeles finos
            e.1 += 1;
            if track {
                cols.push(k);
            }
        }
        let scale = (1u32 << lod) as f64;
        for (k, (sum, n)) in gr {
            let top = ((sum / n as f64) / scale).round() as i32 - 1;
            self.ground.insert(k, top);
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

    /// Nivel 0: los vóxeles base van tal cual a `voxels`; los de diseño, sin expandir, a
    /// `design` (por chunk de diseño). Así una ciudad entera no ocupa ocho veces más. Un
    /// vóxel de diseño nuevo borra los vóxeles base viejos de su bloque (la carga nueva
    /// gana); dentro de la misma carga, lo base gana porque se consulta primero.
    fn apply_fine(&mut self, edits: &Edits, track: bool, cols: &mut Vec<IVec2>) {
        let s = DESIGN_SCALE;
        let note = |max_y: &mut HashMap<IVec2, i32>, v: IVec3, top: i32| {
            let my = max_y.entry(IVec2::new(v.x >> 5, v.z >> 5)).or_insert(i32::MIN);
            *my = (*my).max(top);
        };
        // Diseño: por chunk de diseño, y los índices base que tapa cada bloque, por chunk base.
        let mut by_dchunk: HashMap<IVec3, Vec<(u16, i16, Mat)>> = HashMap::new();
        let mut covered: HashMap<IVec3, Vec<u16>> = HashMap::new();
        for (&d, &m) in &edits.design {
            let h = d.y.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            by_dchunk.entry(chunk_of(d)).or_default().push((local_index(d), h, m));
            let v = d * s;
            note(&mut self.max_y, v, v.y + s - 1);
            let ck = chunk_of(v);
            if self.voxels.contains_key(&ck) {
                let i = local_index(v);
                let list = covered.entry(ck).or_default();
                for dy in 0..s {
                    for dz in 0..s {
                        for dx in 0..s {
                            list.push(i + (dx + dz * CHUNK + dy * CHUNK * CHUNK) as u16);
                        }
                    }
                }
            }
            if track {
                for dz in 0..s {
                    for dx in 0..s {
                        cols.push(IVec2::new(v.x + dx, v.z + dz));
                    }
                }
            }
        }
        for (dk, mut list) in by_dchunk {
            list.sort_unstable_by_key(|e| e.0);
            self.design.entry(dk).or_default().merge(&list);
        }
        for (ck, mut gone) in covered {
            gone.sort_unstable();
            if let Some(c) = self.voxels.get_mut(&ck) {
                c.remove_sorted(&gone);
                if c.len() == 0 {
                    self.voxels.remove(&ck);
                }
            }
        }
        // Base: por chunk, ordenado.
        let mut by_chunk: HashMap<IVec3, Vec<(u16, i16, Mat)>> = HashMap::new();
        for (&v, &m) in &edits.voxels {
            let h = v.y.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            by_chunk.entry(chunk_of(v)).or_default().push((local_index(v), h, m));
            note(&mut self.max_y, v, v.y);
            if track {
                cols.push(IVec2::new(v.x, v.z));
            }
        }
        for (ck, mut list) in by_chunk {
            list.sort_unstable_by_key(|e| e.0);
            self.voxels.entry(ck).or_default().merge(&list);
        }
    }

    /// Niveles gruesos: el ganador de cada vóxel del nivel dentro de esta carga es el vóxel
    /// fino más alto; a igual altura, el de mayor posición (x, z). Así el resultado no depende
    /// del orden en que se recorre el HashMap de ediciones. Un vóxel de diseño cuenta como su
    /// vóxel base de más arriba (con DESIGN_SHIFT ≤ 1 nunca es más grande que el del nivel).
    /// Un vóxel macizo le gana al aire: si en la carga se reemplaza un edificio alto por uno
    /// más bajo, el aire que borra el viejo no tapa el techo del nuevo en lo grueso.
    fn apply_coarse(&mut self, edits: &Edits, track: bool, cols: &mut Vec<IVec2>) {
        let lod = self.lod as i32;
        let s = DESIGN_SCALE;
        let mut staged: HashMap<IVec3, HashMap<u16, (bool, i32, u64, Mat)>> = HashMap::new();
        let design = edits.design.iter().map(|(&d, &m)| (d * s + IVec3::splat(s - 1), m));
        for (v, m) in edits.voxels.iter().map(|(&v, &m)| (v, m)).chain(design) {
            let k = IVec3::new(v.x >> lod, v.y >> lod, v.z >> lod);
            let ck = chunk_of(k);
            let tb = tiebreak(v.x, v.z);
            let solid = m != crate::palette::AIR;
            let e = staged.entry(ck).or_default().entry(local_index(k)).or_insert((false, i32::MIN, 0, m));
            if (solid, v.y, tb) >= (e.0, e.1, e.2) {
                *e = (solid, v.y, tb, m);
            }
            let col = IVec2::new(ck.x, ck.z);
            let my = self.max_y.entry(col).or_insert(i32::MIN);
            *my = (*my).max(k.y);
            if track {
                cols.push(IVec2::new(k.x, k.z));
            }
        }
        // Se compacta y se une con lo que ya había (la carga nueva gana los empates).
        for (ck, map) in staged {
            let mut list: Vec<(u16, i16, Mat)> = map.into_iter().map(|(i, (_, h, _, m))| (i, h.clamp(i16::MIN as i32, i16::MAX as i32) as i16, m)).collect();
            list.sort_unstable_by_key(|e| e.0);
            self.voxels.entry(ck).or_default().merge(&list);
        }
    }
}

impl LevelStore {
    /// Memoria aproximada de las ediciones de este nivel, en bytes.
    pub fn approx_bytes(&self) -> usize {
        let mut b: usize = self.voxels.values().chain(self.design.values()).map(|c| c.bytes()).sum();
        b += (self.voxels.capacity() + self.design.capacity()) * (std::mem::size_of::<(IVec3, ChunkEdits)>() + 1);
        b += self.surface.capacity() * (std::mem::size_of::<(IVec2, Mat)>() + 1);
        b += self.max_y.capacity() * (std::mem::size_of::<(IVec2, i32)>() + 1);
        b
    }

    /// Número de vóxeles editados guardados en este nivel (los de diseño cuentan uno).
    pub fn voxel_count(&self) -> usize {
        self.voxels.values().chain(self.design.values()).map(|c| c.len()).sum()
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

    /// Reemplazar un edificio por uno más bajo (o demolerlo) no deja restos en los niveles
    /// gruesos: dentro de una carga lo sólido le gana al aire, pero la carga nueva reemplaza.
    #[test]
    fn reemplazar_o_demoler_no_deja_restos_en_los_niveles_gruesos() {
        let mut s = EditStore::new(3);
        let mut a = Edits::default();
        a.fill(IVec3::new(0, 0, 0), IVec3::new(8, 8, 8), CAL); // casa de altos
        s.apply(&a, false);
        assert_eq!(s.levels[2].get(IVec3::new(1, 1, 1)), Some(CAL));
        // Se reemplaza: aire arriba, techo más bajo. Un solo vóxel sólido en el grueso basta.
        let mut b = Edits::default();
        b.fill(IVec3::new(0, 4, 0), IVec3::new(8, 8, 8), AIR);
        b.fill(IVec3::new(0, 0, 0), IVec3::new(8, 4, 8), TILE);
        s.apply(&b, false);
        assert_eq!(s.levels[2].get(IVec3::new(1, 1, 1)), Some(AIR), "el piso alto se fue también en el nivel grueso");
        assert_eq!(s.levels[2].get(IVec3::new(1, 0, 1)), Some(TILE));
        assert_eq!(s.levels[1].get(IVec3::new(2, 2, 2)), Some(AIR));
        // Demoler: todo aire.
        let mut c = Edits::default();
        c.fill(IVec3::new(0, 0, 0), IVec3::new(8, 8, 8), AIR);
        s.apply(&c, false);
        assert_eq!(s.levels[2].get(IVec3::new(1, 0, 1)), Some(AIR));
    }

    /// Un vóxel de diseño ocupa su bloque de vóxeles base en el nivel 0 y un vóxel en el 1
    /// (del mismo tamaño); lo base le gana en la misma carga y la carga siguiente reemplaza.
    #[test]
    fn el_diseno_se_expande_en_el_nivel_0() {
        use crate::lod::DESIGN_SCALE as S;
        let mut s = EditStore::new(3);
        let mut e = Edits::default();
        e.dset(IVec3::new(5, 3, 7), CAL);
        e.dsurface(IVec2::new(5, 7), PLAZA);
        e.set(IVec3::new(5 * S, 3 * S, 7 * S), TILE);
        let touched = s.apply(&e, true);
        let l0 = &s.levels[0];
        let mut n = 0;
        for y in 0..S {
            for z in 0..S {
                for x in 0..S {
                    let v = IVec3::new(5 * S + x, 3 * S + y, 7 * S + z);
                    let want = if (x, y, z) == (0, 0, 0) { TILE } else { CAL };
                    assert_eq!(l0.get(v), Some(want), "{v}");
                    assert_eq!(l0.surface.get(&IVec2::new(v.x, v.z)), Some(&PLAZA));
                    n += 1;
                }
            }
        }
        assert_eq!(n, S * S * S);
        assert_eq!(l0.voxel_count(), 2, "el diseño se guarda sin expandir (un vóxel) más el base");
        let mut seen = Vec::new();
        l0.for_each_in_chunk(chunk_of(IVec3::new(5 * S, 3 * S, 7 * S)), |i, m| seen.push((i, m)));
        assert_eq!(seen.len(), (n + 1) as usize, "el bloque expandido y el vóxel base encima");
        assert_eq!(seen.last().map(|e| e.1), Some(TILE));
        assert_eq!(l0.max_y.get(&IVec2::new(5 * S >> 5, 7 * S >> 5)), Some(&(3 * S + S - 1)));
        assert!(touched[0].contains(&IVec2::new(5 * S >> 5, 7 * S >> 5)));
        // En el nivel 1 (vóxel de diseño) es un solo vóxel: gana el más alto, el de diseño.
        let k = IVec3::new(5 * S, 3 * S, 7 * S) >> 1;
        assert_eq!(s.levels[1].get(k), Some(CAL));
        assert_eq!(s.levels[1].voxel_count(), 1);
        // Demoler el vóxel de diseño borra también el vóxel base que tenía adentro.
        let mut d = Edits::default();
        d.dset(IVec3::new(5, 3, 7), AIR);
        s.apply(&d, false);
        assert_eq!(s.levels[0].get(IVec3::new(5 * S, 3 * S, 7 * S)), Some(AIR));
        assert_eq!(s.levels[1].get(k), Some(AIR));
    }
}
