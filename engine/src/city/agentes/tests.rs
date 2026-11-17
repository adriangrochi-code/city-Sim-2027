use super::*;
use crate::city::lots::{zone_code, Density};
use crate::city::roads::{line, RoadKind};
use crate::city::{grid_lines, plaza_frame, City, BLOCK};

/// Dos manzanas de calles separadas por un "río": solo las une un tramo (el puente).
fn dos_orillas() -> (RoadNetwork, u32) {
    let mut net = RoadNetwork::default();
    for x in [0.0, 100.0] {
        net.add_road(&line(Vec2::new(x, 0.0), Vec2::new(x, 200.0)), RoadKind::Stone);
        net.add_road(&line(Vec2::new(x + 300.0, 0.0), Vec2::new(x + 300.0, 200.0)), RoadKind::Stone);
    }
    for y in [0.0, 100.0, 200.0] {
        net.add_road(&line(Vec2::new(0.0, y), Vec2::new(100.0, y)), RoadKind::Stone);
        net.add_road(&line(Vec2::new(300.0, y), Vec2::new(400.0, y)), RoadKind::Stone);
    }
    let antes: Vec<u32> = net.edges.keys().copied().collect();
    net.add_road(&line(Vec2::new(100.0, 200.0), Vec2::new(300.0, 200.0)), RoadKind::Dirt);
    let puente = *net.edges.keys().find(|k| !antes.contains(k)).unwrap();
    (net, puente)
}

fn nodo(net: &RoadNetwork, p: Vec2) -> u32 {
    *net.nodes.iter().find(|(_, q)| q.distance(p) < 0.5).unwrap().0
}

#[test]
fn la_ruta_encuentra_el_camino_y_cruza_el_puente() {
    let (net, puente) = dos_orillas();
    let mut r = Router::default();
    let t = Tiempos::default();
    let (a, b) = (nodo(&net, Vec2::new(0.0, 0.0)), nodo(&net, Vec2::new(400.0, 0.0)));
    for perc in [None, Some(3)] {
        let c = r.camino(&net, &t, a, b, perc).expect("hay camino");
        assert!(c.tramos.iter().any(|x| x.0 == puente), "la única forma de cruzar es el puente");
        // Los tramos encadenan: cada uno empieza donde termina el anterior.
        let mut n = a;
        for &(e, d) in &c.tramos {
            let ed = &net.edges[&e];
            assert_eq!(if d == 0 { ed.a } else { ed.b }, n);
            n = if d == 0 { ed.b } else { ed.a };
        }
        assert_eq!(n, b);
    }
    // A pie: el más corto (100 + 200 + 200 + 100 + 200 = 800 m).
    let c = r.camino(&net, &t, a, b, None).unwrap();
    assert!((c.costo - 800.0).abs() < 1.0, "{}", c.costo);
    // La segunda vez sale de la caché.
    let busq = r.busquedas;
    r.camino(&net, &t, a, b, None).unwrap();
    assert_eq!(r.busquedas, busq);
    // Sin el puente no hay camino.
    let mut sin = dos_orillas().0;
    sin.remove_edge(puente);
    let (a2, b2) = (nodo(&sin, Vec2::new(0.0, 0.0)), nodo(&sin, Vec2::new(400.0, 0.0)));
    assert!(Router::default().camino(&sin, &t, a2, b2, None).is_none());
}

#[test]
fn los_vehiculos_evitan_el_carril_lento() {
    let (net, _) = dos_orillas();
    let mut t = Tiempos::default();
    let (a, b) = (nodo(&net, Vec2::new(0.0, 0.0)), nodo(&net, Vec2::new(100.0, 200.0)));
    let rapido = Router::default().camino(&net, &t, a, b, Some(0)).unwrap();
    // Se atasca el primer tramo de esa ruta: ahora conviene la otra.
    let (e, d) = rapido.tramos[0];
    for _ in 0..20 {
        t.registrar(&net, e, d, 2000.0);
    }
    let otra = Router::default().camino(&net, &t, a, b, Some(0)).unwrap();
    assert!(otra.tramos[0] != (e, d), "evita el carril atascado");
}

#[test]
fn planificar_desde_puntos_arma_patas_continuas() {
    let (net, puente) = dos_orillas();
    let mut ag = Agentes::default();
    let patas = ag.planificar(&net, Vec2::new(5.0, 50.0), Vec2::new(395.0, 150.0), None).unwrap();
    assert!(patas.iter().any(|p| p.tramo == puente));
    for w in patas.windows(2) {
        assert_eq!(fin(&net, &w[0]), {
            let e = &net.edges[&w[1].tramo];
            if w[1].sentido == 0 {
                e.a
            } else {
                e.b
            }
        });
    }
    let largo: f32 = patas.iter().map(|p| p.hasta - p.desde).sum();
    assert!((largo - (150.0 + 100.0 + 200.0 + 100.0 + 50.0)).abs() < 2.0, "{largo}");
}

/// Ciudad chica fundada, con damero y zonas, crecida unos años.
fn ciudad(t: &TerrainGen) -> City {
    let mut city = City::default();
    city.money = 1_000_000;
    let c = Vec2::new(13500.0, 16300.0);
    city.found_plaza(t, c, 0.0).unwrap();
    let (f, s) = plaza_frame(0.0);
    let a = c - s * (BLOCK * 1.5) - f * (BLOCK * 1.5);
    city.build_grid(t, &grid_lines(a, a + s * BLOCK * 3.0, a + f * BLOCK * 3.0), RoadKind::Stone).unwrap();
    city.paint(c, 300.0, zone_code(ZoneKind::Residencial, Density::Media));
    city.paint(c, 60.0, zone_code(ZoneKind::Comercial, Density::Media));
    city.fast_forward(t, 36);
    // Empieza un día nuevo (con la agenda de todos los vecinos de hoy).
    city.agentes.e.dia += 1;
    city
}

/// Corre la ciudad desde las `h0` horas durante `horas` horas del reloj, en cuadros de 1/30 s,
/// y controla en cada cuadro que nadie se monte sobre el de adelante en su carril.
fn correr(city: &mut City, t: &TerrainGen, h0: f32, horas: f32) -> usize {
    let dt = 1.0 / 30.0;
    let cuadros = (horas / crate::city::HOURS_PER_SECOND / dt) as usize;
    let mut max_en_calle = 0;
    for k in 0..cuadros {
        city.agentes.set_hora((h0 + k as f32 * dt * crate::city::HOURS_PER_SECOND) % 24.0);
        city.agentes.tick(t, &city.net, &city.lots, city.plaza, dt);
        max_en_calle = max_en_calle.max(city.agentes.e.agentes.len());
        let mut por_carril: BTreeMap<u32, Vec<(f32, f32)>> = BTreeMap::new();
        for a in city.agentes.e.agentes.iter().filter(|a| a.tipo.vehiculo()) {
            por_carril.entry(carril(a.pata().tramo, a.pata().sentido)).or_default().push((a.s, a.params().largo));
        }
        for v in por_carril.values_mut() {
            v.sort_by(|x, y| y.0.total_cmp(&x.0));
            for w in v.windows(2) {
                assert!(w[0].0 - w[0].1 - w[1].0 > -0.05, "dos vehículos superpuestos: {w:?}");
            }
        }
    }
    max_en_calle
}

#[test]
fn un_dia_de_la_ciudad_sin_choques_y_determinista() {
    let t = TerrainGen::new(1726);
    let mut a = ciudad(&t);
    assert!(a.population() > 50);
    let mut b = ciudad(&t);
    let n = correr(&mut a, &t, 5.5, 3.0);
    correr(&mut b, &t, 5.5, 3.0);
    let st = a.agentes.e.stats;
    assert!(n > 5, "a la mañana hay gente en la calle: {n}");
    assert!(st.viajes_total > 5, "viajes terminados: {st:?}");
    assert!(!a.agentes.e.vecinos.is_empty() && a.agentes.e.vecinos.len() as i32 == a.population());
    assert!(a.agentes.e == b.agentes.e, "misma ciudad, mismas horas: misma simulación");
    // Guardar y cargar a mitad del día: sigue igual que si no se hubiera guardado.
    let data = crate::city::save::CityData::from_city(&a);
    let json = serde_json::to_string(&data).unwrap();
    let mut c = serde_json::from_str::<crate::city::save::CityData>(&json).unwrap().to_city().unwrap();
    assert!(c.agentes.e == a.agentes.e);
    correr(&mut a, &t, 8.5, 1.0);
    correr(&mut c, &t, 8.5, 1.0);
    assert!(c.agentes.e == a.agentes.e, "la partida cargada sigue igual");
}

#[test]
fn el_domingo_van_a_misa() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.agentes.e.dia = 6;
    correr(&mut city, &t, 8.9, 0.6);
    let misa = city.agentes.e.agentes.iter().filter(|a| a.proposito == Proposito::AMisa).count();
    assert!(misa > 3, "{misa} van a misa");
    assert!(city.agentes.e.agentes.iter().all(|a| a.proposito != Proposito::AlTrabajo), "el domingo no se trabaja");
}

#[test]
fn sobre_el_puente_van_por_el_tablero() {
    let t = TerrainGen::montevideo();
    let mut city = City::default();
    city.money = 1_000_000;
    city.found_plaza(&t, t.founding_site(), 0.0).unwrap();
    // Un arroyo cualquiera en un tramo recto: un camino que lo cruza (como en la prueba de
    // puentes de la ciudad).
    let mut linea = None;
    'busca: for arr in &t.arroyos {
        for w in arr.windows(2).step_by(7) {
            let (p, q) = (w[0], w[1]);
            let n = (q - p).normalize_or_zero().perp();
            let m = (p + q) * 0.5;
            let (x0, x1) = (m - n * 30.0, m + n * 30.0);
            if !t.is_water(x0.x, x0.y) && !t.is_water(x1.x, x1.y) && t.is_water(m.x, m.y) && m.x > 2000.0 && m.y > 2000.0 && m.x < 22000.0 && m.y < 22000.0 {
                let l = line(x0, x1);
                if city.road_problem(&t, &l, RoadKind::Stone).is_none() {
                    linea = Some((x0, x1));
                    break 'busca;
                }
            }
        }
    }
    let (x0, x1) = linea.expect("un arroyo para cruzar");
    city.build_road(&t, &line(x0, x1), RoadKind::Stone).unwrap();
    let mid = (x0 + x1) * 0.5;
    city.agentes.tick(&t, &city.net, &city.lots, city.plaza, 0.0);
    let net = &city.net;
    assert!(city.agentes.lanzar(net, Tipo::Carreta, x0, x1, None, Proposito::Paseo, 1, 1, (0, 0)));
    let id = city.agentes.e.agentes[0].id;
    let mut sobre = None;
    for _ in 0..2000 {
        city.agentes.paso(net, PASO);
        match city.agentes.posicion(net, id) {
            Some(p) if p.distance(mid) < 4.0 => {
                let y = city.agentes.altura_de(net, id);
                if let Some(y) = y {
                    sobre = Some(sobre.map_or(y, |s: f32| s.max(y)));
                }
            }
            None => break,
            _ => {}
        }
    }
    let y = sobre.expect("la carreta llegó al medio del puente");
    let agua = t.height(mid.x, mid.y).max(0.0);
    assert!(y > agua + 2.0, "el tablero ({y} m) va por encima del agua ({agua} m)");
}

/// Medición (no corre por defecto): `cargo test --release rendimiento -- --ignored --nocapture`.
/// La ciudad de prueba con el tope de agentes en la calle (viajes al azar entre solares).
#[test]
#[ignore]
fn rendimiento_con_el_tope_de_agentes() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.agentes.tick(&t, &city.net, &city.lots, city.plaza, 0.0);
    let frentes: Vec<Vec2> = city.lots.lots.values().map(frente).collect();
    let mut r = Rng::new(7);
    let mut k = 0;
    while city.agentes.e.agentes.len() < MAX_AGENTES && k < 20 * MAX_AGENTES {
        k += 1;
        let (a, b) = (frentes[r.range(0, frentes.len() as i32 - 1) as usize], frentes[r.range(0, frentes.len() as i32 - 1) as usize]);
        let tipo = match r.range(0, 19) {
            0..=2 => Tipo::Jinete,
            3..=4 => Tipo::Carreta,
            5 => Tipo::Carro,
            _ => Tipo::APie,
        };
        city.agentes.lanzar(&city.net, tipo, a, b, None, Proposito::Paseo, r.range(0, 99_999) as u32, 1, (0, 0));
    }
    let n0 = city.agentes.e.agentes.len();
    for (nombre, foco) in [("con detalle", None), ("foco lejos (LOD)", Some(Vec2::new(0.0, 0.0))), ("foco en la plaza", Some(Vec2::new(13500.0, 16300.0)))] {
        let mut c = Agentes::from_estado(city.agentes.e.clone());
        c.foco = foco;
        c.tick(&t, &city.net, &city.lots, city.plaza, 0.0);
        let t0 = std::time::Instant::now();
        let pasos = 400;
        for _ in 0..pasos {
            c.paso(&city.net, PASO);
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / pasos as f64;
        let mut cajas = Vec::new();
        let t1 = std::time::Instant::now();
        c.cajas(&city.net, Vec2::new(13500.0, 16300.0), &mut cajas);
        let ms_cajas = t1.elapsed().as_secs_f64() * 1000.0;
        // A velocidad 1 hay 40 pasos por segundo real (ESCALA_TIEMPO / PASO).
        eprintln!(
            "RENDIMIENTO {nombre}: {n0} agentes al empezar, {} al final · {ms:.3} ms por paso → {:.1} ms por segundo de juego a velocidad 1 ({:.1} % de un núcleo) · cajas de dibujo {} en {ms_cajas:.2} ms · router {} búsquedas, {} aciertos de caché",
            c.e.agentes.len(),
            ms * 40.0,
            ms * 40.0 / 10.0,
            cajas.len(),
            c.router.busquedas,
            c.router.aciertos
        );
    }
    eprintln!("RENDIMIENTO estado guardado: {} KB de JSON", serde_json::to_string(&city.agentes.e).unwrap().len() / 1024);
}

