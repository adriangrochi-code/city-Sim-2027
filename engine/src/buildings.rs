//! Ediciones de vóxeles (lo que se construye sobre el terreno), generador aleatorio
//! determinista y vegetación (ombúes, monte nativo, árboles de la plaza).
//! Los edificios están en `structures` y las escenas en `structures::scenes`.
//!
//! Todo se escribe como "ediciones" a la resolución base (vóxel de 0,5 m).

use crate::lod::BASE_VOXEL;
use crate::palette::*;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};

pub fn vox(m: f32) -> i32 {
    (m / BASE_VOXEL).floor() as i32
}
use std::collections::HashMap;

#[derive(Default)]
pub struct Edits {
    pub voxels: HashMap<IVec3, Mat>,
    /// Material de la capa superior del terreno (calles, plaza, huertas).
    pub surface: HashMap<IVec2, Mat>,
    /// Suelo nivelado por una obra (calle, plaza): índice del vóxel de arriba de la columna.
    /// Cada nivel de detalle usa esta altura en lugar de la natural, así el pasto no asoma
    /// por encima de lo aplanado en los niveles gruesos.
    pub ground: HashMap<IVec2, i32>,
}

impl Edits {
    pub fn set(&mut self, v: IVec3, m: Mat) {
        self.voxels.insert(v, m);
    }
    pub fn fill(&mut self, a: IVec3, b: IVec3, m: Mat) {
        for y in a.y..b.y {
            for z in a.z..b.z {
                for x in a.x..b.x {
                    self.voxels.insert(IVec3::new(x, y, z), m);
                }
            }
        }
    }
}

/// Generador pseudoaleatorio simple y determinista.
#[derive(Clone, Debug)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: i32, b: i32) -> i32 {
        a + (self.next() % ((b - a) as u64 + 1)) as i32
    }
    /// Estado interno (para guardar la partida y seguir la misma secuencia al cargarla).
    pub fn state(&self) -> u64 {
        self.0
    }
    /// Restaura el estado exacto (xorshift: cualquier estado distinto de cero es válido).
    pub fn from_state(s: u64) -> Self {
        Rng(if s == 0 { 1 } else { s })
    }
}

/// Altura del terreno en índice de vóxel base (el vóxel de superficie).
pub(crate) fn top_at(t: &TerrainGen, vx: i32, vz: i32) -> i32 {
    let h = t.height((vx as f32 + 0.5) * BASE_VOXEL, (vz as f32 + 0.5) * BASE_VOXEL);
    (h / BASE_VOXEL - 0.5).floor() as i32
}

pub fn plaza_block(t: &TerrainGen, e: &mut Edits, mx: i32, mz: i32, side: i32) {
    let base = top_at(t, mx + side / 2, mz + side / 2);
    for z in mz..mz + side {
        for x in mx..mx + side {
            e.surface.insert(IVec2::new(x, z), PLAZA);
        }
    }
    let c = IVec3::new(mx + side / 2, base + 1, mz + side / 2);
    e.fill(c + IVec3::new(-2, 0, -2), c + IVec3::new(2, 1, 2), ROCK);
    e.fill(c + IVec3::new(-1, 1, -1), c + IVec3::new(1, 10, 1), CAL);
    // Algunos árboles en la plaza.
    for (dx, dz) in [(-50, -50), (50, -50), (-50, 50), (50, 50)] {
        let p = IVec2::new(c.x + dx, c.z + dz);
        tree(t, e, p, 5, 3, LEAF);
    }
}

/// Árbol de vóxeles: tronco y copa elipsoidal.
pub fn tree(t: &TerrainGen, e: &mut Edits, p: IVec2, radius: i32, trunk_h: i32, leaf: Mat) {
    let base = top_at(t, p.x, p.y) + 1;
    e.fill(IVec3::new(p.x, base, p.y), IVec3::new(p.x + 2, base + trunk_h + 2, p.y + 2), TRUNK);
    let cy = base + trunk_h + radius * 2 / 3;
    let ry = (radius * 2 / 3).max(2);
    for y in -ry..=ry {
        for z in -radius..=radius {
            for x in -radius..=radius {
                let d = (x * x) as f32 / (radius * radius) as f32 + (y * y) as f32 / (ry * ry) as f32 + (z * z) as f32 / (radius * radius) as f32;
                if d <= 1.0 {
                    e.set(IVec3::new(p.x + 1 + x, cy + y, p.y + 1 + z), leaf);
                }
            }
        }
    }
}

/// Monte nativo junto a los arroyos y ombúes dispersos dentro de un radio.
pub fn vegetation(t: &TerrainGen, e: &mut Edits, center: Vec2, radius: f32, city_half: f32, seed: u64) {
    let mut r = Rng::new(seed);
    let in_city = |p: Vec2| (p.x - center.x).abs() < city_half && (p.y - center.y).abs() < city_half;
    for path in &t.arroyos {
        for (i, a) in path.iter().enumerate() {
            if i % 2 == 1 || a.distance(center) > radius {
                continue;
            }
            for _ in 0..4 {
                let off = Vec2::new(r.f() - 0.5, r.f() - 0.5).normalize_or_zero() * (14.0 + r.f() * 80.0);
                let p = *a + off;
                if t.height(p.x, p.y) > 1.0 && !in_city(p) {
                    tree(t, e, IVec2::new(vox(p.x), vox(p.y)), r.range(3, 5), r.range(2, 4), LEAF_DARK);
                }
            }
        }
    }
    for _ in 0..1400 {
        let p = center + Vec2::new(r.f() * 2.0 - 1.0, r.f() * 2.0 - 1.0) * radius;
        if t.height(p.x, p.y) > 1.5 && !in_city(p) {
            tree(t, e, IVec2::new(vox(p.x), vox(p.y)), r.range(5, 8), r.range(3, 5), LEAF);
        }
    }
}
