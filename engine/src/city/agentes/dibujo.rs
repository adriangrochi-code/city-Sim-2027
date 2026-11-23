//! Dibujo de los agentes: modelos chicos hechos de cajas (estilo vóxel), una instancia por
//! caja en el renderer (`shaders/agentes.wgsl`, iluminado como la flora baja: misma luz,
//! sombra recibida, niebla y curvatura). Porte de `scripts/agents/agent_meshes.gd`.
//!
//! Ejes del modelo: +z hacia delante, +x a la derecha, y hacia arriba; el origen es el
//! suelo bajo el centro del agente.

use super::super::roads::Edge;
use super::super::voxelize::{deck_top, road_height, water_spans};
use super::{pose, Agente, Agentes, Rol, Tipo};
use crate::city::poblacion::modelo::{Estamento, Oficio, Raiz, Sexo};
use crate::detalle::anim;
use crate::detalle::ropa::{mezcla, Quien};
use crate::detalle::girar;
use crate::lod::BASE_VOXEL;
use crate::terrain::TerrainGen;
use glam::{Vec2, Vec3};

/// Solo se dibujan los agentes a menos de esta distancia del centro de la vista (m).
pub const RADIO: f32 = 600.0;
/// Tope de agentes dibujados (los más cercanos).
pub const MAX_DIBUJADOS: usize = 2500;
/// Separación de las muestras de altura de cada tramo (m).
const MUESTRA: f32 = 2.0;

/// Una caja para el renderer (ver `detalle::Caja`).
pub use crate::detalle::Caja;

/// Más cerca que esto (m) los agentes se dibujan con los modelos de micro-vóxeles
/// (`detalle`); más lejos, con las cajas de siempre.
pub const CERCA: f32 = 110.0;

/// Ropa de la gente (paños teñidos con tintes de la época).
pub const ROPA: [[u8; 3]; 8] = [[217, 209, 189], [140, 82, 64], [89, 102, 133], [158, 140, 97], [77, 77, 77], [178, 115, 89], [115, 133, 102], [196, 178, 140]];
/// Pelajes de los caballos: zaino, morocho, bayo, tordillo, alazán.
pub const PELAJE: [[u8; 3]; 5] = [[107, 69, 41], [46, 33, 26], [158, 115, 71], [204, 199, 184], [140, 70, 38]];
const PIEL: [u8; 3] = [204, 158, 122];
const OSCURO: [u8; 3] = [38, 31, 26];
const CALZON: [u8; 3] = [89, 77, 64];
const MADERA: [u8; 3] = [107, 77, 48];
const MADERA_OSC: [u8; 3] = [77, 56, 36];
const CUERO: [u8; 3] = [184, 158, 115];
const BUEY: [u8; 3] = [140, 107, 77];
const BUEY_OSC: [u8; 3] = [102, 77, 56];
const CARGA: [u8; 3] = [150, 120, 80];

/// Color de una pieza: fijo, la ropa del agente o el pelaje de su caballo.
#[derive(Clone, Copy)]
enum Tinte {
    Fijo([u8; 3]),
    Ropa,
    Pelaje,
    PelajeOscuro,
}

/// Pieza: centro en planta (x, z), altura de la base, tamaño (x, y, z) y tinte.
type Pieza = ([f32; 3], [f32; 3], Tinte);

/// Persona de 0,5 × 1,75 m: piernas, torso, cabeza y sombrero.
const PERSONA: [Pieza; 4] = [
    ([0.0, 0.0, 0.0], [0.36, 0.85, 0.24], Tinte::Fijo(CALZON)),
    ([0.0, 0.85, 0.0], [0.5, 0.62, 0.3], Tinte::Ropa),
    ([0.0, 1.47, 0.0], [0.24, 0.24, 0.24], Tinte::Fijo(PIEL)),
    ([0.0, 1.69, 0.0], [0.36, 0.06, 0.36], Tinte::Fijo(OSCURO)),
];

/// Caballo (cuerpo, cuatro patas, cuello, cabeza y cola), con el frente en z = +1,2.
const CABALLO: [Pieza; 8] = [
    ([0.0, 0.8, 0.0], [0.55, 0.6, 1.7], Tinte::Pelaje),
    ([-0.18, 0.0, 0.62], [0.14, 0.8, 0.14], Tinte::PelajeOscuro),
    ([0.18, 0.0, 0.62], [0.14, 0.8, 0.14], Tinte::PelajeOscuro),
    ([-0.18, 0.0, -0.62], [0.14, 0.8, 0.14], Tinte::PelajeOscuro),
    ([0.18, 0.0, -0.62], [0.14, 0.8, 0.14], Tinte::PelajeOscuro),
    ([0.0, 1.15, 0.85], [0.3, 0.5, 0.35], Tinte::Pelaje),
    ([0.0, 1.5, 1.1], [0.24, 0.24, 0.5], Tinte::Pelaje),
    ([0.0, 0.85, -0.9], [0.12, 0.55, 0.12], Tinte::PelajeOscuro),
];

/// Jinete sentado sobre el caballo: piernas a los costados, torso con poncho, cabeza y sombrero.
const JINETE: [Pieza; 5] = [
    ([0.0, 1.0, 0.0], [0.7, 0.42, 0.34], Tinte::Fijo(CALZON)),
    ([0.0, 1.4, -0.05], [0.56, 0.62, 0.36], Tinte::Ropa),
    ([0.0, 2.02, -0.05], [0.24, 0.24, 0.24], Tinte::Fijo(PIEL)),
    ([0.0, 2.26, -0.05], [0.5, 0.06, 0.5], Tinte::Fijo(OSCURO)),
    ([0.0, 2.32, -0.05], [0.24, 0.12, 0.24], Tinte::Fijo(OSCURO)),
];

/// Carreta de bueyes: caja con techo de cuero, dos ruedas grandes, pértigo y la yunta.
const CARRETA: [Pieza; 12] = [
    ([0.0, 0.9, -1.9], [1.7, 0.5, 3.2], Tinte::Fijo(MADERA)),
    ([0.0, 1.4, -1.9], [1.6, 0.9, 3.0], Tinte::Fijo(CUERO)),
    ([-0.95, 0.0, -1.9], [0.15, 1.9, 1.9], Tinte::Fijo(MADERA_OSC)),
    ([0.95, 0.0, -1.9], [0.15, 1.9, 1.9], Tinte::Fijo(MADERA_OSC)),
    ([0.0, 0.9, 1.0], [0.12, 0.12, 2.6], Tinte::Fijo(MADERA)),
    ([-0.5, 0.6, 1.9], [0.7, 0.8, 1.8], Tinte::Fijo(BUEY)),
    ([0.5, 0.6, 1.9], [0.7, 0.8, 1.8], Tinte::Fijo(BUEY)),
    ([-0.5, 0.0, 1.9], [0.5, 0.6, 1.4], Tinte::Fijo(BUEY_OSC)),
    ([0.5, 0.0, 1.9], [0.5, 0.6, 1.4], Tinte::Fijo(BUEY_OSC)),
    ([-0.5, 0.95, 2.95], [0.45, 0.45, 0.5], Tinte::Fijo(BUEY)),
    ([0.5, 0.95, 2.95], [0.45, 0.45, 0.5], Tinte::Fijo(BUEY)),
    ([0.0, 1.42, 2.75], [1.6, 0.1, 0.12], Tinte::Fijo(MADERA_OSC)),
];

/// Carro de dos ruedas con carga, tirado por un caballo (que va adelante, z > 0).
const CARRO: [Pieza; 6] = [
    ([0.0, 0.8, -1.2], [1.3, 0.2, 2.0], Tinte::Fijo(MADERA)),
    ([0.0, 1.0, -1.25], [1.1, 0.6, 1.7], Tinte::Fijo(CARGA)),
    ([-0.75, 0.0, -1.2], [0.12, 1.5, 1.5], Tinte::Fijo(MADERA_OSC)),
    ([0.75, 0.0, -1.2], [0.12, 1.5, 1.5], Tinte::Fijo(MADERA_OSC)),
    ([-0.35, 0.85, 0.5], [0.08, 0.08, 1.6], Tinte::Fijo(MADERA)),
    ([0.35, 0.85, 0.5], [0.08, 0.08, 1.6], Tinte::Fijo(MADERA)),
];

/// Alturas de la superficie de un tramo cada [`MUESTRA`] m: la calzada (la misma cuenta que
/// `voxelize::road_edits`) o el tablero de los puentes, donde cruza agua.
pub fn perfil(t: &TerrainGen, e: &Edge) -> Vec<f32> {
    let spans = water_spans(t, &e.pts);
    let deck_idx: Vec<i32> = spans.iter().map(|&sp| deck_top(t, e, sp)).collect();
    let decks: Vec<f32> = deck_idx.iter().map(|&d| (d + 1) as f32 * BASE_VOXEL).collect();
    let n = (e.len() / MUESTRA).ceil() as usize + 1;
    (0..n)
        .map(|i| {
            let s = (i as f32 * MUESTRA).min(e.len());
            if let Some(k) = spans.iter().position(|&(a, b)| s >= a && s <= b) {
                return decks[k];
            }
            let (p, _) = e.point(s);
            // Índice del vóxel de arriba de la calzada (como `voxelize::top_index`) + 1.
            // [agua en vóxeles] Con las rampas de los puentes (`voxelize::rampa`).
            let k = ((road_height(t, p) / BASE_VOXEL - 0.5).floor() as i32).max(crate::city::voxelize::rampa(&spans, &deck_idx, s));
            (k + 1) as f32 * BASE_VOXEL
        })
        .collect()
}

fn altura(perfil: &[f32], ps: f32) -> f32 {
    if perfil.is_empty() {
        return 0.0;
    }
    let x = (ps / MUESTRA).max(0.0);
    let i = (x.floor() as usize).min(perfil.len() - 1);
    let j = (i + 1).min(perfil.len() - 1);
    perfil[i] + (perfil[j] - perfil[i]) * (x - i as f32).clamp(0.0, 1.0)
}


fn oscurecer(c: [u8; 3]) -> [u8; 3] {
    [(c[0] as f32 * 0.7) as u8, (c[1] as f32 * 0.7) as u8, (c[2] as f32 * 0.7) as u8]
}

fn poner(out: &mut Vec<Caja>, piezas: &[Pieza], base: glam::Vec3, giro: f32, dz: f32, ropa: [u8; 3], pelaje: [u8; 3]) {
    let (sn, cs) = giro.sin_cos();
    for (c, tam, tinte) in piezas {
        let (x, z) = (c[0], c[2] + dz);
        let off = glam::Vec3::new(x * cs + z * sn, c[1], -x * sn + z * cs);
        let color = match tinte {
            Tinte::Fijo(k) => *k,
            Tinte::Ropa => ropa,
            Tinte::Pelaje => pelaje,
            Tinte::PelajeOscuro => oscurecer(pelaje),
        };
        out.push(Caja::lisa(base + off, giro, *tam, color));
    }
}

/// Cajas del modelo de un agente: `base` es el suelo bajo su frente y `giro` su rumbo.
pub fn modelo(out: &mut Vec<Caja>, tipo: Tipo, base: glam::Vec3, giro: f32, ropa: u8, pelaje: u8) {
    let ropa = ROPA[ropa as usize % ROPA.len()];
    let pelaje = PELAJE[pelaje as usize % PELAJE.len()];
    // `s` es el frente del vehículo: el modelo se corre medio largo hacia atrás.
    let dz = -tipo.params().largo * 0.5;
    match tipo {
        Tipo::APie => poner(out, &PERSONA, base, giro, 0.0, ropa, pelaje),
        Tipo::Jinete => {
            poner(out, &CABALLO, base, giro, dz + 0.1, ropa, pelaje);
            poner(out, &JINETE, base, giro, dz + 0.1, ropa, pelaje);
        }
        Tipo::Carreta => {
            poner(out, &CARRETA, base, giro, dz + 0.3, ropa, pelaje);
            // El boyero camina al lado de la yunta, con la picana.
            poner(out, &PERSONA, base, giro, 0.0, ropa, pelaje);
            let n = out.len();
            let (sn, cs) = giro.sin_cos();
            for c in &mut out[n - PERSONA.len()..n] {
                c.pos[0] += -1.25 * cs + (dz + 2.2) * sn;
                c.pos[2] += 1.25 * sn + (dz + 2.2) * cs;
            }
        }
        Tipo::Carro => {
            poner(out, &CARRO, base, giro, dz + 0.2, ropa, pelaje);
            poner(out, &CABALLO, base, giro, dz + 1.3, ropa, pelaje);
        }
        // [ia] La punta del arreo: el capataz a caballo (la tropa la pone `tropa`).
        Tipo::Arreo => {
            poner(out, &CABALLO, base, giro, -1.2, ropa, pelaje);
            poner(out, &JINETE, base, giro, -1.2, ropa, pelaje);
        }
    }
}

/// Lo que hace falta para vestir a los vecinos: la población (personas de verdad), los
/// solares (la casa de los vecinos sintéticos), el año y los segundos para lo que se mueve
/// sin avanzar (mirar alrededor).
#[derive(Clone, Copy, Default)]
pub struct Vestuario<'a> {
    pub poblacion: Option<&'a crate::city::poblacion::Poblacion>,
    pub lots: Option<&'a crate::city::lots::Lots>,
    pub anio: i32,
    pub t: f32,
}

/// Oficio de la demografía para los oficios de los vecinos sintéticos (`OFICIOS`).
fn oficio_sintetico(i: u8) -> Oficio {
    match i {
        1 => Oficio::Pulpero,
        2 | 8 => Oficio::Peon,
        6 => Oficio::Servicio,
        _ => Oficio::Artesano,
    }
}

/// Quién es un agente, para vestirlo: su persona de la demografía si la tiene; si no, lo que
/// se sabe del vecino sintético (nombre, oficio, origen, casa); si no, un carretero.
pub fn quien(ag: &Agentes, a: &Agente, v: &Vestuario) -> Quien {
    if let (Some(id), Some(pob)) = (a.persona, v.poblacion) {
        if let Some(p) = pob.persona(id) {
            let principal = p.riqueza > 600.0 || (p.estamento == Estamento::Vecino && p.riqueza > 250.0);
            return Quien { mujer: p.sexo == Sexo::Mujer, edad: p.edad(pob.mes).clamp(0, 120) as u8, oficio: p.oficio, principal, raiz: p.raiz, semilla: id as u64 ^ 0x5EED_0000 };
        }
    }
    if let Some(vec) = a.vecino.and_then(|k| ag.e.vecinos.get(&k).map(|v| (k, v))) {
        let (k, vc) = vec;
        let s = mezcla(k);
        let edad = match vc.rol {
            Rol::NoTrabaja if s % 10 < 7 => 9,
            Rol::NoTrabaja => 66,
            _ => 22 + (s % 30) as u8,
        };
        let raiz = match vc.origen {
            1 | 2 => Raiz::Europea,
            0 if s % 3 == 0 => Raiz::Mixta,
            0 => Raiz::Europea,
            _ if s % 2 == 0 => Raiz::Indigena,
            _ => Raiz::Mixta,
        };
        let principal = v.lots.and_then(|l| l.lots.get(&vc.casa)).is_some_and(|l| l.level >= 3);
        return Quien { mujer: vc.nombre.0 >= 12, edad, oficio: oficio_sintetico(vc.oficio), principal, raiz, semilla: k };
    }
    let s = mezcla(a.id as u64 ^ ((a.conductor.0 as u64) << 40));
    Quien { mujer: false, edad: 25 + (s % 30) as u8, oficio: Oficio::Peon, principal: false, raiz: [Raiz::Europea, Raiz::Mixta, Raiz::Indigena][(s % 3) as usize], semilla: s }
}

/// Un agente con los modelos de micro-vóxeles: `base` es el suelo bajo su frente, `giro` su
/// rumbo, `dist` lo que lleva recorrido (fase de la marcha y de las ruedas), `vel` su velocidad
/// y `t` los segundos (para mirar alrededor).
#[allow(clippy::too_many_arguments)]
pub fn detallado(out: &mut Vec<Caja>, tipo: Tipo, base: Vec3, giro: f32, q: &Quien, anio: i32, dist: f32, vel: f32, t: f32, id: u32, pelaje: u8) {
    use crate::detalle::{biblioteca, emitir, modelos, ropa, sombra, Id, Parte, VOX};
    let b = biblioteca();
    let en = |v: Vec3| base + girar(v, giro);
    let (at, pal) = ropa::vestir(q, anio);
    let amp = (vel / tipo.params().v0.max(0.5) * 1.2).clamp(0.0, 1.0);
    let mirar = (t * 0.23 + id as f32 * 1.7).sin().powi(5) * 0.5;
    let persona = |out: &mut Vec<Caja>, p: Vec3, d: f32| {
        let ciclo = if at.nino() { 1.0 } else { anim::CICLO_PERSONA };
        let pose = if amp > 0.05 { anim::caminar(anim::fase(d, ciclo), amp, mirar * 0.4) } else { anim::persona_quieta(t, id) };
        emitir(out, b.persona(at, false), &pal, p, giro, &pose, id);
        sombra(out, p, giro, [0.27, 0.2], 130);
    };
    let semilla_caballo = id.wrapping_mul(7).wrapping_add(pelaje as u32);
    match tipo {
        Tipo::APie => persona(out, base, dist),
        Tipo::Jinete => {
            let o = en(Vec3::new(0.0, 0.0, -1.6));
            let f = anim::fase(dist, anim::CICLO_CABALLO);
            let m = b.objeto(Id::CaballoEnsillado);
            emitir(out, m, &modelos::paleta(Id::CaballoEnsillado, semilla_caballo), o, giro, &anim::cuadrupedo(f, amp), semilla_caballo);
            sombra(out, o, giro, [0.3, 1.15], 130);
            // El jinete sube y baja con el paso; las manos adelante, con las riendas.
            let salto = (f * std::f32::consts::TAU * 2.0).sin().abs() * 0.035 * amp;
            let mut pose = anim::caminar(0.0, 0.0, mirar);
            pose.set(Parte::BrazoI, 0.0, -0.55);
            pose.set(Parte::BrazoD, 0.0, -0.55);
            emitir(out, b.persona(at, true), &pal, o + Vec3::Y * (24.0 * VOX + salto), giro, &pose, id);
        }
        Tipo::Carreta => {
            let mut pose = crate::detalle::Pose::default();
            anim::ruedas(&mut pose, dist, 17.0 * VOX);
            let pal_c = modelos::paleta(Id::Carreta, id);
            let o = en(Vec3::new(0.0, 0.0, -6.13));
            emitir(out, b.objeto(Id::Carreta), &pal_c, o, giro, &pose, id);
            sombra(out, o, giro, [1.0, 1.9], 120);
            for (k, x) in [-0.42f32, 0.42].into_iter().enumerate() {
                let s = id.wrapping_mul(3).wrapping_add(k as u32);
                let p = en(Vec3::new(x, 0.0, -1.6));
                emitir(out, b.objeto(Id::Buey), &modelos::paleta(Id::Buey, s), p, giro, &anim::cuadrupedo(anim::fase(dist + k as f32 * 0.5, anim::CICLO_VACUNO), amp), s);
                sombra(out, p, giro, [0.32, 1.1], 120);
            }
            // El boyero camina al lado de la yunta.
            persona(out, en(Vec3::new(-1.15, 0.0, -1.0)), dist);
        }
        // [ia] La punta del arreo: el capataz a caballo (la tropa la pone `tropa`).
        Tipo::Arreo => {
            let o = en(Vec3::new(0.0, 0.0, -1.6));
            let f = anim::fase(dist, anim::CICLO_CABALLO);
            emitir(out, b.objeto(Id::CaballoEnsillado), &modelos::paleta(Id::CaballoEnsillado, semilla_caballo), o, giro, &anim::cuadrupedo(f, amp), semilla_caballo);
            sombra(out, o, giro, [0.3, 1.15], 130);
            let mut pose = anim::caminar(0.0, 0.0, mirar);
            pose.set(Parte::BrazoI, 0.0, -0.55);
            pose.set(Parte::BrazoD, 0.0, -0.55);
            emitir(out, b.persona(at, true), &pal, o + Vec3::Y * (24.0 * VOX), giro, &pose, id);
        }
        Tipo::Carro => {
            let o = en(Vec3::new(0.0, 0.0, -1.6));
            emitir(out, b.objeto(Id::Caballo), &modelos::paleta(Id::Caballo, semilla_caballo), o, giro, &anim::cuadrupedo(anim::fase(dist, anim::CICLO_CABALLO), amp), semilla_caballo);
            sombra(out, o, giro, [0.3, 1.15], 130);
            let mut pose = crate::detalle::Pose::default();
            anim::ruedas(&mut pose, dist, 12.0 * VOX);
            let c = en(Vec3::new(0.0, 0.0, -4.2));
            emitir(out, b.objeto(Id::Carro), &modelos::paleta(Id::Carro, id), c, giro, &pose, id);
            sombra(out, c, giro, [0.75, 1.0], 120);
            let mut pd = anim::caminar(0.0, 0.0, mirar);
            pd.set(Parte::BrazoI, 0.0, -0.6);
            pd.set(Parte::BrazoD, 0.0, -0.6);
            emitir(out, b.persona(at, true), &pal, c + girar(Vec3::new(0.0, 20.0 * VOX, 11.0 * VOX), giro), giro, &pd, id);
        }
    }
}

/// Multitud de prueba para medir el dibujo: `n` agentes (de todos los tipos, como en una
/// ciudad: casi todos a pie) en una grilla alrededor de `centro`, sobre el terreno; los que
/// quedan a menos de [`CERCA`] con los modelos de micro-vóxeles, como en el juego.
pub fn multitud(t: &TerrainGen, centro: Vec2, n: usize) -> Vec<Caja> {
    multitud_con(t, centro, n, CERCA)
}

/// Como [`multitud`], con micro-vóxeles hasta `cerca` m (0: todo con las cajas de siempre).
pub fn multitud_con(t: &TerrainGen, centro: Vec2, n: usize, cerca: f32) -> Vec<Caja> {
    let mut out = Vec::new();
    let lado = (n as f32).sqrt().ceil() as usize;
    for i in 0..n {
        let p = centro + Vec2::new((i % lado) as f32 - lado as f32 * 0.5, (i / lado) as f32 - lado as f32 * 0.5) * 6.0;
        let tipo = match i % 10 {
            0 => Tipo::Jinete,
            1 => Tipo::Carreta,
            2 => Tipo::Carro,
            _ => Tipo::APie,
        };
        let base = Vec3::new(p.x, t.height(p.x, p.y), p.y);
        let giro = i as f32 * 0.7;
        if p.distance(centro) < cerca {
            let s = mezcla(i as u64);
            let q = Quien { mujer: s % 2 == 0, edad: if s % 7 == 0 { 9 } else { 30 }, oficio: [Oficio::Peon, Oficio::Artesano, Oficio::Pulpero, Oficio::Servicio, Oficio::Soldado][(s / 7 % 5) as usize], principal: s % 11 == 0, raiz: Raiz::Mixta, semilla: s };
            detallado(&mut out, tipo, base, giro, &q, 1750, i as f32 * 0.37, tipo.params().v0, 0.0, i as u32, i as u8);
        } else {
            modelo(&mut out, tipo, base, giro, i as u8, i as u8);
        }
    }
    out
}

impl Agentes {
    /// Cajas de los agentes cerca de `centro` (a menos de [`RADIO`], los
    /// [`MAX_DIBUJADOS`] más cercanos), con la posición interpolada entre los dos últimos
    /// pasos de la simulación; vestidos como de costumbre en la colonia.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cajas(&self, net: &super::super::roads::RoadNetwork, centro: Vec2, out: &mut Vec<Caja>) {
        out.clear();
        self.cajas_con(net, centro, &Vestuario::default(), out);
    }

    /// Como [`cajas`](Self::cajas), con el vestuario de la ciudad (agrega a `out`): a menos de
    /// [`CERCA`] m con los modelos de micro-vóxeles, más lejos con las cajas de siempre.
    pub fn cajas_con(&self, net: &super::super::roads::RoadNetwork, centro: Vec2, v: &Vestuario, out: &mut Vec<Caja>) {
        let alfa = self.alfa();
        let mut vis: Vec<(f32, &Agente, Vec2, Vec2)> = Vec::new();
        for a in &self.e.agentes {
            let (p, d) = pose(net, a);
            let (p, d) = match a.antes {
                Some((q, dq)) if q.distance(p) < 12.0 => (q.lerp(p, alfa), dq.lerp(d, alfa).normalize_or(d)),
                _ => (p, d),
            };
            let dist = p.distance(centro);
            if dist < RADIO {
                vis.push((dist, a, p, d));
            }
        }
        if vis.len() > MAX_DIBUJADOS {
            vis.select_nth_unstable_by(MAX_DIBUJADOS, |x, y| x.0.total_cmp(&y.0));
            vis.truncate(MAX_DIBUJADOS);
        }
        for (dist, a, p, d) in vis {
            let pata = a.pata();
            let Some(e) = net.edges.get(&pata.tramo) else { continue };
            // Altura: la más alta entre el frente y el medio del vehículo (el tablero de los
            // puentes arranca con un escalón; así la yunta no queda metida dentro del puente).
            let s_medio = (a.s - a.params().largo * 0.5).max(0.0);
            let ps = |s: f32| if pata.sentido == 0 { s } else { e.len() - s };
            let y = self.perfiles.get(&pata.tramo).map_or(0.0, |pf| altura(pf, ps(a.s)).max(altura(pf, ps(s_medio))));
            let base = Vec3::new(p.x, y, p.y);
            let giro = d.x.atan2(d.y);
            if a.tipo == Tipo::Arreo {
                self.tropa(net, a, centro, v, out); // [ia]
            }
            if dist < CERCA {
                let q = quien(self, a, v);
                // Lo recorrido: la posición en la pata (más un corrimiento por pata).
                let recorrido = a.s + a.li as f32 * 53.0;
                detallado(out, a.tipo, base, giro, &q, v.anio, recorrido, a.v, v.t, a.id, a.pelaje);
            } else {
                modelo(out, a.tipo, base, giro, a.ropa, a.pelaje);
            }
        }
    }

    // [ia]
    /// La tropa de un arreo: las cabezas en columnas de tres detrás de la punta, siguiendo el
    /// camino, y los troperos a caballo a los costados y atrás. Cerca, con los modelos de
    /// micro-vóxeles; lejos, una caja por cabeza.
    fn tropa(&self, net: &super::super::roads::RoadNetwork, a: &Agente, centro: Vec2, v: &Vestuario, out: &mut Vec<Caja>) {
        use crate::detalle::{biblioteca, emitir, modelos, sombra, Id};
        let paso = super::datos::datos().tropas.metros_por_cabeza * 3.0;
        let filas = (a.cabezas as f32 / 3.0).ceil() as usize;
        let b = biblioteca();
        let altura_en = |li: u16, s: f32| {
            let pata = a.patas[li as usize];
            let Some(e) = net.edges.get(&pata.tramo) else { return 0.0 };
            let ps = if pata.sentido == 0 { s } else { e.len() - s };
            self.perfiles.get(&pata.tramo).map_or(0.0, |pf| altura(pf, ps))
        };
        let tropero = |out: &mut Vec<Caja>, d: f32, lado: f32, n: u32| {
            let (p, dir, li, s) = super::pose_atras_con(net, a, d);
            let p = p + Vec2::new(-dir.y, dir.x) * lado;
            let base = Vec3::new(p.x, altura_en(li, s), p.y);
            modelo(out, Tipo::Jinete, base + girar(Vec3::Z * 1.3, dir.x.atan2(dir.y)), dir.x.atan2(dir.y), a.ropa.wrapping_add(n as u8), a.pelaje.wrapping_add(n as u8));
        };
        for f in 0..filas {
            let d = 4.0 + f as f32 * paso;
            let (p, dir, li, s) = super::pose_atras_con(net, a, d);
            if p.distance(centro) > RADIO {
                continue;
            }
            let giro = dir.x.atan2(dir.y);
            let lado = Vec2::new(-dir.y, dir.x);
            let y = altura_en(li, s);
            for c in 0..3u32 {
                let k = f as u32 * 3 + c;
                if k >= a.cabezas as u32 {
                    break;
                }
                // Columnas desparejas (no es un desfile).
                let h = (k.wrapping_mul(2_654_435_761) >> 16) & 0xFF;
                let q = p + lado * ((c as f32 - 1.0) * 1.15 + (h as f32 / 255.0 - 0.5) * 0.5) - dir * ((h & 7) as f32 * 0.12);
                let base = Vec3::new(q.x, y, q.y);
                let g = giro + ((h >> 3) as f32 / 31.0 - 0.5) * 0.4;
                let semilla = a.id.wrapping_mul(131).wrapping_add(k);
                if q.distance(centro) < CERCA {
                    let id = if h % 3 == 0 { Id::VacaOvera } else { Id::Vaca };
                    let fase = anim::fase(a.s + a.li as f32 * 53.0 + k as f32 * 0.37, anim::CICLO_VACUNO);
                    let amp = (a.v / 0.8).clamp(0.0, 1.0);
                    emitir(out, b.objeto(id), &modelos::paleta(id, semilla), base, g, &anim::cuadrupedo(fase, amp), semilla);
                    sombra(out, base, g, [0.35, 1.1], 110);
                } else {
                    let col = if h % 3 == 0 { [180, 160, 140] } else { [120, 82, 52] };
                    out.push(Caja::lisa(base + girar(Vec3::new(0.0, 0.55, 0.0), g), g, [0.55, 0.75, 1.6], col));
                }
            }
        }
        let largo = 4.0 + filas as f32 * paso;
        let troperos = (a.cabezas as u32).div_ceil(super::datos::datos().tropas.cabezas_por_tropero.max(1)).max(1);
        for n in 0..troperos {
            let d = largo * (n + 1) as f32 / (troperos + 1) as f32;
            tropero(out, d, if n % 2 == 0 { 2.6 } else { -2.6 }, n);
        }
        tropero(out, largo + 2.5, 0.0, troperos);
        let _ = v;
    }

    /// Altura de la superficie bajo un agente (calzada o tablero del puente), si se conoce.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn altura_de(&self, net: &super::super::roads::RoadNetwork, id: u32) -> Option<f32> {
        let a = self.e.agentes.iter().find(|a| a.id == id)?;
        let pata = a.pata();
        let e = net.edges.get(&pata.tramo)?;
        let ps = if pata.sentido == 0 { a.s } else { e.len() - a.s };
        Some(altura(self.perfiles.get(&pata.tramo)?, ps))
    }
}
