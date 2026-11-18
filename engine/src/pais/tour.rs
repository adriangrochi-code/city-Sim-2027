//! Recorrido del mapa del país (`motor --demo-pais carpeta`): el país en 1726, 1790, 1850 y
//! 1895 con sus capas, obras del cabildo en cada época (y las que la época no deja), y guardar
//! y cargar el país.

use super::{Accion, Obra, Pais};
use crate::session::Session;
use crate::tour::Act;
use crate::ui::GameUi;

/// Lleva la partida (ciudad y país) a enero del año dado, simulando solo el país (la ciudad no
/// crece: es un recorrido del mapa).
fn avanzar(s: &mut Session, anio: i32) -> String {
    let g = s.game.as_mut().unwrap();
    let mes = ((anio - crate::city::START_YEAR as i32) * 12) as u32;
    let t0 = std::time::Instant::now();
    while g.pais.mes < mes {
        let m = g.pais.mes + 1;
        let pob = super::poblacion_capital_estimada(super::anio_de(m)) as i32;
        g.pais.mes_tick(m, pob, false);
    }
    g.city.month = mes;
    g.city.interior = g.pais.ultimo.clone();
    let p = &g.pais;
    format!(
        "{} · {} pueblos ({} hab. en el interior), {} estancias, {} cabezas, {} saladeros, {} caminos · último mes: {:.0} cueros, {:.0} tasajo, {:.0} de contrabando, {} carretas ({:.1?})",
        g.city.date(),
        p.pueblos.len(),
        p.poblacion_interior() as i64,
        p.estancias.len(),
        p.ganado() as i64,
        p.saladeros.len(),
        p.caminos.len(),
        p.ultimo.cueros,
        p.ultimo.tasajo,
        p.ultimo.contrabando,
        p.ultimo.carretas,
        t0.elapsed()
    )
}

fn obra(s: &mut Session, a: Accion) -> String {
    let g = s.game.as_mut().unwrap();
    g.city.money = g.city.money.max(50_000);
    match g.pais.aplicar(&a, &mut g.city.money) {
        Ok(m) => format!("{}: {m}", a.nombre()),
        Err(e) => format!("{}: NO — {e}", a.nombre()),
    }
}

fn pos_de(s: &Session, id: &str) -> [f32; 2] {
    s.game.as_ref().unwrap().pais.pueblo(id).map_or([200.0, 450.0], |p| p.pos)
}

fn abrir(u: &mut GameUi, centro: Option<([f32; 2], f32)>) {
    u.pais = Default::default();
    u.pais.abierto = true;
    if let Some((c, z)) = centro {
        u.pais.mirar(c, z);
    }
}

pub fn pasos() -> Vec<Act> {
    use Act::*;
    vec![
        Hour(10.5),
        UiShot("pais-1726-general", |_, u| abrir(u, None)),
        UiShot("pais-1726-relieve-sierras", |_, u| {
            abrir(u, Some(([300.0, 470.0], 9.0)));
            u.pais.capas.cobertura = true;
        }),
        Game("el país en 1790", |s| avanzar(s, 1790)),
        UiShot("pais-1790-colonial", |_, u| abrir(u, None)),
        UiShot("pais-1790-sur-carretas", |_, u| abrir(u, Some(([215.0, 470.0], 5.0)))),
        Game("otorgar una estancia cerca de Santa Lucía (colonia)", |s| {
            let p = pos_de(s, "santa_lucia");
            obra(s, Accion::Estancia([p[0] + 10.0, p[1] - 14.0]))
        }),
        Game("fundar un pueblo en la colonia (no se puede)", |s| obra(s, Accion::FundarPueblo([250.0, 330.0]))),
        Game("trazar un camino en la colonia (no se puede)", |s| obra(s, Accion::Camino("minas".into(), "florida".into()))),
        Game("balsa en el primer paso del Camino Real del Norte", |s| {
            let g = s.game.as_ref().unwrap();
            let c = g.pais.caminos.iter().find(|c| !c.pasos.is_empty() && c.pasos.iter().any(|p| p.obra == Obra::Vado));
            match c {
                Some(c) => {
                    let (id, k) = (c.id, c.pasos.iter().position(|p| p.obra == Obra::Vado).unwrap());
                    obra(s, Accion::Paso(id, k, Obra::Balsa))
                }
                None => "no hay vados".into(),
            }
        }),
        UiShot("pais-1790-obras-ficha", |s, u| {
            abrir(u, Some(([215.0, 470.0], 5.0)));
            u.pais.herr = Some(super::ui::Herr::Estancia);
            let id = s.game.as_ref().unwrap().pais.estancias.iter().rev().find(|e| e.jugador).map(|e| e.id);
            u.pais.sel = id.map(super::ui::Sel::Estancia);
        }),
        Game("el país en 1850", |s| avanzar(s, 1850)),
        Game("fundar un pueblo entre Durazno y Tacuarembó", |s| obra(s, Accion::FundarPueblo([255.0, 245.0]))),
        Game("camino del pueblo nuevo a Durazno", |s| {
            let id = s.game.as_ref().unwrap().pais.pueblos.iter().rev().find(|p| p.jugador).map(|p| p.id.clone()).unwrap_or_default();
            obra(s, Accion::Camino(id, "durazno".into()))
        }),
        Game("puente en el paso más ancho del Camino Real del Norte", |s| {
            let g = s.game.as_ref().unwrap();
            let mejor = g.pais.caminos.iter().flat_map(|c| c.pasos.iter().enumerate().map(move |(k, p)| (c.id, k, p.ancho))).max_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
            match mejor {
                Some((id, k, _)) => obra(s, Accion::Paso(id, k, Obra::Puente)),
                None => "no hay pasos".into(),
            }
        }),
        Game("mejorar el Camino Real del Norte", |s| {
            let id = s.game.as_ref().unwrap().pais.caminos.iter().find(|c| c.desde == "florida" && c.hasta == "durazno").map(|c| c.id).unwrap_or(0);
            obra(s, Accion::MejorarCamino(id))
        }),
        Game("el país un año más (1851)", |s| avanzar(s, 1851)),
        UiShot("pais-1851-ciudad-nueva", |_, u| abrir(u, None)),
        UiShot("pais-1851-departamentos", |_, u| {
            abrir(u, None);
            u.pais.capas.departamentos = true;
            u.pais.capas.flujos = false;
        }),
        UiShot("pais-recursos", |_, u| {
            abrir(u, None);
            u.pais.capas.recursos = true;
            u.pais.capas.flujos = false;
            u.pais.capas.caminos = false;
        }),
        UiShot("pais-1851-centro-camino-nuevo", |s, u| {
            abrir(u, Some(([255.0, 270.0], 5.5)));
            let id = s.game.as_ref().unwrap().pais.caminos.iter().rev().find(|c| c.jugador).map(|c| c.id);
            u.pais.sel = id.map(super::ui::Sel::Camino);
        }),
        Game("el país en 1895", |s| avanzar(s, 1895)),
        UiShot("pais-1895-ferrocarril", |_, u| abrir(u, None)),
        UiShot("pais-1895-sur", |_, u| abrir(u, Some(([215.0, 470.0], 5.0)))),
        Game("guardar y cargar el país", guardar_y_cargar),
    ]
}

/// Guarda la partida (con el país), la vuelve a leer y compara; después avanza las dos copias
/// dos años y las compara otra vez (el país es determinista).
fn guardar_y_cargar(s: &mut Session) -> String {
    let g = s.game.as_ref().unwrap();
    let f = crate::save::snapshot(g, s.cam_array(), s.hour, false);
    let json = match serde_json::to_vec(&f) {
        Ok(j) => j,
        Err(e) => return format!("¡error al guardar! {e}"),
    };
    let back: crate::save::SaveFile = match serde_json::from_slice(&json) {
        Ok(b) => b,
        Err(e) => return format!("¡error al leer! {e}"),
    };
    let (g2, _) = match crate::save::restore(&back, &s.terrain) {
        Ok(x) => x,
        Err(e) => return format!("¡error al cargar! {e}"),
    };
    let igual = g2.pais == g.pais && g2.city.interior == g.city.interior;
    let (mut a, mut b): (Pais, Pais) = (g.pais.clone(), g2.pais.clone());
    let fin = a.mes + 24;
    a.mes_tick(fin, 30000, false);
    b.mes_tick(fin, 30000, false);
    let igual2 = a == b;
    format!("guardar y cargar el país: {} · dos años después: {} · {} KB de partida", if igual { "IGUAL" } else { "¡DISTINTO!" }, if igual2 { "IGUAL" } else { "¡DISTINTO!" }, json.len() / 1024)
}
