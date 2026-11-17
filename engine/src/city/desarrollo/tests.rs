//! Pruebas del desarrollo: tecnologías que piden saberes e inversión, industrias de capital
//! que producen más por obrero y exportan, incentivos que atraen inversión y cuestan
//! impuestos, choques de precios y diversificación, escuelas que forman oficiales en la
//! población, y guardar y cargar.

use super::*;
use crate::city::save::CityData;
use crate::city::services::tests::{free_site, town};
use crate::terrain::TerrainGen;
use glam::Vec2;

/// Un contexto de ciudad colonial razonable (precios normales, muelle, gente y ganado).
fn ctx() -> Contexto {
    Contexto {
        mes: 100,
        saberes: [400, 60, 30, 0, 0, 0],
        edificios: vec![(Kind::Escuela, 2), (Kind::Estancia, 2), (Kind::Chacra, 1), (Kind::Horno, 1)],
        muelle: true,
        tasa_aduana: 0.1,
        carne_disponible: 2000.0,
        precios: [1.0; 6],
        world_hides: 1.0,
        dinero: 100_000,
        ladrillos: 5000.0,
        jobs_i: 100,
        habitantes: 3000,
        delito: 0.1,
        opinion: [55.0; 3],
        desocupados: 80,
        ..Default::default()
    }
}

#[test]
fn una_tecnologia_pide_saberes_e_inversion() {
    let mut d = Desarrollo::default();
    let mut c = ctx();
    c.saberes[Saber::Alfabetizados.i()] = 10;
    c.saberes[Saber::Oficiales.i()] = 0;
    let err = d.iniciar(Tec::EscuelaOficios, &c).unwrap_err();
    assert!(err.contains("Saben leer"), "{err}");
    assert!(d.iniciar(Tec::Saladero, &c).is_err(), "el saladero pide oficiales que todavía no hay");
    c.saberes[Saber::Alfabetizados.i()] = 120;
    d.iniciar(Tec::EscuelaOficios, &c).unwrap();
    assert!(!d.habilitado(Kind::EscuelaOficios));
    // Sin plata para la cuota, la inversión se para.
    c.dinero = 10;
    let r = d.mes(&c);
    assert!(d.en_obra(Tec::EscuelaOficios).unwrap().parado);
    assert!(r.noticias.iter().any(|n| n.contains("Se para")));
    // Con plata avanza mes a mes y se paga entera.
    c.dinero = 100_000;
    let mut pagado = 0;
    for _ in 0..tec_info(Tec::EscuelaOficios).meses {
        let r = d.mes(&c);
        pagado -= r.lineas.iter().filter(|l| l.0 == "Inversión en tecnologías").map(|l| l.1).sum::<i64>();
    }
    assert!(d.hecha(Tec::EscuelaOficios) && d.habilitado(Kind::EscuelaOficios));
    assert!(pagado >= tec_info(Tec::EscuelaOficios).costo, "se pagó {pagado}");
    // Las de otras épocas no se pueden empezar.
    assert!(matches!(d.estado(Tec::Frigorifico, &c), EstadoTec::OtraEpoca(_)));
}

/// Un saladero con todos sus peones y oficiales.
fn con_saladero(d: &mut Desarrollo) {
    d.hechas.push(Tec::Saladero);
    d.registrar(Ind::Saladero, 1, Dueno::Hogar(7), 90, false);
    d.establecimientos[0].empleados = (0..20).collect();
}

#[test]
fn la_industria_de_capital_produce_mas_por_obrero_y_exporta() {
    // Perfiles: el saladero produce mucho más por obrero que los talleres.
    let por_obrero = |i: Ind| {
        let inf = ind_info(i);
        let valor: f32 = inf.produce.iter().map(|(b, q)| q * match b { Bien::Res(r) => r.base_price(), Bien::Prod(p) => prod_info(*p).precio }).sum();
        valor / (inf.empleos + inf.saber.map_or(0, |s| s.1)) as f32
    };
    assert!(por_obrero(Ind::Saladero) > 3.0 * por_obrero(Ind::Talleres), "{} vs {}", por_obrero(Ind::Saladero), por_obrero(Ind::Talleres));
    assert!(ind_info(Ind::Saladero).capital > 10 * ind_info(Ind::Talleres).capital);
    // En marcha: exporta tasajo, compra ganado, deja cueros y sebo a la ciudad y gana plata.
    let mut d = Desarrollo::default();
    con_saladero(&mut d);
    let c = ctx();
    let mut sin = Desarrollo::default();
    sin.mes(&c);
    let r = d.mes(&c);
    let f = d.historia.back().unwrap();
    assert!(f.exportado[Prod::Tasajo.i()] > 200.0, "tasajo {:?}", f.exportado);
    assert!(f.exportaciones() > sin.historia.back().unwrap().exportaciones() + 200.0);
    assert!(r.carne_tomada > 300.0 && r.produccion[Res::Cueros as usize] > 40.0);
    assert!(r.ganancias.iter().any(|&(h, g)| h == 7 && g > 0.0), "{:?}", r.ganancias);
    assert!(r.lineas.iter().any(|l| l.0.starts_with("Aduana") && l.1 > 0));
    // Sin peones ni oficiales, apenas marcha.
    let mut vacio = Desarrollo::default();
    con_saladero(&mut vacio);
    vacio.establecimientos[0].empleados.clear();
    vacio.mes(&c);
    assert!(vacio.establecimientos[0].marcha < 0.05 && !vacio.establecimientos[0].falta.is_empty());
}

#[test]
fn los_incentivos_atraen_inversion_y_cuestan_impuestos() {
    let mut c = ctx();
    c.inversores = vec![(3, 4000.0)]; // nadie tiene los 6.000 pesos del saladero
    let correr = |inc: &[Incentivo]| {
        let mut d = Desarrollo::default();
        d.hechas.push(Tec::Saladero);
        for &i in inc {
            d.alternar(i, false).unwrap();
        }
        let mut inversiones = 0;
        for m in 0..24 {
            let mut c = c.clone();
            c.mes = 100 + m;
            if let Some((ind, dueno)) = d.mes(&c).inversion {
                inversiones += 1;
                d.registrar(ind, 100 + m as u32, dueno, c.mes, d.incentivo(Incentivo::Credito));
                d.establecimientos.last_mut().unwrap().empleados = (0..20).collect();
            }
        }
        (inversiones, d)
    };
    let (sin, d0) = correr(&[]);
    assert_eq!(sin, 0, "sin incentivos nadie invierte: {:?}", d0.evaluaciones);
    let (con, d1) = correr(&[Incentivo::Exencion, Incentivo::Credito]);
    assert!(con > 0, "con exención y crédito alguien invierte: {:?}", d1.evaluaciones);
    let ev = |d: &Desarrollo| d.evaluaciones.iter().find(|e| e.ind == Ind::Saladero).unwrap().rinde;
    assert!(ev(&d1) > ev(&d0), "la exención sube el rinde esperado");
    assert!(d1.establecimientos.iter().any(|e| e.deuda > 0.0), "el crédito de fomento presta");
    // La exención cuesta: el mismo saladero paga menos aduana y aparece la renuncia fiscal.
    let aduana = |inc: bool| {
        let mut d = Desarrollo::default();
        con_saladero(&mut d);
        if inc {
            d.alternar(Incentivo::Exencion, false).unwrap();
        }
        let r = d.mes(&ctx());
        (r.lineas.iter().filter(|l| l.0.starts_with("Aduana")).map(|l| l.1).sum::<i64>(), d.renuncia_total)
    };
    let (a_sin, r_sin) = aduana(false);
    let (a_con, r_con) = aduana(true);
    assert!(a_con < a_sin && r_con > 0.0 && r_sin == 0.0, "aduana {a_sin} → {a_con}, renuncia {r_con}");
    // Aranceles y libre comercio no van juntos.
    let mut d = Desarrollo::default();
    assert!(d.alternar(Incentivo::Arancel, true).is_err());
    assert!(d.alternar(Incentivo::Arancel, false).is_ok());
}

#[test]
fn un_choque_de_precios_golpea_mas_a_la_ciudad_sin_diversificar() {
    let mut sola = [0.0; NPROD];
    sola[Prod::Cueros.i()] = 1000.0;
    let mut variada = [0.0; NPROD];
    for (p, v) in [(Prod::Cueros, 350.0), (Prod::Sebo, 100.0), (Prod::Tasajo, 300.0), (Prod::Suela, 150.0), (Prod::Carenas, 100.0)] {
        variada[p.i()] = v;
    }
    assert!(hhi(&sola) > 0.99 && hhi(&variada) < 0.35);
    let (a, b) = (conserva_tras_choque(&sola, Prod::Cueros, 0.55), conserva_tras_choque(&variada, Prod::Cueros, 0.55));
    assert!(a < b, "la ciudad de un solo producto conserva {a}, la diversificada {b}");
    // En la simulación: la caída de las exportaciones enoja a hacendados y comerciantes, y más
    // a la ciudad que solo vende cueros.
    let golpe = |valores: [f32; 6]| {
        let mut d = Desarrollo::default();
        let mut c = ctx();
        c.exportado = valores;
        for _ in 0..12 {
            d.mes(&c);
        }
        c.world_hides = 0.5;
        d.precios[Prod::Cueros.i()] = 0.55;
        d.mes(&c).opinion[2]
    };
    let mut solo_cueros = [0.0; 6];
    solo_cueros[Res::Cueros as usize] = 500.0;
    let mut varios = solo_cueros;
    varios[Res::Cueros as usize] = 150.0;
    varios[Res::Sebo as usize] = 600.0;
    varios[Res::Carne as usize] = 2000.0;
    assert!(golpe(solo_cueros) < golpe(varios), "el choque enoja más a los hacendados de la ciudad de un solo producto");
}

#[test]
fn la_escuela_de_oficios_forma_oficiales_en_la_poblacion() {
    use crate::city::poblacion::{Oficio, TipoEvento};
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    city.fast_forward(&t, 12);
    // Sin la tecnología no se puede construir.
    let c = city.plaza.unwrap().0;
    assert!(city.desarrollo_problema_sitio(&t, Kind::EscuelaOficios, c).is_some_and(|m| m.contains("tecnología")));
    city.desarrollo.hechas.push(Tec::EscuelaOficios);
    let (sc, sf) = free_site(&city, &t, Kind::EscuelaOficios, c + Vec2::new(-60.0, 60.0));
    city.place_service(&t, Kind::EscuelaOficios, sc, sf).unwrap();
    // Jóvenes que saben leer.
    let mes = city.poblacion.mes;
    let vivos = city.poblacion.idx.vivos.clone();
    for id in vivos {
        let p = &mut city.poblacion.personas[id as usize];
        if (15..=25).contains(&p.edad(mes)) {
            p.educacion = p.educacion.max(1);
        }
    }
    city.fast_forward(&t, 8);
    let ofi = city.desarrollo.saber(Saber::Oficiales);
    assert!(ofi >= 8, "oficiales formados: {ofi}");
    for &(id, s) in &city.desarrollo.formados {
        let p = &city.poblacion.personas[id as usize];
        assert_eq!(s, Saber::Oficiales);
        assert_eq!(p.oficio, Oficio::Artesano);
        assert!(p.historial.iter().any(|e| e.tipo == TipoEvento::Aprendio));
    }
    assert!(city.desarrollo.saber(Saber::Alfabetizados) > 0);
}

/// Sin escuela de oficios, el aprendizaje en los talleres no pasa de unos pocos oficiales:
/// para industrializar hay que invertir en la escuela.
#[test]
fn sin_escuela_de_oficios_el_taller_forma_pocos_oficiales() {
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    city.fast_forward(&t, 12);
    // Muchos meses de talleres con aprendizaje acelerado (como si pasaran décadas).
    for _ in 0..40 {
        city.desarrollo.acumulado[Saber::Artesanos.i()] += 5.0;
        city.fast_forward(&t, 1);
    }
    let ofi = city.desarrollo.saber(Saber::Oficiales);
    assert!(ofi <= super::ciudad::TOPE_APRENDIZAJE as u32, "sin escuela: {ofi} oficiales");
}

#[test]
fn guardar_y_cargar_el_desarrollo() {
    use crate::city::roads::{line, RoadKind};
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    city.fast_forward(&t, 6);
    let c = city.plaza.unwrap().0;
    // Un camino hacia afuera y un molino del cabildo; incentivos y una inversión en marcha.
    let end = c + Vec2::new(0.0, -700.0);
    city.build_road(&t, &line(c + Vec2::new(crate::city::BLOCK * 1.5, -crate::city::BLOCK * 1.5), end), RoadKind::Dirt).ok();
    city.desarrollo.hechas.extend([Tec::Molino, Tec::EscuelaOficios, Tec::CurtidoMejorado, Tec::HornoMejorado]);
    let (mc, mf) = free_site(&city, &t, Kind::Molino, end);
    city.place_service(&t, Kind::Molino, mc, mf).unwrap();
    for i in [Incentivo::Exencion, Incentivo::Maestros, Incentivo::Arancel] {
        city.desarrollo_alternar(i).unwrap();
    }
    city.desarrollo.saberes[Saber::Alfabetizados.i()] = 500;
    city.desarrollo.saberes[Saber::Oficiales.i()] = 20;
    city.desarrollo.proyectos.push(Proyecto { tec: Tec::Imprenta, meses_hechos: 2, parado: false, invertido: 600 });
    city.fast_forward(&t, 10);
    assert_eq!(city.desarrollo.establecimientos.len(), 1, "el molino del cabildo");
    assert!(!city.desarrollo.establecimientos[0].empleados.is_empty(), "contrató peones");
    assert!(city.desarrollo.historia.len() >= 10);
    let data = CityData::from_city(&city);
    let json = serde_json::to_string(&data).unwrap();
    let back: CityData = serde_json::from_str(&json).unwrap();
    assert_eq!(back, data);
    let mut loaded = back.to_city().unwrap();
    loaded.rebuild_edits(&t);
    assert_eq!(loaded.desarrollo, city.desarrollo);
    city.fast_forward(&t, 12);
    loaded.fast_forward(&t, 12);
    assert_eq!(loaded.desarrollo, city.desarrollo, "el desarrollo sigue igual un año después");
    assert_eq!((loaded.money, loaded.population()), (city.money, city.population()));
    assert!(loaded.poblacion == city.poblacion);
    // Una partida de antes del desarrollo se carga sin él.
    let mut old = CityData::from_city(&city);
    old.desarrollo = None;
    let json = serde_json::to_string(&old).unwrap().replace(",\"desarrollo\":null", "");
    let back: CityData = serde_json::from_str(&json).unwrap();
    assert_eq!(back.to_city().unwrap().desarrollo, Desarrollo::default());
}
