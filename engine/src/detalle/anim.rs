//! Animación barata de los modelos de detalle: cada parte gira alrededor de su pivote con un
//! ángulo que sale de una fórmula (sin huesos ni cuadros clave). La fase de la marcha es la
//! distancia recorrida dividida por el largo del paso, así los pies no patinan; las ruedas
//! giran lo que avanzaron dividido por su radio.

use super::{Parte, Pose};
use std::f32::consts::{PI, TAU};

/// Largo de un ciclo de marcha (dos pasos), m: persona, caballo al paso, vacuno, oveja,
/// perro, gallina.
pub const CICLO_PERSONA: f32 = 1.5;
pub const CICLO_CABALLO: f32 = 2.6;
pub const CICLO_VACUNO: f32 = 2.2;
pub const CICLO_CHICO: f32 = 0.7;

/// Fase (0..1) de la marcha tras recorrer `dist` m con ciclos de `ciclo` m.
pub fn fase(dist: f32, ciclo: f32) -> f32 {
    (dist / ciclo).rem_euclid(1.0)
}

/// Persona caminando: piernas y brazos en contrafase. `amp` 0 = quieta, 1 = paso normal;
/// `mirar` gira la cabeza (rad).
pub fn caminar(f: f32, amp: f32, mirar: f32) -> Pose {
    let s = (f * TAU).sin() * amp;
    let mut p = Pose::default();
    p.set(Parte::PiernaI, 0.0, 0.45 * s);
    p.set(Parte::PiernaD, 0.0, -0.45 * s);
    p.set(Parte::BrazoI, 0.0, -0.35 * s);
    p.set(Parte::BrazoD, 0.0, 0.35 * s);
    p.set(Parte::Cabeza, mirar, 0.0);
    p
}

/// Cuadrúpedo al paso (cuatro tiempos: trasera izquierda, delantera izquierda, trasera
/// derecha, delantera derecha), con el cabeceo del cuello.
pub fn cuadrupedo(f: f32, amp: f32) -> Pose {
    let mut p = Pose::default();
    let pata = |d: f32| (((f + d) * TAU).sin()) * 0.42 * amp;
    p.set(Parte::PataTI, 0.0, pata(0.0));
    p.set(Parte::PataDI, 0.0, pata(0.25));
    p.set(Parte::PataTD, 0.0, pata(0.5));
    p.set(Parte::PataDD, 0.0, pata(0.75));
    p.set(Parte::Cabeza, 0.0, (f * 2.0 * TAU).sin() * 0.06 * amp);
    p.set(Parte::Cola, (f * TAU).sin() * 0.15 * amp, 0.0);
    p
}

/// Animal pastando: la cabeza abajo con un cabeceo lento al arrancar el pasto, la cola que
/// espanta las moscas; `abajo`: cuánto baja la cabeza (rad). `t` en segundos, `semilla` para que no lo hagan todos a la vez.
pub fn pastar(t: f32, semilla: u32, abajo: f32) -> Pose {
    let o = (semilla % 997) as f32 * 0.37;
    let mut p = Pose::default();
    let bocado = ((t * 1.7 + o).sin() * 0.5 + 0.5).powi(4) * 0.12;
    p.set(Parte::Cabeza, (t * 0.21 + o).sin() * 0.18, abajo + bocado);
    p.set(Parte::Cola, cola(t, o), 0.0);
    p
}

/// Animal parado (atado al palenque, esperando): mira alrededor y mueve la cola.
pub fn quieto(t: f32, semilla: u32) -> Pose {
    let o = (semilla % 991) as f32 * 0.53;
    let mut p = Pose::default();
    // Gira la cabeza de a ratos (pausa larga, giro suave).
    let g = (t * 0.13 + o).sin();
    p.set(Parte::Cabeza, g.signum() * g.abs().powf(3.0) * 0.45, ((t * 0.07 + o).sin() * 0.5 + 0.5) * 0.25);
    p.set(Parte::Cola, cola(t, o), 0.0);
    // Descansa una pata trasera.
    p.set(Parte::PataTD, 0.0, -0.12);
    p
}

/// Persona parada: mira de a ratos a un lado y al otro.
pub fn persona_quieta(t: f32, semilla: u32) -> Pose {
    let o = (semilla % 983) as f32 * 0.61;
    let g = (t * 0.17 + o).sin();
    caminar(0.0, 0.0, g.signum() * g.abs().powf(3.0) * 0.6)
}

/// Gallina picoteando: picotazos rápidos con pausas.
pub fn picotear(t: f32, semilla: u32) -> Pose {
    let o = (semilla % 977) as f32 * 0.71;
    let mut p = Pose::default();
    let ciclo = (t * 0.9 + o).rem_euclid(1.0);
    let pico = if ciclo < 0.35 { (ciclo / 0.35 * PI * 2.0).sin().abs() * 1.1 } else { 0.0 };
    p.set(Parte::Cabeza, (t * 0.5 + o).sin() * 0.4, pico);
    p
}

fn cola(t: f32, o: f32) -> f32 {
    // Coletazos de a ratos.
    let k = (t * 0.6 + o).rem_euclid(TAU);
    if k < 1.2 {
        (k * 5.0).sin() * 0.35
    } else {
        (t * 0.4 + o).sin() * 0.05
    }
}

/// Ruedas: giran `dist / radio` rad.
pub fn ruedas(p: &mut Pose, dist: f32, radio: f32) {
    let a = (dist / radio).rem_euclid(TAU);
    p.set(Parte::RuedaI, 0.0, a);
    p.set(Parte::RuedaD, 0.0, a);
}
