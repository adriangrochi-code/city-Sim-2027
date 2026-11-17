//! Recursos del suelo, aptitud de la tierra y externalidades de las zonas.
//!
//! - **Datos** (ver `docs/sistemas/recursos-del-suelo.md`): el índice de productividad CONEAT
//!   (`productividad.bin`), el grupo CONEAT con su aptitud y la de cultivos de verano
//!   (`suelos.bin` + `suelos.json`), la geología de DINAMIGE (`geologia.bin`) y la ley de once
//!   materiales con tipo (`materiales.bin`: granito, gneis, balasto, canto rodado, arena,
//!   arcilla roja y gris, tosca, conchilla, caliza y turba; ver [`materiales`] y el catálogo
//!   de recursos del Uruguay). Los lee [`TerrainGen`]; en el mapa ficticio (las pruebas) se
//!   estiman del relieve.
//! - **Aptitud** ([`Recursos::aptitud`]): de 0 a 1 para cada [`Uso`]: cultivo (chacras de trigo,
//!   maíz y huerta), pastoreo (estancias) y la extracción de cada material.
//! - **Rinde de los sitios** ([`rinde`]): chacras, estancias, hornos, canteras y caleras
//!   producen según la tierra donde están; la cantera saca lo que hay (granito, gneis, balasto,
//!   canto rodado, arena o tosca), el horno cuece según su barro y la calera necesita caliza o
//!   conchilla ([`materiales::producto_cantera`] y los demás).
//! - **Molestias** ([`molestias`]): olor, ruido, humo y agua sucia de cada industria y servicio,
//!   con su alcance; el agua sucia baja por el arroyo.
//! - **Valor del suelo** ([`valor`]): acceso (plaza, puerto, calle), agua, servicios, costa,
//!   molestias y recursos; decide qué solares crecen primero, los alquileres y la felicidad.
//!
//! API para otros sistemas (industrias, tecnología): [`Recursos::aptitud`],
//! [`Recursos::en_punto`], [`Recursos::en_area`], [`Uso::de_clave`], [`rinde_por_clave`],
//! [`molestias::Perfil::desde_molestia`] y, ya calculado por solar,
//! [`City::recursos_del_lote`](super::City::recursos_del_lote) y
//! [`City::valor_del_suelo`](super::City::valor_del_suelo).

pub mod materiales;
pub mod molestias;
pub mod valor;
#[cfg(test)]
mod tests;

use super::services::Kind;
use crate::terrain::{TerrainGen, BIO_BANADO, BIO_BANCO, BIO_PLAYA, NMAT, SUELOS_JSON};
pub use materiales::Material;
use glam::Vec2;
use serde::Deserialize;
use std::sync::OnceLock;

/// Usos de la tierra y del subsuelo.
#[allow(dead_code)] // API para el sistema de industrias (todavía sin enchufar)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Uso {
    /// Chacras: trigo, maíz y huerta.
    Cultivo,
    /// Estancias: ganado sobre pasturas naturales.
    Pastoreo,
    /// Canteras de piedra (basamento cristalino).
    Piedra,
    /// Barro para ladrillos (limos y arcillas).
    Arcilla,
    Arena,
    /// Canto rodado y grava de los lechos.
    Grava,
    /// Un material con tipo (granito, tosca, conchilla…).
    Mat(Material),
    /// Piedra de cal: caliza o, si no hay, conchilla.
    Cal,
}

#[allow(dead_code)]
impl Uso {
    pub const TODOS: [Uso; 6] = [Uso::Cultivo, Uso::Pastoreo, Uso::Piedra, Uso::Arcilla, Uso::Arena, Uso::Grava];

    pub fn nombre(self) -> &'static str {
        match self {
            Uso::Cultivo => "cultivo",
            Uso::Pastoreo => "pasturas",
            Uso::Piedra => "piedra",
            Uso::Arcilla => "arcilla",
            Uso::Arena => "arena",
            Uso::Grava => "canto rodado",
            Uso::Mat(m) => m.nombre(),
            Uso::Cal => "piedra de cal",
        }
    }

    /// Clave de los perfiles de industria (`recurso: Option<&'static str>`): "cultivo",
    /// "pastura"/"pastoreo", "piedra", "arcilla"/"barro", "arena", "grava"/"canto".
    pub fn de_clave(s: &str) -> Option<Uso> {
        Some(match s {
            "cultivo" | "trigo" | "huerta" => Uso::Cultivo,
            "pastura" | "pasturas" | "pastoreo" | "ganado" => Uso::Pastoreo,
            "piedra" => Uso::Piedra,
            "arcilla" | "barro" => Uso::Arcilla,
            "arena" => Uso::Arena,
            "grava" | "canto" => Uso::Grava,
            "cal" | "calera" => Uso::Cal,
            _ => return Material::de_id(s).map(Uso::Mat),
        })
    }
}

/// Recursos de un punto, de un solar o de la huella de un edificio (todo 0 a 1, salvo `ip`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecursosLote {
    pub cultivo: f32,
    pub pastoreo: f32,
    pub piedra: f32,
    pub arcilla: f32,
    pub arena: f32,
    pub grava: f32,
    /// Índice de productividad CONEAT (media nacional 100).
    pub ip: f32,
    /// Ley de cada material con tipo (índice: [`Material::i`]). Piedra, arcilla, arena y grava
    /// son los grupos de antes, derivados de estos ([`RecursosLote::derivar`]).
    pub mat: [f32; NMAT],
}

impl RecursosLote {
    pub fn de(&self, uso: Uso) -> f32 {
        match uso {
            Uso::Cultivo => self.cultivo,
            Uso::Pastoreo => self.pastoreo,
            Uso::Piedra => self.piedra,
            Uso::Arcilla => self.arcilla,
            Uso::Arena => self.arena,
            Uso::Grava => self.grava,
            Uso::Mat(m) => self.mat[m.i()],
            Uso::Cal => materiales::fuente_de_cal(self).map_or(0.0, |x| x.1),
        }
    }

    /// Los grupos de siempre a partir de los materiales: piedra (granito, gneis), arcilla (roja,
    /// gris), arena y grava (canto rodado, balasto).
    pub fn derivar(&mut self) {
        let m = &self.mat;
        self.piedra = m[Material::Granito.i()].max(m[Material::Gneis.i()]);
        self.arcilla = m[Material::ArcillaRoja.i()].max(m[Material::ArcillaGris.i()]);
        self.arena = m[Material::Arena.i()];
        self.grava = m[Material::CantoRodado.i()].max(0.8 * m[Material::Balasto.i()]);
    }

    /// El material de mayor ley (y la ley).
    #[allow(dead_code)]
    pub fn principal(&self) -> Option<(Material, f32)> {
        Material::TODOS.into_iter().map(|m| (m, self.mat[m.i()])).filter(|x| x.1 >= MINIMO_MATERIAL).max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    fn sumar(&mut self, o: &RecursosLote, w: f32) {
        self.cultivo += o.cultivo * w;
        self.pastoreo += o.pastoreo * w;
        self.piedra += o.piedra * w;
        self.arcilla += o.arcilla * w;
        self.arena += o.arena * w;
        self.grava += o.grava * w;
        self.ip += o.ip * w;
        for k in 0..NMAT {
            self.mat[k] += o.mat[k] * w;
        }
    }

    /// El mejor material para una industria («arcilla y piedra a mano»).
    pub fn materia_prima(&self) -> f32 {
        self.piedra.max(self.arcilla).max(self.arena * 0.8)
    }
}

// --- Tabla de suelos (suelos.json) -----------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
pub struct GrupoConeat {
    pub sc: String,
    pub ip: Option<f32>,
    pub aptitud: Option<String>,
    pub cultivo_verano: Option<String>,
    pub cultivo_verano_aptitud: Option<String>,
    #[serde(default)]
    pub limitantes: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Clone, Debug)]
pub struct UnidadGeologica {
    pub codigo: i32,
    pub unidad: Option<String>,
    #[serde(default)]
    pub periodo: String,
    pub clase: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TablaSuelos {
    pub grupos: Vec<GrupoConeat>,
    pub geologia: Vec<UnidadGeologica>,
}

/// La tabla de grupos CONEAT y unidades geológicas del mapa real.
pub fn tabla() -> &'static TablaSuelos {
    static T: OnceLock<TablaSuelos> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(SUELOS_JSON).expect("suelos.json"))
}

/// Peso de la aptitud general CONEAT (para pasturas).
fn peso_aptitud(a: Option<&str>) -> f32 {
    match a.unwrap_or("") {
        "Tierras Muy Aptas" => 1.0,
        "Tierras Aptas" => 0.85,
        "Tierras Medianamente Aptas" => 0.7,
        "Tierras Marginales" => 0.5,
        "Tierras No Aptas" => 0.3,
        _ => 0.6,
    }
}

/// Peso de la clase de aptitud para cultivos de verano (S1 … N2).
fn peso_cultivo(c: Option<&str>) -> f32 {
    let c = c.unwrap_or("");
    if c.starts_with("S1") {
        1.0
    } else if c.starts_with("S2") {
        0.85
    } else if c.starts_with("S3") {
        0.62
    } else if c.starts_with("N1") {
        0.32
    } else if c.starts_with("N2") {
        0.1
    } else {
        0.6
    }
}

/// Lo que dice el suelo de un punto para la ficha del terreno.
#[derive(Clone, Debug, PartialEq)]
pub struct SueloInfo {
    /// Grupo CONEAT ("10.8b"), su aptitud general y la de cultivos de verano.
    pub grupo: String,
    pub aptitud: String,
    pub cultivo_verano: String,
    pub limitantes: String,
    pub ip: f32,
    /// Zona sin dato CONEAT (la ciudad actual): grupo estimado por vecindad.
    pub estimado: bool,
    /// Unidad geológica (DINAMIGE) y su edad.
    pub geologia: String,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// --- Recursos de un mapa ---------------------------------------------------------------------

/// Recursos y aptitud de la tierra de un mapa (envuelve el terreno: no guarda nada).
#[derive(Clone, Copy)]
pub struct Recursos<'a> {
    pub t: &'a TerrainGen,
}

impl<'a> Recursos<'a> {
    pub fn de(t: &'a TerrainGen) -> Recursos<'a> {
        Recursos { t }
    }

    /// Pendiente (m por m) medida a ±10 m.
    fn pendiente(&self, x: f32, z: f32) -> f32 {
        let t = self.t;
        let dx = (t.height(x + 10.0, z) - t.height(x - 10.0, z)) / 20.0;
        let dz = (t.height(x, z + 10.0) - t.height(x, z - 10.0)) / 20.0;
        (dx * dx + dz * dz).sqrt()
    }

    /// Aptitud (0 a 1) de un punto para un uso.
    pub fn aptitud(&self, uso: Uso, x: f32, z: f32) -> f32 {
        self.en_punto(Vec2::new(x, z)).de(uso)
    }

    /// Todo lo de un punto. En el agua, ceros.
    pub fn en_punto(&self, p: Vec2) -> RecursosLote {
        let t = self.t;
        let (x, z) = (p.x, p.y);
        let h = t.height(x, z);
        if h <= 0.2 || t.is_water(x, z) {
            return RecursosLote::default();
        }
        let s = self.pendiente(x, z);
        // Inundable: bañado oficial, o el fondo de un valle junto al arroyo.
        let bio = t.official_biome(x, z);
        let (da, hw) = t.arroyo_query(x, z);
        let banado = if bio == BIO_BANADO { 1.0 } else { 0.0f32 }.max(if da < hw + 25.0 { 0.6 } else { 0.0 });
        let mut r = match t.productividad(x, z) {
            Some(ip) => {
                let g = t.grupo_coneat(x, z).and_then(|(i, _)| tabla().grupos.get(i as usize - 1));
                let gen = peso_aptitud(g.and_then(|g| g.aptitud.as_deref()));
                let cv = peso_cultivo(g.and_then(|g| g.cultivo_verano.as_deref()));
                let mut mat = [0.0f32; NMAT];
                for (k, v) in mat.iter_mut().enumerate() {
                    *v = t.material(k, x, z).unwrap_or(0.0).clamp(0.0, 1.0);
                }
                RecursosLote {
                    cultivo: 0.5 * (ip / 210.0).clamp(0.0, 1.0) + 0.5 * cv,
                    pastoreo: 0.15 + 0.45 * (ip / 170.0).clamp(0.0, 1.0) + 0.4 * gen,
                    ip,
                    mat,
                    ..Default::default()
                }
            }
            None => self.ficticio(x, z, h, s, da, hw),
        };
        // Relieve: el arado no sube laderas fuertes; el ganado sí, pero menos. La roca a la
        // vista y la arena suelta no dan pasto ni cosecha.
        let roca = t.rock_cover(x, z).max(t.outcrop(x, z) * 0.5);
        let arena_suelta = if matches!(bio, BIO_PLAYA | BIO_BANCO) { 1.0 } else { smoothstep(0.75, 0.95, r.arena) };
        r.cultivo *= (1.0 - smoothstep(0.06, 0.2, s)) * (1.0 - 0.85 * banado) * (1.0 - 1.4 * roca).max(0.0) * (1.0 - 0.9 * arena_suelta);
        r.pastoreo *= (1.0 - 0.5 * smoothstep(0.15, 0.4, s)) * (1.0 - 0.35 * banado) * (1.0 - 1.1 * roca).max(0.1) * (1.0 - 0.75 * arena_suelta);
        // La piedra en laderas y crestas se saca más fácil; en el bañado no hay cantera.
        let k = (0.85 + 0.6 * smoothstep(0.04, 0.2, s)) * (1.0 - 0.8 * banado);
        for m in [Material::Granito, Material::Gneis] {
            r.mat[m.i()] = (r.mat[m.i()] * k).clamp(0.0, 1.0);
        }
        r.derivar();
        r.cultivo = r.cultivo.clamp(0.0, 1.0);
        r.pastoreo = r.pastoreo.clamp(0.0, 1.0);
        r
    }

    /// Mapa ficticio (sin datos): se estima del relieve, el agua y la costa.
    fn ficticio(&self, x: f32, z: f32, h: f32, s: f32, da: f32, hw: f32) -> RecursosLote {
        let t = self.t;
        let costa = t.sea_distance(x, z);
        let arroyo = 1.0 - smoothstep(hw + 20.0, 350.0, da);
        let ip = (130.0 - 300.0 * s - (h - 30.0).max(0.0) * 0.8).clamp(20.0, 200.0);
        let piedra = (0.85 * smoothstep(0.07, 0.22, s) + 0.25 * smoothstep(25.0, 60.0, h)).min(1.0);
        let arcilla = (0.3 + 0.55 * arroyo + 0.3 * (1.0 - smoothstep(60.0, 300.0, costa))).min(0.95);
        let mut mat = [0.0f32; NMAT];
        // Sin carta geológica: granito en lo alto, gneis en las laderas bajas; el barro de los
        // bajos es gris, el de las lomadas rojo; nada de caliza.
        let alto = smoothstep(20.0, 45.0, h);
        mat[Material::Granito.i()] = piedra * alto;
        mat[Material::Gneis.i()] = piedra * (1.0 - alto);
        mat[Material::Balasto.i()] = 0.5 * piedra * (1.0 - smoothstep(0.1, 0.25, s));
        mat[Material::CantoRodado.i()] = 0.6 * (1.0 - smoothstep(hw + 5.0, hw + 40.0, da)) * smoothstep(0.03, 0.1, s).max(0.3);
        mat[Material::Arena.i()] = 0.9 * (1.0 - smoothstep(30.0, 350.0, costa));
        mat[Material::ArcillaRoja.i()] = arcilla * (1.0 - 0.5 * arroyo);
        mat[Material::ArcillaGris.i()] = arcilla * arroyo;
        mat[Material::Tosca.i()] = 0.4 * (1.0 - arroyo) * (1.0 - piedra);
        mat[Material::Turba.i()] = 0.2 * arroyo;
        let mut r = RecursosLote { cultivo: 0.5 * (ip / 210.0) + 0.3, pastoreo: 0.15 + 0.45 * (ip / 170.0).min(1.0) + 0.3, ip, mat, ..Default::default() };
        r.derivar();
        r
    }

    /// Promedio en un solar (centro y cuatro puntos hacia las esquinas).
    pub fn en_lote(&self, lot: &super::lots::Lot) -> RecursosLote {
        self.en_area(lot.c, lot.f, super::lots::W, super::lots::D)
    }

    /// Promedio en una huella rectangular (centro, frente, ancho y fondo).
    pub fn en_area(&self, c: Vec2, f: Vec2, w: f32, d: f32) -> RecursosLote {
        let s = super::lots::side_of(f);
        let mut out = RecursosLote::default();
        let pts = [(0.0, 0.0), (-0.3, -0.3), (0.3, -0.3), (-0.3, 0.3), (0.3, 0.3)];
        for (u, v) in pts {
            out.sumar(&self.en_punto(c + s * (u * w) + f * (v * d)), 1.0 / pts.len() as f32);
        }
        out
    }

    /// Suelo de un punto (grupo CONEAT, aptitud, geología) para la ficha. `None` en el mapa
    /// ficticio o en el agua.
    pub fn suelo(&self, p: Vec2) -> Option<SueloInfo> {
        let (i, estimado) = self.t.grupo_coneat(p.x, p.y)?;
        let g = tabla().grupos.get(i as usize - 1)?;
        let geo = tabla().geologia.get((self.t.unidad_geologica(p.x, p.y) as usize).wrapping_sub(1));
        Some(SueloInfo {
            grupo: g.sc.clone(),
            aptitud: g.aptitud.clone().unwrap_or_else(|| "sin dato".into()),
            cultivo_verano: match (&g.cultivo_verano, &g.cultivo_verano_aptitud) {
                (Some(c), Some(a)) => format!("{a} ({c})"),
                _ => "sin dato".into(),
            },
            limitantes: g.limitantes.clone(),
            ip: g.ip.unwrap_or(100.0),
            estimado,
            geologia: geo.map_or("sin dato".into(), |u| {
                let n = u.unidad.clone().unwrap_or_default().replace("Formacion", "Formación").replace("Transamazonicos", "Transamazónicos").replace("Autoctonos", "Autóctonos");
                if u.periodo.trim().is_empty() { n } else { format!("{n} ({})", u.periodo.trim()) }
            }),
        })
    }

    /// Media de la aptitud para cultivo y pastoreo en el campo alrededor de un punto (muestras
    /// cada 300 m sobre tierra, hasta `radio`): para la producción de las chacras y estancias
    /// privadas del entorno.
    /// La mejor ley de cada material a menos de `radio` del punto (muestras cada 100 m, o
    /// cada `radio / 6` si es más chico), sobre tierra. Para el sistema de materiales de
    /// construcción: qué se puede sacar cerca de una obra.
    #[allow(dead_code)] // API para el sistema de materiales de construcción
    pub fn materiales_cerca(&self, p: Vec2, radio: f32) -> [f32; NMAT] {
        let paso = (radio / 6.0).clamp(20.0, 100.0);
        let n = (radio / paso).ceil() as i32;
        let mut out = [0.0f32; NMAT];
        for j in -n..=n {
            for i in -n..=n {
                let q = p + Vec2::new(i as f32, j as f32) * paso;
                if q.distance(p) > radio {
                    continue;
                }
                let r = self.en_punto(q);
                for k in 0..NMAT {
                    out[k] = out[k].max(r.mat[k]);
                }
            }
        }
        out
    }

    /// Cuánto hay cerca (la mejor ley, 0..1) de un material de construcción ("piedra",
    /// "ladrillo", "teja", "cal", "arena", "adobe"…), según el catálogo; 0 si no sale del suelo
    /// del mapa (se importa).
    #[allow(dead_code)]
    pub fn construccion_cerca(&self, clave: &str, p: Vec2, radio: f32) -> f32 {
        let mats = materiales::materiales_de_construccion(clave);
        if mats.is_empty() {
            return 0.0;
        }
        let cerca = self.materiales_cerca(p, radio);
        mats.iter().map(|m| cerca[m.i()]).fold(0.0, f32::max)
    }

    pub fn media_entorno(&self, centro: Vec2, radio: f32) -> (f32, f32) {
        let paso = 300.0;
        let n = (radio / paso) as i32;
        let (mut c, mut p, mut k) = (0.0f32, 0.0f32, 0.0f32);
        for j in -n..=n {
            for i in -n..=n {
                let q = centro + Vec2::new(i as f32, j as f32) * paso;
                if q.distance(centro) > radio || self.t.height(q.x, q.y) <= 0.5 {
                    continue;
                }
                let r = self.en_punto(q);
                c += r.cultivo;
                p += r.pastoreo;
                k += 1.0;
            }
        }
        if k == 0.0 { (0.0, 0.0) } else { (c / k, p / k) }
    }
}

// --- Rinde de los sitios ---------------------------------------------------------------------

/// Por debajo de esto no hay material que sacar.
pub const MINIMO_MATERIAL: f32 = 0.15;

/// Rinde de un edificio del campo o de la industria según su sitio (1,0 = un sitio normal):
/// la chacra según la tierra para cultivo, la estancia según las pasturas, el horno según su
/// arcilla, la cantera según la ley de lo que saca (granito, gneis, balasto, canto, arena o
/// tosca) y la calera según su caliza o conchilla (sin material no rinden nada). Los demás, 1.
pub fn rinde(k: Kind, r: &RecursosLote) -> f32 {
    match k {
        Kind::Chacra => 0.35 + 1.0 * r.cultivo,
        Kind::Estancia => 0.4 + 0.9 * r.pastoreo,
        Kind::Horno => rinde_material(r.arcilla),
        Kind::Cantera => rinde_material(materiales::ley_cantera(r)),
        Kind::Calera => rinde_material(r.de(Uso::Cal)),
        _ => 1.0,
    }
}

/// Lo que produce un edificio de material según su sitio (cantera, horno, calera); `None` para
/// los demás.
pub fn produccion_sitio(k: Kind, r: &RecursosLote) -> Option<materiales::ProduccionSitio> {
    match k {
        Kind::Cantera => Some(materiales::producto_cantera(r)),
        Kind::Horno => Some(materiales::producto_horno(r)),
        Kind::Calera => Some(materiales::producto_calera(r)),
        _ => None,
    }
}

/// Rinde de un yacimiento: 0 sin material; de 0,45 a 1,3 según cuánto hay.
pub fn rinde_material(m: f32) -> f32 {
    if m < MINIMO_MATERIAL { 0.0 } else { (0.3 + 1.0 * m).min(1.3) }
}

/// Rinde para un perfil de industria por la clave de su recurso (`None` = no depende del
/// sitio: 1,0). Para el sistema de industrias: `rinde_del_sitio(industria, lote)`.
#[allow(dead_code)] // API para el sistema de industrias (todavía sin enchufar)
pub fn rinde_por_clave(recurso: Option<&str>, r: &RecursosLote) -> f32 {
    match recurso.and_then(Uso::de_clave) {
        None => 1.0,
        Some(Uso::Cultivo) => 0.35 + r.cultivo,
        Some(Uso::Pastoreo) => 0.4 + 0.9 * r.pastoreo,
        Some(Uso::Piedra) => rinde_material(materiales::ley_cantera(r).max(r.piedra)),
        Some(u) => rinde_material(r.de(u)),
    }
}

/// Texto de calidad para las fichas («buena», «regular»…).
pub fn calidad(x: f32) -> &'static str {
    match x {
        x if x < 0.15 => "nula",
        x if x < 0.35 => "pobre",
        x if x < 0.55 => "regular",
        x if x < 0.75 => "buena",
        _ => "excelente",
    }
}
