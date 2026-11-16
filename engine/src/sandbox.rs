//! Construcción interactiva: catálogo de estructuras para colocar con el ratón, excavar y
//! rellenar terreno. Todo se traduce a ediciones de vóxeles.

use crate::buildings::Edits;
use crate::palette::*;
use crate::raycast::solid;
use crate::store::LevelStore;
use crate::structures::colonial::{self, LOT_D, LOT_W};
use crate::structures::scenes::place;
use crate::structures::{self, Frame};
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3};
use std::collections::HashMap;

pub struct Kind {
    pub name: &'static str,
    pub width: i32,
    pub depth: i32,
    build: fn(&TerrainGen, &mut Edits, &Frame),
}

fn pulperia(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    colonial::pulperia(t, e, f, true)
}
fn iglesia(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    colonial::iglesia(t, e, f, 5)
}
fn iglesia_obra(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    colonial::iglesia(t, e, f, 2)
}
fn muralla(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    colonial::muralla(t, e, f, true, true)
}
fn aljibe(_t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, PLAZA);
    structures::aljibe(f, e, 1, 1);
}
fn ombu(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let c = f.origin + f.du * (f.width / 2) + f.dw * (f.depth / 2);
    structures::ombu(t, e, c, f.seed)
}

/// Catálogo en el orden de las teclas 1–9 (Tab recorre todos).
pub const CATALOG: &[Kind] = &[
    Kind { name: "Rancho", width: LOT_W, depth: LOT_D, build: colonial::rancho },
    Kind { name: "Casa de azotea", width: LOT_W, depth: LOT_D, build: colonial::casa_azotea },
    Kind { name: "Casa de altos", width: LOT_W, depth: LOT_D, build: colonial::casa_altos },
    Kind { name: "Pulpería", width: LOT_W, depth: LOT_D, build: pulperia },
    Kind { name: "Aljibe", width: 5, depth: 5, build: aljibe },
    Kind { name: "Ombú", width: 4, depth: 4, build: ombu },
    Kind { name: "Cabildo", width: 60, depth: 36, build: colonial::cabildo },
    Kind { name: "Iglesia matriz", width: 30, depth: 84, build: iglesia },
    Kind { name: "Muralla con portón", width: 60, depth: 12, build: muralla },
    Kind { name: "Iglesia en obra", width: 30, depth: 84, build: iglesia_obra },
    Kind { name: "Atahona", width: 40, depth: 40, build: colonial::atahona },
    Kind { name: "Molino", width: 22, depth: 24, build: colonial::molino },
    Kind { name: "Estancia", width: 100, depth: 90, build: colonial::estancia },
    Kind { name: "Saladero", width: 120, depth: 90, build: colonial::saladero },
];

/// Ejes del solar según el giro (0–3): el frente mira al sur, oeste, norte o este.
pub fn orient(rot: u32) -> (IVec2, IVec2) {
    match rot % 4 {
        0 => (IVec2::X, IVec2::NEG_Y),
        1 => (IVec2::NEG_Y, IVec2::NEG_X),
        2 => (IVec2::NEG_X, IVec2::Y),
        _ => (IVec2::Y, IVec2::X),
    }
}

/// Marco de la estructura centrado en un vóxel del suelo.
pub fn frame_at(t: &TerrainGen, kind: &Kind, center: IVec2, rot: u32, seed: u64) -> Frame {
    let (du, dw) = orient(rot);
    let origin = center - du * (kind.width / 2) - dw * (kind.depth / 2);
    place(t, origin, du, dw, kind.width, kind.depth, seed)
}

pub fn build(t: &TerrainGen, kind: &Kind, f: &Frame) -> Edits {
    let mut e = Edits::default();
    (kind.build)(t, &mut e, f);
    e
}

/// Esquinas del solar en vóxeles (mínimo y máximo, para dibujar el contorno).
pub fn footprint(f: &Frame) -> (IVec3, IVec3) {
    let a = f.origin;
    let b = f.origin + f.du * (f.width - 1) + f.dw * (f.depth - 1);
    (IVec3::new(a.x.min(b.x), f.base, a.y.min(b.y)), IVec3::new(a.x.max(b.x) + 1, f.base + 2, a.y.max(b.y) + 1))
}

/// Excava una esfera de radio `r` vóxeles: lo macizo pasa a ser aire.
pub fn dig(t: &TerrainGen, l0: &LevelStore, c: IVec3, r: i32) -> Edits {
    let mut e = Edits::default();
    let mut cache = HashMap::new();
    for y in -r..=r {
        for z in -r..=r {
            for x in -r..=r {
                if x * x + y * y + z * z > r * r {
                    continue;
                }
                let v = c + IVec3::new(x, y, z);
                if solid(t, l0, v, &mut cache) {
                    e.set(v, AIR);
                }
            }
        }
    }
    e
}

/// Rellena una esfera de radio `r` vóxeles con tierra y pasto arriba.
pub fn fill(t: &TerrainGen, l0: &LevelStore, c: IVec3, r: i32) -> Edits {
    let mut e = Edits::default();
    let mut cache = HashMap::new();
    for y in -r..=r {
        for z in -r..=r {
            for x in -r..=r {
                if x * x + y * y + z * z > r * r {
                    continue;
                }
                let v = c + IVec3::new(x, y, z);
                if !solid(t, l0, v, &mut cache) {
                    let above = (x * x + (y + 1) * (y + 1) + z * z) > r * r;
                    e.set(v, if above { GRASS } else { DIRT });
                }
            }
        }
    }
    e
}
