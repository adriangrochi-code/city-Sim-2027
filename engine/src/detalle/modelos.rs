//! Biblioteca de modelos de detalle (animales, carretas, botes y objetos de la calle), pintados
//! en micro-vóxeles de 1/16 m, y sus paletas (pelajes, maderas, pinturas).
//!
//! Ejes: +z hacia delante (la cabeza de los animales, la lanza de la carreta, la proa del
//! bote), +x a un costado, y hacia arriba; el origen es el suelo bajo el centro del modelo.

use super::{r, Id, Lienzo, Modelo, Paleta, Parte};
use std::f32::consts::PI;

/// Paleta base (lo que un modelo no cambia).
pub const BASE: Paleta = [
    [204, 158, 122], // piel
    [40, 30, 24],    // pelo
    [217, 209, 189], // ropa
    [89, 77, 64],    // calzón
    [38, 31, 26],    // sombrero
    [120, 90, 60],   // capa
    [50, 40, 32],    // calzado
    [150, 50, 40],   // acento
    [107, 69, 41],   // pelaje
    [70, 46, 30],    // pelaje oscuro
    [36, 28, 24],    // crin
    [118, 86, 56],   // madera
    [78, 58, 40],    // madera oscura
    [170, 136, 96],  // cuero
    [44, 42, 40],    // hierro
    [222, 216, 202], // claro
];

/// Pelajes de los caballos criollos: (pelaje, patas y hocico, crin).
pub const PELAJES: [([u8; 3], [u8; 3], [u8; 3]); 7] = [
    ([107, 69, 41], [52, 36, 26], [30, 24, 20]),    // zaino
    ([46, 36, 30], [34, 28, 24], [24, 20, 18]),     // morocho
    ([176, 134, 84], [96, 70, 48], [44, 34, 26]),   // bayo
    ([196, 192, 182], [150, 146, 138], [120, 116, 110]), // tordillo
    ([132, 76, 46], [100, 58, 36], [112, 64, 40]),  // alazán
    ([150, 108, 72], [70, 50, 36], [40, 30, 24]),   // gateado
    ([120, 104, 92], [70, 62, 56], [50, 44, 40]),   // lobuno
];

/// Pelajes del ganado criollo: colorado, hosco, negro, barcino, yaguané.
pub const BOVINOS: [([u8; 3], [u8; 3]); 5] =
    [([130, 72, 48], [96, 54, 38]), ([96, 60, 40], [60, 40, 30]), ([42, 36, 34], [30, 26, 24]), ([132, 92, 60], [84, 56, 38]), ([110, 72, 50], [70, 46, 34])];

const PERROS: [([u8; 3], [u8; 3]); 5] = [([150, 110, 70], [96, 70, 46]), ([60, 50, 44], [36, 30, 28]), ([196, 180, 150], [150, 130, 100]), ([120, 84, 52], [70, 50, 34]), ([170, 140, 100], [52, 44, 40])];
const GALLINAS: [([u8; 3], [u8; 3]); 4] = [([150, 82, 44], [70, 40, 26]), ([226, 220, 206], [196, 188, 170]), ([90, 84, 80], [40, 38, 36]), ([176, 130, 70], [110, 60, 34])];
/// Pinturas de los botes (el casco).
const BOTES: [[u8; 3]; 5] = [[52, 76, 64], [60, 70, 104], [140, 60, 44], [200, 196, 184], [110, 86, 60]];
/// Colores de la tabla de los letreros.
const LETREROS: [[u8; 3]; 4] = [[60, 80, 110], [140, 60, 44], [52, 90, 70], [190, 160, 90]];

/// Paleta de una instancia de un modelo: el pelaje o la pintura sale de la semilla.
pub fn paleta(id: Id, semilla: u32) -> Paleta {
    let s = super::ropa::mezcla(semilla as u64 ^ 0xC0FFEE);
    let mut p = BASE;
    let mut set = |k: u8, c: [u8; 3]| p[k as usize] = c;
    match id {
        Id::Caballo | Id::CaballoEnsillado => {
            let (a, b, c) = PELAJES[(s % PELAJES.len() as u64) as usize];
            set(r::PELAJE, a);
            set(r::PELAJE_OSC, b);
            set(r::CRIN, c);
            set(r::HIERRO, [40, 36, 34]);
            // Pellón del recado: lana cruda, negra o teñida.
            set(r::CAPA, [[214, 204, 180], [40, 36, 34], [110, 60, 44]][((s >> 8) % 3) as usize]);
        }
        Id::Vaca | Id::VacaOvera | Id::Buey => {
            let (a, b) = BOVINOS[(s % BOVINOS.len() as u64) as usize];
            set(r::PELAJE, a);
            set(r::PELAJE_OSC, b);
            set(r::CRIN, b);
            set(r::ACENTO, [214, 200, 170]);
            set(r::PIEL, [200, 150, 130]);
            set(r::HIERRO, [40, 36, 34]);
        }
        Id::Oveja => {
            set(r::CLARO, [[222, 214, 192], [206, 196, 170], [214, 206, 190]][(s % 3) as usize]);
            set(r::PELAJE_OSC, [[64, 56, 50], [190, 176, 150]][((s >> 4) % 2) as usize]);
        }
        Id::Perro => {
            let (a, b) = PERROS[(s % PERROS.len() as u64) as usize];
            set(r::PELAJE, a);
            set(r::PELAJE_OSC, b);
        }
        Id::Gallina => {
            let (a, b) = GALLINAS[(s % GALLINAS.len() as u64) as usize];
            set(r::PELAJE, a);
            set(r::PELAJE_OSC, b);
            set(r::ACENTO, [186, 40, 36]);
            set(r::HIERRO, [214, 170, 70]);
        }
        Id::Bote => set(r::CAPA, BOTES[(s % BOTES.len() as u64) as usize]),
        Id::Letrero => {
            set(r::CAPA, LETREROS[(s % LETREROS.len() as u64) as usize]);
            set(r::ACENTO, [226, 212, 170]);
        }
        Id::FarolAceite | Id::FarolGas | Id::FarolBrazo | Id::FarolGasBrazo => {
            set(r::CLARO, [236, 220, 170]);
            set(r::HIERRO, if matches!(id, Id::FarolGas | Id::FarolGasBrazo) { [36, 46, 40] } else { [40, 36, 34] });
        }
        // [ciudad] Hierro forjado: negro, o pintado de verde oscuro o de rojo óxido [E].
        Id::RejaPlana | Id::RejaPlanaAlta | Id::RejaVolada | Id::RejaVoladaAlta => {
            set(r::HIERRO, [[36, 34, 33], [34, 44, 38], [62, 40, 32], [30, 30, 32]][(s % 4) as usize]);
        }
        Id::Maceta => {
            // Barro cocido; geranios rojos, rosados o blancos y la hoja verde [E].
            set(r::ACENTO, [[166, 86, 54], [150, 78, 52], [182, 104, 70]][(s % 3) as usize]);
            set(r::CAPA, [[64, 104, 52], [52, 92, 46], [76, 110, 58]][((s >> 3) % 3) as usize]);
            set(r::CLARO, [[204, 40, 40], [222, 110, 150], [236, 232, 220], [190, 30, 60]][((s >> 6) % 4) as usize]);
        }
        // Aldaba de hierro o llamador de bronce.
        Id::Llamador => set(r::HIERRO, if s % 3 == 0 { [40, 36, 34] } else { [156, 118, 58] }),
        // [epoca3] Lona de rayas: verde, rojo, azul o marrón con crudo [E].
        Id::Toldo => {
            set(r::CAPA, [[52, 96, 64], [150, 46, 40], [44, 70, 120], [120, 84, 52]][(s % 4) as usize]);
            set(r::CLARO, [232, 222, 196]);
            set(r::HIERRO, [36, 34, 33]);
        }
        // [epoca3] Sábanas, camisas, delantales y pantalones de todos los colores.
        Id::RopaTendida => {
            set(r::ROPA, [[222, 216, 200], [176, 60, 52], [70, 96, 150], [200, 180, 120]][(s % 4) as usize]);
            set(r::CALZON, [[60, 58, 70], [96, 80, 60], [230, 226, 214]][((s >> 3) % 3) as usize]);
            set(r::ACENTO, [[214, 120, 60], [120, 150, 90], [190, 170, 200]][((s >> 6) % 3) as usize]);
            set(r::CLARO, [236, 234, 226]);
        }
        Id::BrocalAljibe | Id::Cruz => {
            set(r::CAPA, [206, 202, 194]);
            set(r::HIERRO, [36, 34, 34]);
        }
        Id::PozoRoldana => {
            set(r::CAPA, [150, 144, 132]);
            set(r::HIERRO, [24, 30, 34]);
        }
        // [ia] Oficios, poses y fachadas.
        Id::AparejoSereno => {
            // El farol de sebo encendido.
            set(r::CLARO, [250, 214, 130]);
            set(r::HIERRO, [40, 36, 34]);
        }
        Id::Silla => set(r::CAPA, [[196, 168, 108], [182, 150, 92], [170, 146, 100]][(s % 3) as usize]),
        Id::Postigo | Id::PostigoAbierto => {
            // Postigos pintados (verde, azul, pardo o la madera aceitada) [E].
            let k = (s % 5) as usize;
            set(r::MADERA, [[70, 92, 70], [64, 80, 104], [104, 78, 54], [118, 86, 56], [90, 70, 52]][k]);
            set(r::MADERA_OSC, [[50, 68, 52], [46, 58, 78], [76, 56, 40], [80, 60, 42], [64, 50, 38]][k]);
        }
        Id::Cancel => {
            set(r::HIERRO, [[36, 34, 33], [34, 44, 38], [30, 30, 32]][(s % 3) as usize]);
            set(r::CLARO, [176, 196, 200]);
        }
        Id::BancoPiedra => {
            // Piedra, o ladrillo revocado y blanqueado.
            set(r::CAPA, [[160, 154, 142], [214, 208, 194], [150, 120, 96]][(s % 3) as usize]);
            set(r::ACENTO, [[130, 124, 114], [184, 178, 166], [124, 98, 78]][(s % 3) as usize]);
        }
        Id::Carreta | Id::CarretaParada | Id::Carro | Id::CarroAguatero | Id::Picana | Id::BaldeEnMano => {
            let k = (s % 3) as usize;
            set(r::MADERA, [[118, 86, 56], [128, 98, 66], [104, 80, 58]][k]);
            set(r::MADERA_OSC, [[78, 58, 40], [86, 64, 44], [70, 54, 40]][k]);
            set(r::CUERO, [[176, 142, 100], [160, 124, 86], [190, 160, 118]][((s >> 5) % 3) as usize]);
            set(r::CAPA, [[200, 186, 150], [150, 120, 80], [110, 90, 66]][((s >> 9) % 3) as usize]);
        }
        _ => {
            let k = (s % 3) as usize;
            set(r::MADERA, [[118, 86, 56], [128, 98, 66], [104, 80, 58]][k]);
        }
    }
    p
}

/// ¿Lleva sombra de contacto en el suelo? (los animales y lo que tiene huella).
pub fn con_sombra(id: Id) -> bool {
    // Lo que cuelga de una fachada no toca el suelo.
    !matches!(id, Id::Bote | Id::Veleta | Id::RejaPlana | Id::RejaPlanaAlta | Id::RejaVolada | Id::RejaVoladaAlta | Id::FarolBrazo | Id::FarolGasBrazo | Id::Maceta | Id::Llamador | Id::AparejoSereno | Id::Picana | Id::BaldeEnMano | Id::Postigo | Id::PostigoAbierto | Id::Cancel | Id::Toldo)
}

/// Arma un modelo de la biblioteca.
pub fn armar(id: Id) -> Modelo {
    match id {
        Id::Caballo => caballo(false),
        Id::CaballoEnsillado => caballo(true),
        Id::Vaca => bovino(false, false),
        Id::VacaOvera => bovino(false, true),
        Id::Buey => bovino(true, false),
        Id::Oveja => oveja(),
        Id::Perro => perro(),
        Id::Gallina => gallina(),
        Id::Carreta => carreta(false),
        Id::CarretaParada => carreta(true),
        Id::Carro => carro(),
        Id::Bote => bote(),
        Id::FarolAceite => farol_aceite(),
        Id::FarolGas => farol_gas(),
        Id::Tonel => tonel(),
        Id::Barrica => barrica(),
        Id::Banco => banco(),
        Id::BrocalAljibe => brocal(),
        Id::PozoRoldana => pozo(),
        Id::Cruz => cruz(),
        Id::Veleta => veleta(),
        Id::Letrero => letrero(),
        Id::Palenque => palenque(),
        Id::RejaPlana => reja_plana(32),
        Id::RejaPlanaAlta => reja_plana(40),
        Id::RejaVolada => reja_volada(32),
        Id::RejaVoladaAlta => reja_volada(40),
        Id::FarolBrazo => farol_brazo(),
        Id::FarolGasBrazo => farol_gas_brazo(),
        Id::Maceta => maceta(),
        Id::Llamador => llamador(),
        Id::Toldo => toldo(), // [epoca3]
        Id::RopaTendida => ropa_tendida(),
        Id::CarroAguatero => carro_aguatero(),
        Id::AparejoSereno => aparejo_sereno(),
        Id::Picana => picana(),
        Id::BaldeEnMano => balde_en_mano(),
        Id::Silla => silla(),
        Id::Postigo => postigo(false),
        Id::PostigoAbierto => postigo(true),
        Id::Cancel => cancel(),
        Id::BancoPiedra => banco_piedra(),
    }
}

use Parte::*;

/// Cuatro patas de `ancho` × `fondo` vóxeles, de 0 a `alto`, en x = ±(sep..sep+ancho), con
/// las delanteras en z = zd y las traseras en z = zt (esquina menor); pezuñas o cascos de
/// `casco` (ranura) abajo y la parte baja de `baja` (ranura) hasta `rodilla`.
#[allow(clippy::too_many_arguments)]
fn patas(l: &mut Lienzo, sep: i32, ancho: i32, fondo: i32, alto: i32, zd: i32, zt: i32, alta: u8, baja: u8, rodilla: i32, casco: u8) {
    for (p, x0, z0) in [(PataDI, -sep - ancho, zd), (PataDD, sep, zd), (PataTI, -sep - ancho, zt), (PataTD, sep, zt)] {
        l.pivote(p, [x0 as f32 + ancho as f32 * 0.5, alto as f32, z0 as f32 + fondo as f32 * 0.5]);
        l.caja(p, alta, [x0, rodilla, z0], [x0 + ancho, alto, z0 + fondo]);
        l.caja(p, baja, [x0, 1, z0], [x0 + ancho, rodilla, z0 + fondo]);
        l.caja(p, casco, [x0, 0, z0], [x0 + ancho, 1, z0 + fondo]);
    }
}

/// Redondea las aristas largas (a lo largo de z) de un cuerpo de `-ax..ax` × `y0..y1`.
fn redondear(l: &mut Lienzo, ax: i32, y0: i32, y1: i32, z: [i32; 2]) {
    for (x, y) in [(-ax, y0), (ax - 1, y0), (-ax, y1 - 1), (ax - 1, y1 - 1)] {
        l.borrar([x, y, z[0]], [x + 1, y + 1, z[1]]);
    }
}

/// Caballo criollo (1,40 m a la cruz), con o sin recado.
fn caballo(ensillado: bool) -> Modelo {
    let mut l = Lienzo::new([-8, 0, -18], [8, 36, 28]);
    patas(&mut l, 1, 3, 3, 14, 8, -12, r::PELAJE, r::PELAJE_OSC, 7, r::HIERRO);
    l.pivote(Cabeza, [0.0, 18.0, 11.0]);
    l.pivote(Cola, [0.0, 21.0, -13.5]);
    // Cuello y cabeza (antes que el cuerpo: lo que se pisa con el cuerpo queda fijo). El
    // cuello sube hacia adelante y la cabeza baja en ángulo hacia el hocico.
    l.centrada(Cabeza, r::PELAJE, 2, [16, 23], [10, 15]);
    l.centrada(Cabeza, r::PELAJE, 2, [19, 26], [14, 18]);
    l.centrada(Cabeza, r::PELAJE, 2, [23, 28], [17, 21]);
    l.centrada(Cabeza, r::PELAJE, 2, [21, 26], [21, 24]);
    l.centrada(Cabeza, r::PELAJE, 2, [19, 24], [24, 27]);
    l.caja(Cabeza, r::PELAJE_OSC, [-2, 19, 26], [2, 22, 27]);
    l.caja(Cabeza, r::CRIN, [-2, 25, 20], [-1, 26, 21]);
    l.caja(Cabeza, r::CRIN, [1, 25, 20], [2, 26, 21]);
    l.caja(Cabeza, r::PELAJE, [-2, 28, 17], [-1, 30, 18]);
    l.caja(Cabeza, r::PELAJE, [1, 28, 17], [2, 30, 18]);
    // Crin y copete.
    l.caja(Cabeza, r::CRIN, [-1, 23, 10], [1, 24, 14]);
    l.caja(Cabeza, r::CRIN, [-1, 26, 14], [1, 27, 17]);
    l.caja(Cabeza, r::CRIN, [-1, 28, 18], [1, 29, 20]);
    // Cuerpo, pecho y anca.
    l.centrada(Fijo, r::PELAJE, 4, [13, 22], [-13, 13]);
    redondear(&mut l, 4, 13, 22, [-13, 13]);
    l.centrada(Fijo, r::PELAJE, 3, [14, 21], [13, 15]);
    l.centrada(Fijo, r::PELAJE, 3, [15, 21], [-14, -13]);
    // Cola.
    l.caja(Cola, r::CRIN, [-1, 9, -16], [1, 21, -14]);
    if ensillado {
        // Recado: bastos de cuero, pellón encima, cincha, estribos y cabezada.
        l.caja(Fijo, r::CUERO, [-4, 13, -1], [4, 22, 1]);
        l.centrada(Fijo, r::CUERO, 5, [21, 23], [-5, 5]);
        l.centrada(Fijo, r::CAPA, 4, [23, 24], [-4, 4]);
        for s in [-1, 1] {
            let x = if s < 0 { -6 } else { 5 };
            l.caja(Fijo, r::CUERO, [x, 15, 0], [x + 1, 21, 1]);
            l.caja(Fijo, r::HIERRO, [x, 13, -1], [x + 1, 15, 2]);
        }
        l.caja(Cabeza, r::CUERO, [-2, 22, 24], [2, 23, 25]);
        l.caja(Cabeza, r::CUERO, [-2, 23, 21], [2, 24, 22]);
    }
    l.armar(if ensillado { "caballo ensillado" } else { "caballo" })
}

/// Vaca criolla (con las astas en lira), overa o buey (más grande, astas largas).
fn bovino(buey: bool, overa: bool) -> Modelo {
    let mut l = Lienzo::new([-12, 0, -20], [12, 34, 30]);
    let (alto, ax, y1, z0, z1) = if buey { (13, 5, 25, -15, 15) } else { (12, 5, 23, -14, 13) };
    patas(&mut l, 2, 3, 3, alto, z1 - 5, z0 + 2, r::PELAJE, r::PELAJE, alto - 4, r::HIERRO);
    l.pivote(Cabeza, [0.0, (y1 - 4) as f32, (z1 - 5) as f32]);
    l.pivote(Cola, [0.0, (y1 - 3) as f32, (z0 - 1) as f32]);
    let (hz, hy) = (z1 + 4, y1 - 9);
    // Cuello corto, papada, cabeza ancha, hocico y astas.
    l.centrada(Cabeza, r::PELAJE, 3, [y1 - 10, y1 - 2], [z1 - 2, z1 + 4]);
    l.centrada(Cabeza, r::PELAJE, 1, [y1 - 13, y1 - 9], [z1, z1 + 5]);
    l.centrada(Cabeza, r::PELAJE, 3, [hy, hy + 8], [hz, hz + 5]);
    l.centrada(Cabeza, r::PELAJE, 2, [hy - 1, hy + 4], [hz + 5, hz + 8]);
    l.caja(Cabeza, r::PELAJE_OSC, [-2, hy - 1, hz + 7], [2, hy + 2, hz + 8]);
    l.caja(Cabeza, r::CRIN, [-3, hy + 5, hz + 2], [-2, hy + 6, hz + 3]);
    l.caja(Cabeza, r::CRIN, [2, hy + 5, hz + 2], [3, hy + 6, hz + 3]);
    l.caja(Cabeza, r::PELAJE, [-5, hy + 5, hz + 1], [-3, hy + 6, hz + 2]);
    l.caja(Cabeza, r::PELAJE, [3, hy + 5, hz + 1], [5, hy + 6, hz + 2]);
    let largo = if buey { 7 } else { 4 };
    l.caja(Cabeza, r::ACENTO, [-3 - largo, hy + 7, hz + 1], [3 + largo, hy + 8, hz + 2]);
    l.caja(Cabeza, r::ACENTO, [-4 - largo, hy + 8, hz + 1], [-3 - largo, hy + 11, hz + 2]);
    l.caja(Cabeza, r::ACENTO, [3 + largo, hy + 8, hz + 1], [4 + largo, hy + 11, hz + 2]);
    if overa {
        l.caja(Cabeza, r::CLARO, [-3, hy + 2, hz + 4], [3, hy + 8, hz + 5]);
    }
    // Cuerpo (con la giba del buey) y la cola con la borla.
    l.centrada(Fijo, r::PELAJE, ax, [alto - 1, y1], [z0, z1]);
    redondear(&mut l, ax, alto - 1, y1, [z0, z1]);
    if buey {
        l.centrada(Fijo, r::PELAJE, 3, [y1, y1 + 1], [z1 - 9, z1 - 2]);
    } else {
        l.centrada(Fijo, r::PIEL, 2, [alto - 3, alto - 1], [z0 + 4, z0 + 8]);
    }
    if overa {
        // Manchas blancas a los dos lados (atraviesan el cuerpo).
        l.centrada(Fijo, r::CLARO, ax, [alto, y1 - 4], [z0 + 3, z0 + 10]);
        l.centrada(Fijo, r::CLARO, ax, [alto + 3, y1], [z1 - 12, z1 - 6]);
        redondear(&mut l, ax, alto - 1, y1, [z0, z1]);
    }
    l.caja(Cola, r::PELAJE, [-1, y1 - 12, z0 - 2], [1, y1 - 1, z0]);
    l.caja(Cola, r::CRIN, [-1, y1 - 15, z0 - 2], [1, y1 - 12, z0]);
    l.armar(if buey { "buey" } else if overa { "vaca overa" } else { "vaca" })
}

fn oveja() -> Modelo {
    let mut l = Lienzo::new([-6, 0, -10], [6, 16, 14]);
    patas(&mut l, 1, 2, 2, 6, 3, -6, r::PELAJE_OSC, r::PELAJE_OSC, 3, r::HIERRO);
    l.pivote(Cabeza, [0.0, 10.0, 5.0]);
    l.pivote(Cola, [0.0, 11.0, -7.0]);
    l.caja(Cabeza, r::PELAJE_OSC, [-2, 9, 6], [2, 13, 10]);
    l.caja(Cabeza, r::PELAJE_OSC, [-1, 9, 10], [1, 11, 11]);
    l.caja(Cabeza, r::PELAJE_OSC, [-3, 11, 7], [-2, 12, 8]);
    l.caja(Cabeza, r::PELAJE_OSC, [2, 11, 7], [3, 12, 8]);
    l.caja(Cabeza, r::CLARO, [-2, 13, 6], [2, 14, 8]);
    l.centrada(Fijo, r::CLARO, 4, [5, 13], [-7, 7]);
    redondear(&mut l, 4, 5, 13, [-7, 7]);
    l.centrada(Fijo, r::CLARO, 2, [13, 14], [-5, 5]);
    l.caja(Cola, r::CLARO, [-1, 8, -8], [1, 11, -7]);
    l.armar("oveja")
}

fn perro() -> Modelo {
    let mut l = Lienzo::new([-4, 0, -10], [4, 18, 13]);
    patas(&mut l, 1, 1, 2, 6, 3, -5, r::PELAJE, r::PELAJE, 3, r::PELAJE_OSC);
    l.pivote(Cabeza, [0.0, 10.0, 5.0]);
    l.pivote(Cola, [0.0, 10.0, -6.0]);
    l.caja(Cabeza, r::PELAJE, [-1, 9, 4], [1, 11, 6]);
    l.caja(Cabeza, r::PELAJE, [-2, 10, 5], [2, 13, 9]);
    l.caja(Cabeza, r::PELAJE, [-1, 10, 9], [1, 12, 11]);
    l.caja(Cabeza, r::PELAJE_OSC, [-1, 11, 10], [1, 12, 11]);
    l.caja(Cabeza, r::PELAJE_OSC, [-2, 13, 6], [-1, 15, 7]);
    l.caja(Cabeza, r::PELAJE_OSC, [1, 13, 6], [2, 15, 7]);
    l.centrada(Fijo, r::PELAJE, 2, [6, 10], [-6, 5]);
    l.caja(Fijo, r::PELAJE_OSC, [-2, 9, -4], [2, 10, 3]);
    l.caja(Cola, r::PELAJE, [-1, 9, -8], [0, 10, -6]);
    l.caja(Cola, r::PELAJE, [-1, 10, -9], [0, 13, -8]);
    l.armar("perro")
}

fn gallina() -> Modelo {
    let mut l = Lienzo::new([-3, 0, -6], [3, 12, 7]);
    l.pivote(PataDI, [-1.5, 2.0, 0.5]);
    l.pivote(PataDD, [1.5, 2.0, 0.5]);
    l.pivote(Cabeza, [0.0, 6.0, 2.5]);
    l.pivote(Cola, [0.0, 5.0, -3.0]);
    l.caja(PataDI, r::HIERRO, [-2, 0, 0], [-1, 2, 1]);
    l.caja(PataDD, r::HIERRO, [1, 0, 0], [2, 2, 1]);
    l.caja(Cabeza, r::PELAJE, [-1, 5, 2], [1, 9, 4]);
    l.caja(Cabeza, r::ACENTO, [-1, 9, 2], [1, 10, 4]);
    l.caja(Cabeza, r::ACENTO, [-1, 5, 4], [1, 6, 5]);
    l.caja(Cabeza, r::HIERRO, [-1, 7, 4], [1, 8, 5]);
    l.centrada(Fijo, r::PELAJE, 2, [2, 6], [-3, 3]);
    l.borrar([-2, 5, -3], [2, 6, -2]);
    l.caja(Fijo, r::PELAJE_OSC, [-2, 3, -2], [-1, 5, 2]);
    l.caja(Fijo, r::PELAJE_OSC, [1, 3, -2], [2, 5, 2]);
    l.caja(Cola, r::PELAJE_OSC, [-1, 5, -5], [1, 8, -3]);
    l.armar("gallina")
}

/// Carreta de bueyes rioplatense: ruedas enormes de rayos, caja de madera con toldo de cuero
/// en arco, lanza larga y yugo (en marcha) o la lanza apoyada en el suelo (estacionada).
fn carreta(parada: bool) -> Modelo {
    let mut l = Lienzo::new([-15, 0, -31], [15, 42, 92]);
    let eje = 17;
    l.pivote(RuedaI, [-11.5, eje as f32, 0.0]);
    l.pivote(RuedaD, [11.5, eje as f32, 0.0]);
    // Eje, piso y barandas.
    l.caja(Fijo, r::MADERA_OSC, [-10, eje - 1, -2], [10, eje + 1, 2]);
    l.centrada(Fijo, r::MADERA, 9, [eje + 1, eje + 3], [-28, 28]);
    for x in [-9, 8] {
        l.caja(Fijo, r::MADERA_OSC, [x, eje + 3, -28], [x + 1, eje + 7, 28]);
        for k in 0..8 {
            let z = -28 + k * 8;
            l.caja(Fijo, r::MADERA, [x, eje + 3, z], [x + 1, eje + 8, z + 1]);
        }
    }
    // Toldo de cueros sobre arcos de caña (más alto en el medio), cerrado atrás.
    let alto = |x: i32| -> i32 {
        let u = (x as f32 + 0.5) / 10.5;
        eje + 8 + (10.0 * (1.0 - u * u).max(0.0).sqrt()).round() as i32
    };
    for x in -10..10 {
        let h = alto(x);
        l.caja(Fijo, r::CUERO, [x, h - 1, -27], [x + 1, h + 1, 27]);
        l.caja(Fijo, r::CUERO, [x, eje + 3, -29], [x + 1, h, -28]);
        if x == -10 || x == 9 {
            l.caja(Fijo, r::CUERO, [x, eje + 6, -27], [x + 1, h, 27]);
        }
    }
    // Arcos más oscuros cada tanto (las costuras de los cueros).
    for k in 0..5 {
        let z = -24 + k * 12;
        for x in -10..10 {
            let h = alto(x);
            l.caja(Fijo, r::MADERA_OSC, [x, h, z], [x + 1, h + 1, z + 1]);
        }
    }
    // Lanza (pértigo) y yugo.
    if parada {
        l.pivote(Fijo, [0.0, (eje + 1) as f32, 28.0]);
        l.girada(Fijo, r::MADERA, [-1, eje, 28], [1, eje + 2, 84], (eje as f32 / 56.0).atan());
    } else {
        l.caja(Fijo, r::MADERA, [-1, eje, 28], [1, eje + 2, 72]);
        l.caja(Fijo, r::MADERA, [-1, eje + 2, 70], [1, eje + 4, 87]);
        l.centrada(Fijo, r::MADERA_OSC, 14, [eje + 4, eje + 6], [84, 87]);
        for x in [-9, 6] {
            l.caja(Fijo, r::CUERO, [x, eje + 2, 85], [x + 3, eje + 4, 86]);
        }
    }
    let mut m = l;
    m.rueda(RuedaI, r::MADERA_OSC, r::MADERA, [-13, -10], 17, 3, 14, 12);
    m.rueda(RuedaD, r::MADERA_OSC, r::MADERA, [10, 13], 17, 3, 14, 12);
    m.armar(if parada { "carreta estacionada" } else { "carreta de bueyes" })
}

/// Carro de dos ruedas con varas para un caballo y la carga (bolsas y un barril).
fn carro() -> Modelo {
    let mut l = Lienzo::new([-12, 0, -17], [12, 30, 52]);
    let eje = 12;
    l.pivote(RuedaI, [-9.5, eje as f32, 0.0]);
    l.pivote(RuedaD, [9.5, eje as f32, 0.0]);
    l.caja(Fijo, r::MADERA_OSC, [-8, eje - 1, -1], [8, eje + 1, 1]);
    l.centrada(Fijo, r::MADERA, 7, [eje + 1, eje + 3], [-14, 14]);
    for x in [-7, 6] {
        l.caja(Fijo, r::MADERA_OSC, [x, eje + 3, -14], [x + 1, eje + 7, 14]);
    }
    l.centrada(Fijo, r::MADERA_OSC, 7, [eje + 3, eje + 7], [-14, -13]);
    // Varas hasta las paletas del caballo.
    for x in [-6, 5] {
        l.caja(Fijo, r::MADERA, [x, eje + 1, 14], [x + 1, eje + 3, 50]);
    }
    // Carga: bolsas de arpillera y un barril.
    l.caja(Fijo, r::CAPA, [-6, eje + 3, -12], [0, eje + 8, -3]);
    l.caja(Fijo, r::CAPA, [0, eje + 3, -12], [6, eje + 7, -4]);
    l.caja(Fijo, r::CAPA, [-5, eje + 8, -11], [2, eje + 11, -5]);
    l.cilindro_y(Fijo, r::MADERA_OSC, [0, 8], 4.0, [eje + 3, eje + 11]);
    l.cilindro_y(Fijo, r::HIERRO, [0, 8], 4.2, [eje + 5, eje + 6]);
    // Pescante.
    l.centrada(Fijo, r::MADERA_OSC, 6, [eje + 7, eje + 8], [9, 13]);
    let mut m = l;
    m.rueda(RuedaI, r::MADERA_OSC, r::MADERA, [-11, -8], 12, 2, 12, 10);
    m.rueda(RuedaD, r::MADERA_OSC, r::MADERA, [8, 11], 12, 2, 12, 10);
    m.armar("carro")
}

/// Bote de remos de 5 m (lancha de la bahía), con bancadas y remos; el origen es el fondo
/// de la quilla.
fn bote() -> Modelo {
    let mut l = Lienzo::new([-14, 0, -42], [14, 14, 44]);
    for z in -40..42 {
        let t = z as f32 / 41.0;
        // Proa aguda adelante, popa más llena.
        let hw = if t > 0.0 { 12.0 * (1.0 - t * t).max(0.0).sqrt() } else { 12.0 * (1.0 - t.powi(6)).max(0.0).sqrt() };
        let hw = ((hw / 1.5).round() * 1.5).round() as i32;
        if hw < 1 {
            continue;
        }
        let fondo = (3.0 * t.max(0.0).powi(2) * 3.0).round() as i32;
        let borda = 10 + (2.0 * t.abs().powi(3)).round() as i32;
        l.caja(Fijo, r::CAPA, [-hw, fondo, z], [hw, fondo + 1, z + 1]);
        l.caja(Fijo, r::CAPA, [-hw, fondo + 1, z], [-hw + 1, borda, z + 1]);
        l.caja(Fijo, r::CAPA, [hw - 1, fondo + 1, z], [hw, borda, z + 1]);
        l.caja(Fijo, r::MADERA_OSC, [-hw, borda, z], [-hw + 1, borda + 1, z + 1]);
        l.caja(Fijo, r::MADERA_OSC, [hw - 1, borda, z], [hw, borda + 1, z + 1]);
        l.caja(Fijo, r::MADERA, [-hw + 1, fondo + 1, z], [hw - 1, fondo + 2, z + 1]);
    }
    l.caja(Fijo, r::MADERA_OSC, [-1, 0, -40], [1, 1, 41]);
    for z in [-20, -2, 16] {
        l.centrada(Fijo, r::MADERA, 11, [7, 8], [z, z + 3]);
    }
    // Remos sobre las bancadas.
    l.caja(Fijo, r::MADERA, [-6, 8, -30], [-5, 9, 22]);
    l.caja(Fijo, r::MADERA, [5, 8, -26], [6, 9, 26]);
    l.caja(Fijo, r::MADERA, [-7, 8, 18], [-4, 9, 22]);
    l.armar("bote")
}

/// Farol colonial de aceite en su poste, con el brazo y el farol de cuatro vidrios.
fn farol_aceite() -> Modelo {
    let mut l = Lienzo::new([-4, 0, -2], [4, 44, 14]);
    l.centrada(Fijo, r::MADERA_OSC, 1, [0, 42], [-1, 1]);
    l.caja(Fijo, r::MADERA_OSC, [-1, 39, 1], [1, 41, 9]);
    l.caja(Fijo, r::MADERA_OSC, [-1, 36, 1], [1, 39, 2]);
    l.caja(Fijo, r::HIERRO, [0, 37, 7], [1, 39, 8]);
    farol(&mut l, 8, 28);
    l.armar("farol de aceite")
}

/// Farol de cuatro vidrios con su sombrerete, centrado en z = `zc`, desde y = `y0`.
fn farol(l: &mut Lienzo, zc: i32, y0: i32) {
    l.caja(Fijo, r::CLARO, [-3, y0 + 1, zc - 3], [3, y0 + 7, zc + 3]);
    l.caja(Fijo, r::HIERRO, [-3, y0, zc - 3], [3, y0 + 1, zc + 3]);
    l.caja(Fijo, r::HIERRO, [-2, y0 - 1, zc - 2], [2, y0, zc + 2]);
    for (x, z) in [(-3, zc - 3), (2, zc - 3), (-3, zc + 2), (2, zc + 2)] {
        l.caja(Fijo, r::HIERRO, [x, y0 + 1, z], [x + 1, y0 + 7, z + 1]);
    }
    l.caja(Fijo, r::HIERRO, [-4, y0 + 7, zc - 4], [4, y0 + 8, zc + 4]);
    l.caja(Fijo, r::HIERRO, [-2, y0 + 8, zc - 2], [2, y0 + 9, zc + 2]);
    l.caja(Fijo, r::HIERRO, [-1, y0 + 9, zc - 1], [1, y0 + 10, zc + 1]);
}

/// Farol de gas (para la época siguiente): columna de hierro fundido con base, anillos y la
/// barra para apoyar la escalera del farolero.
fn farol_gas() -> Modelo {
    let mut l = Lienzo::new([-6, 0, -6], [6, 62, 6]);
    l.centrada(Fijo, r::HIERRO, 3, [0, 3], [-3, 3]);
    l.centrada(Fijo, r::HIERRO, 2, [3, 6], [-2, 2]);
    l.centrada(Fijo, r::HIERRO, 1, [6, 46], [-1, 1]);
    for y in [14, 40] {
        l.centrada(Fijo, r::HIERRO, 2, [y, y + 2], [-2, 2]);
    }
    l.caja(Fijo, r::HIERRO, [-5, 42, -1], [5, 43, 1]);
    l.centrada(Fijo, r::HIERRO, 2, [46, 47], [-2, 2]);
    farol(&mut l, 0, 47);
    l.caja(Fijo, r::HIERRO, [0, 57, 0], [1, 60, 1]);
    l.armar("farol de gas")
}

/// Pipa (tonel parado) de duelas con aros de hierro.
fn tonel() -> Modelo {
    let mut l = Lienzo::new([-8, 0, -8], [8, 17, 8]);
    for (y0, y1, rad) in [(0, 2, 5.5), (2, 5, 6.3), (5, 11, 6.8), (11, 14, 6.3), (14, 16, 5.5)] {
        l.cilindro_y(Fijo, r::MADERA, [0, 0], rad, [y0, y1]);
    }
    for (y, rad) in [(1, 5.6), (4, 6.4), (11, 6.4), (14, 5.6)] {
        l.cilindro_y(Fijo, r::HIERRO, [0, 0], rad, [y, y + 1]);
    }
    l.cilindro_y(Fijo, r::MADERA_OSC, [0, 0], 4.6, [15, 16]);
    l.armar("tonel")
}

/// Barrica acostada sobre dos calzas.
fn barrica() -> Modelo {
    let mut l = Lienzo::new([-8, 0, -7], [8, 13, 7]);
    for (x0, x1, rad) in [(-7, -5, 4.2), (-5, -2, 4.9), (-2, 2, 5.3), (2, 5, 4.9), (5, 7, 4.2)] {
        l.cilindro_x(Fijo, r::MADERA, [12, 0], rad, [x0, x1]);
    }
    for (x, rad) in [(-6, 4.3), (-3, 5.0), (2, 5.0), (5, 4.3)] {
        l.cilindro_x(Fijo, r::HIERRO, [12, 0], rad, [x, x + 1]);
    }
    for x in [-5, 3] {
        l.caja(Fijo, r::MADERA_OSC, [x, 0, -4], [x + 2, 2, 4]);
    }
    l.armar("barrica")
}

fn banco() -> Modelo {
    let mut l = Lienzo::new([-15, 0, -4], [15, 9, 4]);
    l.centrada(Fijo, r::MADERA, 14, [7, 8], [-3, 3]);
    for x in [-12, 10] {
        l.caja(Fijo, r::MADERA_OSC, [x, 0, -2], [x + 2, 7, 2]);
    }
    l.caja(Fijo, r::MADERA_OSC, [-10, 2, -1], [10, 3, 1]);
    l.armar("banco")
}

/// Brocal de aljibe de mármol con el arco de hierro forjado, la roldana y el balde.
fn brocal() -> Modelo {
    let mut l = Lienzo::new([-10, 0, -10], [10, 44, 10]);
    l.centrada(Fijo, r::CAPA, 8, [0, 13], [-8, 8]);
    l.centrada(Fijo, r::CLARO, 9, [13, 14], [-9, 9]);
    l.centrada(Fijo, r::HIERRO, 6, [4, 14], [-6, 6]);
    for x in [-7, 6] {
        l.caja(Fijo, r::HIERRO, [x, 14, -1], [x + 1, 37, 1]);
    }
    for (x, y) in [(6, 37), (5, 38), (4, 39), (2, 40)] {
        l.caja(Fijo, r::HIERRO, [-x - 1, y, -1], [x + 1, y + 1, 1]);
    }
    l.caja(Fijo, r::HIERRO, [-1, 34, -2], [1, 38, 2]);
    l.caja(Fijo, r::CUERO, [0, 22, 0], [1, 34, 1]);
    l.caja(Fijo, r::MADERA_OSC, [-2, 18, -2], [2, 22, 2]);
    l.armar("brocal de aljibe")
}

/// Pozo de balde con roldana: brocal de piedra, dos horcones, el travesaño y la roldana.
fn pozo() -> Modelo {
    let mut l = Lienzo::new([-12, 0, -10], [12, 42, 10]);
    l.cilindro_y(Fijo, r::CAPA, [0, 0], 8.5, [0, 9]);
    l.cilindro_y(Fijo, r::HIERRO, [0, 0], 6.2, [0, 8]);
    for s in [-1, 1] {
        let x = if s < 0 { -11 } else { 9 };
        l.caja(Fijo, r::MADERA_OSC, [x, 0, -1], [x + 2, 36, 1]);
        l.caja(Fijo, r::MADERA_OSC, [x - 1, 36, -1], [x, 39, 1]);
        l.caja(Fijo, r::MADERA_OSC, [x + 2, 36, -1], [x + 3, 39, 1]);
    }
    l.caja(Fijo, r::MADERA, [-12, 36, -1], [12, 38, 1]);
    l.caja(Fijo, r::MADERA_OSC, [-1, 31, -2], [1, 36, 2]);
    l.caja(Fijo, r::CUERO, [0, 13, 0], [1, 31, 1]);
    l.caja(Fijo, r::MADERA_OSC, [-2, 9, -2], [2, 13, 2]);
    l.armar("pozo con roldana")
}

/// Cruz de madera sobre un pedestal de dos escalones.
fn cruz() -> Modelo {
    let mut l = Lienzo::new([-9, 0, -7], [9, 48, 7]);
    l.centrada(Fijo, r::CAPA, 6, [0, 3], [-6, 6]);
    l.centrada(Fijo, r::CAPA, 4, [3, 6], [-4, 4]);
    l.centrada(Fijo, r::MADERA_OSC, 1, [6, 46], [-1, 1]);
    l.centrada(Fijo, r::MADERA_OSC, 8, [35, 37], [-1, 1]);
    l.armar("cruz")
}

/// Veleta de hierro con los brazos de los puntos cardinales y la flecha.
fn veleta() -> Modelo {
    let mut l = Lienzo::new([-8, 0, -11], [8, 32, 11]);
    l.caja(Fijo, r::HIERRO, [0, 0, 0], [1, 30, 1]);
    l.caja(Fijo, r::HIERRO, [-6, 20, 0], [7, 21, 1]);
    l.caja(Fijo, r::HIERRO, [0, 20, -6], [1, 21, 7]);
    for (x, z) in [(-7, 0), (7, 0), (0, -7), (0, 7)] {
        l.caja(Fijo, r::HIERRO, [x, 19, z], [x + 1, 22, z + 1]);
    }
    l.pivote(Cabeza, [0.5, 28.0, 0.5]);
    l.caja(Cabeza, r::HIERRO, [0, 27, -8], [1, 28, 9]);
    l.caja(Cabeza, r::HIERRO, [0, 25, -10], [1, 30, -7]);
    l.caja(Cabeza, r::HIERRO, [0, 26, 9], [1, 29, 10]);
    l.armar("veleta")
}

/// Letrero de pulpería: poste, brazo y la tabla pintada colgada de dos ganchos.
fn letrero() -> Modelo {
    let mut l = Lienzo::new([-2, 0, -2], [2, 42, 16]);
    l.centrada(Fijo, r::MADERA_OSC, 1, [0, 40], [-1, 1]);
    l.caja(Fijo, r::MADERA_OSC, [-1, 37, 1], [1, 39, 15]);
    l.caja(Fijo, r::HIERRO, [0, 34, 5], [1, 37, 6]);
    l.caja(Fijo, r::HIERRO, [0, 34, 12], [1, 37, 13]);
    l.caja(Fijo, r::MADERA_OSC, [0, 24, 3], [1, 34, 15]);
    l.caja(Fijo, r::CAPA, [0, 25, 4], [1, 33, 14]);
    // El dibujo de la tabla (una franja y un racimo: vino y almacén).
    l.caja(Fijo, r::ACENTO, [0, 31, 5], [1, 32, 13]);
    l.caja(Fijo, r::ACENTO, [0, 26, 8], [1, 30, 10]);
    l.caja(Fijo, r::ACENTO, [0, 28, 7], [1, 30, 11]);
    l.armar("letrero de pulpería")
}

// --- [ciudad] Detalles de las fachadas -------------------------------------------------------
//
// Se cuelgan de la fachada: el origen es la cara del muro (z = 0, +z hacia la calle) en el
// borde de abajo del vano. Medidas [E] (rejas de 1 m de ancho con barrotes cada 12 cm).

/// Barrotes, marco y planchuelas de una reja de 1 m de ancho y `h` micro-vóxeles de alto, en
/// el plano z = `z`.
fn barrotes_reja(l: &mut Lienzo, h: i32, z: i32) {
    l.caja(Fijo, r::HIERRO, [-8, 0, z], [8, 1, z + 1]);
    l.caja(Fijo, r::HIERRO, [-8, h - 1, z], [8, h, z + 1]);
    for y in [h / 3, 2 * h / 3] {
        l.caja(Fijo, r::HIERRO, [-8, y, z], [8, y + 1, z + 1]);
    }
    for x in [-8, -5, -2, 1, 4, 7] {
        l.caja(Fijo, r::HIERRO, [x, 0, z], [x + 1, h, z + 1]);
    }
}

/// Reja embutida en el vano: barrotes y dos planchuelas, con los extremos de las planchuelas
/// metidos en el muro.
fn reja_plana(h: i32) -> Modelo {
    let mut l = Lienzo::new([-9, 0, 0], [9, h, 1]);
    barrotes_reja(&mut l, h, 0);
    for y in [h / 3, 2 * h / 3] {
        l.caja(Fijo, r::HIERRO, [-9, y, 0], [9, y + 1, 1]);
    }
    l.armar(if h > 32 { "reja plana alta" } else { "reja plana" })
}

/// Reja volada: jaula que sale medio metro (frente y costados de barrotes), con base y techo
/// de planchuela y una voluta en cada esquina de abajo.
fn reja_volada(h: i32) -> Modelo {
    let mut l = Lienzo::new([-9, -2, 0], [9, h, 9]);
    barrotes_reja(&mut l, h, 7);
    for x in [-8, 7] {
        for z in [0, 3, 5] {
            l.caja(Fijo, r::HIERRO, [x, 0, z], [x + 1, h, z + 1]);
        }
        l.caja(Fijo, r::HIERRO, [x, 0, 0], [x + 1, 1, 8]);
        l.caja(Fijo, r::HIERRO, [x, h - 1, 0], [x + 1, h, 8]);
        l.caja(Fijo, r::HIERRO, [x, h / 2, 0], [x + 1, h / 2 + 1, 8]);
        // Voluta: una ménsula de hierro que apoya la jaula en el muro.
        l.caja(Fijo, r::HIERRO, [x, -1, 1], [x + 1, 0, 6]);
        l.caja(Fijo, r::HIERRO, [x, -2, 0], [x + 1, -1, 2]);
    }
    l.armar(if h > 32 { "reja volada alta" } else { "reja volada" })
}

/// Farol de aceite de fachada: placa en el muro, brazo con tornapunta y el farol colgado.
fn farol_brazo() -> Modelo {
    let mut l = Lienzo::new([-5, 0, 0], [5, 20, 14]);
    l.centrada(Fijo, r::HIERRO, 1, [9, 19], [0, 1]);
    l.centrada(Fijo, r::HIERRO, 1, [16, 17], [1, 11]);
    for k in 0..5 {
        l.centrada(Fijo, r::HIERRO, 1, [11 + k, 12 + k], [1 + k * 2, 3 + k * 2]);
    }
    l.centrada(Fijo, r::HIERRO, 1, [12, 16], [9, 10]);
    farol(&mut l, 9, 1);
    l.armar("farol de brazo")
}

/// Farol de gas de fachada: brazo con voluta y farol más alto, de cuatro vidrios con el
/// sombrerete en punta.
fn farol_gas_brazo() -> Modelo {
    let mut l = Lienzo::new([-6, 0, 0], [6, 26, 18]);
    l.centrada(Fijo, r::HIERRO, 2, [12, 24], [0, 1]);
    l.centrada(Fijo, r::HIERRO, 1, [21, 22], [1, 13]);
    // Voluta bajo el brazo.
    for (y, z0, z1) in [(20, 2, 4), (19, 3, 5), (18, 4, 6), (17, 5, 8), (18, 8, 9), (19, 8, 10), (20, 9, 10)] {
        l.centrada(Fijo, r::HIERRO, 1, [y, y + 1], [z0, z1]);
    }
    l.centrada(Fijo, r::HIERRO, 1, [16, 21], [11, 12]);
    let zc = 11;
    l.caja(Fijo, r::CLARO, [-3, 3, zc - 3], [3, 12, zc + 3]);
    l.caja(Fijo, r::HIERRO, [-4, 2, zc - 4], [4, 3, zc + 4]);
    l.caja(Fijo, r::HIERRO, [-2, 0, zc - 2], [2, 2, zc + 2]);
    for (x, z) in [(-4, zc - 4), (3, zc - 4), (-4, zc + 3), (3, zc + 3)] {
        l.caja(Fijo, r::HIERRO, [x, 3, z], [x + 1, 12, z + 1]);
    }
    for (k, y) in (12..16).enumerate() {
        let k = 4 - k as i32;
        l.caja(Fijo, r::HIERRO, [-k, y, zc - k], [k.max(1), y + 1, zc + k.max(1)]);
    }
    l.armar("farol de gas de brazo")
}

/// Maceta de barro con un geranio.
fn maceta() -> Modelo {
    let mut l = Lienzo::new([-5, 0, -5], [5, 11, 5]);
    l.cilindro_y(Fijo, r::ACENTO, [0, 0], 2.4, [0, 3]);
    l.cilindro_y(Fijo, r::ACENTO, [0, 0], 3.0, [3, 5]);
    l.cilindro_y(Fijo, r::CAPA, [0, 0], 3.6, [5, 8]);
    l.cilindro_y(Fijo, r::CAPA, [0, 0], 2.4, [8, 9]);
    for (x, y, z) in [(-2, 8, 1), (1, 9, -1), (2, 7, 2), (-1, 9, -2), (0, 10, 0), (-3, 7, -1)] {
        l.punto(Fijo, r::CLARO, x, y, z);
    }
    l.armar("maceta")
}

/// Aldaba de la puerta de calle: placa y anillo (o la mano del llamador).
fn llamador() -> Modelo {
    let mut l = Lienzo::new([-2, 0, 0], [2, 6, 2]);
    l.caja(Fijo, r::HIERRO, [-1, 4, 0], [1, 6, 1]);
    l.caja(Fijo, r::HIERRO, [-2, 0, 1], [2, 4, 2]);
    l.borrar([-1, 1, 1], [1, 3, 2]);
    l.armar("llamador")
}

// [epoca3] Detalles de la época 3 (ver `docs/sistemas/arquitectura-epoca3.md`).

/// Toldo de lona a rayas de 3 m sobre la puerta de un almacén o un café: brazos de hierro y la
/// lona inclinada hacia la vereda (1,25 m de vuelo) con su faldón.
fn toldo() -> Modelo {
    let mut l = Lienzo::new([-24, -6, 0], [24, 10, 21]);
    for z in 0..20 {
        let y = 9 - z / 3;
        for x in -24..24 {
            let raya = if (x + 24) / 4 % 2 == 0 { r::CAPA } else { r::CLARO };
            l.punto(Fijo, raya, x, y, z);
        }
    }
    // Faldón con borde ondeado.
    for x in -24..24 {
        let raya = if (x + 24) / 4 % 2 == 0 { r::CAPA } else { r::CLARO };
        let largo = if (x as i32).rem_euclid(4) < 2 { 4 } else { 3 };
        l.caja(Fijo, raya, [x, 3 - largo, 19], [x + 1, 3, 20]);
    }
    for x in [-24, 23] {
        l.caja(Fijo, r::HIERRO, [x, 2, 0], [x + 1, 3, 20]);
    }
    l.armar("toldo")
}

/// Ropa tendida: dos palos, la cuerda y siete prendas colgadas (3 m).
fn ropa_tendida() -> Modelo {
    let mut l = Lienzo::new([-26, 0, -1], [26, 32, 2]);
    for x in [-26, 24] {
        l.caja(Fijo, r::MADERA_OSC, [x, 0, 0], [x + 2, 31, 1]);
    }
    l.caja(Fijo, r::MADERA_OSC, [-24, 29, 0], [24, 30, 1]);
    let prendas: [(i32, i32, i32, u8); 7] = [(-22, 6, 12, r::CLARO), (-15, 5, 8, r::ROPA), (-9, 4, 11, r::CALZON), (-4, 7, 14, r::CLARO), (4, 5, 9, r::ACENTO), (10, 6, 10, r::ROPA), (17, 5, 12, r::CALZON)];
    for (x, ancho, largo, c) in prendas {
        l.caja(Fijo, c, [x, 29 - largo, 0], [x + ancho, 29, 1]);
    }
    l.armar("ropa tendida")
}

/// Palenque: dos postes y el travesaño para atar los caballos.
fn palenque() -> Modelo {
    let mut l = Lienzo::new([-26, 0, -2], [26, 18, 2]);
    for x in [-24, 22] {
        l.caja(Fijo, r::MADERA_OSC, [x, 0, -1], [x + 2, 16, 1]);
    }
    l.caja(Fijo, r::MADERA, [-26, 14, -1], [26, 16, 1]);
    l.armar("palenque")
}

/// Cuánto baja la cabeza cada animal al pastar (rad).
pub fn cabeza_pastando(id: Id) -> f32 {
    match id {
        Id::Caballo | Id::CaballoEnsillado => 1.3,
        Id::Vaca | Id::VacaOvera | Id::Buey => 1.15,
        Id::Oveja => 0.9,
        Id::Perro => 0.6,
        _ => PI * 0.25,
    }
}

// --- [ia] Oficios de la colonia, cosas en la mano y más detalles de las fachadas ---------------
//
// Lo que se lleva en la mano usa las partes de los brazos con los pivotes del hombro de un
// hombre adulto (`ropa::figura`: hombro a 20,5 micro-vóxeles, brazo en x = ±4, mano a 12-14):
// se emite con la misma pose que la persona y sigue al brazo. Medidas [E].

/// Hombros de un hombre adulto (los pivotes de `ropa::figura`).
const HOMBRO_I: [f32; 3] = [-4.0, 20.5, 0.0];
const HOMBRO_D: [f32; 3] = [4.0, 20.5, 0.0];

/// Disco de radio `rad` en el plano xy (centro en x = 0, y = `cy`), de z0 a z1.
fn disco_z(l: &mut Lienzo, ranura: u8, cy: i32, rad: f32, z0: i32, z1: i32) {
    let n = rad.ceil() as i32;
    for dy in -n..n {
        let yc = dy as f32 + 0.5;
        let w = (rad * rad - yc * yc).max(0.0).sqrt().round() as i32;
        if w > 0 {
            l.caja(Fijo, ranura, [-w, cy + dy, z0], [w, cy + dy + 1, z1]);
        }
    }
}

/// Carro del aguador: la pipa acostada a lo largo, sobre dos ruedas, con la canilla atrás y
/// un balde colgado; las varas hasta el caballo [E].
fn carro_aguatero() -> Modelo {
    let mut l = Lienzo::new([-12, 0, -18], [12, 34, 52]);
    let eje = 12;
    l.pivote(RuedaI, [-9.5, eje as f32, 0.0]);
    l.pivote(RuedaD, [9.5, eje as f32, 0.0]);
    l.caja(Fijo, r::MADERA_OSC, [-8, eje - 1, -1], [8, eje + 1, 1]);
    l.centrada(Fijo, r::MADERA, 6, [eje + 1, eje + 2], [-14, 14]);
    for x in [-6, 5] {
        l.caja(Fijo, r::MADERA, [x, eje + 1, 14], [x + 1, eje + 3, 50]);
    }
    // La pipa: duelas en rodajas a lo largo de z, más panzona en el medio, con aros de hierro.
    let cy = eje + 8;
    for (z0, z1, rad) in [(-14, -11, 5.2), (-11, -6, 6.0), (-6, 6, 6.5), (6, 11, 6.0), (11, 14, 5.2)] {
        disco_z(&mut l, r::MADERA, cy, rad, z0, z1);
    }
    for (z, rad) in [(-12, 5.6), (-5, 6.7), (5, 6.7), (12, 5.6)] {
        disco_z(&mut l, r::HIERRO, cy, rad, z, z + 1);
    }
    // Boca arriba, canilla atrás y el balde colgado.
    l.caja(Fijo, r::MADERA_OSC, [-1, cy + 6, -1], [1, cy + 8, 2]);
    l.caja(Fijo, r::HIERRO, [-1, cy - 3, -16], [1, cy - 1, -14]);
    l.cilindro_y(Fijo, r::MADERA_OSC, [5, -16], 2.0, [eje - 4, eje + 1]);
    l.caja(Fijo, r::HIERRO, [5, eje + 1, -16], [6, eje + 3, -15]);
    let mut m = l;
    m.rueda(RuedaI, r::MADERA_OSC, r::MADERA, [-11, -8], 12, 2, 12, 10);
    m.rueda(RuedaD, r::MADERA_OSC, r::MADERA, [8, 11], 12, 2, 12, 10);
    m.armar("carro aguatero")
}

/// Lo del sereno: el farol de sebo colgado de la mano izquierda y el chuzo (la pica) en la
/// derecha.
fn aparejo_sereno() -> Modelo {
    let mut l = Lienzo::new([-8, 0, -4], [8, 44, 6]);
    l.pivote(BrazoI, HOMBRO_I);
    l.pivote(BrazoD, HOMBRO_D);
    // Farol: asa, sombrerete, vidrios encendidos y base.
    l.caja(BrazoI, r::HIERRO, [-5, 10, 0], [-4, 12, 1]);
    l.caja(BrazoI, r::HIERRO, [-7, 9, -2], [-2, 10, 3]);
    l.caja(BrazoI, r::CLARO, [-6, 5, -1], [-3, 9, 2]);
    for (x, z) in [(-7, -2), (-3, -2), (-7, 2), (-3, 2)] {
        l.caja(BrazoI, r::HIERRO, [x, 5, z], [x + 1, 9, z + 1]);
    }
    l.caja(BrazoI, r::HIERRO, [-7, 4, -2], [-2, 5, 3]);
    // Chuzo: el asta por la mano y la moharra de hierro arriba.
    l.caja(BrazoD, r::MADERA_OSC, [4, 1, 1], [5, 38, 2]);
    l.caja(BrazoD, r::HIERRO, [4, 38, 1], [5, 42, 2]);
    l.caja(BrazoD, r::HIERRO, [3, 38, 1], [6, 39, 2]);
    l.armar("aparejo del sereno")
}

/// Picana del boyero: caña de unos 2,5 m con el aguijón de hierro en la punta, tomada de la
/// mano derecha y hacia adelante (el brazo levantado la inclina hacia arriba) [E].
fn picana() -> Modelo {
    let mut l = Lienzo::new([2, 0, -6], [8, 22, 46]);
    l.pivote(BrazoD, HOMBRO_D);
    l.caja(BrazoD, r::MADERA, [4, 12, -4], [5, 13, 40]);
    l.caja(BrazoD, r::MADERA_OSC, [4, 12, 8], [5, 13, 10]);
    l.caja(BrazoD, r::HIERRO, [4, 12, 40], [5, 13, 43]);
    l.armar("picana")
}

/// Balde de madera con aros de hierro, en la mano derecha (la cadena de un incendio).
fn balde_en_mano() -> Modelo {
    let mut l = Lienzo::new([0, 0, -4], [10, 22, 6]);
    l.pivote(BrazoD, HOMBRO_D);
    l.cilindro_y(BrazoD, r::MADERA, [5, 2], 2.6, [6, 11]);
    l.cilindro_y(BrazoD, r::HIERRO, [5, 2], 2.8, [7, 8]);
    l.cilindro_y(BrazoD, r::HIERRO, [5, 2], 2.8, [10, 11]);
    l.caja(BrazoD, r::HIERRO, [4, 11, 1], [5, 13, 3]);
    l.armar("balde en la mano")
}

/// Silla de paja: asiento a medio metro, cuatro patas y el respaldo.
fn silla() -> Modelo {
    let mut l = Lienzo::new([-5, 0, -5], [5, 16, 5]);
    for (x, z) in [(-4, -4), (3, -4), (-4, 3), (3, 3)] {
        l.caja(Fijo, r::MADERA_OSC, [x, 0, z], [x + 1, 7, z + 1]);
    }
    l.caja(Fijo, r::CAPA, [-4, 7, -4], [4, 8, 4]);
    for x in [-4, 3] {
        l.caja(Fijo, r::MADERA_OSC, [x, 8, -4], [x + 1, 16, -3]);
    }
    l.caja(Fijo, r::MADERA_OSC, [-3, 13, -4], [3, 14, -3]);
    l.caja(Fijo, r::MADERA_OSC, [-3, 10, -4], [3, 11, -3]);
    l.armar("silla")
}

/// Postigos de madera de una ventana de 1 m × 2 m: cerrados en el vano (dos hojas con
/// tableros en relieve, un ventanillo y la falleba), o abiertos contra el muro a los dos
/// lados. Origen: el borde de abajo del vano en el plano de la hoja (z = 0, +z a la calle).
fn postigo(abierto: bool) -> Modelo {
    let h = 32;
    let mut l = Lienzo::new([-17, 0, 0], [17, h, 2]);
    let hoja = |l: &mut Lienzo, x0: i32| {
        l.caja(Fijo, r::MADERA_OSC, [x0, 0, 0], [x0 + 8, h, 1]);
        // Tableros en relieve.
        for (y0, y1) in [(2, 13), (15, 30)] {
            l.caja(Fijo, r::MADERA, [x0 + 1, y0, 1], [x0 + 7, y1, 2]);
        }
    };
    if abierto {
        hoja(&mut l, -17);
        hoja(&mut l, 9);
        for x in [-9, 8] {
            for y in [4, 27] {
                l.caja(Fijo, r::HIERRO, [x, y, 0], [x + 1, y + 1, 1]);
            }
        }
    } else {
        hoja(&mut l, -8);
        hoja(&mut l, 0);
        // Ventanillo en la hoja izquierda y la falleba.
        l.caja(Fijo, r::MADERA_OSC, [-6, 18, 1], [-2, 24, 2]);
        l.caja(Fijo, r::HIERRO, [-1, 10, 1], [1, 22, 2]);
    }
    l.armar(if abierto { "postigos abiertos" } else { "postigos" })
}

/// Cancel de hierro y vidrio al fondo del zaguán: marco, montantes, travesaños y el abanico
/// de arriba, con vidrios claros entre los hierros. Origen: el piso en el plano del cancel.
fn cancel() -> Modelo {
    let (w, h) = (12, 40);
    let mut l = Lienzo::new([-w, 0, 0], [w, h, 1]);
    l.caja(Fijo, r::CLARO, [-w + 1, 1, 0], [w - 1, h - 1, 1]);
    l.caja(Fijo, r::HIERRO, [-w, 0, 0], [w, 1, 1]);
    l.caja(Fijo, r::HIERRO, [-w, h - 1, 0], [w, h, 1]);
    for x in [-w, -1, w - 1] {
        l.caja(Fijo, r::HIERRO, [x, 0, 0], [x + 1, h, 1]);
    }
    for x in [-8, -5, 4, 7] {
        l.caja(Fijo, r::HIERRO, [x, 1, 0], [x + 1, 28, 1]);
    }
    for y in [10, 28] {
        l.caja(Fijo, r::HIERRO, [-w, y, 0], [w, y + 1, 1]);
    }
    // Abanico: rayos desde el centro del travesaño.
    for k in 1..6 {
        let a = PI * k as f32 / 6.0;
        for t in 1..11 {
            let x = (a.cos() * t as f32).round() as i32;
            let y = 28 + (a.sin() * t as f32).round() as i32;
            if y < h - 1 && x > -w && x < w - 1 {
                l.punto(Fijo, r::HIERRO, x, y, 0);
            }
        }
    }
    l.armar("cancel")
}

/// Poyo junto a la puerta: banco de piedra (o de ladrillo revocado) de 1,25 m × 0,5 m,
/// pegado a la fachada. Origen: la base, en la cara del muro (z = 0, +z a la calle).
fn banco_piedra() -> Modelo {
    let mut l = Lienzo::new([-10, 0, 0], [10, 8, 7]);
    l.caja(Fijo, r::CAPA, [-10, 0, 0], [10, 7, 6]);
    l.caja(Fijo, r::ACENTO, [-10, 7, 0], [10, 8, 7]);
    l.armar("poyo")
}
