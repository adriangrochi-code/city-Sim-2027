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
// Variantes de desgaste y materiales de las estructuras (ver docs/sistemas/estetica-voxel.md).
pub const CAL_SUCIA: Mat = 26;
pub const OCRE_SUCIA: Mat = 27;
pub const ROSA_SUCIA: Mat = 28;
pub const ZOCALO: Mat = 29;
pub const TILE_DARK: Mat = 30;
pub const TILE_LIGHT: Mat = 31;
pub const THATCH_DARK: Mat = 32;
pub const ADOBE_DARK: Mat = 33;
pub const WOOD_DARK: Mat = 34;
pub const REJA: Mat = 35;
pub const STONE: Mat = 36;
pub const STONE_DARK: Mat = 37;
pub const HIDE: Mat = 38;
pub const SCAFFOLD: Mat = 39;
pub const FENCE: Mat = 40;
pub const DOME: Mat = 41;
pub const BRICK_DARK: Mat = 42;
pub const SACK: Mat = 43;
pub const LEAF_OMBU: Mat = 44;
pub const CAL_AZUL: Mat = 45;
pub const AZUL_SUCIA: Mat = 46;
// Biomas del mapa real.
pub const SAND_WET: Mat = 47;
pub const MARSH: Mat = 48;
pub const GRASS_LUSH: Mat = 49;
pub const COAST_ROCK: Mat = 50;
pub const DUNE: Mat = 51;
pub const STREAM: Mat = 52;
pub const FOREST_FLOOR: Mat = 53;
pub const COUNT: usize = 54;

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
    [142, 144, 84],  // pastizal seco
    [150, 104, 80],  // baldosa de azotea
    [228, 206, 158], // cal ocre
    [222, 182, 168], // cal rosa viejo
    [52, 78, 36],    // hojas oscuras (monte)
    [90, 110, 52],   // huerta
    [214, 208, 192], // cal manchada
    [204, 184, 140], // cal ocre manchada
    [200, 164, 152], // cal rosa manchada
    [150, 146, 134], // zócalo (revoque húmedo)
    [128, 64, 46],   // teja oscura
    [184, 108, 76],  // teja clara
    [168, 138, 80],  // paja vieja
    [126, 96, 66],   // adobe húmedo
    [72, 52, 36],    // madera oscura
    [30, 28, 27],    // ventana con reja
    [156, 150, 138], // sillería de piedra
    [128, 122, 112], // sillería oscura
    [150, 104, 64],  // cuero tendido
    [176, 146, 104], // madera de andamio
    [104, 92, 76],   // palo a pique
    [196, 172, 120], // cúpula (azulejo ocre)
    [130, 70, 50],   // ladrillo oscuro
    [196, 180, 140], // bolsas de harina
    [86, 120, 52],   // hojas de ombú
    [196, 210, 216], // cal celeste
    [176, 188, 192], // cal celeste manchada
    [156, 138, 104], // arena húmeda
    [104, 112, 62],  // bañado (juncos y paja brava)
    [108, 134, 64],  // pradera húmeda
    [98, 92, 86],    // piedra de la costa
    [216, 200, 158], // médano
    [64, 82, 82],    // agua del arroyo
    [86, 80, 52],    // hojarasca del monte
];

/// ¿El material deja ver la luz de una vela de noche? (reservado para el ciclo día/noche)
pub fn is_window(m: Mat) -> bool {
    m == WINDOW || m == REJA
}

pub fn color(m: Mat) -> [u8; 3] {
    COLORS[m as usize % COUNT]
}
