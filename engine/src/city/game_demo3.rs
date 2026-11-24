//! Recorrido de la época 3, Inmigración (`MOTOR_EPOCA3=1 motor --demo-ciudad carpeta`): después
//! de la ciudad colonial y de la Ciudad Nueva de las demostraciones anteriores, el paso a la
//! época 3 (con el ferrocarril propio), la estación, la usina eléctrica, las cocheras y las
//! líneas de tranvía, el hotel de inmigrantes y el puerto moderno, unos años de crecimiento
//! con conventillos, y calles de muestra con el kit de la época para las capturas.

use super::epocas::epoca3::Ferrocarril;
use super::game_demo::{apply, hileras, place, PLAZA};
use super::lots::{zone_code, Density, ZoneKind};
use super::materiales::{Material, Obra, Pared, Techo};
use super::services::Kind;
use super::{plaza_frame, BLOCK};
use crate::session::Session;
use glam::Vec2;

/// Calles de muestra de la época 3: (nombre, centro, frente, solares).
pub static MUESTRAS: std::sync::Mutex<Vec<super::game_demo::MuestraArq>> = std::sync::Mutex::new(Vec::new());

/// Pasa a la época 3 con el ferrocarril propio (forzado si faltan requisitos: la demostración
/// dura pocos años).
pub fn pasar(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let faltan: Vec<String> = g.city.requisitos_epoca().into_iter().filter(|r| !r.cumplido).map(|r| r.texto).collect();
    let txt = match g.city.pasar_epoca3(&t, Ferrocarril::Propio, true) {
        Ok((e, aviso)) => {
            g.message = aviso.clone();
            g.message_seq += 1;
            format!("{aviso} · {}", apply(s, Ok(e)))
        }
        Err(m) => format!("ERROR: {m}"),
    };
    format!("{txt} · requisitos que faltaban (forzado para la demostración): {}", if faltan.is_empty() { "ninguno".into() } else { faltan.join("; ") })
}

/// Centro del último ensanche (o un punto al costado de la plaza).
fn centro_ensanche(c: &super::City) -> Vec2 {
    match c.epoca.ensanche.trazados.last() {
        Some(tr) => {
            let (a, u, v) = (Vec2::from(tr.a), Vec2::from(tr.u), Vec2::from(tr.v));
            a + u * BLOCK * tr.nu as f32 * 0.5 + v * BLOCK * tr.nv as f32 * 0.5
        }
        None => PLAZA + plaza_frame(0.0).1 * 500.0,
    }
}

/// Estación, usina eléctrica, cocheras de tranvía, hotel de inmigrantes y puerto moderno.
pub fn servicios(s: &mut Session) -> String {
    let mut out = Vec::new();
    let (ens, pier) = {
        let g = s.game.as_mut().unwrap();
        g.city.money += 400_000;
        let m = &mut g.city.materiales;
        for (mat, q) in [(Material::Cal, 3000.0), (Material::Hierro, 1500.0), (Material::Teja, 3000.0), (Material::Vidrio, 800.0), (Material::Madera, 3000.0), (Material::Arena, 3000.0)] {
            m.ex[mat.idx()].cantidad = m.ex[mat.idx()].cantidad.max(q);
        }
        g.city.economy.stocks[super::economy::Res::Ladrillos as usize].amount = 8000.0;
        g.city.economy.stocks[super::economy::Res::Piedra as usize].amount = 6000.0;
        let pier = g.city.services.port.pier.as_ref().map_or(PLAZA - plaza_frame(0.0).0 * 300.0, |p| p.land);
        (centro_ensanche(&g.city), pier)
    };
    // La estación, del lado de tierra adentro (las líneas llegan del país).
    out.push(place(s, Kind::Estacion, ens + (ens - PLAZA).normalize_or_zero() * 250.0, 900.0));
    out.push(place(s, Kind::UsinaElectrica, PLAZA.lerp(ens, 0.5), 900.0));
    out.push(place(s, Kind::CocherasTranvia, ens, 900.0));
    out.push(place(s, Kind::HotelInmigrantes, pier, 900.0));
    out.push(puerto(s, pier));
    let g = s.game.as_ref().unwrap();
    let c = &g.city;
    format!(
        "{} · {} faroles eléctricos, {} de gas · {} líneas de tranvía ({} km, {}) · {} solares a menos de 300 m de una línea",
        out.join(" · "),
        c.epoca.faroles_electricos.len(),
        c.epoca.faroles_gas.len(),
        c.epoca.tranvias.len(),
        (c.epoca.tranvias.iter().map(|l| l.largo()).sum::<f32>() / 100.0).round() / 10.0,
        if c.epoca.tranvias.iter().any(|l| l.electrica) { "eléctricas" } else { "a caballo" },
        c.solares_con_tranvia()
    )
}

/// El puerto moderno: busca sobre la costa, del muelle viejo hacia afuera, un sitio con el
/// muelle del lado del agua (la calle del lado de tierra).
fn puerto(s: &mut Session, pier: Vec2) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let k = Kind::PuertoModerno;
    for r in (0..90).map(|i| i as f32 * 25.0) {
        let n = ((r / 15.0) as i32).clamp(1, 96);
        for a in 0..n {
            let p = pier + Vec2::from_angle(a as f32 / n as f32 * std::f32::consts::TAU) * r;
            if !super::services::near_coast(&t, p, 220.0) {
                continue;
            }
            let (c, f) = g.city.service_site(k, p);
            if g.city.service_problem(&t, k, c, f).is_none() {
                let r = g.city.place_service(&t, k, c, f);
                return format!("Puerto moderno en ({:.0}, {:.0}): {}", c.x, c.y, apply(s, r));
            }
        }
    }
    let (c, f) = g.city.service_site(k, pier);
    format!("Puerto moderno: sin sitio ({})", g.city.service_problem(&t, k, c, f).unwrap_or_default())
}

/// Barrio de fábricas junto a la estación y casas en el ensanche.
pub fn zonificar(s: &mut Session) -> String {
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    let est = c.services.of_kind(Kind::Estacion).next().map(|b| b.c);
    let mut n = 0;
    if let Some(p) = est {
        n = c.paint(p, 130.0, zone_code(ZoneKind::Industrial, Density::Media)).0;
    }
    format!("{n} solares industriales junto a la estación")
}

/// Años de inmigración: casas de la época 3, conventillos y fábricas.
pub fn crecer(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let months = 48;
    let t0 = std::time::Instant::now();
    let mut e = crate::buildings::Edits::default();
    for _ in 0..months / 6 {
        let m = &mut g.city.materiales;
        for (mat, q) in [(Material::Cal, 1500.0), (Material::Hierro, 500.0), (Material::Teja, 600.0), (Material::Vidrio, 300.0)] {
            m.ex[mat.idx()].cantidad = m.ex[mat.idx()].cantidad.max(q);
        }
        e.extend(g.city.fast_forward(&t, 6));
    }
    let sim = t0.elapsed();
    let c = &g.city;
    let obras: Vec<Obra> = c.lots.lots.values().filter(|l| l.level > 0).filter_map(|l| l.obra).collect();
    let e3 = obras.iter().filter(|o| o.contexto.epoca >= 2).count();
    let out = format!(
        "{months} meses en {sim:.2?} · {} · {} habitantes ({:?}) · casas de la época 3: {e3} · conventillos: {} (sin casa: {}) · faroles eléctricos {} · gas {} · tranvías {} · ",
        c.date(),
        c.habitantes(),
        c.poblacion.epoca,
        c.conventillos(),
        c.deficit_vivienda(),
        c.epoca.faroles_electricos.len(),
        c.epoca.faroles_gas.len(),
        c.epoca.tranvias.len()
    );
    out + &apply(s, Ok(e))
}

/// Calles de muestra de la época 3 (para las capturas).
pub fn muestras(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    let todas = hileras(c, 5);
    let sol = Vec2::new(0.5, -0.42).normalize();
    let al_sol: Vec<Vec<u32>> = todas.iter().filter(|f| c.lots.lots[&f[0]].f.dot(sol) > 0.1).cloned().collect();
    let filas = if al_sol.len() >= 6 { al_sol } else { todas.clone() };
    // (nombre, zona, densidad, nivel, año, clase base, época del kit, conventillo)
    type M = (&'static str, ZoneKind, Density, u8, u16, u8, u8, u16);
    let muestras: [M; 6] = [
        ("casas standard (casas chorizo), 1895", ZoneKind::Residencial, Density::Media, 2, 1895, 1, 2, 0),
        ("edificios de renta, 1910", ZoneKind::Residencial, Density::Alta, 3, 1910, 2, 2, 0),
        ("almacenes y bares de esquina, 1900", ZoneKind::Comercial, Density::Media, 2, 1900, 1, 2, 0),
        ("casas de altos italianizantes, 1888", ZoneKind::Residencial, Density::Media, 3, 1888, 1, 2, 0),
        ("conventillos: casas viejas divididas en piezas", ZoneKind::Residencial, Density::Alta, 2, 1790, 0, 0, 1895),
        ("fábricas y galpones, 1905", ZoneKind::Industrial, Density::Media, 2, 1905, 1, 2, 0),
    ];
    let mut e = crate::buildings::Edits::default();
    let mut lista = Vec::new();
    let mut nombres = std::collections::BTreeMap::<String, usize>::new();
    let mut usadas = std::collections::HashSet::new();
    for (k, (nombre, kind, dens, nivel, anio, clase, epoca, conv)) in muestras.into_iter().enumerate() {
        let esquina = |f: &&Vec<u32>| f.iter().take(8).any(|id| c.esquina_de(&c.lots.lots[id]) > 0);
        let Some(fila) = filas.iter().find(|f| !usadas.contains(&f[0]) && (kind != ZoneKind::Comercial || esquina(f))).or_else(|| filas.iter().find(|f| !usadas.contains(&f[0]))) else {
            continue;
        };
        usadas.insert(fila[0]);
        let ids: Vec<u32> = fila.iter().take(8).copied().collect();
        for (j, &id) in ids.iter().enumerate() {
            {
                let l = c.lots.lots.get_mut(&id).unwrap();
                l.zone = zone_code(kind, dens);
                l.level = nivel;
            }
            let pared = if j % 3 == 2 && epoca == 0 { Pared::Piedra } else { Pared::Ladrillo };
            let mut o = Obra::nueva(pared, Techo::Azotea, true, true);
            o.contexto = c.contexto_solar(id, nivel);
            o.contexto.anio = anio;
            o.contexto.clase = (clase + [0, 1, 0, 1, 2, 0, 1, 0][j % 8]).min(3);
            o.contexto.epoca = epoca;
            o.contexto.reforma = 0;
            o.contexto.conventillo = conv;
            c.elegir_giro(id, nivel, &mut o);
            c.lots.lots.get_mut(&id).unwrap().obra = Some(o);
            if let Some(linea) = c.lineas_estilo(&c.lots.lots[&id]).into_iter().next() {
                let tipo = linea.split(',').next().unwrap_or("").to_string();
                *nombres.entry(tipo).or_default() += 1;
            }
            c.revoxelizar(&t, id, &mut e);
        }
        let _ = k;
        let centro = ids.iter().map(|id| c.lots.lots[id].c).sum::<Vec2>() / ids.len() as f32;
        lista.push((nombre, centro, c.lots.lots[&ids[0]].f, ids));
    }
    let n = lista.len();
    *MUESTRAS.lock().unwrap() = lista;
    let tipos = nombres.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", ");
    format!("{n} calles de muestra · tipos: {tipos} · {}", apply(s, Ok(e)))
}

/// Cámara sobre la calle de muestra `k`.
pub fn view_muestra(k: usize, hacia: f32, lejos: f32, giro: f32, pitch: f32) -> Option<[f32; 5]> {
    let m = MUESTRAS.lock().unwrap();
    let (_, c, f, _) = m.get(k)?;
    let p = *c + *f * hacia;
    Some([p.x, p.y, lejos, f.x.atan2(f.y).to_degrees() + giro, pitch])
}

/// Cámara sobre una línea de tranvía, a lo largo de ella (`frac` del recorrido).
pub fn view_tranvia(s: &Session, frac: f32, lejos: f32, pitch: f32) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let l = c.epoca.tranvias.first()?;
    let pts: Vec<Vec2> = l.pts.iter().map(|p| Vec2::from(*p)).collect();
    let n = (pts.len() - 1) as f32;
    let p = super::roads::point_at(&pts, frac * n);
    let q = super::roads::point_at(&pts, (frac * n + 1.0).min(n));
    let d = (q - p).normalize_or(Vec2::X);
    Some([p.x, p.y, lejos, d.x.atan2(d.y).to_degrees() + 20.0, pitch])
}

/// Un solar que es conventillo (para la ficha).
pub fn solar_conventillo(s: &Session) -> Option<Vec2> {
    let c = &s.game.as_ref()?.city;
    c.lots.lots.values().find(|l| l.level > 0 && l.obra.is_some_and(|o| o.contexto.conventillo > 0)).map(|l| l.c)
}
