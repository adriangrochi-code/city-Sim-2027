//! Modelo de datos de la población: personas, hogares, orígenes, oficios y la historia de
//! vida de cada uno. Almacenamiento compacto: vectores indexados por id (el id de una persona
//! es su posición en `Poblacion::personas`), sin punteros ni grafos con `Rc`.
//!
//! Representación cuidadosa (decisión de diseño, ver `docs/sistemas/poblacion.md`): las
//! personas africanas y afrodescendientes —esclavizadas o libres— y las indígenas son
//! personas con nombre, familia e historia. La desigualdad legal de la época es una
//! RESTRICCIÓN sobre sus vidas (no pueden tener bienes raíces ni elegir patrón; su trabajo
//! beneficia a otro hogar), nunca una mercancía: ninguna persona tiene precio en el modelo y no
//! existe ninguna función para comprar, vender ni transferir personas.

use serde::{Deserialize, Serialize};

pub type PersonaId = u32;
pub type HogarId = u32;
pub type LoteId = u32;

/// Sin referencia (para campos compactos que no usan `Option`).
pub const NADIE: u32 = u32::MAX;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sexo {
    Hombre,
    Mujer,
}

/// Condición legal. Extensible: el resto de la vida jurídica se agrega como variantes o
/// banderas (el patronato de libertos, colonos y pupilos es la bandera `Persona::patronato`;
/// la capacidad de las mujeres casadas queda pendiente).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Condicion {
    Libre,
    /// Sometida a esclavitud: no puede tener bienes raíces, elegir patrón ni mudarse por su
    /// cuenta; su trabajo beneficia al hogar donde vive. Puede casarse, tener familia, ahorrar
    /// un peculio y obtener la libertad.
    Esclavizada,
    /// Liberada (por carta de libertad, testamento, el fondo del cabildo, las armas o la ley).
    /// Puede seguir un tiempo bajo patronato (`Persona::patronato`).
    Liberta,
}

impl Condicion {
    pub const ALL: [Condicion; 3] = [Condicion::Libre, Condicion::Esclavizada, Condicion::Liberta];
    pub fn nombre(self, s: Sexo) -> &'static str {
        match (self, s) {
            (Condicion::Libre, Sexo::Hombre) => "libre",
            (Condicion::Libre, Sexo::Mujer) => "libre",
            (Condicion::Esclavizada, Sexo::Hombre) => "esclavizado",
            (Condicion::Esclavizada, Sexo::Mujer) => "esclavizada",
            (Condicion::Liberta, Sexo::Hombre) => "liberto",
            (Condicion::Liberta, Sexo::Mujer) => "liberta",
        }
    }
    pub fn plural(self) -> &'static str {
        match self {
            Condicion::Libre => "Libres",
            Condicion::Esclavizada => "Esclavizadas",
            Condicion::Liberta => "Libertas y libertos",
        }
    }
}

/// Cómo se obtuvo la libertad.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViaLibertad {
    /// Juntó su peculio y pagó su carta de libertad.
    Ahorro,
    /// Por el testamento de quien la sometía.
    Testamento,
    /// Con ayuda del fondo de manumisión del cabildo.
    Fondo,
    /// Por la ley de abolición.
    Abolicion,
    /// Nació libre por la ley de libertad de vientres.
    Vientres,
    /// Por servir en las armas: las compañías de pardos y morenos (desde 1760) y las levas de
    /// las guerras (1762, 1807, 1811–1816, 1825–1828, 1842) daban la libertad a quien se
    /// alistaba (Frega et al., 2020, cap. 4 y 5).
    Armas,
    /// Carta de libertad que otorgó en vida quien la sometía ("concesión graciosa", muchas
    /// veces con condiciones).
    Otorgada,
    /// Su familia libre juntó lo que faltaba para la carta (las familias liberaban primero a
    /// la esposa y madre, porque la condición se heredaba por vía materna).
    Familia,
    /// Completó la suma con la colecta de su cofradía o de su sala de nación.
    Colecta,
    /// Quien la sometía murió sin herederos. En la época, una herencia vacante pasaba a la Real
    /// Hacienda (y las personas con ella); el juego no modela ese traspaso, que sería tratar a
    /// alguien como un bien, y la deja libre.
    SinHerederos,
}

/// Raíz familiar, solo para las estadísticas y la endogamia (con mezcla). No se usa para
/// ningún efecto económico por sí sola: las diferencias salen de la condición legal, del
/// origen y de la historia de cada familia.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Raiz {
    Europea,
    Africana,
    Indigena,
    Mixta,
}

/// Lugar o región de nacimiento.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origen {
    /// Nacido en la ciudad.
    Montevideo,
    BuenosAires,
    Canarias,
    Galicia,
    Andalucia,
    Cantabrico,
    Cataluna,
    PaisVasco,
    Portugal,
    Brasil,
    /// Pueblos guaraníes de las misiones.
    Misiones,
    /// Minuanes, charrúas y otros pueblos de la región.
    PueblosRegion,
    /// África occidental (Costa da Mina, golfo de Guinea).
    AfricaOccidental,
    /// África centro-occidental (Congo, Angola, Benguela).
    AfricaCentral,
    /// África oriental (Mozambique).
    AfricaOriental,
    /// Interior de la Banda Oriental / del país.
    Interior,
    Italia,
    Francia,
    Britania,
    Alemania,
    /// Siria y el Líbano.
    Levante,
    /// Comunidades judías de Europa del Este y del Mediterráneo.
    EuropaOriental,
    Armenia,
    Paraguay,
    /// Perú, Venezuela, Cuba, República Dominicana… (época contemporánea).
    Latinoamerica,
}

impl Origen {
    pub const ALL: [Origen; 25] = [
        Origen::Montevideo,
        Origen::BuenosAires,
        Origen::Canarias,
        Origen::Galicia,
        Origen::Andalucia,
        Origen::Cantabrico,
        Origen::Cataluna,
        Origen::PaisVasco,
        Origen::Portugal,
        Origen::Brasil,
        Origen::Misiones,
        Origen::PueblosRegion,
        Origen::AfricaOccidental,
        Origen::AfricaCentral,
        Origen::AfricaOriental,
        Origen::Interior,
        Origen::Italia,
        Origen::Francia,
        Origen::Britania,
        Origen::Alemania,
        Origen::Levante,
        Origen::EuropaOriental,
        Origen::Armenia,
        Origen::Paraguay,
        Origen::Latinoamerica,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Origen::Montevideo => "Montevideo",
            Origen::BuenosAires => "Buenos Aires",
            Origen::Canarias => "las islas Canarias",
            Origen::Galicia => "Galicia",
            Origen::Andalucia => "Andalucía",
            Origen::Cantabrico => "Asturias y Cantabria",
            Origen::Cataluna => "Cataluña",
            Origen::PaisVasco => "el País Vasco",
            Origen::Portugal => "Portugal",
            Origen::Brasil => "el Brasil",
            Origen::Misiones => "los pueblos guaraníes de las misiones",
            Origen::PueblosRegion => "los pueblos minuanes y charrúas",
            Origen::AfricaOccidental => "la Costa da Mina (África occidental)",
            Origen::AfricaCentral => "el Congo y Angola (África centro-occidental)",
            Origen::AfricaOriental => "Mozambique (África oriental)",
            Origen::Interior => "el interior",
            Origen::Italia => "Italia",
            Origen::Francia => "Francia",
            Origen::Britania => "las islas británicas",
            Origen::Alemania => "Alemania y Suiza",
            Origen::Levante => "Siria y el Líbano",
            Origen::EuropaOriental => "las comunidades judías de Europa",
            Origen::Armenia => "Armenia",
            Origen::Paraguay => "el Paraguay",
            Origen::Latinoamerica => "otros países de América Latina",
        }
    }

    /// "de Galicia", "del Brasil", "del Congo y Angola…" (contrae "de el").
    pub fn de(self) -> String {
        let n = self.nombre();
        match n.strip_prefix("el ") {
            Some(r) => format!("del {r}"),
            None => format!("de {n}"),
        }
    }

    /// Nombre corto para tablas.
    pub fn corto(self) -> &'static str {
        match self {
            Origen::Montevideo => "Montevideo",
            Origen::BuenosAires => "Buenos Aires",
            Origen::Canarias => "Canarias",
            Origen::Galicia => "Galicia",
            Origen::Andalucia => "Andalucía",
            Origen::Cantabrico => "Cantábrico",
            Origen::Cataluna => "Cataluña",
            Origen::PaisVasco => "País Vasco",
            Origen::Portugal => "Portugal",
            Origen::Brasil => "Brasil",
            Origen::Misiones => "Misiones guaraníes",
            Origen::PueblosRegion => "Minuanes y charrúas",
            Origen::AfricaOccidental => "África occidental",
            Origen::AfricaCentral => "África centro-occ.",
            Origen::AfricaOriental => "África oriental",
            Origen::Interior => "Interior",
            Origen::Italia => "Italia",
            Origen::Francia => "Francia",
            Origen::Britania => "Islas británicas",
            Origen::Alemania => "Alemania y Suiza",
            Origen::Levante => "Siria y Líbano",
            Origen::EuropaOriental => "Judíos de Europa",
            Origen::Armenia => "Armenia",
            Origen::Paraguay => "Paraguay",
            Origen::Latinoamerica => "América Latina",
        }
    }

    pub fn raiz(self) -> Raiz {
        match self {
            Origen::AfricaOccidental | Origen::AfricaCentral | Origen::AfricaOriental => Raiz::Africana,
            Origen::Misiones | Origen::PueblosRegion => Raiz::Indigena,
            Origen::Montevideo | Origen::BuenosAires | Origen::Interior | Origen::Brasil | Origen::Paraguay | Origen::Latinoamerica => Raiz::Mixta,
            _ => Raiz::Europea,
        }
    }

    /// Cultura de los nombres de pila y apellidos.
    pub fn cultura(self) -> Cultura {
        match self {
            Origen::Portugal | Origen::Brasil => Cultura::Portuguesa,
            Origen::Misiones => Cultura::Guarani,
            Origen::AfricaOccidental | Origen::AfricaCentral | Origen::AfricaOriental => Cultura::Africana,
            Origen::Italia => Cultura::Italiana,
            Origen::Francia => Cultura::Francesa,
            Origen::Britania => Cultura::Britanica,
            Origen::Alemania => Cultura::Alemana,
            Origen::Levante => Cultura::Levantina,
            Origen::EuropaOriental => Cultura::Judia,
            Origen::Armenia => Cultura::Armenia,
            Origen::PaisVasco => Cultura::Vasca,
            Origen::Galicia => Cultura::Gallega,
            Origen::Cataluna => Cultura::Catalana,
            Origen::Canarias => Cultura::Canaria,
            _ => Cultura::Hispana,
        }
    }

    /// Grupo amplio para la endogamia: se casan más dentro del grupo, sin impedir la mezcla.
    pub fn grupo(self) -> u8 {
        match self {
            Origen::Montevideo | Origen::BuenosAires | Origen::Interior | Origen::Paraguay | Origen::Latinoamerica => 0,
            Origen::Canarias | Origen::Galicia | Origen::Andalucia | Origen::Cantabrico | Origen::Cataluna | Origen::PaisVasco => 1,
            Origen::Portugal | Origen::Brasil => 2,
            Origen::Misiones | Origen::PueblosRegion => 3,
            Origen::AfricaOccidental | Origen::AfricaCentral | Origen::AfricaOriental => 4,
            Origen::Italia | Origen::Francia | Origen::Britania | Origen::Alemania => 5,
            Origen::Levante | Origen::Armenia => 6,
            Origen::EuropaOriental => 7,
        }
    }
}

/// Familias de nombres (ver `nombres.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cultura {
    Hispana,
    Canaria,
    Gallega,
    Vasca,
    Catalana,
    Portuguesa,
    Guarani,
    Africana,
    Italiana,
    Francesa,
    Britanica,
    Alemana,
    Levantina,
    Judia,
    Armenia,
}

/// Sector de opinión de la economía al que pesa cada oficio (vecinos, comerciantes,
/// hacendados: el orden de `economy::Sector`).
pub type SectorIdx = usize;

/// Oficio. Los salarios y el lugar de trabajo están en `economia.rs`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Oficio {
    Ninguno,
    /// Peón de campo, de obra o del puerto (jornalero).
    Peon,
    /// Labrador de chacra.
    Labrador,
    /// Estanciero o saladerista.
    Hacendado,
    /// Herrero, carpintero, albañil, zapatero, sastre, costurera…
    Artesano,
    Pulpero,
    Comerciante,
    /// Marinero, lanchero o estibador.
    Marinero,
    Soldado,
    /// Servicio doméstico, lavandera, cocinera, aguador.
    Servicio,
    Clerigo,
    /// Escribano, oficial real, empleado del cabildo.
    Funcionario,
    Maestro,
    // --- Épocas siguientes (ya en el modelo para no cambiar el formato después) ---
    Obrero,
    Empleado,
    Profesional,
}

impl Oficio {
    pub fn nombre(self, s: Sexo) -> &'static str {
        let m = s == Sexo::Mujer;
        match self {
            Oficio::Ninguno => "sin oficio",
            Oficio::Peon => if m { "peona" } else { "peón" },
            Oficio::Labrador => if m { "labradora" } else { "labrador" },
            Oficio::Hacendado => if m { "hacendada" } else { "hacendado" },
            Oficio::Artesano => if m { "costurera" } else { "artesano" },
            Oficio::Pulpero => if m { "pulpera" } else { "pulpero" },
            Oficio::Comerciante => if m { "comerciante" } else { "comerciante" },
            Oficio::Marinero => if m { "vendedora del puerto" } else { "marinero" },
            Oficio::Soldado => "soldado",
            Oficio::Servicio => if m { "lavandera" } else { "criado" },
            Oficio::Clerigo => if m { "religiosa" } else { "clérigo" },
            Oficio::Funcionario => if m { "funcionaria" } else { "escribano" },
            Oficio::Maestro => if m { "maestra" } else { "maestro" },
            Oficio::Obrero => if m { "obrera" } else { "obrero" },
            Oficio::Empleado => if m { "empleada" } else { "empleado" },
            Oficio::Profesional => if m { "profesional" } else { "profesional" },
        }
    }

    pub fn sector(self) -> SectorIdx {
        match self {
            Oficio::Comerciante => 1,
            Oficio::Hacendado => 2,
            _ => 0,
        }
    }
}

/// Clase según la riqueza y el ingreso del hogar (se recalcula cada año).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Clase {
    Pobre,
    Modesta,
    Acomodada,
    Principal,
}

impl Clase {
    pub const ALL: [Clase; 4] = [Clase::Pobre, Clase::Modesta, Clase::Acomodada, Clase::Principal];
    pub fn nombre(self) -> &'static str {
        match self {
            Clase::Pobre => "pobre",
            Clase::Modesta => "modesta",
            Clase::Acomodada => "acomodada",
            Clase::Principal => "principal",
        }
    }
}

/// Estamento de la sociedad colonial: el vecino (cabeza de casa poblada con solar, con voz en
/// el cabildo) frente al morador; militares y clero aparte.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Estamento {
    Vecino,
    Morador,
    Militar,
    Clero,
}

impl Estamento {
    pub fn nombre(self) -> &'static str {
        match self {
            Estamento::Vecino => "vecino",
            Estamento::Morador => "morador",
            Estamento::Militar => "militar",
            Estamento::Clero => "clero",
        }
    }
}

/// Rasgos de personalidad (0 a 255 → 0 a 1). Pesan en las decisiones (trabajar, ahorrar,
/// casarse, emigrar, liberar por testamento) y en qué le importa a cada uno para su felicidad.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rasgos(pub [u8; 5]);

impl Rasgos {
    pub const NOMBRES: [&'static str; 5] = ["laboriosidad", "ambición", "devoción", "sociabilidad", "prudencia"];
    pub fn laboriosidad(&self) -> f32 {
        self.0[0] as f32 / 255.0
    }
    pub fn ambicion(&self) -> f32 {
        self.0[1] as f32 / 255.0
    }
    pub fn devocion(&self) -> f32 {
        self.0[2] as f32 / 255.0
    }
    pub fn sociabilidad(&self) -> f32 {
        self.0[3] as f32 / 255.0
    }
    pub fn prudencia(&self) -> f32 {
        self.0[4] as f32 / 255.0
    }
    pub fn get(&self, i: usize) -> f32 {
        self.0[i] as f32 / 255.0
    }
}

/// Causa de muerte.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Causa {
    Natural,
    Parto,
    Epidemia,
    Incendio,
    Inundacion,
    Hambre,
}

impl Causa {
    pub fn texto(self) -> &'static str {
        match self {
            Causa::Natural => "enfermedad o vejez",
            Causa::Parto => "complicaciones del parto",
            Causa::Epidemia => "la epidemia",
            Causa::Incendio => "un incendio",
            Causa::Inundacion => "la inundación",
            Causa::Hambre => "el hambre",
        }
    }
}

/// Un hecho de la vida de una persona. Compacto: el texto se arma al mostrarlo.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Evento {
    pub mes: i32,
    pub tipo: TipoEvento,
    /// Persona relacionada (cónyuge, hijo, difunto…) o `NADIE`.
    pub con: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum TipoEvento {
    Nacio,
    Llego(Origen),
    /// Llegó en contra de su voluntad, por la trata (lo cuenta la crónica con respeto).
    LlegoForzada(Origen),
    /// Llegó como "colono" africano (1832–1835): la trata disfrazada de contrato de trabajo.
    LlegoColono(Origen),
    /// Llegó prisionera tras la matanza de Salsipuedes (1831) y fue repartida a una familia.
    LlegoPrisionera,
    Caso,
    /// Unión consensual (sin casamiento en la iglesia; frecuente en los sectores populares).
    Union,
    Enviudo,
    NacioHijo,
    MurioHijo,
    MurioPadre,
    Libertad(ViaLibertad),
    TomoApellido,
    Aprendio,
    Oficio(Oficio),
    SeMudo,
    CompraSolar,
    MejoroCasa(u8),
    /// Heredó (pesos).
    Heredo(u32),
    /// Pleito de herencia con sus hermanos.
    Pleito,
    Empobrecio,
    Emigro,
    Murio(Causa),
    /// Queda bajo patronato: libre por ley, pero obligada a servir en una casa a cambio de
    /// techo, comida y ropa hasta la fecha de `Persona::patronato` (reglamento de 1837,
    /// colonos africanos de 1832–1835, menores y no aptos para las armas tras la abolición de
    /// 1842, prisioneros charrúas repartidos en 1831).
    Patronato,
    /// Termina su patronato.
    FinPatronato,
}

/// Una persona. Unos 120 bytes más su historial; las listas cortas viven en vectores propios.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Persona {
    pub id: PersonaId,
    /// Índices en el registro de nombres y apellidos (`Poblacion::textos`).
    pub nombre: u32,
    /// Apellido paterno y materno (costumbre hispana). `NADIE` si todavía no tiene (las
    /// personas llegadas por la trata se registraban sin apellido: lo toman al casarse o al
    /// obtener la libertad).
    pub apellidos: [u32; 2],
    pub sexo: Sexo,
    /// Mes de nacimiento (meses desde enero de 1726; negativo antes).
    pub nacimiento: i32,
    pub muerte: Option<i32>,
    /// Mes en que se fue de la ciudad (emigró o volvió a su tierra).
    #[serde(default)]
    pub fuera: Option<i32>,
    pub padre: Option<PersonaId>,
    pub madre: Option<PersonaId>,
    pub conyuge: Option<PersonaId>,
    /// ¿Casamiento en la iglesia o unión consensual?
    #[serde(default)]
    pub union_formal: bool,
    pub hijos: Vec<PersonaId>,
    pub hogar: Option<HogarId>,
    pub origen: Origen,
    pub raiz: Raiz,
    pub condicion: Condicion,
    pub estamento: Estamento,
    pub oficio: Oficio,
    /// 0 no sabe leer · 1 lee · 2 lee y escribe (primeras letras) · 3 estudios.
    pub educacion: u8,
    /// 0 a 1.
    pub salud: f32,
    /// Riqueza propia en pesos (dote, peculio, herencia personal).
    pub riqueza: f32,
    pub rasgos: Rasgos,
    /// Lugar de trabajo (solar comercial o industrial), si tiene.
    pub trabajo: Option<LoteId>,
    /// Felicidad (0 a 1) y sus motivos principales (ver `felicidad.rs`).
    pub felicidad: f32,
    pub motivos: [u8; 2],
    /// Meses de duelo que le quedan.
    pub duelo: u8,
    pub historial: Vec<Evento>,
    /// Mes hasta el que sigue bajo patronato (libre por ley pero obligada a servir en el
    /// hogar donde vive; no elige patrón ni se muda). `None` si no.
    #[serde(default)]
    pub patronato: Option<i32>,
}

impl Persona {
    /// ¿Vive en la ciudad? (ni murió ni se fue).
    pub fn viva(&self) -> bool {
        self.muerte.is_none() && self.fuera.is_none()
    }
    /// Edad en años cumplidos en el mes dado (o al morir o irse).
    pub fn edad(&self, mes: i32) -> i32 {
        ((self.muerte.or(self.fuera).unwrap_or(mes).min(mes) - self.nacimiento) / 12).max(0)
    }
    pub fn libre(&self) -> bool {
        self.condicion != Condicion::Esclavizada
    }
    /// ¿Vive sujeta a otro hogar? Esclavizada, o liberta bajo patronato: no elige patrón, no
    /// se muda por su cuenta y su trabajo beneficia al hogar donde vive.
    pub fn sujeta(&self) -> bool {
        self.condicion == Condicion::Esclavizada || self.patronato.is_some()
    }
    pub fn evento(&mut self, mes: i32, tipo: TipoEvento, con: u32) {
        self.historial.push(Evento { mes, tipo, con });
    }
}

/// Dónde vive un hogar que no está en un solar de la ciudad.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alojamiento {
    /// En un solar residencial (`Hogar::lote`).
    Solar,
    /// Sin casa: agregados, ranchos precarios en el ejido. Es la demanda de vivienda.
    SinCasa,
    /// Cuartel del fuerte (la guarnición).
    Cuartel,
    /// Campamento de la obra del fuerte (las cuadrillas guaraníes de los primeros años).
    Campamento,
    /// En el campo (estancias y chacras fuera de la ciudad).
    Campo,
}

/// Un hogar: quienes viven juntos y comparten la economía (incluidas, con su condición, las
/// personas esclavizadas que viven en él: su trabajo se suma al ingreso del hogar, pero no son
/// bienes del hogar ni tienen precio).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hogar {
    pub id: HogarId,
    pub activo: bool,
    pub lote: Option<LoteId>,
    pub alojamiento: Alojamiento,
    pub miembros: Vec<PersonaId>,
    pub jefe: PersonaId,
    pub origen: Origen,
    /// Ingresos y gastos del último mes (pesos).
    pub ingresos: f32,
    pub gastos: f32,
    pub ahorro: f32,
    pub deuda: f32,
    /// Solares propios (la casa y los que alquila a otros).
    pub solares: Vec<LoteId>,
    /// Cabezas de ganado (estancias y chacras).
    pub ganado: u32,
    pub clase: Clase,
    /// Desglose del último mes (para la ficha): salarios, rentas, comida, alquiler, impuestos.
    pub detalle: [f32; 6],
    /// Meses seguidos sin poder cubrir la comida.
    pub meses_de_hambre: u8,
    /// Mes en que perdió la casa por un incendio o una inundación (si pasó).
    #[serde(default)]
    pub desastre: Option<i32>,
}

impl Hogar {
    pub fn vive_en_solar(&self) -> bool {
        self.alojamiento == Alojamiento::Solar && self.lote.is_some()
    }
}

/// Textos del desglose del hogar (`Hogar::detalle`).
pub const DETALLE: [&str; 6] = ["Salarios y ganancias", "Alquileres y ganado", "Comida", "Alquiler", "Impuestos", "Otros gastos"];
