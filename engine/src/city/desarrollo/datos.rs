//! Tablas del desarrollo: tecnologías por época, perfiles de industria, productos que se
//! comercian, incentivos y choques de precios. Todo es dato: las épocas siguientes ya están
//! cargadas (con `epoca`) aunque todavía no se jueguen.
//!
//! Fechas y hechos: solo los verificados llevan fecha en `historia` (fuentes en
//! `docs/sistemas/desarrollo-y-tecnologia.md`); el resto es licencia de diseño y lo dice.
//! Montos en pesos de plata por mes: ESTIMACIONES de orden de magnitud para el juego.

use super::{Bien, Ind, Prod, Saber, Tec};
use crate::city::economy::Res;
use crate::city::poblacion::Epoca;
use crate::city::services::Kind;

// --- Tecnologías ----------------------------------------------------------------------------

/// Requisito para empezar a invertir en una tecnología.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Req {
    /// Gente con ese saber en la ciudad (personas).
    Saber(Saber, u32),
    /// Edificios de ese tipo construidos.
    Edificio(Kind, u32),
    /// Otra tecnología terminada.
    Tec(Tec),
    /// Muelle en la costa.
    Muelle,
    /// Habitantes de la ciudad.
    Habitantes(u32),
}

/// Lo que da una tecnología al terminarse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Efecto {
    /// Se puede construir ese edificio (institución o industria).
    Edificio(Kind),
    /// Multiplica lo que produce una industria.
    Productividad(Ind, f32),
    /// Un producto nuevo para vender afuera.
    Producto(Prod),
    /// Otro efecto, contado en palabras.
    Texto(&'static str),
}

pub struct TecInfo {
    pub id: Tec,
    pub nombre: &'static str,
    pub desc: &'static str,
    pub epoca: Epoca,
    pub req: &'static [Req],
    /// Inversión total en pesos (se paga en cuotas mensuales) y ladrillos por mes de obra.
    pub costo: i64,
    pub ladrillos_mes: f32,
    pub meses: u32,
    pub efectos: &'static [Efecto],
    /// Nota histórica: fecha verificada (fuente en el documento) o "licencia de diseño".
    pub historia: &'static str,
}

use Saber::*;

pub const TECNOLOGIAS: &[TecInfo] = &[
    // --- Época 1: colonial (jugable) ---
    TecInfo { id: Tec::EscuelaOficios, nombre: "Escuela de dibujo y oficios", desc: "Maestros de dibujo, geometría y taller: los jóvenes que saben leer salen oficiales de carpintería, herrería y albañilería.", epoca: Epoca::Colonial, req: &[Req::Saber(Alfabetizados, 80), Req::Edificio(Kind::Escuela, 1)], costo: 1500, ladrillos_mes: 0.0, meses: 6, efectos: &[Efecto::Edificio(Kind::EscuelaOficios)], historia: "Licencia de diseño: en el Río de la Plata, el Consulado de Buenos Aires abrió una Escuela de Dibujo en 1799; la Escuela de Artes y Oficios de Montevideo es de 1879." },
    TecInfo { id: Tec::CurtidoMejorado, nombre: "Curtido con corteza y noques", desc: "Pozas de curtir, corteza y oficiales que saben el punto: los talleres convierten cueros secos en suela, que vale el doble.", epoca: Epoca::Colonial, req: &[Req::Saber(Artesanos, 20), Req::Saber(Oficiales, 4)], costo: 1200, ladrillos_mes: 10.0, meses: 6, efectos: &[Efecto::Producto(Prod::Suela), Efecto::Productividad(Ind::Talleres, 1.15)], historia: "Licencia de diseño (técnica antigua; sin fecha para Montevideo)." },
    TecInfo { id: Tec::HornoMejorado, nombre: "Horno de ladrillos de bóveda", desc: "Hornos con bóveda y mejor tiraje: un 40 % más de ladrillos con la misma leña.", epoca: Epoca::Colonial, req: &[Req::Edificio(Kind::Horno, 1), Req::Saber(Oficiales, 6)], costo: 1000, ladrillos_mes: 20.0, meses: 4, efectos: &[Efecto::Texto("+40 % de ladrillos en hornos y talleres")], historia: "Licencia de diseño." },
    TecInfo { id: Tec::Saladero, nombre: "Salazón de carnes (saladero)", desc: "Faenar el ganado, salar la carne y secarla al sol: tasajo para vender afuera, además de cueros y sebo.", epoca: Epoca::Colonial, req: &[Req::Edificio(Kind::Estancia, 1), Req::Muelle, Req::Saber(Oficiales, 8)], costo: 3000, ladrillos_mes: 15.0, meses: 10, efectos: &[Efecto::Edificio(Kind::Saladero), Efecto::Producto(Prod::Tasajo)], historia: "Primer saladero de la Banda Oriental: Francisco de Medina, 1781, arroyo Colla; en Montevideo, Antonio Maciel, 1787, junto al Miguelete (INAC). El tasajo se exportó a Cuba." },
    TecInfo { id: Tec::Graseria, nombre: "Grasería", desc: "Calderas para sacar la grasa de los huesos y achuras: el saladero rinde más sebo y algo más de tasajo.", epoca: Epoca::Colonial, req: &[Req::Tec(Tec::Saladero), Req::Saber(Oficiales, 15)], costo: 1500, ladrillos_mes: 10.0, meses: 6, efectos: &[Efecto::Productividad(Ind::Saladero, 1.15), Efecto::Texto("Doble sebo en los saladeros")], historia: "Licencia de diseño." },
    TecInfo { id: Tec::Molino, nombre: "Molino de viento", desc: "Torre, aspas y piedras de moler: más harina y más barata que la atahona de caballo.", epoca: Epoca::Colonial, req: &[Req::Edificio(Kind::Chacra, 1), Req::Saber(Oficiales, 10)], costo: 2000, ladrillos_mes: 15.0, meses: 8, efectos: &[Efecto::Edificio(Kind::Molino)], historia: "Licencia de diseño (atahona → molino; sin fecha verificada para Montevideo)." },
    TecInfo { id: Tec::Astillero, nombre: "Carpintería de ribera", desc: "Gradas, carpinteros y calafates para carenar los barcos de paso y armar lanchas.", epoca: Epoca::Colonial, req: &[Req::Muelle, Req::Saber(Oficiales, 12)], costo: 3500, ladrillos_mes: 10.0, meses: 10, efectos: &[Efecto::Edificio(Kind::Astillero), Efecto::Producto(Prod::Carenas)], historia: "Montevideo fue apostadero de la Real Armada desde 1776 (De Marco); la carpintería de ribera es licencia de diseño." },
    TecInfo { id: Tec::EscuelaNautica, nombre: "Escuela de náutica", desc: "Matemáticas, cartas y pilotaje: pilotos propios para el cabotaje, que venden mejor lo que la ciudad embarca.", epoca: Epoca::Colonial, req: &[Req::Saber(Alfabetizados, 200), Req::Tec(Tec::Astillero)], costo: 2500, ladrillos_mes: 0.0, meses: 8, efectos: &[Efecto::Edificio(Kind::EscuelaNautica), Efecto::Texto("Pilotos: hasta +15 % en el precio de lo que se exporta")], historia: "El Consulado de Buenos Aires (secretario: Manuel Belgrano) abrió su Escuela de Náutica en noviembre de 1799; en Montevideo es licencia de diseño." },
    TecInfo { id: Tec::Imprenta, nombre: "Imprenta", desc: "Prensa, tipos y papel: bandos, cartillas, almanaques y una gaceta con los precios corrientes.", epoca: Epoca::Colonial, req: &[Req::Saber(Alfabetizados, 300), Req::Saber(Oficiales, 6)], costo: 3000, ladrillos_mes: 0.0, meses: 10, efectos: &[Efecto::Edificio(Kind::Imprenta), Efecto::Texto("La gaceta de precios: los comerciantes venden mejor")], historia: "El primer periódico de Montevideo, The Southern Star / La Estrella del Sur, se imprimió en 1807 durante la ocupación británica." },
    // --- Épocas siguientes (datos; todavía no se juegan) ---
    TecInfo { id: Tec::LanaMerina, nombre: "Lana merina y lavadero", desc: "Ovejas finas en las estancias y lavaderos de lana junto a los arroyos: un producto nuevo para Europa.", epoca: Epoca::CiudadNueva, req: &[Req::Edificio(Kind::Estancia, 3)], costo: 4000, ladrillos_mes: 0.0, meses: 12, efectos: &[Efecto::Texto("Lana para exportar")], historia: "La lana empieza a pesar en las exportaciones en la época 2 (GDD); sin fecha verificada." },
    TecInfo { id: Tec::MolinoVapor, nombre: "Molino a vapor", desc: "Máquina de vapor que mueve las piedras sin depender del viento.", epoca: Epoca::CiudadNueva, req: &[Req::Tec(Tec::Molino), Req::Saber(Tecnicos, 10)], costo: 8000, ladrillos_mes: 30.0, meses: 12, efectos: &[Efecto::Productividad(Ind::Molino, 2.0)], historia: "Licencia de diseño." },
    TecInfo { id: Tec::Ferrocarril, nombre: "Ferrocarril", desc: "Vías y estaciones hacia el interior: el ganado y la lana llegan más rápido y más barato al puerto.", epoca: Epoca::Inmigracion, req: &[Req::Saber(Ingenieros, 5), Req::Saber(Tecnicos, 40)], costo: 60000, ladrillos_mes: 200.0, meses: 36, efectos: &[Efecto::Texto("Menos costo de flete desde el campo")], historia: "Fecha a verificar (primer tramo hacia el interior en la década de 1860)." },
    TecInfo { id: Tec::EscuelaArtesOficios, nombre: "Escuela Nacional de Artes y Oficios", desc: "La escuela técnica del Estado: obreros calificados para los talleres y fábricas.", epoca: Epoca::Inmigracion, req: &[Req::Saber(Alfabetizados, 3000)], costo: 15000, ladrillos_mes: 50.0, meses: 18, efectos: &[Efecto::Texto("Forma técnicos")], historia: "Creada en 1879 (algunas fuentes dicen 1878); base de la UTU." },
    TecInfo { id: Tec::Frigorifico, nombre: "Frigorífico", desc: "Cámaras de frío y barcos frigoríficos: carne congelada para Europa. Reemplaza al saladero.", epoca: Epoca::Inmigracion, req: &[Req::Tec(Tec::Saladero), Req::Saber(Tecnicos, 60), Req::Saber(Ingenieros, 4)], costo: 120000, ladrillos_mes: 300.0, meses: 30, efectos: &[Efecto::Texto("Carne congelada para exportar")], historia: "La Frigorífica Uruguaya, en el Cerro de Montevideo, 1902 (INAC)." },
    TecInfo { id: Tec::Electricidad, nombre: "Usina eléctrica", desc: "Dínamos y cables: alumbrado eléctrico, tranvías y motores en los talleres.", epoca: Epoca::Inmigracion, req: &[Req::Saber(Ingenieros, 8)], costo: 80000, ladrillos_mes: 150.0, meses: 24, efectos: &[Efecto::Texto("Más productividad en todas las industrias")], historia: "Sin fecha verificada." },
    TecInfo { id: Tec::FacultadIngenieria, nombre: "Facultad de ingeniería", desc: "Ingenieros propios para puertos, vías, usinas y fábricas.", epoca: Epoca::Inmigracion, req: &[Req::Saber(Alfabetizados, 10000)], costo: 40000, ladrillos_mes: 80.0, meses: 24, efectos: &[Efecto::Texto("Forma ingenieros")], historia: "Sin fecha verificada en este documento." },
    TecInfo { id: Tec::Utu, nombre: "Universidad del Trabajo (UTU)", desc: "Enseñanza técnica en todo el país: técnicos para la industria nacional.", epoca: Epoca::ArtDeco, req: &[Req::Tec(Tec::EscuelaArtesOficios)], costo: 60000, ladrillos_mes: 100.0, meses: 24, efectos: &[Efecto::Texto("Muchos más técnicos")], historia: "Decreto-ley 10.225 del 9 de septiembre de 1942." },
    TecInfo { id: Tec::Textil, nombre: "Industria textil", desc: "Hilanderías y tejedurías con la lana del país: muchos empleos, poco capital por obrero.", epoca: Epoca::ArtDeco, req: &[Req::Tec(Tec::LanaMerina), Req::Saber(Tecnicos, 100)], costo: 50000, ladrillos_mes: 100.0, meses: 18, efectos: &[Efecto::Texto("Fábricas textiles")], historia: "Sin fecha verificada." },
    TecInfo { id: Tec::Refineria, nombre: "Refinería de petróleo", desc: "Combustibles del país: menos importaciones, mucho capital.", epoca: Epoca::ArtDeco, req: &[Req::Saber(Ingenieros, 30)], costo: 300000, ladrillos_mes: 400.0, meses: 36, efectos: &[Efecto::Texto("Combustibles propios")], historia: "Sin fecha verificada." },
    TecInfo { id: Tec::Metalurgia, nombre: "Metalurgia y química", desc: "Talleres metalúrgicos y plantas químicas en los parques industriales.", epoca: Epoca::Modernismo, req: &[Req::Saber(Ingenieros, 60), Req::Saber(Tecnicos, 400)], costo: 200000, ladrillos_mes: 300.0, meses: 30, efectos: &[Efecto::Texto("Industria pesada")], historia: "Sin fecha verificada." },
    TecInfo { id: Tec::Software, nombre: "Software y servicios", desc: "Programadores y empresas de servicios que venden al mundo: pocos empleos, mucho valor.", epoca: Epoca::Contemporanea, req: &[Req::Saber(Ingenieros, 300)], costo: 100000, ladrillos_mes: 0.0, meses: 24, efectos: &[Efecto::Texto("Exportación de servicios")], historia: "Sin fecha verificada." },
    TecInfo { id: Tec::Celulosa, nombre: "Planta de celulosa", desc: "Forestación y una planta enorme: exporta mucho con poca gente, y contamina el río si no se controla.", epoca: Epoca::Contemporanea, req: &[Req::Saber(Ingenieros, 100), Req::Saber(Tecnicos, 500)], costo: 1_000_000, ladrillos_mes: 500.0, meses: 36, efectos: &[Efecto::Texto("Celulosa para exportar")], historia: "Sin fecha verificada." },
    TecInfo { id: Tec::EnergiaEolica, nombre: "Energía eólica", desc: "Molinos de viento modernos: energía propia y renovable.", epoca: Epoca::Contemporanea, req: &[Req::Saber(Ingenieros, 150)], costo: 400_000, ladrillos_mes: 100.0, meses: 30, efectos: &[Efecto::Texto("Energía renovable")], historia: "Sin fecha verificada." },
];

pub fn tec_info(t: Tec) -> &'static TecInfo {
    TECNOLOGIAS.iter().find(|i| i.id == t).unwrap()
}

// --- Industrias -----------------------------------------------------------------------------

/// Perfil de una industria: cuánta gente emplea, cuánto capital pide, qué saber necesita, qué
/// consume, qué produce y cuánto se vende afuera. Los capitales y rindes son estimaciones.
pub struct IndInfo {
    pub id: Ind,
    pub nombre: &'static str,
    pub desc: &'static str,
    pub epoca: Epoca,
    /// Edificio que la aloja (`None`: talleres de las zonas industriales).
    pub edificio: Option<Kind>,
    /// Tecnología que la habilita.
    pub tec: Option<Tec>,
    /// Peones (o empleados) por establecimiento.
    pub empleos: u32,
    /// Gente con saber que necesita (oficiales, pilotos, técnicos…) además de los peones.
    pub saber: Option<(Saber, u32)>,
    /// Capital para abrir un establecimiento (pesos).
    pub capital: i64,
    /// Salario mensual de un peón.
    pub salario: f32,
    /// Insumo por mes a plena marcha (recurso del mercado y cantidad).
    pub insumo: Option<(Res, f32)>,
    /// Lo que produce por mes a plena marcha.
    pub produce: &'static [(Bien, f32)],
    /// Parte de los productos propios (`Bien::Prod`) que se vende afuera.
    pub exporta: f32,
    /// ¿Vende en la ciudad productos que compiten con lo importado? (los aranceles la protegen)
    pub mercado_local: bool,
    /// Recurso del suelo que necesita el sitio ("pastura", "arcilla", "costa", "viento"…),
    /// para el módulo de recursos del suelo (`city::recursos`).
    pub recurso: Option<&'static str>,
    /// Molestia para los vecinos (0 a 1): olor, ruido, agua sucia.
    pub molestia: f32,
}

pub const INDUSTRIAS: &[IndInfo] = &[
    IndInfo { id: Ind::Talleres, nombre: "Talleres de la zona industrial", desc: "Herreros, carpinteros, curtidores y veleros en los solares industriales: muchos brazos y poco capital; producen para la ciudad.", epoca: Epoca::Colonial, edificio: None, tec: None, empleos: 12, saber: None, capital: 400, salario: 7.0, insumo: None, produce: &[(Bien::Res(Res::Ladrillos), 18.0), (Bien::Res(Res::Sebo), 12.0), (Bien::Res(Res::Cueros), 9.6)], exporta: 0.0, mercado_local: true, recurso: None, molestia: 0.5 },
    IndInfo { id: Ind::Saladero, nombre: "Saladero", desc: "Mucho capital y pocos peones por lo que produce: casi todo se exporta.", epoca: Epoca::Colonial, edificio: Some(Kind::Saladero), tec: Some(Tec::Saladero), empleos: 20, saber: Some((Saber::Oficiales, 4)), capital: 6000, salario: 9.0, insumo: Some((Res::Carne, 400.0)), produce: &[(Bien::Prod(Prod::Tasajo), 300.0), (Bien::Res(Res::Cueros), 50.0), (Bien::Res(Res::Sebo), 40.0)], exporta: 0.95, mercado_local: false, recurso: Some("pastura"), molestia: 0.9 },
    IndInfo { id: Ind::Molino, nombre: "Molino de viento", desc: "Poco empleo, capital medio: harina para el pan de la ciudad.", epoca: Epoca::Colonial, edificio: Some(Kind::Molino), tec: Some(Tec::Molino), empleos: 4, saber: Some((Saber::Oficiales, 1)), capital: 2500, salario: 9.0, insumo: None, produce: &[(Bien::Res(Res::Pan), 500.0)], exporta: 0.0, mercado_local: true, recurso: Some("cultivo"), molestia: 0.1 },
    IndInfo { id: Ind::Astillero, nombre: "Carpintería de ribera", desc: "Oficio fino y capital: carenas que pagan los barcos de afuera (exportación de servicios) y lanchas.", epoca: Epoca::Colonial, edificio: Some(Kind::Astillero), tec: Some(Tec::Astillero), empleos: 12, saber: Some((Saber::Oficiales, 8)), capital: 6000, salario: 9.0, insumo: None, produce: &[(Bien::Prod(Prod::Carenas), 6.0)], exporta: 0.7, mercado_local: false, recurso: Some("costa"), molestia: 0.3 },
    IndInfo { id: Ind::Imprenta, nombre: "Imprenta", desc: "Pocos cajistas que saben leer: impresos para la ciudad (en vez de traerlos de afuera).", epoca: Epoca::Colonial, edificio: Some(Kind::Imprenta), tec: Some(Tec::Imprenta), empleos: 4, saber: Some((Saber::Oficiales, 2)), capital: 3000, salario: 10.0, insumo: None, produce: &[(Bien::Prod(Prod::Impresos), 80.0)], exporta: 0.0, mercado_local: true, recurso: None, molestia: 0.05 },
    // Épocas siguientes: perfiles para comparar (todavía no se construyen).
    IndInfo { id: Ind::Frigorifico, nombre: "Frigorífico", desc: "Enorme capital, técnicos e ingenieros; produce muchísimo por obrero y exporta casi todo.", epoca: Epoca::Inmigracion, edificio: None, tec: Some(Tec::Frigorifico), empleos: 600, saber: Some((Saber::Tecnicos, 40)), capital: 120000, salario: 12.0, insumo: Some((Res::Carne, 20000.0)), produce: &[(Bien::Prod(Prod::Tasajo), 18000.0)], exporta: 0.95, mercado_local: false, recurso: Some("pastura"), molestia: 0.8 },
    IndInfo { id: Ind::Textil, nombre: "Fábrica textil", desc: "Muchos empleos, poco capital por obrero: viste al país y sustituye importaciones.", epoca: Epoca::ArtDeco, edificio: None, tec: Some(Tec::Textil), empleos: 300, saber: Some((Saber::Tecnicos, 15)), capital: 50000, salario: 10.0, insumo: None, produce: &[(Bien::Prod(Prod::Impresos), 3000.0)], exporta: 0.1, mercado_local: true, recurso: None, molestia: 0.3 },
    IndInfo { id: Ind::Software, nombre: "Empresa de software", desc: "Pocos empleos, todos calificados; casi todo se vende afuera.", epoca: Epoca::Contemporanea, edificio: None, tec: Some(Tec::Software), empleos: 0, saber: Some((Saber::Ingenieros, 60)), capital: 80000, salario: 0.0, insumo: None, produce: &[(Bien::Prod(Prod::Carenas), 300.0)], exporta: 0.9, mercado_local: false, recurso: None, molestia: 0.0 },
    IndInfo { id: Ind::Celulosa, nombre: "Planta de celulosa", desc: "Capital gigantesco y muy poca gente por lo que produce; exporta todo.", epoca: Epoca::Contemporanea, edificio: None, tec: Some(Tec::Celulosa), empleos: 200, saber: Some((Saber::Ingenieros, 40)), capital: 1_000_000, salario: 30.0, insumo: None, produce: &[(Bien::Prod(Prod::Carenas), 2000.0)], exporta: 1.0, mercado_local: false, recurso: Some("forestal"), molestia: 0.7 },
];

pub fn ind_info(i: Ind) -> &'static IndInfo {
    INDUSTRIAS.iter().find(|x| x.id == i).unwrap()
}

/// Industria que funciona en un edificio dado.
pub fn ind_de_edificio(k: Kind) -> Option<Ind> {
    INDUSTRIAS.iter().find(|i| i.edificio == Some(k)).map(|i| i.id)
}

// --- Productos ------------------------------------------------------------------------------

#[allow(dead_code)]
pub struct ProdInfo {
    pub id: Prod,
    pub nombre: &'static str,
    /// Precio de referencia en el puerto (pesos por unidad) y la unidad.
    pub precio: f32,
    pub unidad: &'static str,
}

pub const PRODUCTOS: [ProdInfo; 8] = [
    ProdInfo { id: Prod::Cueros, nombre: "Cueros", precio: 2.0, unidad: "cuero" },
    ProdInfo { id: Prod::Sebo, nombre: "Sebo", precio: 0.8, unidad: "arroba" },
    ProdInfo { id: Prod::Carne, nombre: "Carne", precio: 0.15, unidad: "ración" },
    ProdInfo { id: Prod::Trigo, nombre: "Trigo y harina", precio: 0.25, unidad: "ración" },
    ProdInfo { id: Prod::Tasajo, nombre: "Tasajo", precio: 1.0, unidad: "arroba" },
    ProdInfo { id: Prod::Suela, nombre: "Suela", precio: 4.0, unidad: "suela" },
    ProdInfo { id: Prod::Carenas, nombre: "Carenas y lanchas", precio: 60.0, unidad: "trabajo" },
    ProdInfo { id: Prod::Impresos, nombre: "Impresos", precio: 1.5, unidad: "pliego" },
];

pub fn prod_info(p: Prod) -> &'static ProdInfo {
    &PRODUCTOS[p as usize]
}

/// El producto del comercio exterior que corresponde a un recurso de la economía.
pub fn prod_de_res(r: Res) -> Option<Prod> {
    match r {
        Res::Cueros => Some(Prod::Cueros),
        Res::Sebo => Some(Prod::Sebo),
        Res::Carne => Some(Prod::Carne),
        Res::Pan => Some(Prod::Trigo),
        _ => None,
    }
}

// --- Incentivos -----------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Incentivo {
    /// Las industrias nuevas no pagan derechos de aduana por diez años.
    Exencion,
    /// El cabildo presta la mitad del capital al 4 % anual.
    Credito,
    /// Aranceles altos a las manufacturas importadas (no con libre comercio).
    Arancel,
    /// Privilegio exclusivo: un solo establecimiento por industria, que paga un canon.
    Privilegio,
    /// El cabildo contrata maestros de oficio en ultramar.
    Maestros,
}

pub struct IncInfo {
    pub id: Incentivo,
    pub nombre: &'static str,
    pub desc: &'static str,
    /// Costo fijo mensual (pesos; lo variable sale en el presupuesto).
    pub mensual: i64,
    /// Opinión de vecinos, comerciantes y hacendados a la larga.
    pub opinion: [f32; 3],
    pub historia: &'static str,
}

pub const INCENTIVOS: [IncInfo; 5] = [
    IncInfo { id: Incentivo::Exencion, nombre: "Exención a industrias nuevas", desc: "Los establecimientos de menos de diez años no pagan derechos de aduana por lo que exportan. Atrae inversores; el cabildo deja de cobrar (la «renuncia fiscal» se ve en el panel).", mensual: 0, opinion: [0.0, -4.0, 6.0], historia: "Las franquicias a industrias nuevas fueron comunes en el siglo XVIII; aplicada al cabildo de Montevideo es licencia de diseño." },
    IncInfo { id: Incentivo::Credito, nombre: "Crédito de fomento", desc: "El cabildo presta la mitad del capital al 4 % anual: alcanza con un vecino que tenga la otra mitad. Si el negocio fracasa, parte del préstamo no vuelve. 10 pesos por mes de administración.", mensual: 10, opinion: [2.0, 3.0, 3.0], historia: "Licencia de diseño (los censos y capellanías eran el crédito de la época)." },
    IncInfo { id: Incentivo::Arancel, nombre: "Aranceles protectores", desc: "Recargo a las manufacturas que llegan de afuera: los molinos, talleres e imprentas venden más caro en la ciudad. Cobra algo más de aduana, pero los vecinos pagan más y los comerciantes se enojan. No se puede con el libre comercio.", mensual: 0, opinion: [-4.0, -8.0, 2.0], historia: "Licencia de diseño; el debate entre protección y libre comercio recorre todo el siglo XIX." },
    IncInfo { id: Incentivo::Privilegio, nombre: "Privilegios exclusivos", desc: "Un solo establecimiento por industria: el primero gana un 30 % más y paga al cabildo un canon de 25 pesos por mes. No entra la competencia.", mensual: 0, opinion: [-2.0, -6.0, -8.0], historia: "Los estancos y privilegios reales existían en el sistema colonial español; aplicados así, licencia de diseño." },
    IncInfo { id: Incentivo::Maestros, nombre: "Maestros de ultramar", desc: "El cabildo paga el viaje y el salario de maestros de oficio europeos que enseñan en los talleres: más oficiales cada año. 60 pesos por mes.", mensual: 60, opinion: [0.0, 2.0, 0.0], historia: "Licencia de diseño." },
];

pub fn inc_info(i: Incentivo) -> &'static IncInfo {
    INCENTIVOS.iter().find(|x| x.id == i).unwrap()
}

// --- Choques de precios en el mundo ---------------------------------------------------------

pub struct Choque {
    pub nombre: &'static str,
    pub prod: Prod,
    pub factor: f32,
    pub meses: u32,
    pub noticia: &'static str,
}

/// Choques de precios (ficticios, con guiños a la historia; GDD 4.11).
pub const CHOQUES: [Choque; 6] = [
    Choque { nombre: "Sobra tasajo en La Habana", prod: Prod::Tasajo, factor: 0.55, meses: 8, noticia: "Gaceta: sobra tasajo en La Habana y en el Brasil; los barcos lo pagan a la mitad." },
    Choque { nombre: "Guerra en Europa", prod: Prod::Cueros, factor: 1.5, meses: 10, noticia: "Gaceta: guerra en Europa; los ejércitos piden cueros para correajes y botas. Suben los precios." },
    Choque { nombre: "Cueros de sobra en Cádiz", prod: Prod::Cueros, factor: 0.55, meses: 9, noticia: "Gaceta: los depósitos de Cádiz rebosan de cueros del Plata. El precio se desploma." },
    Choque { nombre: "Escasez de sebo en ultramar", prod: Prod::Sebo, factor: 1.6, meses: 6, noticia: "Gaceta: falta sebo para las velas de ultramar; se paga bien el de la ciudad." },
    Choque { nombre: "Suela inglesa barata", prod: Prod::Suela, factor: 0.6, meses: 8, noticia: "Gaceta: llega suela inglesa barata a todos los puertos; la nuestra vale menos." },
    Choque { nombre: "Escuadras de paso", prod: Prod::Carenas, factor: 1.6, meses: 6, noticia: "Gaceta: escuadras de paso necesitan carena; la carpintería de ribera no da abasto." },
];
