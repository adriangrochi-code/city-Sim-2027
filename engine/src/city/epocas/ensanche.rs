//! El ensanche de la Ciudad Nueva: una cuadrícula planificada sobre el ejido, fuera de la
//! ciudad vieja, con un bulevar arbolado hacia el interior y plazas cada tantas manzanas.
//!
//! Historia: en 1829 el sargento mayor José María Reyes trazó la "Ciudad Nueva" sobre las
//! tierras del ejido colonial, desde la Ciudadela hasta la actual calle Yaguarón, en damero y
//! sin una nueva muralla que la limitara (las fuentes dan 136 o 160 manzanas) [F, ver
//! `docs/sistemas/arquitectura-epoca2.md`]. Las manzanas siguieron la cuadra de 100 varas de la
//! traza vieja. El bulevar con su alameda y la plaza cada tres manzanas son licencias de
//! diseño [E] (las plazas de la Ciudad Nueva —Cagancha, Independencia— no siguen una regla
//! fija).
//!
//! El trazado se hace con dos clics (un lado y la extensión, como el damero); las calles y los
//! solares son los de siempre: después se zonifica con el pincel.

use super::super::lots::{corners_of, Obstacle};
use super::super::roads::{self, RoadKind};
use super::super::voxelize::{plaza_edits, PLAZA_SIZE};
use super::super::{City, BLOCK};
use crate::buildings::Edits;
use crate::lod::DESIGN_VOXEL;
use crate::palette::*;
use crate::terrain::TerrainGen;
use glam::{IVec3, Vec2};
use serde::{Deserialize, Serialize};

/// Distancia entre los ejes de las dos calzadas del bulevar (dos calles empedradas y el
/// cantero central arbolado de 8 m) [E].
pub const SEPARACION_BULEVAR: f32 = 10.4 + 8.0;
/// Una plaza cada tantas manzanas en cada sentido [E].
pub const PLAZA_CADA: usize = 3;
/// El ensanche va sobre el ejido: ninguna esquina a menos de esto de la plaza mayor (m).
pub const DISTANCIA_MINIMA: f32 = 380.0;
/// Costo de cada plaza (nivelación, árboles, fuente) y de cada árbol de la alameda.
pub const COSTO_PLAZA: i64 = 600;
pub const COSTO_ARBOL: i64 = 3;
/// Árboles de la alameda: uno cada tantos metros.
pub const PASO_ARBOLES: f32 = 9.0;

/// Un trazado de ensanche: esquina, ejes, manzanas y la línea del bulevar.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Trazado {
    pub a: [f32; 2],
    pub u: [f32; 2],
    pub v: [f32; 2],
    pub nu: u32,
    pub nv: u32,
    /// Índice de la línea (a lo largo de `v`) que es bulevar.
    pub bulevar: u32,
    /// Calles de empalme con la red que ya había (ver `empalmar`).
    #[serde(default)]
    pub empalmes: u32,
}

impl Trazado {
    fn ejes(&self) -> (Vec2, Vec2, Vec2) {
        (Vec2::from(self.a), Vec2::from(self.u), Vec2::from(self.v))
    }

    /// ¿El punto está dentro de las manzanas del trazado?
    pub fn contiene(&self, p: Vec2) -> bool {
        let (a, u, v) = self.ejes();
        let q = p - a;
        let (x, y) = (q.dot(u), q.dot(v));
        x >= -5.0 && y >= -5.0 && x <= BLOCK * self.nu as f32 + 5.0 && y <= BLOCK * self.nv as f32 + 5.0
    }
}

/// Lo que se trazó en el ensanche (se guarda; los vóxeles de plazas y alamedas se rehacen al
/// cargar).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Ensanche {
    pub trazados: Vec<Trazado>,
    /// Plazas: centro y frente.
    pub plazas: Vec<([f32; 2], [f32; 2])>,
    /// Árboles de las alamedas.
    pub arboles: Vec<[f32; 2]>,
}

impl Ensanche {
    pub fn contiene(&self, p: Vec2) -> bool {
        self.trazados.iter().any(|t| t.contiene(p))
    }
}

/// El plan de un trazado antes de construirlo.
pub struct Plan {
    pub trazado: Trazado,
    pub lineas: Vec<Vec<Vec2>>,
    pub plazas: Vec<(Vec2, Vec2)>,
    pub arboles: Vec<Vec2>,
}

/// Arma el plan: un lado `a`→`b` (las manzanas a lo largo) extendido hacia `cursor`.
pub fn plan(a: Vec2, b: Vec2, cursor: Vec2) -> Option<Plan> {
    let d = b - a;
    let lu = d.length();
    if lu < BLOCK * 1.5 {
        return None;
    }
    let u = d / lu;
    let mut v = Vec2::new(-u.y, u.x);
    let mut ext = (cursor - a).dot(v);
    if ext < 0.0 {
        v = -v;
        ext = -ext;
    }
    let nu = ((lu / BLOCK).round() as u32).max(2);
    let nv = ((ext / BLOCK).round() as u32).max(2);
    let ib = nu / 2;
    let mut lineas = Vec::new();
    for j in 0..=nv {
        let s = a + v * BLOCK * j as f32;
        lineas.push(roads::line(s, s + u * BLOCK * nu as f32));
    }
    for i in 0..=nu {
        let s = a + u * BLOCK * i as f32;
        if i == ib {
            for k in [-0.5f32, 0.5] {
                let s2 = s + u * SEPARACION_BULEVAR * k;
                lineas.push(roads::line(s2, s2 + v * BLOCK * nv as f32));
            }
        } else {
            lineas.push(roads::line(s, s + v * BLOCK * nv as f32));
        }
    }
    let mut plazas = Vec::new();
    // Una manzana de cada tres (en los dos sentidos) es plaza; las manzanas junto al bulevar
    // son más angostas: la plaza se corre a la de al lado.
    let junto = |i: u32| i == ib || i + 1 == ib;
    for j in (1..nv).step_by(PLAZA_CADA) {
        for i in (1..nu).step_by(PLAZA_CADA) {
            let i = if !junto(i) { Some(i) } else if i >= 1 && !junto(i - 1) { Some(i - 1) } else if i + 1 < nu && !junto(i + 1) { Some(i + 1) } else { None };
            if let Some(i) = i {
                plazas.push((a + u * BLOCK * (i as f32 + 0.5) + v * BLOCK * (j as f32 + 0.5), -v));
            }
        }
    }
    let mut arboles = Vec::new();
    let eje = a + u * BLOCK * ib as f32;
    let mut s = 6.0;
    while s < BLOCK * nv as f32 - 6.0 {
        let k = s % BLOCK;
        if k > 9.0 && k < BLOCK - 9.0 {
            arboles.push(eje + v * s);
        }
        s += PASO_ARBOLES;
    }
    Some(Plan { trazado: Trazado { a: a.into(), u: u.into(), v: v.into(), nu, nv, bulevar: ib, empalmes: 0 }, lineas, plazas, arboles })
}

/// Hasta cuántas calles de empalme une el ensanche con la red vieja, y a qué distancia máxima
/// (m) busca una esquina vieja para cada esquina del borde del trazado [E]. El ensanche no es
/// una ciudad aparte: sus calles siguen hasta las de la ciudad vieja.
pub const EMPALMES_MAX: usize = 6;
pub const EMPALME_LARGO_MAX: f32 = 260.0;
/// Entre dos empalmes, al menos esto (m): una calle de unión por cuadra, no un abanico.
pub const EMPALME_SEPARACION: f32 = BLOCK * 0.9;

/// Candidatos a calle de empalme: (puntaje, esquina del ensanche, esquina vieja), del mejor al
/// peor. Se prefieren las uniones cortas que prolongan una calle del ensanche (alineadas con
/// `u` o `v`): así la calle nueva sigue derecho hasta la red vieja en vez de cruzar en diagonal.
pub fn candidatos_empalme(tr: &Trazado, nodos: &[(u32, Vec2)], dentro: impl Fn(Vec2) -> bool) -> Vec<(f32, Vec2, Vec2)> {
    let (a, u, v) = tr.ejes();
    let (lu, lv) = (BLOCK * tr.nu as f32, BLOCK * tr.nv as f32);
    let borde = |p: Vec2| {
        let q = p - a;
        let (x, y) = (q.dot(u), q.dot(v));
        x < 12.0 || y < 12.0 || x > lu - 12.0 || y > lv - 12.0
    };
    let nuevos: Vec<Vec2> = nodos.iter().map(|x| x.1).filter(|p| tr.contiene(*p) && borde(*p)).collect();
    let viejos: Vec<Vec2> = nodos.iter().map(|x| x.1).filter(|p| !dentro(*p)).collect();
    let centro = a + u * lu * 0.5 + v * lv * 0.5;
    let mut out = Vec::new();
    for &n in &nuevos {
        // Hacia afuera del trazado: la unión no cruza las manzanas nuevas.
        let afuera = (n - centro).normalize_or_zero();
        let mut mejor: Option<(f32, Vec2)> = None;
        for &o in &viejos {
            let d = o.distance(n);
            if !(8.0..EMPALME_LARGO_MAX).contains(&d) {
                continue;
            }
            let dir = (o - n) / d;
            if dir.dot(afuera) < 0.2 {
                continue;
            }
            let alineada = dir.dot(u).abs().max(dir.dot(v).abs());
            let puntaje = d * (2.2 - alineada * 1.2);
            if mejor.is_none_or(|m| puntaje < m.0) {
                mejor = Some((puntaje, o));
            }
        }
        if let Some((p, o)) = mejor {
            out.push((p, n, o));
        }
    }
    out.sort_by(|x, y| x.0.total_cmp(&y.0));
    out
}

/// Árbol de la alameda (paraíso o plátano: los de los paseos del XIX [E]): tronco y copa.
fn arbol(t: &TerrainGen, e: &mut Edits, p: Vec2, seed: u64) {
    let v = (p / DESIGN_VOXEL).floor().as_ivec2();
    let base = crate::structures::top_at(t, v.x, v.y) + 1;
    let alto = 6 + (crate::structures::hash(IVec3::new(v.x, 0, v.y), seed) * 3.0) as i32;
    e.dfill(IVec3::new(v.x, base, v.y), IVec3::new(v.x + 1, base + alto, v.y + 1), TRUNK);
    let r = 3;
    for dy in -1..=3 {
        let rr = if dy == 3 { 1 } else if dy == -1 { 2 } else { r };
        for dz in -rr..=rr {
            for dx in -rr..=rr {
                if dx * dx + dz * dz <= rr * rr + 1 {
                    let q = IVec3::new(v.x + dx, base + alto + dy, v.y + dz);
                    if crate::structures::hash(q, seed ^ 0xA1A) < 0.85 {
                        e.dset(q, if (dx + dz + dy).rem_euclid(4) == 0 { LEAF_DARK } else { LEAF });
                    }
                }
            }
        }
    }
}

/// Vóxeles de una plaza del ensanche: suelo nivelado de la plaza, la columna en el centro y,
/// encima, palmeras, canteros y bancos (ver `structures::epoca2::plaza_ensanche`).
fn plaza_vox(t: &TerrainGen, c: Vec2, f: Vec2, e: &mut Edits) {
    plaza_edits(t, c, f, e);
    let w = (PLAZA_SIZE / DESIGN_VOXEL) as i32;
    // Sobre el suelo nivelado de la plaza (no sobre el terreno de antes).
    let base = super::super::services::road_design_base(t, c);
    let b = super::super::voxelize::build_rotated_at(t, c, f, w, w, 0xE45A, Some(base), &crate::structures::epoca2::plaza_ensanche);
    e.extend(b.edits);
}

impl City {
    /// `None` si el ensanche se puede trazar; si no, el motivo.
    pub fn ensanche_problema(&self, t: &TerrainGen, p: &Plan) -> Option<String> {
        if !self.en_epoca(super::Epoca::CiudadNueva) {
            return Some("El ensanche llega con la época «Ciudad Nueva»".into());
        }
        let plaza = self.lots.plaza_center?;
        let (a, u, v) = p.trazado.ejes();
        let (lu, lv) = (BLOCK * p.trazado.nu as f32, BLOCK * p.trazado.nv as f32);
        for q in [a, a + u * lu, a + v * lv, a + u * lu + v * lv] {
            if q.distance(plaza) < DISTANCIA_MINIMA {
                return Some(format!("El ensanche va sobre el ejido, fuera de la ciudad vieja: a más de {} m de la plaza mayor", DISTANCIA_MINIMA as i32));
            }
        }
        for l in &p.lineas {
            if let Some(m) = self.road_problem(t, l, RoadKind::Stone) {
                return Some(format!("Ensanche: {m}"));
            }
        }
        let costo = self.ensanche_costo(t, p);
        if costo > self.money {
            return Some(format!("Fondos insuficientes: hacen falta {} pesos", super::super::game::fmt_thousands(costo)));
        }
        None
    }

    pub fn ensanche_costo(&self, t: &TerrainGen, p: &Plan) -> i64 {
        p.lineas.iter().map(|l| City::road_cost(t, l, RoadKind::Stone)).sum::<i64>() + p.plazas.len() as i64 * COSTO_PLAZA + p.arboles.len() as i64 * COSTO_ARBOL
    }

    /// Traza el ensanche: calles empedradas, el bulevar con su alameda y las plazas; los
    /// solares nuevos salen solos a lo largo de las calles.
    pub fn trazar_ensanche(&mut self, t: &TerrainGen, a: Vec2, b: Vec2, cursor: Vec2) -> Result<Edits, String> {
        let p = plan(a, b, cursor).ok_or("El ensanche necesita al menos dos manzanas de lado")?;
        if let Some(m) = self.ensanche_problema(t, &p) {
            return Err(m);
        }
        self.money -= self.ensanche_costo(t, &p);
        let mut e = Edits::default();
        // Primero las plazas (para que no se generen solares encima).
        for &(c, f) in &p.plazas {
            self.clear_lots_under(&corners_of(c, f, PLAZA_SIZE, PLAZA_SIZE), &mut e);
            self.lots.obstacles.push(Obstacle { c, f, w: PLAZA_SIZE - 1.0, d: PLAZA_SIZE - 1.0 });
        }
        let mut new = Vec::new();
        for l in &p.lineas {
            new.extend(self.net.add_road(l, RoadKind::Stone));
        }
        new.retain(|id| self.net.edges.contains_key(id));
        new.sort();
        new.dedup();
        self.after_roads(t, &new, &mut e);
        for &(c, f) in &p.plazas {
            plaza_vox(t, c, f, &mut e);
        }
        for q in &p.arboles {
            arbol(t, &mut e, *q, SEMILLA_ARBOL);
        }
        let mut trazado = p.trazado;
        trazado.empalmes = self.empalmar(t, &trazado, &mut e);
        let en = &mut self.epoca.ensanche;
        en.trazados.push(trazado);
        en.plazas.extend(p.plazas.iter().map(|&(c, f)| (c.into(), f.into())));
        en.arboles.extend(p.arboles.iter().map(|&q| <[f32; 2]>::from(q)));
        Ok(e)
    }

    /// Une las calles del ensanche con la red que ya había: calles empedradas desde las esquinas
    /// del borde del trazado hasta las esquinas viejas más cercanas (hasta `EMPALMES_MAX`, una
    /// por cuadra). Sin empalme las redes de gas, agua y colectores (que siguen la calle) no
    /// llegaban al ensanche. Devuelve cuántas se hicieron (se pagan aparte, si alcanza).
    fn empalmar(&mut self, t: &TerrainGen, tr: &Trazado, e: &mut Edits) -> u32 {
        let nodos: Vec<(u32, Vec2)> = self.net.nodes.iter().map(|(&k, &p)| (k, p)).collect();
        let otros = self.epoca.ensanche.trazados.clone();
        let cand = candidatos_empalme(tr, &nodos, |p| tr.contiene(p) || otros.iter().any(|o| o.contiene(p)));
        let mut hechos: Vec<Vec2> = Vec::new();
        for (_, n, o) in cand {
            if hechos.len() >= EMPALMES_MAX {
                break;
            }
            if hechos.iter().any(|h| h.distance(n) < EMPALME_SEPARACION) {
                continue;
            }
            let pts = roads::line(n, o);
            if self.road_problem(t, &pts, RoadKind::Stone).is_some() {
                continue;
            }
            if let Ok(ed) = self.build_road(t, &pts, RoadKind::Stone) {
                e.extend(ed);
                hechos.push(n);
            }
        }
        hechos.len() as u32
    }

    /// Vóxeles de las plazas y alamedas del ensanche (al cargar la partida).
    pub fn ensanche_rebuild(&self, t: &TerrainGen, e: &mut Edits) {
        let en = &self.epoca.ensanche;
        for &(c, f) in &en.plazas {
            plaza_vox(t, Vec2::from(c), Vec2::from(f), e);
        }
        for q in &en.arboles {
            arbol(t, e, Vec2::from(*q), SEMILLA_ARBOL);
        }
    }
}

/// Semilla de los árboles de la alameda (cada uno varía con su posición).
const SEMILLA_ARBOL: u64 = 0x7A1;
