//! Vecinos y vehículos de la época colonial: gente a pie, jinetes, carretas de bueyes y
//! carros. Porte de `scripts/agents/agents.gd` del prototipo de Godot (diseño en
//! `docs/sistemas/agentes-y-conduccion.md`).
//!
//! - **Vecinos**: salen de los solares residenciales habitados (tantos como habitantes tiene
//!   la casa según su nivel y densidad), con casa, trabajo, horario y temperamento fijos.
//! - **Rutina**: de lunes a sábado, al trabajo (comercio, taller, campo o puerto) y de vuelta;
//!   algunos pasan por el mercado de la plaza al mediodía; los domingos, misa en la iglesia
//!   matriz. De noche casi nadie sale. Las carretas traen cueros y trigo del campo de día.
//! - **Rutas**: A* sobre la red de calles ([`ruta`]): el más corto a pie, el más rápido según
//!   el tiempo medido en cada carril para los vehículos.
//! - **Conducción**: modelo de conductor inteligente ([`idm`]) entre los vehículos de un mismo
//!   carril; ceden el paso en los cruces por orden de llegada sin bloquearlos; aflojan en
//!   curvas y giros.
//!
//! La simulación es determinista: azar propio (guardado), orden de los agentes por id y
//! colas por carril ordenadas. Solo depende del estado guardado, de la hora del día y del
//! foco de la cámara (que elige qué agentes se simulan con detalle).

pub mod dibujo;
pub mod fauna; // [personajes]
pub mod idm;
pub mod ruta;

use super::lots::{Lots, ZoneKind, D as LOT_D};
use super::roads::{RoadKind, RoadNetwork};
use crate::buildings::Rng;
use crate::fxhash::FxBuild;
use crate::terrain::TerrainGen;
use glam::Vec2;
pub use idm::Tipo;
use ruta::{carril, Router, Tiempos};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Los agentes se mueven este múltiplo del tiempo real (como en el prototipo): un día de
/// juego dura 8 minutos, así que una hora del reloj son 200 s simulados.
pub const ESCALA_TIEMPO: f32 = 10.0;
/// Paso fijo de la simulación de movimiento (segundos simulados).
pub const PASO: f32 = 0.25;
/// Pasos como máximo por llamada (a velocidad alta o en el avance rápido se descarta el resto).
pub const MAX_PASOS: u32 = 48;
/// Tope de agentes en la calle a la vez.
pub const MAX_AGENTES: usize = 3000;
/// Distancia a la que se pide paso en un cruce (m).
const ACERCAMIENTO: f32 = 22.0;
/// Más lejos que esto del foco de la cámara, los agentes se simulan de a saltos.
pub const LOD_LEJOS: f32 = 900.0;
const LOD_CADA: u64 = 4;
/// Minutos del reloj del juego por segundo simulado: (24 h / 480 s reales) / escala.
pub const MINUTOS_POR_SEGUNDO: f32 = 24.0 * 60.0 / 480.0 / ESCALA_TIEMPO;

/// Temperamento: (nombre, factor de velocidad deseada, factor de distancia de seguridad).
pub const TEMPLES: [(&str, f32, f32); 3] = [("prudente", 0.9, 1.35), ("tranquilo", 1.0, 1.0), ("apurado", 1.12, 0.75)];

const NOMBRES: [&str; 24] = ["Juan", "José", "Francisco", "Manuel", "Pedro", "Domingo", "Bartolomé", "Tomás", "Ignacio", "Antonio", "Luis", "Miguel", "María", "Josefa", "Antonia", "Ana", "Isabel", "Catalina", "Rosa", "Juana", "Petrona", "Manuela", "Teresa", "Francisca"];
const APELLIDOS: [&str; 22] = ["Pérez", "Gómez", "Fernández", "Rodríguez", "Martínez", "Suárez", "Acosta", "Medina", "Correa", "Sosa", "Benítez", "Cabrera", "Díaz", "Ferreira", "Pereira", "Machado", "Hernández", "Morales", "Silva", "Viera", "Larrañaga", "Durán"];
const ORIGENES: [&str; 4] = ["criollo de la ciudad", "llegado de las Islas Atlánticas", "llegado de la Península", "llegado del interior"];
const OFICIOS: [&str; 10] = ["artesano", "pulpero", "jornalero", "carpintero", "herrero", "panadero", "lavandera", "costurera", "peón", "albañil"];

/// Qué hace en el día.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Rol {
    /// Sale a trabajar.
    Fuera,
    /// Trabaja en su casa-taller (en la ciudad colonial, la mayoría).
    EnCasa,
    /// Niño o anciano.
    NoTrabaja,
}

/// Dónde trabaja quien sale.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Trabajo {
    Ninguno,
    /// Un comercio o taller de la ciudad.
    Solar(u32),
    /// Chacras y estancias: sale por el camino al campo.
    Campo,
    /// La orilla del puerto (barracas y lanchones).
    Puerto,
}

/// Dónde está el vecino (para los viajes de vuelta).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Lugar {
    Casa,
    Trabajo,
    Plaza,
    Iglesia,
}

/// Para qué va: el motivo que se muestra al inspeccionar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Proposito {
    AlTrabajo,
    VuelveDelTrabajo,
    AlMercado,
    VuelveDelMercado,
    AMisa,
    VuelveDeMisa,
    /// Carreta que trae cueros y trigo del campo al mercado.
    TraeDelCampo,
    VuelveAlCampo,
    /// Carro que reparte mercancías a una tienda.
    Reparto,
    VuelveDelReparto,
    /// Viaje de prueba (recorrido automático y pruebas).
    Paseo,
}

impl Proposito {
    pub fn texto(self) -> &'static str {
        match self {
            Proposito::AlTrabajo => "al trabajo",
            Proposito::VuelveDelTrabajo => "de vuelta a casa",
            Proposito::AlMercado => "al mercado de la plaza",
            Proposito::VuelveDelMercado => "de vuelta del mercado",
            Proposito::AMisa => "a misa en la iglesia matriz",
            Proposito::VuelveDeMisa => "de vuelta de misa",
            Proposito::TraeDelCampo => "trae cueros y trigo del campo al mercado",
            Proposito::VuelveAlCampo => "vuelve al campo con mercancías",
            Proposito::Reparto => "lleva mercancías a una tienda",
            Proposito::VuelveDelReparto => "vuelve vacío",
            Proposito::Paseo => "de paseo",
        }
    }
}

/// Estado que se ve al inspeccionar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Marcha {
    EnMarcha,
    CediendoElPaso,
    EsperandoEnLaFila,
}

impl Marcha {
    pub fn texto(self) -> &'static str {
        match self {
            Marcha::EnMarcha => "en marcha",
            Marcha::CediendoElPaso => "cediendo el paso",
            Marcha::EsperandoEnLaFila => "esperando en la fila",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vecino {
    pub casa: u32,
    /// Persona del módulo de demografía, cuando exista (hoy los vecinos son sintéticos).
    #[serde(default)]
    pub persona: Option<u32>,
    /// El trabajo lo asignó otro sistema ([`Agentes::asignar_trabajo`]): no se reelige.
    #[serde(default)]
    pub trabajo_fijo: bool,
    /// Nombre y apellido (índices), origen y oficio.
    pub nombre: (u8, u8),
    pub origen: u8,
    pub oficio: u8,
    pub rol: Rol,
    pub trabajo: Trabajo,
    /// Cómo viaja cuando el trayecto es largo (para lo corto, todos caminan).
    pub tipo: Tipo,
    pub temple: u8,
    /// Horario habitual de salida y de vuelta (horas).
    pub entrada: f32,
    pub salida: f32,
    pub semilla_ruta: u32,
    pub cambios_de_ruta: u16,
    pub lugar: Lugar,
    /// Último viaje: (motivo, minutos que tardó, minutos que esperaba).
    pub ultimo: Option<(Proposito, u16, u16)>,
    pub ropa: u8,
}

impl Vecino {
    pub fn nombre(&self) -> String {
        format!("{} {}", NOMBRES[self.nombre.0 as usize % NOMBRES.len()], APELLIDOS[self.nombre.1 as usize % APELLIDOS.len()])
    }
}

/// Un tramo de la ruta: `s` se mide en el sentido de la marcha (0 = de `a` a `b`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pata {
    pub tramo: u32,
    pub sentido: u8,
    pub desde: f32,
    pub hasta: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agente {
    pub id: u32,
    pub tipo: Tipo,
    pub patas: Vec<Pata>,
    pub li: u16,
    /// Posición del frente (m) en la pata actual y velocidad (m/s).
    pub s: f32,
    pub v: f32,
    /// Velocidad deseada y separación propias (tipo, temperamento y algo de cada uno).
    pub v0: f32,
    pub t: f32,
    pub temple: u8,
    pub vecino: Option<u64>,
    /// Persona del módulo de demografía (si el vecino tiene una).
    #[serde(default)]
    pub persona: Option<u32>,
    pub proposito: Proposito,
    /// Reloj simulado al salir, tiempo previsto (s) y tiempo detenido (s).
    pub t0: f64,
    pub previsto: f32,
    pub espera: f32,
    /// Cuándo llegó a la fila del cruce (−1 si no está pidiendo paso).
    pub llegada: f64,
    pub t_carril: f64,
    pub marcha: Marcha,
    pub ropa: u8,
    pub pelaje: u8,
    /// Carretero o jinete sin vecino asignado (nombre).
    pub conductor: (u8, u8),
    /// Posición y dirección en el paso anterior (para interpolar el dibujo; no se guarda).
    #[serde(skip)]
    pub antes: Option<(Vec2, Vec2)>,
}

impl PartialEq for Agente {
    fn eq(&self, o: &Self) -> bool {
        // Todo menos `antes`, que es solo del dibujo.
        (self.id, self.tipo, &self.patas, self.li, self.s, self.v, self.v0, self.t, self.temple, self.vecino, self.persona)
            == (o.id, o.tipo, &o.patas, o.li, o.s, o.v, o.v0, o.t, o.temple, o.vecino, o.persona)
            && (self.proposito, self.t0) == (o.proposito, o.t0)
            && (self.previsto, self.espera, self.llegada, self.t_carril, self.marcha, self.ropa, self.pelaje, self.conductor) == (o.previsto, o.espera, o.llegada, o.t_carril, o.marcha, o.ropa, o.pelaje, o.conductor)
    }
}

impl Agente {
    pub fn params(&self) -> idm::Params {
        self.tipo.params()
    }
    pub fn pata(&self) -> Pata {
        self.patas[self.li as usize]
    }
    fn ultima(&self) -> bool {
        self.li as usize + 1 >= self.patas.len()
    }
}

/// Quién sale en un viaje de la agenda.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Quien {
    Vecino(u64),
    Carreta,
    Carro,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Programado {
    pub hora: f32,
    pub quien: Quien,
    pub que: Proposito,
}

/// Viaje de vuelta que sale más tarde (reloj simulado).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pendiente {
    pub cuando: f64,
    pub tipo: Tipo,
    pub desde: [f32; 2],
    pub hasta: [f32; 2],
    pub que: Proposito,
    pub temple: u8,
    pub conductor: (u8, u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Estadisticas {
    pub viajes_total: u64,
    pub viajes_hoy: u32,
    pub viajes_ayer: u32,
    /// Viajes que no pudieron salir (sin calle cerca, sin camino o con el tope de agentes).
    pub omitidos_hoy: u32,
    /// De esos, los que no tenían ninguna calle cerca (de la casa o del destino).
    #[serde(default)]
    pub sin_calle_hoy: u32,
    /// Minutos de viaje sumados hoy y viajes que tardaron más de 1,5 veces lo previsto.
    pub minutos_hoy: f32,
    pub demorados_hoy: u32,
    pub minutos_ayer: f32,
    pub demorados_ayer: u32,
    /// Del mes en curso: viajes en vehículo y suma de (tardó / previsto), para la opinión.
    pub viajes_mes: u32,
    pub demora_mes: f32,
    pub carretas_al_mercado: u32,
}

/// Todo lo que se guarda con la partida.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Estado {
    pub vecinos: BTreeMap<u64, Vecino>,
    pub agentes: Vec<Agente>,
    pub tiempos: Tiempos,
    /// Segundos simulados desde la fundación.
    pub reloj: f64,
    /// Hora del día (la del reloj del juego) y días transcurridos (0 = lunes).
    pub hora: f32,
    pub dia: u32,
    pub agenda: Vec<Programado>,
    pub agenda_i: usize,
    pub agenda_dia: Option<u32>,
    pub pendientes: Vec<Pendiente>,
    /// Cruce → (agente que tiene el paso, su tiempo detenido al recibirlo).
    pub cruces: BTreeMap<u32, (u32, f32)>,
    pub azar: u64,
    pub proximo_id: u32,
    pub acumulado: f32,
    pub pasos: u64,
    pub stats: Estadisticas,
}

/// Puntos fijos del mapa para los viajes (se recalculan cuando cambia la red).
#[derive(Clone, Debug, Default)]
struct Lugares {
    /// Entrada desde el campo: el extremo de calle más alejado de la plaza.
    campo: Option<Vec2>,
    /// La calle más cercana a la orilla (el puerto, hasta que exista el muelle).
    puerto: Option<Vec2>,
    plaza: Option<Vec2>,
    iglesia: Option<Vec2>,
}

/// Versión de la red de calles vista (cambia al agregar o quitar tramos).
type VersionRed = (u32, u32, usize);

pub struct Agentes {
    pub e: Estado,
    router: Router,
    red: Option<VersionRed>,
    /// Altura de la superficie de cada tramo cada 2 m (calzada o tablero del puente).
    perfiles: HashMap<u32, Vec<f32>, FxBuild>,
    lugares: Lugares,
    firma_solares: (usize, i32),
    /// Centro de la vista: cerca se simula con detalle, lejos de a saltos.
    pub foco: Option<Vec2>,
    // [personajes]
    /// Detalles fijos que pone cualquier sistema (no se guardan; ver `detalle::Detalles`).
    pub detalles: crate::detalle::Detalles,
    /// Animales y detalles derivados de la ciudad, con la firma con que se armaron.
    escena: std::sync::Mutex<Option<(u64, std::sync::Arc<fauna::Escena>)>>,
    /// Muestra de detalles para las capturas: comienzo, dirección y largo de la fila.
    muestra: Option<(Vec2, Vec2, f32)>,
    // [/personajes]
}

impl Default for Agentes {
    fn default() -> Self {
        Agentes::from_estado(Estado { azar: Rng::new(2027).state(), hora: 8.0, proximo_id: 1, ..Default::default() })
    }
}

impl Clone for Agentes {
    fn clone(&self) -> Self {
        Agentes::from_estado(self.e.clone())
    }
}

impl std::fmt::Debug for Agentes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Agentes({} vecinos, {} en la calle)", self.e.vecinos.len(), self.e.agentes.len())
    }
}

/// Lo que se ve de un tramo de calle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfoTramo {
    pub por_tipo: [u32; 4],
    /// Tiempo de paso medido en cada sentido y el libre (s del reloj simulado).
    pub medido: [f32; 2],
    pub libre: f32,
}

fn rng_de(semilla: u64) -> Rng {
    Rng::new(semilla ^ 0x5DEE_CE66_D1CE_4E5B)
}

/// Normal aproximada (suma de tres uniformes), media 0 y desvío ~`d`.
fn normal(r: &mut Rng, d: f32) -> f32 {
    (r.f() + r.f() + r.f() - 1.5) * 2.0 * d
}

fn grado(net: &RoadNetwork, nodo: u32) -> usize {
    net.node_edges.get(&nodo).map_or(0, |v| v.len())
}

/// Nodo donde termina una pata.
fn fin(net: &RoadNetwork, p: &Pata) -> u32 {
    let e = &net.edges[&p.tramo];
    if p.sentido == 0 {
        e.b
    } else {
        e.a
    }
}

/// Frente de un solar (sobre la calle).
pub fn frente(l: &super::lots::Lot) -> Vec2 {
    l.c + l.f * (LOT_D * 0.5)
}

impl Agentes {
    pub fn from_estado(e: Estado) -> Agentes {
        Agentes { e, router: Router::default(), red: None, perfiles: HashMap::default(), lugares: Lugares::default(), firma_solares: (usize::MAX, -1), foco: None, detalles: Default::default(), escena: Default::default(), muestra: None }
    }

    fn rng(&mut self) -> Rng {
        Rng::from_state(self.e.azar)
    }

    fn azar(&mut self) -> f32 {
        let mut r = self.rng();
        let x = r.f();
        self.e.azar = r.state();
        x
    }

    fn azar_u(&mut self, n: u32) -> u32 {
        let mut r = self.rng();
        let x = (r.next() % n.max(1) as u64) as u32;
        self.e.azar = r.state();
        x
    }

    /// La hora del reloj del juego (la sesión la pasa en cada cuadro). Al pasar la
    /// medianoche empieza otro día.
    pub fn set_hora(&mut self, h: f32) {
        let h = h.rem_euclid(24.0);
        if h < self.e.hora - 12.0 {
            self.e.dia += 1;
        } else if h < self.e.hora {
            // Se atrasó el reloj (a mano): la agenda vuelve a esa hora.
            self.e.agenda_i = self.e.agenda.iter().position(|p| p.hora >= h - 0.01).unwrap_or(self.e.agenda.len());
        }
        self.e.hora = h;
    }

    pub fn dia_de_semana(&self) -> &'static str {
        ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"][(self.e.dia % 7) as usize]
    }

    /// Cuántos hay en la calle de cada tipo (orden de [`Tipo::TODOS`]).
    pub fn en_la_calle(&self) -> [u32; 4] {
        let mut n = [0; 4];
        for a in &self.e.agentes {
            n[a.tipo as usize] += 1;
        }
        n
    }

    /// Fracción de lo que se interpola entre el paso anterior y el actual (para el dibujo).
    pub fn alfa(&self) -> f32 {
        (self.e.acumulado / PASO).clamp(0.0, 1.0)
    }

    /// Avanza `dt` segundos de juego (ya multiplicados por la velocidad).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, t: &TerrainGen, net: &RoadNetwork, lots: &Lots, plaza: Option<(Vec2, f32)>, dt: f32) {
        let Some((pc, angle)) = plaza else { return };
        self.preparar(t, net, pc, angle);
        let firma = (lots.lots.len(), lots.population());
        if firma != self.firma_solares {
            self.firma_solares = firma;
            self.sincronizar_vecinos(lots);
        }
        if dt <= 0.0 {
            return;
        }
        if self.e.agenda_dia != Some(self.e.dia) {
            self.armar_agenda(lots);
        }
        self.salidas(net, lots);
        self.e.acumulado += dt * ESCALA_TIEMPO;
        let mut n = 0;
        while self.e.acumulado >= PASO {
            if n == MAX_PASOS {
                self.e.acumulado = 0.0;
                break;
            }
            self.e.acumulado -= PASO;
            self.paso(net, PASO);
            n += 1;
        }
    }

    /// Cuando cambia la red: quita los agentes cuyo camino ya no existe, olvida cachés y
    /// recalcula alturas y lugares.
    fn preparar(&mut self, t: &TerrainGen, net: &RoadNetwork, pc: Vec2, angle: f32) {
        let (nn, ne) = net.counters();
        let v = (nn, ne, net.edges.len());
        if self.red == Some(v) {
            return;
        }
        let primera = self.red.is_none();
        self.red = Some(v);
        if !primera {
            self.e.agentes.retain(|a| a.patas.iter().all(|p| net.edges.contains_key(&p.tramo)));
        }
        self.e.cruces.retain(|n, _| net.nodes.contains_key(n));
        self.e.tiempos.podar(net);
        self.router.vaciar_cache();
        self.perfiles.retain(|k, _| net.edges.contains_key(k));
        for (&id, e) in &net.edges {
            self.perfiles.entry(id).or_insert_with(|| dibujo::perfil(t, e));
        }
        self.lugares = lugares(t, net, pc, angle);
    }

    /// Un vecino por habitante de cada casa habitada; quien se mudó (casa demolida o más
    /// chica) deja de estar. Los atributos salen de la semilla del solar.
    fn sincronizar_vecinos(&mut self, lots: &Lots) {
        self.e.vecinos.retain(|&k, _| lots.lots.get(&((k >> 8) as u32)).is_some_and(|l| (k & 0xFF) < l.residents().max(0) as u64));
        let empleos: Vec<u32> = lots.lots.values().filter(|l| l.level > 0 && matches!(l.kind(), Some(ZoneKind::Comercial | ZoneKind::Industrial))).map(|l| l.id).collect();
        let mut nuevos = Vec::new();
        for l in lots.lots.values() {
            let n = l.residents().clamp(0, 255) as u64;
            for i in 0..n {
                let k = clave_vecino(l.id, i as u8);
                if self.e.vecinos.contains_key(&k) {
                    continue;
                }
                let v = habitante_de_lote(l, i as u8);
                self.e.vecinos.insert(k, v);
                nuevos.push(k);
            }
        }
        // Trabajo: un comercio o taller de la ciudad; algunos son peones del campo o
        // trabajan en la orilla del puerto.
        let keys: Vec<u64> = self.e.vecinos.keys().copied().collect();
        for k in keys {
            let v = &self.e.vecinos[&k];
            if v.rol != Rol::Fuera {
                continue;
            }
            if v.trabajo_fijo {
                continue;
            }
            let ok = match v.trabajo {
                Trabajo::Solar(id) => lots.lots.get(&id).is_some_and(|l| l.level > 0 && id != v.casa),
                Trabajo::Ninguno => false,
                _ => true,
            };
            if ok {
                continue;
            }
            let x = self.azar();
            let w = if x < 0.15 || empleos.is_empty() {
                Trabajo::Campo
            } else if x < 0.25 {
                Trabajo::Puerto
            } else {
                Trabajo::Solar(empleos[self.azar_u(empleos.len() as u32) as usize])
            };
            self.e.vecinos.get_mut(&k).unwrap().trabajo = w;
        }
        // Los que llegaron hoy se suman a lo que queda de la agenda del día.
        if self.e.agenda_dia == Some(self.e.dia) && !nuevos.is_empty() {
            let mut r = self.rng();
            let domingo = self.e.dia % 7 == 6;
            let mut ag = Vec::new();
            for k in nuevos {
                rutina(&mut r, k, &self.e.vecinos[&k], domingo, &mut ag);
            }
            self.e.azar = r.state();
            let hora = self.e.hora;
            ag.retain(|p| p.hora > hora);
            let mut resto = self.e.agenda.split_off(self.e.agenda_i);
            resto.extend(ag);
            resto.sort_by(|a, b| a.hora.total_cmp(&b.hora));
            self.e.agenda.extend(resto);
        }
    }

    /// Agenda del día: horarios de cada vecino con una pequeña variación diaria, el mercado,
    /// la misa del domingo, carretas del campo y carros de reparto.
    fn armar_agenda(&mut self, lots: &Lots) {
        self.e.stats.viajes_ayer = self.e.stats.viajes_hoy;
        self.e.stats.minutos_ayer = self.e.stats.minutos_hoy;
        self.e.stats.demorados_ayer = self.e.stats.demorados_hoy;
        self.e.stats.viajes_hoy = 0;
        self.e.stats.minutos_hoy = 0.0;
        self.e.stats.demorados_hoy = 0;
        self.e.stats.omitidos_hoy = 0;
        self.e.stats.sin_calle_hoy = 0;
        self.e.agenda_dia = Some(self.e.dia);
        self.router.vaciar_cache();
        let mut r = self.rng();
        let domingo = self.e.dia % 7 == 6;
        let mut ag = Vec::new();
        for (&k, v) in &self.e.vecinos {
            rutina(&mut r, k, v, domingo, &mut ag);
        }
        let pop = lots.population();
        let carretas = (4 + pop / 200).min(30) / if domingo { 3 } else { 1 };
        for _ in 0..carretas {
            ag.push(Programado { hora: 6.5 + r.f() * 9.0, quien: Quien::Carreta, que: Proposito::TraeDelCampo });
        }
        let tiendas = lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Comercial)).count() as i32;
        if !domingo && tiendas > 0 {
            for _ in 0..(2 + tiendas / 4).min(20) {
                ag.push(Programado { hora: 7.0 + r.f() * 8.0, quien: Quien::Carro, que: Proposito::Reparto });
            }
        }
        self.e.azar = r.state();
        ag.sort_by(|a, b| a.hora.total_cmp(&b.hora));
        self.e.agenda = ag;
        // Si el día ya empezó (al fundar o al cargar), no se largan de golpe los viajes pasados.
        self.e.agenda_i = self.e.agenda.iter().position(|p| p.hora >= self.e.hora - 0.01).unwrap_or(self.e.agenda.len());
    }

    /// Larga los viajes de la agenda que ya llegaron a su hora y las vueltas pendientes.
    fn salidas(&mut self, net: &RoadNetwork, lots: &Lots) {
        while self.e.agenda_i < self.e.agenda.len() && self.e.agenda[self.e.agenda_i].hora <= self.e.hora {
            let p = self.e.agenda[self.e.agenda_i];
            self.e.agenda_i += 1;
            match p.quien {
                Quien::Vecino(k) => self.viaje_de_vecino(net, lots, k, p.que),
                Quien::Carreta => {
                    let (Some(a), Some(b)) = (self.lugares.campo, self.lugares.plaza) else { continue };
                    let temple = self.azar_u(3) as u8;
                    let c = (self.azar_u(12) as u8, self.azar_u(APELLIDOS.len() as u32) as u8);
                    let sem = self.azar_u(100_000);
                    self.lanzar(net, Tipo::Carreta, a, b, None, Proposito::TraeDelCampo, sem, temple, c);
                }
                Quien::Carro => {
                    let tiendas: Vec<Vec2> = lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Comercial)).map(frente).collect();
                    let talleres: Vec<Vec2> = lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Industrial)).map(frente).collect();
                    let desde = self.lugares.puerto.or_else(|| talleres.first().copied()).or(self.lugares.campo);
                    let (Some(a), false) = (desde, tiendas.is_empty()) else { continue };
                    let b = tiendas[self.azar_u(tiendas.len() as u32) as usize];
                    let c = (self.azar_u(12) as u8, self.azar_u(APELLIDOS.len() as u32) as u8);
                    let sem = self.azar_u(100_000);
                    self.lanzar(net, Tipo::Carro, a, b, None, Proposito::Reparto, sem, 1, c);
                }
            }
        }
        let reloj = self.e.reloj;
        let mut i = 0;
        while i < self.e.pendientes.len() {
            if self.e.pendientes[i].cuando <= reloj {
                let p = self.e.pendientes.remove(i);
                let s = self.azar_u(100_000);
                self.lanzar(net, p.tipo, Vec2::from(p.desde), Vec2::from(p.hasta), None, p.que, s, p.temple, p.conductor);
            } else {
                i += 1;
            }
        }
    }

    fn punto_trabajo(&self, lots: &Lots, w: Trabajo) -> Option<Vec2> {
        match w {
            Trabajo::Ninguno => None,
            Trabajo::Solar(id) => lots.lots.get(&id).filter(|l| l.level > 0).map(frente),
            Trabajo::Campo => self.lugares.campo,
            Trabajo::Puerto => self.lugares.puerto.or(self.lugares.plaza),
        }
    }

    fn viaje_de_vecino(&mut self, net: &RoadNetwork, lots: &Lots, k: u64, que: Proposito) {
        let Some(v) = self.e.vecinos.get(&k).cloned() else { return };
        let Some(casa) = lots.lots.get(&v.casa).map(frente) else { return };
        let lejos = |l: Lugar| match l {
            Lugar::Trabajo => self.punto_trabajo(lots, v.trabajo),
            Lugar::Plaza => self.lugares.plaza,
            Lugar::Iglesia => self.lugares.iglesia,
            Lugar::Casa => Some(casa),
        };
        let (a, b, lugar) = match que {
            Proposito::AlTrabajo => (Some(casa), lejos(Lugar::Trabajo), Lugar::Trabajo),
            Proposito::AlMercado => (Some(casa), lejos(Lugar::Plaza), Lugar::Plaza),
            Proposito::AMisa => (Some(casa), lejos(Lugar::Iglesia), Lugar::Iglesia),
            Proposito::VuelveDelTrabajo | Proposito::VuelveDelMercado | Proposito::VuelveDeMisa => {
                if v.lugar == Lugar::Casa {
                    return;
                }
                (lejos(v.lugar), Some(casa), Lugar::Casa)
            }
            _ => return,
        };
        let (Some(a), Some(b)) = (a, b) else { return };
        // Para trayectos cortos hasta el jinete va a pie.
        let tipo = if a.distance(b) < 150.0 { Tipo::APie } else { v.tipo };
        if self.lanzar(net, tipo, a, b, Some(k), que, v.semilla_ruta, v.temple, v.nombre) {
            self.e.vecinos.get_mut(&k).unwrap().lugar = lugar;
        }
    }

    /// Planifica la ruta de `desde` a `hasta` como patas sobre la red: busca desde los dos
    /// extremos del tramo de salida hacia los dos del de llegada y se queda con la mejor.
    pub fn planificar(&mut self, net: &RoadNetwork, desde: Vec2, hasta: Vec2, percepcion: Option<u32>) -> Option<Vec<Pata>> {
        let n0 = net.nearest_edge(desde, 45.0)?;
        let n1 = net.nearest_edge(hasta, 70.0)?;
        let (e0, e1) = (&net.edges[&n0.edge], &net.edges[&n1.edge]);
        let s0 = e0.cum[n0.seg] + e0.pts[n0.seg].distance(n0.point);
        let s1 = e1.cum[n1.seg] + e1.pts[n1.seg].distance(n1.point);
        let (l0, l1) = (e0.len(), e1.len());
        if n0.edge == n1.edge {
            let p = if s1 >= s0 { Pata { tramo: n0.edge, sentido: 0, desde: s0, hasta: s1 } } else { Pata { tramo: n0.edge, sentido: 1, desde: l0 - s0, hasta: l0 - s1 } };
            return Some(vec![p]);
        }
        // Costo de las puntas: metros a pie, segundos a velocidad de referencia en vehículo.
        let unidad = |k: RoadKind| if percepcion.is_some() { 1.0 / ruta::ref_speed(k) } else { 1.0 };
        let mut best: Option<(f32, u32, u32, std::sync::Arc<ruta::Camino>)> = None;
        for (ini, c_ini) in [(e0.a, s0), (e0.b, l0 - s0)] {
            for (fin_, c_fin) in [(e1.a, s1), (e1.b, l1 - s1)] {
                let Some(c) = self.router.camino(net, &self.e.tiempos, ini, fin_, percepcion) else { continue };
                // No vale volver por el mismo tramo de salida o de llegada.
                if c.tramos.first().is_some_and(|t| t.0 == n0.edge) || c.tramos.last().is_some_and(|t| t.0 == n1.edge) {
                    continue;
                }
                let total = c_ini * unidad(e0.kind) + c.costo + c_fin * unidad(e1.kind);
                if best.as_ref().is_none_or(|b| total < b.0) {
                    best = Some((total, ini, fin_, c));
                }
            }
        }
        let (_, ini, fin_, c) = best?;
        let mut patas = Vec::with_capacity(c.tramos.len() + 2);
        patas.push(if ini == e0.b { Pata { tramo: n0.edge, sentido: 0, desde: s0, hasta: l0 } } else { Pata { tramo: n0.edge, sentido: 1, desde: l0 - s0, hasta: l0 } });
        for &(t, d) in &c.tramos {
            patas.push(Pata { tramo: t, sentido: d, desde: 0.0, hasta: net.edges[&t].len() });
        }
        patas.push(if fin_ == e1.a { Pata { tramo: n1.edge, sentido: 0, desde: 0.0, hasta: s1 } } else { Pata { tramo: n1.edge, sentido: 1, desde: 0.0, hasta: l1 - s1 } });
        patas.retain(|p| p.hasta - p.desde > 0.05);
        (!patas.is_empty()).then_some(patas)
    }

    /// Larga un viaje. Devuelve si salió.
    #[allow(clippy::too_many_arguments)]
    pub fn lanzar(&mut self, net: &RoadNetwork, tipo: Tipo, desde: Vec2, hasta: Vec2, vecino: Option<u64>, que: Proposito, semilla: u32, temple: u8, conductor: (u8, u8)) -> bool {
        if self.e.agentes.len() >= MAX_AGENTES {
            self.e.stats.omitidos_hoy += 1;
            return false;
        }
        let Some(patas) = self.planificar(net, desde, hasta, tipo.vehiculo().then_some(semilla)) else {
            self.e.stats.omitidos_hoy += 1;
            if net.nearest_edge(desde, 45.0).is_none() || net.nearest_edge(hasta, 70.0).is_none() {
                self.e.stats.sin_calle_hoy += 1;
            }
            return false;
        };
        let p = tipo.params();
        let (_, fv, ft) = TEMPLES[temple as usize % 3];
        let v0 = p.v0 * fv * (0.92 + 0.16 * self.azar());
        let previsto: f32 = patas.iter().map(|q| (q.hasta - q.desde) / v0).sum();
        let id = self.e.proximo_id;
        self.e.proximo_id += 1;
        let ropa = match vecino.and_then(|k| self.e.vecinos.get(&k)) {
            Some(v) => v.ropa,
            None => self.azar_u(dibujo::ROPA.len() as u32) as u8,
        };
        let pelaje = self.azar_u(dibujo::PELAJE.len() as u32) as u8;
        let s = patas[0].desde;
        self.e.agentes.push(Agente {
            id,
            tipo,
            patas,
            li: 0,
            s,
            v: 0.0,
            v0,
            t: p.t * ft,
            temple,
            vecino,
            persona: vecino.and_then(|k| self.e.vecinos.get(&k)).and_then(|v| v.persona),
            proposito: que,
            t0: self.e.reloj,
            previsto,
            espera: 0.0,
            llegada: -1.0,
            t_carril: self.e.reloj,
            marcha: Marcha::EnMarcha,
            ropa,
            pelaje,
            conductor,
            antes: None,
        });
        true
    }

    // --- Movimiento ----------------------------------------------------------------

    fn indice(&self, id: u32) -> Option<usize> {
        self.e.agentes.binary_search_by_key(&id, |a| a.id).ok()
    }

    /// Un paso fijo de la simulación de movimiento.
    fn paso(&mut self, net: &RoadNetwork, dt: f32) {
        self.e.reloj += dt as f64;
        self.e.pasos += 1;
        let pasos = self.e.pasos;
        let foco = self.foco;
        let n = self.e.agentes.len();
        let mut dts = vec![dt; n];
        if let Some(f) = foco {
            for (i, a) in self.e.agentes.iter_mut().enumerate() {
                // Cota de la distancia al foco: la de un extremo del tramo menos su largo.
                let e = &net.edges[&a.pata().tramo];
                if e.pts[0].distance(f) - e.len() > LOD_LEJOS {
                    // Nivel de detalle: lejos, un paso de cada LOD_CADA con dt multiplicado.
                    a.antes = None;
                    dts[i] = if (pasos + a.id as u64).is_multiple_of(LOD_CADA) { dt * LOD_CADA as f32 } else { 0.0 };
                    continue;
                }
                // Posición anterior (para interpolar el dibujo) de los que están cerca de la vista.
                let (p, d) = pose(net, a);
                a.antes = (p.distance(f) < dibujo::RADIO + 100.0).then_some((p, d));
            }
        }

        // 1. Carriles: vehículos ordenados por carril y del más adelantado al último.
        let ags = &self.e.agentes;
        let mut orden: Vec<(u32, f32, u32)> = (0..n).filter(|&i| ags[i].tipo.vehiculo()).map(|i| (carril(ags[i].pata().tramo, ags[i].pata().sentido), ags[i].s, i as u32)).collect();
        orden.sort_by(|x, y| x.0.cmp(&y.0).then(y.1.total_cmp(&x.1)).then(x.2.cmp(&y.2)));
        let mut rangos: HashMap<u32, (usize, usize), FxBuild> = HashMap::default();
        let mut lugar = vec![usize::MAX; n];
        for (k, &(c, _, i)) in orden.iter().enumerate() {
            rangos.entry(c).and_modify(|r| r.1 = k + 1).or_insert((k, k + 1));
            lugar[i as usize] = k;
        }
        let largo = |i: u32| ags[i as usize].params().largo;
        let ultimo_de = |c: u32| rangos.get(&c).map(|&(_, e)| orden[e - 1].2);

        // 2. Cruces: pide paso solo el primero de cada fila que va a cruzar; se concede por
        //    orden de llegada y solo si la calle de salida tiene lugar (no se bloquea el cruce).
        let mut pedidos: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for &(a0, b0) in rangos.values() {
            let Some(i) = (a0..b0).map(|k| orden[k].2 as usize).find(|&i| !ags[i].ultima()) else { continue };
            let a = &ags[i];
            let p = a.pata();
            if p.hasta - a.s > ACERCAMIENTO {
                continue;
            }
            let nodo = fin(net, &p);
            if grado(net, nodo) >= 3 {
                pedidos.entry(nodo).or_default().push(i);
            }
        }
        for v in pedidos.values_mut() {
            v.sort_unstable();
        }
        let salida_libre = |i: usize| {
            let a = &ags[i];
            let sig = a.patas[a.li as usize + 1];
            match ultimo_de(carril(sig.tramo, sig.sentido)) {
                None => true,
                Some(u) => ags[u as usize].s - largo(u) - sig.desde > a.params().largo + 3.0,
            }
        };
        let mut cruces = std::mem::take(&mut self.e.cruces);
        cruces.retain(|&nodo, &mut (id, espera0)| {
            let Some(i) = ags.binary_search_by_key(&id, |a| a.id).ok() else { return false };
            let a = &ags[i];
            let p = a.pata();
            let tiene = fin(net, &p) == nodo || (a.li > 0 && fin(net, &a.patas[a.li as usize - 1]) == nodo && a.s - p.desde < 7.0);
            // Salvaguarda: si quien tiene el paso no avanza en un minuto, lo cede.
            tiene && a.espera <= espera0 + 60.0
        });
        let mut llegadas: Vec<usize> = Vec::new();
        for (&nodo, reqs) in &pedidos {
            for &i in reqs {
                if ags[i].llegada < 0.0 {
                    llegadas.push(i);
                }
            }
            if cruces.contains_key(&nodo) {
                continue;
            }
            let reloj = self.e.reloj;
            let lleg = |i: usize| if ags[i].llegada < 0.0 { reloj } else { ags[i].llegada };
            if let Some(&i) = reqs.iter().filter(|&&i| salida_libre(i)).min_by(|&&x, &&y| lleg(x).total_cmp(&lleg(y)).then(ags[x].id.cmp(&ags[y].id))) {
                cruces.insert(nodo, (ags[i].id, ags[i].espera));
            }
        }

        // 3. Conducción (IDM), todos con el estado del paso anterior.
        let mut nuevo: Vec<(f32, f32, Marcha)> = Vec::with_capacity(n);
        for i in 0..n {
            let a = &ags[i];
            let dt_i = dts[i];
            if dt_i == 0.0 {
                nuevo.push((a.s, a.v, a.marcha));
                continue;
            }
            if !a.tipo.vehiculo() {
                nuevo.push((a.s + a.v0 * dt_i, a.v0, Marcha::EnMarcha));
                continue;
            }
            let p = a.params();
            let pata = a.pata();
            let edge = &net.edges[&pata.tramo];
            let hasta_fin = pata.hasta - a.s;
            let (mut hueco, mut v_lider) = (f32::INFINITY, 0.0);
            let k = lugar[i];
            let (c0, _) = rangos[&carril(pata.tramo, pata.sentido)];
            let mut marcha = Marcha::EnMarcha;
            if k > c0 {
                let l = orden[k - 1].2;
                hueco = ags[l as usize].s - largo(l) - a.s;
                v_lider = ags[l as usize].v;
            } else if !a.ultima() {
                let sig = a.patas[a.li as usize + 1];
                if let Some(u) = ultimo_de(carril(sig.tramo, sig.sentido)) {
                    hueco = hasta_fin + ags[u as usize].s - largo(u) - sig.desde;
                    v_lider = ags[u as usize].v;
                }
            }
            if !a.ultima() {
                // Ceder el paso: detenerse antes del cruce si no le toca.
                let nodo = fin(net, &pata);
                if grado(net, nodo) >= 3 && hasta_fin < ACERCAMIENTO && cruces.get(&nodo).map(|c| c.0) != Some(a.id) {
                    let stop = hasta_fin - 5.0;
                    if stop < hueco {
                        hueco = stop;
                        v_lider = 0.0;
                        if a.v < 0.3 {
                            marcha = Marcha::CediendoElPaso;
                        }
                    }
                }
            } else {
                // Llegar al destino frenando suave.
                let stop = hasta_fin + p.s0 + 0.3;
                if stop < hueco {
                    hueco = stop;
                    v_lider = 0.0;
                }
            }
            // Velocidad deseada: tipo de calle, curvas y giros.
            let mut v0 = a.v0 * if edge.kind == RoadKind::Dirt { 0.88 } else { 1.0 };
            v0 = v0.min(velocidad_en_curva(net, &pata, a.s));
            if !a.ultima() && hasta_fin < 18.0 && giro(net, &pata, &a.patas[a.li as usize + 1]) > 0.5 {
                v0 = v0.min(1.6);
            }
            let (ds, v) = idm::paso(&p, v0, a.t, a.v, hueco, v_lider, dt_i);
            if v < 0.1 && marcha == Marcha::EnMarcha {
                marcha = Marcha::EsperandoEnLaFila;
            }
            nuevo.push((a.s + ds, v, marcha));
        }
        let reloj = self.e.reloj;
        for i in llegadas {
            self.e.agentes[i].llegada = reloj;
        }
        self.e.cruces = cruces;
        for ((a, (s, v, m)), &dt_i) in self.e.agentes.iter_mut().zip(nuevo).zip(dts.iter()) {
            a.s = s;
            a.v = v;
            a.marcha = m;
            if a.tipo.vehiculo() && v < 0.1 {
                a.espera += dt_i;
            }
        }

        // 4. Cambios de tramo y llegadas.
        let mut llegados = Vec::new();
        let mut i = 0;
        while i < self.e.agentes.len() {
            match self.avanzar_patas(net, i) {
                Avance::Sigue => i += 1,
                Avance::Llego => llegados.push(self.e.agentes.remove(i)),
                Avance::Perdido => {
                    self.e.agentes.remove(i);
                }
            }
        }
        for a in llegados {
            self.llego(net, a);
        }
    }

    fn avanzar_patas(&mut self, net: &RoadNetwork, i: usize) -> Avance {
        let reloj = self.e.reloj;
        loop {
            let a = &mut self.e.agentes[i];
            let p = a.pata();
            if a.ultima() {
                return if a.s >= p.hasta - 0.6 { Avance::Llego } else { Avance::Sigue };
            }
            if a.s < p.hasta {
                return Avance::Sigue;
            }
            let sobra = a.s - p.hasta;
            // Tiempo medido en el carril (solo si lo recorrió entero).
            let medir = a.tipo.vehiculo() && p.desde < 0.01;
            let tardo = (reloj - a.t_carril) as f32;
            a.li += 1;
            a.t_carril = reloj;
            a.llegada = -1.0;
            let sig = a.pata();
            if !net.edges.contains_key(&sig.tramo) {
                return Avance::Perdido;
            }
            a.s = sig.desde + sobra;
            if medir {
                self.e.tiempos.registrar(net, p.tramo, p.sentido, tardo);
            }
        }
    }

    fn llego(&mut self, net: &RoadNetwork, a: Agente) {
        let st = &mut self.e.stats;
        let tardo = (self.e.reloj - a.t0) as f32;
        st.viajes_total += 1;
        st.viajes_hoy += 1;
        st.minutos_hoy += tardo * MINUTOS_POR_SEGUNDO;
        let demorado = tardo > a.previsto * 1.5 + 5.0;
        if demorado {
            st.demorados_hoy += 1;
        }
        if a.tipo.vehiculo() {
            st.viajes_mes += 1;
            st.demora_mes += (tardo / a.previsto.max(1.0)).min(5.0);
        }
        let aqui = pose(net, &a).0;
        if let Some(k) = a.vecino {
            let nueva = self.azar_u(100_000);
            if let Some(v) = self.e.vecinos.get_mut(&k) {
                v.ultimo = Some((a.proposito, (tardo * MINUTOS_POR_SEGUNDO).round() as u16, (a.previsto * MINUTOS_POR_SEGUNDO).round() as u16));
                // Aprende: si tardó mucho más de lo esperado, la próxima vez prueba otra ruta.
                if demorado && a.tipo.vehiculo() {
                    v.semilla_ruta = nueva;
                    v.cambios_de_ruta += 1;
                }
            }
            return;
        }
        // Carretas y carros: descargan y vuelven más tarde.
        let (vuelta, espera, destino) = match a.proposito {
            Proposito::TraeDelCampo => {
                self.e.stats.carretas_al_mercado += 1;
                (Proposito::VuelveAlCampo, 600.0 + 900.0 * self.azar(), self.lugares.campo)
            }
            Proposito::Reparto => (Proposito::VuelveDelReparto, 120.0 + 280.0 * self.azar(), self.lugares.puerto.or(self.lugares.campo)),
            _ => return,
        };
        if let Some(b) = destino {
            self.e.pendientes.push(Pendiente { cuando: self.e.reloj + espera as f64, tipo: a.tipo, desde: aqui.into(), hasta: b.into(), que: vuelta, temple: a.temple, conductor: a.conductor });
        }
    }

    /// Trabajo dado desde afuera (por ejemplo, por la demografía o un servicio): el vecino
    /// sale a trabajar a ese lugar y no se le reelige otro. Devuelve si el vecino existe.
    #[allow(dead_code)] // para la demografía (todavía no está en el motor)
    pub fn asignar_trabajo(&mut self, vecino: u64, trabajo: Trabajo, persona: Option<u32>) -> bool {
        let Some(v) = self.e.vecinos.get_mut(&vecino) else { return false };
        v.trabajo = trabajo;
        v.trabajo_fijo = trabajo != Trabajo::Ninguno;
        if trabajo != Trabajo::Ninguno {
            v.rol = Rol::Fuera;
        }
        if persona.is_some() {
            v.persona = persona;
        }
        true
    }

    // --- Consultas ---------------------------------------------------------------------

    /// Tránsito de un tramo: cuántos hay ahora de cada tipo y el tiempo de paso medido.
    pub fn info_tramo(&self, net: &RoadNetwork, tramo: u32) -> Option<InfoTramo> {
        net.edges.get(&tramo)?;
        let mut i = InfoTramo { libre: Tiempos::libre(net, tramo), ..Default::default() };
        for a in &self.e.agentes {
            if a.pata().tramo == tramo {
                i.por_tipo[a.tipo as usize] += 1;
            }
        }
        i.medido = [self.e.tiempos.costo(net, tramo, 0), self.e.tiempos.costo(net, tramo, 1)];
        Some(i)
    }

    /// Efecto del tránsito en la opinión de los vecinos este mes (y lo pone en cero): si los
    /// viajes en vehículo tardan mucho más de lo previsto, se quejan; si andan bien, nada.
    pub fn efecto_opinion(&mut self) -> f32 {
        let st = &mut self.e.stats;
        let r = if st.viajes_mes >= 20 { st.demora_mes / st.viajes_mes as f32 } else { 1.0 };
        st.viajes_mes = 0;
        st.demora_mes = 0.0;
        -((r - 1.3) * 8.0).clamp(0.0, 6.0)
    }

    /// Demora media de este mes (tardó / previsto, 1 = como se esperaba).
    pub fn demora_media(&self) -> Option<f32> {
        let st = &self.e.stats;
        (st.viajes_mes > 0).then(|| st.demora_mes / st.viajes_mes as f32)
    }

    /// Posición de un agente (para seguirlo con la cámara o inspeccionarlo).
    pub fn posicion(&self, net: &RoadNetwork, id: u32) -> Option<Vec2> {
        self.indice(id).map(|i| pose(net, &self.e.agentes[i]).0)
    }

    /// El agente más cercano a un punto (a menos de `radio` m).
    pub fn cerca_de(&self, net: &RoadNetwork, p: Vec2, radio: f32) -> Option<u32> {
        self.e.agentes.iter().map(|a| (pose(net, a).0.distance(p), a.id)).filter(|x| x.0 < radio).min_by(|x, y| x.0.total_cmp(&y.0)).map(|x| x.1)
    }

    /// Ficha de un agente: (título, líneas).
    pub fn describir(&self, id: u32) -> Option<(String, Vec<String>)> {
        let a = &self.e.agentes[self.indice(id)?];
        let mut l = Vec::new();
        let titulo = match a.vecino.and_then(|k| self.e.vecinos.get(&k)) {
            Some(v) => {
                l.push(format!("{}, {}", capitalizar(OFICIOS[v.oficio as usize % OFICIOS.len()]), ORIGENES[v.origen as usize % ORIGENES.len()]));
                l.push(format!("Temperamento: {}", TEMPLES[v.temple as usize % 3].0));
                v.nombre()
            }
            None => {
                let quien = format!("{} {}", NOMBRES[a.conductor.0 as usize % NOMBRES.len()], APELLIDOS[a.conductor.1 as usize % APELLIDOS.len()]);
                l.push(format!("Conduce: {quien}"));
                capitalizar(match a.tipo {
                    Tipo::Carreta => "carreta de bueyes",
                    Tipo::Carro => "carro de reparto",
                    Tipo::Jinete => "jinete",
                    Tipo::APie => "caminante",
                })
            }
        };
        l.push(format!("Va {}, {}", a.proposito.texto(), a.tipo.nombre()));
        l.push(format!("Estado: {} · {} km/h", a.marcha.texto(), (a.v * 3.6).round() as i32));
        l.push(format!("En camino: {} min (prevé {})", ((self.e.reloj - a.t0) as f32 * MINUTOS_POR_SEGUNDO) as i32, (a.previsto * MINUTOS_POR_SEGUNDO) as i32));
        Some((titulo, l))
    }
}

/// Clave de un vecino: solar de su casa y número de habitante dentro de ella.
pub fn clave_vecino(lote: u32, idx: u8) -> u64 {
    ((lote as u64) << 8) | idx as u64
}

/// Habitante `idx` de una casa, sintético: sus atributos salen de la semilla del solar (los
/// mismos siempre). Es el único lugar donde se inventan vecinos: cuando exista el módulo de
/// demografía, esto se reemplaza por las personas de los hogares del solar.
pub fn habitante_de_lote(l: &super::lots::Lot, idx: u8) -> Vecino {
    let mut r = rng_de(l.seed.wrapping_add((idx as u64).wrapping_mul(0x9E37_79B9)));
    let rico = l.level >= 3;
    let rol = if r.f() < 0.3 {
        Rol::Fuera
    } else if r.f() < 0.55 {
        Rol::EnCasa
    } else {
        Rol::NoTrabaja
    };
    let tipo = if r.f() < if rico { 0.4 } else { 0.08 } { Tipo::Jinete } else { Tipo::APie };
    Vecino {
        casa: l.id,
        persona: None,
        trabajo_fijo: false,
        nombre: ((r.next() % NOMBRES.len() as u64) as u8, (r.next() % APELLIDOS.len() as u64) as u8),
        origen: (r.next() % ORIGENES.len() as u64) as u8,
        oficio: (r.next() % OFICIOS.len() as u64) as u8,
        rol,
        trabajo: Trabajo::Ninguno,
        tipo,
        temple: (r.next() % 3) as u8,
        entrada: (6.4 + normal(&mut r, 0.7)).clamp(5.0, 9.0),
        salida: (17.2 + normal(&mut r, 0.7)).clamp(15.5, 19.5),
        semilla_ruta: (r.next() % 100_000) as u32,
        cambios_de_ruta: 0,
        lugar: Lugar::Casa,
        ultimo: None,
        ropa: (r.next() % dibujo::ROPA.len() as u64) as u8,
    }
}

/// Los viajes de un vecino en un día: trabajo (con una pequeña variación diaria sobre su
/// horario habitual), algún paso por el mercado y la misa del domingo.
fn rutina(r: &mut Rng, k: u64, v: &Vecino, domingo: bool, ag: &mut Vec<Programado>) {
    let jit = normal(r, 0.12);
    let q = Quien::Vecino(k);
    if domingo {
        if r.f() < 0.6 {
            let go = 9.2 + normal(r, 0.35);
            ag.push(Programado { hora: go, quien: q, que: Proposito::AMisa });
            ag.push(Programado { hora: go + 1.8 + r.f() * 0.6, quien: q, que: Proposito::VuelveDeMisa });
        }
        return;
    }
    if v.rol == Rol::Fuera {
        ag.push(Programado { hora: v.entrada + jit, quien: q, que: Proposito::AlTrabajo });
        ag.push(Programado { hora: v.salida + jit, quien: q, que: Proposito::VuelveDelTrabajo });
    }
    if r.f() < if v.rol == Rol::Fuera { 0.12 } else { 0.2 } {
        let m = 10.5 + r.f() * 1.5;
        ag.push(Programado { hora: m, quien: q, que: Proposito::AlMercado });
        ag.push(Programado { hora: m + 0.8 + r.f() * 0.7, quien: q, que: Proposito::VuelveDelMercado });
    }
}

/// Minutos del reloj del juego, redondeados, de unos segundos simulados.
fn minutos(seg: f32) -> String {
    let m = seg * MINUTOS_POR_SEGUNDO;
    if m < 10.0 {
        format!("{:.1} min", m).replace('.', ",")
    } else {
        format!("{} min", m.round() as i32)
    }
}

/// Líneas de la ficha de una calle: quién la está usando y cuánto se tarda en recorrerla.
pub fn lineas_de_tramo(ag: &Agentes, net: &RoadNetwork, tramo: u32) -> Vec<String> {
    let Some(i) = ag.info_tramo(net, tramo) else { return Vec::new() };
    let mut l = Vec::new();
    let quien: Vec<String> = Tipo::TODOS.iter().filter(|t| i.por_tipo[**t as usize] > 0).map(|t| t.cuantos(i.por_tipo[*t as usize])).collect();
    l.push(format!("Ahora: {}", if quien.is_empty() { "nadie".into() } else { quien.join(", ") }));
    let lento = i.medido[0].max(i.medido[1]) > i.libre * 1.3;
    l.push(format!("Paso en carreta o a caballo: {} / {} (libre {}){}", minutos(i.medido[0]), minutos(i.medido[1]), minutos(i.libre), if lento { " · se demora" } else { "" }));
    l
}

/// Filas del panel de tránsito: (concepto, valor, ¿es malo?).
pub fn resumen(ag: &Agentes) -> Vec<(String, String, bool)> {
    let st = &ag.e.stats;
    let n = ag.en_la_calle();
    let total: u32 = n.iter().sum();
    let mut f = vec![
        ("En la calle ahora".to_string(), format!("{total} ({})", Tipo::TODOS.iter().map(|t| t.cuantos(n[*t as usize])).collect::<Vec<_>>().join(", ")), false),
        ("Vecinos".to_string(), format!("{} ({} salen a trabajar)", ag.e.vecinos.len(), ag.e.vecinos.values().filter(|v| v.rol == Rol::Fuera).count()), false),
        ("Viajes hoy".to_string(), format!("{} (ayer {})", st.viajes_hoy, st.viajes_ayer), false),
    ];
    if st.viajes_hoy > 0 {
        f.push(("Viaje medio".into(), format!("{} min", (st.minutos_hoy / st.viajes_hoy as f32).round() as i32), false));
    }
    f.push(("Demorados hoy".into(), format!("{}", st.demorados_hoy), st.demorados_hoy > st.viajes_hoy / 5 + 2));
    if st.omitidos_hoy > 0 {
        f.push(("No pudieron salir".into(), format!("{} (calles sin conexión con su destino)", st.omitidos_hoy), true));
    }
    if let Some(d) = ag.demora_media() {
        f.push(("Demora en vehículo este mes".into(), format!("{} % de lo previsto", (d * 100.0).round() as i32), d > 1.3));
    }
    f.push(("Carretas llegadas al mercado".into(), format!("{}", st.carretas_al_mercado), false));
    f
}

enum Avance {
    Sigue,
    Llego,
    Perdido,
}

fn capitalizar(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// Posición en el plano y dirección de marcha (con el desvío lateral: los vehículos van por
/// la derecha, la gente a pie por la vereda).
pub fn pose(net: &RoadNetwork, a: &Agente) -> (Vec2, Vec2) {
    let p = a.pata();
    let Some(e) = net.edges.get(&p.tramo) else { return (Vec2::ZERO, Vec2::X) };
    let l = e.len();
    let ps = if p.sentido == 0 { a.s } else { l - a.s };
    let (pt, tan) = e.point(ps);
    let tan = if p.sentido == 0 { tan } else { -tan };
    let w = e.kind.width();
    let lat = if a.tipo.vehiculo() { w * 0.25 } else { w * 0.5 - 1.0 - (a.id % 3) as f32 * 0.35 };
    let derecha = Vec2::new(-tan.y, tan.x);
    (pt + derecha * lat, tan)
}

/// Velocidad máxima cómoda en curva: v = √(aceleración lateral × radio).
fn velocidad_en_curva(net: &RoadNetwork, p: &Pata, s: f32) -> f32 {
    let e = &net.edges[&p.tramo];
    let l = e.len();
    let ps = if p.sentido == 0 { s } else { l - s };
    let ahead = if p.sentido == 0 { 10.0 } else { -10.0 };
    let t0 = e.point(ps).1;
    let t1 = e.point((ps + ahead).clamp(0.0, l)).1;
    let ang = t0.angle_to(t1).abs();
    if ang < 0.02 {
        return f32::INFINITY;
    }
    (1.2 * 10.0 / ang).sqrt()
}

/// Ángulo de giro (rad) al pasar de una pata a la siguiente.
fn giro(net: &RoadNetwork, p: &Pata, q: &Pata) -> f32 {
    let e = &net.edges[&p.tramo];
    let t_in = if p.sentido == 0 { e.point(e.len()).1 } else { -e.point(0.0).1 };
    let f = &net.edges[&q.tramo];
    let t_out = if q.sentido == 0 { f.point(0.0).1 } else { -f.point(f.len()).1 };
    t_in.angle_to(t_out).abs()
}

/// Componente de la red conectada a la plaza (los viajes salen y llegan dentro de ella).
pub(crate) fn componente(net: &RoadNetwork, desde: u32) -> Vec<u32> {
    let mut vistos = std::collections::BTreeSet::new();
    let mut pila = vec![desde];
    while let Some(n) = pila.pop() {
        if !vistos.insert(n) {
            continue;
        }
        for eid in &net.node_edges[&n] {
            let e = &net.edges[eid];
            pila.push(if e.a == n { e.b } else { e.a });
        }
    }
    vistos.into_iter().collect()
}

fn lugares(t: &TerrainGen, net: &RoadNetwork, pc: Vec2, angle: f32) -> Lugares {
    let (f, _) = super::plaza_frame(angle);
    let mut l = Lugares { plaza: Some(pc), iglesia: Some(super::church_center(pc, f)), ..Default::default() };
    let Some((&centro, _)) = net.nodes.iter().min_by(|x, y| x.1.distance(pc).total_cmp(&y.1.distance(pc))) else { return l };
    let comp = componente(net, centro);
    // Entrada del campo: el extremo de calle (sin salida) más alejado de la plaza.
    let lejos = |ns: &mut dyn Iterator<Item = u32>| ns.max_by(|&a, &b| net.nodes[&a].distance(pc).total_cmp(&net.nodes[&b].distance(pc)).then(b.cmp(&a)));
    let entrada = lejos(&mut comp.iter().copied().filter(|&n| grado(net, n) == 1)).or_else(|| lejos(&mut comp.iter().copied()));
    l.campo = entrada.map(|n| net.nodes[&n]);
    // Puerto: el cruce más cerca del agua (hasta 60 m), de los más cercanos a la plaza.
    let mut best: Option<(i32, f32, u32)> = None;
    for &n in &comp {
        let p = net.nodes[&n];
        let r = (1..=3).find(|&k| (0..8).any(|j| {
            let q = p + Vec2::from_angle(j as f32 * std::f32::consts::FRAC_PI_4) * (k as f32 * 20.0);
            t.is_water(q.x, q.y)
        }));
        if let Some(k) = r {
            let cand = (k as i32, p.distance(pc), n);
            if best.is_none_or(|b| (cand.0, cand.1) < (b.0, b.1)) {
                best = Some(cand);
            }
        }
    }
    l.puerto = best.map(|b| net.nodes[&b.2]);
    l
}

#[cfg(test)]
mod tests;
