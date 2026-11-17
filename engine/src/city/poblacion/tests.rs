//! Pruebas de la población: calibración demográfica, integridad de las familias, herencia,
//! libertad, felicidad, políticas, determinismo y guardado.

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
                "{} hab {:6} TBN {:5.1} TBM {:5.1} TMI {:4.0} edad {:4.1} hogar {:3.1} h/m {:4.0} escl {:4.1}% gini {:.2}/{:.2} top10 {:.2} pobreza {:.2} fel {:.2} desc {:.2} sincasa {} leen {:.2} {:?}",
                año_de(c.mes),
                s.habitantes,
                f.natalidad,
                f.mortalidad,
                if f.cuenta.nacimientos > 0 { f.cuenta.muertes_infantiles as f32 / f.cuenta.nacimientos as f32 * 1000.0 } else { 0.0 },
                s.edad_media,
                s.tam_hogar,
                s.hombres_por_100_mujeres,
                s.por_condicion[1] as f32 / s.habitantes.max(1) as f32 * 100.0,
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
        let eligio = x.historial.iter().any(|e| e.tipo == TipoEvento::TomoApellido);
        if let (Some(pa), Some(ma), false) = (x.padre, x.madre, eligio) {
            // El primer apellido es el del padre (o el de la madre si el padre no tenía al
            // nacer el hijo: las personas llegadas por la trata lo toman con la libertad).
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
        assert!(h.activo && h.miembros.contains(&v.id), "{} está en su hogar: activo {} miembro {} {:?} jefe {} {:?} {:?}", p.nombre_completo(v.id), h.activo, h.miembros.contains(&v.id), h.alojamiento, h.jefe, v.historial, v.condicion);
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

#[test]
fn caminos_a_la_libertad() {
    let mut c = ciudad(14, 50);
    let esclavizadas = |c: &CiudadSintetica| c.p.vivos().filter(|p| p.condicion == Condicion::Esclavizada).count();
    assert!(esclavizadas(&c) > 20, "la época colonial tiene personas esclavizadas: {}", esclavizadas(&c));
    // Por ahorro: alguien que juntó la suma de su carta.
    let id = c.p.vivos().find(|p| p.condicion == Condicion::Esclavizada && p.edad(c.mes) >= 20).unwrap().id;
    c.p.personas[id as usize].riqueza = economia::CARTA_DE_LIBERTAD + 5.0;
    let mut ruido = Vec::new();
    let ent = c.entorno(0);
    let mut rng = Rng::new(1);
    c.p.manumisiones(&mut rng, &ent, &mut ruido);
    let p = &c.p.personas[id as usize];
    assert_eq!(p.condicion, Condicion::Liberta);
    assert!(p.historial.iter().any(|e| e.tipo == TipoEvento::Libertad(ViaLibertad::Ahorro)));
    assert_ne!(p.apellidos, [NADIE, NADIE], "toma un apellido");
    assert!(c.p.hogares[p.hogar.unwrap() as usize].miembros.iter().all(|&m| c.p.personas[m as usize].libre()), "vive en un hogar libre");
    // Con el fondo del cabildo hay libertades por el fondo.
    let sin = ciudad(15, 60);
    let mut con = ciudad(15, 40);
    con.ordenanzas.push(Ordinance::FondoManumision);
    for _ in 0..240 {
        con.paso();
    }
    let libres_por = |c: &CiudadSintetica, v: ViaLibertad| c.p.personas.iter().filter(|p| p.historial.iter().any(|e| e.tipo == TipoEvento::Libertad(v))).count();
    assert!(libres_por(&con, ViaLibertad::Fondo) > 0, "el fondo libera");
    assert!(libres_por(&sin, ViaLibertad::Testamento) > 0, "libertades por testamento");
    // Libertad de vientres: después de la ley, nadie nace esclavizado.
    con.ordenanzas.push(Ordinance::LibertadVientres);
    let ley = con.mes + 1;
    for _ in 0..60 {
        con.paso();
    }
    assert!(con.p.personas.iter().filter(|p| p.nacimiento > ley).all(|p| p.condicion != Condicion::Esclavizada));
    assert!(con.p.personas.iter().any(|p| p.historial.iter().any(|e| e.tipo == TipoEvento::Libertad(ViaLibertad::Vientres))), "hay nacidos libres por la ley");
    assert!(con.p.personas.iter().all(|p| !p.historial.iter().any(|e| matches!(e.tipo, TipoEvento::LlegoForzada(_)) && e.mes > ley)), "se prohibió el tráfico");
    // Abolición: nadie queda esclavizado y no hay vuelta atrás.
    con.ordenanzas.push(Ordinance::Abolicion);
    con.paso();
    assert_eq!(esclavizadas(&con), 0);
    con.ordenanzas.clear();
    for _ in 0..24 {
        con.paso();
    }
    assert_eq!(esclavizadas(&con), 0, "derogar la ordenanza no vuelve atrás");
    let mut e = crate::city::economy::Economy::default();
    let mut plata = 1000;
    e.toggle(Ordinance::Abolicion, &mut plata).unwrap();
    assert!(e.toggle(Ordinance::Abolicion, &mut plata).is_err(), "la abolición no se deroga");
}

#[test]
fn en_la_historia_fiel_las_leyes_llegan_en_sus_fechas() {
    let mut p = Poblacion::nueva(3);
    p.fundar(2);
    let mut ent = Entorno { precio_pan: 1.0, precio_carne: 1.0, agua: 0.5, ..Default::default() };
    ent.mes = (1824 - 1726) * 12;
    p.mes(&ent);
    assert!(p.leyes.vientres.is_none());
    ent.mes = (1825 - 1726) * 12;
    p.mes(&ent);
    assert!(p.leyes.vientres.is_some() && p.leyes.abolicion.is_none());
    ent.mes = (1842 - 1726) * 12;
    p.mes(&ent);
    assert!(p.leyes.abolicion.is_some());
    assert!(p.vivos().all(|x| x.condicion != Condicion::Esclavizada));
}

#[test]
fn no_hay_forma_de_comerciar_personas() {
    // Ninguna función ni campo pone precio a una persona ni la compra, vende o transfiere.
    let fuentes = [include_str!("mod.rs"), include_str!("modelo.rs"), include_str!("economia.rs"), include_str!("demografia.rs"), include_str!("felicidad.rs"), include_str!("politicas.rs")];
    for f in fuentes {
        for prohibido in ["fn comprar_persona", "fn vender", "fn subastar", "fn precio_de_persona", "fn transferir_persona", "fn comerciar"] {
            assert!(!f.contains(prohibido), "no puede existir `{prohibido}`");
        }
    }
    let modelo = include_str!("modelo.rs");
    let persona = &modelo[modelo.find("pub struct Persona {").unwrap()..];
    let persona = &persona[..persona.find("\n}").unwrap()];
    for campo in ["precio", "valor", "dueno", "amo", "propietario"] {
        assert!(!persona.contains(&format!("pub {campo}")), "Persona no tiene `{campo}`");
    }
    // Las llegadas forzadas no mueven dinero: nadie paga ni cobra.
    let mut c = ciudad(16, 30);
    let antes = c.p.riqueza_total();
    let mut rng = Rng::new(5);
    let mes = c.mes;
    let n = c.p.llegada_forzada(&mut rng, Origen::AfricaCentral, mes, 10);
    assert_eq!(n, 10);
    assert!((c.p.riqueza_total() - antes).abs() < 1e-6);
    // Y entre las ordenanzas no hay nada para comprar o vender personas.
    for o in crate::city::economy::ORDINANCES {
        let t = format!("{} {}", o.name, o.desc).to_lowercase();
        assert!(!t.contains("comprar esclav") && !t.contains("vender esclav") && !t.contains("mercado de"), "{}", o.name);
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
    let mut b = CiudadSintetica { p, lotes: a.lotes.clone(), mes: a.mes, ordenanzas: a.ordenanzas.clone(), inmigracion: a.inmigracion, precio_pan: a.precio_pan, epidemia_cada: a.epidemia_cada };
    for _ in 0..24 {
        a.paso();
        b.paso();
    }
    assert_eq!(serde_json::to_string(&a.p).unwrap(), serde_json::to_string(&b.p).unwrap(), "y sigue IGUAL dos años después");
    assert_eq!(a.p.stats, b.p.stats);
}

#[test]
fn felicidad_hambre_hacinamiento_y_libertad() {
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
    // Hacinado: se meten otras tres familias en la misma casa.
    let l = c.p.hogares[h as usize].lote.unwrap();
    let otros: Vec<HogarId> = c.p.hogares.iter().filter(|x| x.activo && x.id != h && x.vive_en_solar() && x.lote != Some(l) && !x.miembros.is_empty()).map(|x| x.id).take(3).collect();
    for o in otros {
        c.p.hogares[o as usize].lote = Some(l);
    }
    c.p.reindexar();
    let hacinado = c.p.desglose(j, &ent);
    assert!(hacinado.valores[1] < base.valores[1] && hacinado.total < base.total, "vivir hacinado baja la felicidad: {} → {}", base.total, hacinado.total);
    assert!(hacinado.motivos.contains(&5), "motivo: vive hacinado ({:?})", hacinado.motivos);
    // La libertad la sube.
    let e = c.p.vivos().find(|p| p.condicion == Condicion::Esclavizada && p.edad(c.mes) > 18).unwrap().id;
    let antes = c.p.desglose(e, &ent);
    assert!(antes.motivos.contains(&24), "vive sin libertad ({:?})", antes.motivos);
    let mut ruido = Vec::new();
    c.p.liberar(e, ViaLibertad::Testamento, &mut ruido);
    let despues = c.p.desglose(e, &ent);
    assert!(despues.total > antes.total + 0.08, "la libertad sube la felicidad: {} → {}", antes.total, despues.total);
    assert!(despues.valores[6] > antes.valores[6] + 0.5);
    // Por grupo: las personas esclavizadas son mucho menos felices.
    let s = &c.p.stats;
    assert!(s.fel_condicion[1] < s.fel_condicion[0] - 0.1, "{:?}", s.fel_condicion);
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
        assert!(d.valores[10] <= 0.3, "seguridad {}", d.valores[10]);
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
    assert!(c.p.personas.iter().all(|p| !p.historial.iter().any(|e| matches!(e.tipo, TipoEvento::LlegoForzada(_)) && e.mes > desde)), "sin trata en las épocas tardías");
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
        let ll = g.p.elegir_llegada(&mut rng, false);
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
