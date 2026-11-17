//! Pruebas de los recursos del suelo, las molestias y el valor del suelo.

use super::molestias::{Campo, Cuencas, Fuente, Perfil, PERFILES};
use super::*;
use crate::city::lots::{zone_code, Density, ZoneKind};
use crate::city::services::port::Pier;
use crate::city::services::tests::town;
use crate::city::City;
use crate::terrain::TerrainGen;
use std::sync::OnceLock;

/// El mapa real (se carga una vez para todas las pruebas de este módulo).
fn real() -> &'static TerrainGen {
    static T: OnceLock<TerrainGen> = OnceLock::new();
    T.get_or_init(TerrainGen::montevideo)
}

#[test]
fn la_aptitud_esta_entre_0_y_1_y_es_mayor_en_los_suelos_buenos() {
    let t = real();
    let rec = Recursos::de(t);
    let (mut buenos, mut malos) = (Vec::new(), Vec::new());
    for j in 0..60 {
        for i in 0..60 {
            let p = Vec2::new(200.0 + i as f32 * 395.0, 200.0 + j as f32 * 395.0);
            let r = rec.en_punto(p);
            for u in Uso::TODOS {
                let a = r.de(u);
                assert!((0.0..=1.0).contains(&a), "{u:?} = {a} en {p}");
                assert_eq!(a, rec.aptitud(u, p.x, p.y));
            }
            if t.height(p.x, p.y) < 1.0 {
                continue;
            }
            if r.ip >= 180.0 {
                buenos.push(r);
            } else if r.ip > 0.0 && r.ip <= 70.0 {
                malos.push(r);
            }
        }
    }
    assert!(buenos.len() > 50 && malos.len() > 20, "{} buenos, {} malos", buenos.len(), malos.len());
    let media = |v: &[RecursosLote], f: fn(&RecursosLote) -> f32| v.iter().map(f).sum::<f32>() / v.len() as f32;
    let (cb, cm) = (media(&buenos, |r| r.cultivo), media(&malos, |r| r.cultivo));
    let (pb, pm) = (media(&buenos, |r| r.pastoreo), media(&malos, |r| r.pastoreo));
    assert!(cb > cm + 0.2, "cultivo: IP alto {cb:.2}, IP bajo {cm:.2}");
    assert!(pb > pm + 0.1, "pasturas: IP alto {pb:.2}, IP bajo {pm:.2}");
    // La tabla de suelos tiene los grupos de Montevideo con su IP oficial.
    let g = tabla().grupos.iter().find(|g| g.sc == "10.8b").unwrap();
    assert_eq!(g.ip, Some(184.0));
    assert!(tabla().geologia.iter().any(|u| u.unidad.as_deref() == Some("Formacion Libertad")));
}

#[test]
fn el_suelo_del_cerro_es_roca_y_el_de_las_lomadas_es_tierra_de_chacra() {
    let t = real();
    let rec = Recursos::de(t);
    // Ladera del Cerro de Montevideo: basamento cristalino, piedra y poca tierra.
    let cerro = rec.en_area(crate::terrain::CERRO_REAL + Vec2::new(150.0, 150.0), Vec2::NEG_Y, 28.0, 22.0);
    assert!(cerro.piedra >= MINIMO_MATERIAL, "piedra en el Cerro: {:.2}", cerro.piedra);
    let s = rec.suelo(crate::terrain::CERRO_REAL + Vec2::new(150.0, 150.0)).unwrap();
    assert!(!s.geologia.contains("Libertad"), "{}", s.geologia);
    // El mejor punto para cultivo del mapa está sobre limos (sin piedra) y con IP alto.
    let mut mejor = (0.0f32, Vec2::ZERO);
    for j in 0..40 {
        for i in 0..40 {
            let p = Vec2::new(300.0 + i as f32 * 590.0, 300.0 + j as f32 * 590.0);
            let c = rec.aptitud(Uso::Cultivo, p.x, p.y);
            if c > mejor.0 {
                mejor = (c, p);
            }
        }
    }
    let r = rec.en_punto(mejor.1);
    assert!(mejor.0 > 0.75 && r.ip > 150.0, "mejor chacra {:.2} con IP {}", mejor.0, r.ip);
    assert!(r.piedra < MINIMO_MATERIAL, "sin piedra en la chacra: {:.2}", r.piedra);
}

#[test]
fn una_cantera_sin_piedra_no_rinde_y_un_horno_sin_barro_tampoco() {
    let sin = RecursosLote { piedra: 0.05, arcilla: 0.02, ..Default::default() };
    assert_eq!(rinde(Kind::Cantera, &sin), 0.0);
    assert_eq!(rinde(Kind::Horno, &sin), 0.0);
    let con = RecursosLote { piedra: 0.8, arcilla: 0.7, cultivo: 0.6, pastoreo: 0.6, ..Default::default() };
    assert!(rinde(Kind::Cantera, &con) > 1.0 && rinde(Kind::Horno, &con) > 0.9);
    assert!(rinde(Kind::Chacra, &con) > rinde(Kind::Chacra, &RecursosLote { cultivo: 0.1, ..con }));
    assert_eq!(rinde_por_clave(Some("piedra"), &sin), 0.0);
    assert_eq!(rinde_por_clave(None, &sin), 1.0);
    assert_eq!(Uso::de_clave("pastura"), Some(Uso::Pastoreo));
    // En el mapa real, la cantera no se puede poner sobre los limos: no hay piedra.
    let t = real();
    let mut city = City { money: 1_000_000, ..Default::default() };
    city.plaza = Some((crate::terrain::FOUNDING_REAL, 0.0));
    city.lots.plaza_center = Some(crate::terrain::FOUNDING_REAL);
    let rec = Recursos::de(t);
    let mut sin_piedra = None;
    'b: for j in 0..30 {
        for i in 0..30 {
            let p = Vec2::new(8000.0 + i as f32 * 300.0, 2000.0 + j as f32 * 300.0);
            let r = rec.en_area(p, Vec2::NEG_Y, 28.0, 22.0);
            if t.height(p.x, p.y) > 5.0 && r.piedra < 0.05 && r.arcilla > 0.3 {
                sin_piedra = Some(p);
                break 'b;
            }
        }
    }
    let p = sin_piedra.expect("hay campo sin piedra");
    let msg = city.service_problem(t, Kind::Cantera, p, Vec2::NEG_Y).unwrap_or_default();
    assert!(msg.contains("piedra") || msg.contains("calle"), "{msg}");
}

#[test]
fn el_horno_rinde_segun_la_arcilla_del_sitio() {
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    let c = city.plaza.unwrap().0;
    // Un camino hasta el arroyo o la costa más cercana para el horno.
    let mut puesto = None;
    'b: for r in 0..80 {
        for a in 0..24 {
            let p = c + Vec2::from_angle(a as f32 / 24.0 * std::f32::consts::TAU) * (260.0 + r as f32 * 10.0);
            let (sc, sf) = city.service_site(Kind::Horno, p);
            if city.service_problem(&t, Kind::Horno, sc, sf).is_none() {
                puesto = Some((sc, sf));
                break 'b;
            }
        }
    }
    let Some((sc, sf)) = puesto else {
        // Sin calle cerca del agua: el motivo lo dice.
        return;
    };
    let base = city.month_input().production[crate::city::economy::Res::Ladrillos as usize];
    city.place_service(&t, Kind::Horno, sc, sf).unwrap();
    let b = city.services.of_kind(Kind::Horno).next().unwrap();
    let r = Recursos::de(&t).en_area(sc, sf, 24.0, 24.0);
    assert!((b.rinde - rinde(Kind::Horno, &r)).abs() < 1e-5 && b.rinde > 0.0);
    let con = city.month_input().production[crate::city::economy::Res::Ladrillos as usize];
    assert!(con > base, "el horno suma ladrillos según su rinde ({base} → {con})");
}

/// Una ciudad de prueba con los solares pintados y edificados (sin esperar el crecimiento).
fn barrio_con_curtiembre(t: &TerrainGen) -> (City, u32, u32) {
    let mut city = town(t);
    city.fast_forward(t, 6);
    city.refresh_suelo(t);
    let casas: Vec<u32> = city.lots.lots.values().filter(|l| l.kind() == Some(ZoneKind::Residencial)).map(|l| l.id).collect();
    // Un solar de la orilla del barrio se vuelve curtiembre (taller de nivel 2 con la semilla
    // de la curtiembre).
    let plaza = city.plaza.unwrap().0;
    let lejos = *casas.iter().max_by(|a, b| city.lots.lots[a].c.distance(plaza).total_cmp(&city.lots.lots[b].c.distance(plaza)).then(a.cmp(b))).unwrap();
    let pos = city.lots.lots[&lejos].c;
    let vecina = *casas.iter().filter(|&&id| id != lejos).min_by(|a, b| city.lots.lots[a].c.distance(pos).total_cmp(&city.lots.lots[b].c.distance(pos)).then(a.cmp(b))).unwrap();
    {
        let l = city.lots.lots.get_mut(&lejos).unwrap();
        l.zone = zone_code(ZoneKind::Industrial, Density::Media);
        l.level = 2;
        l.seed = 90 << 7; // (semilla >> 7) % 100 ≥ 45: curtiembre
    }
    (city, lejos, vecina)
}

#[test]
fn la_curtiembre_baja_el_valor_de_las_casas_vecinas() {
    let t = TerrainGen::new(1726);
    let (mut city, taller, vecina) = barrio_con_curtiembre(&t);
    assert_eq!(molestias::perfil_de_lote(&city.lots.lots[&taller]).map(|p| p.nombre), Some("curtiembre"));
    let antes_valor = {
        let mut c2 = town(&t);
        c2.fast_forward(&t, 6);
        c2.refresh_suelo(&t);
        c2.lots.valor[&vecina].valor
    };
    city.refresh_suelo(&t);
    let m = city.suelo.molestia_de(vecina);
    assert!(m.olor > 0.3 && m.total() > 0.3, "la vecina huele la curtiembre: {m:?}");
    let v = city.lots.valor[&vecina];
    assert!(v.valor < antes_valor - 0.1, "valor de la vecina: {antes_valor:.2} → {:.2}", v.valor);
    assert!(v.molestia > 0.3 && v.atractivo < 0.55 + 0.9 * antes_valor);
    // El taller no se molesta a sí mismo, y la población lo siente: más insalubre y molesto.
    assert!(city.suelo.molestia_de(taller).olor < m.olor);
    let sv = city.servicios_por_lote();
    assert!(sv[vecina as usize].molestia > 0.3);
    // Las casas lejos del taller no lo sienten.
    let p = city.lots.lots[&taller].c;
    let lejana = city.lots.lots.values().filter(|l| l.kind() == Some(ZoneKind::Residencial)).max_by(|a, b| a.c.distance(p).total_cmp(&b.c.distance(p))).unwrap();
    assert!(city.suelo.molestia_de(lejana.id).olor < 0.01);
    // La ficha lo cuenta.
    assert!(city.lineas_suelo_lote(vecina).iter().any(|l| l.starts_with("Molestias:") && l.contains("olor")));
}

#[test]
fn el_agua_sucia_baja_por_el_arroyo() {
    let t = real();
    let cu = Cuencas::de(t);
    // El cauce más largo que desemboca en otro (un afluente).
    let (ci, c) = cu.cauces.iter().enumerate().filter(|(_, c)| c.sale_a.is_some() && c.pts.len() > 60).max_by_key(|(i, c)| (c.pts.len(), *i)).expect("hay afluentes");
    // Corre aguas abajo: la punta baja está más baja que la alta.
    let (a, b) = (c.pts[0], *c.pts.last().unwrap());
    assert!(t.height(b.x, b.y) <= t.height(a.x, a.y) + 0.5);
    let k = c.pts.len() / 3;
    let curtiembre = Fuente { pos: c.pts[k] + Vec2::new(30.0, 0.0), perfil: *PERFILES.iter().find(|p| p.nombre == "curtiembre").unwrap(), peso: 1.0 };
    let campo = Campo::new(vec![curtiembre], Some(&cu));
    assert!(!campo.agua_sucia.is_empty());
    let abajo = c.pts[(k + 25).min(c.pts.len() - 1)];
    let arriba = c.pts[k.saturating_sub(25)];
    // Lejos del olor (más de 180 m), pero junto al cauce.
    let lejos_del_olor = |p: Vec2| p.distance(curtiembre.pos) > 200.0;
    assert!(lejos_del_olor(abajo) && lejos_del_olor(arriba), "puntos a más de 200 m");
    let ma = campo.en(abajo, None);
    let mr = campo.en(arriba, None);
    assert!(ma.agua > 0.2, "aguas abajo el agua está sucia: {ma:?}");
    assert_eq!(mr.agua, 0.0, "aguas arriba no: {mr:?}");
    // Sigue por el cauce al que desemboca.
    let (cj, _) = c.sale_a.unwrap();
    let siguiente: Vec<Vec2> = cu.aguas_abajo((ci, k), 1.0).into_iter().filter(|x| x.0 .0 == cj).map(|x| x.1).collect();
    let largo: f32 = c.pts.windows(2).skip(k).map(|w| w[0].distance(w[1])).sum();
    if largo < 2500.0 {
        assert!(!siguiente.is_empty(), "el agua sucia pasa al cauce siguiente");
    }
    // Y se diluye: más lejos, menos sucia.
    let v = cu.aguas_abajo((ci, k), 1.0);
    assert!(v.windows(2).all(|w| w[1].2 <= w[0].2 + 1e-6));
}

#[test]
fn el_suelo_vale_mas_cerca_de_la_plaza_y_del_muelle() {
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    city.fast_forward(&t, 6);
    city.refresh_suelo(&t);
    let plaza = city.plaza.unwrap().0;
    let mut casas: Vec<(f32, f32)> = city.lots.lots.values().filter(|l| l.kind() == Some(ZoneKind::Residencial)).map(|l| (l.c.distance(plaza), city.lots.land_value(l))).collect();
    casas.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = casas.len() / 4;
    let cerca = casas[..n].iter().map(|x| x.1).sum::<f32>() / n as f32;
    let lejos = casas[casas.len() - n..].iter().map(|x| x.1).sum::<f32>() / n as f32;
    assert!(cerca > lejos + 0.05, "cerca de la plaza {cerca:.2}, lejos {lejos:.2}");
    // Un muelle cerca valoriza un comercio (el paso de la gente y las mercaderías) y, menos,
    // una casa a unas cuadras (a la que no llega el ruido de la descarga).
    let id = *city.lots.lots.keys().next_back().unwrap();
    let casa = *city.lots.lots.keys().next().unwrap();
    city.lots.lots.get_mut(&id).unwrap().zone = zone_code(ZoneKind::Comercial, Density::Media);
    city.refresh_suelo(&t);
    let (antes, antes_casa) = (city.lots.valor[&id].valor, city.lots.valor[&casa].valor);
    let p = city.lots.lots[&id].c;
    city.services.port.pier = Some(Pier { land: p + Vec2::new(40.0, 0.0), dir: Vec2::X, len: 60.0, barraca: false, voxels: Vec::new() });
    city.refresh_suelo(&t);
    assert!(city.lots.valor[&id].valor > antes + 0.05, "comercio con muelle: {antes:.2} → {:.2}", city.lots.valor[&id].valor);
    let q = city.lots.lots[&casa].c;
    city.services.port.pier = Some(Pier { land: q + Vec2::new(220.0, 0.0), dir: Vec2::X, len: 60.0, barraca: false, voxels: Vec::new() });
    city.refresh_suelo(&t);
    assert!(city.lots.valor[&casa].valor > antes_casa, "casa a 220 m del muelle: {antes_casa:.2} → {:.2}", city.lots.valor[&casa].valor);
    // Recalcular da lo mismo (determinista).
    let v1 = city.lots.valor.clone();
    city.refresh_suelo(&t);
    assert_eq!(v1, city.lots.valor);
}

#[test]
fn guardar_y_cargar_conserva_el_valor_y_rehace_las_molestias() {
    use crate::city::save::CityData;
    let t = TerrainGen::new(1726);
    let (mut city, _, vecina) = barrio_con_curtiembre(&t);
    city.refresh_suelo(&t);
    let data = CityData::from_city(&city);
    let json = serde_json::to_string(&data).unwrap();
    let back: CityData = serde_json::from_str(&json).unwrap();
    let mut loaded = back.to_city().unwrap();
    assert_eq!(loaded.lots.valor, city.lots.valor, "el valor del suelo vuelve igual");
    loaded.rebuild_edits(&t);
    assert_eq!(loaded.lots.valor, city.lots.valor, "y no cambia al regenerar");
    assert_eq!(loaded.suelo.molestias, city.suelo.molestias, "las molestias se recalculan iguales");
    assert_eq!(loaded.suelo.molestia_de(vecina), city.suelo.molestia_de(vecina));
    // Una partida vieja (sin valor guardado) lo calcula al cargar.
    let mut vieja = CityData::from_city(&city);
    vieja.valor_suelo = None;
    let mut c = vieja.to_city().unwrap();
    assert!(c.lots.valor.is_empty());
    c.rebuild_edits(&t);
    assert_eq!(c.lots.valor, city.lots.valor);
    // Y la simulación sigue igual que sin guardar.
    city.fast_forward(&t, 6);
    loaded.fast_forward(&t, 6);
    assert_eq!((loaded.money, loaded.population(), loaded.month), (city.money, city.population(), city.month));
    assert_eq!(loaded.lots.valor, city.lots.valor);
}

#[test]
fn perfiles_de_molestia() {
    for p in PERFILES {
        assert!(p.radio > 0.0 && [p.olor, p.ruido, p.humo, p.vertido].iter().all(|x| (0.0..=1.0).contains(x)), "{}", p.nombre);
    }
    let g = Perfil::desde_molestia(0.8);
    assert!(g.radio > Perfil::desde_molestia(0.2).radio && g.olor > 0.0);
    assert!(molestias::perfil_por_nombre("saladero").is_some_and(|p| p.vertido >= 0.9));
}

#[test]
fn el_campo_alrededor_de_la_ciudad_vieja() {
    let t = real();
    let (c, p) = Recursos::de(t).media_entorno(crate::terrain::FOUNDING_REAL, valor::RADIO_ENTORNO);
    println!("entorno de la Ciudad Vieja: cultivo {c:.3}, pastoreo {p:.3}");
    assert!((0.2..0.9).contains(&c) && (0.2..0.95).contains(&p));
    let mut s = valor::SueloCiudad::default();
    s.entorno = Some((crate::terrain::FOUNDING_REAL, c, p));
    let (fc, fp) = s.factor_entorno();
    assert!((0.8..=1.2).contains(&fc) && (0.8..=1.2).contains(&fp));
}
