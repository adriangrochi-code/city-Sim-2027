//! [ia] Escenas del recorrido de la IA (`MOTOR_SOLO_IA=1 motor --demo-ciudad carpeta`) para los
//! oficios y sucesos de la colonia (`agentes::oficios`): el aguador en una puerta, el sereno
//! que enciende un farol, la cadena de baldes de un incendio, la salida de misa, la gente
//! sentada en la pulpería y en la vereda, el boyero con la picana, los postigos y el poyo de
//! una casa, y una calle en pendiente sin y con las rampas de los escalones.

use super::agentes::{self, Marcha, Proposito, Tipo};
use super::game_demo::PLAZA;
use crate::session::Session;
use glam::Vec2;
use std::sync::Mutex;

/// Lo que mira la cámara en cada escena (lo deja cada paso, lo lee la vista).
static MIRA: Mutex<Option<[f32; 5]>> = Mutex::new(None);

fn mirar(p: Vec2, dist: f32, yaw: f32, pitch: f32) {
    *MIRA.lock().unwrap() = Some([p.x, p.y, dist, yaw, pitch]);
}

/// La luz del cuadro a la hora de la escena (el reloj de la sesión manda sobre los agentes);
/// sin el cursor ni la ficha de las capturas anteriores (su resaltado taparía la escena).
fn a_la_hora(s: &mut Session) {
    s.hour = s.game.as_ref().unwrap().city.agentes.e.hora;
    sin_cursor(s);
}

fn sin_cursor(s: &mut Session) {
    s.hit = None;
    let g = s.game.as_mut().unwrap();
    g.hover = None;
    g.inspected = None;
}

/// La cámara de la última escena preparada.
pub fn vista(_s: &Session) -> Option<[f32; 5]> {
    *MIRA.lock().unwrap()
}

/// Corre los agentes `n` cuadros de 1/30 s desde la hora `h0` (que avanza como en la ventana)
/// hasta que `listo` dice que sí. Devuelve la hora a la que paró.
fn correr(s: &mut Session, h0: f32, n: usize, mut listo: impl FnMut(&super::City) -> bool) -> f32 {
    let t = s.terrain.clone();
    let c = &mut s.game.as_mut().unwrap().city;
    let dt = 1.0 / 30.0;
    let mut h = h0;
    for _ in 0..n {
        c.agentes.set_hora(h % 24.0);
        c.tick_agentes(&t, dt);
        if listo(c) {
            break;
        }
        h += dt * super::HOURS_PER_SECOND;
    }
    h
}

/// Un día que cumple `cond` (del día de la semana) después del de hoy.
fn ir_al_dia(s: &mut Session, cond: impl Fn(u32) -> bool) {
    let c = &mut s.game.as_mut().unwrap().city;
    let mut d = c.agentes.e.dia + 1;
    while !cond(d) {
        d += 1;
    }
    c.agentes.e.dia = d;
    // Desde la medianoche (si no, al poner la hora de la escena el reloj pasaría otro día).
    c.agentes.e.hora = 0.0;
}

fn yaw_de(dir: Vec2) -> f32 {
    dir.x.atan2(dir.y).to_degrees()
}

/// El aguador: el primer carro del día, hasta que para en una puerta.
pub fn aguador(s: &mut Session) -> String {
    ir_al_dia(s, |d| d % 7 != 6);
    let foco = Vec2::new(s.cam.target.x, s.cam.target.z);
    s.game.as_mut().unwrap().city.agentes.foco = Some(foco);
    correr(s, 6.9, 30, |_| false);
    let c = &s.game.as_ref().unwrap().city;
    let Some(a) = c.agentes.e.agentes.iter().find(|a| a.proposito == Proposito::Aguador) else {
        return format!("sin aguadores ({} viajes hoy)", c.agentes.e.stats.aguadores_hoy);
    };
    let id = a.id;
    let casas0 = c.agentes.e.stats.casas_con_agua_hoy;
    correr(s, 7.0, 9000, |c| c.agentes.e.agentes.iter().find(|a| a.id == id).is_none_or(|a| a.marcha == Marcha::DejaAgua && c.agentes.e.stats.casas_con_agua_hoy > casas0));
    a_la_hora(s);
    let c = &s.game.as_ref().unwrap().city;
    let Some(a) = c.agentes.e.agentes.iter().find(|a| a.id == id) else { return "el aguador volvió antes de verlo".into() };
    let (p, d) = agentes::pose(&c.net, a);
    mirar(p - d * 1.5, 10.0, yaw_de(d) + 125.0, 22.0);
    let st = c.agentes.e.stats;
    format!("aguador parado en una puerta: le quedan {} paradas · {} viajes y agua a {} casas en el día", a.paradas.len(), st.aguadores_hoy, st.casas_con_agua_hoy)
}

/// El sereno al anochecer, parado en un farol.
pub fn sereno(s: &mut Session) -> String {
    let h = correr(s, 19.3, 1800, |c| c.agentes.e.agentes.iter().any(|a| a.proposito == Proposito::EnciendeFaroles));
    let c = &s.game.as_ref().unwrap().city;
    let Some(id) = c.agentes.e.agentes.iter().find(|a| a.proposito == Proposito::EnciendeFaroles).map(|a| a.id) else {
        return "sin serenos (¿hay puesto de serenos con faroles?)".into();
    };
    let mut paradas = 0;
    correr(s, h, 9000, |c| {
        let Some(a) = c.agentes.e.agentes.iter().find(|a| a.id == id) else { return true };
        if a.marcha == Marcha::CantaLaHora {
            paradas += 1;
        }
        paradas > 40
    });
    a_la_hora(s);
    let c = &s.game.as_ref().unwrap().city;
    let Some(a) = c.agentes.e.agentes.iter().find(|a| a.id == id) else { return "el sereno terminó la ronda antes de verlo".into() };
    let (p, d) = agentes::pose(&c.net, a);
    mirar(p, 6.5, yaw_de(d) + 150.0, 18.0);
    format!("sereno: {} · {} faroles encendidos", a.marcha.texto(), c.agentes.e.stats.faroles_hoy)
}

/// Lote del incendio de la escena (para apagarlo después).
static INCENDIO: Mutex<Option<u32>> = Mutex::new(None);

/// Un incendio: la casa con un pozo o un aljibe más cerca se prende; los vecinos corren a
/// mirar y se forma la cadena de baldes.
pub fn incendio(s: &mut Session) -> String {
    use super::services::Kind;
    ir_al_dia(s, |d| d % 7 != 6);
    let c = &mut s.game.as_mut().unwrap().city;
    let fuentes: Vec<Vec2> = c.services.buildings.values().filter(|b| matches!(b.kind, Kind::Pozo | Kind::Aljibe)).map(|b| b.c).collect();
    let Some((lote, _)) = c.lots.lots.values().filter(|l| l.level > 0).filter_map(|l| fuentes.iter().map(|f| f.distance(l.c)).filter(|d| *d > 18.0).min_by(f32::total_cmp).map(|d| (l.id, d))).min_by(|a, b| a.1.total_cmp(&b.1)) else {
        return "no hay casas cerca de un pozo".into();
    };
    c.services.fires.burning.insert(lote, super::services::fires::Burn { i: 0.5, full: 0, fought: true });
    *INCENDIO.lock().unwrap() = Some(lote);
    correr(s, 15.0, 2400, |c| c.agentes.e.agentes.iter().filter(|a| a.marcha == Marcha::MiraElFuego).count() >= 4);
    a_la_hora(s);
    let c = &s.game.as_ref().unwrap().city;
    let Some(inc) = c.agentes.e.incendios.iter().find(|i| i.lote == lote) else { return "el incendio no se registró".into() };
    let pos = Vec2::from(inc.pos);
    let f = inc.fuente.map(Vec2::from).unwrap_or(pos);
    let medio = pos.lerp(f, 0.45);
    let dir = (pos - f).normalize_or(Vec2::X);
    mirar(medio, 24.0, yaw_de(dir) + 90.0, 52.0);
    format!("incendio: {} curiosos, cadena de baldes de {:.0} m · {} mirando", c.agentes.e.stats.curiosos_hoy, pos.distance(f), c.agentes.e.agentes.iter().filter(|a| a.marcha == Marcha::MiraElFuego).count())
}

/// Se apaga el incendio de la escena (para que el guardado no dependa de él).
pub fn apagar(s: &mut Session) -> String {
    let c = &mut s.game.as_mut().unwrap().city;
    if let Some(l) = INCENDIO.lock().unwrap().take() {
        c.services.fires.burning.remove(&l);
        c.agentes.e.incendios.retain(|i| i.lote != l);
    }
    "apagado".into()
}

/// La salida de misa del domingo: la gente en el atrio, conversando en corrillos.
pub fn misa(s: &mut Session) -> String {
    ir_al_dia(s, |d| d % 7 == 6);
    let fin = agentes::datos::datos().oficios.misa_fin_domingo;
    let h = correr(s, 8.2, 20_000, |c| c.agentes.e.hora >= fin + 0.06);
    a_la_hora(s);
    let c = &s.game.as_ref().unwrap().city;
    let en_atrio = c.agentes.e.vecinos.values().filter(|v| v.lugar == agentes::Lugar::Iglesia && v.en_camino.is_none()).count();
    let saliendo = c.agentes.e.agentes.iter().filter(|a| a.proposito == Proposito::VuelveDeMisa).count();
    let pc = c.plaza.map_or(PLAZA, |p| p.0);
    let (f, _) = super::plaza_frame(c.plaza.map_or(0.0, |p| p.1));
    let atrio = pc - f * (super::voxelize::PLAZA_SIZE * 0.5 - 8.0);
    mirar(atrio, 28.0, yaw_de(-f) + 180.0 + 25.0, 34.0);
    format!("salida de misa a las {:.2}: {en_atrio} en el atrio, {saliendo} volviendo", h)
}

/// La mañana en la pulpería: los que hacen la compra, dos sentados en el banco de la puerta.
pub fn tardecita(s: &mut Session) -> String {
    ir_al_dia(s, |d| d % 7 != 6);
    correr(s, 8.0, 30_000, |c| c.agentes.e.hora >= 10.3);
    a_la_hora(s);
    let c = &s.game.as_ref().unwrap().city;
    // La pulpería con más gente.
    let mut n: std::collections::BTreeMap<u32, usize> = Default::default();
    for v in c.agentes.e.vecinos.values() {
        if let (agentes::Lugar::Solar(l), None) = (v.lugar, v.en_camino) {
            if v.paso_plan > 0 && v.plan.get(v.paso_plan as usize - 1).is_some_and(|a| matches!(a.que, Proposito::APulperia | Proposito::ACompra)) {
                *n.entry(l).or_default() += 1;
            }
        }
    }
    let Some((&l, &k)) = n.iter().max_by_key(|x| (*x.1, std::cmp::Reverse(*x.0))) else { return "nadie en las pulperías".into() };
    let lot = &c.lots.lots[&l];
    let fr = agentes::frente(lot);
    mirar(fr + lot.f * 1.2, 6.5, yaw_de(-lot.f) + 200.0, 22.0);
    format!("pulpería con {k} vecinos a las 10:18 (la compra del día)")
}

/// El boyero con la picana: la carreta más cercana a la plaza.
pub fn boyero(s: &mut Session) -> String {
    ir_al_dia(s, |d| d % 7 != 6);
    correr(s, 9.0, 900, |c| c.agentes.e.agentes.iter().filter(|a| a.tipo == Tipo::Carreta && a.v > 0.3).count() >= 2);
    a_la_hora(s);
    let c = &s.game.as_ref().unwrap().city;
    let pc = c.plaza.map_or(PLAZA, |p| p.0);
    let Some(a) = c.agentes.e.agentes.iter().filter(|a| a.tipo == Tipo::Carreta && a.v > 0.3).min_by(|x, y| agentes::pose(&c.net, x).0.distance(pc).total_cmp(&agentes::pose(&c.net, y).0.distance(pc))) else {
        return "sin carretas en marcha".into();
    };
    let (p, d) = agentes::pose(&c.net, a);
    mirar(p - d * 1.0, 9.0, yaw_de(d) + 240.0, 14.0);
    "boyero con la picana".into()
}

/// Los postigos y el poyo de una casa (pedidos de detalle de la fachada).
pub fn fachada(s: &mut Session) -> String {
    use crate::detalle::Id;
    s.hour = 12.5;
    sin_cursor(s);
    let c = &s.game.as_ref().unwrap().city;
    let pc = c.plaza.map_or(PLAZA, |p| p.0);
    let mut mejor: Option<(f32, Vec2, f32, usize)> = None;
    for l in c.lots.lots.values() {
        let d = c.fachadas.de_solar(l.id, &c.lots);
        let poyos = d.iter().filter(|p| p.id == Id::BancoPiedra).count();
        let postigos = d.iter().filter(|p| matches!(p.id, Id::Postigo | Id::PostigoAbierto)).count();
        if poyos == 0 || postigos == 0 {
            continue;
        }
        let p = d.iter().find(|p| p.id == Id::BancoPiedra).unwrap();
        let q = Vec2::new(p.pos.x, p.pos.z);
        let k = q.distance(pc) - 30.0 * postigos as f32;
        if mejor.is_none_or(|m| k < m.0) {
            mejor = Some((k, q, p.giro, postigos));
        }
    }
    let Some((_, q, giro, n)) = mejor else { return "ninguna casa con poyo y postigos".into() };
    let dir = Vec2::new(giro.sin(), giro.cos());
    mirar(q + dir * 0.8, 5.5, giro.to_degrees() + 30.0, 14.0);
    format!("casa con poyo y {n} postigos")
}

/// La calle con más escalones por metro cerca de la plaza (para el antes y después).
pub fn calle_en_pendiente(s: &mut Session) -> String {
    s.hour = 10.5;
    sin_cursor(s);
    let t = s.terrain.clone();
    let c = &s.game.as_ref().unwrap().city;
    let pc = c.plaza.map_or(PLAZA, |p| p.0);
    let plaza = c.plaza_square();
    let mut mejor: Option<(f32, u32, usize)> = None;
    for (&id, e) in &c.net.edges {
        // Una calle empedrada (en las de tierra lo que más se ve son las huellas, no la cuesta).
        if e.pts[0].distance(pc) > 900.0 || e.len() < 30.0 || e.kind != super::roads::RoadKind::Stone {
            continue;
        }
        let r = super::voxelize::escalones(&t, &c.net, id, plaza);
        let k = r.len() as f32 / e.len();
        if mejor.is_none_or(|m| k > m.0) {
            mejor = Some((k, id, r.len()));
        }
    }
    let Some((_, id, n)) = mejor else { return "sin calles en pendiente".into() };
    // Cuántas rampas empalman con la calzada de vóxeles que hay en el mundo.
    let (mut bien, mut total) = (0, 0);
    {
        let store = s.streamer.store.read().unwrap();
        let suelo = &store.levels[0].ground;
        let vs = crate::lod::BASE_VOXEL;
        let col = |q: Vec2| glam::IVec2::new((q.x / vs).floor() as i32, (q.y / vs).floor() as i32);
        for k in super::voxelize::escalones(&t, &c.net, id, plaza) {
            let dir = Vec2::new(k.giro.sin(), k.giro.cos());
            let p = Vec2::new(k.pos[0], k.pos[2]);
            if let (Some(&ar), Some(&ab)) = (suelo.get(&col(p + dir * 0.1)), suelo.get(&col(p - dir * 0.1))) {
                total += 1;
                if ((ar + 1) as f32 * vs - k.pos[1]).abs() < 1e-3 && ar == ab + 1 {
                    bien += 1;
                } else if total - bien <= 3 {
                    println!("  rampa que no empalma: {:?} arriba {ar} abajo {ab}", k.pos);
                }
            }
        }
    }
    let e = &c.net.edges[&id];
    let (medio, tan) = e.point(e.len() * 0.5);
    // Donde hay escalones: la rampa más cercana al medio del tramo.
    let p = super::voxelize::escalones(&t, &c.net, id, plaza).iter().map(|k| Vec2::new(k.pos[0], k.pos[2])).min_by(|a, b| a.distance(medio).total_cmp(&b.distance(medio))).unwrap_or(medio);
    mirar(p - tan * 3.0, 12.0, yaw_de(tan) + 160.0, 26.0);
    format!("calle de {:.0} m con {n} rampas de escalón ({}); {bien} de {total} empalman con la calzada del mundo", e.len(), e.kind.name())
}

/// Sin las rampas de los escalones (para la captura de antes).
pub fn sin_rampas(_s: &mut Session) -> String {
    agentes::oficios::SIN_RAMPAS.store(true, std::sync::atomic::Ordering::Relaxed);
    "sin rampas".into()
}

pub fn con_rampas(s: &mut Session) -> String {
    agentes::oficios::SIN_RAMPAS.store(false, std::sync::atomic::Ordering::Relaxed);
    let centro = Vec2::new(s.cam.target.x, s.cam.target.z);
    let c = &s.game.as_ref().unwrap().city;
    c.agentes.precalentar_rampas(c, &s.terrain, centro);
    "con rampas".into()
}
