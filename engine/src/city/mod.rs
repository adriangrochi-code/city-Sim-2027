//! El juego en el motor: la ciudad colonial (calles, solares, zonas y crecimiento) llevada a
//! vóxeles. Porte de la lógica del prototipo de Godot (`scripts/roads`, `scripts/city/lots.gd`
//! y la parte de herramientas de `scripts/main.gd`).
//!
//! Cada acción devuelve las ediciones de vóxeles que produce; la sesión las aplica al mundo
//! en streaming, que re-malla solo las columnas tocadas.

pub mod economy;
pub mod game;
pub mod game_demo;
pub mod lots;
pub mod roads;
pub mod voxelize;

use crate::buildings::{Edits, Rng};
use crate::palette::AIR;
use crate::structures::colonial::{self, LOT_D, LOT_W};
use crate::terrain::TerrainGen;
use glam::Vec2;
use lots::{corners_of, LevelChange, Lot, Lots, Obstacle};
use roads::{polyline_length, RoadKind, RoadNetwork};
use voxelize::{build_rotated, plaza_edits, road_edits, road_restore, PLAZA_SIZE};

/// Cuadra del damero entre ejes de calle: 100 varas castellanas (0,866 m), como en la traza
/// de Montevideo.
pub const BLOCK: f32 = 86.6;
/// Segundos de juego por mes (a velocidad 1).
pub const MONTH_SECONDS: f32 = 10.0;
pub const START_YEAR: u32 = 1726;
pub const MONTHS: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

pub struct City {
    pub net: RoadNetwork,
    pub lots: Lots,
    pub money: i64,
    pub pool: i32,
    pub month: u32,
    pub plaza: Option<(Vec2, f32)>,
    /// Recursos, impuestos, presupuesto, opinión y ordenanzas.
    pub economy: economy::Economy,
    rng: Rng,
    month_acc: f32,
    growth_acc: f32,
}

impl Default for City {
    fn default() -> Self {
        City { net: RoadNetwork::default(), lots: Lots::default(), money: 20000, pool: 0, month: 2, plaza: None, economy: economy::Economy::default(), rng: Rng::new(1726), month_acc: 0.0, growth_acc: 0.0 }
    }
}

/// Dirección del frente de la plaza para un ángulo (como en el prototipo).
pub fn plaza_frame(angle: f32) -> (Vec2, Vec2) {
    let f = Vec2::new(angle.sin(), angle.cos());
    (f, lots::side_of(f))
}

/// Constructor del catálogo de vóxeles para cada nivel de casa.
fn builder_for(level: u8) -> &'static dyn Fn(&TerrainGen, &mut Edits, &crate::structures::Frame) {
    match level {
        1 => &colonial::rancho,
        2 => &colonial::casa_azotea,
        _ => &colonial::casa_altos,
    }
}

fn iglesia5(t: &TerrainGen, e: &mut Edits, f: &crate::structures::Frame) {
    colonial::iglesia(t, e, f, 5)
}

fn clear_voxels(e: &mut Edits, lot: &Lot) {
    for &v in &lot.voxels {
        e.set(v, AIR);
    }
}

impl City {
    pub fn population(&self) -> i32 {
        self.lots.population()
    }

    pub fn date(&self) -> String {
        format!("{} de {}", MONTHS[(self.month % 12) as usize], START_YEAR + self.month / 12)
    }

    // --- Validación ---------------------------------------------------------------

    /// `None` si la calle se puede construir; si no, el motivo.
    pub fn road_problem(&self, t: &TerrainGen, pts: &[Vec2], kind: RoadKind) -> Option<String> {
        if pts.len() < 2 || polyline_length(pts) < 6.0 {
            return Some("Demasiado corta".into());
        }
        let r = roads::min_radius(pts);
        if r < kind.min_radius() {
            return Some(format!("Curva demasiado cerrada: radio {} m (mínimo {} m)", r as i32, kind.min_radius() as i32));
        }
        for (i, p) in pts.iter().enumerate() {
            let h = t.height(p.x, p.y);
            if h < 0.5 {
                return Some("No se puede construir sobre el agua (aún no hay puentes)".into());
            }
            if i > 0 {
                let q = pts[i - 1];
                let dist = q.distance(*p);
                if dist > 0.5 && (h - t.height(q.x, q.y)).abs() / dist > 0.2 {
                    return Some("Demasiado empinado".into());
                }
            }
        }
        if self.lots.obstacles.iter().any(|o| {
            let cs = corners_of(o.c, o.f, o.w - 1.0, o.d - 1.0);
            pts.iter().any(|p| lots::point_in_poly(*p, &cs))
        }) {
            return Some("Atraviesa la plaza o la iglesia".into());
        }
        None
    }

    pub fn road_cost(pts: &[Vec2], kind: RoadKind) -> i64 {
        (polyline_length(pts) * kind.cost_per_m()) as i64
    }

    pub fn plaza_problem(&self, t: &TerrainGen, c: Vec2, angle: f32) -> Option<String> {
        if self.plaza.is_some() {
            return Some("La plaza mayor ya está fundada".into());
        }
        let (f, _) = plaza_frame(angle);
        let (mut hmin, mut hmax) = (f32::INFINITY, f32::NEG_INFINITY);
        for p in corners_of(c, f, BLOCK + 10.0, BLOCK + 10.0).iter().chain(std::iter::once(&c)) {
            let h = t.height(p.x, p.y);
            if h < 1.0 {
                return Some("Demasiado cerca del agua".into());
            }
            hmin = hmin.min(h);
            hmax = hmax.max(h);
        }
        (hmax - hmin > 7.0).then(|| "El terreno es demasiado irregular".into())
    }

    // --- Acciones (cada una devuelve las ediciones de vóxeles) ---------------------------

    /// Funda la plaza mayor con la iglesia matriz y las calles empedradas que la rodean.
    pub fn found_plaza(&mut self, t: &TerrainGen, c: Vec2, angle: f32) -> Result<Edits, String> {
        if let Some(p) = self.plaza_problem(t, c, angle) {
            return Err(p);
        }
        let (f, s) = plaza_frame(angle);
        self.plaza = Some((c, angle));
        self.lots.plaza_center = Some(c);
        self.lots.obstacles.push(Obstacle { c, f, w: PLAZA_SIZE, d: PLAZA_SIZE });
        let church_c = c - f * (BLOCK * 0.5 + RoadKind::Stone.width() * 0.5 + 0.6 + 21.0);
        self.lots.obstacles.push(Obstacle { c: church_c, f, w: 18.0, d: 44.0 });
        let mut e = Edits::default();
        plaza_edits(t, c, f, &mut e);
        let church = build_rotated(t, church_c, f, 30, 84, 4242, &iglesia5);
        e.voxels.extend(church.edits.voxels);
        e.surface.extend(church.edits.surface);
        let half = BLOCK * 0.5;
        let q = [c + (s + f) * half, c + (-s + f) * half, c + (-s - f) * half, c + (s - f) * half];
        let mut new = Vec::new();
        for i in 0..4 {
            new.extend(self.net.add_road(&roads::line(q[i], q[(i + 1) % 4]), RoadKind::Stone));
        }
        self.after_roads(t, &new, &mut e);
        self.pool = 40;
        Ok(e)
    }

    /// Construye una calle (recta o curva) ya muestreada.
    pub fn build_road(&mut self, t: &TerrainGen, pts: &[Vec2], kind: RoadKind) -> Result<Edits, String> {
        if let Some(p) = self.road_problem(t, pts, kind) {
            return Err(p);
        }
        let cost = Self::road_cost(pts, kind);
        if cost > self.money {
            return Err(format!("Fondos insuficientes: hacen falta {cost} pesos"));
        }
        self.money -= cost;
        let new = self.net.add_road(pts, kind);
        let mut e = Edits::default();
        self.after_roads(t, &new, &mut e);
        Ok(e)
    }

    /// Damero: varias calles a la vez (se validan todas antes de construir).
    pub fn build_grid(&mut self, t: &TerrainGen, lines: &[Vec<Vec2>], kind: RoadKind) -> Result<Edits, String> {
        if lines.is_empty() {
            return Err("Damero vacío".into());
        }
        let mut cost = 0;
        for l in lines {
            if let Some(p) = self.road_problem(t, l, kind) {
                return Err(format!("Damero: {p}"));
            }
            cost += Self::road_cost(l, kind);
        }
        if cost > self.money {
            return Err(format!("Fondos insuficientes: hacen falta {cost} pesos"));
        }
        self.money -= cost;
        let mut new = Vec::new();
        for l in lines {
            new.extend(self.net.add_road(l, kind));
        }
        new.retain(|id| self.net.edges.contains_key(id));
        new.sort();
        new.dedup();
        let mut e = Edits::default();
        self.after_roads(t, &new, &mut e);
        Ok(e)
    }

    /// Después de agregar calles: borra los edificios de los solares que quedaron debajo,
    /// pinta las calzadas nuevas y genera solares nuevos.
    fn after_roads(&mut self, t: &TerrainGen, new: &[u32], e: &mut Edits) {
        let removed = self.lots.on_roads_changed(&self.net, t, new);
        for lot in &removed {
            clear_voxels(e, lot);
        }
        for &eid in new {
            if self.net.edges.contains_key(&eid) {
                road_edits(t, &self.net, eid, e);
            }
        }
    }

    /// Demuele el tramo de calle más cercano a `p` (a menos de 10 m).
    pub fn demolish_road(&mut self, t: &TerrainGen, p: Vec2) -> Result<Edits, String> {
        let near = self.net.nearest_edge(p, 10.0).ok_or("No hay ninguna calle ahí")?;
        let edge = self.net.remove_edge(near.edge).unwrap();
        let mut e = Edits::default();
        let removed = self.lots.on_roads_changed(&self.net, t, &[]);
        for lot in &removed {
            clear_voxels(&mut e, lot);
        }
        road_restore(t, &edge, &mut e);
        // Las calles vecinas se vuelven a pintar encima (cruces, banquinas compartidas).
        let neighbors: Vec<u32> = self.net.edges.iter().filter(|(_, x)| [x.a, x.b].iter().any(|n| self.net.nodes[n].distance(edge.pts[0]) < 1.0 || self.net.nodes[n].distance(*edge.pts.last().unwrap()) < 1.0)).map(|(&id, _)| id).collect();
        for id in neighbors {
            road_edits(t, &self.net, id, &mut e);
        }
        self.money -= 20;
        Ok(e)
    }

    /// Pinta o quita la zona mixta con un pincel. Devuelve las ediciones de las demoliciones.
    pub fn paint(&mut self, center: Vec2, radius: f32, zone: u8) -> (usize, Edits) {
        let (n, demolish) = self.lots.paint(center, radius, zone);
        let mut e = Edits::default();
        for id in demolish {
            let lot = self.lots.lots.get_mut(&id).unwrap();
            clear_voxels(&mut e, lot);
            lot.voxels.clear();
        }
        (n, e)
    }

    /// Lleva a vóxeles los cambios de nivel: el edificio nuevo reemplaza al viejo en el solar.
    pub fn apply_levels(&mut self, t: &TerrainGen, changes: &[LevelChange], e: &mut Edits) {
        for ch in changes {
            let Some(lot) = self.lots.lots.get(&ch.lot) else { continue };
            let b = build_rotated(t, lot.c, lot.f, LOT_W, LOT_D, lot.seed, builder_for(ch.level));
            let new: std::collections::HashSet<glam::IVec3> = b.voxels.iter().copied().collect();
            for &v in &lot.voxels {
                if !new.contains(&v) {
                    e.set(v, AIR);
                }
            }
            e.voxels.extend(b.edits.voxels);
            e.surface.extend(b.edits.surface);
            self.lots.lots.get_mut(&ch.lot).unwrap().voxels = b.voxels;
        }
    }

    /// Avanza el tiempo de juego `dt` segundos: meses (pobladores, impuestos) y crecimiento.
    pub fn tick(&mut self, t: &TerrainGen, dt: f32, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        if self.plaza.is_none() {
            return news;
        }
        self.month_acc += dt;
        while self.month_acc >= MONTH_SECONDS {
            self.month_acc -= MONTH_SECONDS;
            news.extend(self.month_tick());
        }
        self.growth_acc += dt;
        while self.growth_acc >= 0.5 {
            self.growth_acc -= 0.5;
            let (used, changes) = self.lots.grow_step(self.pool, &self.net, &mut self.rng);
            self.pool -= used;
            self.apply_levels(t, &changes, e);
        }
        news
    }

    /// Lo que la ciudad le pasa a la economía este mes. Mientras no existan en el motor las
    /// estancias, hornos y canteras del prototipo, el entorno rural produce según la población.
    pub fn month_input(&self) -> economy::MonthInput {
        use economy::Res;
        let pop = self.population();
        let mut houses = [0; 3];
        for l in self.lots.lots.values() {
            if (1..=3).contains(&l.level) {
                houses[l.level as usize - 1] += 1;
            }
        }
        let mut production = [0.0f32; 6];
        production[Res::Pan as usize] = 300.0 + pop as f32 * 1.05; // chacras privadas del ejido
        production[Res::Carne as usize] = 400.0 + pop as f32 * 0.9;
        production[Res::Ladrillos as usize] = 60.0;
        production[Res::Cueros as usize] = 80.0 + pop as f32 * 0.05;
        production[Res::Sebo as usize] = 40.0;
        let mut construction = [0.0f32; 6];
        construction[Res::Ladrillos as usize] = houses[1] as f32 * 0.4 + houses[2] as f32 * 0.8;
        economy::MonthInput {
            pop,
            houses,
            production,
            construction,
            ships: if self.month % 4 == 0 { 1 } else { 0 },
            water_ratio: 0.6,
            upkeep_roads: (self.net.total_length() * 0.01) as i64,
            world_hides: 1.0,
            ..Default::default()
        }
    }

    fn month_tick(&mut self) -> Vec<String> {
        let mut news = Vec::new();
        self.month += 1;
        let pop = self.population();
        let factor = self.economy.arrivals_factor();
        let mut arrivals = ((4.0 + pop as f32 * 0.03) * factor) as i32;
        if self.month % 4 == 0 {
            let ship = ((20 + self.rng.range(0, 25)) as f32 * self.economy.ship_factor()) as i32 + (pop as f32 * 0.02) as i32;
            arrivals += ship;
            news.push(format!("Llega un barco con {ship} pobladores"));
        }
        self.pool = (self.pool + arrivals).min(600);
        let input = self.month_input();
        let report = self.economy.month(&input);
        self.money += report.balance();
        news.extend(report.news);
        self.lots.age_lots();
        news
    }

    /// Avance rápido de `months` meses (para pruebas): devuelve las ediciones juntas.
    pub fn fast_forward(&mut self, t: &TerrainGen, months: u32) -> Edits {
        let mut e = Edits::default();
        for _ in 0..months {
            self.tick(t, MONTH_SECONDS, &mut e);
        }
        e
    }
}

/// Líneas de un damero: lado `a`→`b` (cuadras de 84 m) extendido hacia `cursor`.
pub fn grid_lines(a: Vec2, b: Vec2, cursor: Vec2) -> Vec<Vec<Vec2>> {
    let u = b - a;
    let lu = u.length();
    if lu < 40.0 {
        return Vec::new();
    }
    let u = u / lu;
    let mut v = Vec2::new(-u.y, u.x);
    let mut ext = (cursor - a).dot(v);
    if ext < 0.0 {
        v = -v;
        ext = -ext;
    }
    let nu = ((lu / BLOCK).round() as i32).max(1);
    let nv = ((ext / BLOCK).round() as i32).max(1);
    let mut lines = Vec::new();
    for j in 0..=nv {
        let s = a + v * BLOCK * j as f32;
        lines.push(roads::line(s, s + u * BLOCK * nu as f32));
    }
    for i in 0..=nu {
        let s = a + u * BLOCK * i as f32;
        lines.push(roads::line(s, s + v * BLOCK * nv as f32));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fundar_trazar_zonificar_y_crecer() {
        let t = TerrainGen::new(1726);
        let mut city = City::default();
        city.money = 1_000_000;
        let c = Vec2::new(13500.0, 16300.0);
        let e = city.found_plaza(&t, c, 0.0).unwrap();
        assert!(e.voxels.len() > 10_000, "plaza, iglesia y calles: {} vóxeles", e.voxels.len());
        let (f, s) = plaza_frame(0.0);
        let a = c - s * (BLOCK * 2.5) - f * (BLOCK * 2.5);
        city.build_grid(&t, &grid_lines(a, a + s * BLOCK * 5.0, a + f * BLOCK * 5.0), RoadKind::Stone).unwrap();
        assert!(city.lots.lots.len() > 100, "{} solares", city.lots.lots.len());
        let (n, _) = city.paint(c, 300.0, 1);
        assert!(n > 100);
        let e = city.fast_forward(&t, 24);
        assert!(city.population() > 100, "población {}", city.population());
        assert!(!e.voxels.is_empty());
        // Ningún edificio pisa una calzada.
        for lot in city.lots.lots.values().filter(|l| l.level > 0) {
            for v in lot.voxels.iter().step_by(37) {
                let p = Vec2::new((v.x as f32 + 0.5) * 0.5, (v.z as f32 + 0.5) * 0.5);
                assert!(city.net.clearance(p, 20.0) > -0.3, "un edificio pisa la calle en {p}");
            }
        }
    }
}
