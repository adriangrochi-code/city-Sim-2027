//! Agenda encadenada de cada vecino (`docs/sistemas/ia-y-transito.md` D1, §4.1): el plan del
//! día es una lista de actividades (adónde va, desde qué hora puede salir y cuánto se queda),
//! y **la salida siguiente se programa al llegar**. Así nadie está en dos lugares a la vez: la
//! vuelta del mercado sale cuando terminó la compra, no a una hora fija aunque la ida no haya
//! llegado.
//!
//! Recados y vida social de la colonia: compra diaria en la pulpería del barrio (una persona
//! por hogar, con fidelidad), recova a media mañana, pulpería a la vuelta del trabajo, misa
//! (el domingo y, los muy devotos, entre semana), paseo de la plaza el domingo por la tarde y
//! visitas a los parientes. Con lluvia o epidemia bajan los viajes optativos.
//!
//! Todo el azar es por contador ([`super::perfil::azar`]): el plan de una persona no cambia
//! si cambian los demás.

use super::datos::datos;
use super::perfil::{azar, azar_n, Motivo};
use super::{Lugar, Proposito, Rol, Vecino};
use serde::{Deserialize, Serialize};

/// Una actividad del plan del día.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Actividad {
    /// Para qué va (lo que se ve en la ficha del agente).
    pub que: Proposito,
    pub destino: Lugar,
    /// No sale antes de esta hora del día.
    pub desde: f32,
    /// Horas que se queda al llegar.
    pub dura: f32,
    /// Optativa: se suspende con lluvia o epidemia.
    pub optativa: bool,
    /// Lo que pasó: horas (absolutas: día × 24 + hora) de salida y de llegada.
    #[serde(default)]
    pub salio: Option<f64>,
    #[serde(default)]
    pub llego: Option<f64>,
}

impl Actividad {
    fn new(que: Proposito, destino: Lugar, desde: f32, dura: f32, optativa: bool) -> Actividad {
        Actividad { que, destino, desde, dura, optativa, salio: None, llego: None }
    }
}

/// Lo que el plan necesita saber del día.
#[derive(Clone, Copy, Debug, Default)]
pub struct Dia {
    pub dia: u32,
    pub domingo: bool,
    pub lluvia: bool,
    pub epidemia: bool,
    /// Hay una pulpería conocida (la del barrio) y la iglesia.
    pub hay_pulperia: bool,
}

fn entre(semilla: u64, k: u64, dia: u32, m: Motivo, n: u32, a: f32, b: f32) -> f32 {
    a + (b - a) * azar_n(semilla, k, dia, m as u32, n)
}

/// ¿Suspende un viaje optativo? Con lluvia, según la prudencia; con epidemia, la mitad (más
/// los prudentes).
fn suspende(semilla: u64, k: u64, v: &Vecino, d: &Dia, n: u32) -> bool {
    let c = &datos().clima;
    let mut p = 0.0;
    if d.lluvia {
        p += c.optativos_con_lluvia + c.optativos_con_lluvia_prudencia * v.perfil.prudencia();
    }
    if d.epidemia {
        p += 0.5 + 0.2 * v.perfil.prudencia();
    }
    p > 0.0 && azar_n(semilla, k, d.dia, Motivo::Lluvia as u32, n) < p
}

/// El plan de un vecino para el día `d` (vacío si no sale).
pub fn plan(semilla: u64, k: u64, v: &Vecino, d: &Dia) -> Vec<Actividad> {
    let r = &datos().recados;
    let pf = &v.perfil;
    let x = |m: Motivo| azar(semilla, k, d.dia, m);
    let mut out: Vec<Actividad> = Vec::new();
    // Los chicos chicos no salen solos.
    if pf.edad > 0 && pf.edad < 7 {
        return out;
    }
    let casa = Actividad::new(Proposito::VuelveACasa, Lugar::Casa, 0.0, 0.0, false);
    // Una salida desde la casa y de vuelta (si no se suspende).
    let salida = |out: &mut Vec<Actividad>, a: Actividad, n: u32| {
        if a.optativa && suspende(semilla, k, v, d, n) {
            return;
        }
        out.push(a);
        out.push(Actividad { desde: a.desde + a.dura, ..casa });
    };
    if d.domingo {
        // Misa: probabilidad según la devoción.
        // [ia] La misa mayor termina a la misma hora para todos; después, un rato en el atrio
        // (más los sociables): la plaza se llena y la salida se ve.
        if x(Motivo::Misa) < r.misa_domingo * (0.35 + 0.65 * pf.devocion()) {
            let o = &datos().oficios;
            let h = 9.0 + entre(semilla, k, d.dia, Motivo::Misa, 1, -0.4, 0.1);
            let charla = entre(semilla, k, d.dia, Motivo::Atrio, 0, 0.0, 0.4) + 0.6 * pf.social();
            let charla = (o.atrio_minutos[0] + (o.atrio_minutos[1] - o.atrio_minutos[0]) * charla.min(1.0)) / 60.0;
            let ida = Actividad::new(Proposito::AMisa, Lugar::Iglesia, h, 0.2, false);
            out.push(ida);
            out.push(Actividad { desde: o.misa_fin_domingo + charla, que: Proposito::VuelveDeMisa, ..casa });
        }
        // Visita a los parientes (padres o hijos casados que viven en otra casa).
        if let Some(l) = v.visita {
            if x(Motivo::Visita) < r.visita_pariente_domingo * (0.5 + pf.social()) {
                let h = entre(semilla, k, d.dia, Motivo::Visita, 1, 14.5, 16.0);
                salida(&mut out, Actividad::new(Proposito::AVisita, Lugar::Solar(l), h, entre(semilla, k, d.dia, Motivo::Duracion, 1, 1.0, 2.0), true), 1);
            }
        }
        // Paseo de la plaza a la tarde.
        if x(Motivo::Paseo) < r.paseo_domingo * (0.3 + 0.7 * pf.social()) {
            let h = entre(semilla, k, d.dia, Motivo::Paseo, 1, r.paseo_hora[0], r.paseo_hora[1]);
            let dura = entre(semilla, k, d.dia, Motivo::Duracion, 2, r.paseo_minutos[0], r.paseo_minutos[1]) / 60.0;
            salida(&mut out, Actividad::new(Proposito::APaseo, Lugar::Plaza, h, dura, true), 2);
        }
        ordenar(&mut out);
        return out;
    }
    let jit = entre(semilla, k, d.dia, Motivo::Variacion, 0, -0.12, 0.12) * (0.5 + pf.flexibilidad());
    // Misa de semana, los muy devotos (antes del trabajo, a las 7).
    let misa_semana = pf.devocion() >= r.misa_semana_devocion_minima && x(Motivo::Misa) < r.misa_semana && v.rol != Rol::Fuera;
    if misa_semana {
        salida(&mut out, Actividad::new(Proposito::AMisa, Lugar::Iglesia, 6.9 + jit * 0.3, 0.6, true), 3);
    }
    if v.rol == Rol::Fuera {
        // Al trabajo y, a la vuelta, quizás la pulpería.
        let entrada = v.entrada + jit;
        let salida_h = v.salida + jit;
        out.push(Actividad::new(Proposito::AlTrabajo, Lugar::Trabajo, entrada, 0.0, false));
        let pulperia = d.hay_pulperia && v.pulperia.is_some() && x(Motivo::Pulperia) < r.pulperia_tras_trabajo * (0.4 + pf.social()) && !suspende(semilla, k, v, d, 4);
        if pulperia {
            let dura = entre(semilla, k, d.dia, Motivo::Duracion, 3, r.pulperia_minutos[0], r.pulperia_minutos[1]) / 60.0;
            out.push(Actividad::new(Proposito::APulperia, Lugar::Solar(v.pulperia.unwrap()), salida_h, dura, true));
            out.push(Actividad { desde: salida_h + dura, ..casa });
        } else {
            out.push(Actividad { desde: salida_h, que: Proposito::VuelveDelTrabajo, ..casa });
        }
    } else {
        // La compra del hogar en la pulpería del barrio.
        if v.compra && d.hay_pulperia && v.pulperia.is_some() && x(Motivo::Compra) < r.compra_por_hogar {
            let h = entre(semilla, k, d.dia, Motivo::Compra, 1, 8.3, 10.8);
            let dura = entre(semilla, k, d.dia, Motivo::Duracion, 4, r.compra_minutos[0], r.compra_minutos[1]) / 60.0;
            salida(&mut out, Actividad::new(Proposito::ACompra, Lugar::Solar(v.pulperia.unwrap()), h, dura, false), 5);
        }
        // La recova (el mercado de la plaza), a media mañana.
        if pf.edad >= 12 && x(Motivo::Recova) < r.recova_por_clase[v.clase.min(3) as usize] * (0.5 + 0.6 * pf.social()) {
            let h = entre(semilla, k, d.dia, Motivo::Recova, 1, 10.0, 11.6);
            let dura = entre(semilla, k, d.dia, Motivo::Duracion, 5, r.recova_minutos[0], r.recova_minutos[1]) / 60.0;
            salida(&mut out, Actividad::new(Proposito::AlMercado, Lugar::Plaza, h, dura, true), 6);
        }
    }
    ordenar(&mut out);
    out
}

/// Ordena las salidas por hora, sin superponerlas: cada ida con su vuelta.
fn ordenar(out: &mut Vec<Actividad>) {
    // Pares (ida, vuelta) ordenados por la hora de la ida; el trabajo (con su vuelta y su
    // pulpería) va como un bloque.
    let mut bloques: Vec<Vec<Actividad>> = Vec::new();
    for a in out.drain(..) {
        if a.destino == Lugar::Casa {
            if let Some(b) = bloques.last_mut() {
                b.push(a);
                continue;
            }
        }
        match bloques.last_mut() {
            Some(b) if b.last().is_some_and(|u| u.destino != Lugar::Casa) => b.push(a),
            _ => bloques.push(vec![a]),
        }
    }
    bloques.sort_by(|x, y| x[0].desde.total_cmp(&y[0].desde));
    // Si un bloque empieza antes de que termine el anterior, se corre.
    let mut fin = 0.0f32;
    for b in &mut bloques {
        if b[0].desde < fin {
            let d = fin - b[0].desde;
            for a in b.iter_mut() {
                a.desde += d;
            }
        }
        fin = b.iter().map(|a| a.desde + a.dura).fold(0.0, f32::max) + 0.25;
        out.extend(b.iter().copied());
    }
}

/// Hora del día de una hora absoluta («6:10»).
pub fn hhmm(h: f64) -> String {
    let h = h.rem_euclid(24.0);
    let m = (h * 60.0).round() as i32;
    format!("{}:{:02}", (m / 60) % 24, m % 60)
}

/// Líneas de la agenda del día para la ficha del vecino: lo que hizo y lo que le falta.
pub fn lineas(v: &Vecino, dia: u32) -> Vec<String> {
    let mut l = Vec::new();
    for (i, a) in v.plan.iter().enumerate() {
        let que = a.que.texto();
        let t = match (a.salio, a.llego) {
            (Some(s), Some(e)) => format!("{} sale {que}; llegó {}", hhmm(s), hhmm(e)),
            (Some(s), None) => format!("{} sale {que} (en camino)", hhmm(s)),
            _ => {
                // La próxima, a la hora programada; las demás, no antes de su hora (o
                // «después», si esa hora ya pasó: se encadenan al llegar).
                let cuando = if i == v.paso_plan as usize { v.sale.map(|s| s - dia as f64 * 24.0) } else { None };
                // Una vuelta sin hora propia sale al terminar lo anterior.
                let fin_previa = v.plan[..i].last().map_or(0.0, |b| b.desde + b.dura);
                let hora = match cuando {
                    Some(c) => format!("~{}", hhmm(c)),
                    None if i > 0 && a.desde <= fin_previa + 1e-3 => "después".to_string(),
                    None => format!("~{}", hhmm(a.desde as f64)),
                };
                format!("{hora} {que}{}", if a.optativa { " (si el tiempo da)" } else { "" })
            }
        };
        l.push(t);
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::agentes::perfil::Perfil;

    fn vecino(rol: Rol, social: u8, prudencia: u8) -> Vecino {
        let mut v = crate::city::agentes::tests::vecino_de_prueba();
        v.rol = rol;
        v.perfil = Perfil { social, prudencia, devocion: 200, edad: 35, paciencia: 20, v_pie: 67, ..Default::default() };
        v.pulperia = Some(9);
        v.compra = true;
        v
    }

    #[test]
    fn cada_ida_tiene_su_vuelta_y_no_se_superponen() {
        for k in 0..400u64 {
            for dia in 0..7 {
                let v = vecino(if k % 2 == 0 { Rol::Fuera } else { Rol::EnCasa }, (k * 37 % 255) as u8, 100);
                let d = Dia { dia, domingo: dia % 7 == 6, hay_pulperia: true, ..Default::default() };
                let p = plan(5, k, &v, &d);
                if p.is_empty() {
                    continue;
                }
                assert_eq!(p.last().unwrap().destino, Lugar::Casa, "{p:?}");
                for w in p.windows(2) {
                    assert!(w[1].desde >= w[0].desde - 1e-3, "{p:?}");
                }
            }
        }
    }

    #[test]
    fn con_lluvia_salen_menos_a_pasear() {
        let mut seco = 0;
        let mut lluvia = 0;
        for k in 0..2000u64 {
            let v = vecino(Rol::EnCasa, 200, 128);
            for (llueve, n) in [(false, &mut seco), (true, &mut lluvia)] {
                let d = Dia { dia: 6, domingo: true, lluvia: llueve, hay_pulperia: true, ..Default::default() };
                *n += plan(5, k, &v, &d).iter().filter(|a| a.que == Proposito::APaseo).count();
            }
        }
        assert!(seco > 300, "{seco}");
        assert!((lluvia as f32) < seco as f32 * 0.7, "con lluvia {lluvia}, sin lluvia {seco}");
        // Los prudentes suspenden más.
        let mut prudentes = 0;
        for k in 0..2000u64 {
            let v = vecino(Rol::EnCasa, 200, 250);
            let d = Dia { dia: 6, domingo: true, lluvia: true, hay_pulperia: true, ..Default::default() };
            prudentes += plan(5, k, &v, &d).iter().filter(|a| a.que == Proposito::APaseo).count();
        }
        assert!(prudentes < lluvia, "prudentes {prudentes}, todos {lluvia}");
    }

    #[test]
    fn los_sociables_van_mas_a_la_pulperia() {
        let n = |social: u8| (0..3000u64).filter(|&k| plan(5, k, &vecino(Rol::Fuera, social, 100), &Dia { dia: 2, hay_pulperia: true, ..Default::default() }).iter().any(|a| a.que == Proposito::APulperia)).count();
        let (poco, mucho) = (n(10), n(245));
        assert!(mucho as f32 > poco as f32 * 1.8, "{poco} {mucho}");
    }
}
