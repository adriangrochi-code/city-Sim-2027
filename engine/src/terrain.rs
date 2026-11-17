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
/// Monte nativo reconstruido para 1726 (cobertura de copas 0..255, misma grilla que las
/// alturas). Ver `docs/sistemas/vegetacion-historica.md`.
const BOSQUE_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/bosque.bin");
const BOSQUE_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/bosque_contexto.bin");
/// Rocosidad de los suelos CONEAT (0..255 = 0..100 % de roca a la vista).
const ROCAS_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/rocas.bin");
const ROCAS_CTX_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/rocas_contexto.bin");

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
        assert_eq!(bytes.len(), n * n * 2, "mapa real incompleto");
        let v = bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]]) as f32 / 200.0 - 50.0).collect();
        Grid { v, n, step, from }
    }
    /// Valores de un byte (0..255) llevados a 0..1.
    fn from_u8(bytes: &[u8], n: usize, step: f32, from: f32) -> Grid {
        assert_eq!(bytes.len(), n * n, "raster incompleto");
        Grid { v: bytes.iter().map(|&b| b as f32 / 255.0).collect(), n, step, from }
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
}

impl RealMap {
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
            seg_hash: HashMap::new(),
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
        let forest_main = Grid::from_u8(BOSQUE_BIN, main_n, main_step, 0.0);
        let forest_ctx = Grid::from_u8(BOSQUE_CTX_BIN, ctx_n, ctx_step, ctx_from);
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
        }));
        for txt in [ARROYOS_JSON, ARROYOS_CTX_JSON] {
            let (arroyos, names) = parse_arroyos(txt);
            for (a, name) in arroyos.into_iter().zip(names) {
                let (pts, widths): (Vec<Vec2>, Vec<f32>) = a.into_iter().unzip();
                t.add_arroyo_raw(pts, widths);
                t.arroyo_nombres.push(name);
            }
        }
        t.build_grid();
        t
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
        // Umbral para que la fracción del suelo por encima sea la rocosidad: cuantiles medidos
        // de `n` (media 0,5, desvío ≈ 0,11) para la fracción que queda arriba.
        const Q: [(f32, f32); 8] = [
            (0.005, 0.74), (0.01, 0.714), (0.05, 0.658), (0.10, 0.626),
            (0.15, 0.603), (0.20, 0.584), (0.30, 0.552), (0.50, 0.498),
        ];
        let c = c.min(0.5);
        let mut thr = Q[0].1;
        for w in Q.windows(2) {
            if c >= w[0].0 && c <= w[1].0 {
                let f = (c - w[0].0) / (w[1].0 - w[0].0);
                thr = w[0].1 + (w[1].1 - w[0].1) * f;
            }
        }
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
            r.forest_ctx.linear(x, z)
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
        let (da, hw) = self.arroyo_query(x, z);
        self.forest_density_at(x, z, da, hw)
    }

    /// Igual que [`forest_density`], con la distancia al eje del arroyo más cercano y su medio
    /// ancho ya calculados: el monte del raster o la galería del arroyo, lo que sea mayor.
    pub fn forest_density_at(&self, x: f32, z: f32, da: f32, hw: f32) -> f32 {
        self.raster_forest(x, z).max(self.gallery(x, z, da - hw, hw))
    }

    /// Monte de galería de 1726: en el Uruguay el monte ribereño acompañaba casi todo arroyo
    /// permanente, como una cinta continua de unas decenas de metros que se ensancha aguas
    /// abajo (Carrere 1990; ver docs/sistemas/vegetacion-historica.md). El raster de 2021 es lo
    /// que quedó de esa cinta (cortada por el pastoreo, la leña y la ciudad): aquí se repone.
    /// `edge`: distancia al borde del agua (m), `hw`: medio ancho del cauce (m).
    /// Ancho mínimo de la cinta desde la orilla (suposición, por debajo de los casos medidos
    /// de 100–200 m en arroyos del interior): ~20 m en una cañada de 2 m, ~30 m en un arroyo
    /// de 6 m, ~50 m en uno de 20 m, ~70 m en el Miguelete o el Pantanoso.
    pub fn gallery(&self, x: f32, z: f32, edge: f32, hw: f32) -> f32 {
        let w = hw * 2.0;
        let band = 8.0 + 9.0 * w.max(0.5).sqrt();
        if edge > band * 1.5 || edge < 0.0 || self.real.is_none() {
            return 0.0;
        }
        // Borde irregular: entrantes y salientes de decenas de metros.
        let lobe = self.n_hills.get_noise_2d(x * 18.0 + 211.0, z * 18.0 - 97.0);
        let b = band * (0.95 + 0.45 * lobe);
        // Las cañadas chicas (menos de 2 m) tienen monte a tramos; los arroyos, casi continuo.
        let gaps = self.n_hills.get_noise_2d(x * 9.0 - 333.0, z * 9.0 + 71.0);
        let cont = smoothstep(-0.45 + (w - 2.0).clamp(0.0, 4.0) * -0.12, -0.15, gaps);
        (1.0 - smoothstep(b * 0.75, b, edge)) * cont
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
            return r.ctx.cubic(x, z);
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
            return r.sea_ctx.linear(x, z);
        }
        let d = r.sea_main.linear(x, z);
        if edge < 400.0 {
            let w = smoothstep(0.0, 400.0, edge);
            return r.sea_ctx.linear(x, z) * (1.0 - w) + d * w;
        }
        d
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
        let mut h = self.real_raw(r, x, z);
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
        let p = Vec2::new(x, z);
        let cx = (x / HASH_CELL).floor() as i32;
        let cz = (z / HASH_CELL).floor() as i32;
        // Se elige el tramo cuyo borde de agua está más cerca (no su eje): así un arroyo
        // ancho "gana" a una cañada que le desemboca al lado.
        let (mut best, mut best_edge, mut best_hw) = (1e9f32, 1e9f32, ARROYO_AGUA);
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
                    }
                }
            }
        }
        (best, best_hw)
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
        // Ruido de manchas (cientos de metros) y de grano fino (metros).
        let blot = self.n_hills.get_noise_2d(x * 3.0, z * 3.0);
        let fine = self.n_detail.get_noise_2d(x * 2.5, z * 2.5);
        let sd = self.sea_distance(x, z);
        let (da, hw) = self.arroyo_query(x, z);
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
        // Monte nativo (bosque.bin): suelo de hojarasca bajo el monte cerrado; en el borde de
        // las manchas, pasto verde a la sombra entre la hojarasca.
        let grain = self.n_detail.get_noise_2d(x * 14.0, z * 14.0) * 0.18;
        let dens = self.forest_density_at(x, z, da, hw);
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
                let (da, hw) = self.arroyo_query(x, z);
                let dens = self.forest_density_at(x, z, da, hw);
                let edge = da - hw;
                if edge < 0.7 {
                    continue; // en el agua
                }
                let fringe = self.bank_fringe(x, z, edge, hw);
                let p_forest = 0.86 * dens;
                let p_fringe = 0.4 * fringe;
                if roll >= p_forest.max(p_fringe).max(0.025) {
                    continue;
                }
                let h = self.height(x, z);
                if h < 1.2 {
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
                        (0.01, pick(u, &[(Coronilla, 3.0), (Molle, 3.0), (Tala, 2.0)])) // matorral costero
                    } else if h > 100.0 || slope > 0.35 {
                        (0.02, pick(u, &[(Espinillo, 3.0), (Coronilla, 2.0), (Molle, 3.0)])) // serranía del Cerro
                    } else {
                        (0.0035, pick(u, &[(Ombu, 4.5), (Tala, 3.0), (Espinillo, 2.5)])) // árbol suelto en la pradera
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
        self.add_arroyo_raw(pts, w);
    }

    /// Registra un arroyo ya muestreado (los del mapa real vienen con meandros).
    fn add_arroyo_raw(&mut self, pts: Vec<Vec2>, widths: Vec<f32>) {
        for (k, w) in pts.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let si = self.seg_a.len() as u32;
            self.seg_a.push(a);
            self.seg_b.push(b);
            self.seg_hw.push((widths[k] + widths[k + 1]) * 0.25);
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
        // Playa de Pocitos: arena en la orilla.
        let mut pocitos = 0;
        for k in 0..400 {
            let (x, z) = (15400.0 + (k % 20) as f32 * 60.0, 15200.0 + (k / 20) as f32 * 60.0);
            if t.official_biome(x, z) == BIO_PLAYA {
                pocitos += 1;
                let h = t.height(x, z);
                if h > -0.3 {
                    let m = t.surface_material(x, z, h, 0.05);
                    assert!(m == SAND || m == SAND_WET || m == STREAM || m == MUD, "material {m} en la playa ({x}, {z})");
                }
            }
        }
        assert!(pocitos > 0, "sin playa en Pocitos");
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

