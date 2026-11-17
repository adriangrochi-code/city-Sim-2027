//! Grandes obras por fases: la iglesia matriz (de la capilla de adobe a la iglesia con
//! cúpula y torres) y la muralla, tramo a tramo, con portones donde cruza una calle. Cada
//! fase cuesta pesos al empezar y consume ladrillos y piedra de las existencias mes a mes: si
//! faltan, la obra se para. Porte de `scripts/city/church.gd` y `scripts/city/wall.gd`.

use super::super::lots::{corners_of, polys_overlap, Obstacle};
use super::super::roads::segment_intersection;
use super::super::voxelize::{build_rotated, build_rotated_at, Built};
use super::super::City;
use crate::buildings::Edits;
use crate::palette::AIR;
use crate::structures::colonial;
use crate::terrain::TerrainGen;
use glam::{IVec3, Vec2};
use std::collections::HashSet;

use super::super::economy::Res;

// --- Iglesia matriz ---------------------------------------------------------------------------

/// Una fase de obra: costo al empezar, materiales (en total) y meses.
pub struct Phase {
    pub name: &'static str,
    pub cost: i64,
    pub ladrillos: f32,
    pub piedra: f32,
    pub months: u32,
}

/// Fases de la iglesia matriz. La 0 es la capilla de adobe con la que se funda la ciudad.
pub const CHURCH: [Phase; 6] = [
    Phase { name: "Capilla de adobe", cost: 0, ladrillos: 0.0, piedra: 0.0, months: 0 },
    Phase { name: "Cimientos de piedra", cost: 6000, ladrillos: 0.0, piedra: 300.0, months: 6 },
    Phase { name: "Muros", cost: 10000, ladrillos: 300.0, piedra: 60.0, months: 10 },
    Phase { name: "Naves y techo de teja", cost: 12000, ladrillos: 250.0, piedra: 0.0, months: 10 },
    Phase { name: "Cúpula", cost: 9000, ladrillos: 150.0, piedra: 0.0, months: 8 },
    Phase { name: "Torres y campanario", cost: 8000, ladrillos: 150.0, piedra: 40.0, months: 8 },
];

/// Habitantes que hacen falta para empezar la obra de material (etapa «Ciudad»).
pub const CHURCH_MIN_POP: i32 = 1000;

#[derive(Clone, Debug, Default)]
pub struct Church {
    /// Última fase terminada (0 = capilla).
    pub phase: u32,
    /// ¿Hay una fase en obra?
    pub building: bool,
    pub months_left: u32,
    /// ¿La obra está parada por falta de materiales?
    pub stalled: bool,
    /// Vóxeles de lo que se ve (para reemplazarlo en la fase siguiente).
    pub voxels: Vec<IVec3>,
}

impl Church {
    pub fn finished(&self) -> bool {
        self.phase as usize >= CHURCH.len() - 1
    }
    /// Fase que se ve en vóxeles: la que está en obra (con andamios) o la última terminada.
    pub fn view_phase(&self) -> u32 {
        if self.building { self.phase + 1 } else { self.phase }
    }
    pub fn next(&self) -> Option<&'static Phase> {
        CHURCH.get(self.phase as usize + 1)
    }
    /// Más pobladores con cada fase terminada: la obra da prestigio a la ciudad.
    pub fn attraction(&self) -> f32 {
        1.0 + 0.08 * self.phase as f32
    }
}

/// Vóxeles de la iglesia (o la capilla) en la fase `view`, en su sitio junto a la plaza.
pub fn church_built(t: &TerrainGen, center: Vec2, f: Vec2, view: u32) -> Built {
    if view == 0 {
        build_rotated(t, center, f, 30, 84, 4242, &colonial::capilla)
    } else {
        build_rotated(t, center, f, 30, 84, 4242, &move |t: &TerrainGen, e: &mut Edits, fr: &crate::structures::Frame| colonial::iglesia(t, e, fr, view))
    }
}

// --- Muralla ----------------------------------------------------------------------------------

/// Pesos por metro (se pagan al trazar el tramo) y piedra por metro (se consume en la obra).
pub const WALL_COST_PER_M: i64 = 18;
pub const WALL_STONE_PER_M: f32 = 3.0;
/// Metros de muralla que se levantan por mes si hay piedra.
pub const WALL_M_PER_MONTH: f32 = 30.0;
pub const WALL_UPKEEP_PER_100M: i64 = 6;
/// Espesor de la huella (m): muro de 4 m más el talud.
pub const WALL_THICK: f32 = 6.0;
/// Largo de cada módulo de vóxeles (m).
const MODULE: f32 = 24.0;
/// Derechos que cobran los guardas de cada portón terminado por mes.
pub const GATE_DUTY: i64 = 25;

#[derive(Clone, Debug)]
pub struct Tramo {
    pub a: Vec2,
    pub b: Vec2,
    /// Metros ya levantados.
    pub built: f32,
    pub voxels: Vec<IVec3>,
}

impl Tramo {
    pub fn len(&self) -> f32 {
        self.a.distance(self.b)
    }
    pub fn done(&self) -> bool {
        self.built >= self.len() - 0.01
    }
}

#[derive(Clone, Debug, Default)]
pub struct Walls {
    pub tramos: Vec<Tramo>,
    /// Portones terminados (donde una calle cruza un tramo ya levantado).
    pub gates_done: u32,
    /// ¿Se avisó que la obra está parada?
    pub stalled: bool,
    pub(crate) sig: (usize, i64),
}

impl Walls {
    pub fn total_length(&self) -> f32 {
        self.tramos.iter().map(Tramo::len).sum()
    }
    pub fn upkeep(&self) -> i64 {
        (self.tramos.iter().map(|t| t.built).sum::<f32>() / 100.0) as i64 * WALL_UPKEEP_PER_100M
    }
    pub fn gate_duty(&self) -> i64 {
        self.gates_done as i64 * GATE_DUTY
    }
    /// Huellas de los tramos (para que no se generen solares debajo).
    pub fn obstacles(&self) -> Vec<Obstacle> {
        self.tramos.iter().map(|t| Obstacle { c: (t.a + t.b) * 0.5, f: (t.b - t.a).normalize_or_zero().perp(), w: t.len(), d: WALL_THICK }).collect()
    }
}

impl City {
    // --- Iglesia ---

    /// Centro y frente de la iglesia (si la plaza está fundada).
    pub fn church_site(&self) -> Option<(Vec2, Vec2)> {
        self.plaza.map(|(c, a)| {
            let f = super::super::plaza_frame(a).0;
            (super::super::church_center(c, f), f)
        })
    }

    /// ¿Se puede empezar la fase siguiente de la iglesia? `None` si sí; si no, el motivo.
    pub fn church_problem(&self) -> Option<String> {
        let ch = &self.services.church;
        if self.plaza.is_none() {
            return Some("Primero funda la plaza mayor".into());
        }
        let next = ch.next()?;
        if ch.building {
            return Some(format!("Ya hay una fase en obra (faltan {} meses)", ch.months_left));
        }
        if self.population() < CHURCH_MIN_POP {
            return Some(format!("Hace falta una ciudad de {CHURCH_MIN_POP} habitantes para empezar la obra"));
        }
        if next.cost > self.money {
            return Some(format!("La fase «{}» cuesta {} pesos", next.name, super::super::game::fmt_thousands(next.cost)));
        }
        None
    }

    /// Ficha de la iglesia para el panel de información.
    pub fn church_lines(&self) -> Vec<String> {
        let ch = &self.services.church;
        let mut out = vec![format!("Estado: {}", CHURCH[ch.phase as usize].name)];
        if ch.building {
            let n = ch.next().unwrap();
            out.push(format!("En obra: {} (faltan {} meses){}", n.name, ch.months_left, if ch.stalled { " · PARADA por falta de materiales" } else { "" }));
            out.push(format!("Materiales por mes: {} ladrillos · {} de piedra", (n.ladrillos / n.months as f32).round() as i32, (n.piedra / n.months as f32).round() as i32));
        } else if let Some(n) = ch.next() {
            out.push(format!("Siguiente fase: {}", n.name));
            out.push(format!("Costo: {} pesos · {} ladrillos · {} de piedra · {} meses", super::super::game::fmt_thousands(n.cost), n.ladrillos as i32, n.piedra as i32, n.months));
            if let Some(p) = self.church_problem() {
                out.push(format!("Falta: {p}"));
            }
        }
        out.push(format!("Atrae: {} % más de pobladores", ((ch.attraction() - 1.0) * 100.0).round() as i32));
        out
    }

    /// Empieza la fase siguiente de la iglesia: paga y monta la obra (andamios).
    pub fn start_church_phase(&mut self, t: &TerrainGen) -> Result<(Edits, String), String> {
        if self.services.church.finished() {
            return Err("La iglesia matriz ya está terminada".into());
        }
        if let Some(p) = self.church_problem() {
            return Err(p);
        }
        let next = self.services.church.next().unwrap();
        self.money -= next.cost;
        let ch = &mut self.services.church;
        ch.building = true;
        ch.months_left = next.months;
        ch.stalled = false;
        let mut e = Edits::default();
        self.church_refresh(t, &mut e);
        Ok((e, format!("Empieza la obra de la iglesia matriz: {}", next.name.to_lowercase())))
    }

    /// Rehace los vóxeles de la iglesia según la fase que se ve.
    pub(crate) fn church_refresh(&mut self, t: &TerrainGen, e: &mut Edits) {
        let Some((c, f)) = self.church_site() else { return };
        let b = church_built(t, c, f, self.services.church.view_phase());
        let new: HashSet<IVec3> = b.voxels.iter().copied().collect();
        for &v in &self.services.church.voxels {
            if !new.contains(&v) {
                e.dset(v, AIR);
            }
        }
        e.extend(b.edits);
        self.services.church.voxels = b.voxels;
    }

    // --- Muralla ---

    /// `None` si el tramo de muralla de `a` a `b` se puede trazar; si no, el motivo.
    pub fn wall_problem(&self, t: &TerrainGen, a: Vec2, b: Vec2) -> Option<String> {
        if self.plaza.is_none() {
            return Some("Primero funda la plaza mayor".into());
        }
        let len = a.distance(b);
        if len < 12.0 {
            return Some("Demasiado corta".into());
        }
        if len > 800.0 {
            return Some("Demasiado larga: traza la muralla por tramos de hasta 800 m".into());
        }
        let n = (len / 6.0) as i32 + 1;
        for i in 0..=n {
            let p = a.lerp(b, i as f32 / n as f32);
            if t.height(p.x, p.y) < 0.5 || t.is_water(p.x, p.y) {
                return Some("No se puede levantar sobre el agua".into());
            }
        }
        // Las calles la cruzan (portones), pero no puede ir por encima de una.
        let (mut run, mut worst) = (0.0f32, 0.0f32);
        let k = (len / 2.0) as i32;
        for i in 0..=k {
            let p = a.lerp(b, i as f32 / k as f32);
            run = if self.net.clearance(p, 20.0) < 0.5 { run + len / k as f32 } else { 0.0 };
            worst = worst.max(run);
        }
        if worst > 24.0 {
            return Some("La muralla no puede ir por encima de una calle: crúzala".into());
        }
        let poly = corners_of((a + b) * 0.5, (b - a).normalize_or_zero().perp(), len, WALL_THICK);
        if self.lots.obstacles.iter().any(|o| polys_overlap(&poly, &corners_of(o.c, o.f, o.w - 0.5, o.d - 0.5))) {
            return Some("Atraviesa la plaza, la iglesia o un edificio".into());
        }
        let cost = (len * WALL_COST_PER_M as f32) as i64;
        if cost > self.money {
            return Some(format!("Fondos insuficientes: hacen falta {} pesos", super::super::game::fmt_thousands(cost)));
        }
        None
    }

    /// Traza un tramo de muralla: se paga y queda en obra (los cimientos); se levanta mes a
    /// mes con la piedra que haya.
    pub fn build_wall(&mut self, t: &TerrainGen, a: Vec2, b: Vec2) -> Result<Edits, String> {
        if let Some(p) = self.wall_problem(t, a, b) {
            return Err(p);
        }
        self.money -= (a.distance(b) * WALL_COST_PER_M as f32) as i64;
        let mut e = Edits::default();
        let poly = corners_of((a + b) * 0.5, (b - a).normalize_or_zero().perp(), a.distance(b), WALL_THICK);
        self.clear_lots_under(&poly, &mut e);
        self.services.walls.tramos.push(Tramo { a, b, built: 0.0, voxels: Vec::new() });
        self.lots.walls = self.services.walls.obstacles();
        let i = self.services.walls.tramos.len() - 1;
        self.wall_edits(t, i, &mut e);
        Ok(e)
    }

    /// Portones de un tramo: dónde lo cruza una calle (m desde `a`) y el ancho de la calle.
    pub fn wall_gates(&self, a: Vec2, b: Vec2) -> Vec<(f32, f32)> {
        let mut out: Vec<(f32, f32)> = Vec::new();
        for ed in self.net.edges.values() {
            for s in ed.pts.windows(2) {
                if let Some(x) = segment_intersection(a, b, s[0], s[1]) {
                    let d = a.distance(x);
                    if !out.iter().any(|g| (g.0 - d).abs() < 14.0) {
                        out.push((d, ed.kind.width()));
                    }
                }
            }
        }
        out.sort_by(|x, y| x.0.total_cmp(&y.0));
        out
    }

    /// Vóxeles de un tramo: módulos de hasta 24 m; los levantados, completos; el que está en
    /// obra, a media altura con andamios; el resto, cimientos con la piedra acopiada.
    pub(crate) fn wall_edits(&mut self, t: &TerrainGen, i: usize, e: &mut Edits) {
        let tr = &self.services.walls.tramos[i];
        let (a, b, built) = (tr.a, tr.b, tr.built);
        let len = a.distance(b);
        let d = (b - a) / len.max(0.01);
        // Hacia afuera: lejos de la plaza.
        let plaza = self.lots.plaza_center.unwrap_or(a);
        let mut n = d.perp();
        if ((a + b) * 0.5 - plaza).dot(n) < 0.0 {
            n = -n;
        }
        let gates = self.wall_gates(a, b);
        // Cortes: cada portón ocupa su propio módulo (calle + pilastras).
        let mut mods: Vec<(f32, f32, Option<(f32, f32)>)> = Vec::new();
        let mut s = 0.0;
        let push_plain = |mods: &mut Vec<(f32, f32, Option<(f32, f32)>)>, s0: f32, s1: f32| {
            let k = ((s1 - s0) / MODULE).ceil().max(1.0) as i32;
            for j in 0..k {
                let (x0, x1) = (s0 + (s1 - s0) * j as f32 / k as f32, s0 + (s1 - s0) * (j + 1) as f32 / k as f32);
                if x1 - x0 > 0.6 {
                    mods.push((x0, x1, None));
                }
            }
        };
        for &(g, w) in &gates {
            let (g0, g1) = ((g - w * 0.5 - 2.5).max(s), (g + w * 0.5 + 2.5).min(len));
            if g0 > s {
                push_plain(&mut mods, s, g0);
            }
            mods.push((g0, g1, Some((g - w * 0.5 - 0.5, g + w * 0.5 + 0.5))));
            s = g1;
        }
        if s < len {
            push_plain(&mut mods, s, len);
        }
        let sv = Vec2::new(-n.y, n.x);
        let flip = sv.dot(d) < 0.0;
        let mut voxels = Vec::new();
        let mut out = Edits::default();
        let mut gates_done = 0;
        let n_mods = mods.len();
        for (k, &(s0, s1, gate)) in mods.iter().enumerate() {
            let phase = if built >= s1 - 0.01 { 2 } else if built > s0 { 1 } else { 0 };
            if phase == 2 && gate.is_some() {
                gates_done += 1;
            }
            let width = ((s1 - s0) / 0.5).round() as i32;
            let c = a + d * ((s0 + s1) * 0.5);
            let gate_u = gate.map(|(g0, g1)| {
                let (u0, u1) = (((g0 - s0) / 0.5).round() as i32, ((g1 - s0) / 0.5).round() as i32);
                if flip { (width - u1, width - u0) } else { (u0, u1) }
            });
            let garita = k == 0 || k + 1 == n_mods;
            let seed = 0x3A11 ^ ((i as u64) << 20) ^ k as u64;
            // El portón se apoya a la altura de la calle (el vano queda libre para las carretas).
            let base = gate.map(|(g0, g1)| super::road_design_base(t, a + d * ((g0 + g1) * 0.5)));
            let built = build_rotated_at(t, c, n, width, 12, seed, base, &move |t: &TerrainGen, e: &mut Edits, f: &crate::structures::Frame| colonial::muralla_tramo(t, e, f, phase, gate_u, garita));
            out.extend(built.edits);
            voxels.extend(built.voxels);
        }
        let new: HashSet<IVec3> = voxels.iter().copied().collect();
        for &v in &self.services.walls.tramos[i].voxels {
            if !new.contains(&v) {
                e.dset(v, AIR);
            }
        }
        e.extend(out);
        self.services.walls.tramos[i].voxels = voxels;
        let _ = gates_done;
        self.services.walls.sig = (self.net.edges.len(), self.net.total_length() as i64);
        self.recount_gates();
    }

    /// Cuenta los portones terminados de todos los tramos.
    fn recount_gates(&mut self) {
        let mut n = 0;
        for tr in &self.services.walls.tramos {
            n += self.wall_gates(tr.a, tr.b).iter().filter(|&&(g, w)| tr.built >= (g + w * 0.5 + 2.5).min(tr.len()) - 0.01).count() as u32;
        }
        self.services.walls.gates_done = n;
    }

    pub(crate) fn walls_rebuild(&mut self, t: &TerrainGen, e: &mut Edits) {
        self.lots.walls = self.services.walls.obstacles();
        for i in 0..self.services.walls.tramos.len() {
            self.services.walls.tramos[i].voxels.clear();
            self.wall_edits(t, i, e);
        }
    }

    // --- El mes de las obras ---

    pub(crate) fn works_month(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        // Iglesia: los materiales de la fase se reparten por mes.
        if self.services.church.building {
            let ph = self.services.church.next().unwrap();
            let (lad, pie) = (ph.ladrillos / ph.months as f32, ph.piedra / ph.months as f32);
            let have = self.economy.stock(Res::Ladrillos).amount >= lad && self.economy.stock(Res::Piedra).amount >= pie;
            if have {
                self.economy.take(Res::Ladrillos, lad);
                self.economy.take(Res::Piedra, pie);
                let ch = &mut self.services.church;
                ch.stalled = false;
                ch.months_left = ch.months_left.saturating_sub(1);
                if ch.months_left == 0 {
                    ch.building = false;
                    ch.phase += 1;
                    news.push(if ch.finished() { "¡Repican las campanas! La iglesia matriz está terminada".into() } else { format!("Terminada la fase «{}» de la iglesia matriz", ph.name) });
                    self.church_refresh(t, e);
                }
            } else if !self.services.church.stalled {
                self.services.church.stalled = true;
                news.push(format!("La obra de la iglesia está parada: faltan {}", if self.economy.stock(Res::Ladrillos).amount < lad { "ladrillos (construye un horno)" } else { "piedra (abre una cantera)" }));
            }
        }
        // Muralla: se levanta lo que alcance la piedra, tramo por tramo.
        let mut budget = WALL_M_PER_MONTH;
        let mut changed = Vec::new();
        let mut short = false;
        for i in 0..self.services.walls.tramos.len() {
            let tr = &self.services.walls.tramos[i];
            if tr.done() || budget <= 0.0 {
                continue;
            }
            let want = (tr.len() - tr.built).min(budget);
            let can = (self.economy.stock(Res::Piedra).amount / WALL_STONE_PER_M).min(want);
            if can < want {
                short = true;
            }
            if can <= 0.1 {
                continue;
            }
            self.economy.take(Res::Piedra, can * WALL_STONE_PER_M);
            let tr = &mut self.services.walls.tramos[i];
            let before = tr.built;
            tr.built = (tr.built + can).min(tr.len());
            budget -= can;
            if (before / MODULE) as i32 != (tr.built / MODULE) as i32 || tr.done() {
                changed.push(i);
            }
            if tr.done() {
                news.push(format!("Terminado un tramo de muralla de {} m", tr.len() as i32));
            }
        }
        if short && !self.services.walls.stalled {
            news.push("La obra de la muralla va lenta: falta piedra (abre una cantera)".into());
        }
        self.services.walls.stalled = short;
        // Si cambiaron las calles, los portones se rehacen.
        let sig = (self.net.edges.len(), self.net.total_length() as i64);
        if sig != self.services.walls.sig {
            changed = (0..self.services.walls.tramos.len()).collect();
        }
        for i in changed {
            self.wall_edits(t, i, e);
        }
        news
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::town;
    use super::*;

    #[test]
    fn la_iglesia_avanza_por_fases_con_materiales_y_se_para_sin_ellos() {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        assert_eq!(city.services.church.phase, 0);
        assert!(!city.services.church.voxels.is_empty(), "la capilla está en el mapa");
        assert!(city.church_problem().is_some_and(|m| m.contains("habitantes")), "hace falta una ciudad");
        // Fuerza la población: casas de altos llenas en todos los solares.
        for l in city.lots.lots.values_mut() {
            l.level = 3;
        }
        assert!(city.population() >= CHURCH_MIN_POP, "población {}", city.population());
        let money = city.money;
        let (e, msg) = city.start_church_phase(&t).unwrap();
        assert!(msg.contains("cimientos"));
        assert!(!e.design.is_empty());
        assert_eq!(city.money, money - CHURCH[1].cost);
        // Sin piedra la obra se para.
        city.economy.stocks[Res::Piedra as usize].amount = 0.0;
        let mut e = Edits::default();
        let n = city.works_month(&t, &mut e);
        assert!(n.iter().any(|m| m.contains("parada")), "{n:?}");
        assert_eq!(city.services.church.months_left, CHURCH[1].months);
        // Con piedra, los cimientos se terminan en sus meses.
        city.economy.stocks[Res::Piedra as usize].amount = 5000.0;
        city.economy.stocks[Res::Ladrillos as usize].amount = 5000.0;
        for _ in 0..CHURCH[1].months {
            city.works_month(&t, &mut e);
        }
        assert_eq!(city.services.church.phase, 1);
        assert!(!city.services.church.building);
        assert!(city.economy.stock(Res::Piedra).amount < 5000.0 - CHURCH[1].piedra + 1.0);
        // Todas las fases hasta las torres.
        let mut top = 0;
        for p in 2..=5usize {
            city.money = 1_000_000;
            city.start_church_phase(&t).unwrap();
            for _ in 0..CHURCH[p].months {
                city.works_month(&t, &mut e);
            }
            assert_eq!(city.services.church.phase as usize, p);
            let h = city.services.church.voxels.iter().map(|v| v.y).max().unwrap();
            assert!(h >= top, "fase {p}: la obra crece");
            top = h;
        }
        assert!(city.services.church.finished());
        assert!(city.start_church_phase(&t).is_err());
        assert!(city.services.attraction() > 1.3);
    }

    #[test]
    fn la_muralla_se_levanta_con_piedra_y_abre_portones_en_las_calles() {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        let c = city.plaza.unwrap().0;
        // Un tramo al norte del damero que cruza las calles norte-sur.
        let y = c.y + super::super::super::BLOCK;
        let (a, b) = (Vec2::new(c.x - 170.0, y), Vec2::new(c.x + 170.0, y));
        assert!(city.wall_problem(&t, a, b).is_none(), "{:?}", city.wall_problem(&t, a, b));
        let gates = city.wall_gates(a, b);
        assert!(gates.len() >= 3, "portones donde cruzan las calles: {gates:?}");
        city.economy.stocks[Res::Piedra as usize].amount = 0.0;
        let e = city.build_wall(&t, a, b).unwrap();
        assert!(!e.design.is_empty(), "los cimientos se ven");
        let mut e = Edits::default();
        let n = city.works_month(&t, &mut e);
        assert!(n.iter().any(|m| m.contains("piedra")));
        assert_eq!(city.services.walls.tramos[0].built, 0.0);
        city.economy.stocks[Res::Piedra as usize].amount = 10_000.0;
        let months = (340.0 / WALL_M_PER_MONTH).ceil() as usize;
        for _ in 0..months {
            city.works_month(&t, &mut e);
        }
        assert!(city.services.walls.tramos[0].done());
        assert!(city.services.walls.gates_done >= 3);
        assert!(city.services.walls.gate_duty() > 0);
        // Ningún vóxel de muro sobre la calzada de una calle que cruza (el portón queda abierto).
        let tr = &city.services.walls.tramos[0];
        let mut over = 0;
        for v in &tr.voxels {
            let p = Vec2::new((v.x as f32 + 0.5) * 0.5, (v.z as f32 + 0.5) * 0.5);
            if city.net.clearance(p, 20.0) < -2.0 {
                let up = v.y - super::super::road_design_base(&t, p);
                assert!(!(0..8).contains(&up), "muro a {up} vóxeles sobre la calle en {p}");
                over += 1;
            }
        }
        assert!(over > 0, "el dintel del portón pasa sobre la calle");
        // No se generan solares debajo de la muralla.
        let poly = corners_of((a + b) * 0.5, (b - a).normalize().perp(), a.distance(b), WALL_THICK);
        assert!(city.lots.lots.values().all(|l| !polys_overlap(&poly, &city.lots.corners(l, 0.6))));
    }
}
