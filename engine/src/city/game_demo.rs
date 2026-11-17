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

fn apply(s: &mut Session, r: Result<crate::buildings::Edits, String>) -> String {
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
