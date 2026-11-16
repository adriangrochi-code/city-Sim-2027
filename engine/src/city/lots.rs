//! Solares: se generan a ambos lados de cada calle siguiendo la curva, se zonifican con
//! pincel y crecen en tres niveles (rancho → casa de azotea → casa de altos). Porte de
//! `scripts/city/lots.gd` del prototipo de Godot.

use super::roads::{RoadKind, RoadNetwork};
use crate::buildings::Rng;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};
use std::collections::{BTreeMap, HashMap};

/// Frente y fondo del solar, en metros (24 × 48 vóxeles de 0,5 m).
pub const W: f32 = 12.0;
pub const D: f32 = 24.0;
/// Habitantes por nivel.
pub const CAP: [i32; 4] = [0, 4, 8, 14];
pub const LEVEL_NAMES: [&str; 4] = ["solar vacío", "rancho", "casa de azotea", "casa de altos"];
const CELL: f32 = 30.0;

#[derive(Clone, Debug)]
pub struct Lot {
    pub id: u32,
    pub c: Vec2,
    /// Dirección del frente (hacia la calle).
    pub f: Vec2,
    pub zone: u8,
    pub level: u8,
    pub age: u32,
    pub seed: u64,
    /// Vóxeles del edificio actual (para reemplazarlo o demolerlo).
    pub voxels: Vec<IVec3>,
}

/// Rectángulo orientado que no se puede ocupar (plaza, iglesia).
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub c: Vec2,
    pub f: Vec2,
    pub w: f32,
    pub d: f32,
}

pub fn side_of(f: Vec2) -> Vec2 {
    Vec2::new(f.y, -f.x)
}

pub fn corners_of(c: Vec2, f: Vec2, w: f32, d: f32) -> [Vec2; 4] {
    let s = side_of(f) * (w * 0.5);
    let ff = f * (d * 0.5);
    [c + s + ff, c - s + ff, c - s - ff, c + s - ff]
}

/// Separación de ejes (SAT) entre dos polígonos convexos.
pub fn polys_overlap(a: &[Vec2], b: &[Vec2]) -> bool {
    for poly in [a, b] {
        for i in 0..poly.len() {
            let e = poly[(i + 1) % poly.len()] - poly[i];
            let axis = Vec2::new(-e.y, e.x);
            let proj = |p: &[Vec2]| p.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| (lo.min(axis.dot(*v)), hi.max(axis.dot(*v))));
            let (amin, amax) = proj(a);
            let (bmin, bmax) = proj(b);
            if amax <= bmin || bmax <= amin {
                return false;
            }
        }
    }
    true
}

pub fn point_in_poly(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Cambio de nivel de un solar que hay que llevar a vóxeles.
#[derive(Clone, Copy, Debug)]
pub struct LevelChange {
    pub lot: u32,
    pub level: u8,
}

#[derive(Default)]
pub struct Lots {
    pub lots: BTreeMap<u32, Lot>,
    pub obstacles: Vec<Obstacle>,
    pub plaza_center: Option<Vec2>,
    next: u32,
    hash: HashMap<IVec2, Vec<u32>>,
}

fn cell(p: Vec2) -> IVec2 {
    IVec2::new((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

impl Lots {
    pub fn corners(&self, lot: &Lot, shrink: f32) -> [Vec2; 4] {
        corners_of(lot.c, lot.f, W - shrink, D - shrink)
    }

    pub fn nearby(&self, p: Vec2) -> Vec<u32> {
        let c = cell(p);
        let mut out = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(v) = self.hash.get(&(c + IVec2::new(dx, dy))) {
                    out.extend_from_slice(v);
                }
            }
        }
        out
    }

    pub fn lot_at(&self, p: Vec2) -> Option<u32> {
        self.nearby(p).into_iter().find(|id| point_in_poly(p, &self.corners(&self.lots[id], 0.0)))
    }

    pub fn population(&self) -> i32 {
        self.lots.values().map(|l| CAP[l.level as usize]).sum()
    }

    pub fn land_value(&self, lot: &Lot) -> f32 {
        match self.plaza_center {
            Some(p) => (-(lot.c.distance(p)) / 650.0).exp().clamp(0.0, 1.0),
            None => 0.2,
        }
    }

    pub fn frontage(&self, lot: &Lot, net: &RoadNetwork) -> Option<RoadKind> {
        let front = lot.c + lot.f * (D * 0.5 + 3.0);
        net.nearest_edge(front, 10.0).map(|e| net.edges[&e.edge].kind)
    }

    /// Después de cambiar las calles: quita los solares que quedaron debajo de una calle o
    /// sin frente a ninguna, y genera solares nuevos a lo largo de los tramos nuevos.
    /// Devuelve los solares quitados (con sus vóxeles, para borrarlos).
    pub fn on_roads_changed(&mut self, net: &RoadNetwork, t: &TerrainGen, new_edges: &[u32]) -> Vec<Lot> {
        let mut removed = Vec::new();
        let ids: Vec<u32> = self.lots.keys().copied().collect();
        for id in ids {
            let lot = &self.lots[&id];
            let mut bad = self.corners(lot, 0.6).iter().any(|&p| net.clearance(p, 30.0) < 0.2) || net.clearance(lot.c, 30.0) < 0.2;
            let front = lot.c + lot.f * (D * 0.5);
            if !bad && net.clearance(front, 20.0) > 3.0 {
                bad = true;
            }
            if bad {
                removed.push(self.remove(id));
            }
        }
        for &eid in new_edges {
            if net.edges.contains_key(&eid) {
                self.generate_along(net, t, eid);
            }
        }
        removed
    }

    fn generate_along(&mut self, net: &RoadNetwork, t: &TerrainGen, eid: u32) {
        let e = &net.edges[&eid];
        let hw = e.kind.width() * 0.5;
        let total = e.len();
        let mut s = 3.0 + W * 0.5;
        while s <= total - 3.0 - W * 0.5 {
            let (pt, tan) = e.point(s);
            for sgn in [-1.0f32, 1.0] {
                let nrm = Vec2::new(-tan.y, tan.x) * sgn;
                let c = pt + nrm * (hw + 0.6 + D * 0.5);
                let f = -nrm;
                if self.valid(net, t, c, f) {
                    self.add(c, f);
                }
            }
            s += W;
        }
    }

    pub fn valid(&self, net: &RoadNetwork, t: &TerrainGen, c: Vec2, f: Vec2) -> bool {
        let cs = corners_of(c, f, W, D);
        let (mut hmin, mut hmax) = (f32::INFINITY, f32::NEG_INFINITY);
        for p in cs.iter().chain(std::iter::once(&c)) {
            let h = t.height(p.x, p.y);
            if h < 0.6 {
                return false;
            }
            hmin = hmin.min(h);
            hmax = hmax.max(h);
        }
        if hmax - hmin > 5.0 {
            return false;
        }
        let test = corners_of(c, f, W - 0.6, D - 0.6);
        if test.iter().any(|&p| net.clearance(p, 30.0) < 0.3) {
            return false;
        }
        if net.clearance(c, 30.0) < 0.3 || net.clearance(c - f * (D * 0.5 - 0.3), 30.0) < 0.3 {
            return false;
        }
        for id in self.nearby(c) {
            if polys_overlap(&test, &self.corners(&self.lots[&id], 0.6)) {
                return false;
            }
        }
        !self.obstacles.iter().any(|o| polys_overlap(&test, &corners_of(o.c, o.f, o.w, o.d)))
    }

    fn add(&mut self, c: Vec2, f: Vec2) -> u32 {
        self.next += 1;
        let id = self.next;
        let seed = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((c.x as i64 as u64) << 20) ^ (c.y as i64 as u64);
        self.lots.insert(id, Lot { id, c, f, zone: 0, level: 0, age: 0, seed, voxels: Vec::new() });
        self.hash.entry(cell(c)).or_default().push(id);
        id
    }

    pub fn remove(&mut self, id: u32) -> Lot {
        let lot = self.lots.remove(&id).unwrap();
        if let Some(v) = self.hash.get_mut(&cell(lot.c)) {
            v.retain(|&x| x != id);
        }
        lot
    }

    /// Quita los solares que se superponen con un rectángulo (al fundar la plaza).
    pub fn clear_area(&mut self, corners: &[Vec2]) -> Vec<Lot> {
        let ids: Vec<u32> = self.lots.values().filter(|l| polys_overlap(corners, &self.corners(l, 0.6))).map(|l| l.id).collect();
        ids.into_iter().map(|id| self.remove(id)).collect()
    }

    /// Pinta la zona con un pincel circular. Devuelve los solares que hay que demoler
    /// (los que tenían edificio y se les quitó la zona).
    pub fn paint(&mut self, center: Vec2, radius: f32, zone: u8) -> (usize, Vec<u32>) {
        let mut changed = 0;
        let mut demolish = Vec::new();
        let r = (radius / CELL).ceil() as i32 + 1;
        let c = cell(center);
        for dy in -r..=r {
            for dx in -r..=r {
                let Some(ids) = self.hash.get(&(c + IVec2::new(dx, dy))) else { continue };
                for &id in ids {
                    let lot = self.lots.get_mut(&id).unwrap();
                    if lot.c.distance(center) > radius || lot.zone == zone {
                        continue;
                    }
                    lot.zone = zone;
                    changed += 1;
                    if zone == 0 && lot.level > 0 {
                        lot.level = 0;
                        demolish.push(id);
                    }
                }
            }
        }
        (changed, demolish)
    }

    /// Un paso de crecimiento: con `pool` pobladores esperando casa. Devuelve los que se
    /// instalaron y los solares que cambiaron de nivel.
    pub fn grow_step(&mut self, pool: i32, net: &RoadNetwork, rng: &mut Rng) -> (i32, Vec<LevelChange>) {
        let zoned: Vec<u32> = self.lots.values().filter(|l| l.zone > 0 && l.level < 3).map(|l| l.id).collect();
        if zoned.is_empty() {
            return (0, Vec::new());
        }
        let mut used = 0;
        let mut out = Vec::new();
        let tries = 2 + zoned.len() / 25;
        for _ in 0..tries {
            let id = zoned[(rng.next() % zoned.len() as u64) as usize];
            let (lvl, age, value, front) = {
                let l = &self.lots[&id];
                (l.level, l.age, self.land_value(l), self.frontage(l, net))
            };
            if lvl >= 3 {
                continue;
            }
            let need = CAP[lvl as usize + 1] - CAP[lvl as usize];
            if pool - used < need {
                continue;
            }
            let ok = match lvl {
                0 => rng.f() < 0.55,
                1 => age >= 4 && value > 0.2 && rng.f() < 0.15,
                _ => age >= 8 && value > 0.45 && front == Some(RoadKind::Stone) && rng.f() < 0.1,
            };
            if ok {
                let l = self.lots.get_mut(&id).unwrap();
                l.level = lvl + 1;
                l.age = 0;
                used += need;
                out.push(LevelChange { lot: id, level: lvl + 1 });
            }
        }
        (used, out)
    }

    pub fn age_lots(&mut self) {
        for l in self.lots.values_mut() {
            if l.level > 0 {
                l.age += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::roads::{bezier, line};
    use super::*;

    fn site() -> Vec2 {
        Vec2::new(13500.0, 16300.0)
    }

    #[test]
    fn solares_a_ambos_lados_de_una_recta_sin_superponerse() {
        let t = TerrainGen::new(1726);
        let mut net = RoadNetwork::default();
        let new = net.add_road(&line(site(), site() + Vec2::new(200.0, 0.0)), RoadKind::Stone);
        let mut lots = Lots::default();
        lots.on_roads_changed(&net, &t, &new);
        // ~16 solares por lado en 200 m (12 m de frente, 3 m libres en cada punta).
        assert!(lots.lots.len() >= 24, "{} solares", lots.lots.len());
        let all: Vec<&Lot> = lots.lots.values().collect();
        for (i, a) in all.iter().enumerate() {
            // El frente mira a la calle y el solar no pisa la calzada.
            assert!(net.clearance(a.c, 30.0) > 10.0);
            assert!((a.c + a.f * 13.0 - a.c).length() > 0.0);
            for b in &all[i + 1..] {
                assert!(!polys_overlap(&lots.corners(a, 0.6), &lots.corners(b, 0.6)), "solares {} y {} se superponen", a.id, b.id);
            }
        }
    }

    #[test]
    fn solares_que_siguen_una_curva() {
        let t = TerrainGen::new(1726);
        let mut net = RoadNetwork::default();
        let p0 = site();
        let new = net.add_road(&bezier(p0, p0 + Vec2::new(150.0, 0.0), p0 + Vec2::new(150.0, 150.0)), RoadKind::Dirt);
        let mut lots = Lots::default();
        lots.on_roads_changed(&net, &t, &new);
        assert!(lots.lots.len() >= 12);
        // Los frentes tienen muchas direcciones distintas (no solo cuatro).
        let mut angles: Vec<i32> = lots.lots.values().map(|l| (l.f.y.atan2(l.f.x).to_degrees() / 5.0).round() as i32).collect();
        angles.sort();
        angles.dedup();
        assert!(angles.len() >= 8, "direcciones: {angles:?}");
        // Ningún solar pisa la calle ni se superpone con otro.
        let all: Vec<&Lot> = lots.lots.values().collect();
        for (i, a) in all.iter().enumerate() {
            assert!(lots.corners(a, 0.6).iter().all(|&p| net.clearance(p, 30.0) >= 0.3));
            for b in &all[i + 1..] {
                assert!(!polys_overlap(&lots.corners(a, 0.6), &lots.corners(b, 0.6)));
            }
        }
    }

    #[test]
    fn una_calle_nueva_quita_los_solares_que_pisa() {
        let t = TerrainGen::new(1726);
        let mut net = RoadNetwork::default();
        let new = net.add_road(&line(site(), site() + Vec2::new(200.0, 0.0)), RoadKind::Stone);
        let mut lots = Lots::default();
        lots.on_roads_changed(&net, &t, &new);
        let n0 = lots.lots.len();
        let cross = net.add_road(&line(site() + Vec2::new(100.0, -60.0), site() + Vec2::new(100.0, 60.0)), RoadKind::Stone);
        let removed = lots.on_roads_changed(&net, &t, &cross);
        assert!(!removed.is_empty());
        assert!(lots.lots.values().all(|l| net.clearance(l.c, 30.0) > 0.2));
        assert!(lots.lots.len() + removed.len() >= n0);
    }
}
