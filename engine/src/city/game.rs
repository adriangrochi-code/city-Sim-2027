//! Herramientas del modo juego (como en el prototipo): plaza mayor, calle recta, curva,
//! damero, zona, quitar zona, demoler e inspeccionar; vistas previas y línea de estado.

use super::lots::{corners_of, growth_blockers, zone_code, zone_name, zone_of, Density, ZoneKind, D, W};
use super::roads::{bezier, line, RoadKind};
use super::services::{self, Kind, Layer, Order, Placeable};
use super::{grid_lines, plaza_frame, City, BLOCK};
use crate::buildings::Edits;
use crate::render::OverlayVertex;
use crate::terrain::TerrainGen;
use glam::Vec2;
use std::sync::Mutex;
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
    /// Colocar un servicio, el muelle o la muralla (`Game::service`); no tiene atajo numérico.
    Service,
    /// Trazar el ensanche de la Ciudad Nueva (época 2; ver `epocas::ensanche`).
    Ensanche,
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

/// Cuánto se marcan las capas del mapa (menú de visibilidad).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visibilidad {
    /// 0: apagadas (solo las que el jugador elige a mano), 1: suaves, 2: marcadas.
    pub intensidad: u8,
    /// Mostrar sola la capa que corresponde a la herramienta (zonas, servicios, campo).
    pub auto: bool,
    /// Concentrar las capas alrededor del cursor (se desvanecen lejos).
    pub foco: bool,
}

impl Default for Visibilidad {
    fn default() -> Self {
        Visibilidad { intensidad: 1, auto: true, foco: true }
    }
}

/// Trazado de una calle con sus enganches (ver [`Game::draft`]).
pub struct Draft {
    /// Punto final (enganchado).
    pub end: Vec2,
    /// Guías a dibujar (pares de puntos).
    pub guides: Vec<(Vec2, Vec2)>,
    /// Qué enganche manda: "ángulo", "alineada" o "cuadra".
    pub locked: Option<&'static str>,
}

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
    /// Shift apretado: la calle se traza libre, sin enganches de ángulo ni de largo.
    pub free_draw: bool,
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
    /// Qué coloca la herramienta de servicios.
    pub service: Placeable,
    /// Capa de datos sobre el mapa (agua, serenos, fuego).
    pub layer: Option<Layer>,
    /// Pestaña abierta del cajón de Servicios (agua, educación, salud, seguridad…).
    pub grupo: services::Grupo,
    /// Órdenes de la interfaz que producen vóxeles (se cumplen en `update`).
    pub orders: Vec<Order>,
    /// Grilla de la capa de terreno ya calculada (clave, vértices): se rehace solo cuando
    /// cambia la capa, la ventana o el mes.
    capa_terreno: Mutex<Option<CapaTerreno>>,
    /// Menú de visibilidad de las capas.
    pub vis: Visibilidad,
    /// Aptitud de los solares vacíos para la zona elegida (zona, mes, solares → aptitud).
    aptitud_cache: Mutex<Option<((u8, u32, usize), std::collections::HashMap<u32, f32>)>>,
}

type CapaTerreno = ((u8, i32, i32, u32, usize), Vec<OverlayVertex>);

/// Ficha de lo inspeccionado para el panel de información.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inspect {
    pub title: String,
    pub lines: Vec<String>,
    /// "¿Por qué no crece?" (vacío si no aplica).
    pub why: Vec<String>,
    pub kind: InspectKind,
    /// Solar inspeccionado y cuántas personas viven en él (para abrir sus vecinos).
    pub lot: Option<u32>,
    pub vecinos: usize,
}

/// Qué se está inspeccionando (decide los botones de acción de la ficha).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InspectKind {
    Lot,
    Road,
    #[default]
    Terrain,
    /// Un edificio de servicio (su id).
    Service(u32),
    /// La iglesia matriz (obra por fases).
    Church,
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
            service: Placeable::Building(Kind::Pozo),
            layer: None,
            grupo: services::Grupo::Agua,
            orders: Vec::new(),
            capa_terreno: Mutex::new(None),
            vis: Visibilidad::default(),
            aptitud_cache: Mutex::new(None),
            plaza_angle: 0.0,
            free_draw: false,
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
        if self.tool == Tool::Service {
            return self.service.name();
        }
        TOOLS.iter().find(|t| t.0 == self.tool).map_or("", |t| t.1)
    }

    /// Elige qué colocar con la herramienta de servicios.
    pub fn set_service(&mut self, p: Placeable) {
        if self.city.plaza.is_none() {
            self.say("Primero funda la plaza mayor");
            return;
        }
        self.tool = Tool::Service;
        self.service = p;
        if let Placeable::Building(k) = p {
            if services::info(k).grupo != services::Grupo::Otros {
                self.grupo = services::info(k).grupo;
            }
        }
        self.stage = 0;
        self.painting = false;
    }

    /// Esc / clic derecho: cancela el trazado en curso. Devuelve si había algo que cancelar.
    pub fn cancel(&mut self) -> bool {
        let had = self.stage > 0 || self.painting;
        self.stage = 0;
        self.painting = false;
        had
    }

    /// Suelta la herramienta (vuelve al cursor libre de inspeccionar). Devuelve si había una.
    pub fn drop_tool(&mut self) -> bool {
        if self.city.plaza.is_none() || matches!(self.tool, Tool::Inspect) {
            return false;
        }
        self.tool = Tool::Inspect;
        self.stage = 0;
        self.painting = false;
        true
    }

    pub fn snap(&self, p: Vec2) -> Vec2 {
        self.city.net.snap(p).pos
    }

    /// Direcciones de referencia para trazar desde `from`: el eje de la plaza mayor (el damero
    /// fundacional) y la calle de la que se arranca, si se arranca de una.
    fn reference_dirs(&self, from: Vec2) -> Vec<Vec2> {
        let mut refs = vec![self.city.plaza.map_or(Vec2::X, |(_, a)| plaza_frame(a).0)];
        let net = &self.city.net;
        let sn = net.snap(from);
        let mut add_edge = |eid: u32, near: Vec2| {
            if let Some(ed) = net.edges.get(&eid) {
                // Tangente del tramo en el punto más cercano a `from`.
                let (mut best, mut bd) = (Vec2::ZERO, f32::MAX);
                for w in ed.pts.windows(2) {
                    let q = super::roads::closest_on_segment(near, w[0], w[1]);
                    let d = q.distance(near);
                    if d < bd {
                        bd = d;
                        best = (w[1] - w[0]).normalize_or_zero();
                    }
                }
                if best != Vec2::ZERO {
                    refs.push(best);
                }
            }
        };
        if let Some(n) = sn.node {
            let ids: Vec<u32> = net.edges.iter().filter(|(_, e)| e.a == n || e.b == n).map(|(&id, _)| id).collect();
            for id in ids {
                add_edge(id, from);
            }
        } else if let Some((eid, _)) = sn.edge {
            add_edge(eid, from);
        }
        refs
    }

    /// Calle recta desde `from` hacia el cursor con los enganches de los juegos de ciudad:
    /// ángulo a 0°, 45° y 90° respecto de la calle de arranque y del damero de la plaza (a
    /// menos de 7°), largo alineado con las esquinas de otras calles (para que las cuadras
    /// salgan parejas) o con el módulo de la cuadra, y enganche a esquinas y calles. Con Shift,
    /// libre.
    pub fn draft(&self, from: Vec2, cursor: Vec2) -> Draft {
        let net = &self.city.net;
        let sn = net.snap(cursor);
        let mut d = Draft { end: sn.pos, guides: Vec::new(), locked: None };
        if sn.node.is_some() || self.free_draw {
            return d;
        }
        let v = cursor - from;
        let len = v.length();
        if len < 2.0 {
            return d;
        }
        // Ángulo más cercano entre los de referencia (cada 45°).
        let mut best: Option<(Vec2, f32)> = None;
        for r in self.reference_dirs(from) {
            for k in 0..8 {
                let c = r.rotate(Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_4));
                let a = c.angle_to(v).abs();
                if best.is_none_or(|b| a < b.1) {
                    best = Some((c, a));
                }
            }
        }
        let Some((dir, err)) = best else { return d };
        if err > 7f32.to_radians() {
            return d; // fuera de los enganches de ángulo: lo que diga el cursor (o la calle cercana)
        }
        let mut l = v.dot(dir).max(1.0);
        d.locked = Some("ángulo");
        // Alineación: una esquina cuya proyección sobre la calle cae cerca del largo actual.
        let mut align: Option<(f32, Vec2)> = None;
        for &q in net.nodes.values() {
            if q.distance(from) < 1.0 || q.distance(from) > 900.0 {
                continue;
            }
            let along = (q - from).dot(dir);
            if along > 5.0 && (along - l).abs() < 4.0 && align.is_none_or(|a| (along - l).abs() < (a.0 - l).abs()) {
                align = Some((along, q));
            }
        }
        if let Some((along, q)) = align {
            l = along;
            d.guides.push((q, from + dir * l));
            d.locked = Some("alineada");
        } else {
            let m = (l / BLOCK).round() * BLOCK;
            if m > 0.0 && (l - m).abs() < 3.0 {
                l = m;
                d.locked = Some("cuadra");
            }
        }
        let end = from + dir * l;
        // Si el final cae sobre una esquina o una calle, se engancha (cruce).
        let s2 = net.snap(end);
        d.end = if s2.node.is_some() || s2.edge.is_some() { s2.pos } else { end };
        d.guides.push((from - dir * 30.0, end + dir * 30.0));
        d
    }

    /// Al zonificar: aptitud del solar vacío bajo el cursor para la zona elegida.
    pub fn zona_label(&self) -> Option<String> {
        if self.tool != Tool::Zone {
            return None;
        }
        let id = self.city.lots.lot_at(self.hover?)?;
        if self.city.lots.lots[&id].level > 0 {
            return None;
        }
        let c = self.aptitud_cache.lock().unwrap();
        let a = *c.as_ref()?.1.get(&id)?;
        let que = match a {
            x if x < 0.25 => "mala",
            x if x < 0.45 => "regular",
            x if x < 0.65 => "buena",
            _ => "muy buena",
        };
        Some(format!("Aptitud para {}: {} ({} %)", self.zone_kind.name().to_lowercase(), que, (a * 100.0).round() as i32))
    }

    /// Texto junto al cursor mientras se traza una calle: largo, ángulo y enganche.
    pub fn draft_label(&self, _t: &TerrainGen) -> Option<String> {
        let h = self.hover?;
        let (from, to) = match (self.tool, self.stage) {
            (Tool::Straight | Tool::Curve, 1) | (Tool::Grid | Tool::Ensanche, 1) => (self.p0, self.draft(self.p0, h)),
            _ => return None,
        };
        let v = to.end - from;
        let deg = (v.y.atan2(v.x).to_degrees() + 360.0) % 360.0;
        // Ángulo respecto del damero de la plaza (lo que importa para cuadricular).
        let base = self.city.plaza.map_or(0.0, |(_, a)| {
            let f = plaza_frame(a).0;
            f.y.atan2(f.x).to_degrees()
        });
        let rel = ((deg - base) % 90.0 + 90.0) % 90.0;
        let rel = if rel > 45.0 { rel - 90.0 } else { rel };
        let mut s = format!("{} m · {:+.0}° del damero", v.length().round() as i32, rel);
        if let Some(l) = to.locked {
            s.push_str(&format!(" · {l}"));
        }
        if self.free_draw {
            s.push_str(" · libre (Shift)");
        }
        Some(s)
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
                // Queda el cursor libre (inspeccionar), para mirar la ciudad sin herramienta.
                self.tool = Tool::Inspect;
                Some(e)
            }
            Tool::Straight => {
                if self.stage == 0 {
                    self.p0 = self.snap(p);
                    self.stage = 1;
                    return None;
                }
                let end = self.draft(self.p0, p).end;
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
                    self.p1 = self.draft(self.p0, p).end;
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
            Tool::Ensanche => match self.stage {
                0 => {
                    self.p0 = self.snap(p);
                    self.stage = 1;
                    None
                }
                1 => {
                    self.p1 = self.draft(self.p0, p).end;
                    if self.p1.distance(self.p0) > BLOCK * 1.5 {
                        self.stage = 2;
                    }
                    None
                }
                _ => {
                    let r = self.city.trazar_ensanche(t, self.p0, self.p1, p);
                    let e = self.report(r)?;
                    self.say("Trazado el ensanche: calles, bulevar con su alameda y plazas. Zonificá las manzanas nuevas");
                    self.stage = 0;
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
                    self.p1 = self.draft(self.p0, p).end;
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
            Tool::Service => self.press_service(t, p),
        }
    }

    fn press_service(&mut self, t: &TerrainGen, p: Vec2) -> Option<Edits> {
        match self.service {
            Placeable::Building(k) => {
                let (c, f) = self.city.service_site(k, p);
                let r = self.city.place_service(t, k, c, f);
                let e = self.report(r)?;
                self.say(format!("Construido: {}", services::info(k).name.to_lowercase()));
                Some(e)
            }
            Placeable::Muelle => {
                let r = self.city.build_pier(t, p);
                let e = self.report(r)?;
                self.say("Construido el muelle: ya pueden atracar los barcos");
                Some(e)
            }
            Placeable::Muralla => {
                if self.stage == 0 {
                    self.p0 = p;
                    self.stage = 1;
                    return None;
                }
                let r = self.city.build_wall(t, self.p0, p);
                let e = self.report(r)?;
                self.say("Trazado un tramo de muralla: se levanta con la piedra de la cantera");
                self.p0 = p;
                Some(e)
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
    /// Ficha con el terreno a mano: la de [`Self::inspect_info`] más el suelo del punto
    /// (grupo CONEAT, productividad, aptitudes, geología, materiales y molestias).
    pub fn inspect_info_en(&self, t: &TerrainGen) -> Option<Inspect> {
        let mut i = self.inspect_info()?;
        if matches!(i.kind, InspectKind::Lot | InspectKind::Terrain) {
            let p = self.inspected?;
            i.lines.extend(lineas_terreno(t, p, i.kind == InspectKind::Terrain));
            if i.kind == InspectKind::Terrain {
                let m = self.city.suelo.campo.en(p, None);
                if m.total() >= 0.03 {
                    i.lines.push(format!("Molestias: {} % (sobre todo {})", (m.total() * 100.0).round() as i32, m.principal()));
                }
            }
        }
        Some(i)
    }

    pub fn inspect_info(&self) -> Option<Inspect> {
        let p = self.inspected?;
        if let Some(id) = self.city.services.at(p) {
            let b = &self.city.services.buildings[&id];
            return Some(Inspect { title: b.info().name.into(), lines: self.city.service_lines(id), why: vec![], kind: InspectKind::Service(id), ..Default::default() });
        }
        if let Some((c, f)) = self.city.church_site() {
            if super::lots::point_in_poly(p, &corners_of(c, f, 18.0, 44.0)) {
                return Some(Inspect { title: "Iglesia matriz".into(), lines: self.city.church_lines(), why: vec![], kind: InspectKind::Church, ..Default::default() });
            }
        }
        if let Some(id) = self.city.lots.lot_at(p) {
            let l = &self.city.lots.lots[&id];
            let front = self.city.lots.frontage(l, &self.city.net);
            let value = self.city.lots.land_value(l);
            // El nombre sigue a los materiales («rancho de adobe», «casa de tablas»).
            let nombre = self.city.obra_de(l).and_then(|o| super::materiales::ciudad::nombre_con_obra(l, &o)).unwrap_or_else(|| l.name().to_string());
            let mut i = Inspect { title: capitalize(&nombre), kind: InspectKind::Lot, lot: Some(id), ..Default::default() };
            i.lines.push(format!("Zona: {}", zone_name(l.zone)));
            let pob = &self.city.poblacion;
            i.vecinos = pob.habitantes_de_lote(id).count();
            match l.kind() {
                Some(ZoneKind::Residencial) | None if pob.fundada => {
                    let familias = pob.hogares_de_lote(id).count();
                    i.lines.push(format!("Habitantes: {} de {} ({} {})", i.vecinos, l.residents(), familias, if familias == 1 { "familia" } else { "familias" }));
                    if let Some(&d) = pob.duenos.get(&id) {
                        i.lines.push(format!("Dueño: familia {}", pob.apellido_hogar(d)));
                    }
                }
                Some(ZoneKind::Residencial) | None => i.lines.push(format!("Habitantes: {}", l.residents())),
                Some(ZoneKind::Comercial) => i.lines.push(format!("Empleos: {} de {} (comercio)", pob.trabajadores_de_lote(id).count(), super::lots::JOBS_C[l.level as usize])),
                Some(ZoneKind::Industrial) => i.lines.push(format!("Empleos: {} de {} (talleres)", pob.trabajadores_de_lote(id).count(), super::lots::JOBS_I[l.level as usize])),
            }
            i.lines.push(format!("Valor del suelo: {} %", (value * 100.0) as i32));
            i.lines.extend(self.city.lineas_suelo_lote(id));
            i.lines.push(format!("Calle: {}", front.map_or("—", |k| k.name())));
            if l.level > 0 {
                i.lines.push(format!("Antigüedad: {} {}", l.age, if l.age == 1 { "mes" } else { "meses" }));
            }
            if l.level > 0 {
                let w = self.city.services.water_of(l.id);
                i.lines.push(format!("Agua: {}", if w <= 0.0 { "no tiene (acarrean del arroyo)" } else if w < 0.6 { "mala" } else if w < 0.95 { "regular" } else { "buena" }));
                // Con qué está hecho y lo que eso significa (ver `materiales`).
                if let Some(o) = self.city.obra_de(l) {
                    i.lines.extend(self.city.lineas_obra(&o));
                }
                // Cómo es la casa: tipología, año y el origen de la familia que la levantó.
                i.lines.extend(self.city.lineas_estilo(l));
                i.lines.push(format!("Techo: {} · riesgo de fuego {} %", super::services::fires::roof_of(l).name(), (self.city.fire_risk(l) * 100.0) as i32));
                if self.city.services.fires.burning.contains_key(&l.id) {
                    i.lines.push("¡Está ardiendo!".into());
                }
                let br = &self.city.services.barrio;
                if l.kind() == Some(ZoneKind::Residencial) {
                    let es = br.escuela_de(l.id);
                    i.lines.push(format!("Escuela: {}", if es <= 0.0 { "no hay lugar cerca" } else if es < 0.9 { "las aulas del convento" } else { "escuela de primeras letras" }));
                }
                let sa = br.salud_de(l.id);
                i.lines.push(format!("Salud: {}", if sa <= 0.0 { "nadie atiende" } else if sa < 0.5 { "botica o médico" } else { "bien atendido" }));
                let ins = br.insalubre_de(l.id);
                i.lines.push(format!("Insalubridad: {} % ({})", (ins * 100.0).round() as i32, if ins < 0.2 { "sana" } else if ins < 0.4 { "regular" } else { "pozos negros, agua mala o basura" }));
                let d = br.delito_de(l.id);
                i.lines.push(format!("Delito: {} · vigilancia {} %", if d < 0.1 { "bajo" } else if d < 0.25 { "medio" } else { "alto" }, (br.vigilancia_de(l.id) * 100.0).round() as i32));
            } else if self.city.services.ruins.contains(&l.id) {
                i.lines.push("Ruinas: se quemó; volverá a edificarse".into());
            } else if self.city.services.barrio.basurales.contains(&l.id) {
                i.lines.push("Basural: la basura del barrio; ensucia el agua y trae enfermedades".into());
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
        // Un vecino, jinete o carreta justo ahí (a menos de 2,5 m).
        if let Some((title, lines)) = self.city.agentes.cerca_de(&self.city.net, p, 2.5).and_then(|id| self.city.agentes.describir(id)) {
            return Some(Inspect { title, lines, why: vec![], kind: InspectKind::Terrain, ..Default::default() });
        }
        if let Some(e) = self.city.net.nearest_edge(p, 8.0) {
            let ed = &self.city.net.edges[&e.edge];
            let mut lines = vec![format!("Largo del tramo: {} m", ed.len() as i32)];
            lines.extend(super::agentes::lineas_de_tramo(&self.city.agentes, &self.city.net, e.edge));
            return Some(Inspect { title: ed.kind.name().into(), lines, kind: InspectKind::Road, ..Default::default() });
        }
        Some(Inspect { title: "Terreno".into(), lines: vec![format!("Posición: {:.0}, {:.0}", p.x, p.y)], kind: InspectKind::Terrain, ..Default::default() })
    }

    /// Pista de la herramienta para la interfaz: (ayuda con el costo en vivo, motivo de error).
    pub fn hint(&self, t: &TerrainGen) -> (String, Option<String>) {
        let help = match self.tool {
            Tool::Plaza => "Elige el sitio de la plaza mayor (terreno firme y llano). R la gira.",
            Tool::Straight => "Clic en el inicio y clic en el final. Sigue encadenando tramos; Esc para terminar.",
            Tool::Curve => "Clic en el inicio, clic en el final y mueve el ratón para doblarla. Un tercer clic la fija.",
            Tool::Grid => "Clic en una esquina, clic en la punta del primer lado y extiende las manzanas (cuadras de 84 m).",
            Tool::Ensanche => "Ensanche sobre el ejido: clic en una esquina, clic en la punta del primer lado y extiende las manzanas hacia el interior. El bulevar arbolado va por el medio y hay una plaza cada tres manzanas.",
            Tool::Zone => "Pinta los solares junto a las calles: residencial (casas), comercial (tiendas) o industrial (talleres), en densidad baja, media o alta.",
            Tool::Dezone => "Pinta para quitar la zona (derriba lo construido).",
            Tool::Demolish => "Clic sobre un tramo de calle para demolerlo.",
            Tool::Inspect => "Clic en un solar, una calle o el terreno para ver su información.",
            Tool::Service => match self.service {
                Placeable::Building(k) => services::info(k).desc,
                Placeable::Muelle => "Clic en una orilla baja junto a una calle: el muelle entra al río hasta el agua profunda.",
                Placeable::Muralla => "Clic en el inicio y clic en el final del tramo. Donde cruza una calle queda un portón.",
            },
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
                let (c, e) = road(&line(self.p0, self.draft(self.p0, h).end));
                (Some(c), e)
            }
            Tool::Curve if self.stage == 2 => {
                let (c, e) = road(&self.curve_points(h));
                (Some(c), e)
            }
            Tool::Ensanche if self.stage == 2 => match super::epocas::ensanche::plan(self.p0, self.p1, h) {
                Some(p) => (Some(self.city.ensanche_costo(t, &p)), self.city.ensanche_problema(t, &p)),
                None => (None, Some("El ensanche necesita al menos dos manzanas de lado".into())),
            },
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
            Tool::Service => match self.service {
                Placeable::Building(k) => {
                    let (c, f) = self.city.service_site(k, h);
                    let i = services::info(k);
                    help = format!("{help}\nCosto: {} pesos", fmt_thousands(i.cost));
                    // La receta y lo que se sustituye con lo que hay hoy (ver `materiales`).
                    if let Some((_, txt)) = self.city.vista_previa_servicio(k) {
                        help = format!("{help}\n{txt}");
                    }
                    if matches!(k, Kind::Chacra | Kind::Estancia | Kind::Horno | Kind::Cantera | Kind::Calera) {
                        let r = super::recursos::Recursos::de(t).en_area(c, f, i.size.0, i.size.1);
                        let (que, x) = match k {
                            Kind::Chacra => ("tierra para cultivo", r.cultivo),
                            Kind::Estancia => ("pasturas", r.pastoreo),
                            Kind::Horno => ("arcilla", r.arcilla),
                            Kind::Calera => ("piedra de cal", r.de(super::recursos::Uso::Cal)),
                            _ => ("material", super::recursos::materiales::ley_cantera(&r)),
                        };
                        help = format!("{help}\nSitio: {que} {} (IP {}) · rinde {} %", super::recursos::calidad(x), r.ip.round() as i32, (super::recursos::rinde(k, &r) * 100.0).round() as i32);
                        if let Some(p) = super::recursos::produccion_sitio(k, &r) {
                            help = format!("{help}\nSaca: {}", p.texto());
                        }
                    }
                    (None, self.city.service_problem(t, k, c, f))
                }
                Placeable::Muelle => (Some(services::port::PIER_COST), self.city.pier_problem(t, h)),
                Placeable::Muralla if self.stage == 1 => {
                    let cost = (self.p0.distance(h) * services::works::WALL_COST_PER_M as f32) as i64;
                    help = format!("{help}\n{} m · {} de piedra en la obra", self.p0.distance(h) as i32, (self.p0.distance(h) * services::works::WALL_STONE_PER_M) as i32);
                    (Some(cost), self.city.wall_problem(t, self.p0, h))
                }
                Placeable::Muralla => (None, None),
            },
            _ => (None, None),
        };
        if let Some(c) = cost {
            if self.tool == Tool::Service {
                help = format!("{help}\nCosto: {} pesos", fmt_thousands(c));
            } else {
                help = format!("{help}\n{} · costo: {} pesos", self.kind.name(), fmt_thousands(c));
            }
        }
        (help, err)
    }

    /// Avanza la simulación. Devuelve ediciones si algo creció.
    pub fn update(&mut self, t: &TerrainGen, dt: f32) -> Option<Edits> {
        let mut e = Edits::default();
        for o in std::mem::take(&mut self.orders) {
            let r = match o {
                Order::FundChurch => self.city.start_church_phase(t).map(|(ed, m)| {
                    e.extend(ed);
                    m
                }),
                Order::Demolish(id) => self.city.demolish_service(t, id).map(|ed| {
                    e.extend(ed);
                    "Servicio demolido".to_string()
                }),
                Order::PasarEpoca(m) => self.city.pasar_epoca(t, m, false).map(|(ed, aviso)| {
                    e.extend(ed);
                    aviso
                }),
                Order::Reforma(id, r) => self.city.rehabilitar(t, id, r, true).map(|ed| {
                    e.extend(ed);
                    format!("{}: hecho", r.nombre())
                }),
            };
            match r {
                Ok(m) | Err(m) => self.say(m),
            }
            if matches!(o, Order::Demolish(_)) {
                self.inspected = None;
            }
        }
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
        let capas0 = out.len();
        // Al zonificar, los solares vacíos se tiñen según su aptitud para la zona elegida (de
        // rojo, mala, a verde, buena), sin abrir ninguna capa.
        let aptitud = (self.tool == Tool::Zone && self.vis.auto && self.vis.intensidad > 0).then(|| {
            let clave = (self.zone_kind as u8, self.city.month, self.city.lots.lots.len());
            let mut c = self.aptitud_cache.lock().unwrap();
            if c.as_ref().is_none_or(|x| x.0 != clave) {
                *c = Some((clave, self.city.aptitud_zona(self.zone_kind)));
            }
            c.as_ref().unwrap().1.clone()
        });
        for l in self.city.lots.lots.values() {
            if !show_free || l.level > 0 {
                continue;
            }
            let col = match zone_of(l.zone) {
                Some((k, d)) => zone_color(k, d),
                None => match aptitud.as_ref().and_then(|a| a.get(&l.id)) {
                    Some(&a) => aptitud_color(a),
                    None => [1.0, 0.97, 0.85, 0.18],
                },
            };
            quad(&mut out, t, &corners_of(l.c, l.f, W - 1.2, D - 1.2), 0.9, col);
        }
        // Capa de datos (o la que corresponde al servicio que se está colocando).
        let auto = self.vis.auto && self.vis.intensidad > 0;
        let layer = self.layer.or(if !auto { None } else { match (self.tool, self.service) {
            (Tool::Service, Placeable::Building(Kind::Pozo | Kind::Aljibe | Kind::Aguadores)) => Some(Layer::Agua),
            (Tool::Service, Placeable::Building(Kind::Serenos)) => Some(Layer::Serenos),
            (Tool::Service, Placeable::Building(Kind::Escuela | Kind::Convento)) => Some(Layer::Educacion),
            (Tool::Service, Placeable::Building(Kind::Botica | Kind::Medico | Kind::Hospital)) => Some(Layer::Salud),
            (Tool::Service, Placeable::Building(Kind::Alcalde | Kind::Carcel)) => Some(Layer::Delito),
            (Tool::Service, Placeable::Building(Kind::Basurero)) => Some(Layer::Saneamiento),
            (Tool::Service, Placeable::Building(Kind::Chacra)) => Some(Layer::Cultivo),
            (Tool::Service, Placeable::Building(Kind::Estancia)) => Some(Layer::Pasturas),
            (Tool::Service, Placeable::Building(Kind::Horno)) => Some(Layer::Arcilla),
            (Tool::Service, Placeable::Building(Kind::Cantera)) => Some(Layer::Piedra),
            (Tool::Service, Placeable::Building(Kind::Calera)) => Some(Layer::Cal),
            _ => None,
        } });
        if let Some(layer) = layer {
            if layer.es_de_terreno() {
                self.terrain_layer(&mut out, t, layer);
            }
            self.layer_overlay(&mut out, t, layer);
        }
        // Visibilidad: las capas (y el plano de zonas) se marcan según el menú y, si se pide, se
        // concentran alrededor del cursor. Una capa elegida a mano se ve aunque estén apagadas.
        let elegida = self.layer.is_some();
        let k = match self.vis.intensidad {
            0 if elegida => 0.6,
            0 => 0.0,
            1 => 0.6,
            _ => 1.0,
        };
        let (r0, r1) = if elegida { (700.0, 1600.0) } else { (220.0, 600.0) };
        let foco = if self.vis.foco { self.hover } else { None };
        for v in &mut out[capas0..] {
            let mut a = k;
            if let Some(h) = foco {
                let d = Vec2::new(v.pos[0], v.pos[2]).distance(h);
                a *= 1.0 - 0.85 * ((d - r0) / (r1 - r0)).clamp(0.0, 1.0);
            }
            v.color[3] *= a;
        }
        // Lo inspeccionado queda marcado mientras la ficha está abierta.
        if let Some(p) = self.inspected {
            if let Some(id) = self.city.services.at(p) {
                let b = &self.city.services.buildings[&id];
                outline(&mut out, t, &b.corners(), 1.6, [1.0, 0.85, 0.3, 0.95]);
                circle(&mut out, t, b.c, b.info().radius, [1.0, 0.85, 0.3, 0.7]);
            } else if let Some(id) = self.city.lots.lot_at(p) {
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
            Tool::Straight | Tool::Curve if self.stage == 1 => {
                let d = self.draft(self.p0, h);
                self.guides_preview(&mut out, t, &d);
                self.road_preview(&mut out, t, &line(self.p0, d.end));
            }
            Tool::Curve if self.stage == 2 => self.road_preview(&mut out, t, &self.curve_points(h)),
            Tool::Grid | Tool::Ensanche if self.stage == 1 => {
                let d = self.draft(self.p0, h);
                self.guides_preview(&mut out, t, &d);
                ribbon(&mut out, t, &line(self.p0, d.end), 3.0, [1.0, 1.0, 1.0, 0.6]);
            }
            Tool::Ensanche if self.stage == 2 => {
                if let Some(p) = super::epocas::ensanche::plan(self.p0, self.p1, h) {
                    let good = self.city.ensanche_problema(t, &p).is_none();
                    for l in &p.lineas {
                        ribbon(&mut out, t, l, RoadKind::Stone.width(), if good { ok } else { bad });
                    }
                    for &(c, f) in &p.plazas {
                        outline(&mut out, t, &corners_of(c, f, 70.0, 70.0), 3.0, [0.4, 0.8, 0.4, 0.7]);
                    }
                }
            }
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
            Tool::Service => self.service_preview(&mut out, t, h),
            Tool::Demolish => {
                if let Some(e) = self.city.net.nearest_edge(h, 10.0) {
                    let ed = &self.city.net.edges[&e.edge];
                    ribbon(&mut out, t, &ed.pts, ed.kind.width() + 1.0, bad);
                }
            }
            _ => {}
        }
        // Marca del punto de enganche.
        if matches!(self.tool, Tool::Straight | Tool::Curve | Tool::Grid | Tool::Ensanche) {
            let sn = self.city.net.snap(h);
            let col = if sn.node.is_some() || sn.edge.is_some() { [1.0, 0.85, 0.3, 0.9] } else { [1.0, 1.0, 1.0, 0.7] };
            let pts: Vec<Vec2> = (0..=12).map(|i| sn.pos + Vec2::from_angle(i as f32 / 12.0 * std::f32::consts::TAU) * 3.0).collect();
            ribbon(&mut out, t, &pts, 1.5, col);
        }
        out
    }

    /// Vista previa del servicio a colocar: huella, alcance y, para la muralla, el tramo.
    fn service_preview(&self, out: &mut Vec<OverlayVertex>, t: &TerrainGen, h: Vec2) {
        let ok = [0.45, 0.95, 0.45, 0.55];
        let bad = [1.0, 0.35, 0.3, 0.55];
        match self.service {
            Placeable::Building(k) => {
                let (c, f) = self.city.service_site(k, h);
                let i = services::info(k);
                let good = self.city.service_problem(t, k, c, f).is_none();
                quad(out, t, &corners_of(c, f, i.size.0, i.size.1), 1.0, if good { ok } else { bad });
                if i.radius > 0.0 {
                    circle(out, t, c, i.radius, [0.35, 0.65, 1.0, 0.8]);
                }
            }
            Placeable::Muelle => {
                let good = self.city.pier_problem(t, h).is_none();
                // El muelle se engancha a la orilla más cercana: se ve dónde va a quedar.
                let s = services::port::pier_start(t, h).unwrap_or(h);
                let dir = services::port::find_sea(t, s).map_or(Vec2::Y * 40.0, |(d, dist)| d * (dist + 25.0).clamp(40.0, services::port::PIER_MAX_LEN));
                ribbon(out, t, &line(s, s + dir), 6.0, if good { ok } else { bad });
            }
            Placeable::Muralla if self.stage == 1 => {
                let good = self.city.wall_problem(t, self.p0, h).is_none();
                ribbon(out, t, &line(self.p0, h), services::works::WALL_THICK, if good { [0.75, 0.72, 0.65, 0.7] } else { bad });
                for (g, w) in self.city.wall_gates(self.p0, h) {
                    let d = (h - self.p0).normalize_or_zero();
                    let p = self.p0 + d * g;
                    ribbon(out, t, &line(p - d * w * 0.5, p + d * w * 0.5), services::works::WALL_THICK + 1.0, [1.0, 0.85, 0.3, 0.8]);
                }
            }
            Placeable::Muralla => circle(out, t, h, 3.0, [1.0, 1.0, 1.0, 0.7]),
        }
        // Los tramos de muralla ya trazados (lo levantado y lo que falta).
        if self.service == Placeable::Muralla {
            for tr in &self.city.services.walls.tramos {
                let d = (tr.b - tr.a).normalize_or_zero();
                ribbon(out, t, &line(tr.a, tr.a + d * tr.built), 2.0, [0.45, 0.95, 0.45, 0.6]);
                ribbon(out, t, &line(tr.a + d * tr.built, tr.b), 2.0, [1.0, 0.85, 0.3, 0.6]);
            }
        }
    }

    /// Colores de una capa de datos sobre los solares edificados.
    fn layer_overlay(&self, out: &mut Vec<OverlayVertex>, t: &TerrainGen, layer: Layer) {
        let s = &self.city.services;
        for l in self.city.lots.lots.values() {
            let baldio_visible = match layer {
                Layer::Fuego => s.ruins.contains(&l.id),
                Layer::Saneamiento | Layer::Delito => s.barrio.basurales.contains(&l.id) || s.ruins.contains(&l.id),
                _ => false,
            };
            // El valor del suelo se ve también en los solares vacíos (adónde conviene crecer).
            if l.level == 0 && !baldio_visible && layer != Layer::Valor {
                continue;
            }
            let br = &s.barrio;
            let residencial = l.kind() == Some(ZoneKind::Residencial);
            let col = match layer {
                Layer::Agua => match s.water_of(l.id) {
                    w if w <= 0.0 => [0.95, 0.35, 0.25, 0.5],
                    w if w < 0.6 => [0.85, 0.75, 0.3, 0.5],
                    w if w < 0.95 => [0.45, 0.75, 0.95, 0.5],
                    _ => [0.2, 0.45, 0.95, 0.6],
                },
                Layer::Serenos if s.watched(l.c) => [1.0, 0.85, 0.35, 0.55],
                Layer::Serenos => [0.25, 0.25, 0.35, 0.4],
                Layer::Fuego if s.fires.burning.contains_key(&l.id) => [1.0, 0.3, 0.05, 0.85],
                Layer::Fuego if s.ruins.contains(&l.id) => [0.2, 0.18, 0.16, 0.7],
                Layer::Fuego => {
                    let r = self.city.fire_risk(l);
                    [0.95, 0.85 - r * 0.6, 0.3, 0.25 + r * 0.4]
                }
                // Educación: verde las casas con escuela, rojo las que no tienen.
                Layer::Educacion if !residencial => continue,
                Layer::Educacion => match br.escuela_de(l.id) {
                    x if x <= 0.0 => [0.95, 0.35, 0.25, 0.5],
                    x if x < 0.9 => [0.6, 0.85, 0.35, 0.55],
                    _ => [0.25, 0.75, 0.3, 0.6],
                },
                // Salud: de rojo (nadie atiende) a verde (bien atendido).
                Layer::Salud => {
                    let x = br.salud_de(l.id);
                    [0.95 - x * 0.7, 0.35 + x * 0.45, 0.3, 0.5]
                }
                // Saneamiento: celeste lo sano, amarillo lo regular, pardo y rojo lo insalubre;
                // los basurales, negros.
                Layer::Saneamiento if l.level == 0 => [0.12, 0.08, 0.05, 0.85],
                Layer::Saneamiento => match br.insalubre_de(l.id) {
                    x if x < 0.15 => [0.3, 0.7, 0.85, 0.55],
                    x if x < 0.3 => [0.85, 0.8, 0.3, 0.55],
                    x if x < 0.45 => [0.85, 0.45, 0.15, 0.6],
                    _ => [0.7, 0.12, 0.08, 0.7],
                },
                // Delito: mapa de calor de amarillo pálido a rojo oscuro; baldíos y ruinas, grises.
                Layer::Delito if l.level == 0 => [0.3, 0.28, 0.28, 0.7],
                Layer::Delito => match br.delito_de(l.id) {
                    x if x < 0.08 => [1.0, 0.95, 0.65, 0.4],
                    x if x < 0.16 => [1.0, 0.78, 0.25, 0.55],
                    x if x < 0.24 => [0.95, 0.45, 0.12, 0.65],
                    _ => [0.7, 0.05, 0.05, 0.75],
                },
                // Valor del suelo: de azul pálido (poco) a amarillo y rojo (mucho).
                Layer::Valor => {
                    let mut c = valor_color(self.city.lots.land_value(l));
                    if l.level == 0 {
                        c[3] *= 0.6;
                    }
                    c
                }
                // Molestias en cada solar: de nada (gris claro) a violeta oscuro.
                Layer::Molestias => {
                    let m = self.city.suelo.molestia_de(l.id).total();
                    if m < 0.03 {
                        [0.85, 0.9, 0.85, 0.25]
                    } else {
                        [0.75 - 0.35 * m, 0.45 - 0.35 * m, 0.85 - 0.3 * m, 0.4 + 0.45 * m]
                    }
                }
                // Recursos: los solares no se pintan (se ve el terreno).
                Layer::Cultivo | Layer::Pasturas | Layer::Piedra | Layer::Arcilla | Layer::Arena | Layer::Cal => continue,
            };
            quad(out, t, &corners_of(l.c, l.f, W - 1.2, D - 1.2), 0.9, col);
        }
        // Alcance de cada fuente o puesto.
        for b in s.buildings.values() {
            let g = b.info().grupo;
            let show = match layer {
                Layer::Agua => b.info().cap > 0,
                Layer::Serenos => b.kind == Kind::Serenos,
                Layer::Fuego => b.info().fire_reach > 0.0,
                Layer::Educacion => g == services::Grupo::Educacion,
                Layer::Salud => matches!(b.kind, Kind::Botica | Kind::Medico | Kind::Hospital),
                Layer::Saneamiento => b.kind == Kind::Basurero,
                Layer::Delito => g == services::Grupo::Seguridad,
                Layer::Molestias => super::recursos::molestias::perfil_de_servicio(b.kind).is_some(),
                Layer::Cultivo => b.kind == Kind::Chacra,
                Layer::Pasturas => b.kind == Kind::Estancia,
                Layer::Piedra | Layer::Arena => b.kind == Kind::Cantera,
                Layer::Arcilla => b.kind == Kind::Horno,
                Layer::Cal => b.kind == Kind::Calera,
                Layer::Valor => false,
            };
            if show {
                let r = match layer {
                    Layer::Fuego => b.info().fire_reach,
                    Layer::Molestias => super::recursos::molestias::perfil_de_servicio(b.kind).map_or(0.0, |p| p.radio),
                    Layer::Cultivo | Layer::Pasturas | Layer::Piedra | Layer::Arcilla | Layer::Arena | Layer::Cal => 0.0,
                    _ => b.info().radius,
                };
                if r > 0.0 {
                    circle(out, t, b.c, r, if layer == Layer::Molestias { [0.6, 0.25, 0.7, 0.75] } else { [0.35, 0.65, 1.0, 0.7] });
                } else {
                    outline(out, t, &b.corners(), 1.6, [1.0, 1.0, 1.0, 0.85]);
                }
            }
            // Pozos y aljibes sucios (pozos negros y basura alrededor).
            if layer == Layer::Saneamiento && matches!(b.kind, Kind::Pozo | Kind::Aljibe) {
                let x = b.contaminacion;
                circle(out, t, b.c, super::services::barrio::RADIO_POZO_NEGRO, [0.4 + x * 0.6, 0.75 - x * 0.5, 0.95 - x * 0.7, 0.85]);
            }
        }
    }

    /// Capa de recursos o de molestias sobre el terreno: una grilla de 6 × 6 km (celdas de
    /// 50 m) alrededor del cursor (o de la plaza), con el color de la aptitud de cada celda; en
    /// la de molestias, además, el arroyo sucio aguas abajo.
    fn terrain_layer(&self, out: &mut Vec<OverlayVertex>, t: &TerrainGen, layer: Layer) {
        const MEDIO: f32 = 3000.0;
        const PASO: f32 = 50.0;
        let centro = self.hover.or(self.city.lots.plaza_center).unwrap_or(if t.is_real() { crate::terrain::FOUNDING_REAL } else { Vec2::splat(12000.0) });
        // La ventana salta de a 500 m: no se rehace con cada movimiento del ratón.
        let cx = (centro.x / 500.0).round() as i32;
        let cz = (centro.y / 500.0).round() as i32;
        let firma = if layer == Layer::Molestias { self.city.month.wrapping_mul(7919).wrapping_add(self.city.suelo.campo.fuentes().len() as u32) } else { 0 };
        let clave = (layer as u8, cx, cz, firma, self.city.services.buildings.len());
        let mut cache = self.capa_terreno.lock().unwrap();
        if cache.as_ref().is_none_or(|c| c.0 != clave) {
            let rec = super::recursos::Recursos::de(t);
            let o = Vec2::new(cx as f32 * 500.0 - MEDIO, cz as f32 * 500.0 - MEDIO);
            let n = (2.0 * MEDIO / PASO) as usize;
            let alto: Vec<f32> = (0..=n)
                .flat_map(|j| (0..=n).map(move |i| (i, j)))
                .map(|(i, j)| {
                    let p = o + Vec2::new(i as f32, j as f32) * PASO;
                    t.height(p.x, p.y).max(0.0) + 2.5
                })
                .collect();
            let mut v = Vec::new();
            for j in 0..n {
                for i in 0..n {
                    let p = o + Vec2::new(i as f32 + 0.5, j as f32 + 0.5) * PASO;
                    if t.height(p.x, p.y) <= 0.2 {
                        continue;
                    }
                    let col = match (layer.uso(), layer.grupo_mat()) {
                        (Some(u), _) => uso_color(u, rec.aptitud(u, p.x, p.y)),
                        (None, Some(g)) => material_color(g, &rec.en_punto(p)),
                        (None, None) => {
                            let m = self.city.suelo.campo.en(p, None);
                            let x = m.total();
                            if x < 0.03 {
                                continue;
                            }
                            [0.55 + 0.35 * m.olor.max(m.humo), 0.25, 0.75 - 0.4 * m.humo, 0.15 + 0.5 * x]
                        }
                    };
                    if col[3] <= 0.0 {
                        continue;
                    }
                    let y = |a: usize, b: usize| alto[a + b * (n + 1)];
                    let q = |a: usize, b: usize| OverlayVertex { pos: [o.x + a as f32 * PASO, y(a, b), o.y + b as f32 * PASO], color: col };
                    v.extend_from_slice(&[q(i, j), q(i + 1, j), q(i + 1, j + 1), q(i, j), q(i + 1, j + 1), q(i, j + 1)]);
                }
            }
            if layer == Layer::Molestias {
                for &(p, x) in &self.city.suelo.campo.agua_sucia {
                    let pts: Vec<Vec2> = (0..=6).map(|k| p + Vec2::from_angle(k as f32 / 6.0 * std::f32::consts::TAU) * 9.0).collect();
                    ribbon(&mut v, t, &pts, 9.0, [0.45, 0.3, 0.1, 0.35 + 0.6 * x]);
                }
            }
            *cache = Some((clave, v));
        }
        out.extend_from_slice(&cache.as_ref().unwrap().1);
    }

    /// Guías de un trazado enganchado: líneas finas punteadas (la dirección y la alineación).
    fn guides_preview(&self, out: &mut Vec<OverlayVertex>, t: &TerrainGen, d: &Draft) {
        for &(a, b) in &d.guides {
            let len = a.distance(b);
            let n = (len / 4.0) as usize;
            for k in (0..n).step_by(2) {
                let p0 = a.lerp(b, k as f32 / n as f32);
                let p1 = a.lerp(b, ((k + 1) as f32 / n as f32).min(1.0));
                ribbon(out, t, &[p0, p1], 0.8, [1.0, 1.0, 1.0, 0.75]);
            }
        }
    }

    fn road_preview(&self, out: &mut Vec<OverlayVertex>, t: &TerrainGen, pts: &[Vec2]) {
        let good = self.city.road_problem(t, pts, self.kind).is_none() && City::road_cost(t, pts, self.kind) <= self.city.money;
        ribbon(out, t, pts, self.kind.width(), if good { [0.45, 0.95, 0.45, 0.55] } else { [1.0, 0.35, 0.3, 0.55] });
    }
}

/// Color de la aptitud de un solar para una zona: rojo (mala), amarillo, verde (buena).
fn aptitud_color(a: f32) -> [f32; 4] {
    let a = a.clamp(0.0, 1.0);
    if a < 0.5 {
        [0.9, 0.35 + a * 1.0, 0.25, 0.42]
    } else {
        [0.9 - (a - 0.5) * 1.3, 0.85, 0.3, 0.42]
    }
}

/// Circunferencia (alcance de un servicio) sobre el terreno.
fn circle(out: &mut Vec<OverlayVertex>, t: &TerrainGen, c: Vec2, r: f32, color: [f32; 4]) {
    // Anillo continuo (cuadriláteros entre el radio de adentro y el de afuera): con una cinta
    // por tramo, en los círculos chicos las puntas se abrían en abanico.
    let n = ((r / 3.0) as usize).clamp(32, 200);
    let w = (r * 0.25).min(1.0);
    let (ri, ro) = (r - w, r + w);
    for i in 0..n {
        let a0 = Vec2::from_angle(i as f32 / n as f32 * std::f32::consts::TAU);
        let a1 = Vec2::from_angle((i + 1) as f32 / n as f32 * std::f32::consts::TAU);
        quad(out, t, &[c + a0 * ri, c + a0 * ro, c + a1 * ro, c + a1 * ri], 1.2, color);
    }
}

fn y_at(t: &TerrainGen, p: Vec2, lift: f32) -> f32 {
    t.height(p.x, p.y).max(0.0) + lift
}

/// Líneas del suelo de un punto para la ficha: grupo CONEAT (o estimado), productividad,
/// aptitud general y para cultivos de verano, geología y, si `todo`, los materiales.
pub fn lineas_terreno(t: &TerrainGen, p: Vec2, todo: bool) -> Vec<String> {
    let rec = super::recursos::Recursos::de(t);
    let r = rec.en_punto(p);
    let mut out = Vec::new();
    match rec.suelo(p) {
        Some(s) => {
            out.push(format!("Suelo CONEAT {}{} · IP {} (media del país 100)", s.grupo, if s.estimado { " (estimado: sin dato en la ciudad actual)" } else { "" }, s.ip.round() as i32));
            out.push(format!("Aptitud: {} · cultivos de verano: {}{}", s.aptitud.replace("Tierras ", "").to_lowercase(), s.cultivo_verano, if s.limitantes.is_empty() { String::new() } else { format!(" — {}", s.limitantes.to_lowercase()) }));
            out.push(format!("Geología: {}", s.geologia));
        }
        None if t.height(p.x, p.y) > 0.2 => out.push(format!("Suelo (estimado del relieve): IP {}", r.ip.round() as i32)),
        None => {}
    }
    if todo || t.is_real() {
        out.push(format!("Para chacra: {} · para estancia: {}", super::recursos::calidad(r.cultivo), super::recursos::calidad(r.pastoreo)));
    }
    if todo {
        out.extend(lineas_materiales(&r));
    }
    out
}

/// Los materiales de un punto para la ficha del terreno: los que hay (ley ≥ 0,15), del de más
/// ley al de menos, con su calidad y para qué sirven; y lo que no hay y se trae de afuera.
pub fn lineas_materiales(r: &super::recursos::RecursosLote) -> Vec<String> {
    use super::recursos::materiales::Material;
    let mut hay: Vec<(Material, f32)> = Material::TODOS.into_iter().map(|m| (m, r.mat[m.i()])).filter(|x| x.1 >= super::recursos::MINIMO_MATERIAL).collect();
    hay.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut out = Vec::new();
    if hay.is_empty() {
        out.push("Materiales: nada que sacar".into());
    } else {
        let txt: Vec<String> = hay.iter().map(|(m, x)| format!("{} {} ({} %)", m.nombre(), super::recursos::calidad(*x), (x * 100.0).round() as i32)).collect();
        out.push(format!("Materiales: {}", txt.join(" · ")));
        let usos: Vec<String> = hay.iter().take(3).map(|(m, _)| format!("{}: {}", m.nombre(), m.entrada().usos.first().cloned().unwrap_or_default())).collect();
        out.push(format!("Sirven para: {}", usos.join(" · ")));
    }
    if r.mat[Material::Caliza.i()] < super::recursos::MINIMO_MATERIAL {
        out.push("Caliza: no hay en la zona (la cal se trae de Colonia o de Minas)".into());
    }
    out
}

/// Color de una celda en una capa de materiales: el del material de más ley del grupo, más
/// intenso cuanto más ley (alfa 0 = no se dibuja).
pub fn material_color(g: super::recursos::materiales::GrupoMat, r: &super::recursos::RecursosLote) -> [f32; 4] {
    let mut mejor = (g.materiales()[0], 0.0f32);
    for &m in g.materiales() {
        let x = r.mat[m.i()];
        if x > mejor.1 + 1e-6 {
            mejor = (m, x);
        }
    }
    if mejor.1 < 0.08 {
        return [0.0; 4];
    }
    let c = mejor.0.color();
    [c[0], c[1], c[2], 0.3 + 0.6 * mejor.1.clamp(0.0, 1.0)]
}

/// Color del valor del suelo (0 a 1): azul pálido, verde, amarillo, naranja, rojo.
pub fn valor_color(x: f32) -> [f32; 4] {
    let x = x.clamp(0.0, 1.0);
    let stops = [(0.0, [0.55, 0.7, 0.95]), (0.3, [0.45, 0.85, 0.45]), (0.55, [1.0, 0.9, 0.3]), (0.75, [1.0, 0.55, 0.15]), (1.0, [0.85, 0.1, 0.1])];
    for w in stops.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if x <= b {
            let k = (x - a) / (b - a);
            return [ca[0] + (cb[0] - ca[0]) * k, ca[1] + (cb[1] - ca[1]) * k, ca[2] + (cb[2] - ca[2]) * k, 0.62];
        }
    }
    [0.85, 0.1, 0.1, 0.62]
}

/// Color de la aptitud de una celda para un uso (alfa 0 = no se dibuja).
pub fn uso_color(u: super::recursos::Uso, x: f32) -> [f32; 4] {
    use super::recursos::Uso;
    let x = x.clamp(0.0, 1.0);
    match u {
        // Tierra: de pardo (mala) a verde oscuro (excelente).
        Uso::Cultivo => [0.75 - 0.6 * x, 0.5 + 0.12 * x, 0.25 - 0.1 * x, 0.42],
        Uso::Pastoreo => [0.85 - 0.55 * x, 0.75 + 0.05 * x, 0.35 + 0.1 * x, 0.42],
        // Materiales: solo donde hay; más intenso cuanto más.
        Uso::Piedra if x >= 0.08 => [0.42, 0.42, 0.5, 0.15 + 0.6 * x],
        Uso::Arcilla if x >= 0.08 => [0.8, 0.33, 0.12, 0.1 + 0.55 * x],
        Uso::Arena if x >= 0.08 => [0.98, 0.85, 0.35, 0.12 + 0.6 * x],
        Uso::Grava if x >= 0.08 => [0.35, 0.5, 0.85, 0.15 + 0.6 * x],
        _ => [0.0, 0.0, 0.0, 0.0],
    }
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

    /// Enganches del trazado: un cursor a pocos grados de la perpendicular da una calle a 90°
    /// exactos del damero; el largo se alinea con la esquina de otra calle; con Shift, libre.
    #[test]
    fn las_calles_se_enganchan_a_90_grados_y_se_alinean() {
        let t = TerrainGen::new(1726);
        let mut g = Game::default();
        let c = Vec2::new(13500.0, 16300.0);
        g.set_tool(1);
        assert!(g.press(&t, c).is_some(), "{}", g.message);
        g.city.money = 1_000_000;
        let corner = c + Vec2::new(BLOCK * 0.5, -BLOCK * 0.5);
        // Hacia el norte, con el cursor torcido 6°.
        let cur = corner + Vec2::from_angle((-90.0f32 + 6.0).to_radians()) * 150.0;
        let d = g.draft(corner, cur);
        let v = (d.end - corner).normalize();
        assert!(v.x.abs() < 1e-3 && v.y < 0.0, "calle a 90°: {v:?}");
        assert!(d.locked.is_some());
        // Torcido 20°: fuera del enganche, sigue al cursor.
        let cur2 = corner + Vec2::from_angle((-90.0f32 + 20.0).to_radians()) * 150.0;
        assert!((g.draft(corner, cur2).end - cur2).length() < 1.0);
        // Con Shift, libre aunque esté cerca de 90°.
        g.free_draw = true;
        assert!((g.draft(corner, cur).end - cur).length() < 1.0);
        g.free_draw = false;
        // Alineación: desde la otra esquina de la plaza, el largo engancha con el final de la
        // calle anterior (cuadras parejas).
        g.set_tool(1);
        g.press(&t, corner);
        let first_end = g.draft(corner, corner + Vec2::new(0.0, -131.0)).end;
        assert!(g.press(&t, corner + Vec2::new(0.0, -131.0)).is_some(), "{}", g.message);
        g.cancel();
        let other = c + Vec2::new(-BLOCK * 0.5, -BLOCK * 0.5);
        let d = g.draft(other, other + Vec2::new(1.5, -(first_end.distance(corner) + 2.5)));
        assert!((d.end.y - first_end.y).abs() < 0.01, "alineada con la esquina: {} vs {}", d.end.y, first_end.y);
        assert_eq!(d.locked, Some("alineada"));
    }

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
