//! Acciones del recorrido automático del modo juego (`--demo-ciudad`).

use std::time::Instant;

use glam::Vec2;

use super::roads::{self, RoadKind};
use super::{grid_lines, plaza_frame, BLOCK};
use crate::session::Session;

/// Plaza mayor de la demostración: la Ciudad Vieja del mapa real, junto a la Plaza Matriz.
pub const PLAZA: Vec2 = crate::terrain::FOUNDING_REAL;
/// Punto medio del camino curvo que sale hacia el este (para la captura de cerca).
pub const CURVE_VIEW: Vec2 = Vec2::new(PLAZA.x + 5.0 * BLOCK, PLAZA.y - 0.6 * BLOCK);

pub(crate) fn apply(s: &mut Session, r: Result<crate::buildings::Edits, String>) -> String {
    match r {
        Ok(e) => {
            let t0 = Instant::now();
            let n = s.apply_edits(&e);
            format!("{} vóxeles, {n} columnas a re-mallar (encolado {:.1?})", e.voxel_count(), t0.elapsed())
        }
        Err(m) => format!("ERROR: {m}"),
    }
}

pub fn found(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    g.speed = 0.0; // el tiempo solo avanza con `grow` (capturas reproducibles)
    g.city.money = 200_000;
    let r = g.city.found_plaza(&t, PLAZA, 0.0);
    if r.is_ok() {
        // Como al fundar con el clic: la gaceta deja de pedir que se funde la ciudad.
        g.message = "¡Fundada la ciudad!".into();
    }
    apply(s, r)
}

/// Damero de 5×5 cuadras alrededor de la plaza, empedrado.
pub fn grid(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let (f, side) = plaza_frame(0.0);
    let a = PLAZA - side * (BLOCK * 2.5) - f * (BLOCK * 2.5);
    let lines = grid_lines(a, a + side * BLOCK * 5.0, a + f * BLOCK * 5.0);
    // En el terreno real alguna cuadra puede caer en la barranca: si el damero entero no se
    // puede, se traza cuadra por cuadra y se saltean las que no.
    let r = g.city.build_grid(&t, &lines, RoadKind::Stone).or_else(|err| {
        println!("Damero completo: {err}; se traza cuadra por cuadra");
        let mut all = crate::buildings::Edits::default();
        for l in &lines {
            let (p0, p1) = (l[0], *l.last().unwrap());
            let n = (p0.distance(p1) / BLOCK).round().max(1.0) as i32;
            for k in 0..n {
                let seg = roads::line(p0.lerp(p1, k as f32 / n as f32), p0.lerp(p1, (k + 1) as f32 / n as f32));
                if let Ok(e) = g.city.build_road(&t, &seg, RoadKind::Stone) {
                    all.extend(e);
                }
            }
        }
        Ok::<_, String>(all)
    });
    let out = apply(s, r);
    let g = s.game.as_ref().unwrap();
    format!("{out} · {} tramos, {} solares", g.city.net.edges.len(), g.city.lots.lots.len())
}

/// Camino de tierra curvo que sale del damero hacia el este, tierra adentro.
pub fn curve(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let (f, side) = plaza_frame(0.0);
    let p0 = PLAZA + side * (BLOCK * 2.5) + f * (BLOCK * 0.5);
    let ctrl = PLAZA + side * (BLOCK * 5.0) - f * (BLOCK * 1.8);
    let p1 = PLAZA + side * (BLOCK * 7.5) - f * (BLOCK * 0.3);
    let pts = roads::bezier(g.snap(p0), ctrl, p1);
    let r = g.city.build_road(&t, &pts, RoadKind::Dirt);
    let out = apply(s, r);
    let g = s.game.as_ref().unwrap();
    format!("{out} · radio mínimo {:.0} m · {} solares", roads::min_radius(&pts), g.city.lots.lots.len())
}

/// Zonas de la demostración: casas de densidad media en todo el damero, comercio de densidad
/// alta alrededor de la plaza y media hacia el este, e industria sobre el camino del norte
/// (lejos de la plaza: el humo y el olor bajan el valor del suelo).
pub fn zone(s: &mut Session) -> String {
    use super::lots::{zone_code, Density, ZoneKind};
    let g = s.game.as_mut().unwrap();
    let (f, side) = plaza_frame(0.0);
    let mut n = 0;
    let mut e = crate::buildings::Edits::default();
    for (c, r, k, d) in [
        (PLAZA - Vec2::new(0.0, 140.0), 480.0, ZoneKind::Residencial, Density::Media),
        (PLAZA, 100.0, ZoneKind::Comercial, Density::Alta),
        (PLAZA + side * (BLOCK * 2.0), 60.0, ZoneKind::Comercial, Density::Media),
        (PLAZA - side * (BLOCK * 1.6) + f * (BLOCK * 1.2), 70.0, ZoneKind::Residencial, Density::Alta),
        (PLAZA + side * (BLOCK * 5.2) - f * (BLOCK * 0.8), 140.0, ZoneKind::Industrial, Density::Media),
    ] {
        let (k_n, k_e) = g.city.paint(c, r, zone_code(k, d));
        n += k_n;
        e.extend(k_e);
    }
    s.apply_edits(&e);
    let g = s.game.as_ref().unwrap();
    let count = |k: ZoneKind| g.city.lots.lots.values().filter(|l| l.kind() == Some(k)).count();
    format!("{n} solares zonificados: {} residenciales, {} comerciales, {} industriales", count(ZoneKind::Residencial), count(ZoneKind::Comercial), count(ZoneKind::Industrial))
}

/// Punto del último puente construido por la demostración (para la captura).
pub static BRIDGE_AT: std::sync::Mutex<Option<Vec2>> = std::sync::Mutex::new(None);

/// Un camino empedrado con puente sobre el arroyo más cercano a la plaza.
pub fn bridge(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let mut best: Option<(f32, Vec<Vec2>)> = None;
    for arr in &t.arroyos {
        for w in arr.windows(2).step_by(3) {
            let (p, q) = (w[0], w[1]);
            let m = (p + q) * 0.5;
            let dist = m.distance(PLAZA);
            if dist > 3000.0 || best.as_ref().is_some_and(|b| b.0 <= dist) {
                continue;
            }
            let n = (q - p).normalize_or_zero().perp();
            let (x0, x1) = (m - n * 35.0, m + n * 35.0);
            if t.is_water(x0.x, x0.y) || t.is_water(x1.x, x1.y) || !t.is_water(m.x, m.y) {
                continue;
            }
            let line = roads::line(x0, x1);
            if g.city.road_problem(&t, &line, RoadKind::Stone).is_none() {
                best = Some((dist, line));
            }
        }
    }
    let Some((_, line)) = best else { return "no hay arroyos cerca para cruzar".into() };
    let mid = (line[0] + *line.last().unwrap()) * 0.5;
    *BRIDGE_AT.lock().unwrap() = Some(mid);
    let r = g.city.build_road(&t, &line, RoadKind::Stone);
    let out = apply(s, r);
    format!("{out} · puente en {:.0}, {:.0}", mid.x, mid.y)
}

pub fn grow(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let months = 60;
    let t0 = Instant::now();
    let e = g.city.fast_forward(&t, months);
    let sim = t0.elapsed();
    let built = g.city.lots.lots.values().filter(|l| l.level > 0).count();
    let b = g.city.lots.built();
    let (jc, ji) = g.city.lots.jobs();
    let d = g.city.demand();
    let pop = g.city.population();
    let date = g.city.date();
    let out = apply(s, Ok(e));
    format!(
        "{months} meses en {sim:.2?} ({:.1} ms/mes) · {date} · {pop} vecinos · {built} edificios · casas {:?} · comercios {:?} ({jc} empleos) · talleres {:?} ({ji} empleos) · demanda R {:.2} C {:.2} I {:.2} · {out}",
        sim.as_secs_f64() * 1000.0 / months as f64,
        b[0],
        b[1],
        b[2],
        d.r,
        d.c,
        d.i
    )
}

/// Cuarenta años de historia (para las capturas de la población: linajes de tres
/// generaciones, herencias, oficios).
pub fn historia(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let months = 480;
    let t0 = Instant::now();
    let e = g.city.fast_forward(&t, months);
    let sim = t0.elapsed();
    let p = &g.city.poblacion;
    let st = &p.stats;
    let out = format!(
        "{months} meses en {sim:.2?} · {} · {} habitantes ({} en casas) · {} personas registradas · Gini {:.2} · felicidad {:.0} % · ",
        g.city.date(),
        p.habitantes(),
        g.city.population(),
        p.personas.len(),
        st.gini_riqueza,
        st.felicidad * 100.0
    );
    out + &apply(s, Ok(e))
}

/// Un mes más al ritmo del juego (20 pasos de crecimiento de 0,5 s), aplicando cada paso
/// como en la ventana: mide el costo de la simulación y del encolado del re-mallado.
pub fn month_realtime(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let (mut worst, mut total, mut voxels, mut cols, mut changed) = (0f64, 0f64, 0, 0, 0);
    for _ in 0..20 {
        let t0 = Instant::now();
        let g = s.game.as_mut().unwrap();
        let before: Vec<u8> = g.city.lots.lots.values().map(|l| l.level).collect();
        let mut e = crate::buildings::Edits::default();
        g.city.tick(&t, 0.5, &mut e);
        changed += g.city.lots.lots.values().map(|l| l.level).zip(before).filter(|(a, b)| a != b).count();
        voxels += e.voxel_count();
        cols += s.apply_edits(&e);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        worst = worst.max(ms);
        total += ms;
    }
    format!("{changed} edificios nuevos o mejorados · {voxels} vóxeles · {cols} columnas · paso de 0,5 s: medio {:.1} ms, peor {worst:.1} ms", total / 20.0)
}

/// Cámara sobre la calle con más vecinos ahora (para las capturas): a `dist` m, mirando a lo
/// largo de la calle.
pub fn calle_concurrida(s: &Session, dist: f32, pitch: f32) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let mut cuenta: std::collections::BTreeMap<u32, u32> = Default::default();
    for a in &c.agentes.e.agentes {
        if a.patas.len() > 1 || a.proposito != super::agentes::Proposito::Paseo {
            *cuenta.entry(a.pata().tramo).or_default() += 1;
        }
    }
    let (&e, _) = cuenta.iter().max_by_key(|x| (*x.1, std::cmp::Reverse(*x.0)))?;
    // El centro de los que van por esa calle.
    let ps: Vec<Vec2> = c.agentes.e.agentes.iter().filter(|a| a.pata().tramo == e).map(|a| super::agentes::pose(&c.net, a).0).collect();
    let m = ps.iter().copied().sum::<Vec2>() / ps.len() as f32;
    let ed = &c.net.edges[&e];
    let tan = ed.point(ed.len() * 0.5).1;
    let yaw = tan.x.atan2(tan.y).to_degrees() + 160.0;
    Some([m.x, m.y, dist, yaw, pitch])
}

/// En el terreno real el damero queda en pedazos (cuadras demasiado empinadas): caminos de
/// tierra unen cada pedazo cercano con el de la plaza, así los vecinos pueden ir y venir.
pub fn unir_calles(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let mut e = crate::buildings::Edits::default();
    let mut hechos = 0;
    for _ in 0..12 {
        let net = &g.city.net;
        let Some((&centro, _)) = net.nodes.iter().min_by(|x, y| x.1.distance(PLAZA).total_cmp(&y.1.distance(PLAZA))) else { break };
        let comp = super::agentes::componente(net, centro);
        // Pares (nodo de afuera, nodo de la plaza) del más corto al más largo.
        let mut pares: Vec<(f32, Vec2, Vec2)> = Vec::new();
        for (n, &p) in &net.nodes {
            if comp.contains(n) || p.distance(PLAZA) > 800.0 {
                continue;
            }
            for m in &comp {
                let q = net.nodes[m];
                pares.push((p.distance(q), p, q));
            }
        }
        pares.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut ok = false;
        for &(_, p, q) in pares.iter().take(40) {
            if let Ok(x) = g.city.build_road(&t, &roads::line(p, q), RoadKind::Dirt) {
                e.extend(x);
                hechos += 1;
                ok = true;
                break;
            }
        }
        if !ok {
            break;
        }
    }
    let n = s.apply_edits(&e);
    format!("{hechos} caminos de tierra · {n} columnas a re-mallar")
}

/// Vecinos y vehículos: un día hábil que empieza a las 5:30 corre hasta las 7:30 a 30
/// cuadros por segundo, como en la ventana; además, una carreta, un jinete y gente a pie
/// cruzan el puente de la demostración. Mide el costo de la simulación.
pub fn vecinos(s: &mut Session) -> String {
    use super::agentes::{Proposito, Tipo};
    let hasta = 7.5;
    let t = s.terrain.clone();
    let foco = Vec2::new(s.cam.target.x, s.cam.target.z);
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    c.agentes.e.dia += 1;
    c.agentes.foco = Some(foco);
    let dt = 1.0 / 30.0;
    let h0 = 5.5;
    let cuadros = ((hasta - h0) / super::HOURS_PER_SECOND / dt) as usize;
    let (mut peor, mut total) = (0f64, 0f64);
    let mut max_calle = 0;
    for k in 0..cuadros {
        c.agentes.set_hora(h0 + k as f32 * dt * super::HOURS_PER_SECOND);
        let t0 = Instant::now();
        c.agentes.tick(&t, &c.net, &c.lots, c.plaza, dt);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        peor = peor.max(ms);
        total += ms;
        max_calle = max_calle.max(c.agentes.e.agentes.len());
    }
    // El puente (sobre un arroyo, del otro lado: no está unido a la ciudad).
    let mut en_puente = 0;
    let bridge = *BRIDGE_AT.lock().unwrap();
    if let Some(n) = bridge.and_then(|mid| c.net.nearest_edge(mid, 10.0)) {
        let pts = c.net.edges[&n.edge].pts.clone();
        let (a, b) = (pts[0], *pts.last().unwrap());
        let viajes = [(Tipo::Carreta, a, b), (Tipo::Jinete, b, a), (Tipo::APie, a, b), (Tipo::APie, b, a), (Tipo::APie, a, b)];
        for (k, (tipo, desde, hasta)) in viajes.into_iter().enumerate() {
            if c.agentes.lanzar(&c.net, tipo, desde, hasta, None, Proposito::Paseo, k as u32, 1, (k as u8, k as u8)) {
                en_puente += 1;
            }
        }
        // Hasta que la carreta llega al medio del puente.
        let carreta = c.agentes.e.agentes.iter().find(|a| a.tipo == Tipo::Carreta && a.proposito == Proposito::Paseo).map(|a| a.id);
        let mid = bridge.unwrap();
        for _ in 0..6000 {
            c.agentes.tick(&t, &c.net, &c.lots, c.plaza, dt);
            if carreta.and_then(|id| c.agentes.posicion(&c.net, id)).is_none_or(|p| p.distance(mid) < 4.0) {
                break;
            }
        }
    }
    let st = c.agentes.e.stats;
    let n = c.agentes.en_la_calle();
    format!(
        "{} vecinos · {} en la calle a las {:.1} h ({} a pie, {} jinetes, {} carretas, {} carros; máximo {max_calle}) · {} viajes terminados, {} sin salir ({} sin calle cerca) · {en_puente} cruzando el puente · simulación por cuadro: media {:.3} ms, peor {peor:.2} ms ({cuadros} cuadros)",
        c.agentes.e.vecinos.len(),
        c.agentes.e.agentes.len(),
        c.agentes.e.hora,
        n[0],
        n[1],
        n[2],
        n[3],
        st.viajes_hoy,
        st.omitidos_hoy,
        st.sin_calle_hoy,
        total / cuadros.max(1) as f64
    )
}

// --- Servicios, puerto, muralla, campo e iglesia (Fase 2 #5) ---------------------------------

use super::services::{self, Kind};

/// Busca un sitio libre para un servicio alrededor de `around` (hasta `max_r` m), arrimado a
/// una calle.
fn service_site(g: &super::game::Game, t: &crate::terrain::TerrainGen, k: Kind, around: Vec2, max_r: f32) -> Option<(Vec2, Vec2)> {
    let mut r = 0.0;
    while r <= max_r {
        let n = ((r / 10.0) as i32).clamp(1, 48);
        for a in 0..n {
            let p = around + Vec2::from_angle(a as f32 / n as f32 * std::f32::consts::TAU) * r;
            let (c, f) = g.city.service_site(k, p);
            if g.city.service_problem(t, k, c, f).is_none() {
                return Some((c, f));
            }
        }
        r += 12.0;
    }
    None
}

pub(crate) fn place(s: &mut Session, k: Kind, around: Vec2, max_r: f32) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let Some((c, f)) = service_site(g, &t, k, around, max_r) else {
        return format!("{}: sin sitio", services::info(k).name);
    };
    let r = g.city.place_service(&t, k, c, f);
    format!("{} en ({:.0}, {:.0}): {}", services::info(k).name, c.x, c.y, apply(s, r))
}

/// El punto con piedra más cercano a la plaza (a más de 350 m: la cantera es ruidosa), en tierra
/// firme y no muy empinada.
fn piedra_cercana(t: &crate::terrain::TerrainGen) -> Option<Vec2> {
    let rec = super::recursos::Recursos::de(t);
    for r in 0..24 {
        for a in 0..48 {
            let p = PLAZA + Vec2::from_angle(a as f32 / 48.0 * std::f32::consts::TAU) * (380.0 + r as f32 * 50.0);
            if t.height(p.x, p.y) > 2.0 && !t.is_water(p.x, p.y) && rec.en_area(p, Vec2::NEG_Y, 28.0, 22.0).piedra >= 0.35 {
                return Some(p);
            }
        }
    }
    None
}

/// Un camino de tierra de `from` hacia `dir`, de a tramos de 60 m mientras se pueda (girando un
/// poco si un tramo es muy empinado). Devuelve el punto final.
fn road_towards(s: &mut Session, from: Vec2, dir: Vec2, len: f32) -> Vec2 {
    let t = s.terrain.clone();
    let goal = dir.normalize_or(Vec2::X);
    let mut p = from;
    let mut d = goal;
    let mut done = 0.0;
    let mut e = crate::buildings::Edits::default();
    while done < len {
        let g = s.game.as_mut().unwrap();
        let mut ok = false;
        // Tramos de 30 m que esquivan lo empinado y vuelven de a poco al rumbo.
        let back = d.angle_to(goal).clamp(-0.3, 0.3);
        for turn in [back, 0.0, 0.3, -0.3, 0.6, -0.6, 0.9, -0.9, 1.2, -1.2] {
            let nd = d.rotate(Vec2::from_angle(turn));
            if nd.dot(goal) < 0.0 {
                continue;
            }
            let q = p + nd * 30.0;
            if let Ok(ed) = g.city.build_road(&t, &roads::line(g.snap(p), q), RoadKind::Dirt) {
                e.extend(ed);
                p = q;
                d = nd;
                ok = true;
                break;
            }
        }
        if !ok {
            break;
        }
        done += 30.0;
    }
    s.apply_edits(&e);
    p
}

/// Punto de la red de calles más alejado de la plaza hacia `dir`.
fn road_end(s: &Session, dir: Vec2) -> Vec2 {
    let g = s.game.as_ref().unwrap();
    g.city.net.nodes.values().copied().max_by(|a, b| (*a - PLAZA).dot(dir).total_cmp(&(*b - PLAZA).dot(dir))).unwrap_or(PLAZA)
}

/// Busca una orilla baja con agua profunda cerca de la plaza y, si hace falta, le lleva un
/// camino corto desde tierra adentro. Devuelve el arranque del muelle.
fn pier_site(s: &mut Session) -> Option<Vec2> {
    let t = s.terrain.clone();
    let mut spurs = 0;
    for r in 1..90 {
        for a in 0..48 {
            let p = PLAZA + Vec2::from_angle(a as f32 / 48.0 * std::f32::consts::TAU) * (r as f32 * 12.0);
            let h = t.height(p.x, p.y);
            if !(0.4..3.5).contains(&h) || t.is_water(p.x, p.y) {
                continue;
            }
            let Some((sea, _)) = services::port::find_sea(&t, p) else { continue };
            if s.game.as_ref().unwrap().city.pier_problem(&t, p).is_none() {
                return Some(p);
            }
            if spurs >= 6 {
                continue;
            }
            spurs += 1;
            // [ciudad] Primero, un camino desde la esquina más cercana de la ciudad: así el
            // muelle (y lo que se arrime a su camino, como el colector) queda en la red de
            // calles y no en un camino suelto.
            let cerca = s.game.as_ref().unwrap().city.net.nodes.values().copied().filter(|n| (25.0..240.0).contains(&n.distance(p))).min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)));
            if let Some(n) = cerca {
                let g = s.game.as_mut().unwrap();
                if let Ok(e) = g.city.build_road(&t, &roads::line(p + (n - p).normalize() * 8.0, n), RoadKind::Dirt) {
                    s.apply_edits(&e);
                    if s.game.as_ref().unwrap().city.pier_problem(&t, p).is_none() {
                        return Some(p);
                    }
                }
            }
            for len in [50.0, 90.0] {
                for turn in [0.0f32, 0.4, -0.4, 0.8, -0.8] {
                    let back = -sea.rotate(Vec2::from_angle(turn));
                    let g = s.game.as_mut().unwrap();
                    if let Ok(e) = g.city.build_road(&t, &roads::line(p + back * 8.0, p + back * len), RoadKind::Dirt) {
                        s.apply_edits(&e);
                        if s.game.as_ref().unwrap().city.pier_problem(&t, p).is_none() {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Fin del camino al campo (para las cámaras).
pub static CAMINO_END: std::sync::Mutex<Option<Vec2>> = std::sync::Mutex::new(None);

/// Servicios de la demostración, antes de hacer crecer la ciudad: agua (aljibe, pozos y
/// aguadores), serenos, el camino al campo con chacra, estancia, horno y cantera, el muelle con
/// la aduana, el lazareto en la costa y un tramo de muralla que cruza el camino.
pub fn servicios(s: &mut Session) -> String {
    use super::economy::Res;
    let mut out = Vec::new();
    {
        let g = s.game.as_mut().unwrap();
        g.city.money += 150_000;
        g.city.economy.stocks[Res::Ladrillos as usize].amount = 2500.0;
        g.city.economy.stocks[Res::Piedra as usize].amount = 2500.0;
    }
    let (f, side) = plaza_frame(0.0);
    out.push(place(s, Kind::Aljibe, PLAZA + side * 60.0, 260.0));
    out.push(place(s, Kind::Pozo, PLAZA - side * (BLOCK * 1.5) + f * BLOCK, 300.0));
    out.push(place(s, Kind::Pozo, PLAZA + side * (BLOCK * 1.5) + f * BLOCK, 300.0));
    out.push(place(s, Kind::Serenos, PLAZA - side * 50.0, 250.0));
    out.push(place(s, Kind::Aguadores, PLAZA - f * BLOCK, 500.0));
    // Camino al campo: sale del damero hacia el este, tierra adentro.
    let start = road_end(s, side);
    let end = road_towards(s, start, side - f * 0.1, 1200.0);
    *CAMINO_END.lock().unwrap() = Some(end);
    out.push(format!("camino al campo de {:.0} m", start.distance(end)));
    let along = |k: f32| start.lerp(end, k);
    out.push(place(s, Kind::Chacra, along(0.45), 260.0));
    out.push(place(s, Kind::Estancia, along(0.9), 300.0));
    // La cantera va sobre la piedra (capa «Piedra»): si el camino al campo no pasa por el
    // basamento, un camino de tierra hasta la piedra más cercana a la ciudad.
    let mut cantera = place(s, Kind::Cantera, along(0.65), 300.0);
    if cantera.contains("sin sitio") {
        if let Some(p) = piedra_cercana(&s.terrain.clone()) {
            let from = s.game.as_ref().unwrap().city.net.nodes.values().copied().min_by(|a, b| a.distance(p).total_cmp(&b.distance(p))).unwrap_or(PLAZA);
            let reached = road_towards(s, from, p - from, from.distance(p));
            cantera = place(s, Kind::Cantera, reached, 250.0);
        }
    }
    out.push(cantera);
    // Muelle y aduana.
    let t = s.terrain.clone();
    match pier_site(s) {
        Some(p) => {
            let g = s.game.as_mut().unwrap();
            let r = g.city.build_pier(&t, p);
            out.push(format!("muelle en ({:.0}, {:.0}): {}", p.x, p.y, apply(s, r)));
            out.push(place(s, Kind::Aduana, p, 450.0));
        }
        None => out.push("muelle: sin orilla".into()),
    }
    // Lazareto: lejos de la plaza, junto a la costa y a un camino (primero cerca del muelle).
    let near_pier = s.game.as_ref().unwrap().city.services.port.pier.as_ref().map_or(PLAZA, |p| p.land);
    let mut laz = place(s, Kind::Lazareto, near_pier, 500.0);
    if laz.contains("sin sitio") {
        laz = place(s, Kind::Lazareto, PLAZA, 1500.0);
    }
    if laz.contains("sin sitio") {
        // Un camino hasta la costa, a más de 500 m de la plaza.
        for a in 0..16 {
            let dir = Vec2::from_angle(a as f32 / 16.0 * std::f32::consts::TAU);
            let p = PLAZA + dir * 650.0;
            if !services::near_coast(&t, p, 140.0) || t.height(p.x, p.y) < 1.0 {
                continue;
            }
            let from = road_end(s, dir);
            let reached = road_towards(s, from, p - from, from.distance(p));
            if reached != from {
                laz = place(s, Kind::Lazareto, reached, 300.0);
                if !laz.contains("sin sitio") {
                    break;
                }
            }
        }
    }
    out.push(laz.clone());
    // El horno necesita barro: cerca del agua (el muelle o el lazareto, si no hay otro lugar).
    let mut horno = place(s, Kind::Horno, along(0.25), 400.0);
    let laz_at = s.game.as_ref().unwrap().city.services.of_kind(Kind::Lazareto).next().map_or(PLAZA, |b| b.c);
    for around in [near_pier, laz_at] {
        if horno.contains("sin sitio") {
            horno = place(s, Kind::Horno, around, 500.0);
        }
    }
    out.push(horno);
    // Tramo de muralla que cruza el camino al campo (con su portón).
    let cross = start.lerp(end, 0.08);
    let n = (end - start).normalize_or(Vec2::X).perp();
    let mut wall = "muralla: sin lugar".to_string();
    for half in [160.0, 120.0, 90.0, 60.0, 40.0] {
        let (a, b) = (cross - n * half, cross + n * half);
        let g = s.game.as_mut().unwrap();
        if g.city.wall_problem(&t, a, b).is_none() {
            let gates = g.city.wall_gates(a, b).len();
            let r = g.city.build_wall(&t, a, b);
            wall = format!("muralla de {:.0} m con {gates} portones: {}", half * 2.0, apply(s, r));
            break;
        }
    }
    out.push(wall);
    let g = s.game.as_ref().unwrap();
    format!("{} · {} servicios · agua para {} solares", out.join(" · "), g.city.services.buildings.len(), g.city.services.water.len())
}

/// Educación, salud, seguridad y saneamiento: escuela y convento, botica, médico y hospital de
/// caridad, garita del alcalde de barrio y cárcel del Cabildo cerca de la plaza, y el basurero
/// sobre el camino al campo; el cabildo paga al maestro y la asistencia a los pobres (la
/// recolección de basura queda sin aprobar: así se ven los basurales).
pub fn barrio(s: &mut Session) -> String {
    use super::economy::{Ordinance, Res};
    {
        let g = s.game.as_mut().unwrap();
        g.city.money += 60_000;
        g.city.economy.stocks[Res::Ladrillos as usize].amount += 1500.0;
        g.city.economy.stocks[Res::Piedra as usize].amount += 800.0;
    }
    let (f, side) = plaza_frame(0.0);
    let mut out = vec![
        place(s, Kind::Carcel, PLAZA + f * (BLOCK * 0.9), 300.0),
        place(s, Kind::Escuela, PLAZA - side * (BLOCK * 1.2) - f * (BLOCK * 0.6), 300.0),
        place(s, Kind::Botica, PLAZA + side * (BLOCK * 0.9) - f * (BLOCK * 0.3), 250.0),
        place(s, Kind::Alcalde, PLAZA - side * (BLOCK * 1.6) + f * (BLOCK * 1.5), 250.0),
        place(s, Kind::Alcalde, PLAZA + side * (BLOCK * 1.8) + f * (BLOCK * 1.6), 250.0),
        place(s, Kind::Medico, PLAZA + side * (BLOCK * 1.5) - f * (BLOCK * 1.6), 300.0),
        place(s, Kind::Hospital, PLAZA - f * (BLOCK * 2.2), 450.0),
        place(s, Kind::Convento, PLAZA + side * (BLOCK * 2.4) + f * (BLOCK * 0.4), 450.0),
    ];
    // El basurero, sobre el camino al campo, fuera de la ciudad.
    let end = (*CAMINO_END.lock().unwrap()).unwrap_or(PLAZA + side * 900.0);
    let dir = (end - PLAZA).normalize_or(side);
    out.push(place(s, Kind::Basurero, PLAZA + dir * 650.0, 400.0));
    let g = s.game.as_mut().unwrap();
    for o in [Ordinance::EscuelasPrimerasLetras, Ordinance::HospitalCaridad] {
        if !g.city.economy.is_on(o) {
            g.city.economy.active.push(o);
        }
    }
    g.city.refresh_barrio();
    let b = g.city.barrio_resumen();
    format!("{} · niños con escuela {}/{} · salud {:.0} % · delito {:.2}", out.join(" · "), b.niños.1, b.niños.0, b.salud * 100.0, b.delito)
}

/// Resumen de escuelas, salud, saneamiento y delito después de crecer.
pub fn barrio_informe(s: &mut Session) -> String {
    let g = s.game.as_ref().unwrap();
    let b = g.city.barrio_resumen();
    let p = &g.city.poblacion;
    format!(
        "niños con escuela {}/{} · saben leer {:.0} % · salud {:.0} % · insalubridad {:.0} % ({} basurales, {} pozos sucios, {} brotes) · delitos en el año {:?} ({} en total, {} presos)",
        b.niños.1,
        b.niños.0,
        p.stats.lectura * 100.0,
        b.salud * 100.0,
        b.insalubre * 100.0,
        b.basurales,
        b.pozos_sucios,
        b.brotes,
        b.delitos_año,
        g.city.services.barrio.delitos.total,
        b.presos
    )
}

/// Cámara sobre un basural (o, si no hay, sobre un rancho con su letrina).
pub fn view_basural(s: &Session) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let id = c.services.barrio.basurales.iter().copied().min_by_key(|id| c.lots.lots[id].c.distance(PLAZA) as i32).or_else(|| c.lots.lots.values().filter(|l| l.level == 1).min_by_key(|l| l.c.distance(PLAZA) as i32).map(|l| l.id))?;
    let l = &c.lots.lots[&id];
    let p = l.c + l.f * 6.0;
    Some([p.x, p.y, 34.0, l.f.x.atan2(l.f.y).to_degrees() + 200.0, 38.0])
}

/// La iglesia matriz: se financian las fases que la ciudad pueda pagar y se deja la siguiente
/// en obra (con andamios) para la captura. Si la ciudad todavía no llega a los 1.000
/// habitantes, la obra se fuerza para poder ver la fase (lo dice el informe).
pub fn iglesia(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    g.city.money += 60_000;
    let pop = g.city.population();
    let mut e = crate::buildings::Edits::default();
    let mut log = Vec::new();
    let forced = pop < services::works::CHURCH_MIN_POP;
    for _ in 0..2 {
        if forced {
            let ch = &mut g.city.services.church;
            if ch.building {
                ch.building = false;
                ch.phase += 1;
            }
            ch.building = true;
            ch.months_left = services::works::CHURCH[ch.phase as usize + 1].months;
            g.city.church_refresh(&t, &mut e);
        } else {
            match g.city.start_church_phase(&t) {
                Ok((ed, m)) => {
                    e.extend(ed);
                    log.push(m);
                    if g.city.services.church.phase == 0 {
                        let months = g.city.services.church.months_left;
                        e.extend(g.city.fast_forward(&t, months));
                    }
                }
                Err(m) => log.push(m),
            }
        }
    }
    let ch = &g.city.services.church;
    let state = format!("fase {} ({}){}", ch.phase, services::works::CHURCH[ch.phase as usize].name, if ch.building { format!(", en obra: {}", services::works::CHURCH[ch.phase as usize + 1].name) } else { String::new() });
    let forced_note = if forced { format!(" · obra forzada para la captura: la ciudad tiene {pop} habitantes (hacen falta {})", services::works::CHURCH_MIN_POP) } else { String::new() };
    let out = apply(s, Ok(e));
    format!("{state}{forced_note} · {} · {out}", log.join(" · "))
}

/// Donde empezó el incendio de la demostración.
pub static FIRE_AT: std::sync::Mutex<Option<Vec2>> = std::sync::Mutex::new(None);

/// Un incendio en un rancho sin agua cerca, con viento fuerte del oeste: el fuego salta a los
/// vecinos a favor del viento y quedan ruinas.
pub fn incendio(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    g.city.water.wind_dir = 270.0;
    g.city.water.wind_speed = 9.0;
    let c = &g.city;
    let built: Vec<_> = c.lots.lots.values().filter(|l| l.level == 1).collect();
    let crowd = |l: &&super::lots::Lot| built.iter().filter(|o| o.c.distance(l.c) < 16.0).count();
    let pick = built.iter().filter(|l| c.services.fire_source(l.c).is_none()).max_by_key(|l| crowd(l)).or_else(|| built.iter().max_by_key(|l| crowd(l))).map(|l| l.id);
    let Some(id) = pick else { return "no hay ranchos".into() };
    *FIRE_AT.lock().unwrap() = Some(g.city.lots.lots[&id].c);
    g.city.ignite(id);
    let mut e = crate::buildings::Edits::default();
    let mut news = Vec::new();
    for _ in 0..40 {
        news.extend(g.city.fire_step(&t, &mut e));
    }
    if let Some(n) = news.last() {
        g.say(n.clone());
    }
    let f = &g.city.services.fires;
    let line = format!("{} quemadas, {} apagados, {} ardiendo · {}", f.destroyed_total, f.put_out_total, f.burning.len(), news.join(" · "));
    let out = apply(s, Ok(e));
    format!("{line} · {out}")
}

/// Cámara sobre el primer servicio de un tipo (x, z, distancia, giro, inclinación).
pub fn view_of(s: &Session, k: Kind, dist: f32) -> Option<[f32; 5]> {
    let b = s.game.as_ref()?.city.services.of_kind(k).next()?;
    // Desde el frente (la calle), un poco de costado.
    Some([b.c.x, b.c.y, dist, b.f.x.atan2(b.f.y).to_degrees() + 30.0, 35.0])
}

pub fn view_pier(s: &Session) -> Option<[f32; 5]> {
    let p = s.game.as_ref()?.city.services.port.pier.as_ref()?;
    // Desde tierra, mirando el muelle hacia el río.
    let near = p.land + p.dir * 30.0;
    Some([near.x, near.y, 95.0, (-p.dir.x).atan2(-p.dir.y).to_degrees() + 35.0, 30.0])
}

pub fn view_wall(s: &Session) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let tr = c.services.walls.tramos.first()?;
    let g = c.wall_gates(tr.a, tr.b).first().map(|g| tr.a + (tr.b - tr.a).normalize() * g.0).unwrap_or((tr.a + tr.b) * 0.5);
    Some([g.x, g.y, 80.0, 230.0, 24.0])
}

pub fn view_church(s: &Session) -> Option<[f32; 5]> {
    let (c, f) = s.game.as_ref()?.city.church_site()?;
    Some([c.x, c.y, 100.0, f.x.atan2(f.y).to_degrees() + 30.0, 30.0])
}

pub fn view_fire(_s: &Session) -> Option<[f32; 5]> {
    let p = (*FIRE_AT.lock().unwrap())?;
    Some([p.x, p.y, 110.0, 200.0, 38.0])
}

pub fn view_lamps(s: &Session) -> Option<[f32; 5]> {
    let b = s.game.as_ref()?.city.services.of_kind(Kind::Serenos).next()?;
    Some([b.c.x, b.c.y, 120.0, 200.0, 30.0])
}

// --- Desarrollo: tecnologías, escuelas de oficios, industrias e inversores -------------------

/// Cinco años de desarrollo sobre la ciudad con historia: el cabildo invierte en las
/// tecnologías a medida que se pueden (escuela de oficios, curtido, horno, saladero, molino,
/// carpintería de ribera, náutica, imprenta), levanta las escuelas, dispone la exención, el
/// crédito de fomento y los maestros de ultramar, y deja que los inversores abran las
/// industrias. Si al final falta alguna industria para las capturas, la levanta el cabildo.
pub fn desarrollo(s: &mut Session) -> String {
    use super::desarrollo::{EstadoTec, Incentivo, Saber, TECNOLOGIAS};
    use super::economy::Res;
    {
        let g = s.game.as_mut().unwrap();
        g.city.money += 150_000;
        g.city.economy.stocks[Res::Ladrillos as usize].amount += 2500.0;
        g.city.economy.stocks[Res::Piedra as usize].amount += 1200.0;
        for i in [Incentivo::Exencion, Incentivo::Credito, Incentivo::Maestros] {
            let _ = g.city.desarrollo_alternar(i);
        }
    }
    let (f, side) = plaza_frame(0.0);
    let mut out = Vec::new();
    let mut iniciadas = Vec::new();
    let pier = s.game.as_ref().unwrap().city.services.port.pier.as_ref().map_or(PLAZA, |p| p.land);
    let camino = (*CAMINO_END.lock().unwrap()).unwrap_or(PLAZA + side * 900.0);
    for mes in 0..60 {
        // Invertir en todo lo que ya se puede.
        {
            let g = s.game.as_mut().unwrap();
            let ctx = g.city.desarrollo_contexto();
            let disp: Vec<_> = TECNOLOGIAS.iter().filter(|t| g.city.desarrollo.estado(t.id, &ctx) == EstadoTec::Disponible).map(|t| t.id).collect();
            for t in disp {
                if g.city.desarrollo_iniciar(t).is_ok() {
                    iniciadas.push(format!("{t:?} (mes {mes})"));
                }
            }
            g.city.economy.stocks[Res::Ladrillos as usize].amount = g.city.economy.stocks[Res::Ladrillos as usize].amount.max(800.0);
        }
        // Las escuelas, en cuanto se habilitan.
        for (k, around) in [(Kind::EscuelaOficios, PLAZA - side * (BLOCK * 2.2) + f * (BLOCK * 0.8)), (Kind::EscuelaNautica, pier)] {
            let g = s.game.as_ref().unwrap();
            if g.city.desarrollo.habilitado(k) && g.city.services.count(k) == 0 {
                out.push(place(s, k, around, 600.0));
            }
        }
        let t = s.terrain.clone();
        let e = s.game.as_mut().unwrap().city.fast_forward(&t, 1);
        s.apply_edits(&e);
    }
    // Lo que no abrieron los inversores lo levanta el cabildo (para verlo en las capturas).
    for (k, around, r) in [(Kind::Saladero, camino, 1500.0), (Kind::Molino, PLAZA + side * 420.0, 900.0), (Kind::Astillero, pier, 900.0), (Kind::Imprenta, PLAZA + f * (BLOCK * 1.2), 450.0)] {
        let g = s.game.as_ref().unwrap();
        if g.city.desarrollo.habilitado(k) && g.city.services.count(k) == 0 {
            out.push(format!("(cabildo) {}", place(s, k, around, r)));
        }
    }
    let t = s.terrain.clone();
    let e = s.game.as_mut().unwrap().city.fast_forward(&t, 6);
    s.apply_edits(&e);
    let g = s.game.as_ref().unwrap();
    let d = &g.city.desarrollo;
    let est: Vec<String> = d.establecimientos.iter().map(|e| format!("{:?} {:?} {} peones {:.0} %", e.ind, e.dueno, e.empleados.len(), e.marcha * 100.0)).collect();
    let ult = d.historia.back();
    format!(
        "{} · tecnologías logradas {:?} · iniciadas {:?} · leen {} · artesanos {} · oficiales {} · pilotos {} · establecimientos [{}] · exportaciones {:.0} pesos/mes · concentración {:.2} · inversión privada {:.0} · {}",
        g.city.date(),
        d.hechas,
        iniciadas,
        d.saber(Saber::Alfabetizados),
        d.saber(Saber::Artesanos),
        d.saber(Saber::Oficiales),
        d.saber(Saber::Pilotos),
        est.join("; "),
        ult.map_or(0.0, |f| f.exportaciones()),
        d.concentracion(),
        d.inversion_total,
        out.join(" · ")
    )
}

// --- Materiales de construcción (ver `city::materiales`) ------------------------------------

/// La hilera de casas de muestra: centro, frente y solares (de un extremo al otro).
pub static MUESTRA: std::sync::Mutex<Option<(Vec2, Vec2, Vec<u32>)>> = std::sync::Mutex::new(None);

/// Una cuadra levantada durante una falta de cal y de ladrillos: seis casas vecinas que se
/// rehacen una tras otra, cada una con lo que había en el depósito en ese momento (de todo;
/// sin cal; sin ladrillos; solo madera; nada; solo piedra y cal). Usa el mismo camino que el
/// juego (`construir_obra`): elige, sustituye y gasta. Después el depósito vuelve a lo que era.
pub fn materiales(s: &mut Session) -> String {
    use super::lots::ZoneKind;
    use super::materiales::Material as M;
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    let plaza = c.lots.plaza_center.unwrap_or(PLAZA);
    // La hilera más larga de casas de una planta con el mismo frente, cerca de la plaza.
    let casas: Vec<(u32, Vec2, Vec2)> = c.lots.lots.values().filter(|l| l.kind() == Some(ZoneKind::Residencial) && l.level == 2).map(|l| (l.id, l.c, l.f)).collect();
    let mut mejor: Option<(usize, f32, Vec<(f32, u32)>, Vec2)> = None;
    for &(_, p, f) in &casas {
        let side = Vec2::new(f.y, -f.x);
        let mut fila: Vec<(f32, u32)> = casas.iter().filter(|o| o.2.dot(f) > 0.99 && (o.1 - p).dot(f).abs() < 1.0 && (o.1 - p).dot(side).abs() < 38.0).map(|o| ((o.1 - p).dot(side), o.0)).collect();
        fila.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Solo solares contiguos (cada 12 m).
        let k = fila.iter().position(|x| x.0.abs() < 0.5).unwrap_or(0);
        let mut a = k;
        while a > 0 && fila[a].0 - fila[a - 1].0 < 13.0 {
            a -= 1;
        }
        let mut b = k;
        while b + 1 < fila.len() && fila[b + 1].0 - fila[b].0 < 13.0 {
            b += 1;
        }
        let fila: Vec<(f32, u32)> = fila[a..=b].to_vec();
        let d = p.distance(plaza);
        let n = fila.len().min(6);
        if mejor.as_ref().is_none_or(|m| n > m.0 || (n == m.0 && d < m.1)) {
            mejor = Some((n, d, fila, f));
        }
    }
    let Some((_, _, fila, f)) = mejor else { return "no hay casas de una planta".into() };
    let ids: Vec<u32> = fila.iter().take(6).map(|x| x.1).collect();
    // Lo que había en el depósito al levantar cada casa.
    let escenarios: [(&str, &[(M, f32)]); 6] = [
        ("de todo", &[(M::Ladrillo, 400.0), (M::Cal, 60.0), (M::Teja, 80.0), (M::Madera, 60.0), (M::Arena, 40.0), (M::Hierro, 6.0)]),
        ("sin cal", &[(M::Ladrillo, 400.0), (M::Teja, 80.0), (M::Madera, 60.0), (M::Arena, 40.0)]),
        ("sin ladrillos", &[(M::Adobe, 200.0), (M::Teja, 80.0), (M::Madera, 60.0)]),
        ("solo madera", &[(M::Madera, 80.0)]),
        ("nada", &[]),
        ("piedra y cal", &[(M::Piedra, 400.0), (M::Cal, 60.0), (M::Teja, 80.0), (M::Madera, 60.0), (M::Arena, 40.0), (M::Hierro, 6.0)]),
    ];
    let guardado = (c.economy.stocks.clone(), c.materiales.ex.clone());
    let mut out = Vec::new();
    for (&id, (nombre, hay)) in ids.iter().zip(escenarios) {
        for m in M::TODOS {
            let q = hay.iter().find(|x| x.0 == m).map_or(0.0, |x| x.1);
            match m.de_la_economia() {
                Some(r) => c.economy.stocks[r as usize].amount = q,
                None => c.materiales.ex[m.idx()].cantidad = q,
            }
        }
        let l = &c.lots.lots[&id];
        let Some(plan) = c.plan_de_solar(l, l.level) else { continue };
        let nivel = c.lots.lots[&id].level;
        let o = c.obra_de_solar(id, nivel, &plan);
        c.lots.lots.get_mut(&id).unwrap().obra = Some(o);
        out.push(format!("{nombre}: {}", o.nombre().to_lowercase()));
    }
    (c.economy.stocks, c.materiales.ex) = guardado;
    let mut e = crate::buildings::Edits::default();
    for &id in &ids {
        c.revoxelizar(&t, id, &mut e);
    }
    let centro = ids.iter().map(|id| c.lots.lots[id].c).sum::<Vec2>() / ids.len().max(1) as f32;
    *MUESTRA.lock().unwrap() = Some((centro, f, ids));
    let r = apply(s, Ok(e));
    format!("{} · {r}", out.join(" · "))
}

/// Cámara de frente a la hilera de muestra, desde la calle.
pub fn view_materiales(_s: &Session) -> Option<[f32; 5]> {
    let (c, f, _) = MUESTRA.lock().unwrap().clone()?;
    let p = c + f * 6.0;
    Some([p.x, p.y, 56.0, f.x.atan2(f.y).to_degrees() + 12.0, 34.0])
}

/// Ladrillos y piedra guardados mientras se muestra una obra sin ellos.
static LADRILLOS: std::sync::Mutex<Option<(f32, f32)>> = std::sync::Mutex::new(None);

/// Deja la ciudad sin ladrillos ni piedra (para la captura de la tarjeta que sustituye).
pub fn guardar_ladrillos(s: &mut Session) {
    use super::economy::Res;
    let eco = &mut s.game.as_mut().unwrap().city.economy;
    *LADRILLOS.lock().unwrap() = Some((eco.stock(Res::Ladrillos).amount, eco.stock(Res::Piedra).amount));
    eco.stocks[Res::Ladrillos as usize].amount = 0.0;
    eco.stocks[Res::Piedra as usize].amount = 0.0;
}

/// Devuelve los ladrillos y la piedra guardados.
pub fn devolver_ladrillos(s: &mut Session) -> String {
    use super::economy::Res;
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    g.inspected = None;
    let eco = &mut g.city.economy;
    match LADRILLOS.lock().unwrap().take() {
        Some((l, p)) => {
            eco.stocks[Res::Ladrillos as usize].amount = l;
            eco.stocks[Res::Piedra as usize].amount = p;
            format!("{l} ladrillos y {p} de piedra de vuelta")
        }
        None => "nada que devolver".into(),
    }
}

// --- Arquitectura: calles de muestra de cada época (ver `city::arquitectura`) ---------------

/// Calles de muestra: (nombre, centro de la hilera, frente, solares).
pub type MuestraArq = (&'static str, Vec2, Vec2, Vec<u32>);
pub static MUESTRAS_ARQ: std::sync::Mutex<Vec<MuestraArq>> = std::sync::Mutex::new(Vec::new());

/// Hileras de solares contiguos (cada 12 m) con el mismo frente, de al menos `n` solares,
/// ordenadas por distancia a la plaza.
pub(crate) fn hileras(c: &super::City, n: usize) -> Vec<Vec<u32>> {
    let plaza = c.lots.plaza_center.unwrap_or(PLAZA);
    let mut vistos = std::collections::HashSet::new();
    let mut out: Vec<(f32, Vec<u32>)> = Vec::new();
    let mut ids: Vec<u32> = c.lots.lots.keys().copied().collect();
    ids.sort();
    for id in ids {
        if vistos.contains(&id) {
            continue;
        }
        let l = &c.lots.lots[&id];
        let side = Vec2::new(-l.f.y, l.f.x);
        let mut fila: Vec<(f32, u32)> = c.lots.lots.values().filter(|o| !vistos.contains(&o.id) && o.f.dot(l.f) > 0.995 && (o.c - l.c).dot(l.f).abs() < 1.0 && (o.c - l.c).dot(side).abs() < 200.0).map(|o| ((o.c - l.c).dot(side), o.id)).collect();
        fila.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut run: Vec<(f32, u32)> = Vec::new();
        let mut flush = |run: &mut Vec<(f32, u32)>| {
            if run.len() >= n {
                let m = run.iter().map(|x| c.lots.lots[&x.1].c).sum::<Vec2>() / run.len() as f32;
                out.push((m.distance(plaza), run.iter().map(|x| x.1).collect()));
            }
            run.clear();
        };
        for x in fila {
            vistos.insert(x.1);
            if run.last().is_some_and(|p| x.0 - p.0 > 13.0) {
                flush(&mut run);
            }
            run.push(x);
        }
        flush(&mut run);
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|x| x.1).collect()
}

/// Mide el dibujo de todos los edificios de los solares: cuántos, cuántos vóxeles y cuánto
/// tarda (en un hilo).
pub fn medir_edificios(c: &super::City, t: &crate::terrain::TerrainGen) -> (usize, usize, std::time::Duration) {
    let t0 = Instant::now();
    let (mut n, mut v) = (0, 0);
    for l in c.lots.lots.values().filter(|l| l.level > 0) {
        let Some((k, d)) = super::lots::zone_of(l.zone) else { continue };
        let b = super::build_lot(t, l.c, l.f, l.seed, k, d, l.level, l.obra);
        n += 1;
        v += b.voxels.len();
    }
    (n, v, t0.elapsed())
}

/// Medida de los edificios de los solares (para comparar antes y después).
pub fn medir(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let c = &s.game.as_ref().unwrap().city;
    let (n, v, d) = medir_edificios(c, &t);
    format!("{n} edificios de solares: {v} vóxeles, {:.0} ms en un hilo ({:.0} vóxeles y {:.2} ms por edificio)", d.as_secs_f64() * 1000.0, v as f64 / n.max(1) as f64, d.as_secs_f64() * 1000.0 / n.max(1) as f64)
}

/// Seis calles de muestra con casas de distintas épocas, clases y familias: ranchos de 1732
/// en las orillas, casas de teja de 1760, azoteas de 1805, altos de 1822, comercio y
/// pulperías de 1795 y quintas de 1812 extramuros. Cada una con sus materiales reales.
pub fn arquitectura(s: &mut Session) -> String {
    use super::lots::{zone_code, Density, ZoneKind};
    use super::materiales::{Obra, Pared, Techo};
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    type Muestra = (&'static str, ZoneKind, Density, u8, u16, u8, [(Pared, Techo, bool, bool); 3]);
    let muestras: [Muestra; 6] = [
        ("comercio y pulperías de 1795", ZoneKind::Comercial, Density::Media, 2, 1795, 1, [(Pared::Ladrillo, Techo::Azotea, true, true), (Pared::Piedra, Techo::Teja, true, true), (Pared::Ladrillo, Techo::Teja, true, true)]),
        ("ranchos de 1732 en las orillas", ZoneKind::Residencial, Density::Baja, 1, 1732, 3, [(Pared::Adobe, Techo::Paja, false, false), (Pared::Barro, Techo::Cuero, false, false), (Pared::Barro, Techo::Paja, false, false)]),
        ("casas de teja de 1760", ZoneKind::Residencial, Density::Media, 2, 1760, 1, [(Pared::Piedra, Techo::Teja, true, true), (Pared::Ladrillo, Techo::Teja, true, true), (Pared::Adobe, Techo::Teja, true, false)]),
        ("casas de azotea de 1805", ZoneKind::Residencial, Density::Media, 2, 1805, 1, [(Pared::Ladrillo, Techo::Azotea, true, true), (Pared::Ladrillo, Techo::Azotea, true, true), (Pared::Piedra, Techo::Teja, true, true)]),
        ("casas de altos de 1822", ZoneKind::Residencial, Density::Media, 3, 1822, 0, [(Pared::Piedra, Techo::Azotea, true, true), (Pared::Ladrillo, Techo::Azotea, true, true), (Pared::Ladrillo, Techo::Azotea, true, true)]),
        ("quintas de 1812 extramuros", ZoneKind::Residencial, Density::Baja, 2, 1812, 3, [(Pared::Ladrillo, Techo::Teja, true, true), (Pared::Piedra, Techo::Azotea, true, true), (Pared::Adobe, Techo::Teja, true, false)]),
    ];
    let mut filas = hileras(c, 6);
    // Para las capturas, las hileras con la fachada al sol de la mañana (ver
    // `render::lighting`: a las 10 el sol viene de (0,5; −0,42) en el plano).
    let sol = Vec2::new(0.5, -0.42).normalize();
    let al_sol: Vec<Vec<u32>> = filas.iter().filter(|f| c.lots.lots[&f[0]].f.dot(sol) > 0.2).cloned().collect();
    if al_sol.len() >= 6 {
        filas = al_sol;
    }
    if filas.len() < muestras.len() {
        return format!("faltan hileras de solares ({} de {})", filas.len(), muestras.len());
    }
    // Cada muestra en su hilera: las del centro para los altos y el comercio, las de afuera
    // para ranchos y quintas.
    let orden = [1, filas.len() - 1, filas.len() / 2, 2, 0, filas.len() - 2];
    let mut usadas = std::collections::HashSet::new();
    let mut e = crate::buildings::Edits::default();
    let mut lista = Vec::new();
    let mut nombres = std::collections::BTreeMap::<String, usize>::new();
    for (k, (nombre, kind, dens, nivel, anio, lugar, mats)) in muestras.into_iter().enumerate() {
        let mut i = orden[k].min(filas.len() - 1);
        if kind == ZoneKind::Comercial {
            // El comercio, en la hilera con más esquinas (las pulperías van en la esquina).
            let esquinas = |f: &Vec<u32>| f.iter().take(8).filter(|id| c.esquina_de(&c.lots.lots[id]) > 0).count();
            if let Some(j) = (0..filas.len()).filter(|j| !usadas.contains(j)).max_by_key(|&j| (esquinas(&filas[j]), usize::MAX - j)) {
                i = j;
            }
        }
        while usadas.contains(&i) {
            i = (i + 1) % filas.len();
        }
        usadas.insert(i);
        let ids: Vec<u32> = filas[i].iter().take(8).copied().collect();
        for (j, &id) in ids.iter().enumerate() {
            let seed = c.lots.lots[&id].seed;
            let (p, te, rev, hierro) = mats[[0, 1, 0, 2, 1, 0, 2, 1][j % 8]];
            {
                let l = c.lots.lots.get_mut(&id).unwrap();
                l.zone = zone_code(kind, dens);
                l.level = nivel;
            }
            let mut o = Obra::nueva(p, te, rev, hierro);
            o.contexto = c.contexto_solar(id, nivel);
            o.contexto.anio = anio;
            o.contexto.lugar = lugar;
            o.contexto.clase = match nivel {
                1 => 0,
                2 => [1, 1, 2, 0, 1, 2, 1, 3][j % 8],
                _ => [2, 3, 2, 3, 2, 2, 3, 2][j % 8],
            };
            let origen = super::arquitectura::origen_probable(anio, seed ^ j as u64);
            o.contexto.tradicion = super::arquitectura::tradicion_de(origen).indice();
            c.elegir_giro(id, nivel, &mut o);
            c.lots.lots.get_mut(&id).unwrap().obra = Some(o);
            if let Some(linea) = c.lineas_estilo(&c.lots.lots[&id]).into_iter().next() {
                let tipo = linea.split(',').next().unwrap_or("").split(" con ").next().unwrap_or("").to_string();
                *nombres.entry(tipo).or_default() += 1;
            }
            c.revoxelizar(&t, id, &mut e);
        }
        let centro = ids.iter().map(|id| c.lots.lots[id].c).sum::<Vec2>() / ids.len() as f32;
        lista.push((nombre, centro, c.lots.lots[&ids[0]].f, ids));
    }
    let esquinas = lista.iter().flat_map(|m| m.3.iter()).filter(|id| c.lots.lots[id].obra.is_some_and(|o| o.contexto.esquina > 0)).count();
    *MUESTRAS_ARQ.lock().unwrap() = lista;
    let tipos = nombres.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", ");
    // [ciudad] Modelos de detalle de las fachadas (rejas, faroles, macetas, llamadores).
    let det = c.fachadas.cuenta(&c.lots).iter().map(|(k, v)| format!("{k:?} {v}")).collect::<Vec<_>>().join(", ");
    let r = apply(s, Ok(e));
    format!("tipos: {tipos} · {esquinas} esquinas · detalles de fachada: {det} · {r}")
}

/// Cámara sobre la esquina de la calle de comercio (la pulpería), en diagonal a las dos calles.
pub fn view_esquina(s: &Session) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let m = MUESTRAS_ARQ.lock().unwrap();
    let todos = [0usize, 3, 2, 4, 5, 1];
    let l = todos.iter().filter_map(|&k| m.get(k)).flat_map(|x| x.3.iter()).filter_map(|id| c.lots.lots.get(id)).find(|l| l.obra.is_some_and(|o| o.contexto.esquina > 0))?;
    let esq = l.obra?.contexto.esquina;
    let u = Vec2::new(-l.f.y, l.f.x) * if esq == 1 { -1.0 } else { 1.0 };
    let p = l.c + l.f * (super::lots::D * 0.5) + u * (super::lots::W * 0.5);
    let mira = (l.f + u).normalize();
    Some([p.x, p.y, 34.0, mira.x.atan2(mira.y).to_degrees(), 26.0])
}

/// Cámara sobre la calle de muestra `k`, mirando las fachadas en diagonal (`hacia` m desde el
/// centro de los solares hacia la calle: 24 es el eje de la calle).
pub fn view_arquitectura(k: usize, hacia: f32, lejos: f32, giro: f32, pitch: f32) -> Option<[f32; 5]> {
    let m = MUESTRAS_ARQ.lock().unwrap();
    let (_, c, f, _) = m.get(k)?;
    let p = *c + *f * hacia;
    Some([p.x, p.y, lejos, f.x.atan2(f.y).to_degrees() + giro, pitch])
}

// [personajes] --- Vistas de cerca de los personajes, los animales y los detalles ------------

/// Cámara sobre el agente de `tipo` más cercano a la plaza (de frente y de costado, a `dist`
/// m, girada `giro` grados desde su rumbo).
pub fn view_agente(s: &Session, tipo: super::agentes::Tipo, dist: f32, giro: f32, pitch: f32) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let plaza = c.plaza.map_or(PLAZA, |p| p.0);
    let (p, d) = c
        .agentes
        .e
        .agentes
        .iter()
        .filter(|a| a.tipo == tipo)
        .map(|a| super::agentes::pose(&c.net, a))
        .min_by(|x, y| x.0.distance(plaza).total_cmp(&y.0.distance(plaza)))?;
    // El centro del vehículo queda medio largo detrás del frente.
    let m = p - d * tipo.params().largo * 0.5;
    Some([m.x, m.y, dist, d.x.atan2(d.y).to_degrees() + giro, pitch])
}

/// Cámara sobre la pastura de la primera estancia (con su rodeo).
pub fn view_pastura(s: &Session, dist: f32) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let b = c.services.of_kind(Kind::Estancia).next()?;
    let (m, _) = super::agentes::fauna::pastura_de(b.c, b.f);
    Some([m.x, m.y, dist, b.f.x.atan2(b.f.y).to_degrees() + 200.0, 22.0])
}

/// Cámara sobre la muestra de detalles armada por [`muestra_detalles`], en la fracción `u`
/// de la fila, desde el costado.
pub fn view_muestra(s: &Session, u: f32, dist: f32, giro: f32) -> Option<[f32; 5]> {
    let (p0, dir, largo) = s.game.as_ref()?.city.agentes.escena_muestra()?;
    let p = p0 + dir * largo * u;
    let lado = super::lots::side_of(dir);
    Some([p.x, p.y, dist, lado.x.atan2(lado.y).to_degrees() + giro, 14.0])
}

/// Cámara sobre el grupo de gente a pie más numeroso (a menos de 8 m unos de otros).
pub fn view_grupo(s: &Session, dist: f32) -> Option<[f32; 5]> {
    use super::agentes::Tipo;
    let c = &s.game.as_ref()?.city;
    let ps: Vec<(Vec2, Vec2)> = c.agentes.e.agentes.iter().filter(|a| a.tipo == Tipo::APie).map(|a| super::agentes::pose(&c.net, a)).collect();
    let (p, d) = ps.iter().max_by_key(|(p, _)| (ps.iter().filter(|(q, _)| q.distance(*p) < 8.0).count(), (p.x * 10.0) as i64))?;
    Some([p.x, p.y, dist, d.x.atan2(d.y).to_degrees() + 140.0, 12.0])
}

/// Cámara sobre una pulpería con caballos en el palenque.
pub fn view_pulperia(s: &Session) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let plaza = c.plaza.map_or(PLAZA, |p| p.0);
    let l = c
        .lots
        .lots
        .values()
        .filter(|l| l.kind() == Some(super::lots::ZoneKind::Comercial) && l.level == 1 && !l.voxels.is_empty())
        .min_by(|a, b| a.c.distance(plaza).total_cmp(&b.c.distance(plaza)))?;
    let p = l.c + l.f * (super::lots::D * 0.5 + 2.0);
    Some([p.x, p.y, 13.0, l.f.x.atan2(l.f.y).to_degrees() + 35.0, 16.0])
}

/// Pone la fila de modelos de detalle de muestra junto a la plaza.
pub fn muestra_detalles(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    let Some((plaza, _)) = c.plaza else { return "sin plaza".into() };
    let n = c.agentes.poner_muestra(&t, &c.net, plaza);
    format!("{n} modelos de detalle de muestra")
}

/// Larga una carreta de bueyes, un carro y un jinete por una calle larga cerca de la plaza y
/// los deja andar hasta que la carreta hizo unos metros (para las capturas de cerca).
pub fn traer_vehiculos(s: &mut Session) -> String {
    use super::agentes::{Proposito, Tipo};
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    let plaza = c.plaza.map_or(PLAZA, |p| p.0);
    let comp = super::agentes::componente(&c.net, match c.net.nodes.iter().min_by(|x, y| x.1.distance(plaza).total_cmp(&y.1.distance(plaza))) {
        Some((&n, _)) => n,
        None => return "sin calles".into(),
    });
    let mut nodos: Vec<Vec2> = comp.iter().map(|n| c.net.nodes[n]).collect();
    nodos.sort_by(|a, b| a.distance(plaza).total_cmp(&b.distance(plaza)).then(a.x.total_cmp(&b.x)));
    let Some(&desde) = nodos.get(1) else { return "sin calles".into() };
    let Some(&hasta) = nodos.iter().find(|p| p.distance(desde) > 220.0) else { return "sin calles".into() };
    let dt = 1.0 / 30.0;
    let mut n = 0;
    for (k, tipo) in [Tipo::Carreta, Tipo::Carro, Tipo::Jinete].into_iter().enumerate() {
        if c.agentes.lanzar(&c.net, tipo, desde, hasta, None, Proposito::Paseo, 90 + k as u32, 1, (k as u8 + 3, k as u8 + 5)) {
            n += 1;
        }
        for _ in 0..40 {
            c.agentes.tick(&t, &c.net, &c.lots, c.plaza, dt);
        }
    }
    format!("{n} vehículos de paseo desde ({:.0}, {:.0})", desde.x, desde.y)
}
// [/personajes]
