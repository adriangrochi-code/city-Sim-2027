//! Pruebas del mapa del país: datos, pueblos por fecha, flujos al puerto, épocas, acciones,
//! guardar y cargar.

use super::datos::geo;
use super::rutas::Red;
use super::*;

fn mes_de(anio: i32) -> u32 {
    ((anio - crate::city::START_YEAR as i32) * 12) as u32
}

#[test]
fn los_datos_del_pais_tienen_la_geografia_real() {
    let g = geo();
    assert_eq!((g.ancho(), g.alto()), (510, 557));
    assert_eq!(g.departamentos.len(), 19);
    // Tierra del país: ~176.000 km² (superficie terrestre del Uruguay).
    let tierra = (0..g.alto()).flat_map(|f| (0..g.ancho()).map(move |c| (c, f))).filter(|&(c, f)| g.es_tierra(c as f32, f as f32, 1726)).count();
    assert!((165_000..185_000).contains(&tierra), "{tierra} celdas de tierra");
    // Montevideo en el departamento de Montevideo; Salto, Rivera y Rocha en los suyos.
    for (id, depto) in [("montevideo", "Montevideo"), ("salto", "Salto"), ("rivera", "Rivera"), ("rocha", "Rocha"), ("colonia", "Colonia"), ("melo", "Cerro Largo")] {
        let p = g.pueblo_hist(id).unwrap();
        assert_eq!(g.nombre_depto(g.depto(p.pos[0], p.pos[1])), Some(depto), "{id}");
    }
    // Laguna Merín y el Río de la Plata son agua; el relieve llega a las sierras del este.
    let merin = g.lagunas.iter().find(|l| l.nombre == "Laguna Merín").unwrap();
    assert!(g.es_agua(merin.pos[0], merin.pos[1], 1726));
    let mvd = g.pueblo_hist("montevideo").unwrap().pos;
    assert!(g.es_agua(mvd[0], mvd[1] + 15.0, 1726), "al sur de Montevideo, el río");
    let max = g.alt.iter().cloned().fold(0.0f32, f32::max);
    assert!((300.0..520.0).contains(&max), "altura máxima {max}");
    // Ríos con nombre: el Río Negro, el Uruguay, el Santa Lucía y el Cebollatí.
    for r in ["Río Negro", "Río Uruguay", "Río Santa Lucía", "Río Cebollatí", "Río Yí"] {
        assert!(g.rios.iter().any(|x| x.nombre == r && x.largo_km > 100.0), "{r}");
    }
    // Rincón del Bonete: agua desde 1945, valle antes.
    let bonete = g.embalses.iter().find(|e| e.nombre.starts_with("Rincón del Bonete")).unwrap();
    let celda = (0..g.capas.len()).find(|&k| g.capas[k][datos::C_EMBALSE] == bonete.indice).unwrap();
    let (c, f) = ((celda % g.ancho()) as f32, (celda / g.ancho()) as f32);
    assert!(!g.es_agua(c, f, 1900) && g.es_agua(c, f, 1950));
    assert!(g.altura(c, f) > 20.0, "el valle del Río Negro tiene relieve: {}", g.altura(c, f));
    // Geología y títulos mineros del catálogo.
    assert!(g.unidad(mvd[0] + 3.0, mvd[1] - 5.0).is_some());
    assert!(g.titulos.len() > 1500);
    // Caminos de la historia trazados sobre el relieve, de pueblo a pueblo.
    for c in &g.historia.caminos {
        let (a, b) = (g.pueblo_hist(&c.desde).unwrap().pos, g.pueblo_hist(&c.hasta).unwrap().pos);
        assert!(c.puntos.len() >= 2);
        assert!(g.km(c.puntos[0], a) < 2.0 && g.km(*c.puntos.last().unwrap(), b) < 2.0, "{}", c.nombre);
    }
}

#[test]
fn pueblos_por_fecha_de_fundacion() {
    let tiene = |p: &Pais, id: &str| p.pueblo(id).is_some();
    let p = Pais::nuevo(2);
    assert!(tiene(&p, "montevideo") && tiene(&p, "colonia") && tiene(&p, "soriano"));
    assert!(!tiene(&p, "maldonado") && !tiene(&p, "canelones"));
    let p = Pais::simulado_hasta(mes_de(1790));
    for id in ["maldonado", "san_carlos", "paysandu", "rosario", "canelones", "minas", "san_jose", "mercedes", "las_piedras", "santa_lucia"] {
        assert!(tiene(&p, id), "{id} en 1790");
    }
    for id in ["rocha", "melo", "florida", "durazno", "tacuarembo", "rivera", "artigas"] {
        assert!(!tiene(&p, id), "{id} no en 1790");
    }
    let p = Pais::simulado_hasta(mes_de(1853) + 6);
    for id in ["florida", "durazno", "tacuarembo", "artigas", "treinta_y_tres", "bella_union", "melo", "rocha"] {
        assert!(tiene(&p, id), "{id} en 1853");
    }
    assert!(!tiene(&p, "rivera") && !tiene(&p, "fray_bentos"));
    // Los años de la tabla, verificados (fuentes en historia.json).
    let g = geo();
    for (id, anio) in [("colonia", 1680), ("montevideo", 1726), ("maldonado", 1755), ("san_carlos", 1763), ("canelones", 1783), ("minas", 1783), ("san_jose", 1783), ("mercedes", 1788), ("rocha", 1793), ("melo", 1795), ("florida", 1809), ("durazno", 1821), ("tacuarembo", 1832), ("artigas", 1852), ("rivera", 1867)] {
        assert_eq!(g.pueblo_hist(id).unwrap().anio, anio, "{id}");
    }
    // Los caminos aparecen con sus pueblos.
    assert!(p.caminos.iter().all(|c| p.pueblo(&c.desde).is_some() && p.pueblo(&c.hasta).is_some()));
    assert!(p.caminos.iter().any(|c| c.nombre == "Camino Real del Norte"));
}

#[test]
fn las_carretas_llevan_cueros_y_tasajo_al_puerto() {
    let p = Pais::simulado_hasta(mes_de(1800));
    let a = &p.ultimo;
    assert!(a.cueros > 20.0 && a.carretas > 5, "{a:?}");
    assert!(a.tasajo > 0.0, "el saladero del Colla embarca tasajo: {a:?}");
    assert!(!p.flujos.is_empty());
    // Los flujos terminan en Montevideo (o en un puerto o una estación).
    let mvd = p.pueblo("montevideo").unwrap().pos;
    assert!(p.flujos.iter().filter(|f| f.destino == 0).all(|f| geo().km(*f.puntos.last().unwrap(), mvd) < 1.0));
    // El aporte entra a la economía de la ciudad.
    let mut inp = crate::city::economy::MonthInput::default();
    a.aplicar(&mut inp);
    use crate::city::economy::Res;
    assert!(inp.production[Res::Cueros as usize] > 20.0 && inp.production[Res::Sebo as usize] > 0.0);
    assert!(inp.other.iter().any(|(n, v)| n.contains("tasajo") && *v > 0) || a.derechos == 0);
    // Y la ciudad lo usa: el mes de la ciudad suma los cueros del interior.
    let mut city = crate::city::City::default();
    let sin = city.month_input().production[Res::Cueros as usize];
    city.interior = a.clone();
    let con = city.month_input().production[Res::Cueros as usize];
    assert!((con - sin - a.cueros).abs() < 0.01);
    // El juego lleva el país con el calendario de la ciudad.
    let mut game = crate::city::game::Game::default();
    let t = crate::terrain::TerrainGen::montevideo();
    game.city.money = 1_000_000;
    game.city.found_plaza(&t, t.founding_site(), 0.0).unwrap();
    let m0 = game.city.month;
    for _ in 0..40 {
        game.update(&t, crate::city::MONTH_SECONDS / game.speed.max(1.0));
    }
    assert!(game.city.month > m0);
    assert_eq!(game.pais.mes, game.city.month);
    assert_eq!(game.city.interior, game.pais.ultimo);
}

#[test]
fn caminos_mejores_y_crecientes_cambian_los_dias_de_viaje() {
    let mut p = Pais::simulado_hasta(mes_de(1835));
    let anio = p.anio();
    let k = p.pueblos.iter().position(|q| q.id == "durazno").unwrap();
    let d0 = Red::armar(&p, anio).dias_pueblo(k);
    assert!(d0.is_finite() && d0 > 3.0, "Durazno a {d0} días");
    // Un camino mejor (época 2) acorta el viaje.
    for c in p.caminos.iter_mut() {
        c.nivel = 2;
    }
    let d1 = Red::armar(&p, anio).dias_pueblo(k);
    assert!(d1 < d0, "{d1} < {d0}");
    // Con todos los ríos crecidos y solo vados, el viaje se corta o se alarga.
    for c in p.caminos.iter_mut() {
        for ps in c.pasos.iter_mut() {
            ps.obra = Obra::Vado;
        }
    }
    let rios: Vec<String> = p.caminos.iter().flat_map(|c| c.pasos.iter().map(|x| x.rio.clone())).collect();
    assert!(!rios.is_empty());
    let normal = Red::armar(&p, anio).dias_pueblo(k);
    for r in &rios {
        p.eventos.push(Evento { tipo: TipoEvento::Creciente, zona: r.clone(), centro: [0.0; 2], radio: 0.0, meses: 2 });
    }
    let crecido = Red::armar(&p, anio).dias_pueblo(k);
    assert!(crecido > normal, "creciente: {crecido} vs {normal}");
    // Con puentes en todos los pasos, la creciente no corta nada.
    for c in p.caminos.iter_mut() {
        for ps in c.pasos.iter_mut() {
            ps.obra = Obra::Puente;
        }
    }
    let con_puentes = Red::armar(&p, anio).dias_pueblo(k);
    assert!(con_puentes <= normal + 1e-3);
}

#[test]
fn cada_epoca_habilita_sus_obras() {
    use crate::city::poblacion::origenes::Epoca;
    let pueblo = Accion::FundarPueblo([250.0, 300.0]);
    assert!(permitido(&pueblo, Epoca::Colonial, 1790).is_err());
    assert!(permitido(&pueblo, Epoca::CiudadNueva, 1840).is_ok());
    assert!(permitido(&Accion::Camino("a".into(), "b".into()), Epoca::Colonial, 1790).is_err());
    assert!(permitido(&Accion::Paso(0, 0, Obra::Puente), Epoca::Colonial, 1790).is_err());
    assert!(permitido(&Accion::Paso(0, 0, Obra::Balsa), Epoca::Colonial, 1790).is_ok());
    assert!(permitido(&Accion::Estancia([0.0; 2]), Epoca::Colonial, 1730).is_ok());
    assert!(permitido(&Accion::Saladero([0.0; 2]), Epoca::Colonial, 1770).is_err());
    assert!(permitido(&Accion::Saladero([0.0; 2]), Epoca::Colonial, 1785).is_ok());
    assert!(permitido(&Accion::Puerto("x".into()), Epoca::Colonial, 1790).is_err());
    assert_eq!(nivel_max(Epoca::Colonial), 1);
    assert_eq!(nivel_max(Epoca::Inmigracion), 3);
    // Aplicadas: en la colonia no se funda, en 1840 sí (y se paga).
    let mut p = Pais::simulado_hasta(mes_de(1790));
    let mut plata = 100_000;
    assert!(p.aplicar(&pueblo, &mut plata).is_err());
    assert_eq!(plata, 100_000);
    let mut p2 = Pais::simulado_hasta(mes_de(1840));
    p2.aplicar(&pueblo, &mut plata).unwrap();
    assert_eq!(plata, 100_000 - COSTO_PUEBLO);
    let nuevo = p2.pueblos.last().unwrap().clone();
    assert!(nuevo.jugador);
    // Otro pueblo pegado no; un camino del nuevo a Durazno sí, con su trazado y sus pasos.
    assert!(p2.aplicar(&Accion::FundarPueblo([251.0, 301.0]), &mut plata).is_err());
    let msg = p2.aplicar(&Accion::Camino(nuevo.id.clone(), "durazno".into()), &mut plata).unwrap();
    assert!(msg.contains("km"), "{msg}");
    let c = p2.caminos.last().unwrap();
    assert!(c.jugador && c.puntos.len() >= 2);
    // Una estancia en el agua no; en la pradera de Florida sí.
    let merin = geo().lagunas.iter().find(|l| l.nombre == "Laguna Merín").unwrap().pos;
    assert!(p.aplicar(&Accion::Estancia(merin), &mut plata).is_err());
    let fl = p2.pueblo("florida").unwrap().pos;
    let mut ok = false;
    for d in 0..30 {
        if p2.aplicar(&Accion::Estancia([fl[0] + 8.0 + d as f32, fl[1] - 6.0]), &mut plata).is_ok() {
            ok = true;
            break;
        }
    }
    assert!(ok, "ninguna estancia en la pradera de Florida");
    // Sin plata no hay obra.
    let mut pobre = 10;
    assert!(p2.aplicar(&Accion::FundarPueblo([120.0, 200.0]), &mut pobre).is_err());
    let _ = p.anio();
}

#[test]
fn el_pais_se_guarda_y_es_determinista() {
    let a = Pais::simulado_hasta(mes_de(1810));
    let b = Pais::simulado_hasta(mes_de(1810));
    assert_eq!(a, b, "dos simulaciones iguales dan lo mismo");
    let json = serde_json::to_string(&a).unwrap();
    let mut c: Pais = serde_json::from_str(&json).unwrap();
    assert_eq!(a, c);
    let mut a2 = a.clone();
    let fin = a.mes + 36;
    a2.mes_tick(fin, 9000, true);
    c.mes_tick(fin, 9000, true);
    assert_eq!(a2, c);
    // Una partida vieja (sin país) arranca con el país simulado hasta su fecha.
    let mut g = crate::city::game::Game::default();
    g.city.month = mes_de(1760);
    let mut f = crate::save::snapshot(&g, [0.0; 5], 10.0, false);
    f.pais = None;
    let json = serde_json::to_string(&f).unwrap();
    assert!(!json.contains("\"pais\""));
    let back: crate::save::SaveFile = serde_json::from_str(&json).unwrap();
    let (g2, _) = crate::save::restore(&back, &crate::terrain::TerrainGen::montevideo()).unwrap();
    assert_eq!(g2.pais.mes, mes_de(1760));
    assert!(g2.pais.pueblo("maldonado").is_some());
    assert_eq!(g2.pais, Pais::simulado_hasta(mes_de(1760)));
}

#[test]
fn la_imagen_del_mapa_se_arma() {
    let img = ui::imagen(1800, false, true);
    assert_eq!(img.size, [510 * 3, 557 * 3]);
    // El Río de la Plata es azul y el interior verde.
    let mvd = geo().pueblo_hist("montevideo").unwrap().pos;
    let px = |c: [f32; 2]| img.pixels[(c[1] as usize * 3 + 1) * img.size[0] + c[0] as usize * 3 + 1];
    let rio = px([mvd[0], mvd[1] + 15.0]);
    assert!(rio.b() > rio.r() + 30, "{rio:?}");
    let campo = px([250.0, 300.0]);
    assert!(campo.g() > campo.b(), "{campo:?}");
}
