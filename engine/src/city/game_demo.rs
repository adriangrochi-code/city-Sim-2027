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
