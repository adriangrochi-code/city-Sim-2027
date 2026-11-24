//! Valor del suelo de cada solar y lo que la ciudad calcula con los recursos cada mes
//! ([`SueloCiudad`], dentro de `City`):
//!
//! - **Acceso**: distancia a la plaza mayor (el centro de todo en la ciudad colonial), al muelle
//!   y frente a calle empedrada.
//! - **Comodidades**: agua buena, salud y escuela cerca, la vista y el aire de la costa.
//! - **Molestias** ([`super::molestias`]): olor, ruido, humo y agua sucia lo bajan para las
//!   casas y algo menos para los comercios; a los talleres no les importan.
//! - **Por zona**: el comercio vive del paso de la gente (vecinos cerca, la plaza, el puerto);
//!   la industria busca otros talleres (se juntan), el puerto y la materia prima del suelo.
//!
//! El valor (0 a 1) decide qué solares crecen y suben de nivel primero (`Lots::grow_step`), los
//! alquileres (`poblacion::economia`) y, con las molestias, la felicidad y la salud de los
//! vecinos (`poblacion::ServiciosLote`). Todo es derivado: se recalcula cada mes, al construir
//! o demoler y al cargar; el valor se guarda además en la partida para que el crecimiento entre
//! dos meses sea el mismo antes y después de cargar.

use super::molestias::{perfil_de_lote, perfil_de_servicio, perfil_por_nombre, Campo, Cuencas, Fuente, Molestia};
use super::{produccion_sitio, rinde, Recursos, RecursosLote};
use super::materiales::ProduccionSitio;
use crate::city::lots::{Lot, ValorLote, ZoneKind};
use crate::city::roads::RoadKind;
use crate::city::City;
use crate::terrain::TerrainGen;
use glam::{IVec2, Vec2};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Lo que no cambia de un solar: sus recursos y la distancia a la costa.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Estatico {
    pub recursos: RecursosLote,
    pub costa: f32,
}

/// Recursos, molestias y entorno rural de la ciudad (derivado).
#[derive(Clone, Debug, Default)]
pub struct SueloCiudad {
    /// Recursos y costa de cada solar (no cambian: se calculan una vez por solar).
    pub estatico: BTreeMap<u32, Estatico>,
    /// Molestias en cada solar.
    pub molestias: BTreeMap<u32, Molestia>,
    /// Campo de molestias del mes (fuentes y arroyos sucios), para las capas.
    pub campo: Campo,
    /// Aptitud media del campo alrededor de la plaza (cultivo, pastoreo) y la plaza con que se
    /// calculó.
    pub entorno: Option<(Vec2, f32, f32)>,
    /// Lo que saca cada cantera, horno y calera (por id de edificio): su material y productos.
    pub sitios: BTreeMap<u32, ProduccionSitio>,
    cuencas: Option<(usize, Arc<Cuencas>)>,
}

/// Radio del entorno rural que abastece a la ciudad (m).
pub const RADIO_ENTORNO: f32 = 6000.0;
/// Aptitud de referencia (la del entorno típico): con ella el campo produce lo de siempre.
pub const REFERENCIA_CULTIVO: f32 = 0.5;
pub const REFERENCIA_PASTOREO: f32 = 0.6;

impl SueloCiudad {
    pub fn molestia_de(&self, lot: u32) -> Molestia {
        self.molestias.get(&lot).copied().unwrap_or_default()
    }

    /// Factor de la producción de las chacras y estancias privadas del entorno (0,8 a 1,2)
    /// según la tierra que rodea a la ciudad: (cultivo, pastoreo).
    pub fn factor_entorno(&self) -> (f32, f32) {
        match self.entorno {
            // Con la tierra de referencia, 1; con el doble, 1,2; con nada, 0,8.
            Some((_, c, p)) => ((0.8 + 0.2 * c / REFERENCIA_CULTIVO).clamp(0.8, 1.2), (0.8 + 0.2 * p / REFERENCIA_PASTOREO).clamp(0.8, 1.2)),
            None => (1.0, 1.0),
        }
    }

    /// Red de arroyos del terreno (se arma una vez por mapa).
    pub fn cuencas(&mut self, t: &TerrainGen) -> Arc<Cuencas> {
        let clave = t.arroyos.len() ^ if t.is_real() { 1 << 30 } else { 0 };
        match &self.cuencas {
            Some((k, c)) if *k == clave => c.clone(),
            _ => {
                let c = Arc::new(Cuencas::de(t));
                self.cuencas = Some((clave, c.clone()));
                c
            }
        }
    }
}

/// Grilla de solares para contar vecinos a una distancia.
struct Cerca {
    celda: f32,
    mapa: HashMap<IVec2, Vec<(Vec2, i32, bool)>>,
}

impl Cerca {
    fn new(lots: &BTreeMap<u32, Lot>) -> Cerca {
        let celda = 100.0;
        let mut mapa: HashMap<IVec2, Vec<(Vec2, i32, bool)>> = HashMap::new();
        for l in lots.values().filter(|l| l.level > 0) {
            mapa.entry((l.c / celda).floor().as_ivec2()).or_default().push((l.c, l.residents(), l.kind() == Some(ZoneKind::Industrial)));
        }
        Cerca { celda, mapa }
    }
    /// (vecinos, talleres) a menos de `r` m (sin el propio solar).
    fn contar(&self, p: Vec2, r: f32) -> (i32, i32) {
        let c = (p / self.celda).floor().as_ivec2();
        let k = (r / self.celda).ceil() as i32;
        let (mut gente, mut talleres) = (0, 0);
        for dz in -k..=k {
            for dx in -k..=k {
                if let Some(v) = self.mapa.get(&(c + IVec2::new(dx, dz))) {
                    for &(q, n, ind) in v {
                        let d = q.distance(p);
                        if d > 0.5 && d <= r {
                            gente += n;
                            talleres += ind as i32;
                        }
                    }
                }
            }
        }
        (gente, talleres)
    }
}

/// Componentes del valor de un solar (para la ficha y las pruebas).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Desglose {
    pub acceso: f32,
    pub puerto: f32,
    pub comodidades: f32,
    pub molestias: f32,
    pub zona: f32,
    pub valor: f32,
}

impl City {
    /// Recalcula recursos, molestias y valor del suelo de todos los solares, el rinde de los
    /// edificios del campo y la tierra del entorno. Solo depende del estado guardado y del
    /// terreno (es determinista). Se llama cada mes, al construir o demoler y al cargar.
    pub fn refresh_suelo(&mut self, t: &TerrainGen) {
        let rec = Recursos::de(t);
        let cuencas = self.suelo.cuencas(t);
        // Recursos de cada solar (una vez).
        let lots = &self.lots.lots;
        self.suelo.estatico.retain(|id, _| lots.contains_key(id));
        for l in lots.values() {
            self.suelo.estatico.entry(l.id).or_insert_with(|| Estatico { recursos: rec.en_lote(l), costa: t.sea_distance(l.c.x, l.c.y) });
        }
        // Rinde de los edificios del campo y la industria (los que no lo tienen).
        for b in self.services.buildings.values_mut() {
            if b.rinde < 0.0 {
                let i = b.info();
                b.rinde = rinde(b.kind, &rec.en_area(b.c, b.f, i.size.0, i.size.1));
            }
        }
        // Material y productos de las canteras, hornos y caleras (una vez por edificio).
        let edificios = &self.services.buildings;
        self.suelo.sitios.retain(|id, _| edificios.contains_key(id));
        for b in edificios.values() {
            if !self.suelo.sitios.contains_key(&b.id) {
                let i = b.info();
                if let Some(p) = produccion_sitio(b.kind, &rec.en_area(b.c, b.f, i.size.0, i.size.1)) {
                    self.suelo.sitios.insert(b.id, p);
                }
            }
        }
        // El campo alrededor de la plaza.
        if let Some(p) = self.lots.plaza_center {
            if self.suelo.entorno.is_none_or(|e| e.0 != p) {
                let (c, pa) = rec.media_entorno(p, RADIO_ENTORNO);
                self.suelo.entorno = Some((p, c, pa));
            }
        }
        // Fuentes de molestias: talleres, servicios que molestan y el muelle.
        let mut fuentes: Vec<Fuente> = Vec::new();
        for l in lots.values() {
            if let Some(p) = perfil_de_lote(l) {
                fuentes.push(Fuente { pos: l.c, perfil: *p, peso: 1.0 });
            }
        }
        for b in self.services.buildings.values() {
            if let Some(p) = perfil_de_servicio(b.kind) {
                fuentes.push(Fuente { pos: b.c, perfil: *p, peso: 1.0 });
            }
        }
        if let (Some(m), Some(p)) = (&self.services.port.pier, perfil_por_nombre("muelle")) {
            fuentes.push(Fuente { pos: m.land, perfil: *p, peso: 1.0 });
        }
        self.suelo.campo = Campo::new(fuentes, Some(&cuencas));
        // Molestias y valor de cada solar.
        let cerca = Cerca::new(lots);
        let mut molestias = BTreeMap::new();
        let mut valores = HashMap::new();
        for l in lots.values() {
            let ind = l.kind() == Some(ZoneKind::Industrial);
            let m = self.suelo.campo.en(l.c, ind.then_some(l.c));
            let d = self.desglose_valor(l, &m, &cerca);
            let atractivo = 0.55 + 0.9 * d.valor;
            valores.insert(l.id, ValorLote { valor: d.valor, molestia: m.total(), atractivo });
            molestias.insert(l.id, m);
        }
        self.suelo.molestias = molestias;
        self.lots.valor = valores;
    }

    /// Aptitud de cada solar vacío para una zona (0 a 1): el valor del suelo que tendría si se
    /// zonificara de ese tipo (industria: talleres juntos, puerto, materia prima; comercio: paso
    /// de la gente; casas: plaza, servicios y sin molestias). Para teñir los solares al zonificar.
    pub fn aptitud_zona(&self, k: ZoneKind) -> HashMap<u32, f32> {
        let cerca = Cerca::new(&self.lots.lots);
        let mut out = HashMap::new();
        for l in self.lots.lots.values().filter(|l| l.level == 0) {
            let mut l2 = l.clone();
            l2.zone = super::super::lots::zone_code(k, super::super::lots::Density::Baja);
            let ind = k == ZoneKind::Industrial;
            let m = self.suelo.campo.en(l.c, ind.then_some(l.c));
            out.insert(l.id, self.desglose_valor(&l2, &m, &cerca).valor);
        }
        out
    }

    /// Valor del suelo de un solar con sus componentes.
    fn desglose_valor(&self, l: &Lot, m: &Molestia, cerca: &Cerca) -> Desglose {
        let est = self.suelo.estatico.get(&l.id).copied().unwrap_or_default();
        let plaza = self.lots.plaza_center.map_or(0.15, |p| (-l.c.distance(p) / 700.0).exp());
        let puerto = self.services.port.pier.as_ref().map_or(0.0, |p| (-l.c.distance(p.land) / 450.0).exp());
        let calle = match self.lots.frontage(l, &self.net) {
            Some(RoadKind::Stone) => 0.06,
            Some(RoadKind::Dirt) => 0.0,
            None => -0.08,
        };
        let br = &self.services.barrio;
        let agua = self.services.water_of(l.id);
        let costa = 0.05 * (1.0 - (est.costa / 250.0).clamp(0.0, 1.0));
        let molestia = m.total();
        let (gente, talleres) = cerca.contar(l.c, 150.0);
        // Época 2: el teatro sube el valor alrededor (la "ciudad culta"), y las plazas del
        // ensanche, el de su manzana [E].
        let teatro = self.services.of_kind(super::super::services::Kind::Teatro).map(|b| 0.14 * (-l.c.distance(b.c) / 350.0).exp()).fold(0.0f32, f32::max);
        let plaza_e = if self.epoca.ensanche.plazas.iter().any(|(c, _)| glam::Vec2::from(*c).distance(l.c) < 110.0) { 0.04 } else { 0.0 };
        let costa = costa + teatro + plaza_e + self.valor_epoca3(l.c); // [epoca3] estación y tranvías
        let (acceso, comodidades, castigo, zona) = match l.kind() {
            Some(ZoneKind::Industrial) => {
                // Talleres juntos (proveedores, oficiales, carretas), el puerto y la materia
                // prima del suelo.
                let junto = (talleres.min(5) as f32 / 5.0) * 0.15;
                (0.4 * plaza + calle, 0.0, 0.0, junto + 0.22 * puerto + 0.18 * est.recursos.materia_prima())
            }
            Some(ZoneKind::Comercial) => {
                // El paso de la gente: vecinos a 150 m, la plaza y el muelle.
                let paso = (gente as f32 / 120.0).min(1.0) * 0.18;
                (0.68 * plaza + calle, 0.05 * agua, 0.25 * molestia, paso + 0.15 * puerto)
            }
            // Las casas: la plaza, el muelle (los comerciantes viven cerca del puerto), el agua,
            // los servicios y la costa.
            _ => (0.78 * plaza + 0.08 * puerto + calle, 0.08 * agua + 0.03 * br.salud_de(l.id) + 0.03 * br.escuela_de(l.id) + costa, 0.5 * molestia, 0.0),
        };
        let valor = (acceso + comodidades + zona - castigo).clamp(0.0, 1.0);
        Desglose { acceso, puerto, comodidades, molestias: castigo, zona, valor }
    }

    /// Valor del suelo de un solar (0 a 1), como lo calculó el último mes.
    #[allow(dead_code)] // API para el sistema de industrias (todavía sin enchufar)
    pub fn valor_del_suelo(&self, lot: u32) -> f32 {
        self.lots.lots.get(&lot).map_or(0.0, |l| self.lots.land_value(l))
    }

    /// Recursos del suelo de un solar (calculados con el terreno; ceros si todavía no).
    #[allow(dead_code)] // API para el sistema de industrias (todavía sin enchufar)
    pub fn recursos_del_lote(&self, lot: u32) -> RecursosLote {
        self.suelo.estatico.get(&lot).map_or_else(RecursosLote::default, |e| e.recursos)
    }

    /// Líneas de la ficha de un solar: molestias, recursos y componentes del valor.
    pub fn lineas_suelo_lote(&self, lot: u32) -> Vec<String> {
        let mut out = Vec::new();
        let Some(l) = self.lots.lots.get(&lot) else { return out };
        let m = self.suelo.molestia_de(lot);
        if m.total() >= 0.03 {
            out.push(format!(
                "Molestias: {} % (olor {} · ruido {} · humo {} · agua sucia {}){}",
                (m.total() * 100.0).round() as i32,
                pct(m.olor),
                pct(m.ruido),
                pct(m.humo),
                pct(m.agua),
                if l.kind() == Some(ZoneKind::Residencial) && m.total() > 0.3 { " — nadie quiere vivir acá" } else { "" }
            ));
        } else {
            out.push("Molestias: ninguna".into());
        }
        if let Some(e) = self.suelo.estatico.get(&lot) {
            let r = e.recursos;
            if l.kind() == Some(ZoneKind::Industrial) {
                out.push(format!("Materia prima: piedra {} · arcilla {} · arena {}", super::calidad(r.piedra), super::calidad(r.arcilla), super::calidad(r.arena)));
            }
            out.push(format!("Tierra: IP {} · cultivo {} · pasturas {}", r.ip.round() as i32, super::calidad(r.cultivo), super::calidad(r.pastoreo)));
        }
        if self.lots.valor.contains_key(&lot) {
            let cerca = Cerca::new(&self.lots.lots);
            let d = self.desglose_valor(l, &m, &cerca);
            out.push(format!(
                "Valor: acceso {:+} · servicios {:+} · {} {:+} · molestias −{}",
                (d.acceso * 100.0).round() as i32,
                (d.comodidades * 100.0).round() as i32,
                match l.kind() {
                    Some(ZoneKind::Industrial) => "talleres, puerto y materia prima",
                    Some(ZoneKind::Comercial) => "paso de gente y puerto",
                    _ => "zona",
                },
                (d.zona * 100.0).round() as i32,
                (d.molestias * 100.0).round() as i32
            ));
        }
        out
    }
}

fn pct(x: f32) -> String {
    format!("{} %", (x * 100.0).round() as i32)
}
