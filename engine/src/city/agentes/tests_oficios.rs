//! Pruebas de los oficios y sucesos (`oficios.rs`), de quién va a caballo y de la salida de
//! misa (ia-y-transito.md §4.1, pendientes de la v0.12.0).

use super::oficios::corrillo;
use super::*;
use crate::city::lots::{zone_code, Density};
use crate::city::roads::{line, RoadKind};
use crate::city::services::tests::free_site;
use crate::city::services::Kind;
use crate::city::{grid_lines, plaza_frame, City, BLOCK};

/// Ciudad de prueba con servicios: plaza, damero de 3 × 3 cuadras, casas, un pozo, un puesto
/// de aguadores y uno de serenos.
fn ciudad_con_servicios(t: &TerrainGen) -> City {
    let mut city = City::default();
    city.money = 1_000_000;
    let c = Vec2::new(13500.0, 16300.0);
    city.found_plaza(t, c, 0.0).unwrap();
    let (f, s) = plaza_frame(0.0);
    let a = c - s * (BLOCK * 1.5) - f * (BLOCK * 1.5);
    city.build_grid(t, &grid_lines(a, a + s * BLOCK * 3.0, a + f * BLOCK * 3.0), RoadKind::Stone).unwrap();
    city.paint(c, 300.0, zone_code(ZoneKind::Residencial, Density::Media));
    city.paint(c, 60.0, zone_code(ZoneKind::Comercial, Density::Media));
    for (k, cerca) in [(Kind::Pozo, c + s * 90.0), (Kind::Serenos, c + f * 90.0)] {
        let (sc, sf) = free_site(&city, t, k, cerca);
        city.place_service(t, k, sc, sf).unwrap();
    }
    // Los aguadores piden un arroyo o la costa cerca (aquí no hay): el puesto va igual, sin
    // obra (solo importa a quién abastece).
    'a: for r in 0..30 {
        for k in 0..16 {
            let p = c - s * 90.0 + Vec2::from_angle(k as f32 / 16.0 * std::f32::consts::TAU) * (r as f32 * 10.0);
            let (sc, sf) = city.service_site(Kind::Aguadores, p);
            if city.service_problem(t, Kind::Aguadores, sc, sf).is_none_or(|m| m.contains("arroyo")) {
                city.services.next += 1;
                let id = city.services.next;
                city.services.buildings.insert(id, crate::city::services::Building { id, kind: Kind::Aguadores, c: sc, f: sf, served: 0, contaminacion: 0.0, rinde: 1.0, voxels: Vec::new() });
                city.refresh_water();
                break 'a;
            }
        }
    }
    city.fast_forward(t, 36);
    city.agentes.e.dia += 1;
    city
}

/// Corre `horas` del reloj desde `h0` en cuadros de 1/30 s; llama a `cada` en cada cuadro.
fn correr_con(city: &mut City, t: &TerrainGen, h0: f32, horas: f32, mut cada: impl FnMut(&City)) {
    let dt = 1.0 / 30.0;
    let cuadros = (horas / crate::city::HOURS_PER_SECOND / dt) as usize;
    for k in 0..cuadros {
        city.agentes.set_hora((h0 + k as f32 * dt * crate::city::HOURS_PER_SECOND) % 24.0);
        city.tick_agentes(t, dt);
        cada(city);
    }
}

#[test]
fn pocos_van_a_caballo() {
    // Arreglo de la v0.12.0: en la demostración iban 203 jinetes de 485 en la calle.
    let t = TerrainGen::new(1726);
    let mut city = ciudad_con_servicios(&t);
    let (mut jinetes, mut todos) = (0usize, 0usize);
    correr_con(&mut city, &t, 6.0, 4.0, |c| {
        let n = c.agentes.en_la_calle();
        jinetes += n[Tipo::Jinete as usize] as usize;
        todos += n.iter().sum::<u32>() as usize;
    });
    assert!(todos > 100, "{todos}");
    let parte = jinetes as f32 / todos as f32;
    assert!(parte < 0.15, "en una ciudad chica casi todos caminan: {:.0} % a caballo", parte * 100.0);
    // La regla: tener caballo no alcanza; a misa o a la pulpería del barrio se va a pie, y al
    // campo a caballo.
    let mut ag = Agentes::default();
    ag.lugares.plaza = Some(Vec2::ZERO);
    ag.radio_intramuros = 450.0;
    let mut v = tests::vecino_de_prueba();
    v.perfil = Perfil { medios: perfil::CABALLO, edad: 40, ..Default::default() };
    v.persona = Some(1);
    v.trabajo = Trabajo::Campo;
    let lejos = Vec2::new(2500.0, 0.0);
    assert!(ag.usa_caballo(&v, Vec2::ZERO, lejos, Proposito::AlTrabajo), "al campo, a caballo");
    assert!(!ag.usa_caballo(&v, Vec2::ZERO, Vec2::new(400.0, 0.0), Proposito::AMisa), "a misa, a pie");
    assert!(!ag.usa_caballo(&v, Vec2::ZERO, Vec2::new(380.0, 0.0), Proposito::AlMercado), "intramuros, a pie");
    v.perfil.medios = 0;
    assert!(!ag.usa_caballo(&v, Vec2::ZERO, lejos, Proposito::AlTrabajo), "sin caballo, a pie");
}

#[test]
fn los_aguadores_reparten_por_las_casas_sin_aljibe() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad_con_servicios(&t);
    let sid = city.services.of_kind(Kind::Aguadores).next().unwrap().id;
    let servidas = city.services.water.values().filter(|w| w.1 == sid).count();
    assert!(servidas > 0, "el puesto abastece casas");
    city.agentes.e.dia = 8; // martes
    let mut parados = 0;
    let mut con_pipa = 0;
    correr_con(&mut city, &t, 6.8, 5.0, |c| {
        for a in c.agentes.e.agentes.iter().filter(|a| a.proposito == Proposito::Aguador) {
            assert_eq!(a.tipo, Tipo::Carro);
            con_pipa += 1;
            if a.marcha == Marcha::DejaAgua {
                parados += 1;
            }
        }
    });
    let st = city.agentes.e.stats;
    assert!(st.aguadores_hoy > 0, "{st:?}");
    assert!(st.casas_con_agua_hoy > 0, "dejaron agua: {st:?}");
    assert!(con_pipa > 0 && parados > 0, "se los ve parar en las puertas: {parados} de {con_pipa}");
    // El domingo descansan.
    let mut dom = ciudad_con_servicios(&t);
    dom.agentes.e.dia = 13;
    correr_con(&mut dom, &t, 6.8, 5.0, |_| {});
    assert_eq!(dom.agentes.e.stats.aguadores_hoy, 0);
}

#[test]
fn los_serenos_encienden_los_faroles_y_rondan() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad_con_servicios(&t);
    assert!(!city.services.lamps.is_empty(), "el puesto de serenos pone faroles");
    let mut serenos_parados = 0;
    correr_con(&mut city, &t, 19.4, 3.5, |c| {
        serenos_parados += c.agentes.e.agentes.iter().filter(|a| matches!(a.proposito, Proposito::EnciendeFaroles | Proposito::Ronda) && a.marcha == Marcha::CantaLaHora).count();
    });
    let st = city.agentes.e.stats;
    assert!(st.rondas_hoy >= 2, "encienden y rondan: {st:?}");
    assert!(st.faroles_hoy > 0, "{st:?}");
    assert!(serenos_parados > 0);
    // De día no hay serenos en la calle.
    let mut dia = ciudad_con_servicios(&t);
    correr_con(&mut dia, &t, 10.0, 2.0, |c| assert!(c.agentes.e.agentes.iter().all(|a| !matches!(a.proposito, Proposito::EnciendeFaroles | Proposito::Ronda))));
}

#[test]
fn un_incendio_junta_curiosos_y_una_cadena_de_baldes() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad_con_servicios(&t);
    let pozo = city.services.of_kind(Kind::Pozo).next().unwrap().c;
    // La casa más cercana al pozo se prende.
    let lote = city.lots.lots.values().filter(|l| l.level > 0).min_by(|a, b| a.c.distance(pozo).total_cmp(&b.c.distance(pozo))).unwrap().id;
    city.services.fires.burning.insert(lote, crate::city::services::fires::Burn { i: 0.4, full: 0, fought: false });
    let mut miran = 0;
    correr_con(&mut city, &t, 15.0, 0.6, |c| miran += c.agentes.e.agentes.iter().filter(|a| a.marcha == Marcha::MiraElFuego).count());
    let inc = city.agentes.e.incendios.iter().find(|i| i.lote == lote).expect("el incendio se ve");
    assert!(inc.fuente.is_some(), "hay un pozo que alcanza: cadena de baldes");
    assert!(city.agentes.e.stats.curiosos_hoy > 2, "{:?}", city.agentes.e.stats);
    assert!(miran > 0, "los curiosos se quedan mirando");
    // La cadena se dibuja (gente con baldes entre el pozo y la casa).
    let pos = Vec2::from(inc.pos);
    let mut con = Vec::new();
    fauna::cajas_ciudad(&city, &t, pos, 15.0, &mut con);
    let guardados = std::mem::take(&mut city.agentes.e.incendios);
    let mut sin = Vec::new();
    fauna::cajas_ciudad(&city, &t, pos, 15.0, &mut sin);
    city.agentes.e.incendios = guardados;
    assert!(con.len() > sin.len() + 40, "la cadena de baldes: {} cajas contra {}", con.len(), sin.len());
    // Pasado el rato, el incendio deja de verse.
    city.agentes.e.reloj += 10_000.0;
    city.services.fires.burning.clear();
    city.tick_agentes(&t, 1.0 / 30.0);
    assert!(city.agentes.e.incendios.is_empty());
}

/// Una calle recta de 400 m con una carreta empantanada en el medio.
fn carreta_en_el_barro() -> (RoadNetwork, Agentes, Vec2) {
    let t = TerrainGen::new(1726);
    let o = Vec2::new(13000.0, 16000.0);
    let mut net = RoadNetwork::default();
    net.add_road(&line(o, o + Vec2::new(400.0, 0.0)), RoadKind::Dirt);
    let mut ag = Agentes::default();
    ag.preparar(&t, &net, o, 0.0);
    ag.e.hora = 10.0;
    assert!(ag.lanzar(&net, Tipo::Carreta, o + Vec2::new(150.0, 0.0), o + Vec2::new(395.0, 0.0), None, Proposito::TraeDelCampo, 1, 1, (0, 0)));
    let c = ag.e.agentes.last_mut().unwrap();
    c.detenido_hasta = 40.0 * 60.0;
    (net, ag, o)
}

#[test]
fn la_carreta_empantanada_espera_ayuda() {
    // Sin nadie que pase, sigue empantanada.
    let (net, mut ag, o) = carreta_en_el_barro();
    for _ in 0..(10.0 * 60.0 / PASO) as usize {
        ag.paso(&net, PASO);
    }
    assert_eq!(ag.e.agentes[0].marcha, Marcha::Empantanada);
    assert_eq!(ag.e.stats.ayudas_hoy, 0);
    // Con un jinete que viene por la misma calle: se para, ayuda y la carreta sale mucho antes.
    let (net, mut ag, _) = carreta_en_el_barro();
    assert!(ag.lanzar(&net, Tipo::Jinete, o + Vec2::new(20.0, 0.0), o + Vec2::new(390.0, 0.0), None, Proposito::Paseo, 2, 0, (1, 1)));
    let mut ayudo = false;
    let mut salio = None;
    for k in 0..(15.0 * 60.0 / PASO) as usize {
        ag.paso(&net, PASO);
        ayudo |= ag.e.agentes.iter().any(|a| a.marcha == Marcha::Ayudando);
        if salio.is_none() && ag.e.agentes.iter().any(|a| a.tipo == Tipo::Carreta && a.v > 0.1) {
            salio = Some(k as f32 * PASO / 60.0);
        }
    }
    assert!(ayudo, "el jinete se paró a ayudar");
    assert_eq!(ag.e.stats.ayudas_hoy, 1);
    let m = salio.expect("la carreta salió del barro");
    assert!(m < 12.0, "salió a los {m:.1} min (sin ayuda esperaba 40)");
}

#[test]
fn la_salida_de_misa_llena_el_atrio() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad_con_servicios(&t);
    city.agentes.e.dia = 13; // domingo
    let fin = datos::datos().oficios.misa_fin_domingo;
    let mut en_misa_al_final = 0;
    let mut salen: Vec<f32> = Vec::new();
    let mut vistos: BTreeSet<u32> = BTreeSet::new();
    correr_con(&mut city, &t, 8.4, 2.6, |c| {
        let h = c.agentes.e.hora;
        if (h - fin).abs() < 0.01 {
            en_misa_al_final = en_misa_al_final.max(c.agentes.e.vecinos.values().filter(|v| v.lugar == Lugar::Iglesia && v.en_camino.is_none()).count());
        }
        for a in c.agentes.e.agentes.iter().filter(|a| a.proposito == Proposito::VuelveDeMisa) {
            if vistos.insert(a.id) {
                salen.push(h);
            }
        }
    });
    assert!(en_misa_al_final > 20, "al terminar la misa están todos: {en_misa_al_final}");
    assert!(salen.len() > 20, "{}", salen.len());
    // Nadie sale antes del final; casi todos en la media hora siguiente (charlan en el atrio;
    // los que llegaron tarde se quedan un poco más).
    assert!(salen.iter().all(|&h| h >= fin - 0.02), "{salen:?}");
    let juntos = salen.iter().filter(|&&h| h <= fin + 0.6).count();
    assert!(juntos * 10 >= salen.len() * 8, "{juntos} de {} salen juntos: {salen:?}", salen.len());
    // Y no todos a la vez: unos se quedan conversando más que otros.
    let (lo, hi) = salen.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
    assert!(hi - lo > 0.15, "salidas entre {lo} y {hi}");
}

#[test]
fn los_corrillos_no_se_pisan() {
    for (n, sem) in [(12, 1u64), (80, 7), (300, 99)] {
        let c = corrillo(n, 6.0 + (n as f32).sqrt(), sem);
        assert_eq!(c.len(), n);
        for (i, a) in c.iter().enumerate() {
            assert!(a.0.y > 0.2, "nadie dentro de la pared: {:?}", a.0);
            for b in &c[i + 1..] {
                assert!(a.0.distance(b.0) > 0.42, "dos personas encimadas: {:?} {:?}", a.0, b.0);
            }
        }
        // Determinista.
        assert_eq!(c, corrillo(n, 6.0 + (n as f32).sqrt(), sem));
    }
}

#[test]
fn las_calles_en_pendiente_tienen_rampas_en_cada_escalon() {
    let t = TerrainGen::new(1726);
    let mut city = City::default();
    city.money = 1_000_000;
    let c = Vec2::new(13500.0, 16300.0);
    city.found_plaza(&t, c, 0.0).unwrap();
    let (f, s) = plaza_frame(0.0);
    let a = c - s * (BLOCK * 1.5) - f * (BLOCK * 1.5);
    city.build_grid(&t, &grid_lines(a, a + s * BLOCK * 3.0, a + f * BLOCK * 3.0), RoadKind::Stone).unwrap();
    let plaza = city.plaza_square();
    let t0 = std::time::Instant::now();
    let n: usize = city.net.edges.keys().map(|&id| crate::city::voxelize::escalones(&t, &city.net, id, plaza).len()).sum();
    eprintln!("RAMPAS {n} en {} tramos: {:.1} ms", city.net.edges.len(), t0.elapsed().as_secs_f64() * 1000.0);
    let mut total = 0;
    for &id in city.net.edges.keys() {
        let r = crate::city::voxelize::escalones(&t, &city.net, id, plaza);
        for k in &r {
            // Losas apenas inclinadas (de 1 m por cada escalón de 25 cm, o del rellano entero),
            // con la cara de arriba en el borde de un vóxel.
            assert!(k.inclina < 0.0 && k.inclina > -0.8, "{k:?}");
            assert!((k.pos[1] / crate::lod::BASE_VOXEL).fract().abs() < 1e-3, "{k:?}");
            assert!(k.tam[2] <= crate::city::voxelize::RAMPA_ESCALON_M.hypot(crate::lod::BASE_VOXEL) * 1.01 && k.tam[0] >= crate::lod::BASE_VOXEL - 1e-4, "{k:?}");
            assert!(k.color[3] > 0, "con el matiz de los micro-vóxeles");
        }
        total += r.len();
        // Determinista.
        assert_eq!(r, crate::city::voxelize::escalones(&t, &city.net, id, plaza));
    }
    eprintln!("RAMPAS {total} en el damero de prueba");
    assert!(total > 20, "el damero de prueba tiene cuestas: {total} rampas");
}

#[test]
fn las_rampas_se_dibujan_cerca_de_la_vista() {
    let t = TerrainGen::new(1726);
    let mut city = City::default();
    city.money = 1_000_000;
    let c = Vec2::new(13500.0, 16300.0);
    city.found_plaza(&t, c, 0.0).unwrap();
    let (f, s) = plaza_frame(0.0);
    let a = c - s * (BLOCK * 1.5) - f * (BLOCK * 1.5);
    city.build_grid(&t, &grid_lines(a, a + s * BLOCK * 3.0, a + f * BLOCK * 3.0), RoadKind::Stone).unwrap();
    let plaza = city.plaza_square();
    let (id, n) = city.net.edges.keys().map(|&id| (id, crate::city::voxelize::escalones(&t, &city.net, id, plaza).len())).max_by_key(|x| x.1).unwrap();
    assert!(n > 0);
    let e = &city.net.edges[&id];
    let centro = e.point(e.len() * 0.5).0;
    city.agentes.precalentar_rampas(&city, &t, centro);
    let mut con = Vec::new();
    city.agentes.cajas_rampas(&city, &t, centro, &mut con);
    assert!(con.len() >= n, "las rampas del tramo se dibujan: {} de {n}", con.len());
    // Con todo lo que se dibuja con los vecinos (como en la ventana).
    let mut todo = Vec::new();
    fauna::cajas_ciudad(&city, &t, centro, 10.0, &mut todo);
    oficios::SIN_RAMPAS.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut sin = Vec::new();
    city.agentes.cajas_rampas(&city, &t, centro, &mut sin);
    let mut todo_sin = Vec::new();
    fauna::cajas_ciudad(&city, &t, centro, 10.0, &mut todo_sin);
    oficios::SIN_RAMPAS.store(false, std::sync::atomic::Ordering::Relaxed);
    assert!(sin.is_empty());
    assert_eq!(todo.len(), todo_sin.len() + con.len(), "las rampas van con los vecinos");
}

#[test]
fn las_rampas_apoyan_en_la_calzada_de_vóxeles() {
    // La cara de arriba de cada rampa va del borde del escalón (la calzada de arriba) a la de
    // abajo: se compara con la superficie que deja `road_edits`.
    let t = TerrainGen::new(1726);
    let mut city = City::default();
    city.money = 1_000_000;
    let c = Vec2::new(13500.0, 16300.0);
    city.found_plaza(&t, c, 0.0).unwrap();
    let (f, s) = plaza_frame(0.0);
    let a = c - s * (BLOCK * 1.5) - f * (BLOCK * 1.5);
    city.build_grid(&t, &grid_lines(a, a + s * BLOCK * 3.0, a + f * BLOCK * 3.0), RoadKind::Stone).unwrap();
    let plaza = city.plaza_square();
    let (id, _) = city.net.edges.keys().map(|&id| (id, crate::city::voxelize::escalones(&t, &city.net, id, plaza).len())).max_by_key(|x| x.1).unwrap();
    let mut e = crate::buildings::Edits::default();
    crate::city::voxelize::road_edits(&t, &city.net, id, plaza, &mut e);
    let vs = crate::lod::BASE_VOXEL;
    let (mut bien, mut total) = (0, 0);
    for k in crate::city::voxelize::escalones(&t, &city.net, id, plaza) {
        // Un poco más allá del borde (arriba) y un poco antes (abajo), en el medio de la corrida.
        let dir = Vec2::new(k.giro.sin(), k.giro.cos());
        let p = Vec2::new(k.pos[0], k.pos[2]);
        let col = |q: Vec2| glam::IVec2::new((q.x / vs).floor() as i32, (q.y / vs).floor() as i32);
        let (arriba, abajo) = (e.ground.get(&col(p + dir * 0.1)), e.ground.get(&col(p - dir * 0.1)));
        if let (Some(&ar), Some(&ab)) = (arriba, abajo) {
            total += 1;
            if ((ar + 1) as f32 * vs - k.pos[1]).abs() < 1e-3 && ar == ab + 1 {
                bien += 1;
            } else if total < 5 {
                eprintln!("rampa {k:?}: arriba {ar}, abajo {ab}");
            }
        }
    }
    eprintln!("RAMPAS apoyadas {bien} de {total}");
    assert!(total > 0 && bien * 10 >= total * 9, "{bien} de {total}");
}
