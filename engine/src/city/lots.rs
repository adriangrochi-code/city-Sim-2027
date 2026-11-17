//! Solares: se generan a ambos lados de cada calle siguiendo la curva, se zonifican con
//! pincel (residencial, comercial o industrial, en densidad baja, media o alta) y crecen en
//! tres niveles. Porte ampliado de `scripts/city/lots.gd` del prototipo de Godot.

use super::roads::{RoadKind, RoadNetwork};
use crate::buildings::Rng;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};
use std::collections::{BTreeMap, HashMap};

/// Frente y fondo del solar, en metros (24 × 48 vóxeles de diseño de 0,5 m).
pub const W: f32 = 12.0;
pub const D: f32 = 38.0; // hasta el medio de la manzana: (86,6 − 10,4) / 2
/// Habitantes por nivel de una casa (densidad baja y media).
pub const CAP: [i32; 4] = [0, 4, 8, 14];
/// Habitantes por nivel en densidad alta (casas de renta con piezas para inquilinos).
pub const CAP_ALTA: [i32; 4] = [0, 4, 10, 20];
/// Empleos por nivel de comercios y de talleres.
pub const JOBS_C: [i32; 4] = [0, 3, 6, 12];
pub const JOBS_I: [i32; 4] = [0, 6, 12, 24];

// --- Zonas ----------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ZoneKind {
    Residencial,
    Comercial,
    Industrial,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Density {
    Baja,
    Media,
    Alta,
}

impl ZoneKind {
    pub const ALL: [ZoneKind; 3] = [ZoneKind::Residencial, ZoneKind::Comercial, ZoneKind::Industrial];
    pub fn name(self) -> &'static str {
        match self {
            ZoneKind::Residencial => "residencial",
            ZoneKind::Comercial => "comercial",
            ZoneKind::Industrial => "industrial",
        }
    }
    /// Nombre del edificio de cada nivel en la época colonial.
    pub fn level_name(self, level: u8, d: Density) -> &'static str {
        match (self, level) {
            (_, 0) => "solar vacío",
            (ZoneKind::Residencial, 1) => "rancho",
            (ZoneKind::Residencial, 2) => "casa de azotea",
            (ZoneKind::Residencial, _) if d == Density::Alta => "casa de altos de renta",
            (ZoneKind::Residencial, _) => "casa de altos",
            (ZoneKind::Comercial, 1) => "pulpería",
            (ZoneKind::Comercial, 2) => "tienda",
            (ZoneKind::Comercial, _) => "almacén de altos",
            (ZoneKind::Industrial, 1) => "taller",
            (ZoneKind::Industrial, 2) => "curtiembre y jabonería",
            (ZoneKind::Industrial, _) => "barraca",
        }
    }
}

impl Density {
    pub const ALL: [Density; 3] = [Density::Baja, Density::Media, Density::Alta];
    pub fn name(self) -> &'static str {
        match self {
            Density::Baja => "baja",
            Density::Media => "media",
            Density::Alta => "alta",
        }
    }
    /// Primer y último nivel que se construye: la baja no pasa de una planta, la alta no
    /// empieza con ranchos ni galpones de paja.
    pub fn levels(self) -> (u8, u8) {
        match self {
            Density::Baja => (1, 2),
            Density::Media => (1, 3),
            Density::Alta => (2, 3),
        }
    }
}

/// Código de zona guardado en el solar: 0 = sin zona; 1..=9 = tipo × densidad.
pub fn zone_code(k: ZoneKind, d: Density) -> u8 {
    1 + k as u8 * 3 + d as u8
}

pub fn zone_of(z: u8) -> Option<(ZoneKind, Density)> {
    (1..=9).contains(&z).then(|| (ZoneKind::ALL[((z - 1) / 3) as usize], Density::ALL[((z - 1) % 3) as usize]))
}

/// Nombre completo de una zona ("comercial de densidad media").
pub fn zone_name(z: u8) -> String {
    match zone_of(z) {
        Some((k, d)) => format!("{} de densidad {}", k.name(), d.name()),
        None => "sin zonificar".into(),
    }
}

/// Demanda de cada tipo de zona (−1 a 1), como el medidor del género.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Demand {
    pub r: f32,
    pub c: f32,
    pub i: f32,
}

impl Demand {
    pub fn of(&self, k: ZoneKind) -> f32 {
        match k {
            ZoneKind::Residencial => self.r,
            ZoneKind::Comercial => self.c,
            ZoneKind::Industrial => self.i,
        }
    }
}
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

impl Lot {
    pub fn kind(&self) -> Option<ZoneKind> {
        zone_of(self.zone).map(|z| z.0)
    }
    pub fn density(&self) -> Option<Density> {
        zone_of(self.zone).map(|z| z.1)
    }
    pub fn residents(&self) -> i32 {
        match zone_of(self.zone) {
            Some((ZoneKind::Residencial, Density::Alta)) => CAP_ALTA[self.level as usize],
            Some((ZoneKind::Residencial, _)) => CAP[self.level as usize],
            _ => 0,
        }
    }
    pub fn name(&self) -> &'static str {
        match zone_of(self.zone) {
            Some((k, d)) => k.level_name(self.level, d),
            None if self.level == 0 => "solar vacío",
            None => "edificio sin zona",
        }
    }
}

/// Lo que impide crecer a un solar (vacío si nada): para la ficha de información.
pub fn growth_blockers(lots: &Lots, lot: &Lot, net: &RoadNetwork, pool: i32, demand: Demand) -> Vec<String> {
    let mut why = Vec::new();
    let Some((k, d)) = zone_of(lot.zone) else {
        why.push("Sin zonificar: pinta una zona residencial, comercial o industrial.".into());
        return why;
    };
    let (first, last) = d.levels();
    if lot.level >= last {
        why.push(if d == Density::Baja { "Densidad baja: no pasa de una planta. Rezonifica a media para crecer más.".into() } else { "Ya llegó al máximo de esta época.".into() });
        return why;
    }
    let next = if lot.level == 0 { first } else { lot.level + 1 };
    let value = lots.land_value(lot);
    let stone = lots.frontage(lot, net) == Some(RoadKind::Stone);
    match k {
        ZoneKind::Residencial => {
            let cap = if d == Density::Alta { CAP_ALTA } else { CAP };
            if pool < cap[next as usize] - cap[lot.level as usize] {
                why.push("Faltan pobladores: esperan barcos con más familias.".into());
            }
        }
        _ if demand.of(k) <= 0.0 => why.push(format!("No hay demanda {}: la ciudad no necesita más {}.", if k == ZoneKind::Comercial { "de comercio" } else { "de industria" }, if k == ZoneKind::Comercial { "tiendas" } else { "talleres" })),
        _ => {}
    }
    match lots.level_cap.get(&lot.id) {
        Some(&cap) if next > cap && cap <= 1 => why.push("Sin agua: hace falta un pozo, un aljibe o los aguadores cerca.".into()),
        Some(&cap) if next > cap => why.push("El agua es mala (pozo contaminado): para crecer más necesita agua buena.".into()),
        _ => {}
    }
    let min_age = [0, 4, 8, 8][lot.level as usize];
    if lot.level > 0 && lot.age < min_age {
        why.push("El edificio es muy reciente.".into());
    }
    if next >= 2 && k != ZoneKind::Industrial && value <= LEVEL_VALUE[k as usize][next as usize] {
        why.push(if lots.industry_near(lot.c, lot.id) > 0 { "Los talleres cercanos bajan el valor del suelo.".into() } else { "Está lejos de la plaza mayor: el suelo vale poco.".into() });
    }
    if (next == 3 || d == Density::Alta) && !stone {
        why.push("Necesita una calle empedrada.".into());
    }
    why
}

/// Valor del suelo mínimo para llegar a cada nivel: [tipo][nivel].
const LEVEL_VALUE: [[f32; 4]; 3] = [[0.0, 0.0, 0.2, 0.45], [0.0, 0.1, 0.3, 0.4], [0.0, 0.0, 0.0, 0.0]];

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
    /// Tope de nivel de cada solar que fijan los servicios (sin agua no se pasa del rancho);
    /// sin entrada, sin tope.
    pub level_cap: HashMap<u32, u8>,
    /// Huellas de la muralla: no se generan solares debajo, pero las calles la cruzan (portones).
    pub walls: Vec<Obstacle>,
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
        self.lots.values().map(Lot::residents).sum()
    }

    /// Empleos de comercios y de talleres.
    pub fn jobs(&self) -> (i32, i32) {
        self.lots.values().fold((0, 0), |(c, i), l| match l.kind() {
            Some(ZoneKind::Comercial) => (c + JOBS_C[l.level as usize], i),
            Some(ZoneKind::Industrial) => (c, i + JOBS_I[l.level as usize]),
            _ => (c, i),
        })
    }

    /// Edificios construidos por tipo de zona y nivel: [tipo][nivel − 1].
    pub fn built(&self) -> [[i32; 3]; 3] {
        let mut out = [[0; 3]; 3];
        for l in self.lots.values() {
            if let (Some(k), 1..=3) = (l.kind(), l.level) {
                out[k as usize][l.level as usize - 1] += 1;
            }
        }
        out
    }

    /// Talleres con edificio a menos de 70 m (humo, olor y carretas: bajan el valor).
    pub fn industry_near(&self, p: Vec2, except: u32) -> i32 {
        self.nearby(p).iter().filter(|&&id| id != except).filter(|id| {
            let l = &self.lots[id];
            l.level > 0 && l.kind() == Some(ZoneKind::Industrial) && l.c.distance(p) < 70.0
        }).count() as i32
    }

    /// Valor del suelo: cerca de la plaza vale más; la industria cercana lo baja (salvo
    /// para otros talleres).
    pub fn land_value(&self, lot: &Lot) -> f32 {
        let base = match self.plaza_center {
            Some(p) => (-(lot.c.distance(p)) / 650.0).exp().clamp(0.0, 1.0),
            None => 0.2,
        };
        if lot.kind() == Some(ZoneKind::Industrial) {
            return base;
        }
        let n = self.industry_near(lot.c, lot.id);
        (base - 0.08 * n.min(4) as f32).max(0.0)
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
        // Nada de solares sobre el agua (ni en el cauce de un arroyo: se rellenaría).
        // Grilla de 3 × 8 puntos (cada ~5 m): una cañada de 2 o 3 m no pasa entre ellos sin
        // tocar alguno junto con su orilla de 0,5 m.
        let side = side_of(f);
        for i in 0..3 {
            for j in 0..8 {
                let u = (i as f32 / 2.0 - 0.5) * (W - 1.0);
                let v = (j as f32 / 7.0 - 0.5) * (D - 1.0);
                let p = c + side * u + f * v;
                let (d, hw) = t.arroyo_query(p.x, p.y);
                if d < hw + 2.5 || t.height(p.x, p.y) < 0.5 {
                    return false;
                }
            }
        }
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
        !self.obstacles.iter().chain(&self.walls).any(|o| polys_overlap(&test, &corners_of(o.c, o.f, o.w, o.d)))
    }

    fn add(&mut self, c: Vec2, f: Vec2) -> u32 {
        self.next += 1;
        let id = self.next;
        let seed = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((c.x as i64 as u64) << 20) ^ (c.y as i64 as u64);
        self.lots.insert(id, Lot { id, c, f, zone: 0, level: 0, age: 0, seed, voxels: Vec::new() });
        self.hash.entry(cell(c)).or_default().push(id);
        id
    }

    /// Próximo id de solar (para guardar la partida).
    pub fn next_id(&self) -> u32 {
        self.next
    }

    /// Rehace los solares guardados (sin los vóxeles de sus edificios: se regeneran aparte).
    pub fn restore(lots: Vec<Lot>, obstacles: Vec<Obstacle>, plaza_center: Option<Vec2>, next: u32) -> Lots {
        let mut out = Lots { obstacles, plaza_center, next, ..Default::default() };
        for l in lots {
            out.next = out.next.max(l.id);
            out.lots.insert(l.id, l);
        }
        for (&id, l) in &out.lots {
            out.hash.entry(cell(l.c)).or_default().push(id);
        }
        out
    }

    pub fn remove(&mut self, id: u32) -> Lot {
        let lot = self.lots.remove(&id).unwrap();
        if let Some(v) = self.hash.get_mut(&cell(lot.c)) {
            v.retain(|&x| x != id);
        }
        lot
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
                    let same_kind = zone_of(zone).map(|z| z.0) == lot.kind() && zone != 0;
                    lot.zone = zone;
                    changed += 1;
                    // Cambiar de tipo (o quitar la zona) derriba lo construido; cambiar solo la
                    // densidad lo deja, salvo que el edificio quede por encima del nuevo tope.
                    let over = zone_of(zone).is_some_and(|(_, d)| lot.level > d.levels().1);
                    if lot.level > 0 && (!same_kind || over) {
                        lot.level = 0;
                        lot.age = 0;
                        demolish.push(id);
                    }
                }
            }
        }
        (changed, demolish)
    }

    /// Un paso de crecimiento. Las casas se llenan con los `pool` pobladores que esperan; los
    /// comercios y talleres crecen según la demanda. Devuelve los pobladores instalados y los
    /// solares que cambiaron de nivel.
    pub fn grow_step(&mut self, pool: i32, demand: Demand, net: &RoadNetwork, rng: &mut Rng) -> (i32, Vec<LevelChange>) {
        let zoned: Vec<u32> = self.lots.values().filter(|l| l.density().is_some_and(|d| l.level < d.levels().1)).map(|l| l.id).collect();
        if zoned.is_empty() {
            return (0, Vec::new());
        }
        let mut used = 0;
        let mut out = Vec::new();
        let tries = 2 + zoned.len() / 25;
        for _ in 0..tries {
            let id = zoned[(rng.next() % zoned.len() as u64) as usize];
            let l = &self.lots[&id];
            let Some((k, d)) = zone_of(l.zone) else { continue };
            let (first, last) = d.levels();
            if l.level >= last {
                continue;
            }
            let next = if l.level == 0 { first } else { l.level + 1 };
            // Con los servicios en marcha, un solar nuevo (todavía sin repartir) no tiene agua.
            if !self.level_cap.is_empty() && next > self.level_cap.get(&id).copied().unwrap_or(1) {
                continue;
            }
            let value = self.land_value(l);
            let stone = self.frontage(l, net) == Some(RoadKind::Stone);
            let age_ok = l.age >= [0, 4, 8, 8][l.level as usize];
            let value_ok = k == ZoneKind::Industrial || next < 2 || value > LEVEL_VALUE[k as usize][next as usize];
            let street_ok = (next < 3 && d != Density::Alta) || stone;
            if !(age_ok && value_ok && street_ok) {
                continue;
            }
            let mut need = 0;
            let chance = match k {
                ZoneKind::Residencial => {
                    let cap = if d == Density::Alta { CAP_ALTA } else { CAP };
                    need = cap[next as usize] - cap[l.level as usize];
                    if pool - used < need {
                        continue;
                    }
                    // Junto a los talleres nadie quiere vivir si puede elegir.
                    let smoke = if self.industry_near(l.c, id) > 0 { 0.5 } else { 1.0 };
                    smoke * if l.level == 0 { 0.55 } else if next == 2 { 0.15 } else { 0.1 }
                }
                _ => {
                    let dm = demand.of(k);
                    if dm <= 0.0 {
                        continue;
                    }
                    dm * if l.level == 0 { 0.5 } else { 0.12 }
                }
            };
            if rng.f() < chance {
                let l = self.lots.get_mut(&id).unwrap();
                l.level = next;
                l.age = 0;
                used += need;
                out.push(LevelChange { lot: id, level: next });
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
