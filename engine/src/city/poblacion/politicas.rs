//! Políticas sociales para manejar la desigualdad. Las de la época colonial son ordenanzas
//! del cabildo (`economy::Ordinance`, se aprueban y derogan como las demás); sus efectos están
//! en la economía de los hogares, la demografía y la felicidad. Las de las épocas siguientes
//! quedan como datos para cuando existan esas épocas en el motor.

use super::origenes::Epoca;
use crate::city::economy::Ordinance;

/// Ordenanzas sociales de la época colonial y dónde actúan.
pub const SOCIALES: [(Ordinance, &str); 9] = [
    (Ordinance::RepartoSolares, "Los hogares pobres sin casa reciben en propiedad los ranchos libres: más vecinos con solar, menos alquiler."),
    (Ordinance::ContribucionProgresiva, "Los hogares pobres no pagan contribución; los acomodados pagan 1,5 veces y los principales 2,5 veces."),
    (Ordinance::PanDeLosPobres, "Los hogares pobres compran el pan a precio normal aunque esté caro; la ciudad paga la diferencia."),
    (Ordinance::HospitalCaridad, "Menos mortalidad y mejor salud de pobres y modestos."),
    (Ordinance::EscuelasPrimerasLetras, "Más niños aprenden a leer: más movilidad (comercio, oficios letrados)."),
    (Ordinance::PrestamosCabildo, "Préstamos baratos a pobres y modestos: menos hambre en los malos meses."),
    (Ordinance::FondoManumision, "El cabildo completa lo que les falta a quienes juntaron buena parte de la suma de su carta de libertad."),
    (Ordinance::LibertadVientres, "Quien nace en la ciudad nace libre y se prohíbe el tráfico. No se puede derogar."),
    (Ordinance::Abolicion, "Todas las personas esclavizadas recuperan la libertad. No se puede derogar."),
];

/// Una política de una época futura (para la interfaz y el diseño).
pub struct PoliticaFutura {
    pub epoca: Epoca,
    pub nombre: &'static str,
    pub efecto: &'static str,
    /// Costo mensual orientativo por cada 1.000 habitantes (pesos de la época).
    pub costo: i64,
}

pub const FUTURAS: &[PoliticaFutura] = &[
    PoliticaFutura { epoca: Epoca::CiudadNueva, nombre: "Escuela pública gratuita", efecto: "Lectura casi universal de los niños de la ciudad.", costo: 40 },
    PoliticaFutura { epoca: Epoca::Inmigracion, nombre: "Reglamento de conventillos", efecto: "Tope de hacinamiento y piezas con agua: menos epidemias, alquileres más caros.", costo: 15 },
    PoliticaFutura { epoca: Epoca::Inmigracion, nombre: "Jornada de ocho horas", efecto: "Más felicidad obrera, algo menos de producción industrial.", costo: 0 },
    PoliticaFutura { epoca: Epoca::Inmigracion, nombre: "Salario mínimo", efecto: "Piso a los salarios: menos pobreza, menos empleo en los talleres chicos.", costo: 0 },
    PoliticaFutura { epoca: Epoca::ArtDeco, nombre: "Jubilaciones y pensiones", efecto: "Los mayores de 60 años tienen ingreso propio: menos pobreza en la vejez.", costo: 120 },
    PoliticaFutura { epoca: Epoca::ArtDeco, nombre: "Asignaciones familiares", efecto: "Ingreso por hijo para los hogares obreros: más natalidad y menos pobreza infantil.", costo: 60 },
    PoliticaFutura { epoca: Epoca::Modernismo, nombre: "Vivienda cooperativa por ayuda mutua", efecto: "Las familias construyen sus casas con préstamo público: más propietarios.", costo: 80 },
    PoliticaFutura { epoca: Epoca::Modernismo, nombre: "Control de precios", efecto: "Pan y carne con precio máximo: alivio inmediato, desabastecimiento si dura.", costo: 30 },
    PoliticaFutura { epoca: Epoca::Contemporanea, nombre: "Alquiler protegido y vivienda social en el centro", efecto: "Menos expulsión del casco histórico por el turismo.", costo: 90 },
    PoliticaFutura { epoca: Epoca::Contemporanea, nombre: "Transferencias a hogares vulnerables", efecto: "Ingreso mínimo por hogar: baja la pobreza extrema.", costo: 100 },
];
