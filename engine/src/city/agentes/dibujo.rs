//! Dibujo de los agentes: modelos chicos hechos de cajas (estilo vóxel), una instancia por
//! caja en el renderer (`shaders/agentes.wgsl`, iluminado como la flora baja: misma luz,
//! sombra recibida, niebla y curvatura). Porte de `scripts/agents/agent_meshes.gd`.
//!
//! Ejes del modelo: +z hacia delante, +x a la derecha, y hacia arriba; el origen es el
//! suelo bajo el centro del agente.

use super::super::roads::Edge;
use super::super::voxelize::{deck_top, road_height, water_spans};
use super::{pose, Agente, Agentes, Tipo};
use crate::lod::BASE_VOXEL;
use crate::terrain::TerrainGen;
use glam::Vec2;

/// Solo se dibujan los agentes a menos de esta distancia del centro de la vista (m).
pub const RADIO: f32 = 600.0;
/// Tope de agentes dibujados (los más cercanos).
pub const MAX_DIBUJADOS: usize = 2500;
/// Separación de las muestras de altura de cada tramo (m).
const MUESTRA: f32 = 2.0;

/// Una caja de un agente para el renderer: base (centro del piso de la caja, en metros),
/// giro alrededor de y (rad; 0 = mirando a +z), tamaño (m) y color sRGB.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Caja {
    pub pos: [f32; 3],
    pub giro: f32,
    pub tam: [f32; 3],
    pub color: [u8; 4],
}

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
    let decks: Vec<f32> = spans.iter().map(|&sp| (deck_top(t, e, sp) + 1) as f32 * BASE_VOXEL).collect();
    let n = (e.len() / MUESTRA).ceil() as usize + 1;
    (0..n)
        .map(|i| {
            let s = (i as f32 * MUESTRA).min(e.len());
            if let Some(k) = spans.iter().position(|&(a, b)| s >= a && s <= b) {
                return decks[k];
            }
            let (p, _) = e.point(s);
            // Índice del vóxel de arriba de la calzada (como `voxelize::top_index`) + 1.
            ((road_height(t, p) / BASE_VOXEL - 0.5).floor() + 1.0) * BASE_VOXEL
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

fn rgb(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
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
        out.push(Caja { pos: (base + off).into(), giro, tam: *tam, color: rgb(color) });
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
            for c in &mut out[n - PERSONA.len()..] {
                c.pos[0] += -1.25 * cs + (dz + 2.2) * sn;
                c.pos[2] += 1.25 * sn + (dz + 2.2) * cs;
            }
        }
        Tipo::Carro => {
            poner(out, &CARRO, base, giro, dz + 0.2, ropa, pelaje);
            poner(out, &CABALLO, base, giro, dz + 1.3, ropa, pelaje);
        }
    }
}

/// Multitud de prueba para medir el dibujo: `n` agentes (de todos los tipos, como en una
/// ciudad: casi todos a pie) en una grilla alrededor de `centro`, sobre el terreno.
pub fn multitud(t: &TerrainGen, centro: Vec2, n: usize) -> Vec<Caja> {
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
        modelo(&mut out, tipo, glam::Vec3::new(p.x, t.height(p.x, p.y), p.y), i as f32 * 0.7, i as u8, i as u8);
    }
    out
}

impl Agentes {
    /// Cajas de los agentes cerca de `centro` (a menos de [`RADIO`], los
    /// [`MAX_DIBUJADOS`] más cercanos), con la posición interpolada entre los dos últimos
    /// pasos de la simulación.
    pub fn cajas(&self, net: &super::super::roads::RoadNetwork, centro: Vec2, out: &mut Vec<Caja>) {
        out.clear();
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
        for (_, a, p, d) in vis {
            let pata = a.pata();
            let Some(e) = net.edges.get(&pata.tramo) else { continue };
            // Altura: la más alta entre el frente y el medio del vehículo (el tablero de los
            // puentes arranca con un escalón; así la yunta no queda metida dentro del puente).
            let s_medio = (a.s - a.params().largo * 0.5).max(0.0);
            let ps = |s: f32| if pata.sentido == 0 { s } else { e.len() - s };
            let y = self.perfiles.get(&pata.tramo).map_or(0.0, |pf| altura(pf, ps(a.s)).max(altura(pf, ps(s_medio))));
            let base = glam::Vec3::new(p.x, y, p.y);
            modelo(out, a.tipo, base, d.x.atan2(d.y), a.ropa, a.pelaje);
        }
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
