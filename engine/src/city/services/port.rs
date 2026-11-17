//! El puerto colonial: muelle de madera con su barraca de cueros, barcos que llegan con
//! pobladores y mercaderías y se van con cueros, y los acontecimientos lejanos (ficticios, con
//! guiños a la historia) que cambian esos flujos. La aduana cobra lo que entra y sale; sin
//! ella crece el contrabando. Los barcos pueden traer fiebre (ver [`super::epidemics`]).
//! Porte de `scripts/city/port.gd`.

use super::super::economy::MonthInput;
use super::super::lots::{corners_of, Obstacle};
use super::super::voxelize::{build_rotated, build_rotated_at};
use super::super::City;
use super::Kind;
use crate::buildings::Edits;
use crate::palette::AIR;
use crate::structures::colonial;
use crate::terrain::TerrainGen;
use glam::{IVec3, Vec2};

pub const PIER_COST: i64 = 4000;
pub const PIER_WOOD_BRICKS: f32 = 80.0;
pub const PIER_UPKEEP: i64 = 40;
/// Parte del comercio que se va por el contrabando sin aduana y con ella.
pub const CONTRABAND_NO_CUSTOMS: f32 = 0.35;
pub const CONTRABAND_CUSTOMS: f32 = 0.05;

/// Un acontecimiento lejano: meses que dura y cuánto cambia los barcos, el precio de los
/// cueros y la gente que viene.
pub struct Event {
    pub name: &'static str,
    pub months: u32,
    pub ships: f32,
    pub hides: f32,
    pub people: f32,
    pub news: &'static str,
}

/// Acontecimientos lejanos de la época colonial. Todos ficticios (GDD 4.11).
pub const EVENTS: [Event; 6] = [
    Event { name: "Guerra entre las Coronas del Norte", months: 8, ships: 0.5, hides: 1.0, people: 0.7, news: "Gaceta: estalla la guerra entre las Coronas del Norte. Llegarán menos barcos y florecerá el contrabando." },
    Event { name: "Reglamento de Comercio Libre", months: 12, ships: 1.8, hides: 1.1, people: 1.2, news: "Gaceta: la Corona publica un reglamento de comercio libre. Se esperan muchos más barcos." },
    Event { name: "Fiebre del cuero en ultramar", months: 10, ships: 1.2, hides: 1.8, people: 1.0, news: "Gaceta: en ultramar pagan los cueros a precio de oro. Las estancias se apuran a carnear." },
    Event { name: "Barco de pobladores de las Islas", months: 1, ships: 1.0, hides: 1.0, people: 3.0, news: "Gaceta: zarpa un barco con muchas familias de las Islas rumbo a la ciudad." },
    Event { name: "Bloqueo del río", months: 4, ships: 0.0, hides: 1.0, people: 0.0, news: "Gaceta: una escuadra extranjera bloquea el río. No entrarán ni saldrán barcos por un tiempo." },
    Event { name: "Fiebre en los barcos", months: 3, ships: 0.8, hides: 1.0, people: 0.6, news: "Gaceta: llegan noticias de fiebres en los puertos de ultramar. Los recién llegados harán cuarentena." },
];

/// El muelle construido.
#[derive(Clone, Debug)]
pub struct Pier {
    /// Donde arranca en tierra (la barraca queda detrás) y hacia dónde entra al río.
    pub land: Vec2,
    pub dir: Vec2,
    pub len: f32,
    /// ¿Tiene barraca de cueros? (si había lugar en tierra).
    pub barraca: bool,
    pub voxels: Vec<IVec3>,
}

impl Pier {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn end(&self) -> Vec2 {
        self.land + self.dir * self.len
    }
    /// Centro y frente de la barraca, detrás del arranque del muelle.
    pub fn barraca_site(&self) -> (Vec2, Vec2) {
        (self.land - self.dir * 14.0, self.dir)
    }
}

#[derive(Clone, Debug)]
pub struct Port {
    pub pier: Option<Pier>,
    /// Acontecimiento lejano en curso (índice en `EVENTS`) y meses que le quedan.
    pub event: Option<(usize, u32)>,
    /// Meses hasta el próximo barco.
    pub ship_timer: f32,
    /// Barcos que llegaron este mes (para el comercio de la economía).
    pub ships_month: i32,
    /// Totales del año: barcos, pobladores y barcos con fiebre.
    pub year: [i32; 3],
}

impl Default for Port {
    fn default() -> Self {
        Port { pier: None, event: None, ship_timer: 2.0, ships_month: 0, year: [0; 3] }
    }
}

impl Port {
    pub fn upkeep(&self) -> i64 {
        if self.pier.is_some() { PIER_UPKEEP } else { 0 }
    }
    /// Multiplicador del acontecimiento lejano en curso.
    pub fn mult(&self, f: fn(&Event) -> f32) -> f32 {
        self.event.map_or(1.0, |(i, _)| f(&EVENTS[i]))
    }
    pub fn event_name(&self) -> Option<&'static str> {
        self.event.map(|(i, _)| EVENTS[i].name)
    }
}

/// Hacia dónde está el agua honda (ver `PIER_DEPTH`) desde `p`: (dirección, distancia).
/// Hondura a la que llega el muelle y hasta dónde se busca. El Río de la Plata frente a
/// Montevideo es muy playo (en el MDT: −0,8 m a 100 m de la orilla, −1,5 m a 300–400 m): los
/// barcos fondeaban afuera y las lanchas descargaban en muelles largos. Un metro de agua alcanza
/// para las lanchas.
pub const PIER_DEPTH: f32 = -0.9;
pub const SEA_SEARCH: f32 = 360.0;
/// Largo máximo del muelle (m).
pub const PIER_MAX_LEN: f32 = 380.0;

pub fn find_sea(t: &TerrainGen, p: Vec2) -> Option<(Vec2, f32)> {
    let mut best: Option<(Vec2, f32)> = None;
    for k in 0..32 {
        let d = Vec2::from_angle(k as f32 / 32.0 * std::f32::consts::TAU);
        let mut dist = 10.0;
        while dist < SEA_SEARCH {
            let q = p + d * dist;
            if t.height(q.x, q.y) < PIER_DEPTH {
                if best.is_none_or(|b| dist < b.1) {
                    best = Some((d, dist));
                }
                break;
            }
            dist += 5.0;
        }
    }
    best
}

/// Arranque del muelle cerca del clic: el punto de la orilla (tierra baja con agua honda
/// enfrente) más cercano, a menos de 120 m. Así no hace falta apuntar justo a la línea de costa.
pub fn pier_start(t: &TerrainGen, click: Vec2) -> Option<Vec2> {
    let ok = |q: Vec2| {
        let h = t.height(q.x, q.y);
        (0.3..=PIER_MAX_HEIGHT).contains(&h) && !t.is_water(q.x, q.y) && find_sea(t, q).is_some()
    };
    if ok(click) {
        return Some(click);
    }
    let mut r = 4.0;
    while r <= 120.0 {
        let n = ((r * 0.8) as usize).clamp(8, 96);
        let mut best: Option<(f32, Vec2)> = None;
        for k in 0..n {
            let q = click + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r;
            if ok(q) {
                // Entre los del mismo anillo, el de agua honda más cerca (muelle más corto).
                let d = find_sea(t, q).map_or(f32::MAX, |x| x.1);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, q));
                }
            }
        }
        if let Some((_, q)) = best {
            return Some(q);
        }
        r += 4.0;
    }
    None
}

/// Alto máximo de la orilla donde arranca el muelle (con una rampa hasta el tablero).
pub const PIER_MAX_HEIGHT: f32 = 8.0;

impl City {
    /// `None` si el muelle se puede construir arrancando en `p`; si no, el motivo.
    pub fn pier_problem(&self, t: &TerrainGen, p: Vec2) -> Option<String> {
        if self.plaza.is_none() {
            return Some("Primero funda la plaza mayor".into());
        }
        if self.services.port.pier.is_some() {
            return Some("La ciudad ya tiene su muelle".into());
        }
        let Some(p) = pier_start(t, p) else {
            return Some(if t.is_water(p.x, p.y) || t.height(p.x, p.y) < 0.3 {
                "Acercá el cursor a la orilla: el muelle arranca en tierra, junto al agua honda".into()
            } else {
                "Lejos de una orilla con agua honda (más de 120 m), o la barranca es muy alta".into()
            });
        };
        if self.net.clearance(p, 200.0) > 150.0 {
            return Some("Lejos de las calles: traza un camino hasta la costa (para las carretas)".into());
        }
        if PIER_COST > self.money {
            return Some(format!("Fondos insuficientes: hacen falta {PIER_COST} pesos"));
        }
        None
    }

    /// Construye el muelle (y la barraca, si hay lugar en tierra).
    pub fn build_pier(&mut self, t: &TerrainGen, p: Vec2) -> Result<Edits, String> {
        if let Some(m) = self.pier_problem(t, p) {
            return Err(m);
        }
        let p = pier_start(t, p).unwrap();
        let (dir, dist) = find_sea(t, p).unwrap();
        self.money -= PIER_COST;
        // La barraca de ladrillo, solo si hay ladrillos (el muelle se hace igual, de madera).
        let ladrillos = self.economy.take(super::super::economy::Res::Ladrillos, PIER_WOOD_BRICKS);
        let mut pier = Pier { land: p, dir, len: (dist + 25.0).clamp(40.0, PIER_MAX_LEN), barraca: false, voxels: Vec::new() };
        let mut e = Edits::default();
        let (bc, bf) = pier.barraca_site();
        let poly = corners_of(bc, bf, 18.0, 12.0);
        let free = ladrillos && poly.iter().all(|q| t.height(q.x, q.y) > 0.5 && self.net.clearance(*q, 30.0) > 0.5) && !self.lots.obstacles.iter().any(|o| super::super::lots::polys_overlap(&poly, &corners_of(o.c, o.f, o.w, o.d)));
        if free {
            self.clear_lots_under(&poly, &mut e);
            self.lots.obstacles.push(Obstacle { c: bc, f: bf, w: 18.0, d: 12.0 });
            pier.barraca = true;
        }
        self.services.port.pier = Some(pier);
        self.port_rebuild(t, &mut e);
        Ok(e)
    }

    /// Vóxeles del muelle y la barraca.
    pub(crate) fn port_rebuild(&mut self, t: &TerrainGen, e: &mut Edits) {
        let Some(pier) = self.services.port.pier.as_mut() else { return };
        for &v in &pier.voxels {
            e.dset(v, AIR);
        }
        let center = pier.land + pier.dir * (pier.len * 0.5);
        let depth = (pier.len / 0.5) as i32;
        let b = build_rotated_at(t, center, -pier.dir, 20, depth, 1726, Some(0), &colonial::muelle_puerto);
        e.extend(b.edits);
        pier.voxels = b.voxels;
        if pier.barraca {
            let (bc, bf) = pier.barraca_site();
            let b = build_rotated(t, bc, bf, 36, 24, 1727, &colonial::barraca);
            e.extend(b.edits);
            pier.voxels.extend(b.voxels);
        }
    }

    /// El mes del puerto: acontecimientos lejanos y barcos (con pobladores, y a veces fiebre).
    pub(crate) fn port_month(&mut self) -> Vec<String> {
        let mut news = Vec::new();
        let port = &mut self.services.port;
        let rng = &mut self.services.rng;
        if self.month % 12 == 0 {
            port.year = [0; 3];
        }
        match port.event {
            Some((i, m)) if m <= 1 => {
                news.push(format!("Gaceta: termina «{}».", EVENTS[i].name));
                port.event = None;
            }
            Some((i, m)) => port.event = Some((i, m - 1)),
            None if rng.f() < 0.05 => {
                let i = (rng.next() % EVENTS.len() as u64) as usize;
                port.event = Some((i, EVENTS[i].months));
                news.push(EVENTS[i].news.into());
            }
            None => {}
        }
        port.ships_month = 0;
        let ships = port.mult(|e| e.ships) * self.economy.ship_factor();
        port.ship_timer -= ships;
        if port.ship_timer > 0.0 || ships <= 0.0 {
            return news;
        }
        port.ship_timer = 3.0 + rng.f() * 1.5;
        port.ships_month = 1;
        port.year[0] += 1;
        let pop = self.lots.population();
        let mut people = ((20 + rng.range(0, 25)) as f32 + pop as f32 * 0.02) as f32 * port.mult(|e| e.people);
        let landing = match &port.pier {
            Some(p) => p.land,
            None => {
                people *= 0.6; // sin muelle desembarcan como pueden en la playa
                self.lots.plaza_center.unwrap_or_default()
            }
        };
        let fever = if port.event.is_some_and(|(i, _)| EVENTS[i].name == "Fiebre en los barcos") { 0.5 } else { 0.06 };
        let sick = rng.f() < fever;
        let has_pier = port.pier.is_some();
        let mut people = people as i32;
        if sick {
            port.year[2] += 1;
            let (now, msg) = self.sick_ship(people, landing);
            news.push(msg);
            people = now;
        }
        self.services.port.year[1] += people;
        if self.poblacion.fundada {
            self.llegados_barco += people;
        } else {
            self.pool = (self.pool + people).min(600);
        }
        if people > 0 {
            news.push(if has_pier { format!("Llega un barco al muelle con {people} pobladores") } else { format!("Un barco desembarca {people} pobladores en la playa (no hay muelle)") });
        }
        news
    }

    /// Lo que el puerto le pasa a la economía: muelle, barcos, cueros, bloqueo y contrabando.
    pub(crate) fn port_input(&self, inp: &mut MonthInput) {
        let port = &self.services.port;
        inp.port = port.pier.is_some();
        inp.ships = port.ships_month;
        inp.world_hides = port.mult(|e| e.hides);
        inp.blockade = port.mult(|e| e.ships) <= 0.0;
        inp.contraband = if self.services.count(Kind::Aduana) > 0 { CONTRABAND_CUSTOMS } else { CONTRABAND_NO_CUSTOMS };
        if port.pier.as_ref().is_some_and(|p| p.barraca) {
            inp.storage[super::super::economy::Res::Cueros as usize] += 1500.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En la costa real de Montevideo: el muelle se construye en la orilla de la bahía y el
    /// comercio y la aduana pasan por él.
    #[test]
    fn muelle_en_la_costa_real_y_aduana() {
        let t = TerrainGen::montevideo();
        let mut city = City::default();
        city.money = 1_000_000;
        let c = t.founding_site();
        city.found_plaza(&t, c, 0.0).unwrap();
        // Busca una orilla baja cerca de la plaza con agua profunda y le lleva un camino corto
        // desde tierra adentro.
        let mut site = None;
        let mut tried = 0;
        'b: for r in 1..120 {
            for a in 0..48 {
                let p = c + Vec2::from_angle(a as f32 / 48.0 * std::f32::consts::TAU) * (r as f32 * 15.0);
                let h = t.height(p.x, p.y);
                let Some((sea, _)) = (if (0.4..3.5).contains(&h) && !t.is_water(p.x, p.y) { find_sea(&t, p) } else { None }) else { continue };
                tried += 1;
                for len in [40.0, 70.0, 110.0] {
                    for turn in [0.0f32, 0.5, -0.5, 1.0, -1.0] {
                        let back = -sea.rotate(Vec2::from_angle(turn));
                        let line = super::super::super::roads::line(p + back * 8.0, p + back * len);
                        if city.build_road(&t, &line, super::super::super::roads::RoadKind::Dirt).is_ok() && city.pier_problem(&t, p).is_none() {
                            site = Some(p);
                            break 'b;
                        }
                    }
                }
            }
        }
        println!("orillas probadas: {tried}");
        let p = site.expect("una orilla para el muelle");
        let e = city.build_pier(&t, p).unwrap();
        assert!(e.design.len() > 200, "el muelle tiene vóxeles: {}", e.design.len());
        let pier = city.services.port.pier.clone().unwrap();
        assert!(t.height(pier.end().x, pier.end().y) < -1.0, "la punta del muelle está en el agua");
        assert!(city.pier_problem(&t, p).is_some(), "un solo muelle");
        let mut inp = MonthInput::default();
        city.port_input(&mut inp);
        assert!(inp.port && inp.contraband > 0.3);
        // Llegan barcos en un año.
        let mut ships = 0;
        for _ in 0..12 {
            city.port_month();
            ships += city.services.port.ships_month;
        }
        assert!(ships >= 2, "{ships} barcos en un año");
    }
}

#[cfg(test)]
mod pier_real_tests {
    use super::*;
    /// En el mapa real, un clic cerca de la costa (a menos de 60 m del agua) encuentra dónde
    /// arrancar el muelle casi siempre.
    #[test]
    fn el_muelle_se_engancha_a_la_costa_real() {
        let t = TerrainGen::montevideo();
        let (mut costa, mut ok) = (0, 0);
        for j in 0..120 {
            for i in 0..120 {
                let p = Vec2::new(i as f32 * 200.0 + 100.0, j as f32 * 200.0 + 100.0);
                let sd = t.sea_distance(p.x, p.y);
                if t.height(p.x, p.y) > 0.3 && sd > 0.0 && sd < 60.0 {
                    costa += 1;
                    ok += pier_start(&t, p).is_some() as i32;
                }
            }
        }
        eprintln!("muelle: {ok} de {costa} puntos de costa");
        assert!(costa > 20 && ok as f32 >= costa as f32 * 0.7, "{ok} de {costa}");
    }
}
