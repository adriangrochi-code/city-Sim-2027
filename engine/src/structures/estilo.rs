//! Estilo de cada casa colonial: la tipología y los parámetros de fachada, planta, color y
//! desgaste con que se dibuja (ver `docs/sistemas/arquitectura-colonial.md` y la guía de
//! fuentes `docs/sistemas/arquitectura-colonial-fuentes.md`).
//!
//! Todo sale de forma determinista de tres cosas: el [`Contexto`] que se guardó con la obra
//! al levantarla (año, clase del hogar, esquina, barrio, puerto y la "tirada" contra la
//! repetición), los materiales reales de la obra ([`MaterialesObra`]) y la semilla del solar.
//! Así la misma partida, guardada y cargada, dibuja exactamente las mismas casas.
//!
//! Marcas de los números: **[F]** dato documentado en una fuente de la guía; **[E]**
//! estimación de la guía (arquitectura colonial rioplatense en general) o de diseño del juego.

use crate::buildings::Rng;
use crate::palette::*;
use serde::{Deserialize, Serialize};

/// Lo que se sabía del solar cuando se levantó la casa. Se guarda con la obra (las partidas
/// viejas no lo tienen: `anio == 0` y se deduce de la semilla).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Contexto {
    /// Año en que se levantó (0 = no se sabe).
    #[serde(default)]
    pub anio: u16,
    /// Clase del hogar: 0 pobre, 1 modesta, 2 acomodada, 3 principal.
    #[serde(default)]
    pub clase: u8,
    /// Esquina: 0 no, 1 la calle lateral está del lado u = 0, 2 del lado u = ancho.
    #[serde(default)]
    pub esquina: u8,
    /// Barrio: 0 centro (cerca de la plaza), 1 intramuros, 2 orillas, 3 extramuros.
    #[serde(default)]
    pub lugar: u8,
    /// Cerca del puerto (almacenes, barracas y miradores para ver los barcos).
    #[serde(default)]
    pub puerto: bool,
    /// Tirada: se vuelve a sortear el estilo para no repetir al vecino (regla anti-monotonía).
    #[serde(default)]
    pub giro: u8,
    /// Tradición constructiva de la familia que la levantó (índice en [`Tradicion::ALL`]).
    #[serde(default)]
    pub tradicion: u8,
    /// Época del kit con que se levantó: 0 colonial, 1 Ciudad Nueva (fachada neoclásica e
    /// italianizante; ver `docs/sistemas/arquitectura-epoca2.md`). Las partidas viejas, 0.
    #[serde(default)]
    pub epoca: u8,
    /// Año en que se modernizó la fachada en el lugar (rehabilitación: misma planta, fachada
    /// nueva; 0 = nunca).
    #[serde(default)]
    pub reforma: u16,
    /// Materiales de antes de la reforma (bit 0: sin revoque; bit 1: sin hierro): la planta se
    /// sigue dibujando con ellos, así la casa reformada conserva su huella.
    #[serde(default)]
    pub antes: u8,
}

/// Tradición constructiva según el origen de la familia (ver [`RASGOS`]). Es un ajuste de
/// pesos sobre el estilo, no una tipología aparte. El mapeo origen → tradición lo hace la
/// ciudad (`city::materiales::ciudad::tradicion_de`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tradicion {
    /// Sin rasgos propios documentados (o no se sabe): el estilo de la época.
    #[default]
    Ninguna,
    /// Criollos de Montevideo y porteños.
    Criolla,
    Canaria,
    Gallega,
    Andaluza,
    /// Cantábricos, vascos y catalanes: comerciantes, pocos y ricos.
    NorteEspana,
    /// Portugueses y luso-brasileños.
    Lusa,
    /// Pueblos guaraníes de las misiones.
    Guarani,
    /// Italianos (casi no los hay antes de 1830).
    Italiana,
    /// Franceses y vascofranceses (época 2 en adelante).
    Francesa,
}

impl Tradicion {
    pub const ALL: [Tradicion; 10] = [Tradicion::Ninguna, Tradicion::Criolla, Tradicion::Canaria, Tradicion::Gallega, Tradicion::Andaluza, Tradicion::NorteEspana, Tradicion::Lusa, Tradicion::Guarani, Tradicion::Italiana, Tradicion::Francesa];
    pub fn indice(self) -> u8 {
        Tradicion::ALL.iter().position(|&t| t == self).unwrap_or(0) as u8
    }
    pub fn de_indice(i: u8) -> Tradicion {
        Tradicion::ALL.get(i as usize).copied().unwrap_or_default()
    }
    pub fn rasgos(self) -> &'static Rasgos {
        RASGOS.iter().find(|r| r.tradicion == self).unwrap_or(&RASGOS[0])
    }
}

/// Rasgos de una tradición: ajustes que se suman (o multiplican) a los pesos de la época y la
/// clase. Son datos, para extenderlos a las épocas siguientes (`desde`/`hasta` acotan cuándo
/// se aplican). Las fuentes de cada fila están en `docs/sistemas/arquitectura-colonial.md`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rasgos {
    pub tradicion: Tradicion,
    /// Años en que la tradición se nota en la ciudad (fuera de ese rango, sin ajuste).
    pub desde: u16,
    pub hasta: u16,
    /// Multiplica la preferencia por la azotea frente a la teja al elegir el techo.
    pub azotea: f32,
    /// Suma a los pesos de color de cal: blanca, hueso, ocre, rosado, almagre, celeste, gris.
    pub colores: [f32; 7],
    /// Suma a la probabilidad de dejar la piedra a la vista (sillería) en casas de piedra.
    pub piedra_vista: f32,
    /// Suma a la probabilidad de reja (plana o voladiza) y a la de que sea voladiza.
    pub reja: f32,
    pub voladiza: f32,
    /// Probabilidad de balcón de madera (en lugar de hierro) en las casas de altos.
    pub balcon_madera: f32,
    /// Suma al peso de la carpintería verde.
    pub verde: f32,
    /// Suma a la probabilidad de galería al frente (casas de teja) y de enramada (ranchos).
    pub galeria: f32,
    /// Probabilidad de alero de teja largo (1 m en lugar de 0,5 m).
    pub alero: f32,
    /// Resta a la probabilidad de casa en "U": más casas de medio patio ("casa chorizo").
    pub medio_patio: f32,
    /// Probabilidad de subir un nivel de ornamento (molduras, cornisas, balaustradas).
    pub ornamento: f32,
    /// Texto de la ficha: "azotea, balcón de madera y carpintería verde".
    pub texto: &'static str,
    /// Marca de la fuente: "F" documentado, "E" estimación, "—" sin rasgos documentados.
    pub marca: &'static str,
}

const SIN: Rasgos = Rasgos { tradicion: Tradicion::Ninguna, desde: 0, hasta: 9999, azotea: 1.0, colores: [0.0; 7], piedra_vista: 0.0, reja: 0.0, voladiza: 0.0, balcon_madera: 0.0, verde: 0.0, galeria: 0.0, alero: 0.0, medio_patio: 0.0, ornamento: 0.0, texto: "", marca: "—" };

/// Tabla origen → rasgos (las fuentes de cada fila, en la documentación).
pub const RASGOS: &[Rasgos] = &[
    SIN,
    // Casa de patio y rejas voladas; la "casa chorizo" porteña es «la mitad de la casa
    // colonial de patios» [F para Buenos Aires, guía §2.4]; los pesos son [E].
    Rasgos { tradicion: Tradicion::Criolla, voladiza: 0.15, reja: 0.1, medio_patio: 0.2, texto: "medio patio y rejas voladas", marca: "F/E", ..SIN },
    // La casa a patio con azotea es «de clara influencia canaria» [F, Delgado Dopazo 2004
    // según Carmona y Álvarez Lenzi]. Balcones de madera y carpintería verde son rasgos de la
    // arquitectura de las islas [E para Montevideo].
    Rasgos { tradicion: Tradicion::Canaria, azotea: 1.8, colores: [0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], balcon_madera: 0.6, verde: 0.35, texto: "azotea, balcón de madera y carpintería verde", marca: "F/E", ..SIN },
    // Piedra de granito a la vista y blanqueo [E].
    Rasgos { tradicion: Tradicion::Gallega, colores: [0.15, 0.0, 0.0, 0.0, 0.0, 0.0, 0.05], piedra_vista: 0.3, texto: "piedra a la vista y cal blanca", marca: "E", ..SIN },
    // Encalado blanco, rejas y patio [E].
    Rasgos { tradicion: Tradicion::Andaluza, colores: [0.25, 0.0, 0.0, 0.0, 0.03, 0.0, 0.0], reja: 0.2, voladiza: 0.1, texto: "cal blanca, rejas y patio", marca: "E", ..SIN },
    // Comerciantes del norte de España: más ornamento y piedra labrada [E].
    Rasgos { tradicion: Tradicion::NorteEspana, ornamento: 0.4, piedra_vista: 0.15, texto: "piedra labrada y cornisas", marca: "E", ..SIN },
    // Casas de piedra portuguesas con techo a dos y cuatro aguas frente a las españolas de
    // ladrillo y azotea (Colonia del Sacramento) [F, guía §4.1]; alero largo y colores fuertes
    // [E]. Los azulejos de fachada son del siglo XIX: quedan para las épocas siguientes.
    Rasgos { tradicion: Tradicion::Lusa, azotea: 0.45, colores: [0.0, 0.0, 0.1, 0.05, 0.08, 0.05, 0.0], alero: 0.8, piedra_vista: 0.1, texto: "techo de teja con alero largo y colores fuertes", marca: "F/E", ..SIN },
    // Corredores (galerías) de las casas de los pueblos de misiones [E: propio de las
    // misiones, no verificado en Montevideo].
    Rasgos { tradicion: Tradicion::Guarani, galeria: 0.5, texto: "galería o enramada al frente", marca: "E", ..SIN },
    // Molduras, cornisas, balaustradas y colores "pompeyanos": italianizante, desde 1830–1860
    // [E]. En la colonia casi no hubo italianos: no se aplica antes de 1830.
    Rasgos { tradicion: Tradicion::Italiana, desde: 1830, colores: [0.0, 0.0, 0.1, 0.1, 0.1, 0.0, 0.0], ornamento: 0.8, texto: "molduras, cornisas y balaustradas", marca: "E", ..SIN },
    // Franceses y vascofranceses (un 27 % de los europeos de 1860, Pollero y González Demuro
    // [F]): persianas de madera, rejas de hierro y el gris "símil piedra" [E].
    Rasgos { tradicion: Tradicion::Francesa, desde: 1830, colores: [0.0, 0.05, 0.0, 0.0, 0.0, 0.0, 0.25], reja: 0.25, verde: 0.35, ornamento: 0.3, texto: "persianas, rejas de hierro y revoque gris piedra", marca: "E", ..SIN },
];

/// Rasgos que se aplican a una casa de este año (fuera de su rango, ninguno).
pub fn rasgos_de(t: Tradicion, anio: u16) -> &'static Rasgos {
    let r = t.rasgos();
    if anio >= r.desde && anio <= r.hasta { r } else { &RASGOS[0] }
}

/// Material de las paredes de la obra (el de `city::materiales::Pared`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Muro {
    Piedra,
    Ladrillo,
    Adobe,
    Tablas,
    Barro,
}

/// Techo de la obra (el de `city::materiales::Techo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cubierta {
    Teja,
    Azotea,
    Tablas,
    Paja,
    Cuero,
}

/// Con qué se levantó la casa (de la obra real): manda sobre el estilo. Una casa de adobe no
/// tiene portada de piedra, sin ladrillo y cal no hay azotea y los colores van solo sobre el
/// revoque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialesObra {
    pub muro: Muro,
    pub cubierta: Cubierta,
    /// Revocada y blanqueada con cal.
    pub revoque: bool,
    /// Hay hierro para rejas y balcones.
    pub hierro: bool,
}

/// Uso del solar (la zona).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Uso {
    Vivienda,
    Comercio,
}

/// Tipologías de la época (guía §1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tipo {
    /// Toldo o rancho de cuero sobre palos (A, 1724–c.1740).
    RanchoCuero,
    /// Rancho de adobe, terrón o quincha con techo de paja (B).
    Rancho,
    /// Casa de piedra o ladrillo con techo de teja a dos aguas (C).
    CasaTeja,
    /// Casa de azotea de una planta, de patio (D).
    CasaAzotea,
    /// Casa de altos, dos plantas con balcones de hierro (E); muy rara vez, tres.
    CasaAltos,
    /// Quinta extramuros con galería, parral y frutales (L).
    Quinta,
    /// Cuartos de alquiler: casa subdividida o de inquilinato (M).
    Cuartos,
}

impl Tipo {
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::RanchoCuero => "rancho de cuero",
            Tipo::Rancho => "rancho",
            Tipo::CasaTeja => "casa de teja",
            Tipo::CasaAzotea => "casa de azotea",
            Tipo::CasaAltos => "casa de altos",
            Tipo::Quinta => "quinta",
            Tipo::Cuartos => "casa de cuartos de alquiler",
        }
    }
}

/// Ventanas de la planta baja (guía §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reja {
    /// Sin hierro: postigos de madera.
    Postigo,
    /// Reja plana embutida en el vano (la más común).
    Plana,
    /// Reja voladiza, como una jaula (ricos, desde 1780).
    Voladiza,
}

/// Posición de la puerta en la fachada.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Puerta {
    Izquierda,
    Centro,
    Derecha,
    /// Descentrada: con una ventana de un lado y el resto del otro (zaguán colonial típico).
    DescIzquierda,
    DescDerecha,
}

/// Todos los parámetros con que se dibuja una casa. Medidas en vóxeles de diseño (0,5 m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estilo {
    pub tipo: Tipo,
    pub uso: Uso,
    pub anio: u16,
    pub clase: u8,
    /// Nivel de ornamento 0–3 (guía §3).
    pub ornamento: u8,
    // --- Materiales y color -----------------------------------------------------------
    /// Pared principal (cal de color, sillería, ladrillo, adobe, quincha, tablas o cuero).
    pub muro: Mat,
    /// Molduras, marcos y pilastras (en blanco sobre muro de color, o al revés).
    pub moldura: Mat,
    /// Zócalo pintado o de piedra (`None` = sin zócalo).
    pub zocalo: Option<Mat>,
    pub h_zocalo: i32,
    pub cubierta: Cubierta,
    /// Material del techo (teja, paja, cuero, tablas o azotea).
    pub techo: Mat,
    /// Carpintería de puertas y postigos.
    pub carpinteria: Mat,
    /// Hay piedra labrada para portadas, alféizares y balcones.
    pub piedra: bool,
    /// Hay hierro (rejas, balcones, faroles).
    pub hierro: bool,
    /// Lo que asoma donde se cae el revoque.
    pub sustrato: Mat,
    /// Años desde el último blanqueo (0–10): más manchas.
    pub blanqueo: u8,
    /// Revoque caído (0–1).
    pub caido: f32,
    /// Edad de las tejas: 0 nuevas, 1 a medias, 2 viejas con líquenes.
    pub teja_edad: u8,
    // --- Fachada ----------------------------------------------------------------------
    /// Frente edificado (el resto del solar queda con tapia y portón).
    pub ancho: i32,
    /// El tramo libre del frente queda del lado u = 0 (si no, del otro).
    pub libre_izq: bool,
    /// Retiro desde la línea de la calle (ranchos y quintas).
    pub retiro: i32,
    /// Altura del muro de la fachada (planta baja; en ranchos y teja, hasta el alero).
    pub alto: i32,
    /// Altura de cada planta alta.
    pub alto_pa: i32,
    pub pisos: u8,
    /// Pretil (0–2) y su tipo: 0 liso, 1 con pilarcitos, 2 balaustrado.
    pub pretil: i32,
    pub pretil_tipo: u8,
    /// Cornisa: 0 ninguna, 1 una faja, 2 dos molduras, 3 con dentículos.
    pub cornisa: u8,
    /// Vanos del frente, contando la puerta.
    pub vanos: u8,
    pub puerta: Puerta,
    pub simetria: bool,
    pub zaguan: bool,
    pub reja: Reja,
    /// Balcón en la planta alta: 0 ninguno, 1 uno por vano, 2 corrido.
    pub balcon: u8,
    /// Portada: 0 ninguna, 1 marco, 2 pilastras y cornisa, 3 pilastras y frontón.
    pub portada: u8,
    pub gargolas: u8,
    /// Puertas de tienda al frente (comercio).
    pub tienda: u8,
    /// Pulpería: reja de mostrador detrás de la puerta.
    pub mostrador: bool,
    /// Esquina (0 no, 1 del lado u = 0, 2 del otro) y entrada: 0 por el frente, 1 puerta
    /// en la arista con pilar de esquina, 2 ochava.
    pub esquina: u8,
    pub entrada_esquina: u8,
    pub farol: bool,
    pub letrero: bool,
    // --- Planta -----------------------------------------------------------------------
    /// Fondo de la crujía del frente.
    pub fondo: i32,
    pub patios: u8,
    /// Alas alrededor del patio (1 en "C" o medio patio, 2 en "U").
    pub alas: u8,
    pub aljibe: bool,
    pub pozo: bool,
    pub parral: bool,
    pub piso_patio: Mat,
    /// Azotea al frente y galpón de teja al fondo (casas que crecieron por etapas).
    pub fondo_teja: bool,
    pub altillo: bool,
    pub mirador: bool,
    /// Cuartos agregados al fondo con el tiempo (0–3).
    pub ampliaciones: u8,
    /// Casa de teja con la cumbrera perpendicular a la calle (hastial al frente).
    pub hastial: bool,
    /// Casa de teja en "L" (segunda crujía hacia el fondo).
    pub ala_l: bool,
    pub galeria: bool,
    pub chimenea: bool,
    pub enramada: bool,
    pub horno: bool,
    pub corral: bool,
    pub cocina_aparte: bool,
    /// Depósito o galpón al fondo (comercio).
    pub deposito: bool,
    /// Cuartos de alquiler con galería alta (casas de altos de renta).
    pub renta: bool,
    // --- Origen de la familia ---------------------------------------------------------
    pub tradicion: Tradicion,
    /// Balcón de madera en lugar de hierro.
    pub balcon_madera: bool,
    /// Vuelo del alero de teja (1 = 0,5 m; 2 = 1 m).
    pub alero: i32,
    // --- Época 2: fachada neoclásica e italianizante ------------------------------------
    /// Fachada de la Ciudad Nueva (pilastras, cornisa, pretil balaustrado, hierro fundido).
    pub neo: bool,
    /// Pilastras: 0 ninguna, 1 en los extremos, 2 también entre los vanos.
    pub pilastras: u8,
    /// Ventanas altas "de balcón", casi hasta el piso, alineadas con la puerta.
    pub ventana_alta: bool,
    /// Cancel de hierro y vidrio al fondo del zaguán (la puerta de calle queda abierta).
    pub cancel: bool,
    /// Escalón y zaguán de mármol importado.
    pub marmol: bool,
    /// Palmera en el jardín del frente (quintas).
    pub palmera: bool,
    /// Año de la fachada modernizada (0 = la original).
    pub reforma: u16,
    pub seed: u64,
}

/// Períodos de la guía (§6.1).
pub fn periodo(anio: u16) -> u8 {
    match anio {
        0..=1739 => 0,
        1740..=1769 => 1,
        1770..=1799 => 2,
        1800..=1814 => 3,
        _ => 4,
    }
}

/// Sorteo con pesos: devuelve el índice.
fn pesos(r: &mut Rng, w: &[f32]) -> usize {
    let total: f32 = w.iter().map(|x| x.max(0.0)).sum();
    if total <= 0.0 {
        return 0;
    }
    let mut x = r.f() * total;
    for (i, &p) in w.iter().enumerate() {
        let p = p.max(0.0);
        if x < p {
            return i;
        }
        x -= p;
    }
    w.len() - 1
}

/// Colores de cal de la guía (§4.2 y §6.5): blanca, hueso, ocre, rosado, almagre, celeste,
/// gris y piedra vista, con los pesos de cada época [E].
const CALES: [Mat; 7] = [CAL, CAL_HUESO, CAL_OCRE, CAL_ROSA, CAL_ALMAGRE, CAL_AZUL, CAL_GRIS];
/// Colores de la época 2 en el mismo orden (el de los rasgos de cada tradición): blanca,
/// hueso, ocre claro, rosado ("pompeyano"), almagre, celeste y gris piedra
/// (docs/epocas/02-ciudad-nueva.md §11) [E].
const NEO: [Mat; 7] = [CAL, CAL_HUESO, OCRE_CLARO, CAL_ROSA, CAL_ALMAGRE, CAL_AZUL, GRIS_PIEDRA];

fn color_de_cal(r: &mut Rng, anio: u16, clase: u8, ra: &Rasgos) -> Mat {
    let mut w: [f32; 7] = match anio {
        0..=1759 => [0.55, 0.15, 0.05, 0.03, 0.02, 0.0, 0.0],
        1760..=1799 => [0.50, 0.15, 0.10, 0.08, 0.04, 0.03, 0.03],
        _ => [0.40, 0.15, 0.12, 0.12, 0.05, 0.06, 0.05],
    };
    match clase {
        // Los pobres blanquean con cal sola; el añil era caro.
        0 => {
            w[0] += 0.15;
            w[5] = 0.0;
        }
        2 | 3 => {
            w[2] += 0.05;
            w[3] += 0.05;
            if w[5] > 0.0 {
                w[5] += 0.03; // el añil, solo cuando ya se usaba (desde 1760)
            }
        }
        _ => {}
    }
    for (i, (a, b)) in w.iter_mut().zip(ra.colores).enumerate() {
        // El celeste de añil no aparece antes de 1760 (guía §6.5), sea cual sea la familia.
        if i != 5 || anio >= 1760 {
            *a += b;
        }
    }
    if clase == 0 {
        w[5] = 0.0;
    }
    CALES[pesos(r, &w)]
}

/// Variante manchada de cada color de muro (lo que usa el desgaste).
pub fn sucio(m: Mat) -> Mat {
    match m {
        CAL => CAL_SUCIA,
        CAL_OCRE => OCRE_SUCIA,
        CAL_ROSA => ROSA_SUCIA,
        CAL_AZUL => AZUL_SUCIA,
        CAL_HUESO => HUESO_SUCIA,
        CAL_ALMAGRE => ALMAGRE_SUCIA,
        CAL_GRIS => GRIS_SUCIA,
        OCRE_CLARO => OCRE_CLARO_SUCIA,
        GRIS_PIEDRA => GRIS_PIEDRA_SUCIA,
        ADOBE => ADOBE_DARK,
        BRICK => BRICK_DARK,
        STONE => STONE_DARK,
        QUINCHA => QUINCHA_DARK,
        MAMPOSTERIA => MAMPOSTERIA_DARK,
        TABLAS => TABLAS_GRIS,
        m => m,
    }
}

/// ¿Es un revoque de cal (limpio o manchado)?
pub fn es_cal(m: Mat) -> bool {
    matches!(m, CAL | CAL_OCRE | CAL_ROSA | CAL_AZUL | CAL_HUESO | CAL_ALMAGRE | CAL_GRIS | CAL_SUCIA | OCRE_SUCIA | ROSA_SUCIA | AZUL_SUCIA | HUESO_SUCIA | ALMAGRE_SUCIA | GRIS_SUCIA | OCRE_CLARO | OCRE_CLARO_SUCIA | GRIS_PIEDRA | GRIS_PIEDRA_SUCIA)
}

/// El color limpio de un revoque manchado.
pub fn limpio(m: Mat) -> Mat {
    match m {
        CAL_SUCIA => CAL,
        OCRE_SUCIA => CAL_OCRE,
        ROSA_SUCIA => CAL_ROSA,
        AZUL_SUCIA => CAL_AZUL,
        HUESO_SUCIA => CAL_HUESO,
        ALMAGRE_SUCIA => CAL_ALMAGRE,
        GRIS_SUCIA => CAL_GRIS,
        OCRE_CLARO_SUCIA => OCRE_CLARO,
        GRIS_PIEDRA_SUCIA => GRIS_PIEDRA,
        m => m,
    }
}

/// Probabilidad de que un hogar sea de cada clase según el nivel de la casa, cuando todavía
/// no vive nadie (casa nueva) [E].
pub fn clase_probable(nivel: u8, valor: f32, seed: u64) -> u8 {
    let mut r = Rng::new(seed ^ 0xC1A5E);
    let x = r.f() * 0.6 + valor.clamp(0.0, 1.0) * 0.4;
    match nivel {
        0 | 1 => u8::from(x > 0.85),
        2 => if x < 0.25 { 0 } else if x < 0.75 { 1 } else { 2 },
        _ => if x < 0.15 { 1 } else if x < 0.65 { 2 } else { 3 },
    }
}

/// Año supuesto para una obra sin contexto (partidas viejas): la época temprana.
const ANIO_SIN_DATO: u16 = 1750;

/// Elige la tipología y todos los parámetros de una casa. Determinista.
pub fn estilo(ctx: &Contexto, m_real: &MaterialesObra, uso: Uso, nivel: u8, alta: bool, seed: u64) -> Estilo {
    // La casa reformada se dibuja con los materiales con que se levantó; la fachada nueva, con
    // los de ahora.
    let m_antes = MaterialesObra { revoque: m_real.revoque && ctx.antes & 1 == 0, hierro: m_real.hierro && ctx.antes & 2 == 0, ..*m_real };
    let m = if ctx.reforma > 0 && ctx.epoca == 0 { &m_antes } else { m_real };
    let anio = if ctx.anio == 0 { ANIO_SIN_DATO } else { ctx.anio };
    // Las casas del kit de la época 2 siguen las reglas de su tiempo aunque la partida (en
    // historia alternativa) llegue antes a la Ciudad Nueva.
    let anio = if ctx.epoca >= 1 { anio.max(ANIO_CIUDAD_NUEVA) } else { anio };
    let per = periodo(anio);
    let s = seed ^ (ctx.giro as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xE57110;
    let mut r = Rng::new(s);
    let clase = ctx.clase.min(3);
    let tradicion = Tradicion::de_indice(ctx.tradicion);
    let ra = rasgos_de(tradicion, anio);
    let extramuros = ctx.lugar >= 3;
    let centro = ctx.lugar == 0;
    let cal = m.revoque && !matches!(m.muro, Muro::Tablas);
    let dos_pisos_posible = matches!(m.muro, Muro::Piedra | Muro::Ladrillo) && m.revoque;

    // 1. Tipología: el nivel y los materiales mandan; la época y el lugar eligen dentro.
    let tipo = match (uso, nivel) {
        (Uso::Vivienda, 0 | 1) => match m.cubierta {
            Cubierta::Cuero => Tipo::RanchoCuero,
            _ => Tipo::Rancho,
        },
        (Uso::Vivienda, 2) => match m.cubierta {
            Cubierta::Paja | Cubierta::Cuero => Tipo::Rancho,
            // Cuartos de alquiler: fines de la colonia [E, guía M].
            _ if alta && anio >= 1770 => Tipo::Cuartos,
            // Quintas extramuros desde 1770 (Vidal 1816–18 [F]; pesos [E]).
            _ if extramuros && anio >= 1770 && r.f() < 0.4 => Tipo::Quinta,
            Cubierta::Azotea => Tipo::CasaAzotea,
            _ => Tipo::CasaTeja,
        },
        (Uso::Vivienda, _) => {
            // Las casas de altos empiezan a fines del XVIII [F]; antes, la casa rica es
            // grande y de una planta (dos patios).
            let p_altos = match anio {
                0..=1779 => 0.0,
                1780..=1799 => 0.45,
                1800..=1814 => 0.6,
                _ => 0.7,
            } + if centro && anio >= 1780 { 0.15 } else { 0.0 };
            if matches!(m.cubierta, Cubierta::Paja | Cubierta::Cuero) {
                Tipo::Rancho
            } else if extramuros && anio >= 1770 && r.f() < 0.55 {
                Tipo::Quinta
            } else if dos_pisos_posible && (alta && anio >= 1780 || r.f() < p_altos) {
                Tipo::CasaAltos
            } else if m.cubierta == Cubierta::Azotea {
                Tipo::CasaAzotea
            } else {
                Tipo::CasaTeja
            }
        }
        (Uso::Comercio, n) => {
            let p_altos = if n >= 3 && anio >= 1780 { 0.8 } else { 0.0 };
            if dos_pisos_posible && r.f() < p_altos {
                Tipo::CasaAltos
            } else if m.cubierta == Cubierta::Azotea {
                Tipo::CasaAzotea
            } else if matches!(m.cubierta, Cubierta::Paja | Cubierta::Cuero) && n <= 1 {
                // Pulpería de campaña: un rancho con mostrador (Gronardo, 1726 [F]).
                Tipo::Rancho
            } else {
                Tipo::CasaTeja
            }
        }
    };

    // 2. Ornamento por clase y época (guía §6.4), con el tope de los materiales.
    let (lo, hi) = match (clase, anio) {
        (0, 0..=1799) => (0, 0),
        (0, _) => (0, 1),
        (1, 0..=1769) => (0, 1),
        (1, _) => (1, 2),
        (2, 0..=1769) => (1, 1),
        (2, _) => (1, 2),
        (_, 0..=1769) => (1, 2),
        _ => (2, 3),
    };
    let mut ornamento = r.range(lo, hi) as u8;
    if ra.ornamento > 0.0 && r.f() < ra.ornamento {
        ornamento = (ornamento + 1).min(3);
    }
    if !cal && !matches!(m.muro, Muro::Piedra | Muro::Ladrillo) {
        ornamento = 0; // adobe, tablas o barro sin revoque: sin molduras
    } else if !cal {
        ornamento = ornamento.min(1);
    }
    if matches!(tipo, Tipo::Rancho | Tipo::RanchoCuero) {
        ornamento = 0;
    }

    // 3. Materiales y color.
    let piedra = matches!(m.muro, Muro::Piedra) && m.revoque;
    let muro = match (m.muro, cal) {
        // Piedra revocada (60 %) o vista (15 %), según la guía (§6.3, casa de teja) [E].
        (Muro::Piedra, true) if r.f() < 0.2 + ra.piedra_vista + if per == 0 { 0.1 } else { 0.0 } => STONE,
        (Muro::Piedra, false) => MAMPOSTERIA,
        (Muro::Ladrillo, false) => BRICK,
        (Muro::Adobe, false) => ADOBE,
        (Muro::Barro, false) => QUINCHA,
        (Muro::Tablas, _) => TABLAS,
        _ => color_de_cal(&mut r, anio, clase, ra),
    };
    let muro = if tipo == Tipo::RanchoCuero { HIDE } else { muro };
    let sustrato = match m.muro {
        Muro::Piedra => MAMPOSTERIA,
        Muro::Ladrillo => BRICK,
        Muro::Adobe => ADOBE,
        Muro::Barro => QUINCHA,
        Muro::Tablas => TABLAS,
    };
    // Molduras: en blanco sobre muro de color, o el mismo color; en piedra, sillería.
    let moldura = if !es_cal(muro) {
        muro
    } else if muro != CAL && r.f() < 0.6 {
        CAL
    } else if muro == CAL && r.f() < 0.25 {
        CAL_HUESO
    } else {
        muro
    };
    // Zócalo en el 60 % de las casas de material (§4.2) [E].
    let zocalo = if !cal || matches!(tipo, Tipo::Rancho | Tipo::RanchoCuero) || r.f() >= 0.6 {
        None
    } else {
        let mut w = [0.3, 0.3, 0.15, 0.25, if piedra { 0.4 } else { 0.0 }];
        if clase == 0 {
            w[0] = 0.0;
        }
        Some([ZOCALO_ALMAGRE, ZOCALO, ZOCALO_OCRE, sucio(muro), STONE][pesos(&mut r, &w)])
    };
    let h_zocalo = r.range(1, 2);
    // Carpintería (§4.2) [E]: el verde inglés crece después de 1810.
    let carpinteria = if clase == 0 || !cal {
        if r.f() < 0.6 { WOOD_DARK } else { WOOD }
    } else {
        let verde = if anio >= 1810 { 0.3 } else { 0.2 } + ra.verde;
        [WOOD_DARK, SHUTTER, CARP_ROJIZA, CARP_GRIS, CARP_OCRE][pesos(&mut r, &[0.5, verde, 0.15, 0.10, 0.05])]
    };
    let cubierta = match tipo {
        Tipo::RanchoCuero => Cubierta::Cuero,
        _ => m.cubierta,
    };
    let techo = match cubierta {
        Cubierta::Teja => TILE,
        Cubierta::Azotea => AZOTEA,
        Cubierta::Paja => THATCH,
        Cubierta::Cuero => HIDE,
        Cubierta::Tablas => TABLAS,
    };
    // Mantenimiento por clase (§6.2): años desde el blanqueo y revoque caído.
    let blanqueo = match clase {
        0 => r.range(3, 10),
        1 => r.range(1, 7),
        _ => r.range(0, 4),
    } as u8;
    let caido = if cal && !piedra || cal && muro != STONE {
        let p = [0.5, 0.25, 0.12, 0.08][clase as usize];
        if r.f() < p { 0.3 + r.f() * 0.7 } else { 0.0 }
    } else {
        0.0
    };
    let teja_edad = pesos(&mut r, &[0.25, 0.5, 0.25]) as u8;

    // 4. Fachada y planta según la tipología.
    let mut e = Estilo {
        tipo,
        uso,
        anio,
        clase,
        ornamento,
        muro,
        moldura,
        zocalo,
        h_zocalo,
        cubierta,
        techo,
        carpinteria,
        piedra,
        hierro: m.hierro,
        sustrato,
        blanqueo,
        caido,
        teja_edad,
        ancho: 24,
        libre_izq: r.f() < 0.5,
        retiro: 0,
        alto: 9,
        alto_pa: 8,
        pisos: 1,
        pretil: 0,
        pretil_tipo: 0,
        cornisa: 0,
        vanos: 3,
        puerta: Puerta::Centro,
        simetria: false,
        zaguan: false,
        reja: Reja::Postigo,
        balcon: 0,
        portada: 0,
        gargolas: 0,
        tienda: 0,
        mostrador: false,
        esquina: ctx.esquina.min(2),
        entrada_esquina: 0,
        farol: false,
        letrero: false,
        fondo: 14,
        patios: 1,
        alas: 1,
        aljibe: false,
        pozo: false,
        parral: false,
        piso_patio: PLAZA,
        fondo_teja: false,
        altillo: false,
        mirador: false,
        ampliaciones: 0,
        hastial: false,
        ala_l: false,
        galeria: false,
        chimenea: false,
        enramada: false,
        horno: false,
        corral: false,
        cocina_aparte: false,
        deposito: false,
        renta: false,
        tradicion,
        balcon_madera: false,
        alero: 1 + i32::from(r.f() < ra.alero),
        neo: false,
        pilastras: 0,
        ventana_alta: false,
        cancel: false,
        marmol: false,
        palmera: false,
        reforma: 0,
        seed: s,
    };
    // Rejas por clase (§6.4) [E]; sin hierro, postigos.
    let mut reja_w: [f32; 3] = match clase {
        0 => [0.75, 0.25, 0.0],
        1 => [0.15, 0.75, 0.10],
        _ => [0.0, 0.5, 0.5],
    };
    reja_w[0] = (reja_w[0] - ra.reja).max(0.0);
    reja_w[1] += ra.reja;
    reja_w[2] += ra.voladiza;
    e.reja = if m.hierro { [Reja::Postigo, Reja::Plana, Reja::Voladiza][pesos(&mut r, &reja_w)] } else { Reja::Postigo };
    if e.reja == Reja::Voladiza && anio < 1780 {
        e.reja = Reja::Plana;
    }
    let puerta_desc = |r: &mut Rng, w: [f32; 3]| -> Puerta {
        match pesos(r, &w) {
            0 => Puerta::Centro,
            1 => if r.f() < 0.5 { Puerta::DescIzquierda } else { Puerta::DescDerecha },
            _ => if r.f() < 0.5 { Puerta::Izquierda } else { Puerta::Derecha },
        }
    };
    let patios_de = |r: &mut Rng| -> u8 {
        match clase {
            0 => r.range(0, 1) as u8,
            1 => r.range(1, 2) as u8,
            _ => 2,
        }
    };
    match tipo {
        Tipo::RanchoCuero => {
            e.ancho = r.range(8, 12);
            e.fondo = r.range(7, 9);
            e.alto = 4;
            e.retiro = r.range(2, 8);
            e.vanos = 1;
            e.corral = r.f() < if extramuros { 0.6 } else { 0.2 };
            e.horno = r.f() < 0.2;
        }
        Tipo::Rancho => {
            // Largo 4–15 m, ancho 3,5–5 m, alero a 2–2,6 m (§6.3) [E].
            let piezas = if nivel >= 2 { r.range(2, 3) } else { r.range(1, 3) };
            e.ancho = (6 + piezas * 5 + r.range(-1, 1)).min(22);
            e.fondo = r.range(7, 10);
            e.alto = r.range(4, 5);
            e.hastial = r.f() < 0.2;
            e.retiro = if extramuros { r.range(4, 12) } else if r.f() < 0.5 { 0 } else { r.range(2, 8) };
            e.vanos = (1 + piezas.min(2)) as u8;
            e.puerta = puerta_desc(&mut r, [0.3, 0.5, 0.2]);
            e.enramada = r.f() < 0.3 + ra.galeria;
            e.horno = r.f() < 0.25;
            e.corral = r.f() < if extramuros { 0.6 } else { 0.1 };
            e.cocina_aparte = r.f() < 0.3;
            if uso == Uso::Comercio {
                e.mostrador = true;
                e.enramada = true;
                e.retiro = 0;
            }
        }
        Tipo::CasaTeja => {
            // Frente 6,5–14 m, alero a 3,5–4,5 m, 2–4 vanos (§6.3) [E].
            e.ancho = if clase == 0 && !centro { r.range(14, 22) } else if r.f() < 0.75 { 24 } else { r.range(18, 22) };
            e.fondo = r.range(11, 14);
            e.alto = r.range(7, 9);
            e.hastial = r.f() < 0.25;
            e.vanos = r.range(2, 4) as u8;
            e.puerta = puerta_desc(&mut r, [0.35, 0.55, 0.10]);
            e.ala_l = clase >= 1 && r.f() < 0.55;
            e.galeria = r.f() < 0.05 + ra.galeria;
            e.patios = patios_de(&mut r).min(1);
            e.pozo = r.f() < 0.35;
            e.piso_patio = if clase >= 2 { PLAZA } else if r.f() < 0.5 { BRICK } else { DIRT };
            e.chimenea = r.f() < 0.25;
            e.zaguan = clase >= 2 && r.f() < 0.5;
            e.ampliaciones = r.range(0, 2) as u8;
            e.cornisa = if ornamento >= 2 { 1 } else { 0 };
        }
        Tipo::CasaAzotea | Tipo::Cuartos => {
            // Frente 8–17 m (casi siempre el solar entero entre medianeras), fachada de
            // 4,5–5,5 m, pretil de 0,8–1,2 m, 1–3 gárgolas, 2–5 vanos (§6.3) [E].
            e.ancho = if r.f() < 0.85 || centro { 24 } else { r.range(18, 22) };
            e.fondo = r.range(12, 15);
            e.alto = r.range(9, 11);
            e.pretil = if r.f() < 0.7 { 2 } else { 1 };
            e.pretil_tipo = if ornamento >= 2 { pesos(&mut r, &[0.5, 0.3, 0.2]) as u8 } else { pesos(&mut r, &[0.8, 0.2, 0.0]) as u8 };
            e.gargolas = r.range(1, 3) as u8;
            e.vanos = r.range(2, 5) as u8;
            e.puerta = puerta_desc(&mut r, [0.40, 0.50, 0.10]);
            e.simetria = anio >= 1800 && r.f() < 0.5;
            e.zaguan = r.f() < 0.8;
            e.patios = patios_de(&mut r).max(1);
            e.alas = if clase >= 2 && r.f() < 0.5 - ra.medio_patio { 2 } else { 1 };
            // Aljibes desde fines del XVIII (Mata et al. 2020 [F]; probabilidad [E]).
            e.aljibe = r.f() < match per { 0 | 1 => 0.0, 2 => 0.25, _ => 0.6 };
            e.pozo = !e.aljibe && r.f() < 0.3;
            e.parral = r.f() < 0.25;
            e.piso_patio = if clase >= 2 { PLAZA } else if r.f() < 0.6 { BRICK } else { DIRT };
            e.fondo_teja = r.f() < 0.35;
            e.altillo = r.f() < match per { 0 | 1 => 0.0, 2 => 0.03, 3 => 0.08, _ => 0.15 } * if clase >= 1 { 1.5 } else { 0.5 };
            // Miradores: el primero documentado es de 1848 [F]; la guía estima pocos antes
            // de 1830. Acá, la mitad de esa estimación y nunca antes de 1790 [E].
            e.mirador = anio >= 1790 && clase >= 2 && r.f() < match per { 2 => 0.005, 3 => 0.015, _ => 0.035 } * if ctx.puerto || centro { 1.5 } else { 1.0 };
            e.chimenea = r.f() < 0.3;
            e.ampliaciones = r.range(0, 2) as u8;
            e.cornisa = match ornamento {
                0 => 0,
                1 => 1,
                2 => r.range(1, 2) as u8,
                _ => 3,
            };
            if tipo == Tipo::Cuartos {
                e.vanos = r.range(3, 4) as u8;
                e.renta = r.f() < 0.5; // fila de piezas al fondo (inquilinato)
                e.zaguan = e.renta;
                e.patios = 1;
                e.altillo = false;
                e.mirador = false;
                e.aljibe = e.aljibe && r.f() < 0.5;
            }
        }
        Tipo::CasaAltos => {
            // PB 4,5–5 m, PA 4–4,5 m, 3–6 vanos por planta, simetría neoclásica creciente,
            // balcones (ninguno 10 %, uno por vano 55 %, corrido 35 %) (§6.3) [E].
            e.ancho = 24;
            e.pisos = if anio >= 1800 && clase >= 3 && r.f() < if centro { 0.03 } else { 0.005 } { 3 } else { 2 };
            e.fondo = r.range(14, 18);
            e.alto = r.range(9, 10);
            e.alto_pa = r.range(8, 9);
            e.pretil = 2;
            e.pretil_tipo = pesos(&mut r, &[0.4, 0.35, 0.25]) as u8;
            e.gargolas = r.range(1, 3) as u8;
            e.vanos = r.range(3, 5) as u8;
            e.simetria = r.f() < if anio < 1800 { 0.45 } else { 0.75 };
            e.puerta = if e.simetria { Puerta::Centro } else { puerta_desc(&mut r, [0.2, 0.6, 0.2]) };
            e.zaguan = true;
            e.balcon_madera = r.f() < ra.balcon_madera || !m.hierro && r.f() < 0.5;
            e.balcon = if m.hierro || e.balcon_madera { pesos(&mut r, &[0.10, 0.55, 0.35]) as u8 } else { 0 };
            e.portada = if r.f() < 0.8 { ornamento.max(1) } else { 0 };
            e.patios = 2;
            e.alas = if r.f() < 0.4 - ra.medio_patio { 2 } else { 1 };
            e.aljibe = r.f() < if per >= 3 { 0.7 } else { 0.4 };
            e.parral = r.f() < 0.2;
            e.piso_patio = if r.f() < 0.8 { PLAZA } else { BRICK };
            e.mirador = anio >= 1790 && r.f() < match per { 2 | 3 => 0.025, _ => 0.075 } * if ctx.puerto || centro { 1.5 } else { 1.0 };
            e.chimenea = r.f() < 0.3;
            e.cornisa = ornamento.clamp(1, 3);
            e.renta = alta && uso == Uso::Vivienda;
            e.ampliaciones = r.range(0, 1) as u8;
            if !m.revoque || matches!(m.cubierta, Cubierta::Teja | Cubierta::Tablas) {
                e.pretil = 0;
                e.gargolas = 0;
            }
        }
        Tipo::Quinta => {
            e.ancho = r.range(16, 22);
            e.fondo = r.range(10, 12);
            e.alto = if cubierta == Cubierta::Azotea { 9 } else { 8 };
            e.retiro = r.range(8, 14);
            e.galeria = true;
            e.vanos = r.range(3, 5) as u8;
            e.puerta = Puerta::Centro;
            e.simetria = true;
            e.pozo = true;
            e.parral = r.f() < 0.8;
            e.horno = r.f() < 0.4;
            e.corral = r.f() < 0.4;
            e.pretil = if cubierta == Cubierta::Azotea { 1 } else { 0 };
            e.piso_patio = BRICK;
            e.libre_izq = r.f() < 0.5;
        }
    }
    // Portada (§6.4): ninguna 50 %, marco 30 %, pilastras y cornisa 15 %, pilastras y
    // frontón 5 % (solo clase alta y desde 1790); nunca más que el ornamento.
    if tipo != Tipo::CasaAltos && ornamento >= 1 {
        let mut w = [0.5, 0.3, 0.15, if clase >= 2 && anio >= 1790 { 0.05 } else { 0.0 }];
        if ornamento >= 2 {
            w[0] = 0.25;
            w[2] += 0.15;
            w[3] *= 3.0;
        }
        e.portada = (pesos(&mut r, &w) as u8).min(ornamento);
    }
    if e.portada >= 2 && !cal && !matches!(m.muro, Muro::Ladrillo) {
        e.portada = 1;
    }
    // Comercio: puertas de tienda, pulpería en la esquina, letrero y depósito.
    if uso == Uso::Comercio {
        e.tienda = if nivel <= 1 { 1 } else { r.range(1, 2) as u8 };
        e.letrero = r.f() < if anio < 1800 { 0.15 } else { 0.6 };
        // Pulpería: casi siempre en la esquina (§6.3: puerta en la esquina 60 %, ochava 15 %).
        e.mostrador = nivel <= 2 && (e.esquina > 0 || r.f() < 0.5);
        e.deposito = nivel >= 2 && (ctx.puerto || r.f() < 0.5);
        if e.tipo == Tipo::CasaAltos {
            // Planta baja: tienda 45 %, almacén 20 %, vivienda 35 % (pero es zona comercial).
            e.tienda = r.range(1, 2) as u8;
        }
    }
    if matches!(tipo, Tipo::Rancho | Tipo::RanchoCuero | Tipo::Quinta) {
        e.esquina = 0; // sueltos en su terreno: la esquina no cambia nada
    }
    if e.esquina > 0 {
        e.entrada_esquina = if uso == Uso::Comercio {
            match pesos(&mut r, &[0.25, 0.6, 0.15]) {
                0 => 0,
                1 => 1,
                _ => 2,
            }
        } else {
            0
        };
        // La esquina va entera hasta la calle lateral.
        e.ancho = 24;
        e.retiro = 0;
        e.libre_izq = e.esquina == 2;
    }
    // Farol en la fachada (§6.4) [E].
    e.farol = m.hierro && r.f() < if anio < 1780 { 0.02 } else { 0.10 } + if e.esquina > 0 { 0.15 } else { 0.0 };
    // Más vanos de los que entran: se recortan en el dibujo; acá se asegura un mínimo.
    e.vanos = e.vanos.max(if e.tienda > 0 { e.tienda + 1 } else { 1 });
    if ctx.epoca >= 1 {
        // Casa nueva de la Ciudad Nueva: fachada y algo de planta de la época 2.
        fachada_neo(&mut e, m, ra, ctx, &mut r, false);
    } else if ctx.reforma > 0 {
        // Casa vieja modernizada en el lugar: la misma planta (todo lo de arriba sale igual),
        // y solo la fachada se rehace con otra tirada.
        let mut r2 = Rng::new(s ^ 0x2EF0_43A ^ (ctx.reforma as u64).wrapping_mul(0x9E37_79B9));
        let ra2 = rasgos_de(tradicion, ctx.reforma.max(ANIO_CIUDAD_NUEVA));
        fachada_neo(&mut e, m_real, ra2, ctx, &mut r2, true);
        if e.neo {
            e.reforma = ctx.reforma;
        }
    }
    e
}

/// Primer año de la Ciudad Nueva: el decreto de demolición de la muralla y el trazado de
/// José María Reyes son de 1829 [F]; la primera Constitución, de 1830.
pub const ANIO_CIUDAD_NUEVA: u16 = 1830;

/// Fachada neoclásica e italianizante de la época 2 (1830–1880): pilastras, cornisa con
/// friso, pretil con balaustres, ventanas altas con rejas de hierro fundido, puerta alta con
/// montante, zaguán con cancel y escalón de mármol; colores ocre claro, gris piedra, hueso y
/// rosado con molduras en blanco piedra (guía: `docs/sistemas/arquitectura-epoca2.md`; las
/// proporciones son [E]). Con `reforma`, solo cambia la cara de la casa (no los vanos, la
/// puerta ni la planta): así se rehabilita sin tocar la huella.
fn fachada_neo(e: &mut Estilo, m: &MaterialesObra, ra: &Rasgos, ctx: &Contexto, r: &mut Rng, reforma: bool) {
    let cal = m.revoque && !matches!(m.muro, Muro::Tablas);
    if !cal || matches!(e.tipo, Tipo::Rancho | Tipo::RanchoCuero) {
        // Sin revoque no hay fachada nueva: el ladrillo, el adobe o la piedra quedan a la vista.
        return;
    }
    e.neo = true;
    e.hierro = m.hierro;
    let clase = e.clase;
    let mut w = [0.10, 0.18, 0.30, 0.12, 0.05, 0.04, 0.21];
    for (a, b) in w.iter_mut().zip(ra.colores) {
        *a += b;
    }
    if clase == 0 {
        w[5] = 0.0;
        w[6] *= 0.5;
    }
    e.muro = NEO[pesos(r, &w)];
    e.sustrato = match m.muro {
        Muro::Piedra => MAMPOSTERIA,
        Muro::Adobe => ADOBE,
        Muro::Barro => QUINCHA,
        _ => BRICK,
    };
    e.moldura = match e.muro {
        CAL => if r.f() < 0.6 { GRIS_PIEDRA } else { CAL_HUESO },
        GRIS_PIEDRA => if r.f() < 0.7 { BLANCO_PIEDRA } else { CAL_HUESO },
        _ => BLANCO_PIEDRA,
    };
    e.piedra = false;
    e.ornamento = match clase {
        0 => 1,
        1 => r.range(1, 2),
        _ => r.range(2, 3),
    } as u8;
    if ra.ornamento > 0.0 && r.f() < ra.ornamento {
        e.ornamento = (e.ornamento + 1).min(3);
    }
    e.cornisa = (e.ornamento + 1).min(3);
    e.pilastras = match clase {
        0 => u8::from(r.f() < 0.4),
        1 => if r.f() < 0.35 { 2 } else { 1 },
        _ => if r.f() < 0.7 { 2 } else { 1 },
    };
    e.zocalo = Some(if e.muro == GRIS_PIEDRA || clase >= 2 && r.f() < 0.5 { MARMOL_GRIS } else { GRIS_PIEDRA_SUCIA });
    e.h_zocalo = 2;
    e.carpinteria = [SHUTTER, WOOD_DARK, CARP_GRIS, CARP_ROJIZA][pesos(r, &[0.5 + ra.verde, 0.25, 0.12, 0.08])];
    e.blanqueo = r.range(0, 2) as u8;
    e.caido = 0.0;
    e.marmol = clase >= 2 || r.f() < 0.3;
    e.ventana_alta = e.alto >= 9 && r.f() < 0.85;
    if m.hierro {
        let mut rw = [0.0, 0.65 + ra.reja, 0.35 + ra.voladiza];
        if clase == 0 {
            rw[2] = 0.1;
        }
        e.reja = [Reja::Postigo, Reja::Plana, Reja::Voladiza][pesos(r, &rw)];
    } else {
        e.reja = Reja::Postigo;
    }
    if e.cubierta == Cubierta::Azotea {
        e.pretil = 2;
        e.pretil_tipo = if r.f() < 0.45 + 0.4 * ra.ornamento + if clase >= 2 { 0.2 } else { 0.0 } { 2 } else { 1 };
        // Caños de bajada en lugar de gárgolas (quedan pocas) [E].
        e.gargolas = u8::from(r.f() < 0.2);
    }
    if e.tipo == Tipo::CasaAltos {
        e.balcon_madera = false;
        e.balcon = if m.hierro { if r.f() < 0.55 { 2 } else { 1 } } else { 0 };
        e.portada = e.portada.max(2);
    } else if e.ornamento >= 2 {
        e.portada = if clase >= 2 && r.f() < 0.4 { 3 } else { 2 };
    } else {
        e.portada = 1;
    }
    e.farol = false;
    if reforma {
        return;
    }
    // Casa nueva: la planta y el ritmo de la época.
    e.simetria = r.f() < 0.75;
    if matches!(e.tipo, Tipo::CasaAzotea | Tipo::CasaAltos | Tipo::Cuartos) {
        e.zaguan = e.tipo != Tipo::Cuartos || e.renta;
        // La "casa de tres vanos": puerta y dos ventanas; las ricas, cinco [E].
        e.vanos = if clase >= 2 && r.f() < 0.4 { 5 } else { 3 };
        if e.simetria {
            e.puerta = Puerta::Centro;
        }
        e.ancho = 24;
        e.libre_izq = false;
        e.ampliaciones = e.ampliaciones.min(1);
    }
    e.cancel = e.zaguan && m.hierro && clase >= 1;
    // Miradores: tema reiterado de la arquitectura montevideana del XIX (MEC) [F]; la
    // frecuencia es [E].
    e.mirador = matches!(e.tipo, Tipo::CasaAzotea | Tipo::CasaAltos) && e.cubierta == Cubierta::Azotea && clase >= 2 && r.f() < 0.18 * if ctx.puerto || ctx.lugar == 0 { 1.5 } else { 1.0 };
    if e.tipo == Tipo::Quinta {
        e.palmera = r.f() < 0.75;
        e.pretil = if e.cubierta == Cubierta::Azotea { 2 } else { 0 };
    }
    if e.uso == Uso::Comercio && e.esquina > 0 {
        // Almacén de esquina con la ochava [E].
        e.entrada_esquina = if r.f() < 0.6 { 2 } else { 1 };
        e.letrero = true;
    }
}

/// Preferencia de techo de una casa de una planta (azotea frente a teja) por año, barrio y
/// tradición de la familia. Guía §6.1: ninguna azotea antes de 1740; ~26 % de las casas
/// hacia 1800 y ~36 % en 1815–30 intramuros (anclas de De María [F]; curvas [E]).
pub fn prob_azotea(anio: u16, lugar: u8, t: Tradicion) -> f32 {
    let base: f32 = match anio {
        0..=1739 => 0.0,
        1740..=1769 => 0.1,
        1770..=1799 => 0.3,
        1800..=1814 => 0.35,
        _ => 0.45,
    };
    let lugar_f = match lugar {
        0 => 1.4,
        1 => 1.0,
        2 => 0.7,
        _ => 0.4,
    };
    (base * lugar_f * rasgos_de(t, anio).azotea).min(0.9)
}

/// Preferencia del cuero sobre la paja en los ranchos. Guía §6.1: rancho de cuero 55 % en
/// 1726–40, 5 % en 1740–70 y después marginal (1729, Cattaneo [F]; curvas [E]).
pub fn prob_cuero(anio: u16) -> f32 {
    match anio {
        0..=1739 => 0.65,
        1740..=1769 => 0.15,
        _ => 0.04,
    }
}

impl Estilo {
    /// Línea de la ficha sobre el origen: "Casa de familia canaria: azotea, balcón de madera…".
    pub fn linea_origen(&self) -> Option<String> {
        let r = rasgos_de(self.tradicion, self.anio);
        (self.tradicion != Tradicion::Ninguna && !r.texto.is_empty()).then(|| {
            let fam = match self.tradicion {
                Tradicion::Criolla => "criolla",
                Tradicion::Canaria => "canaria",
                Tradicion::Gallega => "gallega",
                Tradicion::Andaluza => "andaluza",
                Tradicion::NorteEspana => "del norte de España",
                Tradicion::Lusa => "portuguesa o brasileña",
                Tradicion::Guarani => "guaraní",
                Tradicion::Italiana => "italiana",
                Tradicion::Francesa => "francesa",
                Tradicion::Ninguna => "",
            };
            // Solo los rasgos de la tradición que esta casa muestra de verdad (los materiales
            // mandan: una familia canaria sin cal no tiene azotea).
            let casa = !self.suelto() || self.tipo == Tipo::Quinta;
            let teja = matches!(self.cubierta, Cubierta::Teja);
            let candidatos: &[(bool, &str)] = &[
                (self.tradicion == Tradicion::Criolla && casa && self.alas == 1 && self.patios >= 1, "medio patio"),
                (self.tradicion == Tradicion::Criolla && self.reja == Reja::Voladiza, "rejas voladas"),
                (self.tradicion == Tradicion::Canaria && self.cubierta == Cubierta::Azotea, "azotea"),
                (self.tradicion == Tradicion::Canaria && self.balcon_madera && self.balcon > 0, "balcón de madera"),
                (self.tradicion == Tradicion::Canaria && self.carpinteria == SHUTTER, "carpintería verde"),
                (self.tradicion == Tradicion::Gallega && self.muro == STONE, "piedra a la vista"),
                (matches!(self.tradicion, Tradicion::Gallega | Tradicion::Andaluza) && self.muro == CAL, "cal blanca"),
                (self.tradicion == Tradicion::Andaluza && self.reja != Reja::Postigo, "rejas"),
                (self.tradicion == Tradicion::Andaluza && casa && self.patios >= 1, "patio"),
                (self.tradicion == Tradicion::NorteEspana && self.piedra, "piedra labrada"),
                (matches!(self.tradicion, Tradicion::NorteEspana | Tradicion::Italiana) && self.cornisa >= 2, "cornisas"),
                (self.tradicion == Tradicion::Italiana && self.pretil_tipo == 2, "balaustradas"),
                (self.tradicion == Tradicion::Lusa && teja, "techo de teja"),
                (self.tradicion == Tradicion::Lusa && teja && self.alero >= 2, "alero largo"),
                (self.tradicion == Tradicion::Lusa && matches!(self.muro, CAL_OCRE | CAL_ALMAGRE | CAL_ROSA | CAL_AZUL), "colores fuertes"),
                (self.tradicion == Tradicion::Guarani && self.galeria, "galería al frente"),
                (self.tradicion == Tradicion::Guarani && self.enramada, "enramada"),
                (self.tradicion == Tradicion::Italiana && matches!(self.muro, CAL_ROSA | CAL_ALMAGRE | OCRE_CLARO), "colores pompeyanos"),
                (self.tradicion == Tradicion::Francesa && self.carpinteria == SHUTTER, "persianas"),
                (self.tradicion == Tradicion::Francesa && self.reja != Reja::Postigo, "rejas de hierro"),
                (self.tradicion == Tradicion::Francesa && self.muro == GRIS_PIEDRA, "revoque gris piedra"),
            ];
            let tiene: Vec<&str> = candidatos.iter().filter(|c| c.0).map(|c| c.1).collect();
            if tiene.is_empty() {
                format!("Casa de familia {fam}: sin los rasgos de su tradición ({})", r.texto)
            } else {
                let n = tiene.len();
                let lista = if n == 1 { tiene[0].to_string() } else { format!("{} y {}", tiene[..n - 1].join(", "), tiene[n - 1]) };
                format!("Casa de familia {fam}: {lista}")
            }
        })
    }

    /// Lo que la regla anti-monotonía compara entre vecinos (§6.6): tipología, color de muro,
    /// número de vanos y posición de la puerta.
    pub fn firma(&self) -> (Tipo, Mat, u8, u8) {
        let p = match self.puerta {
            Puerta::Izquierda => 0,
            Puerta::DescIzquierda => 1,
            Puerta::Centro => 2,
            Puerta::DescDerecha => 3,
            Puerta::Derecha => 4,
        };
        (self.tipo, self.muro, self.vanos, p)
    }

    /// Altura de la línea de cornisa (lo más alto del frente), en vóxeles de diseño.
    pub fn linea(&self) -> i32 {
        self.alto + (self.pisos as i32 - 1) * self.alto_pa + self.pretil
    }

    /// Cuántos de los cuatro atributos de la firma comparte con otro estilo.
    pub fn parecido(&self, o: &Estilo) -> u8 {
        let (a, b) = (self.firma(), o.firma());
        u8::from(a.0 == b.0) + u8::from(a.1 == b.1) + u8::from(a.2 == b.2) + u8::from(a.3 == b.3)
    }

    /// ¿Respeta la regla anti-monotonía con el vecino? No comparten más de 2 de los 4
    /// atributos y la cornisa difiere al menos un vóxel.
    pub fn distinto_de(&self, o: &Estilo) -> bool {
        if self.suelto() || o.suelto() {
            // Ranchos y quintas no van entre medianeras: basta con que el volumen difiera
            // (largo, retiro o cumbrera) [E].
            return (self.ancho, self.retiro, self.hastial) != (o.ancho, o.retiro, o.hastial) && (self.tipo != o.tipo || self.ancho != o.ancho || self.alto != o.alto);
        }
        self.parecido(o) <= 2 && self.linea() != o.linea()
    }

    /// Edificio suelto en su terreno (rancho o quinta), no entre medianeras.
    pub fn suelto(&self) -> bool {
        matches!(self.tipo, Tipo::Rancho | Tipo::RanchoCuero | Tipo::Quinta)
    }

    /// Nombre para la ficha: "casa de azotea con zaguán, dos patios y aljibe".
    pub fn nombre(&self) -> String {
        let mut base = match (self.tipo, self.uso) {
            (Tipo::CasaAzotea, Uso::Vivienda) if self.neo && self.reforma == 0 => "casa neoclásica de patio".to_string(),
            (Tipo::CasaAltos, Uso::Vivienda) if self.neo && self.reforma == 0 && !self.renta => "casa de altos neoclásica".to_string(),
            (Tipo::Quinta, _) if self.neo && self.reforma == 0 => "quinta italianizante".to_string(),
            (Tipo::CasaAzotea | Tipo::CasaTeja, Uso::Comercio) if self.neo && self.mostrador && self.esquina > 0 => "almacén de esquina".to_string(),
            (Tipo::CasaAzotea | Tipo::CasaTeja, Uso::Comercio) if self.neo => "casa con almacén al frente".to_string(),
            (Tipo::CasaAltos, _) if self.pisos >= 3 => "casa de tres plantas".to_string(),
            (Tipo::Rancho, Uso::Comercio) => "pulpería de campaña".into(),
            (Tipo::Rancho, _) if self.muro == TABLAS => "rancho de tablas".into(),
            (Tipo::Rancho, _) if self.muro == QUINCHA => "rancho de barro y paja".into(),
            (Tipo::Rancho, _) => format!("rancho de adobe y {}", if self.cubierta == Cubierta::Paja { "paja" } else { "teja" }),
            (Tipo::CasaAzotea | Tipo::CasaTeja, Uso::Comercio) if self.mostrador && self.esquina > 0 => "pulpería de esquina".into(),
            (Tipo::CasaAzotea | Tipo::CasaTeja, Uso::Comercio) if self.mostrador => "pulpería".into(),
            (Tipo::CasaAzotea | Tipo::CasaTeja, Uso::Comercio) => format!("{} con tienda al frente", self.tipo.nombre()),
            (Tipo::CasaAltos, Uso::Comercio) => "casa de altos con tienda".into(),
            (Tipo::CasaAltos, _) if self.renta => "casa de altos de renta".into(),
            (Tipo::CasaTeja | Tipo::Quinta | Tipo::Cuartos, _) if self.muro == TABLAS => format!("{} de tablas", if self.tipo == Tipo::Quinta { "quinta" } else { "casa" }),
            (Tipo::CasaTeja, _) if self.cubierta == Cubierta::Tablas => "casa con techo de tablas".into(),
            (t, _) => t.nombre().into(),
        };
        let mut extras: Vec<&str> = Vec::new();
        if self.zaguan && matches!(self.tipo, Tipo::CasaAzotea | Tipo::CasaAltos | Tipo::CasaTeja) {
            extras.push("zaguán");
        }
        if self.patios >= 2 && !matches!(self.tipo, Tipo::Rancho | Tipo::RanchoCuero | Tipo::Quinta) {
            extras.push("dos patios");
        }
        if self.aljibe {
            extras.push("aljibe");
        }
        if self.galeria {
            extras.push("galería");
        }
        if self.balcon > 0 {
            extras.push(if self.balcon_madera { "balcones de madera" } else { "balcones de hierro" });
        }
        if self.altillo {
            extras.push("altillo");
        }
        if self.mirador {
            extras.push("mirador");
        }
        if self.hastial {
            extras.push("hastial a la calle");
        }
        if self.cancel {
            extras.push("cancel de hierro");
        }
        if self.palmera {
            extras.push("palmera");
        }
        if !extras.is_empty() {
            let n = extras.len();
            let lista = if n == 1 { extras[0].to_string() } else { format!("{} y {}", extras[..n - 1].join(", "), extras[n - 1]) };
            base = format!("{base} con {lista}");
        }
        base
    }
}
