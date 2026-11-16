//! Materiales de los vóxeles y su paleta de colores rioplatense.
//! Un vóxel es un `u8`: 0 es aire; el resto, un material.
#![allow(dead_code)] // la paleta está completa aunque no todos los materiales se usen todavía

pub type Mat = u8;

pub const AIR: Mat = 0;
pub const GRASS: Mat = 1;
pub const DIRT: Mat = 2;
pub const SAND: Mat = 3;
pub const ROCK: Mat = 4;
pub const MUD: Mat = 5;
pub const CAL: Mat = 6;
pub const BRICK: Mat = 7;
pub const TILE: Mat = 8;
pub const THATCH: Mat = 9;
pub const WOOD: Mat = 10;
pub const ADOBE: Mat = 11;
pub const COBBLE: Mat = 12;
pub const DIRT_ROAD: Mat = 13;
pub const WINDOW: Mat = 14;
pub const IRON: Mat = 15;
pub const SHUTTER: Mat = 16;
pub const PLAZA: Mat = 17;
pub const LEAF: Mat = 18;
pub const TRUNK: Mat = 19;
pub const GRASS_DRY: Mat = 20;
pub const AZOTEA: Mat = 21;
pub const CAL_OCRE: Mat = 22;
pub const CAL_ROSA: Mat = 23;
pub const LEAF_DARK: Mat = 24;
pub const GARDEN: Mat = 25;
pub const COUNT: usize = 26;

/// Color base (sRGB) de cada material. El agua no es un vóxel: es una superficie aparte.
pub const COLORS: [[u8; 3]; COUNT] = [
    [0, 0, 0],       // aire
    [118, 132, 70],  // pastizal
    [124, 98, 70],   // tierra
    [190, 170, 128], // arena
    [120, 116, 108], // roca (granito)
    [96, 82, 62],    // barro del lecho del río
    [238, 232, 218], // cal
    [165, 88, 58],   // ladrillo
    [160, 84, 60],   // teja
    [201, 169, 99],  // paja
    [96, 70, 48],    // madera
    [156, 122, 84],  // adobe
    [142, 137, 128], // empedrado
    [140, 112, 80],  // camino de tierra
    [34, 30, 28],    // ventana
    [36, 36, 36],    // hierro
    [62, 90, 68],    // postigo verde
    [186, 178, 160], // piedra de la plaza
    [70, 98, 44],    // hojas
    [84, 64, 46],    // tronco
    [150, 150, 88],  // pastizal seco
    [150, 104, 80],  // baldosa de azotea
    [228, 206, 158], // cal ocre
    [222, 182, 168], // cal rosa viejo
    [52, 78, 36],    // hojas oscuras (monte)
    [90, 110, 52],   // huerta
];

/// ¿El material deja ver la luz de una vela de noche? (reservado para el ciclo día/noche)
pub fn is_window(m: Mat) -> bool {
    m == WINDOW
}

pub fn color(m: Mat) -> [u8; 3] {
    COLORS[m as usize % COUNT]
}
