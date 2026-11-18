//! Rehabilitar y modernizar las casas viejas en el lugar (GDD §4.2: la ciudad con memoria).
//!
//! En la Ciudad Nueva las casas coloniales no se derriban: sus dueños las modernizan cuando
//! tienen con qué. Tres reformas:
//! - **Fachada**: misma planta, misma huella, mismos vanos; se revoca (si era de ladrillo o
//!   piedra a la vista), se le ponen pilastras, cornisa, pretil con balaustres, rejas de hierro
//!   fundido y persianas, con los colores de la época (ver `structures::estilo`, `reforma`).
//! - **Techo de teja**: la paja o el cuero se cambian por teja (menos fuego).
//! - **Altos**: un piso más (la obra se rehace con el kit de la época).
//!
//! Las hacen los dueños solos (los hogares acomodados y principales, si hay cal y hierro en
//! el depósito) a un ritmo lento, así parte del caserío viejo queda: en la misma calle se
//! mezclan casas de las dos épocas. La **ordenanza de fachadas** las acelera en el centro; el
//! jugador también puede pagarlas desde la ficha de la casa.

use super::super::economy::Ordinance;
use super::super::lots::{zone_of, Density, ZoneKind};
use super::super::materiales::ciudad::Deposito;
use super::super::materiales::{Almacenable, Material, Pared, Techo};
use super::super::poblacion::{Clase, Epoca};
use super::super::City;
use crate::buildings::Edits;
use crate::terrain::TerrainGen;
use glam::IVec3;

/// Las reformas que se pueden hacer en una casa vieja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reforma {
    Fachada,
    Techo,
    Altos,
}

impl Reforma {
    pub const ALL: [Reforma; 3] = [Reforma::Fachada, Reforma::Techo, Reforma::Altos];
    pub fn nombre(self) -> &'static str {
        match self {
            Reforma::Fachada => "Modernizar la fachada",
            Reforma::Techo => "Techo de teja",
            Reforma::Altos => "Agregar un piso",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Reforma::Fachada => "Misma planta y misma huella: revoque, pilastras, cornisa, balaustres, rejas de hierro fundido y persianas de la Ciudad Nueva",
            Reforma::Techo => "Cambia la paja o el cuero por teja: menos incendios y más años",
            Reforma::Altos => "Se levanta un piso más con el estilo de la época (si los materiales alcanzan)",
        }
    }
}

/// Cal y hierro que pide una fachada nueva, y tejas de un techo [E].
pub const CAL_FACHADA: f32 = 4.0;
pub const HIERRO_FACHADA: f32 = 1.5;
pub const TEJA_TECHO: f32 = 12.0;
/// Casas que los dueños modernizan solos como mucho por mes.
pub const MAX_POR_MES: usize = 6;

impl City {
    /// Costo en pesos de una reforma en un solar, o el motivo por el que no se puede.
    pub fn reforma_posible(&self, id: u32, r: Reforma) -> Result<i64, String> {
        if !self.en_epoca(Epoca::CiudadNueva) {
            return Err("Las reformas llegan con la época «Ciudad Nueva»".into());
        }
        let l = self.lots.lots.get(&id).ok_or("No hay casa ahí")?;
        let (k, d) = zone_of(l.zone).ok_or("Solar sin zona")?;
        if l.level == 0 || k == ZoneKind::Industrial {
            return Err("Solo las casas y los comercios".into());
        }
        let o = self.obra_de(l).ok_or("Sin obra")?;
        match r {
            Reforma::Fachada => {
                if o.contexto.epoca >= 1 || o.contexto.reforma > 0 {
                    return Err("La fachada ya es de la Ciudad Nueva".into());
                }
                if l.level < 2 || !o.paredes.admite_revoque() || o.paredes == Pared::Tablas {
                    return Err("Un rancho o una casa de tablas no admite fachada nueva".into());
                }
                if matches!(o.techo, Techo::Paja | Techo::Cuero) {
                    return Err("Primero, el techo de teja".into());
                }
                Ok(180 + 60 * l.level as i64)
            }
            Reforma::Techo => {
                if !matches!(o.techo, Techo::Paja | Techo::Cuero) {
                    return Err("Ya tiene techo de teja o azotea".into());
                }
                Ok(90)
            }
            Reforma::Altos => {
                let (_, last) = d.levels();
                if l.level >= last || (k == ZoneKind::Residencial && d == Density::Baja) {
                    return Err("La zona no admite más pisos".into());
                }
                if l.level < 2 {
                    return Err("Primero, una casa de material".into());
                }
                Ok(700)
            }
        }
    }

    /// Hace una reforma (paga la ciudad si `ciudad`, si no el dueño). Devuelve los vóxeles.
    pub fn rehabilitar(&mut self, t: &TerrainGen, id: u32, r: Reforma, ciudad: bool) -> Result<Edits, String> {
        let costo = self.reforma_posible(id, r)?;
        if ciudad && costo > self.money {
            return Err(format!("Fondos insuficientes: hacen falta {costo} pesos"));
        }
        let mut e = Edits::default();
        let anio = self.anio();
        match r {
            Reforma::Fachada => {
                let l = &self.lots.lots[&id];
                let mut o = self.obra_de(l).unwrap();
                let mut dep = Deposito { eco: &mut self.economy, mat: &mut self.materiales };
                dep.tomar(Material::Cal, CAL_FACHADA);
                let hierro = dep.tomar(Material::Hierro, HIERRO_FACHADA) >= HIERRO_FACHADA * 0.99;
                // La planta se sigue dibujando con los materiales con que se levantó; la
                // fachada nueva se revoca (ver `structures::estilo`, `reforma`).
                o.contexto.antes = u8::from(!o.revoque) | (u8::from(!o.rejas) << 1);
                o.revoque = true;
                o.rejas = o.rejas || hierro;
                o.contexto.reforma = anio;
                o.estado = 1.0;
                self.lots.lots.get_mut(&id).unwrap().obra = Some(o);
                self.revoxelizar(t, id, &mut e);
            }
            Reforma::Techo => {
                let l = &self.lots.lots[&id];
                let mut o = self.obra_de(l).unwrap();
                Deposito { eco: &mut self.economy, mat: &mut self.materiales }.tomar(Material::Teja, TEJA_TECHO);
                o.techo = Techo::Teja;
                self.lots.lots.get_mut(&id).unwrap().obra = Some(o);
                self.revoxelizar(t, id, &mut e);
            }
            Reforma::Altos => {
                let nivel = self.lots.lots[&id].level + 1;
                self.lots.lots.get_mut(&id).unwrap().level = nivel;
                self.apply_levels(t, &[super::super::lots::LevelChange { lot: id, level: nivel }], &mut e);
                self.poblacion.mejora_hecha(id, nivel);
            }
        }
        if ciudad {
            self.money -= costo;
        }
        self.epoca.rehabilitadas += 1;
        Ok(e)
    }

    /// Los dueños con medios modernizan sus casas viejas, de a poco (y más rápido con la
    /// ordenanza de fachadas en el centro). Sin cal no hay fachada nueva.
    pub(crate) fn rehabilitar_mes(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let ordenanza = self.economy.is_on(Ordinance::Fachadas);
        let mut cands: Vec<(u32, Reforma)> = Vec::new();
        for l in self.lots.lots.values().filter(|l| l.level >= 1) {
            let Some(o) = l.obra else { continue };
            if o.contexto.epoca >= 1 {
                continue;
            }
            let rico = self.poblacion.hogares_de_lote(l.id).any(|h| h.activo && matches!(h.clase, Clase::Acomodada | Clase::Principal));
            let centro = self.lugar_de(l.c) <= 1;
            let reforma = if matches!(o.techo, Techo::Paja | Techo::Cuero) { Reforma::Techo } else { Reforma::Fachada };
            if self.reforma_posible(l.id, reforma).is_err() {
                continue;
            }
            // Probabilidad por mes [E]: los ricos, un 3 %; los demás, 0,5 %; la ordenanza
            // multiplica por 4 en el centro.
            let mut p = if rico { 0.03 } else { 0.005 };
            if ordenanza && centro {
                p *= 4.0;
            }
            if crate::structures::hash(IVec3::new(l.id as i32, self.month as i32, 11), 0xFAC4DA) < p {
                cands.push((l.id, reforma));
            }
        }
        let mut hechas = [0u32; 2];
        for (id, r) in cands.into_iter().take(MAX_POR_MES) {
            if r == Reforma::Fachada && self.hay_material(Material::Cal) < CAL_FACHADA {
                continue;
            }
            if let Ok(x) = self.rehabilitar(t, id, r, false) {
                e.extend(x);
                hechas[usize::from(r == Reforma::Techo)] += 1;
            }
        }
        let mut news = Vec::new();
        if hechas[0] > 0 {
            news.push(format!("{} {} con fachada nueva: pilastras, cornisas y rejas de hierro en las casas viejas", hechas[0], if hechas[0] == 1 { "casa vieja amanece" } else { "casas viejas amanecen" }));
        }
        if hechas[1] > 0 {
            news.push(format!("{} {} la paja por teja", hechas[1], if hechas[1] == 1 { "rancho cambia" } else { "ranchos cambian" }));
        }
        news
    }
}
