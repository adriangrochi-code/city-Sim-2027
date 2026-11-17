//! Geografía del mapa. Dos mapas:
//! - **Montevideo real** ([`TerrainGen::montevideo`], el del juego): el relieve de
//!   `assets/mapas/montevideo/` (24 × 24 km cada 20 m, más 25 km de entorno cada 100 m), los
//!   arroyos oficiales de la IDE Uruguay y biomas de la época de la fundación (pradera, monte
//!   ribereño, bañados, playas, médanos y puntas de piedra). Playas, bañados, bancos de arena,
//!   lagunas e islas vienen de la hidrografía oficial (`biomas.bin`); el monte nativo, de la
//!   cartografía forestal reconstruida para 1726 (`bosque.bin`); el resto es heurístico.
//!   Ver `docs/sistemas/mapa-real.md` y `docs/sistemas/vegetacion-historica.md`.
//! - **Ficticio** ([`TerrainGen::new`]): puerto directo de `scripts/world/terrain_gen.gd`, usado
//!   por el sandbox, el banco de pruebas y las pruebas.

use crate::palette::*;
use crate::trees::{Species, TreeSpec, VARIANTS};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::Vec2;
use std::collections::HashMap;

#[allow(dead_code)]
pub const MAP_SIZE: f32 = 24000.0;
pub const COAST_Z: f32 = 15600.0;
pub const BAY_X: f32 = 11600.0;
pub const PENINSULA_X: f32 = 13500.0;
pub const CERRO: Vec2 = Vec2::new(10000.0, 15100.0);
const HASH_CELL: f32 = 200.0;

pub struct TerrainGen {
    n_coast: FastNoiseLite,
    n_hills: FastNoiseLite,
    n_detail: FastNoiseLite,
    /// Médanos: crestas de unos 60 m, alargadas a lo largo de la costa.
    n_dune: FastNoiseLite,
    seg_a: Vec<Vec2>,
    seg_b: Vec<Vec2>,
    /// Medio ancho del agua de cada tramo (m): de una cañada (≈1,3 m) al Miguelete (≈9 m).
    seg_hw: Vec<f32>,
    /// Tramos canalizados (rectos por cientos de metros: canales de drenaje del siglo XX que
    /// la hidrografía oficial incluye). En 1726 no existían: no llevan monte ni sauces.
    seg_art: Vec<bool>,
    /// Clase de cada tramo para el monte ribereño (`CAUCE_*`).
    seg_class: Vec<u8>,
    seg_hash: HashMap<(i32, i32), Vec<u32>>,
    /// La misma tabla de segmentos en una grilla densa (más rápida que el HashMap).
    seg_grid: Vec<Vec<u32>>,
    grid_min: (i32, i32),
    grid_size: (i32, i32),
    pub arroyos: Vec<Vec<Vec2>>,
    /// Ancho del agua en cada punto de cada arroyo (m), paralelo a `arroyos`.
    pub arroyo_w: Vec<Vec<f32>>,
    /// Nombre oficial de cada arroyo (paralelo a `arroyos`; `None` si no tiene).
    pub arroyo_nombres: Vec<Option<String>>,
    /// Mapa real (alturas, distancia al mar). `None` = mapa ficticio.
    real: Option<Box<RealMap>>,
    /// Planicie de inundación: el arroyo más cercano de cada punto del mapa (ver [`Flood`]).
    flood: Option<Box<Flood>>,
}

/// Arroyo más cercano a cada celda de 20 m del mapa jugable: un punto de su cauce con el nivel
/// del agua (el fondo del valle en el MDT) y el ancho del cauce ahí. Con eso se sabe a cuántos
/// metros del agua y a qué altura sobre ella está cada punto, que es lo que decide hasta dónde
/// llega el monte ribereño (ocupa la planicie que se inunda).
struct Flood {
    n: usize,
    step: f32,
    /// Índice del punto de cauce más cercano por celda (`u32::MAX`: ninguno).
    near: Vec<u32>,
    /// Puntos de cauce: posición (m), nivel del agua (m) y ancho del cauce (m).
    pts: Vec<(Vec2, f32, f32)>,
}

/// Clases de cauce para el monte ribereño (ver [`TerrainGen::gallery`]).
pub const CAUCE_CANADA: u8 = 0;
pub const CAUCE_CHICO: u8 = 1;
pub const CAUCE_MEDIO: u8 = 2;
pub const CAUCE_MAYOR: u8 = 3;
pub const CAUCE_SAN_JOSE: u8 = 4;
pub const CAUCE_SANTA_LUCIA: u8 = 5;
/// Tramo canalizado (siglo XX): sin monte.
pub const CAUCE_CANAL: u8 = 255;

/// Clase de un cauce por su nombre oficial (la hidrografía no trae el orden de Strahler y el
/// ancho que estimamos por la cuenca no alcanza para distinguir un río de un arroyo).
fn cauce_por_nombre(n: &str) -> Option<u8> {
    let n = n.to_lowercase();
    let tiene = |k: &[&str]| k.iter().any(|w| n.contains(w));
    if tiene(&["santa lucía", "santa lucia"]) && !n.contains("chico") {
        Some(CAUCE_SANTA_LUCIA)
    } else if tiene(&["río san josé", "rio san jose", "río san jose"]) {
        Some(CAUCE_SAN_JOSE)
    } else if tiene(&["colorado", "pando", "solís", "solis", "canelón", "canelon", "santa lucía chico"]) {
        Some(CAUCE_MAYOR)
    } else if tiene(&["miguelete", "pantanoso", "carrasco", "las piedras", "toledo", "manga", "melilla", "cufré", "del cerro", "seco"]) {
        Some(CAUCE_MEDIO)
    } else {
        None
    }
}

/// Umbral del ruido de manchas `0,7·fino + 0,3·grueso` (media 0,5, desvío ≈ 0,11) para que la
/// fracción `c` del suelo quede por encima: cuantiles medidos de ese ruido.
fn patch_threshold(c: f32) -> f32 {
    const Q: [(f32, f32); 8] = [
        (0.005, 0.74), (0.01, 0.714), (0.05, 0.658), (0.10, 0.626),
        (0.15, 0.603), (0.20, 0.584), (0.30, 0.552), (0.50, 0.498),
    ];
    if c > 0.5 {
        // Más de la mitad: simétrico respecto de la media.
        return 1.0 - patch_threshold(1.0 - c);
    }
    let mut thr = Q[0].1;
    for w in Q.windows(2) {
        if c >= w[0].0 && c <= w[1].0 {
            let f = (c - w[0].0) / (w[1].0 - w[0].0);
            thr = w[0].1 + (w[1].1 - w[0].1) * f;
        }
    }
    thr
}

/// Tramos de la costa con su tipo (ver docs/sistemas/costa.md): fracción de roca en la playa y
/// hasta dónde se meten las piedras en el agua.
const COSTA_JSON: &str = include_str!("../../assets/mapas/montevideo/costa.json");

/// Un tramo de costa: segmento (m del mapa), fracción de roca y largo de la restinga (m).
struct Tramo {
    a: Vec2,
    b: Vec2,
    rocas: f32,
    restinga: f32,
}

/// Islotes de piedra que el MDT no tiene (centro, radio y alto, m).
fn parse_islotes(txt: &str) -> Vec<(Vec2, f32, f32)> {
    let v: serde_json::Value = serde_json::from_str(txt).expect("costa.json inválido");
    v["islotes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| {
            let f = |k: &str| i[k].as_f64().unwrap_or(0.0) as f32;
            (Vec2::new(f("x"), f("z")), f("radio_m"), f("alto_m"))
        })
        .collect()
}

fn parse_costa(txt: &str) -> Vec<Tramo> {
    let v: serde_json::Value = serde_json::from_str(txt).expect("costa.json inválido");
    let mut out = Vec::new();
    for t in v["tramos"].as_array().into_iter().flatten() {
        let p = |k: &str| Vec2::new(t[k][0].as_f64().unwrap_or(0.0) as f32, t[k][1].as_f64().unwrap_or(0.0) as f32);
        let r = &t["restinga_m"];
        let restinga = match (r[0].as_f64(), r[1].as_f64()) {
            (Some(a), Some(b)) => ((a + b) * 0.5) as f32,
            _ => r.as_f64().unwrap_or(0.0) as f32,
        };
        out.push(Tramo { a: p("a"), b: p("b"), rocas: t["rocas"].as_f64().unwrap_or(0.0) as f32, restinga });
    }
    out
}

// --- Mapa real ------------------------------------------------------------------------------

/// Datos embebidos en el ejecutable (3 MB): así el mapa no puede faltar en el paquete.
const ALTURA_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/altura.bin");
const CONTEXTO_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/contexto.bin");
const ARROYOS_JSON: &str = include_str!("../../assets/mapas/montevideo/arroyos.json");
/// Cauces del entorno (fuera del mapa jugable): solo los que tienen nombre o son anchos.
const ARROYOS_CTX_JSON: &str = include_str!("../../assets/mapas/montevideo/arroyos_contexto.json");
/// Biomas oficiales (IDE Uruguay), un byte por muestra en las mismas grillas que las alturas.
const BIOMAS_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/biomas.bin");
const BIOMAS_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/biomas_contexto.bin");
const META_JSON: &str = include_str!("../../assets/mapas/montevideo/meta.json");
/// Entorno amplio (ver `tools/mapa_real/construir_entorno_amplio.py`): relieve de 300 × 300 km
/// alrededor del mapa en una grilla de 500 m (metros = valor / 100 − 50) y su cobertura
/// (bañado y monte nativo, un byte cada uno).
const AMPLIO_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/amplio.bin");
const AMPLIO_COB_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/amplio_cobertura.bin");
/// Transición entre el entorno (grilla de 100 m) y el entorno amplio (500 m): los últimos
/// metros de la grilla del entorno se mezclan con la amplia, sin escalón ni corte recto.
pub const WIDE_BLEND: f32 = 3000.0;
/// Fuera de la grilla amplia el relieve se funde, en esta distancia, con una llanura (o con
/// el agua, si el borde es agua): no se estira el último valor en líneas rectas.
pub const WIDE_FADE: f32 = 15000.0;
/// Monte nativo reconstruido para 1726 (cobertura de copas 0..255, misma grilla que las
/// alturas). Ver `docs/sistemas/vegetacion-historica.md`.
const BOSQUE_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/bosque.bin");
const BOSQUE_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/bosque_contexto.bin");
/// Rocosidad de los suelos CONEAT (0..255 = 0..100 % de roca a la vista).
const ROCAS_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/rocas.bin");
const ROCAS_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/rocas_contexto.bin");
// --- Recursos del suelo (ver `docs/sistemas/recursos-del-suelo.md` y `city::recursos`) ---
/// Grupo CONEAT de cada muestra: índice en `suelos.json` (bits 0-6) + 128 si es estimación.
const SUELOS_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/suelos.bin");
const SUELOS_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/suelos_contexto.bin");
/// Índice de productividad CONEAT (media nacional 100), un byte por muestra.
const PRODUCTIVIDAD_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/productividad.bin");
const PRODUCTIVIDAD_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/productividad_contexto.bin");
/// Unidad geológica (DINAMIGE): índice en `suelos.json`.
const GEOLOGIA_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/geologia.bin");
const GEOLOGIA_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/geologia_contexto.bin");
/// Materiales con tipo (granito, gneis, balasto, canto rodado, arena, arcilla roja, arcilla gris,
/// tosca, conchilla, caliza, turba: el orden de `city::recursos::Material`), [`NMAT`] bytes por
/// muestra (ley 0..255); el mapa en una grilla de 40 m, el entorno en una de 200 m.
const MATERIALES_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/materiales.bin");
const MATERIALES_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/materiales_contexto.bin");
/// Canales de `materiales.bin`.
pub const NMAT: usize = 11;
/// Tabla de grupos CONEAT y unidades geológicas (la lee `city::recursos`).
pub const SUELOS_JSON: &str = include_str!("../../assets/mapas/montevideo/suelos.json");

/// Códigos de `biomas.bin`.
pub const BIO_NADA: u8 = 0;
pub const BIO_PLAYA: u8 = 1;
pub const BIO_BANADO: u8 = 2;
pub const BIO_BANCO: u8 = 3;
pub const BIO_LAGUNA: u8 = 4;
/// Isla oficial: tierra con los biomas de siempre.
#[allow(dead_code)]
pub const BIO_ISLA: u8 = 5;

/// Número que sigue a `"clave":` a partir de `desde` en el texto de `meta.json`.
fn meta_num(txt: &str, desde: &str, clave: &str) -> f32 {
    let base = txt.find(desde).unwrap_or(0);
    let rest = &txt[base..];
    let k = rest.find(&format!("\"{clave}\"")).expect("meta.json sin la clave");
    let after = &rest[k + clave.len() + 2..];
    let num: String = after.chars().skip_while(|c| !(c.is_ascii_digit() || *c == '-')).take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.').collect();
    num.parse().expect("número de meta.json")
}
/// Sitio de la fundación (Ciudad Vieja, junto a la Plaza Matriz) y cumbre del Cerro.
pub const FOUNDING_REAL: Vec2 = Vec2::new(11097.7, 14308.9);
pub const CERRO_REAL: Vec2 = Vec2::new(5923.7, 12269.8);
/// Medio ancho del agua de los arroyos del mapa ficticio (los reales traen el suyo).
pub const ARROYO_AGUA: f32 = 5.0;

/// Cauce de un arroyo de medio ancho `hw`: profundidad (m) y distancia hasta donde se hunde.
fn cauce(hw: f32) -> (f32, f32) {
    (0.3 + 0.18 * hw, hw + 4.0 + hw * 1.5)
}

/// Grilla regular de valores (alturas o distancias), con muestreo suave.
struct Grid {
    v: Vec<f32>,
    n: usize,
    step: f32,
    from: f32,
}

impl Grid {
    fn from_u16(bytes: &[u8], n: usize, step: f32, from: f32) -> Grid {
        Grid::from_u16_scaled(bytes, n, step, from, 200.0)
    }
    /// Alturas en u16 con `scale` valores por metro (metros = valor / scale − 50).
    fn from_u16_scaled(bytes: &[u8], n: usize, step: f32, from: f32, scale: f32) -> Grid {
        assert_eq!(bytes.len(), n * n * 2, "mapa real incompleto");
        let v = bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]]) as f32 / scale - 50.0).collect();
        Grid { v, n, step, from }
    }
    /// Último punto de la grilla (m): cubre `[from, to]` en x y en z.
    fn to(&self) -> f32 {
        self.from + (self.n - 1) as f32 * self.step
    }
    /// Distancia (m) al borde de la grilla: positiva adentro, negativa afuera.
    fn inside(&self, x: f32, z: f32) -> f32 {
        let to = self.to();
        (x - self.from).min(z - self.from).min(to - x).min(to - z)
    }
    /// Valores de un byte (0..255) llevados a 0..1.
    fn from_u8(bytes: &[u8], n: usize, step: f32, from: f32) -> Grid {
        assert_eq!(bytes.len(), n * n, "raster incompleto");
        Grid { v: bytes.iter().map(|&b| b as f32 / 255.0).collect(), n, step, from }
    }
    /// Valores de un byte tal cual (0..255), sin llevarlos a 0..1.
    fn from_u8_raw(bytes: &[u8], n: usize, step: f32, from: f32) -> Grid {
        assert_eq!(bytes.len(), n * n, "raster incompleto");
        Grid { v: bytes.iter().map(|&b| b as f32).collect(), n, step, from }
    }
    /// `K` canales intercalados de un byte (0..255 → 0..1).
    fn channels_u8<const K: usize>(bytes: &[u8], n: usize, step: f32, from: f32) -> [Grid; K] {
        assert_eq!(bytes.len(), n * n * K, "raster de materiales incompleto");
        std::array::from_fn(|k| Grid { v: bytes.chunks_exact(K).map(|c| c[k] as f32 / 255.0).collect(), n, step, from })
    }
    #[inline]
    fn at(&self, i: i32, j: i32) -> f32 {
        let m = self.n as i32 - 1;
        self.v[(i.clamp(0, m) + j.clamp(0, m) * self.n as i32) as usize]
    }
    /// Catmull-Rom bicúbico: continuo en la pendiente, sin las facetas ni los quiebres del
    /// bilineal (que de lejos se ven como líneas rectas en las laderas).
    fn cubic(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.from) / self.step).clamp(0.0, (self.n - 1) as f32);
        let fz = ((z - self.from) / self.step).clamp(0.0, (self.n - 1) as f32);
        let (i, j) = (fx.floor() as i32, fz.floor() as i32);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let cr = |a: f32, b: f32, c: f32, d: f32, t: f32| {
            b + 0.5 * t * (c - a + t * (2.0 * a - 5.0 * b + 4.0 * c - d + t * (3.0 * (b - c) + d - a)))
        };
        let mut rows = [0f32; 4];
        for (k, r) in rows.iter_mut().enumerate() {
            let jj = j - 1 + k as i32;
            *r = cr(self.at(i - 1, jj), self.at(i, jj), self.at(i + 1, jj), self.at(i + 2, jj), tx);
        }
        cr(rows[0], rows[1], rows[2], rows[3], tz)
    }
    fn linear(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.from) / self.step).clamp(0.0, (self.n - 1) as f32 - 0.001);
        let fz = ((z - self.from) / self.step).clamp(0.0, (self.n - 1) as f32 - 0.001);
        let (i, j) = (fx as i32, fz as i32);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let a = self.at(i, j) + (self.at(i + 1, j) - self.at(i, j)) * tx;
        let b = self.at(i, j + 1) + (self.at(i + 1, j + 1) - self.at(i, j + 1)) * tx;
        a + (b - a) * tz
    }
    /// Distancia (m) de cada celda de agua a la tierra más cercana.
    fn land_distance(&self) -> Grid {
        let seeds: Vec<bool> = self.v.iter().map(|&h| h > 0.0).collect();
        self.distance_to(&seeds)
    }
    /// Rocosidad de la costa y largo de las restingas por celda, para las celdas a menos de
    /// 800 m de la orilla: el tramo de `costa.json` más cercano (si hay uno a menos de 1,5 km);
    /// si no, según haya playa oficial cerca (arena) o no (piedra).
    fn coast_params(&self, sea: &Grid, land: &Grid, beach: &Grid, tramos: &[Tramo]) -> (Grid, Grid) {
        let n = self.n;
        let (mut r, mut l) = (vec![0f32; n * n], vec![0f32; n * n]);
        for j in 0..n {
            for i in 0..n {
                let k = i + j * n;
                let shore = sea.v[k].max(land.v[k]);
                if shore > 800.0 {
                    continue;
                }
                let p = Vec2::new(self.from + i as f32 * self.step, self.from + j as f32 * self.step);
                let mut best = (1500.0f32, None);
                for t in tramos {
                    let d = p.distance(closest_on_segment(p, t.a, t.b));
                    if d < best.0 {
                        best = (d, Some(t));
                    }
                }
                let (rk, lk) = match best.1 {
                    Some(t) => (t.rocas, t.restinga),
                    None if beach.v[k] < 150.0 => (0.0, 0.0),
                    None => (0.7, 50.0),
                };
                r[k] = rk;
                l[k] = lk;
            }
        }
        (Grid { v: r, n, step: self.step, from: self.from }, Grid { v: l, n, step: self.step, from: self.from })
    }
    /// Distancia (m) de cada celda de tierra al agua más cercana.
    fn sea_distance(&self) -> Grid {
        let seeds: Vec<bool> = self.v.iter().map(|&h| h <= 0.0).collect();
        self.distance_to(&seeds)
    }
    /// Distancia (m) de cada celda a la semilla más cercana: transformada de distancia de
    /// chaflán en dos pasadas (rápida y suficiente para elegir playas y médanos).
    fn distance_to(&self, seeds: &[bool]) -> Grid {
        let n = self.n;
        let big = 1e7f32;
        let mut d: Vec<f32> = seeds.iter().map(|&s| if s { 0.0 } else { big }).collect();
        let (a, b) = (self.step, self.step * std::f32::consts::SQRT_2);
        for j in 0..n {
            for i in 0..n {
                let k = i + j * n;
                let mut v = d[k];
                if i > 0 { v = v.min(d[k - 1] + a); }
                if j > 0 {
                    v = v.min(d[k - n] + a);
                    if i > 0 { v = v.min(d[k - n - 1] + b); }
                    if i + 1 < n { v = v.min(d[k - n + 1] + b); }
                }
                d[k] = v;
            }
        }
        for j in (0..n).rev() {
            for i in (0..n).rev() {
                let k = i + j * n;
                let mut v = d[k];
                if i + 1 < n { v = v.min(d[k + 1] + a); }
                if j + 1 < n {
                    v = v.min(d[k + n] + a);
                    if i + 1 < n { v = v.min(d[k + n + 1] + b); }
                    if i > 0 { v = v.min(d[k + n - 1] + b); }
                }
                d[k] = v;
            }
        }
        Grid { v: d, n, step: self.step, from: self.from }
    }
}

struct RealMap {
    main: Grid,
    ctx: Grid,
    sea_main: Grid,
    sea_ctx: Grid,
    /// Biomas oficiales (códigos `BIO_*`), fila por fila como las alturas.
    bio_main: &'static [u8],
    bio_ctx: &'static [u8],
    /// Distancia a la playa oficial más cercana (m).
    beach_main: Grid,
    beach_ctx: Grid,
    /// Cobertura del monte nativo de 1726 (0..1).
    forest_main: Grid,
    forest_ctx: Grid,
    /// Rocosidad de los suelos (0..1).
    rock_main: Grid,
    rock_ctx: Grid,
    /// Recursos del suelo: productividad CONEAT (IP), grupo CONEAT y geología (bytes crudos,
    /// muestra más cercana) y la ley de cada material (0..1).
    prod_main: Grid,
    prod_ctx: Grid,
    suelo_main: &'static [u8],
    suelo_ctx: &'static [u8],
    geo_main: &'static [u8],
    geo_ctx: &'static [u8],
    material_main: [Grid; NMAT],
    material_ctx: [Grid; NMAT],
    /// Distancia de cada celda de agua a la tierra (m).
    land_main: Grid,
    land_ctx: Grid,
    /// Costa: fracción de roca de la orilla y largo de las restingas (m).
    coast_r_main: Grid,
    coast_r_ctx: Grid,
    coast_l_main: Grid,
    coast_l_ctx: Grid,
    /// Islotes de piedra (centro, radio, alto).
    islotes: Vec<(Vec2, f32, f32)>,
    /// Entorno amplio (500 m): alturas, distancia al agua (m) y cobertura de bañado y de monte
    /// nativo (0..1).
    wide: Grid,
    sea_wide: Grid,
    marsh_wide: Grid,
    forest_wide: Grid,
}

impl RealMap {
    /// Peso de la grilla del entorno (100 m) frente a la amplia: 1 adentro, 0 afuera, con una
    /// transición de [`WIDE_BLEND`] metros contra el borde interior del entorno.
    fn ctx_weight(&self, x: f32, z: f32) -> f32 {
        smoothstep(0.0, WIDE_BLEND, self.ctx.inside(x, z))
    }

    /// Relieve del entorno amplio. Fuera de su grilla (más de 150 km del mapa) se funde con una
    /// llanura de ruido suave (o con el agua, donde el borde es agua) en [`WIDE_FADE`] metros.
    fn wide_height(&self, x: f32, z: f32, hills: &FastNoiseLite) -> f32 {
        let h = self.wide.cubic(x, z);
        let out = -self.wide.inside(x, z);
        if out <= 0.0 {
            return h;
        }
        let k = smoothstep(0.0, WIDE_FADE, out);
        let far = if h > 0.0 { 35.0 + 25.0 * hills.get_noise_2d(x * 0.3, z * 0.3) } else { -7.5 };
        h + (far - h) * k
    }

    /// Mezcla un valor del entorno (100 m) con el del entorno amplio según [`Self::ctx_weight`].
    fn ctx_or_wide(&self, x: f32, z: f32, ctx: impl Fn() -> f32, wide: impl Fn() -> f32) -> f32 {
        let w = self.ctx_weight(x, z);
        if w >= 1.0 {
            ctx()
        } else if w <= 0.0 {
            wide()
        } else {
            ctx() * w + wide() * (1.0 - w)
        }
    }

    /// Distancia al agua fuera del mapa jugable.
    fn outer_sea(&self, x: f32, z: f32) -> f32 {
        self.ctx_or_wide(x, z, || self.sea_ctx.linear(x, z), || self.sea_wide.linear(x, z))
    }

    /// Código de bioma oficial en un punto. Entre las cuatro muestras vecinas gana el código
    /// con más peso bilineal: así el borde de una playa sigue una línea suave y no los
    /// escalones de la grilla.
    fn bio(&self, x: f32, z: f32) -> u8 {
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        let (g, b) = if edge >= 0.0 { (&self.main, self.bio_main) } else { (&self.ctx, self.bio_ctx) };
        let fx = (x - g.from) / g.step;
        let fz = (z - g.from) / g.step;
        let m = g.n as f32 - 1.0;
        if fx < 0.0 || fz < 0.0 || fx > m || fz > m {
            return BIO_NADA;
        }
        let (i, j) = ((fx as usize).min(g.n - 2), (fz as usize).min(g.n - 2));
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let corners = [
            (b[i + j * g.n], (1.0 - tx) * (1.0 - tz)),
            (b[i + 1 + j * g.n], tx * (1.0 - tz)),
            (b[i + (j + 1) * g.n], (1.0 - tx) * tz),
            (b[i + 1 + (j + 1) * g.n], tx * tz),
        ];
        let (mut best, mut best_w) = (BIO_NADA, -1.0f32);
        for &(c, _) in &corners {
            let w: f32 = corners.iter().filter(|(k, _)| *k == c).map(|(_, w)| w).sum();
            if w > best_w {
                best = c;
                best_w = w;
            }
        }
        best
    }
}

/// Lee `{"arroyos": [[[x, z, ancho], ...], ...], "nombres": [...]}` sin dependencias: cada
/// `[x, z, ancho]` es un punto y cada lista de puntos, un arroyo. Devuelve los arroyos y el
/// nombre de cada uno (de la lista paralela `nombres`, si el archivo la trae).
fn parse_arroyos(txt: &str) -> (Vec<Vec<(Vec2, f32)>>, Vec<Option<String>>) {
    // Profundidad de corchetes: 1 = lista de arroyos, 2 = un arroyo, 3 = un punto [x, z, ancho].
    let mut out: Vec<Vec<(Vec2, f32)>> = Vec::new();
    let mut depth = 0;
    let mut num = String::new();
    let mut pair: Vec<f32> = Vec::new();
    let start = txt.find("\"arroyos\"").map(|k| k + 9).unwrap_or(0);
    for ch in txt[start..].chars() {
        match ch {
            '[' => {
                depth += 1;
                if depth == 2 {
                    out.push(Vec::new());
                }
            }
            ']' | ',' => {
                if !num.is_empty() {
                    pair.push(num.parse().unwrap_or(0.0));
                    num.clear();
                }
                if ch == ']' {
                    if depth == 3 {
                        if pair.len() >= 2 {
                            let w = pair.get(2).copied().unwrap_or(ARROYO_AGUA * 2.0);
                            out.last_mut().unwrap().push((Vec2::new(pair[0], pair[1]), w));
                        }
                        pair.clear();
                    }
                    depth -= 1;
                    if depth == 0 {
                        break; // fin de la lista de arroyos
                    }
                }
            }
            c if depth == 3 && (c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+') => num.push(c),
            _ => {}
        }
    }
    let mut names = parse_nombres(txt);
    names.resize(out.len(), None);
    let (out, names): (Vec<_>, Vec<_>) = out.into_iter().zip(names).filter(|(a, _)| a.len() >= 2).unzip();
    (out, names)
}

/// Lista `"nombres": ["...", null, ...]` de `arroyos.json` (vacía si no está).
fn parse_nombres(txt: &str) -> Vec<Option<String>> {
    let Some(k) = txt.find("\"nombres\"") else { return Vec::new() };
    let rest = &txt[k + 9..];
    let Some(open) = rest.find('[') else { return Vec::new() };
    let mut out = Vec::new();
    let mut chars = rest[open + 1..].chars();
    while let Some(c) = chars.next() {
        match c {
            ']' => break,
            'n' => {
                // null
                for _ in 0..3 {
                    chars.next();
                }
                out.push(None);
            }
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(e) = chars.next() {
                                s.push(match e {
                                    'n' => '\n',
                                    't' => '\t',
                                    other => other,
                                });
                            }
                        }
                        other => s.push(other),
                    }
                }
                out.push(Some(s));
            }
            _ => {}
        }
    }
    out
}

fn hash2(x: i32, z: i32, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

fn hashf(x: i32, z: i32, salt: u32) -> f32 {
    (hash2(x, z, salt) & 0xffff) as f32 / 65535.0
}

/// Un árbol del paisaje (ver [`TerrainGen::trees_in`]); su forma sale de [`crate::trees::shape`].
#[derive(Clone, Copy, Debug)]
pub struct Tree {
    pub x: f32,
    pub z: f32,
    /// Altura total (m sobre el suelo), radio de la copa (m) y radio del tronco al pie (m).
    pub height: f32,
    pub radius: f32,
    pub trunk: f32,
    pub species: Species,
    /// Variante de forma (0..[`VARIANTS`]) y semilla del azar de la forma.
    pub variant: u8,
    pub seed: u32,
}

impl Tree {
    pub fn spec(&self) -> TreeSpec {
        TreeSpec { species: self.species, variant: self.variant, height: self.height, radius: self.radius, trunk: self.trunk, seed: self.seed }
    }
}

/// Medidas de cada especie: altura (m), radio de copa (m) y radio del tronco (m), de un árbol
/// adulto común. Ver docs/sistemas/vegetacion-historica.md.
fn species_size(s: Species) -> (f32, f32, f32) {
    match s {
        Species::Sauce => (10.0, 3.2, 0.32),
        Species::Sarandi => (3.0, 1.8, 0.09),
        Species::Ceibo => (6.5, 3.4, 0.3),
        Species::Coronilla => (6.0, 2.8, 0.25),
        Species::Tala => (7.5, 3.8, 0.28),
        Species::Canelon => (8.5, 3.0, 0.25),
        Species::Espinillo => (4.0, 2.6, 0.14),
        Species::Ombu => (11.0, 6.2, 1.2),
        Species::Molle => (4.0, 2.0, 0.14),
    }
}

/// Elige una especie de una lista de (especie, peso) con `u` en 0..1.
fn pick(u: f32, opts: &[(Species, f32)]) -> Species {
    let total: f32 = opts.iter().map(|o| o.1).sum();
    let mut acc = 0.0;
    for &(s, w) in opts {
        acc += w / total;
        if u < acc {
            return s;
        }
    }
    opts[opts.len() - 1].0
}

/// Lado de la celda de la grilla de árboles (m).
pub const TREE_CELL: f32 = 7.0;

/// Ruido configurado como el `FastNoiseLite` por defecto de Godot
/// (OpenSimplex2S, fractal FBM, lacunaridad 2, ganancia 0,5).
fn godot_noise(seed: i32, frequency: f32, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::new();
    n.set_seed(Some(seed));
    n.set_frequency(Some(frequency));
    n.set_noise_type(Some(NoiseType::OpenSimplex2S));
    n.set_fractal_type(Some(FractalType::FBm));
    n.set_fractal_octaves(Some(octaves));
    n.set_fractal_lacunarity(Some(2.0));
    n.set_fractal_gain(Some(0.5));
    n
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn closest_on_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let l2 = ab.length_squared();
    if l2 < 1e-9 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
}

impl TerrainGen {
    pub fn new(seed: i32) -> Self {
        let mut t = TerrainGen {
            n_coast: godot_noise(seed, 0.0007, 5),
            n_hills: godot_noise(seed + 1, 0.00035, 3),
            n_detail: godot_noise(seed + 2, 0.004, 5),
            n_dune: {
                let mut n = godot_noise(seed + 3, 0.016, 3);
                n.set_fractal_type(Some(FractalType::Ridged));
                n
            },
            seg_a: Vec::new(),
            seg_b: Vec::new(),
            seg_hw: Vec::new(),
            seg_art: Vec::new(),
            seg_class: Vec::new(),
            seg_hash: HashMap::new(),
            flood: None,
            seg_grid: Vec::new(),
            grid_min: (0, 0),
            grid_size: (0, 0),
            arroyos: Vec::new(),
            arroyo_nombres: Vec::new(),
            arroyo_w: Vec::new(),
            real: None,
        };
        t.add_arroyo(&[
            (11600.0, 14400.0), (11450.0, 13300.0), (11800.0, 12100.0),
            (11200.0, 10800.0), (11500.0, 9200.0), (10900.0, 7600.0), (11200.0, 6000.0),
        ]);
        t.add_arroyo(&[
            (16400.0, 15900.0), (16600.0, 14600.0), (16200.0, 13300.0),
            (16900.0, 11800.0), (16500.0, 10200.0),
        ]);
        t.build_grid();
        t
    }

    /// El mapa del juego: la geografía real de Montevideo.
    pub fn montevideo() -> Self {
        let mut t = TerrainGen::new(1726);
        t.seg_a.clear();
        t.seg_b.clear();
        t.seg_hw.clear();
        t.seg_art.clear();
        t.seg_class.clear();
        t.seg_hash.clear();
        t.arroyos.clear();
        t.arroyo_nombres.clear();
        // Tamaños según meta.json (la primera "muestras" y "paso_m" son las del mapa jugable).
        let main_n = meta_num(META_JSON, "", "muestras") as usize;
        let main_step = meta_num(META_JSON, "", "paso_m");
        let main = Grid::from_u16(ALTURA_BIN, main_n, main_step, 0.0);
        let ctx_n = meta_num(META_JSON, "\"contexto\"", "muestras") as usize;
        let ctx_step = meta_num(META_JSON, "\"contexto\"", "paso_m");
        let ctx_from = meta_num(META_JSON, "\"contexto\"", "desde_m");
        let ctx = Grid::from_u16(CONTEXTO_BIN, ctx_n, ctx_step, ctx_from);
        assert_eq!(BIOMAS_BIN.len(), main_n * main_n, "biomas.bin no coincide con altura.bin");
        assert_eq!(BIOMAS_CTX_BIN.len(), ctx_n * ctx_n, "biomas_contexto.bin no coincide con contexto.bin");
        let (sea_main, sea_ctx) = (main.sea_distance(), ctx.sea_distance());
        let is_beach = |b: &u8| *b == BIO_PLAYA || *b == BIO_BANCO;
        let beach_main = main.distance_to(&BIOMAS_BIN.iter().map(is_beach).collect::<Vec<_>>());
        let beach_ctx = ctx.distance_to(&BIOMAS_CTX_BIN.iter().map(is_beach).collect::<Vec<_>>());
        let (land_main, land_ctx) = (main.land_distance(), ctx.land_distance());
        let tramos = parse_costa(COSTA_JSON);
        let (coast_r_main, coast_l_main) = main.coast_params(&sea_main, &land_main, &beach_main, &tramos);
        let (coast_r_ctx, coast_l_ctx) = ctx.coast_params(&sea_ctx, &land_ctx, &beach_ctx, &tramos);
        let forest_main = Grid::from_u8(BOSQUE_BIN, main_n, main_step, 0.0);
        let forest_ctx = Grid::from_u8(BOSQUE_CTX_BIN, ctx_n, ctx_step, ctx_from);
        let wide_n = meta_num(META_JSON, "\"amplio\"", "muestras") as usize;
        let wide_step = meta_num(META_JSON, "\"amplio\"", "paso_m");
        let wide_from = meta_num(META_JSON, "\"amplio\"", "desde_m");
        let wide = Grid::from_u16_scaled(AMPLIO_BIN, wide_n, wide_step, wide_from, 100.0);
        let sea_wide = wide.sea_distance();
        let [marsh_wide, forest_wide] = Grid::channels_u8::<2>(AMPLIO_COB_BIN, wide_n, wide_step, wide_from);
        t.real = Some(Box::new(RealMap {
            main,
            ctx,
            sea_main,
            sea_ctx,
            bio_main: BIOMAS_BIN,
            bio_ctx: BIOMAS_CTX_BIN,
            beach_main,
            beach_ctx,
            forest_main,
            forest_ctx,
            rock_main: Grid::from_u8(ROCAS_BIN, main_n, main_step, 0.0),
            rock_ctx: Grid::from_u8(ROCAS_CTX_BIN, ctx_n, ctx_step, ctx_from),
            prod_main: Grid::from_u8_raw(PRODUCTIVIDAD_BIN, main_n, main_step, 0.0),
            prod_ctx: Grid::from_u8_raw(PRODUCTIVIDAD_CTX_BIN, ctx_n, ctx_step, ctx_from),
            suelo_main: SUELOS_BIN,
            suelo_ctx: SUELOS_CTX_BIN,
            geo_main: GEOLOGIA_BIN,
            geo_ctx: GEOLOGIA_CTX_BIN,
            material_main: Grid::channels_u8(MATERIALES_BIN, (main_n - 1) / 2 + 1, main_step * 2.0, 0.0),
            material_ctx: Grid::channels_u8(MATERIALES_CTX_BIN, (ctx_n - 1) / 2 + 1, ctx_step * 2.0, ctx_from),
            land_main,
            land_ctx,
            coast_r_main,
            coast_r_ctx,
            coast_l_main,
            coast_l_ctx,
            islotes: parse_islotes(COSTA_JSON),
            wide,
            sea_wide,
            marsh_wide,
            forest_wide,
        }));
        for txt in [ARROYOS_JSON, ARROYOS_CTX_JSON] {
            let (arroyos, names) = parse_arroyos(txt);
            for (a, name) in arroyos.into_iter().zip(names) {
                let (pts, widths): (Vec<Vec2>, Vec<f32>) = a.into_iter().unzip();
                let class = name.as_deref().and_then(cauce_por_nombre);
                t.add_arroyo_raw(pts, widths, class);
                t.arroyo_nombres.push(name);
            }
        }
        t.build_grid();
        t.build_flood();
        t
    }

    /// Calcula la planicie de inundación ([`Flood`]): siembra cada celda que toca un cauce con
    /// ese punto del cauce (si dos cauces caen en la misma celda gana el más ancho) y propaga a
    /// cada celda el punto sembrado más cercano (dos barridas de la grilla, vecinos en 8
    /// direcciones; el error es de una fracción de celda).
    fn build_flood(&mut self) {
        let Some(r) = &self.real else { return };
        let (n, step) = (r.main.n, r.main.step);
        let mut near = vec![u32::MAX; n * n];
        let mut pts: Vec<(Vec2, f32, f32)> = Vec::new();
        // Nivel del agua: lo más bajo del MDT alrededor del cauce (el fondo del valle).
        let floor_at = |i: usize, j: usize| {
            let mut m = f32::MAX;
            for dj in -1i32..=1 {
                for di in -1i32..=1 {
                    let (a, b) = (i as i32 + di, j as i32 + dj);
                    if a >= 0 && b >= 0 && (a as usize) < n && (b as usize) < n {
                        m = m.min(r.main.v[a as usize + b as usize * n]);
                    }
                }
            }
            m
        };
        let mut seg0 = 0usize;
        for (pl, ws) in self.arroyos.iter().zip(&self.arroyo_w) {
            let nseg = pl.len().saturating_sub(1);
            let first = seg0;
            seg0 += nseg;
            for k in 0..nseg {
                // Los canales no tienen planicie con monte.
                if self.seg_art.get(first + k).copied().unwrap_or(false) {
                    continue;
                }
                let (a, b) = (pl[k], pl[k + 1]);
                let wk = |k: usize| ws.get(k).or(ws.last()).copied().unwrap_or(ARROYO_AGUA * 2.0);
                let w = (wk(k) + wk(k + 1)) * 0.5;
                let steps = ((a.distance(b) / (step * 0.5)).ceil() as usize).max(1);
                for s in 0..=steps {
                    let p = a.lerp(b, s as f32 / steps as f32);
                    let (fi, fj) = (p.x / step, p.y / step);
                    if fi < 0.0 || fj < 0.0 || fi > (n - 1) as f32 || fj > (n - 1) as f32 {
                        continue;
                    }
                    let (i, j) = (fi.round() as usize, fj.round() as usize);
                    let c = i + j * n;
                    if near[c] != u32::MAX && pts[near[c] as usize].2 >= w {
                        continue;
                    }
                    near[c] = pts.len() as u32;
                    pts.push((p, floor_at(i, j), w));
                }
            }
        }
        if pts.is_empty() {
            return;
        }
        let cell = |c: usize| Vec2::new((c % n) as f32 * step, (c / n) as f32 * step);
        let d2 = |c: usize, near: &[u32]| if near[c] == u32::MAX { f32::MAX } else { cell(c).distance_squared(pts[near[c] as usize].0) };
        let mut relax = |near: &mut Vec<u32>, c: usize, o: usize| {
            if near[o] != u32::MAX {
                let cand = cell(c).distance_squared(pts[near[o] as usize].0);
                if cand < d2(c, near) {
                    near[c] = near[o];
                }
            }
        };
        for j in 0..n {
            for i in 0..n {
                let c = i + j * n;
                if i > 0 { relax(&mut near, c, c - 1); }
                if j > 0 {
                    relax(&mut near, c, c - n);
                    if i > 0 { relax(&mut near, c, c - n - 1); }
                    if i + 1 < n { relax(&mut near, c, c - n + 1); }
                }
            }
            for i in (0..n.saturating_sub(1)).rev() {
                let c = i + j * n;
                relax(&mut near, c, c + 1);
            }
        }
        for j in (0..n).rev() {
            for i in (0..n).rev() {
                let c = i + j * n;
                if i + 1 < n { relax(&mut near, c, c + 1); }
                if j + 1 < n {
                    relax(&mut near, c, c + n);
                    if i + 1 < n { relax(&mut near, c, c + n + 1); }
                    if i > 0 { relax(&mut near, c, c + n - 1); }
                }
            }
            for i in 1..n {
                let c = i + j * n;
                relax(&mut near, c, c - 1);
            }
        }
        self.flood = Some(Box::new(Flood { n, step, near, pts }));
    }

    /// Distancia al agua del arroyo más cercano (m, desde la orilla), altura sobre su agua (m)
    /// y ancho del cauce (m), según la planicie calculada al cargar el mapa.
    fn flood_at(&self, x: f32, z: f32, h: f32) -> Option<(f32, f32, f32)> {
        let f = self.flood.as_ref()?;
        let (i, j) = ((x / f.step).round(), (z / f.step).round());
        if i < 0.0 || j < 0.0 || i >= f.n as f32 || j >= f.n as f32 {
            return None;
        }
        let k = f.near[i as usize + j as usize * f.n];
        if k == u32::MAX {
            return None;
        }
        let (p, level, w) = f.pts[k as usize];
        Some(((Vec2::new(x, z).distance(p) - w * 0.5).max(0.0), h - level, w))
    }

    /// Humedal de desembocadura: en el tramo final de los arroyos grandes la planicie más baja
    /// (a menos de ~1 m sobre el agua, cerca del río) no es monte sino bañado, como los
    /// humedales del Santa Lucía y del Carrasco; el monte queda en los albardones más altos.
    fn lowland_marsh(&self, x: f32, z: f32, h: f32, d: f32, hand: f32) -> bool {
        h < 4.0 && hand < 1.0 && d > 15.0 && self.sea_distance(x, z) < 2500.0
    }

    /// ¿Cerca de los médanos? (matorral psamófilo)
    fn biome_is_sandy(&self, x: f32, z: f32) -> bool {
        self.sea_distance(x, z) < self.dune_belt(x, z) * 1.6
    }

    /// ¿Bañado de desembocadura en este punto? (para el bioma)
    fn is_lowland_marsh(&self, x: f32, z: f32, h: f32) -> bool {
        match self.flood_at(x, z, h) {
            Some((d, hand, w)) if w > 4.0 => self.lowland_marsh(x, z, h, d, hand),
            _ => false,
        }
    }

    /// Bioma oficial (`BIO_*`) en un punto; `BIO_NADA` en el mapa ficticio. El punto se corre
    /// unos metros con ruido para que los bordes de la grilla de 20 m no se vean como escalones.
    pub fn official_biome(&self, x: f32, z: f32) -> u8 {
        let Some(r) = &self.real else { return BIO_NADA };
        let jx = self.n_detail.get_noise_2d(x * 0.9, z * 0.9 + 311.0) * 9.0;
        let jz = self.n_detail.get_noise_2d(x * 0.9 + 177.0, z * 0.9) * 9.0;
        r.bio(x + jx, z + jz)
    }

    /// Cuánto de playa tiene la costa más cercana: 1 si la orilla más cercana es una playa
    /// oficial, 0 si es una costa sin playa (piedra, barranca o barro). Para un punto tierra
    /// adentro se compara la distancia a la playa con la distancia al agua.
    pub fn beach_factor(&self, x: f32, z: f32) -> f32 {
        let Some(r) = &self.real else { return 1.0 };
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        let bd = if edge <= 0.0 {
            r.beach_ctx.linear(x, z)
        } else if edge < 400.0 {
            let w = smoothstep(0.0, 400.0, edge);
            r.beach_ctx.linear(x, z) * (1.0 - w) + r.beach_main.linear(x, z) * w
        } else {
            r.beach_main.linear(x, z)
        };
        1.0 - smoothstep(80.0, 350.0, bd - self.sea_distance(x, z))
    }

    /// Cobertura de copas del monte nativo de 1726 (0..1) según `bosque.bin`: dentro del mapa la
    /// grilla de 20 m, afuera la de 100 m, con una transición en el borde. 0 en el mapa ficticio.
    /// Rocosidad del suelo (fracción de roca a la vista, de los suelos CONEAT).
    pub fn rock_cover(&self, x: f32, z: f32) -> f32 {
        let Some(r) = &self.real else { return 0.0 };
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        if edge <= 0.0 {
            r.rock_ctx.linear(x, z)
        } else if edge < 400.0 {
            let w = smoothstep(0.0, 400.0, edge);
            r.rock_ctx.linear(x, z) * (1.0 - w) + r.rock_main.linear(x, z) * w
        } else {
            r.rock_main.linear(x, z)
        }
    }

    /// Índice de productividad CONEAT del suelo (media nacional 100; ver
    /// `docs/sistemas/recursos-del-suelo.md`). `None` en el mapa ficticio.
    pub fn productividad(&self, x: f32, z: f32) -> Option<f32> {
        let r = self.real.as_ref()?;
        Some(self.blend(&r.prod_main, &r.prod_ctx, x, z))
    }

    /// Grupo CONEAT del punto (muestra más cercana): índice en `suelos.json` (1..) y si es una
    /// estimación (zona sin dato, rellenada con el vecino de la misma geología). `None` en el
    /// agua o en el mapa ficticio.
    pub fn grupo_coneat(&self, x: f32, z: f32) -> Option<(u8, bool)> {
        let r = self.real.as_ref()?;
        let b = self.raw_byte(r.suelo_main, r.suelo_ctx, &r.main, &r.ctx, x, z);
        (b & 127 != 0).then_some((b & 127, b & 128 != 0))
    }

    /// Unidad geológica del punto (índice en `suelos.json`, 0 = sin dato).
    pub fn unidad_geologica(&self, x: f32, z: f32) -> u8 {
        match &self.real {
            Some(r) => self.raw_byte(r.geo_main, r.geo_ctx, &r.main, &r.ctx, x, z),
            None => 0,
        }
    }

    /// Ley de un material del suelo (canal de `materiales.bin`, ver `city::recursos::Material`;
    /// 0..1). `None` en el mapa ficticio.
    pub fn material(&self, canal: usize, x: f32, z: f32) -> Option<f32> {
        let r = self.real.as_ref()?;
        Some(self.blend(&r.material_main[canal], &r.material_ctx[canal], x, z))
    }

    /// Byte crudo de un raster (muestra más cercana): el del mapa adentro, el del entorno afuera.
    fn raw_byte(&self, main: &[u8], ctx: &[u8], gm: &Grid, gc: &Grid, x: f32, z: f32) -> u8 {
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        let (bytes, g) = if edge >= 0.0 { (main, gm) } else { (ctx, gc) };
        let m = g.n as i32 - 1;
        let i = (((x - g.from) / g.step).round() as i32).clamp(0, m);
        let j = (((z - g.from) / g.step).round() as i32).clamp(0, m);
        bytes[(i + j * g.n as i32) as usize]
    }

    /// Afloramiento de roca en este punto (0 nada, 1 roca maciza): manchas de 10 a 20 m
    /// (bochas y lajas del basamento) que ocupan, en promedio, la fracción de roca del suelo.
    pub fn outcrop(&self, x: f32, z: f32) -> f32 {
        self.outcrop_field(x, z).map_or(0.0, |(n, thr)| smoothstep(thr - 0.03, thr + 0.03, n))
    }

    /// Relieve del afloramiento (0..1): sube en una rampa más ancha que el borde de la piedra,
    /// así la bocha es un montículo y no un escalón.
    fn outcrop_relief(&self, x: f32, z: f32) -> f32 {
        self.outcrop_field(x, z).map_or(0.0, |(n, thr)| smoothstep(thr - 0.08, thr + 0.06, n))
    }

    /// Ruido de las bochas y su umbral según la rocosidad del suelo.
    fn outcrop_field(&self, x: f32, z: f32) -> Option<(f32, f32)> {
        let c = self.rock_cover(x, z);
        if c < 0.01 {
            return None;
        }
        // Las manchas se agrupan: un ruido grande decide dónde hay campos de piedra y otro
        // chico, cada bocha.
        let field = self.n_hills.get_noise_2d(x * 9.0, z * 9.0) * 0.5 + 0.5;
        let stone = self.n_detail.get_noise_2d(x * 14.0 + 31.0, z * 14.0 - 17.0) * 0.5 + 0.5;
        let n = stone * 0.7 + field * 0.3;
        let thr = patch_threshold(c);
        Some((n, thr))
    }

    /// Variación de un vóxel a otro (−0,5..0,5): mezcla los bordes entre biomas en una banda
    /// en lugar de una línea.
    fn dither(&self, x: f32, z: f32) -> f32 {
        hashf((x / crate::lod::BASE_VOXEL).floor() as i32, (z / crate::lod::BASE_VOXEL).floor() as i32, 97) - 0.5
    }

    pub fn forest_cover(&self, x: f32, z: f32) -> f32 {
        let Some(r) = &self.real else { return 0.0 };
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        if edge <= 0.0 {
            r.ctx_or_wide(x, z, || r.forest_ctx.linear(x, z), || r.forest_wide.linear(x, z))
        } else if edge < 400.0 {
            let w = smoothstep(0.0, 400.0, edge);
            r.forest_ctx.linear(x, z) * (1.0 - w) + r.forest_main.linear(x, z) * w
        } else {
            r.forest_main.linear(x, z)
        }
    }

    /// ¿Dentro del monte? (0..1). La cobertura del raster es una fracción (un 30 % de copas
    /// en la celda); en el terreno se vuelve manchas de monte cerrado de unos 100 m con claros
    /// entre ellas: hay monte donde un ruido de manchas queda por debajo de la cobertura.
    pub fn forest_density(&self, x: f32, z: f32) -> f32 {
        let (da, hw, c) = self.arroyo_info(x, z);
        self.forest_density_at(x, z, da, hw, c, self.height(x, z))
    }

    /// Igual que [`forest_density`], con la distancia al eje del arroyo más cercano y su medio
    /// ancho ya calculados: el monte del raster o la galería del arroyo, lo que sea mayor.
    pub fn forest_density_at(&self, x: f32, z: f32, da: f32, hw: f32, class: u8, h: f32) -> f32 {
        let g = self.gallery(x, z, da - hw, class);
        // La planicie baja de los arroyos grandes es humedal, no monte (ver `lowland_marsh`).
        if g < 0.5 && self.is_lowland_marsh(x, z, h) {
            return 0.0;
        }
        self.raster_forest(x, z).max(g)
    }

    /// Monte de galería de 1726 según la clase del cauce (`CAUCE_*`), con los anchos y la
    /// continuidad que resumen los datos medidos y los testimonios de la época (ver
    /// docs/sistemas/monte-1726-fuentes.md, §7.2): en la jurisdicción de Montevideo el monte fue
    /// siempre una galería angosta a lo largo de los cursos (Gautreau 2006; Síndico del Cabildo,
    /// 1809), ancha solo en el Santa Lucía y el San José. `edge`: distancia al agua (m).
    /// Ancho por margen (mínimo–máximo) y fracción del largo con monte:
    /// cañada 0–5 m (20 %), arroyo chico 5–12 m (45 %), Miguelete/Pantanoso/Carrasco 10–25 m
    /// (75 %), Colorado/Pando/Solís 20–60 m (88 %), San José 40–125 m (95 %), Santa Lucía
    /// 50–150 m, hasta ~250 m en meandros (continuo; sin monte en el tramo salobre).
    pub fn gallery(&self, x: f32, z: f32, edge: f32, class: u8) -> f32 {
        if class == CAUCE_CANAL || edge < 0.0 || self.real.is_none() {
            return 0.0;
        }
        // (ancho mínimo, máximo por margen; umbral del ruido de cortes para esa continuidad)
        let (b0, b1, thr) = match class {
            CAUCE_CANADA => (0.0, 5.0, 0.42),
            CAUCE_CHICO => (5.0, 12.0, 0.04),
            CAUCE_MEDIO => (10.0, 25.0, -0.2),
            CAUCE_MAYOR => (20.0, 60.0, -0.35),
            CAUCE_SAN_JOSE => (40.0, 125.0, -0.5),
            _ => (50.0, 250.0, -2.0),
        };
        if edge > b1 * 1.1 + 2.0 {
            return 0.0;
        }
        if class == CAUCE_SANTA_LUCIA && self.sea_distance(x, z) < 5000.0 {
            return 0.0; // tramo salobre: juncal y pajonal (el monte, en barrancas e islas)
        }
        // Borde irregular: entrantes y salientes; más ancho en las curvas y confluencias.
        let lobe = self.n_hills.get_noise_2d(x * 18.0 + 211.0, z * 18.0 - 97.0) * 0.5 + 0.5;
        let lobe = if class == CAUCE_SANTA_LUCIA { lobe * lobe * 0.8 } else { lobe };
        let b = b0 + (b1 - b0) * lobe;
        // Tramos sin monte: el ruido de cortes (media 0, desvío ≈ 0,3) por encima del umbral.
        let gaps = self.n_hills.get_noise_2d(x * 9.0 - 333.0, z * 9.0 + 71.0);
        let cont = 1.0 - smoothstep(-thr - 0.05, -thr + 0.05, -gaps);
        let cont = if class == CAUCE_SANTA_LUCIA { 1.0 } else { cont };
        (1.0 - smoothstep(b * 0.75, b.max(1.5), edge)) * cont
    }

    /// Monte del raster `bosque.bin` en manchas.
    fn raster_forest(&self, x: f32, z: f32) -> f32 {
        let c = self.forest_cover(x, z);
        if c < 0.015 {
            return 0.0;
        }
        let u = 0.5 + self.n_hills.get_noise_2d(x * 22.0, z * 22.0) * 0.95 + self.n_detail.get_noise_2d(x * 4.0, z * 4.0) * 0.12;
        smoothstep(-0.08, 0.08, c - u.clamp(0.02, 0.98))
    }

    /// Franja de la orilla (0..1): sauces, sarandíes y ceibos sueltos pegados al agua, también
    /// en las cañadas donde no hay monte. Son franjas de pocos metros, más angostas que lo que
    /// distingue la cartografía satelital, y por eso salen de la bibliografía y no del raster.
    /// `edge`: distancia al borde del agua (m); `hw`: medio ancho del cauce (m).
    pub fn bank_fringe(&self, x: f32, z: f32, edge: f32, hw: f32) -> f32 {
        let w = 2.5 + hw * 1.2;
        if !(0.7..w).contains(&edge) {
            return 0.0;
        }
        // Tramos con y sin franja (la mitad, aprox.), más seguidos en los arroyos anchos.
        let n = self.n_hills.get_noise_2d(x * 30.0 + 517.0, z * 30.0);
        smoothstep(-0.1, 0.1, n + 0.05 + hw * 0.04) * (1.0 - smoothstep(w * 0.6, w, edge))
    }

    pub fn is_real(&self) -> bool {
        self.real.is_some()
    }

    /// Centro sugerido para la plaza mayor.
    pub fn founding_site(&self) -> Vec2 {
        if self.is_real() { FOUNDING_REAL } else { Vec2::new(PENINSULA_X, 16300.0) }
    }

    /// Relieve real sin cauces: dentro del mapa, la grilla de 20 m; afuera, la de 100 m, con
    /// una transición de 400 m para que el borde del mapa no se vea como un corte.
    fn real_raw(&self, r: &RealMap, x: f32, z: f32) -> f32 {
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        if edge <= 0.0 {
            // Fuera del mapa: el entorno de 100 m y, más lejos, el amplio.
            return r.ctx_or_wide(x, z, || r.ctx.cubic(x, z), || self.wide_relief(r, x, z));
        }
        let h = r.main.cubic(x, z);
        if edge < 400.0 {
            let w = smoothstep(0.0, 400.0, edge);
            return r.ctx.cubic(x, z) * (1.0 - w) + h * w;
        }
        h
    }

    /// Distancia aproximada al río o a la bahía (m); 0 en el agua.
    pub fn sea_distance(&self, x: f32, z: f32) -> f32 {
        let Some(r) = &self.real else { return (self.coast_z(x) - z).max(0.0) };
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        if edge <= 0.0 {
            return r.outer_sea(x, z);
        }
        let d = r.sea_main.linear(x, z);
        if edge < 400.0 {
            let w = smoothstep(0.0, 400.0, edge);
            return r.sea_ctx.linear(x, z) * (1.0 - w) + d * w;
        }
        d
    }

    /// Islote en este punto: 0 afuera, 1 en el centro (borde irregular de ruido).
    fn islote(&self, x: f32, z: f32) -> (f32, f32) {
        let Some(r) = &self.real else { return (0.0, 0.0) };
        for &(c, rad, alto) in &r.islotes {
            let d = Vec2::new(x, z).distance(c);
            if d < rad * 1.4 {
                let n = self.n_detail.get_noise_2d(x * 6.0 + 3.0, z * 6.0 - 8.0);
                let k = 1.0 - d / (rad * (0.9 + 0.35 * n));
                if k > 0.0 {
                    return (k.min(1.0), alto);
                }
            }
        }
        (0.0, 0.0)
    }

    /// Muestra un par de grillas (mapa / entorno) con la transición del borde.
    fn blend(&self, main: &Grid, ctx: &Grid, x: f32, z: f32) -> f32 {
        let edge = x.min(z).min(MAP_SIZE - x).min(MAP_SIZE - z);
        if edge <= 0.0 {
            ctx.linear(x, z)
        } else if edge < 400.0 {
            let w = smoothstep(0.0, 400.0, edge);
            ctx.linear(x, z) * (1.0 - w) + main.linear(x, z) * w
        } else {
            main.linear(x, z)
        }
    }

    /// Distancia de un punto del agua a la tierra (m); 0 en tierra.
    fn land_distance(&self, x: f32, z: f32) -> f32 {
        self.real.as_ref().map_or(0.0, |r| self.blend(&r.land_main, &r.land_ctx, x, z))
    }

    /// Costa en este punto: fracción de roca de la orilla y largo de las restingas (m).
    pub fn coast_rock(&self, x: f32, z: f32) -> (f32, f32) {
        match &self.real {
            Some(r) => (self.blend(&r.coast_r_main, &r.coast_r_ctx, x, z), self.blend(&r.coast_l_main, &r.coast_l_ctx, x, z)),
            None => (0.0, 0.0),
        }
    }

    /// Ruido de manchas de piedra de la costa (lajas y bochas de 5 a 15 m).
    fn coast_patch(&self, x: f32, z: f32) -> f32 {
        let field = self.n_hills.get_noise_2d(x * 11.0 + 517.0, z * 11.0 - 233.0) * 0.5 + 0.5;
        let stone = self.n_detail.get_noise_2d(x * 18.0 - 71.0, z * 18.0 + 45.0) * 0.5 + 0.5;
        stone * 0.7 + field * 0.3
    }

    /// Piedra de la orilla (0..1, 1 = laja maciza) en la franja de playa: la fracción de roca
    /// del tramo, en manchas.
    pub fn shore_rock(&self, x: f32, z: f32) -> f32 {
        self.shore_rock_ramp(x, z, 0.03, 0.03)
    }

    /// Igual, con una rampa más ancha para el relieve (montículo, no escalón).
    fn shore_rock_ramp(&self, x: f32, z: f32, lo: f32, hi: f32) -> f32 {
        let (r, _) = self.coast_rock(x, z);
        if r < 0.02 {
            return 0.0;
        }
        let thr = patch_threshold(r.min(0.97));
        smoothstep(thr - lo, thr + hi, self.coast_patch(x, z))
    }

    /// Restinga en el agua (0..1): piedras que se meten en el río desde la orilla, cada vez más
    /// salteadas, hasta el largo del tramo.
    pub fn reef(&self, x: f32, z: f32) -> f32 {
        self.reef_ramp(x, z, 0.03, 0.03)
    }

    /// Igual, con la rampa del borde de cada laja elegible (más ancha para el relieve).
    fn reef_ramp(&self, x: f32, z: f32, lo: f32, hi: f32) -> f32 {
        let (r, len) = self.coast_rock(x, z);
        if len < 1.0 || r < 0.02 {
            return 0.0;
        }
        let ld = self.land_distance(x, z);
        if ld <= 0.0 || ld > len {
            return 0.0;
        }
        let f = r * (1.0 - ld / len).powf(0.7);
        if f < 0.01 {
            return 0.0;
        }
        let thr = patch_threshold(f.min(0.97));
        smoothstep(thr - lo, thr + hi, self.coast_patch(x, z))
    }

    /// Ancho de la franja de arena de la costa (m): angosta en la bahía y la Ciudad Vieja,
    /// ancha hacia Carrasco y la costa de Canelones, donde los médanos tapaban los caminos.
    /// En el mapa real solo hay médanos detrás de las playas oficiales ([`Self::beach_factor`]).
    pub fn dune_belt(&self, x: f32, z: f32) -> f32 {
        let bf = self.beach_factor(x, z);
        if bf <= 0.0 {
            return 0.0;
        }
        bf * self.dune_belt_raw(x, z)
    }

    fn dune_belt_raw(&self, x: f32, z: f32) -> f32 {
        let east = smoothstep(13000.0, 25000.0, x);
        // Dentro de la bahía (al norte de la península) casi no hay médanos.
        let bay = 1.0 - smoothstep(13000.0, 15500.0, z) * (1.0 - smoothstep(9000.0, 13000.0, x));
        (70.0 + 480.0 * east) * (1.0 - 0.7 * bay.clamp(0.0, 1.0) * (1.0 - east)) * (0.8 + 0.4 * (self.n_hills.get_noise_2d(x * 2.0, z * 2.0) * 0.5 + 0.5))
    }

    /// Relieve de los médanos (m) en un punto de la franja de arena.
    fn dune_relief(&self, x: f32, z: f32, sd: f32, belt: f32) -> f32 {
        if sd <= 0.0 || sd >= belt {
            return 0.0;
        }
        let east = smoothstep(13000.0, 25000.0, x);
        let amp = 1.5 + 7.5 * east;
        // Crestas paralelas a la costa (este-oeste): se estira el ruido a lo largo de x.
        let ridge = self.n_dune.get_noise_2d(x * 0.45, z) * 0.5 + 0.5;
        let fade = smoothstep(12.0, 70.0, sd) * (1.0 - smoothstep(belt * 0.6, belt, sd));
        amp * ridge * fade
    }

    fn real_height(&self, r: &RealMap, x: f32, z: f32) -> f32 {
        // Entorno amplio (más allá de la grilla de 100 m): solo el relieve, con una ondulación
        // leve. Las costas de piedra, médanos y arroyos salen de los datos del entorno, que acá
        // no llegan (y a esa distancia el vóxel mide 64 m o más).
        let w = r.ctx_weight(x, z);
        if w <= 0.0 {
            return self.wide_relief(r, x, z);
        }
        let raw = self.real_raw(r, x, z);
        let h = self.real_detail(raw, x, z);
        // En la transición al entorno amplio los detalles se apagan con el peso del entorno:
        // la altura es continua en el borde de la grilla de 100 m.
        if w < 1.0 {
            raw + (h - raw) * w
        } else {
            h
        }
    }

    /// Detalles del relieve sobre la grilla (restingas, islotes, piedras de la orilla, médanos,
    /// ondulación, afloramientos y cauces de los arroyos).
    fn real_detail(&self, raw: f32, x: f32, z: f32) -> f32 {
        let mut h = raw;
        // Restingas: en el agua junto a las costas de piedra el fondo sube en lajas que quedan
        // entre medio metro bajo el agua y un metro sobre ella (algunas asoman en bajante).
        if h <= 0.5 {
            let rf = self.reef_ramp(x, z, 0.12, 0.08);
            if rf > 0.0 {
                let bump = self.n_detail.get_noise_2d(x * 12.0 + 9.0, z * 12.0 - 4.0) * 0.5 + 0.5;
                let top = -0.6 + 1.5 * bump;
                if top > h {
                    h += (top - h) * rf;
                }
            }
        }
        // Islotes de piedra que el MDT no tiene (la isla de las Gaviotas).
        let (ik, alto) = self.islote(x, z);
        if ik > 0.0 {
            let bump = self.n_detail.get_noise_2d(x * 15.0, z * 15.0) * 0.5 + 0.5;
            h = h.max(-0.8 + (alto + 0.8) * smoothstep(0.0, 0.45, ik) * (0.75 + 0.25 * bump));
        }
        // Piedras de la orilla: lajas y bochas que levantan entre 0,3 y 1,5 m sobre la arena.
        let sd0 = self.sea_distance(x, z);
        if h > -0.5 && h < 8.0 && sd0 < 70.0 {
            // La franja de piedra se apaga tierra adentro en 30–70 m (sin borde recto).
            let sr = self.shore_rock_ramp(x, z, 0.09, 0.07) * (1.0 - smoothstep(30.0, 70.0, sd0));
            if sr > 0.0 {
                let bump = self.n_detail.get_noise_2d(x * 12.0 - 13.0, z * 12.0 + 7.0) * 0.5 + 0.5;
                h += sr * (0.3 + 1.0 * bump);
            }
        }
        if h > 0.0 && h < 25.0 {
            let sd = self.sea_distance(x, z);
            let belt = self.dune_belt(x, z);
            if sd < belt {
                h += self.dune_relief(x, z, sd, belt);
            }
        }
        if h > 0.3 {
            // Ondulación fina (±0,35 m) para que la ladera no sea una superficie de vidrio.
            h += self.n_detail.get_noise_2d(x, z) * 0.35 * smoothstep(0.5, 3.0, h);
            // Afloramientos: la roca asoma entre 0,5 y 2 m sobre el pasto.
            let o = self.outcrop_relief(x, z);
            if o > 0.0 {
                let bump = self.n_detail.get_noise_2d(x * 40.0, z * 40.0) * 0.5 + 0.5;
                h += o * (0.5 + 1.5 * bump);
            }
            let (da, hw) = self.arroyo_query(x, z);
            let (depth, reach) = cauce(hw);
            if da < reach {
                h -= depth * (1.0 - smoothstep(hw, reach, da));
            }
        }
        h
    }

    fn build_grid(&mut self) {
        let (mut x0, mut z0, mut x1, mut z1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(cx, cz) in self.seg_hash.keys() {
            x0 = x0.min(cx);
            z0 = z0.min(cz);
            x1 = x1.max(cx);
            z1 = z1.max(cz);
        }
        if x0 > x1 {
            self.seg_grid.clear();
            self.grid_size = (0, 0);
            return;
        }
        let (w, h) = (x1 - x0 + 1, z1 - z0 + 1);
        let mut grid = vec![Vec::new(); (w * h) as usize];
        for (&(cx, cz), list) in &self.seg_hash {
            grid[((cx - x0) + (cz - z0) * w) as usize] = list.clone();
        }
        self.seg_grid = grid;
        self.grid_min = (x0, z0);
        self.grid_size = (w, h);
    }

    /// Línea de costa: para cada x, la z donde empieza el agua (al sur está el río).
    pub fn coast_z(&self, x: f32) -> f32 {
        if self.real.is_some() {
            return 0.0; // el mapa real no usa la costa por columna
        }
        let mut z = COAST_Z + self.n_coast.get_noise_2d(x, 0.0) * 380.0;
        z -= 1500.0 * (-((x - BAY_X) / 1100.0).powi(2)).exp();
        z += 1700.0 * (-((x - PENINSULA_X) / 520.0).powi(6)).exp();
        z
    }

    /// Altura del terreno en metros (negativa bajo el agua).
    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.height_with_coast(x, z, self.coast_z(x))
    }

    /// Igual que [`height`], con la costa ya calculada para esta `x` (la costa depende solo
    /// de `x`: al recorrer una grilla se calcula una vez por columna en lugar de por punto).
    #[inline]
    pub fn height_with_coast(&self, x: f32, z: f32, coast: f32) -> f32 {
        if let Some(r) = &self.real {
            return self.real_height(r, x, z);
        }
        let d = coast - z;
        let mut h;
        if d < 0.0 {
            h = (d * 0.015).max(-14.0);
        } else {
            let inland = (d / 3000.0).clamp(0.0, 1.0);
            h = 0.25 + (d * 0.035).min(7.0);
            h += (self.n_hills.get_noise_2d(x, z) * 0.5 + 0.5) * 48.0 * inland;
            h += self.n_detail.get_noise_2d(x, z) * 1.2 * (d / 200.0).clamp(0.0, 1.0);
        }
        let dc = Vec2::new(x, z).distance_squared(CERRO);
        h += 135.0 * (-dc / (2.0 * 380.0 * 380.0)).exp();
        let da = self.arroyo_distance(x, z);
        if da < 160.0 {
            let t = smoothstep(10.0, 160.0, da);
            h = h.min(-1.2) + (h - h.min(-1.2)) * t;
        }
        h
    }

    /// ¿Hay agua acá? El río (bajo el nivel del mar) o el cauce de un arroyo.
    pub fn is_water(&self, x: f32, z: f32) -> bool {
        if self.height(x, z) < 0.5 {
            return true;
        }
        let (d, hw) = self.arroyo_query(x, z);
        d < hw + 0.5
    }

    pub fn arroyo_distance(&self, x: f32, z: f32) -> f32 {
        self.arroyo_query(x, z).0
    }

    /// Distancia al arroyo más cercano y el medio ancho del agua de ese tramo.
    pub fn arroyo_query(&self, x: f32, z: f32) -> (f32, f32) {
        let (d, hw, _) = self.arroyo_query_art(x, z);
        (d, hw)
    }

    /// Igual que [`arroyo_query`], y además si ese tramo es un canal (ver `seg_art`).
    pub fn arroyo_query_art(&self, x: f32, z: f32) -> (f32, f32, bool) {
        let (d, hw, c) = self.arroyo_info(x, z);
        (d, hw, c == CAUCE_CANAL)
    }

    /// Distancia al eje del tramo más cercano, su medio ancho y su clase (`CAUCE_*`; los
    /// tramos canalizados son `CAUCE_CANAL`).
    pub fn arroyo_info(&self, x: f32, z: f32) -> (f32, f32, u8) {
        let p = Vec2::new(x, z);
        let cx = (x / HASH_CELL).floor() as i32;
        let cz = (z / HASH_CELL).floor() as i32;
        // Se elige el tramo cuyo borde de agua está más cerca (no su eje): así un arroyo
        // ancho "gana" a una cañada que le desemboca al lado.
        let (mut best, mut best_edge, mut best_hw, mut best_c) = (1e9f32, 1e9f32, ARROYO_AGUA, CAUCE_CANADA);
        let (gx0, gz0) = self.grid_min;
        let (gw, gh) = self.grid_size;
        for dz in -1..=1 {
            let gz = cz + dz - gz0;
            if gz < 0 || gz >= gh {
                continue;
            }
            for dx in -1..=1 {
                let gx = cx + dx - gx0;
                if gx < 0 || gx >= gw {
                    continue;
                }
                for &si in &self.seg_grid[(gx + gz * gw) as usize] {
                    let q = closest_on_segment(p, self.seg_a[si as usize], self.seg_b[si as usize]);
                    let d = p.distance(q);
                    let hw = self.seg_hw[si as usize];
                    if d - hw < best_edge {
                        best_edge = d - hw;
                        best = d;
                        best_hw = hw;
                        best_c = if self.seg_art[si as usize] { CAUCE_CANAL } else { self.seg_class[si as usize] };
                    }
                }
            }
        }
        (best, best_hw, best_c)
    }

    /// Material de la superficie según altura, pendiente y cercanía al agua.
    pub fn surface_material(&self, x: f32, z: f32, h: f32, slope: f32) -> Mat {
        if self.real.is_some() {
            return self.biome(x, z, h, slope);
        }
        if h < -0.3 {
            return MUD;
        }
        if h < 1.4 {
            return SAND;
        }
        if slope > 0.9 || (h > 118.0 && slope > 0.35) {
            return ROCK;
        }
        // Manchas de pastizal seco en las cuchillas.
        let n = self.n_detail.get_noise_2d(x * 0.3, z * 0.3);
        if n > 0.35 {
            GRASS_DRY
        } else {
            GRASS
        }
    }

    /// Biomas de la costa y la campaña de la fundación (hacia 1726): pradera abierta casi sin
    /// árboles, monte ribereño en las orillas de los arroyos, bañados en los bajos junto a
    /// ellos, playas de arena con médanos y puntas de piedra donde la costa es empinada.
    fn biome(&self, x: f32, z: f32, h: f32, slope: f32) -> Mat {
        // Entorno amplio: biomas gruesos con su cobertura; en la transición se mezclan con los
        // del entorno vóxel a vóxel (sin línea).
        if let Some(r) = &self.real {
            let w = r.ctx_weight(x, z);
            if w <= 0.0 || (w < 1.0 && self.dither(x, z) + 0.5 > w) {
                return self.wide_biome(r, x, z, h, slope);
            }
        }
        // Ruido de manchas (cientos de metros) y de grano fino (metros).
        let blot = self.n_hills.get_noise_2d(x * 3.0, z * 3.0);
        let fine = self.n_detail.get_noise_2d(x * 2.5, z * 2.5);
        let sd = self.sea_distance(x, z);
        let (da, hw, class) = self.arroyo_info(x, z);
        // Islote: piedra desnuda con manchas de pasto arriba.
        let (ik, _) = self.islote(x, z);
        if ik > 0.0 && h > -0.5 {
            return if ik > 0.45 && fine > 0.0 + self.dither(x, z) * 0.3 { GRASS } else { COAST_ROCK };
        }
        // Costa de piedra (costa.json): lajas de la orilla y restingas que se meten en el agua.
        if sd < 70.0 && h > -1.5 && da > hw + 3.0 {
            let d = self.dither(x, z);
            let fade = 1.0 - smoothstep(30.0, 70.0, sd + d * 20.0);
            if self.reef(x, z) > 0.5 + d * 0.3 || (h > -0.3 && self.shore_rock(x, z) * fade > 0.5 + d * 0.3) {
                return COAST_ROCK;
            }
        }
        if h < -0.3 {
            // El cauce de un arroyo ancho que baja del nivel del río sigue siendo arroyo
            // (agua); en el río y la bahía, el fondo: arena cerca de la orilla, limo más adentro.
            if da < hw && sd > 0.0 {
                return STREAM;
            }
            return if h > -2.5 { SAND_WET } else { MUD };
        }
        // Arroyos y cañadas: agua en el centro del cauce y barro en las orillas.
        if da < hw {
            return STREAM;
        }
        if da < hw + 1.0 + hw * 0.3 + fine * 1.2 {
            return MUD;
        }
        // Biomas oficiales (IDE Uruguay): mandan sobre las reglas heurísticas.
        match self.official_biome(x, z) {
            BIO_LAGUNA => return STREAM,
            BIO_BANCO => return if h < 1.0 { SAND_WET } else { SAND },
            BIO_PLAYA => {
                return if h < 0.7 || sd < 10.0 + fine * 6.0 {
                    SAND_WET
                } else if slope > 0.9 {
                    COAST_ROCK
                } else {
                    SAND
                }
            }
            BIO_BANADO if slope < 0.3 => {
                // Pajonal y juncos con algún claro de pasto verde.
                return if fine > 0.55 + blot * 0.2 { GRASS_LUSH } else { MARSH };
            }
            _ => {}
        }
        // Costa: puntas de piedra donde es empinada; si no, playa (húmeda junto al agua),
        // médanos de arena suelta con matas de pasto en las crestas, y un borde irregular
        // hacia la pradera. Donde la IDE no marca playa, la orilla es de piedra o de barro.
        let bf = self.beach_factor(x, z);
        if sd < 60.0 && slope > 0.32 + fine * 0.08 && bf < 0.9 {
            return COAST_ROCK;
        }
        if h < 0.7 && sd < 80.0 {
            return if bf > 0.3 {
                SAND_WET
            } else if slope > 0.1 + fine * 0.05 || (self.official_biome(x, z) == BIO_ISLA && fine > -0.4) {
                // Las islas de la bahía y frente a Carrasco son de piedra.
                COAST_ROCK
            } else {
                MUD
            };
        }
        let belt = self.dune_belt(x, z);
        if sd < belt * 0.3 + 25.0 * bf && h < 12.0 {
            return SAND;
        }
        if sd < belt && h < 25.0 {
            let edge = (sd - belt * 0.3) / (belt * 0.7); // 0 junto a la playa, 1 en el borde
            return if fine > 0.55 - edge * 0.9 { GRASS_DRY } else { DUNE };
        }
        if slope > 0.9 || (h > 105.0 && slope > 0.3 + fine * 0.1) {
            return ROCK;
        }
        // Afloramientos del basamento (suelos CONEAT): roca maciza en el centro de cada
        // mancha, y en el borde piedra y pasto seco mezclados.
        let dith = self.dither(x, z);
        let o = self.outcrop(x, z);
        if o > 0.55 + dith * 0.3 {
            return ROCK;
        }
        if o > 0.15 + dith * 0.2 {
            return GRASS_DRY;
        }
        // Bañados: bajos llanos junto a los arroyos y en sus desembocaduras.
        if slope < 0.05 && h < 5.0 + blot * 2.0 && (da < 25.0 + hw * 22.0 || (da < 700.0 && hw > 5.0 && sd < 350.0)) && fine > -0.35 {
            return MARSH;
        }
        if slope < 0.06 && fine > -0.5 && self.is_lowland_marsh(x, z, h) {
            return MARSH;
        }
        // Monte nativo (bosque.bin): suelo de hojarasca bajo el monte cerrado; en el borde de
        // las manchas, pasto verde a la sombra entre la hojarasca.
        let grain = self.n_detail.get_noise_2d(x * 14.0, z * 14.0) * 0.18;
        let dens = self.forest_density_at(x, z, da, hw, class, h);
        // Bordes mezclados vóxel a vóxel (banda de unos metros), no una línea.
        if dens > 0.78 + fine * 0.15 + dith * 0.25 {
            return FOREST_FLOOR;
        }
        if dens > 0.45 + fine * 0.2 + dith * 0.25 {
            return GRASS_LUSH;
        }
        // Pradera: más verde y húmeda en los bajos, seca y amarillenta en las cuchillas, en las
        // laderas y en la franja costera (suelos arenosos y viento salino).
        // Grano fino (metros) para que los bordes de las manchas no sean líneas limpias.
        let coast = (1.0 - sd / 900.0).max(0.0) * 0.35;
        let dry = (h - 25.0) / 40.0 + blot * 0.6 + fine * 0.15 + grain + coast + slope * 0.6;
        if dry > 0.45 + dith * 0.3 {
            GRASS_DRY
        } else if (h < 18.0 && da < 60.0 + hw * 35.0 && grain > -0.05 + dith * 0.3) || dry < -0.45 + dith * 0.3 || dens > 0.2 + dith * 0.2 {
            GRASS_LUSH
        } else {
            GRASS
        }
    }

    /// Relieve del entorno amplio con una ondulación leve (±6 m en kilómetros) en tierra.
    fn wide_relief(&self, r: &RealMap, x: f32, z: f32) -> f32 {
        let h = r.wide_height(x, z, &self.n_hills);
        if h > 0.3 {
            h + self.n_detail.get_noise_2d(x * 0.05, z * 0.05) * 6.0 * smoothstep(0.5, 20.0, h)
        } else {
            h
        }
    }

    /// Biomas del entorno amplio (vóxeles de 16 a 512 m, a decenas de km): arena en la costa,
    /// bañados y monte nativo según la cobertura de `amplio_cobertura.bin`, roca en las laderas
    /// empinadas de las sierras y pradera, más seca en las cuchillas.
    fn wide_biome(&self, r: &RealMap, x: f32, z: f32, h: f32, slope: f32) -> Mat {
        let d = self.dither(x, z);
        // Manchas de kilómetros (los mismos ruidos, a escala más grande).
        let blot = self.n_hills.get_noise_2d(x * 0.6, z * 0.6);
        let fine = self.n_detail.get_noise_2d(x * 0.05, z * 0.05);
        if h < -0.3 {
            return if h > -2.5 { SAND_WET } else { MUD };
        }
        let sd = r.outer_sea(x, z);
        if sd < 350.0 + fine * 150.0 && h < 8.0 {
            return SAND;
        }
        if slope > 0.45 + fine * 0.1 || (h > 220.0 && slope > 0.2 + d * 0.1) {
            return ROCK;
        }
        let marsh = r.marsh_wide.linear(x, z);
        if marsh > 0.35 + d * 0.4 && slope < 0.08 {
            return if fine > 0.45 { GRASS_LUSH } else { MARSH };
        }
        let forest = r.forest_wide.linear(x, z);
        if forest > 0.3 + d * 0.5 {
            return FOREST_FLOOR;
        }
        if forest > 0.12 + d * 0.2 {
            return GRASS_LUSH;
        }
        let dry = (h - 40.0) / 60.0 + blot * 0.6 + fine * 0.2 + slope * 0.8;
        if dry > 0.45 + d * 0.3 {
            GRASS_DRY
        } else if dry < -0.45 + d * 0.3 {
            GRASS_LUSH
        } else {
            GRASS
        }
    }

    /// Árboles con el tronco en el rectángulo `[x0, x1) × [z0, z1)` (en metros): el monte nativo
    /// de 1726 (`bosque.bin`, en manchas), la franja de sauces, sarandíes y ceibos de las
    /// orillas, el matorral costero y serrano, y árboles sueltos en la pradera (ombúes, talas,
    /// espinillos). La especie depende de la distancia al agua, la pendiente y lo cerrado del
    /// monte; la forma, de la especie y de una de sus variantes. Determinista: el mismo lugar
    /// da el mismo árbol en todos los niveles de detalle. Solo el mapa real tiene árboles.
    pub fn trees_in(&self, x0: f32, z0: f32, x1: f32, z1: f32, out: &mut Vec<Tree>) {
        if self.real.is_none() {
            return;
        }
        let (i0, i1) = ((x0 / TREE_CELL).floor() as i32, ((x1 / TREE_CELL).ceil() as i32));
        let (j0, j1) = ((z0 / TREE_CELL).floor() as i32, ((z1 / TREE_CELL).ceil() as i32));
        for j in j0..j1 {
            for i in i0..i1 {
                let x = (i as f32 + 0.15 + 0.7 * hashf(i, j, 1)) * TREE_CELL;
                let z = (j as f32 + 0.15 + 0.7 * hashf(i, j, 2)) * TREE_CELL;
                if x < x0 || x >= x1 || z < z0 || z >= z1 {
                    continue;
                }
                let roll = hashf(i, j, 3);
                // Descarte barato: casi toda la pradera no tiene árboles.
                if roll >= 0.88 {
                    continue;
                }
                let (da, hw, class) = self.arroyo_info(x, z);
        let art = class == CAUCE_CANAL;
                let h = self.height(x, z);
                let dens = self.forest_density_at(x, z, da, hw, class, h);
                let edge = da - hw;
                if edge < 0.7 {
                    continue; // en el agua
                }
                let fringe = if art { 0.0 } else { self.bank_fringe(x, z, edge, hw) };
                let p_forest = 0.86 * dens;
                let p_fringe = 0.4 * fringe;
                if roll >= p_forest.max(p_fringe).max(0.025) {
                    continue;
                }
                // Ni en la tierra baja que inundan las mareas y las sudestadas junto al río.
                if h < 1.2 || (h < 2.0 && self.sea_distance(x, z) < 3000.0) {
                    continue;
                }
                // Sin árboles en playas, bañados, bancos de arena ni lagunas oficiales.
                // Ni sobre la roca de los afloramientos.
                if self.outcrop(x, z) > 0.3 || matches!(self.official_biome(x, z), BIO_PLAYA | BIO_BANADO | BIO_BANCO | BIO_LAGUNA) {
                    continue;
                }
                let u = hashf(i, j, 5);
                // Pendiente aproximada (quebradas y laderas del Cerro).
                let slope = ((self.height(x + 3.0, z) - self.height(x - 3.0, z)).abs() + (self.height(x, z + 3.0) - self.height(x, z - 3.0)).abs()) / 6.0;
                use Species::*;
                let species = if roll < p_forest {
                    if edge < 3.0 + hw * 0.6 {
                        // Pegado al agua: sauce criollo, sarandí y ceibo.
                        pick(u, &[(Sauce, 4.0), (Sarandi, 3.0), (Ceibo, 3.0)])
                    } else if slope > 0.25 {
                        // Monte de quebrada y de ladera.
                        pick(u, &[(Coronilla, 3.5), (Molle, 2.5), (Tala, 3.0), (Canelon, 1.0), (Espinillo, 1.0)])
                    } else if dens > 0.85 && edge < 45.0 {
                        // Monte cerrado de la galería.
                        pick(u, &[(Canelon, 2.5), (Tala, 2.5), (Coronilla, 2.5), (Ceibo, 1.0), (Sauce, 0.8), (Molle, 0.7)])
                    } else {
                        // Borde exterior del monte, más seco y abierto.
                        pick(u, &[(Coronilla, 3.0), (Tala, 3.5), (Espinillo, 2.5), (Molle, 1.5)])
                    }
                } else if roll < p_fringe {
                    pick(u, &[(Sauce, 3.0), (Sarandi, 4.5), (Ceibo, 2.5)])
                } else {
                    // Lo suelto (pradera, costa, serranía), lejos de las orillas.
                    if edge < 8.0 + hw {
                        continue;
                    }
                    let sd = self.sea_distance(x, z);
                    let (p, s) = if sd < 110.0 || sd < self.dune_belt(x, z) {
                        (0.0, Tala) // playa y médanos: arena suelta, sin árboles
                    } else if sd < 400.0 {
                        // Matorral costero (psamófilo): solo en los médanos de la costa oeste, en
                        // macizos; ni en las puntas de piedra ni en la costa este (§7.4).
                        let p = if x < 9000.0 && self.biome_is_sandy(x, z) { 0.02 } else { 0.0 };
                        (p, pick(u, &[(Coronilla, 3.0), (Molle, 3.0), (Tala, 2.0)]))
                    } else if h > 100.0 || slope > 0.35 {
                        (0.02, pick(u, &[(Espinillo, 3.0), (Coronilla, 2.0), (Molle, 3.0)])) // serranía del Cerro
                    } else {
                        // Parque de espinillos en las laderas bajas que dominan la planicie de los
                        // cauces grandes (50–300 m del monte); en las lomadas, casi nada (§7.4).
                        if class >= CAUCE_MAYOR && class != CAUCE_CANAL && edge < 300.0 && slope < 0.15 {
                            (0.05, pick(u, &[(Espinillo, 6.0), (Tala, 2.0), (Ombu, 0.5)]))
                        } else {
                            (0.0012, pick(u, &[(Ombu, 4.5), (Tala, 3.0), (Espinillo, 2.5)])) // árbol suelto en la pradera
                        }
                    };
                    if roll >= p {
                        continue;
                    }
                    s
                };
                let variant = (hash2(i, j, 7) % VARIANTS as u32) as u8;
                let (bh, br, bt) = species_size(species);
                // Tamaño: la variante 3 es joven (salvo el sauce, que es viejo y ancho).
                let age = if variant == 3 && species != Sauce { 0.62 } else { 1.0 };
                let r2 = hashf(i, j, 4);
                let k = age * (0.82 + 0.36 * r2);
                // En el borde del monte y en la pradera los árboles son algo más bajos y anchos.
                let open = if roll < p_forest { 1.0 - 0.15 * (1.0 - dens) } else { 0.9 };
                // En el monte cerrado las copas se tocan.
                let closed = if roll < p_forest { 1.0 + 0.22 * dens } else { 1.0 };
                out.push(Tree {
                    x,
                    z,
                    height: bh * k * open * (1.0 + (closed - 1.0) * 0.5),
                    radius: br * k * (2.0 - open) * closed,
                    trunk: bt * (0.7 + 0.3 * k) * age.sqrt(),
                    species,
                    variant,
                    seed: hash2(i, j, 11),
                });
            }
        }
    }

    /// Radio máximo que alcanza un árbol desde su tronco (para buscar árboles que entran en una
    /// columna desde las vecinas): el ombú más grande.
    pub const MAX_TREE_RADIUS: f32 = 13.0;

    fn add_arroyo(&mut self, raw: &[(f32, f32)]) {
        let raw: Vec<Vec2> = raw.iter().map(|&(x, z)| Vec2::new(x, z)).collect();
        let pts = self.smooth_path(&raw, 40.0);
        let w = vec![ARROYO_AGUA * 2.0; pts.len()];
        self.add_arroyo_raw(pts, w, None);
    }

    /// Registra un arroyo ya muestreado (los del mapa real vienen con meandros).
    /// `class`: clase del cauce por su nombre oficial (`None`: según el ancho de cada tramo).
    fn add_arroyo_raw(&mut self, pts: Vec<Vec2>, widths: Vec<f32>, class: Option<u8>) {
        // Tramos rectos: desde cada punto, si los próximos 400 m (o más) casi no se desvían de
        // la cuerda (cuerda / largo > 0,985), todo ese tramo es un canal.
        let mut art = vec![false; pts.len().saturating_sub(1)];
        for k in 0..art.len() {
            let mut len = 0.0;
            for e in k..art.len() {
                len += pts[e].distance(pts[e + 1]);
                if len >= 400.0 {
                    if pts[k].distance(pts[e + 1]) / len > 0.985 {
                        art[k..=e].iter_mut().for_each(|a| *a = true);
                    }
                    break;
                }
            }
        }
        self.seg_art.extend_from_slice(&art);
        for (k, w) in pts.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let si = self.seg_a.len() as u32;
            self.seg_a.push(a);
            self.seg_b.push(b);
            let hw = (widths[k] + widths[k + 1]) * 0.25;
            self.seg_hw.push(hw);
            self.seg_class.push(class.unwrap_or(if hw * 2.0 < 2.5 { CAUCE_CANADA } else if hw * 2.0 < 6.0 { CAUCE_CHICO } else { CAUCE_MEDIO }));
            let (x0, x1) = ((a.x.min(b.x) / HASH_CELL).floor() as i32, (a.x.max(b.x) / HASH_CELL).floor() as i32);
            let (z0, z1) = ((a.y.min(b.y) / HASH_CELL).floor() as i32, (a.y.max(b.y) / HASH_CELL).floor() as i32);
            for cz in z0..=z1 {
                for cx in x0..=x1 {
                    self.seg_hash.entry((cx, cz)).or_default().push(si);
                }
            }
        }
        self.arroyos.push(pts);
        self.arroyo_w.push(widths);
    }

    /// Catmull-Rom con meandros de ruido (igual que en Godot).
    fn smooth_path(&self, raw: &[Vec2], step: f32) -> Vec<Vec2> {
        let mut out = Vec::new();
        let last = raw.len() - 1;
        for i in 0..last {
            let p0 = raw[i.saturating_sub(1)];
            let p1 = raw[i];
            let p2 = raw[i + 1];
            let p3 = raw[(i + 2).min(last)];
            let n = ((p1.distance(p2) / step) as usize).max(2);
            let tan = (p2 - p1).normalize();
            let perp = Vec2::new(-tan.y, tan.x);
            for k in 0..n {
                let t = k as f32 / n as f32;
                let (t2, t3) = (t * t, t * t * t);
                let mut q = 0.5
                    * ((2.0 * p1) + (p2 - p0) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3);
                let fade = if i == 0 && k < n / 2 { 0.0 } else { 1.0 };
                q += perp * self.n_detail.get_noise_2d(q.x * 0.05, q.y * 0.05) * 70.0 * fade;
                out.push(q);
            }
        }
        out.push(raw[last]);
        out
    }
}

// --- Agua de los arroyos ----------------------------------------------------------------------

impl TerrainGen {
    /// Superficie del agua de los arroyos y cañadas: cintas de triángulos que siguen el cauce
    /// por debajo de las orillas, a la altura del lecho de cada tramo (bajan aguas abajo).
    /// Cada vértice lleva [x, y, z, fx, fz]: (fx, fz) es la dirección y velocidad de la
    /// corriente (m/s), para que el agua se vea correr. Solo en el mapa real.
    pub fn stream_ribbons(&self) -> Vec<[f32; 5]> {
        let mut out = Vec::new();
        if self.real.is_none() {
            return out;
        }
        for (pts, ws) in self.arroyos.iter().zip(&self.arroyo_w) {
            if pts.len() < 2 || ws.len() != pts.len() {
                continue;
            }
            // La corriente va de lo alto a lo bajo.
            let down = self.height(pts[0].x, pts[0].y) >= self.height(pts[pts.len() - 1].x, pts[pts.len() - 1].y);
            // Muestreo cada ~5 m a lo largo del arroyo.
            let mut samples: Vec<(Vec2, f32)> = Vec::new();
            for k in 0..pts.len() - 1 {
                let (a, b) = (pts[k], pts[k + 1]);
                let n = (a.distance(b) / 5.0).ceil().max(1.0) as usize;
                for j in 0..n {
                    let u = j as f32 / n as f32;
                    samples.push((a.lerp(b, u), ws[k] + (ws[k + 1] - ws[k]) * u));
                }
            }
            samples.push((pts[pts.len() - 1], ws[ws.len() - 1]));
            let mut prev: Option<([f32; 5], [f32; 5])> = None;
            for i in 0..samples.len() {
                let (p, w) = samples[i];
                let (a, b) = (samples[i.saturating_sub(1)].0, samples[(i + 1).min(samples.len() - 1)].0);
                let tan = (b - a).normalize_or_zero();
                if tan == Vec2::ZERO {
                    continue;
                }
                let hw = w * 0.5;
                let (depth, reach) = cauce(hw);
                // En el río (bajo el nivel del mar) ya está el plano del río.
                let raw = self.height(p.x, p.y) + depth;
                if raw < 0.6 {
                    prev = None;
                    continue;
                }
                // Nivel del agua: un tercio de la profundidad del cauce por debajo de las orillas.
                let y = raw - depth * 0.35;
                // Hasta donde la orilla sube por encima del nivel del agua.
                let half = hw + (reach - hw) * 0.55;
                let nrm = Vec2::new(-tan.y, tan.x) * half;
                // Más rápido en cauces angostos: unos 0,3 a 1 m/s.
                let speed = (0.35 + 1.2 / (1.0 + hw)).min(1.0);
                let f = if down { tan * speed } else { -tan * speed };
                let l = [p.x + nrm.x, y, p.y + nrm.y, f.x, f.y];
                let r = [p.x - nrm.x, y, p.y - nrm.y, f.x, f.y];
                if let Some((pl, pr)) = prev {
                    out.extend_from_slice(&[pl, pr, r, pl, r, l]);
                }
                prev = Some((l, r));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_monte_es_una_galeria_angosta_como_en_1726() {
        let t0 = std::time::Instant::now();
        let t = TerrainGen::montevideo();
        eprintln!("mapa real con planicie: {:?}", t0.elapsed());
        // Fracción de la tierra del mapa jugable con monte: 2–5 % según los datos medidos y los
        // testimonios (docs/sistemas/monte-1726-fuentes.md, §7.1).
        let (mut tierra, mut monte) = (0, 0);
        for j in 0..240 {
            for i in 0..240 {
                let (x, z) = (i as f32 * 100.0 + 50.0, j as f32 * 100.0 + 50.0);
                let h = t.height(x, z);
                if h < 1.0 {
                    continue;
                }
                tierra += 1;
                let (da, hw, c) = t.arroyo_info(x, z);
                if t.forest_density_at(x, z, da, hw, c, h) > 0.5 {
                    monte += 1;
                }
            }
        }
        let frac = monte as f32 / tierra as f32;
        eprintln!("monte: {:.1} % de la tierra", frac * 100.0);
        assert!(frac > 0.015 && frac < 0.06, "monte en {:.1} %", frac * 100.0);
        // Las clases salen del nombre oficial.
        assert_eq!(cauce_por_nombre("Arroyo Miguelete"), Some(CAUCE_MEDIO));
        assert_eq!(cauce_por_nombre("Río Santa Lucía"), Some(CAUCE_SANTA_LUCIA));
        assert_eq!(cauce_por_nombre("Arroyo Colorado"), Some(CAUCE_MAYOR));
        assert_eq!(cauce_por_nombre("Cañada de la Chacarita"), None);
    }

    #[test]
    fn peninsula_es_tierra_y_el_sur_es_rio() {
        let t = TerrainGen::new(1726);
        assert!(t.height(13500.0, 16300.0) > 2.0, "el centro de la península debe ser tierra firme");
        assert!(t.height(13500.0, 20000.0) < 0.0, "al sur está el río");
        assert!(t.height(8000.0, 16900.0) < 0.0, "la bahía/río al oeste");
    }

    #[test]
    fn la_grilla_densa_y_la_costa_cacheada_dan_lo_mismo() {
        let t = TerrainGen::new(1726);
        let slow = |x: f32, z: f32| {
            let p = Vec2::new(x, z);
            let (cx, cz) = ((x / HASH_CELL).floor() as i32, (z / HASH_CELL).floor() as i32);
            let mut best = 1e9f32;
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if let Some(list) = t.seg_hash.get(&(cx + dx, cz + dz)) {
                        for &si in list {
                            best = best.min(p.distance(closest_on_segment(p, t.seg_a[si as usize], t.seg_b[si as usize])));
                        }
                    }
                }
            }
            best
        };
        for k in 0..4000 {
            let x = 6000.0 + (k % 80) as f32 * 157.3;
            let z = 5000.0 + (k / 80) as f32 * 233.1;
            assert_eq!(t.arroyo_distance(x, z), slow(x, z));
            assert_eq!(t.height(x, z), t.height_with_coast(x, z, t.coast_z(x)));
        }
    }

    /// Punto local (m) de una latitud y longitud, con el origen y la escala de meta.json.
    fn local(lat: f32, lon: f32) -> (f32, f32) {
        let (lat0, lon0) = (meta_num(META_JSON, "origen_noroeste", "lat"), meta_num(META_JSON, "origen_noroeste", "lon"));
        let (kx, ky) = (meta_num(META_JSON, "metros_por_grado", "lon"), meta_num(META_JSON, "metros_por_grado", "lat"));
        ((lon - lon0) * kx, (lat0 - lat) * ky)
    }

    #[test]
    fn el_entorno_amplio_tiene_la_geografia_real() {
        let t = TerrainGen::montevideo();
        let r = t.real.as_ref().unwrap();
        assert!(r.wide.inside(12000.0, 12000.0) >= 149000.0, "la grilla amplia llega a 150 km del centro del mapa");
        // Tierra adentro (Florida, San José, Canelones), las sierras de Maldonado, Punta del
        // Este, la costa argentina de Punta Indio y el medio del Río de la Plata.
        let alto = |lat: f32, lon: f32| {
            let (x, z) = local(lat, lon);
            assert!(r.ctx.inside(x, z) < 0.0, "({lat}, {lon}) está en el entorno de 100 m");
            t.height(x, z)
        };
        assert!(alto(-34.10, -56.21) > 30.0, "Florida");
        assert!(alto(-34.34, -56.71) > 10.0, "San José de Mayo");
        assert!(alto(-34.742, -55.324) > 300.0, "sierra de las Ánimas: {}", alto(-34.742, -55.324));
        assert!(alto(-34.90, -54.95) > 0.5, "Maldonado");
        assert!(alto(-35.05, -55.40) < 0.0, "el mar frente a Piriápolis");
        assert!(alto(-35.15, -56.60) < 0.0, "el medio del Río de la Plata");
        assert!(alto(-35.35, -57.35) > 0.5, "Punta Indio (Argentina): {}", alto(-35.35, -57.35));
        // Biomas del entorno amplio: hay pradera, monte o bañado, y nada de lo del entorno de
        // 100 m estirado (las costas de piedra de costa.json no llegan hasta acá).
        let (x, z) = local(-34.10, -56.21);
        let m = t.surface_material(x, z, t.height(x, z), 0.02);
        assert!(matches!(m, GRASS | GRASS_DRY | GRASS_LUSH | FOREST_FLOOR | MARSH), "material {m} en Florida");
    }

    #[test]
    fn sin_cortes_entre_las_grillas_del_mapa_el_entorno_y_el_amplio() {
        let t = TerrainGen::montevideo();
        let r = t.real.as_ref().unwrap();
        let (c_lo, c_hi, w_lo, w_hi) = (r.ctx.from, r.ctx.to(), r.wide.from, r.wide.to());
        // Altura continua a ambos lados de cada borde (mapa / entorno / amplio / afuera), en
        // los cuatro lados: a 1 m de distancia, menos de 1 m de diferencia.
        let mut worst = (0.0f32, 0.0, 0.0);
        for k in 0..120 {
            let s = k as f32 / 119.0;
            let along_c = c_lo + 500.0 + (c_hi - c_lo - 1000.0) * s;
            let along_w = w_lo + 500.0 + (w_hi - w_lo - 1000.0) * s;
            for (x, z, dx, dz) in [
                (0.0, 300.0 + 23400.0 * s, 1.0, 0.0),
                (MAP_SIZE, 300.0 + 23400.0 * s, 1.0, 0.0),
                (c_lo, along_c, 1.0, 0.0),
                (c_hi, along_c, 1.0, 0.0),
                (along_c, c_lo, 0.0, 1.0),
                (along_c, c_hi, 0.0, 1.0),
                (w_lo, along_w, 1.0, 0.0),
                (w_hi, along_w, 1.0, 0.0),
                (along_w, w_lo, 0.0, 1.0),
                (along_w, w_hi, 0.0, 1.0),
            ] {
                let (a, b) = (t.height(x - dx, z - dz), t.height(x + dx, z + dz));
                if (a - b).abs() > worst.0 {
                    worst = ((a - b).abs(), x, z);
                }
            }
        }
        assert!(worst.0 < 1.0, "salto de {} m en ({}, {})", worst.0, worst.1, worst.2);
        // Cerca del borde del entorno (dentro de la transición) tampoco hay escalones.
        for k in 0..400 {
            let z = c_lo + 2000.0 + k as f32 * 150.0;
            for x0 in [c_lo, c_hi - WIDE_BLEND] {
                let mut prev = t.height(x0, z);
                for i in 1..=60 {
                    let h = t.height(x0 + i as f32 * 50.0, z);
                    assert!((h - prev).abs() < 12.0, "escalón de {} m en ({}, {z})", h - prev, x0 + i as f32 * 50.0);
                    prev = h;
                }
            }
        }
        // Más allá de la grilla amplia no se estira el borde en líneas rectas: después de
        // WIDE_FADE el relieve es una llanura de ruido (o agua), distinto del valor del borde.
        let mut differ = 0;
        for k in 0..100 {
            let z = w_lo + 3000.0 + k as f32 * 2900.0;
            let edge = t.height(w_lo, z);
            let far = t.height(w_lo - WIDE_FADE - 2000.0, z);
            if edge > 0.0 {
                assert!(far > 5.0 && far < 70.0, "llanura afuera: {far} m (borde {edge} m)");
            } else {
                assert!(far < 0.0, "agua afuera: {far} (borde {edge})");
            }
            let mid = t.height(w_lo - WIDE_FADE * 0.5, z);
            differ += ((mid - edge).abs() > 0.5) as usize;
        }
        assert!(differ > 20, "afuera de la grilla amplia el relieve copia el borde ({differ} de 100 distintos)");
    }

    #[test]
    fn mapa_real_de_montevideo() {
        let t = TerrainGen::montevideo();
        assert!(t.height(FOUNDING_REAL.x, FOUNDING_REAL.y) > 2.0, "la Ciudad Vieja es tierra firme");
        assert!(t.height(CERRO_REAL.x, CERRO_REAL.y) > 100.0, "el Cerro");
        assert!(t.height(12000.0, 22000.0) < 0.0, "al sur, el Río de la Plata");
        assert!(t.arroyos.len() >= 10, "{} arroyos", t.arroyos.len());
        // El cauce de un arroyo es agua.
        let p = t.arroyos[0][t.arroyos[0].len() / 2];
        assert_eq!(t.surface_material(p.x, p.y, t.height(p.x, p.y), 0.0), STREAM);
        // Borde del mapa sin saltos: a ambos lados del límite la altura es parecida.
        for k in 0..50 {
            let z = 2000.0 + k as f32 * 400.0;
            let (a, b) = (t.height(MAP_SIZE - 1.0, z), t.height(MAP_SIZE + 1.0, z));
            assert!((a - b).abs() < 1.5, "salto en el borde: {a} / {b} en z={z}");
        }
        // Relieve suave: sin escalones entre muestras vecinas de 1 m en tierra firme.
        let mut worst: f32 = 0.0;
        for k in 0..2000 {
            let (x, z) = (9000.0 + (k % 40) as f32 * 37.0, 9000.0 + (k / 40) as f32 * 41.0);
            worst = worst.max((t.height(x + 1.0, z) - t.height(x, z)).abs());
        }
        assert!(worst < 1.5, "escalón de {worst} m en 1 m");
        // Hay árboles, sobre todo cerca de los arroyos.
        let mut trees = Vec::new();
        t.trees_in(p.x - 300.0, p.y - 300.0, p.x + 300.0, p.y + 300.0, &mut trees);
        let mut prairie = Vec::new();
        t.trees_in(9000.0, 9000.0, 9600.0, 9600.0, &mut prairie);
        assert!(trees.len() > prairie.len() * 3, "monte {} / pradera {}", trees.len(), prairie.len());
    }

    #[test]
    fn biomas_y_arroyos_oficiales() {
        let t = TerrainGen::montevideo();
        let r = t.real.as_ref().unwrap();
        let cuenta = |c: u8| r.bio_main.iter().filter(|&&b| b == c).count();
        // Playas (Pocitos, Carrasco, el oeste) y bañados (Pantanoso, Santa Lucía) de la IDE.
        assert!(cuenta(BIO_PLAYA) > 1000, "playas: {}", cuenta(BIO_PLAYA));
        assert!(cuenta(BIO_BANADO) > 5000, "bañados: {}", cuenta(BIO_BANADO));
        // Playa de Pocitos: arena en la orilla, con alguna piedra (11 % de roca medido, del lado
        // de Trouville; ver costa.json).
        let (mut pocitos, mut arena) = (0, 0);
        for k in 0..400 {
            let (x, z) = (15400.0 + (k % 20) as f32 * 60.0, 15200.0 + (k / 20) as f32 * 60.0);
            if t.official_biome(x, z) == BIO_PLAYA {
                let h = t.height(x, z);
                if h > -0.3 {
                    pocitos += 1;
                    let m = t.surface_material(x, z, h, 0.05);
                    arena += matches!(m, SAND | SAND_WET | STREAM | MUD) as i32;
                }
            }
        }
        assert!(pocitos > 0, "sin playa en Pocitos");
        assert!(arena as f32 >= pocitos as f32 * 0.8, "Pocitos: {arena} de {pocitos} con arena");
        // Sin árboles en los bañados.
        let mut trees = Vec::new();
        t.trees_in(0.0, 0.0, 3000.0, 3000.0, &mut trees);
        assert!(trees.iter().all(|a| t.official_biome(a.x, a.z) != BIO_BANADO));
        // Los arroyos traen su nombre oficial.
        let tiene = |n: &str| t.arroyo_nombres.iter().any(|a| a.as_deref() == Some(n));
        assert!(tiene("Arroyo Miguelete"), "falta el Miguelete");
        assert!(tiene("Arroyo Pantanoso"), "falta el Pantanoso");
        assert_eq!(t.arroyo_nombres.len(), t.arroyos.len());
        // Isla de las Ratas (isla oficial) y de Flores (en el entorno): tierra firme.
        let cumbre = |cx: f32, cz: f32| {
            (0..900).map(|k| t.height(cx - 300.0 + (k % 30) as f32 * 20.0, cz - 300.0 + (k / 30) as f32 * 20.0)).fold(f32::MIN, f32::max)
        };
        assert!(cumbre(8306.0, 12309.0) > 2.0, "Isla de las Ratas");
        assert!(cumbre(36217.0, 18388.0) > 2.0, "Isla de Flores");
        // Sin médanos en la costa rocosa de la Ciudad Vieja (sin playa oficial).
        assert!(t.dune_belt(11000.0, 15050.0) < 30.0, "médanos inventados en la Ciudad Vieja");
    }

    #[test]
    fn monte_nativo_de_1726() {
        let t = TerrainGen::montevideo();
        // El raster de monte existe y tiene monte ribereño a lo largo del Miguelete.
        let migue: Vec<usize> = (0..t.arroyos.len()).filter(|&k| t.arroyo_nombres[k].as_deref() == Some("Arroyo Miguelete")).collect();
        assert!(!migue.is_empty());
        let (mut junto, mut n) = (0.0, 0);
        for &k in &migue {
            for p in t.arroyos[k].iter().step_by(3) {
                if p.x > 0.0 && p.y > 0.0 && p.x < MAP_SIZE && p.y < MAP_SIZE {
                    junto += (0..8).map(|a| {
                        let ang = a as f32 * std::f32::consts::FRAC_PI_4;
                        t.forest_cover(p.x + ang.cos() * 25.0, p.y + ang.sin() * 25.0)
                    }).sum::<f32>() / 8.0;
                    n += 1;
                }
            }
        }
        let junto = junto / n as f32;
        // Cobertura media en tierra firme, lejos de todo.
        let (mut campo, mut m) = (0.0, 0);
        for k in 0..4000 {
            let (x, z) = (300.0 + (k % 80) as f32 * 290.0, 300.0 + (k / 80) as f32 * 230.0);
            if t.height(x, z) > 2.0 {
                campo += t.forest_cover(x, z);
                m += 1;
            }
        }
        let campo = campo / m as f32;
        assert!(junto > 0.3 && junto > campo * 5.0, "Miguelete {junto:.2}, campo {campo:.3}");
        // Galería más ancha en los arroyos grandes que en las cañadas: cobertura a 20–60 m del
        // agua según el ancho del cauce.
        let (mut ancho, mut na, mut angosto, mut nn) = (0.0, 0, 0.0, 0);
        for k in 0..250_000 {
            let (x, z) = (200.0 + (k % 500) as f32 * 47.0, 200.0 + (k / 500) as f32 * 31.0);
            let (d, hw) = t.arroyo_query(x, z);
            let e = d - hw;
            if (20.0..60.0).contains(&e) && t.height(x, z) > 1.0 {
                if hw > 4.0 {
                    ancho += t.forest_cover(x, z);
                    na += 1;
                } else if hw < 1.6 {
                    angosto += t.forest_cover(x, z);
                    nn += 1;
                }
            }
        }
        let (ancho, angosto) = (ancho / na as f32, angosto / nn as f32);
        assert!(na > 100 && nn > 100);
        assert!(ancho > 0.15 && ancho > angosto * 3.0, "a 20–60 m: arroyos {ancho:.3} ({na}), cañadas {angosto:.3} ({nn})");
        // Ni árboles en el agua, las playas, los bañados ni los bancos de arena; y la franja de
        // la orilla y el monte de las cañadas son angostos.
        let mut trees = Vec::new();
        t.trees_in(4000.0, 2000.0, 16000.0, 11000.0, &mut trees);
        assert!(trees.len() > 2000, "{} árboles", trees.len());
        let mut por_especie = std::collections::HashMap::new();
        for a in &trees {
            assert!(!t.is_water(a.x, a.z), "árbol en el agua ({}, {})", a.x, a.z);
            assert!(t.height(a.x, a.z) > 1.0);
            let b = t.official_biome(a.x, a.z);
            assert!(!matches!(b, BIO_PLAYA | BIO_BANADO | BIO_BANCO | BIO_LAGUNA), "árbol en bioma {b}");
            *por_especie.entry(a.species).or_insert(0) += 1;
            let (reach, _) = crate::trees::extent(&crate::trees::shape(&a.spec()));
            assert!(reach <= TerrainGen::MAX_TREE_RADIUS, "{:?} alcanza {reach} m", a.species);
        }
        // Monte variado: varias especies y variantes de forma.
        assert!(por_especie.len() >= 7, "especies: {por_especie:?}");
        let variantes: std::collections::HashSet<(Species, u8)> = trees.iter().map(|a| (a.species, a.variant)).collect();
        assert!(variantes.len() >= 20, "{} combinaciones de especie y variante", variantes.len());
        // Sauces, sarandíes y ceibos junto al agua; ombúes lejos.
        for a in &trees {
            let (d, hw) = t.arroyo_query(a.x, a.z);
            if a.species == Species::Ombu {
                assert!(d - hw > 5.0, "ombú en la orilla");
            }
        }
    }

    #[test]
    fn lee_los_nombres_de_los_arroyos() {
        let (a, n) = parse_arroyos(r#"{"arroyos":[[[0,0,2],[10,0,2]],[[5,5]],[[1,1,3],[2,2,3]]],"nombres":["Arroyo \"A\"",null,"Cañada B"]}"#);
        assert_eq!(a.len(), 2);
        assert_eq!(n, vec![Some("Arroyo \"A\"".to_string()), Some("Cañada B".to_string())]);
    }

    #[test]
    fn el_cerro_es_alto() {
        let t = TerrainGen::new(1726);
        assert!(t.height(CERRO.x, CERRO.y) > 100.0);
    }

    #[test]
    fn los_arroyos_tienen_agua() {
        let t = TerrainGen::new(1726);
        let p = t.arroyos[0][t.arroyos[0].len() / 2];
        assert!(t.height(p.x, p.y) < 0.0);
    }
}

