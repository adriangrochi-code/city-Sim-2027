//! Estado de los servicios para guardar la partida (va dentro de `CityData`). Los vóxeles no
//! se guardan: se regeneran con [`City::services_rebuild`](super::super::City::services_rebuild).

use super::epidemics::Epidemic;
use super::fires::{Burn, Fires};
use super::port::{Pier, Port};
use super::works::{Church, Tramo, Walls};
use super::{info, kind_from_key, Building, Services};
use crate::buildings::Rng;
use glam::Vec2;
use serde::{Deserialize, Serialize};

fn v2(p: Vec2) -> [f32; 2] {
    [p.x, p.y]
}

fn p2(a: [f32; 2]) -> Vec2 {
    Vec2::new(a[0], a[1])
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BuildingData {
    pub id: u32,
    pub tipo: String,
    pub centro: [f32; 2],
    pub frente: [f32; 2],
    pub abastece: i32,
    /// Pozos y aljibes: cuánto se ensució el agua (las partidas viejas, limpia).
    #[serde(default)]
    pub contaminacion: f32,
    /// Rinde del sitio (las partidas viejas lo calculan al cargar).
    #[serde(default)]
    pub rinde: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServicesData {
    pub edificios: Vec<BuildingData>,
    pub proximo: u32,
    /// Agua de cada solar: (solar, calidad, fuente).
    pub agua: Vec<(u32, f32, u32)>,
    /// Tope de nivel de cada solar por el agua.
    pub topes: Vec<(u32, u8)>,
    /// Iglesia: fase terminada, en obra, meses que faltan, parada.
    pub iglesia: (u32, bool, u32, bool),
    /// Muralla: tramos (a, b, metros levantados) y si está parada.
    pub muralla: Vec<([f32; 2], [f32; 2], f32)>,
    pub muralla_parada: bool,
    /// Muelle: arranque, dirección, largo y barraca.
    pub muelle: Option<([f32; 2], [f32; 2], f32, bool)>,
    /// Acontecimiento lejano (índice, meses), meses al próximo barco, barcos del mes y del año.
    pub acontecimiento: Option<(usize, u32)>,
    pub proximo_barco: f32,
    pub barcos_mes: i32,
    pub anio_puerto: [i32; 3],
    /// Fuegos: (solar, intensidad, pasos al máximo, combatido) y totales.
    pub fuegos: Vec<(u32, f32, u32, bool)>,
    pub quemadas: u32,
    pub apagados: u32,
    pub ruinas: Vec<u32>,
    /// Epidemia: enfermos (solar, meses), cuarentenas (meses, personas) y totales.
    pub enfermos: Vec<(u32, u32)>,
    pub cuarentena: Vec<(u32, i32)>,
    pub muertes: i32,
    pub brotes: u32,
    pub en_cuarentena_total: i32,
    pub azar: u64,
    /// Faroles (columna de diseño, base) y la firma de las calles con que se repartieron.
    #[serde(default)]
    pub faroles: Vec<([i32; 2], i32)>,
    #[serde(default)]
    pub faroles_firma: (usize, i64),
    /// Basurales, presos, delitos y brotes (las partidas viejas empiezan sin ellos).
    #[serde(default)]
    pub barrio: super::barrio::BarrioData,
}

impl ServicesData {
    pub fn from_services(s: &Services, caps: &std::collections::HashMap<u32, u8>) -> ServicesData {
        let mut topes: Vec<(u32, u8)> = caps.iter().map(|(&k, &v)| (k, v)).collect();
        topes.sort();
        let p = &s.port;
        ServicesData {
            edificios: s.buildings.values().map(|b| BuildingData { id: b.id, tipo: info(b.kind).key.into(), centro: v2(b.c), frente: v2(b.f), abastece: b.served, contaminacion: b.contaminacion, rinde: (b.rinde >= 0.0).then_some(b.rinde) }).collect(),
            proximo: s.next,
            agua: s.water.iter().map(|(&l, &(q, src))| (l, q, src)).collect(),
            topes,
            iglesia: (s.church.phase, s.church.building, s.church.months_left, s.church.stalled),
            muralla: s.walls.tramos.iter().map(|t| (v2(t.a), v2(t.b), t.built)).collect(),
            muralla_parada: s.walls.stalled,
            muelle: p.pier.as_ref().map(|m| (v2(m.land), v2(m.dir), m.len, m.barraca)),
            acontecimiento: p.event,
            proximo_barco: p.ship_timer,
            barcos_mes: p.ships_month,
            anio_puerto: p.year,
            fuegos: s.fires.burning.iter().map(|(&id, b)| (id, b.i, b.full, b.fought)).collect(),
            quemadas: s.fires.destroyed_total,
            apagados: s.fires.put_out_total,
            ruinas: s.ruins.iter().copied().collect(),
            enfermos: s.epidemic.infected.iter().map(|(&k, &v)| (k, v)).collect(),
            cuarentena: s.epidemic.quarantine.clone(),
            muertes: s.epidemic.deaths_total,
            brotes: s.epidemic.outbreaks_total,
            en_cuarentena_total: s.epidemic.quarantined_total,
            azar: s.rng.state(),
            faroles: s.lamps.iter().map(|&(v, b)| ([v.x, v.y], b)).collect(),
            faroles_firma: s.lamp_sig,
            barrio: s.barrio.guardar(),
        }
    }

    /// Rehace los servicios (sin vóxeles) y los topes de nivel de los solares.
    pub fn to_services(&self) -> Result<(Services, std::collections::HashMap<u32, u8>), String> {
        let mut s = Services::default();
        for b in &self.edificios {
            let kind = kind_from_key(&b.tipo).ok_or_else(|| format!("servicio desconocido: {}", b.tipo))?;
            s.buildings.insert(b.id, Building { id: b.id, kind, c: p2(b.centro), f: p2(b.frente), served: b.abastece, contaminacion: b.contaminacion.clamp(0.0, 1.0), rinde: b.rinde.map_or(-1.0, |r| r.clamp(0.0, 2.0)), voxels: Vec::new() });
        }
        s.next = self.proximo.max(s.buildings.keys().copied().max().unwrap_or(0));
        s.water = self.agua.iter().map(|&(l, q, src)| (l, (q, src))).collect();
        let (phase, building, months, stalled) = self.iglesia;
        s.church = Church { phase: phase.min(5), building: building && phase < 5, months_left: months, stalled, voxels: Vec::new() };
        s.walls = Walls { tramos: self.muralla.iter().map(|&(a, b, built)| Tramo { a: p2(a), b: p2(b), built, voxels: Vec::new() }).collect(), stalled: self.muralla_parada, ..Default::default() };
        s.port = Port {
            pier: self.muelle.map(|(l, d, len, barraca)| Pier { land: p2(l), dir: p2(d), len, barraca, voxels: Vec::new() }),
            event: self.acontecimiento.filter(|e| e.0 < super::port::EVENTS.len()),
            ship_timer: self.proximo_barco,
            ships_month: self.barcos_mes,
            year: self.anio_puerto,
        };
        s.fires = Fires { burning: self.fuegos.iter().map(|&(id, i, full, fought)| (id, Burn { i, full, fought })).collect(), destroyed_total: self.quemadas, put_out_total: self.apagados };
        s.ruins = self.ruinas.iter().copied().collect();
        s.epidemic = Epidemic { infected: self.enfermos.iter().copied().collect(), quarantine: self.cuarentena.clone(), deaths_total: self.muertes, outbreaks_total: self.brotes, quarantined_total: self.en_cuarentena_total };
        s.rng = Rng::from_state(self.azar);
        s.lamps = self.faroles.iter().map(|&(v, b)| (glam::IVec2::new(v[0], v[1]), b)).collect();
        s.lamp_sig = self.faroles_firma;
        s.barrio = super::barrio::Barrio::cargar(&self.barrio);
        Ok((s, self.topes.iter().copied().collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::save::CityData;
    use super::super::super::BLOCK;
    use super::super::tests::{free_site, town};
    use super::super::Kind;
    use crate::buildings::Edits;
    use crate::terrain::TerrainGen;
    use glam::Vec2;

    /// Servicios, iglesia en obra, muralla, fuego, ruinas y epidemia: se guardan, se cargan con
    /// los mismos vóxeles y la simulación sigue igual que si no se hubiera guardado.
    #[test]
    fn guardar_y_cargar_los_servicios() {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        city.fast_forward(&t, 6);
        let c = city.plaza.unwrap().0;
        for (k, at) in [(Kind::Pozo, c + Vec2::new(60.0, 60.0)), (Kind::Aljibe, c - Vec2::new(60.0, 60.0)), (Kind::Serenos, c + Vec2::new(-80.0, 40.0))] {
            let (sc, sf) = free_site(&city, &t, k, at);
            city.place_service(&t, k, sc, sf).unwrap();
        }
        let y = c.y + BLOCK;
        city.build_wall(&t, Vec2::new(c.x - 170.0, y), Vec2::new(c.x + 170.0, y)).unwrap();
        city.services.church.phase = 1;
        city.services.church.building = true;
        city.services.church.months_left = 7;
        let mut e = Edits::default();
        city.church_refresh(&t, &mut e);
        city.fast_forward(&t, 6);
        // Un fuego en curso y una epidemia.
        let l = city.lots.lots.values().find(|l| l.level > 0).unwrap().id;
        city.ignite(l);
        city.fire_step(&t, &mut e);
        city.sick_ship(30, c);
        let data = CityData::from_city(&city);
        let json = serde_json::to_string(&data).unwrap();
        let back: CityData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, data);
        let mut loaded = back.to_city().unwrap();
        let ed = loaded.rebuild_edits(&t);
        assert!(!ed.design.is_empty());
        let s0 = &city.services;
        let s1 = &loaded.services;
        assert_eq!(s1.buildings.len(), s0.buildings.len());
        for (a, b) in s1.buildings.values().zip(s0.buildings.values()) {
            assert_eq!((a.id, a.kind, a.c, a.f, a.served), (b.id, b.kind, b.c, b.f, b.served));
            assert_eq!(a.voxels.len(), b.voxels.len(), "{:?}", a.kind);
        }
        assert_eq!(s1.church.voxels.len(), s0.church.voxels.len(), "iglesia");
        assert_eq!((s1.church.phase, s1.church.building, s1.church.months_left), (s0.church.phase, s0.church.building, s0.church.months_left));
        assert_eq!(s1.walls.tramos[0].voxels.len(), s0.walls.tramos[0].voxels.len(), "muralla");
        assert_eq!(s1.lamps, s0.lamps, "faroles");
        assert_eq!(s1.water, s0.water);
        assert_eq!(loaded.lots.level_cap, city.lots.level_cap);
        assert_eq!(s1.fires.burning, s0.fires.burning);
        assert_eq!(s1.epidemic.infected, s0.epidemic.infected);
        // Misma simulación después de cargar.
        city.fast_forward(&t, 12);
        loaded.fast_forward(&t, 12);
        assert_eq!((loaded.money, loaded.pool, loaded.population()), (city.money, city.pool, city.population()));
        let lv = |c: &super::super::super::City| c.lots.lots.values().map(|l| (l.level, l.age)).collect::<Vec<_>>();
        assert_eq!(lv(&loaded), lv(&city));
        assert_eq!(loaded.services.ruins, city.services.ruins);
        assert_eq!(loaded.services.fires.destroyed_total, city.services.fires.destroyed_total);
        assert_eq!(loaded.services.walls.tramos[0].built, city.services.walls.tramos[0].built);
        assert_eq!(loaded.services.epidemic.deaths_total, city.services.epidemic.deaths_total);
        // Una partida vieja (sin servicios) se sigue cargando.
        let mut old = CityData::from_city(&city);
        old.servicios = None;
        let json = serde_json::to_string(&old).unwrap().replace(",\"servicios\":null", "");
        let back: CityData = serde_json::from_str(&json).unwrap();
        assert!(back.to_city().unwrap().services.buildings.is_empty());
    }
}
