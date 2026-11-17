use super::ciudad::*;
use super::*;
use crate::buildings::Edits;
use crate::city::economy::Res;
use crate::city::lots::{zone_code, Density, ZoneKind};
use crate::city::save::CityData;
use crate::city::services::tests::town;
use crate::city::City;
use crate::palette::*;
use crate::terrain::TerrainGen;
use glam::IVec3;

/// Depósito de prueba: todo lo que se pida, en las cantidades dadas.
fn deposito(pares: &[(Material, f32)]) -> [f32; N] {
    let mut d = [0.0; N];
    for &(m, q) in pares {
        d[m.idx()] = q;
    }
    d
}

fn casa(level: u8) -> Plan {
    plan_solar(ZoneKind::Residencial, Density::Media, level, 1 << 7, false)
}

#[test]
fn elige_lo_mejor_que_hay_y_sustituye_en_orden() {
    let plan = casa(2);
    let todo = deposito(&[(Material::Ladrillo, 500.0), (Material::Cal, 50.0), (Material::Arena, 50.0), (Material::Teja, 100.0), (Material::Madera, 100.0), (Material::Hierro, 10.0), (Material::Adobe, 500.0), (Material::Paja, 100.0)]);
    let o = elegir(&plan, &todo);
    assert_eq!(o.paredes, Pared::Ladrillo);
    assert!(o.revoque && o.rejas && o.faltas == 0, "{o:?}");
    // Sin cal: ladrillo sin revoque y la azotea (que pide cal) pasa a teja.
    let mut sin_cal = todo;
    sin_cal[Material::Cal.idx()] = 0.0;
    let o = elegir(&plan, &sin_cal);
    assert_eq!((o.paredes, o.revoque), (Pared::Ladrillo, false));
    assert_ne!(o.techo, Techo::Azotea);
    assert!(o.falto(Material::Cal));
    // Sin ladrillos ni piedra: adobe; sin adobe: tablas si hay madera.
    let mut sin_lad = todo;
    sin_lad[Material::Ladrillo.idx()] = 0.0;
    assert_eq!(elegir(&plan, &sin_lad).paredes, Pared::Adobe);
    sin_lad[Material::Adobe.idx()] = 0.0;
    let o = elegir(&plan, &sin_lad);
    assert_eq!(o.paredes, Pared::Tablas, "solo madera: casa de tablas");
    // Se anota lo que faltó de la primera preferencia, no de cada alternativa.
    assert!(o.falto(Material::Ladrillo) && !o.falto(Material::Adobe) && !o.falto(Material::Piedra));
    // La teja y las tablas compiten por la misma madera: lo reservado para las paredes no se
    // usa dos veces.
    let solo_madera = deposito(&[(Material::Madera, 30.0)]);
    let o = elegir(&plan, &solo_madera);
    assert_eq!(o.paredes, Pared::Tablas);
    assert_eq!(o.techo, Techo::Paja, "la madera alcanzó solo para las paredes: {o:?}");
}

#[test]
fn sin_nada_se_construye_igual_con_barro_y_paja() {
    let vacio = [0.0f32; N];
    for level in 1..=2 {
        for k in ZoneKind::ALL {
            let plan = plan_solar(k, Density::Media, level, 7, false);
            let o = elegir(&plan, &vacio);
            assert_eq!((o.paredes, o.techo), (Pared::Barro, Techo::Paja), "{k:?} nivel {level}");
            assert!(!o.revoque && !o.rejas);
        }
    }
    // Y consumir lo que no hay no rompe nada.
    let mut d = vacio;
    let o = elegir(&casa(2), &d);
    consumir(&o, 2.0, &mut d);
    assert!(d.iter().all(|&x| x >= 0.0));
}

#[test]
fn los_materiales_deciden_fuego_duracion_humedad_valor_y_pisos() {
    let rancho = Obra::nueva(Pared::Barro, Techo::Paja, false, false);
    let adobe = Obra::nueva(Pared::Adobe, Techo::Paja, false, false);
    let azotea = Obra::nueva(Pared::Ladrillo, Techo::Azotea, true, true);
    let tablas = Obra::nueva(Pared::Tablas, Techo::Tablas, false, false);
    let piedra = Obra::nueva(Pared::Piedra, Techo::Azotea, true, true);
    assert!(rancho.fuego() > azotea.fuego() * 8.0 && tablas.fuego() > azotea.fuego() * 5.0);
    assert!(tablas.contagio() > azotea.contagio());
    assert!(rancho.vida() < adobe.vida() && adobe.vida() < azotea.vida() && azotea.vida() < piedra.vida());
    assert!(rancho.humedad() > adobe.humedad() && adobe.humedad() > azotea.humedad());
    assert!(piedra.prestigio() > azotea.prestigio() && azotea.prestigio() > adobe.prestigio() && adobe.prestigio() > rancho.prestigio());
    assert_eq!((rancho.pisos_max(), adobe.pisos_max(), tablas.pisos_max()), (1, 1, 1));
    assert_eq!((azotea.pisos_max(), piedra.pisos_max()), (2, 2));
    assert_eq!(Obra::nueva(Pared::Ladrillo, Techo::Teja, false, false).pisos_max(), 1, "sin cal no hay altos");
    assert!(rancho.efectos().contains("arde fácil") && rancho.efectos().contains("húmeda"), "{}", rancho.efectos());
    assert!(azotea.efectos().contains("casi no arde"), "{}", azotea.efectos());
    // Los altos no se levantan sin cal aunque sobren ladrillos.
    let plan = casa(3);
    let o = elegir(&plan, &deposito(&[(Material::Ladrillo, 900.0), (Material::Madera, 100.0), (Material::Arena, 100.0)]));
    assert!(o.pisos_max() < 2);
    let o = elegir(&plan, &deposito(&[(Material::Ladrillo, 900.0), (Material::Madera, 100.0), (Material::Arena, 100.0), (Material::Cal, 100.0)]));
    assert_eq!((o.paredes, o.pisos_max()), (Pared::Ladrillo, 2));
}

#[test]
fn la_ciudad_gasta_lo_que_construye_y_registra_la_demanda() {
    let mut city = City::default();
    city.economy.stocks[Res::Ladrillos as usize].amount = 100.0;
    city.economy.stocks[Res::Piedra as usize].amount = 0.0;
    city.materiales.ex[Material::Cal.idx()].cantidad = 10.0;
    let plan = casa(2);
    let o = city.construir_obra(&plan);
    assert_eq!(o.paredes, Pared::Ladrillo);
    assert!(city.economy.stock(Res::Ladrillos).amount < 100.0 - 59.0, "se gastaron ladrillos: {}", city.economy.stock(Res::Ladrillos).amount);
    assert!(city.materiales.ex[Material::Cal.idx()].cantidad < 10.0);
    // La segunda ya no tiene ladrillos: adobe, y queda anotada la sustitución.
    city.materiales.ex[Material::Adobe.idx()].cantidad = 500.0;
    let o2 = city.construir_obra(&plan);
    assert_eq!(o2.paredes, Pared::Adobe);
    assert!(city.materiales.mes_sust[Material::Ladrillo.idx()] >= 1);
    assert!(city.materiales.mes_pedido[Material::Ladrillo.idx()] >= 120.0);
    assert_eq!(city.materiales.obras_mes, (2, 2));
    // La demanda de ladrillos llega a la economía (precio y encargos al puerto).
    let ent = city.entorno_materiales();
    let news = city.materiales.cerrar_mes(&ent, false, 0.0, &|_| 0.0);
    assert!(news.iter().any(|n| n.starts_with("Faltan ladrillos")), "{news:?}");
    assert!(city.materiales.pedidos_economia()[Res::Ladrillos as usize] > 20.0);
}

/// Una fuente local sin nada (para ver solo lo que trae el puerto).
struct Nada;
impl FuenteLocal for Nada {
    fn disponibilidad_local(&self, _: Material) -> f32 {
        0.0
    }
}

#[test]
fn lo_que_no_hay_en_el_lugar_se_importa_por_el_puerto() {
    let mut m = Materiales::default();
    m.ex[Material::Cal.idx()].cantidad = 0.0;
    m.mes_pedido[Material::Cal.idx()] = 60.0;
    m.mes_sust[Material::Cal.idx()] = 3;
    let news = m.cerrar_mes(&Nada, false, 0.0, &|_| 0.0);
    assert!(news.iter().any(|n| n.starts_with("Falta cal")), "{news:?}");
    assert_eq!(m.ex[Material::Cal.idx()].importado, 0.0, "sin barcos no llega nada");
    let precio_sin = m.ex[Material::Cal.idx()].precio;
    assert!(precio_sin > 1.2, "la falta sube el precio: {precio_sin}");
    m.mes_pedido[Material::Cal.idx()] = 60.0;
    let news = m.cerrar_mes(&Nada, true, 200.0, &|_| 0.0);
    assert!(m.ex[Material::Cal.idx()].importado > 20.0, "{:?}", m.ex[Material::Cal.idx()]);
    assert!(news.iter().any(|n| n.contains("Llegó cal")), "{news:?}");
    // Lo que no se importa (adobe) no llega por el puerto.
    assert_eq!(m.ex[Material::Adobe.idx()].importado, 0.0);
    // La clave común con el catálogo de minerales.
    for k in ["piedra", "ladrillo", "teja", "cal", "arena", "madera", "adobe", "paja"] {
        assert_eq!(Material::de_clave(k).map(Material::clave), Some(k));
    }
}

#[test]
fn los_voxeles_muestran_los_materiales_reales() {
    let nominal = Obra::nueva(Pared::Ladrillo, Techo::Azotea, true, true);
    let pts = [(IVec3::new(0, 1, 0), CAL), (IVec3::new(1, 1, 0), CAL_OCRE), (IVec3::new(0, 5, 0), AZOTEA), (IVec3::new(2, 2, 0), REJA), (IVec3::new(3, 2, 0), WINDOW)];
    let base = || {
        let mut e = Edits::default();
        for (p, m) in pts {
            e.design.insert(p, m);
        }
        e
    };
    let e = base();
    // Los mismos materiales del dibujo: no cambia nada.
    let mut igual = base();
    remapear(&mut igual, &nominal, &nominal, 1);
    assert_eq!(igual.design, e.design);
    // Sin cal: ladrillo visto. Sin hierro: ventana sin reja. Con techo de tablas.
    let mut sin_cal = base();
    remapear(&mut sin_cal, &Obra::nueva(Pared::Ladrillo, Techo::Tablas, false, false), &nominal, 1);
    assert!(matches!(sin_cal.design[&IVec3::new(0, 1, 0)], BRICK | BRICK_DARK));
    assert!(matches!(sin_cal.design[&IVec3::new(0, 5, 0)], TABLAS | TABLAS_GRIS));
    assert_eq!(sin_cal.design[&IVec3::new(2, 2, 0)], WINDOW);
    assert_eq!(sin_cal.design[&IVec3::new(3, 2, 0)], WINDOW);
    let mut tablas = base();
    remapear(&mut tablas, &Obra::nueva(Pared::Tablas, Techo::Paja, false, false), &nominal, 1);
    assert!(matches!(tablas.design[&IVec3::new(1, 1, 0)], TABLAS | TABLAS_GRIS));
    assert!(matches!(tablas.design[&IVec3::new(0, 5, 0)], THATCH | THATCH_DARK));
    let mut barro = base();
    remapear(&mut barro, &Obra::nueva(Pared::Barro, Techo::Cuero, false, false), &nominal, 1);
    assert!(matches!(barro.design[&IVec3::new(0, 1, 0)], QUINCHA | QUINCHA_DARK));
    assert_eq!(barro.design[&IVec3::new(0, 5, 0)], HIDE);
}

/// Una ciudad que crece sin cal ni hierro y con pocos ladrillos: casas de adobe y de tablas,
/// sin altos; el fuego, la humedad y el valor siguen a los materiales; y todo se guarda.
#[test]
fn una_ciudad_sin_cal_crece_sin_altos_y_guarda_sus_materiales() {
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    let c = city.plaza.unwrap().0;
    city.paint(c, 300.0, zone_code(ZoneKind::Residencial, Density::Media));
    // Agua para que las casas puedan crecer.
    for d in [glam::Vec2::new(-90.0, -90.0), glam::Vec2::new(90.0, 90.0), glam::Vec2::new(-90.0, 90.0), glam::Vec2::new(90.0, -90.0)] {
        let (sc, sf) = crate::city::services::tests::free_site(&city, &t, crate::city::services::Kind::Aljibe, c + d);
        city.place_service(&t, crate::city::services::Kind::Aljibe, sc, sf).unwrap();
    }
    city.economy.stocks[Res::Ladrillos as usize].amount = 300.0;
    city.materiales.ex[Material::Cal.idx()].cantidad = 0.0;
    city.materiales.ex[Material::Madera.idx()].cantidad = 400.0;
    city.fast_forward(&t, 40);
    let niveles: Vec<u8> = city.lots.lots.values().map(|l| l.level).collect();
    eprintln!("niveles {:?}", (0..4).map(|n| niveles.iter().filter(|&&x| x == n).count()).collect::<Vec<_>>());
    let obras: Vec<Obra> = city.lots.lots.values().filter(|l| l.level > 0).map(|l| l.obra.expect("toda casa edificada tiene obra")).collect();
    assert!(obras.len() > 30, "{} casas", obras.len());
    assert!(city.lots.lots.values().all(|l| l.level < 3), "sin cal no hay casas de altos");
    assert!(obras.iter().all(|o| !o.revoque), "sin cal no hay revoque");
    let kinds: std::collections::BTreeSet<Pared> = obras.iter().map(|o| o.paredes).collect();
    assert!(kinds.len() >= 2, "materiales distintos según lo que había: {kinds:?}");
    assert!(city.lots.sin_altos);
    let l = city.lots.lots.values().find(|l| l.level == 2).expect("alguna casa de una planta");
    assert!(crate::city::lots::growth_blockers(&city.lots, l, &city.net, 1000, city.demand()).iter().any(|w| w.contains("cal")));
    // El fuego sigue al techo real y la humedad enferma.
    let mut paja = city.lots.lots.values().find(|l| l.level > 0).unwrap().clone();
    paja.obra = Some(Obra::nueva(Pared::Adobe, Techo::Paja, false, false));
    let mut azotea = paja.clone();
    azotea.obra = Some(Obra::nueva(Pared::Ladrillo, Techo::Azotea, true, true));
    assert!(crate::city::services::fires::fuego_de(&paja).0 > crate::city::services::fires::fuego_de(&azotea).0 * 10.0);
    assert!(city.fire_risk(&paja) > city.fire_risk(&azotea));
    // Guardar y cargar conserva las obras y el depósito.
    let data = CityData::from_city(&city);
    let json = serde_json::to_string(&data).unwrap();
    let back: CityData = serde_json::from_str(&json).unwrap();
    let mut loaded = back.to_city().unwrap();
    for (a, b) in loaded.lots.lots.values().zip(city.lots.lots.values()) {
        assert_eq!(a.obra.filter(|_| a.level > 0), b.obra.filter(|_| b.level > 0), "solar {}", a.id);
    }
    assert_eq!(loaded.materiales, city.materiales);
    let e = loaded.rebuild_edits(&t);
    assert!(e.voxel_count() > 1000);
    // Llega la cal: ahora sí hay altos.
    loaded.materiales.ex[Material::Cal.idx()].cantidad = 2000.0;
    loaded.economy.stocks[Res::Ladrillos as usize].amount = 3000.0;
    assert!(loaded.altos_posibles());
}

#[test]
fn una_partida_vieja_deduce_los_materiales_de_cada_casa() {
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    city.fast_forward(&t, 20);
    let mut d = CityData::from_city(&city);
    d.materiales = None;
    for l in &mut d.solares.solares {
        l.obra = None;
    }
    let json = serde_json::to_string(&d).unwrap();
    let back: CityData = serde_json::from_str(&json).unwrap();
    let c = back.to_city().unwrap();
    for l in c.lots.lots.values().filter(|l| l.level > 0) {
        let (k, dd) = crate::city::lots::zone_of(l.zone).unwrap();
        assert_eq!(l.obra, Some(nominal(k, dd, l.level, l.seed, None)), "la de su dibujo");
    }
    let r = c.lots.lots.values().find(|l| l.level == 1);
    if let Some(r) = r {
        assert_eq!(r.obra.unwrap().techo, Techo::Paja, "los ranchos, de paja");
    }
}

#[test]
fn un_servicio_sin_ladrillos_se_levanta_igual() {
    use crate::city::services::Kind;
    let t = TerrainGen::new(1726);
    let mut city = town(&t);
    city.economy.stocks[Res::Ladrillos as usize].amount = 0.0;
    city.economy.stocks[Res::Piedra as usize].amount = 0.0;
    city.materiales.ex[Material::Adobe.idx()].cantidad = 1000.0;
    let c = city.plaza.unwrap().0;
    let (sc, sf) = crate::city::services::tests::free_site(&city, &t, Kind::Escuela, c);
    assert!(city.service_problem(&t, Kind::Escuela, sc, sf).is_none(), "los materiales no la frenan");
    let (_, txt) = city.vista_previa_servicio(Kind::Escuela).unwrap();
    assert!(txt.contains("Falta") && txt.contains("adobe"), "{txt}");
    city.place_service(&t, Kind::Escuela, sc, sf).unwrap();
    let id = *city.services.buildings.keys().next_back().unwrap();
    assert_eq!(city.materiales.servicios[&id].paredes, Pared::Adobe);
    assert!(city.service_lines(id).iter().any(|l| l.contains("adobe")));
}
