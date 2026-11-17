//! Herramientas del modo juego (como en el prototipo): plaza mayor, calle recta, curva,
//! damero, zona, quitar zona, demoler e inspeccionar; vistas previas y línea de estado.

use super::lots::{corners_of, growth_blockers, zone_code, zone_name, zone_of, Density, ZoneKind, D, W};
use super::roads::{bezier, line, RoadKind};
use super::{grid_lines, plaza_frame, City, BLOCK};
use crate::buildings::Edits;
use crate::render::OverlayVertex;
use crate::terrain::TerrainGen;
use glam::Vec2;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Plaza,
    Straight,
    Curve,
    Grid,
    Zone,
    Dezone,
    Demolish,
    Inspect,
}

pub const TOOLS: [(Tool, &str); 8] = [
    (Tool::Plaza, "plaza mayor"),
    (Tool::Straight, "calle recta"),
    (Tool::Curve, "calle curva"),
    (Tool::Grid, "damero"),
    (Tool::Zone, "zonificar"),
    (Tool::Dezone, "quitar zona"),
    (Tool::Demolish, "demoler calle"),
    (Tool::Inspect, "inspeccionar"),
];

pub struct Game {
    /// Nombre de la ciudad (el que eligió el jugador al empezar la partida).
    pub city_name: String,
    pub city: City,
    pub tool: Tool,
    pub kind: RoadKind,
    pub stage: u8,
    pub p0: Vec2,
    pub p1: Vec2,
    pub brush: f32,
    pub plaza_angle: f32,
    pub painting: bool,
    pub hover: Option<Vec2>,
    /// Velocidad de la simulación (0 = pausa).
    pub speed: f32,
    pub message: String,
    message_at: Instant,
    /// Sube con cada aviso nuevo (la interfaz lo usa para mostrar la gaceta).
    pub message_seq: u64,
    /// Punto inspeccionado con la herramienta Inspeccionar (la interfaz muestra su ficha).
    pub inspected: Option<Vec2>,
    /// Zona que pinta el pincel: tipo y densidad.
    pub zone_kind: ZoneKind,
    pub density: Density,
}

/// Ficha de lo inspeccionado para el panel de información.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inspect {
    pub title: String,
    pub lines: Vec<String>,
    /// "¿Por qué no crece?" (vacío si no aplica).
    pub why: Vec<String>,
    pub kind: InspectKind,
}

/// Qué se está inspeccionando (decide los botones de acción de la ficha).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InspectKind {
    Lot,
    Road,
    #[default]
    Terrain,
}

/// Nombre de la etapa de la ciudad según su población (como en el prototipo).
pub fn stage_name(pop: i32) -> &'static str {
    match pop {
        p if p >= 4000 => "Plaza fuerte",
        p if p >= 1000 => "Ciudad",
        p if p >= 200 => "Villa",
        _ => "Fundación",
    }
}

impl Default for Game {
    fn default() -> Self {
        Game {
            city_name: crate::ui::CITY_NAME.to_string(),
            city: City::default(),
            tool: Tool::Plaza,
            kind: RoadKind::Dirt,
            stage: 0,
            p0: Vec2::ZERO,
            p1: Vec2::ZERO,
            brush: 30.0,
            zone_kind: ZoneKind::Residencial,
            density: Density::Baja,
            plaza_angle: 0.0,
            painting: false,
            hover: None,
            speed: 1.0,
            message: "Funda la ciudad: elige el sitio de la plaza mayor (clic)".into(),
            message_at: Instant::now(),
            message_seq: 1,
            inspected: None,
        }
    }
}

impl Game {
    pub fn say(&mut self, m: impl Into<String>) {
        self.message = m.into();
        self.message_at = Instant::now();
        self.message_seq += 1;
        println!("{}", self.message);
    }

    pub fn set_tool(&mut self, i: usize) {
        let tool = TOOLS[i.min(TOOLS.len() - 1)].0;
        if self.city.plaza.is_none() && !matches!(tool, Tool::Plaza | Tool::Inspect) {
            self.say("Primero funda la plaza mayor");
            self.tool = Tool::Plaza;
        } else {
            self.tool = tool;
        }
        self.stage = 0;
        self.painting = false;
    }

    pub fn tool_name(&self) -> &'static str {
        TOOLS.iter().find(|t| t.0 == self.tool).map_or("", |t| t.1)
    }

    /// Esc / clic derecho: cancela el trazado en curso. Devuelve si había algo que cancelar.
    pub fn cancel(&mut self) -> bool {
        let had = self.stage > 0 || self.painting;
        self.stage = 0;
        self.painting = false;
        had
    }

    pub fn snap(&self, p: Vec2) -> Vec2 {
        self.city.net.snap(p).pos
    }

    fn curve_points(&self, cursor: Vec2) -> Vec<Vec2> {
        let ctrl = cursor * 2.0 - (self.p0 + self.p1) * 0.5;
        bezier(self.p0, ctrl, self.snap(self.p1))
    }

    fn report<T>(&mut self, r: Result<T, String>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.say(e);
                None
            }
        }
    }

    /// Clic izquierdo en `p`. Devuelve ediciones si se construyó algo.
    pub fn press(&mut self, t: &TerrainGen, p: Vec2) -> Option<Edits> {
        match self.tool {
            Tool::Plaza => {
                let r = self.city.found_plaza(t, p, self.plaza_angle);
                let e = self.report(r)?;
                self.say("¡Fundada la ciudad! Traza calles (2, 3, 4) y zonifica solares (5)");
                self.tool = Tool::Straight;
                Some(e)
            }
            Tool::Straight => {
                if self.stage == 0 {
                    self.p0 = self.snap(p);
                    self.stage = 1;
                    return None;
                }
                let end = self.snap(p);
                let r = self.city.build_road(t, &line(self.p0, end), self.kind);
                let e = self.report(r)?;
                self.p0 = end;
                Some(e)
            }
            Tool::Curve => match self.stage {
                0 => {
                    self.p0 = self.snap(p);
                    self.stage = 1;
                    None
                }
                1 => {
                    self.p1 = self.snap(p);
                    if self.p1.distance(self.p0) > 5.0 {
                        self.stage = 2;
                    }
                    None
                }
                _ => {
                    let pts = self.curve_points(p);
                    let r = self.city.build_road(t, &pts, self.kind);
                    let e = self.report(r)?;
                    self.p0 = self.p1;
                    self.stage = 1;
                    Some(e)
                }
            },
            Tool::Grid => match self.stage {
                0 => {
                    self.p0 = self.snap(p);
                    self.stage = 1;
                    None
                }
                1 => {
                    self.p1 = p;
                    if self.p1.distance(self.p0) > 40.0 {
                        self.stage = 2;
                    }
                    None
                }
                _ => {
                    let r = self.city.build_grid(t, &grid_lines(self.p0, self.p1, p), self.kind);
                    let e = self.report(r)?;
                    self.stage = 0;
                    Some(e)
                }
            },
            Tool::Zone | Tool::Dezone => {
                self.painting = true;
                self.paint_at(p)
            }
            Tool::Demolish => {
                let r = self.city.demolish_road(t, p);
                self.report(r)
            }
            Tool::Inspect => {
                self.inspected = Some(p);
                None
            }
        }
    }

    pub fn release(&mut self) {
        self.painting = false;
    }

    /// Pincel de zonas mientras se mantiene apretado el botón.
    pub fn paint_at(&mut self, p: Vec2) -> Option<Edits> {
        if !self.painting {
            return None;
        }
        let zone = if self.tool == Tool::Zone { zone_code(self.zone_kind, self.density) } else { 0 };
        let (n, e) = self.city.paint(p, self.brush, zone);
        (n > 0 && !e.is_empty()).then_some(e)
    }

    /// Ficha del punto inspeccionado: solar (nivel, habitantes, valor del suelo y por qué no
    /// crece), calle (tipo y largo) o terreno.
    pub fn inspect_info(&self) -> Option<Inspect> {
        let p = self.inspected?;
        if let Some(id) = self.city.lots.lot_at(p) {
            let l = &self.city.lots.lots[&id];
            let front = self.city.lots.frontage(l, &self.city.net);
            let value = self.city.lots.land_value(l);
            let mut i = Inspect { title: capitalize(l.name()), kind: InspectKind::Lot, ..Default::default() };
            i.lines.push(format!("Zona: {}", zone_name(l.zone)));
            match l.kind() {
                Some(ZoneKind::Residencial) | None => i.lines.push(format!("Habitantes: {}", l.residents())),
                Some(ZoneKind::Comercial) => i.lines.push(format!("Empleos: {} (comercio)", super::lots::JOBS_C[l.level as usize])),
                Some(ZoneKind::Industrial) => i.lines.push(format!("Empleos: {} (talleres)", super::lots::JOBS_I[l.level as usize])),
            }
            i.lines.push(format!("Valor del suelo: {} %", (value * 100.0) as i32));
            i.lines.push(format!("Calle: {}", front.map_or("—", |k| k.name())));
            if l.level > 0 {
                i.lines.push(format!("Antigüedad: {} {}", l.age, if l.age == 1 { "mes" } else { "meses" }));
            }
            if let Some((k, _)) = zone_of(l.zone) {
                let d = self.city.demand().of(k);
                i.lines.push(format!("Demanda {}: {}{} %", k.name(), if d >= 0.0 { "+" } else { "" }, (d * 100.0).round() as i32));
            }
            i.why = growth_blockers(&self.city.lots, l, &self.city.net, self.city.pool, self.city.demand());
            if i.why.is_empty() {
                i.why.push("Nada le falta: crecerá con el tiempo.".into());
            }
            return Some(i);
        }
        if let Some(e) = self.city.net.nearest_edge(p, 8.0) {
            let ed = &self.city.net.edges[&e.edge];
            return Some(Inspect { title: ed.kind.name().into(), lines: vec![format!("Largo del tramo: {} m", ed.len() as i32)], why: vec![], kind: InspectKind::Road });
        }
        Some(Inspect { title: "Terreno".into(), lines: vec![format!("Posición: {:.0}, {:.0}", p.x, p.y)], why: vec![], kind: InspectKind::Terrain })
    }

    /// Pista de la herramienta para la interfaz: (ayuda con el costo en vivo, motivo de error).
    pub fn hint(&self, t: &TerrainGen) -> (String, Option<String>) {
        let help = match self.tool {
            Tool::Plaza => "Elige el sitio de la plaza mayor (terreno firme y llano). R la gira.",
            Tool::Straight => "Clic en el inicio y clic en el final. Sigue encadenando tramos; Esc para terminar.",
            Tool::Curve => "Clic en el inicio, clic en el final y mueve el ratón para doblarla. Un tercer clic la fija.",
            Tool::Grid => "Clic en una esquina, clic en la punta del primer lado y extiende las manzanas (cuadras de 84 m).",
            Tool::Zone => "Pinta los solares junto a las calles: residencial (casas), comercial (tiendas) o industrial (talleres), en densidad baja, media o alta.",
            Tool::Dezone => "Pinta para quitar la zona (derriba lo construido).",
            Tool::Demolish => "Clic sobre un tramo de calle para demolerlo.",
            Tool::Inspect => "Clic en un solar, una calle o el terreno para ver su información.",
        };
        let mut help = help.to_string();
        let Some(h) = self.hover else { return (help, None) };
        let road = |pts: &[Vec2]| -> (i64, Option<String>) {
            let cost = City::road_cost(t, pts, self.kind);
            let err = self.city.road_problem(t, pts, self.kind).or_else(|| (cost > self.city.money).then(|| format!("Fondos insuficientes: hacen falta {} pesos", fmt_thousands(cost))));
            (cost, err)
        };
        let (cost, err) = match self.tool {
            Tool::Plaza => (None, self.city.plaza_problem(t, h, self.plaza_angle)),
            Tool::Straight | Tool::Curve if self.stage == 1 => {
                let (c, e) = road(&line(self.p0, self.snap(h)));
                (Some(c), e)
            }
            Tool::Curve if self.stage == 2 => {
                let (c, e) = road(&self.curve_points(h));
                (Some(c), e)
            }
            Tool::Grid if self.stage == 2 => {
                let lines = grid_lines(self.p0, self.p1, h);
                let cost: i64 = lines.iter().map(|l| City::road_cost(t, l, self.kind)).sum();
                let err = lines.iter().find_map(|l| self.city.road_problem(t, l, self.kind)).or_else(|| (cost > self.city.money).then(|| format!("Fondos insuficientes: hacen falta {} pesos", fmt_thousands(cost))));
                (Some(cost), err)
            }
            Tool::Zone | Tool::Dezone => {
                help = format!("{help} Pincel: {} m ([ y ] lo cambian).", self.brush as i32);
                (None, None)
            }
            _ => (None, None),
        };
        if let Some(c) = cost {
            help = format!("{help}\n{} · costo: {} pesos", self.kind.name(), fmt_thousands(c));
        }
        (help, err)
    }

    /// Avanza la simulación. Devuelve ediciones si algo creció.
    pub fn update(&mut self, t: &TerrainGen, dt: f32) -> Option<Edits> {
        let mut e = Edits::default();
        let news = self.city.tick(t, dt * self.speed, &mut e);
        for n in news {
            self.say(n);
        }
        (!e.is_empty()).then_some(e)
    }

    pub fn status(&self) -> String {
        let msg = if self.message_at.elapsed().as_secs() < 6 { format!(" · {}", self.message) } else { String::new() };
        format!(
            "{} · {} pesos · población {} · esperan casa {} · {} solares · herramienta: {} ({}) · pincel {} m{}{}",
            self.city.date(),
            self.city.money,
            self.city.population(),
            self.city.pool,
            self.city.lots.lots.len(),
            self.tool_name(),
            self.kind.name(),
            self.brush as i32,
            if self.speed == 0.0 { " · PAUSA" } else { "" },
            msg
        )
    }

    // --- Vistas previas -----------------------------------------------------------------

    pub fn overlay(&self, t: &TerrainGen) -> Vec<OverlayVertex> {
        let mut out = Vec::new();
        let ok = [0.45, 0.95, 0.45, 0.55];
        let bad = [1.0, 0.35, 0.3, 0.55];
        // Solares: el plano de zonas (los zonificados con su color y los libres) solo se ve con
        // la herramienta de zonas; fuera de ella, el mapa queda limpio.
        let show_free = matches!(self.tool, Tool::Zone | Tool::Dezone);
        for l in self.city.lots.lots.values() {
            if !show_free || l.level > 0 {
                continue;
            }
            let col = match zone_of(l.zone) {
                Some((k, d)) => zone_color(k, d),
                None if show_free => [1.0, 0.97, 0.85, 0.18],
                None => continue,
            };
            quad(&mut out, t, &corners_of(l.c, l.f, W - 1.2, D - 1.2), 0.9, col);
        }
        // Lo inspeccionado queda marcado mientras la ficha está abierta.
        if let Some(p) = self.inspected {
            if let Some(id) = self.city.lots.lot_at(p) {
                let l = &self.city.lots.lots[&id];
                outline(&mut out, t, &corners_of(l.c, l.f, W, D), 1.6, [1.0, 0.85, 0.3, 0.95]);
            } else if let Some(e) = self.city.net.nearest_edge(p, 8.0) {
                let ed = &self.city.net.edges[&e.edge];
                ribbon(&mut out, t, &ed.pts, ed.kind.width() + 1.0, [1.0, 0.85, 0.3, 0.7]);
            }
        }
        let Some(h) = self.hover else { return out };
        match self.tool {
            Tool::Plaza if self.city.plaza.is_none() => {
                let (f, _) = plaza_frame(self.plaza_angle);
                let good = self.city.plaza_problem(t, h, self.plaza_angle).is_none();
                outline(&mut out, t, &corners_of(h, f, BLOCK + 8.0, BLOCK + 8.0), 4.0, if good { ok } else { bad });
            }
            Tool::Straight if self.stage == 1 => self.road_preview(&mut out, t, &line(self.p0, self.snap(h))),
            Tool::Curve if self.stage == 1 => self.road_preview(&mut out, t, &line(self.p0, self.snap(h))),
            Tool::Curve if self.stage == 2 => self.road_preview(&mut out, t, &self.curve_points(h)),
            Tool::Grid if self.stage == 1 => ribbon(&mut out, t, &line(self.p0, h), 3.0, [1.0, 1.0, 1.0, 0.6]),
            Tool::Grid if self.stage == 2 => {
                let lines = grid_lines(self.p0, self.p1, h);
                let good = lines.iter().all(|l| self.city.road_problem(t, l, self.kind).is_none());
                for l in &lines {
                    ribbon(&mut out, t, l, self.kind.width(), if good { ok } else { bad });
                }
            }
            Tool::Zone | Tool::Dezone => {
                let pts: Vec<Vec2> = (0..=48).map(|i| h + Vec2::from_angle(i as f32 / 48.0 * std::f32::consts::TAU) * self.brush).collect();
                ribbon(&mut out, t, &pts, 2.0, if self.tool == Tool::Zone { [0.5, 0.9, 0.4, 0.8] } else { [1.0, 0.45, 0.35, 0.8] });
            }
            Tool::Demolish => {
                if let Some(e) = self.city.net.nearest_edge(h, 10.0) {
                    let ed = &self.city.net.edges[&e.edge];
                    ribbon(&mut out, t, &ed.pts, ed.kind.width() + 1.0, bad);
                }
            }
            _ => {}
        }
        // Marca del punto de enganche.
        if matches!(self.tool, Tool::Straight | Tool::Curve | Tool::Grid) {
            let sn = self.city.net.snap(h);
            let col = if sn.node.is_some() || sn.edge.is_some() { [1.0, 0.85, 0.3, 0.9] } else { [1.0, 1.0, 1.0, 0.7] };
            let pts: Vec<Vec2> = (0..=12).map(|i| sn.pos + Vec2::from_angle(i as f32 / 12.0 * std::f32::consts::TAU) * 3.0).collect();
            ribbon(&mut out, t, &pts, 1.5, col);
        }
        out
    }

    fn road_preview(&self, out: &mut Vec<OverlayVertex>, t: &TerrainGen, pts: &[Vec2]) {
        let good = self.city.road_problem(t, pts, self.kind).is_none() && City::road_cost(t, pts, self.kind) <= self.city.money;
        ribbon(out, t, pts, self.kind.width(), if good { [0.45, 0.95, 0.45, 0.55] } else { [1.0, 0.35, 0.3, 0.55] });
    }
}

fn y_at(t: &TerrainGen, p: Vec2, lift: f32) -> f32 {
    t.height(p.x, p.y).max(0.0) + lift
}

/// Cinta translúcida que sigue una polilínea sobre el terreno.
/// Color de cada zona en el plano: verde las casas, azul el comercio, ocre la industria; la
/// densidad, más intensa cuanto más alta.
pub fn zone_color(k: ZoneKind, d: Density) -> [f32; 4] {
    let a = [0.35, 0.5, 0.65][d as usize];
    match k {
        ZoneKind::Residencial => [0.42, 0.85, 0.36, a],
        ZoneKind::Comercial => [0.35, 0.6, 0.95, a],
        ZoneKind::Industrial => [0.95, 0.72, 0.25, a],
    }
}

pub fn ribbon(out: &mut Vec<OverlayVertex>, t: &TerrainGen, pts: &[Vec2], w: f32, color: [f32; 4]) {
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        let n = (b - a).normalize_or_zero().perp() * (w * 0.5);
        let (ya, yb) = (y_at(t, a, 1.2), y_at(t, b, 1.2));
        let v = |p: Vec2, y: f32| OverlayVertex { pos: [p.x, y, p.y], color };
        out.extend_from_slice(&[v(a + n, ya), v(a - n, ya), v(b - n, yb), v(a + n, ya), v(b - n, yb), v(b + n, yb)]);
    }
}

fn outline(out: &mut Vec<OverlayVertex>, t: &TerrainGen, corners: &[Vec2; 4], w: f32, color: [f32; 4]) {
    let mut pts = Vec::new();
    for i in 0..4 {
        for k in 0..8 {
            pts.push(corners[i].lerp(corners[(i + 1) % 4], k as f32 / 8.0));
        }
    }
    pts.push(corners[0]);
    ribbon(out, t, &pts, w, color);
}

fn quad(out: &mut Vec<OverlayVertex>, t: &TerrainGen, c: &[Vec2; 4], lift: f32, color: [f32; 4]) {
    let v = |p: Vec2| OverlayVertex { pos: [p.x, y_at(t, p, lift), p.y], color };
    out.extend_from_slice(&[v(c[0]), v(c[1]), v(c[2]), v(c[0]), v(c[2]), v(c[3])]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El flujo de clics de la ventana: plaza, recta encadenada, curva en 3 clics, damero,
    /// zona con pincel, demoler y cancelar.
    #[test]
    fn los_clics_de_las_herramientas_construyen_la_ciudad() {
        let t = TerrainGen::new(1726);
        let mut g = Game::default();
        let c = Vec2::new(13500.0, 16300.0);
        g.set_tool(1);
        assert_eq!(g.tool, Tool::Plaza, "antes de fundar solo se puede fundar");
        assert!(g.press(&t, c).is_some(), "{}", g.message);
        assert_eq!(g.city.net.edges.len(), 4);
        g.city.money = 1_000_000;
        // Recta: el primer clic se engancha a la esquina de la plaza, el segundo construye.
        g.set_tool(1);
        let corner = c + Vec2::new(BLOCK * 0.5, -BLOCK * 0.5);
        assert!(g.press(&t, corner + Vec2::new(3.0, 2.0)).is_none());
        assert_eq!(g.p0, corner);
        assert!(g.press(&t, corner + Vec2::new(0.0, -120.0)).is_some(), "{}", g.message);
        // Curva: inicio, fin y el tercer clic da la forma.
        g.set_tool(2);
        g.press(&t, corner + Vec2::new(0.0, -120.0));
        g.press(&t, corner + Vec2::new(-160.0, -200.0));
        assert_eq!(g.stage, 2);
        let n = g.city.net.edges.len();
        assert!(g.press(&t, corner + Vec2::new(-40.0, -200.0)).is_some(), "{}", g.message);
        assert!(g.city.net.edges.len() > n);
        // Una curva demasiado cerrada no se construye y deja el aviso.
        g.cancel();
        g.press(&t, c + Vec2::new(-42.0, 42.0));
        g.press(&t, c + Vec2::new(-42.0, 100.0));
        assert!(g.press(&t, c + Vec2::new(-200.0, 72.0)).is_none());
        assert!(g.message.contains("cerrada") || g.message.contains("radio"), "{}", g.message);
        assert!(g.cancel());
        assert!(!g.cancel(), "sin nada que cancelar, Esc sale");
        // Damero al sur de la plaza.
        g.set_tool(3);
        g.press(&t, c + Vec2::new(-42.0, 42.0));
        g.press(&t, c + Vec2::new(126.0, 42.0));
        let n = g.city.net.edges.len();
        assert!(g.press(&t, c + Vec2::new(0.0, 210.0)).is_some(), "{}", g.message);
        assert!(g.city.net.edges.len() > n + 4);
        // Zona con el pincel y crecimiento.
        g.set_tool(4);
        g.brush = 150.0;
        g.press(&t, c);
        g.paint_at(c + Vec2::new(0.0, 150.0));
        g.release();
        assert!(g.city.lots.lots.values().filter(|l| l.zone > 0).count() > 20);
        assert!(g.update(&t, 30.0).is_some(), "algo crece en 3 meses");
        assert!(g.city.population() > 0);
        // Demoler un tramo quita la calle.
        g.set_tool(6);
        let n = g.city.net.edges.len();
        assert!(g.press(&t, corner + Vec2::new(0.0, -60.0)).is_some(), "{}", g.message);
        assert_eq!(g.city.net.edges.len(), n - 1);
    }
}


/// Número con puntos de miles, a la uruguaya: 12.500.
pub fn fmt_thousands(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}
