//! Materiales con tipo y el catálogo de los recursos naturales del Uruguay.
//!
//! - **Catálogo** ([`catalogo`]): `assets/datos/recursos_uruguay.json`, todos los recursos del
//!   país (rocas, calizas, áridos, arcillas, minerales industriales, gemas, metales,
//!   combustibles, agro, ganado y monte) con dónde están, para qué sirven, desde cuándo se
//!   explotan, su precio y su trabajo, el estado del dato (MEDIDO, DOCUMENTADO o ESTIMADO) y sus
//!   fuentes. Ver `docs/sistemas/recursos-de-uruguay.md`.
//! - **Materiales del mapa** ([`Material`]): los once que tienen un canal en `materiales.bin`
//!   (la ley de cada uno en cada punto del mapa de Montevideo y su entorno). Cada uno apunta a su
//!   entrada del catálogo y dice qué material de construcción da ([`Material::construccion`]).
//! - **Productos de los sitios** ([`producto_cantera`], [`producto_horno`], [`fuente_de_cal`]):
//!   la cantera saca lo que hay (granito → adoquines y sillería; gneis → piedra de mampostería;
//!   balasto, canto rodado, arena o tosca), el horno cuece según su barro (arcilla roja →
//!   ladrillos; gris → ladrillos y tejas) y la calera necesita caliza o conchilla.
//!
//! API para el sistema de materiales de construcción: [`Material::construccion`],
//! [`Entrada::construccion`], [`de_construccion`], [`se_saca_del_suelo`] y
//! [`super::Recursos::materiales_cerca`] / [`super::Recursos::construccion_cerca`].

// Buena parte del catálogo es API para el sistema de materiales de construcción y para el mapa
// del país (todavía sin enchufar): se lee en las pruebas.
#![allow(dead_code)]

use super::RecursosLote;
use crate::terrain::NMAT;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// El catálogo (texto) que se compila con el juego.
pub const CATALOGO_JSON: &str = include_str!("../../../../assets/datos/recursos_uruguay.json");

// --- Catálogo ---------------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
pub struct Fuente {
    pub titulo: String,
    pub url: String,
    pub tipo: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Hito {
    pub desde: i32,
    pub texto: String,
    pub estado: String,
    pub fuente: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct EnMontevideo {
    pub hay: bool,
    pub material: Option<String>,
    pub texto: String,
}

/// Un recurso del catálogo.
#[derive(Deserialize, Clone, Debug)]
pub struct Entrada {
    pub id: String,
    pub nombre: String,
    pub categoria: String,
    /// MEDIDO, DOCUMENTADO o ESTIMADO.
    pub estado: String,
    pub fuentes: Vec<String>,
    #[serde(default)]
    pub sustancias_catastro: Vec<String>,
    /// Peso (0..1) en cada unidad geológica de DINAMIGE (por código), para el mapa del país.
    #[serde(default)]
    pub unidades_dinamige: BTreeMap<String, f32>,
    pub geologia: String,
    #[serde(default)]
    pub localidades: Vec<String>,
    #[serde(default)]
    pub usos: Vec<String>,
    #[serde(default)]
    pub productos: Vec<String>,
    /// Material de construcción que da: "piedra", "ladrillo", "teja", "cal", "arena",
    /// "madera", "adobe", "paja", "cuero"… (`None`: no se usa para construir).
    pub construccion: Option<String>,
    #[serde(default)]
    pub construccion_tambien: Vec<String>,
    /// Producción nacional (miles de toneladas) por año, cuando hay estadística.
    #[serde(default)]
    pub produccion_kt: BTreeMap<String, f32>,
    #[serde(default)]
    pub historia: Vec<Hito>,
    pub disponible_desde: i32,
    pub precio: String,
    pub trabajo: String,
    pub capital: String,
    pub en_montevideo: EnMontevideo,
}

impl Entrada {
    /// ¿Da este material de construcción (como principal o como secundario)?
    pub fn da(&self, clave: &str) -> bool {
        self.construccion.as_deref() == Some(clave) || self.construccion_tambien.iter().any(|c| c == clave)
    }
}

/// Un producto que sale de los recursos.
#[derive(Deserialize, Clone, Debug)]
pub struct ProductoCat {
    pub id: String,
    pub nombre: String,
    pub insumos: Vec<String>,
    pub desde: i32,
    pub construccion: Option<String>,
    pub estado: String,
    pub fuente: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Catalogo {
    pub fuentes: BTreeMap<String, Fuente>,
    pub categorias: BTreeMap<String, String>,
    pub recursos: Vec<Entrada>,
    pub productos: Vec<ProductoCat>,
}

pub fn catalogo() -> &'static Catalogo {
    static C: OnceLock<Catalogo> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(CATALOGO_JSON).expect("recursos_uruguay.json"))
}

/// Entrada del catálogo por id ("granito", "caliza", "amatista"…).
pub fn entrada(id: &str) -> Option<&'static Entrada> {
    catalogo().recursos.iter().find(|e| e.id == id)
}

/// Recursos del catálogo que dan un material de construcción ("piedra", "cal", "teja"…).
pub fn de_construccion(clave: &str) -> Vec<&'static Entrada> {
    catalogo().recursos.iter().filter(|e| e.da(clave)).collect()
}

/// Materiales del mapa que dan un material de construcción.
pub fn materiales_de_construccion(clave: &str) -> Vec<Material> {
    Material::TODOS.into_iter().filter(|m| m.entrada().da(clave)).collect()
}

/// ¿Sale este material de construcción del suelo del mapa de Montevideo? (si no, se importa:
/// la cal, el mármol, la teja fina…). Según el catálogo (`en_montevideo.hay`).
pub fn se_saca_del_suelo(clave: &str) -> bool {
    de_construccion(clave).iter().any(|e| e.en_montevideo.hay)
}

// --- Materiales del mapa ----------------------------------------------------------------------

/// Los materiales con canal en `materiales.bin` (el orden es el de los canales).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Material {
    Granito,
    Gneis,
    Balasto,
    CantoRodado,
    Arena,
    ArcillaRoja,
    ArcillaGris,
    Tosca,
    Conchilla,
    Caliza,
    Turba,
}

/// Grupos de materiales (una capa de datos cada uno).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GrupoMat {
    /// Granito, gneis.
    Piedra,
    /// Arcilla roja, gris.
    Arcilla,
    /// Arena, canto rodado, balasto.
    Aridos,
    /// Caliza, conchilla, tosca, turba.
    CalYOtros,
}

impl GrupoMat {
    pub fn materiales(self) -> &'static [Material] {
        use Material::*;
        match self {
            GrupoMat::Piedra => &[Granito, Gneis],
            GrupoMat::Arcilla => &[ArcillaRoja, ArcillaGris],
            GrupoMat::Aridos => &[Arena, CantoRodado, Balasto],
            GrupoMat::CalYOtros => &[Caliza, Conchilla, Tosca, Turba],
        }
    }
    pub fn nombre(self) -> &'static str {
        match self {
            GrupoMat::Piedra => "Piedra",
            GrupoMat::Arcilla => "Arcilla",
            GrupoMat::Aridos => "Arena y grava",
            GrupoMat::CalYOtros => "Cal, tosca y turba",
        }
    }
}

impl Material {
    pub const TODOS: [Material; NMAT] = [
        Material::Granito,
        Material::Gneis,
        Material::Balasto,
        Material::CantoRodado,
        Material::Arena,
        Material::ArcillaRoja,
        Material::ArcillaGris,
        Material::Tosca,
        Material::Conchilla,
        Material::Caliza,
        Material::Turba,
    ];

    pub fn i(self) -> usize {
        self as usize
    }

    /// Id del catálogo (y nombre del canal en `meta.json`).
    pub fn id(self) -> &'static str {
        match self {
            Material::Granito => "granito",
            Material::Gneis => "gneis",
            Material::Balasto => "balasto",
            Material::CantoRodado => "canto_rodado",
            Material::Arena => "arena",
            Material::ArcillaRoja => "arcilla_roja",
            Material::ArcillaGris => "arcilla_gris",
            Material::Tosca => "tosca",
            Material::Conchilla => "conchilla",
            Material::Caliza => "caliza",
            Material::Turba => "turba",
        }
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Material::Granito => "granito",
            Material::Gneis => "gneis",
            Material::Balasto => "balasto",
            Material::CantoRodado => "canto rodado",
            Material::Arena => "arena",
            Material::ArcillaRoja => "arcilla roja",
            Material::ArcillaGris => "arcilla gris",
            Material::Tosca => "tosca",
            Material::Conchilla => "conchilla",
            Material::Caliza => "caliza",
            Material::Turba => "turba",
        }
    }

    pub fn de_id(s: &str) -> Option<Material> {
        Material::TODOS.into_iter().find(|m| m.id() == s || m.nombre() == s)
    }

    pub fn entrada(self) -> &'static Entrada {
        entrada(self.id()).expect("cada material tiene su entrada en el catálogo")
    }

    /// Material de construcción que da (del catálogo): "piedra", "ladrillo", "teja", "cal",
    /// "arena"; `None` si no sirve para construir (balasto, tosca, turba).
    pub fn construccion(self) -> Option<&'static str> {
        self.entrada().construccion.as_deref()
    }

    pub fn grupo(self) -> GrupoMat {
        use Material::*;
        match self {
            Granito | Gneis => GrupoMat::Piedra,
            ArcillaRoja | ArcillaGris => GrupoMat::Arcilla,
            Arena | CantoRodado | Balasto => GrupoMat::Aridos,
            Caliza | Conchilla | Tosca | Turba => GrupoMat::CalYOtros,
        }
    }

    /// Color de la capa (el mismo que la vista previa de `recursos_suelo.py`).
    pub fn color(self) -> [f32; 3] {
        match self {
            Material::Granito => [0.85, 0.45, 0.55],
            Material::Gneis => [0.40, 0.42, 0.50],
            Material::Balasto => [0.62, 0.45, 0.30],
            Material::CantoRodado => [0.35, 0.55, 0.85],
            Material::Arena => [0.98, 0.86, 0.40],
            Material::ArcillaRoja => [0.80, 0.30, 0.12],
            Material::ArcillaGris => [0.50, 0.58, 0.62],
            Material::Tosca => [0.92, 0.62, 0.18],
            Material::Conchilla => [0.97, 0.97, 0.92],
            Material::Caliza => [0.92, 0.92, 0.80],
            Material::Turba => [0.22, 0.16, 0.10],
        }
    }
}

// --- Lo que saca cada sitio -------------------------------------------------------------------

/// Por debajo de esto no hay material que sacar (lo mismo que `super::MINIMO_MATERIAL`).
const MINIMO: f32 = super::MINIMO_MATERIAL;

/// Lo que puede sacar una cantera, en orden de preferencia cuando las leyes empatan.
pub const DE_CANTERA: [Material; 6] = [Material::Granito, Material::Gneis, Material::Balasto, Material::CantoRodado, Material::Arena, Material::Tosca];

/// Productos propios de los sitios que van al comercio (los del desarrollo, `Prod`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extra {
    Adoquines,
    Silleria,
    Arena,
    Tejas,
    Cal,
}

/// Lo que da una cantera según su material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProduccionSitio {
    /// El material que manda en el sitio (`None`: no hay nada que sacar).
    pub material: Option<Material>,
    /// Su ley (0..1).
    pub ley: f32,
    /// Multiplicador de lo que va al recurso de la ciudad (piedra o ladrillos) a plena marcha.
    pub base: f32,
    /// Productos propios por mes a rinde 1 (unidades del producto).
    pub extras: [(Extra, f32); 2],
}

impl ProduccionSitio {
    fn nada() -> Self {
        ProduccionSitio { material: None, ley: 0.0, base: 0.0, extras: [(Extra::Adoquines, 0.0), (Extra::Silleria, 0.0)] }
    }
    pub fn texto(&self) -> String {
        match self.material {
            None => "nada que sacar".into(),
            Some(m) => {
                let extras: Vec<String> = self.extras.iter().filter(|e| e.1 > 0.0).map(|e| format!("{} {}", e.1.round() as i32, extra_nombre(e.0))).collect();
                if extras.is_empty() { m.nombre().to_string() } else { format!("{} ({} por mes)", m.nombre(), extras.join(", ")) }
            }
        }
    }
}

pub fn extra_nombre(e: Extra) -> &'static str {
    match e {
        Extra::Adoquines => "adoquines",
        Extra::Silleria => "sillares",
        Extra::Arena => "carretadas de arena",
        Extra::Tejas => "tejas (cientos)",
        Extra::Cal => "fanegas de cal",
    }
}

/// El material que manda entre unos candidatos (la mayor ley; si empatan, el primero).
fn dominante(r: &RecursosLote, candidatos: &[Material]) -> Option<(Material, f32)> {
    let mut mejor: Option<(Material, f32)> = None;
    for &m in candidatos {
        let x = r.mat[m.i()];
        if mejor.is_none_or(|b| x > b.1 + 1e-6) {
            mejor = Some((m, x));
        }
    }
    mejor.filter(|b| b.1 >= MINIMO)
}

/// Lo que saca una cantera: el material que manda en el sitio. Granito → piedra, adoquines y
/// sillería; gneis → piedra de mampostería; balasto y canto rodado → algo de piedra (pedregullo);
/// arena → arena (para el comercio); tosca → poca piedra (relleno de caminos).
pub fn producto_cantera(r: &RecursosLote) -> ProduccionSitio {
    let Some((m, ley)) = dominante(r, &DE_CANTERA) else {
        return ProduccionSitio::nada();
    };
    let (base, extras) = match m {
        Material::Granito => (1.0, [(Extra::Adoquines, 40.0), (Extra::Silleria, 10.0)]),
        Material::Gneis => (1.0, [(Extra::Silleria, 4.0), (Extra::Adoquines, 0.0)]),
        Material::Balasto => (0.55, [(Extra::Arena, 4.0), (Extra::Adoquines, 0.0)]),
        Material::CantoRodado => (0.6, [(Extra::Arena, 6.0), (Extra::Adoquines, 0.0)]),
        Material::Arena => (0.0, [(Extra::Arena, 30.0), (Extra::Adoquines, 0.0)]),
        _ => (0.3, [(Extra::Arena, 0.0), (Extra::Adoquines, 0.0)]),
    };
    ProduccionSitio { material: Some(m), ley, base, extras }
}

/// Ley del material que saca la cantera (0 si no hay nada).
pub fn ley_cantera(r: &RecursosLote) -> f32 {
    dominante(r, &DE_CANTERA).map_or(0.0, |x| x.1)
}

/// Lo que cuece un horno según su barro: arcilla roja → ladrillos; gris → menos ladrillos y
/// tejas.
pub fn producto_horno(r: &RecursosLote) -> ProduccionSitio {
    let Some((m, ley)) = dominante(r, &[Material::ArcillaRoja, Material::ArcillaGris]) else {
        return ProduccionSitio::nada();
    };
    let (base, tejas) = if m == Material::ArcillaRoja { (1.0, 0.0) } else { (0.6, 12.0) };
    ProduccionSitio { material: Some(m), ley, base, extras: [(Extra::Tejas, tejas), (Extra::Cal, 0.0)] }
}

/// Ley de la piedra de cal del sitio: la caliza; si no hay, la conchilla (rinde menos cal).
pub fn fuente_de_cal(r: &RecursosLote) -> Option<(Material, f32)> {
    let c = r.mat[Material::Caliza.i()];
    let k = r.mat[Material::Conchilla.i()] * 0.7;
    if c.max(k) < MINIMO {
        None
    } else if c >= k {
        Some((Material::Caliza, c))
    } else {
        Some((Material::Conchilla, k))
    }
}

/// Cal por mes de una calera a rinde 1 (fanegas).
pub const CALERA_CAL: f32 = 30.0;

/// Lo que da una calera (cal) según su piedra de cal.
pub fn producto_calera(r: &RecursosLote) -> ProduccionSitio {
    match fuente_de_cal(r) {
        None => ProduccionSitio::nada(),
        Some((m, ley)) => ProduccionSitio { material: Some(m), ley, base: 0.0, extras: [(Extra::Cal, CALERA_CAL), (Extra::Adoquines, 0.0)] },
    }
}
