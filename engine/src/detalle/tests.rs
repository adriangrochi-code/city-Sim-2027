use super::ropa::{atuendo, vestir, Atuendo, Quien, REGLAS};
use super::*;
use crate::city::poblacion::modelo::{Oficio, Raiz};

fn todos() -> Vec<&'static Modelo> {
    let b = biblioteca();
    b.objetos.iter().chain(b.personas.iter()).collect()
}

#[test]
fn los_modelos_son_validos() {
    let b = biblioteca();
    assert_eq!(b.objetos.len(), Id::TODOS.len());
    assert_eq!(b.personas.len(), Atuendo::TODOS.len() * 2);
    for (i, id) in Id::TODOS.iter().enumerate() {
        assert_eq!(*id as usize, i, "Id::TODOS en el orden del enum");
    }
    for m in todos() {
        assert!(!m.cajas.is_empty(), "{} sin cajas", m.nombre);
        assert!(m.cajas.len() < 400, "{}: {} cajas", m.nombre, m.cajas.len());
        for c in &m.cajas {
            assert!((c.ranura as usize) < r::N, "{}: ranura {}", m.nombre, c.ranura);
            for e in 0..3 {
                assert!(c.a[e] < c.b[e], "{}: caja vacía {:?}", m.nombre, c);
                assert!(c.a[e] >= m.min[e] && c.b[e] <= m.max[e]);
            }
            // Nada más grande que una carreta (8 m) ni que un farol de gas (4 m de alto).
            assert!((m.max[2] - m.min[2]) as f32 * VOX < 8.0 && (m.max[1] - m.min[1]) as f32 * VOX < 4.0, "{}", m.nombre);
        }
        // Las partes que se mueven tienen su pivote dentro (o cerca) del modelo.
        for p in Parte::TODAS.iter().skip(1) {
            if m.cajas.iter().any(|c| c.parte == *p) {
                let v = m.pivotes[*p as usize];
                for e in 0..3 {
                    assert!(v[e] >= m.min[e] as f32 - 2.0 && v[e] <= m.max[e] as f32 + 2.0, "{}: pivote de {:?} fuera: {:?}", m.nombre, p, v);
                }
            }
        }
    }
}

#[test]
fn las_cajas_de_la_grilla_no_se_pisan_y_cubren_lo_pintado() {
    let mut l = Lienzo::new([-4, 0, -4], [4, 8, 4]);
    l.caja(Parte::Fijo, r::ROPA, [-4, 0, -4], [4, 4, 4]);
    l.caja(Parte::Cabeza, r::PIEL, [-2, 3, -2], [2, 8, 2]);
    l.borrar([-4, 0, -4], [-3, 1, -3]);
    let m = l.armar("prueba");
    let vol: i32 = m.cajas.iter().map(|c| (0..3).map(|e| (c.b[e] - c.a[e]) as i32).product::<i32>()).sum();
    assert_eq!(vol, 8 * 3 * 8 + 4 * 5 * 4 + (8 * 8 - 4 * 4) - 1);
    for (i, a) in m.cajas.iter().enumerate() {
        for b in &m.cajas[i + 1..] {
            let solapa = (0..3).all(|e| a.a[e] < b.b[e] && b.a[e] < a.b[e]);
            assert!(!solapa, "{a:?} y {b:?} se pisan");
        }
    }
    // Una persona: pocas cajas para muchos vóxeles.
    let p = biblioteca().persona(Atuendo::Paisano, false);
    assert!(p.cajas.len() < 60, "persona con {} cajas", p.cajas.len());
}

#[test]
fn las_personas_tienen_su_altura() {
    let b = biblioteca();
    for a in Atuendo::TODOS {
        let m = b.persona(a, false);
        let alto = m.cajas.iter().filter(|c| c.parte != Parte::Cabeza || true).map(|c| c.b[1]).max().unwrap() as f32 * VOX;
        let (lo, hi) = if a.nino() { (1.0, 1.4) } else { (1.55, 2.1) };
        assert!(alto > lo && alto < hi, "{a:?}: {alto} m");
        assert!(m.cajas.iter().all(|c| c.a[1] >= 0), "{a:?}: nada bajo el suelo");
        // Montado: las piernas cuelgan a los costados del caballo (fuera de su lomo).
        let mm = b.persona(a, true);
        assert!(mm.cajas.iter().filter(|c| c.b[1] <= 0).all(|c| c.a[0] >= 4 || c.b[0] <= -4), "{a:?} montado: algo dentro del caballo");
    }
}

fn q(mujer: bool, edad: u8, oficio: Oficio, semilla: u64) -> Quien {
    Quien { mujer, edad, oficio, principal: false, raiz: Raiz::Mixta, semilla }
}

#[test]
fn la_ropa_sale_de_la_persona_y_es_siempre_la_misma() {
    for s in 0..200u64 {
        for anio in [1726, 1760, 1820, 1900, 1960, 2027] {
            let x = q(s % 2 == 0, (s % 80) as u8, Oficio::Artesano, s);
            assert_eq!(vestir(&x, anio), vestir(&x, anio));
            let a = atuendo(&x, anio);
            assert_eq!(a.mujer(), x.mujer, "{a:?} para {x:?}");
            assert_eq!(a.nino(), x.edad < 13, "{a:?} para {x:?}");
        }
    }
    // Oficios y épocas.
    assert_eq!(atuendo(&q(false, 40, Oficio::Clerigo, 1), 1750), Atuendo::Fraile);
    assert_eq!(atuendo(&q(false, 30, Oficio::Soldado, 2), 1750), Atuendo::Soldado);
    assert_eq!(atuendo(&q(false, 30, Oficio::Marinero, 3), 1750), Atuendo::Marinero);
    assert_eq!(atuendo(&Quien { principal: true, ..q(false, 50, Oficio::Comerciante, 4) }, 1750), Atuendo::Principal);
    assert_eq!(atuendo(&Quien { principal: true, ..q(false, 50, Oficio::Comerciante, 4) }, 1850), Atuendo::Levita);
    assert_eq!(atuendo(&q(true, 30, Oficio::Ninguno, 5), 1960), Atuendo::MujerModerna);
    assert!(matches!(atuendo(&q(false, 30, Oficio::Peon, 6), 1750), Atuendo::Paisano | Atuendo::Peon));
    // Las reglas cubren todas las épocas, sexos y grupos (ninguna elección falla).
    for anio in (1700..2100).step_by(7) {
        for of in [Oficio::Ninguno, Oficio::Peon, Oficio::Hacendado, Oficio::Artesano, Oficio::Pulpero, Oficio::Comerciante, Oficio::Marinero, Oficio::Soldado, Oficio::Servicio, Oficio::Clerigo, Oficio::Funcionario, Oficio::Maestro, Oficio::Obrero, Oficio::Empleado, Oficio::Profesional] {
            for mujer in [false, true] {
                atuendo(&q(mujer, 30, of, anio as u64), anio);
            }
        }
    }
    assert!(REGLAS.iter().all(|r| r.opciones.iter().all(|o| o.0.mujer() == r.mujer)));
    // Distintas personas visten distinto (colores).
    let pals: std::collections::BTreeSet<_> = (0..50u64).map(|s| vestir(&q(false, 30, Oficio::Peon, s), 1750).1).collect();
    assert!(pals.len() > 20);
}

#[test]
fn las_fases_de_la_animacion() {
    use anim::*;
    let a = caminar(0.25, 1.0, 0.0);
    let b = caminar(0.75, 1.0, 0.0);
    assert!((a.get(Parte::PiernaI).1 + b.get(Parte::PiernaI).1).abs() < 1e-5, "medio ciclo después, la pierna al revés");
    assert!((a.get(Parte::PiernaI).1 + a.get(Parte::PiernaD).1).abs() < 1e-5, "las piernas en contrafase");
    assert!(a.get(Parte::PiernaI).1 * a.get(Parte::BrazoI).1 < 0.0, "brazo y pierna del mismo lado al revés");
    assert_eq!(caminar(0.3, 0.0, 0.0).get(Parte::PiernaI).1, 0.0);
    assert!((fase(CICLO_PERSONA * 3.25, CICLO_PERSONA) - 0.25).abs() < 1e-4);
    let mut p = Pose::default();
    ruedas(&mut p, 2.0, 0.5);
    assert!((p.get(Parte::RuedaI).1 - 4.0).abs() < 1e-5);
    // Cuadrúpedo al paso: las cuatro patas en tiempos distintos.
    let c = cuadrupedo(0.1, 1.0);
    let v: Vec<f32> = [Parte::PataTI, Parte::PataDI, Parte::PataTD, Parte::PataDD].iter().map(|x| c.get(*x).1).collect();
    for i in 0..4 {
        for j in i + 1..4 {
            assert!((v[i] - v[j]).abs() > 1e-3);
        }
    }
    // Pastando, la cabeza abajo.
    assert!(pastar(3.0, 7, 1.0).get(Parte::Cabeza).1 >= 1.0);
}

#[test]
fn emitir_y_detalles_por_zona() {
    let b = biblioteca();
    let m = b.objeto(Id::Carreta);
    let mut out = Vec::new();
    let mut pose = Pose::default();
    anim::ruedas(&mut pose, 3.0, 1.0);
    emitir(&mut out, m, &modelos::paleta(Id::Carreta, 1), Vec3::new(10.0, 2.0, 5.0), 0.3, &pose, 1);
    assert_eq!(out.len(), m.cajas.len());
    assert!(out.iter().all(|c| !c.es_sombra() && c.color[3] > 0));
    // Las ruedas giran alrededor de su eje: su pivote está a la altura del eje.
    assert!(out.iter().any(|c| (c.inclina - 3.0).abs() < 1e-4 && (c.pos[1] - (2.0 + 17.0 * VOX)).abs() < 1e-4));
    let mut d = Detalles::default();
    for i in 0..100 {
        d.poner(Id::Tonel, Vec3::new(i as f32 * 10.0, 0.0, 0.0), 0.0, i);
    }
    assert_eq!(d.len(), 100);
    assert_eq!(d.cerca(Vec2::new(0.0, 0.0), 55.0).count(), 6);
    d.quitar(|i| i.pos.x > 500.0);
    assert_eq!(d.len(), 51);
    let mut cajas = Vec::new();
    d.cajas(Vec2::new(0.0, 0.0), &mut cajas);
    assert!(cajas.iter().filter(|c| c.es_sombra()).count() == d.cerca(Vec2::ZERO, RADIO_DETALLES).count());
    let (n, bytes) = biblioteca().medida();
    eprintln!("BIBLIOTECA: {n} cajas, {} KB", bytes / 1024);
    for m in todos() {
        eprintln!("  {}: {} cajas", m.nombre, m.cajas.len());
    }
    assert!(Atuendo::TODOS.iter().all(|a| !a.nombre().is_empty()));
}
