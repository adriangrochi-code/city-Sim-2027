//! Escuelas, salud, saneamiento y seguridad de los barrios (época colonial). Todo se calcula
//! por solar y llega a la población por [`crate::city::poblacion::Entorno::servicios`]:
//!
//! - **Educación**: cada escuela (o el convento) tiene lugar para tantos alumnos de 6 a 12
//!   años; se reparte, como el agua, a las casas más cercanas dentro de su alcance hasta llenarse.
//!   Cuentan todos los niños de esa edad. La población decide quién va según la clase del
//!   hogar y si el cabildo paga al maestro (ordenanza de escuelas).
//! - **Salud**: botica, médico y cirujano y hospital de caridad atienden a tantas personas a su
//!   alrededor (el médico y el hospital, por la red de calles). Baja la mortalidad (más la de
//!   los niños) y sube la salud.
//! - **Saneamiento**: cada casa tiene su letrina o pozo negro (peor el del rancho); filtran a
//!   los pozos de agua cercanos, que se van contaminando mes a mes, y la basura que nadie junta
//!   forma basurales en los baldíos. Agua mala, pozos negros y basurales hacen la
//!   **insalubridad** de cada solar: fiebres y diarreas (sobre todo de los niños) y brotes de
//!   disentería.
//! - **Seguridad**: el riesgo de **delito** de cada solar sale de la pobreza y la desigualdad
//!   del vecindario, la gente sin trabajo, la oscuridad (sin faroles), la densidad, los baldíos y
//!   ruinas y la cercanía del puerto (contrabando); los serenos, el alcalde de barrio y la
//!   cárcel lo bajan. Hay robos, pulperías asaltadas y contrabando en la gaceta; pesa en la
//!   felicidad («seguridad»), en la alcabala y en la opinión de los comerciantes.
//!
//! Los números son **estimaciones de diseño**; la base histórica y las fuentes están en
//! `docs/sistemas/educacion-salud-seguridad.md`.

use super::super::economy::{MonthInput, Ordinance};
use super::super::lots::{zone_of, Density, Lot, ZoneKind};
use super::super::poblacion::origenes::Epoca;
use super::super::voxelize::build_rotated;
use super::super::City;
use super::{comp_at, components, need, Grupo, Kind};
use crate::buildings::Edits;
use crate::lod::DESIGN_VOXEL;
use crate::palette::AIR;
use crate::structures::colonial;
use crate::terrain::TerrainGen;
use glam::{IVec2, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Edad de la escuela de primeras letras.
pub const EDAD_ESCOLAR: std::ops::RangeInclusive<i32> = 6..=12;
/// Pozos negros a menos de esta distancia filtran a un pozo de agua (m).
pub const RADIO_POZO_NEGRO: f32 = 40.0;
/// Un basural ensucia lo que tiene a menos de esto (m).
pub const RADIO_BASURAL: f32 = 60.0;
/// El basurero ensucia los pozos y las casas a menos de esto (m).
pub const RADIO_BASURERO: f32 = 250.0;
/// Una calle con un farol a menos de esto no está a oscuras (m).
pub const RADIO_FAROL: f32 = 35.0;
/// Contrabando y marineros: el puerto pesa hasta esta distancia del muelle (m).
pub const RADIO_PUERTO: f32 = 300.0;
/// Vecindario para la pobreza, la desigualdad y la desocupación (m).
pub const RADIO_VECINDARIO: f32 = 120.0;

/// Contadores de delitos: robos en casas, asaltos a comercios y contrabando.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Delitos {
    pub mes: [u32; 3],
    pub año: [u32; 3],
    pub total: u32,
    /// Ladrones prendidos por los serenos, el alcalde o la guardia.
    pub prendidos: u32,
}

/// Lo que se guarda en la partida (el resto se recalcula al cargar).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct BarrioData {
    pub basurales: Vec<u32>,
    pub presos: i32,
    pub delitos: Delitos,
    pub perdidas: i64,
    pub contrabando: f32,
    pub brotes: u32,
    pub muertes_brotes: u32,
}

/// Escuelas, salud, saneamiento y delito de cada solar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Barrio {
    // --- Derivado (se recalcula cada mes, al construir y al cargar) ---
    /// Acceso a la escuela de cada casa (0 a 1: lugar en una escuela por lo que rinde).
    pub escuela: BTreeMap<u32, f32>,
    /// Cobertura de salud (0 a 1).
    pub salud: BTreeMap<u32, f32>,
    /// Insalubridad (0 a 1).
    pub insalubre: BTreeMap<u32, f32>,
    /// Vigilancia (0 a 1) y riesgo de delito (0 a 1, después de la vigilancia).
    pub vigilancia: BTreeMap<u32, f32>,
    pub delito: BTreeMap<u32, f32>,
    /// Niños libres en edad escolar y cuántos tienen lugar en una escuela.
    pub niños: (i32, i32),
    // --- Guardado ---
    /// Baldíos con basural.
    pub basurales: BTreeSet<u32>,
    /// Presos en la cárcel.
    pub presos: i32,
    pub delitos: Delitos,
    /// Alcabala que se perdió el último mes por los asaltos a los comercios.
    pub perdidas: i64,
    /// Contrabando extra por el delito alrededor del muelle (0 a 1).
    pub contrabando: f32,
    /// Brotes de disentería y sus muertos.
    pub brotes: u32,
    pub muertes_brotes: u32,
}

impl Barrio {
    pub fn guardar(&self) -> BarrioData {
        BarrioData { basurales: self.basurales.iter().copied().collect(), presos: self.presos, delitos: self.delitos, perdidas: self.perdidas, contrabando: self.contrabando, brotes: self.brotes, muertes_brotes: self.muertes_brotes }
    }

    pub fn cargar(d: &BarrioData) -> Barrio {
        Barrio { basurales: d.basurales.iter().copied().collect(), presos: d.presos.max(0), delitos: d.delitos, perdidas: d.perdidas, contrabando: d.contrabando.clamp(0.0, 1.0), brotes: d.brotes, muertes_brotes: d.muertes_brotes, ..Default::default() }
    }

    pub fn escuela_de(&self, lot: u32) -> f32 {
        self.escuela.get(&lot).copied().unwrap_or(0.0)
    }
    pub fn salud_de(&self, lot: u32) -> f32 {
        self.salud.get(&lot).copied().unwrap_or(0.0)
    }
    pub fn insalubre_de(&self, lot: u32) -> f32 {
        self.insalubre.get(&lot).copied().unwrap_or(0.0)
    }
    pub fn delito_de(&self, lot: u32) -> f32 {
        self.delito.get(&lot).copied().unwrap_or(0.0)
    }
    pub fn vigilancia_de(&self, lot: u32) -> f32 {
        self.vigilancia.get(&lot).copied().unwrap_or(0.0)
    }

    /// Media de un mapa (0 si está vacío).
    pub fn media(m: &BTreeMap<u32, f32>) -> f32 {
        if m.is_empty() { 0.0 } else { m.values().sum::<f32>() / m.len() as f32 }
    }
}

// --- Servicios de las épocas siguientes (datos para la interfaz y el diseño) ---------------

/// Un servicio de una época futura: se muestra como tarjeta desactivada («Época N»).
pub struct Futuro {
    pub grupo: Grupo,
    pub nombre: &'static str,
    pub desde: Epoca,
    pub efecto: &'static str,
    /// Fecha histórica verificada (con la fuente en el documento del sistema), si la hay.
    pub fecha: Option<&'static str>,
}

pub const FUTUROS: &[Futuro] = &[
    Futuro { grupo: Grupo::Agua, nombre: "Agua corriente", desde: Epoca::Inmigracion, efecto: "Toma en el río Santa Lucía y cañerías: agua buena en las casas de la red.", fecha: Some("1871") },
    Futuro { grupo: Grupo::Educacion, nombre: "Universidad", desde: Epoca::CiudadNueva, efecto: "Abogados, médicos e ingenieros: oficios letrados y prestigio de la ciudad.", fecha: Some("1849") },
    Futuro { grupo: Grupo::Educacion, nombre: "Escuela pública", desde: Epoca::Inmigracion, efecto: "Gratuita y obligatoria: lectura casi universal de los niños de su barrio.", fecha: Some("1877") },
    Futuro { grupo: Grupo::Educacion, nombre: "Liceo", desde: Epoca::Inmigracion, efecto: "Enseñanza secundaria: empleados, maestros y comercio.", fecha: None },
    Futuro { grupo: Grupo::Educacion, nombre: "Escuela técnica", desde: Epoca::ArtDeco, efecto: "Artes y oficios para la industria: obreros calificados.", fecha: None },
    Futuro { grupo: Grupo::Salud, nombre: "Hospital moderno", desde: Epoca::ArtDeco, efecto: "Pabellones, quirófanos y laboratorio: mucha menos mortalidad.", fecha: None },
    Futuro { grupo: Grupo::Salud, nombre: "Policlínica barrial", desde: Epoca::Modernismo, efecto: "Atención cerca de casa: vacunas y control de los niños.", fecha: None },
    Futuro { grupo: Grupo::Seguridad, nombre: "Comisaría", desde: Epoca::CiudadNueva, efecto: "Policía con sección propia: patrullas por el barrio.", fecha: None },
    Futuro { grupo: Grupo::Seguridad, nombre: "Cuartel de bomberos", desde: Epoca::Inmigracion, efecto: "Bombas a vapor y bomberos de oficio: los incendios se apagan antes.", fecha: None },
    Futuro { grupo: Grupo::Saneamiento, nombre: "Colectores", desde: Epoca::CiudadNueva, efecto: "Caños bajo las calles que se llevan las aguas servidas: adiós a los pozos negros.", fecha: Some("mediados del siglo XIX") },
    Futuro { grupo: Grupo::Saneamiento, nombre: "Carros de basura", desde: Epoca::Inmigracion, efecto: "Carros de la basura con recorrido fijo y usina de residuos.", fecha: None },
];

/// Número de la época (1 a 6) para las tarjetas.
pub fn numero_epoca(e: Epoca) -> usize {
    Epoca::ALL.iter().position(|&x| x == e).unwrap_or(0) + 1
}

// --- Ayudas ---------------------------------------------------------------------------------

/// Pozo negro o letrina de un solar (cuánto ensucia el suelo y el agua de alrededor): la
/// letrina del rancho es un hoyo poco hondo; las casas de material tienen pozo negro de
/// ladrillo; las curtiembres y velerías tiran sus aguas a la calle.
pub fn foco(l: &Lot) -> f32 {
    match zone_of(l.zone) {
        Some((ZoneKind::Residencial, d)) => [0.0, 1.0, 0.75, 0.6][l.level as usize] * if d == Density::Alta { 1.3 } else { 1.0 },
        Some((ZoneKind::Comercial, _)) if l.level > 0 => 0.5,
        Some((ZoneKind::Industrial, _)) if l.level >= 2 => 2.0,
        Some((ZoneKind::Industrial, _)) if l.level > 0 => 0.8,
        _ => 0.0,
    }
}

/// Grilla de puntos para buscar vecinos a una distancia.
struct Grilla {
    celda: f32,
    mapa: HashMap<IVec2, Vec<(u32, Vec2)>>,
}

impl Grilla {
    fn new(celda: f32, puntos: impl Iterator<Item = (u32, Vec2)>) -> Grilla {
        let mut mapa: HashMap<IVec2, Vec<(u32, Vec2)>> = HashMap::new();
        for (id, p) in puntos {
            mapa.entry((p / celda).floor().as_ivec2()).or_default().push((id, p));
        }
        Grilla { celda, mapa }
    }

    /// Puntos a menos de `r` de `p` (en orden de id, para que todo sea determinista).
    fn cerca(&self, p: Vec2, r: f32) -> Vec<(u32, Vec2)> {
        let c = (p / self.celda).floor().as_ivec2();
        let k = (r / self.celda).ceil() as i32;
        let mut out: Vec<(u32, Vec2)> = Vec::new();
        for dy in -k..=k {
            for dx in -k..=k {
                if let Some(v) = self.mapa.get(&(c + IVec2::new(dx, dy))) {
                    out.extend(v.iter().filter(|(_, q)| q.distance(p) <= r));
                }
            }
        }
        out.sort_by_key(|x| x.0);
        out
    }

    fn hay(&self, p: Vec2, r: f32) -> bool {
        let c = (p / self.celda).floor().as_ivec2();
        let k = (r / self.celda).ceil() as i32;
        (-k..=k).any(|dy| (-k..=k).any(|dx| self.mapa.get(&(c + IVec2::new(dx, dy))).is_some_and(|v| v.iter().any(|(_, q)| q.distance(p) <= r))))
    }
}

/// Lo que la población cuenta de cada solar (o una estimación antes de fundar la población).
#[derive(Clone, Copy, Debug, Default)]
struct DatosLote {
    /// Niños libres de 6 a 12 años.
    niños: i32,
    hogares: f32,
    pobres: f32,
    ricos: f32,
    hombres: f32,
    parados: f32,
}

/// Barrio de un punto para la gaceta.
fn lugar(c: &City, p: Vec2) -> &'static str {
    if c.services.port.pier.as_ref().is_some_and(|m| m.land.distance(p) < RADIO_PUERTO) {
        return "junto al puerto";
    }
    match c.lots.plaza_center.map_or(0.0, |q| q.distance(p)) {
        d if d < 250.0 => "cerca de la plaza",
        d if d < 600.0 => "en el barrio",
        _ => "en las afueras",
    }
}

impl City {
    fn datos_lotes(&self) -> HashMap<u32, DatosLote> {
        use super::super::poblacion::{economia::zona_de_trabajo, Clase, Oficio, Sexo};
        let mut out: HashMap<u32, DatosLote> = HashMap::new();
        let p = &self.poblacion;
        if p.fundada {
            for h in p.hogares.iter().filter(|h| h.activo && h.vive_en_solar()) {
                let d = out.entry(h.lote.unwrap()).or_default();
                d.hogares += 1.0;
                match h.clase {
                    Clase::Pobre => d.pobres += 1.0,
                    Clase::Acomodada | Clase::Principal => d.ricos += 1.0,
                    _ => {}
                }
            }
            for per in p.vivos() {
                let Some(lote) = per.hogar.and_then(|h| p.hogar(h)).filter(|h| h.vive_en_solar()).and_then(|h| h.lote) else { continue };
                let edad = per.edad(p.mes);
                let d = out.entry(lote).or_default();
                if EDAD_ESCOLAR.contains(&edad) {
                    d.niños += 1;
                }
                if (16..=60).contains(&edad) && per.sexo == Sexo::Hombre {
                    d.hombres += 1.0;
                    if per.oficio == Oficio::Ninguno || (zona_de_trabajo(per.oficio).is_some() && per.trabajo.is_none()) {
                        d.parados += 1.0;
                    }
                }
            }
        } else {
            let (jc, ji) = self.lots.jobs();
            let paro = (1.0 - (jc + ji) as f32 / (self.workers() as f32).max(1.0)).clamp(0.0, 1.0);
            for l in self.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Residencial)) {
                let n = l.residents() as f32;
                out.insert(l.id, DatosLote { niños: (n * 0.18).round() as i32, hogares: 1.0, pobres: [0.0, 0.6, 0.25, 0.05][l.level as usize], ricos: [0.0, 0.0, 0.1, 0.5][l.level as usize], hombres: n * 0.3, parados: n * 0.3 * paro });
            }
        }
        out
    }

    /// Recalcula escuelas, salud, insalubridad, vigilancia y delito de cada solar (sin azar:
    /// solo depende del estado guardado). Se llama cada mes, al construir o demoler y al cargar.
    pub fn refresh_barrio(&mut self) {
        let datos = self.datos_lotes();
        let s = &self.services;
        let lots = &self.lots.lots;
        let built: Vec<&Lot> = lots.values().filter(|l| l.level > 0).collect();
        let mut served: Vec<(u32, i32)> = Vec::new();

        // 1. Escuelas: lugares para los niños libres de 6 a 12 años de las casas más cercanas.
        let mut escuela = BTreeMap::new();
        let mut order: Vec<u32> = s.buildings.values().filter(|b| b.info().grupo == Grupo::Educacion).map(|b| b.id).collect();
        order.sort_by(|a, b| s.buildings[b].info().quality.total_cmp(&s.buildings[a].info().quality).then(a.cmp(b)));
        for sid in order {
            let b = &s.buildings[&sid];
            let inf = b.info();
            let mut cands: Vec<(f32, u32)> = built.iter().filter(|l| l.kind() == Some(ZoneKind::Residencial) && !escuela.contains_key(&l.id)).map(|l| (l.c.distance(b.c), l.id)).filter(|x| x.0 <= inf.radius).collect();
            cands.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            let mut used = 0;
            for (_, id) in cands {
                let n = datos.get(&id).map_or(0, |d| d.niños);
                if used + n > inf.plazas {
                    break;
                }
                used += n;
                escuela.insert(id, inf.quality);
            }
            served.push((sid, used));
        }
        let total_niños: i32 = built.iter().filter(|l| l.kind() == Some(ZoneKind::Residencial)).map(|l| datos.get(&l.id).map_or(0, |d| d.niños)).sum();
        let con_escuela: i32 = escuela.keys().map(|id| datos.get(id).map_or(0, |d| d.niños)).sum();

        // 2. Salud: cada uno atiende a tantas personas; el médico y el hospital, por la calle.
        let mut salud: BTreeMap<u32, f32> = BTreeMap::new();
        let por_red = |k: Kind| matches!(k, Kind::Medico | Kind::Hospital);
        let comp = if s.buildings.values().any(|b| por_red(b.kind)) { components(&self.net) } else { BTreeMap::new() };
        let mut order: Vec<u32> = s.buildings.values().filter(|b| matches!(b.kind, Kind::Botica | Kind::Medico | Kind::Hospital)).map(|b| b.id).collect();
        order.sort_by(|a, b| s.buildings[b].info().quality.total_cmp(&s.buildings[a].info().quality).then(a.cmp(b)));
        for sid in order {
            let b = &s.buildings[&sid];
            let inf = b.info();
            let src = if por_red(b.kind) { comp_at(&self.net, &comp, b.c, 40.0) } else { None };
            let mut cands: Vec<(f32, u32)> = built
                .iter()
                .filter(|l| l.c.distance(b.c) <= inf.radius)
                .filter(|l| !por_red(b.kind) || (src.is_some() && comp_at(&self.net, &comp, l.c + l.f * (super::super::lots::D * 0.5 + 3.0), 12.0) == src))
                .map(|l| (l.c.distance(b.c), l.id))
                .collect();
            cands.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            let mut used = 0;
            for (_, id) in cands {
                let n = need(&lots[&id]);
                if used + n > inf.plazas {
                    break;
                }
                used += n;
                let e = salud.entry(id).or_insert(0.0);
                *e = 1.0 - (1.0 - *e) * (1.0 - inf.quality);
            }
            served.push((sid, used));
        }

        // 3. Insalubridad: agua mala, pozos negros alrededor, basurales y el basurero.
        let lejos = self.economy.is_on(Ordinance::PozosNegrosLejos);
        let limpieza = self.economy.is_on(Ordinance::Limpieza);
        let focos = Grilla::new(40.0, built.iter().map(|l| (l.id, l.c)));
        let basurales = Grilla::new(60.0, s.barrio.basurales.iter().filter_map(|id| lots.get(id).map(|l| (*id, l.c))));
        let basureros: Vec<Vec2> = s.of_kind(Kind::Basurero).map(|b| b.c).collect();
        let mut insalubre = BTreeMap::new();
        for l in &built {
            let w = s.water_of(l.id);
            let agua = if l.kind() == Some(ZoneKind::Industrial) { 0.1 } else if w <= 0.0 { 0.3 } else { (1.0 - w) * 0.6 };
            let pozos: f32 = focos.cerca(l.c, 30.0).iter().map(|(id, _)| foco(&lots[id])).sum::<f32>() * if lejos { 0.03 } else { 0.06 };
            let basura = if basurales.hay(l.c, RADIO_BASURAL) { 0.35 } else if basurales.hay(l.c, RADIO_BASURAL * 2.0) { 0.15 } else { 0.0 };
            let vertedero = if basureros.iter().any(|b| b.distance(l.c) < RADIO_BASURERO) { 0.2 } else { 0.0 };
            let v = (agua + pozos.min(0.35) + basura + vertedero) * if limpieza { 0.85 } else { 1.0 };
            insalubre.insert(l.id, v.clamp(0.0, 1.0));
        }

        // 4. Vigilancia y delito.
        let guardias: Vec<(Vec2, f32, f32)> = s.buildings.values().filter(|b| matches!(b.kind, Kind::Serenos | Kind::Alcalde | Kind::Carcel)).map(|b| (b.c, b.info().radius, b.info().quality)).collect();
        // La cárcel disuade en toda la ciudad (menos si está llena).
        let disuasion = match s.of_kind(Kind::Carcel).next() {
            Some(b) if s.barrio.presos < b.info().plazas => 0.1,
            Some(_) => 0.04,
            None => 0.0,
        };
        let faroles = Grilla::new(40.0, s.lamps.iter().enumerate().map(|(i, &(v, _))| (i as u32, (v.as_vec2() + 0.5) * DESIGN_VOXEL)));
        let vacios = Grilla::new(60.0, s.ruins.iter().chain(&s.barrio.basurales).filter_map(|id| lots.get(id).map(|l| (*id, l.c))));
        let muelle = s.port.pier.as_ref().map(|p| p.land);
        let mut vigilancia = BTreeMap::new();
        let mut delito = BTreeMap::new();
        for l in &built {
            let mut libre = 1.0f32;
            for &(c, r, q) in &guardias {
                if c.distance(l.c) <= r {
                    libre *= 1.0 - q;
                }
            }
            let vig = 1.0 - libre * (1.0 - disuasion);
            // El vecindario: pobreza, desigualdad (ricos y pobres juntos), desocupación, densidad.
            let mut v = DatosLote::default();
            let cerca = focos.cerca(l.c, RADIO_VECINDARIO);
            for (id, _) in &cerca {
                if let Some(d) = datos.get(id) {
                    v.hogares += d.hogares;
                    v.pobres += d.pobres;
                    v.ricos += d.ricos;
                    v.hombres += d.hombres;
                    v.parados += d.parados;
                }
            }
            let pob = if v.hogares > 0.0 { v.pobres / v.hogares } else { 0.3 };
            let ricos = if v.hogares > 0.0 { v.ricos / v.hogares } else { 0.0 };
            let paro = if v.hombres > 0.0 { v.parados / v.hombres } else { 0.0 };
            let densidad = (cerca.iter().filter(|x| x.1.distance(l.c) < 60.0).count() as f32 / 14.0).min(1.0);
            let oscuro = if faroles.hay(l.c, RADIO_FAROL) { 0.0 } else { 1.0 };
            let baldios = (vacios.cerca(l.c, 60.0).len() as f32 / 3.0).min(1.0);
            let puerto = muelle.map_or(0.0, |m| (1.0 - m.distance(l.c) / RADIO_PUERTO).max(0.0));
            let tienda = if l.kind() == Some(ZoneKind::Comercial) { 0.05 } else { 0.0 };
            let riesgo = 0.06 + 0.30 * pob + 0.25 * (pob * ricos * 4.0).min(1.0) + 0.20 * paro + 0.15 * oscuro + 0.08 * densidad + 0.12 * baldios + 0.15 * puerto + tienda;
            vigilancia.insert(l.id, vig);
            delito.insert(l.id, (riesgo.clamp(0.0, 1.0) * (1.0 - vig)).clamp(0.0, 1.0));
        }

        let s = &mut self.services;
        for (sid, used) in served {
            if let Some(b) = s.buildings.get_mut(&sid) {
                b.served = used;
            }
        }
        let br = &mut s.barrio;
        br.escuela = escuela;
        br.salud = salud;
        br.insalubre = insalubre;
        br.vigilancia = vigilancia;
        br.delito = delito;
        br.niños = (total_niños, con_escuela);
    }

    /// Los pozos y aljibes se ensucian (o se limpian) de a poco con los pozos negros, los
    /// basurales y el basurero de alrededor; aparecen basurales donde nadie junta la basura y
    /// las carretillas los levantan.
    pub(crate) fn saneamiento_month(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        let lejos = self.economy.is_on(Ordinance::PozosNegrosLejos);
        let recoleccion = self.economy.is_on(Ordinance::RecoleccionBasura);
        let lots = &self.lots.lots;
        let built = Grilla::new(40.0, lots.values().filter(|l| l.level > 0).map(|l| (l.id, l.c)));
        let basurales: Vec<Vec2> = self.services.barrio.basurales.iter().filter_map(|id| lots.get(id).map(|l| l.c)).collect();
        let basureros: Vec<Vec2> = self.services.of_kind(Kind::Basurero).map(|b| b.c).collect();
        // Pozos de agua.
        let ids: Vec<u32> = self.services.buildings.values().filter(|b| matches!(b.kind, Kind::Pozo | Kind::Aljibe)).map(|b| b.id).collect();
        let mut sucio = 0;
        for id in ids {
            let b = &self.services.buildings[&id];
            let pozos: f32 = built.cerca(b.c, RADIO_POZO_NEGRO).iter().map(|(l, _)| foco(&lots[l])).sum::<f32>() * if lejos { 0.035 } else { 0.1 };
            let basura = basurales.iter().filter(|p| p.distance(b.c) < RADIO_BASURAL).count() as f32 * 0.3;
            let vertedero = if basureros.iter().any(|p| p.distance(b.c) < RADIO_BASURERO) { 0.5 } else { 0.0 };
            let objetivo = (pozos + basura + vertedero).clamp(0.0, 1.0) * if b.kind == Kind::Aljibe { 0.5 } else { 1.0 };
            let antes = b.contaminacion;
            let b = self.services.buildings.get_mut(&id).unwrap();
            b.contaminacion = (b.contaminacion + (objetivo - b.contaminacion) * 0.12).clamp(0.0, 1.0);
            if antes < 0.5 && b.contaminacion >= 0.5 {
                sucio += 1;
            }
        }
        if sucio > 0 {
            news.push(if sucio == 1 { "El agua de un pozo huele mal: los pozos negros de alrededor la ensuciaron".to_string() } else { format!("El agua de {sucio} pozos huele mal: los pozos negros de alrededor la ensuciaron") });
        }
        // Basurales: en los baldíos rodeados de casas, si nadie se lleva la basura.
        let comp = if recoleccion && !basureros.is_empty() { components(&self.net) } else { BTreeMap::new() };
        let destinos: Vec<(Vec2, Option<u32>)> = self.services.of_kind(Kind::Basurero).map(|b| (b.c, comp_at(&self.net, &comp, b.c, 40.0))).collect();
        let alcance = super::info(Kind::Basurero).radius;
        let cubierto = |l: &Lot| recoleccion && destinos.iter().any(|&(c, k)| k.is_some() && c.distance(l.c) <= alcance && comp_at(&self.net, &comp, l.c + l.f * (super::super::lots::D * 0.5 + 3.0), 14.0) == k);
        let mut nuevos = Vec::new();
        let mut limpios = Vec::new();
        for l in lots.values() {
            if l.level > 0 || self.services.ruins.contains(&l.id) {
                continue;
            }
            let tiene = self.services.barrio.basurales.contains(&l.id);
            let cub = cubierto(l);
            if tiene {
                if cub && self.services.rng.f() < 0.5 {
                    limpios.push(l.id);
                }
                continue;
            }
            let casas = built.cerca(l.c, 60.0).len();
            if casas < 5 || cub {
                continue;
            }
            let base = if recoleccion { 0.008 } else if basureros.iter().any(|b| b.distance(l.c) < 700.0) { 0.012 } else { 0.025 };
            if self.services.rng.f() < base * (casas as f32 / 10.0).clamp(0.5, 1.5) && nuevos.len() < 3 {
                nuevos.push(l.id);
            }
        }
        for id in &limpios {
            self.services.barrio.basurales.remove(id);
            if let Some(l) = self.lots.lots.get_mut(id) {
                for &v in &l.voxels {
                    e.dset(v, AIR);
                }
                l.voxels.clear();
            }
        }
        for &id in &nuevos {
            self.services.barrio.basurales.insert(id);
            self.basural_edits(t, id, e);
        }
        if let Some(&id) = nuevos.first() {
            let p = self.lots.lots[&id].c;
            news.push(format!("Crece un basural en un baldío {}: moscas, ratas y mal olor", lugar(self, p)));
        }
        if !limpios.is_empty() {
            news.push(format!("Las carretillas de la basura limpiaron {} {}", limpios.len(), if limpios.len() == 1 { "basural" } else { "basurales" }));
        }
        news
    }

    /// Vóxeles de un basural sobre un baldío.
    pub(crate) fn basural_edits(&mut self, t: &TerrainGen, id: u32, e: &mut Edits) {
        let Some(l) = self.lots.lots.get(&id) else { return };
        let b = build_rotated(t, l.c, l.f, colonial::LOT_W, colonial::LOT_D, l.seed ^ 0xBA5, &colonial::basural);
        e.extend(b.edits);
        self.lots.lots.get_mut(&id).unwrap().voxels = b.voxels;
    }

    /// Brotes de disentería en los barrios más insalubres (con la población de personas, mueren
    /// sobre todo niños y pobres).
    pub(crate) fn salubridad_month(&mut self) -> Vec<String> {
        let br = &self.services.barrio;
        let casas: Vec<(u32, f32, i32)> = self.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Residencial)).map(|l| (l.id, br.insalubre_de(l.id), l.residents())).collect();
        let hab: i32 = casas.iter().map(|c| c.2).sum();
        if hab < 60 || self.services.epidemic.active() {
            return Vec::new();
        }
        let media = casas.iter().map(|c| c.1 * c.2 as f32).sum::<f32>() / hab as f32;
        if media <= 0.3 || self.services.rng.f() >= (media - 0.3) * 0.25 {
            return Vec::new();
        }
        let mut peores = casas.clone();
        peores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let lotes: Vec<u32> = peores.iter().take(8).map(|c| c.0).collect();
        let muertos = if self.poblacion.fundada {
            self.poblacion.choque_epidemia(0.015 + media * 0.02, Some(&lotes)) as i32
        } else {
            let n = (lotes.len() as i32).min(self.pool);
            self.pool -= n;
            n
        };
        let br = &mut self.services.barrio;
        br.brotes += 1;
        br.muertes_brotes += muertos.max(0) as u32;
        let donde = self.lots.lots.get(&lotes[0]).map_or("en el barrio", |l| lugar(self, l.c));
        vec![format!("Disentería {donde}: el agua mala, los pozos negros y la basura se cobran {muertos} {}", if muertos == 1 { "vida" } else { "vidas" })]
    }

    /// Los delitos del mes: robos en casas, asaltos a pulperías y tiendas y contrabando junto al
    /// puerto, según el riesgo de cada solar; la vigilancia prende a algunos ladrones.
    pub(crate) fn delito_month(&mut self) -> Vec<String> {
        let mut ev = [0u32; 3];
        let mut prendidos = 0;
        let mut perdidas = 0i64;
        let mut destacado: Option<(usize, u32, bool)> = None;
        let carcel = self.services.of_kind(Kind::Carcel).next().map(|b| b.info().plazas);
        let muelle = self.services.port.pier.as_ref().map(|p| p.land);
        let ids: Vec<(u32, f32, f32, bool, u8, Vec2)> = self.lots.lots.values().filter(|l| l.level > 0).map(|l| (l.id, self.services.barrio.delito_de(l.id), self.services.barrio.vigilancia_de(l.id), l.kind() == Some(ZoneKind::Comercial), l.level, l.c)).collect();
        for (id, d, vig, comercio, nivel, c) in ids {
            let p = d * 0.02 * if comercio { 2.0 } else { 1.0 };
            if self.services.rng.f() >= p {
                continue;
            }
            let junto_al_puerto = muelle.is_some_and(|m| m.distance(c) < RADIO_PUERTO);
            let tipo = if junto_al_puerto && self.services.rng.f() < 0.5 { 2 } else if comercio { 1 } else { 0 };
            ev[tipo] += 1;
            if tipo == 1 {
                perdidas += 15 + nivel as i64 * 10;
            }
            if self.poblacion.fundada && tipo < 2 {
                self.poblacion.choque_robo(id, tipo == 1);
            }
            let preso = self.services.rng.f() < vig * 0.8;
            if preso {
                prendidos += 1;
                if carcel.is_some_and(|cap| self.services.barrio.presos < cap) {
                    self.services.barrio.presos += 1;
                }
            }
            if destacado.is_none() {
                destacado = Some((tipo, id, preso));
            }
        }
        let br = &mut self.services.barrio;
        // Los presos cumplen su pena y salen.
        br.presos -= (br.presos + 5) / 6;
        if self.month % 12 == 0 {
            br.delitos.año = [0; 3];
        }
        br.delitos.mes = ev;
        for k in 0..3 {
            br.delitos.año[k] += ev[k];
        }
        let total: u32 = ev.iter().sum();
        br.delitos.total += total;
        br.delitos.prendidos += prendidos;
        br.perdidas = perdidas;
        // Contrabando: el delito alrededor del muelle se suma a lo que se pierde de la aduana.
        br.contrabando = match muelle {
            Some(m) => {
                let cerca: Vec<f32> = self.lots.lots.values().filter(|l| l.level > 0 && l.c.distance(m) < RADIO_PUERTO).map(|l| br.delito_de(l.id)).collect();
                if cerca.is_empty() { 0.0 } else { 0.25 * cerca.iter().sum::<f32>() / cerca.len() as f32 }
            }
            None => 0.0,
        };
        let Some((tipo, id, preso)) = destacado else { return Vec::new() };
        if total < 3 && self.services.rng.f() > 0.35 {
            return Vec::new();
        }
        let donde = self.lots.lots.get(&id).map_or("en el barrio", |l| lugar(self, l.c));
        let que = match tipo {
            0 => format!("Roban en una casa {donde}"),
            1 => format!("Asaltan una pulpería {donde}"),
            _ => format!("Contrabando {donde}: cueros y tabaco que no pasan por la aduana"),
        };
        let cola = if preso {
            if carcel.is_some() { "; la ronda prende al ladrón y va a la cárcel del Cabildo".to_string() } else { "; lo prenden, pero no hay cárcel donde tenerlo".to_string() }
        } else {
            String::new()
        };
        let resto = if total > 1 { format!(" ({total} delitos este mes)") } else { String::new() };
        vec![format!("{que}{cola}{resto}")]
    }

    /// Lo que el barrio le cuenta a la economía: el delito en los comercios (opinión de los
    /// comerciantes), la alcabala que se pierde y el contrabando del puerto.
    pub(crate) fn barrio_input(&self, inp: &mut MonthInput) {
        let br = &self.services.barrio;
        let tiendas: Vec<f32> = self.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Comercial)).map(|l| br.delito_de(l.id)).collect();
        inp.crimen = if tiendas.is_empty() { Barrio::media(&br.delito) } else { tiendas.iter().sum::<f32>() / tiendas.len() as f32 };
        if br.perdidas > 0 {
            inp.other.push(("Alcabala perdida por robos y asaltos".into(), -br.perdidas));
        }
        inp.contraband = (inp.contraband + br.contrabando).min(0.9);
    }

    /// Resumen para los paneles: (niños con escuela, niños), cobertura de salud de los
    /// habitantes, insalubridad media, delitos del año, basurales, pozos contaminados.
    pub fn barrio_resumen(&self) -> BarrioResumen {
        let br = &self.services.barrio;
        let casas: Vec<&Lot> = self.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Residencial)).collect();
        let hab: i32 = casas.iter().map(|l| l.residents()).sum::<i32>().max(1);
        let pesado = |m: &BTreeMap<u32, f32>| casas.iter().map(|l| m.get(&l.id).copied().unwrap_or(0.0) * l.residents() as f32).sum::<f32>() / hab as f32;
        BarrioResumen {
            niños: br.niños,
            salud: pesado(&br.salud),
            insalubre: pesado(&br.insalubre),
            delito: pesado(&br.delito),
            delitos_año: br.delitos.año,
            basurales: br.basurales.len(),
            pozos_sucios: self.services.buildings.values().filter(|b| matches!(b.kind, Kind::Pozo | Kind::Aljibe) && b.contaminacion >= 0.5).count(),
            presos: br.presos,
            brotes: br.brotes,
        }
    }

    /// Ficha de un edificio de educación, salud, seguridad o saneamiento.
    pub(crate) fn barrio_lines(&self, id: u32, out: &mut Vec<String>) {
        let b = &self.services.buildings[&id];
        let inf = b.info();
        let br = &self.services.barrio;
        match b.kind {
            Kind::Escuela | Kind::Convento => {
                out.push(format!("Alumnos de 6 a 12 años: {} de {}", b.served, inf.plazas));
                out.push(format!("Alcance: {} m · rinde {} %", inf.radius as i32, (inf.quality * 100.0) as i32));
                if !self.economy.is_on(Ordinance::EscuelasPrimerasLetras) {
                    out.push("Sin la ordenanza de escuelas, los niños pobres casi no van (hay que pagar al maestro)".into());
                }
            }
            Kind::Botica | Kind::Medico | Kind::Hospital => {
                out.push(format!("Atiende a {} de {} personas", b.served, inf.plazas));
                out.push(format!("Alcance: {} m{}", inf.radius as i32, if b.kind == Kind::Botica { "" } else { " por la calle" }));
            }
            Kind::Alcalde => out.push(format!("Ronda: {} m · ladrones prendidos en la ciudad: {}", inf.radius as i32, br.delitos.prendidos)),
            Kind::Carcel => {
                out.push(format!("Presos: {} de {}", br.presos, inf.plazas));
                out.push(format!("Guardia: {} m; disuade en toda la ciudad", inf.radius as i32));
            }
            Kind::Basurero => {
                out.push(format!("Basurales en la ciudad: {}", br.basurales.len()));
                if !self.economy.is_on(Ordinance::RecoleccionBasura) {
                    out.push("Sin la ordenanza de recolección, las carretillas no salen".into());
                }
                out.push(format!("Ensucia los pozos a menos de {} m", RADIO_BASURERO as i32));
            }
            _ => {}
        }
    }
}

/// Resumen del barrio para la interfaz.
#[derive(Clone, Copy, Debug, Default)]
pub struct BarrioResumen {
    pub niños: (i32, i32),
    pub salud: f32,
    pub insalubre: f32,
    pub delito: f32,
    pub delitos_año: [u32; 3],
    pub basurales: usize,
    pub pozos_sucios: usize,
    pub presos: i32,
    pub brotes: u32,
}

#[cfg(test)]
mod tests {
    use super::super::tests::{free_site, town};
    use super::*;

    /// Solares de casas con chicos (o, antes de fundar, casas construidas).
    fn ciudad_crecida() -> (TerrainGen, City) {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        let c = city.plaza.unwrap().0;
        for (k, at) in [(Kind::Aljibe, c + Vec2::new(60.0, 60.0)), (Kind::Aljibe, c - Vec2::new(60.0, 60.0))] {
            let (sc, sf) = free_site(&city, &t, k, at);
            city.place_service(&t, k, sc, sf).unwrap();
        }
        city.fast_forward(&t, 30);
        (t, city)
    }

    #[test]
    fn la_escuela_tiene_cupo_y_alcance() {
        let (t, mut city) = ciudad_crecida();
        let c = city.plaza.unwrap().0;
        let (sc, sf) = free_site(&city, &t, Kind::Escuela, c + Vec2::new(-40.0, 80.0));
        city.place_service(&t, Kind::Escuela, sc, sf).unwrap();
        let esc = city.services.of_kind(Kind::Escuela).next().unwrap().clone();
        assert!(esc.voxels.len() > 300, "la escuela tiene vóxeles: {}", esc.voxels.len());
        let br = &city.services.barrio;
        assert!(!br.escuela.is_empty(), "alguna casa tiene escuela");
        assert!(esc.served <= 100, "no pasa del cupo: {}", esc.served);
        for &lot in br.escuela.keys() {
            assert!(city.lots.lots[&lot].c.distance(esc.c) <= 400.0, "dentro del alcance");
        }
        // Con el cupo achicado, quedan niños sin escuela.
        assert!(br.niños.0 > 0, "hay niños en la ciudad");
        let con_todo = br.niños.1;
        let datos = city.datos_lotes();
        let mut cands: Vec<(f32, u32)> = city.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Residencial)).map(|l| (l.c.distance(esc.c), l.id)).collect();
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let primeros: i32 = cands.iter().take(3).map(|x| datos.get(&x.1).map_or(0, |d| d.niños)).sum();
        assert!(con_todo >= primeros.min(100), "las casas más cercanas entran primero");
    }

    #[test]
    fn con_escuelas_aprenden_a_leer_mas_ninos() {
        let correr = |escuelas: bool| {
            let (t, mut city) = ciudad_crecida();
            let c = city.plaza.unwrap().0;
            if escuelas {
                for at in [c + Vec2::new(-40.0, 80.0), c + Vec2::new(80.0, -60.0)] {
                    let (sc, sf) = free_site(&city, &t, Kind::Escuela, at);
                    city.place_service(&t, Kind::Escuela, sc, sf).unwrap();
                }
                city.economy.active.push(Ordinance::EscuelasPrimerasLetras);
            }
            city.fast_forward(&t, 96);
            let p = &city.poblacion;
            let jovenes: Vec<_> = p.vivos().filter(|x| (10..=20).contains(&x.edad(p.mes))).collect();
            let leen = jovenes.iter().filter(|x| x.educacion > 0).count() as f32 / jovenes.len().max(1) as f32;
            (leen, jovenes.len())
        };
        let (sin, n0) = correr(false);
        let (con, n1) = correr(true);
        assert!(n0 > 10 && n1 > 10, "hay jóvenes: {n0}, {n1}");
        assert!(con > sin + 0.1, "con escuelas leen más jóvenes: {con} contra {sin}");
    }

    #[test]
    fn los_pozos_negros_ensucian_los_pozos_y_enferman() {
        let (t, mut city) = ciudad_crecida();
        let c = city.plaza.unwrap().0;
        let (pc, pf) = free_site(&city, &t, Kind::Pozo, c + Vec2::new(0.0, 100.0));
        city.place_service(&t, Kind::Pozo, pc, pf).unwrap();
        let id = city.services.of_kind(Kind::Pozo).next().unwrap().id;
        let servidos: Vec<u32> = city.services.water.iter().filter(|w| w.1 .1 == id).map(|w| *w.0).collect();
        assert!(!servidos.is_empty());
        city.refresh_barrio();
        let antes: f32 = servidos.iter().map(|l| city.services.barrio.insalubre_de(*l)).sum();
        let q0 = city.services.water_of(servidos[0]);
        // Un año de pozos negros alrededor.
        let mut e = Edits::default();
        for _ in 0..24 {
            city.saneamiento_month(&t, &mut e);
        }
        let sucio = city.services.buildings[&id].contaminacion;
        assert!(sucio > 0.2, "el pozo se ensucia: {sucio}");
        city.refresh_water();
        city.refresh_barrio();
        let q1 = city.services.water_of(servidos[0]);
        assert!(q1 < q0, "el agua empeora: {q0} → {q1}");
        let despues: f32 = servidos.iter().filter(|l| city.services.water_of(**l) > 0.0).map(|l| city.services.barrio.insalubre_de(*l)).sum();
        assert!(despues > antes, "más insalubridad: {antes} → {despues}");
        // Alejar los pozos negros lo limpia de a poco.
        city.economy.active.push(Ordinance::PozosNegrosLejos);
        for _ in 0..36 {
            city.saneamiento_month(&t, &mut e);
        }
        assert!(city.services.buildings[&id].contaminacion < sucio, "con la ordenanza el pozo se limpia");
    }

    #[test]
    fn la_insalubridad_y_la_salud_mueven_la_mortalidad() {
        use super::super::super::poblacion::simulacion::CiudadSintetica;
        use super::super::super::poblacion::ServiciosLote;
        // La misma población sintética con barrios insalubres y sin médicos, o sanos y atendidos.
        let correr = |sv: ServiciosLote| {
            let mut c = CiudadSintetica::nueva(7);
            c.epidemia_cada = 0;
            c.servicios = Some(sv);
            let mut muertes = 0;
            let mut infantiles = 0;
            for _ in 0..20 * 12 {
                c.paso();
                let f = c.p.historial.back().unwrap();
                muertes += f.cuenta.muertes;
                infantiles += f.cuenta.muertes_infantiles;
            }
            let salud = c.p.vivos().map(|p| p.salud).sum::<f32>() / c.p.vivos().count().max(1) as f32;
            (muertes, infantiles, salud, c.p.habitantes())
        };
        let malo = correr(ServiciosLote { insalubre: 0.8, ..Default::default() });
        let bueno = correr(ServiciosLote { salud: 0.9, ..Default::default() });
        let tasa = |x: (u32, u32, f32, u32)| x.0 as f32 / x.3.max(1) as f32;
        assert!(tasa(bueno) < tasa(malo), "más muertes con insalubridad y sin salud: {malo:?} contra {bueno:?}");
        assert!(bueno.1 < malo.1, "menos muertes de niños: {} contra {}", bueno.1, malo.1);
        assert!(bueno.2 > malo.2, "mejor salud: {} contra {}", bueno.2, malo.2);
    }

    #[test]
    fn el_delito_sube_con_la_pobreza_y_la_oscuridad_y_baja_con_la_vigilancia() {
        let (t, mut city) = ciudad_crecida();
        city.refresh_barrio();
        let media = |c: &City| Barrio::media(&c.services.barrio.delito);
        let base = media(&city);
        assert!(base > 0.0);
        // Más pobreza: todos los hogares pobres.
        for h in city.poblacion.hogares.iter_mut() {
            h.clase = super::super::super::poblacion::Clase::Pobre;
        }
        city.refresh_barrio();
        let con_pobreza = media(&city);
        assert!(con_pobreza > base, "la pobreza sube el delito: {base} → {con_pobreza}");
        // Faroles (serenos) y alcalde de barrio lo bajan.
        let c = city.plaza.unwrap().0;
        let (sc, sf) = free_site(&city, &t, Kind::Serenos, c + Vec2::new(-20.0, 40.0));
        city.place_service(&t, Kind::Serenos, sc, sf).unwrap();
        let mut e = Edits::default();
        city.refresh_lamps(&t, &mut e);
        city.refresh_barrio();
        let con_serenos = media(&city);
        assert!(con_serenos < con_pobreza, "los serenos y los faroles lo bajan: {con_pobreza} → {con_serenos}");
        let (ac, af) = free_site(&city, &t, Kind::Alcalde, c + Vec2::new(40.0, -40.0));
        city.place_service(&t, Kind::Alcalde, ac, af).unwrap();
        let con_alcalde = media(&city);
        assert!(con_alcalde < con_serenos, "el alcalde de barrio también: {con_serenos} → {con_alcalde}");
        // Los delitos llegan a la gaceta y a la economía.
        let mut news = Vec::new();
        for _ in 0..24 {
            news.extend(city.delito_month());
        }
        assert!(city.services.barrio.delitos.total > 0, "hay delitos");
        let mut inp = MonthInput::default();
        city.barrio_input(&mut inp);
        assert!(inp.crimen > 0.0);
    }

    #[test]
    fn la_oscuridad_sube_el_delito() {
        let (t, mut city) = ciudad_crecida();
        let c = city.plaza.unwrap().0;
        let (sc, sf) = free_site(&city, &t, Kind::Serenos, c);
        city.place_service(&t, Kind::Serenos, sc, sf).unwrap();
        let mut e = Edits::default();
        city.refresh_lamps(&t, &mut e);
        city.refresh_barrio();
        let con_luz = Barrio::media(&city.services.barrio.delito);
        // Sin faroles (pero con la misma ronda de serenos): más delito.
        city.services.lamps.clear();
        city.refresh_barrio();
        let a_oscuras = Barrio::media(&city.services.barrio.delito);
        assert!(a_oscuras > con_luz, "a oscuras hay más delito: {con_luz} → {a_oscuras}");
    }

    /// Escuelas, hospital, cárcel, pozos sucios, basurales y delitos: se guardan, se cargan con
    /// los mismos vóxeles y la ciudad sigue igual que si no se hubiera guardado. Una partida
    /// vieja (sin estos datos) carga.
    #[test]
    fn guardar_y_cargar_el_barrio() {
        use super::super::super::save::CityData;
        let (t, mut city) = ciudad_crecida();
        let c = city.plaza.unwrap().0;
        for (k, at) in [(Kind::Escuela, c + Vec2::new(-40.0, 80.0)), (Kind::Botica, c + Vec2::new(60.0, -60.0)), (Kind::Alcalde, c + Vec2::new(-60.0, 0.0)), (Kind::Pozo, c + Vec2::new(0.0, 100.0)), (Kind::Carcel, c + Vec2::new(30.0, 30.0))] {
            let (sc, sf) = free_site(&city, &t, k, at);
            city.place_service(&t, k, sc, sf).unwrap();
        }
        city.economy.active.push(Ordinance::EscuelasPrimerasLetras);
        city.services.barrio.presos = 3;
        let libre = city.lots.lots.values().find(|l| l.level == 0).map(|l| l.id);
        if let Some(id) = libre {
            city.services.barrio.basurales.insert(id);
            let mut e = Edits::default();
            city.basural_edits(&t, id, &mut e);
        }
        city.fast_forward(&t, 14);
        let data = CityData::from_city(&city);
        let json = serde_json::to_string(&data).unwrap();
        let back: CityData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, data);
        let mut loaded = back.to_city().unwrap();
        loaded.rebuild_edits(&t);
        // Lo derivado (escuelas, salud, delito por solar) se recalcula de lo guardado: igual
        // que en la ciudad original con el mismo estado.
        city.refresh_barrio();
        assert_eq!(loaded.services.barrio, city.services.barrio, "el barrio cargado es IGUAL");
        for (a, b) in loaded.services.buildings.values().zip(city.services.buildings.values()) {
            assert_eq!((a.kind, a.served, a.contaminacion, a.voxels.len()), (b.kind, b.served, b.contaminacion, b.voxels.len()), "{:?}", a.kind);
        }
        for id in &city.services.barrio.basurales {
            assert_eq!(loaded.lots.lots[id].voxels.len(), city.lots.lots[id].voxels.len(), "basural");
        }
        city.fast_forward(&t, 12);
        loaded.fast_forward(&t, 12);
        city.refresh_barrio();
        loaded.refresh_barrio();
        assert_eq!(loaded.services.barrio, city.services.barrio, "misma simulación después de cargar");
        assert_eq!((loaded.money, loaded.poblacion.habitantes()), (city.money, city.poblacion.habitantes()));
        // Partida vieja: sin el barrio ni la contaminación.
        let json = serde_json::to_string(&CityData::from_city(&city)).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["servicios"].as_object_mut().unwrap().remove("barrio");
        for b in v["servicios"]["edificios"].as_array_mut().unwrap() {
            b.as_object_mut().unwrap().remove("contaminacion");
        }
        let old: CityData = serde_json::from_value(v).unwrap();
        let viejo = old.to_city().unwrap();
        assert!(viejo.services.barrio.basurales.is_empty());
        assert!(viejo.services.buildings.values().all(|b| b.contaminacion == 0.0));
    }

    #[test]
    fn basurales_y_recoleccion() {
        let (t, mut city) = ciudad_crecida();
        // Unos baldíos en medio de las casas (se quita la zona de algunos solares).
        let c = city.plaza.unwrap().0;
        for d in [Vec2::new(70.0, 40.0), Vec2::new(-60.0, -70.0), Vec2::new(-80.0, 60.0), Vec2::new(60.0, -90.0)] {
            if let Some(id) = city.lots.lot_at(c + d).or_else(|| city.lots.nearby(c + d).into_iter().find(|id| city.lots.lots[id].level > 0)) {
                let p = city.lots.lots[&id].c;
                city.paint(p, 1.0, 0);
            }
        }
        let mut e = Edits::default();
        for _ in 0..60 {
            city.saneamiento_month(&t, &mut e);
        }
        let n = city.services.barrio.basurales.len();
        assert!(n > 0, "sin recolección aparecen basurales");
        let id = *city.services.barrio.basurales.iter().next().unwrap();
        assert!(!city.lots.lots[&id].voxels.is_empty(), "el basural se ve");
        // Basurero lejos de la plaza, con un camino, y la ordenanza de recolección.
        let c = city.plaza.unwrap().0;
        let fin = c + Vec2::new(0.0, -700.0);
        city.build_road(&t, &super::super::super::roads::line(c + Vec2::new(super::super::super::BLOCK * 1.5, -super::super::super::BLOCK * 1.5), fin), super::super::super::roads::RoadKind::Dirt).ok();
        let (bc, bf) = free_site(&city, &t, Kind::Basurero, fin);
        city.place_service(&t, Kind::Basurero, bc, bf).unwrap();
        city.economy.active.push(Ordinance::RecoleccionBasura);
        for _ in 0..12 {
            city.saneamiento_month(&t, &mut e);
        }
        assert!(city.services.barrio.basurales.len() < n, "las carretillas los levantan: {n} → {}", city.services.barrio.basurales.len());
    }
}
