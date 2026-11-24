//! Alumbrado eléctrico y tranvías de la época 3.
//!
//! - **Alumbrado eléctrico** (en Montevideo, la luz incandescente de la usina del Yerbal es de
//!   1886 [F]): la usina reparte la corriente por las calles de su red, hasta su alcance, con un
//!   farol eléctrico cada 35 m [E]. Donde llega, los faroles de gas se sacan (como el gas sacó a
//!   los de sebo). La luz va por la misma vía que la del gas (`luces_cerca`), más fuerte y de
//!   más alcance; el shader no cambia.
//! - **Tranvías** (a caballo desde 1868, eléctricos desde 1906 en Montevideo [F]): cada cochera
//!   tiende dos líneas por las calles (Dijkstra sobre la red): una hasta la plaza mayor y otra
//!   hasta la punta más lejana de la red a su alcance. Las líneas suben el valor del suelo a
//!   menos de 300 m (`valor_epoca3`), y así la ciudad crece donde llegan. Con una usina
//!   eléctrica, las líneas se electrifican: postes de hierro con el brazo y el cable de
//!   contacto. Los rieles van en la calzada (dos hilos de hierro de un vóxel base).
//!
//! Los coches de tranvía como vehículos (agentes) quedan pendientes (los agentes son de otra
//! línea de trabajo; ver `docs/sistemas/arquitectura-epoca3.md`).

use super::super::services::{comps_cerca, components, Kind};
use super::super::voxelize::road_height;
use super::super::City;
use crate::buildings::Edits;
use crate::lod::{BASE_VOXEL, DESIGN_VOXEL};
use crate::palette::{AIR, HIERRO_FUNDIDO};
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashSet};

/// Un farol eléctrico cada tantos metros de calle [E].
pub const PASO_FAROL_ELECTRICO: f32 = 35.0;
/// Alcance de la luz del farol eléctrico (m) y su intensidad (el de gas: 11 m y 1,0) [E].
pub const LUZ_ELECTRICA: (f32, f32) = (16.0, 1.6);
/// Los solares a menos de esto de una línea de tranvía la usan (m) [E].
pub const RADIO_TRANVIA: f32 = 300.0;
/// Un poste del tranvía eléctrico cada tantos metros [E].
pub const PASO_POSTE: f32 = 30.0;
/// Separación de los rieles (m): la trocha normal, 1,435 m [F].
pub const TROCHA: f32 = 1.435;

/// Una línea de tranvía.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LineaTranvia {
    /// La cochera de donde sale (id del servicio).
    pub cochera: u32,
    /// El recorrido por el eje de las calles (m).
    pub pts: Vec<[f32; 2]>,
    /// Eléctrica (si no, a caballo).
    pub electrica: bool,
    /// Mes en que se tendió.
    pub desde: u32,
}

impl LineaTranvia {
    pub fn largo(&self) -> f32 {
        self.pts.windows(2).map(|w| Vec2::from(w[0]).distance(Vec2::from(w[1]))).sum()
    }
    /// Distancia de un punto al recorrido.
    pub fn distancia(&self, p: Vec2) -> f32 {
        self.pts.windows(2).map(|w| super::super::roads::closest_on_segment(p, Vec2::from(w[0]), Vec2::from(w[1])).distance(p)).fold(f32::MAX, f32::min)
    }
    /// Puntos cada `paso` metros con la dirección.
    fn muestras(&self, paso: f32) -> Vec<(Vec2, Vec2)> {
        let mut out = Vec::new();
        let mut resto = 0.0;
        for w in self.pts.windows(2) {
            let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
            let l = a.distance(b);
            if l < 1e-3 {
                continue;
            }
            let d = (b - a) / l;
            let mut s = resto;
            while s < l {
                out.push((a + d * s, d));
                s += paso;
            }
            resto = s - l;
        }
        out
    }
}

/// Índice del vóxel base de arriba de la calzada en `p`.
fn tope_calzada(t: &TerrainGen, p: Vec2) -> i32 {
    (road_height(t, p) / BASE_VOXEL - 0.5).floor() as i32
}

/// Vóxel de diseño apoyado sobre la calzada en `p`.
fn base_calzada(t: &TerrainGen, p: Vec2) -> i32 {
    (tope_calzada(t, p) + 2).div_euclid(2)
}

fn celda(q: Vec2) -> IVec2 {
    IVec2::new((q.x / DESIGN_VOXEL).floor() as i32, (q.y / DESIGN_VOXEL).floor() as i32)
}

impl City {
    /// Faroles eléctricos del mapa (m).
    pub fn faroles_electricos_m(&self) -> impl Iterator<Item = Vec2> + '_ {
        self.epoca.faroles_electricos.iter().map(|&(v, _)| (Vec2::new(v[0] as f32, v[1] as f32) + 0.5) * DESIGN_VOXEL)
    }

    fn firma_electrica(&self) -> (usize, i64, usize) {
        (self.net.edges.len(), self.net.total_length() as i64, self.services.count(Kind::UsinaElectrica))
    }

    fn firma_tranvias(&self) -> (usize, i64, usize, usize) {
        (self.net.edges.len(), self.net.total_length() as i64, self.services.count(Kind::CocherasTranvia), self.services.count(Kind::UsinaElectrica))
    }

    /// Reparte los faroles eléctricos por las calles de la red de cada usina y saca los de
    /// gas que quedan a su luz.
    pub fn refresh_electrico(&mut self, t: &TerrainGen, e: &mut Edits) {
        let usinas: Vec<(Vec2, f32)> = self.services.of_kind(Kind::UsinaElectrica).map(|b| (b.c, b.info().radius)).collect();
        let mut out: Vec<([i32; 2], i32)> = Vec::new();
        if !usinas.is_empty() {
            let comp = components(&self.net);
            let fuentes: Vec<(Vec2, f32, Vec<u32>)> = usinas.iter().map(|&(c, r)| (c, r, comps_cerca(&self.net, &comp, c, super::redes::RADIO_EMPALME))).collect();
            let mut vistos = HashSet::new();
            for ed in self.net.edges.values() {
                let ce = comp.get(&ed.a).copied();
                let hw = ed.kind.width() * 0.5;
                let mut s = 8.0;
                let mut lado = -1.0;
                while s < ed.len() - 3.0 {
                    let (p, tan) = ed.point(s);
                    if fuentes.iter().any(|(c, r, k)| ce.is_some_and(|ce| k.contains(&ce)) && p.distance(*c) <= *r) {
                        let q = p + Vec2::new(-tan.y, tan.x) * lado * (hw - 0.75);
                        let v = celda(q);
                        if self.net.clearance(q, 20.0) > -1.2 && vistos.insert(v / 12) {
                            out.push(([v.x, v.y], base_calzada(t, q)));
                        }
                    }
                    s += PASO_FAROL_ELECTRICO;
                    lado = -lado;
                }
            }
        }
        let viejos: HashSet<([i32; 2], i32)> = self.epoca.faroles_electricos.iter().copied().collect();
        let nuevos: HashSet<([i32; 2], i32)> = out.iter().copied().collect();
        for &(v, b) in viejos.difference(&nuevos) {
            for y in b..b + 10 {
                e.dset(IVec3::new(v[0], y, v[1]), AIR);
            }
        }
        for &(v, b) in &out {
            crate::structures::epoca3::farol_electrico(e, IVec2::new(v[0], v[1]), b);
        }
        self.epoca.faroles_electricos = out;
        self.epoca.electrica_firma = self.firma_electrica();
        self.quitar_gas_bajo_electrico(e);
    }

    /// Los faroles de gas a la luz de uno eléctrico se sacan.
    pub(crate) fn quitar_gas_bajo_electrico(&mut self, e: &mut Edits) -> usize {
        if self.epoca.faroles_electricos.is_empty() || self.epoca.faroles_gas.is_empty() {
            return 0;
        }
        let el: Vec<Vec2> = self.faroles_electricos_m().collect();
        let antes = self.epoca.faroles_gas.len();
        let mut quedan = Vec::new();
        for &(v, b) in &self.epoca.faroles_gas {
            let q = (Vec2::new(v[0] as f32, v[1] as f32) + 0.5) * DESIGN_VOXEL;
            if el.iter().any(|g| g.distance(q) < 24.0) {
                for y in b..b + 8 {
                    e.dset(IVec3::new(v[0], y, v[1]), AIR);
                }
            } else {
                quedan.push((v, b));
            }
        }
        self.epoca.faroles_gas = quedan;
        antes - self.epoca.faroles_gas.len()
    }

    /// Luces de los faroles eléctricos cerca de un punto (para `luces_cerca`).
    pub(crate) fn luces_electricas(&self, c: Vec2, radio: f32, v: &mut Vec<(f32, [f32; 5])>) {
        for &(p, b) in &self.epoca.faroles_electricos {
            let q = (Vec2::new(p[0] as f32, p[1] as f32) + 0.5) * DESIGN_VOXEL;
            let d = q.distance(c);
            if d < radio {
                v.push((d, [q.x, (b + 8) as f32 * DESIGN_VOXEL + 0.25, q.y, LUZ_ELECTRICA.0, LUZ_ELECTRICA.1]));
            }
        }
    }

    /// Distancia a la línea de tranvía más cercana.
    pub fn distancia_tranvia(&self, p: Vec2) -> Option<f32> {
        self.epoca.tranvias.iter().map(|l| l.distancia(p)).reduce(f32::min)
    }

    /// Solares edificados a menos de [`RADIO_TRANVIA`] de una línea.
    pub fn solares_con_tranvia(&self) -> usize {
        self.lots.lots.values().filter(|l| l.level > 0 && self.distancia_tranvia(l.c).is_some_and(|d| d < RADIO_TRANVIA)).count()
    }

    /// Camino más corto por la red de calles entre dos nodos (Dijkstra): los puntos del eje.
    fn camino(&self, desde: u32, hasta: u32) -> Option<Vec<Vec2>> {
        let mut ady: BTreeMap<u32, Vec<(u32, u32, f32)>> = BTreeMap::new();
        for (&id, ed) in &self.net.edges {
            let l = ed.len();
            ady.entry(ed.a).or_default().push((id, ed.b, l));
            ady.entry(ed.b).or_default().push((id, ed.a, l));
        }
        let mut dist: BTreeMap<u32, f32> = BTreeMap::new();
        let mut prev: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
        let mut cola = BinaryHeap::new();
        dist.insert(desde, 0.0);
        cola.push(std::cmp::Reverse(((0.0f32).to_bits(), desde)));
        while let Some(std::cmp::Reverse((db, n))) = cola.pop() {
            let d = f32::from_bits(db);
            if n == hasta {
                break;
            }
            if d > dist.get(&n).copied().unwrap_or(f32::MAX) {
                continue;
            }
            for &(eid, m, l) in ady.get(&n).map_or(&[][..], |v| &v[..]) {
                let nd = d + l;
                if nd < dist.get(&m).copied().unwrap_or(f32::MAX) {
                    dist.insert(m, nd);
                    prev.insert(m, (n, eid));
                    cola.push(std::cmp::Reverse((nd.to_bits(), m)));
                }
            }
        }
        if !dist.contains_key(&hasta) {
            return None;
        }
        let mut tramos = Vec::new();
        let mut n = hasta;
        while n != desde {
            let (p, eid) = prev[&n];
            tramos.push((p, eid));
            n = p;
        }
        tramos.reverse();
        let mut pts: Vec<Vec2> = Vec::new();
        for (de, eid) in tramos {
            let ed = &self.net.edges[&eid];
            let mut v = ed.pts.clone();
            if ed.a != de {
                v.reverse();
            }
            if !pts.is_empty() {
                v.remove(0);
            }
            pts.extend(v);
        }
        Some(pts)
    }

    /// Nodo de la red más cercano a un punto.
    fn nodo_cerca(&self, p: Vec2) -> Option<u32> {
        self.net.nodes.iter().min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|(&k, _)| k)
    }

    /// Tiende las líneas de cada cochera (o las electrifica) y redibuja rieles y postes.
    pub fn refresh_tranvias(&mut self, t: &TerrainGen, e: &mut Edits) {
        let electrica = self.services.count(Kind::UsinaElectrica) > 0;
        let viejas = std::mem::take(&mut self.epoca.tranvias);
        self.borrar_tranvias(t, &viejas, e);
        let plaza = self.lots.plaza_center;
        let cocheras: Vec<(u32, Vec2, f32)> = self.services.of_kind(Kind::CocherasTranvia).map(|b| (b.id, b.c, b.info().radius)).collect();
        let mut nuevas = Vec::new();
        for (id, c, radio) in cocheras {
            let Some(n0) = self.nodo_cerca(c) else { continue };
            let antes = viejas.iter().find(|l| l.cochera == id).map(|l| l.desde).unwrap_or(self.month);
            let mut destinos = Vec::new();
            if let Some(p) = plaza.and_then(|p| self.nodo_cerca(p)) {
                destinos.push(p);
            }
            // La punta más lejana de la plaza a su alcance (la línea que abre barrios nuevos).
            let lejos = self.net.nodes.iter().filter(|(_, q)| q.distance(c) < radio && self.lots.nearby(**q).iter().any(|id| self.lots.lots.get(id).is_some_and(|l| l.level > 0))).max_by(|a, b| {
                let da = plaza.map_or(0.0, |p| a.1.distance(p)) + a.1.distance(c) * 0.3;
                let db = plaza.map_or(0.0, |p| b.1.distance(p)) + b.1.distance(c) * 0.3;
                da.total_cmp(&db)
            });
            if let Some((&k, _)) = lejos {
                if !destinos.contains(&k) {
                    destinos.push(k);
                }
            }
            for d in destinos {
                if d == n0 {
                    continue;
                }
                if let Some(pts) = self.camino(n0, d) {
                    if pts.len() >= 2 {
                        nuevas.push(LineaTranvia { cochera: id, pts: pts.iter().map(|p| [p.x, p.y]).collect(), electrica, desde: antes });
                    }
                }
            }
        }
        self.epoca.tranvias = nuevas;
        self.epoca.tranvia_firma = self.firma_tranvias();
        let lineas = self.epoca.tranvias.clone();
        for l in &lineas {
            self.dibujar_tranvia(t, l, e);
        }
    }

    /// Rieles (y postes, si es eléctrica) de una línea.
    fn dibujar_tranvia(&self, t: &TerrainGen, l: &LineaTranvia, e: &mut Edits) {
        let mut hechos = HashSet::new();
        for (p, d) in l.muestras(BASE_VOXEL * 0.7) {
            let n = Vec2::new(-d.y, d.x);
            for lado in [-0.5, 0.5] {
                let q = p + n * lado * TROCHA;
                let v = IVec2::new((q.x / BASE_VOXEL).floor() as i32, (q.y / BASE_VOXEL).floor() as i32);
                if hechos.insert(v) {
                    e.set(IVec3::new(v.x, tope_calzada(t, q), v.y), HIERRO_FUNDIDO);
                }
            }
        }
        if l.electrica {
            for v in self.piezas_electricas(t, l) {
                e.set(v, HIERRO_FUNDIDO);
            }
        }
    }

    /// Vóxeles base (0,25 m) de los postes, los brazos y el cable de contacto de una línea
    /// eléctrica: un poste de 6,5 m cada [`PASO_POSTE`] m alternando de vereda, el brazo hasta
    /// el eje y el cable sobre el eje, a 6 m de la calzada.
    fn piezas_electricas(&self, t: &TerrainGen, l: &LineaTranvia) -> Vec<IVec3> {
        let celda_b = |q: Vec2| IVec2::new((q.x / BASE_VOXEL).floor() as i32, (q.y / BASE_VOXEL).floor() as i32);
        let alto = (6.0 / BASE_VOXEL) as i32;
        let mut out = HashSet::new();
        let mut lado = 1.0;
        for (p, d) in l.muestras(PASO_POSTE) {
            let n = Vec2::new(-d.y, d.x) * lado;
            let ancho = self.net.nearest_edge(p, 20.0).map_or(8.0, |ne| self.net.edges[&ne.edge].kind.width());
            let q = p + n * (ancho * 0.5 - 0.4);
            let v = celda_b(q);
            let y0 = tope_calzada(t, q);
            for y in y0 + 1..=y0 + alto + 2 {
                out.insert(IVec3::new(v.x, y, v.y));
            }
            let ya = tope_calzada(t, p) + alto;
            let largo = q.distance(p);
            let mut s = 0.0;
            while s <= largo {
                let w = celda_b(q.lerp(p, s / largo.max(0.01)));
                out.insert(IVec3::new(w.x, ya + 1, w.y));
                s += BASE_VOXEL * 0.7;
            }
            lado = -lado;
        }
        for (p, _) in l.muestras(BASE_VOXEL * 0.7) {
            let v = celda_b(p);
            out.insert(IVec3::new(v.x, tope_calzada(t, p) + alto, v.y));
        }
        let mut v: Vec<IVec3> = out.into_iter().collect();
        v.sort_by_key(|p| (p.x, p.y, p.z));
        v
    }

    /// Borra los postes y el cable de unas líneas y repone la calzada bajo sus rieles.
    fn borrar_tranvias(&self, t: &TerrainGen, lineas: &[LineaTranvia], e: &mut Edits) {
        let mut tramos = BTreeSet::new();
        for l in lineas {
            for (p, _) in l.muestras(4.0) {
                if let Some(ne) = self.net.nearest_edge(p, 6.0) {
                    tramos.insert(ne.edge);
                }
            }
            if l.electrica {
                for v in self.piezas_electricas(t, l) {
                    e.set(v, AIR);
                }
            }
        }
        let plaza = self.plaza.map(|(c, a)| (c, super::super::plaza_frame(a).0));
        for id in tramos {
            if self.net.edges.contains_key(&id) {
                super::super::voxelize::road_edits(t, &self.net, id, plaza, e);
            }
        }
    }

    /// Lo que cambia al levantar o demoler un servicio de la época 3.
    pub(crate) fn servicio_epoca3(&mut self, t: &TerrainGen, k: Kind, e: &mut Edits) {
        match k {
            Kind::UsinaElectrica => {
                self.refresh_electrico(t, e);
                self.refresh_tranvias(t, e);
            }
            Kind::CocherasTranvia => self.refresh_tranvias(t, e),
            _ => {}
        }
    }

    /// El mes del alumbrado y los tranvías: al día con las calles y las usinas.
    pub(crate) fn tranvias_mes(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        if self.epoca.electrica_firma != self.firma_electrica() {
            let antes = self.epoca.faroles_electricos.len();
            self.refresh_electrico(t, e);
            if antes == 0 && !self.epoca.faroles_electricos.is_empty() {
                news.push(format!("Se encienden los primeros {} faroles eléctricos: la luz blanca de la usina reemplaza al gas en las calles de su red", self.epoca.faroles_electricos.len()));
            }
        } else {
            self.quitar_gas_bajo_electrico(e);
        }
        if self.epoca.tranvia_firma != self.firma_tranvias() {
            let antes = self.epoca.tranvias.iter().any(|l| l.electrica);
            self.refresh_tranvias(t, e);
            if !antes && self.epoca.tranvias.iter().any(|l| l.electrica) {
                news.push("Corre el primer tranvía eléctrico: las chispas del trole se ven de noche y los caballos dejan las vías".into());
            }
        }
        news
    }

    /// Vuelve a dibujar lo de la época 3 que no es de un solar ni de un servicio (al cargar).
    pub fn epoca3_rebuild(&self, t: &TerrainGen, e: &mut Edits) {
        for &(v, b) in &self.epoca.faroles_electricos {
            crate::structures::epoca3::farol_electrico(e, IVec2::new(v[0], v[1]), b);
        }
        for l in &self.epoca.tranvias {
            self.dibujar_tranvia(t, l, e);
        }
    }
}
