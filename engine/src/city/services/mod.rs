//! Servicios de la ciudad colonial en el motor: agua (pozos, aljibes y aguadores), serenos con
//! sus faroles, lazareto, el campo (chacras y estancias), hornos y canteras, la aduana, y los
//! sistemas que dependen de ellos: incendios ([`fires`]), el puerto y lo que trae
//! ([`port`]), las epidemias ([`epidemics`]) y las grandes obras por fases: la iglesia matriz y
//! la muralla ([`works`]); además escuelas, salud, saneamiento y seguridad de los barrios
//! ([`barrio`]). Porte de `scripts/city/{services,fires,port,wall,church,epidemics}.gd`
//! del prototipo; el diseño y las fuentes están en `docs/sistemas/servicios.md`.
//!
//! Todo vive en [`Services`] (dentro de `City`) y se engancha a la ciudad con pocos puntos:
//! - cada paso de crecimiento (medio segundo de juego): fuego y contagios ([`City::services_step`]);
//! - cada mes, antes de la economía: agua, obras, barcos y epidemias ([`City::services_month`]);
//! - lo que se le pasa a la economía: producción del campo, barcos, agua, faroles, contrabando
//!   y mantenimiento ([`City::services_input`]);
//! - los vóxeles al cargar la partida ([`City::services_rebuild`]).
//!
//! Pensado para crecer por épocas: cada servicio tiene su ficha en [`CATALOG`] y las épocas
//! siguientes agregan tipos (bomba de incendios, red de agua corriente, comisaría…) sin tocar
//! la lógica de cobertura.

pub mod barrio;
pub mod epidemics;
pub mod fires;
pub mod port;
pub mod save;
pub mod works;

use super::economy::{MonthInput, Ordinance, Res};
use super::lots::{corners_of, point_in_poly, polys_overlap, Lot, Lots, Obstacle, ZoneKind, CAP};
use super::roads::RoadNetwork;
use super::voxelize::{build_rotated, road_height};
use super::City;
use crate::buildings::{Edits, Rng};
use crate::lod::{BASE_VOXEL, DESIGN_VOXEL};
use crate::palette::AIR;
use crate::structures::colonial;
use crate::structures::Frame;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};
use std::collections::{BTreeMap, BTreeSet, HashSet};

// --- Catálogo -------------------------------------------------------------------------------

/// Tipos de edificio de servicio que se colocan sobre el mapa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Pozo,
    Aljibe,
    Aguadores,
    Serenos,
    Lazareto,
    Chacra,
    Estancia,
    Horno,
    Cantera,
    Aduana,
    // --- Educación, salud, seguridad y saneamiento (ver [`barrio`]) ---
    Escuela,
    Convento,
    Botica,
    Medico,
    Hospital,
    Alcalde,
    Carcel,
    Basurero,
    // --- Desarrollo: instituciones e industrias (ver `city::desarrollo`) ---
    EscuelaOficios,
    EscuelaNautica,
    Saladero,
    Molino,
    Astillero,
    Imprenta,
}

/// Grupos de servicios (pestañas del cajón de Servicios y capas de datos).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Grupo {
    Agua,
    Educacion,
    Salud,
    Seguridad,
    Saneamiento,
    /// Iglesia, campo, industria, puerto.
    Otros,
}

impl Grupo {
    /// Pestañas del cajón de Servicios (en este orden).
    pub const PESTAÑAS: [Grupo; 6] = [Grupo::Agua, Grupo::Educacion, Grupo::Salud, Grupo::Seguridad, Grupo::Saneamiento, Grupo::Otros];

    pub fn nombre(self) -> &'static str {
        match self {
            Grupo::Agua => "Agua",
            Grupo::Educacion => "Educación",
            Grupo::Salud => "Salud",
            Grupo::Seguridad => "Seguridad",
            Grupo::Saneamiento => "Saneamiento",
            Grupo::Otros => "Iglesia",
        }
    }
}

type Builder = fn(&TerrainGen, &mut Edits, &Frame);

/// Ficha de un tipo de servicio.
pub struct Info {
    pub kind: Kind,
    pub name: &'static str,
    /// Clave para guardar la partida.
    pub key: &'static str,
    pub desc: &'static str,
    pub cost: i64,
    pub ladrillos: f32,
    pub piedra: f32,
    /// Mantenimiento mensual (pesos).
    pub upkeep: i64,
    /// Agua: personas que abastece, calidad (0 a 1) y alcance (m).
    pub cap: i32,
    pub quality: f32,
    pub radius: f32,
    /// Alcance para apagar incendios con baldes (m; 0 = no sirve).
    pub fire_reach: f32,
    /// Frente y fondo (m) y el marco de vóxeles de diseño.
    pub size: (f32, f32),
    pub frame: (i32, i32),
    pub builder: Builder,
    /// Grupo (pestaña del cajón).
    pub grupo: Grupo,
    /// Escuelas: alumnos; salud: personas que atiende; cárcel: presos. En las escuelas, la
    /// salud y la seguridad, `quality` es lo que rinde (0 a 1) y `radius` el alcance.
    pub plazas: i32,
}

pub const CATALOG: [Info; 24] = [
    Info { kind: Kind::Pozo, name: "Pozo de balde", key: "pozo", desc: "Agua de pozo para unas 60 personas cercanas. Calidad regular; los talleres cercanos la ensucian.", cost: 400, ladrillos: 0.0, piedra: 20.0, upkeep: 4, cap: 60, quality: 0.7, radius: 110.0, fire_reach: 140.0, size: (5.0, 5.0), frame: (10, 10), builder: colonial::pozo, grupo: Grupo::Agua, plazas: 0 },
    Info { kind: Kind::Aljibe, name: "Aljibe público", key: "aljibe", desc: "Junta el agua de lluvia de los techos en una cisterna. Buena agua para unas 160 personas; en la sequía rinde menos.", cost: 1500, ladrillos: 60.0, piedra: 20.0, upkeep: 12, cap: 160, quality: 1.0, radius: 170.0, fire_reach: 180.0, size: (10.0, 10.0), frame: (20, 20), builder: colonial::aljibe_publico, grupo: Grupo::Agua, plazas: 0 },
    Info { kind: Kind::Aguadores, name: "Puesto de aguadores", key: "aguadores", desc: "Los aguadores llenan sus pipas en un arroyo o manantial y las reparten en carro por las calles: llegan lejos, pero solo a donde llega la calle.", cost: 2500, ladrillos: 30.0, piedra: 0.0, upkeep: 30, cap: 360, quality: 0.9, radius: 1100.0, fire_reach: 450.0, size: (14.0, 18.0), frame: (28, 36), builder: colonial::puesto_aguadores, grupo: Grupo::Agua, plazas: 0 },
    Info { kind: Kind::Serenos, name: "Puesto de serenos", key: "serenos", desc: "De noche los serenos encienden los faroles de sebo, cantan la hora y dan la voz de fuego: los incendios se atacan antes.", cost: 1200, ladrillos: 40.0, piedra: 0.0, upkeep: 25, cap: 0, quality: 0.3, radius: 260.0, fire_reach: 0.0, size: (6.0, 8.0), frame: (12, 16), builder: colonial::puesto_serenos, grupo: Grupo::Seguridad, plazas: 0 },
    Info { kind: Kind::Lazareto, name: "Lazareto", key: "lazareto", desc: "Cuarentena para los barcos con fiebre a bordo: la enfermedad no entra a la ciudad. Lejos de la plaza y junto a la costa.", cost: 3000, ladrillos: 150.0, piedra: 50.0, upkeep: 25, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (30.0, 24.0), frame: (60, 48), builder: colonial::lazareto, grupo: Grupo::Salud, plazas: 0 },
    Info { kind: Kind::Chacra, name: "Chacra", key: "chacra", desc: "Trigo y maíz en el ejido para el pan de la ciudad. La cosecha depende del año: la sequía y las heladas la achican.", cost: 800, ladrillos: 0.0, piedra: 0.0, upkeep: 0, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (60.0, 80.0), frame: (120, 160), builder: colonial::chacra, grupo: Grupo::Otros, plazas: 0 },
    Info { kind: Kind::Estancia, name: "Estancia", key: "estancia", desc: "Ganado en el campo, lejos de la ciudad: carne, cueros y sebo. Las pasturas siguen al clima.", cost: 3000, ladrillos: 0.0, piedra: 0.0, upkeep: 0, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (50.0, 45.0), frame: (100, 90), builder: colonial::estancia, grupo: Grupo::Otros, plazas: 0 },
    Info { kind: Kind::Horno, name: "Horno de ladrillos", key: "horno", desc: "Cuece ladrillos con barro de un arroyo o de la costa. Las casas de azotea, las obras y la iglesia los necesitan. Echa humo: lejos de la plaza.", cost: 2000, ladrillos: 0.0, piedra: 0.0, upkeep: 10, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (24.0, 24.0), frame: (48, 48), builder: colonial::horno_ladrillos, grupo: Grupo::Otros, plazas: 0 },
    Info { kind: Kind::Cantera, name: "Cantera de piedra", key: "cantera", desc: "Saca piedra para empedrar, para los cimientos de la iglesia y para la muralla. Ruidosa: lejos de la plaza.", cost: 1800, ladrillos: 0.0, piedra: 0.0, upkeep: 10, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (28.0, 22.0), frame: (56, 44), builder: colonial::cantera, grupo: Grupo::Otros, plazas: 0 },
    Info { kind: Kind::Aduana, name: "Aduana", key: "aduana", desc: "Revisa lo que entra y sale por el muelle y cobra los derechos: sin aduana, buena parte del comercio se va por el contrabando.", cost: 4000, ladrillos: 200.0, piedra: 80.0, upkeep: 30, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (24.0, 20.0), frame: (48, 40), builder: colonial::aduana, grupo: Grupo::Otros, plazas: 0 },
    // Educación: la escuela del cabildo (gratis para los pobres si el cabildo paga al maestro) y
    // las aulas del convento. Alumnos de 6 a 12 años; alcance a pie.
    Info { kind: Kind::Escuela, name: "Escuela de primeras letras", key: "escuela", desc: "Un maestro enseña a leer, escribir, contar y la doctrina a unos cien niños de 6 a 12 años del barrio. Con la ordenanza de escuelas, el cabildo le paga y los pobres no pagan.", cost: 1500, ladrillos: 50.0, piedra: 10.0, upkeep: 35, cap: 0, quality: 1.0, radius: 400.0, fire_reach: 0.0, size: (12.0, 16.0), frame: (24, 32), builder: colonial::escuela, grupo: Grupo::Educacion, plazas: 100 },
    Info { kind: Kind::Convento, name: "Convento con aulas", key: "convento", desc: "Los frailes enseñan primeras letras y latín a unos sesenta niños en el claustro; llega lejos y cuesta poco por mes, pero levantarlo es caro. Uno por ciudad.", cost: 5000, ladrillos: 250.0, piedra: 80.0, upkeep: 15, cap: 0, quality: 0.8, radius: 650.0, fire_reach: 0.0, size: (30.0, 36.0), frame: (60, 72), builder: colonial::convento, grupo: Grupo::Educacion, plazas: 60 },
    // Salud: botica, médico y cirujano (va a las casas por la calle) y el hospital de caridad.
    Info { kind: Kind::Botica, name: "Botica", key: "botica", desc: "El boticario prepara remedios para unas 400 personas cercanas: algo menos de enfermedad, poco alcance.", cost: 900, ladrillos: 20.0, piedra: 0.0, upkeep: 8, cap: 0, quality: 0.35, radius: 300.0, fire_reach: 0.0, size: (8.0, 12.0), frame: (16, 24), builder: colonial::botica, grupo: Grupo::Salud, plazas: 400 },
    Info { kind: Kind::Medico, name: "Médico y cirujano", key: "medico", desc: "Casa del médico y cirujano, que sale a caballo a ver enfermos: unas 700 personas, solo por la calle (como los aguadores).", cost: 1400, ladrillos: 40.0, piedra: 0.0, upkeep: 30, cap: 0, quality: 0.5, radius: 900.0, fire_reach: 0.0, size: (12.0, 18.0), frame: (24, 36), builder: colonial::casa_medico, grupo: Grupo::Salud, plazas: 700 },
    Info { kind: Kind::Hospital, name: "Hospital de caridad", key: "hospital", desc: "Salas de camas alrededor de un patio, capilla y botica: atiende a unas 1.500 personas de su red de calles. Baja la mortalidad, sobre todo la de los niños y los pobres.", cost: 6000, ladrillos: 300.0, piedra: 100.0, upkeep: 60, cap: 0, quality: 0.85, radius: 1200.0, fire_reach: 0.0, size: (36.0, 30.0), frame: (72, 60), builder: colonial::hospital_caridad, grupo: Grupo::Salud, plazas: 1500 },
    // Seguridad: el alcalde de barrio (los serenos ya están) y la cárcel del cabildo.
    Info { kind: Kind::Alcalde, name: "Garita del alcalde de barrio", key: "alcalde", desc: "El alcalde de barrio ronda su cuartel, conoce a los vecinos y prende a los ladrones: menos robos a su alrededor.", cost: 300, ladrillos: 10.0, piedra: 0.0, upkeep: 10, cap: 0, quality: 0.35, radius: 220.0, fire_reach: 0.0, size: (4.0, 4.0), frame: (8, 8), builder: colonial::garita_alcalde, grupo: Grupo::Seguridad, plazas: 0 },
    Info { kind: Kind::Carcel, name: "Cárcel del Cabildo", key: "carcel", desc: "Calabozos con cuerpo de guardia cerca de la plaza: la guardia vigila el centro y, con lugar para treinta presos, los ladrones lo piensan dos veces en toda la ciudad. Una por ciudad.", cost: 5000, ladrillos: 200.0, piedra: 120.0, upkeep: 45, cap: 0, quality: 0.3, radius: 600.0, fire_reach: 0.0, size: (24.0, 20.0), frame: (48, 40), builder: colonial::carcel, grupo: Grupo::Seguridad, plazas: 30 },
    // Saneamiento: el hueco donde las carretillas descargan la basura.
    Info { kind: Kind::Basurero, name: "Basurero fuera de la muralla", key: "basurero", desc: "Un hueco lejos de las casas donde las carretillas descargan la basura: con la recolección, los basurales del barrio desaparecen. Ensucia los pozos cercanos.", cost: 300, ladrillos: 0.0, piedra: 0.0, upkeep: 5, cap: 0, quality: 0.0, radius: 1600.0, fire_reach: 0.0, size: (30.0, 30.0), frame: (60, 60), builder: colonial::basurero, grupo: Grupo::Saneamiento, plazas: 0 },
    // Desarrollo (ver `city::desarrollo`): se desbloquean con tecnologías. Las instituciones las
    // paga el cabildo; las industrias las puede levantar el cabildo o un inversor privado (sin
    // mantenimiento del cabildo: sus costos van en la cuenta del establecimiento).
    Info { kind: Kind::EscuelaOficios, name: "Escuela de dibujo y oficios", key: "escuela_oficios", desc: "Aulas de dibujo y geometría y un taller-escuela, donde los jóvenes que ya saben leer salen oficiales de carpintería, herrería y albañilería en dos años. Hace falta la tecnología.", cost: 2500, ladrillos: 80.0, piedra: 10.0, upkeep: 40, cap: 0, quality: 1.0, radius: 0.0, fire_reach: 0.0, size: (20.0, 18.0), frame: (40, 36), builder: colonial::escuela_oficios, grupo: Grupo::Educacion, plazas: 30 },
    Info { kind: Kind::EscuelaNautica, name: "Escuela de náutica", key: "escuela_nautica", desc: "Matemáticas, cartas y pilotaje para muchachos que leen y escriben. Forma pilotos para el cabotaje y la carpintería de ribera. Una por ciudad.", cost: 3000, ladrillos: 100.0, piedra: 20.0, upkeep: 50, cap: 0, quality: 1.0, radius: 0.0, fire_reach: 0.0, size: (12.0, 16.0), frame: (24, 32), builder: colonial::escuela_nautica, grupo: Grupo::Educacion, plazas: 15 },
    Info { kind: Kind::Saladero, name: "Saladero", key: "saladero", desc: "Faena ganado y sala la carne al sol para hacer tasajo, que se exporta, y deja cueros y sebo. Mucho capital, pocos peones por lo que produce. Huele mal: lejos de la plaza, junto a un arroyo o la costa.", cost: 6000, ladrillos: 150.0, piedra: 20.0, upkeep: 0, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (60.0, 45.0), frame: (120, 90), builder: colonial::saladero, grupo: Grupo::Otros, plazas: 0 },
    Info { kind: Kind::Molino, name: "Molino de viento", key: "molino", desc: "Muele el trigo de las chacras con el viento, y da más harina y más barata que las atahonas, con cuatro molineros.", cost: 2500, ladrillos: 40.0, piedra: 30.0, upkeep: 0, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (10.0, 10.0), frame: (20, 20), builder: colonial::molino, grupo: Grupo::Otros, plazas: 0 },
    Info { kind: Kind::Astillero, name: "Carpintería de ribera", key: "astillero", desc: "Gradas, carpinteros y calafates que carenan los barcos de paso (un servicio que pagan de afuera) y arman lanchas. Va sobre la costa.", cost: 6000, ladrillos: 60.0, piedra: 40.0, upkeep: 0, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (35.0, 25.0), frame: (70, 50), builder: colonial::astillero, grupo: Grupo::Otros, plazas: 0 },
    Info { kind: Kind::Imprenta, name: "Imprenta", key: "imprenta", desc: "Prensa de madera, tipos y papel para bandos, almanaques, cartillas y una gaceta con los precios corrientes. Una por ciudad, cerca de la plaza.", cost: 3000, ladrillos: 50.0, piedra: 0.0, upkeep: 0, cap: 0, quality: 0.0, radius: 0.0, fire_reach: 0.0, size: (10.0, 14.0), frame: (20, 28), builder: colonial::imprenta, grupo: Grupo::Otros, plazas: 0 },
];

pub fn info(k: Kind) -> &'static Info {
    CATALOG.iter().find(|i| i.kind == k).unwrap()
}

pub fn kind_from_key(s: &str) -> Option<Kind> {
    CATALOG.iter().find(|i| i.key == s).map(|i| i.kind)
}

/// Producción mensual del campo y de las industrias de servicio.
pub const CHACRA_GRAIN_YEAR: f32 = 3600.0;
pub const ESTANCIA_CARNE: f32 = 500.0;
pub const ESTANCIA_CUEROS: f32 = 60.0;
pub const ESTANCIA_SEBO: f32 = 20.0;
pub const HORNO_LADRILLOS: f32 = 40.0;
pub const CANTERA_PIEDRA: f32 = 60.0;
/// Un pozo a menos de esta distancia de un taller grande (curtiembre, velería) da agua mala.
pub const CONTAMINATION: f32 = 150.0;
/// Sebo de un farol por mes (lo cobra la economía al precio del sebo).
pub const LAMP_SPACING: f32 = 30.0;

/// Un edificio de servicio en vóxeles, con los materiales de su obra (ver `materiales`).
fn build_servicio(t: &TerrainGen, c: Vec2, f: Vec2, inf: &Info, seed: u64, obra: Option<super::materiales::Obra>) -> super::voxelize::Built {
    match obra {
        Some(o) => {
            let nom = super::materiales::ciudad::nominal_servicio();
            build_rotated(t, c, f, inf.frame.0, inf.frame.1, seed, &|t: &TerrainGen, e: &mut Edits, fr: &Frame| {
                (inf.builder)(t, e, fr);
                super::materiales::ciudad::remapear(e, &o, &nom, seed);
            })
        }
        None => build_rotated(t, c, f, inf.frame.0, inf.frame.1, seed, &inf.builder),
    }
}

/// Lo que el jugador puede colocar con la herramienta de servicios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placeable {
    Building(Kind),
    /// El muelle (con su barraca), en un punto de la costa.
    Muelle,
    /// Un tramo de muralla (dos clics).
    Muralla,
}

impl Placeable {
    pub fn name(self) -> &'static str {
        match self {
            Placeable::Building(k) => info(k).name,
            Placeable::Muelle => "Muelle",
            Placeable::Muralla => "Muralla",
        }
    }
}

/// Órdenes de la interfaz que cambian vóxeles (se cumplen en `Game::update`, que tiene el
/// terreno a mano).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Financiar la fase siguiente de la iglesia matriz.
    FundChurch,
    /// Demoler un edificio de servicio.
    Demolish(u32),
}

/// Capas de datos sobre el mapa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Agua: azul la buena, celeste la regular, rojo sin agua.
    Agua,
    /// Calles con faroles y vigiladas por los serenos.
    Serenos,
    /// Riesgo de incendio (techo de paja, sin agua cerca) y lo que arde.
    Fuego,
    /// Niños con escuela cerca y el alcance de escuelas y conventos.
    Educacion,
    /// Cobertura de botica, médico y hospital.
    Salud,
    /// Insalubridad: agua mala, pozos negros, basurales; pozos contaminados.
    Saneamiento,
    /// Mapa de calor del delito.
    Delito,
    // --- Recursos del suelo y zonas (ver `recursos`): sobre el terreno ---
    /// Tierra para cultivo (chacras): índice CONEAT, aptitud, pendiente, bañados.
    Cultivo,
    /// Pasturas (estancias).
    Pasturas,
    /// Piedra para canteras (basamento cristalino).
    Piedra,
    /// Arcilla para hornos de ladrillos.
    Arcilla,
    /// Arena (playas, médanos, arenales).
    Arena,
    /// Olor, ruido, humo y agua sucia de la industria; el arroyo sucio aguas abajo.
    Molestias,
    /// Valor del suelo de cada solar.
    Valor,
}

impl Layer {
    /// Capas que se dibujan sobre el terreno (además de los solares).
    pub fn es_de_terreno(self) -> bool {
        matches!(self, Layer::Cultivo | Layer::Pasturas | Layer::Piedra | Layer::Arcilla | Layer::Arena | Layer::Molestias)
    }
    /// El uso del suelo de una capa de recursos.
    pub fn uso(self) -> Option<super::recursos::Uso> {
        use super::recursos::Uso;
        Some(match self {
            Layer::Cultivo => Uso::Cultivo,
            Layer::Pasturas => Uso::Pastoreo,
            Layer::Piedra => Uso::Piedra,
            Layer::Arcilla => Uso::Arcilla,
            Layer::Arena => Uso::Arena,
            _ => return None,
        })
    }
}

/// Un edificio de servicio construido.
#[derive(Clone, Debug)]
pub struct Building {
    pub id: u32,
    pub kind: Kind,
    pub c: Vec2,
    /// Frente (hacia la calle).
    pub f: Vec2,
    /// Personas abastecidas de agua el último mes (escuelas: alumnos; salud: personas que
    /// atiende).
    pub served: i32,
    /// Pozos y aljibes: cuánto se ensució el agua con los pozos negros y la basura de
    /// alrededor (0 a 1; sube y baja de a poco, mes a mes).
    pub contaminacion: f32,
    /// Rinde del sitio (chacra, estancia, horno, cantera: según la tierra o el material del
    /// suelo, ver `recursos::rinde`; 1 = normal). Negativo = sin calcular todavía.
    pub rinde: f32,
    /// Vóxeles (para demolerlo); se regeneran al cargar.
    pub voxels: Vec<IVec3>,
}

impl Building {
    pub fn info(&self) -> &'static Info {
        info(self.kind)
    }
    pub fn corners(&self) -> [Vec2; 4] {
        let i = self.info();
        corners_of(self.c, self.f, i.size.0, i.size.1)
    }
}

/// Estado de todos los servicios de la ciudad.
#[derive(Clone, Debug)]
pub struct Services {
    pub buildings: BTreeMap<u32, Building>,
    pub next: u32,
    /// Calidad del agua de cada solar edificado (0 = sin agua) y quién lo abastece.
    pub water: BTreeMap<u32, (f32, u32)>,
    /// Faroles: posición (columna de diseño) y base, y sus vóxeles.
    pub lamps: Vec<(IVec2, i32)>,
    pub(crate) lamp_sig: (usize, i64),
    pub church: works::Church,
    pub walls: works::Walls,
    pub port: port::Port,
    pub fires: fires::Fires,
    pub epidemic: epidemics::Epidemic,
    /// Solares con los restos de una casa quemada (se ven hasta que se vuelve a edificar).
    pub ruins: BTreeSet<u32>,
    /// Escuelas, salud, saneamiento y delito de cada solar (ver [`barrio`]).
    pub barrio: barrio::Barrio,
    pub rng: Rng,
}

impl Default for Services {
    fn default() -> Self {
        Services {
            buildings: BTreeMap::new(),
            next: 0,
            water: BTreeMap::new(),
            lamps: Vec::new(),
            lamp_sig: (0, 0),
            church: works::Church::default(),
            walls: works::Walls::default(),
            port: port::Port::default(),
            fires: fires::Fires::default(),
            epidemic: epidemics::Epidemic::default(),
            ruins: BTreeSet::new(),
            barrio: barrio::Barrio::default(),
            rng: Rng::new(1730),
        }
    }
}

impl Services {
    pub fn of_kind(&self, k: Kind) -> impl Iterator<Item = &Building> {
        self.buildings.values().filter(move |b| b.kind == k)
    }

    pub fn count(&self, k: Kind) -> usize {
        self.of_kind(k).count()
    }

    pub fn at(&self, p: Vec2) -> Option<u32> {
        self.buildings.values().find(|b| point_in_poly(p, &b.corners())).map(|b| b.id)
    }

    /// Calidad del agua de un solar (0 = sin agua).
    pub fn water_of(&self, lot: u32) -> f32 {
        self.water.get(&lot).map_or(0.0, |w| w.0)
    }

    /// ¿Hay un puesto de serenos que vigila este punto?
    pub fn watched(&self, p: Vec2) -> bool {
        self.of_kind(Kind::Serenos).any(|b| b.c.distance(p) <= b.info().radius)
    }

    /// Fuente de agua más cercana que alcanza para apagar un fuego en `p`.
    pub fn fire_source(&self, p: Vec2) -> Option<u32> {
        self.buildings.values().filter(|b| b.info().fire_reach > 0.0 && b.c.distance(p) <= b.info().fire_reach).min_by(|a, b| a.c.distance(p).total_cmp(&b.c.distance(p))).map(|b| b.id)
    }

    /// Más pobladores con cada fase de la iglesia; la epidemia los espanta.
    pub fn attraction(&self) -> f32 {
        self.church.attraction() * if self.epidemic.active() { 0.5 } else { 1.0 }
    }

    /// Mantenimiento mensual de los servicios, el muelle y la muralla.
    pub fn upkeep(&self) -> i64 {
        self.buildings.values().map(|b| b.info().upkeep).sum::<i64>() + self.port.upkeep() + self.walls.upkeep()
    }
}

// --- Geometría ------------------------------------------------------------------------------

/// Índice del vóxel de diseño apoyado sobre la calzada en `p`.
pub(crate) fn road_design_base(t: &TerrainGen, p: Vec2) -> i32 {
    let top = (road_height(t, p) / BASE_VOXEL - 0.5).floor() as i32;
    (top + 2).div_euclid(2)
}

/// ¿Hay costa (agua del río) a menos de `r` m?
pub fn near_coast(t: &TerrainGen, c: Vec2, r: f32) -> bool {
    (0..16).any(|k| {
        let a = k as f32 / 16.0 * std::f32::consts::TAU;
        [0.33, 0.66, 1.0].iter().any(|d| {
            let q = c + Vec2::new(a.cos(), a.sin()) * r * d;
            t.height(q.x, q.y) < -0.5
        })
    })
}

/// Componente conexa de la red de calles de cada nodo (los aguadores van por la calle).
fn components(net: &RoadNetwork) -> BTreeMap<u32, u32> {
    let mut comp = BTreeMap::new();
    for &start in net.nodes.keys() {
        if comp.contains_key(&start) {
            continue;
        }
        let mut stack = vec![start];
        comp.insert(start, start);
        while let Some(n) = stack.pop() {
            for eid in net.node_edges.get(&n).into_iter().flatten() {
                let e = &net.edges[eid];
                for m in [e.a, e.b] {
                    if let std::collections::btree_map::Entry::Vacant(v) = comp.entry(m) {
                        v.insert(start);
                        stack.push(m);
                    }
                }
            }
        }
    }
    comp
}

/// Componente de la calle más cercana a `p` (a menos de `maxd` m).
fn comp_at(net: &RoadNetwork, comp: &BTreeMap<u32, u32>, p: Vec2, maxd: f32) -> Option<u32> {
    let n = net.nearest_edge(p, maxd)?;
    comp.get(&net.edges[&n.edge].a).copied()
}

/// Personas que consumen agua en un solar (casas: sus vecinos; comercios y talleres: como una
/// casa del mismo nivel).
fn need(l: &Lot) -> i32 {
    match l.kind() {
        Some(ZoneKind::Residencial) => l.residents(),
        _ => CAP[l.level as usize],
    }
}

/// Riesgo de contaminación de un pozo: talleres grandes cerca.
fn contaminated(lots: &Lots, p: Vec2) -> bool {
    lots.lots.values().any(|l| l.kind() == Some(ZoneKind::Industrial) && l.level >= 2 && l.c.distance(p) < CONTAMINATION)
}

/// Reparte el agua: primero las mejores fuentes, cada una hasta su capacidad (achicada por la
/// sequía), a los solares edificados más cercanos dentro de su alcance. Los aguadores solo
/// llegan a los solares de su misma red de calles. Fija además el tope de nivel de cada casa y
/// comercio: sin agua no se pasa del rancho; con agua mala, de una planta. Los talleres tienen
/// su propio pozo.
pub fn distribute_water(s: &mut Services, lots: &mut Lots, net: &RoadNetwork, factor: f32, private_l3: bool) {
    s.water.clear();
    let comp = components(net);
    let mut order: Vec<u32> = s.buildings.values().filter(|b| b.info().cap > 0).map(|b| b.id).collect();
    order.sort_by(|a, b| s.buildings[b].info().quality.total_cmp(&s.buildings[a].info().quality).then(a.cmp(b)));
    for sid in order {
        let b = &s.buildings[&sid];
        let inf = b.info();
        let src_comp = if b.kind == Kind::Aguadores { comp_at(net, &comp, b.c, 40.0) } else { None };
        let mut cands: Vec<(f32, u32)> = lots
            .lots
            .values()
            .filter(|l| l.level > 0 && !s.water.contains_key(&l.id))
            .filter_map(|l| {
                let d = l.c.distance(b.c);
                if d > inf.radius {
                    return None;
                }
                if b.kind == Kind::Aguadores {
                    let front = l.c + l.f * (super::lots::D * 0.5 + 3.0);
                    if src_comp.is_none() || comp_at(net, &comp, front, 12.0) != src_comp {
                        return None;
                    }
                }
                Some((d, l.id))
            })
            .collect();
        cands.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        // Los talleres grandes ensucian los pozos cercanos; los pozos negros y la basura de
        // alrededor, de a poco (ver `barrio`): el aljibe, que junta el agua de los techos, menos.
        let base = if b.kind == Kind::Pozo && contaminated(lots, b.c) { 0.35 } else { inf.quality };
        let quality = base * (1.0 - b.contaminacion * if b.kind == Kind::Aljibe { 0.3 } else if b.kind == Kind::Pozo { 0.65 } else { 0.0 });
        let cap = (inf.cap as f32 * factor) as i32;
        let mut used = 0;
        for (_, id) in cands {
            let n = need(&lots.lots[&id]);
            if used + n > cap {
                break;
            }
            used += n;
            s.water.insert(id, (quality, sid));
        }
        s.buildings.get_mut(&sid).unwrap().served = used;
    }
    if private_l3 {
        for l in lots.lots.values().filter(|l| l.level == 3) {
            s.water.insert(l.id, (1.0, 0));
        }
    }
    lots.level_cap.clear();
    for l in lots.lots.values() {
        // Los talleres se arreglan con su propio pozo (y ensucian los de los vecinos).
        let q = if l.kind() == Some(ZoneKind::Industrial) { 1.0 } else { s.water_of(l.id) };
        let cap = if q <= 0.0 { 1 } else if q < 0.6 { 2 } else { 3 };
        lots.level_cap.insert(l.id, cap);
    }
}

// --- La ciudad con sus servicios -------------------------------------------------------------

impl City {
    /// Sitio de un edificio de servicio cerca del cursor: se arrima a la calle más cercana
    /// (a menos de 60 m) con el frente hacia ella, como los solares. Devuelve (centro, frente).
    pub fn service_site(&self, k: Kind, cursor: Vec2) -> (Vec2, Vec2) {
        let inf = info(k);
        match self.net.nearest_edge(cursor, 60.0) {
            Some(n) if n.dist > 0.5 => {
                let e = &self.net.edges[&n.edge];
                let f = (n.point - cursor).normalize_or(Vec2::NEG_Y);
                let c = n.point - f * (e.kind.width() * 0.5 + 0.6 + inf.size.1 * 0.5);
                (c, f)
            }
            _ => (cursor, Vec2::NEG_Y),
        }
    }

    /// `None` si el servicio se puede construir ahí; si no, el motivo.
    pub fn service_problem(&self, t: &TerrainGen, k: Kind, c: Vec2, f: Vec2) -> Option<String> {
        let inf = info(k);
        if self.plaza.is_none() {
            return Some("Primero funda la plaza mayor".into());
        }
        let cs = corners_of(c, f, inf.size.0, inf.size.1);
        let (mut hmin, mut hmax) = (f32::INFINITY, f32::NEG_INFINITY);
        for p in cs.iter().chain(std::iter::once(&c)) {
            let h = t.height(p.x, p.y);
            if h < 0.8 || t.is_water(p.x, p.y) {
                return Some("Demasiado cerca del agua".into());
            }
            hmin = hmin.min(h);
            hmax = hmax.max(h);
        }
        if hmax - hmin > 8.0 {
            return Some("El terreno es demasiado irregular".into());
        }
        let inner = corners_of(c, f, inf.size.0 - 0.6, inf.size.1 - 0.6);
        if inner.iter().chain(std::iter::once(&c)).any(|&p| self.net.clearance(p, 30.0) < 0.3) {
            return Some("Está encima de una calle".into());
        }
        if self.net.clearance(c, 120.0) > inf.size.0.max(inf.size.1) * 0.5 + 12.0 {
            return Some("Debe estar junto a una calle o camino".into());
        }
        if self.lots.obstacles.iter().chain(&self.lots.walls).any(|o| polys_overlap(&inner, &corners_of(o.c, o.f, o.w, o.d))) {
            return Some("El lugar está ocupado".into());
        }
        let plaza = self.lots.plaza_center.unwrap_or(c);
        let far = |m: f32| c.distance(plaza) < m;
        match k {
            Kind::Estancia if far(600.0) => return Some("Las estancias están en el campo, a más de 600 m de la plaza".into()),
            Kind::Chacra if far(300.0) => return Some("Las chacras van en el ejido, a más de 300 m de la plaza".into()),
            Kind::Lazareto if far(400.0) => return Some("El lazareto va lejos de la ciudad (a más de 400 m de la plaza)".into()),
            Kind::Lazareto if !near_coast(t, c, 150.0) => return Some("El lazareto va junto a la costa (a menos de 150 m)".into()),
            Kind::Cantera if far(300.0) => return Some("La cantera es ruidosa: a más de 300 m de la plaza".into()),
            Kind::Horno if far(250.0) => return Some("El horno echa humo: a más de 250 m de la plaza".into()),
            Kind::Horno if t.arroyo_distance(c.x, c.y) > 300.0 && !near_coast(t, c, 300.0) => return Some("El horno necesita barro: un arroyo o la costa a menos de 300 m".into()),
            Kind::Horno | Kind::Cantera => {
                let r = super::recursos::Recursos::de(t).en_area(c, f, inf.size.0, inf.size.1);
                if k == Kind::Horno && r.arcilla < super::recursos::MINIMO_MATERIAL {
                    return Some("Acá no hay arcilla para ladrillos: busca limos o los bajos junto a un arroyo (capa «Arcilla»)".into());
                }
                if k == Kind::Cantera && r.piedra < super::recursos::MINIMO_MATERIAL {
                    return Some("Acá no hay piedra: la cantera va sobre el basamento (capa «Piedra»)".into());
                }
            }
            Kind::Aguadores if t.arroyo_distance(c.x, c.y) > 400.0 && !near_coast(t, c, 250.0) => return Some("Los aguadores necesitan un arroyo a menos de 400 m o la costa a 250 m".into()),
            Kind::Aduana if !self.services.port.pier.as_ref().is_some_and(|p| p.land.distance(c) < 500.0) => return Some("La aduana va junto al muelle (a menos de 500 m): primero el muelle".into()),
            Kind::Aduana if self.services.count(Kind::Aduana) > 0 => return Some("La ciudad ya tiene aduana".into()),
            Kind::Convento if self.services.count(Kind::Convento) > 0 => return Some("La ciudad ya tiene convento".into()),
            Kind::Carcel if self.services.count(Kind::Carcel) > 0 => return Some("La ciudad ya tiene cárcel".into()),
            Kind::Carcel if !far(400.0) => return Some("La cárcel del Cabildo va cerca de la plaza (a menos de 400 m)".into()),
            Kind::Basurero if far(450.0) => return Some("El basurero va fuera de la ciudad, a más de 450 m de la plaza".into()),
            _ => {}
        }
        // Instituciones e industrias del desarrollo: tecnología y sitio (ver `city::desarrollo`).
        if let Some(p) = self.desarrollo_problema_sitio(t, k, c) {
            return Some(p);
        }
        if inf.cost > self.money {
            return Some(format!("Fondos insuficientes: hacen falta {} pesos", super::game::fmt_thousands(inf.cost)));
        }
        // Los materiales no frenan la obra: si faltan, se levanta con lo que hay (ver
        // `materiales`).
        None
    }

    /// Construye un servicio: cobra, despeja los solares que pisa y lo levanta en vóxeles.
    pub fn place_service(&mut self, t: &TerrainGen, k: Kind, c: Vec2, f: Vec2) -> Result<Edits, String> {
        if let Some(p) = self.service_problem(t, k, c, f) {
            return Err(p);
        }
        let inf = info(k);
        self.money -= inf.cost;
        // Con lo que hay: si faltan ladrillos, de adobe o de tablas (ver `materiales`).
        let obra = (!super::materiales::ciudad::servicio_sin_obra(k)).then(|| self.construir_obra(&super::materiales::ciudad::plan_servicio(k)));
        let mut e = Edits::default();
        self.clear_lots_under(&corners_of(c, f, inf.size.0, inf.size.1), &mut e);
        self.lots.obstacles.push(Obstacle { c, f, w: inf.size.0, d: inf.size.1 });
        self.services.next += 1;
        let id = self.services.next;
        let seed = 0x5E41_u64 ^ (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let b = build_servicio(t, c, f, inf, seed, obra);
        e.extend(b.edits);
        if let Some(o) = obra {
            self.materiales.servicios.insert(id, o);
        }
        let rinde = super::recursos::rinde(k, &super::recursos::Recursos::de(t).en_area(c, f, inf.size.0, inf.size.1));
        self.services.buildings.insert(id, Building { id, kind: k, c, f, served: 0, contaminacion: 0.0, rinde, voxels: b.voxels });
        self.refresh_water();
        if k == Kind::Serenos {
            self.refresh_lamps(t, &mut e);
        }
        self.refresh_barrio();
        self.refresh_suelo(t);
        Ok(e)
    }

    /// Derriba un edificio de servicio (sin devolver lo gastado).
    pub fn demolish_service(&mut self, t: &TerrainGen, id: u32) -> Result<Edits, String> {
        let b = self.services.buildings.remove(&id).ok_or("No hay ningún servicio ahí")?;
        self.materiales.servicios.remove(&id);
        let mut e = Edits::default();
        for &v in &b.voxels {
            e.dset(v, AIR);
        }
        self.lots.obstacles.retain(|o| o.c.distance(b.c) > 0.01);
        self.refresh_water();
        if b.kind == Kind::Serenos {
            self.refresh_lamps(t, &mut e);
        }
        self.refresh_barrio();
        self.refresh_suelo(t);
        self.money -= 20;
        Ok(e)
    }

    /// Quita los solares que pisa un polígono (y sus edificios).
    pub(crate) fn clear_lots_under(&mut self, poly: &[Vec2; 4], e: &mut Edits) {
        let ids: Vec<u32> = self.lots.lots.values().filter(|l| polys_overlap(poly, &self.lots.corners(l, 0.6))).map(|l| l.id).collect();
        for id in ids {
            let l = self.lots.remove(id);
            for &v in &l.voxels {
                e.dset(v, AIR);
            }
            self.services.ruins.remove(&id);
            self.services.barrio.basurales.remove(&id);
        }
    }

    /// Vuelve a repartir el agua (al construir o demoler, y cada mes).
    pub fn refresh_water(&mut self) {
        let factor = self.economy.water_factor();
        let private = self.economy.is_on(Ordinance::AljibeObligatorio);
        distribute_water(&mut self.services, &mut self.lots, &self.net, factor, private);
    }

    /// Faroles a los lados de las calles que vigila cada puesto de serenos (uno cada 30 m,
    /// alternando de vereda). Quita los que ya no van y pone los nuevos.
    pub fn refresh_lamps(&mut self, t: &TerrainGen, e: &mut Edits) {
        let mut out: Vec<(IVec2, i32)> = Vec::new();
        let posts: Vec<(Vec2, f32)> = self.services.of_kind(Kind::Serenos).map(|b| (b.c, b.info().radius)).collect();
        let mut seen = HashSet::new();
        for ed in self.net.edges.values() {
            let hw = ed.kind.width() * 0.5;
            let mut s = 6.0;
            let mut side = 1.0;
            while s < ed.len() - 4.0 {
                let (p, tan) = ed.point(s);
                if posts.iter().any(|(c, r)| p.distance(*c) <= *r) {
                    let q = p + Vec2::new(-tan.y, tan.x) * side * (hw - 0.5);
                    let v = IVec2::new((q.x / DESIGN_VOXEL).floor() as i32, (q.y / DESIGN_VOXEL).floor() as i32);
                    // Ni dentro de otra calzada (cruces) ni duplicado.
                    if self.net.clearance(q, 20.0) > -1.0 && seen.insert(v / 8) {
                        out.push((v, road_design_base(t, q)));
                    }
                }
                s += LAMP_SPACING;
                side = -side;
            }
        }
        let old: HashSet<(IVec2, i32)> = self.services.lamps.iter().copied().collect();
        let new: HashSet<(IVec2, i32)> = out.iter().copied().collect();
        for &(v, b) in old.difference(&new) {
            for y in b..b + 8 {
                e.dset(IVec3::new(v.x, y, v.y), AIR);
            }
        }
        for &(v, b) in &out {
            colonial::farol(e, v, b);
        }
        self.services.lamps = out;
        self.services.lamp_sig = (self.net.edges.len(), self.net.total_length() as i64);
    }

    /// Un paso de medio segundo de juego: fuego y contagios.
    pub fn services_step(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        if self.lots.level_cap.is_empty() && !self.lots.lots.is_empty() {
            self.refresh_water();
        }
        let mut news = self.fire_step(t, e);
        news.extend(self.epidemic_step());
        // Los solares que se volvieron a edificar ya no muestran ruinas.
        let lots = &self.lots.lots;
        self.services.ruins.retain(|id| lots.get(id).is_some_and(|l| l.level == 0));
        self.services.barrio.basurales.retain(|id| lots.get(id).is_some_and(|l| l.level == 0));
        news
    }

    /// El mes de los servicios (antes de cerrar la economía): reparto del agua, obras, barcos,
    /// epidemias y faroles.
    pub fn services_month(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        // Saneamiento (pozos que se ensucian, basurales), el agua, y con ella escuelas, salud,
        // insalubridad y delito de cada solar; después, los brotes y los delitos del mes.
        news.extend(self.saneamiento_month(t, e));
        self.refresh_water();
        self.refresh_barrio();
        // Recursos, molestias y valor del suelo de cada solar (ver `recursos`).
        self.refresh_suelo(t);
        news.extend(self.salubridad_month());
        news.extend(self.delito_month());
        news.extend(self.works_month(t, e));
        news.extend(self.port_month());
        news.extend(self.epidemic_month(t, e));
        if self.services.lamp_sig != (self.net.edges.len(), self.net.total_length() as i64) && self.services.count(Kind::Serenos) > 0 {
            self.refresh_lamps(t, e);
        }
        news
    }

    /// Lo que los servicios le cuentan a la economía este mes.
    pub fn services_input(&self, inp: &mut MonthInput) {
        let s = &self.services;
        // Cada edificio rinde según su sitio (la tierra o el material del suelo; ver `recursos`).
        let n = |k: Kind| s.of_kind(k).map(|b| if b.rinde < 0.0 { 1.0 } else { b.rinde }).sum::<f32>();
        inp.production[Res::Pan as usize] += self.climate.grain_this_month(self.month, CHACRA_GRAIN_YEAR * n(Kind::Chacra));
        let herd = self.climate.livestock_factor();
        inp.production[Res::Carne as usize] += ESTANCIA_CARNE * herd * n(Kind::Estancia);
        inp.production[Res::Cueros as usize] += ESTANCIA_CUEROS * herd * n(Kind::Estancia);
        inp.production[Res::Sebo as usize] += ESTANCIA_SEBO * herd * n(Kind::Estancia);
        inp.production[Res::Ladrillos as usize] += HORNO_LADRILLOS * self.climate.brick_factor() * n(Kind::Horno);
        inp.production[Res::Piedra as usize] += CANTERA_PIEDRA * n(Kind::Cantera);
        inp.upkeep_services += s.upkeep();
        inp.lamps += s.lamps.len() as i32;
        let pop = self.population();
        if pop > 0 {
            let served: i32 = self.lots.lots.values().filter(|l| s.water_of(l.id) > 0.0).map(|l| l.residents()).sum();
            inp.water_ratio = served as f32 / pop as f32;
        }
        inp.epidemic = s.epidemic.active();
        self.port_input(inp);
        self.barrio_input(inp);
        let gates = self.services.walls.gate_duty();
        if gates > 0 {
            inp.other.push(("Derechos de los portones".into(), gates));
        }
    }

    /// Vóxeles de los servicios, el muelle, la muralla, los faroles y las ruinas (al cargar).
    pub fn services_rebuild(&mut self, t: &TerrainGen, e: &mut Edits) {
        let ids: Vec<u32> = self.services.buildings.keys().copied().collect();
        for id in ids {
            let b = &self.services.buildings[&id];
            let inf = b.info();
            let seed = 0x5E41_u64 ^ (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let built = build_servicio(t, b.c, b.f, inf, seed, self.materiales.servicios.get(&id).copied());
            e.extend(built.edits);
            self.services.buildings.get_mut(&id).unwrap().voxels = built.voxels;
        }
        let ruins: Vec<u32> = self.services.ruins.iter().copied().collect();
        for id in ruins {
            self.ruin_edits(t, id, e);
        }
        let basurales: Vec<u32> = self.services.barrio.basurales.iter().copied().collect();
        for id in basurales {
            self.basural_edits(t, id, e);
        }
        // Los faroles tal como se guardaron (no recalculados): si las calles cambiaron después
        // del último reparto, el mes siguiente los pone al día igual que en la partida original.
        if self.services.lamps.is_empty() && self.services.count(Kind::Serenos) > 0 {
            // Partidas viejas, sin los faroles guardados.
            self.refresh_lamps(t, e);
        }
        for &(v, b) in &self.services.lamps {
            colonial::farol(e, v, b);
        }
        self.port_rebuild(t, e);
        self.walls_rebuild(t, e);
        // Escuelas, salud y delito de cada solar (derivados: no se guardan); recursos,
        // molestias y valor del suelo (el valor se guarda: queda el de la partida).
        self.refresh_barrio();
        let valor = std::mem::take(&mut self.lots.valor);
        self.refresh_suelo(t);
        if !valor.is_empty() {
            self.lots.valor = valor;
        }
    }

    /// Lleva un solar a ruinas (después de un incendio).
    pub(crate) fn ruin_edits(&mut self, t: &TerrainGen, id: u32, e: &mut Edits) {
        let Some(l) = self.lots.lots.get(&id) else { return };
        let b = build_rotated(t, l.c, l.f, colonial::LOT_W, colonial::LOT_D, l.seed ^ 0xF1E, &colonial::ruina);
        let new: HashSet<IVec3> = b.voxels.iter().copied().collect();
        for &v in &l.voxels {
            if !new.contains(&v) {
                e.dset(v, AIR);
            }
        }
        e.extend(b.edits);
        self.lots.lots.get_mut(&id).unwrap().voxels = b.voxels;
    }

    /// Ficha de un edificio de servicio para el panel de información.
    pub fn service_lines(&self, id: u32) -> Vec<String> {
        let b = &self.services.buildings[&id];
        let inf = b.info();
        let mut out = vec![inf.desc.to_string()];
        match b.kind {
            Kind::Pozo | Kind::Aljibe | Kind::Aguadores => {
                let cap = (inf.cap as f32 * self.economy.water_factor()) as i32;
                out.push(format!("Abastece: {} de {} personas", b.served, cap));
                out.push(format!("Alcance: {} m (para el fuego, {} m)", inf.radius as i32, inf.fire_reach as i32));
                if b.kind == Kind::Pozo && contaminated(&self.lots, b.c) {
                    out.push("Agua: contaminada por los talleres".into());
                }
                if b.kind != Kind::Aguadores {
                    out.push(format!("Pozos negros y basura alrededor: agua {} % sucia{}", (b.contaminacion * 100.0).round() as i32, if b.contaminacion >= 0.5 { " (huele mal)" } else { "" }));
                }
                if self.economy.in_drought() {
                    out.push("Sequía: rinde menos".into());
                }
            }
            Kind::Serenos => {
                out.push(format!("Faroles encendidos: {}", self.services.lamps.len()));
                out.push(format!("Alcance: {} m", inf.radius as i32));
            }
            Kind::Lazareto => out.push(format!("En cuarentena: {} personas", self.services.epidemic.quarantined_now())),
            Kind::Chacra => out.push(format!("Trigo y maíz: {} raciones al año (año normal)", (CHACRA_GRAIN_YEAR * b.rinde.max(0.0)) as i32)),
            Kind::Estancia => {
                let k = self.climate.livestock_factor() * b.rinde.max(0.0);
                out.push(format!("Por mes: {} de carne, {} cueros, {} de sebo", (ESTANCIA_CARNE * k) as i32, (ESTANCIA_CUEROS * k) as i32, (ESTANCIA_SEBO * k) as i32))
            }
            Kind::Horno => out.push(format!("Ladrillos por mes: {}", (HORNO_LADRILLOS * self.climate.brick_factor() * b.rinde.max(0.0)) as i32)),
            Kind::Cantera => out.push(format!("Piedra por mes: {}", (CANTERA_PIEDRA * b.rinde.max(0.0)) as i32)),
            Kind::Aduana => out.push("Contrabando: baja de 35 % a 5 %".into()),
            k if super::desarrollo::es_de_desarrollo(k) => self.desarrollo_lines(id, &mut out),
            _ => self.barrio_lines(id, &mut out),
        }
        if matches!(b.kind, Kind::Chacra | Kind::Estancia | Kind::Horno | Kind::Cantera) && b.rinde >= 0.0 {
            let que = match b.kind {
                Kind::Chacra => "tierra para cultivo",
                Kind::Estancia => "pasturas",
                Kind::Horno => "arcilla",
                _ => "piedra",
            };
            out.push(format!("Sitio: rinde {} % de lo normal ({})", (b.rinde * 100.0).round() as i32, que));
        }
        if let Some(p) = super::recursos::molestias::perfil_de_servicio(b.kind) {
            let m = [(p.olor, "olor"), (p.ruido, "ruido"), (p.humo, "humo")].iter().filter(|x| x.0 >= 0.2).map(|x| x.1).collect::<Vec<_>>().join(", ");
            out.push(format!("Molesta: {} hasta {} m{}", if m.is_empty() { "poco".into() } else { m }, p.radio as i32, if p.vertido >= 0.2 { "; ensucia el arroyo aguas abajo" } else { "" }));
        }
        if inf.upkeep > 0 {
            out.push(format!("Mantenimiento: {} pesos al mes", inf.upkeep));
        }
        if let Some(o) = self.materiales.servicios.get(&id) {
            out.extend(self.lineas_obra(o));
        }
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::roads::{line, RoadKind};
    use super::super::{grid_lines, plaza_frame, BLOCK};
    use super::*;

    /// Ciudad de prueba: plaza, damero de 3 × 3 cuadras empedradas y casas de densidad media.
    pub fn town(t: &TerrainGen) -> City {
        use super::super::lots::{zone_code, Density};
        let mut city = City::default();
        city.money = 1_000_000;
        let c = Vec2::new(13500.0, 16300.0);
        city.found_plaza(t, c, 0.0).unwrap();
        let (f, s) = plaza_frame(0.0);
        let a = c - s * (BLOCK * 1.5) - f * (BLOCK * 1.5);
        city.build_grid(t, &grid_lines(a, a + s * BLOCK * 3.0, a + f * BLOCK * 3.0), RoadKind::Stone).unwrap();
        city.paint(c, 300.0, zone_code(ZoneKind::Residencial, Density::Media));
        city.economy.stocks[Res::Ladrillos as usize].amount = 2000.0;
        city.economy.stocks[Res::Piedra as usize].amount = 2000.0;
        city
    }

    /// Un sitio libre junto a una calle de la ciudad para un servicio.
    pub fn free_site(city: &City, t: &TerrainGen, k: Kind, near: Vec2) -> (Vec2, Vec2) {
        for r in 0..40 {
            for a in 0..16 {
                let p = near + Vec2::from_angle(a as f32 / 16.0 * std::f32::consts::TAU) * (r as f32 * 12.0);
                let (c, f) = city.service_site(k, p);
                if city.service_problem(t, k, c, f).is_none() {
                    return (c, f);
                }
            }
        }
        panic!("no hay sitio para {:?}", k);
    }

    #[test]
    fn el_agua_cubre_dentro_del_alcance_y_hasta_la_capacidad() {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        city.fast_forward(&t, 12);
        assert!(city.lots.lots.values().filter(|l| l.level > 0).count() > 20);
        // Sin agua pública, nadie pasa del rancho.
        assert!(city.lots.lots.values().all(|l| l.level <= 1), "sin agua no hay casas de azotea");
        let c = city.plaza.unwrap().0;
        let (pc, pf) = free_site(&city, &t, Kind::Pozo, c + Vec2::new(60.0, 60.0));
        city.place_service(&t, Kind::Pozo, pc, pf).unwrap();
        let pozo = city.services.buildings.values().next().unwrap().clone();
        assert!(pozo.voxels.len() > 30, "el pozo tiene vóxeles");
        assert!(pozo.served > 0 && pozo.served <= 60, "abastece {}", pozo.served);
        for (&lot, &(q, src)) in &city.services.water {
            assert_eq!(src, pozo.id);
            assert!(city.lots.lots[&lot].c.distance(pozo.c) <= 110.0, "dentro del alcance");
            assert!((q - 0.7).abs() < 1e-4);
        }
        // Un aljibe cubre más y con mejor agua; la sequía achica la capacidad.
        let (ac, af) = free_site(&city, &t, Kind::Aljibe, c - Vec2::new(80.0, 60.0));
        city.place_service(&t, Kind::Aljibe, ac, af).unwrap();
        let covered = city.services.water.len();
        assert!(covered > 0);
        city.economy.drought_months = 3;
        city.refresh_water();
        let dry: i32 = city.services.buildings.values().map(|b| b.served).sum();
        city.economy.drought_months = 0;
        city.refresh_water();
        let wet: i32 = city.services.buildings.values().map(|b| b.served).sum();
        assert!(dry <= wet, "la sequía no da más agua ({dry} vs {wet})");
        assert!(city.services.buildings.values().all(|b| b.served <= (b.info().cap as f32) as i32));
        // Con agua, las casas crecen.
        city.fast_forward(&t, 24);
        let watered: Vec<&Lot> = city.lots.lots.values().filter(|l| city.services.water_of(l.id) > 0.0).collect();
        assert!(watered.iter().any(|l| l.level >= 2), "con agua aparecen casas de azotea");
    }

    #[test]
    fn los_aguadores_solo_llegan_por_la_calle() {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        city.fast_forward(&t, 8);
        // Un barrio suelto, sin calle que lo una con la ciudad.
        let c = city.plaza.unwrap().0;
        let mut far = Vec2::ZERO;
        for k in 0..24 {
            let p = c + Vec2::from_angle(k as f32 / 24.0 * std::f32::consts::TAU) * 520.0;
            if city.build_road(&t, &line(p, p + Vec2::new(150.0, 0.0)), RoadKind::Dirt).is_ok() {
                far = p;
                break;
            }
        }
        assert!(far != Vec2::ZERO, "un barrio aparte");
        city.paint(far + Vec2::new(75.0, 0.0), 90.0, super::super::lots::zone_code(ZoneKind::Residencial, super::super::lots::Density::Media));
        city.fast_forward(&t, 8);
        let isolated: Vec<u32> = city.lots.lots.values().filter(|l| l.level > 0 && l.c.distance(far) < 200.0).map(|l| l.id).collect();
        assert!(!isolated.is_empty());
        // Puesto de aguadores en la ciudad (en el mapa sintético hay arroyos o costa cerca).
        let k = Kind::Aguadores;
        let mut site = None;
        'b: for r in 0..60 {
            for a in 0..16 {
                let p = c + Vec2::from_angle(a as f32 / 16.0 * std::f32::consts::TAU) * (r as f32 * 10.0);
                let (sc, sf) = city.service_site(k, p);
                if city.service_problem(&t, k, sc, sf).is_none() && sc.distance(c) < 200.0 {
                    site = Some((sc, sf));
                    break 'b;
                }
            }
        }
        let Some((sc, sf)) = site else {
            // Sin arroyo cerca no hay aguadores: el motivo lo dice.
            let (sc, sf) = city.service_site(k, c + Vec2::new(40.0, 40.0));
            assert!(city.service_problem(&t, k, sc, sf).is_some_and(|m| m.contains("arroyo") || m.contains("ocupado") || m.contains("calle")));
            return;
        };
        city.place_service(&t, k, sc, sf).unwrap();
        assert!(isolated.iter().all(|id| city.services.water_of(*id) == 0.0), "el barrio sin calle no recibe aguadores");
        assert!(!city.services.water.is_empty());
    }

    #[test]
    fn el_campo_produce_y_el_clima_lo_cambia() {
        let t = TerrainGen::new(1726);
        let mut city = town(&t);
        let base = city.month_input();
        let c = city.plaza.unwrap().0;
        // Un camino al campo y una estancia al final.
        let end = c + Vec2::new(0.0, -900.0);
        city.build_road(&t, &line(c + Vec2::new(BLOCK * 1.5, -BLOCK * 1.5), end), RoadKind::Dirt).ok();
        let (ec, ef) = free_site(&city, &t, Kind::Estancia, end);
        city.place_service(&t, Kind::Estancia, ec, ef).unwrap();
        let with = city.month_input();
        assert!(with.production[Res::Carne as usize] > base.production[Res::Carne as usize] + 300.0);
        assert!(with.production[Res::Cueros as usize] > base.production[Res::Cueros as usize]);
        // Con las pasturas secas, el ganado da menos.
        city.climate.pasture = 0.4;
        let dry = city.month_input();
        assert!(dry.production[Res::Carne as usize] < with.production[Res::Carne as usize]);
        // Una chacra suma trigo en la cosecha (diciembre y enero).
        let (cc, cf) = free_site(&city, &t, Kind::Chacra, end + Vec2::new(0.0, 200.0));
        city.place_service(&t, Kind::Chacra, cc, cf).unwrap();
        let annual = CHACRA_GRAIN_YEAR;
        let year: f32 = (0..12).map(|m| city.climate.grain_this_month(m, annual)).sum();
        assert!(year > annual * 0.5, "una chacra da {year} raciones al año");
        // Una estancia junto a la plaza no se puede.
        let (pc, pf) = city.service_site(Kind::Estancia, c + Vec2::new(50.0, 0.0));
        assert!(city.service_problem(&t, Kind::Estancia, pc, pf).is_some());
    }
}
