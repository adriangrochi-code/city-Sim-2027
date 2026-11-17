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
    let mut con = RecursosLote { cultivo: 0.6, pastoreo: 0.6, ..Default::default() };
    con.mat[Material::Gneis.i()] = 0.8;
    con.mat[Material::ArcillaRoja.i()] = 0.7;
    con.derivar();
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
    // Sobre los limos no hay piedra: lo que saca una cantera ahí es tosca (para caminos), que
    // casi no da piedra.
    let p = sin_piedra.expect("hay campo sin piedra");
    let r = rec.en_area(p, Vec2::NEG_Y, 28.0, 22.0);
    let prod = materiales::producto_cantera(&r);
    assert!(matches!(prod.material, Some(Material::Tosca) | None), "{prod:?}");
    assert!(prod.base < 0.5);
    // Y donde no hay nada que sacar (el bañado, la playa sin médano), no se puede.
    let nada = RecursosLote::default();
    assert!(materiales::producto_cantera(&nada).material.is_none());
    let _ = city.service_problem(t, Kind::Cantera, p, Vec2::NEG_Y);
}

// --- Materiales con tipo y catálogo ------------------------------------------------------------

/// Muestras de tierra del mapa real y su entorno al norte (cada 300 m) con su geología (el
/// Granito de La Paz queda al norte del mapa jugable).
fn muestras(t: &TerrainGen) -> Vec<(Vec2, RecursosLote, String)> {
    let rec = Recursos::de(t);
    let mut v = Vec::new();
    for j in 0..100 {
        for i in 0..100 {
            let p = Vec2::new(150.0 + i as f32 * 300.0 - 3000.0, 150.0 + j as f32 * 300.0 - 6000.0);
            if t.height(p.x, p.y) < 1.0 || t.is_water(p.x, p.y) {
                continue;
            }
            let geo = rec.suelo(p).map(|s| s.geologia).unwrap_or_default();
            v.push((p, rec.en_punto(p), geo));
        }
    }
    v
}

#[test]
fn cada_material_esta_donde_dice_la_geologia() {
    let t = real();
    let rec = Recursos::de(t);
    let v = muestras(t);
    let media = |f: &dyn Fn(&str) -> bool, m: Material| {
        let xs: Vec<f32> = v.iter().filter(|x| f(&x.2)).map(|x| x.1.mat[m.i()]).collect();
        assert!(xs.len() > 5, "pocas muestras para {m:?}");
        xs.iter().sum::<f32>() / xs.len() as f32
    };
    // El Cerro y la costa: gneis de la Formación Montevideo (la piedra de la Ciudad Vieja).
    let cerro = rec.en_area(crate::terrain::CERRO_REAL + Vec2::new(150.0, 150.0), Vec2::NEG_Y, 28.0, 22.0);
    assert!(cerro.mat[Material::Gneis.i()] >= MINIMO_MATERIAL && cerro.mat[Material::Gneis.i()] >= cerro.mat[Material::Granito.i()], "{:?}", cerro.mat);
    let montevideo = |g: &str| g.contains("Formación Montevideo");
    assert!(media(&montevideo, Material::Gneis) > 0.5);
    assert!(media(&montevideo, Material::Granito) < 0.15);
    // El Granito de La Paz: granito (y el balasto de su alteración).
    let la_paz = |g: &str| g.contains("La Paz");
    assert!(media(&la_paz, Material::Granito) > 0.6, "granito en La Paz {}", media(&la_paz, Material::Granito));
    assert!(media(&la_paz, Material::Balasto) > 0.4);
    assert!(media(&la_paz, Material::Gneis) < media(&la_paz, Material::Granito));
    // Los limos de la Formación Libertad: arcilla roja y tosca, nada de piedra.
    let libertad = |g: &str| g.contains("Libertad");
    assert!(media(&libertad, Material::ArcillaRoja) > 0.5, "arcilla roja {}", media(&libertad, Material::ArcillaRoja));
    assert!(media(&libertad, Material::Tosca) > 0.3);
    assert!(media(&libertad, Material::Granito) + media(&libertad, Material::Gneis) < 0.1);
    // Playas y médanos: arena.
    let playas: Vec<f32> = v.iter().filter(|x| t.official_biome(x.0.x, x.0.y) == BIO_PLAYA).map(|x| x.1.mat[Material::Arena.i()]).collect();
    assert!(!playas.is_empty() && playas.iter().sum::<f32>() / playas.len() as f32 > 0.8);
    // Arenas litorales y fluviales (médanos de Carrasco y del oeste): arena.
    let arenas = |g: &str| g.contains("Arenas litorales");
    assert!(media(&arenas, Material::Arena) > 0.6);
    // No hay caliza en el mapa ni en su entorno.
    assert!(v.iter().all(|x| x.1.mat[Material::Caliza.i()] == 0.0));
    for p in [Vec2::new(-20000.0, 5000.0), Vec2::new(40000.0, -15000.0), Vec2::new(30000.0, 20000.0)] {
        assert_eq!(t.material(Material::Caliza.i(), p.x, p.y), Some(0.0));
    }
    // Los grupos de siempre salen de los materiales.
    for (_, r, _) in v.iter().take(400) {
        assert_eq!(r.piedra, r.mat[Material::Granito.i()].max(r.mat[Material::Gneis.i()]));
        assert!(r.mat.iter().all(|x| (0.0..=1.0).contains(x)));
    }
}

#[test]
fn la_cantera_saca_segun_el_material_y_el_horno_segun_el_barro() {
    let con = |m: Material, x: f32| {
        let mut r = RecursosLote::default();
        r.mat[m.i()] = x;
        r.derivar();
        r
    };
    use materiales::{producto_cantera, producto_horno, Extra};
    let g = producto_cantera(&con(Material::Granito, 0.8));
    assert_eq!(g.material, Some(Material::Granito));
    assert!(g.base >= 1.0 && g.extras.iter().any(|e| e.0 == Extra::Adoquines && e.1 > 0.0) && g.extras.iter().any(|e| e.0 == Extra::Silleria && e.1 > 0.0));
    let n = producto_cantera(&con(Material::Gneis, 0.8));
    assert_eq!(n.material, Some(Material::Gneis));
    assert!(n.base >= 1.0 && !n.extras.iter().any(|e| e.0 == Extra::Adoquines && e.1 > 0.0), "el gneis no da adoquines");
    let a = producto_cantera(&con(Material::Arena, 0.9));
    assert_eq!(a.base, 0.0);
    assert!(a.extras.iter().any(|e| e.0 == Extra::Arena && e.1 > 0.0));
    let b = producto_cantera(&con(Material::Balasto, 0.7));
    assert!(b.base > 0.0 && b.base < 1.0);
    assert!(rinde(Kind::Cantera, &con(Material::Arena, 0.9)) > 1.0);
    // El horno: la arcilla roja da ladrillos; la gris, menos ladrillos y tejas.
    let roja = producto_horno(&con(Material::ArcillaRoja, 0.7));
    let gris = producto_horno(&con(Material::ArcillaGris, 0.7));
    assert!(roja.base > gris.base);
    assert!(gris.extras.iter().any(|e| e.0 == Extra::Tejas && e.1 > 0.0));
    assert!(!roja.extras.iter().any(|e| e.0 == Extra::Tejas && e.1 > 0.0));
    // En el mapa real, una cantera en el Cerro saca gneis y en La Paz granito.
    let t = real();
    let rec = Recursos::de(t);
    let cerro = producto_cantera(&rec.en_area(crate::terrain::CERRO_REAL + Vec2::new(150.0, 150.0), Vec2::NEG_Y, 28.0, 22.0));
    assert_eq!(cerro.material, Some(Material::Gneis));
    let v = muestras(t);
    let la_paz = v.iter().filter(|x| x.2.contains("La Paz")).map(|x| producto_cantera(&x.1)).filter(|p| p.material.is_some()).collect::<Vec<_>>();
    // En La Paz se saca el granito sano y, sobre todo, el alterado: el balasto del pedregullo
    // de Montevideo (memoria de la carta geológica).
    let granito = la_paz.iter().filter(|p| p.material == Some(Material::Granito)).count();
    let balasto = la_paz.iter().filter(|p| p.material == Some(Material::Balasto)).count();
    assert!((granito + balasto) * 10 > la_paz.len() * 9 && granito > 0 && balasto > 0, "La Paz: {granito} granito, {balasto} balasto de {}", la_paz.len());
}

#[test]
fn la_calera_necesita_caliza_o_conchilla_y_si_no_la_cal_se_importa() {
    use crate::city::desarrollo::{Contexto, Desarrollo, Prod};
    use materiales::fuente_de_cal;
    // Sin caliza ni conchilla no hay piedra de cal; con conchilla sí (rinde menos que la caliza).
    let mut r = RecursosLote::default();
    assert!(fuente_de_cal(&r).is_none());
    assert_eq!(rinde(Kind::Calera, &r), 0.0);
    r.mat[Material::Conchilla.i()] = 0.5;
    assert_eq!(fuente_de_cal(&r).map(|x| x.0), Some(Material::Conchilla));
    let con_conchilla = rinde(Kind::Calera, &r);
    r.mat[Material::Caliza.i()] = 0.5;
    assert_eq!(fuente_de_cal(&r).map(|x| x.0), Some(Material::Caliza));
    assert!(rinde(Kind::Calera, &r) > con_conchilla && con_conchilla > 0.0);
    // En el mapa real, sobre los limos no se puede: no hay caliza.
    let t = real();
    let rec = Recursos::de(t);
    let mut city = City { money: 1_000_000, ..Default::default() };
    city.plaza = Some((crate::terrain::FOUNDING_REAL, 0.0));
    city.lots.plaza_center = Some(crate::terrain::FOUNDING_REAL);
    let p = muestras(t).into_iter().find(|x| x.2.contains("Libertad") && x.0.distance(crate::terrain::FOUNDING_REAL) > 2000.0 && fuente_de_cal(&rec.en_area(x.0, Vec2::NEG_Y, 20.0, 20.0)).is_none()).unwrap().0;
    let msg = city.service_problem(t, Kind::Calera, p, Vec2::NEG_Y).unwrap_or_default();
    assert!(msg.contains("caliza") || msg.contains("calle"), "{msg}");
    // La cal que piden las obras y no dan las caleras se trae por barco y cuesta.
    let ctx = |demanda: f32, local: f32| {
        let mut c = Contexto { mes: 10, muelle: true, desocupados: 0, habitantes: 0, cal_demanda: demanda, ..Default::default() };
        c.prod_sitios[Prod::Cal.i()] = local;
        c
    };
    let mut d = Desarrollo::default();
    let sin_obras = d.mes(&ctx(0.0, 0.0));
    let imp0 = d.historia.back().unwrap().importado;
    let mut d = Desarrollo::default();
    let con_obras = d.mes(&ctx(100.0, 0.0));
    let imp1 = d.historia.back().unwrap().importado;
    assert_eq!(sin_obras.cal_importada, 0.0);
    assert!((con_obras.cal_importada - 100.0).abs() < 1e-3);
    assert!(imp1 > imp0 + 80.0, "la cal importada pesa en las importaciones: {imp0} → {imp1}");
    // Con una calera que alcanza, no se importa y el cabildo vende la cal.
    let mut d = Desarrollo::default();
    let propia = d.mes(&ctx(100.0, 120.0));
    assert_eq!(propia.cal_importada, 0.0);
    assert!(propia.lineas.iter().any(|l| l.0.contains("caleras") && l.1 > 0), "{:?}", propia.lineas);
}

#[test]
fn el_catalogo_carga_y_cada_entrada_tiene_fuente() {
    let c = materiales::catalogo();
    assert!(c.recursos.len() >= 40, "{} recursos", c.recursos.len());
    let estados = ["MEDIDO", "DOCUMENTADO", "ESTIMADO"];
    for e in &c.recursos {
        assert!(!e.fuentes.is_empty(), "{} sin fuente", e.id);
        for f in &e.fuentes {
            assert!(c.fuentes.contains_key(f), "{}: fuente {f} desconocida", e.id);
        }
        assert!(estados.contains(&e.estado.as_str()), "{}: estado {}", e.id, e.estado);
        assert!(c.categorias.contains_key(&e.categoria), "{}: categoría {}", e.id, e.categoria);
        for h in &e.historia {
            assert!(c.fuentes.contains_key(&h.fuente) && estados.contains(&h.estado.as_str()), "{}: {}", e.id, h.texto);
        }
        for p in &e.productos {
            assert!(c.productos.iter().any(|x| &x.id == p), "{}: producto {p}", e.id);
        }
        // Lo MEDIDO cita un servicio o una estadística oficial.
        if e.estado == "MEDIDO" {
            assert!(e.fuentes.iter().any(|f| matches!(c.fuentes[f].tipo.as_str(), "servicio" | "estadistica")), "{}", e.id);
        }
    }
    for p in &c.productos {
        assert!(c.fuentes.contains_key(&p.fuente), "{}", p.id);
        for i in &p.insumos {
            assert!(materiales::entrada(i).is_some() || c.productos.iter().any(|x| &x.id == i), "{}: insumo {i}", p.id);
        }
    }
    // Cada material del mapa tiene su entrada, y lo que dice de Montevideo coincide.
    for m in Material::TODOS {
        let e = m.entrada();
        assert_eq!(e.en_montevideo.material.as_deref(), Some(m.id()), "{m:?}");
    }
    assert!(!Material::Caliza.entrada().en_montevideo.hay);
    assert_eq!(Material::Granito.construccion(), Some("piedra"));
    assert_eq!(Material::ArcillaRoja.construccion(), Some("ladrillo"));
    assert_eq!(Material::ArcillaGris.construccion(), Some("teja"));
    assert_eq!(Material::Arena.construccion(), Some("arena"));
    assert_eq!(Material::Tosca.construccion(), None);
    // La cal sale de la caliza (que no hay en Montevideo) y de la conchilla; el mármol no.
    assert!(materiales::de_construccion("cal").iter().any(|e| e.id == "caliza"));
    assert!(materiales::se_saca_del_suelo("piedra") && materiales::se_saca_del_suelo("ladrillo") && materiales::se_saca_del_suelo("madera"));
    assert!(!materiales::se_saca_del_suelo("mármol"));
    // La amatista no es lo mismo que la caliza: distinta categoría, precio, usos y lugar.
    let (am, ca) = (materiales::entrada("amatista").unwrap(), materiales::entrada("caliza").unwrap());
    assert_eq!((am.categoria.as_str(), ca.categoria.as_str()), ("gema", "carbonato"));
    assert!(am.localidades.iter().any(|l| l.contains("Artigas")) && ca.localidades.iter().any(|l| l.contains("Minas")));
}

#[test]
fn los_materiales_cerca_de_una_obra() {
    let t = real();
    let rec = Recursos::de(t);
    let cerro = rec.materiales_cerca(crate::terrain::CERRO_REAL, 400.0);
    assert!(cerro[Material::Gneis.i()] > 0.3);
    assert!(rec.construccion_cerca("piedra", crate::terrain::CERRO_REAL, 400.0) > 0.3);
    assert_eq!(rec.construccion_cerca("madera", crate::terrain::CERRO_REAL, 400.0), 0.0, "la madera no es un material del subsuelo");
    // Más radio, nunca menos.
    let lejos = rec.materiales_cerca(crate::terrain::FOUNDING_REAL, 1500.0);
    let cerca = rec.materiales_cerca(crate::terrain::FOUNDING_REAL, 300.0);
    assert!(lejos.iter().zip(&cerca).all(|(a, b)| a >= b));
}

#[test]
fn el_dataset_del_pais_tiene_cada_recurso_en_su_departamento() {
    #[derive(serde::Deserialize)]
    struct Grilla {
        lon_oeste: f32,
        lat_norte: f32,
        dlon: f32,
        dlat: f32,
        columnas: usize,
        filas: usize,
    }
    #[derive(serde::Deserialize)]
    struct PorMaterial {
        titulos: u32,
        departamentos: std::collections::BTreeMap<String, u32>,
    }
    #[derive(serde::Deserialize)]
    struct Titulos {
        por_material: std::collections::BTreeMap<String, PorMaterial>,
    }
    #[derive(serde::Deserialize)]
    struct Pais {
        grilla: Grilla,
        titulos: Titulos,
    }
    let pais: Pais = serde_json::from_str(include_str!("../../../../assets/datos/uruguay/pais.json")).unwrap();
    let geo = include_bytes!("../../../../assets/datos/uruguay/geologia_pais.bin");
    let g = &pais.grilla;
    assert_eq!(geo.len(), g.columnas * g.filas);
    let unidad = |lon: f32, lat: f32| {
        let i = ((lon - g.lon_oeste) / g.dlon).round() as usize;
        let j = ((g.lat_norte - lat) / g.dlat).round() as usize;
        geo[i + j * g.columnas] as i32 - 1
    };
    // Los Catalanes (Artigas): basalto de Arapey; Minas (Lavalleja) y Montevideo: basamento.
    assert_eq!(unidad(-56.28, -30.83), 54);
    assert!((0..=41).contains(&unidad(-55.24, -34.37)), "Minas: {}", unidad(-55.24, -34.37));
    assert!((0..=41).contains(&unidad(-56.26, -34.89)), "el Cerro: {}", unidad(-56.26, -34.89));
    let mayor = |m: &str| pais.titulos.por_material[m].departamentos.iter().max_by_key(|x| x.1).map(|x| x.0.clone()).unwrap();
    assert_eq!(mayor("amatista"), "Artigas");
    assert_eq!(mayor("agata"), "Artigas");
    assert_eq!(mayor("oro"), "Rivera");
    assert_eq!(mayor("bentonita"), "Cerro Largo");
    assert!(["Lavalleja", "Treinta y Tres"].contains(&mayor("caliza").as_str()));
    assert!(pais.titulos.por_material["tosca"].titulos > 300, "la tosca es lo que más títulos tiene");
    // Cada material del catálogo con sustancias del catastro aparece en algún título.
    for e in &materiales::catalogo().recursos {
        if !e.sustancias_catastro.is_empty() && e.estado == "MEDIDO" && e.id != "arenisca" {
            assert!(pais.titulos.por_material.contains_key(&e.id), "{} sin títulos", e.id);
        }
    }
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
