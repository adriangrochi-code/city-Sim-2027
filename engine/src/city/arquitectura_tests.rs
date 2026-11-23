//! Pruebas de la variedad de la arquitectura colonial (ver `arquitectura` y
//! `structures::estilo`).

use super::arquitectura::*;
use super::lots::{Density, ZoneKind};
use super::materiales::{Obra, Pared, Techo};
use crate::buildings::Edits;
use crate::palette::*;
use crate::structures::casas::construir;
use crate::structures::estilo::{self, es_cal, Contexto, Estilo, Tipo, Tradicion, Uso};
use crate::structures::Frame;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3};
use std::collections::HashSet;

fn ctx(anio: u16, clase: u8) -> Contexto {
    Contexto { anio, clase, lugar: 1, ..Default::default() }
}

fn obra(p: Pared, t: Techo, rev: bool, rejas: bool, c: Contexto) -> Obra {
    let mut o = Obra::nueva(p, t, rev, rejas);
    o.contexto = c;
    o
}

fn est(o: &Obra, uso: Uso, nivel: u8, seed: u64) -> Estilo {
    estilo::estilo(&o.contexto, &materiales_obra(o), uso, nivel, false, seed)
}

/// Dibuja una casa en un solar alineado (24 × 76) y devuelve sus vóxeles en coordenadas
/// locales (u, y, w) con su material.
fn dibujar(t: &TerrainGen, es: &Estilo, seed: u64) -> Vec<((i32, i32, i32), Mat)> {
    let f = Frame { origin: IVec2::new(54400, 64400), du: IVec2::X, dw: IVec2::Y, base: 40, width: 24, depth: 76, seed };
    let mut e = Edits::default();
    construir(t, &mut e, &f, es);
    let mut v: Vec<((i32, i32, i32), Mat)> = e.design.iter().map(|(p, m)| (f.local(*p), *m)).filter(|((_, y, _), _)| *y >= 0).collect();
    v.sort();
    v
}

#[test]
fn la_misma_semilla_da_la_misma_casa() {
    let t = TerrainGen::new(1726);
    for (k, (p, te)) in [(Pared::Ladrillo, Techo::Azotea), (Pared::Piedra, Techo::Teja), (Pared::Adobe, Techo::Paja)].into_iter().enumerate() {
        let o = obra(p, te, p != Pared::Adobe, p != Pared::Adobe, ctx(1805, 2));
        let a = est(&o, Uso::Vivienda, 2, 77 + k as u64);
        let b = est(&o, Uso::Vivienda, 2, 77 + k as u64);
        assert_eq!(a, b);
        assert_eq!(dibujar(&t, &a, 9), dibujar(&t, &b, 9), "mismo estilo, mismos vóxeles");
    }
    // Y la casa de un solar de la ciudad, por el camino del juego (girada).
    let o = obra(Pared::Ladrillo, Techo::Azotea, true, true, ctx(1800, 1));
    let c = glam::Vec2::new(13600.0, 16100.0);
    let f = glam::Vec2::new(0.6, 0.8);
    let a = super::build_lot(&t, c, f, 4242, ZoneKind::Residencial, Density::Media, 2, Some(o));
    let b = super::build_lot(&t, c, f, 4242, ZoneKind::Residencial, Density::Media, 2, Some(o));
    let (mut va, mut vb) = (a.voxels.clone(), b.voxels.clone());
    va.sort_by_key(|v| (v.x, v.y, v.z));
    vb.sort_by_key(|v| (v.x, v.y, v.z));
    assert_eq!(va, vb);
    assert_eq!(a.edits.design.len(), b.edits.design.len());
}

#[test]
fn casas_de_la_misma_zona_y_nivel_son_muy_distintas() {
    // 60 casas de una planta de 1800, con lo que hay: muchas combinaciones distintas.
    let mut firmas = HashSet::new();
    let mut colores = HashSet::new();
    let mut tipos = HashSet::new();
    let mut detalles = HashSet::new();
    for s in 0..60u64 {
        let techo = if s % 3 == 0 { Techo::Teja } else { Techo::Azotea };
        let o = obra(Pared::Ladrillo, techo, true, true, Contexto { tradicion: (s % 9) as u8, ..ctx(1800, (s % 4) as u8) });
        let e = est(&o, Uso::Vivienda, 2, s * 7919 + 13);
        firmas.insert(e.firma());
        colores.insert(e.muro);
        tipos.insert(e.tipo);
        detalles.insert((e.reja as u8, e.cornisa, e.pretil_tipo, e.portada, e.zocalo, e.carpinteria, e.alto));
    }
    assert!(firmas.len() >= 30, "{} combinaciones de tipología, color, vanos y puerta", firmas.len());
    assert!(colores.len() >= 4, "colores de cal: {colores:?}");
    assert!(tipos.len() >= 2);
    assert!(detalles.len() >= 40, "{} combinaciones de detalles", detalles.len());
    // Los ranchos también varían (tamaño, enramada, horno, corral, cumbrera).
    let ranchos: HashSet<_> = (0..30u64).map(|s| {
        let e = est(&obra(Pared::Adobe, Techo::Paja, false, false, ctx(1740, 0)), Uso::Vivienda, 1, s * 31 + 1);
        (e.ancho, e.fondo, e.enramada, e.horno, e.corral, e.hastial)
    }).collect();
    assert!(ranchos.len() >= 15, "{} ranchos distintos", ranchos.len());
}

#[test]
fn los_vecinos_no_se_repiten() {
    // Una ciudad que crece: las casas vecinas cumplen la regla anti-monotonía (no comparten más
    // de 2 de los 4 atributos y la cornisa difiere), salvo alguna en que no hubo tirada que
    // alcanzara.
    let t = TerrainGen::new(1726);
    let mut city = super::services::tests::town(&t);
    let c = city.plaza.unwrap().0;
    for d in [glam::Vec2::new(-90.0, -90.0), glam::Vec2::new(90.0, 90.0), glam::Vec2::new(-90.0, 90.0), glam::Vec2::new(90.0, -90.0)] {
        let (sc, sf) = super::services::tests::free_site(&city, &t, super::services::Kind::Aljibe, c + d);
        city.place_service(&t, super::services::Kind::Aljibe, sc, sf).unwrap();
    }
    // Cal, tejas y madera de sobra: casas de material, no solo ranchos.
    for m in [super::materiales::Material::Cal, super::materiales::Material::Teja, super::materiales::Material::Madera] {
        city.materiales.ex[m.idx()].cantidad = 5000.0;
    }
    city.month = 12 * 70; // 1796: azoteas, teja y algún altos
    city.fast_forward(&t, 36);
    let (mut pares, mut buenos) = (0, 0);
    let mut tipos = HashSet::new();
    let ids: Vec<u32> = city.lots.lots.keys().copied().collect();
    for &a in &ids {
        let la = &city.lots.lots[&a];
        let Some((k, d)) = super::lots::zone_of(la.zone) else { continue };
        let Some(ea) = la.obra.filter(|o| la.level > 0 && o.contexto.anio > 0).and_then(|o| estilo_de(k, d, la.level, la.seed, &o)) else { continue };
        let s = glam::Vec2::new(-la.f.y, la.f.x);
        for &b in &ids {
            let lb = &city.lots.lots[&b];
            if b <= a || lb.f.dot(la.f) < 0.9 || (lb.c - la.c).dot(la.f).abs() > 4.0 || ((lb.c - la.c).dot(s).abs() - super::lots::W).abs() > 3.0 {
                continue;
            }
            let Some((k2, d2)) = super::lots::zone_of(lb.zone) else { continue };
            let Some(eb) = lb.obra.filter(|o| lb.level > 0 && o.contexto.anio > 0).and_then(|o| estilo_de(k2, d2, lb.level, lb.seed, &o)) else { continue };
            pares += 1;
            tipos.insert(ea.tipo);
            buenos += usize::from(ea.distinto_de(&eb));
            if !ea.distinto_de(&eb) {
                let (ca, cb) = (la.obra.unwrap().contexto, lb.obra.unwrap().contexto);
                eprintln!("{:?} {} / {:?} {} · giros {} {} · años {} {}", ea.firma(), ea.linea(), eb.firma(), eb.linea(), ca.giro, cb.giro, ca.anio, cb.anio);
            }
        }
    }
    eprintln!("vecinos distintos: {buenos} de {pares} · tipos {tipos:?}");
    assert!(tipos.len() >= 2, "{tipos:?}");
    assert!(pares >= 20, "{pares} pares de vecinos");
    assert!(buenos * 10 >= pares * 9, "{buenos} de {pares} pares cumplen la regla");
}

#[test]
fn la_epoca_manda_sin_miradores_ni_altos_en_1730() {
    let (mut miradores_1825, mut altos_1825) = (0, 0);
    for s in 0..3000u64 {
        for clase in 0..4u8 {
            let o = obra(Pared::Piedra, Techo::Azotea, true, true, Contexto { puerto: true, lugar: 0, ..ctx(1730, clase) });
            for nivel in [2u8, 3] {
                let e = est(&o, Uso::Vivienda, nivel, s);
                assert!(!e.mirador && !e.altillo, "mirador o altillo en 1730");
                assert_ne!(e.tipo, Tipo::CasaAltos, "casa de altos en 1730");
                assert!(!e.aljibe, "aljibe en 1730");
                assert_ne!(e.reja, estilo::Reja::Voladiza);
                assert_ne!(e.muro, CAL_AZUL, "celeste de añil en 1730");
            }
        }
        let o = obra(Pared::Piedra, Techo::Azotea, true, true, Contexto { puerto: true, lugar: 0, ..ctx(1825, 3) });
        let e = est(&o, Uso::Vivienda, 3, s);
        miradores_1825 += usize::from(e.mirador);
        altos_1825 += usize::from(e.tipo == Tipo::CasaAltos);
    }
    // En 1825 los miradores siguen siendo raros (el primero documentado es de 1848).
    assert!(miradores_1825 > 0 && miradores_1825 < 3000 / 10, "{miradores_1825} miradores en 3000 casas ricas de 1825");
    assert!(altos_1825 > 1500, "{altos_1825} casas de altos");
    // Antes de 1740 no hay azoteas preferidas; en 1815 sí.
    assert_eq!(estilo::prob_azotea(1730, 1, Tradicion::Canaria), 0.0);
    assert!(estilo::prob_azotea(1820, 0, Tradicion::Ninguna) > 0.4);
}

#[test]
fn los_materiales_mandan_sobre_el_estilo() {
    let t = TerrainGen::new(1726);
    for s in 0..40u64 {
        // Adobe sin revoque: ni cal, ni piedra labrada, ni portada.
        let o = obra(Pared::Adobe, Techo::Teja, false, false, ctx(1810, 3));
        let e = est(&o, Uso::Vivienda, 2, s);
        assert_eq!(e.muro, ADOBE);
        assert_eq!(e.portada, 0);
        assert!(!e.piedra && !e.hierro);
        let v = dibujar(&t, &e, s);
        assert!(v.iter().all(|(_, m)| !es_cal(*m) && !matches!(*m, STONE | STONE_DARK | REJA | AZOTEA)), "adobe sin cal con cal, sillería, rejas o azotea");
        // Sin ladrillo y cal no hay azotea: con teja nunca sale una casa de azotea.
        let o = obra(Pared::Ladrillo, Techo::Teja, true, true, ctx(1810, 2));
        let e = est(&o, Uso::Vivienda, 2, s);
        assert_ne!(e.tipo, Tipo::CasaAzotea);
        assert!(dibujar(&t, &e, s).iter().all(|(_, m)| *m != AZOTEA));
        // Ladrillo sin cal: ladrillo visto, sin colores.
        let o = obra(Pared::Ladrillo, Techo::Teja, false, true, ctx(1810, 2));
        let e = est(&o, Uso::Vivienda, 2, s);
        assert!(dibujar(&t, &e, s).iter().all(|(_, m)| !es_cal(*m)), "colores sin revoque");
        // Tablas: sin revoque.
        let o = obra(Pared::Tablas, Techo::Tablas, true, false, ctx(1810, 1));
        let e = est(&o, Uso::Vivienda, 2, s);
        assert_eq!(e.muro, TABLAS);
        // Paja: rancho, aunque el nivel sea de casa.
        let o = obra(Pared::Adobe, Techo::Paja, false, false, ctx(1790, 1));
        assert_eq!(est(&o, Uso::Vivienda, 2, s).tipo, Tipo::Rancho);
        // Sin piedra en la obra, la portada no es de sillería.
        let o = obra(Pared::Ladrillo, Techo::Azotea, true, true, ctx(1815, 3));
        let e = est(&o, Uso::Vivienda, 2, s);
        assert!(!e.piedra);
        assert!(dibujar(&t, &e, s).iter().all(|(_, m)| !matches!(*m, STONE | STONE_DARK) || false), "sillería en una casa de ladrillo");
    }
}

#[test]
fn todo_queda_dentro_del_solar() {
    let t = TerrainGen::new(1726);
    let combos = [(Pared::Ladrillo, Techo::Azotea, true, true), (Pared::Piedra, Techo::Teja, true, true), (Pared::Adobe, Techo::Paja, false, false), (Pared::Barro, Techo::Cuero, false, false), (Pared::Piedra, Techo::Azotea, true, true)];
    for s in 0..60u64 {
        let (p, te, rev, rej) = combos[(s % 5) as usize];
        let c = Contexto { anio: 1726 + (s as u16 * 13) % 105, clase: (s % 4) as u8, esquina: (s % 3) as u8, lugar: (s % 4) as u8, puerto: s % 2 == 0, giro: 0, tradicion: (s % 9) as u8, ..Default::default() };
        let o = obra(p, te, rev, rej, c);
        for (uso, nivel) in [(Uso::Vivienda, 1), (Uso::Vivienda, 2), (Uso::Vivienda, 3), (Uso::Comercio, 1), (Uso::Comercio, 3)] {
            let e = estilo::estilo(&o.contexto, &materiales_obra(&o), uso, nivel, s % 5 == 0, s);
            let v = dibujar(&t, &e, s);
            assert!(v.len() > 100, "{:?} vacía", e.tipo);
            for &((u, _, w), _) in &v {
                assert!((0..24).contains(&u) && (-2..76).contains(&w), "{:?} fuera del solar en u={u} w={w}", e.tipo);
            }
        }
    }
}

#[test]
fn el_origen_de_la_familia_cambia_los_rasgos() {
    // Dos casas iguales en todo salvo el origen de la familia.
    let base = |t: Tradicion| Contexto { tradicion: t.indice(), ..ctx(1815, 3) };
    let (mut madera_can, mut madera_lusa, mut alero_lusa, mut alero_can) = (0, 0, 0, 0);
    let mut distintas = 0;
    for s in 0..400u64 {
        let can = est(&obra(Pared::Piedra, Techo::Azotea, true, true, base(Tradicion::Canaria)), Uso::Vivienda, 3, s);
        let lusa = est(&obra(Pared::Piedra, Techo::Azotea, true, true, base(Tradicion::Lusa)), Uso::Vivienda, 3, s);
        madera_can += usize::from(can.balcon_madera);
        madera_lusa += usize::from(lusa.balcon_madera);
        alero_lusa += usize::from(lusa.alero == 2);
        alero_can += usize::from(can.alero == 2);
        distintas += usize::from(can != lusa);
    }
    assert!(madera_can > madera_lusa * 3 + 50, "balcones de madera: canarios {madera_can}, lusos {madera_lusa}");
    assert!(alero_lusa > alero_can * 3 + 50, "aleros largos: lusos {alero_lusa}, canarios {alero_can}");
    assert!(distintas > 300);
    // La preferencia de techo: los canarios, más azoteas; los portugueses, más teja.
    assert!(estilo::prob_azotea(1800, 1, Tradicion::Canaria) > estilo::prob_azotea(1800, 1, Tradicion::Lusa) * 2.0);
    // Los italianos casi no estaban en la colonia: antes de 1830 su casa es la de la época.
    for s in 0..50u64 {
        let it = est(&obra(Pared::Ladrillo, Techo::Azotea, true, true, base(Tradicion::Italiana)), Uso::Vivienda, 2, s);
        let ning = est(&obra(Pared::Ladrillo, Techo::Azotea, true, true, base(Tradicion::Ninguna)), Uso::Vivienda, 2, s);
        assert_eq!(Estilo { tradicion: Tradicion::Ninguna, ..it }, ning);
        assert!(it.linea_origen().is_none());
    }
    // Los rasgos van a la ficha.
    let can = est(&obra(Pared::Ladrillo, Techo::Azotea, true, true, base(Tradicion::Canaria)), Uso::Vivienda, 2, 1);
    let linea = can.linea_origen().unwrap();
    assert!(linea.starts_with("Casa de familia canaria: azotea"), "{linea}");
    // Sin cal no hay azotea: la ficha no inventa rasgos que la casa no tiene.
    let sin_cal = est(&obra(Pared::Adobe, Techo::Teja, false, false, base(Tradicion::Canaria)), Uso::Vivienda, 2, 1);
    assert_eq!(sin_cal.linea_origen().as_deref(), Some("Casa de familia canaria: sin los rasgos de su tradición (azotea, balcón de madera y carpintería verde)"));
    // El origen sale del hogar: canarios → canaria, africanos → sin rasgos inventados.
    assert_eq!(tradicion_de(super::poblacion::Origen::Canarias), Tradicion::Canaria);
    assert_eq!(tradicion_de(super::poblacion::Origen::AfricaCentral), Tradicion::Ninguna);
    // Determinismo del origen probable.
    assert_eq!(origen_probable(1790, 5), origen_probable(1790, 5));
}

#[test]
fn el_contexto_se_guarda_y_las_partidas_viejas_cargan() {
    let o = obra(Pared::Ladrillo, Techo::Azotea, true, true, Contexto { anio: 1799, clase: 2, esquina: 1, lugar: 0, puerto: true, giro: 3, tradicion: 2, ..Default::default() });
    let j = serde_json::to_string(&o).unwrap();
    let back: Obra = serde_json::from_str(&j).unwrap();
    assert_eq!(back, o);
    // Una obra guardada antes de este sistema (sin contexto) carga y se dibuja igual siempre.
    let vieja = r#"{"paredes":"Ladrillo","techo":"Azotea","revoque":true,"rejas":true,"estado":1.0}"#;
    let o: Obra = serde_json::from_str(vieja).unwrap();
    assert_eq!(o.contexto, Contexto::default());
    let a = estilo_de(ZoneKind::Residencial, Density::Media, 2, 99, &o).unwrap();
    assert_eq!(Some(a), estilo_de(ZoneKind::Residencial, Density::Media, 2, 99, &o));
}

#[test]
fn las_casas_huecas_ahorran_voxeles() {
    // Una casa de azotea con dos patios dibujada con estilo no pasa los vóxeles de la de antes.
    let t = TerrainGen::new(1726);
    let mut total = 0;
    for s in 0..20u64 {
        let o = obra(Pared::Ladrillo, Techo::Azotea, true, true, ctx(1805, (s % 4) as u8));
        total += dibujar(&t, &est(&o, Uso::Vivienda, 2, s), s).len();
    }
    let f = Frame { origin: IVec2::new(54400, 64400), du: IVec2::X, dw: IVec2::Y, base: 40, width: 24, depth: 76, seed: 3 };
    let mut e = Edits::default();
    crate::structures::colonial::casa_azotea(&t, &mut e, &f);
    let antes = e.design.keys().filter(|p| p.y >= 40).count();
    let _ = &antes;
    eprintln!("casa de azotea con estilo: {} vóxeles de media; la del catálogo: {antes}", total / 20);
    assert!(total / 20 < antes * 12 / 10, "{} contra {antes}", total / 20);
    let _ = IVec3::ZERO;
}

/// [ciudad] Las casas de la ciudad piden sus detalles de fachada (rejas, faroles, macetas,
/// llamadores) como modelos de micro-vóxeles; fuera de la ciudad (sin detalles) la reja sigue
/// en vóxeles.
#[test]
fn las_fachadas_piden_rejas_y_faroles_como_detalles() {
    use crate::detalle::Id;
    let t = TerrainGen::new(1726);
    let c = glam::Vec2::new(13600.0, 16100.0);
    let mut ids = std::collections::BTreeMap::<Id, usize>::new();
    for k in 0..60u64 {
        let ang = k as f32 * 0.37;
        let f = glam::Vec2::new(ang.cos(), ang.sin());
        let (anio, ep) = if k % 2 == 0 { (1805, 0) } else { (1860, 1) };
        let mut cx = ctx(anio, (k % 4) as u8);
        cx.epoca = ep;
        let o = obra(Pared::Ladrillo, Techo::Azotea, true, true, cx);
        let nivel = 2 + (k % 2) as u8;
        let b = super::build_lot(&t, c, f, 1000 + k, ZoneKind::Residencial, Density::Media, nivel, Some(o));
        let d = b.edits.detalles.as_ref().expect("los solares piden detalles");
        let suelo = t.height(c.x, c.y);
        for p in d {
            *ids.entry(p.id).or_default() += 1;
            // Dentro del solar (12 × 38 m, con la vereda) y a la altura de la fachada.
            let q = glam::Vec2::new(p.pos.x, p.pos.z) - c;
            assert!(q.length() < 24.0, "{:?} fuera del solar: {q}", p.id);
            assert!(p.pos.y > suelo - 3.0 && p.pos.y < suelo + 16.0, "{:?} a {} m (suelo {suelo})", p.id, p.pos.y);
            // Los de la fachada miran a la calle (salvo las macetas, que giran al azar).
            if p.id != Id::Maceta {
                let mira = glam::Vec2::new(p.giro.sin(), p.giro.cos());
                assert!(mira.dot(f) > 0.99, "{:?} no mira a la calle", p.id);
                assert!(q.dot(f) > 14.0, "{:?} lejos de la fachada: {}", p.id, q.dot(f));
            }
        }
    }
    let rejas = ids.iter().filter(|(k, _)| matches!(k, Id::RejaPlana | Id::RejaPlanaAlta | Id::RejaVolada | Id::RejaVoladaAlta)).map(|x| x.1).sum::<usize>();
    assert!(rejas > 20, "rejas: {ids:?}");
    assert!(ids.contains_key(&Id::Llamador) && ids.contains_key(&Id::Maceta), "{ids:?}");
    assert!(ids.contains_key(&Id::FarolBrazo) || ids.contains_key(&Id::FarolGasBrazo), "{ids:?}");
    // Sin detalles (catálogo, sandbox): la reja sigue en vóxeles.
    let o = obra(Pared::Ladrillo, Techo::Azotea, true, true, ctx(1805, 2));
    let mut con_reja = 0;
    for seed in 0..12 {
        let v = dibujar(&t, &est(&o, Uso::Vivienda, 2, seed), seed);
        con_reja += usize::from(v.iter().any(|(_, m)| *m == REJA));
    }
    assert!(con_reja > 0);
}
