//! Epidemias que llegan por barco. Con lazareto, los recién llegados hacen cuarentena en una
//! punta alejada y la enfermedad no entra; sin lazareto, la fiebre desembarca con ellos y se
//! propaga de casa en casa, más rápido donde el agua es mala (y menos con la limpieza de
//! calles). Porte de `scripts/city/epidemics.gd`.

use super::super::City;
use super::Kind;
use crate::buildings::Edits;
use crate::terrain::TerrainGen;
use glam::Vec2;
use std::collections::BTreeMap;

/// Distancia de contagio entre casas (m).
pub const SPREAD_DIST: f32 = 40.0;
/// Meses que dura enferma una casa.
pub const SICK_MONTHS: u32 = 4;
/// Meses de cuarentena en el lazareto.
pub const QUARANTINE_MONTHS: u32 = 2;

#[derive(Clone, Debug, Default)]
pub struct Epidemic {
    /// Solar enfermo → meses que lleva.
    pub infected: BTreeMap<u32, u32>,
    /// Grupos en cuarentena: (meses que faltan, personas).
    pub quarantine: Vec<(u32, i32)>,
    pub deaths_total: i32,
    pub outbreaks_total: u32,
    pub quarantined_total: i32,
}

impl Epidemic {
    pub fn active(&self) -> bool {
        !self.infected.is_empty()
    }
    pub fn quarantined_now(&self) -> i32 {
        self.quarantine.iter().map(|q| q.1).sum()
    }
}

impl City {
    /// Llegó un barco con fiebre a bordo. Devuelve cuántas personas pasan ya a la ciudad.
    pub fn sick_ship(&mut self, people: i32, landing: Vec2) -> (i32, String) {
        let ep = &mut self.services.epidemic;
        if self.services.buildings.values().any(|b| b.kind == Kind::Lazareto) {
            ep.quarantine.push((QUARANTINE_MONTHS, people));
            ep.quarantined_total += people;
            return (0, format!("Fiebre a bordo: {people} pasajeros hacen cuarentena en el lazareto"));
        }
        ep.outbreaks_total += 1;
        // La enfermedad empieza en las casas más cercanas al desembarco.
        let mut cands: Vec<(f32, u32)> = self.lots.lots.values().filter(|l| l.level > 0).map(|l| (l.c.distance(landing), l.id)).collect();
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for &(_, id) in cands.iter().take(3) {
            self.services.epidemic.infected.insert(id, 0);
        }
        (people, "¡Fiebre! Un barco trae la enfermedad y no hay lazareto".into())
    }

    /// Contagios entre vecinos (cada medio segundo de juego).
    pub(crate) fn epidemic_step(&mut self) -> Vec<String> {
        if self.services.epidemic.infected.is_empty() {
            return Vec::new();
        }
        let mult = self.economy.epidemic_factor();
        let mut new_cases = Vec::new();
        let ids: Vec<u32> = self.services.epidemic.infected.keys().copied().collect();
        for id in ids {
            let Some(l) = self.lots.lots.get(&id).filter(|l| l.level > 0) else {
                self.services.epidemic.infected.remove(&id);
                continue;
            };
            for oid in self.lots.nearby(l.c) {
                if oid == id || self.services.epidemic.infected.contains_key(&oid) || new_cases.contains(&oid) {
                    continue;
                }
                let o = &self.lots.lots[&oid];
                if o.level == 0 || o.c.distance(l.c) > SPREAD_DIST {
                    continue;
                }
                let w = self.services.water_of(oid);
                let factor = if w < 0.5 { 1.6 } else if w < 0.85 { 1.0 } else { 0.45 };
                if self.services.rng.f() < 0.012 * factor * mult {
                    new_cases.push(oid);
                }
            }
        }
        for id in new_cases {
            self.services.epidemic.infected.insert(id, 0);
        }
        Vec::new()
    }

    /// Una vez por mes: la fiebre se cobra víctimas (algunas casas pierden un nivel), las casas
    /// se recuperan y terminan las cuarentenas (los sanos entran a la ciudad).
    pub(crate) fn epidemic_month(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        let ep = &mut self.services.epidemic;
        let mut released = 0;
        ep.quarantine.retain_mut(|q| {
            q.0 = q.0.saturating_sub(1);
            if q.0 == 0 {
                released += q.1;
                false
            } else {
                true
            }
        });
        if released > 0 {
            if self.poblacion.fundada {
                self.llegados_barco += released;
            } else {
                self.pool = (self.pool + released).min(600);
            }
            news.push(format!("Terminó la cuarentena: {released} pobladores entran sanos a la ciudad"));
        }
        if self.services.epidemic.infected.is_empty() {
            return news;
        }
        let mut dead = 0;
        let mut drops = Vec::new();
        let ids: Vec<u32> = self.services.epidemic.infected.keys().copied().collect();
        let afectados = ids.clone();
        for id in ids {
            let months = self.services.epidemic.infected.get_mut(&id).unwrap();
            *months += 1;
            let m = *months;
            if let Some(l) = self.lots.lots.get(&id) {
                if self.services.rng.f() < 0.18 {
                    dead += 1 + (l.residents() / 6);
                    if l.level > 1 && l.density().is_none_or(|d| l.level > d.levels().0) && self.services.rng.f() < 0.4 {
                        drops.push(super::super::lots::LevelChange { lot: id, level: l.level - 1 });
                    }
                }
            }
            if m >= SICK_MONTHS {
                self.services.epidemic.infected.remove(&id);
            }
        }
        for d in &drops {
            self.lots.lots.get_mut(&d.lot).unwrap().level = d.level;
        }
        // Bajan de nivel sin obra nueva: se dibujan con sus mismos materiales.
        for d in &drops {
            self.revoxelizar(t, d.lot, e);
        }
        if self.poblacion.fundada {
            // Con la población de personas, mueren personas de los hogares de esos solares.
            dead = self.poblacion.choque_epidemia(0.04, Some(&afectados)) as i32;
        } else {
            self.pool = (self.pool - dead).max(0);
        }
        self.services.epidemic.deaths_total += dead;
        if dead > 0 {
            news.push(format!("La fiebre se cobra {dead} víctimas este mes"));
        }
        if self.services.epidemic.infected.is_empty() {
            news.push("Terminó la epidemia".into());
        }
        news
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::town;
    use super::*;

    /// Un barco con fiebre: sin lazareto la enfermedad entra y se propaga; con lazareto los
    /// pasajeros hacen cuarentena y entran sanos después.
    #[test]
    fn la_cuarentena_del_lazareto_frena_la_fiebre_del_barco() {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        city.fast_forward(&t, 10);
        let c = city.plaza.unwrap().0;
        let (people, msg) = city.sick_ship(40, c);
        assert_eq!(people, 40);
        assert!(msg.contains("no hay lazareto"));
        assert!(city.services.epidemic.active());
        let first = city.services.epidemic.infected.len();
        for _ in 0..60 {
            city.epidemic_step();
        }
        assert!(city.services.epidemic.infected.len() > first, "la fiebre se contagia entre vecinos");
        assert!(city.services.attraction() < 1.0, "con epidemia llega menos gente");
        let mut e = Edits::default();
        let pool0 = city.pool;
        for _ in 0..8 {
            city.epidemic_month(&t, &mut e);
        }
        assert!(!city.services.epidemic.active(), "la epidemia termina");
        assert!(city.services.epidemic.deaths_total > 0 || city.pool <= pool0);

        // Con lazareto (en la costa): cuarentena de dos meses, sin contagios.
        let mut city = town(&t);
        city.fast_forward(&t, 10);
        // El lazareto se coloca a mano (en el mapa sintético la costa queda lejos de la plaza).
        city.services.buildings.insert(99, super::super::Building { id: 99, kind: Kind::Lazareto, c: c + Vec2::new(0.0, 900.0), f: Vec2::NEG_Y, served: 0, contaminacion: 0.0, rinde: 1.0, voxels: Vec::new() });
        let (now, msg) = city.sick_ship(40, c);
        assert_eq!(now, 0, "nadie baja del barco");
        assert!(msg.contains("cuarentena"));
        assert!(!city.services.epidemic.active());
        assert_eq!(city.services.epidemic.quarantined_now(), 40);
        let pool = city.pool;
        let n1 = city.epidemic_month(&t, &mut e);
        assert!(n1.is_empty() && city.pool == pool, "primer mes: siguen en cuarentena");
        let n2 = city.epidemic_month(&t, &mut e);
        assert!(n2.iter().any(|n| n.contains("cuarentena")), "{n2:?}");
        // Los sanos entran a la ciudad: a la espera de casa o, con la población de personas, a las
        // llegadas del mes.
        assert_eq!(city.pool + city.llegados_barco, (pool + 40).min(600), "entran sanos");
        assert_eq!(city.services.epidemic.quarantined_now(), 0);
    }
}
