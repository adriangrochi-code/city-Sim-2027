//! Búsqueda de rutas sobre la red de calles (porte de `scripts/agents/router.gd`).
//!
//! - Peatones: el camino más corto.
//! - Vehículos: el más rápido según el tiempo *medido* en cada carril (la congestión real),
//!   con una percepción personal distinta para cada vecino (±15 %). Así no todos eligen la
//!   misma calle y la gente evita los atascos que ya conoce.
//!
//! A* sobre los nodos de [`RoadNetwork`], con los tramos de cada nodo como vecinos (los
//! puentes son tramos como cualquier otro). Las rutas entre nodos se guardan en caché; la
//! caché se vacía cuando cambia la red y cada día (los tiempos medidos cambian despacio).

use super::super::roads::{RoadKind, RoadNetwork};
use crate::fxhash::FxBuild;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
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

/// Segundos medidos en cada carril (media móvil): parte del estado guardado de la partida.
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tiempos(pub BTreeMap<u32, f32>);

impl Tiempos {
    pub fn libre(net: &RoadNetwork, tramo: u32) -> f32 {
        let e = &net.edges[&tramo];
        e.len() / ref_speed(e.kind)
    }

    /// Tiempo esperado de un carril: el medido, nunca menos que el libre.
    pub fn costo(&self, net: &RoadNetwork, tramo: u32, sentido: u8) -> f32 {
        let libre = Self::libre(net, tramo);
        self.0.get(&carril(tramo, sentido)).copied().unwrap_or(libre).max(libre)
    }

    /// Registra cuánto tardó un vehículo en recorrer un carril completo.
    pub fn registrar(&mut self, net: &RoadNetwork, tramo: u32, sentido: u8, segundos: f32) {
        if !net.edges.contains_key(&tramo) {
            return;
        }
        let k = carril(tramo, sentido);
        let viejo = self.0.get(&k).copied().unwrap_or_else(|| Self::libre(net, tramo));
        self.0.insert(k, viejo + (segundos - viejo) * 0.25);
    }

    /// Olvida los carriles de tramos que ya no existen.
    pub fn podar(&mut self, net: &RoadNetwork) {
        self.0.retain(|k, _| net.edges.contains_key(&(k / 2)));
    }
}

/// Buscador de rutas con caché (no se guarda: se rehace solo).
#[derive(Default)]
pub struct Router {
    cache: HashMap<(u32, u32, u32), Option<Arc<Camino>>, FxBuild>,
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
    /// vehículo con la percepción `k` (el más rápido según lo medido).
    pub fn camino(&mut self, net: &RoadNetwork, tiempos: &Tiempos, desde: u32, hasta: u32, percepcion: Option<u32>) -> Option<Arc<Camino>> {
        let modo = percepcion.map_or(0, |k| 1 + k % PERCEPCIONES);
        let key = (desde, hasta, modo);
        if let Some(c) = self.cache.get(&key) {
            self.aciertos += 1;
            return c.clone();
        }
        self.busquedas += 1;
        let r = Self::buscar(net, tiempos, desde, hasta, percepcion.map(|k| k % PERCEPCIONES)).map(Arc::new);
        if self.cache.len() > 50_000 {
            self.cache.clear();
        }
        self.cache.insert(key, r.clone());
        r
    }

    fn buscar(net: &RoadNetwork, tiempos: &Tiempos, desde: u32, hasta: u32, percepcion: Option<u32>) -> Option<Camino> {
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
                if otro == nodo {
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
