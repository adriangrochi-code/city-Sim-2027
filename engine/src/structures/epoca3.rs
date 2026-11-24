//! Kit de vóxeles de la época 3, Inmigración y Belle Époque (≈1870–1930): la casa standard
//! ("casa chorizo"), la casa de altos italianizante, el edificio de renta de tres o cuatro
//! plantas con cúpula o mansarda en la esquina, el almacén y bar de esquina, el conventillo
//! (una casa vieja dividida en piezas), la fábrica de ladrillo con su chimenea, y los edificios
//! de servicio: estación de ferrocarril, usina eléctrica, cocheras de tranvía, hotel de
//! inmigrantes y puerto moderno con muelle y grúas.
//!
//! Vóxeles de diseño de 0,5 m; el frente (w = 0) da a la calle. Las fuentes y las marcas
//! [F]/[E] están en `docs/sistemas/arquitectura-epoca3.md`.

use super::casas::{self, Frente, Hueco};
use super::estilo::*;
use super::*;
use crate::buildings::{Edits, Rng};
use crate::palette::*;
use crate::terrain::TerrainGen;

/// Primer año de la época 3: en 1869 corre el primer tren (Bella Vista–Las Piedras, 1 de enero)
/// y en 1871 llega a Montevideo [F]; 1870 es el año redondo del cambio de época.
pub const ANIO_INMIGRACION: u16 = 1870;

/// Tipos del kit de la época 3 (`Estilo::e3`).
pub mod tipo3 {
    /// Casa standard o "casa chorizo": una planta, zaguán y piezas en hilera sobre un patio
    /// lateral con galería.
    pub const STANDARD: u8 = 1;
    /// Casa de altos italianizante (se dibuja como la casa de altos, con los colores y los
    /// balcones de la época).
    pub const ALTOS: u8 = 2;
    /// Edificio de renta de tres o cuatro plantas.
    pub const RENTA: u8 = 3;
    /// Almacén y bar de esquina, con la ochava.
    pub const ESQUINA: u8 = 4;
    /// Casa de inquilinato de nueva planta.
    pub const INQUILINATO: u8 = 5;
}

/// Colores de la paleta de la época (`docs/epocas/03-inmigracion.md` §11) y sus pesos [E].
const COLORES3: [(Mat, f32); 6] = [(ROSA_VIEJO, 0.24), (AMARILLO_PALIDO, 0.20), (CELESTE_GASTADO, 0.12), (OCRE_CLARO, 0.16), (GRIS_PIEDRA, 0.16), (CAL_HUESO, 0.12)];

fn sortear(r: &mut Rng, v: &[(Mat, f32)]) -> Mat {
    let total: f32 = v.iter().map(|x| x.1).sum();
    let mut x = r.f() * total;
    for &(m, p) in v {
        if x < p {
            return m;
        }
        x -= p;
    }
    v[v.len() - 1].0
}

/// La parte de la época 3 del estilo: tipología, colores y remates (sobre la fachada
/// neoclásica, que ya se eligió). Sin revoque no hay fachada nueva: el ladrillo queda visto.
pub fn estilo3(e: &mut Estilo, m: &MaterialesObra, ctx: &Contexto, r: &mut Rng, nivel: u8, alta: bool) {
    if !e.neo {
        return;
    }
    e.muro = sortear(r, &COLORES3);
    e.moldura = if matches!(e.muro, CAL_HUESO | AMARILLO_PALIDO) && r.f() < 0.5 { GRIS_PIEDRA } else { BLANCO_PIEDRA };
    // Más ornamento que en la Ciudad Nueva: molduras de símil piedra, ménsulas y jarrones [E].
    e.ornamento = (e.ornamento + 1).min(3);
    e.cornisa = 3;
    e.pilastras = e.pilastras.max(1);
    let firme = matches!(m.muro, Muro::Piedra | Muro::Ladrillo);
    match (e.uso, e.tipo) {
        (Uso::Vivienda, Tipo::CasaAzotea) => {
            e.e3 = tipo3::STANDARD;
            e.vanos = 3;
            e.simetria = false;
            e.puerta = if ala_izq(e) { Puerta::Derecha } else { Puerta::Izquierda };
            e.zaguan = true;
            e.pretil = 2;
            e.pretil_tipo = 2;
            e.ancho = 24;
            e.fondo = r.range(10, 12);
            e.alto = 10;
            e.mirador = false;
        }
        (Uso::Vivienda, Tipo::Cuartos) => e.e3 = tipo3::INQUILINATO,
        (_, Tipo::CasaAltos) if firme && nivel >= 3 && (alta || e.clase >= 2 && r.f() < 0.5 || e.uso == Uso::Comercio && r.f() < 0.5) => {
            e.e3 = tipo3::RENTA;
            e.pisos = if e.clase >= 3 || alta && r.f() < 0.5 { 4 } else { 3 };
            e.alto = 10;
            e.alto_pa = 8;
            e.fondo = 16;
            e.vanos = if e.clase >= 2 { 5 } else { 3 };
            e.simetria = true;
            e.puerta = Puerta::Centro;
            e.zaguan = true;
            e.balcon = 2;
            e.pretil = 2;
            e.pretil_tipo = 2;
            e.mirador = false;
            e.remate = if e.esquina > 0 {
                let x = r.f();
                if x < 0.55 { 1 } else if x < 0.8 { 2 } else { 0 }
            } else {
                u8::from(r.f() < 0.15) * 2
            };
            if e.uso == Uso::Comercio {
                e.tienda = 2;
            }
        }
        (Uso::Vivienda, Tipo::CasaAltos) => {
            e.e3 = tipo3::ALTOS;
            e.balcon = if m.hierro { if r.f() < 0.7 { 2 } else { 1 } } else { 0 };
        }
        (Uso::Comercio, Tipo::CasaAzotea | Tipo::CasaTeja) if e.esquina > 0 => {
            // Almacén y bar en la ochava (la ochava reglamentaria es de fines del XIX) [E].
            e.e3 = tipo3::ESQUINA;
            e.entrada_esquina = 2;
            e.letrero = true;
            e.mostrador = true;
            e.tienda = 2;
        }
        _ => {}
    }
    let _ = ctx;
}

/// Lado de las piezas de la casa standard: sobre la calle lateral en la esquina; si no, al
/// azar con la semilla (el patio queda del otro lado).
fn ala_izq(es: &Estilo) -> bool {
    es.esquina > 0 || (es.seed >> 13) & 1 == 0
}

/// Casa vieja dividida en piezas de alquiler: la fachada no cambia; adentro, una fila de
/// piezas sobre el patio (como el inquilinato) y la ropa tendida. Revoque viejo y descuidado.
pub fn conventillar(e: &mut Estilo) {
    match e.tipo {
        Tipo::CasaAzotea | Tipo::CasaTeja => {
            e.tipo = Tipo::Cuartos;
            e.renta = true;
        }
        Tipo::CasaAltos => e.renta = true,
        _ => {}
    }
    if e.e3 != tipo3::RENTA {
        e.e3 = 0;
    }
    e.blanqueo = e.blanqueo.max(7);
    e.caido = e.caido.max(0.3);
    e.mirador = false;
    e.altillo = false;
    e.farol = false;
    e.fondo_teja = false;
    e.deposito = false;
    e.ampliaciones = 0;
}

/// Nombre para la ficha de los tipos de la época 3.
pub fn nombre3(es: &Estilo) -> Option<String> {
    let comercio = es.uso == Uso::Comercio;
    Some(match es.e3 {
        tipo3::STANDARD => "casa standard (casa chorizo) con zaguán y patio lateral".into(),
        tipo3::ALTOS => "casa de altos italianizante".into(),
        tipo3::RENTA => {
            let r = match es.remate {
                1 => " con cúpula en la esquina",
                2 => " con mansarda",
                _ => "",
            };
            format!("edificio de renta de {} plantas{r}{}", es.pisos, if comercio { ", con comercio abajo" } else { "" })
        }
        tipo3::ESQUINA => "almacén y bar de esquina".into(),
        tipo3::INQUILINATO => "casa de inquilinato".into(),
        _ => return None,
    })
}

/// ¿Lo dibuja este módulo (y no los constructores de `casas`)?
pub fn propio(es: &Estilo) -> bool {
    matches!(es.e3, tipo3::STANDARD | tipo3::RENTA) && es.conventillo == 0
}

/// Dibuja una casa de un tipo propio de la época 3.
pub(super) fn construir3(t: &TerrainGen, e: &mut Edits, f: &Frame, es: &Estilo) -> Option<Frente> {
    match es.e3 {
        tipo3::RENTA => edificio_renta(t, e, f, es),
        _ => casa_standard(t, e, f, es),
    }
}

/// Casa standard o "casa chorizo" (tipología dominante del ensanche de fines del XIX [F]):
/// crujía al frente con la fachada italianizante (puerta al costado con zaguán, dos ventanas
/// altas, pilastras, cornisa y pretil balaustrado), y hacia el fondo las piezas en hilera sobre
/// un patio lateral largo con galería de columnas de hierro fundido, baldosas y macetas; al
/// fondo, la cocina, el baño y el fondo de tierra con una higuera.
fn casa_standard(t: &TerrainGen, e: &mut Edits, f0: &Frame, es: &Estilo) -> Option<Frente> {
    let f = &casas::marco_esquina(f0, es);
    let (x0, x1) = (0, f.width);
    let (fd, h) = (es.fondo, es.alto);
    f.foundation(t, e, x0, x1, 0, fd, ZOCALO);
    casas::caja(f, e, x0, x1, 0, h, 0, fd, es.muro);
    let vanos = casas::fachada_pb(t, f, e, es, x0, x1, h);
    casas::losa(f, e, x0, x1, 0, fd, h - 1);
    casas::cornisa_estilo(f, e, es, x0, x1, h - 1);
    casas::pretil(f, e, es, x0, x1, 0, fd, h);
    // Ménsulas bajo la cornisa, sobre cada vano [E].
    for &(a, b, _) in &vanos {
        f.fill(e, a, a + 1, h - 2, h - 1, -1, 0, es.moldura);
        f.fill(e, b - 1, b, h - 2, h - 1, -1, 0, es.moldura);
    }
    if es.esquina > 0 {
        casas::esquina(f, e, es, fd, h);
    }
    if es.hierro && es.clase >= 2 {
        let u = vanos.iter().find(|v| v.2 == Hueco::Puerta).map_or(2, |v| v.0 - 1);
        casas::farol_fachada(f, e, es, u.max(1));
    }
    // Las piezas en hilera sobre el patio lateral.
    let izq = ala_izq(es);
    let aw = 9;
    let (a0, a1) = if izq { (x0, x0 + aw) } else { (x1 - aw, x1) };
    let w1 = f.depth - 16;
    let hp = h - 2;
    f.foundation(t, e, a0, a1, fd, w1, ZOCALO);
    ring(f, e, a0, a1, 0, hp, fd, w1, es.muro);
    casas::losa(f, e, a0, a1, fd, w1, hp - 1);
    ring(f, e, a0, a1, hp, hp + 1, fd, w1, es.muro);
    // Medianera del otro lado del patio.
    let mu = if izq { x1 - 1 } else { x0 };
    f.fill(e, mu, mu + 1, 0, 6, fd, w1, es.muro);
    let (pu0, pu1) = if izq { (a1, x1 - 1) } else { (x0 + 1, a0) };
    // Baldosas: mármol en damero en las casas buenas, ladrillo en las modestas.
    for w in fd..w1 {
        for u in pu0..pu1 {
            let m = if es.clase >= 1 { if (u + w) % 2 == 0 { MARMOL } else { MARMOL_GRIS } } else { BRICK };
            f.raw(e, u, -1, w, m);
        }
    }
    // Una puerta y una ventana por pieza, de cara al patio.
    let cara = if izq { a1 - 1 } else { a0 };
    let mut w = fd + 2;
    while w + 6 < w1 {
        f.clear(e, cara, cara + 1, 0, 6, w, w + 2);
        f.raw_fill(e, cara, cara + 1, 0, 6, w, w + 2, es.carpinteria);
        f.raw_fill(e, cara, cara + 1, 5, 6, w, w + 2, if es.hierro { HIERRO_FUNDIDO } else { WOOD_DARK });
        f.clear(e, cara, cara + 1, 2, 5, w + 4, w + 5);
        f.raw_fill(e, cara, cara + 1, 2, 5, w + 4, w + 5, WINDOW);
        w += 8;
    }
    // Galería: alero de la losa sobre columnas de hierro fundido.
    let (g0, g1) = if izq { (a1, a1 + 3) } else { (a0 - 3, a0) };
    f.fill(e, g0, g1, hp - 1, hp, fd, w1, AZOTEA);
    let cu = if izq { g1 - 1 } else { g0 };
    let mut w = fd + 4;
    while w < w1 {
        f.raw_fill(e, cu, cu + 1, 0, hp - 1, w, w + 1, if es.hierro { HIERRO_FUNDIDO } else { WOOD_DARK });
        w += 6;
    }
    // Macetas en el patio [E].
    let mut w = fd + 3;
    let mut k = 0u32;
    while w < w1 - 2 {
        let u = if izq { pu1 - 1 } else { pu0 };
        f.detalle(e, crate::detalle::Id::Maceta, u as f32 + 0.5, 0.0, w as f32 + 0.5, 0.0, es.seed as u32 ^ k);
        w += 5;
        k += 1;
    }
    // Fondo: cocina y baño (con el agua corriente, el baño entra a la casa), y el fondo de
    // tierra con su higuera.
    let wc = w1;
    f.foundation(t, e, x0, x1, wc, wc + 7, ZOCALO);
    ring(f, e, x0 + 1, x1 - 1, 0, 6, wc, wc + 7, es.muro);
    casas::losa(f, e, x0 + 1, x1 - 1, wc, wc + 7, 5);
    f.surface(e, x0 + 1, x1 - 1, wc + 7, f.depth - 1, GARDEN);
    let (hu, hw) = (if izq { x1 - 6 } else { x0 + 3 }, f.depth - 5);
    f.raw_fill(e, hu, hu + 1, 0, 4, hw, hw + 1, TRUNK);
    f.raw_fill(e, hu - 2, hu + 3, 4, 7, hw - 2, hw + 3, LEAF_DARK);
    Some(Frente { g: *f, u0: x0, u1: x1, h })
}

/// Edificio de renta (italianizante o ecléctico, de tres o cuatro plantas): planta baja con
/// zaguán (o comercio), balcón corrido en el primer piso y balcones por vano arriba, pilastras,
/// cornisa fuerte y balaustrada; en la esquina, cúpula de pizarra o cobre, o mansarda con
/// buhardillas. Detrás, un ala con galerías sobre el patio de aire y luz.
fn edificio_renta(t: &TerrainGen, e: &mut Edits, f0: &Frame, es: &Estilo) -> Option<Frente> {
    let g = casas::marco_esquina(f0, es);
    let (x0, x1) = (0, g.width);
    let fd = es.fondo;
    let h1 = es.alto;
    let pisos = es.pisos.max(3) as i32;
    let h = h1 + es.alto_pa * (pisos - 1);
    g.foundation(t, e, x0, x1, 0, fd, ZOCALO);
    casas::caja(&g, e, x0, x1, 0, h, 0, fd, es.muro);
    for p in 1..pisos {
        let y = h1 + es.alto_pa * (p - 1);
        g.fill(e, x0 + 1, x1 - 1, y, y + 1, 2, 4, AZOTEA);
    }
    let vanos = casas::fachada_pb(t, &g, e, es, x0, x1, h1);
    let m = es.moldura;
    for p in 1..pisos {
        let y = h1 + es.alto_pa * (p - 1);
        g.fill(e, x0, x1, y, y + 1, -1, 0, m);
        for &(a, b, _) in &vanos {
            let c = (a + b) / 2;
            let (va, vb) = (c - 1, c + 1);
            let alto = es.alto_pa - 2;
            let abierta = hash(g.at(va, y, 0), es.seed ^ p as u64) < 0.5;
            opening(&g, e, va, vb, y + 1, y + 1 + alto, 2, if abierta { WINDOW } else { es.carpinteria });
            if abierta {
                g.raw_fill(e, va, va + 1, y + 1, y + alto, 1, 2, es.carpinteria);
            }
            // Guardapolvo: con frontón en el primer piso (el "piso noble").
            g.fill(e, va - 1, vb + 1, y + 1 + alto, y + 2 + alto, -1, 0, m);
            if p == 1 {
                g.fill(e, va, vb, y + 2 + alto, y + 3 + alto, -1, 0, m);
            }
            if p > 1 {
                casas::balcon(&g, e, es, va - 1, vb + 1, y);
            }
        }
        if p == 1 {
            casas::balcon(&g, e, es, x0 + 1, x1 - 1, y);
        }
    }
    casas::pilastras(&g, e, es, x0, x1, &vanos, h1 + 1, h - 1);
    casas::losa(&g, e, x0, x1, 0, fd, h - 1);
    casas::cornisa_estilo(&g, e, es, x0, x1, h - 1);
    g.fill(e, x0, x1, h - 3, h - 2, -1, 0, m); // friso
    casas::pretil(&g, e, es, x0, x1, 0, fd, h);
    if es.esquina > 0 {
        casas::esquina(&g, e, es, fd, h1);
        let side = g.left_side();
        for p in 1..pisos {
            let y = h1 + es.alto_pa * (p - 1);
            side.fill(e, 0, fd, y, y + 1, -1, 0, m);
            let mut w = 6;
            while w + 3 < fd {
                opening(&side, e, w, w + 2, y + 1, y + es.alto_pa - 1, 1, WINDOW);
                side.fill(e, w - 1, w + 3, y + es.alto_pa - 1, y + es.alto_pa, -1, 0, m);
                w += 5;
            }
        }
        casas::cornisa_estilo(&side, e, es, 0, fd, h - 1);
    }
    match es.remate {
        1 => cupula(&g, e, es, h),
        2 => mansarda(&g, e, es, x0, x1, fd, h),
        _ => {}
    }
    // Ala con galerías sobre el patio de aire y luz.
    let izq = es.esquina > 0 || (es.seed >> 9) & 1 == 0;
    let aw = 9;
    let w1 = (fd + 22).min(g.depth - 16);
    let (a0, a1) = if izq { (x0, x0 + aw) } else { (x1 - aw, x1) };
    let ha = h - es.alto_pa;
    g.foundation(t, e, a0, a1, fd, w1, ZOCALO);
    ring(&g, e, a0, a1, 0, ha, fd, w1, es.muro);
    casas::losa(&g, e, a0, a1, fd, w1, ha - 1);
    ring(&g, e, a0, a1, ha, ha + 1, fd, w1, es.muro);
    let gu = if izq { a1 } else { a0 - 2 };
    let bu = if izq { gu + 1 } else { gu };
    for p in 1..pisos - 1 {
        let y = h1 + es.alto_pa * (p - 1);
        g.raw_fill(e, gu, gu + 2, y, y + 1, fd, w1, AZOTEA);
        g.raw_fill(e, bu, bu + 1, y + 1, y + 3, fd, w1, HIERRO_FUNDIDO);
    }
    let mut w = fd + 2;
    while w < w1 {
        g.raw_fill(e, bu, bu + 1, 0, ha - 1, w, w + 1, HIERRO_FUNDIDO);
        w += 6;
    }
    let (pu0, pu1) = if izq { (a1, x1 - 1) } else { (x0 + 1, a0) };
    g.surface(e, pu0, pu1, fd, w1, PLAZA);
    let mu = if izq { x1 - 1 } else { x0 };
    g.fill(e, mu, mu + 1, 0, 8, fd, w1, es.muro);
    // Fondo de servicio.
    g.foundation(t, e, x0, x1, w1, w1 + 8, ZOCALO);
    ring(&g, e, x0 + 1, x1 - 1, 0, 7, w1, w1 + 8, es.muro);
    casas::losa(&g, e, x0 + 1, x1 - 1, w1, w1 + 8, 6);
    g.surface(e, x0 + 1, x1 - 1, w1 + 8, g.depth - 1, DIRT);
    Some(Frente { g, u0: x0, u1: x1, h })
}

/// Cúpula de la esquina: tambor octogonal con ventanas, la cúpula de pizarra o cobre verde y
/// el pináculo (el remate de los edificios de renta de esquina) [E].
fn cupula(g: &Frame, e: &mut Edits, es: &Estilo, h: i32) {
    let (cu, cw) = (4.0, 4.0);
    cylinder(g, e, cu, cw, 4.2, h, h + 6, es.muro);
    for (u, w) in [(4, 0), (4, 7), (0, 4), (7, 4)] {
        g.raw_fill(e, u, u + 1, h + 2, h + 5, w, w + 1, WINDOW);
    }
    cylinder(g, e, cu, cw, 4.8, h + 6, h + 7, es.moldura);
    dome(g, e, cu, cw, 4.2, h + 7, COBRE_VERDE);
    g.raw_fill(e, 4, 5, h + 12, h + 15, 4, 5, HIERRO_FUNDIDO);
}

/// Mansarda: techo quebrado de pizarra sobre la cornisa del frente, con buhardillas [E].
#[allow(clippy::too_many_arguments)]
fn mansarda(g: &Frame, e: &mut Edits, es: &Estilo, x0: i32, x1: i32, fd: i32, h: i32) {
    for k in 0..5 {
        let y = h + 1 + k;
        let d = 1 + k / 2;
        g.fill(e, x0 + d - 1, x1 - d + 1, y, y + 1, d - 1, fd - d + 1, COBRE_VERDE);
    }
    g.fill(e, x0 + 3, x1 - 3, h + 6, h + 7, 3, fd - 3, GRIS_PIEDRA_SUCIA);
    let mut u = x0 + 3;
    while u + 3 < x1 - 2 {
        g.fill(e, u, u + 3, h + 2, h + 5, 0, 2, es.moldura);
        g.raw_fill(e, u + 1, u + 2, h + 2, h + 4, -1, 0, WINDOW);
        g.fill(e, u, u + 3, h + 5, h + 6, 0, 2, COBRE_VERDE);
        u += 7;
    }
}

/// Detalles de la época 3 sobre una casa ya dibujada: toldo y letrero del almacén y bar,
/// ropa tendida en el patio del conventillo.
pub fn detalles3(e: &mut Edits, f0: &Frame, es: &Estilo) {
    if !Frame::con_detalles(e) {
        return;
    }
    if es.e3 == tipo3::ESQUINA || es.e3 == tipo3::RENTA && es.uso == Uso::Comercio {
        let f = casas::marco_esquina(f0, es);
        let hp = (es.alto - 2).clamp(5, 8) as f32;
        f.detalle(e, crate::detalle::Id::Toldo, 13.0, hp - 0.4, 0.0, 0.0, es.seed as u32 ^ 0x701D);
        f.detalle(e, crate::detalle::Id::Letrero, 6.0, hp + 0.3, -0.3, 0.0, es.seed as u32 ^ 0x1E7);
    }
    if es.conventillo > 0 || es.e3 == tipo3::INQUILINATO {
        for (k, w) in [es.fondo + 14, es.fondo + 26, es.fondo + 38].into_iter().enumerate() {
            if w < f0.depth - 12 {
                f0.detalle(e, crate::detalle::Id::RopaTendida, 12.0, 0.0, w as f32, std::f32::consts::FRAC_PI_2, es.seed as u32 ^ k as u32);
            }
        }
    }
}

// --- Industria ------------------------------------------------------------------------------

/// Fábrica de ladrillo (textil, fideera, curtiembre a vapor, cervecería…): galpón con techo de
/// diente de sierra (las caras de vidrio al sur, para la luz pareja), frente con pilastras de
/// ladrillo, portón y letrero, y la chimenea de ladrillo alta y troncocónica [E]. Para un
/// solar industrial (24 × 76).
pub fn fabrica(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (u0, u1, w0, w1) = (1, 23, 1, 58);
    let h = 12;
    f.foundation(t, e, u0, u1, w0, w1, STONE);
    ring(f, e, u0, u1, 0, h, w0, w1, BRICK);
    zocalo(f, e, u0, u1, w0, w1, 2, BRICK_DARK);
    // Diente de sierra: cada diente de 8 vóxeles, la cara vertical vidriada.
    let mut w = w0;
    while w < w1 {
        for k in 0..8 {
            let y = h + (k * 5) / 8;
            if w + k < w1 {
                f.fill(e, u0, u1, y, y + 1, w + k, w + k + 1, GRIS_PIEDRA_SUCIA);
            }
        }
        if w + 8 <= w1 {
            f.raw_fill(e, u0 + 1, u1 - 1, h, h + 5, w + 7, w + 8, WINDOW);
        }
        w += 8;
    }
    // Frente: pilastras, ventanas en arco, portón y letrero.
    let mut u = u0;
    while u < u1 {
        f.fill(e, u, u + 1, 0, h + 2, w0 - 1, w0, BRICK_DARK);
        u += 5;
    }
    f.fill(e, u0, u1, h, h + 1, w0 - 1, w0, BRICK_DARK);
    for &u in &[u0 + 2, u1 - 4] {
        opening(f, e, u, u + 2, 3, 9, 1, WINDOW);
        f.fill(e, u, u + 2, 9, 10, w0 - 1, w0, BRICK_DARK);
    }
    opening(f, e, 9, 15, 0, 8, 1, WOOD_DARK);
    f.raw_fill(e, 8, 16, 10, 11, w0 - 1, w0, WOOD_DARK);
    // Chimenea: base cuadrada y fuste que se afina, con el remate.
    let (cu, cw) = (12.0, 66.0);
    f.foundation(t, e, 8, 16, 62, 70, STONE);
    f.fill(e, 9, 15, 0, 6, 63, 69, BRICK_DARK);
    let mut y = 6;
    let mut r = 2.8;
    while y < 64 {
        cylinder(f, e, cu, cw, r, y, y + 8, BRICK);
        r = (r - 0.25).max(1.6);
        y += 8;
    }
    cylinder(f, e, cu, cw, 2.4, 64, 66, BRICK_DARK);
    // Patio de maniobras con fardos y barriles.
    f.surface(e, 0, f.width, w1, f.depth, DIRT_ROAD);
    for k in 0..3 {
        f.raw_fill(e, 2 + k * 3, 4 + k * 3, 0, 2, w1 + 3, w1 + 5, if k % 2 == 0 { SACK } else { WOOD });
    }
}

/// Galpón de depósito de chapa sobre estructura de hierro, con portones corredizos (puerto y
/// ferrocarril) [E]. Para un solar industrial (24 × 76).
pub fn galpon_chapa(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (u0, u1, w0, w1) = (1, 23, 1, 70);
    galpon_de_chapa(t, f, e, u0, u1, w0, w1, 11);
    for w in [w0, w1 - 1] {
        opening(f, e, 7, 17, 0, 8, 0, WOOD_DARK);
        let _ = w;
    }
}

/// Galpón de chapa genérico (cubierta curva de chapa acanalada sobre cerchas) [E].
#[allow(clippy::too_many_arguments)]
fn galpon_de_chapa(t: &TerrainGen, f: &Frame, e: &mut Edits, u0: i32, u1: i32, w0: i32, w1: i32, h: i32) {
    f.foundation(t, e, u0, u1, w0, w1, STONE);
    ring(f, e, u0, u1, 0, h, w0, w1, GRIS_SUCIA);
    zocalo(f, e, u0, u1, w0, w1, 2, BRICK);
    // Bóveda rebajada: la cubierta sube hacia el medio.
    let ancho = (u1 - u0) as f32;
    for u in u0..u1 {
        let x = (u - u0) as f32 + 0.5 - ancho * 0.5;
        let dy = ((ancho * 0.5).powi(2) - x * x).max(0.0).sqrt() * 0.45;
        f.fill(e, u, u + 1, h + dy as i32, h + dy as i32 + 1, w0, w1, GRIS_PIEDRA_SUCIA);
        f.fill(e, u, u + 1, h, h + dy as i32, w0, w0 + 1, GRIS_SUCIA);
        f.fill(e, u, u + 1, h, h + dy as i32, w1 - 1, w1, GRIS_SUCIA);
    }
}

// --- Servicios ------------------------------------------------------------------------------

/// Rieles en el suelo de un marco: dos hilos de hierro a lo largo de `w` en `u` y `u + 3`
/// (trocha de 1,435 m, que en vóxeles de 0,5 m queda en tres) sobre balasto.
fn via(f: &Frame, e: &mut Edits, u: i32, w0: i32, w1: i32) {
    for w in w0..w1 {
        for du in -1..5 {
            f.raw(e, u + du, -1, w, if du == 0 || du == 3 { HIERRO_FUNDIDO } else if w % 2 == 0 { WOOD_DARK } else { STONE_DARK });
        }
    }
}

/// Estación de ferrocarril (la Central de Montevideo, de Luis Andreoni, se inauguró el 23 de
/// junio de 1897 [F]; la primera, de 1869–1871, era mucho más modesta): edificio de pasajeros
/// sobre la calle, de dos plantas, con el cuerpo central más alto, el reloj y el frontón, y
/// detrás el galpón de andenes de hierro y vidrio sobre tres vías. Marco: 100 × 72.
pub fn estacion(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, PLAZA);
    // Edificio de pasajeros.
    let (u0, u1, w0, w1) = (6, 94, 4, 22);
    let h = 18;
    f.foundation(t, e, u0, u1, w0, w1, STONE);
    ring(f, e, u0, u1, 0, h, w0, w1, AMARILLO_PALIDO);
    zocalo(f, e, u0, u1, w0, w1, 3, GRIS_PIEDRA);
    f.fill(e, u0 + 1, u1 - 1, h - 1, h, w0 + 1, w1 - 1, AZOTEA);
    f.fill(e, u0 + 1, u1 - 1, 9, 10, w0 + 1, w1 - 1, AZOTEA);
    let fr = f.sub(0, w0, f.width, w1 - w0, 3);
    fr.fill(e, u0 - 1, u1 + 1, h - 1, h, -1, 0, BLANCO_PIEDRA);
    fr.fill(e, u0, u1, 9, 10, -1, 0, BLANCO_PIEDRA);
    ring(f, e, u0, u1, h, h + 2, w0, w1, AMARILLO_PALIDO);
    let mut u = u0 + 3;
    while u + 2 < u1 - 2 {
        if !(42..56).contains(&u) {
            opening(&fr, e, u, u + 2, 2, 8, 1, WINDOW);
            opening(&fr, e, u, u + 2, 11, 16, 1, WINDOW);
            fr.fill(e, u - 1, u + 3, 16, 17, -1, 0, BLANCO_PIEDRA);
        }
        u += 5;
    }
    // Cuerpo central: más alto, tres arcos de entrada, frontón y reloj.
    let (c0, c1) = (40, 60);
    ring(f, e, c0, c1, 0, h + 8, w0 - 1, w1, AMARILLO_PALIDO);
    for k in 0..3 {
        let a = c0 + 3 + k * 5;
        arch(f, e, a, a + 4, 0, 9, w0 - 1, w0 + 1);
        f.raw_fill(e, a, a + 4, 0, 9, w0 + 1, w0 + 2, WOOD_DARK);
    }
    let fc = f.sub(0, w0 - 1, f.width, 4, 4);
    for u in [c0, c0 + 1, c1 - 2, c1 - 1] {
        fc.fill(e, u, u + 1, 0, h + 8, -1, 0, BLANCO_PIEDRA);
    }
    let mut k = 0;
    while c0 + k < c1 - k && k < 6 {
        fc.fill(e, c0 + k, c1 - k, h + 8 + k, h + 9 + k, 0, 1, BLANCO_PIEDRA);
        k += 1;
    }
    cylinder(&fc, e, 50.0, -0.5, 2.6, h + 2, h + 3, BLANCO_PIEDRA);
    fc.raw_fill(e, 48, 52, h + 1, h + 5, -1, 0, BLANCO_PIEDRA);
    fc.raw_fill(e, 49, 51, h + 2, h + 4, -2, -1, CAL); // esfera del reloj
    fc.raw(e, 50, h + 3, -3, HIERRO_FUNDIDO);
    // Galpón de andenes: arcos de hierro y la cubierta de vidrio y chapa.
    let (gw0, gw1) = (w1, f.depth - 2);
    let (g0, g1) = (14, 86);
    let ancho = (g1 - g0) as f32;
    let mut w = gw0;
    while w < gw1 {
        for u in g0..g1 {
            let x = (u - g0) as f32 + 0.5 - ancho * 0.5;
            let dy = ((ancho * 0.5).powi(2) - x * x).max(0.0).sqrt() * 0.42;
            let y = 6 + dy as i32;
            let costilla = (w - gw0) % 8 == 0;
            let m = if costilla { HIERRO_FUNDIDO } else if (u - g0) % 6 < 3 { WINDOW } else { GRIS_PIEDRA_SUCIA };
            f.raw(e, u, y, w, m);
        }
        if (w - gw0) % 8 == 0 {
            for u in [g0, g1 - 1] {
                f.raw_fill(e, u, u + 1, 0, 7, w, w + 1, HIERRO_FUNDIDO);
            }
        }
        w += 1;
    }
    // Andenes y vías.
    for (a0, a1) in [(g0 + 1, g0 + 7), (46, 54), (g1 - 7, g1 - 1)] {
        f.fill(e, a0, a1, 0, 1, gw0, gw1, GRIS_PIEDRA);
    }
    for u in [g0 + 8, g0 + 22, 58, 72] {
        via(f, e, u, gw0, f.depth);
    }
}

/// Usina eléctrica (en Montevideo, la luz incandescente de la usina del Yerbal empieza en 1886
/// [F]; después la del Arroyo Seco): sala de máquinas de ladrillo con ventanales en arco y
/// techo a dos aguas, sala de calderas con dos chimeneas altas, la carbonera y el patio de
/// transformadores con postes. Marco: 72 × 60.
pub fn usina_electrica(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, DIRT_ROAD);
    let (u0, u1, w0, w1) = (4, 40, 4, 34);
    let h = 16;
    f.foundation(t, e, u0, u1, w0, w1, STONE);
    ring(f, e, u0, u1, 0, h, w0, w1, BRICK);
    zocalo(f, e, u0, u1, w0, w1, 2, BRICK_DARK);
    gable(f, e, u0, u1, w0, w1, h, 2, 1, GRIS_PIEDRA_SUCIA, BRICK, true);
    let fr = f.sub(0, w0, f.width, w1 - w0, 5);
    let mut u = u0 + 3;
    while u + 3 < u1 - 1 {
        opening(&fr, e, u, u + 3, 3, 12, 1, WINDOW);
        fr.fill(e, u - 1, u + 4, 12, 13, -1, 0, BRICK_DARK);
        fr.raw(e, u + 1, 13, -1, BRICK_DARK);
        u += 6;
    }
    // Letrero de la usina sobre la puerta.
    fr.fill(e, u0 + 10, u1 - 10, 14, 15, -1, 0, BLANCO_PIEDRA);
    // Sala de calderas y chimeneas.
    galpon_de_chapa(t, f, e, 42, 66, 8, 34, 12);
    for cu in [48.0, 60.0] {
        f.foundation(t, e, cu as i32 - 3, cu as i32 + 3, 36, 42, STONE);
        let mut y = 0;
        let mut r = 2.6;
        while y < 60 {
            cylinder(f, e, cu, 39.0, r, y, y + 10, BRICK);
            r = (r - 0.2).max(1.6);
            y += 10;
        }
        cylinder(f, e, cu, 39.0, 2.2, 60, 62, BRICK_DARK);
    }
    // Carbonera.
    for (u, w, rr) in [(14.0, 46.0, 4.5), (24.0, 48.0, 3.5)] {
        dome(f, e, u, w, rr, 0, STONE_DARK);
    }
    // Patio de transformadores: postes con aisladores.
    for (u, w) in [(44, 46), (52, 46), (60, 46), (44, 54), (52, 54), (60, 54)] {
        f.raw_fill(e, u, u + 1, 0, 12, w, w + 1, WOOD_DARK);
        f.raw_fill(e, u - 1, u + 2, 11, 12, w, w + 1, HIERRO_FUNDIDO);
    }
}

/// Cocheras de tranvía (la primera línea de tranvías a caballo de Montevideo es de 1868 y el
/// eléctrico, de 1906 [F]): galpón largo con cuatro portones en arco hacia la calle, las vías que
/// salen de cada portón, las caballerizas al fondo y la oficina. Marco: 80 × 60.
pub fn cocheras_tranvia(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, COBBLE);
    let (u0, u1, w0, w1) = (4, 60, 6, 44);
    let h = 12;
    f.foundation(t, e, u0, u1, w0, w1, STONE);
    ring(f, e, u0, u1, 0, h, w0, w1, BRICK);
    gable(f, e, u0, u1, w0, w1, h, 3, 1, GRIS_PIEDRA_SUCIA, BRICK, false);
    let fr = f.sub(0, w0, f.width, w1 - w0, 6);
    for k in 0..4 {
        let a = u0 + 4 + k * 13;
        arch(f, e, a, a + 7, 0, 9, w0, w0 + 1);
        fr.fill(e, a - 1, a + 8, 9, 10, -1, 0, BRICK_DARK);
        via(f, e, a + 2, 0, w1);
    }
    // Caballerizas (para los tranvías de caballos), al fondo.
    ring(f, e, u0, u1, 0, 7, w1 + 2, w1 + 12, BRICK);
    gable(f, e, u0, u1, w1 + 2, w1 + 12, 7, 2, 1, TILE, BRICK, true);
    for k in 0..8 {
        let u = u0 + 3 + k * 7;
        opening(&f.sub(0, w1 + 2, f.width, 10, 7), e, u, u + 2, 0, 4, 0, WOOD_DARK);
    }
    // Oficina con fachada de la época.
    let o = f.sub(62, 6, 16, 14, 9);
    o.foundation(t, e, 0, 16, 0, 14, STONE);
    ring(&o, e, 0, 16, 0, 10, 0, 14, ROSA_VIEJO);
    o.fill(e, 1, 15, 9, 10, 1, 13, AZOTEA);
    o.fill(e, 0, 16, 9, 10, -1, 0, BLANCO_PIEDRA);
    ring(&o, e, 0, 16, 10, 12, 0, 14, ROSA_VIEJO);
    opening(&o, e, 6, 10, 0, 7, 1, SHUTTER);
}

/// Hotel de inmigrantes (en Montevideo abrió en 1908 [F]): pabellón de dos plantas con galería
/// de hierro sobre un patio, comedor, el letrero sobre la entrada y la bandera. Marco: 72 × 48.
pub fn hotel_inmigrantes(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, PLAZA);
    let (u0, u1, w0, w1) = (4, 68, 4, 16);
    let h = 16;
    f.foundation(t, e, u0, u1, w0, f.depth - 3, STONE);
    ring(f, e, u0, u1, 0, h, w0, w1, CAL_HUESO);
    zocalo(f, e, u0, u1, w0, w1, 2, GRIS_PIEDRA);
    f.fill(e, u0 + 1, u1 - 1, h - 1, h, w0 + 1, w1 - 1, AZOTEA);
    let fr = f.sub(0, w0, f.width, w1 - w0, 3);
    fr.fill(e, u0 - 1, u1 + 1, h - 1, h, -1, 0, BLANCO_PIEDRA);
    fr.fill(e, u0, u1, 8, 9, -1, 0, BLANCO_PIEDRA);
    ring(f, e, u0, u1, h, h + 2, w0, w1, CAL_HUESO);
    let mut u = u0 + 3;
    while u + 2 < u1 - 2 {
        if !(30..38).contains(&u) {
            opening(&fr, e, u, u + 2, 2, 7, 1, WINDOW);
            opening(&fr, e, u, u + 2, 10, 14, 1, WINDOW);
        }
        u += 4;
    }
    opening(&fr, e, 31, 37, 0, 8, 2, WOOD_DARK);
    fr.fill(e, 29, 39, 9, 11, -1, 0, BLANCO_PIEDRA);
    fr.raw_fill(e, 30, 38, 9, 10, -2, -1, HIERRO_FUNDIDO); // letras del letrero
    // Mástil y bandera.
    f.raw_fill(e, 34, 35, h + 2, h + 14, 8, 9, WOOD_DARK);
    f.raw_fill(e, 35, 39, h + 11, h + 14, 8, 9, CAL);
    // Alas de dormitorios alrededor del patio, con galería de hierro.
    for (a0, a1) in [(u0, u0 + 12), (u1 - 12, u1)] {
        ring(f, e, a0, a1, 0, h, w1, f.depth - 3, CAL_HUESO);
        f.fill(e, a0 + 1, a1 - 1, h - 1, h, w1, f.depth - 4, AZOTEA);
        let gu = if a0 == u0 { a1 } else { a0 - 2 };
        f.raw_fill(e, gu, gu + 2, 8, 9, w1, f.depth - 3, AZOTEA);
        let mut w = w1 + 2;
        while w < f.depth - 3 {
            f.raw_fill(e, if a0 == u0 { gu + 1 } else { gu }, if a0 == u0 { gu + 2 } else { gu + 1 }, 0, 11, w, w + 1, HIERRO_FUNDIDO);
            w += 5;
        }
    }
    // Comedor al fondo.
    ring(f, e, u0 + 12, u1 - 12, 0, 9, f.depth - 14, f.depth - 3, CAL_HUESO);
    gable(f, e, u0 + 12, u1 - 12, f.depth - 14, f.depth - 3, 9, 2, 1, TILE, CAL_HUESO, true);
    // Bancos y faroles en el patio.
    for u in [24, 44] {
        f.raw_fill(e, u, u + 4, 0, 1, 26, 27, WOOD_DARK);
        super::epoca2::farol_en(f, e, u + 2, 22);
    }
}

/// Puerto moderno (inaugurado el 25 de agosto de 1909 [F]): el muelle de piedra con bitas y
/// las vías del ferrocarril del puerto, tres grúas de pórtico de hierro sobre rieles, y dos
/// galpones de depósito de chapa. Marco: 120 × 80 (el muelle es la franja del fondo, hacia el
/// agua).
pub fn puerto_moderno(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, COBBLE);
    // Galpones de depósito.
    galpon_de_chapa(t, f, e, 4, 54, 6, 40, 12);
    galpon_de_chapa(t, f, e, 66, 116, 6, 40, 12);
    for a in [14, 30, 76, 92] {
        opening(f, e, a, a + 8, 0, 9, 0, WOOD_DARK);
    }
    // Muelle de piedra: borde de sillería con bitas.
    let (mw0, mw1) = (48, f.depth);
    f.foundation(t, e, 0, f.width, mw0, mw1, STONE);
    f.fill(e, 0, f.width, -1, 0, mw0, mw1, STONE);
    f.fill(e, 0, f.width, -1, 0, mw1 - 2, mw1, STONE_DARK);
    let mut u = 4;
    while u < f.width - 2 {
        f.raw_fill(e, u, u + 1, 0, 2, mw1 - 2, mw1 - 1, HIERRO_FUNDIDO);
        u += 12;
    }
    // Vías del puerto y de las grúas.
    via(f, e, 2, mw0 - 6, mw0 - 2);
    for k in 0..30 {
        let u = k * 4;
        if u + 4 < f.width {
            f.raw(e, u, -1, mw0 + 3, HIERRO_FUNDIDO);
            f.raw(e, u + 2, -1, mw0 + 3, HIERRO_FUNDIDO);
        }
    }
    f.raw_fill(e, 0, f.width, -1, 0, mw0 + 3, mw0 + 4, HIERRO_FUNDIDO);
    f.raw_fill(e, 0, f.width, -1, 0, mw0 + 13, mw0 + 14, HIERRO_FUNDIDO);
    // Grúas de pórtico: patas sobre los rieles, la cabina y el brazo hacia el agua.
    for cu in [20, 58, 96] {
        for (du, w) in [(0, mw0 + 3), (6, mw0 + 3), (0, mw0 + 13), (6, mw0 + 13)] {
            f.raw_fill(e, cu + du, cu + du + 1, 0, 14, w, w + 1, HIERRO_FUNDIDO);
        }
        f.raw_fill(e, cu, cu + 7, 14, 15, mw0 + 3, mw0 + 14, HIERRO_FUNDIDO);
        f.raw_fill(e, cu + 1, cu + 6, 15, 20, mw0 + 6, mw0 + 11, ROSA_VIEJO);
        f.raw_fill(e, cu + 1, cu + 6, 20, 21, mw0 + 6, mw0 + 11, GRIS_PIEDRA_SUCIA);
        f.raw_fill(e, cu + 2, cu + 5, 17, 19, mw0 + 11, mw0 + 12, WINDOW);
        // Brazo inclinado hacia el agua, con el cable y el gancho.
        for k in 0..22 {
            let y = 20 + k / 3;
            f.raw(e, cu + 3, y, mw0 + 11 + k, HIERRO_FUNDIDO);
        }
        f.raw_fill(e, cu + 3, cu + 4, 18, 27, mw0 + 32, mw0 + 33, HIERRO_FUNDIDO);
        f.raw(e, cu + 3, 17, mw0 + 32, IRON);
    }
    // Fardos y bolsas sobre el muelle.
    for k in 0..6 {
        let u = 8 + k * 17;
        f.raw_fill(e, u, u + 3, 0, 2, mw0 + 6, mw0 + 8, if k % 2 == 0 { SACK } else { HIDE });
    }
}

/// Farol eléctrico de la calle: columna de hierro de 4,5 m con el brazo y el globo (el vidrio
/// brilla de noche, con la luz de los faroles; ver `city::epocas::tranvias`).
pub fn farol_electrico(e: &mut Edits, v: glam::IVec2, base: i32) {
    use glam::IVec3;
    e.dfill(IVec3::new(v.x, base, v.y), IVec3::new(v.x + 1, base + 9, v.y + 1), HIERRO_FUNDIDO);
    e.dset(IVec3::new(v.x, base + 9, v.y), HIERRO_FUNDIDO);
    e.dset(IVec3::new(v.x, base + 8, v.y), LUZ_GAS);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obra(ctx: Contexto, uso: Uso, nivel: u8, alta: bool, seed: u64) -> Estilo {
        let m = MaterialesObra { muro: Muro::Ladrillo, cubierta: Cubierta::Azotea, revoque: true, hierro: true };
        estilo(&ctx, &m, uso, nivel, alta, seed)
    }

    #[test]
    fn tipos_de_la_epoca_3() {
        let mut vistos = std::collections::BTreeSet::new();
        for s in 0..400u64 {
            let ctx = Contexto { anio: 1890, clase: (s % 4) as u8, esquina: (s % 3) as u8, lugar: 1, epoca: 2, ..Default::default() };
            let uso = if s % 5 == 0 { Uso::Comercio } else { Uso::Vivienda };
            let es = obra(ctx, uso, 2 + (s % 2) as u8, s % 7 == 0, s);
            assert!(es.neo, "fachada nueva");
            assert!(es.anio >= ANIO_INMIGRACION);
            vistos.insert(es.e3);
            if es.e3 == tipo3::RENTA {
                assert!(es.pisos >= 3);
            }
        }
        for k in [tipo3::STANDARD, tipo3::ALTOS, tipo3::RENTA, tipo3::ESQUINA] {
            assert!(vistos.contains(&k), "falta el tipo {k}: {vistos:?}");
        }
        // Las de la Ciudad Nueva no son de la época 3.
        let es = obra(Contexto { anio: 1850, epoca: 1, ..Default::default() }, Uso::Vivienda, 2, false, 3);
        assert_eq!(es.e3, 0);
    }

    #[test]
    fn el_conventillo_conserva_la_fachada() {
        let ctx = Contexto { anio: 1795, clase: 1, lugar: 1, ..Default::default() };
        let antes = obra(ctx, Uso::Vivienda, 2, false, 11);
        let conv = obra(Contexto { conventillo: 1890, ..ctx }, Uso::Vivienda, 2, false, 11);
        assert_eq!((antes.muro, antes.vanos, antes.puerta, antes.ancho), (conv.muro, conv.vanos, conv.puerta, conv.ancho));
        assert!(conv.renta && conv.conventillo == 1890);
        assert!(conv.nombre().starts_with("conventillo"), "{}", conv.nombre());
    }
}
