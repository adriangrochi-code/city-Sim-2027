//! Casas coloniales con variedad: cada solar se dibuja con su [`Estilo`] (tipología, fachada,
//! planta, color y desgaste; ver `estilo.rs` y `docs/sistemas/arquitectura-colonial.md`).
//!
//! Los constructores dibujan directamente con los materiales reales de la obra (el estilo ya
//! los conoce), así que no hace falta remapear después. Todo queda dentro del solar
//! (24 × 76 vóxeles de diseño; la fachada puede volar hasta 1 m sobre la vereda).
//!
//! Para ahorrar vóxeles, los cuerpos son cajas huecas (muros de 0,5 m, frente de 1 m y losa o
//! techo): por dentro no se ve nada y la cuenta de vóxeles baja a menos de la mitad que con
//! cuerpos macizos.

use super::estilo::*;
use super::*;
use super::colonial::letrina;
use crate::buildings::Rng;

/// Frente principal de una casa (para las manchas de las gárgolas): marco, tramo y altura.
pub(super) struct Frente {
    pub(super) g: Frame,
    pub(super) u0: i32,
    pub(super) u1: i32,
    /// Altura de la losa de la azotea (donde desaguan las gárgolas); 0 = sin gárgolas.
    pub(super) h: i32,
}

/// Dibuja la casa de un solar con su estilo. Llamar con el marco del solar (24 × 76).
pub fn construir(t: &TerrainGen, e: &mut Edits, f: &Frame, es: &Estilo) {
    let frente = match es.tipo {
        // [epoca3] Los tipos propios de la época 3 (casa standard, edificio de renta).
        _ if super::epoca3::propio(es) => super::epoca3::construir3(t, e, f, es),
        Tipo::RanchoCuero => rancho_cuero(t, e, f, es),
        Tipo::Rancho => rancho(t, e, f, es),
        Tipo::CasaTeja | Tipo::CasaAzotea | Tipo::Cuartos => casa_un_piso(t, e, f, es),
        Tipo::CasaAltos => casa_altos(t, e, f, es),
        Tipo::Quinta => quinta(t, e, f, es),
    };
    super::epoca3::detalles3(e, f, es); // [epoca3] toldos, letreros y ropa tendida
    envejecer(e, f, es);
    if let Some(fr) = frente {
        manchas_gargolas(e, es, &fr);
    }
    recortar(e, f);
}

pub(super) fn rng(es: &Estilo, salt: u64) -> Rng {
    Rng::new(es.seed ^ salt.wrapping_mul(0x2545_F491_4F6C_DD1D))
}

// --- Piezas -------------------------------------------------------------------------------

/// Caja hueca: muros de un vóxel y el frente (w0) de dos, de y0 a y1.
#[allow(clippy::too_many_arguments)]
pub(super) fn caja(f: &Frame, e: &mut Edits, u0: i32, u1: i32, y0: i32, y1: i32, w0: i32, w1: i32, m: Mat) {
    ring(f, e, u0, u1, y0, y1, w0, w1, m);
    f.fill(e, u0, u1, y0, y1, w0 + 1, w0 + 2, m);
}

/// Azotea: losa de ladrillo bajo el borde del muro (en `y`).
pub(super) fn losa(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, y: i32) {
    f.fill(e, u0 + 1, u1 - 1, y, y + 1, w0 + 1, w1 - 1, AZOTEA);
}

/// Media agua: techo de una sola pendiente (un vóxel cada dos), alto del lado `alto_u0`.
#[allow(clippy::too_many_arguments)]
fn media_agua(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, y: i32, alto_u0: bool, roof: Mat, wall: Mat) {
    let n = u1 - u0;
    for k in 0..n {
        let u = if alto_u0 { u0 + k } else { u1 - 1 - k };
        let dy = (n - 1 - k) / 2;
        for w in (w0 - 1)..=w1 {
            f.set(e, u, y + dy, w, roof);
        }
        // Hastiales de los extremos.
        for yy in y..y + dy {
            f.set(e, u, yy, w0, wall);
            f.set(e, u, yy, w1 - 1, wall);
        }
    }
}

/// Material de molduras, alféizares y losas de balcón: sillería si hay piedra, si no la cal
/// de las molduras (o ladrillo, en una casa sin revoque).
pub(super) fn piedra_o_moldura(es: &Estilo) -> Mat {
    if es.piedra {
        STONE
    } else if es.muro == BRICK {
        BRICK_DARK
    } else {
        es.moldura
    }
}

/// Un marco en contraste con el muro (para la portada más sencilla).
fn marco(es: &Estilo) -> Mat {
    if es.piedra {
        STONE
    } else if es.moldura != es.muro {
        es.moldura
    } else if es.muro == CAL {
        CAL_HUESO
    } else if es_cal(es.muro) {
        CAL
    } else {
        sucio(es.muro)
    }
}

/// Ventana de la planta baja según el tipo de reja (guía §3).
#[allow(clippy::too_many_arguments)]
fn ventana(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, y0: i32, y1: i32, tipo: Reja) {
    let ms = piedra_o_moldura(es);
    match tipo {
        Reja::Voladiza => {
            // Jaula que vuela 0,5 m, con base y remate.
            opening(f, e, u0, u1, y0, y1, 1, WINDOW);
            // [ciudad] En la ciudad, la reja es un modelo de detalle (barrotes de verdad);
            // si no, el vóxel de reja de siempre.
            if !reja_detalle(f, e, es, u0, u1, y0, y1, 0.0, true) {
                f.raw_fill(e, u0, u1, y0, y1, -1, 0, REJA);
            }
            f.fill(e, u0 - 1, u1 + 1, y0 - 1, y0, -1, 0, ms);
            f.fill(e, u0 - 1, u1 + 1, y1, y1 + 1, -1, 0, ms);
        }
        Reja::Plana => {
            // Barrotes embutidos en el vano, con el postigo atrás.
            opening(f, e, u0, u1, y0, y1, 2, WINDOW);
            if !reja_detalle(f, e, es, u0, u1, y0, y1, 1.5, false) {
                f.raw_fill(e, u0, u1, y0, y1, 1, 2, REJA);
            }
            if es.ornamento >= 1 {
                f.fill(e, u0, u1, y0 - 1, y0, -1, 0, ms); // alféizar o poyo
            }
        }
        Reja::Postigo => {
            // Postigos de madera: cerrados, o con una hoja abierta. [ia] En la ciudad, como
            // modelos de detalle (tableros, ventanillo, bisagras): cerrados en el vano o
            // abiertos contra el muro, con el vidrio atrás.
            let abierto = hash(f.at(u0, y0, 0), es.seed ^ 0x9051) < 0.4;
            if u1 - u0 == 2 && y1 - y0 == 4 && Frame::con_detalles(e) {
                opening(f, e, u0, u1, y0, y1, 1, WINDOW);
                let (id, w) = if abierto { (crate::detalle::Id::PostigoAbierto, 0.0) } else { (crate::detalle::Id::Postigo, 1.0) };
                f.detalle(e, id, (u0 + u1) as f32 * 0.5, y0 as f32, w, 0.0, es.seed as u32 ^ 0x9052);
            } else {
                opening(f, e, u0, u1, y0, y1, 1, es.carpinteria);
                if abierto {
                    f.raw_fill(e, u0, u0 + 1, y0, y1, 1, 2, WINDOW);
                }
            }
            if es.ornamento >= 1 {
                f.fill(e, u0 - 1, u1 + 1, y1, y1 + 1, 0, 1, marco(es));
            }
        }
    }
}

/// [ciudad] Reja de ventana como modelo de detalle (1 m de ancho, 2 o 2,5 m de alto), en el
/// plano `w`. Devuelve `false` si no corresponde (otro tamaño, o quien construye no dibuja
/// detalles): entonces va la reja de vóxeles.
#[allow(clippy::too_many_arguments)]
fn reja_detalle(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, y0: i32, y1: i32, w: f32, volada: bool) -> bool {
    use crate::detalle::Id;
    if u1 - u0 != 2 || !Frame::con_detalles(e) {
        return false;
    }
    let id = match (y1 - y0, volada) {
        (4, false) => Id::RejaPlana,
        (5, false) => Id::RejaPlanaAlta,
        (4, true) => Id::RejaVolada,
        (5, true) => Id::RejaVoladaAlta,
        _ => return false,
    };
    // Todas las rejas de una casa son del mismo herrero (la semilla de la casa).
    f.detalle(e, id, (u0 + u1) as f32 * 0.5, y0 as f32, w, 0.0, es.seed as u32)
}

/// [ciudad] Riqueza de la casa para los detalles de la fachada: 0 pobre … 3 rica.
pub(super) fn riqueza(es: &Estilo) -> u8 {
    es.clase.min(3).max(u8::from(es.ornamento >= 2))
}

/// [ciudad] Aldaba o llamador en la puerta principal (casas con hierro de clase media para
/// arriba) [E]: en el plano de la hoja (`w`), a 1,5 m del suelo.
pub(super) fn llamador(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, w: i32) {
    if es.hierro && riqueza(es) >= 1 {
        f.detalle(e, crate::detalle::Id::Llamador, (u0 + u1) as f32 * 0.5, 2.6, w as f32, 0.0, es.seed as u32 ^ 0x11A);
    }
}

/// [ia] Poyo (banco de piedra o de material) junto a la puerta principal de las casas
/// modestas: un poco más de un tercio [E], donde haya 1,5 m de muro libre al lado de la
/// puerta (a la derecha, o si no a la izquierda). Ahí se sientan los vecinos a la tardecita.
#[allow(clippy::too_many_arguments)]
fn poyo(t: &TerrainGen, f: &Frame, e: &mut Edits, es: &Estilo, a: i32, b: i32, vanos: &[(i32, i32, Hueco)], u1: i32) {
    if !Frame::con_detalles(e) || riqueza(es) > 1 || es.neo || hash(f.at(a, 0, 0), es.seed ^ 0x9070) >= 0.38 {
        return;
    }
    let libre_der = vanos.iter().filter(|v| v.0 >= b).map(|v| v.0).min().unwrap_or(u1) - b;
    let libre_izq = a - vanos.iter().filter(|v| v.1 <= a).map(|v| v.1).max().unwrap_or(0);
    let u = if libre_der >= 4 {
        b as f32 + 1.75
    } else if libre_izq >= 4 {
        a as f32 - 1.75
    } else {
        return;
    };
    // Solo si el piso de la casa está a nivel de la vereda (en una cuesta, la casa queda
    // levantada sobre su cimiento y el poyo quedaría colgado).
    let p = f.at(u as i32, 0, -1);
    if crate::buildings::design_top_at(t, p.x, p.z) + 1 < f.base {
        return;
    }
    f.detalle(e, crate::detalle::Id::BancoPiedra, u, 0.0, 0.0, 0.0, es.seed as u32 ^ 0x9071);
}

/// Puerta (de una o dos hojas) rehundida, con montante de hierro en las casas buenas.
#[allow(clippy::too_many_arguments)]
fn puerta(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, hp: i32, recess: i32) {
    opening(f, e, u0, u1, 0, hp, recess, es.carpinteria);
    if es.ornamento >= 2 && hp >= 6 && es.hierro {
        f.raw_fill(e, u0, u1, hp - 1, hp, recess, recess + 1, REJA);
    }
    if es.piedra && es.ornamento >= 1 {
        f.fill(e, u0, u1, hp, hp + 1, 0, 1, STONE); // dintel
    }
}

/// Portada de la puerta principal (guía §3): 1 marco, 2 pilastras y cornisa, 3 con frontón.
#[allow(clippy::too_many_arguments)]
pub(super) fn portada(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, hp: i32, techo: i32) {
    let m = if es.portada >= 2 { piedra_o_moldura(es) } else { marco(es) };
    match es.portada {
        0 => {}
        1 => {
            f.fill(e, u0 - 1, u0, 0, hp + 1, 0, 1, m);
            f.fill(e, u1, u1 + 1, 0, hp + 1, 0, 1, m);
            f.fill(e, u0 - 1, u1 + 1, hp, hp + 1, 0, 1, m);
        }
        _ => {
            f.fill(e, u0 - 1, u0, 0, hp + 1, -1, 0, m);
            f.fill(e, u1, u1 + 1, 0, hp + 1, -1, 0, m);
            f.fill(e, u0 - 2, u1 + 2, hp + 1, hp + 2, -1, 0, m);
            if es.portada >= 3 {
                let mut k = 0;
                while u0 - 1 + k < u1 + 1 - k && hp + 2 + k < techo - 1 {
                    f.fill(e, u0 - 1 + k, u1 + 1 - k, hp + 2 + k, hp + 3 + k, -1, 0, m);
                    k += 1;
                }
            }
        }
    }
}

/// Cornisa bajo el pretil o el alero: 1 faja, 2 dos molduras, 3 con dentículos.
pub(super) fn cornisa_estilo(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, y: i32) {
    let m = piedra_o_moldura(es);
    match es.cornisa {
        0 => {}
        1 => f.fill(e, u0, u1, y, y + 1, -1, 0, m),
        2 => {
            f.fill(e, u0, u1, y, y + 1, -1, 0, m);
            f.fill(e, u0, u1, y - 1, y, -1, 0, m);
        }
        _ => {
            f.fill(e, u0, u1, y, y + 1, -1, 0, m);
            for u in (u0..u1).step_by(2) {
                f.fill(e, u, u + 1, y - 1, y, -1, 0, m);
            }
        }
    }
}

/// Pretil sobre la azotea: liso, con pilarcitos (y macetones en las casas más ricas) o
/// balaustrado.
#[allow(clippy::too_many_arguments)]
pub(super) fn pretil(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, w0: i32, w1: i32, h: i32) {
    let p = es.pretil.max(1);
    ring(f, e, u0, u1, h, h + p, w0, w1, es.muro);
    let m = piedra_o_moldura(es);
    if es.neo && es.pretil_tipo == 2 && p >= 2 {
        // Balaustrada italianizante: pilares cada 4 vóxeles (con macetones o pináculos en las
        // casas más ornamentadas), balaustres claros entre ellos y la albardilla.
        f.fill(e, u0, u1, h, h + 1, w0, w0 + 1, es.muro);
        for u in u0..u1 {
            let pilar = (u - u0) % 4 == 0 || u == u1 - 1;
            if pilar {
                f.fill(e, u, u + 1, h + 1, h + p, w0, w0 + 1, es.muro);
            } else if (u - u0) % 2 == 1 {
                f.clear(e, u, u + 1, h + 1, h + p, w0, w0 + 1);
                f.raw(e, u, h + 1, w0, m);
            } else {
                f.raw_fill(e, u, u + 1, h + 1, h + p, w0, w0 + 1, m);
            }
        }
        f.fill(e, u0, u1, h + p, h + p + 1, w0, w0 + 1, m);
        if es.ornamento >= 2 {
            let mut u = u0;
            while u < u1 {
                f.raw(e, u, h + p + 1, w0, m);
                u += 8;
            }
            f.raw(e, u1 - 1, h + p + 1, w0, m);
        }
        return;
    }
    match es.pretil_tipo {
        1 => {
            let mut u = u0;
            while u < u1 {
                f.fill(e, u, u + 1, h + p, h + p + 1, w0, w0 + 1, m);
                if es.ornamento >= 3 {
                    f.raw(e, u, h + p + 1, w0, sucio(m)); // macetón o pináculo
                }
                u += if u + 4 >= u1 - 1 && u != u1 - 1 { (u1 - 1 - u).max(1) } else { 4 };
            }
        }
        2 if p >= 2 => {
            // Balaustres entre pilares (los huecos, rehundidos y en sombra), con la albardilla.
            for u in (u0 + 1)..(u1 - 1) {
                if (u - u0) % 4 != 0 && (u - u0) % 2 == 1 {
                    f.clear(e, u, u + 1, h + p - 1, h + p, w0, w0 + 1);
                    f.set(e, u, h + p - 1, w0 + 1, sucio(es.muro));
                }
            }
            f.fill(e, u0, u1, h + p, h + p + 1, w0, w0 + 1, m);
        }
        _ => {}
    }
}

/// Gárgolas: caños de teja o de hojalata que vuelan 0,5 m sobre la vereda.
pub(super) fn gargolas(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, h: i32) {
    let n = es.gargolas as i32;
    for k in 0..n {
        let u = u0 + (k + 1) * (u1 - u0) / (n + 1);
        let m = if es.hierro && hash(f.at(u, h, 0), es.seed) < 0.3 { GRIS_SUCIA } else { TILE_LIGHT }; // hojalata o teja
        f.raw(e, u, h, -1, m);
    }
}

/// Lamidos de humedad bajo cada gárgola (después del desgaste, para que se vean).
fn manchas_gargolas(e: &mut Edits, es: &Estilo, fr: &Frente) {
    if fr.h == 0 || !es_cal(es.muro) {
        return;
    }
    let n = es.gargolas as i32;
    for k in 0..n {
        let u = fr.u0 + (k + 1) * (fr.u1 - fr.u0) / (n + 1);
        if hash(fr.g.at(u, 0, 0), es.seed ^ 0x6A6) >= 0.7 {
            continue;
        }
        let largo = 2 + (hash(fr.g.at(u, 1, 0), es.seed) * 4.0) as i32;
        for y in (fr.h - largo).max(1)..fr.h {
            let p = fr.g.at(u, y, 0);
            if e.design.get(&p).is_some_and(|&m| es_cal(m)) {
                e.design.insert(p, sucio(es.muro));
            }
        }
    }
}

/// Aljibe o pozo con brocal en un patio.
pub(super) fn agua_patio(f: &Frame, e: &mut Edits, es: &Estilo, u: i32, w: i32) {
    if es.aljibe {
        aljibe(f, e, u, w);
    } else if es.pozo {
        ring(f, e, u, u + 3, 0, 2, w, w + 3, if es.piedra { STONE } else if es_cal(es.muro) { CAL } else { MAMPOSTERIA });
        f.raw(e, u + 1, 0, w + 1, MUD);
        f.raw_fill(e, u, u + 1, 2, 5, w + 1, w + 2, WOOD_DARK);
        f.raw_fill(e, u + 2, u + 3, 2, 5, w + 1, w + 2, WOOD_DARK);
        f.raw_fill(e, u, u + 3, 5, 6, w + 1, w + 2, WOOD);
    }
}

/// Parral: cuatro horcones y la parra encima.
fn parral(f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32) {
    for (u, w) in [(u0, w0), (u1 - 1, w0), (u0, w1 - 1), (u1 - 1, w1 - 1)] {
        f.raw_fill(e, u, u + 1, 0, 5, w, w + 1, WOOD_DARK);
    }
    for w in w0..w1 {
        for u in u0..u1 {
            if hash(f.at(u, 5, w), 0xBA77) < 0.8 {
                f.raw(e, u, 5, w, if (u + w) % 3 == 0 { LEAF_DARK } else { LEAF });
            }
        }
    }
}

/// Frutal (durazno, higuera): tronco y copa chica.
fn frutal(f: &Frame, e: &mut Edits, u: i32, w: i32, seed: u64) {
    f.raw_fill(e, u, u + 1, 0, 3, w, w + 1, TRUNK);
    for y in 3..6 {
        let r = if y == 5 { 1 } else { 2 };
        for dw in -r..=r {
            for du in -r..=r {
                if du * du + dw * dw <= r * r + 1 && hash(f.at(u + du, y, w + dw), seed) < 0.85 {
                    f.raw(e, u + du, y, w + dw, if (du + dw + y) % 4 == 0 { LEAF_DARK } else { LEAF });
                }
            }
        }
    }
}

// --- Piezas de la fachada neoclásica (época 2) ----------------------------------------------

/// Ventana alta de la Ciudad Nueva: persianas (algunas abiertas) detrás de una reja de hierro
/// fundido, plana o volada sobre una losita de mármol, con el guardapolvo de moldura arriba y
/// un frontoncito en las casas más ornamentadas.
#[allow(clippy::too_many_arguments)]
pub(super) fn ventana_neo(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, y0: i32, y1: i32) {
    let abierta = hash(f.at(u0, y0, 0), es.seed ^ 0x9E0) < 0.45;
    opening(f, e, u0, u1, y0, y1, 2, if abierta { WINDOW } else { es.carpinteria });
    if abierta {
        // Las hojas de la persiana abiertas contra el derrame.
        f.raw_fill(e, u0, u0 + 1, y0, y1, 1, 2, es.carpinteria);
    }
    match es.reja {
        Reja::Voladiza => {
            f.raw_fill(e, u0, u1, y0, y1, -1, 0, HIERRO_FUNDIDO);
            f.fill(e, u0 - 1, u1 + 1, y0 - 1, y0, -1, 0, if es.marmol { MARMOL } else { es.moldura });
            f.raw_fill(e, u0, u1, y1, y1 + 1, -1, 0, HIERRO_FUNDIDO);
        }
        Reja::Plana => f.raw_fill(e, u0, u1, y0, y1, 1, 2, HIERRO_FUNDIDO),
        Reja::Postigo => {}
    }
    // Guardapolvo (y frontón en las de más ornamento).
    f.fill(e, u0 - 1, u1 + 1, y1, y1 + 1, -1, 0, es.moldura);
    if es.ornamento >= 3 {
        f.fill(e, u0, u1, y1 + 1, y1 + 2, -1, 0, es.moldura);
    }
    if es.reja != Reja::Voladiza {
        f.fill(e, u0 - 1, u1 + 1, y0 - 1, y0, -1, 0, es.moldura); // alféizar
    }
}

/// Puerta alta de la Ciudad Nueva: dos hojas con montante de hierro fundido; con zaguán, la
/// puerta de calle abierta contra las jambas, el zaguán de mármol y el cancel de hierro y
/// vidrio al fondo. Escalón de mármol sobre la vereda.
#[allow(clippy::too_many_arguments)]
pub(super) fn puerta_neo(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, hp: i32, zaguan: bool) {
    if zaguan {
        // [ia] En la ciudad, el cancel es un modelo de detalle (hierros, vidrios y abanico) con
        // el patio claro atrás; si no, el vóxel de hierro fundido de siempre.
        let cancel_detalle = es.cancel && u1 - u0 == 3 && Frame::con_detalles(e);
        opening(f, e, u0, u1, 0, hp, 5, if cancel_detalle { WINDOW } else if es.cancel { HIERRO_FUNDIDO } else { es.carpinteria });
        if cancel_detalle {
            f.detalle(e, crate::detalle::Id::Cancel, (u0 + u1) as f32 * 0.5, 0.0, 4.95, 0.0, es.seed as u32 ^ 0xCA4C);
        }
        f.raw_fill(e, u0, u0 + 1, 0, hp - 1, 1, 3, es.carpinteria);
        f.raw_fill(e, u1 - 1, u1, 0, hp - 1, 1, 3, es.carpinteria);
        if es.marmol {
            for w in 0..5 {
                for u in u0..u1 {
                    let m = if (u + w) % 2 == 0 { MARMOL } else { MARMOL_GRIS };
                    f.raw(e, u, -1, w, m);
                }
            }
        }
    } else {
        opening(f, e, u0, u1, 0, hp, 1, es.carpinteria);
    }
    if es.hierro {
        f.raw_fill(e, u0, u1, hp - 1, hp, 1, 2, HIERRO_FUNDIDO); // montante
    }
    if es.marmol {
        f.raw_fill(e, u0 - 1, u1 + 1, 0, 1, -1, 0, MARMOL);
    }
}

/// Pilastras de la fachada: en los extremos y, con `pilastras == 2`, entre los vanos.
pub(super) fn pilastras(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, vanos: &[(i32, i32, Hueco)], y0: i32, y1: i32) {
    if es.pilastras == 0 {
        return;
    }
    let mut us = vec![u0, u1 - 1];
    if es.pilastras >= 2 {
        for w in vanos.windows(2) {
            let (b, a) = (w[0].1, w[1].0);
            if a - b >= 3 {
                us.push((a + b) / 2);
            }
        }
    }
    for u in us {
        f.fill(e, u, u + 1, y0, y1, -1, 0, es.moldura);
        // Basa y capitel, un vóxel más anchos.
        f.fill(e, u - 1, u + 2, y1 - 1, y1, -1, 0, es.moldura);
    }
}

/// Palmera canaria (*Phoenix canariensis*), la de los jardines del XIX: tronco grueso de
/// escamas y una corona de hojas que se arquean.
pub fn palmera(f: &Frame, e: &mut Edits, u: i32, w: i32, alto: i32, seed: u64) {
    f.raw_fill(e, u, u + 2, 0, alto, w, w + 2, TRUNK_GRAY);
    f.raw_fill(e, u, u + 2, alto, alto + 1, w, w + 2, LEAF_DARK);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU + hash(f.at(u, alto, w), seed) * 0.6;
        let (du, dw) = (a.cos(), a.sin());
        for s in 1..=6 {
            let fu = u as f32 + 0.5 + du * s as f32 * 0.9;
            let fw = w as f32 + 0.5 + dw * s as f32 * 0.9;
            let y = alto + 1 + if s <= 3 { 1 } else { 2 - (s - 3) };
            f.raw(e, fu.round() as i32, y, fw.round() as i32, if (s + k) % 3 == 0 { LEAF_DARK } else { LEAF });
        }
    }
}

/// Farol de hierro con brazo, colgado de la fachada. [ciudad] En la ciudad es el modelo de
/// detalle (de aceite; de gas en la Ciudad Nueva desde 1853, cuando se enciende la usina de los
/// Ísola [F, ver `docs/sistemas/arquitectura-epoca2.md`]); si no, dos vóxeles.
pub(super) fn farol_fachada(f: &Frame, e: &mut Edits, es: &Estilo, u: i32) {
    let id = if es.neo && es.anio >= 1853 { crate::detalle::Id::FarolGasBrazo } else { crate::detalle::Id::FarolBrazo };
    if !f.detalle(e, id, u as f32 + 0.5, 6.0, 0.0, 0.0, es.seed as u32 ^ 0xFA0) {
        f.raw(e, u, 8, -1, IRON);
        f.raw(e, u, 7, -1, WINDOW);
    }
}

/// [ciudad] ¿Farol de fachada en una casa nueva de la Ciudad Nueva? Las ricas y los comercios
/// de esquina, si hay hierro [E] (en la colonia lo decide `Estilo::farol`).
fn farol_neo(es: &Estilo) -> bool {
    es.neo && es.hierro && (riqueza(es) >= 2 || es.esquina > 0 && es.uso == Uso::Comercio) && hash(IVec3::new(es.seed as i32, 7, 0), es.seed ^ 0xFA1) < 0.45
}

// --- Fachada ------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Hueco {
    Puerta,
    Tienda,
    Ventana,
}

/// Reparte los vanos del frente entre `u0` y `u1` (ritmo de la guía §2.3: el lleno domina, la
/// puerta muchas veces descentrada; con simetría neoclásica, todo parejo).
fn repartir(es: &Estilo, u0: i32, u1: i32, ancho_puerta: i32) -> Vec<(i32, i32, Hueco)> {
    let mut r = rng(es, 0xFAC);
    let mut n = es.vanos.max(1) as usize;
    if es.simetria && n % 2 == 0 {
        if n < 5 { n += 1 } else { n -= 1 }
    }
    let di = match es.puerta {
        Puerta::Izquierda => 0,
        Puerta::Derecha => n - 1,
        Puerta::Centro => n / 2,
        Puerta::DescIzquierda => 1.min(n - 1),
        Puerta::DescDerecha => n.saturating_sub(2),
    };
    let mut kinds: Vec<Hueco> = (0..n).map(|i| if i == di { Hueco::Puerta } else { Hueco::Ventana }).collect();
    // Tiendas: los vanos más cercanos a la puerta.
    let mut t = es.tienda as usize;
    let mut d = 1;
    while t > 0 && d < n {
        for j in [di as isize - d as isize, (di + d) as isize] {
            if t > 0 && j >= 0 && (j as usize) < n && kinds[j as usize] == Hueco::Ventana {
                kinds[j as usize] = Hueco::Tienda;
                t -= 1;
            }
        }
        d += 1;
    }
    if es.tipo == Tipo::Cuartos && !es.renta {
        // Casa subdividida: una puerta por cuarto, con su ventanita.
        for (i, k) in kinds.iter_mut().enumerate() {
            if i % 2 == di % 2 {
                *k = Hueco::Puerta;
            }
        }
    }
    let ancho = |k: Hueco| match k {
        Hueco::Puerta => ancho_puerta,
        Hueco::Tienda => 4,
        Hueco::Ventana => 2,
    };
    let largo = u1 - u0;
    // Si no entran (con 1,5 m de muro entre vanos), se sacan ventanas lejos de la puerta.
    while kinds.len() > 1 && kinds.iter().map(|&k| ancho(k) + 3).sum::<i32>() + 1 > largo {
        let pd = kinds.iter().position(|&k| k == Hueco::Puerta).unwrap_or(0) as i32;
        let far = kinds.iter().enumerate().filter(|(_, &k)| k == Hueco::Ventana).max_by_key(|(i, _)| (*i as i32 - pd).abs()).map(|(i, _)| i);
        match far.or_else(|| kinds.iter().rposition(|&k| k == Hueco::Tienda)) {
            Some(i) => {
                kinds.remove(i);
            }
            None => break,
        }
    }
    let n = kinds.len() as i32;
    let total: i32 = kinds.iter().map(|&k| ancho(k)).sum();
    let libre = (largo - total).max(n + 1);
    let mut gaps = vec![libre / (n + 1); (n + 1) as usize];
    let mut resto = libre - gaps.iter().sum::<i32>();
    if es.simetria {
        let mut i = 0;
        while resto >= 2 {
            gaps[i] += 1;
            let j = gaps.len() - 1 - i;
            gaps[j] += 1;
            resto -= 2;
            i = (i + 1) % gaps.len().div_ceil(2);
        }
        if resto > 0 {
            let mid = gaps.len() / 2;
            gaps[mid] += resto;
        }
    } else {
        while resto > 0 {
            let i = (r.next() % gaps.len() as u64) as usize;
            gaps[i] += 1;
            resto -= 1;
        }
        // Un poco de irregularidad: se pasa medio metro de un hueco de muro a otro.
        for _ in 0..2 {
            let i = (r.next() % gaps.len() as u64) as usize;
            let j = (r.next() % gaps.len() as u64) as usize;
            if gaps[i] > 3 {
                gaps[i] -= 1;
                gaps[j] += 1;
            }
        }
    }
    let mut out = Vec::new();
    let mut u = u0 + gaps[0];
    for (i, &k) in kinds.iter().enumerate() {
        out.push((u, u + ancho(k), k));
        u += ancho(k) + gaps[i + 1];
    }
    out
}

/// Fachada de la planta baja entre `u0` y `u1`, con el techo de la planta en `techo`.
/// Devuelve los vanos (para alinear la planta alta).
pub(super) fn fachada_pb(t: &TerrainGen, f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, techo: i32) -> Vec<(i32, i32, Hueco)> {
    if let Some(z) = es.zocalo {
        f.fill(e, u0, u1, 0, es.h_zocalo, 0, 1, z);
    }
    let rico = es.clase >= 2 || es.zaguan;
    let ap = if es.tipo == Tipo::Cuartos && !es.renta { 2 } else if rico { 3 } else { 2 + i32::from(es.ornamento >= 1) };
    let hp = if es.neo { (techo - 2).clamp(5, 8) } else if rico && techo >= 9 { 6 } else { 5 };
    let vanos = repartir(es, u0, u1, ap);
    let alta = es.clase >= 2 && techo >= 9;
    let mut principal = true;
    if es.neo {
        pilastras(f, e, es, u0, u1, &vanos, es.h_zocalo, techo - 1);
    }
    for &(a, b, k) in &vanos {
        match k {
            Hueco::Puerta if es.neo => {
                puerta_neo(f, e, es, a, b, hp, es.zaguan && principal);
                if principal {
                    portada(f, e, es, a, b, hp, techo);
                    if !es.zaguan {
                        llamador(f, e, es, a, b, 2);
                    }
                    principal = false;
                }
            }
            Hueco::Ventana if es.neo => {
                let y0 = if es.ventana_alta { 1 } else { 2 };
                ventana_neo(f, e, es, a, b, y0, hp);
            }
            Hueco::Puerta => {
                let rec = if es.zaguan && principal { 2 } else { 1 };
                puerta(f, e, es, a, b, hp, rec);
                if principal {
                    portada(f, e, es, a, b, hp, techo);
                    llamador(f, e, es, a, b, rec);
                    poyo(t, f, e, es, a, b, &vanos, u1); // [ia]
                    principal = false;
                }
            }
            Hueco::Tienda => {
                // Puerta de tienda de dos hojas abiertas, sin reja (guía §1.2 G).
                opening(f, e, a, b, 0, hp, 1, WINDOW);
                f.raw_fill(e, a, a + 1, 0, hp, 1, 2, es.carpinteria);
                f.raw_fill(e, b - 1, b, 0, hp, 1, 2, es.carpinteria);
                if es.mostrador && es.hierro {
                    f.raw_fill(e, a + 1, b - 1, 2, hp - 1, 1, 2, REJA); // reja de mostrador
                }
                if es.letrero {
                    f.raw_fill(e, a - 1, b + 1, hp + 1, hp + 2, -1, 0, WOOD_DARK);
                }
            }
            Hueco::Ventana => {
                let y1 = if alta { 7 } else { 6 };
                ventana(f, e, es, a, b, 2, y1, es.reja);
            }
        }
    }
    vanos
}

/// Tapia con portón sobre el tramo libre del frente (casas que no ocupan todo el solar).
pub(super) fn tapia(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, hasta: i32) {
    if u1 - u0 < 2 {
        return;
    }
    let m = es.muro;
    f.fill(e, u0, u1, 0, 5, 0, 1, m);
    let lado = if u0 == 0 { 0 } else { u1 - 1 };
    f.fill(e, lado, lado + 1, 0, 4, 1, hasta, m);
    if u1 - u0 >= 4 {
        let a = u0 + (u1 - u0 - 3) / 2;
        opening(f, e, a, a + 3, 0, 4, 0, WOOD_DARK);
        f.clear(e, a, a + 3, 4, 5, 0, 1);
    }
}

/// Marco con la esquina siempre del lado u = 0 (espejado si la calle lateral está a la derecha).
pub(super) fn marco_esquina(f: &Frame, es: &Estilo) -> Frame {
    if es.esquina == 2 {
        Frame { origin: f.origin + f.du * (f.width - 1), du: -f.du, ..*f }
    } else {
        *f
    }
}

/// Fachada lateral de una casa de esquina: zócalo, cornisa y ventanas sobre la calle de al
/// lado, y la entrada por la arista (pilar de esquina) o por la ochava.
pub(super) fn esquina(f: &Frame, e: &mut Edits, es: &Estilo, fondo: i32, techo: i32) {
    let side = f.left_side();
    if let Some(z) = es.zocalo {
        side.fill(e, 0, fondo, 0, es.h_zocalo, 0, 1, z);
    }
    let mut w = 6;
    while w + 3 < fondo {
        ventana(&side, e, es, w, w + 2, 2, 6, es.reja);
        w += 6;
    }
    cornisa_estilo(&side, e, es, 0, fondo, techo - 1);
    let hp = 6;
    match es.entrada_esquina {
        1 => {
            // Puerta en la arista con un pilar de esquina: una puerta a cada calle.
            let pilar = piedra_o_moldura(es);
            opening(f, e, 1, 4, 0, hp, 1, es.carpinteria);
            opening(&side, e, 1, 4, 0, hp, 1, es.carpinteria);
            f.fill(e, 0, 1, 0, hp + 1, 0, 1, pilar);
            if es.mostrador && es.hierro {
                f.raw_fill(e, 2, 4, 2, hp - 1, 2, 3, REJA);
            }
            f.raw(e, 0, 0, -1, STONE); // guardacantón
        }
        2 => {
            // Ochava (la reglamentaria es posterior: pocas en la colonia).
            let c = 4;
            for w in 0..c {
                for u in 0..c {
                    if u + w < c {
                        f.clear(e, u, u + 1, -1, techo + 3, w, w + 1);
                    }
                }
            }
            for u in 0..=c {
                let w = c - u;
                let m = if (1..c).contains(&u) { es.carpinteria } else { es.muro };
                f.fill(e, u, u + 1, 0, hp, w, w + 1, m);
                f.fill(e, u, u + 1, hp, techo + es.pretil, w, w + 1, es.muro);
            }
            if es.letrero {
                for u in 1..c {
                    f.raw(e, u, hp, c - u - 1, WOOD_DARK);
                }
            }
        }
        _ => {}
    }
}

// --- Casa de una planta (teja, azotea, cuartos) --------------------------------------------

pub(super) fn casa_un_piso(t: &TerrainGen, e: &mut Edits, f0: &Frame, es: &Estilo) -> Option<Frente> {
    let f = &marco_esquina(f0, es);
    let mut r = rng(es, 0xCA5A);
    let azotea = es.cubierta == Cubierta::Azotea;
    let (x0, x1) = if es.libre_izq { (f.width - es.ancho, f.width) } else { (0, es.ancho) };
    let (fd, h) = (es.fondo, es.alto);
    let galeria = if es.galeria && !azotea { 4 } else { 0 };
    let w0 = galeria;
    let g = f.sub(0, w0, f.width, f.depth - w0, 0);
    let g = Frame { seed: f.seed, ..g };
    g.foundation(t, e, x0, x1, 0, fd, if es.piedra { STONE } else { ZOCALO });
    caja(&g, e, x0, x1, 0, h, 0, fd, es.muro);
    let vanos = fachada_pb(t, &g, e, es, x0, x1, h);
    let mut frente = None;
    if azotea {
        losa(&g, e, x0, x1, 0, fd, h - 1);
        cornisa_estilo(&g, e, es, x0, x1, h - 1);
        pretil(&g, e, es, x0, x1, 0, fd, h);
        gargolas(&g, e, es, x0, x1, h);
        frente = Some(Frente { g, u0: x0, u1: x1, h });
    } else if galeria == 0 {
        cornisa_estilo(&g, e, es, x0, x1, h - 1);
        gable(&g, e, x0, x1, 0, fd, h, 2, es.alero, es.techo, es.muro, !es.hastial);
    }
    if galeria > 0 {
        // Corredor con horcones (o pilares encalados) bajo el alero.
        let pm = if es_cal(es.muro) { es.muro } else { WOOD_DARK };
        let mut u = x0;
        while u < x1 {
            f.fill(e, u, u + 1, 0, h, 0, 1, pm);
            u += 4;
        }
        f.fill(e, x1 - 1, x1, 0, h, 0, 1, pm);
        f.surface(e, x0, x1, 0, galeria, BRICK);
        gable(f, e, x0, x1, 0, fd + galeria, h, 2, es.alero, es.techo, es.muro, true);
    }
    if es.esquina > 0 {
        esquina(&g, e, es, fd, h);
    } else if x0 > 0 || x1 < f.width {
        let (a, b) = if x0 > 0 { (0, x0) } else { (x1, f.width) };
        tapia(&g, e, es, a, b, fd + 10);
    }
    if es.farol || farol_neo(es) {
        let u = if es.esquina > 0 { 1 } else { vanos.iter().find(|v| v.2 == Hueco::Puerta).map_or(x0 + 2, |v| v.0 - 1) };
        farol_fachada(&g, e, es, u.max(x0));
    }
    // Altillo o mirador sobre la azotea.
    if azotea && es.altillo {
        let (au, aw) = (if r.f() < 0.5 { x0 + 2 } else { x1 - 10 }, if r.f() < 0.6 { 2 } else { fd - 9 });
        caja(&g, e, au, au + 8, h, h + 7, aw, aw + 8, es.muro);
        losa(&g, e, au, au + 8, aw, aw + 8, h + 6);
        ring(&g, e, au, au + 8, h + 7, h + 8, aw, aw + 8, es.muro);
        opening(&g, e, au + 3, au + 5, h + 2, h + 5, 1, WINDOW);
    }
    if azotea && es.mirador {
        mirador(&g, e, es, if es.libre_izq { x1 - 9 } else { x0 + 3 }, 3, h);
    }
    // Planta: patios, alas, fondo.
    fondo_casa(t, e, &g, es, x0, x1, fd, h, &mut r);
    frente
}

/// Mirador: cuarto alto con ventanas a los cuatro lados y su azotea.
pub(super) fn mirador(g: &Frame, e: &mut Edits, es: &Estilo, u: i32, w: i32, h: i32) {
    let s = 6;
    caja(g, e, u, u + s, h, h + 6, w, w + s, es.muro);
    losa(g, e, u, u + s, w, w + s, h + 5);
    ring(g, e, u, u + s, h + 6, h + 7, w, w + s, es.muro);
    let m = piedra_o_moldura(es);
    g.fill(e, u - 1, u + s + 1, h + 5, h + 6, w - 1, w + s + 1, m);
    g.clear(e, u, u + s, h + 5, h + 6, w, w + s);
    ring(g, e, u, u + s, h + 5, h + 6, w, w + s, es.muro);
    for (a, b, c, d) in [(u + 2, u + 4, w, w + 1), (u + 2, u + 4, w + s - 1, w + s), (u, u + 1, w + 2, w + 4), (u + s - 1, u + s, w + 2, w + 4)] {
        g.raw_fill(e, a, b, h + 2, h + 5, c, d, WINDOW);
    }
}

/// Patios, alas, segundo patio, fondo, ampliaciones, huerta, corral y letrina.
#[allow(clippy::too_many_arguments)]
pub(super) fn fondo_casa(t: &TerrainGen, e: &mut Edits, g: &Frame, es: &Estilo, x0: i32, x1: i32, fd: i32, h: i32, r: &mut Rng) {
    let azotea = es.cubierta == Cubierta::Azotea;
    let depth = g.depth;
    let ha = (h - 1).max(6);
    let aw = 7;
    let mut w;
    // Lado del ala: en la esquina, sobre la calle lateral.
    let ala_izq = if es.esquina > 0 { true } else { r.f() < 0.5 };
    let ala = |e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, hh: i32, izq: bool| {
        g.foundation(t, e, u0, u1, w0, w1, ZOCALO);
        ring(g, e, u0, u1, 0, hh, w0, w1, es.muro);
        if azotea {
            losa(g, e, u0, u1, w0, w1, hh - 1);
            ring(g, e, u0, u1, hh, hh + 1, w0, w1, es.muro);
        } else {
            media_agua(g, e, u0, u1, w0, w1, hh, !izq, es.techo, es.muro);
        }
    };
    if es.tipo == Tipo::CasaTeja && es.ala_l {
        // Casa en "L": segunda crujía hacia el fondo, con su techo a dos aguas.
        let l = r.range(14, 24);
        let (u0, u1) = if ala_izq { (x0, x0 + 8) } else { (x1 - 8, x1) };
        g.foundation(t, e, u0, u1, fd, fd + l, ZOCALO);
        ring(g, e, u0, u1, 0, h - 1, fd, fd + l, es.muro);
        gable(g, e, u0, u1, fd, fd + l, h - 1, 2, 1, es.techo, es.muro, false);
        if es.chimenea {
            chimney(g, e, u0 + 3, fd + l - 4, 0, h + 4);
        }
        let (pu0, pu1) = if ala_izq { (u1, x1 - 1) } else { (x0 + 1, u0) };
        g.surface(e, pu0, pu1, fd, fd + l, es.piso_patio);
        agua_patio(g, e, es, (pu0 + pu1) / 2 - 1, fd + 5);
        w = fd + l;
    } else if es.tipo == Tipo::Cuartos && es.renta {
        // Inquilinato: una fila de piezas a lo largo de un patio angosto, cada una con su puerta.
        let (u0, u1) = if ala_izq { (x0, x0 + 9) } else { (x1 - 9, x1) };
        let w1 = depth - 6;
        ala(e, u0, u1, fd, w1, ha, ala_izq);
        let cara = if ala_izq { u1 - 1 } else { u0 };
        let mut ww = fd + 3;
        while ww + 4 < w1 {
            g.clear(e, cara, cara + 1, 0, 5, ww, ww + 2);
            g.raw_fill(e, cara, cara + 1, 0, 5, ww, ww + 2, es.carpinteria);
            g.clear(e, cara, cara + 1, 2, 4, ww + 4, ww + 5);
            g.raw_fill(e, cara, cara + 1, 2, 4, ww + 4, ww + 5, WINDOW);
            ww += 8;
        }
        let (pu0, pu1) = if ala_izq { (u1, x1 - 1) } else { (x0 + 1, u0) };
        g.surface(e, pu0, pu1, fd, w1, es.piso_patio);
        agua_patio(g, e, es, (pu0 + pu1) / 2 - 1, fd + 6);
        // Ropa tendida.
        for k in 0..3 {
            let wr = fd + 16 + k * 12;
            if wr + 1 >= w1 {
                break;
            }
            g.raw_fill(e, pu0, pu0 + 1, 0, 5, wr, wr + 1, WOOD_DARK);
            g.raw_fill(e, pu1 - 1, pu1, 0, 5, wr, wr + 1, WOOD_DARK);
            g.raw_fill(e, pu0 + 1, pu1 - 1, 4, 5, wr, wr + 1, if (k + es.seed as i32) % 2 == 0 { SACK } else { CAL });
        }
        letrina(g, e, (pu0 + pu1) / 2, depth - 4);
        return;
    } else if es.patios >= 1 {
        let p1 = if es.clase >= 2 { r.range(14, 18) } else { r.range(11, 15) };
        let w1 = (fd + p1).min(depth - 20);
        let dos_alas = es.alas >= 2 && x1 - x0 >= 22;
        let (a0, a1) = if ala_izq { (x0, x0 + aw) } else { (x1 - aw, x1) };
        ala(e, a0, a1, fd, w1, ha, ala_izq);
        let (mut pu0, mut pu1) = if ala_izq { (a1, x1) } else { (x0, a0) };
        if dos_alas {
            let (b0, b1) = if ala_izq { (x1 - aw, x1) } else { (x0, x0 + aw) };
            ala(e, b0, b1, fd, w1, ha, !ala_izq);
            if ala_izq { pu1 = b0 } else { pu0 = b1 }
        } else {
            // Medianera del otro lado del patio.
            let u = if ala_izq { x1 - 1 } else { x0 };
            g.fill(e, u, u + 1, 0, 5, fd, w1, es.muro);
            if ala_izq { pu1 -= 1 } else { pu0 += 1 }
        }
        g.surface(e, pu0, pu1, fd, w1, es.piso_patio);
        agua_patio(g, e, es, (pu0 + pu1) / 2 - 1, fd + p1 / 2 - 1);
        if es.chimenea {
            chimney(g, e, a0 + 2, w1 - 4, 0, ha + 3);
        }
        w = w1;
        if es.patios >= 2 && w + 22 < depth {
            // Crujía entre patios y segundo patio, de servicio.
            let cw = 8;
            g.foundation(t, e, x0, x1, w, w + cw, ZOCALO);
            ring(g, e, x0, x1, 0, ha, w, w + cw, es.muro);
            if azotea {
                losa(g, e, x0, x1, w, w + cw, ha - 1);
                ring(g, e, x0, x1, ha, ha + 1, w, w + cw, es.muro);
            } else {
                gable(g, e, x0, x1, w, w + cw, ha, 2, 1, es.techo, es.muro, true);
            }
            g.clear(e, (pu0 + pu1) / 2 - 1, (pu0 + pu1) / 2 + 1, 0, 5, w, w + cw); // paso
            w += cw;
            let p2 = r.range(9, 12);
            let w2 = (w + p2).min(depth - 10);
            let (s0, s1) = if ala_izq { (x0, x0 + 6) } else { (x1 - 6, x1) };
            ala(e, s0, s1, w, w2, ha - 1, ala_izq);
            let (q0, q1) = if ala_izq { (s1, x1 - 1) } else { (x0 + 1, s0) };
            g.surface(e, q0, q1, w, w2, if es.clase >= 2 { BRICK } else { DIRT });
            if es.parral {
                parral(g, e, q0 + 1, q0 + 7, w + 2, w + 8);
            }
            w = w2;
        } else if es.parral && pu1 - pu0 >= 8 {
            parral(g, e, pu0 + 1, pu0 + 6, w - 7, w - 2);
        }
    } else {
        // Sin patio: un corral o huerta enseguida, con la cocina en un rancho del fondo.
        g.surface(e, x0 + 1, x1 - 1, fd, fd + 8, DIRT);
        w = fd + 8;
    }
    // Fondo: galpón de teja (azotea al frente y teja atrás), depósito o cocina.
    if (es.fondo_teja || es.deposito) && w + 14 < depth - 4 {
        let (bw, bd) = if es.deposito { (x1 - x0 - 2, 14) } else { (r.range(10, 14), r.range(8, 10)) };
        let u0 = if ala_izq { x0 + 1 } else { x1 - 1 - bw };
        let wb = w + 2;
        let wall = if es.deposito && !es_cal(es.muro) { es.muro } else if es.deposito { BRICK } else { es.muro };
        g.foundation(t, e, u0, u0 + bw, wb, wb + bd, ZOCALO);
        ring(g, e, u0, u0 + bw, 0, 7, wb, wb + bd, wall);
        gable(g, e, u0, u0 + bw, wb, wb + bd, 7, 2, 1, if es.cubierta == Cubierta::Teja || es.cubierta == Cubierta::Azotea { TILE } else { es.techo }, wall, true);
        if es.deposito {
            opening(g, e, u0 + bw / 2 - 2, u0 + bw / 2 + 2, 0, 6, 0, WOOD_DARK);
            for k in 0..3 {
                g.raw_fill(e, x0 + 2 + k * 3, x0 + 4 + k * 3, 0, 2, wb - 2, wb, if k % 2 == 0 { HIDE } else { SACK });
            }
        }
        w = wb + bd;
    }
    // Ampliaciones: cuartos que se fueron sumando, con otro tono de cal y otro techo.
    for k in 0..es.ampliaciones {
        let (cw, cd) = (r.range(6, 8), r.range(6, 8));
        if w + cd + 6 >= depth {
            break;
        }
        let izq = (k % 2 == 0) == ala_izq;
        let u0 = if izq { x0 } else { x1 - cw };
        let tono = if es_cal(es.muro) { [CAL, CAL_HUESO, es.muro][(k as usize + es.seed as usize) % 3] } else { es.muro };
        g.foundation(t, e, u0, u0 + cw, w, w + cd, ZOCALO);
        ring(g, e, u0, u0 + cw, 0, 6, w, w + cd, tono);
        if azotea && k == 0 {
            losa(g, e, u0, u0 + cw, w, w + cd, 5);
            ring(g, e, u0, u0 + cw, 6, 7, w, w + cd, tono);
        } else {
            media_agua(g, e, u0, u0 + cw, w, w + cd, 6, izq, if es.cubierta == Cubierta::Azotea { TILE } else { es.techo }, tono);
        }
        let puerta_u = if izq { u0 + cw - 1 } else { u0 };
        g.clear(e, puerta_u, puerta_u + 1, 0, 4, w + 2, w + 4);
        g.raw_fill(e, puerta_u, puerta_u + 1, 0, 4, w + 2, w + 4, es.carpinteria);
        w += cd + r.range(0, 2);
    }
    // Huerta, corral y letrina hasta el fondo del solar.
    if w + 4 < depth {
        g.surface(e, x0 + 1, x1 - 1, w, depth - 1, if es.corral { DIRT } else { GARDEN });
        if es.corral {
            fence(g, e, x0, x1, w, depth - 1, (x0 + x1) / 2);
        } else if r.f() < 0.5 {
            for k in 0..r.range(1, 3) {
                frutal(g, e, x0 + 3 + k * 6, (w + depth) / 2 + k % 2 * 3, es.seed ^ k as u64);
            }
        }
        letrina(g, e, if ala_izq { x1 - 5 } else { x0 + 3 }, depth - 5);
    }
}

// --- Casa de altos ------------------------------------------------------------------------

pub(super) fn casa_altos(t: &TerrainGen, e: &mut Edits, f0: &Frame, es: &Estilo) -> Option<Frente> {
    let g = marco_esquina(f0, es);
    let mut r = rng(es, 0xA170);
    let (x0, x1) = (0, g.width);
    let fd = es.fondo;
    let h1 = es.alto;
    let h = h1 + es.alto_pa * (es.pisos as i32 - 1);
    let azotea = es.cubierta == Cubierta::Azotea;
    g.foundation(t, e, x0, x1, 0, fd, if es.piedra { STONE } else { ZOCALO });
    caja(&g, e, x0, x1, 0, h, 0, fd, es.muro);
    // Entrepisos (losas interiores, para que no se vea vacío por las ventanas abiertas).
    for p in 1..es.pisos as i32 {
        let y = h1 + es.alto_pa * (p - 1);
        g.fill(e, x0 + 1, x1 - 1, y, y + 1, 2, 4, AZOTEA);
    }
    let vanos = fachada_pb(t, &g, e, es, x0, x1, h1);
    // Faja de entrepiso (impostas).
    let m = piedra_o_moldura(es);
    for p in 1..es.pisos as i32 {
        let y = h1 + es.alto_pa * (p - 1);
        g.fill(e, x0, x1, y, y + 1, -1, 0, m);
        // Puertas-ventana alineadas con los vanos de abajo.
        for &(a, b, _) in &vanos {
            let c = (a + b) / 2;
            let (va, vb) = (c - 1, c + 1);
            let alto = if es.neo { (es.alto_pa - 2).max(5) } else { (es.alto_pa - 3).max(4) };
            if es.neo {
                // Puerta-ventana con persianas y montante; guardapolvo con frontón.
                let abierta = hash(g.at(va, y, 0), es.seed) < 0.5;
                opening(&g, e, va, vb, y + 1, y + 1 + alto, 2, if abierta { WINDOW } else { es.carpinteria });
                if abierta {
                    g.raw_fill(e, va, va + 1, y + 1, y + alto, 1, 2, es.carpinteria);
                }
                g.raw_fill(e, va, vb, y + alto, y + 1 + alto, 1, 2, HIERRO_FUNDIDO);
                g.fill(e, va - 1, vb + 1, y + 1 + alto, y + 2 + alto, -1, 0, m);
                if es.ornamento >= 3 {
                    g.fill(e, va, vb, y + 2 + alto, y + 3 + alto, -1, 0, m);
                }
            } else {
                opening(&g, e, va, vb, y + 1, y + 1 + alto, 1, if hash(g.at(va, y, 0), es.seed) < 0.3 { es.carpinteria } else { WINDOW });
            }
            if es.ornamento >= 2 && !es.neo {
                g.fill(e, va - 1, vb + 1, y + 1 + alto, y + 2 + alto, -1, 0, m); // guardapolvo
            }
            if es.balcon == 1 {
                balcon(&g, e, es, va - 1, vb + 1, y);
            }
        }
        if es.balcon == 2 {
            balcon(&g, e, es, x0 + 1, x1 - 1, y);
        }
    }
    if es.neo {
        // Pilastras de la planta alta, sobre las de abajo.
        let h1p = h1 + 1;
        pilastras(&g, e, es, x0, x1, &vanos, h1p, h - 1);
    }
    let mut frente = None;
    if azotea {
        losa(&g, e, x0, x1, 0, fd, h - 1);
        cornisa_estilo(&g, e, es, x0, x1, h - 1);
        if es.pretil > 0 {
            pretil(&g, e, es, x0, x1, 0, fd, h);
            gargolas(&g, e, es, x0, x1, h);
            frente = Some(Frente { g, u0: x0, u1: x1, h });
        }
        if es.mirador {
            mirador(&g, e, es, if r.f() < 0.5 { 2 } else { x1 - 8 }, 3, h);
        }
    } else {
        cornisa_estilo(&g, e, es, x0, x1, h - 1);
        gable(&g, e, x0, x1, 0, fd, h, 2, es.alero, es.techo, es.muro, true);
    }
    if es.esquina > 0 {
        esquina(&g, e, es, fd, h1);
        // Ventanas altas también sobre la calle lateral.
        let side = g.left_side();
        let mut w = 6;
        while w + 3 < fd {
            opening(&side, e, w, w + 2, h1 + 1, h1 + 6, 1, WINDOW);
            w += 6;
        }
        cornisa_estilo(&side, e, es, 0, fd, h - 1);
    }
    if es.farol || farol_neo(es) {
        farol_fachada(&g, e, es, if es.esquina > 0 { 1 } else { x0 + 1 });
    }
    // Ala de dos plantas alrededor del patio (de renta: con galería de madera).
    let izq = es.esquina > 0 || r.f() < 0.5;
    let aw = 8;
    let w1 = (fd + r.range(14, 20)).min(g.depth - 20);
    let (a0, a1) = if izq { (x0, x0 + aw) } else { (x1 - aw, x1) };
    let ha = if es.renta || r.f() < 0.6 { h - 1 } else { h1 };
    g.foundation(t, e, a0, a1, fd, w1, ZOCALO);
    ring(&g, e, a0, a1, 0, ha, fd, w1, es.muro);
    if azotea {
        losa(&g, e, a0, a1, fd, w1, ha - 1);
        ring(&g, e, a0, a1, ha, ha + 1, fd, w1, es.muro);
    } else {
        media_agua(&g, e, a0, a1, fd, w1, ha, !izq, es.techo, es.muro);
    }
    let (pu0, pu1) = if izq { (a1, x1 - 1) } else { (x0 + 1, a0) };
    if es.renta || ha > h1 {
        // Galería alta hacia el patio, con baranda y pies derechos.
        let gu = if izq { a1 } else { a0 - 2 };
        g.raw_fill(e, gu, gu + 2, h1, h1 + 1, fd, w1, WOOD);
        let bu = if izq { gu + 1 } else { gu };
        g.raw_fill(e, bu, bu + 1, h1 + 1, h1 + 3, fd, w1, if es.hierro { REJA } else { WOOD_DARK });
        let mut w = fd + 2;
        while w < w1 {
            g.raw_fill(e, bu, bu + 1, 0, h1, w, w + 1, WOOD_DARK);
            w += 6;
        }
    }
    if !es.renta {
        let u = if izq { x1 - 1 } else { x0 };
        g.fill(e, u, u + 1, 0, 6, fd, w1, es.muro);
    }
    g.surface(e, pu0, pu1, fd, w1, es.piso_patio);
    agua_patio(&g, e, es, (pu0 + pu1) / 2 - 1, (fd + w1) / 2 - 1);
    if es.chimenea {
        chimney(&g, e, a0 + 3, w1 - 4, 0, ha + 3);
    }
    // Segundo patio y fondo como en la casa de una planta.
    let resto = Estilo { patios: 0, alas: 1, aljibe: false, pozo: false, cubierta: es.cubierta, ..*es };
    fondo_casa_desde(t, e, &g, &resto, x0, x1, w1, h1, &mut r);
    frente
}

/// El fondo de una casa desde la profundidad `w` (galpón, ampliaciones, huerta, letrina).
#[allow(clippy::too_many_arguments)]
pub(super) fn fondo_casa_desde(t: &TerrainGen, e: &mut Edits, g: &Frame, es: &Estilo, x0: i32, x1: i32, w: i32, h: i32, r: &mut Rng) {
    let sub = Frame { seed: g.seed, ..g.sub(0, w, g.width, g.depth - w, 0) };
    fondo_casa(t, e, &sub, &Estilo { patios: 0, ..*es }, x0, x1, 0, h, r);
}

/// Balcón: losa volada de piedra o de cal sobre ménsulas y baranda de hierro (o de madera,
/// con su tejadito, en las casas de familias canarias).
pub(super) fn balcon(f: &Frame, e: &mut Edits, es: &Estilo, u0: i32, u1: i32, y: i32) {
    if es.balcon_madera {
        f.raw_fill(e, u0, u1, y, y + 1, -2, 0, WOOD);
        f.raw_fill(e, u0, u1, y + 1, y + 3, -2, -1, WOOD_DARK);
        f.raw_fill(e, u0, u0 + 1, y + 1, y + 3, -1, 0, WOOD_DARK);
        f.raw_fill(e, u1 - 1, u1, y + 1, y + 3, -1, 0, WOOD_DARK);
        // Pies derechos y tejadito.
        f.raw_fill(e, u0, u0 + 1, y + 3, y + 6, -2, -1, WOOD_DARK);
        f.raw_fill(e, u1 - 1, u1, y + 3, y + 6, -2, -1, WOOD_DARK);
        f.raw_fill(e, u0, u1, y + 6, y + 7, -2, 0, TILE);
    } else {
        let m = if es.neo && es.marmol { MARMOL } else { piedra_o_moldura(es) };
        let hierro = if es.neo { HIERRO_FUNDIDO } else { REJA };
        f.fill(e, u0, u1, y, y + 1, -2, 0, m);
        f.raw_fill(e, u0, u1, y + 1, y + 3, -2, -1, hierro);
        f.raw_fill(e, u0, u0 + 1, y + 1, y + 3, -1, 0, hierro);
        f.raw_fill(e, u1 - 1, u1, y + 1, y + 3, -1, 0, hierro);
        if es.neo {
            // Ménsulas bajo la losa.
            let mut u = u0;
            while u < u1 {
                f.raw(e, u, y - 1, -1, es.moldura);
                u += 3;
            }
        }
        // [ciudad] Macetas con geranios en los balcones de las casas de clase media para
        // arriba (más en la Ciudad Nueva) [E].
        if riqueza(es) >= 1 {
            let p = if es.neo { 0.55 } else { 0.35 };
            let mut u = u0 + 1;
            while u < u1 - 1 {
                if hash(f.at(u, y, -1), es.seed ^ 0xAC37) < p {
                    let giro = hash(f.at(u, y, -2), es.seed) * std::f32::consts::TAU;
                    f.detalle(e, crate::detalle::Id::Maceta, u as f32 + 0.5, (y + 1) as f32, -0.55, giro, es.seed as u32 ^ u as u32);
                }
                u += 2;
            }
        }
    }
}

// --- Ranchos ------------------------------------------------------------------------------

fn rancho(t: &TerrainGen, e: &mut Edits, f: &Frame, es: &Estilo) -> Option<Frente> {
    let mut r = rng(es, 0x2A);
    // Con la cumbrera perpendicular, el lado corto da a la calle.
    let (lu, lw) = if es.hastial { (es.fondo, es.ancho.min(20)) } else { (es.ancho.min(f.width - 2), es.fondo) };
    let u0 = ((f.width - lu) / 2 + r.range(-3, 3)).clamp(1, f.width - 1 - lu);
    let w0 = es.retiro;
    let (u1, w1) = (u0 + lu, w0 + lw);
    let h = es.alto;
    f.foundation(t, e, u0, u1, w0, w1, es.muro);
    ring(f, e, u0, u1, 0, h, w0, w1, es.muro);
    match es.cubierta {
        Cubierta::Paja => {
            gable(f, e, u0, u1, w0, w1, h, 1, 1, THATCH, es.muro, !es.hastial);
            gable(f, e, u0, u1, w0, w1, h + 1, 1, 1, THATCH, es.muro, !es.hastial);
        }
        _ => gable(f, e, u0, u1, w0, w1, h, 2, 1, es.techo, es.muro, !es.hastial),
    }
    let g = f.sub(0, w0, f.width, f.depth - w0, 0);
    let g = Frame { seed: f.seed, ..g };
    // Puerta de una hoja y ventanucos con postigo (guía §2.3).
    let vanos = repartir(&Estilo { tienda: 0, ..*es }, u0 + 1, u1 - 1, 2);
    for &(a, b, k) in &vanos {
        match k {
            Hueco::Puerta => opening(&g, e, a, b, 0, 4, 1, if es.carpinteria == WOOD { WOOD_DARK } else { es.carpinteria }),
            _ => {
                if es.mostrador && es.hierro {
                    opening(&g, e, a, a + 2, 2, 4, 1, WINDOW);
                    g.raw_fill(e, a, a + 2, 2, 4, 0, 1, REJA);
                } else {
                    opening(&g, e, a, a + 2, 2, 4, 1, if r.f() < 0.5 { es.carpinteria } else { WINDOW });
                }
            }
        }
    }
    // Enramada de palos y ramas al frente (o atrás si no hay lugar).
    if es.enramada {
        let (ew0, ew1) = if w0 >= 4 { (w0 - 4, w0) } else { (w1, w1 + 4) };
        for u in [u0, (u0 + u1) / 2, u1 - 1] {
            let w = if w0 >= 4 { ew0 } else { ew1 - 1 };
            f.raw_fill(e, u, u + 1, 0, h - 1, w, w + 1, WOOD_DARK);
        }
        for w in ew0..ew1 {
            for u in u0..u1 {
                f.raw(e, u, h - 1, w, if hash(f.at(u, h, w), es.seed) < 0.5 { THATCH_DARK } else { LEAF_DARK });
            }
        }
        if es.mostrador {
            // Palenque para atar los caballos.
            f.raw_fill(e, u0, u1, 2, 3, -2, -1, WOOD_DARK);
            f.raw_fill(e, u0, u0 + 1, 0, 2, -2, -1, WOOD_DARK);
            f.raw_fill(e, u1 - 1, u1, 0, 2, -2, -1, WOOD_DARK);
        }
    }
    // Horno de barro al costado, si entra en el solar.
    if es.horno {
        let ou = if u1 + 6 <= f.width { Some(u1 + 2) } else if u0 >= 6 { Some(u0 - 5) } else { None };
        if let Some(ou) = ou {
            dome(f, e, ou as f32 + 1.5, (w0 + 3) as f32, 2.2, 0, ADOBE);
        }
    }
    let mut back = w1 + 3;
    // Cocina aparte: otro ranchito atrás.
    if es.cocina_aparte && back + 8 < f.depth - 8 {
        let cu = if r.f() < 0.5 { 2 } else { f.width - 8 };
        f.foundation(t, e, cu, cu + 6, back, back + 6, es.muro);
        ring(f, e, cu, cu + 6, 0, 4, back, back + 6, es.muro);
        gable(f, e, cu, cu + 6, back, back + 6, 4, 1, 1, if es.cubierta == Cubierta::Paja { THATCH } else { es.techo }, es.muro, true);
        f.clear(e, cu + 2, cu + 4, 0, 3, back, back + 1);
        back += 8;
    }
    let back = back.min(f.depth - 6);
    f.surface(e, 2, f.width - 2, back, f.depth - 2, if es.corral { DIRT } else { GARDEN });
    if es.corral || r.f() < 0.5 {
        fence(f, e, 1, f.width - 1, back, f.depth - 1, f.width / 2);
    }
    if es.corral {
        let along = r.f() < 0.5;
        animal(f, e, 6, back + 6, along, if r.f() < 0.5 { HIDE } else { WOOD_DARK });
    }
    letrina(f, e, f.width - 6, f.depth - 6);
    None
}

fn rancho_cuero(_t: &TerrainGen, e: &mut Edits, f: &Frame, es: &Estilo) -> Option<Frente> {
    let mut r = rng(es, 0xC0E);
    let (lu, lw) = (es.ancho, es.fondo);
    let u0 = ((f.width - lu) / 2 + r.range(-4, 4)).clamp(1, f.width - 1 - lu);
    let w0 = es.retiro;
    let (u1, w1) = (u0 + lu, w0 + lw);
    let h = es.alto;
    // Palos esquineros y cueros cosidos.
    for (u, w) in [(u0, w0), (u1 - 1, w0), (u0, w1 - 1), (u1 - 1, w1 - 1)] {
        f.raw_fill(e, u, u + 1, 0, h, w, w + 1, FENCE);
    }
    f.fill(e, u0 + 1, u1 - 1, 0, h - 1, w0, w0 + 1, HIDE);
    f.fill(e, u0 + 1, u1 - 1, 0, h - 1, w1 - 1, w1, HIDE);
    f.fill(e, u0, u0 + 1, 0, h - 1, w0 + 1, w1 - 1, HIDE);
    f.fill(e, u1 - 1, u1, 0, h - 1, w0 + 1, w1 - 1, HIDE);
    gable(f, e, u0, u1, w0, w1, h - 1, 2, 1, HIDE, HIDE, true);
    let d = u0 + 1 + r.range(0, (lu - 4).max(0));
    f.clear(e, d, d + 2, 0, 3, w0, w0 + 1);
    f.raw_fill(e, d, d + 2, 0, 3, w0 + 1, w0 + 2, WINDOW);
    // Fogón al aire libre.
    let fw = (w1 + 2).min(f.depth - 4);
    ring(f, e, u0, u0 + 3, 0, 1, fw, fw + 3, STONE);
    f.raw(e, u0 + 1, 0, fw + 1, BRICK_DARK);
    if es.corral {
        round_fence(f, e, (f.width / 2) as f32, (f.depth - 16) as f32, 8.0);
        animal(f, e, f.width / 2 - 2, f.depth - 17, true, HIDE);
    }
    f.surface(e, 2, f.width - 2, w1 + 1, f.depth - 2, DIRT);
    None
}

// --- Quinta -------------------------------------------------------------------------------

fn quinta(t: &TerrainGen, e: &mut Edits, f: &Frame, es: &Estilo) -> Option<Frente> {
    let mut r = rng(es, 0x0F1);
    let azotea = es.cubierta == Cubierta::Azotea;
    let x0 = if es.libre_izq { f.width - es.ancho - 1 } else { 1 };
    let x1 = x0 + es.ancho;
    let wg = es.retiro; // frente de la galería
    let w0 = wg + 4; // muro de la casa
    let fd = es.fondo;
    let h = es.alto;
    let g = f.sub(0, w0, f.width, f.depth - w0, 0);
    let g = Frame { seed: f.seed, ..g };
    g.foundation(t, e, x0, x1, 0, fd, ZOCALO);
    caja(&g, e, x0, x1, 0, h, 0, fd, es.muro);
    let _ = fachada_pb(t, &g, e, es, x0, x1, h);
    // Galería con pilares encalados (o horcones) al frente; en la quinta italianizante,
    // columnas claras con capitel y piso de mármol en damero.
    let pm = if es.neo { es.moldura } else if es_cal(es.muro) { es.muro } else { WOOD_DARK };
    let mut u = x0;
    while u < x1 {
        f.fill(e, u, u + 1, 0, h - 1, wg, wg + 1, pm);
        if es.neo {
            f.fill(e, u, u + 1, h - 2, h - 1, wg - 1, wg + 2, pm);
        }
        u += 4;
    }
    f.fill(e, x1 - 1, x1, 0, h - 1, wg, wg + 1, pm);
    if es.neo {
        for w in wg..w0 {
            for u in x0..x1 {
                f.raw(e, u, -1, w, if (u + w) % 2 == 0 { MARMOL } else { MARMOL_GRIS });
            }
        }
    } else {
        f.surface(e, x0, x1, wg, w0, BRICK);
    }
    if es.neo && es.pilastras > 0 {
        pilastras(&g, e, es, x0, x1, &[], 0, h - 1);
    }
    if es.palmera && wg >= 6 {
        palmera(f, e, if es.libre_izq { 3 } else { f.width - 5 }, wg / 2 - 1, 16, es.seed);
    }
    if azotea && es.neo {
        f.fill(e, x0, x1, h - 1, h, wg, w0 + 1, AZOTEA);
        losa(&g, e, x0, x1, 0, fd, h - 1);
        ring(f, e, x0, x1, h, h + 1, wg + 1, w0 + fd, es.muro);
        // Balaustrada sobre la galería.
        pretil(f, e, &Estilo { pretil_tipo: 2, pretil: 2, ..*es }, x0, x1, wg, wg + 2, h - 1);
        cornisa_estilo(f, e, es, x0, x1, h - 2);
    } else if azotea {
        f.fill(e, x0, x1, h - 1, h, wg, w0 + 1, AZOTEA);
        losa(&g, e, x0, x1, 0, fd, h - 1);
        ring(f, e, x0, x1, h, h + 1, wg, w0 + fd, es.muro);
    } else {
        gable(f, e, x0, x1, wg, w0 + fd, h - 1, 2, es.alero, es.techo, es.muro, true);
    }
    // Cerco del frente: tuna (verde) o pirca de piedra, con la tranquera.
    let cerco = if r.f() < 0.5 { LEAF_DARK } else { MAMPOSTERIA };
    for u in 0..f.width {
        if (u - f.width / 2).abs() > 2 {
            f.raw_fill(e, u, u + 1, 0, 2, 0, 1, cerco);
        }
    }
    f.surface(e, f.width / 2 - 2, f.width / 2 + 3, 0, wg, DIRT_ROAD);
    // Parral, pozo, horno, frutales y huerta.
    let back = w0 + fd + 2;
    if es.parral {
        parral(f, e, 2, 9, back, back + 7);
    }
    agua_patio(f, e, &Estilo { aljibe: false, pozo: true, ..*es }, f.width - 6, back + 2);
    if es.horno {
        dome(f, e, (f.width - 4) as f32, (back + 10) as f32, 2.2, 0, ADOBE);
    }
    let huerta = back + 12;
    if huerta + 6 < f.depth {
        for w in huerta..f.depth - 2 {
            f.surface(e, 2, f.width - 2, w, w + 1, if w % 3 == 0 { DIRT } else { GARDEN });
        }
        for k in 0..r.range(3, 6) {
            let (u, w) = (3 + (k * 7) % (f.width - 6), huerta + 3 + (k * 11) % (f.depth - huerta - 6).max(1));
            frutal(f, e, u, w, es.seed ^ (k as u64 + 1));
        }
    }
    if es.corral {
        fence(f, e, 1, f.width - 1, f.depth - 18, f.depth - 1, 6);
    }
    None
}

// --- Desgaste y recorte -------------------------------------------------------------------

/// Desgaste según los años desde el blanqueo, el revoque caído y la edad de las tejas
/// (guía §4.3): la cal se lava arriba y se ensucia abajo; donde se cae el revoque asoma el
/// muro (piedra, ladrillo o adobe).
pub fn envejecer(e: &mut Edits, f: &Frame, es: &Estilo) {
    let b = es.blanqueo as f32;
    let sus = es.sustrato;
    for (p, m) in e.design.iter_mut() {
        let (_, y, _) = f.local(*p);
        let h = hash(*p, es.seed ^ 0xA6E);
        if es_cal(*m) {
            let base = limpio(*m);
            let pd = if y <= 2 { 0.12 + 0.035 * b } else if y >= es.alto - 2 { 0.01 + 0.006 * b } else { 0.02 + 0.015 * b };
            *m = if h < pd { sucio(base) } else { base };
            if es.caido > 0.0 && y >= 0 && y < 7 {
                let j = (hash(IVec3::new(p.x, 0, p.z), es.seed) * 3.0) as i32;
                let celda = IVec3::new((p.x + j).div_euclid(3), (p.y + j).div_euclid(3), (p.z + j).div_euclid(3));
                if hash(celda, es.seed ^ 0xCA1D) < es.caido * if y <= 2 { 0.16 } else { 0.06 } && hash(*p, es.seed ^ 7) < 0.7 {
                    *m = if hash(*p, es.seed ^ 9) < 0.3 { sucio(sus) } else { sus };
                }
            }
        } else if matches!(*m, TILE | TILE_DARK | TILE_LIGHT) {
            *m = match (es.teja_edad, *m) {
                (0, TILE_DARK) if h < 0.7 => TILE,
                (0, TILE) if h < 0.3 => TILE_LIGHT,
                (2, TILE) if h < 0.15 => TILE_LIQUEN,
                (2, TILE_LIGHT) if h < 0.6 => TILE_DARK,
                (_, m) => m,
            };
        } else if *m == THATCH && es.blanqueo >= 6 && h < 0.3 {
            *m = THATCH_DARK;
        }
    }
}

/// Saca lo que se salió del solar (aleros sobre el vecino): todo queda en u ∈ [0, ancho) y
/// w ∈ [−2, fondo).
fn recortar(e: &mut Edits, f: &Frame) {
    let dentro = |u: i32, w: i32| u >= 0 && u < f.width && w >= -2 && w < f.depth;
    e.design.retain(|p, _| {
        let (u, _, w) = f.local(*p);
        dentro(u, w)
    });
    e.design_surface.retain(|c, _| {
        let (u, _, w) = f.local(IVec3::new(c.x, 0, c.y));
        dentro(u, w)
    });
}
