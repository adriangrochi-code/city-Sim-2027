//! Geografía del mapa: península, bahía, cerro, arroyos y cuchillas.
//! Es un puerto directo de `scripts/world/terrain_gen.gd` (mismas constantes y fórmulas),
//! para que el prototipo de Godot y el motor de vóxeles compartan el mismo mapa.

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
    seg_a: Vec<Vec2>,
    seg_b: Vec<Vec2>,
    seg_hash: HashMap<(i32, i32), Vec<u32>>,
    /// La misma tabla de segmentos en una grilla densa (más rápida que el HashMap).
    seg_grid: Vec<Vec<u32>>,
    grid_min: (i32, i32),
    grid_size: (i32, i32),
    pub arroyos: Vec<Vec<Vec2>>,
}

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
            seg_a: Vec::new(),
            seg_b: Vec::new(),
            seg_hash: HashMap::new(),
            seg_grid: Vec::new(),
            grid_min: (0, 0),
            grid_size: (0, 0),
            arroyos: Vec::new(),
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

    fn build_grid(&mut self) {
        let (mut x0, mut z0, mut x1, mut z1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(cx, cz) in self.seg_hash.keys() {
            x0 = x0.min(cx);
            z0 = z0.min(cz);
            x1 = x1.max(cx);
            z1 = z1.max(cz);
        }
        if x0 > x1 {
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
        let p = Vec2::new(x, z);
        let cx = (x / HASH_CELL).floor() as i32;
        let cz = (z / HASH_CELL).floor() as i32;
        let mut best = 1e9f32;
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
                {
                    let list = &self.seg_grid[(gx + gz * gw) as usize];
                    for &si in list {
                        let q = closest_on_segment(p, self.seg_a[si as usize], self.seg_b[si as usize]);
                        best = best.min(p.distance(q));
                    }
                }
            }
        }
        best
    }

    /// Material de la superficie según altura, pendiente y cercanía al agua.
    pub fn surface_material(&self, x: f32, z: f32, h: f32, slope: f32) -> Mat {
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

    fn add_arroyo(&mut self, raw: &[(f32, f32)]) {
        let raw: Vec<Vec2> = raw.iter().map(|&(x, z)| Vec2::new(x, z)).collect();
        let pts = self.smooth_path(&raw, 40.0);
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let si = self.seg_a.len() as u32;
            self.seg_a.push(a);
            self.seg_b.push(b);
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
