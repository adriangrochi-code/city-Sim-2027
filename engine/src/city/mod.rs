//! El juego en el motor: la ciudad colonial (calles, solares, zonas y crecimiento) llevada a
//! vóxeles. Porte de la lógica del prototipo de Godot (`scripts/roads`, `scripts/city/lots.gd`
//! y la parte de herramientas de `scripts/main.gd`).
//!
//! Cada acción devuelve las ediciones de vóxeles que produce; la sesión las aplica al mundo
//! en streaming, que re-malla solo las columnas tocadas.

pub mod agentes;
pub mod climate;
pub mod desarrollo;
pub mod economy;
pub mod game;
pub mod game_demo;
pub mod lots;
pub mod materiales;
pub mod poblacion;
pub mod recursos;
pub mod roads;
pub mod save;
pub mod services;
pub mod voxelize;
pub mod water;

use crate::buildings::{Edits, Rng};
use crate::palette::AIR;
use crate::structures::colonial::{self, LOT_D, LOT_W};
use crate::terrain::TerrainGen;
use glam::Vec2;
use lots::{corners_of, Demand, LevelChange, Lot, Lots, Obstacle, ZoneKind};
use roads::{polyline_length, RoadKind, RoadNetwork};
use voxelize::{build_rotated, plaza_edits, road_edits, road_restore, PLAZA_SIZE};

/// Cuadra del damero entre ejes de calle: 100 varas castellanas (0,866 m), como en la traza
/// de Montevideo.
pub const BLOCK: f32 = 86.6;
/// Segundos de juego por mes (a velocidad 1).
pub const MONTH_SECONDS: f32 = 10.0;
/// Horas del reloj del día por segundo de juego (un día cada 8 minutos).
pub const HOURS_PER_SECOND: f32 = 24.0 / 480.0;

/// Inundación pendiente de evaluar.
#[derive(Clone, Copy, Debug)]
pub enum Flood {
    /// El río subió hasta este nivel (m).
    Rio(f32),
    /// Crecientes de los arroyos.
    Arroyos,
}
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
    /// Clima y estaciones (normales del INUMET) y el calendario del campo.
    pub climate: climate::Climate,
    /// Nivel del río: mareas, sudestadas, pampero y crecientes.
    pub water: water::WaterLevel,
    /// Vecinos y vehículos en la calle (ver `agentes`).
    pub agentes: agentes::Agentes,
    /// Servicios (agua, serenos, campo, puerto, aduana), incendios, epidemias y grandes obras.
    pub services: services::Services,
    /// Los habitantes uno por uno: familias, linajes, economía de los hogares y felicidad.
    pub poblacion: poblacion::Poblacion,
    /// Pobladores que trajeron los barcos (o salieron de la cuarentena) este mes y todavía no
    /// entraron a la población: se suman a las llegadas del mes.
    pub llegados_barco: i32,
    /// Saberes, tecnologías, industrias, inversión e incentivos, y el comercio exterior
    /// (ver `desarrollo`).
    pub desarrollo: desarrollo::Desarrollo,
    /// Recursos del suelo, molestias de la industria y valor del suelo de cada solar
    /// (derivado; ver `recursos`).
    pub suelo: recursos::valor::SueloCiudad,
    /// Materiales de construcción: existencias, demanda, importación y con qué se hizo cada
    /// servicio (las casas lo guardan en su solar; ver `materiales`).
    pub materiales: materiales::ciudad::Materiales,
    /// Mejoras de casas que pidieron sus dueños este mes (se aplican en el mismo `tick`).
    mejoras: Vec<(u32, u8)>,
    rng: Rng,
    pending_flood: Option<Flood>,
    month_acc: f32,
    growth_acc: f32,
}

impl Default for City {
    fn default() -> Self {
        City { net: RoadNetwork::default(), lots: Lots::default(), money: 20000, pool: 0, month: 2, plaza: None, economy: economy::Economy::default(), climate: climate::Climate::default(), water: water::WaterLevel::default(), agentes: agentes::Agentes::default(), services: services::Services::default(), poblacion: poblacion::Poblacion::nueva(1726), suelo: Default::default(), desarrollo: desarrollo::Desarrollo::default(), materiales: Default::default(), mejoras: Vec::new(), llegados_barco: 0, rng: Rng::new(1726), pending_flood: None, month_acc: 0.0, growth_acc: 0.0 }
    }
}

/// Dirección del frente de la plaza para un ángulo (como en el prototipo).
pub fn plaza_frame(angle: f32) -> (Vec2, Vec2) {
    let f = Vec2::new(angle.sin(), angle.cos());
    (f, lots::side_of(f))
}

type Builder = &'static dyn Fn(&TerrainGen, &mut Edits, &crate::structures::Frame);

fn pulperia_l(t: &TerrainGen, e: &mut Edits, f: &crate::structures::Frame) {
    colonial::pulperia(t, e, f, true)
}

fn pulperia_r(t: &TerrainGen, e: &mut Edits, f: &crate::structures::Frame) {
    colonial::pulperia(t, e, f, false)
}

/// Constructor del catálogo de vóxeles para cada tipo de zona, densidad y nivel. Cada solar
/// elige un estilo con su semilla, así la cuadra no se ve repetida; la casa de una planta
/// sigue su techo real si lo tiene (azotea plana o techo a dos aguas; ver `materiales`).
fn builder_for(kind: ZoneKind, density: lots::Density, level: u8, seed: u64, techo: Option<materiales::Techo>) -> Builder {
    use lots::Density::*;
    let pick = (seed >> 7) % 100;
    let teja = match techo {
        Some(materiales::Techo::Azotea) => false,
        Some(_) => true,
        None => pick < 45,
    };
    match (kind, level) {
        (ZoneKind::Residencial, 1) => &colonial::rancho,
        (ZoneKind::Residencial, 2) if density == Alta => &colonial::inquilinato,
        (ZoneKind::Residencial, 2) if teja => &colonial::casa_teja,
        (ZoneKind::Residencial, 2) => &colonial::casa_azotea,
        (ZoneKind::Residencial, _) if density == Alta => &colonial::altos_renta,
        (ZoneKind::Residencial, _) => &colonial::casa_altos,
        (ZoneKind::Comercial, 1) if seed % 2 == 0 => &pulperia_l,
        (ZoneKind::Comercial, 1) => &pulperia_r,
        (ZoneKind::Comercial, 2) if pick < 40 => &colonial::fonda,
        (ZoneKind::Comercial, 2) => &colonial::tienda,
        (ZoneKind::Comercial, _) if density == Alta || pick < 45 => &colonial::recova,
        (ZoneKind::Comercial, _) => &colonial::almacen,
        (ZoneKind::Industrial, 1) if pick < 45 => &colonial::panaderia,
        (ZoneKind::Industrial, 1) => &colonial::taller,
        (ZoneKind::Industrial, 2) if pick < 45 => &colonial::veleria,
        (ZoneKind::Industrial, 2) => &colonial::curtiembre,
        (ZoneKind::Industrial, _) if pick < 45 => &colonial::deposito,
        (ZoneKind::Industrial, _) => &colonial::barraca_solar,
    }
}

/// Edificio de un solar en vóxeles, con los materiales de su obra (si no tiene, los del
/// catálogo).
#[allow(clippy::too_many_arguments)]
fn build_lot(t: &TerrainGen, c: Vec2, f: Vec2, seed: u64, kind: ZoneKind, density: lots::Density, level: u8, obra: Option<materiales::Obra>) -> voxelize::Built {
    let techo = obra.map(|o| o.techo);
    let b = builder_for(kind, density, level, seed, techo);
    match obra {
        Some(o) => {
            let nom = materiales::ciudad::nominal(kind, density, level, seed, techo);
            build_rotated(t, c, f, LOT_W, LOT_D, seed, &|t: &TerrainGen, e: &mut Edits, fr: &crate::structures::Frame| {
                b(t, e, fr);
                materiales::ciudad::remapear(e, &o, &nom, seed);
            })
        }
        None => build_rotated(t, c, f, LOT_W, LOT_D, seed, b),
    }
}

fn clear_voxels(e: &mut Edits, lot: &Lot) {
    for &v in &lot.voxels {
        e.dset(v, AIR);
    }
}

/// Centro de la iglesia matriz: del lado de atrás de la plaza, cruzando la calle.
pub(crate) fn church_center(c: Vec2, f: Vec2) -> Vec2 {
    c - f * (BLOCK * 0.5 + RoadKind::Stone.width() * 0.5 + 0.6 + 21.0)
}

/// Vóxeles de la plaza mayor y de la iglesia matriz en la fase que se ve (0 = capilla de
/// adobe; ver `services::works`). Devuelve también los vóxeles de la iglesia.
fn plaza_church_edits(t: &TerrainGen, c: Vec2, f: Vec2, church_phase: u32) -> (Edits, Vec<glam::IVec3>) {
    let mut e = Edits::default();
    plaza_edits(t, c, f, &mut e);
    let church = services::works::church_built(t, church_center(c, f), f, church_phase);
    e.extend(church.edits);
    (e, church.voxels)
}

impl City {
    /// Centro y frente de la plaza mayor, si ya se fundó.
    fn plaza_square(&self) -> voxelize::PlazaSquare {
        self.plaza.map(|(c, angle)| (c, plaza_frame(angle).0))
    }

    /// Vuelve a generar todos los vóxeles de la ciudad a partir de su estado (al cargar una
    /// partida): plaza e iglesia, todas las calles y el edificio de cada solar construido.
    /// La voxelización es determinista (semillas de los solares), así que el resultado es el
    /// mismo mundo que se guardó. Llena los vóxeles de cada solar para poder reemplazarlo.
    pub fn rebuild_edits(&mut self, t: &TerrainGen) -> Edits {
        let mut e = match self.plaza {
            Some((c, angle)) => {
                let (e, church) = plaza_church_edits(t, c, plaza_frame(angle).0, self.services.church.view_phase());
                self.services.church.voxels = church;
                e
            }
            None => Edits::default(),
        };
        // Calles y edificios en paralelo (cada tramo y cada edificio depende solo de la red y
        // de su solar); se juntan en orden de id, así el resultado no depende de los hilos.
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1);
        let ids: Vec<u32> = self.net.edges.keys().copied().collect();
        let net = &self.net;
        let plaza = self.plaza_square();
        let per = ids.len().div_ceil(threads).max(1);
        let roads: Vec<Edits> = std::thread::scope(|sc| {
            let hs: Vec<_> = ids.chunks(per).map(|part| sc.spawn(move || part.iter().map(|&id| {
                let mut r = Edits::default();
                road_edits(t, net, id, plaza, &mut r);
                r
            }).collect::<Vec<_>>())).collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        for r in roads {
            e.extend(r);
        }
        type Job = (u32, Vec2, Vec2, u64, (ZoneKind, lots::Density), u8, Option<materiales::Obra>);
        let jobs: Vec<Job> = self.lots.lots.values().filter_map(|l| Some((l.id, l.c, l.f, l.seed, lots::zone_of(l.zone)?, l.level, l.obra))).filter(|j| j.5 > 0).collect();
        let per = jobs.len().div_ceil(threads).max(1);
        let built: Vec<(u32, voxelize::Built)> = std::thread::scope(|sc| {
            let hs: Vec<_> = jobs.chunks(per).map(|part| sc.spawn(move || part.iter().map(|&(id, c, f, seed, kind, level, obra)| (id, build_lot(t, c, f, seed, kind.0, kind.1, level, obra))).collect::<Vec<_>>())).collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        for l in self.lots.lots.values_mut() {
            l.voxels.clear();
        }
        for (id, b) in built {
            e.extend(b.edits);
            self.lots.lots.get_mut(&id).unwrap().voxels = b.voxels;
        }
        self.services_rebuild(t, &mut e);
        e
    }

    pub fn population(&self) -> i32 {
        self.lots.population()
    }

    /// Habitantes de verdad (personas de la población, incluidas la guarnición, las
    /// cuadrillas y quienes no tienen casa). Antes de fundar, lo que caben en las casas.
    pub fn habitantes(&self) -> i32 {
        if self.poblacion.fundada { self.poblacion.habitantes() as i32 } else { self.population() }
    }

    /// Los solares tal como los ve la población.
    pub fn lotes_info(&self) -> Vec<poblacion::LoteInfo> {
        self.lots
            .lots
            .values()
            .map(|l| poblacion::LoteInfo {
                id: l.id,
                tipo: l.kind(),
                nivel: l.level,
                capacidad: l.residents(),
                empleos: match l.kind() {
                    Some(ZoneKind::Comercial) => lots::JOBS_C[l.level as usize],
                    Some(ZoneKind::Industrial) => lots::JOBS_I[l.level as usize],
                    _ => 0,
                },
                // Las casas de buenos materiales valen más (prestigio; ver `materiales`).
                valor: (self.lots.land_value(l) * self.obra_de(l).map_or(1.0, |o| 0.85 + 0.3 * o.prestigio())).min(1.0),
                distancia: self.lots.plaza_center.map_or(0.0, |p| l.c.distance(p)),
            })
            .collect()
    }

    /// Lo que la ciudad le cuenta a la población este mes (también para la ficha de cada
    /// persona en la interfaz).
    pub fn entorno_poblacion(&self, llegadas: u32, agua: f32, epidemia: bool, sequia: bool) -> poblacion::Entorno {
        poblacion::Entorno {
            mes: self.month as i32,
            lotes: self.lotes_info(),
            precio_pan: self.economy.price(economy::Res::Pan),
            precio_carne: self.economy.price(economy::Res::Carne),
            agua,
            epidemia,
            sequia,
            llegadas,
            contribucion: self.economy.tax(economy::Tax::Contribucion),
            ordenanzas: self.economy.active.clone(),
            iglesia: self.plaza.is_some(),
            servicios: self.servicios_por_lote(),
            con_servicios: true,
        }
    }

    /// Escuela, salud, insalubridad y delito de cada solar, por id (ver `services::barrio`).
    pub fn servicios_por_lote(&self) -> Vec<poblacion::ServiciosLote> {
        let br = &self.services.barrio;
        let max = self.lots.lots.keys().next_back().map_or(0, |&k| k as usize + 1);
        let mut v = vec![poblacion::ServiciosLote::default(); max];
        for l in self.lots.lots.values().filter(|l| l.level > 0) {
            // Las molestias de la industria: el agua sucia del arroyo y el olor también enferman.
            let m = self.suelo.molestia_de(l.id);
            // La humedad de las paredes de barro y adobe también (ver `materiales`).
            let humedad = self.obra_de(l).map_or(0.0, |o| o.humedad());
            let insalubre = (br.insalubre_de(l.id) + 0.3 * m.agua + 0.1 * (m.olor + m.humo) + 0.25 * humedad).min(1.0);
            v[l.id as usize] = poblacion::ServiciosLote { escuela: br.escuela_de(l.id), salud: br.salud_de(l.id), insalubre, delito: br.delito_de(l.id), vigilancia: br.vigilancia_de(l.id), molestia: m.total() };
        }
        v
    }

    /// El entorno de la población como está ahora (sin llegadas), para la interfaz.
    pub fn entorno_actual(&self) -> poblacion::Entorno {
        let agua = (0.4 + self.climate.soil / climate::SUELO_MAX * 0.4).min(0.8);
        self.entorno_poblacion(0, agua, false, self.climate.last.drought)
    }

    /// Mejoras de casas pagadas por sus dueños: se construyen si el solar lo permite (calle,
    /// valor del suelo, densidad, antigüedad); la falta de pobladores no las frena.
    fn aplicar_mejoras(&mut self, t: &TerrainGen, e: &mut Edits) {
        let pedidas = std::mem::take(&mut self.mejoras);
        self.lots.sin_altos = !self.altos_posibles();
        let mut hechas = Vec::new();
        for (id, nivel) in pedidas {
            let Some(l) = self.lots.lots.get(&id) else { continue };
            if l.level + 1 != nivel || !lots::growth_blockers(&self.lots, l, &self.net, i32::MAX, self.demand()).is_empty() {
                continue;
            }
            let l = self.lots.lots.get_mut(&id).unwrap();
            l.level = nivel;
            l.age = 0;
            hechas.push(LevelChange { lot: id, level: nivel });
            self.poblacion.mejora_hecha(id, nivel);
        }
        if !hechas.is_empty() {
            // Las fotos de la población, al día con lo que pagaron los dueños.
            self.poblacion.stats = self.poblacion.estadisticas();
        }
        self.apply_levels(t, &hechas, e);
    }

    /// Trabajadores: algo menos de la mitad de los habitantes (el resto, niños y ancianos).
    pub fn workers(&self) -> i32 {
        (self.population() as f32 * 0.45) as i32
    }

    /// Demanda residencial, comercial e industrial (−1 a 1), como el medidor del género:
    /// - residencial: familias esperando casa y empleos sin cubrir;
    /// - comercial: una tienda cada tantos vecinos, más el comercio del puerto;
    /// - industrial: brazos sin trabajo y la venta de cueros y sebo al exterior.
    /// Los impuestos altos la frenan (alcabala al comercio, aduana a la industria).
    pub fn demand(&self) -> Demand {
        use economy::Tax;
        let pop = self.population() as f32;
        let workers = self.workers() as f32;
        let (jc, ji) = self.lots.jobs();
        let jobs = (jc + ji) as f32;
        let eco = &self.economy;
        let over = |t: Tax| (eco.tax(t) - t.normal()) as f32;
        let r = (self.pool as f32 + (jobs - workers).max(0.0) * 0.8 - (workers - jobs).max(0.0) * 0.3) / 60.0;
        let c_need = 6.0 + pop * 0.08;
        let c = (c_need - jc as f32) / c_need.max(20.0) - over(Tax::Alcabala) * 0.05;
        let i_need = 8.0 + (workers - jc as f32) * 0.8 + eco.stock(economy::Res::Cueros).price.max(1.0) * 5.0;
        let i = (i_need - ji as f32) / i_need.max(25.0) - over(Tax::Aduana) * 0.04;
        Demand { r: r.clamp(-1.0, 1.0), c: c.clamp(-1.0, 1.0), i: i.clamp(-1.0, 1.0) }
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
        // Donde cruza agua, la calle es un puente (hasta MAX_BRIDGE m en esta época).
        let spans = voxelize::water_spans(t, pts);
        if let Some(&(a, b)) = spans.iter().find(|&&(a, b)| b - a > voxelize::MAX_BRIDGE) {
            return Some(format!("El cruce de agua mide {} m: los puentes de esta época llegan a {} m", (b - a) as i32, voxelize::MAX_BRIDGE as i32));
        }
        // Pendiente de la calle: en tramos de 10 m y con la altura promediada en unos metros a
        // la redonda (la calzada se nivela con cortes y rellenos: una bocha de piedra o la
        // ondulación fina del suelo no la hacen "empinada"). Máximo 20 %.
        let ground = |p: Vec2| {
            let o = [Vec2::ZERO, Vec2::new(3.0, 0.0), Vec2::new(-3.0, 0.0), Vec2::new(0.0, 3.0), Vec2::new(0.0, -3.0)];
            o.iter().map(|d| t.height(p.x + d.x, p.y + d.y)).sum::<f32>() / o.len() as f32
        };
        let total = polyline_length(pts);
        // Punto a `d` metros a lo largo de la polilínea.
        let at = |d: f32| {
            let mut acc = 0.0;
            for w in pts.windows(2) {
                let l = w[0].distance(w[1]);
                if acc + l >= d {
                    return w[0].lerp(w[1], ((d - acc) / l.max(1e-6)).clamp(0.0, 1.0));
                }
                acc += l;
            }
            *pts.last().unwrap()
        };
        let step = 10.0f32.min(total);
        let mut d = 0.0;
        while d + step <= total + 0.01 {
            let (a, b) = (d, (d + step).min(total));
            let on_bridge = spans.iter().any(|&(s0, s1)| b >= s0 - 1.0 && a <= s1 + 1.0);
            if !on_bridge && (ground(at(b)) - ground(at(a))).abs() / (b - a).max(0.5) > 0.2 {
                return Some("Demasiado empinado".into());
            }
            d += step;
        }
        if self.lots.obstacles.iter().any(|o| {
            let cs = corners_of(o.c, o.f, o.w - 1.0, o.d - 1.0);
            pts.iter().any(|p| lots::point_in_poly(*p, &cs))
        }) {
            return Some("Atraviesa la plaza o la iglesia".into());
        }
        None
    }

    /// Costo de una calle: por metro según el tipo, más los puentes (mucho más caros).
    pub fn road_cost(t: &TerrainGen, pts: &[Vec2], kind: RoadKind) -> i64 {
        let bridge: f32 = voxelize::water_spans(t, pts).iter().map(|(a, b)| b - a).sum();
        let per_bridge_m = match kind {
            RoadKind::Dirt => 25.0,  // puente de madera
            RoadKind::Stone => 60.0, // puente de piedra con arcos
        };
        (polyline_length(pts) * kind.cost_per_m() + bridge * per_bridge_m) as i64
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
        self.lots.obstacles.push(Obstacle { c: church_center(c, f), f, w: 18.0, d: 44.0 });
        let (mut e, church) = plaza_church_edits(t, c, f, self.services.church.view_phase());
        self.services.church.voxels = church;
        let half = BLOCK * 0.5;
        let q = [c + (s + f) * half, c + (-s + f) * half, c + (-s - f) * half, c + (s - f) * half];
        let mut new = Vec::new();
        for i in 0..4 {
            new.extend(self.net.add_road(&roads::line(q[i], q[(i + 1) % 4]), RoadKind::Stone));
        }
        self.after_roads(t, &new, &mut e);
        self.pool = 40;
        self.poblacion.fundar(self.month as i32);
        Ok(e)
    }

    /// Construye una calle (recta o curva) ya muestreada.
    pub fn build_road(&mut self, t: &TerrainGen, pts: &[Vec2], kind: RoadKind) -> Result<Edits, String> {
        if let Some(p) = self.road_problem(t, pts, kind) {
            return Err(p);
        }
        let cost = Self::road_cost(t, pts, kind);
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
            cost += Self::road_cost(t, l, kind);
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
                road_edits(t, &self.net, eid, self.plaza_square(), e);
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
        road_restore(t, &edge, self.plaza_square(), &mut e);
        // Las calles vecinas se vuelven a pintar encima (cruces, banquinas compartidas).
        let neighbors: Vec<u32> = self.net.edges.iter().filter(|(_, x)| [x.a, x.b].iter().any(|n| self.net.nodes[n].distance(edge.pts[0]) < 1.0 || self.net.nodes[n].distance(*edge.pts.last().unwrap()) < 1.0)).map(|(&id, _)| id).collect();
        for id in neighbors {
            road_edits(t, &self.net, id, self.plaza_square(), &mut e);
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
            lot.obra = None;
        }
        (n, e)
    }

    /// Lleva a vóxeles los cambios de nivel: el edificio nuevo se levanta con los materiales
    /// que hay (los saca del depósito; ver `materiales`) y reemplaza al viejo en el solar.
    /// Si no alcanzan para unos altos, la casa se queda en una planta.
    pub fn apply_levels(&mut self, t: &TerrainGen, changes: &[LevelChange], e: &mut Edits) {
        for ch in changes {
            let Some(lot) = self.lots.lots.get(&ch.lot) else { continue };
            let Some(mut plan) = self.plan_de_solar(lot, ch.level) else { continue };
            if plan.pisos >= 2 && materiales::elegir(&plan, &self.vista_materiales()).pisos_max() < 2 {
                let l = self.lots.lots.get_mut(&ch.lot).unwrap();
                l.level = ch.level - 1;
                if l.obra.is_some() && !l.voxels.is_empty() {
                    continue;
                }
                let Some(p) = self.plan_de_solar(&self.lots.lots[&ch.lot], ch.level - 1) else { continue };
                plan = p;
            }
            let obra = self.construir_obra(&plan);
            self.lots.lots.get_mut(&ch.lot).unwrap().obra = Some(obra);
            self.revoxelizar(t, ch.lot, e);
        }
    }

    /// Vuelve a dibujar el edificio de un solar con su nivel y sus materiales.
    pub fn revoxelizar(&mut self, t: &TerrainGen, id: u32, e: &mut Edits) {
        let Some(lot) = self.lots.lots.get(&id) else { return };
        let Some((kind, density)) = lots::zone_of(lot.zone) else { return };
        if lot.level == 0 {
            return;
        }
        let b = build_lot(t, lot.c, lot.f, lot.seed, kind, density, lot.level, lot.obra);
        let new: std::collections::HashSet<glam::IVec3> = b.voxels.iter().copied().collect();
        for &v in &lot.voxels {
            if !new.contains(&v) {
                e.dset(v, AIR);
            }
        }
        e.extend(b.edits);
        self.lots.lots.get_mut(&id).unwrap().voxels = b.voxels;
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
            news.extend(self.services_month(t, e));
            news.extend(self.desarrollo_month(t, e));
            news.extend(self.materiales_mes(t, e));
            news.extend(self.month_tick());
            self.aplicar_mejoras(t, e);
        }
        // El río: horas del reloj del día (24 h cada 8 minutos, como el ciclo de día y noche).
        let was = self.water.wind;
        if let Some(n) = self.water.advance(dt * HOURS_PER_SECOND, self.month % 12) {
            news.push(n);
        }
        if matches!(was, water::Wind::Sudestada { .. }) && self.water.wind == water::Wind::Calma {
            self.pending_flood = Some(Flood::Rio(self.water.last_peak + 0.3));
        }
        if let Some(f) = self.pending_flood.take() {
            if let Some(n) = self.flood(t, f, e) {
                news.push(n);
            }
        }
        self.growth_acc += dt;
        while self.growth_acc >= 0.5 {
            self.growth_acc -= 0.5;
            let demand = self.demand();
            self.lots.sin_altos = !self.altos_posibles();
            let (used, changes) = self.lots.grow_step(self.pool, demand, &self.net, &mut self.rng);
            self.pool -= used;
            self.apply_levels(t, &changes, e);
            news.extend(self.services_step(t, e));
        }
        // Vecinos y vehículos.
        self.agentes.tick(t, &self.net, &self.lots, self.plaza, dt);
        news
    }

    /// Lo que la ciudad le pasa a la economía este mes. Mientras no existan en el motor las
    /// estancias, hornos y canteras del prototipo, el entorno rural produce según la población.
    pub fn month_input(&self) -> economy::MonthInput {
        use economy::Res;
        let pop = self.population();
        let built = self.lots.built();
        let houses = built[ZoneKind::Residencial as usize];
        let (jobs_c, jobs_i) = self.lots.jobs();
        let mut production = [0.0f32; 6];
        // Chacras privadas del ejido: el grano entra con las cosechas (trigo en diciembre y
        // enero, maíz en otoño), según el año que haya tocado.
        // La tierra del entorno (CONEAT) pesa: buena tierra de chacra o buenas pasturas
        // alrededor de la plaza rinden más (ver `recursos`).
        let (tierra_cultivo, tierra_pasto) = self.suelo.factor_entorno();
        let annual_grain = (300.0 + pop as f32 * 1.05) * 12.0 * tierra_cultivo;
        production[Res::Pan as usize] = self.climate.grain_this_month(self.month, annual_grain);
        // Ganado del campo: depende de las pasturas.
        let herd = self.climate.livestock_factor() * tierra_pasto;
        production[Res::Carne as usize] = (400.0 + pop as f32 * 0.9) * herd;
        production[Res::Ladrillos as usize] = 60.0 * self.climate.brick_factor();
        production[Res::Cueros as usize] = (80.0 + pop as f32 * 0.05) * herd;
        production[Res::Sebo as usize] = 40.0 * herd;
        // Los talleres de las zonas industriales: ladrillos, sebo y cueros curtidos.
        production[Res::Ladrillos as usize] += jobs_i as f32 * 1.5 * self.climate.brick_factor();
        production[Res::Sebo as usize] += jobs_i as f32 * 1.0;
        production[Res::Cueros as usize] += jobs_i as f32 * 0.8;
        // El grano de la cosecha se guarda en las casas y graneros hasta la próxima.
        let mut storage = [0.0f32; 6];
        storage[Res::Pan as usize] = pop as f32 * 12.0;
        // Las casas ya no gastan ladrillos por mes: al construirlas sacan del depósito lo que
        // piden y los arreglos se hacen con los materiales de cada una (ver `materiales`).
        let construction = [0.0f32; 6];
        let mut inp = economy::MonthInput {
            pop,
            houses,
            shops: built[ZoneKind::Comercial as usize],
            workshops: built[ZoneKind::Industrial as usize],
            jobs_c,
            jobs_i,
            production,
            construction,
            pedidos: self.materiales.pedidos_economia(),
            storage,
            ships: if self.month % 4 == 0 { 1 } else { 0 },
            water_ratio: (0.4 + self.climate.soil / climate::SUELO_MAX * 0.4).min(0.8),
            upkeep_roads: (self.net.total_length() * 0.01) as i64,
            world_hides: 1.0,
            ..Default::default()
        };
        self.services_input(&mut inp);
        self.desarrollo_input(&mut inp);
        inp
    }

    /// Fase de la luna (0 nueva, 0,5 llena): un mes del calendario es una lunación.
    pub fn moon_phase(&self) -> f32 {
        water::moon_phase(self.month, self.month_acc / MONTH_SECONDS)
    }

    /// Nivel del río (m) a esta hora del día: marea, viento y caudal.
    pub fn water_level(&self, hour: f32) -> f32 {
        self.water.level(hour, self.moon_phase())
    }

    /// Daños de una inundación: el río que subió con la sudestada (hasta `nivel` m) o las
    /// crecientes de los arroyos tras un temporal. Los ranchos de barro en lo bajo se pierden;
    /// las casas de material aguantan. Devuelve el aviso para la gaceta.
    fn flood(&mut self, t: &TerrainGen, f: Flood, e: &mut Edits) -> Option<String> {
        let mut lost = Vec::new();
        let mut mojados = Vec::new();
        let mut wet = 0;
        for l in self.lots.lots.values() {
            if l.level == 0 {
                continue;
            }
            let h = t.height(l.c.x, l.c.y);
            let under = match f {
                Flood::Rio(level) => h < level,
                Flood::Arroyos => {
                    let (d, hw) = t.arroyo_query(l.c.x, l.c.y);
                    d < hw + 25.0 + hw * 3.0
                }
            };
            if under {
                wet += 1;
                mojados.push(l.id);
                // El agua se lleva las paredes de barro y de adobe (ver `materiales`).
                if self.obra_de(l).map_or(l.level == 1, |o| o.paredes.se_lava()) {
                    lost.push(l.id);
                }
            }
        }
        let ahogados = self.poblacion.choque_inundacion(&mojados, &lost);
        for id in &lost {
            let lot = self.lots.lots.get_mut(id).unwrap();
            for &v in &lot.voxels {
                e.dset(v, AIR);
            }
            lot.voxels.clear();
            lot.level = 0;
            lot.age = 0;
            lot.obra = None;
        }
        if wet == 0 {
            return None;
        }
        let muertos = if ahogados > 0 { format!(" · {ahogados} muertos") } else { String::new() };
        Some(match f {
            Flood::Rio(level) => format!("La sudestada subió el río {:.1} m: {} solares bajo el agua, {} casas de barro o adobe perdidas{muertos}", level, wet, lost.len()),
            Flood::Arroyos => format!("Crecieron los arroyos: {} solares inundados en los bajos, {} casas de barro o adobe perdidas{muertos}", wet, lost.len()),
        })
    }

    fn month_tick(&mut self) -> Vec<String> {
        let mut news = Vec::new();
        self.month += 1;
        let pop = self.population();
        // Con trabajo de sobra llegan más familias; con desocupación, menos.
        let (jc, ji) = self.lots.jobs();
        let (jobs, workers) = ((jc + ji) as f32, self.workers() as f32);
        let work = if workers > 20.0 && jobs < workers * 0.6 { 0.75 } else if jobs > workers { 1.2 } else { 1.0 };
        // Los barcos (con sus pobladores) los trae el puerto (`services::port`).
        // La gente que ya vive en la ciudad cuenta cómo le va: su felicidad atrae o espanta.
        let animo = if self.poblacion.fundada { (0.75 + 0.5 * self.poblacion.stats.felicidad).clamp(0.8, 1.15) } else { 1.0 };
        let factor = self.economy.arrivals_factor() * work * self.services.attraction() * animo;
        let arrivals = ((4.0 + pop as f32 * 0.03) * factor) as i32;
        // Clima del mes: cosechas, pasturas, sequía y el caudal de los ríos.
        let (weather, cnews) = self.climate.month_tick(self.month % 12);
        news.extend(cnews);
        self.economy.climate_drought = Some(weather.drought);
        if let Some(n) = self.water.month_tick(weather.drought) {
            news.push(n);
        }
        if weather.deluge {
            self.pending_flood = Some(Flood::Arroyos);
        }
        let mut input = self.month_input();
        // La población: llegan las familias, nacen, mueren, se casan, trabajan y buscan casa.
        if self.poblacion.fundada {
            // Sin casas no viene más gente: con muchos sin techo, llegan cada vez menos.
            let sin_techo = (1.0 - (self.pool as f32 - 100.0).max(0.0) / 500.0).max(0.1);
            let llegan = ((arrivals.max(0) + std::mem::take(&mut self.llegados_barco)) as f32 * sin_techo) as u32;
            let ent = self.entorno_poblacion(llegan, input.water_ratio, input.epidemic, weather.drought);
            let inf = self.poblacion.mes(&ent);
            // Los que esperan casa son la gente sin casa (y la que sobra en casas hacinadas).
            self.pool = inf.sin_casa.min(600);
            self.mejoras = inf.mejoras;
            input.animo = Some(inf.animo);
            input.social = inf.lineas;
            news.extend(inf.noticias);
        }
        // Ganancias, salarios, escuelas y contratación del desarrollo (después de la población).
        self.desarrollo_tras_poblacion();
        if !self.poblacion.fundada {
            self.pool = (self.pool + arrivals).min(600);
        }
        let report = self.economy.month(&input);
        self.desarrollo_opinion();
        self.money += report.balance();
        news.extend(report.news);
        // El tránsito: si los viajes tardan mucho más de lo previsto, los vecinos se quejan.
        let o = &mut self.economy.opinion[0];
        *o = (*o + self.agentes.efecto_opinion()).clamp(0.0, 100.0);
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
        assert!(e.voxel_count() > 10_000, "plaza, iglesia y calles: {} vóxeles", e.voxel_count());
        let (f, s) = plaza_frame(0.0);
        let a = c - s * (BLOCK * 2.5) - f * (BLOCK * 2.5);
        city.build_grid(&t, &grid_lines(a, a + s * BLOCK * 5.0, a + f * BLOCK * 5.0), RoadKind::Stone).unwrap();
        assert!(city.lots.lots.len() > 100, "{} solares", city.lots.lots.len());
        let (n, _) = city.paint(c, 300.0, 1);
        assert!(n > 100);
        let e = city.fast_forward(&t, 24);
        assert!(city.population() > 100, "población {}", city.population());
        assert!(!e.is_empty());
        // Ningún edificio pisa una calzada.
        for lot in city.lots.lots.values().filter(|l| l.level > 0) {
            for v in lot.voxels.iter().step_by(37) {
                let p = Vec2::new((v.x as f32 + 0.5) * crate::lod::DESIGN_VOXEL, (v.z as f32 + 0.5) * crate::lod::DESIGN_VOXEL);
                assert!(city.net.clearance(p, 20.0) > -0.3, "un edificio pisa la calle en {p}");
            }
        }
    }

    #[test]
    fn zonas_residencial_comercial_e_industrial_con_densidad() {
        use lots::{zone_code, Density};
        let t = TerrainGen::new(1726);
        let mut city = City::default();
        city.money = 1_000_000;
        let c = Vec2::new(13500.0, 16300.0);
        city.found_plaza(&t, c, 0.0).unwrap();
        let (f, s) = plaza_frame(0.0);
        let a = c - s * (BLOCK * 2.5) - f * (BLOCK * 2.5);
        city.build_grid(&t, &grid_lines(a, a + s * BLOCK * 5.0, a + f * BLOCK * 5.0), RoadKind::Stone).unwrap();
        city.paint(c, 400.0, zone_code(ZoneKind::Residencial, Density::Baja));
        city.paint(c, 90.0, zone_code(ZoneKind::Comercial, Density::Media));
        let far = c - f * (BLOCK * 2.0) + s * (BLOCK * 2.0);
        city.paint(far, 50.0, zone_code(ZoneKind::Industrial, Density::Alta));
        assert!(city.demand().c > 0.0 && city.demand().i > 0.0, "{:?}", city.demand());
        city.fast_forward(&t, 48);
        let (jc, ji) = city.lots.jobs();
        assert!(city.population() > 100 && jc > 0 && ji > 0, "pob {} · empleos {jc}/{ji}", city.population());
        for l in city.lots.lots.values() {
            let (_, d) = lots::zone_of(l.zone).unwrap_or((ZoneKind::Residencial, Density::Media));
            let (first, last) = d.levels();
            assert!(l.level == 0 || (first..=last).contains(&l.level), "nivel {} fuera de la densidad {d:?}", l.level);
        }
        // Densidad baja: ninguna casa de altos; densidad alta: ningún galpón de paja.
        assert!(city.lots.lots.values().filter(|l| l.zone == zone_code(ZoneKind::Residencial, Density::Baja)).all(|l| l.level <= 2));
        // Los talleres bajan el valor del suelo de las casas vecinas.
        let near = city.lots.lots.values().filter(|l| l.kind() == Some(ZoneKind::Residencial)).min_by(|x, y| x.c.distance(far).total_cmp(&y.c.distance(far))).unwrap();
        if city.lots.industry_near(near.c, near.id) > 0 {
            let mut clean = near.clone();
            clean.c = c + s * 1.0;
            assert!(city.lots.land_value(near) < city.lots.land_value(&clean));
        }
        // Rezonificar de residencial a comercial derriba lo construido.
        let built = city.lots.lots.values().find(|l| l.level > 0 && l.kind() == Some(ZoneKind::Residencial)).unwrap().clone();
        let (_, e) = city.paint(built.c, 1.0, zone_code(ZoneKind::Comercial, Density::Baja));
        assert!(!e.is_empty());
        assert_eq!(city.lots.lots[&built.id].level, 0);
    }

    #[test]
    fn puentes_sobre_arroyos_y_nada_de_solares_en_el_agua() {
        let t = TerrainGen::montevideo();
        let mut city = City::default();
        city.money = 1_000_000;
        city.found_plaza(&t, t.founding_site(), 0.0).unwrap();
        // Una cañada o arroyo cualquiera del mapa, en un tramo recto: una calle que lo cruza.
        let (mut a, mut b) = (Vec2::ZERO, Vec2::ZERO);
        'busca: for arr in &t.arroyos {
            for w in arr.windows(2).step_by(7) {
                let (p, q) = (w[0], w[1]);
                let dir = (q - p).normalize_or_zero();
                let n = Vec2::new(-dir.y, dir.x);
                let m = (p + q) * 0.5;
                let (x0, x1) = (m - n * 30.0, m + n * 30.0);
                if !t.is_water(x0.x, x0.y) && !t.is_water(x1.x, x1.y) && t.is_water(m.x, m.y) && m.x > 2000.0 && m.y > 2000.0 && m.x < 22000.0 && m.y < 22000.0 {
                    let line = roads::line(x0, x1);
                    if city.road_problem(&t, &line, RoadKind::Stone).is_none() {
                        a = x0;
                        b = x1;
                        break 'busca;
                    }
                }
            }
        }
        assert!(a != b, "no se encontró un arroyo para cruzar");
        let line = roads::line(a, b);
        let spans = voxelize::water_spans(&t, &line);
        assert_eq!(spans.len(), 1, "un solo puente: {spans:?}");
        assert!(City::road_cost(&t, &line, RoadKind::Stone) > (60.0 * RoadKind::Stone.cost_per_m()) as i64, "el puente cuesta más");
        let e = city.build_road(&t, &line, RoadKind::Stone).unwrap();
        // El agua sigue corriendo: el centro del cauce no tiene suelo nivelado ni superficie.
        let mid = (a + b) * 0.5;
        let vs = crate::lod::BASE_VOXEL;
        let c = glam::IVec2::new((mid.x / vs).floor() as i32, (mid.y / vs).floor() as i32);
        assert!(!e.ground.contains_key(&c) && !e.surface.contains_key(&c), "el cauce no se rellena");
        assert!(e.voxels.iter().any(|(v, &m)| v.x == c.x && v.z == c.y && m == crate::palette::STONE), "hay tablero de piedra sobre el agua");
        // Ningún solar sobre el agua.
        for l in city.lots.lots.values() {
            assert!(!t.is_water(l.c.x, l.c.y), "solar {} en el agua", l.id);
        }
        // Un cruce de más de 80 m (el río) no se puede.
        let fs = t.founding_site();
        let sea = roads::line(fs + Vec2::new(0.0, 300.0), fs + Vec2::new(0.0, 1500.0));
        assert!(city.road_problem(&t, &sea, RoadKind::Stone).is_some_and(|m| m.contains("puentes")), "{:?}", city.road_problem(&t, &sea, RoadKind::Stone));
    }
}
