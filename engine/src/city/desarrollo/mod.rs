//! Desarrollo: saberes de la población, tecnologías por época, industrias con su perfil
//! (empleo, capital, saber, insumos, exportación), inversión pública y privada con incentivos,
//! y el comercio exterior por producto con precios del mundo que fluctúan. El diseño, las
//! tablas y las fuentes están en `docs/sistemas/desarrollo-y-tecnologia.md`.
//!
//! La idea (pedido del juego): si la educación avanza y se quiere industria, hay que crear una
//! escuela de oficios, invertir en ella y favorecer la inversión en industrias que no
//! necesariamente dan más empleo pero sí exportan, para que entre plata de afuera.
//!
//! Partes:
//! - [`datos`]: tablas (tecnologías, industrias, productos, incentivos, choques de precios);
//! - este módulo: el estado ([`Desarrollo`]) y el mes, lógica pura sobre un [`Contexto`];
//! - [`ciudad`]: cómo se engancha a la ciudad (saberes desde la población, escuelas, peones,
//!   salarios y ganancias a los hogares, edificios que levantan los inversores).
//!
//! Todo el estado se guarda con la partida (campos con `serde(default)`: las partidas viejas
//! empiezan sin desarrollo) y el azar es propio y determinista.

pub mod ciudad;
pub mod datos;
#[cfg(test)]
mod tests;

pub use ciudad::es_de_desarrollo;
pub use datos::*;

use crate::city::economy::Res;
use crate::city::poblacion::Epoca;
use crate::city::services::Kind;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// --- Tipos ----------------------------------------------------------------------------------

/// Saberes de la población (personas que los tienen).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Saber {
    /// Adultos que saben leer (de las escuelas de primeras letras).
    Alfabetizados,
    /// Adultos con oficio de artesano.
    Artesanos,
    /// Oficiales formados (escuela de oficios, aprendizaje en los talleres, maestros de afuera).
    Oficiales,
    /// Pilotos de la escuela de náutica.
    Pilotos,
    /// Técnicos (épocas siguientes).
    Tecnicos,
    /// Ingenieros (épocas siguientes).
    Ingenieros,
}

impl Saber {
    pub const ALL: [Saber; 6] = [Saber::Alfabetizados, Saber::Artesanos, Saber::Oficiales, Saber::Pilotos, Saber::Tecnicos, Saber::Ingenieros];
    pub fn nombre(self) -> &'static str {
        match self {
            Saber::Alfabetizados => "Saben leer",
            Saber::Artesanos => "Artesanos",
            Saber::Oficiales => "Oficiales formados",
            Saber::Pilotos => "Pilotos",
            Saber::Tecnicos => "Técnicos",
            Saber::Ingenieros => "Ingenieros",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Saber::Alfabetizados => "Adultos que leen: salen de las escuelas de primeras letras y del convento. Sin ellos no hay alumnos para las escuelas de oficios ni cajistas para la imprenta.",
            Saber::Artesanos => "Adultos con oficio de artesano (carpinteros, herreros, curtidores, zapateros).",
            Saber::Oficiales => "Oficiales formados: egresados de la escuela de oficios, aprendices de los talleres y alumnos de los maestros de ultramar. Las industrias nuevas los necesitan.",
            Saber::Pilotos => "Pilotos de la escuela de náutica: venden mejor lo que la ciudad embarca y saben de carenas.",
            Saber::Tecnicos => "Técnicos de las escuelas industriales (épocas siguientes).",
            Saber::Ingenieros => "Ingenieros de la facultad (épocas siguientes).",
        }
    }
    pub fn i(self) -> usize {
        self as usize
    }
}

/// Tecnologías (las tablas están en [`datos::TECNOLOGIAS`]).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tec {
    EscuelaOficios,
    CurtidoMejorado,
    HornoMejorado,
    Saladero,
    Graseria,
    Molino,
    Astillero,
    EscuelaNautica,
    Imprenta,
    LanaMerina,
    MolinoVapor,
    Ferrocarril,
    EscuelaArtesOficios,
    Frigorifico,
    Electricidad,
    FacultadIngenieria,
    Utu,
    Textil,
    Refineria,
    Metalurgia,
    Software,
    Celulosa,
    EnergiaEolica,
}

/// Industrias (perfiles en [`datos::INDUSTRIAS`]).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ind {
    Talleres,
    Saladero,
    Molino,
    Astillero,
    Imprenta,
    Frigorifico,
    Textil,
    Software,
    Celulosa,
}

/// Productos del comercio exterior (el orden es el de [`datos::PRODUCTOS`]). Los cuatro
/// primeros son recursos de la economía (los embarca `economy`); los demás, de la industria.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Prod {
    Cueros,
    Sebo,
    Carne,
    Trigo,
    Tasajo,
    Suela,
    Carenas,
    Impresos,
    // Materiales de canteras, hornos y caleras (ver `recursos::materiales`).
    Adoquines,
    Silleria,
    Arena,
    Tejas,
    Cal,
}

pub const NPROD: usize = 13;

impl Prod {
    pub const ALL: [Prod; NPROD] = [
        Prod::Cueros,
        Prod::Sebo,
        Prod::Carne,
        Prod::Trigo,
        Prod::Tasajo,
        Prod::Suela,
        Prod::Carenas,
        Prod::Impresos,
        Prod::Adoquines,
        Prod::Silleria,
        Prod::Arena,
        Prod::Tejas,
        Prod::Cal,
    ];
    pub fn i(self) -> usize {
        self as usize
    }
}

/// Algo que produce una industria: un recurso del mercado de la ciudad o un producto propio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bien {
    Res(Res),
    Prod(Prod),
}

/// Dueño de un establecimiento.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dueno {
    /// Lo levantó el cabildo: la ganancia (o la pérdida) va al presupuesto.
    Cabildo,
    /// Un hogar de la ciudad: la ganancia va a su economía (y a la desigualdad).
    Hogar(u32),
    /// Un comerciante de afuera (Buenos Aires, ultramar): da empleo, pero la ganancia se va.
    Forastero,
}

/// Una industria funcionando en un edificio.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Establecimiento {
    /// Id del edificio de servicio (`services::Building`).
    pub edificio: u32,
    pub ind: Ind,
    pub dueno: Dueno,
    /// Mes en que abrió.
    pub desde: i32,
    /// Peones que trabajan (personas de la población).
    #[serde(default)]
    pub empleados: Vec<u32>,
    /// Lo que debe al crédito de fomento.
    #[serde(default)]
    pub deuda: f32,
    /// Último mes: marcha (0 a 1), ventas y ganancia.
    #[serde(default)]
    pub marcha: f32,
    #[serde(default)]
    pub ventas: f32,
    #[serde(default)]
    pub ganancia: f32,
    /// Por qué no marcha a pleno (texto corto para la ficha).
    #[serde(default)]
    pub falta: String,
    /// Rinde del sitio (recursos del suelo; ver [`rinde_del_sitio`]).
    #[serde(default = "uno")]
    pub sitio: f32,
}

fn uno() -> f32 {
    1.0
}

impl Establecimiento {
    pub fn info(&self) -> &'static IndInfo {
        ind_info(self.ind)
    }
}

/// Una inversión en una tecnología, en marcha.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Proyecto {
    pub tec: Tec,
    pub meses_hechos: u32,
    #[serde(default)]
    pub parado: bool,
    /// Pesos invertidos hasta ahora.
    #[serde(default)]
    pub invertido: i64,
}

/// Arreglos por producto de las partidas guardadas: las viejas tienen menos productos (se
/// completan con `relleno`).
fn por_producto<'de, D: serde::Deserializer<'de>>(d: D, relleno: f32) -> Result<[f32; NPROD], D::Error> {
    let v: Vec<f32> = Vec::deserialize(d)?;
    let mut out = [relleno; NPROD];
    for (o, x) in out.iter_mut().zip(v) {
        *o = x;
    }
    Ok(out)
}
fn por_producto_cero<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[f32; NPROD], D::Error> {
    por_producto(d, 0.0)
}
fn por_producto_uno<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[f32; NPROD], D::Error> {
    por_producto(d, 1.0)
}

/// Foto mensual del comercio y del desarrollo (para los gráficos).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Foto {
    pub mes: i32,
    /// Valor exportado por producto (pesos).
    #[serde(deserialize_with = "por_producto_cero")]
    pub exportado: [f32; NPROD],
    /// Valor importado (pesos).
    pub importado: f32,
    pub hhi: f32,
    /// Inversión privada del mes (pesos) y renuncia fiscal por la exención.
    pub inversion: f32,
    pub renuncia: f32,
    pub saberes: [u32; 6],
    /// Precio de cada producto en el mundo (1,0 = normal).
    #[serde(deserialize_with = "por_producto_uno")]
    pub precios: [f32; NPROD],
}

impl Foto {
    pub fn exportaciones(&self) -> f32 {
        self.exportado.iter().sum()
    }
}

/// Cómo ve un inversor una industria: rinde esperado y los motivos, uno por uno.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Evaluacion {
    pub ind: Ind,
    /// Rinde anual esperado sobre el capital (0,15 = 15 %).
    pub rinde: f32,
    /// (motivo, ¿se cumple?)
    pub motivos: Vec<(String, bool)>,
}

impl Evaluacion {
    pub fn conviene(&self) -> bool {
        self.motivos.iter().all(|m| m.1)
    }
}

/// Lo que el mes del desarrollo le deja a la ciudad (se aplica en la economía y en la
/// población del mismo mes).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Resultado {
    /// Producción extra por recurso (puede ser negativa en los cueros: se vuelven suela).
    pub produccion: [f32; 6],
    pub carne_tomada: f32,
    pub ladrillos_tomados: f32,
    /// Líneas del presupuesto del cabildo.
    pub lineas: Vec<(String, i64)>,
    pub noticias: Vec<String>,
    /// Empujón mensual a la opinión de vecinos, comerciantes y hacendados.
    pub opinion: [f32; 3],
    /// Ganancias (o pérdidas) de los hogares dueños: (hogar, pesos).
    pub ganancias: Vec<(u32, f32)>,
    /// Valor agregado de la suela en los talleres (para los dueños de los solares
    /// industriales).
    pub renta_talleres: f32,
    /// Inversión decidida este mes: industria y dueño.
    pub inversion: Option<(Ind, Dueno)>,
    /// Multiplicador del precio de los cueros en el mundo (choques y pilotos).
    pub factor_cueros: f32,
    /// Cal que la ciudad tuvo que traer por barco (fanegas): la que piden las obras y no dan
    /// las caleras.
    pub cal_importada: f32,
}

/// Lo que la ciudad le cuenta al desarrollo cada mes.
#[derive(Clone, Debug, Default)]
pub struct Contexto {
    pub mes: i32,
    pub epoca: Epoca,
    pub saberes: [u32; 6],
    /// Edificios de servicio construidos, por tipo.
    pub edificios: Vec<(Kind, u32)>,
    pub muelle: bool,
    pub bloqueo: bool,
    /// Derechos de aduana efectivos (tasa × lo que se cobra de verdad, sin el contrabando).
    pub tasa_aduana: f32,
    /// Ganado (carne) que se puede comprar este mes sin dejar sin comer a la ciudad.
    pub carne_disponible: f32,
    /// Precios relativos de los recursos de la ciudad (1,0 = normal).
    pub precios: [f32; 6],
    pub world_hides: f32,
    pub dinero: i64,
    pub ladrillos: f32,
    pub jobs_i: i32,
    /// Ladrillos que producen hornos y talleres este mes (para el horno mejorado).
    pub ladrillos_base: f32,
    pub habitantes: i32,
    /// Riesgo de delito medio (0 a 1).
    pub delito: f32,
    pub opinion: [f32; 3],
    /// Hogares que podrían invertir: (hogar, ahorro), del más rico al menos rico.
    pub inversores: Vec<(u32, f32)>,
    /// Gente sin trabajo que podría tomar un puesto de peón.
    pub desocupados: u32,
    /// Lo que la economía embarcó y trajo el último mes (por recurso).
    pub exportado: [f32; 6],
    pub importado: [f32; 6],
    /// Productos de las canteras, hornos y caleras del mes (adoquines, sillería, arena, tejas,
    /// cal), por producto.
    pub prod_sitios: [f32; NPROD],
    /// Cal que piden las obras de la ciudad este mes (fanegas; morteros de los ladrillos y la
    /// piedra que se usaron).
    pub cal_demanda: f32,
}

impl Contexto {
    pub fn edificios(&self, k: Kind) -> u32 {
        self.edificios.iter().find(|e| e.0 == k).map_or(0, |e| e.1)
    }
}

/// Estado de una tecnología para la interfaz.
#[derive(Clone, Debug, PartialEq)]
pub enum EstadoTec {
    Hecha,
    EnObra { hechos: u32, meses: u32, parado: bool },
    Disponible,
    /// Faltan requisitos (los que no se cumplen).
    Bloqueada(Vec<String>),
    /// Es de una época que todavía no llegó.
    OtraEpoca(Epoca),
}

// --- Estado ---------------------------------------------------------------------------------

/// Salario mensual de un oficial formado (estimación).
pub const SALARIO_OFICIAL: f32 = 16.0;
/// Rinde mínimo para que un inversor ponga su plata (un censo rendía alrededor del 5 %:
/// estimación; se pide el doble por el riesgo).
pub const RINDE_MINIMO: f32 = 0.10;
/// Años de exención para una industria nueva.
pub const MESES_EXENCION: i32 = 120;
/// Manufacturas importadas por habitante y por mes (telas, herramientas, loza: pesos).
pub const MANUFACTURAS_POR_HABITANTE: f32 = 0.25;
/// La cal traída por agua (de Colonia o de ultramar) cuesta el precio del mundo más el flete.
pub const FLETE_CAL: f32 = 1.25;

/// Parte de los productos de canteras y hornos que se embarca (lo demás lo usa la ciudad):
/// estimación; la arena y el canto se exportaban a Buenos Aires (ver
/// `docs/sistemas/recursos-de-uruguay.md`).
pub fn exporta_sitio(p: Prod) -> f32 {
    match p {
        Prod::Arena => 0.8,
        Prod::Adoquines => 0.4,
        Prod::Silleria => 0.3,
        Prod::Tejas => 0.2,
        _ => 0.0,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Desarrollo {
    /// Tecnologías terminadas.
    pub hechas: Vec<Tec>,
    pub proyectos: Vec<Proyecto>,
    pub incentivos: Vec<Incentivo>,
    pub establecimientos: Vec<Establecimiento>,
    /// Personas formadas y su saber (egresados, aprendices, pilotos).
    pub formados: Vec<(u32, Saber)>,
    /// Fracción acumulada de egresados por saber (escuelas y aprendizaje).
    pub acumulado: [f32; 6],
    /// Saberes de la población según el último mes.
    pub saberes: [u32; 6],
    /// Plazas de las escuelas que quedaron vacías el último mes por falta de alumnos que lean.
    pub sin_alumnos: f32,
    /// Precios de los productos en el mundo (1,0 = normal).
    #[serde(deserialize_with = "por_producto_uno")]
    pub precios: [f32; NPROD],
    /// Choque de precios en curso (índice en `CHOQUES`, meses que quedan).
    pub choque: Option<(usize, u32)>,
    /// Últimos 24 meses.
    pub historia: VecDeque<Foto>,
    /// Lo que se exportó el último mes (cantidad por producto).
    #[serde(deserialize_with = "por_producto_cero")]
    pub cantidades: [f32; NPROD],
    /// Cómo ven los inversores cada industria (último mes).
    pub evaluaciones: Vec<Evaluacion>,
    /// Lo que dejó el mes (se aplica en la economía y la población del mismo mes).
    pub pendiente: Resultado,
    /// Totales: inversión privada y renuncia fiscal acumuladas.
    pub inversion_total: f32,
    pub renuncia_total: f32,
    pub azar: u64,
}

impl Default for Desarrollo {
    fn default() -> Self {
        Desarrollo {
            hechas: Vec::new(),
            proyectos: Vec::new(),
            incentivos: Vec::new(),
            establecimientos: Vec::new(),
            formados: Vec::new(),
            acumulado: [0.0; 6],
            saberes: [0; 6],
            sin_alumnos: 0.0,
            precios: [1.0; NPROD],
            choque: None,
            historia: VecDeque::new(),
            cantidades: [0.0; NPROD],
            evaluaciones: Vec::new(),
            pendiente: Resultado::default(),
            inversion_total: 0.0,
            renuncia_total: 0.0,
            azar: 0x0DE5_A220_1726,
        }
    }
}

/// Índice de Herfindahl de las participaciones (0 = muy diversificado, 1 = un solo producto).
pub fn hhi(valores: &[f32]) -> f32 {
    let total: f32 = valores.iter().map(|v| v.max(0.0)).sum();
    if total <= 1e-6 {
        return 0.0;
    }
    valores.iter().map(|v| (v.max(0.0) / total).powi(2)).sum()
}

/// Qué parte del valor exportado se conserva si el precio de un producto cambia por `factor`
/// (con las mismas cantidades): una ciudad que vende un solo producto pierde todo el choque.
#[cfg_attr(not(test), allow(dead_code))]
pub fn conserva_tras_choque(valores: &[f32; NPROD], p: Prod, factor: f32) -> f32 {
    let total: f32 = valores.iter().sum();
    if total <= 1e-6 {
        return 1.0;
    }
    (total - valores[p.i()] * (1.0 - factor)) / total
}

impl Desarrollo {
    // --- Consultas ----------------------------------------------------------------------

    pub fn hecha(&self, t: Tec) -> bool {
        self.hechas.contains(&t)
    }

    pub fn en_obra(&self, t: Tec) -> Option<&Proyecto> {
        self.proyectos.iter().find(|p| p.tec == t)
    }

    pub fn incentivo(&self, i: Incentivo) -> bool {
        self.incentivos.contains(&i)
    }

    pub fn saber(&self, s: Saber) -> u32 {
        self.saberes[s.i()]
    }

    /// Tecnología que habilita un edificio (si es del desarrollo).
    pub fn tec_del_edificio(k: Kind) -> Option<Tec> {
        TECNOLOGIAS.iter().find(|t| t.efectos.contains(&Efecto::Edificio(k))).map(|t| t.id)
    }

    /// ¿Ya se puede construir este edificio? (los que no son del desarrollo, siempre)
    pub fn habilitado(&self, k: Kind) -> bool {
        Self::tec_del_edificio(k).is_none_or(|t| self.hecha(t))
    }

    /// Requisitos de una tecnología con su cumplimiento, en palabras.
    pub fn requisitos(&self, t: Tec, ctx: &Contexto) -> Vec<(String, bool)> {
        let inf = tec_info(t);
        let mut v = Vec::new();
        if inf.epoca > ctx.epoca {
            v.push((format!("Época «{}»", inf.epoca.nombre()), false));
        }
        for r in inf.req {
            v.push(match *r {
                Req::Saber(s, n) => (format!("{}: {} de {}", s.nombre(), ctx.saberes[s.i()].min(n), n), ctx.saberes[s.i()] >= n),
                Req::Edificio(k, n) => (format!("{}: {} de {}", crate::city::services::info(k).name, ctx.edificios(k).min(n), n), ctx.edificios(k) >= n),
                Req::Tec(o) => (format!("Tecnología «{}»", tec_info(o).nombre), self.hecha(o)),
                Req::Muelle => ("Muelle en la costa".to_string(), ctx.muelle),
                Req::Habitantes(n) => (format!("{} habitantes", n), ctx.habitantes as u32 >= n),
            });
        }
        v
    }

    pub fn estado(&self, t: Tec, ctx: &Contexto) -> EstadoTec {
        let inf = tec_info(t);
        if self.hecha(t) {
            return EstadoTec::Hecha;
        }
        if let Some(p) = self.en_obra(t) {
            return EstadoTec::EnObra { hechos: p.meses_hechos, meses: inf.meses, parado: p.parado };
        }
        if inf.epoca > ctx.epoca {
            return EstadoTec::OtraEpoca(inf.epoca);
        }
        let faltan: Vec<String> = self.requisitos(t, ctx).into_iter().filter(|r| !r.1).map(|r| r.0).collect();
        if faltan.is_empty() {
            EstadoTec::Disponible
        } else {
            EstadoTec::Bloqueada(faltan)
        }
    }

    /// Empieza a invertir en una tecnología: hacen falta los requisitos y la primera cuota.
    pub fn iniciar(&mut self, t: Tec, ctx: &Contexto) -> Result<String, String> {
        let inf = tec_info(t);
        match self.estado(t, ctx) {
            EstadoTec::Hecha => return Err(format!("«{}» ya está lograda", inf.nombre)),
            EstadoTec::EnObra { .. } => return Err(format!("«{}» ya está en marcha", inf.nombre)),
            EstadoTec::OtraEpoca(e) => return Err(format!("«{}» llega con la época «{}»", inf.nombre, e.nombre())),
            EstadoTec::Bloqueada(f) => return Err(format!("Para «{}» falta: {}", inf.nombre, f.join(", "))),
            EstadoTec::Disponible => {}
        }
        let cuota = Self::cuota(t);
        if ctx.dinero < cuota {
            return Err(format!("Hacen falta al menos {cuota} pesos para la primera cuota"));
        }
        self.proyectos.push(Proyecto { tec: t, meses_hechos: 0, parado: false, invertido: 0 });
        Ok(format!("El cabildo invierte en «{}»: {} pesos en {} meses", inf.nombre, inf.costo, inf.meses))
    }

    /// Cuota mensual de una tecnología.
    pub fn cuota(t: Tec) -> i64 {
        let inf = tec_info(t);
        (inf.costo as f32 / inf.meses.max(1) as f32).ceil() as i64
    }

    /// Prende o apaga un incentivo. Los aranceles protectores no van con el libre comercio.
    pub fn alternar(&mut self, i: Incentivo, libre_comercio: bool) -> Result<String, String> {
        let nombre = inc_info(i).nombre.to_lowercase();
        if let Some(k) = self.incentivos.iter().position(|&x| x == i) {
            self.incentivos.remove(k);
            return Ok(format!("El cabildo levanta: {nombre}"));
        }
        if i == Incentivo::Arancel && libre_comercio {
            return Err("Con el libre comercio vigente no se pueden poner aranceles protectores".into());
        }
        self.incentivos.push(i);
        Ok(format!("El cabildo dispone: {nombre}"))
    }

    /// Multiplicador de productividad que dan las tecnologías a una industria.
    pub fn multiplicador(&self, ind: Ind) -> f32 {
        self.hechas
            .iter()
            .flat_map(|t| tec_info(*t).efectos.iter())
            .filter_map(|e| match e {
                Efecto::Productividad(i, m) if *i == ind => Some(*m),
                _ => None,
            })
            .product()
    }

    /// Lo que valen las exportaciones afuera según los pilotos propios (hasta +15 %).
    pub fn bono_pilotos(&self) -> f32 {
        1.0 + (self.saber(Saber::Pilotos) as f32 * 0.01).min(0.15)
    }

    /// Precio en el mundo de un producto (pesos por unidad), con choques y pilotos.
    pub fn precio_mundo(&self, p: Prod) -> f32 {
        prod_info(p).precio * self.precios[p.i()] * self.bono_pilotos()
    }

    /// Exportaciones de los últimos 12 meses por producto (pesos).
    pub fn exportado_12(&self) -> [f32; NPROD] {
        let mut v = [0.0; NPROD];
        for f in self.historia.iter().rev().take(12) {
            for k in 0..NPROD {
                v[k] += f.exportado[k];
            }
        }
        v
    }

    /// Índice de concentración de las exportaciones (12 meses).
    pub fn concentracion(&self) -> f32 {
        hhi(&self.exportado_12())
    }

    fn azar(&mut self) -> f32 {
        // xorshift64*: propio y guardado con la partida.
        let mut x = self.azar.max(1);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.azar = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }

    // --- Rinde de una industria ---------------------------------------------------------

    /// Ganancia mensual de un establecimiento a plena marcha, con los precios de hoy: lo que
    /// mira un inversor. Devuelve (ganancia, ventas, salarios).
    pub fn cuenta_plena(&self, ind: Ind, ctx: &Contexto, exento: bool) -> (f32, f32, f32) {
        let inf = ind_info(ind);
        let mult = self.multiplicador(ind);
        let mut ventas = 0.0;
        let mut derechos = 0.0;
        for &(b, q) in inf.produce {
            let q = q * mult * self.extra(ind, b);
            match b {
                Bien::Res(r) => ventas += q * r.base_price() * ctx.precios[r as usize],
                Bien::Prod(p) => {
                    let exp = q * inf.exporta;
                    let v = exp * self.precio_mundo(p) * if ctx.muelle { 1.0 } else { 0.7 };
                    if !exento {
                        derechos += v * ctx.tasa_aduana;
                    }
                    ventas += v + (q - exp) * prod_info(p).precio * self.recargo(inf);
                }
            }
        }
        if self.incentivo(Incentivo::Privilegio) {
            ventas *= 1.15;
        }
        let salarios = inf.empleos as f32 * inf.salario + inf.saber.map_or(0.0, |(_, n)| n as f32 * SALARIO_OFICIAL);
        let insumo = inf.insumo.map_or(0.0, |(r, q)| q * r.base_price() * ctx.precios[r as usize]);
        let otros = Self::gastos_fijos(ind, inf.produce.first().map_or(0.0, |x| x.1 * mult));
        (ventas - derechos - salarios - insumo - otros, ventas, salarios)
    }

    /// Mantenimiento y otros gastos (la sal del saladero, que se importa).
    fn gastos_fijos(ind: Ind, principal: f32) -> f32 {
        let inf = ind_info(ind);
        inf.capital as f32 * 0.004 + if ind == Ind::Saladero { principal * 0.06 } else { 0.0 }
    }

    /// Recargo de precio en la ciudad por los aranceles protectores.
    fn recargo(&self, inf: &IndInfo) -> f32 {
        if inf.mercado_local && self.incentivo(Incentivo::Arancel) { 1.25 } else { 1.0 }
    }

    /// Lo que suma la grasería al sebo del saladero.
    fn extra(&self, ind: Ind, b: Bien) -> f32 {
        if ind == Ind::Saladero && b == Bien::Res(Res::Sebo) && self.hecha(Tec::Graseria) { 2.0 } else { 1.0 }
    }

    /// Cómo ve un inversor cada industria habilitada (con edificio), con sus motivos.
    pub fn evaluar(&self, ctx: &Contexto) -> Vec<Evaluacion> {
        let oficiales_usados: u32 = self.establecimientos.iter().filter_map(|e| e.info().saber).filter(|s| s.0 == Saber::Oficiales).map(|s| s.1).sum();
        let libres = ctx.saberes[Saber::Oficiales.i()].saturating_sub(oficiales_usados);
        // El ganado que ya compran los establecimientos abiertos no está libre para uno nuevo.
        let carne_usada: f32 = self.establecimientos.iter().filter_map(|e| e.info().insumo).filter(|x| x.0 == Res::Carne).map(|x| x.1).sum();
        let carne_libre = (ctx.carne_disponible - carne_usada).max(0.0);
        let mut out = Vec::new();
        for inf in INDUSTRIAS.iter().filter(|i| i.edificio.is_some() && i.epoca <= ctx.epoca) {
            let tec = inf.tec.unwrap();
            if !self.hecha(tec) {
                continue;
            }
            let exento = self.incentivo(Incentivo::Exencion);
            let (g, _, _) = self.cuenta_plena(inf.id, ctx, exento);
            let credito = self.incentivo(Incentivo::Credito);
            let interes = if credito { inf.capital as f32 * 0.5 * 0.04 / 12.0 } else { 0.0 };
            let rinde = (g - interes) * 12.0 / inf.capital as f32;
            let mut m = Vec::new();
            if inf.exporta > 0.0 {
                m.push(("Muelle para embarcar".to_string(), ctx.muelle));
            }
            if let Some((s, n)) = inf.saber {
                let libres = if s == Saber::Oficiales { libres } else { ctx.saberes[s.i()] };
                m.push((format!("{}: {} libres (necesita {})", s.nombre(), libres, n), libres >= n));
            }
            if let Some((r, q)) = inf.insumo {
                m.push((format!("{} libre para comprar: {} (necesita {})", if r == Res::Carne { "Ganado" } else { r.name() }, carne_libre as i32, q as i32), carne_libre >= q * 1.2));
            }
            m.push((format!("Brazos: {} sin trabajo (necesita {})", ctx.desocupados, inf.empleos), ctx.desocupados >= inf.empleos / 2));
            m.push((format!("Seguridad: delito {} %", (ctx.delito * 100.0) as i32), ctx.delito < 0.45));
            m.push((format!("Rinde esperado {} % al año (mínimo {} %)", (rinde * 100.0).round() as i32, (RINDE_MINIMO * 100.0) as i32), rinde >= RINDE_MINIMO));
            let falta = (inf.capital as f32 * if credito { 0.5 } else { 1.0 }) as i64;
            let rico = ctx.inversores.first().map_or(0.0, |x| x.1);
            let forastero = rinde >= 0.18 && ctx.muelle && ctx.opinion[1] >= 35.0;
            m.push((
                if rico >= falta as f32 {
                    format!("Capital: un vecino tiene {} pesos (necesita {}{})", rico as i64, falta, if credito { ", con crédito" } else { "" })
                } else if forastero {
                    format!("Capital: no hay vecino con {} pesos, pero un comerciante de afuera pone la plata", falta)
                } else {
                    format!("Capital: ningún vecino tiene {} pesos{}", falta, if credito { "" } else { " (el crédito de fomento pide la mitad)" })
                },
                rico >= falta as f32 || forastero,
            ));
            if self.incentivo(Incentivo::Privilegio) && self.establecimientos.iter().any(|e| e.ind == inf.id) {
                m.push(("Privilegio exclusivo: ya hay uno".to_string(), false));
            }
            if inf.molestia > 0.5 {
                m.push((format!("Opinión de los vecinos sobre el olor: {}", ctx.opinion[0] as i32), ctx.opinion[0] >= 25.0));
            }
            out.push(Evaluacion { ind: inf.id, rinde, motivos: m });
        }
        out
    }

    /// Anota un establecimiento que acaba de abrir. Devuelve las líneas del presupuesto (el
    /// préstamo del crédito de fomento, si lo hubo).
    pub fn registrar(&mut self, ind: Ind, edificio: u32, dueno: Dueno, mes: i32, credito: bool) -> Vec<(String, i64)> {
        let inf = ind_info(ind);
        let deuda = if credito { inf.capital as f32 * 0.5 } else { 0.0 };
        self.establecimientos.push(Establecimiento { edificio, ind, dueno, desde: mes, empleados: Vec::new(), deuda, marcha: 0.0, ventas: 0.0, ganancia: 0.0, falta: String::new(), sitio: 1.0 });
        if dueno != Dueno::Cabildo {
            self.inversion_total += inf.capital as f32;
        }
        if deuda > 0.0 { vec![("Crédito de fomento: préstamo".into(), -(deuda as i64))] } else { Vec::new() }
    }

    /// Capital que pone el dueño (el resto lo presta el cabildo con el crédito de fomento).
    pub fn capital_propio(&self, ind: Ind, dueno: Dueno) -> f32 {
        let c = ind_info(ind).capital as f32;
        if dueno != Dueno::Forastero && self.incentivo(Incentivo::Credito) { c * 0.5 } else { c }
    }

    // --- El mes ---------------------------------------------------------------------------

    fn precios_mes(&mut self, r: &mut Resultado) {
        // Choque en curso o uno nuevo.
        match self.choque {
            Some((i, m)) if m <= 1 => {
                r.noticias.push(format!("Gaceta: pasó «{}»; los precios vuelven a lo normal.", CHOQUES[i].nombre));
                self.choque = None;
            }
            Some((i, m)) => self.choque = Some((i, m - 1)),
            None => {
                if self.azar() < 0.03 {
                    let i = ((self.azar() * CHOQUES.len() as f32) as usize).min(CHOQUES.len() - 1);
                    self.choque = Some((i, CHOQUES[i].meses));
                    r.noticias.push(CHOQUES[i].noticia.into());
                }
            }
        }
        // Paseo al azar que vuelve a la normalidad, más el choque.
        for k in 0..NPROD {
            let ruido = (self.azar() - 0.5) * 0.06;
            let base = self.precios[k] / self.choque.filter(|c| CHOQUES[c.0].prod.i() == k).map_or(1.0, |c| CHOQUES[c.0].factor);
            let base = (base + (1.0 - base) * 0.15 + ruido).clamp(0.7, 1.3);
            let objetivo = base * self.choque.filter(|c| CHOQUES[c.0].prod.i() == k).map_or(1.0, |c| CHOQUES[c.0].factor);
            self.precios[k] = objetivo;
        }
    }

    fn proyectos_mes(&mut self, ctx: &Contexto, r: &mut Resultado) {
        let mut dinero = ctx.dinero;
        let mut ladrillos = ctx.ladrillos;
        let mut gasto = 0i64;
        let mut terminadas = Vec::new();
        for p in &mut self.proyectos {
            let inf = tec_info(p.tec);
            let cuota = Self::cuota(p.tec);
            if dinero >= cuota && ladrillos >= inf.ladrillos_mes {
                dinero -= cuota;
                ladrillos -= inf.ladrillos_mes;
                r.ladrillos_tomados += inf.ladrillos_mes;
                gasto += cuota;
                p.invertido += cuota;
                p.parado = false;
                p.meses_hechos += 1;
                if p.meses_hechos >= inf.meses {
                    terminadas.push(p.tec);
                }
            } else if !p.parado {
                p.parado = true;
                r.noticias.push(format!("Se para la inversión en «{}»: faltan {}", inf.nombre, if dinero < cuota { "pesos para la cuota" } else { "ladrillos" }));
            }
        }
        if gasto > 0 {
            r.lineas.push(("Inversión en tecnologías".into(), -gasto));
        }
        for t in terminadas {
            self.proyectos.retain(|p| p.tec != t);
            self.hechas.push(t);
            r.noticias.push(format!("Gaceta: ¡lograda «{}»! {}", tec_info(t).nombre, tec_info(t).desc));
        }
    }

    /// Cierra el mes del desarrollo: precios, inversiones del cabildo, producción y cuentas de
    /// cada establecimiento, suela y ladrillos de los talleres, comercio exterior, opinión y
    /// las decisiones de los inversores.
    pub fn mes(&mut self, ctx: &Contexto) -> Resultado {
        let mut r = Resultado { factor_cueros: 1.0, ..Default::default() };
        self.precios_mes(&mut r);
        self.proyectos_mes(ctx, &mut r);
        let mut disp = ctx.saberes;
        let mut carne = ctx.carne_disponible.max(0.0);
        let mut exp_q = [0.0f32; NPROD];
        let mut exp_v = [0.0f32; NPROD];
        let (mut derechos, mut renuncia, mut cabildo, mut cobros, mut sal, mut local_manuf) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let exencion = self.incentivo(Incentivo::Exencion);
        let privilegio = self.incentivo(Incentivo::Privilegio);
        let mut canon = 0.0f32;
        for k in 0..self.establecimientos.len() {
            let e = &self.establecimientos[k];
            let inf = e.info();
            let mut falta = Vec::new();
            let f_saber = match inf.saber {
                Some((s, n)) => {
                    let tiene = disp[s.i()].min(n);
                    disp[s.i()] -= tiene;
                    if tiene < n {
                        falta.push(format!("faltan {} {}", n - tiene, s.nombre().to_lowercase()));
                    }
                    0.4 + 0.6 * tiene as f32 / n.max(1) as f32
                }
                None => 1.0,
            };
            let f_brazos = (e.empleados.len() as f32 / inf.empleos.max(1) as f32).min(1.0);
            if f_brazos < 1.0 {
                falta.push(format!("faltan {} peones", inf.empleos as usize - e.empleados.len().min(inf.empleos as usize)));
            }
            let mut marcha = f_saber * f_brazos * e.sitio;
            let mut costo_insumo = 0.0;
            if let Some((res, q)) = inf.insumo {
                let quiere = q * marcha;
                let hay = quiere.min(carne);
                carne -= hay;
                r.carne_tomada += hay;
                if quiere > 0.0 {
                    if hay < quiere * 0.95 {
                        falta.push("falta ganado".into());
                    }
                    marcha *= hay / quiere;
                }
                costo_insumo = hay * res.base_price() * ctx.precios[res as usize];
            }
            let mult = self.multiplicador(e.ind);
            let exento = exencion && ctx.mes - e.desde < MESES_EXENCION;
            let mut ventas = 0.0;
            let mut der = 0.0;
            for &(b, q) in inf.produce {
                let q = q * marcha * mult * self.extra(e.ind, b);
                match b {
                    Bien::Res(res) => {
                        r.produccion[res as usize] += q;
                        ventas += q * res.base_price() * ctx.precios[res as usize];
                    }
                    Bien::Prod(p) => {
                        let exp = q * inf.exporta;
                        let local = q - exp;
                        let pr = prod_info(p).precio;
                        let valor = if ctx.bloqueo {
                            // Sin barcos se malvende en la ciudad.
                            exp * pr * 0.4
                        } else {
                            let v = exp * self.precio_mundo(p) * if ctx.muelle { 1.0 } else { 0.7 };
                            exp_q[p.i()] += exp;
                            exp_v[p.i()] += v;
                            let d = v * ctx.tasa_aduana;
                            if exento {
                                renuncia += d;
                            } else {
                                der += d;
                            }
                            v
                        };
                        ventas += valor + local * pr * self.recargo(inf);
                        if inf.mercado_local {
                            local_manuf += local * pr;
                        }
                        if e.ind == Ind::Saladero {
                            sal += q * 0.06;
                        }
                    }
                }
            }
            if privilegio && e.dueno != Dueno::Cabildo {
                ventas *= 1.15;
                canon += 25.0;
            }
            let oficiales = inf.saber.map_or(0.0, |(_, n)| n as f32 * (f_saber - 0.4) / 0.6);
            let salarios = e.empleados.len() as f32 * inf.salario + oficiales * SALARIO_OFICIAL;
            let otros = Self::gastos_fijos(e.ind, inf.produce.first().map_or(0.0, |x| x.1 * marcha * mult));
            let mut ganancia = ventas - der - salarios - costo_insumo - otros - if privilegio && e.dueno != Dueno::Cabildo { 25.0 } else { 0.0 };
            derechos += der;
            // Crédito de fomento: cuota de capital (1,5 % por mes) e interés (4 % anual).
            let deuda = self.establecimientos[k].deuda;
            if deuda > 0.5 {
                let cuota = deuda * 0.015 + deuda * 0.04 / 12.0;
                if ganancia > cuota {
                    ganancia -= cuota;
                    cobros += cuota;
                    self.establecimientos[k].deuda -= deuda * 0.015;
                } else if ganancia < 0.0 && self.azar() < 0.04 {
                    self.establecimientos[k].deuda = 0.0;
                    r.noticias.push(format!("El dueño de un {} no puede pagar el crédito de fomento: el cabildo pierde {} pesos", inf.nombre.to_lowercase(), deuda as i64));
                }
            }
            let e = &mut self.establecimientos[k];
            e.marcha = marcha * mult;
            e.ventas = ventas;
            e.ganancia = ganancia;
            e.falta = falta.join(", ");
            match e.dueno {
                Dueno::Cabildo => cabildo += ganancia,
                Dueno::Hogar(h) => r.ganancias.push((h, ganancia)),
                Dueno::Forastero => {}
            }
        }
        // Talleres: suela (curtido mejorado) y ladrillos (horno de bóveda).
        if self.hecha(Tec::CurtidoMejorado) && ctx.jobs_i > 0 {
            let suela = ctx.jobs_i as f32 * 0.25 * self.multiplicador(Ind::Talleres);
            r.produccion[Res::Cueros as usize] -= suela;
            let exp = suela * 0.6;
            let v = if ctx.bloqueo { 0.0 } else { exp * self.precio_mundo(Prod::Suela) };
            if v > 0.0 {
                exp_q[Prod::Suela.i()] += exp;
                exp_v[Prod::Suela.i()] += v;
                derechos += v * ctx.tasa_aduana;
            }
            let local = (suela - exp) * prod_info(Prod::Suela).precio * if self.incentivo(Incentivo::Arancel) { 1.25 } else { 1.0 };
            local_manuf += (suela - exp) * prod_info(Prod::Suela).precio;
            // Valor agregado: la suela vale más que el cuero seco que usa.
            r.renta_talleres = (v * (1.0 - ctx.tasa_aduana) + local - suela * Res::Cueros.base_price() * ctx.precios[Res::Cueros as usize]).max(0.0) * 0.5;
        }
        if self.hecha(Tec::HornoMejorado) {
            r.produccion[Res::Ladrillos as usize] += ctx.ladrillos_base * 0.4;
        }
        // Canteras, hornos y caleras: la ciudad usa lo suyo (la cal, hasta lo que piden las
        // obras; de lo demás, una parte) y lo que sobra se embarca. Lo vende el cabildo, que es
        // el dueño.
        let mut ventas_sitios = 0.0f32;
        for p in Prod::ALL {
            let q = ctx.prod_sitios[p.i()];
            if q <= 0.0 {
                continue;
            }
            let local = if p == Prod::Cal { q.min(ctx.cal_demanda) } else { q * (1.0 - exporta_sitio(p)) };
            ventas_sitios += local * prod_info(p).precio;
            let exp = q - local;
            if exp > 0.0 && !ctx.bloqueo {
                let v = exp * self.precio_mundo(p) * if ctx.muelle { 1.0 } else { 0.7 };
                exp_q[p.i()] += exp;
                exp_v[p.i()] += v;
                derechos += v * ctx.tasa_aduana;
                ventas_sitios += v * (1.0 - ctx.tasa_aduana);
            }
        }
        if ventas_sitios >= 1.0 {
            r.lineas.push(("Canteras, hornos y caleras: ventas".into(), ventas_sitios as i64));
        }
        // La cal que piden las obras y no dan las caleras se trae por barco (con el flete).
        r.cal_importada = (ctx.cal_demanda - ctx.prod_sitios[Prod::Cal.i()]).max(0.0);
        let cal_de_afuera = r.cal_importada * self.precio_mundo(Prod::Cal) * FLETE_CAL;
        // Lo que embarcó la economía (cueros, sebo, carne, trigo) al precio del mundo.
        for res in Res::ALL {
            if let Some(p) = prod_de_res(res) {
                let q = ctx.exportado[res as usize];
                exp_q[p.i()] += q;
                exp_v[p.i()] += q * res.base_price() * if res == Res::Cueros { ctx.world_hides } else { self.precios[p.i()] };
            }
        }
        // Importaciones: lo que trajo la economía, la sal y las manufacturas (menos lo que
        // ahora se hace en la ciudad).
        let manuf = ctx.habitantes.max(0) as f32 * MANUFACTURAS_POR_HABITANTE;
        let sustituye = local_manuf.min(manuf * 0.6);
        let importado: f32 = Res::ALL.iter().map(|r| ctx.importado[*r as usize] * r.base_price()).sum::<f32>() + sal + (manuf - sustituye) + cal_de_afuera;
        if self.incentivo(Incentivo::Arancel) {
            r.lineas.push(("Aranceles protectores".into(), ((manuf - sustituye) * 0.1) as i64));
        }
        // Presupuesto del cabildo.
        if derechos >= 1.0 {
            r.lineas.push(("Aduana: lo que exporta la industria".into(), derechos as i64));
        }
        if cabildo.abs() >= 1.0 {
            r.lineas.push(("Establecimientos del cabildo".into(), cabildo as i64));
        }
        if canon >= 1.0 {
            r.lineas.push(("Canon de los privilegios exclusivos".into(), canon as i64));
        }
        if cobros >= 1.0 {
            r.lineas.push(("Crédito de fomento: cobros".into(), cobros as i64));
        }
        for i in &self.incentivos {
            let m = inc_info(*i).mensual;
            if m > 0 {
                r.lineas.push((inc_info(*i).nombre.to_string(), -m));
            }
        }
        if self.incentivo(Incentivo::Maestros) {
            self.acumulado[Saber::Oficiales.i()] += 0.3;
        }
        // Opinión: incentivos, industrias (olor, venta de ganado) y caída de las exportaciones.
        for i in &self.incentivos {
            for k in 0..3 {
                r.opinion[k] += inc_info(*i).opinion[k] * 0.2;
            }
        }
        for e in &self.establecimientos {
            let inf = e.info();
            r.opinion[0] -= inf.molestia * 0.2;
            if inf.insumo.is_some_and(|x| x.0 == Res::Carne) {
                r.opinion[2] += 0.8;
            }
        }
        let total: f32 = exp_v.iter().sum();
        let previo: Vec<f32> = self.historia.iter().rev().take(12).map(|f| f.exportaciones()).collect();
        if previo.len() >= 6 {
            let media = previo.iter().sum::<f32>() / previo.len() as f32;
            if media > 50.0 {
                let caida = (1.0 - total / media).clamp(0.0, 1.0);
                if caida > 0.15 {
                    r.opinion[1] -= caida * 1.5;
                    r.opinion[2] -= caida * 2.5;
                }
            }
        }
        r.factor_cueros = self.precios[Prod::Cueros.i()] * self.bono_pilotos();
        // Inversores.
        self.evaluaciones = self.evaluar(ctx);
        let mejor = self.evaluaciones.iter().filter(|e| e.conviene()).max_by(|a, b| a.rinde.total_cmp(&b.rinde)).map(|e| (e.ind, e.rinde));
        if let Some((ind, rinde)) = mejor {
            if self.azar() < 0.4 {
                let falta = self.capital_propio(ind, Dueno::Hogar(0));
                let dueno = match ctx.inversores.first() {
                    Some(&(h, ahorro)) if ahorro >= falta => Dueno::Hogar(h),
                    _ if rinde >= 0.18 => Dueno::Forastero,
                    _ => Dueno::Cabildo,
                };
                if dueno != Dueno::Cabildo {
                    r.inversion = Some((ind, dueno));
                }
            }
        }
        // Cuentas del comercio exterior.
        self.cantidades = exp_q;
        self.renuncia_total += renuncia;
        let foto = Foto { mes: ctx.mes, exportado: exp_v, importado, hhi: 0.0, inversion: 0.0, renuncia, saberes: ctx.saberes, precios: self.precios };
        self.historia.push_back(foto);
        while self.historia.len() > 24 {
            self.historia.pop_front();
        }
        let h = self.concentracion();
        if let Some(f) = self.historia.back_mut() {
            f.hhi = h;
        }
        r
    }

    /// Suma la inversión privada del mes a la foto (la anota la ciudad al abrir el edificio).
    pub fn anotar_inversion(&mut self, pesos: f32) {
        if let Some(f) = self.historia.back_mut() {
            f.inversion += pesos;
        }
    }

    /// Remesas: lo que ganan los dueños forasteros y se va de la ciudad (último mes).
    pub fn remesas(&self) -> f32 {
        self.establecimientos.iter().filter(|e| e.dueno == Dueno::Forastero).map(|e| e.ganancia.max(0.0)).sum()
    }
}
