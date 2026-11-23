//! Vecinos y vehículos de la época colonial: gente a pie, jinetes, carretas de bueyes y
//! carros. Porte de `scripts/agents/agents.gd` del prototipo de Godot (diseño en
//! `docs/sistemas/agentes-y-conduccion.md`).
//!
//! - **Vecinos**: las personas de la población (`Poblacion::habitantes_de_lote`) con su casa,
//!   su trabajo (`lugar_de_trabajo` u oficio), su edad, clase y rasgos; antes de fundar la
//!   población, vecinos sintéticos de la semilla del solar. Cada uno tiene un **perfil de
//!   conducta** ([`perfil`]) que decide su prisa, prudencia, paciencia y vida social.
//! - **Rutina**: una **agenda encadenada** ([`agenda`]): la salida siguiente se programa al
//!   llegar (nadie está en dos lugares a la vez). Trabajo, compra en la pulpería del barrio,
//!   recova, pulpería a la vuelta, misa, paseo de la plaza y visitas; con lluvia salen menos.
//!   Las carretas traen cueros y trigo del campo de día; los arreos llevan la tropa a los
//!   corrales por los caminos de afuera.
//! - **Portones** de la muralla con aduana ([`portones`]): cola, revisión, derecho y cierre
//!   de noche; los contrabandistas los evitan.
//! - **Rutas**: A* sobre la red de calles ([`ruta`]): el más corto a pie, el más rápido según
//!   el tiempo medido en cada carril para los vehículos.
//! - **Conducción**: modelo de conductor inteligente ([`idm`]) entre los vehículos de un mismo
//!   carril; en los cruces tiene prioridad la calle empedrada y los de la secundaria esperan un
//!   hueco (que achican con la espera); no bloquean el cruce; aflojan en curvas y giros; los
//!   jinetes poco prudentes adelantan carretas por el carril contrario; los peatones cruzan la
//!   calzada cuando hay hueco y los vehículos frenan si hay alguien cruzando.
//!
//! La simulación es determinista: azar por contador para los vecinos ([`perfil::azar`]), una
//! secuencia guardada para el resto, orden de los agentes por id y colas por carril ordenadas.
//! Solo depende del estado guardado, de la hora del día, de lo que cuenta la ciudad
//! ([`Contexto`]) y del foco de la cámara (que elige qué agentes se simulan con detalle).

pub mod agenda;
pub mod datos;
pub mod dibujo;
pub mod fauna; // [personajes]
pub mod idm;
pub mod perfil;
pub mod portones;
pub mod ruta;

use super::lots::{Lots, ZoneKind, D as LOT_D};
use super::roads::{RoadKind, RoadNetwork};
use crate::buildings::Rng;
use crate::fxhash::FxBuild;
use crate::terrain::TerrainGen;
use glam::Vec2;
pub use idm::Tipo;
use agenda::Actividad;
use perfil::{azar, azar_n, Motivo, Perfil};
use portones::{Porton, StatsPorton};
use ruta::{carril, Router, Tiempos};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    /// El cabildo y el fuerte, en la plaza (funcionarios, soldados).
    Plaza,
    /// La iglesia matriz (clérigos).
    Iglesia,
}

/// Dónde está el vecino (para los viajes de vuelta).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Lugar {
    Casa,
    Trabajo,
    Plaza,
    Iglesia,
    /// Otro solar: la pulpería del barrio o la casa de un pariente.
    Solar(u32),
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
    /// De vuelta a casa (después de cualquier salida).
    VuelveACasa,
    /// A la pulpería (a la vuelta del trabajo, a tomar algo y conversar).
    APulperia,
    /// La compra del día para el hogar.
    ACompra,
    /// El paseo de la plaza (domingo por la tarde).
    APaseo,
    /// Visita a los parientes.
    AVisita,
    /// Tropa de ganado que va a los corrales.
    ArreoALosCorrales,
}

impl Proposito {
    pub fn texto(self) -> &'static str {
        match self {
            Proposito::AlTrabajo => "al trabajo",
            Proposito::VuelveDelTrabajo => "de vuelta a casa",
            Proposito::AlMercado => "a la recova (el mercado de la plaza)",
            Proposito::VuelveDelMercado => "de vuelta del mercado",
            Proposito::AMisa => "a misa en la iglesia matriz",
            Proposito::VuelveDeMisa => "de vuelta de misa",
            Proposito::TraeDelCampo => "trae cueros y trigo del campo al mercado",
            Proposito::VuelveAlCampo => "vuelve al campo con mercancías",
            Proposito::Reparto => "lleva mercancías a una tienda",
            Proposito::VuelveDelReparto => "vuelve vacío",
            Proposito::Paseo => "de paseo",
            Proposito::VuelveACasa => "de vuelta a casa",
            Proposito::APulperia => "a la pulpería",
            Proposito::ACompra => "a hacer la compra a la pulpería",
            Proposito::APaseo => "al paseo de la plaza",
            Proposito::AVisita => "a visitar a los parientes",
            Proposito::ArreoALosCorrales => "lleva la tropa a los corrales por fuera de la muralla",
        }
    }
}

/// Estado que se ve al inspeccionar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Marcha {
    EnMarcha,
    CediendoElPaso,
    EsperandoEnLaFila,
    /// En la cola del portón o esperando que abra.
    EsperandoElPorton,
    /// El guarda revisa la carga.
    EnRevision,
    /// Carreta empantanada en el barro.
    Empantanada,
    /// Adelantando por el carril contrario.
    Adelantando,
}

impl Marcha {
    pub fn texto(self) -> &'static str {
        match self {
            Marcha::EnMarcha => "en marcha",
            Marcha::CediendoElPaso => "cediendo el paso",
            Marcha::EsperandoEnLaFila => "esperando en la fila",
            Marcha::EsperandoElPorton => "esperando en el portón",
            Marcha::EnRevision => "el guarda revisa la carga",
            Marcha::Empantanada => "empantanada en el barro",
            Marcha::Adelantando => "adelantando",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vecino {
    pub casa: u32,
    /// Persona de la población (cuando la población está fundada, todos los vecinos son
    /// personas de verdad).
    #[serde(default)]
    pub persona: Option<u32>,
    /// El trabajo lo asignó otro sistema ([`Agentes::asignar_trabajo`]) o la población: no
    /// se reelige.
    #[serde(default)]
    pub trabajo_fijo: bool,
    /// Nombre y apellido (índices), origen y oficio (de los vecinos sintéticos).
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
    // --- Agenda encadenada y perfil (ia-y-transito.md §3, §4.1) ---
    #[serde(default)]
    pub perfil: Perfil,
    /// Clase del hogar (0 pobre a 3 principal).
    #[serde(default)]
    pub clase: u8,
    /// Hace la compra del hogar (una persona por hogar).
    #[serde(default)]
    pub compra: bool,
    /// La pulpería del barrio (fidelidad) y la casa de un pariente.
    #[serde(default)]
    pub pulperia: Option<u32>,
    #[serde(default)]
    pub visita: Option<u32>,
    /// Plan del día, en qué paso va y de qué día es.
    #[serde(default)]
    pub plan: Vec<Actividad>,
    #[serde(default)]
    pub paso_plan: u8,
    #[serde(default)]
    pub plan_dia: u32,
    /// Hora absoluta (día × 24 + hora) de la próxima salida programada.
    #[serde(default)]
    pub sale: Option<f64>,
    /// Agente que lo lleva ahora (si está en la calle).
    #[serde(default)]
    pub en_camino: Option<u32>,
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

/// Un portón en el camino de un agente: pata, posición en ella y clave del portón.
pub type CrucePorton = (u16, f32, u64);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agente {
    pub id: u32,
    pub tipo: Tipo,
    pub patas: Vec<Pata>,
    pub li: u16,
    /// Posición del frente (m) en la pata actual y velocidad (m/s).
    pub s: f32,
    pub v: f32,
    /// Velocidad deseada y separación propias (tipo, perfil y algo de cada uno).
    pub v0: f32,
    pub t: f32,
    pub temple: u8,
    pub vecino: Option<u64>,
    /// Persona de la población (si el vecino tiene una).
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
    // --- ia-y-transito.md §4.1 ---
    /// Perfil de conducta (del vecino, o sintético para carreteros y troperos).
    #[serde(default)]
    pub perfil: Perfil,
    /// Adónde va (para recalcular la ruta si la calle cambia, D15).
    #[serde(default)]
    pub destino: [f32; 2],
    /// Largo de más (la tropa de un arreo) y cabezas.
    #[serde(default)]
    pub largo_extra: f32,
    #[serde(default)]
    pub cabezas: u16,
    /// Carreta que trae carga (revisión larga en el portón) y contrabandista.
    #[serde(default)]
    pub cargada: bool,
    #[serde(default)]
    pub contrabando: bool,
    /// Portones en el camino, el próximo, cuándo termina la revisión y desde cuándo hace cola.
    #[serde(default)]
    pub portones: Vec<CrucePorton>,
    #[serde(default)]
    pub porton_i: u8,
    #[serde(default)]
    pub revision: f64,
    #[serde(default)]
    pub en_cola: f64,
    /// Vehículo que está adelantando (por el carril contrario).
    #[serde(default)]
    pub adelantando: Option<u32>,
    /// Quieta hasta (empantanada).
    #[serde(default)]
    pub detenido_hasta: f64,
    /// Ruta: cómo la eligió (para la ficha).
    #[serde(default)]
    pub ruta: Eleccion,
    /// Posición y dirección en el paso anterior (para interpolar el dibujo; no se guarda).
    #[serde(skip)]
    pub antes: Option<(Vec2, Vec2)>,
}

/// Cómo eligió la ruta (se muestra en la ficha).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Eleccion {
    /// A pie: la más corta.
    #[default]
    MasCorta,
    /// En vehículo: la más rápida según lo que tardan las calles a esta hora, con su percepción.
    MasRapida,
    /// Evita los portones (contrabando).
    SinPortones,
    /// Por fuera de la muralla (arreo).
    PorFuera,
    /// La recalculó porque cambió la calle.
    Recalculada,
}

impl Eleccion {
    pub fn texto(self) -> &'static str {
        match self {
            Eleccion::MasCorta => "a pie: el camino más corto",
            Eleccion::MasRapida => "la más rápida según lo que tardan las calles a esta hora (con su propia idea de cada calle)",
            Eleccion::SinPortones => "evita los portones (contrabando)",
            Eleccion::PorFuera => "por los caminos de afuera: el ganado no entra intramuros",
            Eleccion::Recalculada => "la recalculó en el camino porque cambió la calle",
        }
    }
}

impl PartialEq for Agente {
    fn eq(&self, o: &Self) -> bool {
        // Todo menos `antes`, que es solo del dibujo.
        (self.id, self.tipo, &self.patas, self.li, self.s, self.v, self.v0, self.t, self.temple, self.vecino, self.persona)
            == (o.id, o.tipo, &o.patas, o.li, o.s, o.v, o.v0, o.t, o.temple, o.vecino, o.persona)
            && (self.proposito, self.t0) == (o.proposito, o.t0)
            && (self.previsto, self.espera, self.llegada, self.t_carril, self.marcha, self.ropa, self.pelaje, self.conductor) == (o.previsto, o.espera, o.llegada, o.t_carril, o.marcha, o.ropa, o.pelaje, o.conductor)
            && (self.perfil, self.destino, self.largo_extra, self.cabezas, self.cargada, self.contrabando, &self.portones, self.porton_i, self.revision, self.en_cola, self.adelantando, self.detenido_hasta)
                == (o.perfil, o.destino, o.largo_extra, o.cabezas, o.cargada, o.contrabando, &o.portones, o.porton_i, o.revision, o.en_cola, o.adelantando, o.detenido_hasta)
            && self.ruta == o.ruta
    }
}

impl Agente {
    pub fn params(&self) -> idm::Params {
        self.tipo.params()
    }
    /// Largo del vehículo (con la tropa de un arreo).
    pub fn largo(&self) -> f32 {
        self.tipo.params().largo + self.largo_extra
    }
    pub fn pata(&self) -> Pata {
        self.patas[self.li as usize]
    }
    fn ultima(&self) -> bool {
        self.li as usize + 1 >= self.patas.len()
    }
    /// El próximo portón en la pata actual: (posición en la pata, clave).
    fn porton_aqui(&self) -> Option<(f32, u64)> {
        self.portones.get(self.porton_i as usize).filter(|c| c.0 == self.li).map(|c| (c.1, c.2))
    }
}

/// Quién sale en un viaje de la agenda.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Quien {
    /// Vecino (agenda de versiones anteriores: hoy los vecinos tienen su agenda encadenada).
    Vecino(u64),
    Carreta,
    Carro,
    /// Una tropa de ganado de la estancia a los corrales.
    Arreo,
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
    // --- ia-y-transito.md §4.1 ---
    #[serde(default)]
    pub arreos_hoy: u32,
    #[serde(default)]
    pub contrabando_hoy: u32,
    #[serde(default)]
    pub contrabando_mes: u32,
    #[serde(default)]
    pub adelantamientos_hoy: u32,
    #[serde(default)]
    pub empantanadas_hoy: u32,
    /// Agentes que recalcularon la ruta porque cambió la calle (y los que no pudieron).
    #[serde(default)]
    pub recalculados: u32,
    #[serde(default)]
    pub perdidos: u32,
    /// Salidas optativas suspendidas hoy (lluvia, epidemia).
    #[serde(default)]
    pub suspendidos_hoy: u32,
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
    // --- ia-y-transito.md §4.0-4.1 ---
    /// Semilla del azar por contador.
    #[serde(default)]
    pub semilla: u64,
    /// Salidas programadas de los vecinos: (hora absoluta en milésimas de hora, vecino).
    #[serde(default)]
    pub salidas: BTreeSet<(u64, u64)>,
    /// Lo que pasó en cada portón.
    #[serde(default)]
    pub portones: BTreeMap<u64, StatsPorton>,
    /// ¿Llueve hoy? ¿Hay barro? (se sortea al empezar el día).
    #[serde(default)]
    pub lluvia: bool,
    #[serde(default)]
    pub barro: bool,
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
    /// Los corrales de extramuros (adonde van los arreos): el cruce de afuera más cercano a la
    /// plaza.
    corrales: Option<Vec2>,
}

/// Lo que la ciudad le cuenta a los agentes en cada cuadro.
#[derive(Clone, Copy, Default)]
pub struct Contexto<'a> {
    /// Las personas (si la población está fundada).
    pub poblacion: Option<&'a crate::city::poblacion::Poblacion>,
    /// Días de lluvia del mes (para sortear si llueve hoy).
    pub dias_de_lluvia: f32,
    pub epidemia: bool,
}

/// Versión de la red de calles vista (cambia al agregar o quitar tramos).
type VersionRed = (u32, u32, usize);

/// Clave de los vecinos que son personas de la población (más el id de la persona).
pub const CLAVE_PERSONA: u64 = 1 << 48;

pub struct Agentes {
    pub e: Estado,
    router: Router,
    red: Option<VersionRed>,
    /// Altura de la superficie de cada tramo cada 2 m (calzada o tablero del puente).
    perfiles: HashMap<u32, Vec<f32>, FxBuild>,
    lugares: Lugares,
    firma_solares: (usize, i32, i32, usize),
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
    // --- Muralla, portones y recinto (derivados; no se guardan) ---
    firma_muralla: u64,
    muralla: Vec<(Vec2, Vec2)>,
    posiciones_portones: Vec<Vec2>,
    pub portones: Vec<Porton>,
    portones_por_tramo: HashMap<u32, Vec<usize>, FxBuild>,
    /// Tramos con portón (los contrabandistas no pasan) y tramos intramuros (los arreos no
    /// entran).
    veto_portones: BTreeSet<u32>,
    pub intramuros: BTreeSet<u32>,
    radio_intramuros: f32,
    /// Comercios de la ciudad (las pulperías): solar y frente.
    pulperias: Vec<(u32, Vec2)>,
    dias_de_lluvia: f32,
    epidemia: bool,
    /// El último lanzamiento no salió porque había otro vehículo encima del punto de salida.
    bloqueado: bool,
}

impl Default for Agentes {
    fn default() -> Self {
        Agentes::from_estado(Estado { azar: Rng::new(2027).state(), hora: 8.0, proximo_id: 1, semilla: 2027, ..Default::default() })
    }
}

impl Clone for Agentes {
    fn clone(&self) -> Self {
        let mut a = Agentes::from_estado(self.e.clone());
        a.set_muralla(self.firma_muralla, self.muralla.clone(), self.posiciones_portones.clone());
        a
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
    pub por_tipo: [u32; 5],
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

/// ¿El cruce tiene calle principal? (llegan calles empedradas y de tierra).
fn con_jerarquia(net: &RoadNetwork, nodo: u32) -> bool {
    let Some(es) = net.node_edges.get(&nodo) else { return false };
    let piedra = es.iter().filter(|e| net.edges[e].kind == RoadKind::Stone).count();
    piedra > 0 && piedra < es.len()
}

/// Frente de un solar (sobre la calle).
pub fn frente(l: &super::lots::Lot) -> Vec2 {
    l.c + l.f * (LOT_D * 0.5)
}

/// Hora absoluta en milésimas (clave de las salidas programadas).
fn mil(h: f64) -> u64 {
    (h.max(0.0) * 1000.0).round() as u64
}

/// Lo que se decide en la fase de conducción y se aplica después (todos con el estado del
/// paso anterior).
enum Evento {
    /// Empieza la revisión en el portón: termina en (reloj).
    Revision(usize, f64),
    /// Pasó el portón (agente, clave).
    Paso(usize, u64),
    /// Se puso en la cola del portón.
    EnCola(usize),
    /// Empieza a adelantar a (id).
    Adelanta(usize, u32),
    /// Terminó de adelantar (o lo abandonó).
    Vuelve(usize),
}

impl Agentes {
    pub fn from_estado(e: Estado) -> Agentes {
        Agentes {
            e,
            router: Router::default(),
            red: None,
            perfiles: HashMap::default(),
            lugares: Lugares::default(),
            firma_solares: (usize::MAX, -1, -1, 0),
            foco: None,
            detalles: Default::default(),
            escena: Default::default(),
            muestra: None,
            firma_muralla: 0,
            muralla: Vec::new(),
            posiciones_portones: Vec::new(),
            portones: Vec::new(),
            portones_por_tramo: HashMap::default(),
            veto_portones: BTreeSet::new(),
            intramuros: BTreeSet::new(),
            radio_intramuros: datos::datos().tropas.radio_intramuros_m,
            pulperias: Vec::new(),
            dias_de_lluvia: 0.0,
            epidemia: false,
            bloqueado: false,
        }
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

    /// Hora absoluta: días × 24 + hora del reloj.
    pub fn ahora(&self) -> f64 {
        self.e.dia as f64 * 24.0 + self.e.hora as f64
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
    pub fn en_la_calle(&self) -> [u32; 5] {
        let mut n = [0; 5];
        for a in &self.e.agentes {
            n[a.tipo as usize] += 1;
        }
        n
    }

    /// Fracción de lo que se interpola entre el paso anterior y el actual (para el dibujo).
    pub fn alfa(&self) -> f32 {
        (self.e.acumulado / PASO).clamp(0.0, 1.0)
    }

    // --- Muralla y portones -----------------------------------------------------------

    /// Firma de la muralla con que se ubicaron los portones (la ciudad la compara con la suya).
    pub fn firma_muralla(&self) -> u64 {
        self.firma_muralla
    }

    /// La muralla trazada (tramos) y los portones terminados (posiciones): la ciudad los pasa
    /// cuando cambian.
    pub fn set_muralla(&mut self, firma: u64, tramos: Vec<(Vec2, Vec2)>, portones: Vec<Vec2>) {
        self.firma_muralla = firma;
        self.muralla = tramos;
        self.posiciones_portones = portones;
        // Se ubican sobre la red en `preparar` (la red puede no estar lista todavía).
        self.red = None;
    }

    /// Ubica los portones, el recinto intramuros y los corrales (con la red y la plaza).
    fn ubicar_muralla(&mut self, net: &RoadNetwork, pc: Vec2) {
        self.portones = portones::ubicar(net, &self.posiciones_portones, Some(pc));
        self.portones_por_tramo.clear();
        self.veto_portones.clear();
        for (i, g) in self.portones.iter().enumerate() {
            self.portones_por_tramo.entry(g.tramo).or_default().push(i);
            self.veto_portones.insert(g.tramo);
            self.e.portones.entry(g.clave).or_default();
        }
        // El recinto: hasta la muralla (si hay) o el radio de los datos.
        self.radio_intramuros = self.muralla.iter().map(|&(a, b)| dist_segmento(pc, a, b)).fold(f32::INFINITY, f32::min);
        if !self.radio_intramuros.is_finite() || self.radio_intramuros < 60.0 {
            self.radio_intramuros = datos::datos().tropas.radio_intramuros_m;
        }
        let r = self.radio_intramuros;
        self.intramuros = net.edges.iter().filter(|(_, e)| e.pts.iter().any(|p| p.distance(pc) < r)).map(|(&k, _)| k).collect();
        // Corrales: el cruce de afuera más cercano a la plaza con alguna calle de afuera.
        self.lugares.corrales = net
            .nodes
            .iter()
            .filter(|(n, p)| p.distance(pc) >= r + 40.0 && net.node_edges.get(n).is_some_and(|es| es.iter().any(|e| !self.intramuros.contains(e))))
            .min_by(|x, y| x.1.distance(pc).total_cmp(&y.1.distance(pc)).then(x.0.cmp(y.0)))
            .map(|(_, &p)| p);
        // Los agentes en la calle: sus portones de nuevo.
        for i in 0..self.e.agentes.len() {
            let c = self.cruces_de_portones(net, &self.e.agentes[i].patas);
            let a = &mut self.e.agentes[i];
            let resto = &a.portones[(a.porton_i as usize).min(a.portones.len())..];
            if c.ends_with(resto) && c[..c.len() - resto.len()].iter().all(|x| x.0 < a.li || (x.0 == a.li && x.1 <= a.s)) {
                // Los mismos portones por delante (al cargar la partida): no se toca.
                continue;
            }
            let (li, s) = (a.li, a.s);
            a.portones = c.into_iter().filter(|x| x.0 > li || (x.0 == li && x.1 > s)).collect();
            a.porton_i = 0;
            a.revision = 0.0;
            a.en_cola = 0.0;
        }
    }

    /// Portones que cruza una ruta.
    fn cruces_de_portones(&self, net: &RoadNetwork, patas: &[Pata]) -> Vec<CrucePorton> {
        let mut out = Vec::new();
        if self.portones.is_empty() {
            return out;
        }
        for (li, p) in patas.iter().enumerate() {
            let Some(gs) = self.portones_por_tramo.get(&p.tramo) else { continue };
            let mut aqui: Vec<CrucePorton> = Vec::new();
            for &gi in gs {
                let g = &self.portones[gi];
                let Some(l) = net.edges.get(&p.tramo).map(|e| e.len()) else { continue };
                let pos = if p.sentido == 0 { g.s } else { l - g.s };
                if pos > p.desde + 0.5 && pos <= p.hasta {
                    aqui.push((li as u16, pos, g.clave));
                }
            }
            aqui.sort_by(|x, y| x.1.total_cmp(&y.1));
            out.extend(aqui);
        }
        out
    }

    /// Un portón por su clave.
    pub fn porton(&self, clave: u64) -> Option<&Porton> {
        self.portones.iter().find(|g| g.clave == clave)
    }

    // --- Tick ---------------------------------------------------------------------------

    /// Avanza `dt` segundos de juego (ya multiplicados por la velocidad), sin la población
    /// (vecinos sintéticos) ni el clima.
    pub fn tick(&mut self, t: &TerrainGen, net: &RoadNetwork, lots: &Lots, plaza: Option<(Vec2, f32)>, dt: f32) {
        self.tick_con(t, net, lots, plaza, &Contexto::default(), dt);
    }

    /// Avanza `dt` segundos de juego con lo que cuenta la ciudad (población, clima).
    pub fn tick_con(&mut self, t: &TerrainGen, net: &RoadNetwork, lots: &Lots, plaza: Option<(Vec2, f32)>, ctx: &Contexto, dt: f32) {
        let Some((pc, angle)) = plaza else { return };
        if self.e.semilla == 0 {
            self.e.semilla = self.e.azar | 1;
        }
        self.dias_de_lluvia = ctx.dias_de_lluvia;
        self.epidemia = ctx.epidemia;
        self.preparar(t, net, pc, angle);
        let pob = ctx.poblacion.filter(|p| p.fundada);
        let firma = (lots.lots.len(), lots.population(), pob.map_or(-1, |p| p.mes), pob.map_or(0, |p| p.idx.vivos.len()));
        // Sin población no se tocan los vecinos que ya son personas (cuadros sin contexto).
        let personas = self.e.vecinos.keys().next_back().is_some_and(|&k| k >= CLAVE_PERSONA);
        if firma != self.firma_solares && (pob.is_some() || !personas) {
            self.firma_solares = firma;
            self.sincronizar_vecinos(net, lots, pob);
        }
        if dt <= 0.0 {
            return;
        }
        if self.e.agenda.iter().any(|p| matches!(p.quien, Quien::Vecino(_))) {
            // Agenda de una versión anterior: se arma la de hoy con la agenda encadenada.
            self.e.agenda_dia = None;
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

    /// Cuando cambia la red: recalcula el camino de los agentes cuyo camino ya no existe,
    /// olvida cachés y recalcula alturas, lugares, portones y el recinto.
    fn preparar(&mut self, t: &TerrainGen, net: &RoadNetwork, pc: Vec2, angle: f32) {
        let (nn, ne) = net.counters();
        let v = (nn, ne, net.edges.len());
        if self.red == Some(v) {
            return;
        }
        let primera = self.red.is_none() && self.e.agentes.iter().all(|a| a.patas.iter().all(|p| net.edges.contains_key(&p.tramo)));
        self.red = Some(v);
        self.e.cruces.retain(|n, _| net.nodes.contains_key(n));
        self.e.tiempos.podar(net);
        self.router.vaciar_cache();
        self.perfiles.retain(|k, _| net.edges.contains_key(k));
        for (&id, e) in &net.edges {
            self.perfiles.entry(id).or_insert_with(|| dibujo::perfil(t, e));
        }
        self.lugares = lugares(t, net, pc, angle);
        self.ubicar_muralla(net, pc);
        if !primera {
            // D15: nadie desaparece porque cambió una calle: recalcula desde donde está.
            let rotos: Vec<u32> = self.e.agentes.iter().filter(|a| !a.patas.iter().all(|p| net.edges.contains_key(&p.tramo))).map(|a| a.id).collect();
            for id in rotos {
                self.recalcular(net, id);
            }
        }
    }

    /// Recalcula la ruta de un agente desde donde está (el tramo actual puede no existir más:
    /// sale del más cercano). Si no hay camino, el viaje se da por terminado ahí.
    fn recalcular(&mut self, net: &RoadNetwork, id: u32) {
        let Some(i) = self.indice(id) else { return };
        let a = &self.e.agentes[i];
        // Desde donde está; si su tramo ya no existe, desde el próximo que sigue en pie.
        let aqui = (a.li as usize..a.patas.len()).find(|&j| net.edges.contains_key(&a.patas[j].tramo)).map(|j| if j == a.li as usize { pose(net, a).0 } else { pose_en(net, a, j as u16, a.patas[j].desde).0 }).unwrap_or_else(|| Vec2::from(a.destino));
        let destino = Vec2::from(a.destino);
        let perc = a.tipo.vehiculo().then_some(a.id);
        match self.planificar(net, aqui, destino, perc) {
            Some(patas) => {
                let c = self.cruces_de_portones(net, &patas);
                let a = &mut self.e.agentes[i];
                a.s = patas[0].desde;
                a.patas = patas;
                a.li = 0;
                a.portones = c;
                a.porton_i = 0;
                a.revision = 0.0;
                a.adelantando = None;
                a.ruta = Eleccion::Recalculada;
                self.e.stats.recalculados += 1;
            }
            None => {
                self.e.stats.perdidos += 1;
                let a = self.e.agentes.remove(i);
                self.vecino_llego(a.vecino, self.ahora(), true);
            }
        }
    }

    // --- Vecinos --------------------------------------------------------------------------

    /// Los vecinos: con la población fundada, las personas que viven en cada solar; si no,
    /// un vecino sintético por habitante de cada casa. Quien se mudó deja de estar.
    fn sincronizar_vecinos(&mut self, net: &RoadNetwork, lots: &Lots, pob: Option<&crate::city::poblacion::Poblacion>) {
        // Comercios (las pulperías de la colonia).
        self.pulperias = lots.lots.values().filter(|l| l.level > 0 && l.kind() == Some(ZoneKind::Comercial)).map(|l| (l.id, frente(l))).collect();
        let _ = net;
        let antes: BTreeSet<u64> = self.e.vecinos.keys().copied().collect();
        match pob {
            Some(p) => self.vecinos_de_poblacion(lots, p),
            None => self.vecinos_sinteticos(lots),
        }
        // Los que se fueron: fuera de la agenda.
        let quedan: BTreeSet<u64> = self.e.vecinos.keys().copied().collect();
        if antes != quedan {
            self.e.salidas.retain(|(_, k)| quedan.contains(k));
        }
        // Pulpería del barrio (fidelidad: una de las tres más cercanas) y quién hace la compra.
        let semilla = self.e.semilla;
        let pul = self.pulperias.clone();
        let mut casas: BTreeMap<u32, Vec2> = BTreeMap::new();
        for v in self.e.vecinos.values() {
            if let Some(l) = lots.lots.get(&v.casa) {
                casas.insert(v.casa, frente(l));
            }
        }
        for (&k, v) in self.e.vecinos.iter_mut() {
            let ok = v.pulperia.is_some_and(|p| pul.iter().any(|q| q.0 == p));
            if ok || pul.is_empty() {
                if pul.is_empty() {
                    v.pulperia = None;
                }
                continue;
            }
            let Some(&c) = casas.get(&v.casa) else { continue };
            let mut cerca: Vec<(f32, u32)> = pul.iter().map(|q| (q.1.distance(c), q.0)).collect();
            cerca.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            cerca.truncate(3);
            let j = ((azar(semilla, k, 0, Motivo::Fidelidad) * cerca.len() as f32) as usize).min(cerca.len() - 1);
            // La más cercana pesa el doble.
            let j = if azar(semilla, k, 1, Motivo::Fidelidad) < 0.5 { 0 } else { j };
            v.pulperia = Some(cerca[j].1);
        }
        // Los que llegaron hoy se suman a la agenda del día (desde ahora).
        if self.e.agenda_dia == Some(self.e.dia) {
            let nuevos: Vec<u64> = quedan.difference(&antes).copied().collect();
            for k in nuevos {
                self.planear_vecino(k);
            }
        }
    }

    fn vecinos_sinteticos(&mut self, lots: &Lots) {
        self.e.vecinos.retain(|&k, _| k < CLAVE_PERSONA && lots.lots.get(&((k >> 8) as u32)).is_some_and(|l| (k & 0xFF) < l.residents().max(0) as u64));
        let empleos: Vec<u32> = lots.lots.values().filter(|l| l.level > 0 && matches!(l.kind(), Some(ZoneKind::Comercial | ZoneKind::Industrial))).map(|l| l.id).collect();
        let semilla = self.e.semilla;
        for l in lots.lots.values() {
            let n = l.residents().clamp(0, 255) as u64;
            for i in 0..n {
                let k = clave_vecino(l.id, i as u8);
                if self.e.vecinos.contains_key(&k) {
                    continue;
                }
                let v = habitante_de_lote(l, i as u8, semilla);
                self.e.vecinos.insert(k, v);
            }
        }
        // Trabajo: un comercio o taller de la ciudad (más probable el cercano: modelo
        // gravitatorio, D4); algunos son peones del campo o trabajan en la orilla del puerto.
        let keys: Vec<u64> = self.e.vecinos.keys().copied().collect();
        for k in keys {
            let v = self.e.vecinos[&k].clone();
            if v.rol != Rol::Fuera || v.trabajo_fijo {
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
                let casa = lots.lots.get(&v.casa).map(frente).unwrap_or(Vec2::ZERO);
                // Peso exp(−d / 600 m): lejos es posible, pero menos probable.
                let pesos: Vec<f32> = empleos.iter().map(|id| (-(lots.lots[id].c.distance(casa)) / 600.0).exp()).collect();
                let total: f32 = pesos.iter().sum();
                let mut y = self.azar() * total;
                let mut j = 0;
                while j + 1 < pesos.len() && y > pesos[j] {
                    y -= pesos[j];
                    j += 1;
                }
                Trabajo::Solar(empleos[j])
            };
            self.e.vecinos.get_mut(&k).unwrap().trabajo = w;
        }
    }

    /// Vecinos = personas de la población que viven en solares; su trabajo, horario y perfil
    /// salen de la persona (se recalculan cada mes).
    fn vecinos_de_poblacion(&mut self, lots: &Lots, pob: &crate::city::poblacion::Poblacion) {
        use crate::city::poblacion::modelo::Oficio;
        let semilla = self.e.semilla;
        let mes = pob.mes;
        let mut vistos: BTreeSet<u64> = BTreeSet::new();
        let mut compra_hecha: BTreeSet<u32> = BTreeSet::new();
        let mut nuevos: Vec<(u64, Vecino)> = Vec::new();
        for l in lots.lots.values() {
            for p in pob.habitantes_de_lote(l.id) {
                if !p.viva() {
                    continue;
                }
                let k = CLAVE_PERSONA | p.id as u64;
                vistos.insert(k);
                let (clase, ganado) = p.hogar.and_then(|h| pob.hogar(h)).map_or((crate::city::poblacion::modelo::Clase::Modesta, 0), |h| (h.clase, h.ganado));
                let perfil = Perfil::de_persona(p, clase, ganado, mes, semilla);
                let edad = p.edad(mes);
                let horario = datos::datos().horario(&format!("{:?}", p.oficio));
                let corr = (azar(semilla, k, 0, Motivo::Horario) - 0.5) * 2.0 * horario.desvio;
                let (rol, trabajo) = match (p.trabajo, p.oficio) {
                    _ if edad < 12 || edad >= 72 => (Rol::NoTrabaja, Trabajo::Ninguno),
                    (Some(w), _) if w == l.id => (Rol::EnCasa, Trabajo::Ninguno),
                    (Some(w), _) if lots.lots.get(&w).is_some_and(|x| x.level > 0) => (Rol::Fuera, Trabajo::Solar(w)),
                    (_, Oficio::Peon | Oficio::Labrador | Oficio::Hacendado) => (Rol::Fuera, Trabajo::Campo),
                    (_, Oficio::Marinero) => (Rol::Fuera, Trabajo::Puerto),
                    (_, Oficio::Soldado | Oficio::Funcionario) => (Rol::Fuera, Trabajo::Plaza),
                    (_, Oficio::Clerigo) => (Rol::Fuera, Trabajo::Iglesia),
                    (_, Oficio::Ninguno) => (Rol::NoTrabaja, Trabajo::Ninguno),
                    _ => (Rol::EnCasa, Trabajo::Ninguno),
                };
                // La casa de un pariente (padres o hijos) que vive en otro solar.
                let visita = p.padre.into_iter().chain(p.madre).chain(p.hijos.iter().copied()).filter_map(|q| pob.casa_de(q)).find(|&c| c != l.id && lots.lots.contains_key(&c));
                // La compra la hace una persona por hogar: la primera adulta que no sale.
                let compra = rol != Rol::Fuera && edad >= 14 && p.hogar.is_some_and(|h| compra_hecha.insert(h));
                let temple = if perfil.prudencia() > 0.62 {
                    0
                } else if perfil.prisa() > 0.55 && perfil.prudencia() < 0.42 {
                    2
                } else {
                    1
                };
                let tipo = if perfil.caballo() && rol == Rol::Fuera { Tipo::Jinete } else { Tipo::APie };
                match self.e.vecinos.get_mut(&k) {
                    Some(v) => {
                        // Cambia lo que cambia con el mes; la agenda del día no se toca.
                        v.casa = l.id;
                        v.perfil = perfil;
                        v.rol = rol;
                        v.trabajo = trabajo;
                        v.clase = clase as u8;
                        v.compra = compra;
                        v.visita = visita;
                        v.temple = temple;
                        v.tipo = tipo;
                        v.entrada = (horario.entrada + corr).clamp(4.5, 10.0);
                        v.salida = (horario.salida + corr).clamp(v.entrada + 3.0, 22.0);
                    }
                    None => {
                        let s = perfil::azar_n(semilla, k, 0, Motivo::Rasgo as u32, 7);
                        nuevos.push((
                            k,
                            Vecino {
                                casa: l.id,
                                persona: Some(p.id),
                                trabajo_fijo: true,
                                nombre: (0, 0),
                                origen: 0,
                                oficio: 0,
                                rol,
                                trabajo,
                                tipo,
                                temple,
                                entrada: (horario.entrada + corr).clamp(4.5, 10.0),
                                salida: (horario.salida + corr).clamp(horario.entrada + corr + 3.0, 22.0),
                                semilla_ruta: (s * 100_000.0) as u32,
                                cambios_de_ruta: 0,
                                lugar: Lugar::Casa,
                                ultimo: None,
                                ropa: (azar(semilla, k, 0, Motivo::Perfil) * dibujo::ROPA.len() as f32) as u8,
                                perfil,
                                clase: clase as u8,
                                compra,
                                pulperia: None,
                                visita,
                                plan: Vec::new(),
                                paso_plan: 0,
                                plan_dia: 0,
                                sale: None,
                                en_camino: None,
                            },
                        ));
                    }
                }
            }
        }
        self.e.vecinos.retain(|k, _| vistos.contains(k));
        self.e.vecinos.extend(nuevos);
    }

    /// Agenda del día: el plan encadenado de cada vecino (desde ahora), las carretas del
    /// campo, los carros de reparto y los arreos.
    fn armar_agenda(&mut self, lots: &Lots) {
        let st = &mut self.e.stats;
        st.viajes_ayer = st.viajes_hoy;
        st.minutos_ayer = st.minutos_hoy;
        st.demorados_ayer = st.demorados_hoy;
        st.viajes_hoy = 0;
        st.minutos_hoy = 0.0;
        st.demorados_hoy = 0;
        st.omitidos_hoy = 0;
        st.sin_calle_hoy = 0;
        st.arreos_hoy = 0;
        st.contrabando_hoy = 0;
        st.adelantamientos_hoy = 0;
        st.empantanadas_hoy = 0;
        st.suspendidos_hoy = 0;
        for g in self.e.portones.values_mut() {
            g.nuevo_dia();
        }
        self.e.agenda_dia = Some(self.e.dia);
        self.e.tiempos.olvidar(self.e.reloj);
        // ¿Llueve hoy? Según los días de lluvia del mes; barro con dos días seguidos.
        let llueve = |d: u32| d > 0 && azar(self.e.semilla, 0, d, Motivo::Lluvia) < self.dias_de_lluvia / 30.0;
        self.e.lluvia = llueve(self.e.dia);
        let barro_dias = datos::datos().clima.barro_dias.max(1);
        self.e.barro = (0..barro_dias).all(|j| self.e.dia >= j && llueve(self.e.dia - j));
        let keys: Vec<u64> = self.e.vecinos.keys().copied().collect();
        for k in keys {
            self.planear_vecino(k);
        }
        let mut r = self.rng();
        let domingo = self.e.dia % 7 == 6;
        let mut ag = Vec::new();
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
        // Arreos: unos por semana (por contador: el mismo día tiene siempre los mismos).
        let tr = &datos::datos().tropas;
        if !domingo {
            let esperado = tr.por_semana as f32 / 6.0;
            let n = esperado.floor() as u32 + u32::from(azar(self.e.semilla, 1, self.e.dia, Motivo::Arreo) < esperado.fract());
            for j in 0..n {
                let h = tr.hora_min + (tr.hora_max - tr.hora_min) * azar_n(self.e.semilla, 2, self.e.dia, Motivo::Arreo as u32, j);
                ag.push(Programado { hora: h, quien: Quien::Arreo, que: Proposito::ArreoALosCorrales });
            }
        }
        ag.sort_by(|a, b| a.hora.total_cmp(&b.hora));
        self.e.agenda = ag;
        // Si el día ya empezó (al fundar o al cargar), no se largan de golpe los viajes pasados.
        self.e.agenda_i = self.e.agenda.iter().position(|p| p.hora >= self.e.hora - 0.01).unwrap_or(self.e.agenda.len());
    }

    /// Arma el plan de hoy de un vecino que está en su casa (si está afuera, sigue con el de
    /// ayer hasta volver) y programa su primera salida.
    fn planear_vecino(&mut self, k: u64) {
        let dia = self.e.dia;
        let hora = self.e.hora;
        let Some(v) = self.e.vecinos.get(&k) else { return };
        if v.en_camino.is_some() || v.lugar != Lugar::Casa {
            return;
        }
        let d = agenda::Dia { dia, domingo: dia % 7 == 6, lluvia: self.e.lluvia, epidemia: self.epidemia, hay_pulperia: !self.pulperias.is_empty() };
        let mut plan = agenda::plan(self.e.semilla, k, v, &d);
        // Sin lluvia, ¿cuántas salidas optativas tendría? (para contar las suspendidas)
        if d.lluvia || d.epidemia {
            let seco = agenda::plan(self.e.semilla, k, v, &agenda::Dia { lluvia: false, epidemia: false, ..d });
            let n = |p: &[Actividad]| p.iter().filter(|a| a.optativa).count();
            self.e.stats.suspendidos_hoy += n(&seco).saturating_sub(n(&plan)) as u32;
        }
        // Lo que ya pasó del día (al fundar, al cargar o al mudarse): se salta hasta la
        // primera salida de la casa que todavía no pasó.
        let mut i = 0;
        while i < plan.len() && plan[i].desde < hora - 0.01 {
            i += 1;
        }
        while i < plan.len() && plan[i].destino == Lugar::Casa {
            i += 1;
        }
        plan.drain(..i);
        let sale = plan.first().map(|a| dia as f64 * 24.0 + a.desde as f64);
        let v = self.e.vecinos.get_mut(&k).unwrap();
        if let Some(t) = v.sale.take() {
            self.e.salidas.remove(&(mil(t), k));
        }
        v.plan = plan;
        v.paso_plan = 0;
        v.plan_dia = dia;
        v.sale = sale;
        if let Some(t) = sale {
            self.e.salidas.insert((mil(t), k));
        }
    }

    /// Programa la salida siguiente de un vecino que acaba de llegar (o que no pudo salir).
    fn programar_siguiente(&mut self, k: u64, ahora: f64, dura: f32) {
        let Some(v) = self.e.vecinos.get_mut(&k) else { return };
        if let Some(t) = v.sale.take() {
            self.e.salidas.remove(&(mil(t), k));
        }
        let Some(sig) = v.plan.get(v.paso_plan as usize) else { return };
        let t = (ahora + dura as f64).max(v.plan_dia as f64 * 24.0 + sig.desde as f64);
        v.sale = Some(t);
        self.e.salidas.insert((mil(t), k));
    }

    /// El vecino del agente llegó (o su viaje se dio por terminado): pasa a su destino y
    /// programa la salida siguiente.
    fn vecino_llego(&mut self, k: Option<u64>, ahora: f64, perdido: bool) {
        let Some(k) = k else { return };
        let Some(v) = self.e.vecinos.get_mut(&k) else { return };
        v.en_camino = None;
        let i = v.paso_plan as usize;
        let Some(a) = v.plan.get_mut(i) else { return };
        a.llego = Some(ahora);
        let (destino, dura) = (a.destino, a.dura);
        v.lugar = destino;
        v.paso_plan += 1;
        let _ = perdido;
        self.programar_siguiente(k, ahora, dura);
    }

    /// Larga los viajes de la agenda que ya llegaron a su hora, las salidas de los vecinos y
    /// las vueltas pendientes.
    fn salidas(&mut self, net: &RoadNetwork, lots: &Lots) {
        while self.e.agenda_i < self.e.agenda.len() && self.e.agenda[self.e.agenda_i].hora <= self.e.hora {
            let p = self.e.agenda[self.e.agenda_i];
            self.e.agenda_i += 1;
            match p.quien {
                Quien::Vecino(_) => {}
                Quien::Carreta => {
                    let (Some(a), Some(b)) = (self.lugares.campo, self.lugares.plaza) else { continue };
                    let temple = self.azar_u(3) as u8;
                    let c = (self.azar_u(12) as u8, self.azar_u(APELLIDOS.len() as u32) as u8);
                    let sem = self.azar_u(100_000);
                    // Contrabando: algunas carretas buscan un camino sin portones.
                    let contrabando = !self.portones.is_empty() && self.azar() < portones::fraccion_contrabando();
                    if contrabando && self.lanzar_con(net, Tipo::Carreta, a, b, None, Proposito::TraeDelCampo, sem, temple, c, Some(1)) {
                        self.e.stats.contrabando_hoy += 1;
                        self.e.stats.contrabando_mes += 1;
                        let n = self.portones.len().max(1) as f32;
                        for g in &self.portones {
                            if let Some(st) = self.e.portones.get_mut(&g.clave) {
                                st.contrabando_mes += 1.0 / n;
                            }
                        }
                        continue;
                    }
                    if self.lanzar(net, Tipo::Carreta, a, b, None, Proposito::TraeDelCampo, sem, temple, c) {
                        self.e.agentes.last_mut().unwrap().cargada = true;
                    }
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
                Quien::Arreo => {
                    self.lanzar_arreo(net);
                }
            }
        }
        // Vecinos: las salidas que ya llegaron a su hora.
        let ahora = self.ahora();
        while let Some(&(t, k)) = self.e.salidas.first() {
            if t > mil(ahora) {
                break;
            }
            self.e.salidas.remove(&(t, k));
            if let Some(v) = self.e.vecinos.get_mut(&k) {
                v.sale = None;
            }
            self.viaje_de_vecino(net, lots, k);
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

    /// Larga una tropa de ganado del campo a los corrales, por fuera del recinto.
    pub fn lanzar_arreo(&mut self, net: &RoadNetwork) -> bool {
        let (Some(a), Some(b)) = (self.lugares.campo, self.lugares.corrales) else { return false };
        let pc = self.lugares.plaza.unwrap_or(a);
        if a.distance(pc) < self.radio_intramuros {
            return false;
        }
        let tr = &datos::datos().tropas;
        let dia = self.e.dia;
        let n = self.e.stats.arreos_hoy;
        let cabezas = tr.cabezas_min + ((tr.cabezas_max - tr.cabezas_min) as f32 * azar_n(self.e.semilla, 3, dia, Motivo::Arreo as u32, n)) as u32;
        let c = (self.azar_u(12) as u8, self.azar_u(APELLIDOS.len() as u32) as u8);
        if !self.lanzar_con(net, Tipo::Arreo, a, b, None, Proposito::ArreoALosCorrales, 0, 1, c, Some(2)) {
            return false;
        }
        let ag = self.e.agentes.last_mut().unwrap();
        ag.cabezas = cabezas as u16;
        ag.largo_extra = (cabezas as f32 * tr.metros_por_cabeza - Tipo::Arreo.params().largo).max(0.0);
        ag.v0 = tr.velocidad_kmh / 3.6;
        ag.previsto = ag.patas.iter().map(|q| (q.hasta - q.desde) / ag.v0).sum();
        self.e.stats.arreos_hoy += 1;
        true
    }

    fn punto_trabajo(&self, lots: &Lots, w: Trabajo) -> Option<Vec2> {
        match w {
            Trabajo::Ninguno => None,
            Trabajo::Solar(id) => lots.lots.get(&id).filter(|l| l.level > 0).map(frente),
            Trabajo::Campo => self.lugares.campo,
            Trabajo::Puerto => self.lugares.puerto.or(self.lugares.plaza),
            Trabajo::Plaza => self.lugares.plaza,
            Trabajo::Iglesia => self.lugares.iglesia.or(self.lugares.plaza),
        }
    }

    fn punto(&self, lots: &Lots, v: &Vecino, l: Lugar) -> Option<Vec2> {
        match l {
            Lugar::Casa => lots.lots.get(&v.casa).map(frente),
            Lugar::Trabajo => self.punto_trabajo(lots, v.trabajo),
            Lugar::Plaza => self.lugares.plaza,
            Lugar::Iglesia => self.lugares.iglesia,
            Lugar::Solar(id) => lots.lots.get(&id).filter(|l| l.level > 0).map(frente),
        }
    }

    /// Sale el vecino hacia la actividad que le toca de su plan.
    fn viaje_de_vecino(&mut self, net: &RoadNetwork, lots: &Lots, k: u64) {
        let ahora = self.ahora();
        let Some(v) = self.e.vecinos.get(&k).cloned() else { return };
        if v.en_camino.is_some() {
            // Todavía no llegó (no debería pasar: la salida se programa al llegar).
            self.programar_siguiente(k, ahora, 0.25);
            return;
        }
        let i = v.paso_plan as usize;
        let Some(act) = v.plan.get(i).copied() else { return };
        let a = self.punto(lots, &v, v.lugar);
        let b = self.punto(lots, &v, act.destino);
        let (Some(a), Some(b)) = (a, b) else {
            // El lugar ya no existe (el taller cerró, la pulpería se demolió): se salta la
            // salida hasta la vuelta a casa.
            self.saltar_salida(k, ahora);
            return;
        };
        if act.destino == v.lugar || a.distance(b) < 3.0 {
            // Ya está ahí.
            if let Some(x) = self.e.vecinos.get_mut(&k).and_then(|v| v.plan.get_mut(i)) {
                x.salio = Some(ahora);
            }
            self.vecino_llego(Some(k), ahora, false);
            return;
        }
        // Para trayectos cortos todos caminan; lejos, a caballo quien tiene (D14, simple).
        let tipo = if a.distance(b) < 150.0 || !v.perfil.caballo() && v.tipo != Tipo::Jinete { Tipo::APie } else { Tipo::Jinete };
        if self.lanzar(net, tipo, a, b, Some(k), act.que, v.semilla_ruta, v.temple, v.nombre) {
            let id = self.e.agentes.last().unwrap().id;
            let v = self.e.vecinos.get_mut(&k).unwrap();
            v.en_camino = Some(id);
            v.plan[i].salio = Some(ahora);
        } else if self.bloqueado {
            // Hay otro vehículo saliendo de ahí: sale unos minutos después.
            self.programar_siguiente(k, ahora, 0.05);
        } else if act.destino == Lugar::Casa {
            // No puede volver por la calle (se cortó): vuelve igual (no se ve).
            self.vecino_llego(Some(k), ahora, true);
        } else {
            self.saltar_salida(k, ahora);
        }
    }

    /// No puede ir: se queda donde está y salta hasta después de la próxima vuelta a casa.
    fn saltar_salida(&mut self, k: u64, ahora: f64) {
        let Some(v) = self.e.vecinos.get_mut(&k) else { return };
        let mut i = v.paso_plan as usize;
        if v.lugar == Lugar::Casa {
            while i < v.plan.len() && v.plan[i].destino != Lugar::Casa {
                i += 1;
            }
            i += 1;
        } else {
            // Afuera: va directo a la vuelta.
            while i < v.plan.len() && v.plan[i].destino != Lugar::Casa {
                i += 1;
            }
        }
        v.paso_plan = i.min(v.plan.len()) as u8;
        self.programar_siguiente(k, ahora, 0.0);
    }

    /// Planifica la ruta de `desde` a `hasta` como patas sobre la red: busca desde los dos
    /// extremos del tramo de salida hacia los dos del de llegada y se queda con la mejor.
    pub fn planificar(&mut self, net: &RoadNetwork, desde: Vec2, hasta: Vec2, percepcion: Option<u32>) -> Option<Vec<Pata>> {
        self.planificar_con(net, desde, hasta, percepcion, None)
    }

    /// Como [`planificar`](Self::planificar), con una restricción: 1 = sin portones
    /// (contrabando), 2 = sin entrar intramuros (arreos).
    fn planificar_con(&mut self, net: &RoadNetwork, desde: Vec2, hasta: Vec2, percepcion: Option<u32>, veto: Option<u32>) -> Option<Vec<Pata>> {
        let vetados = match veto {
            Some(1) => Some(&self.veto_portones),
            Some(_) => Some(&self.intramuros),
            None => None,
        };
        let n0 = cercano_permitido(net, desde, 45.0, vetados)?;
        let n1 = cercano_permitido(net, hasta, 70.0, vetados)?;
        let (e0, e1) = (&net.edges[&n0.edge], &net.edges[&n1.edge]);
        let s0 = e0.cum[n0.seg] + e0.pts[n0.seg].distance(n0.point);
        let s1 = e1.cum[n1.seg] + e1.pts[n1.seg].distance(n1.point);
        let (l0, l1) = (e0.len(), e1.len());
        if n0.edge == n1.edge {
            let p = if s1 >= s0 { Pata { tramo: n0.edge, sentido: 0, desde: s0, hasta: s1 } } else { Pata { tramo: n0.edge, sentido: 1, desde: l0 - s0, hasta: l0 - s1 } };
            return Some(vec![p]);
        }
        // Los tiempos medidos que valen: los de esta franja, olvidados hasta ahora.
        // (de la foto de esta hora; ver `Tiempos::foto`).
        let hora_abs = self.ahora().floor() as i64;
        if self.e.tiempos.foto_hora != hora_abs {
            self.e.tiempos.fotografiar(hora_abs);
            self.router.vaciar_cache();
        }
        self.e.tiempos.ahora = hora_abs as f64 * 200.0;
        self.e.tiempos.franja = ruta::franja(self.e.hora);
        // Costo de las puntas: metros a pie, segundos a velocidad de referencia en vehículo.
        let unidad = |k: RoadKind| if percepcion.is_some() { 1.0 / ruta::ref_speed(k) } else { 1.0 };
        let mut best: Option<(f32, u32, u32, std::sync::Arc<ruta::Camino>)> = None;
        for (ini, c_ini) in [(e0.a, s0), (e0.b, l0 - s0)] {
            for (fin_, c_fin) in [(e1.a, s1), (e1.b, l1 - s1)] {
                let Some(c) = self.router.camino_con(net, &self.e.tiempos, ini, fin_, percepcion, veto.zip(vetados)) else { continue };
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
        self.lanzar_con(net, tipo, desde, hasta, vecino, que, semilla, temple, conductor, None)
    }

    /// Como [`lanzar`](Self::lanzar), con una restricción de ruta (ver `planificar_con`).
    #[allow(clippy::too_many_arguments)]
    fn lanzar_con(&mut self, net: &RoadNetwork, tipo: Tipo, desde: Vec2, hasta: Vec2, vecino: Option<u64>, que: Proposito, semilla: u32, temple: u8, conductor: (u8, u8), veto: Option<u32>) -> bool {
        if self.e.agentes.len() >= MAX_AGENTES {
            self.e.stats.omitidos_hoy += 1;
            return false;
        }
        let Some(patas) = self.planificar_con(net, desde, hasta, tipo.vehiculo().then_some(semilla), veto) else {
            if veto.is_none() {
                self.e.stats.omitidos_hoy += 1;
                if net.nearest_edge(desde, 45.0).is_none() || net.nearest_edge(hasta, 70.0).is_none() {
                    self.e.stats.sin_calle_hoy += 1;
                }
            }
            return false;
        };
        // Un vehículo no sale encima de otro (dos de la misma casa a la misma hora): espera.
        self.bloqueado = false;
        if tipo.vehiculo() {
            let p0 = patas[0];
            let largo = tipo.params().largo;
            if self.e.agentes.iter().any(|b| b.tipo.vehiculo() && b.pata().tramo == p0.tramo && b.pata().sentido == p0.sentido && b.s - b.largo() < p0.desde + 1.0 && b.s > p0.desde - largo - 2.0) {
                self.bloqueado = true;
                if vecino.is_none() {
                    self.e.pendientes.push(Pendiente { cuando: self.e.reloj + 20.0, tipo, desde: desde.into(), hasta: hasta.into(), que, temple, conductor });
                }
                return false;
            }
        }
        let id = self.e.proximo_id;
        let vec = vecino.and_then(|k| self.e.vecinos.get(&k)).map(|v| (v.perfil, v.ropa, v.persona));
        // Perfil: el del vecino; si no, uno sintético del conductor (por su temperamento).
        let perfil = vec.map(|v| v.0).filter(|p| p.v_pie > 0).unwrap_or_else(|| Perfil::sintetico(id as u64 ^ 0xC0_FFEE, temple, 35, tipo == Tipo::Jinete, self.e.semilla));
        let p = tipo.params();
        let (fv, ft, _, _) = perfil.factores_idm();
        let x = self.azar();
        let mut v0 = if tipo.vehiculo() { p.v0 * fv * (0.97 + 0.06 * x) } else { perfil.velocidad_a_pie() };
        if !tipo.vehiculo() && self.e.lluvia {
            v0 *= datos::datos().clima.pie_con_lluvia;
        }
        let previsto: f32 = patas.iter().map(|q| (q.hasta - q.desde) / v0).sum();
        self.e.proximo_id += 1;
        let ropa = match vec {
            Some(v) => v.1,
            None => self.azar_u(dibujo::ROPA.len() as u32) as u8,
        };
        let persona = vec.and_then(|v| v.2);
        let pelaje = self.azar_u(dibujo::PELAJE.len() as u32) as u8;
        let s = patas[0].desde;
        let portones = self.cruces_de_portones(net, &patas);
        let ruta = match (veto, tipo.vehiculo()) {
            (Some(1), _) => Eleccion::SinPortones,
            (Some(_), _) => Eleccion::PorFuera,
            (None, true) => Eleccion::MasRapida,
            (None, false) => Eleccion::MasCorta,
        };
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
            persona,
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
            perfil,
            destino: hasta.into(),
            largo_extra: 0.0,
            cabezas: 0,
            cargada: false,
            contrabando: veto == Some(1),
            portones,
            porton_i: 0,
            revision: 0.0,
            en_cola: 0.0,
            adelantando: None,
            detenido_hasta: 0.0,
            ruta,
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
                // Posición anterior (para interpolar el dibujo) de los que están cerca de la vista
                // (la misma cota descarta sin calcular la pose a los que no se dibujan).
                if e.pts[0].distance(f) - e.len() > dibujo::RADIO + 100.0 {
                    a.antes = None;
                    continue;
                }
                let (p, d) = pose(net, a);
                a.antes = (p.distance(f) < dibujo::RADIO + 100.0).then_some((p, d));
            }
        }
        let reloj = self.e.reloj;
        let abierto = portones::abierto(self.e.hora);
        let barro = self.e.barro;
        let clima = &datos::datos().clima;

        // 1. Carriles: vehículos ordenados por carril y del más adelantado al último (los que
        //    adelantan van por el carril contrario: fuera de la fila).
        let ags = &self.e.agentes;
        let mut orden: Vec<(u32, f32, u32)> = (0..n).filter(|&i| ags[i].tipo.vehiculo() && ags[i].adelantando.is_none()).map(|i| (carril(ags[i].pata().tramo, ags[i].pata().sentido), ags[i].s, i as u32)).collect();
        orden.sort_unstable_by(|x, y| x.0.cmp(&y.0).then(y.1.total_cmp(&x.1)).then(x.2.cmp(&y.2)));
        let mut rangos: HashMap<u32, (usize, usize), FxBuild> = HashMap::default();
        let mut lugar = vec![usize::MAX; n];
        for (k, &(c, _, i)) in orden.iter().enumerate() {
            rangos.entry(c).and_modify(|r| r.1 = k + 1).or_insert((k, k + 1));
            lugar[i as usize] = k;
        }
        let largo = |i: u32| ags[i as usize].largo();
        let ultimo_de = |c: u32| rangos.get(&c).map(|&(_, e)| orden[e - 1].2);

        // 2. Cruces. Quien llega a cada cruce: el primero de cada fila (para la prioridad y
        //    para los peatones que cruzan) y los que piden paso (a menos de ACERCAMIENTO).
        let mut primeros: HashMap<u32, Vec<(usize, f32)>, FxBuild> = HashMap::default();
        let mut pedidos: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for &(a0, b0) in rangos.values() {
            let Some(i) = (a0..b0).map(|k| orden[k].2 as usize).find(|&i| !ags[i].ultima()) else { continue };
            let a = &ags[i];
            let p = a.pata();
            let d = p.hasta - a.s;
            if d > 60.0 {
                continue;
            }
            let nodo = fin(net, &p);
            // Tiempo hasta el cruce (s).
            primeros.entry(nodo).or_default().push((i, d / a.v.max(0.5)));
            if d <= ACERCAMIENTO && grado(net, nodo) >= 3 {
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
                Some(u) => ags[u as usize].s - largo(u) - sig.desde > a.largo() + 3.0,
            }
        };
        let mut cruces = std::mem::take(&mut self.e.cruces);
        cruces.retain(|&nodo, &mut (id, espera0)| {
            let Some(i) = ags.binary_search_by_key(&id, |a| a.id).ok() else { return false };
            let a = &ags[i];
            let p = a.pata();
            // Lo tiene hasta que la cola del vehículo (la tropa entera) salió del cruce.
            let tiene = fin(net, &p) == nodo || (a.li > 0 && fin(net, &a.patas[a.li as usize - 1]) == nodo && a.s - p.desde < 7.0 + a.largo_extra);
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
            let lleg = |i: usize| if ags[i].llegada < 0.0 { reloj } else { ags[i].llegada };
            // Prioridad: en un cruce con calle empedrada, pasan primero los que vienen por
            // ella; los de la calle de tierra esperan un hueco (más corto cuanto más esperan).
            let jer = con_jerarquia(net, nodo);
            let principal = |i: usize| !jer || net.edges[&ags[i].pata().tramo].kind == RoadKind::Stone;
            let hueco_ok = |i: usize| {
                let espera = (reloj - lleg(i)) as f32;
                // Red de seguridad (como la del que tiene el paso): después de un minuto y
                // medio entra igual.
                if espera > 90.0 {
                    return true;
                }
                // El hueco se mide desde la línea de detención (Raff y Hart): hay que llegar.
                let a = &ags[i];
                if a.pata().hasta - a.s > 8.0 {
                    return false;
                }
                // Al hueco de la persona se suma lo que tarda el vehículo en despejar el cruce
                // (una carreta de bueyes necesita un hueco mucho más largo que un jinete).
                let critico = a.perfil.hueco_critico(espera) + (a.largo() + 6.0) / a.v0.max(0.3);
                primeros.get(&nodo).is_none_or(|v| v.iter().all(|&(j, tta)| j == i || !principal(j) || tta >= critico || (reqs.contains(&j) && !salida_libre(j))))
            };
            let libres: Vec<usize> = reqs.iter().copied().filter(|&i| salida_libre(i)).collect();
            let elegible: Vec<usize> = if libres.iter().any(|&i| principal(i)) { libres.iter().copied().filter(|&i| principal(i)).collect() } else { libres.iter().copied().filter(|&i| hueco_ok(i)).collect() };
            if let Some(&i) = elegible.iter().min_by(|&&x, &&y| lleg(x).total_cmp(&lleg(y)).then(ags[x].id.cmp(&ags[y].id))) {
                cruces.insert(nodo, (ags[i].id, ags[i].espera));
            }
        }
        // Tramos donde alguien ya está adelantando (uno por vez).
        let adelantan: BTreeSet<u32> = ags.iter().filter(|a| a.adelantando.is_some()).map(|a| a.pata().tramo).collect();
        // Peatones en la calzada (los que acaban de doblar una esquina): los vehículos frenan.
        let mut cruzando: BTreeSet<u32> = BTreeSet::new();
        for a in ags.iter().filter(|a| !a.tipo.vehiculo() && a.li > 0) {
            let p = a.pata();
            if a.s - p.desde < 3.0 {
                cruzando.insert(fin(net, &a.patas[a.li as usize - 1]));
            }
        }

        // 3. Conducción (IDM), todos con el estado del paso anterior.
        let mut nuevo: Vec<(f32, f32, Marcha)> = Vec::with_capacity(n);
        let mut eventos: Vec<Evento> = Vec::new();
        let mut colas: BTreeMap<u64, ([u16; 2], u16)> = BTreeMap::new();
        for i in 0..n {
            let a = &ags[i];
            let dt_i = dts[i];
            if dt_i == 0.0 {
                nuevo.push((a.s, a.v, a.marcha));
                continue;
            }
            let pata = a.pata();
            let edge = &net.edges[&pata.tramo];
            let hasta_fin = pata.hasta - a.s;
            // El portón de esta pata (si hay): cola, revisión y cierre.
            let mut porton: Option<(f32, u64)> = a.porton_aqui();
            if let Some((pos, clave)) = porton {
                if a.revision > 0.0 && reloj >= a.revision {
                    eventos.push(Evento::Paso(i, clave));
                    porton = None;
                } else if pos - a.s < 80.0 {
                    if a.en_cola == 0.0 {
                        eventos.push(Evento::EnCola(i));
                    }
                    let adentro = self.porton(clave).is_some_and(|g| g.hacia_adentro == pata.sentido);
                    let c = colas.entry(clave).or_default();
                    c.0[usize::from(!adentro)] += 1;
                    if !abierto {
                        c.1 += 1;
                    }
                }
            }
            if !a.tipo.vehiculo() {
                // A pie: por la vereda, a su paso; en las esquinas cruzan la calzada con hueco.
                let mut s1 = a.s + a.v0 * dt_i;
                let mut marcha = Marcha::EnMarcha;
                if let Some((pos, clave)) = porton {
                    if !abierto {
                        s1 = s1.min(pos - 0.5).max(a.s);
                        marcha = Marcha::EsperandoElPorton;
                    } else if s1 >= pos {
                        eventos.push(Evento::Paso(i, clave));
                    }
                }
                if !a.ultima() && s1 >= pata.hasta - 0.3 && grado(net, fin(net, &pata)) >= 2 {
                    let nodo = fin(net, &pata);
                    let hueco = a.perfil.hueco_peaton();
                    if primeros.get(&nodo).is_some_and(|v| v.iter().any(|&(j, tta)| tta < hueco && ags[j].v > 0.3)) {
                        s1 = (pata.hasta - 0.3).max(a.s);
                        marcha = Marcha::CediendoElPaso;
                    }
                }
                let v = (s1 - a.s) / dt_i;
                nuevo.push((s1, v, marcha));
                continue;
            }
            if reloj < a.detenido_hasta {
                nuevo.push((a.s, 0.0, Marcha::Empantanada));
                continue;
            }
            let p = a.params();
            let (mut hueco, mut v_lider) = (f32::INFINITY, 0.0);
            let mut marcha = Marcha::EnMarcha;
            let adelantando = a.adelantando.is_some();
            let mut lider: Option<usize> = None;
            if !adelantando {
                let k = lugar[i];
                let (c0, _) = rangos[&carril(pata.tramo, pata.sentido)];
                if k > c0 {
                    let l = orden[k - 1].2;
                    hueco = ags[l as usize].s - largo(l) - a.s;
                    v_lider = ags[l as usize].v;
                    lider = Some(l as usize);
                } else if !a.ultima() {
                    let sig = a.patas[a.li as usize + 1];
                    if let Some(u) = ultimo_de(carril(sig.tramo, sig.sentido)) {
                        hueco = hasta_fin + ags[u as usize].s - largo(u) - sig.desde;
                        v_lider = ags[u as usize].v;
                    }
                }
            } else if let Some(j) = a.adelantando.and_then(|id| ags.binary_search_by_key(&id, |x| x.id).ok()) {
                // Adelantando: vuelve a su carril cuando dejó atrás al otro con lugar.
                let b = &ags[j];
                // (o si quedó atrás; nunca encimado: si se acaba la calle sigue por afuera hasta
                // librarlo).
                if b.pata().tramo != pata.tramo || b.pata().sentido != pata.sentido || a.s - a.largo() >= b.s + 3.0 || (hasta_fin < 4.0 && b.s - b.largo() >= a.s + 2.0) {
                    eventos.push(Evento::Vuelve(i));
                }
            } else {
                eventos.push(Evento::Vuelve(i));
            }
            // El portón: se detiene antes; el primero de la fila espera la revisión.
            if let Some((pos, _)) = porton {
                let d = pos - a.s;
                let stop = d - 1.0;
                if stop < hueco {
                    hueco = stop;
                    v_lider = 0.0;
                }
                let primero = lider.is_none_or(|l| ags[l].s - ags[l].largo() > pos);
                if a.revision > 0.0 {
                    marcha = Marcha::EnRevision;
                } else if !abierto && d < 30.0 {
                    marcha = Marcha::EsperandoElPorton;
                } else if abierto && primero && d < 4.0 && a.v < 0.5 {
                    let (seg, _) = portones::revision(a.tipo, a.cargada, a.cabezas, azar_n(self.e.semilla, a.id as u64, 0, Motivo::Revision as u32, a.porton_i as u32));
                    eventos.push(Evento::Revision(i, reloj + seg.max(0.25) as f64));
                    marcha = Marcha::EnRevision;
                } else if d < 30.0 && a.v < 0.3 {
                    marcha = Marcha::EsperandoElPorton;
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
                        if a.v < 0.3 && marcha == Marcha::EnMarcha {
                            marcha = Marcha::CediendoElPaso;
                        }
                    }
                }
                // Gente cruzando la calzada adelante: frena (no había reglas, pero nadie los
                // atropella).
                if hasta_fin < 14.0 && hasta_fin > 4.5 && cruzando.contains(&nodo) && cruces.get(&nodo).map(|c| c.0) != Some(a.id) {
                    let stop = hasta_fin - 4.5;
                    if stop < hueco {
                        hueco = stop;
                        v_lider = 0.0;
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
            // Velocidad deseada: tipo de calle (y barro), curvas y giros.
            let tierra = edge.kind == RoadKind::Dirt;
            let mut v0 = a.v0 * if tierra { 0.88 } else { 1.0 };
            if tierra && barro && a.tipo == Tipo::Carreta {
                v0 *= clima.carreta_en_barro;
            }
            v0 = v0.min(velocidad_en_curva(net, &pata, a.s));
            if !a.ultima() && hasta_fin < 18.0 && giro(net, &pata, &a.patas[a.li as usize + 1]) > 0.5 {
                v0 = v0.min(1.6);
            }
            // Adelantar: un jinete poco prudente detrás de una carreta o un carro lento, con
            // el carril contrario libre (nunca a un arreo ni cerca del cruce o del portón).
            if let (Some(l), Some(libre), false) = (lider, a.perfil.adelanta(), adelantando) {
                let b = &ags[l];
                if a.tipo == Tipo::Jinete && matches!(b.tipo, Tipo::Carreta | Tipo::Carro) && hueco < 14.0 && b.v < 0.7 * a.v0 && a.revision == 0.0 {
                    // Lo que recorre hasta pasarlo con lugar.
                    let rel = (a.v0 - b.v).max(0.3);
                    let falta = b.s + 3.0 + a.largo() - a.s;
                    let recorre = falta / rel * a.v0;
                    let sin_porton = porton.is_none_or(|(pos, _)| pos > a.s + recorre + 5.0);
                    // Y lugar adelante del otro para volver al carril.
                    let k = lugar[l];
                    let (c0, _) = rangos[&carril(pata.tramo, pata.sentido)];
                    let lugar_adelante = k == c0 || {
                        let ll = orden[k - 1].2 as usize;
                        ags[ll].s - ags[ll].largo() > a.s + recorre + 10.0
                    };
                    if a.s + recorre < pata.hasta - 12.0 && sin_porton && lugar_adelante && contrario_libre(ags, &adelantan, &rangos, &orden, net, &pata, a.s, recorre.max(libre)) {
                        eventos.push(Evento::Adelanta(i, b.id));
                    }
                }
            }
            if adelantando {
                marcha = Marcha::Adelantando;
            }
            let (ds, v) = idm::paso(&p, v0, a.t, a.v, hueco, v_lider, dt_i);
            if v < 0.1 && marcha == Marcha::EnMarcha {
                marcha = Marcha::EsperandoEnLaFila;
            }
            nuevo.push((a.s + ds, v, marcha));
        }
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
        // Lo decidido: portones y adelantamientos.
        for g in self.e.portones.values_mut() {
            g.cola = [0, 0];
            g.cerrado_esperan = 0;
        }
        for (clave, (c, cerrado)) in colas {
            if let Some(g) = self.e.portones.get_mut(&clave) {
                g.cola = c;
                g.cerrado_esperan = cerrado;
                g.cola_max_hoy = g.cola_max_hoy.max(c[0] + c[1]);
            }
        }
        for ev in eventos {
            match ev {
                Evento::Revision(i, hasta) => self.e.agentes[i].revision = hasta,
                Evento::EnCola(i) => self.e.agentes[i].en_cola = reloj,
                Evento::Paso(i, clave) => {
                    let a = &mut self.e.agentes[i];
                    if a.porton_aqui().map(|x| x.1) != Some(clave) {
                        continue;
                    }
                    let espera = if a.en_cola > 0.0 { (reloj - a.en_cola) as f32 } else { 0.0 };
                    let (_, reales) = portones::revision(a.tipo, a.cargada, a.cabezas, 0.0);
                    a.porton_i += 1;
                    a.revision = 0.0;
                    a.en_cola = 0.0;
                    let vehiculo = a.tipo.vehiculo();
                    if let Some(g) = self.e.portones.get_mut(&clave) {
                        g.pasados_hoy += 1;
                        g.reales_hoy += reales;
                        g.reales_total += reales as f64;
                        if vehiculo {
                            g.espera_hoy += espera;
                            g.esperas_hoy += 1;
                        }
                    }
                }
                Evento::Adelanta(i, id) => {
                    if self.e.agentes[i].adelantando.is_none() {
                        self.e.agentes[i].adelantando = Some(id);
                        self.e.stats.adelantamientos_hoy += 1;
                    }
                }
                Evento::Vuelve(i) => self.e.agentes[i].adelantando = None,
            }
        }

        // 4. Cambios de tramo y llegadas.
        let mut llegados = Vec::new();
        let mut perdidos = Vec::new();
        let mut i = 0;
        while i < self.e.agentes.len() {
            match self.avanzar_patas(net, i) {
                Avance::Sigue => i += 1,
                Avance::Llego => llegados.push(self.e.agentes.remove(i)),
                Avance::Perdido => {
                    perdidos.push(self.e.agentes[i].id);
                    i += 1;
                }
            }
        }
        for id in perdidos {
            self.recalcular(net, id);
        }
        for a in llegados {
            self.llego(net, a);
        }
    }

    fn avanzar_patas(&mut self, net: &RoadNetwork, i: usize) -> Avance {
        let reloj = self.e.reloj;
        let hora = self.e.hora;
        let (barro, semilla, dia) = (self.e.barro, self.e.semilla, self.e.dia);
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
            // Tiempo medido en el carril (solo si lo recorrió entero), relativo a lo que tarda
            // ese tipo de vehículo con la calle libre (D6).
            let medir = a.tipo.vehiculo() && p.desde < 0.01;
            let tardo = (reloj - a.t_carril) as f32;
            a.li += 1;
            a.t_carril = reloj;
            a.llegada = -1.0;
            a.adelantando = None;
            let sig = a.pata();
            let Some(e) = net.edges.get(&sig.tramo) else { return Avance::Perdido };
            a.s = sig.desde + sobra;
            // Barro: una carreta que entra a un camino de tierra puede quedar empantanada.
            if barro && a.tipo == Tipo::Carreta && e.kind == RoadKind::Dirt {
                let c = &datos::datos().clima;
                if azar_n(semilla, a.id as u64, dia, Motivo::Empantanar as u32, a.li as u32) < c.empantanarse {
                    let m = c.empantanado_minutos[0] + (c.empantanado_minutos[1] - c.empantanado_minutos[0]) * azar_n(semilla, a.id as u64, dia, Motivo::Empantanar as u32, 1000 + a.li as u32);
                    a.detenido_hasta = reloj + m as f64 * 60.0;
                    self.e.stats.empantanadas_hoy += 1;
                }
            }
            if medir {
                let pe = net.edges.get(&p.tramo);
                if let Some(pe) = pe {
                    let a = &self.e.agentes[i];
                    let v_libre = a.tipo.params().v0 * if pe.kind == RoadKind::Dirt { 0.88 } else { 1.0 };
                    let v_libre = if a.tipo == Tipo::Arreo { a.v0 } else { v_libre };
                    let libre_tipo = pe.len() / v_libre.max(0.1);
                    self.e.tiempos.registrar(net, p.tramo, p.sentido, tardo, libre_tipo, reloj, hora);
                }
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
            if self.e.vecinos.get(&k).is_some_and(|v| v.en_camino == Some(a.id)) {
                self.vecino_llego(Some(k), self.ahora(), false);
            }
            return;
        }
        // Carretas y carros: descargan y vuelven más tarde; los troperos vuelven a caballo.
        let (vuelta, tipo, espera, destino) = match a.proposito {
            Proposito::TraeDelCampo => {
                self.e.stats.carretas_al_mercado += 1;
                (Proposito::VuelveAlCampo, a.tipo, 600.0 + 900.0 * self.azar(), self.lugares.campo)
            }
            Proposito::Reparto => (Proposito::VuelveDelReparto, a.tipo, 120.0 + 280.0 * self.azar(), self.lugares.puerto.or(self.lugares.campo)),
            Proposito::ArreoALosCorrales => (Proposito::VuelveAlCampo, Tipo::Jinete, 300.0 + 300.0 * self.azar(), self.lugares.campo),
            _ => return,
        };
        if let Some(b) = destino {
            self.e.pendientes.push(Pendiente { cuando: self.e.reloj + espera as f64, tipo, desde: aqui.into(), hasta: b.into(), que: vuelta, temple: a.temple, conductor: a.conductor });
        }
    }

    /// Trabajo dado desde afuera (por ejemplo, por un servicio): el vecino sale a trabajar a
    /// ese lugar y no se le reelige otro. Devuelve si el vecino existe.
    #[allow(dead_code)]
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

    /// Tránsito de un tramo: cuántos hay ahora de cada tipo y el tiempo de paso medido (en la
    /// franja de ahora).
    pub fn info_tramo(&self, net: &RoadNetwork, tramo: u32) -> Option<InfoTramo> {
        net.edges.get(&tramo)?;
        let mut i = InfoTramo { libre: Tiempos::libre(net, tramo), ..Default::default() };
        for a in &self.e.agentes {
            if a.pata().tramo == tramo {
                i.por_tipo[a.tipo as usize] += 1;
            }
        }
        let f = ruta::franja(self.e.hora);
        i.medido = [i.libre * self.e.tiempos.demora(tramo, 0, f, self.e.reloj), i.libre * self.e.tiempos.demora(tramo, 1, f, self.e.reloj)];
        Some(i)
    }

    /// Efecto del tránsito en la opinión de los vecinos este mes (y lo pone en cero): si los
    /// viajes en vehículo tardan mucho más de lo previsto, se quejan; si andan bien, nada.
    pub fn efecto_opinion(&mut self) -> f32 {
        let st = &mut self.e.stats;
        let r = if st.viajes_mes >= 20 { st.demora_mes / st.viajes_mes as f32 } else { 1.0 };
        st.viajes_mes = 0;
        st.demora_mes = 0.0;
        st.contrabando_mes = 0;
        for g in self.e.portones.values_mut() {
            g.contrabando_mes = 0.0;
        }
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

    /// Ficha de un agente: (título, líneas). Con la población, el nombre, el oficio y la edad
    /// de la persona; con la agenda del día y por qué eligió su ruta.
    pub fn describir(&self, id: u32, pob: Option<&crate::city::poblacion::Poblacion>) -> Option<(String, Vec<String>)> {
        let a = &self.e.agentes[self.indice(id)?];
        let mut l = Vec::new();
        let vecino = a.vecino.and_then(|k| self.e.vecinos.get(&k));
        let persona = a.persona.and_then(|p| pob.and_then(|pob| pob.persona(p).map(|x| (pob, x))));
        let titulo = match (vecino, persona) {
            (Some(v), Some((pob, p))) => {
                let clase = crate::city::poblacion::modelo::Clase::ALL[v.clase.min(3) as usize].nombre();
                l.push(format!("{}, {} años · familia {}", capitalizar(p.oficio.nombre(p.sexo)), p.edad(pob.mes), clase));
                l.push(perfil_texto(&v.perfil));
                pob.nombre_completo(p.id)
            }
            (Some(v), None) => {
                l.push(format!("{}, {}", capitalizar(OFICIOS[v.oficio as usize % OFICIOS.len()]), ORIGENES[v.origen as usize % ORIGENES.len()]));
                l.push(format!("Temperamento: {}", TEMPLES[v.temple as usize % 3].0));
                v.nombre()
            }
            _ => {
                let quien = format!("{} {}", NOMBRES[a.conductor.0 as usize % NOMBRES.len()], APELLIDOS[a.conductor.1 as usize % APELLIDOS.len()]);
                l.push(format!("{}: {quien}", if a.tipo == Tipo::Arreo { "Capataz" } else { "Conduce" }));
                capitalizar(match a.tipo {
                    Tipo::Carreta if a.contrabando => "carreta de contrabando",
                    Tipo::Carreta => "carreta de bueyes",
                    Tipo::Carro => "carro de reparto",
                    Tipo::Jinete => "jinete",
                    Tipo::APie => "caminante",
                    Tipo::Arreo => "arreo de ganado",
                })
            }
        };
        if a.tipo == Tipo::Arreo {
            l.push(format!("Tropa de {} cabezas ({} m de largo), {} troperos a caballo", a.cabezas, a.largo().round() as i32, (a.cabezas as u32).div_ceil(datos::datos().tropas.cabezas_por_tropero.max(1))));
        }
        l.push(format!("Va {}, {}", a.proposito.texto(), a.tipo.nombre()));
        l.push(format!("Estado: {} · {} km/h", a.marcha.texto(), (a.v * 3.6).round() as i32));
        // D3: la duración física del viaje y la del reloj, aparte.
        let fisico = (self.e.reloj - a.t0) as f32 / 60.0;
        l.push(format!("En camino: {} min ({} min del reloj; prevé {})", fisico.round() as i32, ((self.e.reloj - a.t0) as f32 * MINUTOS_POR_SEGUNDO) as i32, (a.previsto * MINUTOS_POR_SEGUNDO) as i32));
        l.push(format!("Ruta: {}", a.ruta.texto()));
        if let Some(v) = vecino {
            if v.cambios_de_ruta > 0 {
                l.push(format!("Cambió de ruta {} {} después de un viaje malo", v.cambios_de_ruta, if v.cambios_de_ruta == 1 { "vez" } else { "veces" }));
            }
            if !v.plan.is_empty() {
                l.push(format!("Agenda del {}:", self.dia_de_semana()));
                l.extend(agenda::lineas(v, self.e.dia).into_iter().map(|x| format!("  {x}")));
            }
        }
        Some((titulo, l))
    }

    /// Ficha de un portón cerca de `p` (a menos de 14 m): (título, líneas).
    pub fn ficha_porton(&self, p: Vec2) -> Option<(String, Vec<String>)> {
        let g = self.portones.iter().filter(|g| g.pos.distance(p) < 14.0).min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p)))?;
        let st = self.e.portones.get(&g.clave).copied().unwrap_or_default();
        let d = &datos::datos().porton;
        let mut l = Vec::new();
        let h = |x: f32| agenda::hhmm(x as f64);
        if portones::abierto(self.e.hora) {
            l.push(format!("Abierto (de {} a {})", h(d.abre), h(d.cierra)));
        } else {
            l.push(format!("Cerrado hasta las {}: {} esperan afuera", h(d.abre), st.cerrado_esperan));
        }
        l.push(format!("Cola ahora: {} entrando, {} saliendo (máxima de hoy {})", st.cola[0], st.cola[1], st.cola_max_hoy));
        match st.espera_media() {
            Some(e) => l.push(format!("Espera media en la cola: {} ({} min del reloj)", minutos_fisicos(e), (e * MINUTOS_POR_SEGUNDO).round() as i32)),
            None => l.push("Espera media: todavía no pasó nadie".into()),
        }
        l.push(format!("Pasaron hoy: {} (ayer {})", st.pasados_hoy, st.pasados_ayer));
        l.push(format!("Derechos cobrados: {} reales hoy, {} ayer, {} pesos en total", st.reales_hoy.round() as i32, st.reales_ayer.round() as i32, (st.reales_total / 8.0).round() as i64));
        if st.contrabando_mes > 0.05 {
            l.push(format!("Contrabando estimado este mes: {:.1} carretas evitaron los portones", st.contrabando_mes).replace('.', ","));
        }
        l.push("Abrir otro portón reparte la cola: los carreteros eligen la ruta por lo que tardan las calles.".into());
        Some(("Portón de la muralla".into(), l))
    }
}

/// ¿Está libre el carril contrario por delante (de `s` a `s + largo`, en el sentido de la
/// marcha)? Sin nadie que venga de frente ni otro adelantando en el tramo.
fn contrario_libre(ags: &[Agente], adelantan: &BTreeSet<u32>, rangos: &HashMap<u32, (usize, usize), FxBuild>, orden: &[(u32, f32, u32)], net: &RoadNetwork, pata: &Pata, s: f32, largo: f32) -> bool {
    if adelantan.contains(&pata.tramo) {
        return false;
    }
    let l = net.edges[&pata.tramo].len();
    if let Some(&(a0, b0)) = rangos.get(&carril(pata.tramo, 1 - pata.sentido)) {
        for &(_, so, j) in &orden[a0..b0] {
            // Frente del que viene, en nuestras coordenadas; su cola queda más adelante.
            let x = l - so;
            let cola = x + ags[j as usize].largo();
            if cola > s - 5.0 && x < s + largo {
                return false;
            }
        }
    }
    true
}

/// El tramo más cercano a un punto (a menos de `maxd`) que no esté vetado: si el más cercano
/// lo está, se busca entre todos (pasa pocas veces: arreos y contrabandistas).
fn cercano_permitido(net: &RoadNetwork, p: Vec2, maxd: f32, veto: Option<&BTreeSet<u32>>) -> Option<super::roads::Nearest> {
    let n = net.nearest_edge(p, maxd)?;
    let Some(v) = veto.filter(|v| v.contains(&n.edge)) else { return Some(n) };
    let mut best: Option<(f32, super::roads::Nearest)> = None;
    for (&id, e) in &net.edges {
        if v.contains(&id) {
            continue;
        }
        for (k, w) in e.pts.windows(2).enumerate() {
            let q = punto_en_segmento(p, w[0], w[1]);
            let d = q.distance(p);
            if d <= maxd && best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, super::roads::Nearest { edge: id, seg: k, point: q, dist: d }));
            }
        }
    }
    best.map(|b| b.1)
}

fn punto_en_segmento(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-6 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    a + ab * t
}

/// Distancia de un punto a un segmento.
fn dist_segmento(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-6 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// Segundos simulados como minutos físicos («3,5 min»).
fn minutos_fisicos(seg: f32) -> String {
    let m = seg / 60.0;
    if m < 10.0 {
        format!("{:.1} min", m).replace('.', ",")
    } else {
        format!("{} min", m.round() as i32)
    }
}

/// El perfil en palabras (para la ficha).
fn perfil_texto(p: &Perfil) -> String {
    let nivel = |x: f32, bajo: &'static str, medio: &'static str, alto: &'static str| if x < 0.34 { bajo } else if x < 0.67 { medio } else { alto };
    format!(
        "Carácter: {}, {}, {}",
        nivel(p.prudencia(), "poco prudente", "prudente a medias", "muy prudente"),
        nivel(p.prisa(), "sin apuro", "con algo de apuro", "siempre apurado"),
        nivel(p.social(), "de pocos amigos", "sociable", "muy sociable")
    )
}

/// Clave de un vecino sintético: solar de su casa y número de habitante dentro de ella.
pub fn clave_vecino(lote: u32, idx: u8) -> u64 {
    ((lote as u64) << 8) | idx as u64
}

/// Habitante `idx` de una casa, sintético (antes de fundar la población): sus atributos salen
/// de la semilla del solar (los mismos siempre).
pub fn habitante_de_lote(l: &super::lots::Lot, idx: u8, semilla: u64) -> Vecino {
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
    let nombre = ((r.next() % NOMBRES.len() as u64) as u8, (r.next() % APELLIDOS.len() as u64) as u8);
    let origen = (r.next() % ORIGENES.len() as u64) as u8;
    let oficio = (r.next() % OFICIOS.len() as u64) as u8;
    let temple = (r.next() % 3) as u8;
    let entrada = (6.4 + normal(&mut r, 0.7)).clamp(5.0, 9.0);
    let salida = (17.2 + normal(&mut r, 0.7)).clamp(15.5, 19.5);
    let semilla_ruta = (r.next() % 100_000) as u32;
    let ropa = (r.next() % dibujo::ROPA.len() as u64) as u8;
    let k = clave_vecino(l.id, idx);
    let edad = if rol == Rol::NoTrabaja { if r.f() < 0.7 { 9 } else { 66 } } else { 22 + (r.next() % 30) as u8 };
    Vecino {
        casa: l.id,
        persona: None,
        trabajo_fijo: false,
        nombre,
        origen,
        oficio,
        rol,
        trabajo: Trabajo::Ninguno,
        tipo,
        temple,
        entrada,
        salida,
        semilla_ruta,
        cambios_de_ruta: 0,
        lugar: Lugar::Casa,
        ultimo: None,
        ropa,
        perfil: Perfil::sintetico(k, temple, edad, tipo == Tipo::Jinete, semilla),
        clase: if rico { 2 } else { 1 },
        compra: idx == 0 && rol != Rol::Fuera,
        pulperia: None,
        visita: None,
        plan: Vec::new(),
        paso_plan: 0,
        plan_dia: 0,
        sale: None,
        en_camino: None,
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
    l.push(format!("Paso a esta hora: {} / {} (libre {}){}", minutos(i.medido[0]), minutos(i.medido[1]), minutos(i.libre), if lento { " · se demora" } else { "" }));
    if ag.intramuros.contains(&tramo) {
        l.push("Intramuros: los arreos no pasan".into());
    }
    if ag.veto_portones.contains(&tramo) {
        l.push("Pasa por un portón de la muralla".into());
    }
    l
}

/// Filas del panel de tránsito: (concepto, valor, ¿es malo?).
pub fn resumen(ag: &Agentes) -> Vec<(String, String, bool)> {
    let st = &ag.e.stats;
    let n = ag.en_la_calle();
    let total: u32 = n.iter().sum();
    let mut f = vec![
        ("En la calle ahora".to_string(), format!("{total} ({})", Tipo::TODOS.iter().filter(|t| **t != Tipo::Arreo || n[**t as usize] > 0).map(|t| t.cuantos(n[*t as usize])).collect::<Vec<_>>().join(", ")), false),
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
    if !ag.portones.is_empty() {
        let cola: u32 = ag.e.portones.values().map(|g| (g.cola[0] + g.cola[1]) as u32).sum();
        f.push(("En cola en los portones".into(), format!("{cola}"), cola > 8));
        f.push(("Contrabando este mes".into(), format!("{} carretas", st.contrabando_mes), st.contrabando_mes > 10));
    }
    if st.arreos_hoy > 0 {
        f.push(("Arreos hoy".into(), format!("{}", st.arreos_hoy), false));
    }
    if ag.e.lluvia {
        f.push(("Lluvia".into(), format!("{} salidas suspendidas{}", st.suspendidos_hoy, if ag.e.barro { " · barro en los caminos" } else { "" }), ag.e.barro));
    }
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
/// la derecha, la gente a pie por la vereda; quien adelanta, por el carril contrario).
pub fn pose(net: &RoadNetwork, a: &Agente) -> (Vec2, Vec2) {
    pose_en(net, a, a.li, a.s)
}

/// Pose en una pata y posición dadas (para la tropa de un arreo, que va detrás de la punta).
fn pose_en(net: &RoadNetwork, a: &Agente, li: u16, s: f32) -> (Vec2, Vec2) {
    let p = a.patas[li as usize];
    let Some(e) = net.edges.get(&p.tramo) else { return (Vec2::ZERO, Vec2::X) };
    let l = e.len();
    let ps = if p.sentido == 0 { s } else { l - s };
    let (pt, tan) = e.point(ps);
    let tan = if p.sentido == 0 { tan } else { -tan };
    let w = e.kind.width();
    let lat = match a.tipo {
        Tipo::APie => w * 0.5 - 1.0 - (a.id % 3) as f32 * 0.35,
        Tipo::Arreo => 0.0,
        _ if a.adelantando.is_some() => -w * 0.25,
        _ => w * 0.25,
    };
    let derecha = Vec2::new(-tan.y, tan.x);
    (pt + derecha * lat, tan)
}

/// Pose `d` metros detrás del frente de un agente, siguiendo su camino hacia atrás, con la
/// pata y la posición en ella.
pub fn pose_atras_con(net: &RoadNetwork, a: &Agente, d: f32) -> (Vec2, Vec2, u16, f32) {
    let mut li = a.li;
    let mut s = a.s - d;
    while s < a.patas[li as usize].desde && li > 0 {
        let falta = a.patas[li as usize].desde - s;
        li -= 1;
        s = a.patas[li as usize].hasta - falta;
    }
    let s = s.max(a.patas[li as usize].desde);
    let (p, dir) = pose_en(net, a, li, s);
    (p, dir, li, s)
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
pub(crate) mod tests;
