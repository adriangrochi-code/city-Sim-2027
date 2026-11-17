//! Catálogo de estructuras de la época colonial, armadas con el kit de `structures`.
//! Proporciones (en vóxeles de diseño de 0,5 m): planta baja 4,5–5 m, planta alta 4 m, puerta 1,5 × 3 m,
//! ventana 1 × 2 m con antepecho de 1 m, zócalo 1 m, pretil 0,5–1 m, solar de 12 × 38 m
//! (frente de 14 varas y fondo hasta el medio de la manzana de 100 varas).

use super::*;

/// Paleta de revoques de la ciudad colonial (la cal blanca domina).
const CALES: [Mat; 6] = [CAL, CAL, CAL, CAL_OCRE, CAL_ROSA, CAL_AZUL];
const SHUTTERS: [Mat; 2] = [SHUTTER, WOOD];

pub const LOT_W: i32 = 24;
pub const LOT_D: i32 = 76;

/// Rancho de barro y paja, retirado de la calle, con huerta, horno de barro y corral de
/// palo a pique.
pub fn rancho(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(1);
    let bw = r.range(12, 15);
    let bd = r.range(9, 11);
    let u0 = (f.width - bw) / 2 + r.range(-2, 2);
    let w0 = r.range(4, 9);
    let (u1, w1) = (u0 + bw, w0 + bd);
    let wall = 5;
    f.foundation(t, e, u0, u1, w0, w1, ADOBE);
    f.fill(e, u0, u1, 0, wall, w0, w1, ADOBE);
    thatch_roof(f, e, u0, u1, w0, w1, wall, ADOBE);
    let door = u0 + bw / 3;
    f.clear(e, door, door + 2, 0, 4, w0, w0 + 1);
    f.raw_fill(e, door, door + 2, 0, 4, w0 + 1, w0 + 2, WOOD_DARK);
    f.clear(e, u1 - 4, u1 - 2, 2, 4, w0, w0 + 1);
    f.raw_fill(e, u1 - 4, u1 - 2, 2, 4, w0 + 1, w0 + 2, WINDOW);
    // Horno de barro al costado.
    // Horno de barro al costado, solo si entra en el solar (no invade al vecino).
    let ou = if u1 + 6 <= f.width { Some(u1 + 2) } else if u0 >= 6 { Some(u0 - 5) } else { None };
    if let Some(ou) = ou {
        dome(f, e, ou as f32 + 1.5, (w0 + 3) as f32, 2.2, 0, ADOBE);
    }
    let back = (w1 + 2).min(f.depth - 6);
    f.surface(e, 2, f.width - 2, back, f.depth - 2, GARDEN);
    if r.f() < 0.6 {
        fence(f, e, 1, f.width - 1, back, f.depth - 1, f.width / 2);
    }
}

/// Fachada de planta baja: zócalo, zaguán y ventanas con reja volada.
fn ground_facade(f: &Frame, e: &mut Edits, h_zoc: i32, zoc: Mat, door_left: bool, shutter: Mat, windows: i32) {
    zocalo(f, e, 0, f.width, 0, 2, h_zoc, zoc);
    let door = if door_left { 2 } else { f.width - 5 };
    opening(f, e, door, door + 3, 0, 6, 1, shutter);
    f.fill(e, door - 1, door + 4, 6, 7, -1, 0, STONE);
    for k in 0..windows {
        let wx = if door_left { 8 + k * 6 } else { f.width - 11 - k * 6 };
        window_reja(f, e, wx, wx + 2, 2, 6);
    }
}

/// Casa de azotea: una planta encalada sobre la línea de la calle, zaguán, rejas voladas,
/// pretil, patio con aljibe y ala lateral. Variante: ala trasera con media agua de teja.
pub fn casa_azotea(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(2);
    let h = r.range(9, 10);
    let fd = 14;
    let wall = CALES[(r.next() % CALES.len() as u64) as usize];
    let shutter = SHUTTERS[(r.next() % 2) as usize];
    let left = r.next() % 2 == 0;
    f.foundation(t, e, 0, f.width, 0, fd, ZOCALO);
    f.fill(e, 0, f.width, 0, h, 0, fd, wall);
    f.fill(e, 1, f.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    ring(f, e, 0, f.width, h, h + 1, 0, fd, wall);
    // Remate del pretil: pilastras o pináculos según la casa.
    if r.f() < 0.5 {
        for u in [0, f.width - 1] {
            f.fill(e, u, u + 1, h + 1, h + 2, 0, 1, wall);
        }
    }
    cornisa(f, e, 0, f.width, h - 1, wall);
    ground_facade(f, e, 2, ZOCALO, left, shutter, 2);
    // Ala lateral alrededor del patio.
    let wing_u = if left { f.width - 7 } else { 0 };
    let w1 = f.depth - 8;
    f.foundation(t, e, wing_u, wing_u + 7, fd, w1, ZOCALO);
    f.fill(e, wing_u, wing_u + 7, 0, h - 1, fd, w1, wall);
    if r.f() < 0.4 {
        // Media agua de teja sobre el ala del fondo.
        for k in 0..4 {
            f.fill(e, wing_u - 1, wing_u + 8, h - 1 + k, h + k, fd + k * 2, w1 + 1, TILE);
        }
    } else {
        f.fill(e, wing_u, wing_u + 7, h - 2, h - 1, fd, w1, AZOTEA);
        ring(f, e, wing_u, wing_u + 7, h - 1, h, fd, w1, wall);
    }
    chimney(f, e, wing_u + 2, w1 - 4, 0, h + 2);
    let patio_u = if left { 1 } else { 8 };
    let patio_w = f.width - 9;
    f.surface(e, patio_u, patio_u + patio_w, fd, w1, PLAZA);
    aljibe(f, e, patio_u + patio_w / 2 - 1, fd + 6);
    f.surface(e, 1, f.width - 1, w1, f.depth - 1, GARDEN);
}

/// Casa de altos: dos plantas, zócalo de piedra, cornisas, balcones de hierro en la planta
/// alta, pretil con pilastras y mirador para vigilar los barcos.
pub fn casa_altos(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(3);
    let (h1, h) = (9, 17);
    let fd = 18;
    let wall = CALES[(r.next() % CALES.len() as u64) as usize];
    let shutter = SHUTTERS[(r.next() % 2) as usize];
    let left = r.next() % 2 == 0;
    f.foundation(t, e, 0, f.width, 0, fd, STONE);
    f.fill(e, 0, f.width, 0, h, 0, fd, wall);
    f.fill(e, 1, f.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    ground_facade(f, e, 2, STONE, left, shutter, 2);
    cornisa(f, e, 0, f.width, h1, wall);
    cornisa(f, e, 0, f.width, h - 1, wall);
    // Planta alta: tres puertas-ventana alineadas, con balcón corrido o balcones sueltos.
    let continuous = r.f() < 0.4;
    for k in 0..3 {
        let wx = 3 + k * 8;
        opening(f, e, wx, wx + 2, h1 + 1, h1 + 6, 1, WINDOW);
        f.fill(e, wx - 1, wx + 3, h1 + 6, h1 + 7, -1, 0, wall);
        if !continuous {
            balcony(f, e, wx - 1, wx + 3, h1);
        }
    }
    if continuous {
        balcony(f, e, 1, f.width - 1, h1);
    }
    // Pretil con pilastras (balaustrada).
    ring(f, e, 0, f.width, h, h + 1, 0, fd, wall);
    let mut u = 0;
    while u < f.width {
        f.fill(e, u, u + 1, h + 1, h + 2, 0, 1, wall);
        if u + 1 < f.width - 1 {
            f.fill(e, u + 1, (u + 4).min(f.width - 1), h + 1, h + 2, 0, 1, REJA);
        }
        u += 4;
    }
    f.fill(e, f.width - 1, f.width, h + 1, h + 2, 0, 1, wall);
    // Mirador.
    let mu = if left { f.width - 8 } else { 2 };
    f.fill(e, mu, mu + 6, h, h + 6, 3, 9, wall);
    for (fu, fw) in [(mu + 2, 2), (mu + 2, 9)] {
        f.clear(e, fu, fu + 2, h + 2, h + 5, fw, fw + 1);
        f.raw_fill(e, fu, fu + 2, h + 2, h + 5, fw + if fw == 2 { 1 } else { -1 }, fw + if fw == 2 { 2 } else { 0 }, WINDOW);
    }
    f.fill(e, mu - 1, mu + 7, h + 6, h + 7, 2, 10, wall);
    f.fill(e, mu, mu + 6, h + 6, h + 7, 3, 9, AZOTEA);
    // Ala de dos plantas alrededor del patio.
    let wing_u = if left { 0 } else { f.width - 7 };
    let w1 = f.depth - 8;
    f.foundation(t, e, wing_u, wing_u + 7, fd, w1, STONE);
    f.fill(e, wing_u, wing_u + 7, 0, h - 2, fd, w1, wall);
    f.fill(e, wing_u, wing_u + 7, h - 3, h - 2, fd, w1, AZOTEA);
    chimney(f, e, wing_u + 3, w1 - 3, 0, h + 1);
    let patio_u = if left { 8 } else { 1 };
    f.surface(e, patio_u, patio_u + f.width - 9, fd, w1, PLAZA);
    aljibe(f, e, patio_u + 6, fd + 7);
}

/// Pulpería de esquina: ochava con la puerta y el letrero, reja de mostrador, palenque
/// para atar los caballos y una barrica. `corner_left` = la esquina está en u = 0.
pub fn pulperia(t: &TerrainGen, e: &mut Edits, f: &Frame, corner_left: bool) {
    let mut r = f.rng(4);
    let h = 10;
    let fd = 22;
    let wall = if r.f() < 0.6 { CAL } else { CAL_OCRE };
    // Trabajamos en un marco donde la esquina siempre está en u = 0.
    let g = if corner_left { *f } else { Frame { origin: f.origin + f.du * (f.width - 1), du: -f.du, ..*f } };
    g.foundation(t, e, 0, g.width, 0, fd, ZOCALO);
    g.fill(e, 0, g.width, 0, h, 0, fd, wall);
    g.fill(e, 1, g.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    ring(&g, e, 0, g.width, h, h + 1, 0, fd, wall);
    zocalo(&g, e, 0, g.width, 0, fd, 2, ZOCALO);
    cornisa(&g, e, 0, g.width, h - 1, wall);
    // Ochava: se corta la esquina en diagonal (2,5 m de cateto).
    let c = 5;
    for w in 0..c {
        for u in 0..c {
            if u + w < c {
                g.clear(e, u, u + 1, -1, h + 2, w, w + 1);
            }
        }
    }
    // La cornisa también se corta en la ochava.
    g.clear(e, 0, c, h - 1, h, -1, 0);
    for u in 0..=c {
        let w = c - u;
        // Puerta en la ochava (madera) con letrero encima; el resto, pared.
        let m = if (1..c).contains(&u) { WOOD } else { wall };
        g.fill(e, u, u + 1, 0, 6, w, w + 1, m);
        g.fill(e, u, u + 1, 6, 8, w, w + 1, if (1..c).contains(&u) { WOOD_DARK } else { wall });
        g.fill(e, u, u + 1, h, h + 1, w, w + 1, wall);
    }
    // Reja de mostrador sobre el frente y ventana lateral.
    window_reja(&g, e, 9, 13, 2, 6);
    window_reja(&g, e, 16, 18, 2, 6);
    let side = g.left_side();
    window_reja(&side, e, 9, 11, 2, 6);
    opening(&g, e, g.width - 4, g.width - 1, 0, 6, 1, SHUTTER);
    // Palenque y barrica sobre la vereda.
    for u in [8, 15] {
        g.raw_fill(e, u, u + 1, 0, 3, -4, -3, WOOD_DARK);
    }
    g.raw_fill(e, 8, 16, 2, 3, -4, -3, WOOD_DARK);
    g.raw_fill(e, 6, 7, 0, 2, -1, 0, WOOD);
    g.surface(e, 1, g.width - 1, fd, g.depth - 1, DIRT);
    if r.f() < 0.7 {
        fence(&g, e, 0, g.width, fd, g.depth, g.width - 4);
    }
}

/// Cabildo: dos plantas con recova de arcos hacia la plaza, balcones, pretil y torre con
/// campanario. Ocupa un frente de 30 m.
pub fn cabildo(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (h1, h) = (10, 19);
    let fd = 30;
    let wall = CAL;
    f.foundation(t, e, 0, f.width, 0, fd, STONE);
    f.fill(e, 0, f.width, 0, h, 0, fd, wall);
    f.fill(e, 1, f.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    zocalo(f, e, 0, f.width, 0, fd, 2, STONE);
    // Recova: galería de 3 m de fondo detrás de una arquería.
    f.clear(e, 2, f.width - 2, 0, h1 - 2, 2, 8);
    let bays = (f.width - 2) / 6;
    for k in 0..bays {
        let u0 = 2 + k * 6;
        arch(f, e, u0, u0 + 4, 0, h1 - 2, 0, 2);
        // Puertas al fondo de la galería.
        f.raw_fill(e, u0 + 1, u0 + 3, 0, 5, 8, 9, if k % 2 == 0 { WOOD } else { WINDOW });
    }
    f.surface(e, 0, f.width, 0, 8, PLAZA);
    cornisa(f, e, 0, f.width, h1 - 1, STONE);
    // Planta alta: puertas-ventana sobre cada arco, con balcón; escudo al centro.
    for k in 0..bays {
        let u0 = 2 + k * 6;
        if k == bays / 2 {
            f.fill(e, u0, u0 + 4, h1 + 1, h1 + 6, -1, 0, STONE);
            f.fill(e, u0 + 1, u0 + 3, h1 + 2, h1 + 5, -2, -1, CAL_OCRE);
            continue;
        }
        opening(f, e, u0 + 1, u0 + 3, h1 + 1, h1 + 6, 1, WINDOW);
        balcony(f, e, u0, u0 + 4, h1);
    }
    cornisa(f, e, 0, f.width, h - 1, STONE);
    ring(f, e, 0, f.width, h, h + 2, 0, fd, wall);
    let mut u = 0;
    while u < f.width {
        f.fill(e, u, u + 1, h + 2, h + 3, 0, 1, wall);
        u += 6;
    }
    // Torre del campanario.
    let (tu, tw, ts) = (f.width / 2 - 5, 6, 10);
    f.fill(e, tu, tu + ts, h, h + 9, tw, tw + ts, wall);
    for (fr, a, b) in [(f.sub(tu, tw, ts, ts, 11), 2, 8)] {
        arch(&fr, e, a, b, h + 2, h + 8, 0, 2);
        fr.raw_fill(e, a + 2, b - 2, h + 2, h + 5, 2, 3, IRON);
    }
    f.fill(e, tu - 1, tu + ts + 1, h + 9, h + 10, tw - 1, tw + ts + 1, STONE);
    f.fill(e, tu + 2, tu + ts - 2, h + 10, h + 14, tw + 2, tw + ts - 2, wall);
    dome(f, e, (tu + ts / 2) as f32, (tw + ts / 2) as f32, 3.5, h + 14, TILE);
    f.raw_fill(e, tu + ts / 2, tu + ts / 2 + 1, h + 18, h + 21, tw + ts / 2, tw + ts / 2 + 1, IRON);
    // Reloj.
    f.fill(e, tu + 4, tu + 6, h + 10, h + 12, tw + 1, tw + 2, CAL_OCRE);
}

/// Iglesia matriz en sus fases de obra (1 a 5): cimientos, muros, naves con techo, cúpula y
/// torres. Mientras se construye se ve el ladrillo y hay andamios; al final se revoca a la
/// cal. Fachada hacia w = 0. Marco recomendado: 30 × 84 vóxeles (15 × 42 m).
pub fn iglesia(t: &TerrainGen, e: &mut Edits, f: &Frame, phase: u32) {
    let phase = phase.clamp(1, 5);
    let done = phase == 5;
    let wall = if done { CAL } else { BRICK };
    let (nave_w0, nave_w1) = (4, 64);
    let (cr_w0, cr_w1) = (48, 62);
    let hw = 24;
    let ts = 9; // lado de cada torre
    f.foundation(t, e, -6, f.width + 6, 0, 76, STONE);
    // Fase 1: cimientos de piedra con la planta marcada y materiales acopiados.
    let shell = |e: &mut Edits, y1: i32, m: Mat| {
        ring(f, e, 2, f.width - 2, 0, y1, nave_w0, nave_w1, m);
        ring(f, e, 3, f.width - 3, 0, y1, nave_w0 + 1, nave_w1 - 1, m);
        ring(f, e, -6, f.width + 6, 0, y1, cr_w0, cr_w1, m);
        ring(f, e, 6, f.width - 6, 0, y1, nave_w1 - 1, 74, m);
        f.fill(e, 0, ts, 0, y1, 0, ts, m);
        f.fill(e, f.width - ts, f.width, 0, y1, 0, ts, m);
        f.fill(e, ts, f.width - ts, 0, y1, 0, 4, m);
    };
    if phase == 1 {
        shell(e, 1, STONE);
        f.surface(e, -8, f.width + 8, -4, 78, DIRT);
        let mut r = f.rng(5);
        for k in 0..6 {
            let (u, w) = (r.range(6, f.width - 8), 12 + k * 7);
            f.fill(e, u, u + 2, 0, 2, w, w + 2, if k % 2 == 0 { STONE } else { BRICK });
        }
        f.raw_fill(e, -6, -2, 0, 1, 20, 26, SAND);
        return;
    }
    // Fase 2: muros a media altura con andamios.
    let walls_h = if phase == 2 { hw / 2 } else { hw };
    shell(e, walls_h, wall);
    zocalo(f, e, 2, f.width - 2, nave_w0, nave_w1, 2, STONE);
    if phase == 2 {
        f.surface(e, -8, f.width + 8, -4, 78, DIRT);
        scaffold(f, e, 0, f.width, walls_h + 2);
        let side = f.left_side();
        scaffold(&side, e, 6, 70, walls_h + 2);
        let side_r = f.right_side();
        scaffold(&side_r, e, 6, 70, walls_h + 2);
        return;
    }
    // Fase 3: naves con techo de teja, frontón y torres a la altura del cuerpo.
    gable(f, e, 2, f.width - 2, nave_w0 + 4, nave_w1, hw, 2, 1, TILE, wall, false);
    gable(f, e, -6, f.width + 6, cr_w0, cr_w1, hw, 2, 1, TILE, wall, true);
    gable(f, e, 6, f.width - 6, nave_w1, 74, hw - 2, 2, 1, TILE, wall, false);
    // Frontón de la fachada (entre las torres).
    for k in 0..6 {
        f.fill(e, ts + k, f.width - ts - k, hw + k, hw + k + 1, 0, 4, wall);
    }
    f.fill(e, ts - 1, f.width - ts + 1, hw, hw + 1, -1, 0, STONE);
    // Puerta principal en arco y óculo.
    arch(f, e, f.width / 2 - 3, f.width / 2 + 3, 0, 11, 0, 2);
    f.raw_fill(e, f.width / 2 - 3, f.width / 2 + 3, 0, 9, 2, 3, WOOD);
    f.raw_fill(e, f.width / 2 - 1, f.width / 2 + 1, 15, 18, 0, 1, WINDOW);
    // Ventanas altas de la nave.
    let mut w = nave_w0 + 8;
    while w < cr_w0 - 2 {
        // En los marcos laterales la pared de la nave queda a 2 vóxeles del borde.
        for fr in [f.left_side(), f.right_side()] {
            fr.clear(e, w, w + 2, 14, 19, 2, 3);
            fr.raw_fill(e, w, w + 2, 14, 19, 3, 4, WINDOW);
        }
        w += 8;
    }
    let tower_h = match phase {
        3 => hw,
        4 => hw + 8,
        _ => 44,
    };
    for tu in [0, f.width - ts] {
        f.fill(e, tu, tu + ts, 0, tower_h, 0, ts, wall);
    }
    if phase == 3 {
        f.surface(e, -8, f.width + 8, -4, 2, DIRT);
        scaffold(f, e, 0, f.width, hw + 6);
        return;
    }
    // Fase 4: tambor y cúpula sobre el crucero, con linterna.
    let (cu, cw) = (f.width as f32 / 2.0, (cr_w0 + cr_w1) as f32 / 2.0);
    cylinder(f, e, cu, cw, 7.5, hw, hw + 7, wall);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let (u, w) = ((cu + a.cos() * 7.0) as i32, (cw + a.sin() * 7.0) as i32);
        f.raw_fill(e, u, u + 1, hw + 3, hw + 6, w, w + 1, WINDOW);
    }
    f.fill(e, cu as i32 - 8, cu as i32 + 8, hw + 7, hw + 8, cw as i32 - 8, cw as i32 + 8, STONE);
    dome(f, e, cu, cw, 7.5, hw + 8, DOME);
    f.fill(e, cu as i32 - 1, cu as i32 + 1, hw + 15, hw + 19, cw as i32 - 1, cw as i32 + 1, wall);
    f.raw_fill(e, cu as i32, cu as i32 + 1, hw + 19, hw + 22, cw as i32, cw as i32 + 1, IRON);
    f.raw_fill(e, cu as i32 - 1, cu as i32 + 2, hw + 20, hw + 21, cw as i32, cw as i32 + 1, IRON);
    if phase == 4 {
        f.surface(e, -8, f.width + 8, -4, 2, DIRT);
        for tu in [0, f.width - ts] {
            let tf = f.sub(tu, 0, ts, ts, 21);
            scaffold(&tf, e, -2, ts + 2, tower_h + 3);
        }
        return;
    }
    // Fase 5: torres con campanario, cupulines y cruces; todo revocado a la cal.
    for tu in [0, f.width - ts] {
        let tf = f.sub(tu, 0, ts, ts, 31);
        cornisa(&tf, e, -1, ts + 1, hw, STONE);
        tf.fill(e, -1, ts + 1, 34, 35, -1, ts + 1, STONE);
        // Campanario con arcos en las cuatro caras.
        for side in [tf, tf.left_side(), tf.right_side()] {
            arch(&side, e, 3, 6, 36, 42, 0, 1);
        }
        tf.fill(e, -1, ts + 1, 44, 45, -1, ts + 1, STONE);
        dome(&tf, e, ts as f32 / 2.0, ts as f32 / 2.0, 3.6, 45, DOME);
        tf.raw_fill(e, ts / 2, ts / 2 + 1, 49, 53, ts / 2, ts / 2 + 1, IRON);
        tf.raw_fill(e, ts / 2 - 1, ts / 2 + 2, 51, 52, ts / 2, ts / 2 + 1, IRON);
    }
    f.surface(e, -8, f.width + 8, -6, 2, PLAZA);
}

/// Tramo de muralla con talud, cordón, adarve y almenas; portón con arco en el centro y
/// garita en el extremo izquierdo. El exterior de la ciudad está hacia w < 0.
pub fn muralla(t: &TerrainGen, e: &mut Edits, f: &Frame, gate: bool, garita: bool) {
    let h = 12;
    let (w0, w1) = (2, 10);
    f.foundation(t, e, 0, f.width, 0, w1, STONE);
    f.fill(e, 0, f.width, 0, h, w0, w1, STONE);
    // Talud: la base se ensancha hacia afuera.
    f.fill(e, 0, f.width, 0, 3, 0, w0, STONE);
    f.fill(e, 0, f.width, 3, 6, 1, w0, STONE);
    f.raw_fill(e, 0, f.width, 6, 7, 1, 2, STONE_DARK);
    // Adarve y parapeto con almenas.
    f.fill(e, 0, f.width, h, h + 2, w0, w0 + 2, STONE);
    let mut u = 0;
    while u < f.width {
        f.fill(e, u, (u + 2).min(f.width), h + 2, h + 3, w0, w0 + 2, STONE);
        u += 4;
    }
    f.fill(e, 0, f.width, h, h + 1, w1 - 1, w1, STONE);
    if !gate {
        if garita {
            garita_at(f, e, h);
        }
        return;
    }
    // Portón.
    let c = f.width / 2;
    arch(f, e, c - 4, c + 4, 0, 10, 0, w1);
    f.surface(e, c - 4, c + 4, -6, w1 + 6, COBBLE);
    for (pu, du) in [(c - 6, 0), (c + 4, 0)] {
        f.fill(e, pu + du, pu + du + 2, 0, h + 3, -1, w0, STONE_DARK);
    }
    f.fill(e, c - 6, c + 6, h + 2, h + 6, w0 - 1, w0 + 1, STONE);
    f.fill(e, c - 2, c + 2, h + 3, h + 5, w0 - 2, w0 - 1, CAL);
    f.raw_fill(e, c - 1, c + 1, h + 3, h + 5, w0 - 3, w0 - 2, CAL_OCRE);
    // Hojas del portón abiertas contra los costados.
    f.raw_fill(e, c - 4, c - 3, 0, 8, w0 + 1, w0 + 5, WOOD);
    f.raw_fill(e, c + 3, c + 4, 0, 8, w0 + 1, w0 + 5, WOOD);
    if garita {
        garita_at(f, e, h);
    }
}

/// Garita en el extremo izquierdo: torrecilla volada sobre ménsulas, con mirilla y cupulín.
fn garita_at(f: &Frame, e: &mut Edits, h: i32) {
    let (gu, gw) = (2.5f32, 0.0f32);
    for (k, rr) in [(7, 1.2), (8, 2.0), (9, 2.6)] {
        cylinder(f, e, gu, gw, rr, k, k + 1, STONE);
    }
    cylinder(f, e, gu, gw, 2.8, 10, h + 4, STONE);
    f.raw_fill(e, 2, 3, h + 1, h + 3, -3, -2, WINDOW);
    dome(f, e, gu, gw, 2.8, h + 4, STONE);
    f.raw(e, 2, h + 7, 0, STONE_DARK);
}

/// Muelle de madera sobre pilotes, con bolardos y una lancha amarrada. El marco va con
/// `dw` hacia el agua y `base` = 0 (nivel del río); la cubierta queda a 1,5 m.
pub fn muelle(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let deck = 3;
    f.raw_fill(e, 0, 12, deck, deck + 1, 0, f.depth, WOOD);
    let mut w = 2;
    while w < f.depth {
        for u in [0, 11] {
            let p = f.at(u, 0, w);
            let ground = top_at(t, p.x, p.z);
            for y in (ground + 1)..(f.base + deck) {
                e.dset(IVec3::new(p.x, y, p.z), WOOD_DARK);
            }
        }
        if w % 12 == 2 {
            for u in [0, 11] {
                f.raw_fill(e, u, u + 1, deck + 1, deck + 3, w, w + 1, WOOD_DARK);
            }
        }
        w += 6;
    }
    // Lancha amarrada al costado.
    let lw = f.depth - 22;
    f.raw_fill(e, 14, 19, 0, 2, lw, lw + 12, WOOD);
    f.clear(e, 15, 18, 1, 2, lw + 1, lw + 11);
    f.raw_fill(e, 15, 18, 0, 1, lw + 1, lw + 11, WOOD_DARK);
    f.raw_fill(e, 15, 18, 1, 2, lw + 4, lw + 6, SACK);
}

/// Barraca de cueros junto al muelle: galpón de ladrillo con techo de teja, portón y fardos.
pub fn barraca(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    barraca_galpon(t, e, f);
    f.surface(e, -2, f.width + 2, -8, 0, DIRT_ROAD);
}

fn barraca_galpon(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (wd, dp, h) = (f.width, 20, 10);
    f.foundation(t, e, 0, wd, 0, dp, STONE);
    f.fill(e, 0, wd, 0, h, 0, dp, BRICK);
    zocalo(f, e, 0, wd, 0, dp, 2, STONE);
    gable(f, e, 0, wd, 0, dp, h, 2, 1, TILE, BRICK, true);
    opening(f, e, wd / 2 - 3, wd / 2 + 3, 0, 8, 1, WOOD_DARK);
    for u in [4, wd - 6] {
        opening(f, e, u, u + 2, 5, 7, 1, WINDOW);
    }
    // Fardos de cueros y bolsas en la vereda.
    for (u, m) in [(2, HIDE), (6, HIDE), (wd - 10, SACK), (wd - 6, HIDE)] {
        f.raw_fill(e, u, u + 3, 0, 2, -4, -1, m);
    }
}

// --- Comercio e industria en solares (zonas comerciales e industriales) ----------------------

/// Frente de tienda sobre una fachada ya construida: puertas anchas de madera en lugar de
/// ventanas, letrero corrido y toldo de lona; mercadería en la vereda.
fn shopfront(f: &Frame, e: &mut Edits, r: &mut Rng, h_sign: i32) {
    let doors = [2, 9, 16];
    for &u in &doors {
        // Borra rejas y alféizares de la fachada anterior y abre la puerta de la tienda.
        f.clear(e, u - 1, u + 5, 1, h_sign, -1, 0);
        opening(f, e, u, u + 4, 0, 6, 1, if r.f() < 0.5 { WOOD } else { SHUTTER });
    }
    f.fill(e, 1, f.width - 1, h_sign, h_sign + 1, -1, 0, WOOD_DARK);
    if r.f() < 0.6 {
        f.raw_fill(e, 1, f.width - 1, 6, 7, -3, 0, SACK);
    }
    for (u, m) in [(1, SACK), (7, WOOD), (f.width - 3, SACK)] {
        if r.f() < 0.7 {
            f.raw_fill(e, u, u + 2, 0, 2, -3, -1, m);
        }
    }
}

/// Tienda: casa de azotea con la planta baja abierta a la calle.
pub fn tienda(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    casa_azotea(t, e, f);
    let mut r = f.rng(21);
    shopfront(f, e, &mut r, 7);
}

/// Almacén de altos: casa de altos con almacén y depósito en la planta baja.
pub fn almacen(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    casa_altos(t, e, f);
    let mut r = f.rng(22);
    shopfront(f, e, &mut r, 7);
    // Depósito al fondo del patio con portón para las carretas.
    let (w0, w1) = (f.depth - 22, f.depth - 6);
    f.foundation(t, e, 1, f.width - 1, w0, w1, STONE);
    f.fill(e, 1, f.width - 1, 0, 8, w0, w1, BRICK);
    gable(f, e, 1, f.width - 1, w0, w1, 8, 2, 1, TILE, BRICK, true);
}

/// Taller de herrero o carpintero: galpón con alero sobre horcones, fragua con chimenea,
/// pila de leña, una rueda de carreta y el patio de trabajo cercado.
pub fn taller(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(23);
    let (w0, w1, h) = (4, 20, 7);
    let wall = if r.f() < 0.5 { ADOBE } else { BRICK };
    f.foundation(t, e, 1, f.width - 1, w0, w1, ADOBE);
    f.fill(e, 1, f.width - 1, 0, h, w0, w1, wall);
    // Frente abierto: el taller trabaja a la vista.
    f.clear(e, 3, f.width - 3, 0, h - 1, w0, w0 + 4);
    let roof = if r.f() < 0.5 { TILE } else { THATCH };
    gable(f, e, 1, f.width - 1, w0, w1, h, 2, 1, roof, wall, true);
    // Alero sobre horcones hacia la calle.
    for u in [1, f.width / 2, f.width - 2] {
        f.raw_fill(e, u, u + 1, 0, h - 1, 1, 2, WOOD_DARK);
    }
    f.raw_fill(e, 0, f.width, h - 1, h, 0, w0, roof);
    // Fragua y chimenea.
    f.fill(e, 4, 7, 0, 2, w0 + 6, w0 + 9, BRICK_DARK);
    chimney(f, e, 4, w1 - 4, 0, h + 4);
    // Leña, tablones y una rueda.
    f.surface(e, 1, f.width - 1, w1, f.depth - 2, DIRT);
    for k in 0..3 {
        f.raw_fill(e, 2, 9, 0, 2 + k % 2, w1 + 3 + k * 3, w1 + 5 + k * 3, WOOD);
    }
    f.raw_fill(e, f.width - 8, f.width - 2, 0, 1, w1 + 4, w1 + 12, WOOD_DARK);
    cylinder(f, e, (f.width - 5) as f32, (w1 + 20) as f32, 1.6, 0, 1, WOOD_DARK);
    fence(f, e, 0, f.width, w1, f.depth - 1, f.width - 5);
}

/// Curtiembre y jabonería: edificio de ladrillo sobre la calle, pozas de curtir en el fondo,
/// tendederos con cueros y la caldera del jabón con su chimenea.
pub fn curtiembre(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(24);
    let (fd, h) = (16, 9);
    f.foundation(t, e, 0, f.width, 0, fd, STONE);
    f.fill(e, 0, f.width, 0, h, 0, fd, BRICK);
    zocalo(f, e, 0, f.width, 0, fd, 2, STONE);
    gable(f, e, 0, f.width, 0, fd, h, 2, 1, TILE, BRICK, true);
    opening(f, e, f.width / 2 - 3, f.width / 2 + 3, 0, 7, 1, WOOD_DARK);
    for u in [3, f.width - 5] {
        opening(f, e, u, u + 2, 4, 6, 1, WINDOW);
    }
    chimney(f, e, f.width - 5, fd - 4, 0, h + 6);
    // Pozas de curtir: rectángulos de piedra con agua turbia.
    let back = f.depth - 2;
    f.surface(e, 0, f.width, fd, back, DIRT);
    for k in 0..3 {
        let w = fd + 3 + k * 6;
        ring(f, e, 2, 10, 0, 1, w, w + 5, STONE);
        f.raw_fill(e, 3, 9, 0, 1, w + 1, w + 4, MUD);
    }
    // Tendederos con cueros.
    let racks = 3 + (r.next() % 2) as i32;
    for k in 0..racks {
        let w = fd + 4 + k * 8;
        if w + 2 >= back {
            break;
        }
        for u in [12, f.width - 2] {
            f.raw_fill(e, u, u + 1, 0, 5, w, w + 1, WOOD_DARK);
        }
        f.raw_fill(e, 12, f.width - 1, 4, 5, w, w + 1, WOOD);
        f.raw_fill(e, 13, f.width - 2, 1, 4, w, w + 1, HIDE);
    }
    fence(f, e, 0, f.width, fd, f.depth - 1, 4);
}

// --- Más estilos por tipo y densidad ------------------------------------------------------

/// Casa de teja: una planta encalada con techo de teja a dos aguas sobre la calle, puerta al
/// medio y dos ventanas; huerta y pozo en el fondo.
pub fn casa_teja(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(31);
    let (fd, h) = (14, 8);
    let wall = CALES[(r.next() % CALES.len() as u64) as usize];
    f.foundation(t, e, 0, f.width, 0, fd, ZOCALO);
    f.fill(e, 0, f.width, 0, h, 0, fd, wall);
    zocalo(f, e, 0, f.width, 0, fd, 2, ZOCALO);
    gable(f, e, 0, f.width, 0, fd, h, 2, 1, if r.f() < 0.5 { TILE } else { TILE_DARK }, wall, true);
    let mid = f.width / 2;
    opening(f, e, mid - 1, mid + 2, 0, 6, 1, SHUTTERS[(r.next() % 2) as usize]);
    for u in [3, f.width - 6] {
        window_reja(f, e, u, u + 3, 2, 6);
    }
    chimney(f, e, f.width - 4, fd - 4, 0, h + 5);
    f.surface(e, 1, f.width - 1, fd, fd + 12, PLAZA);
    aljibe(f, e, 3, fd + 4);
    f.surface(e, 1, f.width - 1, fd + 12, f.depth - 2, GARDEN);
    fence(f, e, 0, f.width, fd + 12, f.depth - 1, f.width / 2);
}

/// Casa de inquilinato (densidad alta): frente de azotea y, a lo largo de un patio angosto,
/// una fila de piezas con su puerta cada una, hasta el fondo del solar.
pub fn inquilinato(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(32);
    let h = 9;
    let wall = CALES[(r.next() % CALES.len() as u64) as usize];
    let fd = 12;
    f.foundation(t, e, 0, f.width, 0, fd, ZOCALO);
    f.fill(e, 0, f.width, 0, h, 0, fd, wall);
    f.fill(e, 1, f.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    ring(f, e, 0, f.width, h, h + 1, 0, fd, wall);
    cornisa(f, e, 0, f.width, h - 1, wall);
    ground_facade(f, e, 2, ZOCALO, true, SHUTTER, 3);
    // Piezas en fila sobre un costado, de 4 m cada una, con su puerta al patio.
    let (pu0, pu1) = (f.width - 10, f.width);
    let w1 = f.depth - 4;
    f.foundation(t, e, pu0, pu1, fd, w1, ZOCALO);
    f.fill(e, pu0, pu1, 0, h - 1, fd, w1, wall);
    f.fill(e, pu0 + 1, pu1 - 1, h - 2, h - 1, fd + 1, w1 - 1, AZOTEA);
    ring(f, e, pu0, pu1, h - 1, h, fd, w1, wall);
    let mut w = fd + 3;
    while w + 4 < w1 {
        // Puerta y ventanita hacia el patio (cara u = pu0).
        f.clear(e, pu0 - 1, pu0 + 1, 0, 5, w, w + 2);
        f.raw_fill(e, pu0, pu0 + 1, 0, 5, w, w + 2, WOOD_DARK);
        f.clear(e, pu0 - 1, pu0 + 1, 2, 4, w + 4, w + 5);
        f.raw_fill(e, pu0, pu0 + 1, 2, 4, w + 4, w + 5, WINDOW);
        w += 8;
    }
    // Patio largo con piletas y ropa tendida.
    f.surface(e, 1, pu0, fd, w1, PLAZA);
    aljibe(f, e, 4, fd + 10);
    for k in 0..3 {
        let ww = fd + 20 + k * 14;
        f.raw_fill(e, 2, 3, 0, 5, ww, ww + 1, WOOD_DARK);
        f.raw_fill(e, pu0 - 2, pu0 - 1, 0, 5, ww, ww + 1, WOOD_DARK);
        f.raw_fill(e, 3, pu0 - 2, 4, 5, ww, ww + 1, if r.f() < 0.5 { SACK } else { CAL });
    }
}

/// Casa de altos de renta (densidad alta): dos plantas en todo el frente y un ala de dos
/// plantas hasta el fondo, con galería de madera hacia el patio.
pub fn altos_renta(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    casa_altos(t, e, f);
    let mut r = f.rng(33);
    let wall = CALES[(r.next() % CALES.len() as u64) as usize];
    let (h, fd) = (17, 18);
    let wing = if r.f() < 0.5 { 0 } else { f.width - 8 };
    let w1 = f.depth - 6;
    f.foundation(t, e, wing, wing + 8, fd, w1, STONE);
    f.fill(e, wing, wing + 8, 0, h - 1, fd, w1, wall);
    f.fill(e, wing, wing + 8, h - 2, h - 1, fd, w1, AZOTEA);
    ring(f, e, wing, wing + 8, h - 1, h, fd, w1, wall);
    // Galería del primer piso hacia el patio, con baranda y pies derechos.
    let gu = if wing == 0 { 8 } else { wing - 2 };
    f.raw_fill(e, gu, gu + 2, 8, 9, fd, w1, WOOD);
    f.raw_fill(e, if wing == 0 { gu + 1 } else { gu }, if wing == 0 { gu + 2 } else { gu + 1 }, 9, 11, fd, w1, REJA);
    let mut w = fd + 2;
    while w < w1 {
        f.raw_fill(e, if wing == 0 { gu + 1 } else { gu }, if wing == 0 { gu + 2 } else { gu + 1 }, 0, 8, w, w + 1, WOOD_DARK);
        w += 6;
    }
}

/// Fonda y posada: una planta con techo de teja, letrero, palenque y un patio de carretas al
/// costado con portón.
pub fn fonda(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(34);
    let (fd, h) = (16, 9);
    let wall = if r.f() < 0.5 { CAL_OCRE } else { CAL };
    let bw = f.width - 8;
    f.foundation(t, e, 0, bw, 0, fd, ZOCALO);
    f.fill(e, 0, bw, 0, h, 0, fd, wall);
    zocalo(f, e, 0, bw, 0, fd, 2, ZOCALO);
    gable(f, e, 0, bw, 0, fd, h, 2, 1, TILE, wall, true);
    opening(f, e, 3, 7, 0, 6, 1, WOOD);
    window_reja(f, e, 10, 13, 2, 6);
    f.fill(e, 2, bw - 2, 7, 8, -1, 0, WOOD_DARK); // letrero
    for u in [1, 8] {
        f.raw_fill(e, u, u + 1, 0, 3, -4, -3, WOOD_DARK);
    }
    f.raw_fill(e, 1, 9, 2, 3, -4, -3, WOOD_DARK);
    // Patio de carretas con portón y galpón al fondo.
    f.surface(e, bw, f.width, 0, f.depth - 2, DIRT_ROAD);
    f.fill(e, bw, f.width, 0, 5, -1, 0, WOOD_DARK);
    f.clear(e, bw + 1, f.width - 1, 0, 5, -1, 0);
    let (w0, w1) = (f.depth - 22, f.depth - 4);
    f.foundation(t, e, 1, f.width - 1, w0, w1, ADOBE);
    f.fill(e, 1, f.width - 1, 0, 7, w0, w1, ADOBE);
    f.clear(e, 2, f.width - 2, 0, 6, w0, w0 + 1);
    gable(f, e, 1, f.width - 1, w0, w1, 7, 2, 1, THATCH, ADOBE, true);
    // Una carreta en el patio.
    f.raw_fill(e, bw + 2, bw + 6, 1, 3, fd + 4, fd + 10, WOOD);
    f.raw_fill(e, bw + 1, bw + 2, 0, 3, fd + 5, fd + 8, WOOD_DARK);
    f.surface(e, 1, bw, fd, w0, DIRT);
}

/// Casa de comercio con recova: dos plantas con arcos en la planta baja sobre la vereda (la
/// recova), balcones arriba y depósito al fondo.
pub fn recova(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(35);
    let (h1, h, fd) = (9, 17, 20);
    let wall = CALES[(r.next() % CALES.len() as u64) as usize];
    f.foundation(t, e, 0, f.width, 0, fd, STONE);
    f.fill(e, 0, f.width, 0, h, 0, fd, wall);
    f.fill(e, 1, f.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    ring(f, e, 0, f.width, h, h + 1, 0, fd, wall);
    cornisa(f, e, 0, f.width, h1, wall);
    cornisa(f, e, 0, f.width, h - 1, wall);
    // Recova: tres arcos de 3,5 m sobre una galería de 3 m de fondo.
    for k in 0..3 {
        let u0 = 1 + k * 8;
        arch(f, e, u0, u0 + 7, 0, 8, 0, 6);
    }
    f.surface(e, 0, f.width, 0, 6, STONE);
    for k in 0..3 {
        let u0 = 2 + k * 8;
        opening(f, e, u0 + 1, u0 + 4, 0, 6, 7, WOOD);
    }
    for k in 0..3 {
        let wx = 3 + k * 8;
        opening(f, e, wx, wx + 2, h1 + 1, h1 + 6, 1, WINDOW);
        balcony(f, e, wx - 1, wx + 3, h1);
    }
    let (w0, w1) = (fd + 10, f.depth - 6);
    f.foundation(t, e, 1, f.width - 1, w0, w1, STONE);
    f.fill(e, 1, f.width - 1, 0, 8, w0, w1, BRICK);
    gable(f, e, 1, f.width - 1, w0, w1, 8, 2, 1, TILE, BRICK, false);
    f.surface(e, 1, f.width - 1, fd, w0, PLAZA);
}

/// Panadería: casa de adobe con el horno de barro en el patio, leña y bolsas de harina.
pub fn panaderia(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(36);
    let (fd, h) = (12, 8);
    let wall = if r.f() < 0.5 { CAL } else { ADOBE };
    f.foundation(t, e, 0, f.width, 0, fd, ZOCALO);
    f.fill(e, 0, f.width, 0, h, 0, fd, wall);
    f.fill(e, 1, f.width - 1, h - 1, h, 1, fd - 1, AZOTEA);
    ring(f, e, 0, f.width, h, h + 1, 0, fd, wall);
    opening(f, e, 3, 7, 0, 6, 1, WOOD);
    window_reja(f, e, 12, 15, 2, 6);
    f.fill(e, 2, 10, 6, 7, -1, 0, WOOD_DARK);
    // Horno de barro grande y su techito.
    f.surface(e, 1, f.width - 1, fd, f.depth - 2, DIRT);
    dome(f, e, 8.0, (fd + 10) as f32, 4.5, 0, ADOBE);
    chimney(f, e, 7, fd + 6, 3, 9);
    for k in 0..4 {
        f.raw_fill(e, 15, 21, 0, 1 + k % 2, fd + 4 + k * 3, fd + 6 + k * 3, WOOD);
    }
    for (u, w) in [(3, fd + 18), (6, fd + 19), (4, fd + 21)] {
        f.raw_fill(e, u, u + 2, 0, 2, w, w + 2, SACK);
    }
    fence(f, e, 0, f.width, fd, f.depth - 1, 4);
}

/// Velería y jabonería: galpón de ladrillo con la caldera del sebo, chimenea alta y barricas.
pub fn veleria(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(37);
    let (fd, h) = (22, 9);
    f.foundation(t, e, 0, f.width, 0, fd, STONE);
    f.fill(e, 0, f.width, 0, h, 0, fd, BRICK);
    zocalo(f, e, 0, f.width, 0, fd, 2, STONE);
    gable(f, e, 0, f.width, 0, fd, h, 2, 1, if r.f() < 0.5 { TILE } else { TILE_DARK }, BRICK, false);
    opening(f, e, 8, 15, 0, 7, 1, WOOD_DARK);
    chimney(f, e, 3, fd - 5, 0, h + 9);
    // Patio con la caldera al aire libre y barricas de sebo.
    f.surface(e, 1, f.width - 1, fd, f.depth - 2, DIRT);
    cylinder(f, e, 12.0, (fd + 9) as f32, 3.0, 0, 3, BRICK_DARK);
    cylinder(f, e, 12.0, (fd + 9) as f32, 2.0, 3, 4, IRON);
    for k in 0..6 {
        let (u, w) = (2 + (k % 3) * 3, fd + 18 + (k / 3) * 3);
        cylinder(f, e, u as f32 + 1.0, w as f32 + 1.0, 1.0, 0, 3, WOOD);
    }
    fence(f, e, 0, f.width, fd, f.depth - 1, f.width - 5);
}

/// Depósito de frutos del país: un galpón largo de ladrillo hasta el fondo del solar, con
/// portones laterales y fardos en la vereda.
pub fn deposito(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(38);
    let (h, w1) = (10, f.depth - 4);
    f.foundation(t, e, 1, f.width - 1, 0, w1, STONE);
    f.fill(e, 1, f.width - 1, 0, h, 0, w1, BRICK);
    zocalo(f, e, 1, f.width - 1, 0, w1, 2, STONE);
    gable(f, e, 1, f.width - 1, 0, w1, h, 2, 1, if r.f() < 0.5 { TILE } else { TILE_DARK }, BRICK, false);
    opening(f, e, 8, 16, 0, 8, 1, WOOD_DARK);
    let side = f.left_side();
    let mut w = 10;
    while w + 6 < w1 {
        opening(&side, e, w, w + 4, 0, 7, 1, WOOD_DARK);
        w += 16;
    }
    for (u, m) in [(2, HIDE), (6, SACK), (17, HIDE), (20, SACK)] {
        f.raw_fill(e, u, u + 3, 0, 2, -4, -1, m);
    }
}

/// Barraca de solar: el galpón sobre la calle y un segundo depósito al fondo, con el patio
/// de carga lleno de fardos.
pub fn barraca_solar(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    barraca_galpon(t, e, f);
    let (w0, w1) = (f.depth - 26, f.depth - 4);
    f.foundation(t, e, 0, f.width, w0, w1, STONE);
    f.fill(e, 0, f.width, 0, 9, w0, w1, BRICK);
    gable(f, e, 0, f.width, w0, w1, 9, 2, 1, TILE_DARK, BRICK, false);
    f.surface(e, 1, f.width - 1, 20, w0, DIRT);
    let mut r = f.rng(25);
    for k in 0..6 {
        let u = 2 + (r.next() % (f.width as u64 - 6)) as i32;
        let w = 22 + k * 4;
        if w + 3 < w0 {
            f.raw_fill(e, u, u + 3, 0, 1 + (k % 2), w, w + 3, if k % 3 == 0 { SACK } else { HIDE });
        }
    }
}

/// Saladero: galpones, grasería con chimenea, tendederos de cueros, corrales y ganado.
/// Marco recomendado: 120 × 90 vóxeles.
pub fn saladero(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, f.width, 0, f.depth, MUD);
    // Galpón abierto: pilares de ladrillo y techo de teja.
    let ga = f.sub(4, 4, 60, 16, 1);
    ga.foundation(t, e, 0, 60, 0, 16, BRICK);
    let mut u = 0;
    while u < 60 {
        for w in [0, 14] {
            ga.fill(e, u, u + 2, 0, 9, w, w + 2, BRICK);
        }
        u += 8;
    }
    ga.fill(e, 0, 60, 0, 9, 14, 16, BRICK);
    gable(&ga, e, 0, 60, 0, 16, 9, 2, 1, TILE, BRICK, true);
    for k in 0..5 {
        ga.raw_fill(e, 6 + k * 10, 10 + k * 10, 0, 2, 4, 8, HIDE);
    }
    // Galpón cerrado (depósito de tasajo) perpendicular.
    let gb = f.sub(70, 4, 16, 50, 2);
    gb.foundation(t, e, 0, 16, 0, 50, BRICK);
    gb.fill(e, 0, 16, 0, 9, 0, 50, BRICK);
    gable(&gb, e, 0, 16, 0, 50, 9, 2, 1, TILE, BRICK, false);
    let side = gb.left_side();
    for k in 0..4 {
        opening(&side, e, 6 + k * 12, 10 + k * 12, 0, 6, 1, WOOD_DARK);
    }
    // Grasería: chimenea alta y calderos.
    chimney(f, e, 66, 24, 0, 26);
    for (cu, cw) in [(62.0, 30.0), (66.0, 32.0)] {
        cylinder(f, e, cu, cw, 1.8, 0, 3, IRON);
    }
    // Tendederos: postes con una vara y cueros estaqueados colgando.
    let mut w = 30;
    while w < 72 {
        let mut u = 4;
        while u < 60 {
            f.raw_fill(e, u, u + 1, 0, 7, w, w + 1, FENCE);
            if u + 10 < 60 {
                f.raw_fill(e, u, u + 10, 6, 7, w, w + 1, WOOD_DARK);
                f.raw_fill(e, u + 2, u + 5, 2, 6, w, w + 1, HIDE);
                f.raw_fill(e, u + 6, u + 9, 3, 6, w, w + 1, HIDE);
            }
            u += 10;
        }
        w += 8;
    }
    // Corral con ganado.
    fence(f, e, 92, 118, 40, 86, 105);
    f.surface(e, 92, 118, 40, 86, DIRT);
    let mut r = f.rng(9);
    for _ in 0..7 {
        let (u, w) = (r.range(95, 112), r.range(44, 80));
        animal(f, e, u, w, r.f() < 0.5, if r.f() < 0.5 { TRUNK } else { WOOD_DARK });
    }
}

/// Casco de estancia: casa encalada con galería, techo de teja, aljibe, mangrullo, corral
/// redondo con caballos y un ombú. Marco recomendado: 100 × 90 vóxeles.
pub fn estancia(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let casa = f.sub(10, 10, 50, 16, 1);
    casa.foundation(t, e, 0, 50, 0, 16, STONE);
    casa.fill(e, 0, 50, 0, 9, 0, 16, CAL);
    zocalo(&casa, e, 0, 50, 0, 16, 2, ZOCALO);
    gable(&casa, e, 0, 50, 0, 16, 9, 2, 1, TILE, CAL, true);
    for k in 0..5 {
        let u = 4 + k * 9;
        let m = if k == 2 { WOOD } else { SHUTTER };
        opening(&casa, e, u, u + if k == 2 { 3 } else { 2 }, if k == 2 { 0 } else { 2 }, 6, 1, m);
    }
    // Galería: horcones de madera y media agua de teja.
    for k in 0..7 {
        casa.raw_fill(e, k * 8, k * 8 + 1, 0, 7, -6, -5, WOOD_DARK);
    }
    for k in 0..3 {
        casa.fill(e, -1, 51, 7 + k, 8 + k, -7 + k * 2, -5 + k * 2, TILE);
    }
    casa.surface(e, 0, 50, -6, 0, PLAZA);
    chimney(&casa, e, 44, 10, 0, 14);
    aljibe(f, e, 34, 2);
    // Mangrullo: torre de vigía de palos con plataforma y techito de paja.
    let (mu, mw) = (70, 12);
    for (du, dw) in [(0, 0), (4, 0), (0, 4), (4, 4)] {
        f.raw_fill(e, mu + du, mu + du + 1, 0, 20, mw + dw, mw + dw + 1, WOOD_DARK);
    }
    f.raw_fill(e, mu - 1, mu + 6, 18, 19, mw - 1, mw + 6, WOOD);
    f.raw_fill(e, mu - 1, mu + 6, 21, 22, mw - 1, mw + 6, THATCH);
    f.raw_fill(e, mu, mu + 5, 22, 23, mw, mw + 5, THATCH);
    // Corral redondo con caballos.
    round_fence(f, e, 76.0, 60.0, 16.0);
    f.surface(e, 62, 92, 46, 76, DIRT);
    let mut r = f.rng(7);
    for _ in 0..4 {
        let (u, w) = (r.range(68, 82), r.range(52, 66));
        animal(f, e, u, w, r.f() < 0.5, if r.f() < 0.5 { TRUNK } else { CAL_SUCIA });
    }
    let p = f.at(28, 0, 60);
    ombu(t, e, IVec2::new(p.x, p.z), f.seed ^ 3);
    f.surface(e, 4, 24, 30, 46, GARDEN);
}

/// Atahona: molino de harina movido por un caballo, en una construcción redonda con techo
/// cónico, depósito y bolsas. Marco recomendado: 40 × 40 vóxeles.
pub fn atahona(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (cu, cw) = (13.0, 20.0);
    f.foundation(t, e, 4, 23, 11, 30, ADOBE);
    cylinder(f, e, cu, cw, 8.5, 0, 8, CAL);
    cone(f, e, cu, cw, 10.5, 8, TILE);
    opening(f, e, 11, 14, 0, 6, 1, WOOD);
    f.clear(e, 11, 14, 0, 6, 11, 13);
    f.raw_fill(e, 11, 14, 0, 6, 13, 14, WOOD_DARK);
    let dep = f.sub(24, 12, 14, 14, 2);
    dep.foundation(t, e, 0, 14, 0, 14, ADOBE);
    dep.fill(e, 0, 14, 0, 8, 0, 14, CAL);
    gable(&dep, e, 0, 14, 0, 14, 8, 2, 1, TILE, CAL, false);
    opening(&dep, e, 5, 8, 0, 6, 1, WOOD);
    for (u, w) in [(1, -3), (4, -3), (9, -3), (11, -2)] {
        dep.raw_fill(e, u, u + 2, 0, 1 + (u % 2), w, w + 2, SACK);
    }
    // Carro de bueyes esperando la harina.
    f.raw_fill(e, 2, 6, 1, 3, 2, 8, WOOD);
    f.raw_fill(e, 1, 2, 0, 3, 3, 6, WOOD_DARK);
    f.raw_fill(e, 6, 7, 0, 3, 3, 6, WOOD_DARK);
    f.surface(e, 0, 40, 0, 10, DIRT_ROAD);
}

/// Molino de viento: torre redonda encalada, capucha de teja y cuatro aspas de lona.
pub fn molino(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (cu, cw) = (10.0, 12.0);
    f.foundation(t, e, 3, 18, 5, 20, STONE);
    cylinder(f, e, cu, cw, 5.5, 0, 18, CAL);
    cone(f, e, cu, cw, 6.0, 18, TILE);
    opening(f, e, 9, 12, 0, 6, 1, WOOD);
    f.clear(e, 9, 12, 0, 6, 6, 7);
    f.raw_fill(e, 9, 12, 0, 6, 7, 8, WOOD_DARK);
    // Eje y aspas en el plano de la fachada.
    let (hu, hy, hw) = (10, 16, 5);
    f.raw_fill(e, hu, hu + 1, hy, hy + 1, hw - 2, hw + 1, WOOD_DARK);
    for (du, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for k in 1..13 {
            let (u, y) = (hu + du * k, hy + dy * k);
            f.raw(e, u, y, hw - 2, WOOD_DARK);
            if k > 3 {
                // Lona de cada aspa (a un costado del brazo).
                let (su, sy) = (u + dy, y - du);
                f.raw(e, su, sy, hw - 2, CAL_SUCIA);
                f.raw(e, su + dy, sy - du, hw - 2, CAL_SUCIA);
            }
        }
    }
}

// --- Servicios, obras y campo (Fase 2 #5) -------------------------------------------------
// Medidas reales en vóxeles de diseño de 0,5 m. Las que no salen de una fuente son
// estimaciones de proporción (ver docs/sistemas/servicios.md).

/// Pozo de balde: brocal de piedra de 1,5 m de diámetro y 1 m de alto, dos horcones con
/// travesaño y roldana, y el balde. Marco: 10 × 10 vóxeles (5 × 5 m).
pub fn pozo(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.foundation(t, e, 1, 9, 1, 9, STONE);
    f.surface(e, 0, 10, 0, 10, DIRT);
    f.fill(e, 1, 9, 0, 1, 1, 9, STONE);
    cylinder(f, e, 5.0, 5.0, 1.6, 1, 3, STONE);
    f.clear(e, 4, 6, 1, 3, 4, 6);
    f.raw_fill(e, 4, 6, 0, 1, 4, 6, MUD);
    for u in [2, 7] {
        f.raw_fill(e, u, u + 1, 1, 6, 4, 5, WOOD_DARK);
    }
    f.raw_fill(e, 2, 8, 6, 7, 4, 5, WOOD_DARK);
    f.raw(e, 5, 5, 4, WOOD);
    f.raw(e, 5, 4, 4, IRON);
    // Balde apoyado junto al brocal.
    f.raw(e, 7, 1, 7, WOOD);
}

/// Aljibe público: losa de la cisterna de piedra (8 × 8 m) un poco elevada, brocal de cal con
/// arco de hierro y roldana, pileta de piedra y caño de bajada. Marco: 20 × 20 (10 × 10 m).
pub fn aljibe_publico(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.foundation(t, e, 2, 18, 2, 18, STONE);
    f.surface(e, 0, 20, 0, 20, PLAZA);
    f.fill(e, 2, 18, 0, 1, 2, 18, STONE);
    ring(f, e, 2, 18, 1, 2, 2, 18, STONE_DARK);
    // Brocal grande encalado con arco de hierro.
    ring(f, e, 8, 12, 1, 3, 8, 12, CAL);
    f.raw_fill(e, 9, 11, 1, 2, 9, 11, MUD);
    for u in [8, 11] {
        f.raw_fill(e, u, u + 1, 3, 7, 10, 11, IRON);
    }
    f.raw_fill(e, 8, 12, 7, 8, 10, 11, IRON);
    f.raw(e, 10, 6, 10, WOOD);
    // Pileta para los aguadores y los caballos.
    f.fill(e, 13, 17, 1, 2, 4, 6, STONE);
    f.raw_fill(e, 14, 16, 1, 2, 4, 5, MUD);
    // Canaleta de bajada desde una casa vecina (caño de lata sobre un pilar).
    f.fill(e, 3, 4, 1, 6, 15, 16, STONE);
    f.raw_fill(e, 3, 9, 6, 7, 15, 16, IRON);
}

/// Carro de aguatero: dos ruedas altas, pipa de madera acostada y varas.
pub fn carro_aguatero(f: &Frame, e: &mut Edits, u: i32, w: i32) {
    for du in [0, 5] {
        f.raw_fill(e, u + du, u + du + 1, 0, 4, w + 1, w + 3, WOOD_DARK);
        f.raw_fill(e, u + du, u + du + 1, 1, 3, w, w + 4, WOOD_DARK);
    }
    // Pipa (tonel) acostada a lo largo del carro.
    f.raw_fill(e, u + 1, u + 5, 2, 5, w + 1, w + 3, WOOD);
    f.raw_fill(e, u + 1, u + 5, 3, 4, w, w + 4, WOOD);
    f.raw_fill(e, u + 1, u + 2, 2, 5, w + 1, w + 3, IRON);
    f.raw_fill(e, u + 4, u + 5, 2, 5, w + 1, w + 3, IRON);
    // Varas hacia adelante.
    f.raw_fill(e, u + 1, u + 2, 2, 3, w - 5, w, WOOD_DARK);
    f.raw_fill(e, u + 4, u + 5, 2, 3, w - 5, w, WOOD_DARK);
}

/// Puesto de aguadores: rancho de adobe con techo de teja, corral para las mulas, carros con
/// su pipa y pipas de repuesto. Marco: 28 × 36 (14 × 18 m).
pub fn puesto_aguadores(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, 28, 0, 36, DIRT_ROAD);
    let casa = f.sub(2, 20, 16, 12, 1);
    casa.foundation(t, e, 0, 16, 0, 12, ADOBE);
    casa.fill(e, 0, 16, 0, 6, 0, 12, ADOBE);
    gable(&casa, e, 0, 16, 0, 12, 6, 2, 1, TILE, ADOBE, true);
    opening(&casa, e, 6, 9, 0, 5, 1, WOOD_DARK);
    carro_aguatero(f, e, 3, 8);
    carro_aguatero(f, e, 12, 8);
    for (u, w) in [(21, 4), (24, 4), (21, 7)] {
        cylinder(f, e, u as f32 + 0.5, w as f32 + 0.5, 1.2, 0, 3, WOOD);
    }
    fence(f, e, 20, 28, 18, 34, 24);
    let mut r = f.rng(3);
    animal(f, e, 22, 24 + r.range(0, 3), false, WOOD_DARK);
}

/// Farol de sebo sobre un poste de madera (3 m), con el fanal de vidrio que se ve de noche.
pub fn farol(e: &mut Edits, p: IVec2, base: i32) {
    e.dfill(IVec3::new(p.x, base, p.y), IVec3::new(p.x + 1, base + 6, p.y + 1), WOOD_DARK);
    e.dset(IVec3::new(p.x, base + 6, p.y), WINDOW);
    e.dset(IVec3::new(p.x, base + 7, p.y), IRON);
}

/// Puesto de serenos: casita encalada con techo de teja, puerta, farol en la esquina y el
/// banco de la guardia. Marco: 12 × 16 (6 × 8 m).
pub fn puesto_serenos(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let fr = f.sub(0, 2, 12, 14, 4);
    fr.foundation(t, e, 0, 12, 0, 14, STONE);
    fr.fill(e, 0, 12, 0, 8, 0, 14, CAL);
    zocalo(&fr, e, 0, 12, 0, 14, 2, ZOCALO);
    gable(&fr, e, 0, 12, 0, 14, 8, 2, 1, TILE, CAL, false);
    opening(&fr, e, 4, 7, 0, 6, 1, WOOD_DARK);
    window_reja(&fr, e, 8, 10, 2, 5);
    f.surface(e, 0, 12, 0, 2, PLAZA);
    // Farol de hierro en la esquina, sobre una ménsula.
    fr.raw_fill(e, 0, 1, 6, 7, -1, 0, IRON);
    fr.raw(e, 0, 5, -1, WINDOW);
    // Banco de la guardia junto a la puerta.
    f.raw_fill(e, 1, 4, 0, 1, 0, 1, WOOD);
}

/// Lazareto: dos pabellones largos encalados con techo de teja alrededor de un patio, capilla
/// chica con espadaña, cerco de piedra y pozo. Marco: 60 × 48 (30 × 24 m).
pub fn lazareto(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, 60, 0, 48, DIRT);
    for (w0, salt) in [(4, 1u64), (34, 2)] {
        let p = f.sub(4, w0, 36, 10, salt);
        p.foundation(t, e, 0, 36, 0, 10, STONE);
        p.fill(e, 0, 36, 0, 7, 0, 10, CAL);
        zocalo(&p, e, 0, 36, 0, 10, 2, ZOCALO);
        gable(&p, e, 0, 36, 0, 10, 7, 2, 1, TILE, CAL, true);
        let mut u = 3;
        while u < 34 {
            window_reja(&p, e, u, u + 2, 2, 5);
            u += 6;
        }
        opening(&p, e, 16, 19, 0, 5, 1, WOOD_DARK);
    }
    // Capilla del lazareto con espadaña.
    let c = f.sub(44, 18, 10, 14, 3);
    c.foundation(t, e, 0, 10, 0, 14, STONE);
    c.fill(e, 0, 10, 0, 9, 0, 14, CAL);
    gable(&c, e, 0, 10, 0, 14, 9, 2, 1, TILE, CAL, false);
    c.fill(e, 3, 7, 9, 15, 0, 1, CAL);
    arch(&c, e, 4, 6, 11, 14, 0, 1);
    c.raw(e, 4, 12, 0, IRON);
    opening(&c, e, 4, 6, 0, 5, 1, WOOD);
    // Cerco de piedra y pozo en el patio.
    ring(f, e, 0, 60, 0, 3, 0, 48, STONE);
    f.clear(e, 26, 32, 0, 3, 0, 1);
    pozo(t, e, &f.sub(14, 19, 10, 10, 5));
}

/// Chacra: rancho con enramada, huerta, sembrados en surcos (trigo o maíz), parva de paja,
/// pozo y cerco de palo a pique. Marco: 120 × 160 (60 × 80 m).
pub fn chacra(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(11);
    // Sembrado: surcos alternados de tierra y espigas.
    let crop = if r.f() < 0.5 { GRASS_DRY } else { GARDEN };
    for w in 40..156 {
        f.surface(e, 4, 116, w, w + 1, if (w / 2) % 2 == 0 { DIRT } else { crop });
    }
    let ra = f.sub(10, 8, 14, 10, 1);
    ra.foundation(t, e, 0, 14, 0, 10, ADOBE);
    ra.fill(e, 0, 14, 0, 5, 0, 10, ADOBE);
    thatch_roof(&ra, e, 0, 14, 0, 10, 5, ADOBE);
    opening(&ra, e, 5, 7, 0, 4, 1, WOOD_DARK);
    // Enramada: horcones y techo de ramas.
    for (u, w) in [(26, 8), (34, 8), (26, 16), (34, 16)] {
        f.raw_fill(e, u, u + 1, 0, 5, w, w + 1, WOOD_DARK);
    }
    f.raw_fill(e, 25, 36, 5, 6, 7, 18, LEAF_DARK);
    f.surface(e, 70, 100, 8, 30, GARDEN);
    // Parva de paja.
    cone(f, e, 60.0, 20.0, 5.0, 0, THATCH);
    pozo(t, e, &f.sub(40, 8, 10, 10, 2));
    fence(f, e, 2, 118, 2, 158, 20);
}

/// Aduana: casa de dos plantas sobre la calle con portón de carga en arco, balcón corrido,
/// escudo sobre el portón y patio de revisión con fardos. Marco: 48 × 40 (24 × 20 m).
pub fn aduana(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let (wd, dp, h1, h2) = (48, 24, 10, 18);
    f.foundation(t, e, 0, wd, 0, dp, STONE);
    ring(f, e, 0, wd, 0, h2, 0, dp, CAL);
    ring(f, e, 1, wd - 1, 0, h2, 1, dp - 1, CAL);
    f.fill(e, 2, wd - 2, h1 - 1, h1, 2, dp - 2, WOOD_DARK);
    f.fill(e, 0, wd, h2, h2 + 1, 0, dp, AZOTEA);
    zocalo(f, e, 0, wd, 0, dp, 3, STONE);
    cornisa(f, e, 0, wd, h1, STONE);
    cornisa(f, e, 0, wd, h2, STONE);
    f.fill(e, 0, wd, h2 + 1, h2 + 2, 0, 1, CAL);
    // Portón de carga en arco y escudo.
    arch(f, e, wd / 2 - 4, wd / 2 + 4, 0, 9, 0, dp);
    f.surface(e, wd / 2 - 4, wd / 2 + 4, 0, dp + 12, COBBLE);
    f.fill(e, wd / 2 - 2, wd / 2 + 2, h1 + 1, h1 + 4, -1, 0, STONE);
    f.raw_fill(e, wd / 2 - 1, wd / 2 + 1, h1 + 2, h1 + 3, -2, -1, CAL_OCRE);
    for k in 0..4 {
        let u = 4 + k * 5 + if k >= 2 { 18 } else { 0 };
        window_reja(f, e, u, u + 2, 2, 7);
        opening(f, e, u, u + 2, h1 + 2, h2 - 2, 1, SHUTTER);
    }
    balcony(f, e, 3, 19, h1 + 1);
    balcony(f, e, 29, 45, h1 + 1);
    // Patio de revisión con fardos de cueros y bolsas.
    f.surface(e, 0, wd, dp, f.depth, DIRT_ROAD);
    let mut r = f.rng(4);
    for k in 0..6 {
        let (u, w) = (4 + k * 7, dp + 3 + r.range(0, 8));
        f.raw_fill(e, u, u + 3, 0, 2, w, w + 2, if k % 2 == 0 { HIDE } else { SACK });
    }
}

/// Horno de ladrillos: horno de campaña (caja de ladrillo con bocas de fuego), tendal de
/// adobes secándose bajo un techo de paja, pisadero de barro y pilas de ladrillos cocidos.
/// Marco: 48 × 48 (24 × 24 m).
pub fn horno_ladrillos(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, 48, 0, 48, DIRT);
    let h = f.sub(6, 6, 16, 16, 1);
    h.foundation(t, e, 0, 16, 0, 16, BRICK);
    h.fill(e, 0, 16, 0, 9, 0, 16, BRICK_DARK);
    h.fill(e, 1, 15, 9, 10, 1, 15, BRICK);
    for u in [3, 7, 11] {
        arch(&h, e, u, u + 2, 0, 3, 0, 2);
        h.raw_fill(e, u, u + 2, 0, 1, 1, 2, MUD);
    }
    chimney(&h, e, 7, 7, 10, 13);
    // Tendal: horcones y techo de paja sobre hileras de adobes.
    let tg = f.sub(26, 4, 18, 30, 2);
    for (u, w) in [(0, 0), (17, 0), (0, 29), (17, 29), (0, 14), (17, 14)] {
        tg.raw_fill(e, u, u + 1, 0, 6, w, w + 1, WOOD_DARK);
    }
    tg.raw_fill(e, -1, 19, 6, 7, -1, 31, THATCH);
    let mut w = 2;
    while w < 28 {
        tg.raw_fill(e, 2, 16, 0, 1, w, w + 1, ADOBE);
        w += 3;
    }
    f.surface(e, 6, 22, 30, 44, MUD);
    for u in [4, 10, 16] {
        f.raw_fill(e, u, u + 4, 0, 3, 25, 27, BRICK);
    }
}

/// Cantera de piedra: frente de roca cortado en escalones, bloques sueltos, cabria de madera
/// para levantar los sillares y una carreta cargada. Marco: 56 × 44 (28 × 22 m).
pub fn cantera(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    f.surface(e, 0, 56, 0, 44, ROCK);
    f.foundation(t, e, 4, 52, 26, 42, ROCK);
    for k in 0..4 {
        f.fill(e, 4, 52, 0, 2 + k * 2, 26 + k * 4, 30 + k * 4, if k % 2 == 0 { ROCK } else { STONE_DARK });
    }
    let mut r = f.rng(6);
    for _ in 0..9 {
        let (u, w) = (r.range(4, 48), r.range(6, 22));
        f.fill(e, u, u + 3, 0, 2, w, w + 2, STONE);
    }
    // Cabria: trípode de palos con la roldana.
    for (u, w) in [(20, 14), (28, 14), (24, 20)] {
        f.raw_fill(e, u, u + 1, 0, 10, w, w + 1, WOOD_DARK);
    }
    f.raw_fill(e, 20, 29, 10, 11, 14, 21, WOOD);
    f.raw_fill(e, 24, 25, 4, 10, 17, 18, IRON);
    // Carreta cargada.
    f.raw_fill(e, 6, 12, 1, 2, 4, 9, WOOD);
    f.raw_fill(e, 7, 11, 2, 3, 5, 8, STONE);
    for u in [5, 12] {
        f.raw_fill(e, u, u + 1, 0, 3, 5, 8, WOOD_DARK);
    }
}

/// Muelle de madera sobre pilotes que entra al río, con bolardos, una grúa de pluma en la
/// punta y una lancha amarrada al costado. El marco va con `w` hacia el agua y la base en el
/// nivel medio del río; la cubierta queda a 2 m. Ancho 12 (6 m), largo `f.depth`.
pub fn muelle_puerto(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let deck = 4;
    let wd = 12;
    f.raw_fill(e, 0, wd, deck, deck + 1, 0, f.depth, WOOD);
    let mut w = 1;
    while w < f.depth {
        for u in [0, wd - 1] {
            // Pilotes desde el lecho del río hasta la cubierta.
            let p = f.at(u, 0, w);
            let ground = top_at(t, p.x, p.z);
            for y in (ground + 1)..(f.base + deck) {
                e.dset(IVec3::new(p.x, y, p.z), WOOD_DARK);
            }
            if w % 12 == 1 {
                f.raw_fill(e, u, u + 1, deck + 1, deck + 3, w, w + 1, WOOD_DARK);
            }
        }
        w += 4;
    }
    // Grúa de pluma en la punta.
    let tip = f.depth - 6;
    f.raw_fill(e, 2, 3, deck + 1, deck + 16, tip, tip + 1, WOOD_DARK);
    f.raw_fill(e, 2, 3, deck + 15, deck + 16, tip - 8, tip + 1, WOOD);
    f.raw_fill(e, 2, 3, deck + 9, deck + 15, tip - 8, tip - 7, IRON);
    f.raw_fill(e, 4, 7, deck + 1, deck + 3, tip - 2, tip + 1, HIDE);
    // Lancha amarrada al costado (flota a ras del agua).
    let lw = f.depth - 24;
    if lw > 4 {
        f.raw_fill(e, wd + 2, wd + 7, 1, 3, lw, lw + 12, WOOD);
        f.clear(e, wd + 3, wd + 6, 2, 3, lw + 1, lw + 11);
        f.raw_fill(e, wd + 3, wd + 6, 1, 2, lw + 4, lw + 7, SACK);
        f.raw_fill(e, wd + 4, wd + 5, 3, 10, lw + 5, lw + 6, WOOD_DARK);
    }
}

/// Tramo de muralla en obra o terminado. `phase`: 0 cimientos y piedra acopiada, 1 muro a
/// media altura con andamios, 2 muro completo con adarve y almenas. `gate` = (u0, u1) del vano
/// del portón (donde cruza una calle). El exterior queda hacia w < 0. Alto 6 m, espesor 4 m.
pub fn muralla_tramo(t: &TerrainGen, e: &mut Edits, f: &Frame, phase: u32, gate: Option<(i32, i32)>, garita: bool) {
    let h = 12;
    let (w0, w1) = (2, 10);
    f.foundation(t, e, 0, f.width, 0, w1, STONE);
    let open = |u: i32| gate.is_some_and(|(a, b)| u >= a && u < b);
    // Camino de ronda de tierra por dentro (y la huella de la obra), en todas las fases.
    for u in (0..f.width).filter(|&u| !open(u)) {
        f.surface(e, u, u + 1, w1, w1 + 6, DIRT_ROAD);
    }
    let top = match phase {
        0 => 1,
        1 => h / 2,
        _ => h,
    };
    for u in 0..f.width {
        if open(u) {
            continue;
        }
        f.fill(e, u, u + 1, 0, top, w0, w1, STONE);
        f.fill(e, u, u + 1, 0, top.min(3), 0, w0, STONE);
        if top >= 6 {
            f.fill(e, u, u + 1, 3, 6, 1, w0, STONE);
        }
    }
    if phase == 0 {
        let mut r = f.rng(2);
        for _ in 0..(f.width / 10).max(1) {
            let u = r.range(0, (f.width - 3).max(0));
            f.raw_fill(e, u, u + 3, 0, 2, w1 + 2, w1 + 4, STONE);
        }
        return;
    }
    if phase == 1 {
        match gate {
            Some((a, b)) => {
                scaffold(f, e, 0, a, top + 2);
                scaffold(f, e, b, f.width, top + 2);
            }
            None => scaffold(f, e, 0, f.width, top + 2),
        }
        return;
    }
    // Cordón, adarve y parapeto con almenas hacia afuera.
    for u in (0..f.width).filter(|&u| !open(u)) {
        f.raw(e, u, 6, 1, STONE_DARK);
    }
    for u in 0..f.width {
        if open(u) {
            continue;
        }
        f.fill(e, u, u + 1, h, h + 2, w0, w0 + 2, STONE);
        if u.rem_euclid(4) < 2 {
            f.fill(e, u, u + 1, h + 2, h + 3, w0, w0 + 2, STONE);
        }
        f.fill(e, u, u + 1, h, h + 1, w1 - 1, w1, STONE);
    }
    if let Some((a, b)) = gate {
        // Portón: arco de piedra sobre el vano, pilastras y la hornacina con el escudo.
        let span = b - a;
        f.fill(e, a - 2, a, 0, h + 3, -1, w1, STONE_DARK);
        f.fill(e, b, b + 2, 0, h + 3, -1, w1, STONE_DARK);
        // Dintel de piedra a 5 m: el vano queda libre de lado a lado para las carretas.
        f.fill(e, a, b, h - 2, h + 3, w0 - 1, w1, STONE);
        f.raw_fill(e, a, b, h - 3, h - 2, w0 - 1, w0, STONE_DARK);
        let _ = span;
        f.fill(e, (a + b) / 2 - 2, (a + b) / 2 + 2, h + 3, h + 6, w0 - 1, w0 + 1, STONE);
        f.raw_fill(e, (a + b) / 2 - 1, (a + b) / 2 + 1, h + 4, h + 5, w0 - 2, w0 - 1, CAL_OCRE);
    }
    if garita {
        garita_at(f, e, h);
    }
}

/// Capilla de adobe y paja (la primera iglesia, antes de la matriz de material): nave de 6 ×
/// 14 m, espadaña con la campana y una cruz de madera. Va en el marco de la iglesia (30 × 84).
pub fn capilla(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let c = f.sub(f.width / 2 - 6, 6, 12, 28, 1);
    c.foundation(t, e, 0, 12, 0, 28, ADOBE);
    c.fill(e, 0, 12, 0, 8, 0, 28, ADOBE);
    thatch_roof(&c, e, 0, 12, 0, 28, 8, ADOBE);
    // Espadaña sobre la fachada con la campana.
    c.fill(e, 3, 9, 8, 17, 0, 1, ADOBE);
    arch(&c, e, 5, 7, 12, 16, 0, 1);
    c.raw(e, 5, 13, 0, IRON);
    c.raw_fill(e, 5, 7, 17, 18, 0, 1, WOOD_DARK);
    c.raw(e, 6, 18, 0, WOOD_DARK);
    opening(&c, e, 4, 8, 0, 6, 1, WOOD_DARK);
    // Cruz de madera en el atrio.
    f.raw_fill(e, f.width / 2, f.width / 2 + 1, 0, 6, 1, 2, WOOD_DARK);
    f.raw_fill(e, f.width / 2 - 1, f.width / 2 + 2, 4, 5, 1, 2, WOOD_DARK);
    f.surface(e, f.width / 2 - 8, f.width / 2 + 8, 0, 6, DIRT);
}

/// Restos de una casa quemada: muros bajos ennegrecidos y desparejos, vigas caídas y ceniza.
/// Va en el marco del solar (24 × 76).
pub fn ruina(t: &TerrainGen, e: &mut Edits, f: &Frame) {
    let mut r = f.rng(13);
    let (u0, w0) = (r.range(4, 6), r.range(3, 7));
    let (u1, w1) = (u0 + r.range(12, 15), w0 + r.range(9, 12));
    f.foundation(t, e, u0, u1, w0, w1, ADOBE_DARK);
    f.surface(e, u0 - 2, u1 + 2, w0 - 2, w1 + 2, MUD);
    for u in u0..u1 {
        for w in w0..w1 {
            if u != u0 && u != u1 - 1 && w != w0 && w != w1 - 1 {
                continue;
            }
            let h = r.range(1, 4);
            f.raw_fill(e, u, u + 1, 0, h, w, w + 1, if r.f() < 0.5 { ADOBE_DARK } else { WOOD_DARK });
        }
    }
    for k in 0..3 {
        let w = w0 + 2 + k * 3;
        f.raw_fill(e, u0 + 1 + k, u1 - 2, 0, 1, w, w + 1, WOOD_DARK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(t: &TerrainGen, seed: u64, width: i32, depth: i32) -> Frame {
        // Un solar en tierra firme de la península (13500, 16300 m).
        let origin = IVec2::new(27000, 32600);
        let base = top_at(t, origin.x, origin.y) + 1;
        Frame { origin, du: IVec2::X, dw: IVec2::Y, base, width, depth, seed }
    }

    fn sorted(e: &Edits) -> Vec<(i32, i32, i32, Mat)> {
        let mut v: Vec<_> = e.design.iter().map(|(p, m)| (p.x, p.y, p.z, *m)).collect();
        v.sort();
        v
    }

    type Builder = fn(&TerrainGen, &mut Edits, &Frame);
    const HOUSES: [(&str, Builder); 3] = [("rancho", rancho), ("casa de azotea", casa_azotea), ("casa de altos", casa_altos)];

    #[test]
    fn misma_semilla_mismo_edificio() {
        let t = TerrainGen::new(1726);
        for (name, b) in HOUSES {
            let (mut a, mut c, mut d) = (Edits::default(), Edits::default(), Edits::default());
            b(&t, &mut a, &frame(&t, 42, LOT_W, LOT_D));
            b(&t, &mut c, &frame(&t, 42, LOT_W, LOT_D));
            b(&t, &mut d, &frame(&t, 43, LOT_W, LOT_D));
            assert_eq!(sorted(&a), sorted(&c), "{name}: la misma semilla debe dar el mismo edificio");
            assert_ne!(sorted(&a), sorted(&d), "{name}: otra semilla debería dar una variante");
        }
    }

    #[test]
    fn las_casas_no_salen_del_solar() {
        let t = TerrainGen::new(1726);
        for seed in 0..12u64 {
            let mut builds: Vec<(&str, Edits)> = Vec::new();
            for (name, b) in HOUSES {
                let mut e = Edits::default();
                b(&t, &mut e, &frame(&t, seed, LOT_W, LOT_D));
                builds.push((name, e));
            }
            for corner in [true, false] {
                let mut e = Edits::default();
                pulperia(&t, &mut e, &frame(&t, seed, LOT_W, LOT_D), corner);
                builds.push(("pulpería", e));
            }
            const TRADES: [(&str, Builder); 13] = [
                ("tienda", tienda),
                ("almacén", almacen),
                ("taller", taller),
                ("curtiembre", curtiembre),
                ("barraca", barraca_solar),
                ("casa de teja", casa_teja),
                ("inquilinato", inquilinato),
                ("altos de renta", altos_renta),
                ("fonda", fonda),
                ("recova", recova),
                ("panadería", panaderia),
                ("velería", veleria),
                ("depósito", deposito),
            ];
            for (name, b) in TRADES {
                let mut e = Edits::default();
                b(&t, &mut e, &frame(&t, seed, LOT_W, LOT_D));
                assert!(e.design.len() > 500, "{name}: muy pocos vóxeles");
                builds.push((name, e));
            }
            for (name, e) in builds {
                let f = frame(&t, seed, LOT_W, LOT_D);
                for p in e.design.keys() {
                    let (u, _, w) = f.local(*p);
                    // Se permiten balcones, rejas voladas y el palenque sobre la vereda (w ≥ −4).
                    assert!((-1..=LOT_W).contains(&u) && (-4..LOT_D).contains(&w), "{name} (semilla {seed}): vóxel fuera del solar en u={u}, w={w}");
                }
            }
        }
    }

    #[test]
    fn la_iglesia_crece_fase_a_fase_y_al_final_no_quedan_andamios() {
        let t = TerrainGen::new(1726);
        let mut prev = (0usize, i32::MIN);
        for phase in 1..=5 {
            let mut e = Edits::default();
            let f = frame(&t, 9, 30, 84);
            iglesia(&t, &mut e, &f, phase);
            let above: Vec<_> = e.design.iter().filter(|(p, _)| p.y >= f.base).collect();
            let count = above.len();
            let top = above.iter().map(|(p, _)| p.y).max().unwrap();
            assert!(count > prev.0, "fase {phase}: debería haber más obra que en la anterior");
            assert!(top >= prev.1, "fase {phase}: la obra no puede bajar de altura");
            let scaffolds = e.design.values().filter(|&&m| m == SCAFFOLD).count();
            if phase == 5 {
                assert_eq!(scaffolds, 0, "la iglesia terminada no tiene andamios");
            } else if phase >= 2 {
                assert!(scaffolds > 0, "fase {phase}: en obra hay andamios");
            }
            prev = (count, top);
        }
    }

    #[test]
    fn el_desgaste_es_determinista_y_no_cambia_el_tipo_de_material() {
        for k in 0..2000 {
            let p = IVec3::new(k * 7, k % 13, k * 3);
            let a = weather(CAL, p, 5, 1);
            assert_eq!(a, weather(CAL, p, 5, 1));
            assert!(a == CAL || a == CAL_SUCIA);
            let tl = weather(TILE, p, 5, 1);
            assert!([TILE, TILE_DARK, TILE_LIGHT].contains(&tl));
        }
    }
}
