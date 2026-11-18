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
// Árboles nativos (ver docs/sistemas/vegetacion-historica.md).
pub const LEAF_SAUCE: Mat = 54;
pub const LEAF_ESPINILLO: Mat = 55;
pub const CEIBO_FLOR: Mat = 56;
pub const LEAF_TALA: Mat = 57;
pub const TRUNK_GRAY: Mat = 58;
// Flora baja (matas, juncos y flores; ver docs/sistemas/vegetacion-historica.md). La paleta del
// render tiene 64 lugares: con estos quedan todos ocupados.
pub const FLOR_AMARILLA: Mat = 59;
pub const FLOR_VIOLETA: Mat = 60;
pub const FLOR_BLANCA: Mat = 61;
pub const PASTO_MATA: Mat = 62;
pub const JUNCO: Mat = 63;
// Materiales de construcción que faltaban (ver docs/sistemas/materiales-de-construccion.md):
// la paleta del render se amplió a 80 lugares.
/// Tablas de madera nuevas (casas y techos de tablas).
pub const TABLAS: Mat = 64;
/// Tablas grises, curtidas por la intemperie.
pub const TABLAS_GRIS: Mat = 65;
/// Barro con paja (chorizo, quincha o terrón): las paredes de los ranchos más pobres.
pub const QUINCHA: Mat = 66;
pub const QUINCHA_DARK: Mat = 67;
/// Mampostería de piedra sin labrar asentada en barro (sin cal para la sillería).
pub const MAMPOSTERIA: Mat = 68;
pub const MAMPOSTERIA_DARK: Mat = 69;
// Variedad de la arquitectura colonial (ver docs/sistemas/arquitectura-colonial.md): los
// colores de la guía de fuentes (§4.2), con su variante manchada. La paleta del render se
// amplió a 96 lugares.
/// Blanco hueso o crema (`cal_hueso`, #E6DCC4) [E].
pub const CAL_HUESO: Mat = 70;
pub const HUESO_SUCIA: Mat = 71;
/// Almagre: rojo colonial de tierra (`almagre`, #A8553C) [E].
pub const CAL_ALMAGRE: Mat = 72;
pub const ALMAGRE_SUCIA: Mat = 73;
/// Gris perla: cal con negro de humo (`gris_cal`, #CFCBC2) [E].
pub const CAL_GRIS: Mat = 74;
pub const GRIS_SUCIA: Mat = 75;
/// Zócalos pintados: almagre oscuro (#8E4A35) y ocre oscuro (#9C7A44) [E].
pub const ZOCALO_ALMAGRE: Mat = 76;
pub const ZOCALO_OCRE: Mat = 77;
/// Carpinterías pintadas: marrón rojizo (#6B3A2A), gris azulado (#4F5D66) y ocre (#8A6A3A) [E].
pub const CARP_ROJIZA: Mat = 78;
pub const CARP_GRIS: Mat = 79;
pub const CARP_OCRE: Mat = 80;
/// Teja vieja con líquenes (#6F6A5A) [E].
pub const TILE_LIQUEN: Mat = 81;
// Época 2, Ciudad Nueva (ver docs/sistemas/arquitectura-epoca2.md y la paleta de
// docs/epocas/02-ciudad-nueva.md §11).
/// Revoque neoclásico ocre claro (#D9C29A) y su variante manchada.
pub const OCRE_CLARO: Mat = 82;
pub const OCRE_CLARO_SUCIA: Mat = 83;
/// Revoque "símil piedra" gris (#A8A39A) y su variante manchada.
pub const GRIS_PIEDRA: Mat = 84;
pub const GRIS_PIEDRA_SUCIA: Mat = 85;
/// Molduras, cornisas y pilastras en blanco piedra (#EEE9DF).
pub const BLANCO_PIEDRA: Mat = 86;
/// Hierro fundido de rejas, balcones, faroles y columnas (negro verdoso, #2F3530).
pub const HIERRO_FUNDIDO: Mat = 87;
/// Mármol de Carrara importado (escalones, zaguanes, patios en damero) y mármol gris.
pub const MARMOL: Mat = 88;
pub const MARMOL_GRIS: Mat = 89;
/// Vidrio del farol de gas: de noche brilla ámbar (#F2B862).
pub const LUZ_GAS: Mat = 90;
pub const COUNT: usize = 91;

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
    [90, 86, 54],    // hojarasca del monte
    [112, 144, 62],  // hojas de sauce criollo (verde claro)
    [118, 130, 70],  // hojas de espinillo (verde oliva, finas)
    [176, 40, 38],   // flor de ceibo
    [78, 96, 54],    // hojas de tala (verde grisáceo)
    [112, 104, 92],  // corteza gris (ombú, sauce viejo)
    [212, 184, 76],  // flor amarilla (senecio, margarita de campo, cola de zorro en flor)
    [150, 118, 168], // flor violeta (macachín, cardo, camalote)
    [232, 228, 212], // flor blanca (margarita, verbena blanca)
    [146, 146, 84],  // mata de paja (flechilla, paja brava, espartillo)
    [82, 104, 56],   // junco y totora
    [150, 116, 78],  // tablas de madera
    [128, 118, 102], // tablas grises (intemperie)
    [138, 110, 72],  // barro con paja
    [112, 90, 62],   // barro con paja húmedo
    [132, 124, 110], // mampostería de piedra
    [106, 100, 90],  // mampostería oscura
    [230, 220, 196], // cal hueso
    [206, 194, 168], // cal hueso manchada
    [176, 96, 70],   // almagre (un poco más claro que el pigmento: la cal lo aclara)
    [148, 82, 62],   // almagre manchado
    [207, 203, 194], // gris perla
    [180, 176, 166], // gris perla manchado
    [142, 74, 53],   // zócalo de almagre
    [156, 122, 68],  // zócalo ocre oscuro
    [107, 58, 42],   // carpintería marrón rojiza
    [79, 93, 102],   // carpintería gris azulada
    [138, 106, 58],  // carpintería ocre
    [128, 86, 62],   // teja vieja con líquenes
    [217, 194, 154], // revoque ocre claro (época 2)
    [192, 172, 138], // ocre claro manchado
    [168, 163, 154], // revoque gris piedra
    [146, 141, 133], // gris piedra manchado
    [238, 233, 223], // blanco piedra (molduras)
    [47, 53, 48],    // hierro fundido
    [228, 225, 218], // mármol blanco
    [142, 140, 136], // mármol gris
    [242, 184, 98],  // vidrio del farol de gas
];

/// ¿El material deja ver la luz de una vela de noche? (reservado para el ciclo día/noche)
pub fn is_window(m: Mat) -> bool {
    m == WINDOW || m == REJA
}

/// ¿El material es la luz de un farol (brilla de noche)?
pub fn is_lamp(m: Mat) -> bool {
    m == LUZ_GAS
}

pub fn color(m: Mat) -> [u8; 3] {
    COLORS[m as usize % COUNT]
}
