//! Las épocas de la ciudad (GDD §4.2): la mecánica central de progresión. La ciudad guarda en
//! qué época está ([`EstadoEpoca`]); cada época desbloquea edificios, tecnologías, ordenanzas,
//! las tablas de llegadas de la población, el alumbrado y los materiales, y el paso a la
//! siguiente lo decide el jugador cuando se cumplen los hitos (ver `docs/epocas/`).
//!
//! Por ahora se juegan dos: la **plaza fuerte colonial** (1726–1830) y la **Ciudad Nueva**
//! (1830–1880). El paso a la segunda:
//! - Requisitos (`docs/epocas/01-colonial.md` §9): varios miles de vecinos (acá, 1.500), la
//!   iglesia matriz terminada, muelle y aduana (comercio del puerto) y el acontecimiento del
//!   **nuevo orden** (ficticio: la ciudad deja de depender de la Corona de ultramar).
//! - En el modo fiel a la historia el nuevo orden llega entre 1828 y 1830 (la Constitución
//!   es de 1830 y el decreto de demolición de la muralla, de 1829 [F]); en historia
//!   alternativa, cuando la ciudad está madura (iglesia terminada y más de 1.200 vecinos), con
//!   una probabilidad por mes.
//! - La decisión: qué hacer con la muralla (derribarla, conservar un tramo como monumento o
//!   conservarla entera). Después se abre el ejido al **ensanche** ([`ensanche`]).
//!
//! En la Ciudad Nueva: casas con fachada neoclásica e italianizante (`structures::estilo`),
//! rehabilitación de las casas viejas en el lugar ([`rehabilitar`]), usina y faroles de gas,
//! agua corriente y colectores ([`redes`]), teatro, y las llegadas de la época 2.

pub mod ensanche;
pub mod redes;
pub mod rehabilitar;
#[cfg(test)]
mod tests;

use super::poblacion::Epoca;
use super::{City, START_YEAR};
use crate::buildings::Edits;
use crate::palette::AIR;
use crate::terrain::TerrainGen;
use glam::IVec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Habitantes que pide el paso a la Ciudad Nueva [E: el diseño dice "varios miles"; con el
/// tamaño de las partidas del motor, 1.500].
pub const HABITANTES_EPOCA2: i32 = 1500;
/// En historia alternativa, el nuevo orden puede llegar con estos habitantes y la iglesia.
pub const HABITANTES_NUEVO_ORDEN: i32 = 1200;
/// Año desde el que llega el nuevo orden en el modo fiel (y el último: llega seguro).
pub const NUEVO_ORDEN_DESDE: u32 = 1828;
pub const NUEVO_ORDEN_HASTA: u32 = 1830;

/// Qué se hace con la muralla al pasar a la Ciudad Nueva.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Muralla {
    /// Derribarla toda: máximo espacio para el ensanche; la piedra vuelve al depósito.
    Derribar,
    /// Conservar el tramo más cercano a la plaza como monumento (turismo en épocas futuras).
    Monumento,
    /// Conservarla entera (sigue cobrando en los portones; el ensanche queda afuera).
    Conservar,
}

impl Muralla {
    pub const ALL: [Muralla; 3] = [Muralla::Derribar, Muralla::Monumento, Muralla::Conservar];
    pub fn nombre(self) -> &'static str {
        match self {
            Muralla::Derribar => "Derribar la muralla",
            Muralla::Monumento => "Conservar un tramo como monumento",
            Muralla::Conservar => "Conservar la muralla entera",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Muralla::Derribar => "Se derriba toda: los solares de su huella quedan libres y la piedra vuelve al depósito. Se pierde el patrimonio.",
            Muralla::Monumento => "Se derriba casi toda y queda el tramo más cercano a la plaza, como monumento: algo de piedra y un recuerdo para las épocas futuras.",
            Muralla::Conservar => "No se toca: los portones siguen cobrando, pero la ciudad crece afuera, por el ensanche.",
        }
    }
}

/// Resumen de una época terminada (pantalla de resumen y crónica).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Resumen {
    pub epoca: Epoca,
    pub desde: i32,
    pub hasta: i32,
    pub lineas: Vec<String>,
}

/// La época de la ciudad y lo que trae cada una (se guarda con la partida).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct EstadoEpoca {
    pub actual: Epoca,
    /// Mes absoluto en que empezó la época actual.
    pub desde_mes: u32,
    /// Mes del acontecimiento del nuevo orden (si ya ocurrió).
    pub nuevo_orden: Option<u32>,
    /// Ya se avisó en la gaceta que se puede pasar de época.
    pub avisada: bool,
    /// Lo que se decidió con la muralla.
    pub muralla: Option<Muralla>,
    /// Resúmenes de las épocas terminadas.
    pub resumenes: Vec<Resumen>,
    /// El resumen de la última época todavía no se mostró (la interfaz lo abre una vez).
    pub mostrar_resumen: bool,
    /// Trazados del ensanche, plazas y alamedas.
    pub ensanche: ensanche::Ensanche,
    /// Faroles de gas: columna de diseño y base.
    pub faroles_gas: Vec<([i32; 2], i32)>,
    /// Firma de las calles y usinas con que se repartieron los faroles de gas.
    pub gas_firma: (usize, i64, usize),
    /// Casas modernizadas en el lugar (desde el comienzo de la época).
    pub rehabilitadas: u32,
    /// Derivado: solares con agua corriente y con colector (se recalcula; no se guarda).
    #[serde(skip)]
    pub agua_red: BTreeSet<u32>,
    #[serde(skip)]
    pub cloaca: BTreeSet<u32>,
}

/// Un requisito para pasar de época.
#[derive(Clone, Debug, PartialEq)]
pub struct Requisito {
    pub texto: String,
    pub cumplido: bool,
}

/// Hash determinista de un mes (sin tocar el azar de la ciudad, para no cambiar la partida).
fn tirada(mes: u32, sal: u64) -> f32 {
    crate::structures::hash(IVec3::new(mes as i32, 7, 3), sal)
}

/// Época desde la que se puede aprobar cada ordenanza.
pub fn epoca_de_ordenanza(o: super::economy::Ordinance) -> Epoca {
    match o {
        super::economy::Ordinance::Fachadas => Epoca::CiudadNueva,
        _ => Epoca::Colonial,
    }
}

impl City {
    /// ¿La ciudad ya llegó a esta época?
    pub fn en_epoca(&self, e: Epoca) -> bool {
        self.epoca.actual >= e
    }

    /// Época siguiente (la única jugable por ahora es la Ciudad Nueva).
    pub fn epoca_siguiente(&self) -> Option<Epoca> {
        match self.epoca.actual {
            Epoca::Colonial => Some(Epoca::CiudadNueva),
            _ => None,
        }
    }

    /// Requisitos para pasar a la época siguiente.
    pub fn requisitos_epoca(&self) -> Vec<Requisito> {
        match self.epoca.actual {
            Epoca::Colonial => {
                let hab = self.habitantes();
                let muelle = self.services.port.pier.is_some();
                let aduana = self.services.count(super::services::Kind::Aduana) > 0;
                vec![
                    Requisito { texto: format!("{} habitantes (hay {})", super::game::fmt_thousands(HABITANTES_EPOCA2 as i64), super::game::fmt_thousands(hab as i64)), cumplido: hab >= HABITANTES_EPOCA2 },
                    Requisito { texto: "Iglesia matriz terminada".into(), cumplido: self.services.church.finished() },
                    Requisito { texto: "Comercio del puerto: muelle y aduana".into(), cumplido: muelle && aduana },
                    Requisito {
                        texto: match self.epoca.nuevo_orden {
                            Some(m) => format!("Nuevo orden (desde {} de {})", super::MONTHS[(m % 12) as usize], START_YEAR + m / 12),
                            None => "Acontecimiento del nuevo orden (la ciudad deja de depender de la Corona)".into(),
                        },
                        cumplido: self.epoca.nuevo_orden.is_some(),
                    },
                ]
            }
            _ => vec![Requisito { texto: "La época 3 (inmigración) todavía no se juega en el motor".into(), cumplido: false }],
        }
    }

    /// ¿Se cumplen todos los requisitos de la época siguiente?
    pub fn puede_pasar_epoca(&self) -> bool {
        self.epoca_siguiente().is_some() && self.requisitos_epoca().iter().all(|r| r.cumplido)
    }

    /// El mes de las épocas: el nuevo orden, el aviso de que se puede pasar, y en la Ciudad
    /// Nueva las redes (gas, agua, colectores) y la modernización de las casas viejas.
    pub fn epoca_mes(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        let anio = START_YEAR + self.month / 12;
        if self.epoca.actual == Epoca::Colonial && self.epoca.nuevo_orden.is_none() && self.plaza.is_some() {
            let llega = if self.poblacion.fiel_historia {
                anio >= NUEVO_ORDEN_HASTA || (anio >= NUEVO_ORDEN_DESDE && tirada(self.month, 0x0E0E) < 1.0 / 12.0)
            } else {
                self.services.church.finished() && self.habitantes() >= HABITANTES_NUEVO_ORDEN && tirada(self.month, 0x0E0F) < 1.0 / 36.0
            };
            if llega {
                self.epoca.nuevo_orden = Some(self.month);
                news.push("Nuevo orden: la Junta de la ciudad jura una carta propia y deja de responder a la Corona de ultramar. Se habla de derribar la muralla y de abrir el ejido".into());
            }
        }
        if self.epoca.actual == Epoca::Colonial && !self.epoca.avisada && self.puede_pasar_epoca() {
            self.epoca.avisada = true;
            news.push("La ciudad está lista para la Ciudad Nueva: decidí qué hacer con la muralla en el panel de la época".into());
        }
        if self.en_epoca(Epoca::CiudadNueva) {
            news.extend(self.redes_mes(t, e));
            news.extend(self.rehabilitar_mes(t, e));
        }
        news
    }

    /// Resumen de la época que termina (para la pantalla de resumen y la crónica).
    fn resumen_epoca(&self) -> Resumen {
        use super::lots::ZoneKind;
        let desde = START_YEAR as i32 + (self.epoca.desde_mes / 12) as i32;
        let hasta = START_YEAR as i32 + (self.month / 12) as i32;
        let mut lineas = vec![
            format!("{} habitantes en {} casas, {} comercios y {} talleres", super::game::fmt_thousands(self.habitantes() as i64), self.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Residencial)).count(), self.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Comercial)).count(), self.lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Industrial)).count()),
            format!("{} km de calles · {} edificios de servicio", (self.net.total_length() / 1000.0 * 10.0).round() / 10.0, self.services.buildings.len()),
        ];
        // Casas por tipo (las del catálogo con estilo).
        let mut tipos = std::collections::BTreeMap::<String, u32>::new();
        for l in self.lots.lots.values().filter(|l| l.level > 0) {
            if let Some(n) = self.lineas_estilo(l).into_iter().next() {
                let t = n.split(',').next().unwrap_or("").split(" con ").next().unwrap_or("").to_string();
                *tipos.entry(t).or_default() += 1;
            }
        }
        let mut v: Vec<(String, u32)> = tipos.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        if !v.is_empty() {
            lineas.push(format!("Casas: {}", v.iter().take(6).map(|(k, n)| format!("{k} ({n})")).collect::<Vec<_>>().join(", ")));
        }
        if self.services.church.finished() {
            lineas.push("La iglesia matriz, terminada, domina el perfil desde el río".into());
        }
        let tec = self.desarrollo.hechas.len();
        if tec > 0 {
            lineas.push(format!("{tec} tecnologías incorporadas"));
        }
        lineas.push(format!("Tesoro: {} pesos", super::game::fmt_thousands(self.money)));
        Resumen { epoca: self.epoca.actual, desde, hasta, lineas }
    }

    /// Pasa a la época siguiente con la decisión de la muralla. Con `forzar` no se miran los
    /// requisitos (demostración y pruebas). Devuelve los vóxeles que cambian y el aviso.
    pub fn pasar_epoca(&mut self, t: &TerrainGen, muralla: Muralla, forzar: bool) -> Result<(Edits, String), String> {
        let Some(sig) = self.epoca_siguiente() else { return Err("No hay otra época jugable todavía".into()) };
        if self.plaza.is_none() {
            return Err("Primero funda la plaza mayor".into());
        }
        if !forzar && !self.puede_pasar_epoca() {
            let faltan: Vec<String> = self.requisitos_epoca().into_iter().filter(|r| !r.cumplido).map(|r| r.texto).collect();
            return Err(format!("Todavía no: {}", faltan.join("; ")));
        }
        let mut res = self.resumen_epoca();
        let mut e = Edits::default();
        let piedra = self.decidir_muralla(t, muralla, &mut e);
        res.lineas.push(format!("La muralla: {}", muralla.nombre().to_lowercase()));
        self.epoca.resumenes.push(res);
        self.epoca.mostrar_resumen = true;
        self.epoca.actual = sig;
        self.epoca.desde_mes = self.month;
        self.epoca.muralla = Some(muralla);
        if self.epoca.nuevo_orden.is_none() {
            self.epoca.nuevo_orden = Some(self.month);
        }
        // La población llega con las tablas de la época (ver `poblacion::origenes`).
        self.poblacion.epoca = sig;
        let mut aviso = format!("Comienza la época «{}»: se abre el ejido al ensanche, llegan franceses, vascos e italianos, y se pueden levantar la usina de gas, el teatro, el agua corriente y los colectores", sig.nombre());
        if piedra > 0.0 {
            aviso.push_str(&format!(". La piedra de la muralla vuelve al depósito: {} carretadas", piedra.round() as i32));
        }
        self.poblacion.anotar(self.month as i32, format!("Fin de la época colonial. {}", aviso));
        Ok((e, aviso))
    }

    /// Derriba (o no) la muralla. Devuelve la piedra que vuelve al depósito.
    fn decidir_muralla(&mut self, t: &TerrainGen, m: Muralla, e: &mut Edits) -> f32 {
        if m == Muralla::Conservar || self.services.walls.tramos.is_empty() {
            return 0.0;
        }
        let plaza = self.lots.plaza_center.unwrap_or_default();
        let guardar = if m == Muralla::Monumento {
            self.services.walls.tramos.iter().enumerate().min_by(|a, b| ((a.1.a + a.1.b) * 0.5).distance(plaza).total_cmp(&((b.1.a + b.1.b) * 0.5).distance(plaza))).map(|x| x.0)
        } else {
            None
        };
        let mut piedra = 0.0;
        let tramos = std::mem::take(&mut self.services.walls.tramos);
        let mut quedan = Vec::new();
        let mut zonas = Vec::new();
        for (i, tr) in tramos.into_iter().enumerate() {
            if Some(i) == guardar {
                quedan.push(tr);
                continue;
            }
            piedra += tr.built * super::services::works::WALL_STONE_PER_M * 0.5;
            for &v in &tr.voxels {
                e.dset(v, AIR);
            }
            zonas.push((tr.a, tr.b));
        }
        self.services.walls.tramos = quedan;
        self.services.walls.gates_done = 0;
        self.lots.walls = self.services.walls.obstacles();
        if piedra > 0.0 {
            self.economy.stocks[super::economy::Res::Piedra as usize].amount += piedra;
        }
        // Los solares vuelven a la huella de la muralla: se generan a lo largo de las calles
        // que pasaban cerca.
        let cerca: Vec<u32> = self.net.edges.iter().filter(|(_, ed)| ed.pts.iter().any(|p| zonas.iter().any(|&(a, b)| super::roads::closest_on_segment(*p, a, b).distance(*p) < 60.0))).map(|(&id, _)| id).collect();
        let removed = self.lots.on_roads_changed(&self.net, t, &cerca);
        for l in &removed {
            for &v in &l.voxels {
                e.dset(v, AIR);
            }
        }
        piedra
    }
}
