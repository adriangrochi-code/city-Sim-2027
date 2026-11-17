//! Geografía del mapa. Dos mapas:
//! - **Montevideo real** ([`TerrainGen::montevideo`], el del juego): el relieve de
//!   `assets/mapas/montevideo/` (24 × 24 km cada 20 m, más 6 km de entorno cada 100 m), los
//!   arroyos reales y biomas de la época de la fundación (pradera, monte ribereño, bañados,
//!   playas, médanos y puntas de piedra). Ver `docs/sistemas/mapa-real.md`.
//! - **Ficticio** ([`TerrainGen::new`]): puerto directo de `scripts/world/terrain_gen.gd`, usado
//!   por el sandbox, el banco de pruebas y las pruebas.

use crate::palette::*;
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
    /// Mapa real (alturas, distancia al mar). `None` = mapa ficticio.
    real: Option<Box<RealMap>>,
}

// --- Mapa real ------------------------------------------------------------------------------

/// Datos embebidos en el ejecutable (3 MB): así el mapa no puede faltar en el paquete.
const ALTURA_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/altura.bin");
const CONTEXTO_BIN: &[u8] = include_bytes!("../../assets/mapas/montevideo/contexto.bin");
const ARROYOS_JSON: &str = include_str!("../../assets/mapas/montevideo/arroyos.json");
const META_JSON: &str = include_str!("../../assets/mapas/montevideo/meta.json");
const MAIN_N: usize = 1201;
const MAIN_STEP: f32 = 20.0;

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
    /// Distancia (m) de cada celda de tierra al agua más cercana: transformada de distancia
    /// de chaflán en dos pasadas (rápida y suficiente para elegir playas y médanos).
    fn sea_distance(&self) -> Grid {
        let n = self.n;
        let big = 1e7f32;
        let mut d: Vec<f32> = self.v.iter().map(|&h| if h <= 0.0 { 0.0 } else { big }).collect();
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
}

/// Lee `{"arroyos": [[[x, z], ...], ...]}` sin dependencias: cada `[x, z]` es un punto y cada
/// lista de puntos, un arroyo.
fn parse_arroyos(txt: &str) -> Vec<Vec<(Vec2, f32)>> {
    // Profundidad de corchetes: 1 = lista de arroyos, 2 = un arroyo, 3 = un punto [x, z, ancho].
    let mut out: Vec<Vec<(Vec2, f32)>> = Vec::new();
    let mut depth = 0;
    let mut num = String::new();
    let mut pair: Vec<f32> = Vec::new();
    for ch in txt.chars() {
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
                }
            }
            c if depth == 3 && (c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+') => num.push(c),
            _ => {}
        }
    }
    out.retain(|a| a.len() >= 2);
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

/// Un árbol del paisaje (ver [`TerrainGen::trees_in`]).
#[derive(Clone, Copy, Debug)]
pub struct Tree {
    pub x: f32,
    pub z: f32,
    /// Altura de la copa (m sobre el suelo) y radio de la copa (m).
    pub height: f32,
    pub radius: f32,
    pub trunk: f32,
    pub leaf: Mat,
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
        let main = Grid::from_u16(ALTURA_BIN, MAIN_N, MAIN_STEP, 0.0);
        let ctx_n = meta_num(META_JSON, "\"contexto\"", "muestras") as usize;
        let ctx_step = meta_num(META_JSON, "\"contexto\"", "paso_m");
        let ctx_from = meta_num(META_JSON, "\"contexto\"", "desde_m");
        let ctx = Grid::from_u16(CONTEXTO_BIN, ctx_n, ctx_step, ctx_from);
        let (sea_main, sea_ctx) = (main.sea_distance(), ctx.sea_distance());
        t.real = Some(Box::new(RealMap { main, ctx, sea_main, sea_ctx }));
        for a in parse_arroyos(ARROYOS_JSON) {
            let (pts, widths): (Vec<Vec2>, Vec<f32>) = a.into_iter().unzip();
            t.add_arroyo_raw(pts, widths);
        }
        t.build_grid();
        t
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
    pub fn dune_belt(&self, x: f32, z: f32) -> f32 {
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
        if h < -0.3 {
            // Fondo: arena cerca de la orilla, limo más adentro.
            return if h > -2.5 { SAND_WET } else { MUD };
        }
        let sd = self.sea_distance(x, z);
        let (da, hw) = self.arroyo_query(x, z);
        // Arroyos y cañadas: agua en el centro del cauce y barro en las orillas.
        if da < hw {
            return STREAM;
        }
        if da < hw + 1.0 + hw * 0.3 + fine * 1.2 {
            return MUD;
        }
        // Costa: puntas de piedra donde es empinada; si no, playa (húmeda junto al agua),
        // médanos de arena suelta con matas de pasto en las crestas, y un borde irregular
        // hacia la pradera.
        if sd < 60.0 && slope > 0.32 + fine * 0.08 && x < 21000.0 {
            return COAST_ROCK;
        }
        if h < 0.7 && sd < 80.0 {
            return SAND_WET;
        }
        let belt = self.dune_belt(x, z);
        if sd < belt * 0.3 + 25.0 && h < 12.0 {
            return SAND;
        }
        if sd < belt && h < 25.0 {
            let edge = (sd - belt * 0.3) / (belt * 0.7); // 0 junto a la playa, 1 en el borde
            return if fine > 0.55 - edge * 0.9 { GRASS_DRY } else { DUNE };
        }
        if slope > 0.9 || (h > 105.0 && slope > 0.3 + fine * 0.1) {
            return ROCK;
        }
        // Bañados: bajos llanos junto a los arroyos y en sus desembocaduras.
        if slope < 0.05 && h < 5.0 + blot * 2.0 && (da < 25.0 + hw * 22.0 || (da < 700.0 && hw > 5.0 && sd < 350.0)) && fine > -0.35 {
            return MARSH;
        }
        // Monte ribereño: suelo de hojarasca bajo los árboles.
        if da < 4.0 + hw * 4.5 + blot * (2.0 + hw * 1.5) {
            return FOREST_FLOOR;
        }
        // Pradera: más verde y húmeda en los bajos, seca y amarillenta en las cuchillas.
        // Grano fino (metros) para que los bordes de las manchas no sean líneas limpias.
        let grain = self.n_detail.get_noise_2d(x * 14.0, z * 14.0) * 0.18;
        let dry = (h - 25.0) / 40.0 + blot * 0.6 + fine * 0.15 + grain;
        if dry > 0.45 {
            GRASS_DRY
        } else if (h < 18.0 && da < 60.0 + hw * 35.0 && grain > -0.05) || dry < -0.45 {
            GRASS_LUSH
        } else {
            GRASS
        }
    }

    /// Árboles con el tronco en el rectángulo `[x0, x1) × [z0, z1)` (en metros), según el bioma:
    /// monte cerrado en las orillas de los arroyos, algún coronilla y tala cerca de la costa, y
    /// árboles sueltos en la pradera (ombúes, talas). Determinista: el mismo lugar da el mismo
    /// árbol en todos los niveles de detalle. Solo el mapa real tiene árboles.
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
                // Descarte barato antes de calcular alturas: casi toda la pradera no tiene árboles.
                let (da, hw) = self.arroyo_query(x, z);
                let (dense, open) = (2.0 + hw * 4.0, 4.0 + hw * 7.5);
                let p_max = if da < open { 0.8 } else { 0.025 };
                if roll >= p_max {
                    continue;
                }
                let h = self.height(x, z);
                if h < 1.2 {
                    continue;
                }
                let (p, kind) = if da < hw + 2.0 {
                    (0.0, 0)
                } else if da < dense {
                    (0.8, 1) // monte ribereño cerrado
                } else if da < open {
                    (0.3, 1)
                } else {
                    let sd = self.sea_distance(x, z);
                    if sd < 110.0 || sd < self.dune_belt(x, z) {
                        (0.0, 0) // playa y médanos: arena suelta, sin árboles
                    } else if sd < 400.0 {
                        (0.01, 2) // matorral costero
                    } else if h > 100.0 {
                        (0.02, 2) // espinillos en el Cerro
                    } else {
                        (0.0035, 3) // árbol suelto en la pradera
                    }
                };
                if roll >= p {
                    continue;
                }
                let r2 = hashf(i, j, 4);
                let t = match kind {
                    1 => Tree { x, z, height: 5.5 + r2 * 4.0, radius: 2.4 + r2 * 1.6, trunk: 0.35, leaf: if hashf(i, j, 5) < 0.6 { LEAF_DARK } else { LEAF } },
                    2 => Tree { x, z, height: 3.5 + r2 * 2.0, radius: 1.8 + r2 * 1.0, trunk: 0.3, leaf: LEAF_DARK },
                    _ if hashf(i, j, 6) < 0.45 => Tree { x, z, height: 8.0 + r2 * 3.0, radius: 5.5 + r2 * 2.0, trunk: 1.0, leaf: LEAF_OMBU },
                    _ => Tree { x, z, height: 6.0 + r2 * 2.0, radius: 3.0 + r2 * 1.0, trunk: 0.4, leaf: LEAF },
                };
                out.push(t);
            }
        }
    }

    /// Radio máximo de copa (para buscar árboles que entran en una columna desde las vecinas).
    pub const MAX_TREE_RADIUS: f32 = 7.5;

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
