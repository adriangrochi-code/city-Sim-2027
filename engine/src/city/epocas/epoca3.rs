//! La época 3, Inmigración y Belle Époque (≈1870–1930; `docs/epocas/03-inmigracion.md`): el
//! paso desde la Ciudad Nueva, la decisión del ferrocarril, las oleadas de inmigrantes, los
//! conventillos y lo que cobra y da la estación.
//!
//! - **Paso de época** (`docs/epocas/02-ciudad-nueva.md` §12: "teatro + llegada del
//!   ferrocarril → estación central"): 3.000 habitantes [E], el teatro, el ensanche trazado y
//!   el acontecimiento de la **llegada del ferrocarril**. En el modo fiel llega entre 1867 y
//!   1869 (el primer tren corre el 1 de enero de 1869 entre Bella Vista y Las Piedras, y llega a
//!   Montevideo en 1871 [F]); en historia alternativa, con el teatro y 2.500 habitantes, 1/36
//!   por mes.
//! - **La decisión**: dar la línea en **concesión** a una compañía de ultramar (paga un canon
//!   ahora y levanta la estación; los pasajes son suyos) o **construirla con dinero propio**
//!   (la estación se paga y los pasajes entran al tesoro). Guiño al ferrocarril de capital
//!   británico (Central Uruguay Railway, 1878 [F]).
//! - **Inmigración**: la población llega con la tabla de la época 3 (`poblacion::origenes`);
//!   el hotel de inmigrantes, la estación y el puerto moderno traen más gente
//!   (`Services::attraction`), y las **oleadas** (la *crisis en la Península*) multiplican las
//!   llegadas unos meses.
//! - **Conventillos**: si falta vivienda barata, las casas viejas del centro y del puerto se
//!   dividen en piezas (misma fachada, más familias, más insalubridad). El jugador puede
//!   **inspeccionarlos** (menos conversiones, un costo por mes) o dejarlos.
//!
//! El alumbrado eléctrico y los tranvías están en [`super::tranvias`].

use super::super::lots::{zone_code, zone_of, Density, ZoneKind};
use super::super::poblacion::Epoca;
use super::super::services::Kind;
use super::super::{City, START_YEAR};
use super::{tirada, Requisito};
use crate::buildings::Edits;
use crate::terrain::TerrainGen;
use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Habitantes que pide el paso a la época 3 [E].
pub const HABITANTES_EPOCA3: i32 = 3000;
/// En historia alternativa, el ferrocarril puede llegar con estos habitantes y el teatro [E].
pub const HABITANTES_FERROCARRIL: i32 = 2500;
/// Años de la llegada del ferrocarril en el modo fiel (el último: llega seguro) [F: 1869].
pub const FERROCARRIL_DESDE: u32 = 1867;
pub const FERROCARRIL_HASTA: u32 = 1869;
/// Canon que paga la compañía de ultramar por la concesión (pesos, una vez) [E].
pub const CANON_CONCESION: i64 = 25_000;
/// Pasajes por habitante y por mes de la línea propia (pesos) [E].
pub const PASAJE_POR_HABITANTE: f32 = 0.04;
/// Sin casa (o personas esperando lugar) desde donde empiezan los conventillos [E].
pub const DEFICIT_CONVENTILLO: u32 = 40;
/// Costo por mes de la inspección de conventillos (pesos) [E].
pub const COSTO_INSPECCION: i64 = 120;
/// Una oleada dura estos meses y multiplica las llegadas por esto [E].
pub const OLEADA_MESES: u32 = 8;
pub const OLEADA_FACTOR: f32 = 1.8;

/// Qué se hace con el ferrocarril al pasar a la época 3.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ferrocarril {
    /// Concesión a una compañía de ultramar: canon ahora, la estación gratis, los pasajes son
    /// de la compañía (poco control después).
    Concesion,
    /// Con dinero propio: la estación se paga, los pasajes entran al tesoro.
    Propio,
}

impl Ferrocarril {
    pub const ALL: [Ferrocarril; 2] = [Ferrocarril::Concesion, Ferrocarril::Propio];
    pub fn nombre(self) -> &'static str {
        match self {
            Ferrocarril::Concesion => "Dar la línea en concesión",
            Ferrocarril::Propio => "Construirla con dinero propio",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Ferrocarril::Concesion => "Una compañía de ultramar tiende la línea y levanta la estación: paga un canon ahora (25.000 pesos) y se queda con los pasajes. Ingresos ahora, poco control después.",
            Ferrocarril::Propio => "La ciudad paga la estación (30.000 pesos) y cobra los pasajes cada mes. Más caro al principio, más tuyo después.",
        }
    }
}

impl City {
    /// Requisitos para pasar a la época 3.
    pub fn requisitos_epoca3(&self) -> Vec<Requisito> {
        let hab = self.habitantes();
        vec![
            Requisito { texto: format!("{} habitantes (hay {})", super::super::game::fmt_thousands(HABITANTES_EPOCA3 as i64), super::super::game::fmt_thousands(hab as i64)), cumplido: hab >= HABITANTES_EPOCA3 },
            Requisito { texto: "Teatro levantado".into(), cumplido: self.services.count(Kind::Teatro) > 0 },
            Requisito { texto: "Ensanche trazado".into(), cumplido: !self.epoca.ensanche.trazados.is_empty() },
            Requisito {
                texto: match self.epoca.ferrocarril {
                    Some(m) => format!("Llegada del ferrocarril (desde {} de {})", super::super::MONTHS[(m % 12) as usize], START_YEAR + m / 12),
                    None => "Acontecimiento: la llegada del ferrocarril".into(),
                },
                cumplido: self.epoca.ferrocarril.is_some(),
            },
        ]
    }

    /// El acontecimiento de la llegada del ferrocarril (en la Ciudad Nueva).
    pub(crate) fn ferrocarril_mes(&mut self) -> Option<String> {
        if self.epoca.actual != Epoca::CiudadNueva || self.epoca.ferrocarril.is_some() {
            return None;
        }
        let anio = START_YEAR + self.month / 12;
        let llega = if self.poblacion.fiel_historia {
            anio >= FERROCARRIL_HASTA || (anio >= FERROCARRIL_DESDE && tirada(self.month, 0xFE77) < 1.0 / 12.0)
        } else {
            self.services.count(Kind::Teatro) > 0 && self.habitantes() >= HABITANTES_FERROCARRIL && tirada(self.month, 0xFE78) < 1.0 / 36.0
        };
        if !llega {
            return None;
        }
        self.epoca.ferrocarril = Some(self.month);
        Some("Llega el ferrocarril: una compañía de ultramar ofrece tender la línea hasta la ciudad. Hay que decidir si se le da en concesión o si se construye con dinero propio (panel de la época)".into())
    }

    /// Pasa a la época 3 con la decisión del ferrocarril. Con `forzar` no se miran los
    /// requisitos (demostración y pruebas).
    pub fn pasar_epoca3(&mut self, _t: &TerrainGen, fc: Ferrocarril, forzar: bool) -> Result<(Edits, String), String> {
        if self.epoca.actual != Epoca::CiudadNueva {
            return Err("La época 3 sigue a la Ciudad Nueva".into());
        }
        if !forzar && !self.puede_pasar_epoca() {
            let faltan: Vec<String> = self.requisitos_epoca().into_iter().filter(|r| !r.cumplido).map(|r| r.texto).collect();
            return Err(format!("Todavía no: {}", faltan.join("; ")));
        }
        let mut res = self.resumen_epoca();
        res.lineas.push(format!("El ferrocarril: {}", fc.nombre().to_lowercase()));
        self.epoca.resumenes.push(res);
        self.epoca.mostrar_resumen = true;
        self.epoca.actual = Epoca::Inmigracion;
        self.epoca.desde_mes = self.month;
        self.epoca.concesion = Some(fc);
        if self.epoca.ferrocarril.is_none() {
            self.epoca.ferrocarril = Some(self.month);
        }
        self.poblacion.epoca = Epoca::Inmigracion;
        let mut aviso = format!("Comienza la época «{}»: llegan los barcos de inmigrantes (italianos, españoles y de todas partes), y se pueden levantar la estación, la usina eléctrica, los tranvías, el hotel de inmigrantes y el puerto moderno", Epoca::Inmigracion.nombre());
        if fc == Ferrocarril::Concesion {
            self.money += CANON_CONCESION;
            aviso.push_str(&format!(". La compañía del ferrocarril paga el canon: {} pesos", super::super::game::fmt_thousands(CANON_CONCESION)));
        }
        self.poblacion.anotar(self.month as i32, format!("Fin de la Ciudad Nueva. {}", aviso));
        Ok((Edits::default(), aviso))
    }

    /// Costo de levantar un servicio para la ciudad (con la concesión, la estación la paga la
    /// compañía).
    pub fn costo_servicio(&self, k: Kind) -> i64 {
        if k == Kind::Estacion && self.epoca.concesion == Some(Ferrocarril::Concesion) {
            0
        } else {
            super::super::services::info(k).cost
        }
    }

    /// Pasajes de la línea propia por mes (pesos).
    pub fn pasajes(&self) -> i64 {
        if self.epoca.concesion == Some(Ferrocarril::Propio) && self.services.count(Kind::Estacion) > 0 {
            (self.habitantes() as f32 * PASAJE_POR_HABITANTE).round() as i64
        } else {
            0
        }
    }

    /// Las llegadas del mes, multiplicadas en una oleada.
    pub fn factor_inmigracion(&self) -> f32 {
        if self.epoca.oleada_hasta.is_some_and(|m| self.month < m) {
            OLEADA_FACTOR
        } else {
            1.0
        }
    }

    /// El mes de la época 3: oleadas, conventillos, pasajes, inspección, alumbrado y tranvías.
    pub(crate) fn epoca3_mes(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        let anio = START_YEAR + self.month / 12;
        // Oleadas: en el modo fiel, sobre todo en las décadas de 1880 y de 1905–1913 (los
        // picos de la inmigración europea al Río de la Plata [F: conocimiento general]); la
        // frecuencia es [E].
        if self.epoca.oleada_hasta.is_none_or(|m| self.month >= m + 24) {
            let p = if !self.poblacion.fiel_historia || (1880..1890).contains(&anio) || (1905..1914).contains(&anio) { 1.0 / 30.0 } else { 1.0 / 90.0 };
            if tirada(self.month, 0x01EA) < p {
                self.epoca.oleada_hasta = Some(self.month + OLEADA_MESES);
                news.push("Crisis en la Península: llega una oleada de barcos con inmigrantes. Hace falta vivienda barata, o las casas viejas se llenarán de piezas de alquiler".into());
            }
        }
        let pas = self.pasajes();
        if pas > 0 {
            self.money += pas;
        }
        if self.epoca.inspeccion {
            self.money -= COSTO_INSPECCION;
        }
        news.extend(self.conventillos_mes(t, e));
        news.extend(self.tranvias_mes(t, e));
        news
    }

    /// Falta de vivienda: personas sin casa (o que esperan lugar en la ciudad).
    pub fn deficit_vivienda(&self) -> u32 {
        if self.poblacion.fundada {
            self.poblacion.stats.sin_casa
        } else {
            self.pool.max(0) as u32
        }
    }

    /// Casas viejas que se dividen en piezas si falta vivienda (hasta 3 por mes; con
    /// inspección o con el hotel de inmigrantes, menos).
    pub(crate) fn conventillos_mes(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let deficit = self.deficit_vivienda();
        let mut umbral = DEFICIT_CONVENTILLO;
        if self.services.count(Kind::HotelInmigrantes) > 0 {
            umbral *= 2;
        }
        if deficit < umbral {
            return Vec::new();
        }
        let mut n = ((deficit - umbral) / 50 + 1).min(3);
        if self.epoca.inspeccion {
            if tirada(self.month, 0x1E5) > 0.25 {
                return Vec::new();
            }
            n = 1;
        }
        let anio = self.anio();
        let plaza = self.lots.plaza_center.unwrap_or_default();
        let puerto = self.services.port.pier.as_ref().map(|p| p.land);
        let mut cand: Vec<(f32, u32)> = self
            .lots
            .lots
            .values()
            .filter(|l| l.level >= 2 && matches!(zone_of(l.zone), Some((ZoneKind::Residencial, Density::Baja | Density::Media))))
            .filter_map(|l| {
                let o = l.obra?;
                let c = &o.contexto;
                // Casas viejas de familias modestas, en el centro o junto al puerto.
                let vieja = c.conventillo == 0 && c.clase <= 1 && (c.epoca == 0 || c.anio > 0 && c.anio + 25 <= anio);
                if !vieja {
                    return None;
                }
                let d = l.c.distance(plaza).min(puerto.map_or(f32::MAX, |p| p.distance(l.c)));
                Some((d, l.id))
            })
            .collect();
        cand.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut hechos = 0;
        for &(_, id) in cand.iter().take(n as usize) {
            self.convertir_conventillo(t, id, e);
            hechos += 1;
        }
        if hechos > 0 && self.epoca.conventillos == hechos {
            return vec![format!("Falta vivienda barata: una casa vieja del centro se divide en piezas de alquiler, una familia por pieza y un solo patio. Es el primer conventillo de la ciudad")];
        }
        Vec::new()
    }

    /// Convierte una casa en conventillo: más familias (densidad alta), misma fachada.
    pub fn convertir_conventillo(&mut self, t: &TerrainGen, id: u32, e: &mut Edits) {
        let anio = self.anio();
        let Some(l) = self.lots.lots.get_mut(&id) else { return };
        let Some(mut o) = l.obra else { return };
        o.contexto.conventillo = anio;
        l.obra = Some(o);
        l.zone = zone_code(ZoneKind::Residencial, Density::Alta);
        self.epoca.conventillos += 1;
        self.revoxelizar(t, id, e);
    }

    /// Conventillos que hay ahora en la ciudad.
    pub fn conventillos(&self) -> usize {
        self.lots.lots.values().filter(|l| l.level > 0 && l.obra.is_some_and(|o| o.contexto.conventillo > 0)).count()
    }

    /// Lo que sube el valor del suelo de la época 3: la estación y las líneas de tranvía [E].
    pub fn valor_epoca3(&self, p: Vec2) -> f32 {
        if !self.en_epoca(Epoca::Inmigracion) {
            return 0.0;
        }
        let est = self.services.of_kind(Kind::Estacion).map(|b| 0.08 * (-p.distance(b.c) / 400.0).exp()).fold(0.0f32, f32::max);
        let tr = self.distancia_tranvia(p).map_or(0.0, |d| if d < super::tranvias::RADIO_TRANVIA { 0.07 * (1.0 - d / super::tranvias::RADIO_TRANVIA) } else { 0.0 });
        est + tr
    }

    /// Líneas de la ficha de los servicios de la época 3.
    pub fn lineas_epoca3(&self, k: Kind) -> Vec<String> {
        match k {
            Kind::Estacion => vec![
                match self.epoca.concesion {
                    Some(Ferrocarril::Concesion) => "Línea en concesión: los pasajes son de la compañía".into(),
                    _ => format!("Línea propia: {} pesos de pasajes por mes", self.pasajes()),
                },
                "Sube el valor del suelo alrededor y trae familias y comercio".into(),
            ],
            Kind::UsinaElectrica => vec![
                format!("Faroles eléctricos: {}", self.epoca.faroles_electricos.len()),
                format!("Tranvías eléctricos: {} de {} líneas", self.epoca.tranvias.iter().filter(|l| l.electrica).count(), self.epoca.tranvias.len()),
            ],
            Kind::CocherasTranvia => vec![
                format!("Líneas: {} ({} km)", self.epoca.tranvias.len(), (self.epoca.tranvias.iter().map(|l| l.largo()).sum::<f32>() / 100.0).round() / 10.0),
                format!("Tracción: {}", if self.epoca.tranvias.iter().any(|l| l.electrica) { "eléctrica" } else { "a caballo (eléctrica con una usina)" }),
                format!("Solares a menos de {} m de una línea: {}", super::tranvias::RADIO_TRANVIA as i32, self.solares_con_tranvia()),
            ],
            Kind::HotelInmigrantes => vec![
                format!("Sin casa: {} · conventillos: {}", self.deficit_vivienda(), self.conventillos()),
                "Más llegadas por el puerto y menos casas que se vuelven conventillo".into(),
            ],
            Kind::PuertoModerno => vec!["Los barcos grandes atracan en el muelle: más comercio y más llegadas".into()],
            _ => Vec::new(),
        }
    }
}
