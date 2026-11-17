//! Pruebas de la población: calibración demográfica, integridad de las familias, herencia,
//! felicidad, políticas, determinismo, guardado y la decisión de no representar la esclavitud.

use super::simulacion::CiudadSintetica;
use super::*;
use crate::city::economy::Ordinance;

/// Imprime la calibración década por década (no es una prueba:
/// `cargo test --release calibracion -- --ignored --nocapture`).
#[test]
#[ignore]
fn calibracion() {
    let mut c = CiudadSintetica::nueva(7);
    let t0 = std::time::Instant::now();
    for _ in 0..100 * 12 {
        c.paso();
        if c.mes % 120 == 11 {
            let f = c.p.anual.last().unwrap();
            let s = &c.p.stats;
            println!(
                "{} hab {:6} TBN {:5.1} TBM {:5.1} TMI {:4.0} edad {:4.1} hogar {:3.1} h/m {:4.0} afro {:4.1}% indig {:3.1}% gini {:.2}/{:.2} top10 {:.2} pobreza {:.2} fel {:.2} desc {:.2} sincasa {} leen {:.2} {:?}",
                año_de(c.mes),
                s.habitantes,
                f.natalidad,
                f.mortalidad,
                if f.cuenta.nacimientos > 0 { f.cuenta.muertes_infantiles as f32 / f.cuenta.nacimientos as f32 * 1000.0 } else { 0.0 },
                s.edad_media,
                s.tam_hogar,
                s.hombres_por_100_mujeres,
                s.afrodescendientes as f32 / s.habitantes.max(1) as f32 * 100.0,
                s.indigenas as f32 / s.habitantes.max(1) as f32 * 100.0,
                s.gini_riqueza,
                s.gini_ingreso,
                s.top10,
                s.pobreza,
                s.felicidad,
                s.descontentos,
                s.sin_casa,
                s.lectura,
                s.motivos.iter().take(3).map(|m| (felicidad::motivo_texto(m.0), m.1)).collect::<Vec<_>>()
            );
        }
    }
    println!("{:?} · {} personas registradas", t0.elapsed(), c.p.personas.len());
}

fn ciudad(semilla: u64, años: i32) -> CiudadSintetica {
    let mut c = CiudadSintetica::nueva(semilla);
    for _ in 0..años * 12 {
        c.paso();
    }
    c
}

#[test]
fn esperanza_de_vida_de_la_tabla() {
    // Pollero: entre 33 y 41 años en el régimen pretransicional de Montevideo.
    let e = demografia::esperanza_de_vida();
    assert!((33.0..=41.0).contains(&e), "e0 = {e}");
}

#[test]
fn crece_desde_la_fundacion_a_ritmo_plausible() {
    let c = ciudad(11, 60);
    let s = &c.p.stats;
    assert!((1500..20000).contains(&s.habitantes), "habitantes a los 60 años: {}", s.habitantes);
    // Tasas de los últimos 30 años (por mil y por año).
    let años = &c.p.anual[c.p.anual.len() - 30..];
    let hab: f32 = años.iter().map(|f| f.habitantes as f32).sum();
    let n: f32 = años.iter().map(|f| f.cuenta.nacimientos as f32).sum();
    let m: f32 = años.iter().map(|f| f.cuenta.muertes as f32).sum();
    let mi: f32 = años.iter().map(|f| f.cuenta.muertes_infantiles as f32).sum();
    let (tbn, tbm, tmi) = (n / hab * 1000.0, m / hab * 1000.0, mi / n * 1000.0);
    assert!((28.0..=50.0).contains(&tbn), "natalidad {tbn}");
    assert!((18.0..=40.0).contains(&tbm), "mortalidad {tbm}");
    assert!((120.0..=300.0).contains(&tmi), "mortalidad infantil {tmi}");
    assert!((20.0..=33.0).contains(&s.edad_media), "edad media {}", s.edad_media);
    assert!((2.0..=7.0).contains(&s.tam_hogar), "tamaño del hogar {}", s.tam_hogar);
    assert!(s.hombres_por_100_mujeres > 100.0, "ciudad masculinizada: {}", s.hombres_por_100_mujeres);
    // Nadie vive más de 110 años ni tiene edad negativa.
    assert!(c.p.vivos().all(|p| (0..=110).contains(&p.edad(c.mes))));
    // Hay generaciones nacidas en la ciudad.
    assert!(c.p.vivos().filter(|p| p.origen == Origen::Montevideo).count() as u32 > s.habitantes / 3);
}

/// Ancestros de una persona: si hubiera un ciclo, la búsqueda no terminaría.
fn ancestros(p: &Poblacion, id: PersonaId) -> usize {
    let mut pila = vec![id];
    let mut vistos = 0;
    while let Some(x) = pila.pop() {
        let q = &p.personas[x as usize];
        for a in [q.padre, q.madre].into_iter().flatten() {
            assert_ne!(a, id, "ciclo en el linaje de {id}");
            vistos += 1;
            assert!(vistos <= p.personas.len(), "ciclo en el linaje de {id}");
            pila.push(a);
        }
    }
    vistos
}

#[test]
fn familias_integras_y_linajes() {
    let c = ciudad(12, 70);
    let p = &c.p;
    let mut con_abuelos = 0;
    for x in &p.personas {
        for (padre, min, max) in [(x.padre, 14, 80), (x.madre, 13, 52)] {
            if let Some(a) = padre {
                let pa = &p.personas[a as usize];
                let edad = (x.nacimiento - pa.nacimiento) / 12;
                assert!((min..=max).contains(&edad), "{} tuvo un hijo a los {edad} años", p.nombre_completo(a));
                assert!(pa.hijos.contains(&x.id), "el hijo figura en la lista del padre o la madre");
                assert!(pa.muerte.is_none_or(|m| m >= x.nacimiento - 10), "nadie nace mucho después de morir su padre");
            }
        }
        if let (Some(pa), Some(ma)) = (x.padre, x.madre) {
            // El primer apellido es el del padre (o el de la madre si el padre no tenía).
            let (ap, am) = (p.personas[pa as usize].apellidos[0], p.personas[ma as usize].apellidos[0]);
            let propio = [ap, am, NADIE].contains(&x.apellidos[0]) || x.apellidos[1] == NADIE;
            assert!(propio, "{} lleva el apellido de su familia", p.nombre_completo(x.id));
            if ap != NADIE && am != NADIE {
                assert!(x.apellidos[0] == ap || x.apellidos[0] == am, "{} lleva el apellido de su familia", p.nombre_completo(x.id));
            }
        }
        if ancestros(p, x.id) >= 4 {
            con_abuelos += 1;
        }
        if let Some(cy) = x.conyuge {
            assert_ne!(cy, x.id);
        }
    }
    assert!(con_abuelos > 100, "linajes de tres generaciones: {con_abuelos}");
    // Cada persona viva está en el hogar que dice y cada hogar activo tiene jefe vivo dentro.
    for v in p.vivos() {
        let h = &p.hogares[v.hogar.unwrap() as usize];
        assert!(h.activo && h.miembros.contains(&v.id), "{} está en su hogar: activo {} miembro {} {:?} jefe {} {:?}", p.nombre_completo(v.id), h.activo, h.miembros.contains(&v.id), h.alojamiento, h.jefe, v.historial);
    }
    for h in p.hogares.iter().filter(|h| h.activo && !h.miembros.is_empty()) {
        assert!(h.miembros.contains(&h.jefe), "el jefe vive en su hogar {}", h.id);
        assert!(p.personas[h.jefe as usize].viva());
        for &m in &h.miembros {
            assert!(p.personas[m as usize].viva(), "en el hogar {} solo viven personas vivas", h.id);
        }
    }
}

#[test]
fn la_herencia_conserva_la_riqueza() {
    let mut c = ciudad(13, 40);
    let jefes: Vec<PersonaId> = c.p.hogares.iter().filter(|h| h.activo && !Poblacion::colectivo(h) && !h.miembros.is_empty()).map(|h| h.jefe).take(150).collect();
    let mut con_herederos_afuera = 0;
    for j in jefes {
        if !c.p.personas[j as usize].viva() {
            continue;
        }
        if c.p.herederos(j).iter().any(|&x| c.p.personas[x as usize].hogar != c.p.personas[j as usize].hogar) {
            con_herederos_afuera += 1;
        }
        let antes = c.p.riqueza_total();
        let mut ruido = Vec::new();
        c.p.morir(j, Causa::Natural, &mut ruido);
        let despues = c.p.riqueza_total();
        assert!((antes - despues).abs() < 0.05 + antes.abs() * 1e-6, "herencia de {j}: {antes} → {despues}");
    }
    assert!(con_herederos_afuera > 5, "se probaron herencias con herederos en otras casas: {con_herederos_afuera}");
    // Y la ciudad sigue andando.
    for _ in 0..24 {
        c.paso();
    }
}

/// Palabras de la esclavitud que no pueden aparecer en ningún texto que el juego muestre:
/// comienzos de palabra, palabras enteras (`=`) y frases.
const PALABRAS_PROHIBIDAS: &[&str] = &["esclav", "libert", "manumis", "coartaci", "patronato", "aboli", "vientre", "salsipuedes", "caserío", "peculio", "sometid", "cofradía", "colono", "prisioner", "=trata", "=amo", "=amos", "=ama", "=leva", "de nación", "carta de"];

fn sin_palabras_prohibidas(donde: &str, t: &str) {
    let t = t.to_lowercase();
    let palabras: Vec<&str> = t.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    for w in PALABRAS_PROHIBIDAS {
        let hay = match w.strip_prefix('=') {
            Some(entera) => palabras.iter().any(|p| *p == entera),
            None if w.contains(' ') => t.contains(w),
            None => palabras.iter().any(|p| p.starts_with(w)),
        };
        assert!(!hay, "{donde}: «{t}» contiene «{w}»");
    }
}

#[test]
fn nadie_vive_sin_libertad() {
    // El juego no representa la esclavitud: no hay condición legal, nadie vive sujeto a otro
    // hogar y el modelo no tiene campos ni hechos de esa clase.
    let c = ciudad(14, 70);
    let p = &c.p;
    let json = serde_json::to_string(p).unwrap();
    for campo in ["\"condicion\"", "\"patronato\"", "\"leyes\"", "Esclavizada", "Liberta", "LlegoForzada", "LlegoColono", "LlegoPrisionera", "Libertad", "fondo_libertad", "llegados_forzados", "libertades"] {
        assert!(!json.contains(campo), "la partida guardada no tiene `{campo}`");
    }
    // Todas las personas viven en su propio hogar o en el de su familia: nadie vive en un hogar
    // ajeno sin parentesco salvo el cuartel y el campamento (colectivos), y quien llega lo hace
    // con su familia o solo.
    for x in p.vivos() {
        for e in &x.historial {
            if let TipoEvento::Llego(_) = e.tipo {
                let h = &p.hogares[x.hogar.unwrap() as usize];
                let pariente = |a: &Persona, b: &Persona| a.id == b.id || a.conyuge == Some(b.id) || b.conyuge == Some(a.id) || a.padre == Some(b.id) || a.madre == Some(b.id) || b.padre == Some(a.id) || b.madre == Some(a.id) || (a.padre.is_some() && a.padre == b.padre) || (a.madre.is_some() && a.madre == b.madre);
                let jefe = &p.personas[h.jefe as usize];
                assert!(Poblacion::colectivo(h) || e.mes < c.mes - 12 || h.jefe == x.id || pariente(x, jefe) || p.personas.iter().any(|y| pariente(x, y) && y.id != x.id && y.hogar == x.hogar), "{} llegó y vive con su familia o sola", p.nombre_completo(x.id));
            }
        }
    }
    // Hay personas de origen africano e indígena, libres, con nombre, apellido y oficio.
    let africanas: Vec<&Persona> = p.personas.iter().filter(|x| x.origen.raiz() == Raiz::Africana).collect();
    assert!(!africanas.is_empty(), "llegan personas de origen africano");
    assert!(africanas.iter().all(|x| x.apellidos[0] != NADIE), "con apellido");
    assert!(p.stats.afrodescendientes > 0 && p.stats.indigenas > 0);
    // Ningún texto de la crónica, de las historias de vida ni de los nombres habla de esclavitud.
    for (_, t) in &p.cronica {
        sin_palabras_prohibidas("crónica", t);
    }
    for x in p.personas.iter().take(4000) {
        sin_palabras_prohibidas("nombre", &p.nombre_completo(x.id));
        for e in &x.historial {
            sin_palabras_prohibidas("historia de vida", &p.texto_evento(x, e));
        }
    }
}

#[test]
fn los_textos_no_hablan_de_esclavitud() {
    // Las tablas de textos que muestra el juego: ordenanzas, políticas, motivos y factores de
    // la felicidad, épocas, orígenes, oficios y causas de muerte.
    for o in crate::city::economy::ORDINANCES {
        sin_palabras_prohibidas(o.name, &format!("{} {}", o.name, o.desc));
    }
    for (o, t) in politicas::SOCIALES {
        sin_palabras_prohibidas(&format!("{o:?}"), t);
    }
    for f in politicas::FUTURAS {
        sin_palabras_prohibidas(f.nombre, &format!("{} {}", f.nombre, f.efecto));
    }
    for k in 0..=255u8 {
        sin_palabras_prohibidas("motivo", felicidad::motivo_texto(k));
    }
    for f in felicidad::FACTORES {
        sin_palabras_prohibidas("factor", f);
    }
    for e in Epoca::ALL {
        sin_palabras_prohibidas("época", e.nombre());
    }
    for o in Origen::ALL {
        sin_palabras_prohibidas("origen", &format!("{} {}", o.nombre(), o.corto()));
    }
    for s in [Sexo::Hombre, Sexo::Mujer] {
        for o in [Oficio::Ninguno, Oficio::Peon, Oficio::Labrador, Oficio::Hacendado, Oficio::Artesano, Oficio::Pulpero, Oficio::Comerciante, Oficio::Marinero, Oficio::Soldado, Oficio::Servicio, Oficio::Clerigo, Oficio::Funcionario, Oficio::Maestro, Oficio::Obrero, Oficio::Empleado, Oficio::Profesional] {
            sin_palabras_prohibidas("oficio", o.nombre(s));
        }
    }
    for c in [Causa::Natural, Causa::Parto, Causa::Epidemia, Causa::Incendio, Causa::Inundacion, Causa::Hambre] {
        sin_palabras_prohibidas("causa", c.texto());
    }
    // Y las cadenas literales de los fuentes de la población y de sus paneles (como un grep).
    let fuentes = [
        ("mod.rs", include_str!("mod.rs")),
        ("modelo.rs", include_str!("modelo.rs")),
        ("demografia.rs", include_str!("demografia.rs")),
        ("economia.rs", include_str!("economia.rs")),
        ("felicidad.rs", include_str!("felicidad.rs")),
        ("estadisticas.rs", include_str!("estadisticas.rs")),
        ("textos.rs", include_str!("textos.rs")),
        ("nombres.rs", include_str!("nombres.rs")),
        ("origenes.rs", include_str!("origenes.rs")),
        ("politicas.rs", include_str!("politicas.rs")),
        ("ui/poblacion.rs", include_str!("../../ui/poblacion.rs")),
        ("economy.rs", include_str!("../economy.rs")),
    ];
    for (nombre, f) in fuentes {
        for linea in f.lines().filter(|l| !l.trim_start().starts_with("//")) {
            // Solo lo que está entre comillas.
            for (i, trozo) in linea.split('"').enumerate() {
                if i % 2 == 1 {
                    sin_palabras_prohibidas(nombre, trozo);
                }
            }
        }
    }
}

#[test]
fn una_partida_vieja_carga_sin_esclavitud() {
    // Una partida guardada por una versión anterior trae campos y hechos que el juego ya no
    // tiene: se ignoran, y las llegadas de otro tipo se leen como llegadas.
    let c = ciudad(17, 12);
    let mut v: serde_json::Value = serde_json::to_value(&c.p).unwrap();
    let personas = v["personas"].as_array_mut().unwrap();
    let n = personas.len();
    for (i, x) in personas.iter_mut().enumerate() {
        x["condicion"] = serde_json::json!(if i % 3 == 0 { "Esclavizada" } else if i % 3 == 1 { "Liberta" } else { "Libre" });
        x["patronato"] = serde_json::json!(if i % 5 == 0 { serde_json::Value::from(400) } else { serde_json::Value::Null });
        let h = x["historial"].as_array_mut().unwrap();
        h.push(serde_json::json!({"mes": 30, "tipo": {"LlegoForzada": "AfricaCentral"}, "con": u32::MAX}));
        h.push(serde_json::json!({"mes": 31, "tipo": {"Libertad": "Ahorro"}, "con": u32::MAX}));
        h.push(serde_json::json!({"mes": 32, "tipo": "Patronato", "con": u32::MAX}));
        h.push(serde_json::json!({"mes": 33, "tipo": "TomoApellido", "con": u32::MAX}));
    }
    v["leyes"] = serde_json::json!({"vientres": 1248, "abolicion": null});
    v["fondo_libertad"] = serde_json::json!(12.5);
    v["hitos"] = serde_json::json!(7);
    v["mes_actual"]["llegados_forzados"] = serde_json::json!(3);
    v["mes_actual"]["libertades"] = serde_json::json!(1);
    let mut p: Poblacion = serde_json::from_value(v).unwrap();
    p.reconstruir();
    assert_eq!(p.personas.len(), n);
    for (x, y) in p.personas.iter().zip(&c.p.personas) {
        // Los hechos desconocidos se descartan; la llegada vieja queda como `Llego`.
        assert_eq!(x.historial.len(), y.historial.len() + 1);
        assert_eq!(x.historial.last().unwrap().tipo, TipoEvento::Llego(Origen::AfricaCentral));
    }
    // Y la partida sigue andando.
    let mut b = CiudadSintetica { p, lotes: c.lotes.clone(), mes: c.mes, ordenanzas: c.ordenanzas.clone(), inmigracion: c.inmigracion, precio_pan: c.precio_pan, epidemia_cada: c.epidemia_cada, servicios: c.servicios };
    for _ in 0..12 {
        b.paso();
    }
}

#[test]
fn determinismo() {
    let a = ciudad(21, 25);
    let b = ciudad(21, 25);
    assert_eq!(serde_json::to_string(&a.p).unwrap(), serde_json::to_string(&b.p).unwrap());
    let c = ciudad(22, 25);
    assert_ne!(serde_json::to_string(&a.p).unwrap(), serde_json::to_string(&c.p).unwrap());
}

#[test]
fn guardar_y_cargar_da_la_misma_poblacion() {
    let mut a = ciudad(23, 30);
    let json = serde_json::to_string(&a.p).unwrap();
    let mut p: Poblacion = serde_json::from_str(&json).unwrap();
    p.reconstruir();
    assert!(p == a.p, "IGUAL al cargar");
    assert_eq!(p.estadisticas(), a.p.stats);
    let mut b = CiudadSintetica { p, lotes: a.lotes.clone(), mes: a.mes, ordenanzas: a.ordenanzas.clone(), inmigracion: a.inmigracion, precio_pan: a.precio_pan, epidemia_cada: a.epidemia_cada, servicios: a.servicios };
    for _ in 0..24 {
        a.paso();
        b.paso();
    }
    assert_eq!(serde_json::to_string(&a.p).unwrap(), serde_json::to_string(&b.p).unwrap(), "y sigue IGUAL dos años después");
    assert_eq!(a.p.stats, b.p.stats);
}

#[test]
fn felicidad_hambre_y_hacinamiento() {
    let mut c = ciudad(24, 30);
    let ent = c.entorno(0);
    // Un hogar con casa y comida.
    let h = c.p.hogares.iter().find(|h| h.activo && h.vive_en_solar() && h.miembros.len() >= 2 && h.clase >= Clase::Modesta && h.meses_de_hambre == 0).unwrap().id;
    let j = c.p.hogares[h as usize].jefe;
    let base = c.p.desglose(j, &ent);
    // Sin comida.
    let guardado = c.p.hogares[h as usize].clone();
    {
        let hg = &mut c.p.hogares[h as usize];
        hg.ingresos = 0.0;
        hg.ahorro = 0.0;
        hg.meses_de_hambre = 3;
    }
    let hambre = c.p.desglose(j, &ent);
    assert!(hambre.total < base.total - 0.05, "el hambre baja la felicidad: {} → {}", base.total, hambre.total);
    assert_eq!(hambre.motivos[0] / 4, 0, "el motivo principal es la comida: {:?}", hambre.motivos);
    c.p.hogares[h as usize] = guardado;
    // Hacinado: se meten otras familias en la misma casa hasta pasar del doble de su lugar.
    let l = c.p.hogares[h as usize].lote.unwrap();
    let cap = c.lotes[l as usize].capacidad;
    let otros: Vec<HogarId> = c.p.hogares.iter().filter(|x| x.activo && x.id != h && x.vive_en_solar() && x.lote != Some(l) && !x.miembros.is_empty()).map(|x| x.id).collect();
    for o in otros {
        if c.p.hacinamiento(l, cap) > 2.0 {
            break;
        }
        c.p.hogares[o as usize].lote = Some(l);
        c.p.reindexar();
    }
    let hacinado = c.p.desglose(j, &ent);
    assert!(hacinado.valores[1] < base.valores[1] && hacinado.total < base.total, "vivir hacinado baja la felicidad: {} → {}", base.total, hacinado.total);
    assert!(hacinado.motivos.contains(&5), "motivo: vive hacinado ({:?})", hacinado.motivos);
    assert_eq!(base.valores.len(), felicidad::FACTORES.len());
}

#[test]
fn politicas_contra_la_desigualdad() {
    let correr = |ords: Vec<Ordinance>| {
        let mut c = CiudadSintetica::nueva(31);
        c.precio_pan = 2.4;
        c.ordenanzas = ords;
        let mut hambre = 0;
        let mut gasto = 0i64;
        for _ in 0..30 * 12 {
            let inf = c.paso();
            let activos = c.p.hogares.iter().filter(|h| h.activo && !h.miembros.is_empty()).count().max(1);
            hambre += c.p.hogares.iter().filter(|h| h.activo && h.meses_de_hambre > 0).count() * 1000 / activos;
            gasto += inf.lineas.iter().filter(|l| l.1 < 0).map(|l| -l.1).sum::<i64>();
        }
        (c, hambre, gasto)
    };
    let (sin, hambre_sin, gasto_sin) = correr(vec![]);
    let (con, hambre_con, gasto_con) = correr(vec![Ordinance::PanDeLosPobres, Ordinance::EscuelasPrimerasLetras, Ordinance::HospitalCaridad]);
    assert!(hambre_con < hambre_sin, "pan barato: {hambre_con} meses de hambre contra {hambre_sin}");
    assert!(gasto_con > gasto_sin, "y le cuesta a la ciudad");
    assert!(con.p.stats.lectura > sin.p.stats.lectura + 0.05, "escuelas: {} contra {}", con.p.stats.lectura, sin.p.stats.lectura);
    assert!(con.p.stats.felicidad > sin.p.stats.felicidad, "se vive mejor: {} contra {}", con.p.stats.felicidad, sin.p.stats.felicidad);
}

#[test]
fn api_para_agentes_y_servicios() {
    let mut c = ciudad(41, 20);
    let lote = c.lotes.iter().find(|l| c.p.hogares_de_lote(l.id).next().is_some()).unwrap().id;
    for p in c.p.habitantes_de_lote(lote) {
        assert_eq!(c.p.casa_de(p.id), Some(lote));
    }
    let (mut con_trabajo, mut total) = (0, 0);
    for p in c.p.vivos() {
        total += 1;
        if let Some(l) = c.p.lugar_de_trabajo(p.id) {
            con_trabajo += 1;
            assert!(c.p.trabajadores_de_lote(l).any(|q| q.id == p.id));
        }
    }
    assert!(con_trabajo > total / 20, "hay gente trabajando en comercios y talleres: {con_trabajo}/{total}");
    // Choques de mortalidad: una epidemia fuerte mata; un incendio deja sin casa.
    let antes = c.p.habitantes();
    let muertos = c.p.choque_epidemia(0.2, None);
    assert!(muertos > 0 && c.p.habitantes() == antes - muertos);
    let h = c.p.hogares_de_lote(lote).next().unwrap().id;
    c.p.choque_incendio(lote);
    assert_eq!(c.p.hogares[h as usize].alojamiento, Alojamiento::SinCasa);
    // Perder la casa en un incendio pesa en la seguridad.
    if let Some(&m) = c.p.hogares[h as usize].miembros.first() {
        let d = c.p.desglose(m, &c.entorno(0));
        assert!(d.valores[9] <= 0.3, "seguridad {}", d.valores[9]);
    }
    assert!(c.p.choque_hambruna(1.0) < c.p.habitantes());
    c.paso();
}

#[test]
fn inmigracion_por_epocas_y_poblacion_agregada() {
    let mut c = ciudad(51, 5);
    c.p.fiel_historia = false;
    c.p.epoca = Epoca::Inmigracion;
    c.inmigracion = 0.3;
    let desde = c.mes;
    for _ in 0..24 {
        c.paso();
    }
    let italianos = c.p.vivos().filter(|p| p.origen == Origen::Italia).count();
    assert!(italianos > 20, "en la época de la inmigración llegan italianos: {italianos}");
    assert!(c.p.personas.iter().filter(|p| p.historial.iter().any(|e| matches!(e.tipo, TipoEvento::Llego(_)) && e.mes > desde)).all(|p| p.origen.raiz() != Raiz::Africana || p.origen == Origen::Montevideo), "las llegadas siguen la tabla de la época");
    // Con un límite de individuos chico, el resto pasa a la población agregada.
    c.p.limite_individuos = 400;
    for _ in 0..24 {
        c.paso();
    }
    let total: f32 = c.p.cohortes.iter().map(|x| x[0] + x[1]).sum();
    assert!(total > 0.0, "hay población agregada");
    assert!(c.p.habitantes() as f32 >= c.p.idx.vivos.len() as f32 + total - 1.0);
}

/// Banco de pruebas: 100 años desde la fundación, y un año con 100.000 personas
/// (`cargo test --release banco_poblacion -- --ignored --nocapture`).
#[test]
#[ignore]
fn banco_poblacion() {
    let t0 = std::time::Instant::now();
    let c = ciudad(1726, 100);
    let dt = t0.elapsed();
    let bytes_personas: usize = c.p.personas.iter().map(|p| std::mem::size_of::<Persona>() + p.hijos.capacity() * 4 + p.historial.capacity() * std::mem::size_of::<Evento>()).sum();
    let bytes_hogares: usize = c.p.hogares.iter().map(|h| std::mem::size_of::<Hogar>() + (h.miembros.capacity() + h.solares.capacity()) * 4).sum();
    let json = serde_json::to_string(&c.p).unwrap();
    println!(
        "100 años: {dt:.2?} ({:.2} ms por mes) · {} habitantes en {} · {} personas registradas (vivas y muertas), {} hogares · memoria ≈ {:.1} MB · partida {:.1} MB · RSS {:.0} MB",
        dt.as_secs_f64() * 1000.0 / 1200.0,
        c.p.habitantes(),
        año_de(c.mes),
        c.p.personas.len(),
        c.p.hogares.len(),
        (bytes_personas + bytes_hogares) as f64 / 1e6,
        json.len() as f64 / 1e6,
        crate::session::memory_mb().0
    );
    // Escala de épocas tardías: 100.000 personas, un año.
    let mut g = ciudad(99, 1);
    g.p.fiel_historia = false;
    g.p.epoca = Epoca::Inmigracion;
    while g.p.habitantes() < 100_000 {
        let mut rng = Rng::new(g.p.habitantes() as u64);
        let ll = g.p.elegir_llegada(&mut rng);
        let tam = g.p.tam_familia(&mut rng, ll);
        let mes = g.mes;
        g.p.llegar_familia(&mut rng, ll, mes, tam);
    }
    g.p.reindexar();
    g.inmigracion = 0.0;
    for _ in 0..3 {
        g.paso();
    }
    let t1 = std::time::Instant::now();
    for _ in 0..12 {
        g.paso();
    }

    println!("100.000 personas: {:.1} ms por mes · {} habitantes", t1.elapsed().as_secs_f64() * 1000.0 / 12.0, g.p.habitantes());
}

