//! Incendios. No son castigos al azar: dependen de cómo está construida la ciudad. Los
//! ranchos de paja se prenden con facilidad y el fuego salta a los vecinos, sobre todo a favor
//! del viento (el de `water.rs`: dirección y velocidad de cada hora). Si hay agua cerca (un
//! pozo, un aljibe o los aguadores), los vecinos forman una cadena de baldes y lo apagan; con
//! serenos la voz de fuego se da enseguida. Si no, la casa se quema y quedan las ruinas.
//! Porte de `scripts/city/fires.gd`.

use super::super::lots::{Lot, ZoneKind};
use super::super::City;
use crate::buildings::Edits;
use crate::terrain::TerrainGen;
use glam::Vec2;
use std::collections::BTreeMap;

/// Techo (y material) de un edificio: lo que decide cuánto arde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Roof {
    /// Rancho de barro y paja: se prende con una chispa.
    Paja,
    /// Techo de teja sobre tirantes de madera.
    Teja,
    /// Azotea de ladrillo: casi no arde.
    Azotea,
    /// Techo de tablas: arde bastante (ver `materiales`).
    Tablas,
    /// Techo de cueros: arde menos que la paja, más que la teja.
    Cuero,
}

impl Roof {
    /// Igniciones por mes y por edificio (estimación de diseño, ver docs/sistemas/servicios.md).
    pub fn ignition_month(self) -> f32 {
        match self {
            Roof::Paja => 0.0015,
            Roof::Teja => 0.0003,
            Roof::Azotea => 0.0001,
            Roof::Tablas => 0.0010,
            Roof::Cuero => 0.0007,
        }
    }
    /// Facilidad para que le salte el fuego de un vecino.
    pub fn catch(self) -> f32 {
        match self {
            Roof::Paja => 1.0,
            Roof::Teja => 0.25,
            Roof::Azotea => 0.08,
            Roof::Tablas => 0.6,
            Roof::Cuero => 0.4,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Roof::Paja => "techo de paja",
            Roof::Teja => "techo de teja",
            Roof::Azotea => "azotea de ladrillo",
            Roof::Tablas => "techo de tablas",
            Roof::Cuero => "techo de cuero",
        }
    }
}

/// Techo de lo que hay en un solar: el de sus materiales (ver `materiales`) o, si no los
/// tiene, el que dibuja el catálogo para su tipo.
pub fn roof_of(l: &Lot) -> Roof {
    use super::super::materiales::Techo;
    if let Some(o) = l.obra.filter(|_| l.level > 0) {
        return match o.techo {
            Techo::Paja => Roof::Paja,
            Techo::Teja => Roof::Teja,
            Techo::Azotea => Roof::Azotea,
            Techo::Tablas => Roof::Tablas,
            Techo::Cuero => Roof::Cuero,
        };
    }
    let pick = (l.seed >> 7) % 100;
    let alta = l.density() == Some(super::super::lots::Density::Alta);
    match (l.kind(), l.level) {
        (_, 0 | 1) => Roof::Paja,
        (Some(ZoneKind::Residencial), 2) if !alta && pick < 45 => Roof::Teja,
        (Some(ZoneKind::Comercial), 2) if pick < 40 => Roof::Teja,
        _ => Roof::Azotea,
    }
}

/// Igniciones por mes y facilidad de contagio de un edificio: el techo y las paredes de sus
/// materiales (las tablas arden; ver `materiales`).
pub fn fuego_de(l: &Lot) -> (f32, f32) {
    match l.obra.filter(|_| l.level > 0) {
        Some(o) => (Roof::Paja.ignition_month() * o.fuego(), o.contagio()),
        None => {
            let r = roof_of(l);
            (r.ignition_month(), r.catch())
        }
    }
}

/// Distancia máxima para que el fuego salte de una casa a otra sin viento (m).
pub const SPREAD_DIST: f32 = 20.0;
/// Pasos a máxima intensidad antes de derrumbarse (cada paso: medio segundo de juego).
pub const BURN_OUT: u32 = 12;
/// Pasos de crecimiento por mes (un mes son 10 s de juego).
pub const STEPS_PER_MONTH: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burn {
    /// Intensidad (0 a 1).
    pub i: f32,
    /// Pasos que lleva a máxima intensidad.
    pub full: u32,
    /// ¿Ya llegaron los vecinos con los baldes?
    pub fought: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Fires {
    pub burning: BTreeMap<u32, Burn>,
    pub destroyed_total: u32,
    pub put_out_total: u32,
}

/// Hacia dónde sopla el viento (vector unitario x este, z sur) a partir de la dirección de
/// la que viene (grados, 0 = norte, 90 = este).
pub fn wind_to(from_deg: f32) -> Vec2 {
    let a = from_deg.to_radians();
    -Vec2::new(a.sin(), -a.cos())
}

/// Probabilidad de que el fuego salte de un solar que arde con intensidad `i` a uno vecino a
/// `d` (vector del que arde al vecino), con viento de `speed` m/s soplando hacia `to`.
pub fn spread_chance(d: Vec2, i: f32, catch: f32, to: Vec2, speed: f32) -> f32 {
    let dist = d.length();
    let along = if dist > 0.01 { (d / dist).dot(to) } else { 0.0 };
    // A favor del viento las chispas llegan más lejos.
    let reach = SPREAD_DIST + along.max(0.0) * speed * 2.0;
    if dist > reach {
        return 0.0;
    }
    let w = (1.0 + 0.8 * along * (speed / 4.0).min(3.0)).max(0.15);
    (0.035 * catch * w * i).min(0.9)
}

impl City {
    /// Enciende un solar (si tiene edificio y no arde ya).
    pub fn ignite(&mut self, id: u32) -> bool {
        let Some(l) = self.lots.lots.get(&id) else { return false };
        if l.level == 0 || self.services.fires.burning.contains_key(&id) {
            return false;
        }
        // Con serenos la voz de alarma se da antes: el fuego empieza más chico.
        let i = if self.services.watched(l.c) { 0.12 } else { 0.2 };
        self.services.fires.burning.insert(id, Burn { i, full: 0, fought: false });
        true
    }

    /// Un paso de fuego: igniciones según el techo, lucha con baldes, propagación con el viento
    /// y casas que se derrumban (quedan en ruinas, en vóxeles).
    pub(crate) fn fire_step(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        let rng = &mut self.services.rng;
        let thatch = self.economy.thatch_fire_factor();
        let plaza = self.lots.plaza_center;
        // Sequía y viento fuerte: la paja seca arde más.
        let dry = if self.economy.in_drought() { 1.8 } else { 1.0 } * (1.0 + (self.water.wind_speed - 4.0).max(0.0) * 0.1);
        let mut new_fires = Vec::new();
        for l in self.lots.lots.values() {
            if l.level == 0 || self.services.fires.burning.contains_key(&l.id) {
                continue;
            }
            let roof = roof_of(l);
            let mut p = fuego_de(l).0 / STEPS_PER_MONTH * dry;
            if l.kind() == Some(ZoneKind::Industrial) {
                p *= 2.0; // fraguas, calderas y hornos
            }
            if roof == Roof::Paja && plaza.is_some_and(|c| c.distance(l.c) < 400.0) {
                p *= thatch;
            }
            if rng.f() < p {
                new_fires.push(l.id);
            }
        }
        for id in new_fires {
            if self.ignite(id) {
                let name = self.lots.lots[&id].name();
                news.push(format!("¡Fuego! Se incendia un {name}"));
            }
        }
        let to = wind_to(self.water.wind_dir);
        let speed = self.water.wind_speed;
        let ids: Vec<u32> = self.services.fires.burning.keys().copied().collect();
        let (mut lost, mut out, mut spread) = (Vec::new(), 0, Vec::new());
        for id in ids {
            let Some(l) = self.lots.lots.get(&id).filter(|l| l.level > 0) else {
                self.services.fires.burning.remove(&id);
                continue;
            };
            let c = l.c;
            let water = self.services.fire_source(c).is_some();
            let watched = self.services.watched(c);
            let b = self.services.fires.burning.get_mut(&id).unwrap();
            if water {
                b.fought = true;
                b.i -= 0.09 + if watched { 0.04 } else { 0.0 };
            } else {
                b.i = (b.i + 0.12 * (1.0 + speed / 10.0) - if watched { 0.03 } else { 0.0 }).min(1.0);
            }
            if b.i <= 0.0 {
                self.services.fires.burning.remove(&id);
                out += 1;
                continue;
            }
            if b.i >= 1.0 {
                b.full += 1;
                if b.full >= BURN_OUT {
                    lost.push(id);
                    continue;
                }
            }
            let i = b.i;
            if i > 0.5 {
                for oid in self.lots.nearby(c) {
                    if oid == id || self.services.fires.burning.contains_key(&oid) {
                        continue;
                    }
                    let o = &self.lots.lots[&oid];
                    if o.level == 0 {
                        continue;
                    }
                    let p = spread_chance(o.c - c, i, fuego_de(o).1, to, speed);
                    if p > 0.0 && self.services.rng.f() < p {
                        spread.push(oid);
                    }
                }
            }
        }
        for id in spread {
            self.ignite(id);
        }
        if out > 0 {
            self.services.fires.put_out_total += out;
            news.push(if out == 1 { "Los vecinos apagaron el fuego con una cadena de baldes".into() } else { format!("Los vecinos apagaron {out} fuegos con cadenas de baldes") });
        }
        for &id in &lost {
            self.services.fires.burning.remove(&id);
            self.services.fires.destroyed_total += 1;
            if self.poblacion.fundada {
                self.poblacion.choque_incendio(id);
            }
            self.ruin_edits(t, id, e);
            let l = self.lots.lots.get_mut(&id).unwrap();
            l.level = 0;
            l.age = 0;
            l.obra = None;
            self.services.ruins.insert(id);
            self.services.water.remove(&id);
        }
        if !lost.is_empty() {
            news.push(format!("El fuego destruyó {} {}{}", lost.len(), if lost.len() == 1 { "casa" } else { "casas" }, if self.services.fire_source(self.lots.lots[&lost[0]].c).is_none() { ": no había agua cerca para apagarlo" } else { "" }));
        }
        news
    }

    /// Riesgo de incendio de un solar (0 a 1) para la capa de datos: techo, agua y serenos.
    pub fn fire_risk(&self, l: &Lot) -> f32 {
        if l.level == 0 {
            return 0.0;
        }
        let roof = roof_of(l);
        let mut r: f32 = match roof {
            Roof::Paja => 0.7,
            Roof::Teja => 0.3,
            Roof::Azotea => 0.1,
            Roof::Tablas => 0.5,
            Roof::Cuero => 0.45,
        };
        // Las paredes de tablas también arden.
        if l.obra.is_some_and(|o| o.paredes == super::super::materiales::Pared::Tablas) {
            r += 0.1;
        }
        if self.services.fire_source(l.c).is_none() {
            r += 0.3;
        }
        if self.services.watched(l.c) {
            r -= 0.1;
        }
        r.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{free_site, town};
    use super::super::Kind;
    use super::*;
    use crate::buildings::Rng;

    #[test]
    fn el_viento_lleva_el_fuego_a_sotavento() {
        // Muchas tiradas del mismo vecino a 15 m: a favor y en contra de un viento del oeste.
        let to = wind_to(270.0);
        assert!((to - Vec2::X).length() < 1e-4, "viento del oeste sopla hacia el este: {to}");
        let mut rng = Rng::new(7);
        let (mut east, mut west) = (0, 0);
        for _ in 0..20000 {
            if rng.f() < spread_chance(Vec2::new(15.0, 0.0), 1.0, 1.0, to, 8.0) {
                east += 1;
            }
            if rng.f() < spread_chance(Vec2::new(-15.0, 0.0), 1.0, 1.0, to, 8.0) {
                west += 1;
            }
        }
        assert!(east > west * 3, "a sotavento {east}, a barlovento {west}");
        // Con viento fuerte las chispas alcanzan más lejos a favor del viento.
        assert!(spread_chance(Vec2::new(30.0, 0.0), 1.0, 1.0, to, 8.0) > 0.0);
        assert_eq!(spread_chance(Vec2::new(-30.0, 0.0), 1.0, 1.0, to, 8.0), 0.0);
        // La azotea casi no se prende.
        assert!(spread_chance(Vec2::new(10.0, 0.0), 1.0, Roof::Azotea.catch(), to, 0.0) < spread_chance(Vec2::new(10.0, 0.0), 1.0, Roof::Paja.catch(), to, 0.0) * 0.2);
    }

    /// Ranchos de una cuadra con viento fuerte del oeste: el fuego avanza hacia el este, y
    /// sin agua quema casas que quedan en ruinas; con un aljibe cerca los vecinos lo apagan.
    #[test]
    fn el_fuego_se_propaga_con_el_viento_y_el_agua_lo_apaga() {
        let t = TerrainGen::new(1726);
        let run = |with_water: bool| {
            let mut city = town(&t);
            city.fast_forward(&t, 30);
            city.water.wind_dir = 270.0;
            city.water.wind_speed = 9.0;
            let c = city.plaza.unwrap().0;
            if with_water {
                let (wc, wf) = free_site(&city, &t, Kind::Aljibe, c);
                city.place_service(&t, Kind::Aljibe, wc, wf).unwrap();
            }
            // Arde el rancho con más vecinos edificados al este y al oeste (en el centro de una
            // hilera que corre a lo largo del viento).
            let built: Vec<&Lot> = city.lots.lots.values().filter(|l| l.level > 0).collect();
            let start = (*built.iter().max_by_key(|l| (built.iter().filter(|o| o.c.distance(l.c) < 16.0 && (o.c.y - l.c.y).abs() < 4.0).count(), -(l.c.distance(c) as i32))).unwrap()).clone();
            assert!(city.ignite(start.id));
            let mut e = Edits::default();
            let mut ever = std::collections::BTreeSet::new();
            for _ in 0..80 {
                ever.extend(city.services.fires.burning.keys().copied());
                city.fire_step(&t, &mut e);
            }
            let caught: Vec<Vec2> = ever.iter().filter(|&&id| id != start.id).filter_map(|id| city.lots.lots.get(id)).map(|l| l.c - start.c).collect();
            (city, caught, e, start)
        };
        let (dry, caught, e, start) = run(false);
        assert!(!caught.is_empty(), "sin agua el fuego salta a los vecinos");
        let east = caught.iter().filter(|d| d.x > 1.0).count();
        let west = caught.iter().filter(|d| d.x < -1.0).count();
        assert!(east >= west, "a favor del viento (este) {east}, en contra {west}");
        assert!(dry.services.fires.destroyed_total > 0, "sin agua se quema alguna casa");
        assert!(dry.services.ruins.contains(&start.id) || !dry.services.ruins.is_empty());
        assert!(!e.design.is_empty(), "las ruinas cambian los vóxeles");
        let ruin = *dry.services.ruins.iter().next().unwrap();
        assert_eq!(dry.lots.lots[&ruin].level, 0);
        assert!(!dry.lots.lots[&ruin].voxels.is_empty(), "las ruinas quedan en el solar");
        let (wet, _, _, _) = run(true);
        assert!(wet.services.fires.put_out_total > 0, "con agua los vecinos lo apagan");
        assert!(wet.services.fires.destroyed_total < dry.services.fires.destroyed_total, "con agua se quema menos ({} vs {})", wet.services.fires.destroyed_total, dry.services.fires.destroyed_total);
    }
}
