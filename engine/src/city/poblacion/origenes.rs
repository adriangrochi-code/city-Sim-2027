//! De dónde llegan las familias en cada época: tablas de datos (extensibles) con el peso de
//! cada origen, el tamaño de las familias, su riqueza al llegar, su alfabetización y los
//! oficios que traen.
//!
//! Cifras: los pesos relativos son ESTIMACIONES de diseño guiadas por los datos verificados
//! (Pollero y González Demuro, *Población y sociedad de Montevideo*, 2024, ver la tabla de
//! calibración en `docs/sistemas/poblacion.md`): en 1836, la mitad de los extranjeros eran
//! europeos (63 % de ellos canarios, 13 % portugueses); en 1860 los europeos eran un tercio
//! españoles, un tercio italianos y un 27 % franceses. Las riquezas y los oficios por origen
//! son estimaciones de orden de magnitud.
//!
//! El juego no representa la esclavitud (decisión de diseño): todas las personas llegan
//! libres, con su familia, sus bienes y su oficio. Las personas de origen africano llegan como
//! las demás; su peso en la tabla colonial es una estimación de diseño, no una cifra histórica.

use super::modelo::{Oficio, Origen};
use serde::{Deserialize, Serialize};

/// Las seis épocas del juego (ver `docs/epocas`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Epoca {
    #[default]
    Colonial,
    CiudadNueva,
    Inmigracion,
    ArtDeco,
    Modernismo,
    Contemporanea,
}

impl Epoca {
    pub const ALL: [Epoca; 6] = [Epoca::Colonial, Epoca::CiudadNueva, Epoca::Inmigracion, Epoca::ArtDeco, Epoca::Modernismo, Epoca::Contemporanea];

    /// Época que corresponde a un año en el modo "fiel a la historia".
    pub fn del_año(año: i32) -> Epoca {
        match año {
            ..=1829 => Epoca::Colonial,
            1830..=1869 => Epoca::CiudadNueva,
            1870..=1929 => Epoca::Inmigracion,
            1930..=1954 => Epoca::ArtDeco,
            1955..=1989 => Epoca::Modernismo,
            _ => Epoca::Contemporanea,
        }
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Epoca::Colonial => "Plaza fuerte colonial",
            Epoca::CiudadNueva => "Ciudad Nueva",
            Epoca::Inmigracion => "Inmigración y Belle Époque",
            Epoca::ArtDeco => "Art Déco y Rambla",
            Epoca::Modernismo => "Modernismo",
            Epoca::Contemporanea => "Contemporánea",
        }
    }
}

/// Cómo llega la gente de un origen en una época.
#[derive(Clone, Copy, Debug)]
pub struct Llegada {
    pub origen: Origen,
    /// Peso relativo en la época (se normaliza).
    pub peso: f32,
    /// Tamaño medio de la familia que llega (1 = llegan solos).
    pub familia: f32,
    /// Riqueza mediana del hogar al llegar (pesos de la época) y dispersión (log-normal).
    pub riqueza: f32,
    pub dispersion: f32,
    /// Probabilidad de que un adulto sepa leer.
    pub lectura: f32,
    /// Oficios de los adultos (peso relativo).
    pub oficios: &'static [(Oficio, f32)],
}

const CAMPO: &[(Oficio, f32)] = &[(Oficio::Labrador, 4.0), (Oficio::Peon, 4.0), (Oficio::Artesano, 1.0), (Oficio::Pulpero, 0.5)];
const CANARIOS: &[(Oficio, f32)] = &[(Oficio::Labrador, 6.0), (Oficio::Peon, 2.0), (Oficio::Artesano, 1.0), (Oficio::Pulpero, 0.5)];
const PORTENOS: &[(Oficio, f32)] = &[(Oficio::Labrador, 2.0), (Oficio::Hacendado, 1.0), (Oficio::Artesano, 2.0), (Oficio::Pulpero, 1.0), (Oficio::Comerciante, 0.7), (Oficio::Peon, 2.0)];
const NORTE_ES: &[(Oficio, f32)] = &[(Oficio::Comerciante, 2.0), (Oficio::Pulpero, 3.0), (Oficio::Artesano, 2.0), (Oficio::Marinero, 1.0), (Oficio::Peon, 1.0), (Oficio::Funcionario, 0.3)];
const MERCADERES: &[(Oficio, f32)] = &[(Oficio::Comerciante, 4.0), (Oficio::Pulpero, 2.0), (Oficio::Artesano, 1.0), (Oficio::Marinero, 1.0)];
const MARINOS: &[(Oficio, f32)] = &[(Oficio::Marinero, 3.0), (Oficio::Peon, 2.0), (Oficio::Artesano, 1.0), (Oficio::Pulpero, 1.0)];
const MISIONES: &[(Oficio, f32)] = &[(Oficio::Peon, 5.0), (Oficio::Artesano, 2.0), (Oficio::Labrador, 1.0)];
const REGION: &[(Oficio, f32)] = &[(Oficio::Peon, 5.0), (Oficio::Servicio, 1.0)];
/// Oficios urbanos y del puerto (artesanos, peones, servicio, marineros, pulperos).
const URBANOS: &[(Oficio, f32)] = &[(Oficio::Artesano, 3.0), (Oficio::Peon, 3.0), (Oficio::Servicio, 2.0), (Oficio::Marinero, 1.5), (Oficio::Pulpero, 0.5)];
const OBREROS: &[(Oficio, f32)] = &[(Oficio::Obrero, 4.0), (Oficio::Artesano, 2.0), (Oficio::Peon, 2.0), (Oficio::Pulpero, 1.0), (Oficio::Comerciante, 0.5), (Oficio::Servicio, 1.0)];
const TENDEROS: &[(Oficio, f32)] = &[(Oficio::Comerciante, 3.0), (Oficio::Pulpero, 2.0), (Oficio::Artesano, 1.0), (Oficio::Obrero, 1.0)];
const MODERNOS: &[(Oficio, f32)] = &[(Oficio::Empleado, 4.0), (Oficio::Obrero, 3.0), (Oficio::Servicio, 2.0), (Oficio::Comerciante, 1.0), (Oficio::Profesional, 1.0)];
const PROFESIONALES: &[(Oficio, f32)] = &[(Oficio::Profesional, 3.0), (Oficio::Empleado, 3.0), (Oficio::Comerciante, 1.0), (Oficio::Obrero, 1.0)];

macro_rules! ll {
    ($o:ident, $p:expr, $f:expr, $r:expr, $d:expr, $l:expr, $of:expr) => {
        Llegada { origen: Origen::$o, peso: $p, familia: $f, riqueza: $r, dispersion: $d, lectura: $l, oficios: $of }
    };
}

/// Época colonial (1726–1829). Pesos: estimación (ver documentación).
pub const COLONIAL: &[Llegada] = &[
    ll!(Canarias, 16.0, 4.6, 40.0, 0.7, 0.15, CANARIOS),
    ll!(BuenosAires, 14.0, 4.5, 140.0, 1.0, 0.25, PORTENOS),
    ll!(Galicia, 7.0, 1.6, 60.0, 0.9, 0.3, NORTE_ES),
    ll!(Andalucia, 5.0, 2.0, 80.0, 0.9, 0.25, MARINOS),
    ll!(Cantabrico, 3.0, 1.4, 120.0, 1.0, 0.45, NORTE_ES),
    ll!(Cataluna, 2.0, 1.6, 200.0, 1.0, 0.5, MERCADERES),
    ll!(PaisVasco, 2.0, 1.5, 250.0, 1.0, 0.5, MERCADERES),
    ll!(Portugal, 3.0, 2.0, 80.0, 0.9, 0.25, MARINOS),
    ll!(Brasil, 4.0, 2.5, 60.0, 1.0, 0.15, CAMPO),
    ll!(Misiones, 6.0, 1.8, 5.0, 0.5, 0.1, MISIONES),
    ll!(PueblosRegion, 1.0, 2.0, 2.0, 0.5, 0.0, REGION),
    ll!(Interior, 5.0, 3.5, 30.0, 0.8, 0.05, CAMPO),
    ll!(Britania, 0.5, 1.2, 400.0, 1.0, 0.8, MERCADERES),
    ll!(Francia, 0.5, 1.3, 200.0, 1.0, 0.6, MERCADERES),
    ll!(AfricaCentral, 4.0, 1.8, 15.0, 0.6, 0.05, URBANOS),
    ll!(AfricaOccidental, 2.0, 1.8, 15.0, 0.6, 0.05, URBANOS),
    ll!(AfricaOriental, 1.0, 1.6, 15.0, 0.6, 0.05, URBANOS),
];

/// Ciudad Nueva (1830–1869): canarios, vascos, gallegos, franceses e italianos; argentinos.
pub const CIUDAD_NUEVA: &[Llegada] = &[
    ll!(Canarias, 14.0, 4.5, 50.0, 0.8, 0.2, CANARIOS),
    ll!(Galicia, 8.0, 1.8, 60.0, 0.9, 0.35, NORTE_ES),
    ll!(PaisVasco, 7.0, 2.0, 150.0, 1.0, 0.5, TENDEROS),
    ll!(Francia, 12.0, 2.2, 150.0, 1.0, 0.6, TENDEROS),
    ll!(Italia, 12.0, 2.0, 60.0, 0.9, 0.35, OBREROS),
    ll!(BuenosAires, 10.0, 3.0, 120.0, 1.0, 0.4, PORTENOS),
    ll!(Brasil, 5.0, 2.5, 60.0, 1.0, 0.2, CAMPO),
    ll!(Portugal, 3.0, 2.0, 80.0, 0.9, 0.3, MARINOS),
    ll!(Britania, 2.0, 1.5, 500.0, 1.0, 0.9, MERCADERES),
    ll!(Interior, 10.0, 3.5, 30.0, 0.8, 0.15, CAMPO),
];

/// Inmigración (1870–1929): la gran oleada italiana y española; sirio-libaneses, judíos y
/// armenios desde fines del siglo XIX.
pub const INMIGRACION: &[Llegada] = &[
    ll!(Italia, 34.0, 2.6, 50.0, 0.9, 0.45, OBREROS),
    ll!(Galicia, 14.0, 1.8, 50.0, 0.9, 0.5, TENDEROS),
    ll!(Canarias, 3.0, 3.5, 50.0, 0.8, 0.4, CANARIOS),
    ll!(PaisVasco, 4.0, 2.0, 120.0, 1.0, 0.6, TENDEROS),
    ll!(Cataluna, 3.0, 2.0, 150.0, 1.0, 0.7, TENDEROS),
    ll!(Andalucia, 3.0, 2.2, 40.0, 0.9, 0.4, OBREROS),
    ll!(Francia, 5.0, 2.0, 150.0, 1.0, 0.8, TENDEROS),
    ll!(Britania, 2.0, 1.6, 600.0, 1.0, 0.95, PROFESIONALES),
    ll!(Alemania, 2.0, 2.0, 200.0, 1.0, 0.9, TENDEROS),
    ll!(Levante, 3.0, 2.5, 40.0, 0.9, 0.4, TENDEROS),
    ll!(EuropaOriental, 2.0, 3.0, 40.0, 0.9, 0.7, TENDEROS),
    ll!(Armenia, 1.0, 3.0, 30.0, 0.9, 0.5, TENDEROS),
    ll!(Brasil, 5.0, 2.5, 50.0, 1.0, 0.3, OBREROS),
    ll!(BuenosAires, 6.0, 2.5, 150.0, 1.0, 0.7, MODERNOS),
    ll!(Interior, 11.0, 3.5, 40.0, 0.8, 0.4, OBREROS),
];

/// Art Déco y Rambla (1930–1954): migración interna, refugiados judíos y armenios, últimos
/// gallegos e italianos.
pub const ART_DECO: &[Llegada] = &[
    ll!(Interior, 32.0, 3.5, 60.0, 0.8, 0.7, OBREROS),
    ll!(Galicia, 12.0, 2.0, 80.0, 0.9, 0.7, TENDEROS),
    ll!(Italia, 12.0, 2.5, 80.0, 0.9, 0.7, OBREROS),
    ll!(EuropaOriental, 8.0, 2.8, 60.0, 1.0, 0.9, TENDEROS),
    ll!(Levante, 4.0, 2.8, 80.0, 1.0, 0.7, TENDEROS),
    ll!(Armenia, 3.0, 3.0, 60.0, 1.0, 0.7, TENDEROS),
    ll!(Alemania, 3.0, 2.0, 300.0, 1.0, 0.95, PROFESIONALES),
    ll!(Brasil, 5.0, 2.5, 60.0, 1.0, 0.6, OBREROS),
    ll!(BuenosAires, 6.0, 2.5, 200.0, 1.0, 0.9, MODERNOS),
];

/// Modernismo (1955–1989): sobre todo del interior; la ciudad empieza a expulsar gente.
pub const MODERNISMO: &[Llegada] = &[
    ll!(Interior, 60.0, 3.2, 80.0, 0.8, 0.85, MODERNOS),
    ll!(BuenosAires, 8.0, 2.5, 300.0, 1.0, 0.95, MODERNOS),
    ll!(Brasil, 8.0, 2.5, 100.0, 1.0, 0.8, MODERNOS),
    ll!(Galicia, 6.0, 2.0, 150.0, 1.0, 0.9, TENDEROS),
    ll!(Italia, 5.0, 2.0, 150.0, 1.0, 0.9, TENDEROS),
    ll!(Paraguay, 5.0, 2.5, 60.0, 0.9, 0.8, MODERNOS),
    ll!(Latinoamerica, 3.0, 2.0, 100.0, 1.0, 0.9, PROFESIONALES),
];

/// Contemporánea (1990–): migración latinoamericana no fronteriza y retorno.
pub const CONTEMPORANEA: &[Llegada] = &[
    ll!(Interior, 40.0, 2.8, 150.0, 0.9, 0.95, MODERNOS),
    ll!(Latinoamerica, 30.0, 2.0, 120.0, 1.0, 0.95, MODERNOS),
    ll!(BuenosAires, 10.0, 2.3, 400.0, 1.0, 0.98, PROFESIONALES),
    ll!(Brasil, 10.0, 2.3, 200.0, 1.0, 0.95, MODERNOS),
    ll!(Paraguay, 5.0, 2.5, 80.0, 0.9, 0.9, MODERNOS),
    ll!(Italia, 2.5, 2.0, 400.0, 1.0, 0.98, PROFESIONALES),
    ll!(Galicia, 2.5, 2.0, 400.0, 1.0, 0.98, PROFESIONALES),
];

pub fn tabla(e: Epoca) -> &'static [Llegada] {
    match e {
        Epoca::Colonial => COLONIAL,
        Epoca::CiudadNueva => CIUDAD_NUEVA,
        Epoca::Inmigracion => INMIGRACION,
        Epoca::ArtDeco => ART_DECO,
        Epoca::Modernismo => MODERNISMO,
        Epoca::Contemporanea => CONTEMPORANEA,
    }
}

/// Peso de los orígenes indígenas según el año (estimación). Las cuadrillas guaraníes de las
/// misiones fueron muchas en los años de la obra del fuerte; después, la población indígena de
/// Montevideo fue pequeña ("menos del 2 %" a fines de la década de 1770, en su mayoría guaraníes
/// de las misiones; Pollero y González Demuro, 2024). Tras la conquista portuguesa de los
/// pueblos orientales de las misiones (1801) y la guerra, llegan algo más.
pub fn factor_origen(o: Origen, año: i32) -> f32 {
    match (o, año) {
        (Origen::Misiones | Origen::PueblosRegion, ..=1740) => 1.0,
        (Origen::Misiones, 1801..=1830) => 0.6,
        (Origen::Misiones, _) => 0.4,
        (Origen::PueblosRegion, _) => 0.3,
        _ => 1.0,
    }
}

/// Tamaño de la guarnición y de las cuadrillas de la obra del fuerte en la fundación: Zabala
/// informaba en 1728 de unas 350 personas: un centenar de canarios, otro centenar de indios
/// traídos de las misiones, cien soldados y unas cincuenta personas de oficios (Azarola Gil,
/// citado por Pollero y González Demuro).
pub const GUARNICION_1728: u32 = 100;
pub const CUADRILLA_GUARANI_1728: u32 = 100;

/// Llegadas históricas fijas en el modo fiel a la historia (mes absoluto, origen, familias,
/// personas). Seis familias de Buenos Aires (33 personas) ya estaban cuando llegaron los
/// canarios del navío *Nuestra Señora de la Encina* (noviembre de 1726); los contingentes
/// canarios sumaron algo más de 200 personas (el segundo, en 1729, de unas treinta familias).
pub const LLEGADAS_HISTORICAS: &[(i32, Origen, u32, u32)] = &[(10, Origen::Canarias, 20, 96), (38, Origen::Canarias, 25, 110)];
