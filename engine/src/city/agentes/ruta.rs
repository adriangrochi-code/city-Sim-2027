//! Búsqueda de rutas sobre la red de calles (porte de `scripts/agents/router.gd`).
//!
//! - Peatones: el camino más corto.
//! - Vehículos: el más rápido según el tiempo *medido* en cada carril (la congestión real),
//!   con una percepción personal distinta para cada vecino (±15 %). Así no todos eligen la
//!   misma calle y la gente evita los atascos que ya conoce.
//!
//! A* sobre los nodos de [`RoadNetwork`], con los tramos de cada nodo como vecinos (los
//! puentes son tramos como cualquier otro). Las rutas entre nodos se guardan en caché (por
//! franja horaria); la caché se vacía cuando cambia la red y cada día (los tiempos medidos
//! cambian despacio).

use super::super::roads::{RoadKind, RoadNetwork};
use crate::fxhash::FxBuild;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::sync::Arc;

/// Velocidad de referencia de cada tipo de calle para el tiempo "libre" de un carril (m/s).
pub fn ref_speed(k: RoadKind) -> f32 {
    match k {
        RoadKind::Dirt => 2.6,
        RoadKind::Stone => 3.2,
    }
}

/// Variantes de percepción personal (la semilla de ruta de cada vecino cae en una).
pub const PERCEPCIONES: u32 = 8;

/// Clave de un carril: tramo y sentido (0 = de `a` a `b`).
pub fn carril(tramo: u32, sentido: u8) -> u32 {
    tramo * 2 + sentido as u32
}

/// Una ruta entre dos nodos: tramos con su sentido, y su costo total.
#[derive(Clone, Debug, PartialEq)]
pub struct Camino {
    pub tramos: Vec<(u32, u8)>,
    pub costo: f32,
}

#[derive(Clone, Copy, PartialEq)]
struct Abierto {
    f: f32,
    g: f32,
    nodo: u32,
}

impl Eq for Abierto {}
impl Ord for Abierto {
    // Montículo de mínimos; a igual costo decide el id del nodo (orden determinista).
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f).then_with(|| o.nodo.cmp(&self.nodo))
    }
}
impl PartialOrd for Abierto {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Segundos simulados en que una demora medida se olvida a 1/e (D5): medio día de juego. Un
/// atasco viejo que nadie vuelve a medir queda en ≤ 1,2 veces lo libre en menos de dos días.
pub const OLVIDO: f64 = 2400.0;
/// Demora relativa máxima que se registra (medido / libre del tipo).
pub const DEMORA_MAX: f32 = 8.0;
/// Franjas del día para los tiempos medidos (D7): madrugada, mañana, tarde y noche.
pub const FRANJAS: u8 = 4;

/// Franja horaria de una hora del reloj.
pub fn franja(hora: f32) -> u8 {
    match hora.rem_euclid(24.0) {
        h if h < 6.0 => 0,
        h if h < 12.0 => 1,
        h if h < 18.0 => 2,
        _ => 3,
    }
}

/// Tiempos medidos en cada carril, guardados con la partida.
///
/// Se guarda la **demora relativa** (lo que tardó / lo que tarda ese mismo tipo de vehículo
/// con la calle libre), no los segundos: una calle que recorren las carretas a su paso normal
/// no queda cara para los jinetes (D6). Hay una medida por **franja horaria** (D7), y cada
/// una **se olvida** hacia lo libre con el tiempo ([`OLVIDO`], D5).
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Tiempos {
    /// (carril × 4 + franja) → (demora relativa, reloj de la última medida).
    #[serde(default)]
    pub demoras: BTreeMap<u64, (f32, f64)>,
    /// Foto de las demoras con que se eligen las rutas (se saca una vez por hora del reloj):
    /// una ruta pedida dos veces en la misma hora sale igual, esté o no en la caché (y una
    /// partida cargada elige como la que no se guardó). La hora (absoluta) de la foto.
    #[serde(default)]
    pub foto: BTreeMap<u64, (f32, f64)>,
    #[serde(default)]
    pub foto_hora: i64,
    /// Reloj y franja con que se consultan los costos (los pone quien planifica).
    #[serde(skip)]
    pub ahora: f64,
    #[serde(skip)]
    pub franja: u8,
}

/// Iguales si guardan lo mismo (el reloj y la franja de consulta no se guardan).
impl PartialEq for Tiempos {
    fn eq(&self, o: &Self) -> bool {
        (&self.demoras, &self.foto, self.foto_hora) == (&o.demoras, &o.foto, o.foto_hora)
    }
}

fn clave_demora(tramo: u32, sentido: u8, franja: u8) -> u64 {
    carril(tramo, sentido) as u64 * FRANJAS as u64 + franja.min(FRANJAS - 1) as u64
}

/// Demora con el olvido aplicado hasta `ahora`.
fn olvidada(r: f32, t: f64, ahora: f64) -> f32 {
    let k = (-((ahora - t).max(0.0) / OLVIDO)).exp() as f32;
    1.0 + (r - 1.0) * k
}

impl Tiempos {
    /// Tiempo libre de un carril a la velocidad de referencia de la calle (para el costo).
    pub fn libre(net: &RoadNetwork, tramo: u32) -> f32 {
        let e = &net.edges[&tramo];
        e.len() / ref_speed(e.kind)
    }

    /// Demora relativa medida de un carril en una franja, con el olvido hasta `ahora` (1 =
    /// libre).
    pub fn demora(&self, tramo: u32, sentido: u8, franja: u8, ahora: f64) -> f32 {
        self.demoras.get(&clave_demora(tramo, sentido, franja)).map_or(1.0, |&(r, t)| olvidada(r, t, ahora)).max(1.0)
    }

    /// Saca la foto de las demoras para elegir rutas (una vez por hora del reloj).
    pub fn fotografiar(&mut self, hora_abs: i64) {
        self.foto = self.demoras.clone();
        self.foto_hora = hora_abs;
    }

    /// Tiempo esperado de un carril para elegir la ruta (de la foto, en la franja y el reloj
    /// de la consulta): lo libre por la demora, nunca menos que lo libre.
    pub fn costo(&self, net: &RoadNetwork, tramo: u32, sentido: u8) -> f32 {
        let d = self.foto.get(&clave_demora(tramo, sentido, self.franja)).map_or(1.0, |&(r, t)| olvidada(r, t, self.ahora)).max(1.0);
        Self::libre(net, tramo) * d
    }

    /// Registra cuánto tardó un vehículo en recorrer un carril completo: `libre_tipo` es lo
    /// que tarda ese tipo de vehículo con la calle libre; `reloj` y `hora`, cuándo.
    pub fn registrar(&mut self, net: &RoadNetwork, tramo: u32, sentido: u8, segundos: f32, libre_tipo: f32, reloj: f64, hora: f32) {
        if !net.edges.contains_key(&tramo) || libre_tipo <= 0.0 {
            return;
        }
        let k = clave_demora(tramo, sentido, franja(hora));
        let x = (segundos / libre_tipo).clamp(0.5, DEMORA_MAX);
        let viejo = self.demoras.get(&k).map_or(1.0, |&(r, t)| olvidada(r, t, reloj));
        self.demoras.insert(k, (viejo + (x - viejo) * 0.25, reloj));
    }

    /// Olvida los carriles de tramos que ya no existen.
    pub fn podar(&mut self, net: &RoadNetwork) {
        let existe = |k: &u64| net.edges.contains_key(&((k / (FRANJAS as u64 * 2)) as u32));
        self.demoras.retain(|k, _| existe(k));
        self.foto.retain(|k, _| existe(k));
    }

    /// Olvida las medidas que ya volvieron a lo libre (a menos de un 2 %): una vez por día.
    pub fn olvidar(&mut self, ahora: f64) {
        self.demoras.retain(|_, &mut (r, t)| (olvidada(r, t, ahora) - 1.0).abs() > 0.02);
    }
}

/// Buscador de rutas con caché (no se guarda: se rehace solo).
#[derive(Default)]
pub struct Router {
    cache: HashMap<(u32, u32, u32, u8), Option<Arc<Camino>>, FxBuild>,
    pub busquedas: u64,
    pub aciertos: u64,
}

impl Router {
    pub fn vaciar_cache(&mut self) {
        self.cache.clear();
    }

    /// Percepción personal: cada variante "ve" cada tramo entre 0,85 y 1,15 veces su costo.
    fn ruido(tramo: u32, percepcion: u32) -> f32 {
        let mut h = (tramo as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((percepcion as u64 + 1).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
        h ^= h >> 29;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 32;
        0.85 + 0.3 * ((h >> 40) as f32 / (1u64 << 24) as f32)
    }

    /// Ruta de `desde` a `hasta`. `percepcion`: `None` = peatón (el más corto); `Some(k)` =
    /// vehículo con la percepción `k` (el más rápido según lo medido, en la franja de
    /// `tiempos.franja`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn camino(&mut self, net: &RoadNetwork, tiempos: &Tiempos, desde: u32, hasta: u32, percepcion: Option<u32>) -> Option<Arc<Camino>> {
        self.camino_con(net, tiempos, desde, hasta, percepcion, None)
    }

    /// Como [`camino`](Self::camino), sin pasar por los tramos de `veto` (clase, tramos): los
    /// arreos no entran intramuros, los contrabandistas no pasan por los portones. La clase
    /// separa las rutas guardadas en la caché.
    pub fn camino_con(&mut self, net: &RoadNetwork, tiempos: &Tiempos, desde: u32, hasta: u32, percepcion: Option<u32>, veto: Option<(u32, &BTreeSet<u32>)>) -> Option<Arc<Camino>> {
        let modo = percepcion.map_or(0, |k| 1 + k % PERCEPCIONES) + veto.map_or(0, |v| 16 * (1 + v.0));
        let key = (desde, hasta, modo, if percepcion.is_some() { tiempos.franja } else { 0 });
        if let Some(c) = self.cache.get(&key) {
            self.aciertos += 1;
            return c.clone();
        }
        self.busquedas += 1;
        let r = Self::buscar(net, tiempos, desde, hasta, percepcion.map(|k| k % PERCEPCIONES), veto.map(|v| v.1)).map(Arc::new);
        if self.cache.len() > 50_000 {
            self.cache.clear();
        }
        self.cache.insert(key, r.clone());
        r
    }

    fn buscar(net: &RoadNetwork, tiempos: &Tiempos, desde: u32, hasta: u32, percepcion: Option<u32>, veto: Option<&BTreeSet<u32>>) -> Option<Camino> {
        net.nodes.get(&desde)?;
        let pb = *net.nodes.get(&hasta)?;
        if desde == hasta {
            return Some(Camino { tramos: Vec::new(), costo: 0.0 });
        }
        // Heurística admisible: distancia en línea recta (a pie) o a 4,5 m/s (más rápido que
        // cualquier vehículo de la época con la percepción más optimista).
        let h = |n: u32| {
            let d = net.nodes[&n].distance(pb);
            if percepcion.is_some() {
                d / 4.5
            } else {
                d
            }
        };
        let mut g: HashMap<u32, (f32, u32, u8), FxBuild> = HashMap::default();
        let mut open = BinaryHeap::new();
        g.insert(desde, (0.0, u32::MAX, 0));
        open.push(Abierto { f: h(desde), g: 0.0, nodo: desde });
        while let Some(Abierto { g: gn, nodo, .. }) = open.pop() {
            if gn > g[&nodo].0 {
                continue;
            }
            if nodo == hasta {
                let mut tramos = Vec::new();
                let mut n = hasta;
                while n != desde {
                    let (_, eid, dir) = g[&n];
                    tramos.push((eid, dir));
                    let e = &net.edges[&eid];
                    n = if dir == 0 { e.a } else { e.b };
                }
                tramos.reverse();
                return Some(Camino { tramos, costo: gn });
            }
            for &eid in &net.node_edges[&nodo] {
                let e = &net.edges[&eid];
                let (otro, dir) = if e.a == nodo { (e.b, 0u8) } else { (e.a, 1u8) };
                if otro == nodo || veto.is_some_and(|v| v.contains(&eid)) {
                    continue;
                }
                let c = match percepcion {
                    None => e.len(),
                    Some(k) => tiempos.costo(net, eid, dir) * Self::ruido(eid, k),
                };
                let ng = gn + c;
                if g.get(&otro).is_none_or(|x| ng < x.0 || (ng == x.0 && eid < x.1)) {
                    g.insert(otro, (ng, eid, dir));
                    open.push(Abierto { f: ng + h(otro), g: ng, nodo: otro });
                }
            }
        }
        None
    }
}
