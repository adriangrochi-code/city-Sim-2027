//! Las redes de la Ciudad Nueva: gas de alumbrado, agua corriente y colectores.
//!
//! - **Gas** (Montevideo, 1853: la usina de los hermanos Ísola en la costa norte de la bahía y
//!   los primeros faroles en la Plaza Constitución [F]): la usina destila carbón de piedra
//!   importado y lo reparte por cañerías bajo las calles de su red, hasta su alcance. Cada
//!   calle de la red lleva faroles de gas cada 25 m, que reemplazan a los de sebo de los
//!   serenos: más luz (de noche se ven los charcos ámbar al pie de cada farol), menos delito y
//!   un costo por mes en carbón.
//! - **Agua corriente** (18 de julio de 1871: el agua del río Santa Lucía llega a una fuente de
//!   la Plaza Constitución [F]): la usina de agua bombea a los solares de su red de calles;
//!   agua buena para todos (se reparte en `services::distribute_water`).
//! - **Colectores** (contrato de Arteaga de 1854, ley de saneamiento de 1856, primeras obras en
//!   1856–57: la primera red de colectores de América Latina [F]): una boca de desagüe en la
//!   costa recoge las aguas servidas de los solares de su red; los pozos negros dejan de
//!   ensuciar los pozos de agua y bajan la insalubridad.

use super::super::services::{comp_at, comps_cerca, components, Kind};
use super::super::voxelize::road_height;
use super::super::City;
use crate::buildings::Edits;
use crate::lod::{BASE_VOXEL, DESIGN_VOXEL};
use crate::palette::AIR;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};
use std::collections::{BTreeSet, HashSet};

/// Un farol de gas cada tantos metros de calle (alternando de vereda) [E].
pub const PASO_FAROL_GAS: f32 = 25.0;
/// Carbón de piedra por farol de gas y por mes, en pesos (importado) [E].
pub const CARBON_POR_FAROL: f32 = 1.5;
/// Una calle con un farol de gas a menos de esto está bien iluminada (m).
pub const RADIO_FAROL_GAS: f32 = 45.0;
/// Alcance de la luz de cada farol en el render (m) y su intensidad (el de sebo, menos).
pub const LUZ_GAS: (f32, f32) = (11.0, 1.0);
pub const LUZ_SEBO: (f32, f32) = (6.0, 0.35);

/// Una usina o la boca de un colector se empalma con todas las calles a menos de esto (m): con
/// la más cercana sola, un colector junto al camino suelto del muelle no servía a nadie.
pub const RADIO_EMPALME: f32 = 60.0;

/// Base del farol: el vóxel de diseño apoyado sobre la calzada.
fn base_calzada(t: &TerrainGen, p: Vec2) -> i32 {
    let top = (road_height(t, p) / BASE_VOXEL - 0.5).floor() as i32;
    (top + 2).div_euclid(2)
}

/// Farol de gas: columna de hierro fundido de 3 m con su base, el farol de vidrio y el
/// sombrerete.
pub fn farol_gas(e: &mut Edits, v: IVec2, base: i32) {
    use crate::palette::{HIERRO_FUNDIDO, LUZ_GAS as VIDRIO};
    e.dfill(IVec3::new(v.x, base, v.y), IVec3::new(v.x + 1, base + 6, v.y + 1), HIERRO_FUNDIDO);
    e.dset(IVec3::new(v.x, base + 6, v.y), VIDRIO);
    e.dset(IVec3::new(v.x, base + 7, v.y), HIERRO_FUNDIDO);
}

impl City {
    /// Faroles de gas del mapa (posición en metros).
    pub fn faroles_gas_m(&self) -> impl Iterator<Item = Vec2> + '_ {
        self.epoca.faroles_gas.iter().map(|&(v, _)| (Vec2::new(v[0] as f32, v[1] as f32) + 0.5) * DESIGN_VOXEL)
    }

    /// Luces de los faroles cerca de un punto (para el render de noche): posición (x, y, z),
    /// alcance e intensidad. Los más cercanos primero, hasta `max`.
    pub fn luces_cerca(&self, c: Vec2, radio: f32, max: usize) -> Vec<[f32; 5]> {
        let mut v: Vec<(f32, [f32; 5])> = Vec::new();
        for &(p, b) in &self.epoca.faroles_gas {
            let q = (Vec2::new(p[0] as f32, p[1] as f32) + 0.5) * DESIGN_VOXEL;
            let d = q.distance(c);
            if d < radio {
                v.push((d, [q.x, (b + 6) as f32 * DESIGN_VOXEL + 0.25, q.y, LUZ_GAS.0, LUZ_GAS.1]));
            }
        }
        for &(p, b) in &self.services.lamps {
            let q = (p.as_vec2() + 0.5) * DESIGN_VOXEL;
            let d = q.distance(c);
            if d < radio {
                v.push((d, [q.x, (b + 6) as f32 * DESIGN_VOXEL + 0.25, q.y, LUZ_SEBO.0, LUZ_SEBO.1]));
            }
        }
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.truncate(max);
        v.into_iter().map(|x| x.1).collect()
    }

    /// Firma de lo que decide el reparto de los faroles de gas.
    fn firma_gas(&self) -> (usize, i64, usize) {
        (self.net.edges.len(), self.net.total_length() as i64, self.services.count(Kind::UsinaGas))
    }

    /// Reparte los faroles de gas por las calles de la red de cada usina (hasta su alcance) y
    /// saca los faroles de sebo que quedan a la luz del gas.
    pub fn refresh_gas(&mut self, t: &TerrainGen, e: &mut Edits) {
        let usinas: Vec<(Vec2, f32)> = self.services.of_kind(Kind::UsinaGas).map(|b| (b.c, b.info().radius)).collect();
        let mut out: Vec<([i32; 2], i32)> = Vec::new();
        if !usinas.is_empty() {
            let comp = components(&self.net);
            let fuentes: Vec<(Vec2, f32, Vec<u32>)> = usinas.iter().map(|&(c, r)| (c, r, comps_cerca(&self.net, &comp, c, RADIO_EMPALME))).collect();
            let mut seen = HashSet::new();
            for ed in self.net.edges.values() {
                let ce = comp.get(&ed.a).copied();
                let hw = ed.kind.width() * 0.5;
                let mut s = 4.0;
                let mut lado = 1.0;
                while s < ed.len() - 3.0 {
                    let (p, tan) = ed.point(s);
                    if fuentes.iter().any(|(c, r, k)| ce.is_some_and(|ce| k.contains(&ce)) && p.distance(*c) <= *r) {
                        let q = p + Vec2::new(-tan.y, tan.x) * lado * (hw - 0.75);
                        let v = IVec2::new((q.x / DESIGN_VOXEL).floor() as i32, (q.y / DESIGN_VOXEL).floor() as i32);
                        if self.net.clearance(q, 20.0) > -1.2 && seen.insert(v / 10) {
                            out.push(([v.x, v.y], base_calzada(t, q)));
                        }
                    }
                    s += PASO_FAROL_GAS;
                    lado = -lado;
                }
            }
        }
        let viejos: HashSet<([i32; 2], i32)> = self.epoca.faroles_gas.iter().copied().collect();
        let nuevos: HashSet<([i32; 2], i32)> = out.iter().copied().collect();
        for &(v, b) in viejos.difference(&nuevos) {
            for y in b..b + 8 {
                e.dset(IVec3::new(v[0], y, v[1]), AIR);
            }
        }
        for &(v, b) in &out {
            farol_gas(e, IVec2::new(v[0], v[1]), b);
        }
        self.epoca.faroles_gas = out;
        self.epoca.gas_firma = self.firma_gas();
        self.quitar_sebo_bajo_gas(e);
    }

    /// Los faroles de sebo a la luz de uno de gas se sacan (los faroleros reemplazan a los
    /// serenos en esas calles).
    pub(crate) fn quitar_sebo_bajo_gas(&mut self, e: &mut Edits) {
        if self.epoca.faroles_gas.is_empty() {
            return;
        }
        let gas: Vec<Vec2> = self.faroles_gas_m().collect();
        let mut quedan = Vec::new();
        for &(v, b) in &self.services.lamps {
            let q = (v.as_vec2() + 0.5) * DESIGN_VOXEL;
            if gas.iter().any(|g| g.distance(q) < 18.0) {
                for y in b..b + 8 {
                    e.dset(IVec3::new(v.x, y, v.y), AIR);
                }
            } else {
                quedan.push((v, b));
            }
        }
        self.services.lamps = quedan;
    }

    /// Solares de la red de calles de los edificios de un tipo, dentro de su alcance (agua
    /// corriente, colectores).
    pub fn cobertura_red(&self, k: Kind) -> BTreeSet<u32> {
        let fuentes: Vec<(Vec2, f32)> = self.services.of_kind(k).map(|b| (b.c, b.info().radius)).collect();
        if fuentes.is_empty() {
            return BTreeSet::new();
        }
        let comp = components(&self.net);
        let ks: Vec<(Vec2, f32, Vec<u32>)> = fuentes.iter().map(|&(c, r)| (c, r, comps_cerca(&self.net, &comp, c, RADIO_EMPALME))).collect();
        self.lots
            .lots
            .values()
            .filter(|l| l.level > 0)
            .filter(|l| {
                let frente = comp_at(&self.net, &comp, l.c + l.f * (super::super::lots::D * 0.5 + 3.0), 14.0);
                ks.iter().any(|(c, r, kk)| frente.is_some_and(|f| kk.contains(&f)) && c.distance(l.c) <= *r)
            })
            .map(|l| l.id)
            .collect()
    }

    /// Recalcula lo derivado de las redes (agua corriente y colectores).
    pub fn refresh_redes(&mut self) {
        self.epoca.cloaca = self.cobertura_red(Kind::Colector);
        let s = &self.services;
        self.epoca.agua_red = s.water.iter().filter(|(_, &(_, src))| s.buildings.get(&src).is_some_and(|b| b.kind == Kind::AguaCorriente)).map(|(&id, _)| id).collect();
    }

    /// El mes de las redes: faroles de gas al día con las calles y las usinas.
    pub(crate) fn redes_mes(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        if self.epoca.gas_firma != self.firma_gas() {
            let antes = self.epoca.faroles_gas.len();
            self.refresh_gas(t, e);
            let n = self.epoca.faroles_gas.len();
            if antes == 0 && n > 0 {
                news.push(format!("Se encienden los primeros {n} faroles de gas: la noche se ilumina por primera vez y los faroleros reemplazan a los serenos"));
            }
        }
        news
    }

    /// Carbón de piedra importado para la usina de gas (pesos por mes).
    pub fn costo_carbon(&self) -> i64 {
        (self.epoca.faroles_gas.len() as f32 * CARBON_POR_FAROL).round() as i64
    }
}
