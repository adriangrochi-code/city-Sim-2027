//! Pruebas de las épocas: transición, contenido bloqueado antes de tiempo, ensanche,
//! rehabilitación, gas, agua corriente, colectores y guardar/cargar; y la época 3 (paso con el
//! ferrocarril, alumbrado eléctrico, tranvías, conventillos, casas de la época).

use super::super::lots::{zone_code, Density, ZoneKind, D};
use super::super::materiales::{Obra, Pared, Techo};
use super::super::roads::RoadKind;
use super::super::services::Kind;
use super::super::{grid_lines, plaza_frame, City, BLOCK};
use super::*;
use crate::terrain::TerrainGen;
use glam::Vec2;

const C: Vec2 = Vec2::new(13500.0, 16300.0);

/// Ciudad fundada con un damero de 5 × 5 manzanas, zonificada y crecida dos años.
fn ciudad(t: &TerrainGen) -> City {
    let mut city = City::default();
    city.money = 2_000_000;
    city.found_plaza(t, C, 0.0).unwrap();
    let (f, s) = plaza_frame(0.0);
    let a = C - s * (BLOCK * 2.5) - f * (BLOCK * 2.5);
    city.build_grid(t, &grid_lines(a, a + s * BLOCK * 5.0, a + f * BLOCK * 5.0), RoadKind::Stone).unwrap();
    city.paint(C, 400.0, zone_code(ZoneKind::Residencial, Density::Media));
    city.fast_forward(t, 24);
    city
}

/// Un lugar del mapa de prueba donde entra un ensanche de 4 × 4 manzanas (sin arroyos que
/// cruzar), pegado al damero viejo.
fn sitio_ensanche(city: &City, t: &TerrainGen, n: f32) -> (Vec2, Vec2, Vec2) {
    let (f, s) = plaza_frame(0.0);
    for (u, v) in [(s, f), (-s, f), (f, s), (-f, s), (s, -f), (-s, -f), (f, -s), (-f, -s)] {
        for k in [3.0, 4.0, 5.0, 6.0] {
            let a = C + u * (BLOCK * k) - v * (BLOCK * 2.0);
            let (b, cursor) = (a + u * BLOCK * n, a + v * BLOCK * n);
            if let Some(p) = ensanche::plan(a, b, cursor) {
                let e = city.ensanche_problema(t, &p);
                if e.is_none() || e.as_deref().is_some_and(|m| m.contains("Ciudad Nueva")) {
                    if p.lineas.iter().all(|l| city.road_problem(t, l, RoadKind::Stone).is_none()) {
                        return (a, b, cursor);
                    }
                }
            }
        }
    }
    panic!("no hay lugar para el ensanche en el mapa de prueba");
}

/// Un sitio para un servicio grande a más de 300 m de la plaza, sobre una calle del damero.
fn sitio_servicio(city: &City, t: &TerrainGen, k: Kind) -> (Vec2, Vec2) {
    let (f, s) = plaza_frame(0.0);
    for (u, v) in [(s, f), (-s, f), (f, s), (-f, s)] {
        for d in [-1.5f32, -0.5, 0.5, 1.5] {
            let cursor = C + u * (BLOCK * 2.5 + 30.0) + v * (BLOCK * d);
            let (c, fr) = city.service_site(k, cursor);
            if city.service_problem(t, k, c, fr).is_none() {
                return (c, fr);
            }
        }
    }
    panic!("no hay sitio para {k:?}");
}

/// Huella de un edificio: columnas (x, z) de sus vóxeles a más de `margen` m detrás de la
/// línea de fachada.
fn huella(l: &super::super::lots::Lot, margen: f32) -> std::collections::BTreeSet<(i32, i32)> {
    let dv = crate::lod::DESIGN_VOXEL;
    l.voxels
        .iter()
        .filter(|v| {
            let p = Vec2::new((v.x as f32 + 0.5) * dv, (v.z as f32 + 0.5) * dv);
            D * 0.5 - (p - l.c).dot(l.f) > margen
        })
        .map(|v| (v.x, v.z))
        .collect()
}

#[test]
fn transicion_con_requisitos_nuevo_orden_y_resumen() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    assert_eq!(city.epoca.actual, Epoca::Colonial);
    assert!(!city.puede_pasar_epoca());
    assert!(city.pasar_epoca(&t, Muralla::Conservar, false).is_err(), "sin requisitos no se pasa");
    // Modo fiel: el nuevo orden llega a más tardar en 1830.
    assert!(city.poblacion.fiel_historia);
    let mut e = crate::buildings::Edits::default();
    city.epoca_mes(&t, &mut e);
    assert!(city.epoca.nuevo_orden.is_none(), "en 1728 no hay nuevo orden");
    city.month = (NUEVO_ORDEN_HASTA - 1726) * 12;
    let news = city.epoca_mes(&t, &mut e);
    assert!(city.epoca.nuevo_orden.is_some() && news.iter().any(|n| n.contains("Nuevo orden")), "{news:?}");
    // Requisitos: el nuevo orden ya cuenta; los demás siguen faltando.
    let req = city.requisitos_epoca();
    assert_eq!(req.len(), 4);
    assert!(req[3].cumplido && !req[1].cumplido);
    // Forzada (demostración), pasa: población, resumen y crónica.
    let (_, aviso) = city.pasar_epoca(&t, Muralla::Derribar, true).unwrap();
    assert!(aviso.contains("Ciudad Nueva"));
    assert_eq!(city.epoca.actual, Epoca::CiudadNueva);
    assert_eq!(city.poblacion.epoca, Epoca::CiudadNueva);
    assert!(city.epoca.mostrar_resumen);
    let r = &city.epoca.resumenes[0];
    assert_eq!(r.epoca, Epoca::Colonial);
    assert!(r.lineas.iter().any(|l| l.contains("habitantes")), "{:?}", r.lineas);
    // La población sigue en la época de la ciudad aunque el calendario diga otra cosa.
    city.fast_forward(&t, 2);
    assert_eq!(city.poblacion.epoca, Epoca::CiudadNueva);
    assert!(city.pasar_epoca(&t, Muralla::Derribar, true).is_err(), "a la época 3 se pasa con el ferrocarril");
}

#[test]
fn historia_alternativa_no_trae_el_nuevo_orden_sin_ciudad_madura() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.poblacion.fiel_historia = false;
    city.month = (1835 - 1726) * 12;
    let mut e = crate::buildings::Edits::default();
    for _ in 0..24 {
        city.epoca_mes(&t, &mut e);
        city.month += 1;
    }
    assert!(city.epoca.nuevo_orden.is_none(), "sin iglesia terminada ni 1.200 vecinos no llega");
}

#[test]
fn nada_de_la_epoca_2_antes_de_la_transicion() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    for k in [Kind::UsinaGas, Kind::Teatro, Kind::AguaCorriente, Kind::Colector] {
        let (c, f) = city.service_site(k, C + Vec2::new(300.0, 0.0));
        assert!(city.service_problem(&t, k, c, f).is_some_and(|m| m.contains("época")), "{k:?}");
    }
    let (a, b, cursor) = sitio_ensanche(&city, &t, 4.0);
    let p = ensanche::plan(a, b, cursor).unwrap();
    assert!(city.ensanche_problema(&t, &p).is_some_and(|m| m.contains("Ciudad Nueva")));
    let casa = city.lots.lots.values().find(|l| l.level >= 1).unwrap().id;
    assert!(city.reforma_posible(casa, rehabilitar::Reforma::Fachada).is_err());
    assert!(!city.en_epoca(epoca_de_ordenanza(crate::city::economy::Ordinance::Fachadas)));
    // Las casas que se levantan son del kit colonial y las llegadas, de la tabla colonial.
    assert!(city.lots.lots.values().filter_map(|l| l.obra).all(|o| o.contexto.epoca == 0));
    assert_eq!(city.contexto_solar(casa, 2).epoca, 0);
    assert_eq!(city.poblacion.epoca, Epoca::Colonial);
    let mut e = crate::buildings::Edits::default();
    assert!(city.rehabilitar_mes(&t, &mut e).is_empty() && city.epoca.rehabilitadas == 0);
}

#[test]
fn ensanche_con_bulevar_plazas_y_solares() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.pasar_epoca(&t, Muralla::Conservar, true).unwrap();
    let (f, s) = plaza_frame(0.0);
    let (a, b, cursor) = sitio_ensanche(&city, &t, 4.0);
    let p = ensanche::plan(a, b, cursor).unwrap();
    assert_eq!((p.trazado.nu, p.trazado.nv), (4, 4));
    assert!(!p.plazas.is_empty() && !p.arboles.is_empty());
    // Demasiado cerca de la plaza mayor: no.
    let cerca = ensanche::plan(C, C + s * BLOCK * 3.0, C + f * BLOCK * 3.0).unwrap();
    assert!(city.ensanche_problema(&t, &cerca).is_some_and(|m| m.contains("ejido")));
    let antes = city.lots.lots.len();
    let dinero = city.money;
    let e = city.trazar_ensanche(&t, a, b, cursor).expect("el ensanche se traza");
    assert!(!e.is_empty() && city.money < dinero);
    let nuevos: Vec<_> = city.lots.lots.values().filter(|l| city.epoca.ensanche.contiene(l.c)).collect();
    assert!(city.lots.lots.len() > antes + 60 && nuevos.len() > 60, "solares nuevos: {}", nuevos.len());
    // Ningún solar sobre las plazas ni en el cantero del bulevar.
    let tr = city.epoca.ensanche.trazados[0];
    let eje = Vec2::from(tr.a) + Vec2::from(tr.u) * BLOCK * tr.bulevar as f32;
    for l in &nuevos {
        assert!((l.c - eje).dot(Vec2::from(tr.u)).abs() > 6.0, "solar en el cantero del bulevar");
        for (c, _) in &city.epoca.ensanche.plazas {
            assert!(l.c.distance(Vec2::from(*c)) > 30.0, "solar sobre una plaza");
        }
    }
    // El ensanche se empalma solo con la red vieja: una sola red de calles.
    assert!(tr.empalmes >= 1, "sin calles de empalme");
    let comp = super::super::services::components(&city.net);
    let redes: std::collections::BTreeSet<u32> = comp.values().copied().collect();
    assert_eq!(redes.len(), 1, "el ensanche quedó suelto de la ciudad vieja");
    // Las manzanas del ensanche cuentan como ciudad (no extramuros).
    assert_eq!(city.lugar_de(nuevos[0].c), 1);
    // Zonificadas, crecen con casas de la época 2.
    let centro = Vec2::from(tr.a) + Vec2::from(tr.u) * BLOCK * 2.0 + Vec2::from(tr.v) * BLOCK * 2.0;
    city.paint(centro, 250.0, zone_code(ZoneKind::Residencial, Density::Media));
    city.fast_forward(&t, 18);
    let nuevas = city.lots.lots.values().filter(|l| l.level > 0 && city.epoca.ensanche.contiene(l.c)).filter_map(|l| l.obra).filter(|o| o.contexto.epoca == 1).count();
    assert!(nuevas > 5, "casas de la época 2 en el ensanche: {nuevas}");
}

#[test]
fn rehabilitar_cambia_la_fachada_y_conserva_la_huella() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    let id = city.lots.lots.values().find(|l| l.level >= 1 && l.kind() == Some(ZoneKind::Residencial)).unwrap().id;
    {
        // Una casa de azotea de ladrillo visto de 1790, sin revoque ni rejas.
        let l = city.lots.lots.get_mut(&id).unwrap();
        l.level = 2;
        let mut o = Obra::nueva(Pared::Ladrillo, Techo::Azotea, false, false);
        o.contexto.anio = 1790;
        o.contexto.clase = 2;
        o.contexto.lugar = 1;
        l.obra = Some(o);
    }
    let mut e = crate::buildings::Edits::default();
    city.revoxelizar(&t, id, &mut e);
    let l0 = city.lots.lots[&id].clone();
    let es0 = city.estilo_de_solar(id).unwrap();
    assert!(!es0.neo);
    city.pasar_epoca(&t, Muralla::Conservar, true).unwrap();
    city.economy.stocks[crate::city::economy::Res::Ladrillos as usize].amount += 100.0;
    let costo = city.reforma_posible(id, rehabilitar::Reforma::Fachada).unwrap();
    let dinero = city.money;
    city.rehabilitar(&t, id, rehabilitar::Reforma::Fachada, true).unwrap();
    assert_eq!(city.money, dinero - costo);
    let l1 = city.lots.lots[&id].clone();
    let es1 = city.estilo_de_solar(id).unwrap();
    assert!(es1.neo && es1.reforma > 0, "fachada nueva");
    assert_eq!((es0.tipo, es0.ancho, es0.fondo, es0.patios, es0.alas, es0.vanos, es0.puerta), (es1.tipo, es1.ancho, es1.fondo, es1.patios, es1.alas, es1.vanos, es1.puerta), "misma planta");
    assert_ne!(es0.muro, es1.muro, "el ladrillo visto se revoca");
    let (h0, h1) = (huella(&l0, 1.6), huella(&l1, 1.6));
    let dif = h0.symmetric_difference(&h1).count();
    assert!(dif * 50 < h0.len(), "la huella se conserva: {dif} columnas distintas de {}", h0.len());
    assert!(city.reforma_posible(id, rehabilitar::Reforma::Fachada).is_err(), "no dos veces");
    assert!(city.lineas_estilo(&city.lots.lots[&id]).iter().any(|x| x.contains("modernizada")));
    assert_eq!(city.epoca.rehabilitadas, 1);
}

#[test]
fn los_duenos_modernizan_de_a_poco_y_queda_caserio_viejo() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.fast_forward(&t, 12);
    city.pasar_epoca(&t, Muralla::Conservar, true).unwrap();
    city.economy.toggle(crate::city::economy::Ordinance::Fachadas, &mut city.money).unwrap();
    // Sin cal no hay revoque: llega por el puerto.
    city.materiales.ex[crate::city::materiales::Material::Cal.idx()].cantidad += 2000.0;
    city.fast_forward(&t, 24);
    let obras: Vec<_> = city.lots.lots.values().filter(|l| l.level >= 1).filter_map(|l| l.obra).collect();
    let viejas = obras.iter().filter(|o| o.contexto.epoca == 0 && o.contexto.reforma == 0).count();
    let reformadas = obras.iter().filter(|o| o.contexto.reforma > 0).count();
    assert!(reformadas + city.epoca.rehabilitadas as usize > 0, "algunas casas se modernizan");
    assert!(viejas > 0, "otras siguen como eran ({} de {})", viejas, obras.len());
}

#[test]
fn faroles_de_gas_iluminan_y_bajan_el_delito() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.pasar_epoca(&t, Muralla::Conservar, true).unwrap();
    let (c, fr) = sitio_servicio(&city, &t, Kind::UsinaGas);
    let carbon0 = city.costo_carbon();
    let mut sin = city.clone_barrio_delito();
    city.place_service(&t, Kind::UsinaGas, c, fr).expect("usina de gas");
    assert!(city.epoca.faroles_gas.len() > 20, "faroles de gas: {}", city.epoca.faroles_gas.len());
    assert!(city.costo_carbon() > carbon0);
    // Luces para el render: las de gas, más fuertes que las de sebo.
    let luces = city.luces_cerca(C, 300.0, 256);
    assert!(luces.iter().any(|l| l[4] >= 1.0));
    // El delito baja donde hay gas.
    let con = city.clone_barrio_delito();
    let mut bajan = 0;
    let mut suben = 0;
    for (id, d) in &con {
        let antes = sin.remove(id).unwrap_or(*d);
        if *d < antes - 1e-4 {
            bajan += 1;
        } else if *d > antes + 1e-4 {
            suben += 1;
        }
    }
    assert!(bajan > 10 && suben == 0, "bajan {bajan}, suben {suben}");
}

#[test]
fn agua_corriente_y_colectores_cubren_su_red() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.pasar_epoca(&t, Muralla::Conservar, true).unwrap();
    city.refresh_barrio();
    let insalubre0 = super::super::services::barrio::Barrio::media(&city.services.barrio.insalubre);
    let (_, s) = plaza_frame(0.0);
    let (c, f) = sitio_servicio(&city, &t, Kind::AguaCorriente);
    city.place_service(&t, Kind::AguaCorriente, c, f).expect("agua corriente");
    let casas = city.lots.lots.values().filter(|l| l.level > 0).count();
    assert!(city.epoca.agua_red.len() * 10 > casas * 9, "agua corriente: {} de {casas}", city.epoca.agua_red.len());
    // El colector va junto a la costa; en el mapa de prueba se lo pone a mano.
    let id = 9999;
    let boca = C + s * 60.0;
    city.services.buildings.insert(id, super::super::services::Building { id, kind: Kind::Colector, c: boca, f, served: 0, contaminacion: 0.0, rinde: 1.0, voxels: Vec::new() });
    // Un camino suelto (como el del muelle) pegado a la boca: el colector igual se empalma con
    // las calles de la ciudad que pasan cerca.
    city.net.add_road(&super::super::roads::line(boca + Vec2::new(-20.0, 4.0), boca + Vec2::new(20.0, 4.0)), RoadKind::Dirt);
    city.refresh_redes();
    assert!(city.epoca.cloaca.len() * 10 > casas * 9, "colector: {} de {casas}", city.epoca.cloaca.len());
    city.refresh_barrio();
    let insalubre1 = super::super::services::barrio::Barrio::media(&city.services.barrio.insalubre);
    assert!(insalubre1 < insalubre0 * 0.6, "insalubridad {insalubre0} → {insalubre1}");
}

#[test]
fn guardar_y_cargar_la_ciudad_nueva() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad(&t);
    city.pasar_epoca(&t, Muralla::Monumento, true).unwrap();
    let (a, b, cursor) = sitio_ensanche(&city, &t, 3.0);
    city.trazar_ensanche(&t, a, b, cursor).unwrap();
    let (c, fr) = sitio_servicio(&city, &t, Kind::UsinaGas);
    city.place_service(&t, Kind::UsinaGas, c, fr).unwrap();
    city.fast_forward(&t, 6);
    let data = crate::city::save::CityData::from_city(&city);
    let json = serde_json::to_string(&data).unwrap();
    let back: crate::city::save::CityData = serde_json::from_str(&json).unwrap();
    let mut c2 = back.to_city().unwrap();
    c2.rebuild_edits(&t);
    assert_eq!(c2.epoca.actual, Epoca::CiudadNueva);
    assert_eq!(c2.epoca.ensanche, city.epoca.ensanche);
    assert_eq!(c2.epoca.faroles_gas, city.epoca.faroles_gas);
    assert_eq!(c2.poblacion.epoca, Epoca::CiudadNueva);
    for (id, l) in &city.lots.lots {
        assert_eq!(l.obra, c2.lots.lots[id].obra);
    }
    // Una partida vieja (sin época) es colonial.
    let mut v: serde_json::Value = serde_json::to_value(&data).unwrap();
    v.as_object_mut().unwrap().remove("epoca");
    let vieja: crate::city::save::CityData = serde_json::from_value(v).unwrap();
    let c3 = vieja.to_city().unwrap();
    assert_eq!(c3.epoca.actual, Epoca::Colonial);
    assert_eq!(c3.poblacion.epoca, Epoca::Colonial);
}

impl City {
    /// Delito de cada solar recalculado ahora (para comparar).
    fn clone_barrio_delito(&mut self) -> std::collections::BTreeMap<u32, f32> {
        self.refresh_barrio();
        self.services.barrio.delito.clone()
    }
}

// --- [epoca3] Época 3, Inmigración ----------------------------------------------------------

/// Ciudad de prueba ya en la Ciudad Nueva.
fn ciudad_nueva(t: &TerrainGen) -> City {
    let mut city = ciudad(t);
    city.pasar_epoca(t, Muralla::Conservar, true).unwrap();
    city
}

#[test]
fn transicion_a_la_epoca3_con_el_ferrocarril() {
    use super::epoca3::*;
    let t = TerrainGen::new(1726);
    let mut city = ciudad_nueva(&t);
    assert_eq!(city.epoca_siguiente(), Some(Epoca::Inmigracion));
    let req = city.requisitos_epoca();
    assert_eq!(req.len(), 4);
    assert!(!city.puede_pasar_epoca());
    assert!(city.pasar_epoca3(&t, Ferrocarril::Propio, false).is_err(), "sin requisitos no se pasa");
    // Modo fiel: el ferrocarril llega a más tardar en 1869.
    let mut e = crate::buildings::Edits::default();
    city.month = (1850 - 1726) * 12;
    city.epoca_mes(&t, &mut e);
    assert!(city.epoca.ferrocarril.is_none(), "en 1850 no hay ferrocarril");
    city.month = (FERROCARRIL_HASTA - 1726) * 12;
    let news = city.epoca_mes(&t, &mut e);
    assert!(city.epoca.ferrocarril.is_some() && news.iter().any(|n| n.contains("ferrocarril")), "{news:?}");
    assert!(city.requisitos_epoca()[3].cumplido);
    // Con la muralla no se pasa a la época 3: se decide el ferrocarril.
    assert!(city.pasar_epoca(&t, Muralla::Derribar, true).is_err());
    let dinero = city.money;
    let (_, aviso) = city.pasar_epoca3(&t, Ferrocarril::Concesion, true).unwrap();
    assert!(aviso.contains("Inmigración"), "{aviso}");
    assert_eq!(city.epoca.actual, Epoca::Inmigracion);
    assert_eq!(city.poblacion.epoca, Epoca::Inmigracion);
    assert_eq!(city.money, dinero + CANON_CONCESION, "la compañía paga el canon");
    assert_eq!(city.epoca.concesion, Some(Ferrocarril::Concesion));
    assert_eq!(city.costo_servicio(Kind::Estacion), 0, "con la concesión la estación la paga la compañía");
    assert_eq!(city.epoca.resumenes.len(), 2);
    assert_eq!(city.epoca.resumenes[1].epoca, Epoca::CiudadNueva);
    assert!(city.epoca.resumenes[1].lineas.iter().any(|l| l.contains("ferrocarril")));
    // La población sigue en la época de la ciudad y no hay otra época jugable.
    city.fast_forward(&t, 2);
    assert_eq!(city.poblacion.epoca, Epoca::Inmigracion);
    assert!(city.epoca_siguiente().is_none());
    assert!(city.pasar_epoca3(&t, Ferrocarril::Propio, true).is_err());
}

#[test]
fn nada_de_la_epoca3_antes_de_la_transicion() {
    let t = TerrainGen::new(1726);
    let mut city = ciudad_nueva(&t);
    for k in [Kind::Estacion, Kind::UsinaElectrica, Kind::CocherasTranvia, Kind::HotelInmigrantes, Kind::PuertoModerno] {
        let (c, f) = city.service_site(k, C + Vec2::new(300.0, 0.0));
        assert!(city.service_problem(&t, k, c, f).is_some_and(|m| m.contains("época")), "{k:?}");
    }
    let casa = city.lots.lots.values().find(|l| l.level >= 1).unwrap().id;
    assert_eq!(city.contexto_solar(casa, 2).epoca, 1, "en la Ciudad Nueva, el kit de la época 2");
    city.pool = 500;
    let mut e = crate::buildings::Edits::default();
    city.epoca_mes(&t, &mut e);
    assert_eq!(city.conventillos(), 0, "sin época 3 no hay conventillos");
    assert!(city.epoca.faroles_electricos.is_empty() && city.epoca.tranvias.is_empty());
}

#[test]
fn alumbrado_electrico_reemplaza_al_gas() {
    use super::epoca3::Ferrocarril;
    let t = TerrainGen::new(1726);
    let mut city = ciudad_nueva(&t);
    let (c, fr) = sitio_servicio(&city, &t, Kind::UsinaGas);
    city.place_service(&t, Kind::UsinaGas, c, fr).unwrap();
    let gas0 = city.epoca.faroles_gas.len();
    assert!(gas0 > 20);
    city.pasar_epoca3(&t, Ferrocarril::Propio, true).unwrap();
    city.money += 100_000;
    let (c, fr) = sitio_servicio(&city, &t, Kind::UsinaElectrica);
    city.place_service(&t, Kind::UsinaElectrica, c, fr).expect("usina eléctrica");
    let n = city.epoca.faroles_electricos.len();
    assert!(n > 15, "faroles eléctricos: {n}");
    assert!(city.epoca.faroles_gas.len() < gas0 / 2, "el gas queda donde no llega la luz eléctrica: {} de {gas0}", city.epoca.faroles_gas.len());
    // Luces más fuertes que las de gas, por la misma vía del render.
    let luces = city.luces_cerca(C, 400.0, 256);
    assert!(luces.iter().any(|l| l[4] > 1.5 && l[3] > 12.0));
    // El mes siguiente no repone el gas que se sacó.
    let gas1 = city.epoca.faroles_gas.len();
    city.fast_forward(&t, 2);
    assert!(city.epoca.faroles_gas.len() <= gas1);
}

#[test]
fn tranvias_a_caballo_y_electricos_suben_el_valor() {
    use super::epoca3::Ferrocarril;
    let t = TerrainGen::new(1726);
    let mut city = ciudad_nueva(&t);
    city.pasar_epoca3(&t, Ferrocarril::Propio, true).unwrap();
    city.money += 200_000;
    let (c, fr) = sitio_servicio(&city, &t, Kind::CocherasTranvia);
    city.place_service(&t, Kind::CocherasTranvia, c, fr).expect("cocheras");
    assert!(!city.epoca.tranvias.is_empty(), "sin líneas");
    let l = city.epoca.tranvias[0].clone();
    assert!(l.largo() > 100.0 && !l.electrica, "línea a caballo de {} m", l.largo());
    // Una línea llega a la plaza mayor.
    let plaza = city.lots.plaza_center.unwrap();
    assert!(city.distancia_tranvia(plaza).unwrap() < 60.0);
    assert!(city.solares_con_tranvia() > 10);
    // Sube el valor del suelo junto a la línea, no lejos.
    let p = Vec2::from(l.pts[l.pts.len() / 2]);
    assert!(city.valor_epoca3(p) > 0.03);
    assert!(city.valor_epoca3(p + Vec2::new(5000.0, 0.0)) < 1e-3);
    // Con la usina, eléctricas.
    let (c, fr) = sitio_servicio(&city, &t, Kind::UsinaElectrica);
    city.place_service(&t, Kind::UsinaElectrica, c, fr).unwrap();
    assert!(city.epoca.tranvias.iter().all(|l| l.electrica));
    // Los rieles están en los vóxeles al regenerar la ciudad.
    let e = city.rebuild_edits(&t);
    assert!(e.voxels.values().any(|&m| m == crate::palette::HIERRO_FUNDIDO), "rieles");
}

#[test]
fn conventillos_cuando_falta_vivienda() {
    use super::epoca3::*;
    let t = TerrainGen::new(1726);
    let mut city = ciudad_nueva(&t);
    city.pasar_epoca3(&t, Ferrocarril::Propio, true).unwrap();
    // Casas viejas de familias modestas.
    let ids: Vec<u32> = city.lots.lots.values().filter(|l| l.level >= 1 && l.kind() == Some(ZoneKind::Residencial)).map(|l| l.id).take(12).collect();
    assert!(ids.len() >= 4);
    for &id in &ids {
        let l = city.lots.lots.get_mut(&id).unwrap();
        let mut o = l.obra.unwrap_or_else(|| Obra::nueva(Pared::Ladrillo, Techo::Azotea, true, true));
        o.contexto.clase = 1;
        o.contexto.epoca = 0;
        o.contexto.anio = 1790;
        l.obra = Some(o);
        l.level = 2;
        l.zone = zone_code(ZoneKind::Residencial, Density::Media);
    }
    let mut e = crate::buildings::Edits::default();
    city.poblacion.fundada = false;
    city.pool = 10;
    assert!(city.conventillos_mes(&t, &mut e).is_empty() && city.conventillos() == 0, "con vivienda no hay conventillos");
    city.pool = 300;
    let news = city.conventillos_mes(&t, &mut e);
    assert!(city.conventillos() >= 1 && !news.is_empty(), "{news:?}");
    let id = city.lots.lots.values().find(|l| l.obra.is_some_and(|o| o.contexto.conventillo > 0)).unwrap().id;
    let l = &city.lots.lots[&id];
    assert_eq!(crate::city::lots::zone_of(l.zone).map(|z| z.1), Some(Density::Alta), "más familias");
    assert!(city.lineas_estilo(l)[0].contains("onventillo"), "{:?}", city.lineas_estilo(l));
    // Con la inspección, menos (y cuesta).
    city.epoca.inspeccion = true;
    let n0 = city.conventillos();
    for _ in 0..4 {
        city.conventillos_mes(&t, &mut e);
        city.month += 1;
    }
    assert!(city.conventillos() <= n0 + 4);
    let dinero = city.money;
    city.epoca3_mes(&t, &mut e);
    assert!(city.money < dinero);
}

#[test]
fn casas_de_la_epoca3_y_oleadas() {
    use super::epoca3::Ferrocarril;
    let t = TerrainGen::new(1726);
    let mut city = ciudad_nueva(&t);
    city.pasar_epoca3(&t, Ferrocarril::Propio, true).unwrap();
    let casa = city.lots.lots.values().find(|l| l.level >= 1).unwrap().id;
    assert_eq!(city.contexto_solar(casa, 2).epoca, 2, "lo nuevo, con el kit de la época 3");
    // Zonas nuevas: crecen con casas de la época 3.
    city.paint(C, 450.0, zone_code(ZoneKind::Residencial, Density::Media));
    city.materiales.ex[crate::city::materiales::Material::Cal.idx()].cantidad += 3000.0;
    city.money += 200_000;
    city.fast_forward(&t, 24);
    let nuevas = city.lots.lots.values().filter_map(|l| l.obra).filter(|o| o.contexto.epoca == 2).count();
    assert!(nuevas > 0, "casas de la época 3: {nuevas}");
    // Una oleada multiplica las llegadas unos meses.
    city.epoca.oleada_hasta = Some(city.month + 3);
    assert!(city.factor_inmigracion() > 1.5);
    city.month += 4;
    assert_eq!(city.factor_inmigracion(), 1.0);
}

#[test]
fn guardar_y_cargar_la_epoca3() {
    use super::epoca3::Ferrocarril;
    let t = TerrainGen::new(1726);
    let mut city = ciudad_nueva(&t);
    city.pasar_epoca3(&t, Ferrocarril::Propio, true).unwrap();
    city.money += 300_000;
    for k in [Kind::UsinaElectrica, Kind::CocherasTranvia] {
        let (c, fr) = sitio_servicio(&city, &t, k);
        city.place_service(&t, k, c, fr).unwrap();
    }
    let id = city.lots.lots.values().find(|l| l.level >= 1 && l.kind() == Some(ZoneKind::Residencial) && l.obra.is_some()).unwrap().id;
    city.lots.lots.get_mut(&id).unwrap().level = 2;
    let mut e = crate::buildings::Edits::default();
    city.convertir_conventillo(&t, id, &mut e);
    city.fast_forward(&t, 3);
    let data = crate::city::save::CityData::from_city(&city);
    let json = serde_json::to_string(&data).unwrap();
    let back: crate::city::save::CityData = serde_json::from_str(&json).unwrap();
    let mut c2 = back.to_city().unwrap();
    c2.rebuild_edits(&t);
    assert_eq!(c2.epoca, city.epoca);
    assert_eq!(c2.poblacion.epoca, Epoca::Inmigracion);
    for (id, l) in &city.lots.lots {
        assert_eq!(l.obra, c2.lots.lots[id].obra);
    }
    // Una partida de la v0.12 (sin lo de la época 3) carga como Ciudad Nueva.
    let mut v: serde_json::Value = serde_json::to_value(&data).unwrap();
    let ep = v.get_mut("epoca").unwrap().as_object_mut().unwrap();
    for k in ["ferrocarril", "concesion", "faroles_electricos", "electrica_firma", "tranvias", "tranvia_firma", "conventillos", "inspeccion", "oleada_hasta", "avisada3"] {
        ep.remove(k);
    }
    ep.insert("actual".into(), serde_json::json!("CiudadNueva"));
    let vieja: crate::city::save::CityData = serde_json::from_value(v).unwrap();
    let c3 = vieja.to_city().unwrap();
    assert_eq!(c3.epoca.actual, Epoca::CiudadNueva);
    assert!(c3.epoca.tranvias.is_empty() && c3.epoca.concesion.is_none());
}
