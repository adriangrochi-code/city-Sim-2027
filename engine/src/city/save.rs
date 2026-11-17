//! Estado de la ciudad para guardar y cargar partidas: una copia plana de [`City`] (calles,
//! solares, tesoro, mes, plaza, economía y el estado del azar) que se escribe en JSON.
//!
//! No se guardan los vóxeles: al cargar se regeneran con [`City::rebuild_edits`], que es
//! determinista a partir de las semillas de los solares.

use super::economy::{Economy, Line, MonthReport, Ordinance, Stock, ORDINANCES};
use super::lots::{Lot, Lots, Obstacle};
use super::roads::{RoadKind, RoadNetwork};
use super::City;
use crate::buildings::Rng;
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn v2(p: Vec2) -> [f32; 2] {
    [p.x, p.y]
}

fn p2(a: [f32; 2]) -> Vec2 {
    Vec2::new(a[0], a[1])
}

pub fn kind_key(k: RoadKind) -> &'static str {
    match k {
        RoadKind::Dirt => "tierra",
        RoadKind::Stone => "empedrada",
    }
}

pub fn kind_from(s: &str) -> Result<RoadKind, String> {
    match s {
        "tierra" => Ok(RoadKind::Dirt),
        "empedrada" => Ok(RoadKind::Stone),
        o => Err(format!("tipo de calle desconocido: {o}")),
    }
}

fn ord_key(o: Ordinance) -> String {
    format!("{o:?}")
}

fn ord_from(s: &str) -> Option<Ordinance> {
    ORDINANCES.iter().map(|i| i.id).find(|&o| ord_key(o) == s)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EdgeData {
    pub id: u32,
    pub a: u32,
    pub b: u32,
    pub tipo: String,
    pub puntos: Vec<[f32; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RoadsData {
    pub nodos: Vec<(u32, [f32; 2])>,
    pub tramos: Vec<EdgeData>,
    pub proximo_nodo: u32,
    pub proximo_tramo: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LotData {
    pub id: u32,
    pub centro: [f32; 2],
    pub frente: [f32; 2],
    pub zona: u8,
    pub nivel: u8,
    pub edad: u32,
    pub semilla: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ObstacleData {
    pub centro: [f32; 2],
    pub frente: [f32; 2],
    pub ancho: f32,
    pub fondo: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LotsData {
    pub solares: Vec<LotData>,
    pub obstaculos: Vec<ObstacleData>,
    pub centro_plaza: Option<[f32; 2]>,
    pub proximo: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StockData {
    pub cantidad: f32,
    pub capacidad: f32,
    pub producido: f32,
    pub consumido: f32,
    pub importado: f32,
    pub exportado: f32,
    pub precio: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EconomyData {
    pub existencias: Vec<StockData>,
    pub impuestos: [i32; 3],
    pub ordenanzas: Vec<String>,
    pub opinion: [f32; 3],
    pub posito: f32,
    pub meses_de_sequia: u32,
    /// Último presupuesto cerrado: (concepto, monto).
    pub ultimo_mes: Vec<(String, i64)>,
    pub ultimo_subsidio: i64,
    pub azar: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CityData {
    pub dinero: i64,
    pub esperan_casa: i32,
    pub mes: u32,
    /// Centro y ángulo de la plaza mayor (si está fundada).
    pub plaza: Option<([f32; 2], f32)>,
    pub azar: u64,
    pub acumulado_mes: f32,
    pub acumulado_crecimiento: f32,
    pub calles: RoadsData,
    pub solares: LotsData,
    pub economia: EconomyData,
    /// Clima y río (desde el formato 1 ampliado; las partidas viejas empiezan con clima normal).
    #[serde(default)]
    pub clima: Option<(Vec<f32>, u64)>,
    #[serde(default)]
    pub rio: Option<(Vec<f32>, u64)>,
    /// Vecinos y vehículos (las partidas viejas los generan de nuevo desde los solares).
    #[serde(default)]
    pub vecinos: Option<super::agentes::Estado>,
    /// Servicios, obras, puerto, incendios y epidemias (las partidas viejas empiezan sin ellos).
    #[serde(default)]
    pub servicios: Option<super::services::save::ServicesData>,
    /// Los habitantes (personas, hogares, linajes, crónica). Las partidas anteriores a la
    /// población la generan al cargar a partir de la fundación y de las casas.
    #[serde(default)]
    pub poblacion: Option<super::poblacion::Poblacion>,
}

impl CityData {
    pub fn from_city(c: &City) -> CityData {
        let (nn, ne) = c.net.counters();
        let calles = RoadsData {
            nodos: c.net.nodes.iter().map(|(&k, &p)| (k, v2(p))).collect(),
            tramos: c.net.edges.iter().map(|(&id, e)| EdgeData { id, a: e.a, b: e.b, tipo: kind_key(e.kind).into(), puntos: e.pts.iter().map(|&p| v2(p)).collect() }).collect(),
            proximo_nodo: nn,
            proximo_tramo: ne,
        };
        let solares = LotsData {
            solares: c.lots.lots.values().map(|l| LotData { id: l.id, centro: v2(l.c), frente: v2(l.f), zona: l.zone, nivel: l.level, edad: l.age, semilla: l.seed }).collect(),
            obstaculos: c.lots.obstacles.iter().map(|o| ObstacleData { centro: v2(o.c), frente: v2(o.f), ancho: o.w, fondo: o.d }).collect(),
            centro_plaza: c.lots.plaza_center.map(v2),
            proximo: c.lots.next_id(),
        };
        let e = &c.economy;
        let economia = EconomyData {
            existencias: e.stocks.iter().map(|s| StockData { cantidad: s.amount, capacidad: s.capacity, producido: s.produced, consumido: s.consumed, importado: s.imported, exportado: s.exported, precio: s.price }).collect(),
            impuestos: e.taxes,
            ordenanzas: e.active.iter().map(|&o| ord_key(o)).collect(),
            opinion: e.opinion,
            posito: e.posito,
            meses_de_sequia: e.drought_months,
            ultimo_mes: e.last.lines.iter().map(|l| (l.concept.clone(), l.amount)).collect(),
            ultimo_subsidio: e.last_subsidy,
            azar: e.rng.state(),
        };
        CityData {
            dinero: c.money,
            esperan_casa: c.pool,
            mes: c.month,
            plaza: c.plaza.map(|(p, a)| (v2(p), a)),
            azar: c.rng.state(),
            acumulado_mes: c.month_acc,
            acumulado_crecimiento: c.growth_acc,
            calles,
            solares,
            economia,
            clima: Some(c.climate.to_state()),
            rio: Some(c.water.to_state()),
            vecinos: Some(c.agentes.e.clone()),
            servicios: Some(super::services::save::ServicesData::from_services(&c.services, &c.lots.level_cap)),
            poblacion: c.poblacion.fundada.then(|| c.poblacion.clone()),
        }
    }

    /// Rehace la ciudad (sin vóxeles: después hay que llamar a [`City::rebuild_edits`]).
    pub fn to_city(&self) -> Result<City, String> {
        let nodes: BTreeMap<u32, Vec2> = self.calles.nodos.iter().map(|&(k, p)| (k, p2(p))).collect();
        let mut edges = Vec::new();
        for e in &self.calles.tramos {
            edges.push((e.id, e.a, e.b, e.puntos.iter().map(|&p| p2(p)).collect(), kind_from(&e.tipo)?));
        }
        let net = RoadNetwork::restore(nodes, edges, self.calles.proximo_nodo, self.calles.proximo_tramo)?;
        let lots: Vec<Lot> = self.solares.solares.iter().map(|l| Lot { id: l.id, c: p2(l.centro), f: p2(l.frente), zone: l.zona.min(9), level: l.nivel.min(3), age: l.edad, seed: l.semilla, voxels: Vec::new() }).collect();
        let obstacles = self.solares.obstaculos.iter().map(|o| Obstacle { c: p2(o.centro), f: p2(o.frente), w: o.ancho, d: o.fondo }).collect();
        let mut lots = Lots::restore(lots, obstacles, self.solares.centro_plaza.map(p2), self.solares.proximo);
        let (services, caps) = match &self.servicios {
            Some(d) => d.to_services()?,
            None => Default::default(),
        };
        lots.level_cap = caps;
        lots.walls = services.walls.obstacles();
        let ed = &self.economia;
        let mut economy = Economy::default();
        for (s, d) in economy.stocks.iter_mut().zip(&ed.existencias) {
            *s = Stock { amount: d.cantidad, capacity: d.capacidad, produced: d.producido, consumed: d.consumido, imported: d.importado, exported: d.exportado, price: d.precio };
        }
        economy.taxes = ed.impuestos;
        economy.active = ed.ordenanzas.iter().filter_map(|s| ord_from(s)).collect();
        economy.opinion = ed.opinion;
        economy.posito = ed.posito;
        economy.drought_months = ed.meses_de_sequia;
        economy.last = MonthReport { lines: ed.ultimo_mes.iter().map(|(c, a)| Line { concept: c.clone(), amount: *a }).collect(), ..Default::default() };
        economy.last_subsidy = ed.ultimo_subsidio;
        economy.rng = Rng::from_state(ed.azar);
        let mut city = City {
            net,
            lots,
            money: self.dinero,
            pool: self.esperan_casa,
            month: self.mes,
            plaza: self.plaza.map(|(p, a)| (p2(p), a)),
            economy,
            climate: self.clima.as_ref().map(|(v, a)| super::climate::Climate::from_state(v, *a)).unwrap_or_default(),
            water: self.rio.as_ref().map(|(v, a)| super::water::WaterLevel::from_state(v, *a)).unwrap_or_default(),
            agentes: self.vecinos.clone().map(super::agentes::Agentes::from_estado).unwrap_or_default(),
            services,
            pending_flood: None,
            rng: Rng::from_state(self.azar),
            month_acc: self.acumulado_mes,
            growth_acc: self.acumulado_crecimiento,
            poblacion: super::poblacion::Poblacion::nueva(1726),
            mejoras: Vec::new(),
            llegados_barco: 0,
        };
        city.poblacion = match &self.poblacion {
            Some(p) => {
                let mut p = p.clone();
                p.reconstruir();
                p
            }
            None if city.plaza.is_some() => super::poblacion::Poblacion::desde_partida_vieja(1726 ^ self.azar, self.mes as i32, city.population()),
            None => super::poblacion::Poblacion::nueva(1726),
        };
        if self.poblacion.is_none() {
            city.poblacion.conocer_lotes(&city.lotes_info());
            city.poblacion.stats = city.poblacion.estadisticas();
        }
        Ok(city)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{grid_lines, plaza_frame, BLOCK};
    use super::*;
    use crate::terrain::TerrainGen;

    /// Una ciudad fundada, con damero, zonas, ordenanzas e impuestos, crecida unos años.
    pub fn sample_city(t: &TerrainGen) -> City {
        use super::super::lots::{zone_code, Density, ZoneKind};
        let mut city = City::default();
        city.money = 1_000_000;
        let c = Vec2::new(13500.0, 16300.0);
        city.found_plaza(t, c, 0.0).unwrap();
        let (f, s) = plaza_frame(0.0);
        let a = c - s * (BLOCK * 1.5) - f * (BLOCK * 1.5);
        city.build_grid(t, &grid_lines(a, a + s * BLOCK * 3.0, a + f * BLOCK * 3.0), RoadKind::Stone).unwrap();
        city.paint(c, 300.0, zone_code(ZoneKind::Residencial, Density::Media));
        city.paint(c, 60.0, zone_code(ZoneKind::Comercial, Density::Media));
        let mut money = city.money;
        city.economy.toggle(Ordinance::Posito, &mut money).unwrap();
        city.money = money;
        city.economy.set_tax(super::super::economy::Tax::Alcabala, 7);
        city.fast_forward(t, 30);
        city
    }

    #[test]
    fn guardar_y_cargar_da_la_misma_ciudad() {
        let t = TerrainGen::new(1726);
        let mut city = sample_city(&t);
        let data = CityData::from_city(&city);
        let json = serde_json::to_string(&data).unwrap();
        let back: CityData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, data, "el JSON ida y vuelta conserva todo");
        let mut loaded = back.to_city().unwrap();
        assert_eq!(loaded.net.edges.len(), city.net.edges.len());
        assert_eq!(loaded.net.nodes, city.net.nodes);
        assert_eq!(loaded.net.node_edges, city.net.node_edges);
        assert_eq!(loaded.lots.lots.len(), city.lots.lots.len());
        for (a, b) in loaded.lots.lots.values().zip(city.lots.lots.values()) {
            assert_eq!((a.id, a.c, a.f, a.zone, a.level, a.age, a.seed), (b.id, b.c, b.f, b.zone, b.level, b.age, b.seed));
        }
        assert_eq!((loaded.money, loaded.pool, loaded.month, loaded.population()), (city.money, city.pool, city.month, city.population()));
        assert_eq!(loaded.economy.taxes, city.economy.taxes);
        assert_eq!(loaded.economy.active, city.economy.active);
        assert_eq!(loaded.economy.opinion, city.economy.opinion);
        assert_eq!(loaded.economy.last.balance(), city.economy.last.balance());
        assert_eq!(loaded.demand(), city.demand());
        assert_eq!(loaded.climate.to_state(), city.climate.to_state(), "clima");
        assert_eq!(loaded.water.to_state(), city.water.to_state(), "río");
        assert_eq!(loaded.economy.drought_months, city.economy.drought_months);
        assert_eq!(loaded.economy.posito, city.economy.posito);
        assert!(city.poblacion.habitantes() > 100, "la ciudad tiene habitantes: {}", city.poblacion.habitantes());
        assert!(loaded.poblacion == city.poblacion, "la población cargada es IGUAL");
        assert_eq!(loaded.poblacion.stats, city.poblacion.stats);
        for (a, b) in loaded.economy.stocks.iter().zip(&city.economy.stocks) {
            assert_eq!((a.amount, a.price, a.capacity), (b.amount, b.price, b.capacity), "existencias");
        }
        // Los vóxeles regenerados son los mismos edificios que había.
        let e = loaded.rebuild_edits(&t);
        assert!(e.voxel_count() > 10_000);
        for (a, b) in loaded.lots.lots.values().zip(city.lots.lots.values()) {
            assert_eq!(a.voxels.len(), b.voxels.len(), "solar {}", a.id);
        }
        // Y la simulación sigue igual que si no se hubiera guardado (mismo azar).
        city.fast_forward(&t, 12);
        loaded.fast_forward(&t, 12);
        assert_eq!((loaded.money, loaded.pool, loaded.population(), loaded.month), (city.money, city.pool, city.population(), city.month));
        let lv = |c: &City| c.lots.lots.values().map(|l| (l.level, l.age)).collect::<Vec<_>>();
        assert_eq!(lv(&loaded), lv(&city));
        assert!(loaded.poblacion == city.poblacion, "la población sigue IGUAL un año después");
        assert_eq!(loaded.economy.opinion, city.economy.opinion);
        // Las calles cargadas se pueden seguir editando (índices reconstruidos).
        let p = Vec2::new(13500.0, 16300.0) + Vec2::new(BLOCK * 0.5, -BLOCK * 0.5);
        assert!(loaded.net.nearest_edge(p, 2.0).is_some());
        assert!(loaded.demolish_road(&t, p + Vec2::new(0.0, -20.0)).is_ok());
    }

    #[test]
    fn una_partida_de_antes_de_la_poblacion_la_genera() {
        let t = TerrainGen::new(1726);
        let city = sample_city(&t);
        let mut d = CityData::from_city(&city);
        d.poblacion = None;
        let json = serde_json::to_string(&d).unwrap().replace(",\"poblacion\":null", "");
        let back: CityData = serde_json::from_str(&json).unwrap();
        let mut c = back.to_city().unwrap();
        assert!(c.poblacion.fundada && c.poblacion.habitantes() as i32 >= city.population());
        c.fast_forward(&t, 6);
        assert!(c.poblacion.habitantes() > 0);
    }

    #[test]
    fn una_partida_rota_no_se_carga() {
        let t = TerrainGen::new(1726);
        let mut d = CityData::from_city(&sample_city(&t));
        d.calles.tramos[0].tipo = "adoquines".into();
        assert!(d.to_city().is_err());
        let mut d2 = CityData::from_city(&City::default());
        d2.calles.tramos.push(EdgeData { id: 1, a: 7, b: 8, tipo: "tierra".into(), puntos: vec![[0.0, 0.0], [1.0, 1.0]] });
        assert!(d2.to_city().is_err(), "tramo con nodos que no existen");
    }
}
