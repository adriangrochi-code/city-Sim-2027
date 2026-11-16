//! Estructuras de vóxeles con un sistema modular de "kits" (ver
//! `docs/sistemas/estetica-voxel.md`).
//!
//! Cada edificio se arma sobre un [`Frame`] (el marco del solar) con piezas comunes:
//! zócalo, cuerpo, coronación (cornisa, pretil, almenas), vanos (puertas, ventanas, rejas,
//! balcones) y elementos (techos de teja y de paja, cúpulas, chimeneas, aljibes, andamios,
//! cercos de palo a pique). Todo es determinista: la misma semilla da el mismo edificio.
//!
//! Escala: vóxel de 0,5 m. El detalle más fino (barrotes de las rejas, canales de las tejas,
//! hebras de la paja, juntas de la sillería) lo dibuja el shader por material.

pub mod colonial;
pub mod scenes;

use crate::buildings::{top_at, Edits, Rng};
use crate::palette::*;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3};

/// Hash determinista de una posición y una semilla, en [0, 1).
pub fn hash(p: IVec3, seed: u64) -> f32 {
    let mut h = (p.x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (p.y as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (p.z as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ seed.wrapping_mul(0x27D4_EB2F_1656_67C5);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Desgaste por vóxel: devuelve la variante del material para esta posición.
/// `y` es la altura local sobre la base del edificio (la humedad sube desde el suelo).
pub fn weather(m: Mat, p: IVec3, y: i32, seed: u64) -> Mat {
    let h = hash(p, seed);
    let damp = if y <= 2 { 0.35 } else { 0.07 };
    match m {
        CAL => if h < damp { CAL_SUCIA } else { CAL },
        CAL_OCRE => if h < damp { OCRE_SUCIA } else { m },
        CAL_ROSA => if h < damp { ROSA_SUCIA } else { m },
        CAL_AZUL => if h < damp { AZUL_SUCIA } else { m },
        TILE => if h < 0.22 { TILE_DARK } else if h > 0.88 { TILE_LIGHT } else { TILE },
        THATCH => if h < 0.3 { THATCH_DARK } else { THATCH },
        ADOBE => if y <= 0 || h < 0.2 { ADOBE_DARK } else { ADOBE },
        WOOD => if h < 0.3 { WOOD_DARK } else { WOOD },
        BRICK => if h < 0.3 { BRICK_DARK } else { BRICK },
        STONE => {
            // Sillares de dos vóxeles en tresbolillo: el tono cambia por sillar, no por vóxel.
            let b = IVec3::new((p.x + p.z + (p.y & 1)) >> 1, p.y, 0);
            if hash(b, seed ^ 7) < 0.35 { STONE_DARK } else { STONE }
        }
        _ => m,
    }
}

/// Marco de un solar o edificio: `u` a lo largo del frente, `w` hacia el fondo
/// (0 = línea de la calle; negativo = sobre la vereda), `y` hacia arriba desde `base`.
/// Los ejes son siempre cardinales: en vóxeles los edificios no se giran en ángulos libres.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: IVec2,
    pub du: IVec2,
    pub dw: IVec2,
    pub base: i32,
    pub width: i32,
    pub depth: i32,
    pub seed: u64,
}

impl Frame {
    pub fn at(&self, u: i32, y: i32, w: i32) -> IVec3 {
        let p = self.origin + self.du * u + self.dw * w;
        IVec3::new(p.x, self.base + y, p.y)
    }
    /// Coordenadas locales (u, y, w) de un vóxel del mundo.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn local(&self, p: IVec3) -> (i32, i32, i32) {
        let d = IVec2::new(p.x, p.z) - self.origin;
        (d.dot(self.du), p.y - self.base, d.dot(self.dw))
    }
    pub fn rng(&self, salt: u64) -> Rng {
        Rng::new(self.seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
    /// Un vóxel con desgaste.
    pub fn set(&self, e: &mut Edits, u: i32, y: i32, w: i32, m: Mat) {
        let p = self.at(u, y, w);
        e.set(p, weather(m, p, y, self.seed));
    }
    /// Un vóxel tal cual (sin desgaste).
    pub fn raw(&self, e: &mut Edits, u: i32, y: i32, w: i32, m: Mat) {
        e.set(self.at(u, y, w), m);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn fill(&self, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, w0: i32, w1: i32, m: Mat) {
        for y in y0..y1 {
            for w in w0..w1 {
                for u in u0..u1 {
                    self.set(e, u, y, w, m);
                }
            }
        }
    }
    /// Vacía (quita vóxeles editados: vanos, galerías, patios).
    #[allow(clippy::too_many_arguments)]
    pub fn clear(&self, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, w0: i32, w1: i32) {
        for y in y0..y1 {
            for w in w0..w1 {
                for u in u0..u1 {
                    e.voxels.remove(&self.at(u, y, w));
                }
            }
        }
    }
    /// Material de la superficie del terreno (patios, calles, corrales).
    pub fn surface(&self, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, m: Mat) {
        for w in w0..w1 {
            for u in u0..u1 {
                let p = self.at(u, 0, w);
                e.surface.insert(IVec2::new(p.x, p.z), m);
            }
        }
    }
    /// Cimientos: rellena desde el terreno hasta la base (en laderas, el zócalo "baja").
    #[allow(clippy::too_many_arguments)]
    pub fn foundation(&self, t: &TerrainGen, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, m: Mat) {
        for w in w0..w1 {
            for u in u0..u1 {
                let p = self.at(u, 0, w);
                let ground = top_at(t, p.x, p.z);
                for y in (ground + 1)..self.base {
                    e.set(IVec3::new(p.x, y, p.z), weather(m, IVec3::new(p.x, y, p.z), 0, self.seed));
                }
            }
        }
    }
    /// Marco hijo con la misma orientación, desplazado dentro de este.
    pub fn sub(&self, u: i32, w: i32, width: i32, depth: i32, salt: u64) -> Frame {
        let p = self.origin + self.du * u + self.dw * w;
        Frame { origin: p, width, depth, seed: self.seed ^ salt.wrapping_mul(0x2545_F491_4F6C_DD1D), ..*self }
    }
    /// Marco cuyo frente es el lado izquierdo (u = 0) de este: para fachadas laterales.
    pub fn left_side(&self) -> Frame {
        Frame { origin: self.origin, du: self.dw, dw: self.du, width: self.depth, depth: self.width, ..*self }
    }
    /// Marco cuyo frente es el lado derecho (u = width - 1).
    pub fn right_side(&self) -> Frame {
        Frame { origin: self.origin + self.du * (self.width - 1), du: self.dw, dw: -self.du, width: self.depth, depth: self.width, ..*self }
    }
}

// --- Piezas del kit ---------------------------------------------------------------------

/// Anillo de un vóxel de espesor en el perímetro de un rectángulo, de y0 a y1.
#[allow(clippy::too_many_arguments)]
pub fn ring(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, w0: i32, w1: i32, m: Mat) {
    f.fill(e, u0, u1, y0, y1, w0, w0 + 1, m);
    f.fill(e, u0, u1, y0, y1, w1 - 1, w1, m);
    f.fill(e, u0, u0 + 1, y0, y1, w0, w1, m);
    f.fill(e, u1 - 1, u1, y0, y1, w0, w1, m);
}

/// Zócalo: las primeras hiladas de la fachada en otro material (revoque húmedo o piedra).
pub fn zocalo(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, h: i32, m: Mat) {
    ring(f, e, u0, u1, 0, h, w0, w1, m);
}

/// Cornisa: una hilada que sobresale medio metro de la fachada.
pub fn cornisa(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y: i32, m: Mat) {
    f.fill(e, u0, u1, y, y + 1, -1, 0, m);
}

/// Vano rehundido: se vacía la fachada `recess` vóxeles y al fondo va el material.
#[allow(clippy::too_many_arguments)]
pub fn opening(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, recess: i32, m: Mat) {
    f.clear(e, u0, u1, y0, y1, 0, recess);
    f.fill(e, u0, u1, y0, y1, recess, recess + 1, m);
}

/// Ventana con reja volada: vidrio rehundido, reja que sobresale, alféizar y sombrerete.
pub fn window_reja(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32) {
    opening(f, e, u0, u1, y0, y1, 1, WINDOW);
    f.fill(e, u0, u1, y0, y1, -1, 0, REJA);
    f.fill(e, u0 - 1, u1 + 1, y0 - 1, y0, -1, 0, STONE);
    f.fill(e, u0 - 1, u1 + 1, y1, y1 + 1, -1, 0, STONE);
}

/// Balcón: losa de piedra volada y baranda de hierro de 1 m (dos hiladas de reja).
pub fn balcony(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y: i32) {
    f.fill(e, u0, u1, y, y + 1, -2, 0, STONE);
    f.fill(e, u0, u1, y + 1, y + 3, -2, -1, REJA);
    f.fill(e, u0, u0 + 1, y + 1, y + 3, -1, 0, REJA);
    f.fill(e, u1 - 1, u1, y + 1, y + 3, -1, 0, REJA);
}

/// Arco de medio punto: vacía un vano con la parte superior redondeada, a través de
/// `w0..w1` (galerías, portones, campanarios).
#[allow(clippy::too_many_arguments)]
pub fn arch(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, w0: i32, w1: i32) {
    let wd = (u1 - u0) as f32;
    let r = wd * 0.5;
    let spring = (y1 - y0) as f32 - r;
    for u in u0..u1 {
        let cx = (u - u0) as f32 + 0.5 - r;
        let top = y0 as f32 + spring + (r * r - cx * cx).max(0.0).sqrt();
        f.clear(e, u, u + 1, y0, top.round() as i32, w0, w1);
    }
}

/// Techo a dos aguas escalonado. `along_u` = la cumbrera corre a lo largo del frente.
/// `run` vóxeles horizontales por cada vóxel de altura (2 ≈ 27°, como la teja colonial).
/// Los hastiales (tímpanos) quedan en el material de la pared.
#[allow(clippy::too_many_arguments)]
pub fn gable(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, y: i32, run: i32, over: i32, roof: Mat, wall: Mat, along_u: bool) {
    let (a0, a1, b0, b1) = if along_u { (u0, u1, w0, w1) } else { (w0, w1, u0, u1) };
    let mut k = 0;
    loop {
        let lo = b0 - over + k * run;
        let hi = b1 + over - k * run;
        if lo >= hi {
            break;
        }
        let ridge = hi - lo <= 2 * run;
        for b in lo..hi {
            let edge = b < lo + run || b >= hi - run || ridge;
            for a in (a0 - over)..(a1 + over) {
                let end = a < a0 || a >= a1;
                let m = if edge { roof } else if end { continue } else { wall };
                let (u, w) = if along_u { (a, b) } else { (b, a) };
                f.set(e, u, y + k, w, m);
            }
        }
        k += 1;
    }
}

/// Techo de paja: más empinado (45°), grueso y con alero caído.
pub fn thatch_roof(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, y: i32, wall: Mat) {
    gable(f, e, u0, u1, w0, w1, y, 1, 1, THATCH, wall, true);
    gable(f, e, u0, u1, w0, w1, y + 1, 1, 1, THATCH, wall, true);
    f.fill(e, u0 - 1, u1 + 1, y - 1, y, w0 - 1, w0, THATCH);
    f.fill(e, u0 - 1, u1 + 1, y - 1, y, w1, w1 + 1, THATCH);
}

/// Disco macizo (planta circular) de radio `r` centrado en (cu, cw).
#[allow(clippy::too_many_arguments)]
pub fn cylinder(f: &Frame, e: &mut Edits, cu: f32, cw: f32, r: f32, y0: i32, y1: i32, m: Mat) {
    let ri = r.ceil() as i32 + 1;
    for w in (cw as i32 - ri)..=(cw as i32 + ri) {
        for u in (cu as i32 - ri)..=(cu as i32 + ri) {
            let (du, dw) = (u as f32 + 0.5 - cu, w as f32 + 0.5 - cw);
            if du * du + dw * dw <= r * r {
                f.fill(e, u, u + 1, y0, y1, w, w + 1, m);
            }
        }
    }
}

/// Cono escalonado (techo de la atahona o del molino).
pub fn cone(f: &Frame, e: &mut Edits, cu: f32, cw: f32, r: f32, y: i32, m: Mat) {
    let mut k = 0;
    let mut rr = r;
    while rr > 0.6 {
        cylinder(f, e, cu, cw, rr, y + k, y + k + 1, m);
        rr -= 1.0;
        k += 1;
    }
}

/// Cúpula: media esfera de vóxeles sobre (cu, cw) desde la altura `y`.
pub fn dome(f: &Frame, e: &mut Edits, cu: f32, cw: f32, r: f32, y: i32, m: Mat) {
    let n = r.ceil() as i32;
    for k in 0..=n {
        let dy = k as f32 + 0.5;
        let rr = (r * r - dy * dy).max(0.0).sqrt() + 0.35;
        if rr < 0.6 {
            break;
        }
        cylinder(f, e, cu, cw, rr, y + k, y + k + 1, m);
    }
}

/// Chimenea de ladrillo con su remate.
pub fn chimney(f: &Frame, e: &mut Edits, u: i32, w: i32, y0: i32, y1: i32) {
    f.fill(e, u, u + 2, y0, y1, w, w + 2, BRICK);
    f.fill(e, u, u + 2, y1, y1 + 1, w, w + 2, BRICK_DARK);
}

/// Aljibe de patio: brocal encalado, boca oscura y arco de hierro con la roldana.
pub fn aljibe(f: &Frame, e: &mut Edits, u: i32, w: i32) {
    ring(f, e, u, u + 3, 0, 2, w, w + 3, CAL);
    f.raw(e, u + 1, 0, w + 1, MUD);
    for du in [0, 2] {
        f.fill(e, u + du, u + du + 1, 2, 5, w + 1, w + 2, IRON);
    }
    f.fill(e, u, u + 3, 5, 6, w + 1, w + 2, IRON);
    f.raw(e, u + 1, 4, w + 1, WOOD);
}

/// Cerco de palo a pique alrededor de un rectángulo, con una tranquera en el frente.
pub fn fence(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, gate_u: i32) {
    let post = |e: &mut Edits, u: i32, w: i32| {
        let h = if (u + w) % 3 == 0 { 3 } else { 2 };
        f.fill(e, u, u + 1, 0, h, w, w + 1, FENCE);
    };
    for u in u0..u1 {
        if (u - gate_u).abs() > 2 {
            post(e, u, w0);
        }
        post(e, u, w1 - 1);
    }
    for w in w0..w1 {
        post(e, u0, w);
        post(e, u1 - 1, w);
    }
}

/// Corral redondo de palo a pique.
pub fn round_fence(f: &Frame, e: &mut Edits, cu: f32, cw: f32, r: f32) {
    let steps = (r * 7.0) as i32;
    for k in 0..steps {
        let a = k as f32 / steps as f32 * std::f32::consts::TAU;
        if (a - 1.5).abs() < 0.12 {
            continue; // tranquera
        }
        let (u, w) = ((cu + a.cos() * r) as i32, (cw + a.sin() * r) as i32);
        f.fill(e, u, u + 1, 0, 2 + (k % 2), w, w + 1, FENCE);
    }
}

/// Andamio de madera frente a una fachada (w < 0): pies derechos cada 2 m y tablones
/// cada 2 m de altura.
pub fn scaffold(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y1: i32) {
    let mut u = u0;
    while u < u1 {
        f.raw_fill(e, u, u + 1, 0, y1, -3, -2, SCAFFOLD);
        u += 4;
    }
    let mut y = 3;
    while y < y1 {
        f.raw_fill(e, u0, u1, y, y + 1, -3, -1, SCAFFOLD);
        y += 4;
    }
}

impl Frame {
    /// Relleno sin desgaste (madera nueva de andamios, metales).
    #[allow(clippy::too_many_arguments)]
    pub fn raw_fill(&self, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, w0: i32, w1: i32, m: Mat) {
        for y in y0..y1 {
            for w in w0..w1 {
                for u in u0..u1 {
                    self.raw(e, u, y, w, m);
                }
            }
        }
    }
}

/// Ombú: tronco grueso con raíces que asoman y una copa ancha y achatada.
pub fn ombu(t: &TerrainGen, e: &mut Edits, p: IVec2, seed: u64) {
    let base = top_at(t, p.x, p.y) + 1;
    let mut r = Rng::new(seed);
    // Raíces y tronco.
    for (dx, dz, h) in [(-3, 0, 2), (3, 0, 2), (0, -3, 1), (0, 3, 2), (-2, -2, 1), (2, 2, 1)] {
        e.fill(IVec3::new(p.x + dx, base, p.y + dz), IVec3::new(p.x + dx + 2, base + h, p.y + dz + 2), TRUNK);
    }
    e.fill(IVec3::new(p.x - 1, base, p.y - 1), IVec3::new(p.x + 3, base + 6, p.y + 3), TRUNK);
    // Ramas gruesas.
    for (dx, dz) in [(-4, 0), (4, 0), (0, -4), (0, 4)] {
        e.fill(IVec3::new(p.x + dx / 2, base + 5, p.y + dz / 2), IVec3::new(p.x + dx / 2 + 2, base + 8, p.y + dz / 2 + 2), TRUNK);
    }
    let (rad, ry) = (r.range(11, 14), 5);
    let cy = base + 10;
    for y in -ry..=ry {
        for z in -rad..=rad {
            for x in -rad..=rad {
                let d = (x * x + z * z) as f32 / (rad * rad) as f32 + (y * y) as f32 / (ry * ry) as f32;
                if d <= 1.0 {
                    let q = IVec3::new(p.x + 1 + x, cy + y, p.y + 1 + z);
                    let leaf = if hash(q, seed) < 0.3 { LEAF } else { LEAF_OMBU };
                    e.set(q, leaf);
                }
            }
        }
    }
}

/// Animal de corral (vaca o caballo) de unos pocos vóxeles.
pub fn animal(f: &Frame, e: &mut Edits, u: i32, w: i32, along_u: bool, coat: Mat) {
    let (lu, lw) = if along_u { (4, 2) } else { (2, 4) };
    f.raw_fill(e, u, u + lu, 1, 3, w, w + lw, coat);
    // Patas.
    for (du, dw) in [(0, 0), (lu - 1, 0), (0, lw - 1), (lu - 1, lw - 1)] {
        f.raw(e, u + du, 0, w + dw, coat);
    }
    // Cabeza.
    if along_u {
        f.raw_fill(e, u + lu, u + lu + 1, 2, 4, w, w + 2, coat);
    } else {
        f.raw_fill(e, u, u + 2, 2, 4, w + lw, w + lw + 1, coat);
    }
}
