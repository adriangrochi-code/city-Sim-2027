//! Recorrido de la época 2, Ciudad Nueva (`MOTOR_EPOCA2=1 motor --demo-ciudad carpeta`):
//! después de la ciudad colonial de la demostración, el paso de época, el ensanche, la usina
//! de gas, el teatro, el agua corriente y el colector, unos años de crecimiento con casas
//! nuevas y viejas modernizadas, y calles de muestra para las capturas.

use super::epocas::{ensanche, Muralla};
use super::game_demo::{apply, hileras, place, PLAZA};
use super::lots::{zone_code, Density, ZoneKind};
use super::materiales::{Material, Obra, Pared, Techo};
use super::poblacion::Epoca;
use super::services::Kind;
use super::{plaza_frame, BLOCK};
use crate::session::Session;
use glam::Vec2;

/// Calles de muestra de la época 2: (nombre, centro, frente, solares).
pub static MUESTRAS: std::sync::Mutex<Vec<super::game_demo::MuestraArq>> = std::sync::Mutex::new(Vec::new());

/// Pasa a la Ciudad Nueva: la iglesia terminada y el nuevo orden si faltan (la demostración
/// dura pocos años), y la muralla se derriba.
pub fn pasar(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let faltan: Vec<String> = g.city.requisitos_epoca().into_iter().filter(|r| !r.cumplido).map(|r| r.texto).collect();
    let r = g.city.pasar_epoca(&t, Muralla::Derribar, true);
    let txt = match r {
        Ok((e, aviso)) => {
            g.message = aviso.clone();
            g.message_seq += 1;
            format!("{aviso} · {}", apply(s, Ok(e)))
        }
        Err(m) => format!("ERROR: {m}"),
    };
    format!("{txt} · requisitos que faltaban (forzado para la demostración): {}", if faltan.is_empty() { "ninguno".into() } else { faltan.join("; ") })
}

/// Busca dónde entra un ensanche de `n` × `m` manzanas pegado al damero viejo, del lado de
/// tierra adentro.
fn sitio_ensanche(c: &super::City, t: &crate::terrain::TerrainGen) -> Option<(Vec2, Vec2, Vec2)> {
    let (f, s) = plaza_frame(0.0);
    for (nu, nv) in [(5.0, 4.0), (4.0, 4.0), (4.0, 3.0), (3.0, 3.0)] {
        for (u, v) in [(f, s), (-f, s), (f, -s), (-f, -s), (s, f), (s, -f), (-s, f), (-s, -f)] {
            for k in [3.0, 3.5, 4.0, 5.0, 6.0] {
                for d in [0.0, -1.0, 1.0, -2.0] {
                    // `a`: la esquina; las manzanas se extienden en `u` (a lo largo) y `v` (hacia afuera).
                    let a = PLAZA + v * (BLOCK * k) + u * (BLOCK * (d - nu * 0.5));
                    let (b, cursor) = (a + u * BLOCK * nu, a + v * BLOCK * nv);
                    let Some(p) = ensanche::plan(a, b, cursor) else { continue };
                    if c.ensanche_problema(t, &p).is_none() {
                        return Some((a, b, cursor));
                    }
                }
            }
        }
    }
    None
}

/// El ensanche sobre el ejido.
pub fn ensanche(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    g.city.money += 300_000;
    let Some((a, b, cursor)) = sitio_ensanche(&g.city, &t) else { return "ERROR: no hay lugar para el ensanche".into() };
    // Las calles de unión con la ciudad vieja las hace el propio trazado (`City::empalmar`).
    let r = g.city.trazar_ensanche(&t, a, b, cursor);
    let en = &g.city.epoca.ensanche;
    let info = en.trazados.last().map_or(String::new(), |tr| format!("{} × {} manzanas, {} plazas, {} árboles, {} calles de empalme con la red vieja", tr.nu, tr.nv, en.plazas.len(), en.arboles.len(), tr.empalmes));
    let n = g.city.lots.lots.values().filter(|l| g.city.epoca.ensanche.contiene(l.c)).count();
    let comp = super::services::components(&g.city.net);
    let n_comp: std::collections::BTreeSet<u32> = comp.values().copied().collect();
    format!("{info} · {n} solares · {} redes de calles (1 = todo unido) · {}", n_comp.len(), apply(s, r))
}

/// Zonifica el ensanche: casas, y comercio sobre el bulevar.
pub fn zonificar(s: &mut Session) -> String {
    let g = s.game.as_mut().unwrap();
    let Some(tr) = g.city.epoca.ensanche.trazados.last().copied() else { return "sin ensanche".into() };
    let (a, u, v) = (Vec2::from(tr.a), Vec2::from(tr.u), Vec2::from(tr.v));
    let centro = a + u * BLOCK * tr.nu as f32 * 0.5 + v * BLOCK * tr.nv as f32 * 0.5;
    let (n, _) = g.city.paint(centro, BLOCK * tr.nu.max(tr.nv) as f32 * 0.75, zone_code(ZoneKind::Residencial, Density::Media));
    let eje = a + u * BLOCK * tr.bulevar as f32;
    let mut c = 0;
    let mut d = 30.0;
    while d < BLOCK * tr.nv as f32 {
        c += g.city.paint(eje + v * d, 32.0, zone_code(ZoneKind::Comercial, Density::Media)).0;
        d += 60.0;
    }
    format!("{n} solares residenciales, {c} comerciales sobre el bulevar")
}

/// Usina de gas, teatro, agua corriente y colector; materiales para crecer y la ordenanza de
/// fachadas.
pub fn servicios(s: &mut Session) -> String {
    use super::economy::{Ordinance, Res};
    let mut out = Vec::new();
    let (f, side) = plaza_frame(0.0);
    let (centro_e, esquina_e) = {
        let g = s.game.as_mut().unwrap();
        g.city.money += 400_000;
        g.city.economy.stocks[Res::Ladrillos as usize].amount = 6000.0;
        g.city.economy.stocks[Res::Piedra as usize].amount = 4000.0;
        let m = &mut g.city.materiales;
        for (mat, q) in [(Material::Cal, 3000.0), (Material::Hierro, 800.0), (Material::Teja, 4000.0), (Material::Vidrio, 500.0), (Material::Madera, 3000.0), (Material::Arena, 3000.0)] {
            m.ex[mat.idx()].cantidad = q;
        }
        if !g.city.economy.is_on(Ordinance::Fachadas) {
            let _ = g.city.economy.toggle(Ordinance::Fachadas, &mut g.city.money);
        }
        let tr = g.city.epoca.ensanche.trazados.last().copied();
        match tr {
            Some(tr) => {
                let (a, u, v) = (Vec2::from(tr.a), Vec2::from(tr.u), Vec2::from(tr.v));
                (a + u * BLOCK * tr.nu as f32 * 0.5 + v * BLOCK * tr.nv as f32 * 0.5, a)
            }
            None => (PLAZA + side * 500.0, PLAZA + side * 400.0),
        }
    };
    // El teatro, entre la ciudad vieja y el ensanche (como el Solís, frente a la plaza
    // Independencia, donde estaba la Ciudadela).
    out.push(place(s, Kind::Teatro, PLAZA.lerp(esquina_e, 0.6), 450.0));
    out.push(place(s, Kind::UsinaGas, esquina_e, 700.0));
    out.push(place(s, Kind::AguaCorriente, centro_e, 700.0));
    let pier = s.game.as_ref().unwrap().city.services.port.pier.as_ref().map_or(PLAZA - f * 300.0, |p| p.land);
    // El colector, junto a la costa más cercana a la plaza (así su red es la de la ciudad).
    let mut col = place(s, Kind::Colector, PLAZA, 900.0);
    if col.contains("sin sitio") {
        col = place(s, Kind::Colector, pier, 600.0);
    }
    out.push(col);
    let g = s.game.as_ref().unwrap();
    format!("{} · {} faroles de gas · {} solares con agua corriente · {} con colector (de {} edificados)", out.join(" · "), g.city.epoca.faroles_gas.len(), g.city.epoca.agua_red.len(), g.city.epoca.cloaca.len(), g.city.lots.lots.values().filter(|l| l.level > 0).count())
}

/// Años de Ciudad Nueva: casas nuevas en el ensanche, viejas modernizadas en la ciudad vieja.
pub fn crecer(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let months = 48;
    let t0 = std::time::Instant::now();
    let mut e = crate::buildings::Edits::default();
    for _ in 0..months / 6 {
        // Lo que trae el puerto en la Ciudad Nueva: cal, hierro y teja para las obras.
        let m = &mut g.city.materiales;
        for (mat, q) in [(Material::Cal, 1500.0), (Material::Hierro, 300.0), (Material::Teja, 600.0)] {
            m.ex[mat.idx()].cantidad = m.ex[mat.idx()].cantidad.max(q);
        }
        e.extend(g.city.fast_forward(&t, 6));
    }
    let sim = t0.elapsed();
    let obras: Vec<Obra> = g.city.lots.lots.values().filter(|l| l.level > 0).filter_map(|l| l.obra).collect();
    let nuevas = obras.iter().filter(|o| o.contexto.epoca >= 1).count();
    let reformadas = obras.iter().filter(|o| o.contexto.reforma > 0).count();
    let viejas = obras.len() - nuevas - reformadas;
    // [ciudad] Modelos de detalle de las fachadas.
    let det = g.city.fachadas.cuenta(&g.city.lots).iter().map(|(k, v)| format!("{k:?} {v}")).collect::<Vec<_>>().join(", ");
    let out = format!(
        "{months} meses en {sim:.2?} · {} · {} habitantes ({:?}) · casas de la época 2: {nuevas} · modernizadas: {reformadas} · coloniales: {viejas} · faroles de gas {} · sebo {} · detalles de fachada: {det} · ",
        g.city.date(),
        g.city.habitantes(),
        g.city.poblacion.epoca,
        g.city.epoca.faroles_gas.len(),
        g.city.services.lamps.len()
    );
    out + &apply(s, Ok(e))
}

/// Calles de muestra de la época 2 (para las capturas): casas neoclásicas de patio, casas de
/// altos, almacenes de esquina en el ensanche, y una calle de la ciudad vieja con casas
/// coloniales y modernizadas mezcladas.
pub fn muestras(s: &mut Session) -> String {
    let t = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    let c = &mut g.city;
    let todas = hileras(c, 5);
    let sol = Vec2::new(0.5, -0.42).normalize();
    // Las hileras con la fachada al sol de la mañana (para las capturas); si no alcanzan, todas.
    let elegir = |ens: bool, min: usize| -> Vec<Vec<u32>> {
        let de = |f: &&Vec<u32>| c.epoca.ensanche.contiene(c.lots.lots[&f[0]].c) == ens;
        let al_sol: Vec<Vec<u32>> = todas.iter().filter(de).filter(|f| c.lots.lots[&f[0]].f.dot(sol) > 0.1).cloned().collect();
        if al_sol.len() >= min { al_sol } else { todas.iter().filter(de).cloned().collect() }
    };
    let en_ens = elegir(true, 3);
    let viejas = elegir(false, 1);
    type M = (&'static str, ZoneKind, u8, u16, bool);
    let muestras: [M; 4] = [
        ("casas neoclásicas de patio, 1850", ZoneKind::Residencial, 2, 1850, true),
        ("casas de altos neoclásicas, 1862", ZoneKind::Residencial, 3, 1862, true),
        ("almacenes y casas con tienda, 1855", ZoneKind::Comercial, 2, 1855, true),
        ("calle vieja: coloniales y modernizadas", ZoneKind::Residencial, 2, 1795, false),
    ];
    let mut e = crate::buildings::Edits::default();
    let mut lista = Vec::new();
    let mut nombres = std::collections::BTreeMap::<String, usize>::new();
    let mut usadas = std::collections::HashSet::new();
    for (k, (nombre, kind, nivel, anio, nueva)) in muestras.into_iter().enumerate() {
        let filas = if nueva { &en_ens } else { &viejas };
        let Some(fila) = filas.iter().find(|f| !usadas.contains(&f[0]) && (kind != ZoneKind::Comercial || f.iter().take(8).any(|id| c.esquina_de(&c.lots.lots[id]) > 0))).or_else(|| filas.iter().find(|f| !usadas.contains(&f[0]))) else {
            continue;
        };
        usadas.insert(fila[0]);
        let ids: Vec<u32> = fila.iter().take(8).copied().collect();
        for (j, &id) in ids.iter().enumerate() {
            let seed = c.lots.lots[&id].seed;
            {
                let l = c.lots.lots.get_mut(&id).unwrap();
                l.zone = zone_code(kind, Density::Media);
                l.level = nivel;
            }
            let pared = if j % 3 == 2 { Pared::Piedra } else { Pared::Ladrillo };
            let mut o = Obra::nueva(pared, Techo::Azotea, nueva || j % 4 != 1, true);
            o.contexto = c.contexto_solar(id, nivel);
            o.contexto.anio = anio;
            o.contexto.clase = [1, 2, 1, 3, 2, 1, 2, 3][j % 8];
            let ep = if nueva { Epoca::CiudadNueva } else { Epoca::Colonial };
            o.contexto.tradicion = super::arquitectura::tradicion_de(super::arquitectura::origen_probable_en(ep, anio, seed ^ (j as u64 + k as u64 * 17))).indice();
            o.contexto.epoca = u8::from(nueva);
            o.contexto.reforma = 0;
            if !nueva && j % 2 == 0 {
                // La mitad de la calle vieja, modernizada en 1858: misma casa, cara nueva.
                o.contexto.antes = u8::from(!o.revoque);
                o.revoque = true;
                o.contexto.reforma = 1858;
            }
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
    let n = lista.len();
    *MUESTRAS.lock().unwrap() = lista;
    let tipos = nombres.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", ");
    format!("{n} calles de muestra · tipos: {tipos} · {}", apply(s, Ok(e)))
}

/// Cámara sobre la calle de muestra `k` (como `game_demo::view_arquitectura`).
pub fn view_muestra(k: usize, hacia: f32, lejos: f32, giro: f32, pitch: f32) -> Option<[f32; 5]> {
    let m = MUESTRAS.lock().unwrap();
    let (_, c, f, _) = m.get(k)?;
    let p = *c + *f * hacia;
    Some([p.x, p.y, lejos, f.x.atan2(f.y).to_degrees() + giro, pitch])
}

/// Cámara sobre el ensanche: `alto` (m) y `pitch`; mira por el bulevar.
pub fn view_ensanche(s: &Session, frac: f32, lejos: f32, pitch: f32) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let tr = c.epoca.ensanche.trazados.last()?;
    let (a, u, v) = (Vec2::from(tr.a), Vec2::from(tr.u), Vec2::from(tr.v));
    let p = a + u * BLOCK * tr.bulevar as f32 + v * BLOCK * tr.nv as f32 * frac;
    Some([p.x, p.y, lejos, v.x.atan2(v.y).to_degrees() + 180.0 + 25.0, pitch])
}

/// Cámara sobre una plaza del ensanche.
pub fn view_plaza(s: &Session, lejos: f32, pitch: f32) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let (p, f) = c.epoca.ensanche.plazas.first()?;
    let f = Vec2::from(*f);
    Some([p[0], p[1], lejos, f.x.atan2(f.y).to_degrees() + 30.0, pitch])
}

/// Cámara sobre un servicio de la época 2.
pub fn view_servicio(s: &Session, k: Kind, lejos: f32, pitch: f32) -> Option<[f32; 5]> {
    let c = &s.game.as_ref()?.city;
    let b = c.services.of_kind(k).next()?;
    Some([b.c.x, b.c.y, lejos, b.f.x.atan2(b.f.y).to_degrees() + 25.0, pitch])
}

/// Un solar modernizado de la calle vieja (para la ficha).
pub fn solar_modernizado(s: &Session) -> Option<Vec2> {
    let c = &s.game.as_ref()?.city;
    c.lots.lots.values().find(|l| l.obra.is_some_and(|o| o.contexto.reforma > 0)).map(|l| l.c)
}

/// Un solar colonial que se puede modernizar (para la ficha con los botones de reforma).
pub fn solar_colonial(s: &Session) -> Option<Vec2> {
    let c = &s.game.as_ref()?.city;
    c.lots.lots.values().find(|l| c.reforma_posible(l.id, super::epocas::rehabilitar::Reforma::Fachada).is_ok()).map(|l| l.c)
}
