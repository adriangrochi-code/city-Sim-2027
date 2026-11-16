//! Ciudad colonial de prueba hecha de vóxeles: plaza mayor con la iglesia matriz, calles
//! empedradas en damero (cuadras de 84 m) y casas de los tres niveles del prototipo:
//! rancho de barro y paja, casa de azotea encalada y casa de altos con mirador.
//! También siembra ombúes y monte nativo.
//!
//! Todo se escribe como "ediciones" a la resolución base (vóxel de 0,5 m).

use crate::lod::BASE_VOXEL;
use crate::palette::*;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};
use std::collections::HashMap;

#[derive(Default)]
pub struct Edits {
    pub voxels: HashMap<IVec3, Mat>,
    /// Material de la capa superior del terreno (calles, plaza, huertas).
    pub surface: HashMap<IVec2, Mat>,
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
}

fn vox(m: f32) -> i32 {
    (m / BASE_VOXEL).floor() as i32
}

/// Altura del terreno en índice de vóxel base (el vóxel de superficie).
fn top_at(t: &TerrainGen, vx: i32, vz: i32) -> i32 {
    let h = t.height((vx as f32 + 0.5) * BASE_VOXEL, (vz as f32 + 0.5) * BASE_VOXEL);
    (h / BASE_VOXEL - 0.5).floor() as i32
}

/// Marco local de un solar: `u` a lo largo del frente, `w` hacia el fondo (0 = calle),
/// `y` hacia arriba desde la base.
#[derive(Clone, Copy)]
struct Lot {
    origin: IVec2, // esquina del frente (u = 0, w = 0) en vóxeles
    du: IVec2,
    dw: IVec2,
    base: i32,
    width: i32,
    depth: i32,
}

impl Lot {
    fn at(&self, u: i32, y: i32, w: i32) -> IVec3 {
        let p = self.origin + self.du * u + self.dw * w;
        IVec3::new(p.x, self.base + y, p.y)
    }
    fn fill(&self, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, w0: i32, w1: i32, m: Mat) {
        for y in y0..y1 {
            for w in w0..w1 {
                for u in u0..u1 {
                    e.set(self.at(u, y, w), m);
                }
            }
        }
    }
    /// Cimientos: rellena desde el punto más bajo del terreno hasta la base.
    fn foundation(&self, t: &TerrainGen, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, m: Mat) {
        for w in w0..w1 {
            for u in u0..u1 {
                let p = self.at(u, 0, w);
                let ground = top_at(t, p.x, p.z);
                for y in (ground + 1)..self.base {
                    e.set(IVec3::new(p.x, y, p.z), m);
                }
            }
        }
    }
    fn surface(&self, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, m: Mat) {
        for w in w0..w1 {
            for u in u0..u1 {
                let p = self.at(u, 0, w);
                e.surface.insert(IVec2::new(p.x, p.z), m);
            }
        }
    }
}

const CAL_COLORS: [Mat; 4] = [CAL, CAL, CAL_OCRE, CAL_ROSA];

fn rancho(t: &TerrainGen, e: &mut Edits, lot: &Lot, r: &mut Rng) {
    let bw = r.range(12, 15);
    let bd = r.range(9, 11);
    let u0 = (lot.width - bw) / 2 + r.range(-2, 2);
    let w0 = r.range(4, 9);
    let (u1, w1) = (u0 + bw, w0 + bd);
    let wall = 5;
    lot.foundation(t, e, u0, u1, w0, w1, ADOBE);
    lot.fill(e, u0, u1, 0, wall, w0, w1, ADOBE);
    // Techo de paja a dos aguas, escalonado vóxel a vóxel.
    let layers = (bd + 2) / 2;
    for k in 0..layers {
        lot.fill(e, u0 - 1, u1 + 1, wall + k, wall + k + 1, w0 - 1 + k, w1 + 1 - k, THATCH);
    }
    let door = u0 + bw / 3;
    lot.fill(e, door, door + 2, 0, 4, w0, w0 + 1, WOOD);
    lot.fill(e, u1 - 4, u1 - 2, 2, 4, w0, w0 + 1, WINDOW);
    lot.surface(e, 2, lot.width - 2, w1 + 3, (w1 + 13).min(lot.depth - 2), GARDEN);
}

fn azotea(t: &TerrainGen, e: &mut Edits, lot: &Lot, r: &mut Rng) {
    let h = r.range(9, 10);
    let fd = 14;
    let wall = CAL_COLORS[(r.next() % 4) as usize];
    lot.foundation(t, e, 0, lot.width, 0, fd, wall);
    lot.fill(e, 0, lot.width, 0, h, 0, fd, wall);
    lot.fill(e, 1, lot.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    lot.fill(e, 0, lot.width, h, h + 1, 0, 1, wall);
    lot.fill(e, 0, lot.width, h, h + 1, fd - 1, fd, wall);
    lot.fill(e, 0, 1, h, h + 1, 0, fd, wall);
    lot.fill(e, lot.width - 1, lot.width, h, h + 1, 0, fd, wall);
    let side = r.next() % 2 == 0;
    let door = if side { 2 } else { lot.width - 5 };
    lot.fill(e, door, door + 3, 0, 5, 0, 1, SHUTTER);
    for k in 0..2 {
        let wx = if side { 8 + k * 6 } else { lot.width - 11 - k * 6 };
        lot.fill(e, wx, wx + 2, 2, 6, 0, 1, WINDOW);
        lot.fill(e, wx - 1, wx + 3, 1, 2, -1, 0, IRON);
    }
    // Ala lateral alrededor del patio.
    let wing_u = if side { lot.width - 7 } else { 0 };
    let w1 = lot.depth - 8;
    lot.foundation(t, e, wing_u, wing_u + 7, fd, w1, wall);
    lot.fill(e, wing_u, wing_u + 7, 0, h - 1, fd, w1, wall);
    lot.fill(e, wing_u, wing_u + 7, h - 2, h - 1, fd, w1, AZOTEA);
    let patio_u = if side { 1 } else { 8 };
    lot.surface(e, patio_u, patio_u + lot.width - 9, fd, w1, PLAZA);
}

fn altos(t: &TerrainGen, e: &mut Edits, lot: &Lot, r: &mut Rng) {
    let h = 17;
    let fd = 18;
    let wall = CAL_COLORS[(r.next() % 4) as usize];
    lot.foundation(t, e, 0, lot.width, 0, fd, ROCK);
    lot.fill(e, 0, lot.width, 0, h, 0, fd, wall);
    lot.fill(e, 0, lot.width, 0, 1, 0, 1, ROCK);
    lot.fill(e, 1, lot.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    for (u0, u1, w0, w1) in [(0, lot.width, 0, 1), (0, lot.width, fd - 1, fd), (0, 1, 0, fd), (lot.width - 1, lot.width, 0, fd)] {
        lot.fill(e, u0, u1, h, h + 1, w0, w1, wall);
    }
    // Cornisa entre plantas.
    lot.fill(e, 0, lot.width, 8, 9, -1, 0, wall);
    let side = r.next() % 2 == 0;
    let door = if side { 2 } else { lot.width - 5 };
    lot.fill(e, door, door + 3, 1, 6, 0, 1, SHUTTER);
    for k in 0..2 {
        let wx = if side { 8 + k * 6 } else { lot.width - 11 - k * 6 };
        lot.fill(e, wx, wx + 2, 2, 6, 0, 1, WINDOW);
    }
    // Planta alta: tres puertas-ventana con balcón de hierro.
    for k in 0..3 {
        let wx = 3 + k * 7;
        lot.fill(e, wx, wx + 2, 10, 15, 0, 1, WINDOW);
        lot.fill(e, wx - 1, wx + 3, 9, 10, -2, 0, ROCK);
        lot.fill(e, wx - 1, wx + 3, 10, 12, -2, -1, IRON);
    }
    // Mirador en la azotea.
    let mu = if side { lot.width - 8 } else { 2 };
    lot.fill(e, mu, mu + 6, h, h + 6, 2, 8, wall);
    lot.fill(e, mu + 2, mu + 4, h + 2, h + 5, 1, 2, WINDOW);
    lot.fill(e, mu, mu + 6, h + 6, h + 7, 2, 8, AZOTEA);
    lot.fill(e, mu - 1, mu + 7, h + 6, h + 7, 1, 2, wall);
    let wing_u = if side { 0 } else { lot.width - 7 };
    let w1 = lot.depth - 8;
    lot.foundation(t, e, wing_u, wing_u + 7, fd, w1, wall);
    lot.fill(e, wing_u, wing_u + 7, 0, h - 2, fd, w1, wall);
    lot.fill(e, wing_u, wing_u + 7, h - 3, h - 2, fd, w1, AZOTEA);
}

fn church(t: &TerrainGen, e: &mut Edits, lot: &Lot) {
    let wall = CAL;
    let (wd, len, h) = (lot.width, lot.depth, 24);
    lot.foundation(t, e, 0, wd, 0, len, ROCK);
    lot.fill(e, 0, wd, 0, h, 8, len, wall);
    let layers = wd / 2 + 1;
    for k in 0..layers {
        lot.fill(e, -1 + k, wd + 1 - k, h + k, h + k + 1, 7, len + 1, TILE);
    }
    // Fachada con dos torres hacia la plaza.
    lot.fill(e, 0, wd, 0, h + 4, 8, 10, wall);
    lot.fill(e, wd / 2 - 2, wd / 2 + 2, 0, 9, 7, 8, WOOD);
    lot.fill(e, wd / 2 - 1, wd / 2 + 1, 13, 17, 7, 8, WINDOW);
    for tu in [0, wd - 9] {
        lot.fill(e, tu, tu + 9, 0, 44, 0, 9, wall);
        lot.fill(e, tu - 1, tu + 10, 36, 37, -1, 10, wall);
        for (a, b) in [(3, 6)] {
            lot.fill(e, tu + a, tu + b, 38, 42, -1, 0, WINDOW);
        }
        for k in 0..5 {
            lot.fill(e, tu + k, tu + 9 - k, 44 + k, 45 + k, k, 9 - k, TILE);
        }
    }
}

/// Construye la ciudad de prueba alrededor de la plaza y devuelve las ediciones.
pub fn colonial_city(t: &TerrainGen, plaza: Vec2, blocks: i32, seed: u64) -> Edits {
    let mut e = Edits::default();
    let mut r = Rng::new(seed);
    let block = 84.0;
    let road = 8.0;
    let half = block * (blocks as f32 + 0.5);
    let x0 = plaza.x - half;
    let z0 = plaza.y - half;
    let n_lines = blocks * 2 + 2;
    // Calles empedradas.
    for k in 0..n_lines {
        let line = -half + block * k as f32;
        for a in vox(-half - road * 0.5)..vox(half + road * 0.5) {
            for b in vox(line - road * 0.5)..vox(line + road * 0.5) {
                let (px, pz) = (vox(plaza.x) + a, vox(plaza.y) + b);
                if t.height((px as f32 + 0.5) * BASE_VOXEL, (pz as f32 + 0.5) * BASE_VOXEL) > 0.6 {
                    e.surface.insert(IVec2::new(px, pz), COBBLE);
                    e.surface.insert(IVec2::new(vox(plaza.x) + b, vox(plaza.y) + a), COBBLE);
                }
            }
        }
    }
    // Manzanas.
    let inner = block - road;
    let lot_w = 24; // 12 m
    let lot_d = 48; // 24 m
    for bz in 0..(n_lines - 1) {
        for bx in 0..(n_lines - 1) {
            let mx = vox(x0 + block * bx as f32 + road * 0.5);
            let mz = vox(z0 + block * bz as f32 + road * 0.5);
            let side = vox(inner);
            let center = Vec2::new((mx + side / 2) as f32 * BASE_VOXEL, (mz + side / 2) as f32 * BASE_VOXEL);
            let land = |vx: i32, vz: i32| t.height((vx as f32 + 0.5) * BASE_VOXEL, (vz as f32 + 0.5) * BASE_VOXEL) > 1.0;
            let mid = blocks;
            if bx == mid && bz == mid {
                plaza_block(t, &mut e, mx, mz, side);
                continue;
            }
            if bx == mid && bz == mid - 1 {
                let base = top_at(t, mx + side / 2, mz + side / 2) + 1;
                let ch = Lot { origin: IVec2::new(mx + side / 2 - 13, mz + side), du: IVec2::X, dw: IVec2::NEG_Y, base, width: 26, depth: 64 };
                church(t, &mut e, &ch);
                continue;
            }
            let dist = center.distance(plaza);
            // Solares a lo largo de los lados norte y sur, y dos más en cada lado este y oeste.
            let mut lots = Vec::new();
            let per = side / lot_w;
            for k in 0..per {
                lots.push((IVec2::new(mx + k * lot_w, mz), IVec2::X, IVec2::Y));
                lots.push((IVec2::new(mx + (k + 1) * lot_w - 1, mz + side - 1), IVec2::NEG_X, IVec2::NEG_Y));
            }
            for k in 0..2 {
                lots.push((IVec2::new(mx, mz + lot_d + 4 + (k + 1) * lot_w - 1 - 4), IVec2::NEG_Y, IVec2::X));
                lots.push((IVec2::new(mx + side - 1, mz + lot_d + k * lot_w), IVec2::Y, IVec2::NEG_X));
            }
            for (origin, du, dw) in lots {
                let c = origin + du * (lot_w / 2) + dw * (lot_d / 2);
                let corners = [origin, origin + du * (lot_w - 1), origin + dw * (lot_d - 1), origin + du * (lot_w - 1) + dw * (lot_d - 1)];
                if !corners.iter().all(|p| land(p.x, p.y)) {
                    continue;
                }
                let mut base = i32::MIN;
                for p in corners.iter().chain([&c]) {
                    base = base.max(top_at(t, p.x, p.y) + 1);
                }
                let lot = Lot { origin, du, dw, base, width: lot_w, depth: lot_d };
                let x = r.f();
                let kind = if dist < 220.0 {
                    if x < 0.35 { 3 } else if x < 0.85 { 2 } else { 1 }
                } else if dist < 360.0 {
                    if x < 0.08 { 3 } else if x < 0.55 { 2 } else if x < 0.92 { 1 } else { 0 }
                } else if x < 0.15 { 2 } else if x < 0.7 { 1 } else { 0 };
                match kind {
                    3 => altos(t, &mut e, &lot, &mut r),
                    2 => azotea(t, &mut e, &lot, &mut r),
                    1 => rancho(t, &mut e, &lot, &mut r),
                    _ => {}
                }
            }
        }
    }
    e
}

fn plaza_block(t: &TerrainGen, e: &mut Edits, mx: i32, mz: i32, side: i32) {
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
