//! Edificios de la época 2, Ciudad Nueva (1830–1880): usina de gas, teatro, usina de agua
//! corriente, boca del colector y las plazas del ensanche. Vóxeles de diseño de 0,5 m; el
//! frente (w = 0) da a la calle. Las fuentes y las marcas [F]/[E] están en
//! `docs/sistemas/arquitectura-epoca2.md`.

use super::*;
use crate::buildings::Edits;
use crate::palette::*;
use crate::terrain::TerrainGen;

/// Anillo hueco (planta circular) de radio `r` y un vóxel de espesor.
#[allow(clippy::too_many_arguments)]
fn anillo(f: &Frame, e: &mut Edits, cu: f32, cw: f32, r: f32, y0: i32, y1: i32, m: Mat) {
    let ri = r.ceil() as i32 + 1;
    for w in (cw as i32 - ri)..=(cw as i32 + ri) {
        for u in (cu as i32 - ri)..=(cu as i32 + ri) {
            let (du, dw) = (u as f32 + 0.5 - cu, w as f32 + 0.5 - cw);
            let d = (du * du + dw * dw).sqrt();
            if d <= r && d > r - 1.2 {
                f.fill(e, u, u + 1, y0, y1, w, w + 1, m);
            }
        }
    }
}

/// Farol de gas dibujado dentro de un marco (plazas, teatro): columna, vidrio y sombrerete.
pub fn farol_en(f: &Frame, e: &mut Edits, u: i32, w: i32) {
    f.raw_fill(e, u, u + 1, 0, 6, w, w + 1, HIERRO_FUNDIDO);
    f.raw(e, u, 6, w, LUZ_GAS);
    f.raw(e, u, 7, w, HIERRO_FUNDIDO);
}

/// Galpón de ladrillo con techo de teja a dos aguas, ventanas altas en arco y su zócalo.
#[allow(clippy::too_many_arguments)]
fn galpon(t: &TerrainGen, f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, h: i32) {
    f.foundation(t, e, u0, u1, w0, w1, STONE);
    ring(f, e, u0, u1, 0, h, w0, w1, BRICK);
    zocalo(f, e, u0, u1, w0, w1, 2, STONE_DARK);
    gable(f, e, u0, u1, w0, w1, h, 2, 1, TILE, BRICK, true);
    let mut u = u0 + 3;
    while u + 2 < u1 - 1 {
        opening(f, e, u, u + 2, 3, h - 3, 1, WINDOW);
        f.fill(e, u, u + 2, h - 3, h - 2, -1 + w0, w0, BRICK_DARK); // arco de ladrillo
        u += 5;
    }
}

/// Usina de gas (1853: la primera, en la costa norte de la bahía [F]): el gasómetro de hierro
/// con su jaula de columnas, la casa de retortas de ladrillo con su chimenea, la carbonera y la
/// oficina con fachada de la época. Marco: 70 × 60 (35 × 30 m).
pub fn usina_gas(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, DIRT_ROAD);
    // Gasómetro: campana de hierro en su pileta, con la jaula de guía.
    let (cu, cw, r) = (20.0, 36.0, 15.0);
    let g = f.sub(0, 0, f.width, f.depth, 3);
    cylinder(&g, e, cu, cw, r + 1.0, 0, 2, BRICK_DARK);
    anillo(&g, e, cu, cw, r, 2, 18, HIERRO_FUNDIDO);
    cylinder(&g, e, cu, cw, r - 0.5, 18, 19, GRIS_PIEDRA_SUCIA);
    cylinder(&g, e, cu, cw, r - 4.0, 19, 20, GRIS_PIEDRA_SUCIA);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let (u, w) = ((cu + (r + 2.0) * a.cos()) as i32, (cw + (r + 2.0) * a.sin()) as i32);
        f.raw_fill(e, u, u + 1, 0, 24, w, w + 1, HIERRO_FUNDIDO);
        f.raw(e, u, 24, w, IRON);
    }
    anillo(&g, e, cu, cw, r + 2.5, 23, 24, HIERRO_FUNDIDO);
    anillo(&g, e, cu, cw, r + 2.5, 12, 13, HIERRO_FUNDIDO);
    // Casa de retortas y chimenea.
    galpon(t, f, e, 42, 68, 24, 52, 14);
    chimney(f, e, 64, 48, 0, 40);
    f.fill(e, 63, 67, 40, 42, 47, 51, BRICK_DARK);
    // Carbonera: montañas de carbón de piedra importado.
    for (u, w, rr) in [(48.0, 57.0, 3.5), (56.0, 57.5, 2.5)] {
        crate::structures::dome(f, e, u, w, rr, 0, STONE_DARK);
    }
    // Oficina con fachada de la época, sobre la calle.
    let o = f.sub(44, 0, 22, 14, 9);
    o.foundation(t, e, 0, 22, 0, 14, STONE);
    o.fill(e, 0, 22, 0, 10, 0, 14, OCRE_CLARO);
    zocalo(&o, e, 0, 22, 0, 14, 2, MARMOL_GRIS);
    o.fill(e, 0, 22, 9, 10, -1, 0, BLANCO_PIEDRA);
    o.fill(e, 0, 22, 10, 12, 0, 1, OCRE_CLARO);
    o.fill(e, 0, 22, 12, 13, 0, 1, BLANCO_PIEDRA);
    o.fill(e, 1, 21, 10, 11, 1, 13, AZOTEA);
    opening(&o, e, 9, 13, 0, 7, 1, SHUTTER);
    for u in [3, 16] {
        opening(&o, e, u, u + 2, 2, 7, 2, WINDOW);
        o.raw_fill(e, u, u + 2, 2, 7, 1, 2, HIERRO_FUNDIDO);
    }
    for u in [0, 21] {
        o.fill(e, u, u + 1, 2, 9, -1, 0, BLANCO_PIEDRA);
    }
    // Cerco de hierro sobre la calle.
    for u in 0..42 {
        f.raw(e, u, 0, 0, if u % 6 == 0 { BRICK } else { HIERRO_FUNDIDO });
        if u % 6 == 0 {
            f.raw(e, u, 1, 0, BRICK);
        }
    }
}

/// Teatro (el Solís: proyecto de Carlo Zucchi de 1840, adaptado por Francisco Javier de
/// Garmendia, inaugurado el 25 de agosto de 1856 [F]; las alas laterales, de 1874 [F]): un
/// pórtico de columnas con entablamento y balaustrada sobre una escalinata de mármol, el
/// volumen de la sala con su azotea, la caja escénica más alta al fondo y las alas laterales.
/// Las proporciones son [E]. Marco: 80 × 96 (40 × 48 m).
pub fn teatro(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (w0, w1) = (10, 88);
    let (u0, u1) = (6, 74);
    f.foundation(t, e, u0 - 4, u1 + 4, 0, w1, STONE);
    f.surface(e, 0, f.width, 0, f.depth, PLAZA);
    // Escalinata.
    for k in 0..3 {
        f.fill(e, 16 - k, 64 + k, 0, 3 - k, 4 - 2 * k, w0, MARMOL);
    }
    // Volumen de la sala.
    let h = 28;
    caja_neo(f, e, 16, 64, w0, 60, h, OCRE_CLARO);
    // Alas laterales, más bajas.
    caja_neo(f, e, u0, 16, w0 + 6, w1 - 6, 18, OCRE_CLARO);
    caja_neo(f, e, 64, u1, w0 + 6, w1 - 6, 18, OCRE_CLARO);
    // Caja escénica.
    caja_neo(f, e, 18, 62, 60, w1, 38, OCRE_CLARO);
    gable(f, e, 18, 62, 60, w1, 38, 3, 1, TILE_DARK, OCRE_CLARO, false);
    // Pórtico: 8 columnas con basa y capitel.
    let base = 3;
    for k in 0..8 {
        let u = 18 + k * 6;
        f.fill(e, u, u + 2, base, base + 18, 5, 7, BLANCO_PIEDRA);
        f.fill(e, u - 1, u + 3, base, base + 1, 4, 8, BLANCO_PIEDRA);
        f.fill(e, u - 1, u + 3, base + 17, base + 18, 4, 8, BLANCO_PIEDRA);
    }
    // Entablamento, friso y balaustrada del pórtico.
    f.fill(e, 16, 64, base + 18, base + 21, 3, w0, BLANCO_PIEDRA);
    f.fill(e, 16, 64, base + 19, base + 20, 2, 3, GRIS_PIEDRA);
    for u in 16..64 {
        let pilar = (u - 16) % 6 == 0 || u == 63;
        if pilar {
            f.fill(e, u, u + 1, base + 21, base + 24, 3, 4, BLANCO_PIEDRA);
        } else if u % 2 == 1 {
            f.raw(e, u, base + 22, 3, BLANCO_PIEDRA);
        }
    }
    f.fill(e, 16, 64, base + 24, base + 25, 3, 4, BLANCO_PIEDRA);
    f.fill(e, 16, 64, base + 21, base + 22, 4, w0, AZOTEA);
    // Puertas y ventanas de la fachada (detrás de las columnas).
    let fs = f.sub(0, w0, f.width, f.depth - w0, 0);
    for k in 0..7 {
        let u = 21 + k * 6;
        opening(&fs, e, u, u + 2, base, base + 9, 1, WOOD_DARK);
        fs.raw_fill(e, u, u + 2, base + 7, base + 9, 1, 2, HIERRO_FUNDIDO);
        opening(&fs, e, u, u + 2, base + 11, base + 17, 2, WINDOW);
    }
    // Faroles a los lados de la escalinata.
    for u in [12, 67] {
        farol_en(f, e, u, 2);
    }
}

/// Caja con fachada de la época: muro de color, zócalo, cornisa, pretil con balaustres y
/// azotea; ventanas altas a los lados.
#[allow(clippy::too_many_arguments)]
fn caja_neo(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, h: i32, m: Mat) {
    ring(f, e, u0, u1, 0, h, w0, w1, m);
    f.fill(e, u0 + 1, u1 - 1, h - 1, h, w0 + 1, w1 - 1, AZOTEA);
    zocalo(f, e, u0, u1, w0, w1, 3, GRIS_PIEDRA);
    // Cornisa en las cuatro caras.
    f.fill(e, u0 - 1, u1 + 1, h - 1, h, w0 - 1, w0, BLANCO_PIEDRA);
    f.fill(e, u0 - 1, u1 + 1, h - 1, h, w1, w1 + 1, BLANCO_PIEDRA);
    f.fill(e, u0 - 1, u0, h - 1, h, w0, w1, BLANCO_PIEDRA);
    f.fill(e, u1, u1 + 1, h - 1, h, w0, w1, BLANCO_PIEDRA);
    ring(f, e, u0, u1, h, h + 2, w0, w1, m);
    ring(f, e, u0, u1, h + 2, h + 3, w0, w1, BLANCO_PIEDRA);
    // Ventanas altas en los costados.
    let mut w = w0 + 4;
    while w + 3 < w1 - 2 {
        for (u, d) in [(u0, 1), (u1 - 1, -1)] {
            let _ = d;
            f.clear(e, u, u + 1, 4, h - 5, w, w + 2);
            f.raw_fill(e, u, u + 1, 4, h - 5, w, w + 2, WINDOW);
        }
        w += 7;
    }
}

/// Usina de agua corriente (la toma del Santa Lucía y las bombas a vapor, 1871 [F]; en el
/// mapa de la ciudad, el depósito con la casa de bombas): casa de máquinas de ladrillo con
/// ventanas en arco y chimenea, y el tanque elevado de hierro sobre pilares. Marco: 48 × 44.
pub fn agua_corriente(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, DIRT_ROAD);
    galpon(t, f, e, 2, 28, 4, 24, 12);
    chimney(f, e, 24, 20, 0, 30);
    // Tanque elevado: cuatro pilares de ladrillo y el depósito de hierro.
    let (u0, w0) = (31, 18);
    for (du, dw) in [(0, 0), (12, 0), (0, 12), (12, 12)] {
        f.foundation(t, e, u0 + du, u0 + du + 2, w0 + dw, w0 + dw + 2, STONE);
        f.fill(e, u0 + du, u0 + du + 2, 0, 18, w0 + dw, w0 + dw + 2, BRICK);
    }
    f.fill(e, u0 - 1, u0 + 15, 18, 19, w0 - 1, w0 + 15, STONE);
    ring(f, e, u0 - 1, u0 + 15, 19, 26, w0 - 1, w0 + 15, HIERRO_FUNDIDO);
    f.fill(e, u0 - 1, u0 + 15, 26, 27, w0 - 1, w0 + 15, GRIS_PIEDRA_SUCIA);
    // Fuente pública al frente (como la de la Plaza Constitución, 1871 [F]).
    let (fu, fw) = (38.0, 6.0);
    cylinder(f, e, fu, fw, 4.0, 0, 1, MARMOL_GRIS);
    cylinder(f, e, fu, fw, 3.0, 1, 2, MARMOL);
    f.raw_fill(e, fu as i32, fu as i32 + 1, 2, 5, fw as i32, fw as i32 + 1, MARMOL);
    f.raw(e, fu as i32, 1, fw as i32 + 2, STREAM);
}

/// Boca del colector (red de Arteaga, 1856 [F]): casilla de inspección de ladrillo con su
/// bóveda y la boca de desagüe hacia el río. Marco: 24 × 24.
pub fn colector(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, COBBLE);
    let c = f.sub(4, 2, 16, 12, 5);
    c.foundation(t, e, 0, 16, 0, 12, STONE);
    ring(&c, e, 0, 16, 0, 8, 0, 12, BRICK);
    gable(&c, e, 0, 16, 0, 12, 8, 2, 1, TILE, BRICK, true);
    arch(&c, e, 6, 10, 0, 6, 0, 1);
    c.raw_fill(e, 6, 10, 0, 5, 1, 2, HIERRO_FUNDIDO);
    // Bóveda del caño maestro que sale por detrás.
    for w in 14..f.depth {
        f.fill(e, 9, 15, 0, 1, w, w + 1, BRICK_DARK);
        f.fill(e, 9, 10, 1, 3, w, w + 1, BRICK_DARK);
        f.fill(e, 14, 15, 1, 3, w, w + 1, BRICK_DARK);
        f.fill(e, 10, 14, 3, 4, w, w + 1, BRICK);
    }
    // Registros de hierro en la vereda.
    for u in [3, 20] {
        f.raw(e, u, -1, 1, HIERRO_FUNDIDO);
    }
}

/// Plaza del ensanche (encima del suelo nivelado de la plaza): canteros con césped, cuatro
/// palmeras, bancos, faroles de gas alrededor del centro y la columna del medio (la de la
/// plaza nivelada; en Montevideo, la Columna de la Paz de la plaza Cagancha es de 1867 [F]).
/// Marco: el lado de la plaza.
pub fn plaza_ensanche(_t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let n = f.width;
    let c = n / 2;
    // Canteros en las cuatro esquinas (caminos en cruz y en diagonal quedan de piedra).
    for w in 4..n - 4 {
        for u in 4..n - 4 {
            let (du, dw) = ((u - c).abs(), (w - c).abs());
            let camino = du < 4 || dw < 4 || (du - dw).abs() < 3 || du.max(dw) < 14;
            if !camino {
                f.raw(e, u, 0, w, if (u + w) % 7 == 0 { GRASS_LUSH } else { GARDEN });
            }
        }
    }
    let q = n / 4;
    for (u, w) in [(q, q), (n - q - 2, q), (q, n - q - 2), (n - q - 2, n - q - 2)] {
        casas::palmera(f, e, u, w, 18, f.seed ^ (u * 31 + w) as u64);
    }
    // Faroles de gas alrededor de la columna y bancos de madera en los caminos.
    for (du, dw) in [(-9, -9), (9, -9), (-9, 9), (9, 9)] {
        farol_en(f, e, c + du, c + dw);
    }
    for (u, w, a) in [(c - 22, c - 6, true), (c + 20, c + 5, true), (c - 6, c - 22, false), (c + 5, c + 20, false)] {
        let (lu, lw) = if a { (1, 3) } else { (3, 1) };
        f.raw_fill(e, u, u + lu, 0, 1, w, w + lw, WOOD_DARK);
    }
}
