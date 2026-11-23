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
        t.registrar(&net, e, d, 2000.0, 30.0, 0.0, 8.0);
    }
    t.fotografiar(1);
    t.franja = ruta::franja(8.0);
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
        city.tick_agentes(t, dt);
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
    // Los vecinos son las personas de la población que viven en solares (todas).
    let en_solares: usize = a.lots.lots.keys().map(|&l| a.poblacion.habitantes_de_lote(l).filter(|p| p.viva()).count()).sum();
    assert!(a.poblacion.fundada && en_solares > 50);
    assert_eq!(a.agentes.e.vecinos.len(), en_solares);
    assert!(a.agentes.e.vecinos.values().all(|v| v.persona.is_some()), "todos los vecinos son personas");
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


// --- ia-y-transito.md §4.0-4.1 -------------------------------------------------------------

/// Un vecino cualquiera para las pruebas de la agenda.
pub(crate) fn vecino_de_prueba() -> Vecino {
    Vecino {
        casa: 1,
        persona: None,
        trabajo_fijo: false,
        nombre: (0, 0),
        origen: 0,
        oficio: 0,
        rol: Rol::EnCasa,
        trabajo: Trabajo::Ninguno,
        tipo: Tipo::APie,
        temple: 1,
        entrada: 6.5,
        salida: 17.5,
        semilla_ruta: 1,
        cambios_de_ruta: 0,
        lugar: Lugar::Casa,
        ultimo: None,
        ropa: 0,
        perfil: Perfil::default(),
        clase: 1,
        compra: false,
        pulperia: None,
        visita: None,
        plan: Vec::new(),
        paso_plan: 0,
        plan_dia: 0,
        sale: None,
        en_camino: None,
    }
}

/// Corre la ciudad `horas` horas y controla en cada cuadro que ningún vecino esté en dos
/// lugares: a lo sumo un agente por vecino, y el que tiene es el que el vecino dice tener.
fn correr_sin_ubicuidad(city: &mut City, t: &TerrainGen, h0: f32, horas: f32) -> (usize, usize) {
    let dt = 1.0 / 20.0;
    let cuadros = (horas / crate::city::HOURS_PER_SECOND / dt) as usize;
    let (mut max_calle, mut con_vecino) = (0, 0);
    for k in 0..cuadros {
        city.agentes.set_hora((h0 + k as f32 * dt * crate::city::HOURS_PER_SECOND) % 24.0);
        city.tick_agentes(t, dt);
        let ag = &city.agentes.e;
        max_calle = max_calle.max(ag.agentes.len());
        let mut vistos = BTreeSet::new();
        for a in &ag.agentes {
            if let Some(v) = a.vecino {
                con_vecino += 1;
                assert!(vistos.insert(v), "el vecino {v} está dos veces en la calle");
                assert_eq!(ag.vecinos.get(&v).and_then(|x| x.en_camino), Some(a.id), "el vecino {v} no sabe que está en camino");
            }
        }
        // Y nadie que figure en camino falta en la calle.
        for (k, v) in &ag.vecinos {
            if let Some(id) = v.en_camino {
                assert!(ag.agentes.iter().any(|a| a.id == id && a.vecino == Some(*k)), "el vecino {k} figura en camino pero no está");
            }
        }
    }
    (max_calle, con_vecino)
}

#[test]
fn ningun_vecino_esta_en_dos_lugares() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    // Un día hábil completo y un domingo.
    let (n, con_vecino) = correr_sin_ubicuidad(&mut city, &t, 4.0, 20.0);
    assert!(n > 5 && con_vecino > 100, "hubo gente en la calle: {n}");
    city.agentes.e.dia = 6;
    city.agentes.e.agenda_dia = None;
    correr_sin_ubicuidad(&mut city, &t, 4.0, 20.0);
    // Toda vuelta salió después de la llegada de la ida.
    let mut vueltas = 0;
    for v in city.agentes.e.vecinos.values() {
        for w in v.plan.windows(2) {
            if let (Some(llego), Some(salio)) = (w[0].llego, w[1].salio) {
                assert!(salio >= llego - 1e-9, "salió ({salio}) antes de llegar ({llego}): {:?}", v.plan);
                vueltas += 1;
            }
        }
    }
    assert!(vueltas > 20, "{vueltas} vueltas encadenadas");
}

#[test]
fn los_tiempos_olvidan_un_atasco_viejo() {
    let (net, _) = dos_orillas();
    let mut t = Tiempos::default();
    let e = *net.edges.keys().next().unwrap();
    let libre = Tiempos::libre(&net, e);
    for _ in 0..30 {
        t.registrar(&net, e, 0, libre * 20.0, libre, 0.0, 8.0);
    }
    t.fotografiar(1);
    t.franja = ruta::franja(8.0);
    t.ahora = 0.0;
    assert!(t.costo(&net, e, 0) > libre * 5.0, "atascado: {}", t.costo(&net, e, 0));
    // Nadie vuelve a pasar: en dos días de juego (9600 s simulados) vuelve a ≤ 120 % de lo libre.
    t.ahora = 2.0 * 24.0 * 200.0;
    assert!(t.costo(&net, e, 0) <= libre * 1.2, "{} frente a {libre}", t.costo(&net, e, 0));
    // A la madrugada no se sabe nada del atasco de la mañana (D7).
    t.ahora = 0.0;
    t.franja = ruta::franja(3.0);
    assert!((t.costo(&net, e, 0) - libre).abs() < 1e-3);
}

#[test]
fn el_jinete_no_evita_la_calle_que_dejaron_las_carretas() {
    let (net, _) = dos_orillas();
    let (a, b) = (nodo(&net, Vec2::new(0.0, 0.0)), nodo(&net, Vec2::new(100.0, 200.0)));
    let mut t = Tiempos { franja: ruta::franja(9.0), ..Default::default() };
    let antes = Router::default().camino(&net, &t, a, b, Some(0)).unwrap();
    // Muchas carretas recorren esa ruta a su paso normal (1,1 m/s, contra 3,8 de un jinete).
    for &(e, d) in &antes.tramos {
        let largo = net.edges[&e].len();
        let libre_carreta = largo / Tipo::Carreta.params().v0;
        for _ in 0..20 {
            t.registrar(&net, e, d, libre_carreta, libre_carreta, 0.0, 9.0);
        }
    }
    t.fotografiar(1);
    let despues = Router::default().camino(&net, &t, a, b, Some(0)).unwrap();
    assert_eq!(antes.tramos, despues.tramos, "la calle de las carretas sigue siendo la mejor para el jinete");
    // En cambio, si las carretas se atascaron ahí (el triple de lo normal para ellas), la evita.
    let (e, d) = antes.tramos[0];
    let libre_carreta = net.edges[&e].len() / Tipo::Carreta.params().v0;
    for _ in 0..20 {
        t.registrar(&net, e, d, libre_carreta * 3.0, libre_carreta, 0.0, 9.0);
    }
    t.fotografiar(2);
    let atasco = Router::default().camino(&net, &t, a, b, Some(0)).unwrap();
    assert_ne!(atasco.tramos[0], (e, d), "evita la calle atascada");
}

/// Una calle recta de 600 m, con la plaza en un extremo y un portón en el medio.
fn calle_con_porton() -> (TerrainGen, RoadNetwork, Agentes, Vec2) {
    let t = TerrainGen::new(1726);
    let o = Vec2::new(13000.0, 16000.0);
    let mut net = RoadNetwork::default();
    net.add_road(&line(o, o + Vec2::new(600.0, 0.0)), RoadKind::Stone);
    let mut ag = Agentes::default();
    let g = o + Vec2::new(300.0, 0.0);
    ag.set_muralla(7, vec![(g - Vec2::Y * 60.0, g + Vec2::Y * 60.0)], vec![g]);
    ag.preparar(&t, &net, o, 0.0);
    assert_eq!(ag.portones.len(), 1, "el portón quedó sobre la calle");
    ag.e.hora = 10.0;
    (t, net, ag, o)
}

/// Larga `n` carretas cargadas del extremo de afuera hacia la plaza, de a una, separadas.
fn carretas_hacia_el_porton(ag: &mut Agentes, net: &RoadNetwork, o: Vec2, n: usize) {
    for k in 0..n {
        assert!(ag.lanzar(net, Tipo::Carreta, o + Vec2::new(595.0, 0.0), o + Vec2::new(5.0, 0.0), None, Proposito::TraeDelCampo, k as u32, 1, (0, 0)));
        ag.e.agentes.last_mut().unwrap().cargada = true;
        assert_eq!(ag.e.agentes.last().unwrap().portones.len(), 1, "la ruta pasa por el portón");
        for _ in 0..80 {
            ag.paso(net, PASO);
        }
    }
}

#[test]
fn el_porton_forma_cola_y_cobra() {
    let (_t, net, mut ag, o) = calle_con_porton();
    carretas_hacia_el_porton(&mut ag, &net, o, 6);
    let mut en_revision = false;
    for _ in 0..12_000 {
        ag.paso(&net, PASO);
        en_revision |= ag.e.agentes.iter().any(|a| a.marcha == Marcha::EnRevision);
        if ag.e.agentes.is_empty() {
            break;
        }
    }
    let st = ag.e.portones.values().next().copied().unwrap();
    assert!(en_revision, "el guarda revisa la carga");
    assert_eq!(st.pasados_hoy, 6, "{st:?}");
    let reales = datos::datos().porton.derecho_reales.carreta_cargada * 6.0;
    assert!((st.reales_hoy - reales).abs() < 1e-3, "cobró {} reales (esperaba {reales})", st.reales_hoy);
    assert!(st.cola_max_hoy >= 3, "se formó cola: {st:?}");
    let espera = st.espera_media().unwrap();
    assert!(espera > 30.0, "la espera media ({espera} s) incluye la revisión de las de adelante");
    // La ficha del portón lo cuenta.
    let (_, l) = ag.ficha_porton(o + Vec2::new(300.0, 3.0)).unwrap();
    assert!(l.iter().any(|x| x.contains("Pasaron hoy: 6")), "{l:?}");
}

#[test]
fn de_noche_el_porton_esta_cerrado() {
    let (_t, net, mut ag, o) = calle_con_porton();
    ag.e.hora = 23.0;
    carretas_hacia_el_porton(&mut ag, &net, o, 2);
    assert!(ag.lanzar(&net, Tipo::APie, o + Vec2::new(590.0, 0.0), o + Vec2::new(10.0, 0.0), None, Proposito::Paseo, 9, 1, (0, 0)));
    for _ in 0..4000 {
        ag.paso(&net, PASO);
    }
    // Nadie pasó: esperan afuera, del lado de afuera del portón.
    assert_eq!(ag.e.agentes.len(), 3);
    assert!(ag.e.agentes.iter().all(|a| a.porton_i == 0 && a.s < a.portones[0].1), "nadie cruza el portón cerrado");
    assert!(ag.e.agentes.iter().any(|a| a.marcha == Marcha::EsperandoElPorton));
    assert_eq!(ag.e.portones.values().next().unwrap().pasados_hoy, 0);
    let (_, l) = ag.ficha_porton(o + Vec2::new(300.0, 0.0)).unwrap();
    assert!(l[0].starts_with("Cerrado"), "{l:?}");
    // A la mañana abre y pasan.
    ag.e.hora = 6.5;
    for _ in 0..8000 {
        ag.paso(&net, PASO);
    }
    assert!(ag.e.agentes.is_empty(), "abrió y siguieron: {:?}", ag.e.agentes.iter().map(|a| (a.tipo, a.marcha, a.s)).collect::<Vec<_>>());
    assert_eq!(ag.e.portones.values().next().unwrap().pasados_hoy, 3);
}

#[test]
fn los_contrabandistas_evitan_el_porton() {
    // Dos caminos: uno con portón, el otro (más largo) sin.
    let t = TerrainGen::new(1726);
    let o = Vec2::new(13000.0, 16000.0);
    let mut net = RoadNetwork::default();
    net.add_road(&line(o, o + Vec2::new(600.0, 0.0)), RoadKind::Stone);
    net.add_road(&line(o, o + Vec2::new(0.0, 300.0)), RoadKind::Dirt);
    net.add_road(&line(o + Vec2::new(0.0, 300.0), o + Vec2::new(600.0, 300.0)), RoadKind::Dirt);
    net.add_road(&line(o + Vec2::new(600.0, 300.0), o + Vec2::new(600.0, 0.0)), RoadKind::Dirt);
    let mut ag = Agentes::default();
    let g = o + Vec2::new(300.0, 0.0);
    ag.set_muralla(7, vec![(g - Vec2::Y * 60.0, g + Vec2::Y * 60.0)], vec![g]);
    ag.preparar(&t, &net, o, 0.0);
    // Del camino de afuera (este) al de adentro (oeste).
    let (a, b) = (o + Vec2::new(600.0, 100.0), o + Vec2::new(0.0, 100.0));
    assert!(ag.lanzar(&net, Tipo::Carreta, a, b, None, Proposito::TraeDelCampo, 1, 1, (0, 0)));
    assert!(ag.lanzar_con(&net, Tipo::Carreta, a, b, None, Proposito::TraeDelCampo, 1, 1, (0, 0), Some(1)));
    let (normal, contra) = (&ag.e.agentes[0], &ag.e.agentes[1]);
    assert_eq!(normal.portones.len(), 1);
    assert!(contra.portones.is_empty() && contra.contrabando && contra.ruta == Eleccion::SinPortones);
    assert!(contra.patas.iter().all(|p| !ag.veto_portones.contains(&p.tramo)));
}

/// Una «ciudad» en cruz alrededor de la plaza, un camino del campo desde el norte y los
/// corrales al oeste, con un desvío por fuera del recinto más largo que cruzar la ciudad.
fn cruz_con_campo() -> (TerrainGen, RoadNetwork, Agentes, Vec2) {
    let t = TerrainGen::new(1726);
    let c = Vec2::new(13000.0, 16000.0);
    let p = |x: f32, y: f32| c + Vec2::new(x, y);
    let mut net = RoadNetwork::default();
    for (a, b, k) in [
        (p(0.0, 0.0), p(0.0, -200.0), RoadKind::Stone),
        (p(0.0, 0.0), p(-200.0, 0.0), RoadKind::Stone),
        (p(0.0, 0.0), p(200.0, 0.0), RoadKind::Stone),
        (p(0.0, 0.0), p(0.0, 200.0), RoadKind::Stone),
        (p(0.0, -200.0), p(0.0, -700.0), RoadKind::Dirt),
        (p(0.0, -700.0), p(0.0, -1200.0), RoadKind::Dirt),
        (p(-200.0, 0.0), p(-520.0, 0.0), RoadKind::Dirt),
        (p(0.0, -700.0), p(-700.0, -700.0), RoadKind::Dirt),
        (p(-700.0, -700.0), p(-700.0, 0.0), RoadKind::Dirt),
        (p(-700.0, 0.0), p(-520.0, 0.0), RoadKind::Dirt),
    ] {
        net.add_road(&line(a, b), k);
    }
    let mut ag = Agentes::default();
    ag.preparar(&t, &net, c, 0.0);
    ag.e.hora = 9.0;
    (t, net, ag, c)
}

#[test]
fn el_arreo_no_entra_intramuros() {
    let (_t, net, mut ag, c) = cruz_con_campo();
    assert!(ag.lugares.campo.unwrap().distance(c + Vec2::new(0.0, -1200.0)) < 1.0, "{:?}", ag.lugares.campo);
    assert!(ag.lugares.corrales.unwrap().distance(c + Vec2::new(-520.0, 0.0)) < 1.0, "{:?}", ag.lugares.corrales);
    // El camino más corto (a pie, sin restricciones) cruza la ciudad...
    let corto = ag.planificar(&net, ag.lugares.campo.unwrap(), ag.lugares.corrales.unwrap(), None).unwrap();
    assert!(corto.iter().any(|p| ag.intramuros.contains(&p.tramo)));
    // ...pero la tropa va por afuera.
    assert!(ag.lanzar_arreo(&net));
    let a = ag.e.agentes.last().unwrap().clone();
    assert_eq!(a.tipo, Tipo::Arreo);
    assert!(a.cabezas >= datos::datos().tropas.cabezas_min as u16 && a.largo() > 20.0);
    assert!(a.patas.iter().all(|p| !ag.intramuros.contains(&p.tramo)), "el arreo no entra intramuros");
    assert!(a.patas.iter().all(|p| net.edges[&p.tramo].pts.iter().all(|q| q.distance(c) >= ag.radio_intramuros - 1.0)));
    // Y llega a los corrales; los troperos vuelven a caballo.
    for _ in 0..40_000 {
        ag.paso(&net, PASO);
        if ag.e.agentes.is_empty() {
            break;
        }
    }
    assert!(ag.e.agentes.is_empty(), "la tropa llegó");
    assert!(ag.e.pendientes.iter().any(|p| p.tipo == Tipo::Jinete && p.que == Proposito::VuelveAlCampo));
}

#[test]
fn nadie_adelanta_un_arreo_pero_si_una_carreta() {
    // Calles rectas largas: un jinete apurado detrás de un arreo, otro detrás de una carreta.
    let t = TerrainGen::new(1726);
    let o = Vec2::new(13000.0, 16000.0);
    let mut net = RoadNetwork::default();
    net.add_road(&line(o, o + Vec2::new(900.0, 0.0)), RoadKind::Stone);
    net.add_road(&line(o + Vec2::new(0.0, 300.0), o + Vec2::new(900.0, 300.0)), RoadKind::Stone);
    let mut ag = Agentes::default();
    ag.preparar(&t, &net, o + Vec2::new(-5000.0, 0.0), 0.0);
    let mut ids = Vec::new();
    for (y, lento) in [(0.0, Tipo::Arreo), (300.0, Tipo::Carreta)] {
        let (a, b) = (o + Vec2::new(5.0, y), o + Vec2::new(895.0, y));
        assert!(ag.lanzar(&net, lento, a, b, None, Proposito::Paseo, 1, 1, (0, 0)));
        ids.push(ag.e.agentes.last().unwrap().id);
        if lento == Tipo::Arreo {
            let x = ag.e.agentes.last_mut().unwrap();
            x.cabezas = 90;
            x.largo_extra = 48.0;
            x.v0 = 0.83;
        }
        for _ in 0..if lento == Tipo::Arreo { 700 } else { 300 } {
            ag.paso(&net, PASO);
        }
        assert!(ag.lanzar(&net, Tipo::Jinete, a, b, None, Proposito::Paseo, 2, 2, (0, 0)));
        ids.push(ag.e.agentes.last().unwrap().id);
        // Un jinete apurado y poco prudente.
        let j = ag.e.agentes.last_mut().unwrap();
        j.perfil.prudencia = 20;
        j.perfil.prisa = 230;
    }
    let (arreo, jinete_a, carreta, jinete_c) = (ids[0], ids[1], ids[2], ids[3]);
    let mut paso_carreta = false;
    for _ in 0..6000 {
        ag.paso(&net, PASO);
        let s = |id: u32| ag.e.agentes.iter().find(|a| a.id == id).map(|a| a.s);
        if let (Some(sa), Some(sj)) = (s(arreo), s(jinete_a)) {
            assert!(sj < sa - 1.0, "el jinete adelantó al arreo");
            assert!(ag.e.agentes.iter().find(|a| a.id == jinete_a).unwrap().adelantando.is_none());
        }
        match (s(carreta), s(jinete_c)) {
            (Some(sc), Some(sj)) if sj > sc + 3.0 => paso_carreta = true,
            (Some(_), None) => paso_carreta = true,
            _ => {}
        }
    }
    assert!(paso_carreta, "el jinete apurado adelantó a la carreta");
    assert!(ag.e.stats.adelantamientos_hoy >= 1);
}

#[test]
fn el_perfil_es_determinista() {
    let t = TerrainGen::new(1726);
    let a = ciudad(&t);
    let b = ciudad(&t);
    // Misma persona → mismo perfil.
    assert!(!a.agentes.e.vecinos.is_empty());
    for (k, v) in &a.agentes.e.vecinos {
        assert_eq!(Some(&v.perfil), b.agentes.e.vecinos.get(k).map(|w| &w.perfil));
    }
    let p = a.agentes.e.vecinos.values().find_map(|v| v.persona).unwrap();
    let persona = a.poblacion.persona(p).unwrap();
    let clase = crate::city::poblacion::modelo::Clase::Modesta;
    let uno = Perfil::de_persona(persona, clase, 0, a.poblacion.mes, 9);
    // Sortear otras cosas (otros motivos, otras personas) no cambia el perfil.
    let mut x = 0.0;
    for i in 0..100 {
        x += perfil::azar(9, i, 3, Motivo::Compra);
    }
    assert!(x > 0.0);
    assert_eq!(uno, Perfil::de_persona(persona, clase, 0, a.poblacion.mes, 9));
    // Los planes de un vecino no dependen de los demás: quitar la mitad de los vecinos no
    // cambia el plan de los que quedan.
    let dia = agenda::Dia { dia: 10, hay_pulperia: true, ..Default::default() };
    let planes: Vec<_> = a.agentes.e.vecinos.iter().map(|(k, v)| agenda::plan(a.agentes.e.semilla, *k, v, &dia)).collect();
    let mitad: Vec<_> = a.agentes.e.vecinos.iter().step_by(2).map(|(k, v)| agenda::plan(a.agentes.e.semilla, *k, v, &dia)).collect();
    for (i, p) in mitad.iter().enumerate() {
        assert_eq!(p, &planes[i * 2]);
    }
}

#[test]
fn los_agentes_son_las_personas_y_se_visten_como_ellas() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    correr(&mut city, &t, 5.5, 2.5);
    let pob = &city.poblacion;
    let con_persona: Vec<&Agente> = city.agentes.e.agentes.iter().filter(|a| a.vecino.is_some()).collect();
    assert!(!con_persona.is_empty());
    let v = dibujo::Vestuario { poblacion: Some(pob), lots: Some(&city.lots), anio: 1760, t: 0.0 };
    for a in con_persona {
        let id = a.persona.expect("el agente de un vecino es una persona");
        let p = pob.persona(id).unwrap();
        // El personaje dibujado es esa persona: sexo, edad y oficio.
        let q = dibujo::quien(&city.agentes, a, &v);
        assert_eq!(q.mujer, p.sexo == crate::city::poblacion::modelo::Sexo::Mujer);
        assert_eq!(q.edad as i32, p.edad(pob.mes).clamp(0, 120));
        assert_eq!(q.oficio, p.oficio);
        // Y la ficha lo nombra, con su agenda del día.
        let (titulo, l) = city.agentes.describir(a.id, Some(pob)).unwrap();
        assert_eq!(titulo, pob.nombre_completo(id));
        assert!(l.iter().any(|x| x.starts_with("Agenda del")), "{l:?}");
    }
    // Trabajo: el de la persona.
    for v in city.agentes.e.vecinos.values() {
        if let (Some(p), Trabajo::Solar(w)) = (v.persona, v.trabajo) {
            assert_eq!(pob.lugar_de_trabajo(p), Some(w));
        }
    }
}

#[test]
fn en_el_cruce_la_calle_empedrada_tiene_prioridad() {
    // Cruce en T: la empedrada de este a oeste, la de tierra llega del sur.
    let t = TerrainGen::new(1726);
    let o = Vec2::new(13000.0, 16000.0);
    let mut net = RoadNetwork::default();
    net.add_road(&line(o + Vec2::new(-300.0, 0.0), o + Vec2::new(300.0, 0.0)), RoadKind::Stone);
    net.add_road(&line(o, o + Vec2::new(0.0, 300.0)), RoadKind::Dirt);
    let mut ag = Agentes::default();
    ag.preparar(&t, &net, o + Vec2::new(-5000.0, 0.0), 0.0);
    // Una fila de jinetes por la empedrada; un carro prudente por la de tierra, que llega
    // mientras pasan.
    for k in 0..8 {
        assert!(ag.lanzar(&net, Tipo::Jinete, o + Vec2::new(-200.0, 0.0), o + Vec2::new(250.0, 0.0), None, Proposito::Paseo, 3 + k, 1, (0, 0)));
        for _ in 0..14 {
            ag.paso(&net, PASO);
        }
    }
    // El carro sale cuando el primer jinete está por llegar al cruce.
    while ag.e.agentes.iter().all(|a| pose(&net, a).0.distance(o) > 15.0) {
        ag.paso(&net, PASO);
    }
    assert!(ag.lanzar(&net, Tipo::Carro, o + Vec2::new(0.0, 30.0), o + Vec2::new(-250.0, 0.0), None, Proposito::Paseo, 1, 0, (0, 0)));
    let carro = ag.e.agentes.last().unwrap().id;
    ag.e.agentes.last_mut().unwrap().perfil.prudencia = 250;
    let mut cedio = false;
    for _ in 0..3000 {
        ag.paso(&net, PASO);
        let c = ag.e.agentes.iter().find(|a| a.id == carro);
        cedio |= c.is_some_and(|c| c.marcha == Marcha::CediendoElPaso);
        if c.is_none() {
            break;
        }
    }
    assert!(cedio, "el carro de la calle de tierra cedió el paso");
    assert!(!ag.e.agentes.iter().any(|a| a.id == carro), "y después pasó (no se traba)");
}

#[test]
fn una_partida_vieja_carga() {
    // Estado de antes de la IA colonial: tiempos como mapa de carril → segundos, sin los campos
    // nuevos de vecinos, agentes y estado.
    let viejo = r#"{"vecinos":{"256":{"casa":1,"nombre":[0,1],"origen":0,"oficio":2,"rol":"Fuera","trabajo":"Campo","tipo":"APie","temple":1,"entrada":6.5,"salida":17.0,"semilla_ruta":3,"cambios_de_ruta":0,"lugar":"Casa","ultimo":null,"ropa":2}},"agentes":[],"tiempos":{"12":34.5},"reloj":100.0,"hora":8.0,"dia":3,"agenda":[{"hora":9.0,"quien":{"Vecino":256},"que":"AlTrabajo"}],"agenda_i":0,"agenda_dia":3,"pendientes":[],"cruces":{},"azar":5,"proximo_id":1,"acumulado":0.0,"pasos":0,"stats":{"viajes_total":0,"viajes_hoy":0,"viajes_ayer":0,"omitidos_hoy":0,"minutos_hoy":0.0,"demorados_hoy":0,"minutos_ayer":0.0,"demorados_ayer":0,"viajes_mes":0,"demora_mes":0.0,"carretas_al_mercado":0}}"#;
    let e: Estado = serde_json::from_str(viejo).expect("la partida vieja carga");
    assert_eq!(e.vecinos.len(), 1);
    assert!(e.tiempos.demoras.is_empty() && e.salidas.is_empty() && e.semilla == 0);
}
