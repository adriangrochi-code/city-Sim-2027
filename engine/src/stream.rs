//! Mundo en streaming: genera y malla columnas de chunks alrededor de la cámara en hilos de
//! trabajo, de la más cercana a la más lejana, y las entrega de a poco para subirlas a la GPU.
//!
//! Cada trabajo es una *columna* de chunks de un nivel de detalle. Como el terreno es una
//! función y las ediciones están en el [`EditStore`], cada columna se genera sola, sin
//! depender de que las vecinas existan: el borde de un vóxel que necesita el mallado se
//! calcula directamente. Fuera del anillo del nivel se considera aire, igual que antes: así
//! los bordes entre niveles generan "faldones" que tapan grietas.

use crate::lod::{choose_first_level, level_columns_from, LevelSpec, CHUNK};
use crate::mesher::{self, Mesh};
use crate::palette::*;
use crate::store::EditStore;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2, Vec3};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtOrd};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::JoinHandle;

const N: i32 = CHUNK;
const Q: i32 = N + 2;

pub type Key = (u32, IVec2);

/// Tope de columnas terminadas esperando a ser subidas: los hilos de trabajo se detienen si
/// el render no alcanza a consumirlas (pipeline con trabajo en vuelo acotado, como el
/// `tbb::parallel_pipeline` de Tisma engine en src/gcode/writer.cpp). Así la memoria no crece
/// sin límite cuando se generan columnas más rápido de lo que se suben.
pub const MAX_READY: usize = 96;
/// Tope de seguridad de columnas retiradas que siguen en la GPU esperando que su zona quede
/// cubierta (las tablas de la GPU tienen 65.536 lugares; lo normal son unos pocos cientos).
const RETIRED_SAFETY: usize = 24_000;

type FxMap<K, V> = HashMap<K, V, crate::fxhash::FxBuild>;
type FxSet<K> = HashSet<K, crate::fxhash::FxBuild>;

/// Contadores de tiempo de CPU (ns) para el banco de pruebas: generación del terreno y
/// armado de las copias con borde, y mallado.
pub static GEN_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static HEIGHT_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static SURF_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static CHUNKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static MESH_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Job {
    prio: f32,
    key: Key,
    version: u32,
    ring: Arc<HashSet<IVec2>>,
}
impl PartialEq for Job {
    fn eq(&self, o: &Self) -> bool {
        self.prio == o.prio
    }
}
impl Eq for Job {}
impl PartialOrd for Job {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Job {
    // BinaryHeap es de máximos: invertimos para sacar primero la prioridad más baja (más cerca).
    fn cmp(&self, o: &Self) -> Ordering {
        o.prio.partial_cmp(&self.prio).unwrap_or(Ordering::Equal)
    }
}

pub struct ColumnResult {
    pub key: Key,
    pub version: u32,
    pub prio: f32,
    pub mesh: Mesh,
}

struct Shared {
    queue: Mutex<BinaryHeap<Job>>,
    cv: Condvar,
    quit: AtomicBool,
    in_flight: AtomicUsize,
}

/// Cambios para el renderer.
pub enum Op {
    Set(Key, Mesh),
    Remove(Key),
}

#[derive(Default, Clone, Copy)]
pub struct Stats {
    pub columns_built: usize,
    pub uploads: usize,
}

pub struct Streamer {
    pub store: Arc<RwLock<EditStore>>,
    pub specs: Vec<LevelSpec>,
    sets: Vec<Arc<HashSet<IVec2>>>,
    regions: Vec<(IVec2, IVec2)>,
    versions: FxMap<Key, u32>,
    ready: FxMap<Key, u32>,
    uploads: Vec<ColumnResult>,
    retired: FxSet<Key>,
    next_version: u32,
    center: Vec2,
    /// Nivel más fino que se genera (LOD por tamaño en pantalla).
    pub first: u32,
    shared: Arc<Shared>,
    rx: Receiver<ColumnResult>,
    workers: Vec<JoinHandle<()>>,
    pub stats: Stats,
}

impl Streamer {
    pub fn new(terrain: Arc<TerrainGen>, store: Arc<RwLock<EditStore>>, levels: u32, threads: usize) -> Streamer {
        let specs: Vec<LevelSpec> = (0..levels).map(|lod| LevelSpec { lod }).collect();
        let shared = Arc::new(Shared { queue: Mutex::new(BinaryHeap::new()), cv: Condvar::new(), quit: AtomicBool::new(false), in_flight: AtomicUsize::new(0) });
        let (tx, rx) = sync_channel(MAX_READY);
        let mut workers = Vec::new();
        for _ in 0..threads.max(1) {
            let (sh, tx, t, s, specs): (Arc<Shared>, SyncSender<ColumnResult>, Arc<TerrainGen>, Arc<RwLock<EditStore>>, Vec<LevelSpec>) = (shared.clone(), tx.clone(), terrain.clone(), store.clone(), specs.clone());
            workers.push(std::thread::spawn(move || worker(sh, tx, t, s, specs)));
        }
        Streamer {
            store,
            specs,
            sets: Vec::new(),
            regions: Vec::new(),
            versions: FxMap::default(),
            ready: FxMap::default(),
            uploads: Vec::new(),
            retired: FxSet::default(),
            next_version: 1,
            center: Vec2::ZERO,
            first: 0,
            shared,
            rx,
            workers,
            stats: Stats::default(),
        }
    }

    fn bump(&mut self, key: Key) {
        self.versions.insert(key, self.next_version);
        self.next_version += 1;
    }

    fn prio(&self, key: Key) -> f32 {
        let cm = self.specs[key.0 as usize].chunk_m();
        let c = Vec2::new((key.1.x as f32 + 0.5) * cm, (key.1.y as f32 + 0.5) * cm);
        c.distance(self.center) + key.0 as f32
    }

    /// Recentra los anillos y elige el nivel más fino según la altura de la cámara sobre el
    /// terreno, para no generar detalle que mediría menos de medio píxel.
    pub fn update_view(&mut self, center: Vec2, eye_height: f32, fov_y: f32, view_h: f32) {
        let first = choose_first_level(self.first, eye_height, fov_y, view_h, self.specs.len() as u32);
        self.update_with(center, first);
    }

    /// Centro de los anillos de detalle y radio del círculo que el último anillo cubre seguro
    /// (m): el anillo es un cuadrado de [`crate::lod::RING_CHUNKS`] chunks alineado de a dos,
    /// así el centro puede quedar hasta un chunk corrido.
    pub fn reach(&self) -> (Vec2, f32) {
        let last = self.specs.last().map_or(0.0, |s| s.chunk_m());
        (self.center, (crate::lod::RING_CHUNKS / 2 - 1) as f32 * last)
    }

    /// Recentra los anillos de detalle si la cámara se movió lo suficiente (todos los niveles).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn update(&mut self, center: Vec2) {
        self.update_with(center, 0);
    }

    fn update_with(&mut self, center: Vec2, first: u32) {
        self.center = center;
        let mut regions: Vec<(IVec2, IVec2)> = self.specs.iter().map(|s| s.region(center)).collect();
        regions.push((IVec2::splat(first as i32), IVec2::ZERO));
        if regions == self.regions {
            return;
        }
        self.first = first;
        let cols = level_columns_from(&self.specs, center, first);
        let mut new_sets = Vec::new();
        for (li, list) in cols.into_iter().enumerate() {
            let lod = li as u32;
            let new: HashSet<IVec2> = list.into_iter().collect();
            let old: HashSet<IVec2> = self.sets.get(li).map(|s| (**s).clone()).unwrap_or_default();
            let added: Vec<IVec2> = new.difference(&old).copied().collect();
            let removed: Vec<IVec2> = old.difference(&new).copied().collect();
            for c in &removed {
                self.versions.remove(&(lod, *c));
                if self.ready.contains_key(&(lod, *c)) {
                    self.retired.insert((lod, *c));
                }
            }
            // Las columnas que siguen y tocan el borde cambian de faldón: hay que re-mallarlas.
            let mut border = HashSet::new();
            for c in added.iter().chain(removed.iter()) {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let n = *c + IVec2::new(dx, dz);
                        if new.contains(&n) && old.contains(&n) {
                            border.insert(n);
                        }
                    }
                }
            }
            for c in added {
                self.retired.remove(&(lod, c));
                self.bump((lod, c));
            }
            for c in border {
                self.bump((lod, c));
            }
            new_sets.push(Arc::new(new));
        }
        self.sets = new_sets;
        self.regions = regions;
        self.requeue_all();
    }

    /// Rehace la cola con todo lo que falta, ordenado por cercanía a la cámara actual.
    fn requeue_all(&mut self) {
        let mut jobs = BinaryHeap::new();
        for (&key, &v) in &self.versions {
            if self.ready.get(&key) != Some(&v) {
                jobs.push(Job { prio: self.prio(key), key, version: v, ring: self.sets[key.0 as usize].clone() });
            }
        }
        *self.shared.queue.lock().unwrap() = jobs;
        self.shared.cv.notify_all();
    }

    fn enqueue(&mut self, keys: &[Key], urgent: bool) {
        let mut q = self.shared.queue.lock().unwrap();
        for &key in keys {
            if let Some(&v) = self.versions.get(&key) {
                let prio = if urgent { -1.0 + key.0 as f32 * 0.01 } else { self.prio(key) };
                q.push(Job { prio, key, version: v, ring: self.sets[key.0 as usize].clone() });
            }
        }
        drop(q);
        self.shared.cv.notify_all();
    }

    /// Aplica ediciones y re-malla solo las columnas tocadas (en todos los niveles).
    /// Devuelve cuántas columnas se volvieron a encolar.
    pub fn edit(&mut self, edits: &crate::buildings::Edits) -> Vec<Key> {
        let touched = self.store.write().unwrap().apply(edits, true);
        self.requeue_touched(touched, true)
    }

    fn requeue_touched(&mut self, touched: Vec<HashSet<IVec2>>, urgent: bool) -> Vec<Key> {
        let mut keys = Vec::new();
        for (li, set) in touched.into_iter().enumerate() {
            for c in set {
                let key = (li as u32, c);
                if self.versions.contains_key(&key) {
                    keys.push(key);
                }
            }
        }
        for &k in &keys {
            self.bump(k);
        }
        self.enqueue(&keys, urgent);
        keys
    }

    /// Reemplaza todas las ediciones (por ejemplo, cuando termina de cargarse la escena) y
    /// regenera solo las columnas que tienen ediciones.
    pub fn replace_store(&mut self, store: EditStore) -> usize {
        let mut touched: Vec<HashSet<IVec2>> = vec![HashSet::new(); self.specs.len()];
        for (li, l) in store.levels.iter().enumerate() {
            let set = &mut touched[li];
            let mut add = |c: IVec2| {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        set.insert(c + IVec2::new(dx, dz));
                    }
                }
            };
            for c in l.edited_columns() {
                add(c);
            }
            let cols: HashSet<IVec2> = l.surface.keys().map(|c| IVec2::new(c.x >> 5, c.y >> 5)).collect();
            for c in cols {
                add(c);
            }
        }
        *self.store.write().unwrap() = store;
        self.requeue_touched(touched, false).len()
    }

    /// Recibe columnas terminadas y devuelve cambios para la GPU, con un presupuesto de
    /// vértices por cuadro (para no trabar la imagen).
    pub fn poll(&mut self, vertex_budget: usize) -> Vec<Op> {
        while let Ok(r) = self.rx.try_recv() {
            self.stats.columns_built += 1;
            if self.versions.get(&r.key) == Some(&r.version) {
                self.uploads.push(r);
            }
        }
        // Lo más cercano primero.
        self.uploads.sort_by(|a, b| b.prio.partial_cmp(&a.prio).unwrap_or(Ordering::Equal));
        let mut ops = Vec::new();
        let mut used = 0usize;
        while let Some(r) = self.uploads.pop() {
            if self.versions.get(&r.key) != Some(&r.version) {
                continue;
            }
            used += r.mesh.quad_count().max(1) * 4;
            self.ready.insert(r.key, r.version);
            self.stats.uploads += 1;
            ops.push(Op::Set(r.key, r.mesh));
            if used >= vertex_budget {
                break;
            }
        }
        // Lo que quedó fuera de los anillos se borra apenas su zona está cubierta por columnas
        // nuevas ya subidas (antes se esperaba a que no quedara nada pendiente, y al moverse
        // rápido se veían mallas de dos niveles superpuestas).
        //
        // Nunca antes: una columna se quita solo cuando las que la reemplazan ya están en la
        // GPU. Antes se borraba todo lo retirado (1) si el streamer "parecía" ocioso, pero un
        // hilo entrega la malla al canal *antes* de bajar `in_flight`, así que la columna que
        // cubría el lugar podía estar todavía en el canal, y (2) si se juntaban más de 3.000
        // retiradas (viajes o zoom rápidos). En los dos casos quedaban huecos rectangulares
        // por un cuadro o más (se veía el plano del río debajo). Solo queda un tope de
        // seguridad muy alto para no agotar las tablas de la GPU.
        if !self.retired.is_empty() {
            let flush = self.retired.len() > RETIRED_SAFETY;
            let done: Vec<Key> = self.retired.iter().copied().filter(|k| flush || self.covered(*k)).collect();
            for key in done {
                self.retired.remove(&key);
                self.ready.remove(&key);
                ops.push(Op::Remove(key));
            }
        }
        ops
    }

    /// ¿La zona de esta columna (que ya no se quiere) está cubierta por columnas de otros
    /// niveles que ya están en la GPU con su versión al día?
    fn covered(&self, key: Key) -> bool {
        let (l, c) = (key.0 as i32, key.1);
        // Todos los niveles (antes solo ±3): un zoom rápido puede saltar el nivel más fino de
        // 0 a 5 y la columna retirada la cubre un nivel mucho más grueso (o más fino).
        for k in 0..self.specs.len() as i32 {
            if k == l {
                continue;
            }
            if k < l {
                let f = 1i32 << (l - k);
                // Hijas en el nivel fino: si son muchas, se recorre lo que ese nivel quiere.
                match self.sets.get(k as usize) {
                    Some(set) if ((f * f) as usize) > set.len() => {
                        for &cc in set.iter() {
                            if IVec2::new(cc.x.div_euclid(f), cc.y.div_euclid(f)) == c && !self.is_current((k as u32, cc)) {
                                return false;
                            }
                        }
                    }
                    _ => {
                        for dz in 0..f {
                            for dx in 0..f {
                                let kk = (k as u32, c * f + IVec2::new(dx, dz));
                                if !self.is_current(kk) {
                                    return false;
                                }
                            }
                        }
                    }
                }
            } else {
                let f = 1 << (k - l);
                let kk = (k as u32, IVec2::new(c.x.div_euclid(f), c.y.div_euclid(f)));
                if !self.is_current(kk) {
                    return false;
                }
            }
        }
        true
    }

    /// Una columna que no se quiere no bloquea; una que se quiere tiene que estar subida.
    fn is_current(&self, key: Key) -> bool {
        match self.versions.get(&key) {
            None => true,
            Some(v) => self.ready.get(&key) == Some(v),
        }
    }

    fn idle_now(&self) -> bool {
        self.uploads.is_empty() && self.shared.in_flight.load(AtOrd::SeqCst) == 0 && self.shared.queue.lock().unwrap().is_empty()
    }

    /// ¿Está todo generado y entregado para la posición actual?
    pub fn idle(&mut self) -> bool {
        if !self.idle_now() {
            return false;
        }
        // Un trabajo pudo terminar justo entre la última lectura del canal y esta comprobación.
        if let Ok(r) = self.rx.try_recv() {
            self.stats.columns_built += 1;
            if self.versions.get(&r.key) == Some(&r.version) {
                self.uploads.push(r);
            }
            return false;
        }
        self.retired.is_empty()
    }

    pub fn pending(&self) -> usize {
        self.shared.queue.lock().unwrap().len() + self.shared.in_flight.load(AtOrd::SeqCst) + self.uploads.len()
    }

    pub fn wanted_columns(&self) -> usize {
        self.versions.len()
    }
}

impl Drop for Streamer {
    fn drop(&mut self) {
        self.shared.quit.store(true, AtOrd::SeqCst);
        self.shared.cv.notify_all();
        for w in self.workers.drain(..) {
            // Un hilo puede estar trabado entregando una columna con el canal lleno: se vacía
            // el canal mientras termina (si no, cerrar una partida colgaría el juego).
            while !w.is_finished() {
                while self.rx.try_recv().is_ok() {}
                std::thread::yield_now();
            }
            let _ = w.join();
        }
    }
}

fn worker(sh: Arc<Shared>, tx: SyncSender<ColumnResult>, terrain: Arc<TerrainGen>, store: Arc<RwLock<EditStore>>, specs: Vec<LevelSpec>) {
    loop {
        let job = {
            let mut q = sh.queue.lock().unwrap();
            loop {
                if sh.quit.load(AtOrd::SeqCst) {
                    return;
                }
                if let Some(j) = q.pop() {
                    sh.in_flight.fetch_add(1, AtOrd::SeqCst);
                    break j;
                }
                q = sh.cv.wait(q).unwrap();
            }
        };
        let mesh = {
            let s = store.read().unwrap();
            build_column(&terrain, &s, specs[job.key.0 as usize], job.key.1, &job.ring)
        };
        let _ = tx.send(ColumnResult { key: job.key, version: job.version, prio: job.prio, mesh });
        sh.in_flight.fetch_sub(1, AtOrd::SeqCst);
    }
}

/// Índice del vóxel de superficie del terreno a partir de la altura (en metros).
#[inline]
pub fn top_from_height(h: f32, vs: f32) -> i32 {
    // Bajo el agua, la cara superior del vóxel no debe asomar sobre el nivel del río.
    if h < 0.0 {
        (h / vs - 1.0).floor() as i32
    } else {
        // En tierra firme la cara de arriba queda por lo menos un vóxel sobre el nivel del mar:
        // si no, en los niveles gruesos (vóxel de 4 a 64 m) toda la tierra baja (bañados,
        // costas, planicies de hasta medio vóxel de alto) se redondea al nivel del mar y el
        // plano del río (con la marea encima) la tapa, y cambia al mover la cámara.
        ((h / vs - 0.5).floor() as i32).max(0)
    }
}

/// Igual que [`top_from_height`] en los niveles gruesos, cuya malla se dibuja `drop` metros
/// más abajo (ver `lod::far_drop`): la cara de arriba del vóxel `k` queda en `(k + 1)·vs −
/// drop`. Se elige el vóxel cuya cara queda más cerca de la altura; la tierra firme, por lo
/// menos en el vóxel 0 (3 m sobre el agua), y el fondo, bajo el agua.
#[inline]
pub fn top_from_height_far(h: f32, vs: f32, drop: f32) -> i32 {
    let k = ((h + drop) / vs).round() as i32 - 1;
    if h < 0.0 {
        k.min(((drop / vs).floor() as i32) - 1)
    } else {
        k.max(0)
    }
}

#[inline]
fn layer(depth: i32, soil: i32, surf: Mat) -> Mat {
    if depth == 0 {
        surf
    } else if depth <= soil {
        match surf {
            SAND | MUD => surf,
            SAND_WET | DUNE => SAND,
            STREAM | MARSH => MUD,
            ROCK | COAST_ROCK => ROCK,
            _ => DIRT,
        }
    } else {
        ROCK
    }
}

/// Un árbol ya llevado a vóxeles del nivel: tronco y ramas (cilindros que se afinan) y una
/// copa de varias masas de follaje (ver [`crate::trees`]).
struct VoxTree {
    /// Caja del árbol en vóxeles (inclusive).
    lo: IVec3,
    hi: IVec3,
    /// Tramos de tronco y rama: puntas (vóxeles) y radios (vóxeles).
    limbs: Vec<(Vec3, Vec3, f32, f32)>,
    /// Masas de follaje: centro (vóxeles), 1/rh², 1/rv² y si cuelga (sauce).
    blobs: Vec<(Vec3, f32, f32, bool)>,
    top: i32,
    leaf: Mat,
    bark: Mat,
    holes: u32,
    flower: Option<(Mat, u32)>,
    seed: u32,
    /// Largo de las hebras de las ramas colgantes (vóxeles del nivel, unos 1,5 m).
    strand: i32,
    /// Racimos de hojas: 1 / tamaño del racimo (en vóxeles del nivel) y cuánto pesan (0 en los
    /// niveles gruesos, donde un racimo sería menos de un vóxel y solo haría ruido).
    clump_k: f32,
    clump_amp: f32,
}

/// Ruido de valor 3D (0..1) en una grilla entera, para los racimos de hojas.
fn vnoise3(p: Vec3, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let h = |x: i32, y: i32, z: i32| {
        let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xcb1a_b31f) ^ (z as u32).wrapping_mul(0xd816_3841) ^ seed;
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1_e995);
        h ^= h >> 15;
        (h & 0xffff) as f32 / 65535.0
    };
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = l(h(x, y, z), h(x + 1, y, z), u.x);
    let x10 = l(h(x, y + 1, z), h(x + 1, y + 1, z), u.x);
    let x01 = l(h(x, y, z + 1), h(x + 1, y, z + 1), u.x);
    let x11 = l(h(x, y + 1, z + 1), h(x + 1, y + 1, z + 1), u.x);
    l(l(x00, x10, u.y), l(x01, x11, u.y), u.z)
}

/// Hojas sueltas en la superficie de las copas: de cada cara de follaje que da al aire (arriba
/// o a los costados) asoma, a veces, una hoja o un par de hojas de 0,125 m (micro-vóxeles,
/// ver `trees::Micro`), con su propio tono. Así el contorno de la copa deja de ser una pared de
/// cubos de 0,25 m. `lo`: vóxel global de la esquina de la copia con borde (`pad`).
fn leaf_fuzz(pad: &[Mat], lo: IVec3, vs: f32, top: &[i32], out: &mut Vec<crate::trees::Micro>) {
    const MICRO: f32 = crate::trees::MICRO;
    let k = (vs / MICRO).round() as i32;
    let is_leaf = |m: Mat| matches!(m, LEAF | LEAF_DARK | LEAF_OMBU | LEAF_SAUCE | LEAF_ESPINILLO | LEAF_TALA | CEIBO_FLOR);
    let at = |i: i32, j: i32, y: i32| pad[(i + j * Q + y * Q * Q) as usize];
    // Caras: (dx, dy, dz).
    const FACES: [(i32, i32, i32); 5] = [(0, 1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)];
    for y in 1..=N {
        for j in 1..=N {
            for i in 1..=N {
                let m = at(i, j, y);
                if !is_leaf(m) {
                    continue;
                }
                let g = lo + IVec3::new(i, y, j);
                let ground = (top[(i + j * Q) as usize] + 1) as f32 * vs;
                for (f, &(dx, dy, dz)) in FACES.iter().enumerate() {
                    if at(i + dx, j + dz, y + dy) != AIR {
                        continue;
                    }
                    let h = hash3(g.x, g.y * 7 + f as i32, g.z);
                    if h % 100 >= 30 {
                        continue;
                    }
                    // Posición en la grilla de micro-vóxeles, justo afuera de la cara.
                    let (sx, sy, sz) = (((h >> 8) & 1) as i32, ((h >> 9) & 1) as i32, ((h >> 10) & 1) as i32);
                    let (mx, my, mz) = match (dx, dy, dz) {
                        (0, 1, 0) => (g.x * k + sx * (k - 1), (g.y + 1) * k, g.z * k + sz * (k - 1)),
                        (1, 0, 0) => ((g.x + 1) * k, g.y * k + sy * (k - 1), g.z * k + sz * (k - 1)),
                        (-1, 0, 0) => (g.x * k - 1, g.y * k + sy * (k - 1), g.z * k + sz * (k - 1)),
                        (0, 0, 1) => (g.x * k + sx * (k - 1), g.y * k + sy * (k - 1), (g.z + 1) * k),
                        _ => (g.x * k + sx * (k - 1), g.y * k + sy * (k - 1), g.z * k - 1),
                    };
                    let y_m = my as f32 * MICRO;
                    let above = ((y_m - ground) / MICRO).clamp(0.0, 255.0) as u8;
                    // A veces dos hojas en fila (una ramita).
                    let size = if (h >> 12) % 4 == 0 { if dy == 1 { [1, 2, 1, 0] } else { [1, 1, 2, 0] } } else { [1, 1, 1, 0] };
                    out.push(crate::trees::Micro {
                        pos: [mx as f32 * MICRO, y_m, mz as f32 * MICRO],
                        size,
                        // Material, matiz, altura sobre el suelo, y 1 = hoja (se mece poco y se
                        // achica en el lugar al alejarse).
                        info: [m, ((h >> 16) & 0xff) as u8, above, 1],
                    });
                }
            }
        }
    }
}

#[inline]
fn hash3(x: i32, y: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xcb1a_b31f) ^ (z as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    h
}

/// Niveles con árboles: hasta 4 m por vóxel (más lejos, una copa sería un solo vóxel).
const TREE_MAX_VOXEL: f32 = 4.0;

fn hash_col(x: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0x9e37_79b9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65536.0
}

/// Monte de lejos: en los niveles sin árboles sueltos, cuántos vóxeles de copa tiene cada
/// columna de la copia con borde (0 = ninguno) y de qué follaje. Las copas cubren la fracción
/// del suelo que da la densidad del monte; su altura media es la del monte (5,5–9 m: sauces,
/// canelones y talas de la galería), repartida al azar entre 0 y 1 vóxel cuando el vóxel es
/// más grande que el monte, así en promedio la masa tiene la altura justa.
fn forest_canopy(t: &TerrainGen, spec: LevelSpec, base: IVec2, top: &[i32], surf: &[Mat], natural: &[bool]) -> (Vec<i32>, Vec<Mat>) {
    let n = (Q * Q) as usize;
    let mut c = vec![0i32; n];
    let mut m = vec![AIR; n];
    let vs = spec.voxel();
    if vs <= TREE_MAX_VOXEL || !t.is_real() {
        return (c, m);
    }
    for j in 0..Q {
        for i in 0..Q {
            let k = (i + j * Q) as usize;
            if !natural[k] || top[k] == i32::MIN || !matches!(surf[k], FOREST_FLOOR | GRASS_LUSH | GRASS | GRASS_DRY) {
                continue;
            }
            if (top[k] as f32 + 1.0) * vs < 1.2 {
                continue;
            }
            let (gx, gz) = (base.x + i - 1, base.y + j - 1);
            let (x, z) = ((gx as f32 + 0.5) * vs, (gz as f32 + 0.5) * vs);
            let dens = t.forest_density(x, z);
            if dens < 0.25 || dens < 0.3 + 0.45 * hash_col(gx, gz, 1) {
                continue;
            }
            let h = (5.5 + 3.5 * dens) * (0.85 + 0.3 * hash_col(gx, gz, 2));
            c[k] = (h / vs + hash_col(gx, gz, 3)).floor() as i32;
            let u = hash_col(gx, gz, 4);
            m[k] = if u < 0.45 { LEAF } else if u < 0.75 { LEAF_DARK } else if u < 0.92 { LEAF_TALA } else { LEAF_SAUCE };
        }
    }
    (c, m)
}

/// Árboles cuyo tronco o copa caen en la columna, salvo donde hay obras (calles, plaza o
/// edificios): ahí el monte se taló.
fn column_trees(t: &TerrainGen, ls: &crate::store::LevelStore, spec: LevelSpec, col: IVec2) -> Vec<VoxTree> {
    let mut out = Vec::new();
    if spec.voxel() > TREE_MAX_VOXEL || !t.is_real() {
        return out;
    }
    let vs = spec.voxel();
    let r = TerrainGen::MAX_TREE_RADIUS;
    let (x0, z0) = (col.x as f32 * N as f32 * vs, col.y as f32 * N as f32 * vs);
    let (x1, z1) = (x0 + N as f32 * vs, z0 + N as f32 * vs);
    let mut trees = Vec::new();
    t.trees_in(x0 - r - vs, z0 - r - vs, x1 + r + vs, z1 + r + vs, &mut trees);
    for tr in trees {
        let (gx, gz) = ((tr.x / vs).floor() as i32, (tr.z / vs).floor() as i32);
        let shape = crate::trees::shape(&tr.spec());
        let (reach, height) = crate::trees::extent(&shape);
        // ¿La copa alcanza la columna? (descarte antes de mirar las obras)
        if tr.x + reach < x0 - vs || tr.x - reach > x1 + vs || tr.z + reach < z0 - vs || tr.z - reach > z1 + vs {
            continue;
        }
        // ¿Obras cerca del tronco? (superficie pintada o vóxeles editados en el chunk).
        // Si la copa toca una obra (calle, plaza), el árbol se taló: se mira el tronco y ocho
        // puntos alrededor, a la distancia del radio de la copa.
        let rr = ((tr.radius + 1.0) / vs).ceil() as i32;
        let cleared = std::iter::once((0, 0))
            .chain((0..8).map(|k| {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                ((a.cos() * rr as f32) as i32, (a.sin() * rr as f32) as i32)
            }))
            .any(|(dx, dz)| ls.surface.contains_key(&IVec2::new(gx + dx, gz + dz)) || ls.ground.contains_key(&IVec2::new(gx + dx, gz + dz)));
        if cleared {
            continue;
        }
        let ground = top_from_height(t.height(tr.x, tr.z), vs) + 1;
        let ck = IVec2::new(gx >> 5, gz >> 5);
        let cy = ground >> 5;
        if (-1..=1).any(|dy| ls.has_chunk(IVec3::new(ck.x, cy + dy, ck.y))) {
            continue;
        }
        // De metros (pie del tronco en el origen) a vóxeles del nivel. El pie se hunde medio
        // vóxel para que el tronco no flote en las laderas.
        let base = Vec3::new(tr.x / vs, ground as f32 - 0.5, tr.z / vs);
        let to_vox = |p: Vec3| base + p / vs;
        let mut limbs = Vec::with_capacity(shape.limbs.len());
        for l in &shape.limbs {
            // El tronco siempre tiene al menos un vóxel de grueso; las ramas finas, solo de cerca.
            let (r0, r1) = if l.main {
                ((l.r0 / vs).max(0.55), (l.r1 / vs).max(0.5))
            } else if vs <= 1.0 {
                ((l.r0 / vs).max(0.45), (l.r1 / vs).max(0.42))
            } else {
                continue;
            };
            limbs.push((to_vox(l.a), to_vox(l.b), r0, r1));
        }
        let blobs: Vec<(Vec3, f32, f32, bool)> = shape
            .blobs
            .iter()
            .map(|b| {
                let (rh, rv) = ((b.rh / vs).max(0.8), (b.rv / vs).max(0.7));
                (to_vox(b.c), 1.0 / (rh * rh), 1.0 / (rv * rv), b.droop)
            })
            .collect();
        let rv = reach / vs + 1.0;
        let top = (base.y + height / vs + 1.0).ceil() as i32;
        out.push(VoxTree {
            lo: IVec3::new((base.x - rv).floor() as i32, ground - 1, (base.z - rv).floor() as i32),
            hi: IVec3::new((base.x + rv).ceil() as i32, top, (base.z + rv).ceil() as i32),
            limbs,
            blobs,
            top,
            leaf: shape.leaf,
            bark: shape.bark,
            // De lejos (2 y 4 m por vóxel) los huecos del borde de la copa solo agregan caras.
            holes: if vs >= 2.0 { shape.holes / 4 } else { shape.holes },
            flower: shape.flower,
            seed: (gx as u32).wrapping_mul(73_856_093) ^ (gz as u32).wrapping_mul(19_349_663),
            strand: ((1.5 / vs).round() as i32).max(1),
            // Racimos de unos 0,8 m; se notan con vóxeles de 0,25 y 0,5 m, no más lejos.
            clump_k: vs / 0.8,
            clump_amp: (1.0 - (vs - 0.5).max(0.0) / 1.0).clamp(0.0, 1.0),
        });
    }
    out
}

/// ¿El punto `p` está dentro del tramo de tronco (a, b, r0, r1)?
#[inline]
fn in_limb(p: Vec3, l: &(Vec3, Vec3, f32, f32)) -> bool {
    let ab = l.1 - l.0;
    let t = ((p - l.0).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    let r = l.2 + (l.3 - l.2) * t;
    (p - (l.0 + ab * t)).length_squared() <= r * r
}

/// Escribe un árbol en el bloque `pad` (Q³ desde `lo`); solo ocupa aire y columnas presentes.
/// Devuelve los vóxeles puestos.
fn raster_tree(tr: &VoxTree, lo: IVec3, top: &[i32], pad: &mut [Mat]) -> usize {
    let mut n = 0;
    let (u0, u1) = ((tr.lo.x - lo.x).max(0), (tr.hi.x - lo.x).min(Q - 1));
    let (w0, w1) = ((tr.lo.z - lo.z).max(0), (tr.hi.z - lo.z).min(Q - 1));
    let (y0, y1) = ((tr.lo.y - lo.y).max(0), (tr.top - lo.y).min(Q - 1));
    if u0 > u1 || w0 > w1 || y0 > y1 {
        return 0;
    }
    for w in w0..=w1 {
        for u in u0..=u1 {
            if top[(u + w * Q) as usize] == i32::MIN {
                continue;
            }
            let (gx, gz) = (u + lo.x, w + lo.z);
            let col_h = tr.seed ^ (gx as u32).wrapping_mul(2_654_435_761) ^ (gz as u32).wrapping_mul(97_331);
            for y in y0..=y1 {
                let gy = y + lo.y;
                let k = (u + w * Q + y * Q * Q) as usize;
                if pad[k] != AIR {
                    continue;
                }
                let p = Vec3::new(gx as f32 + 0.5, gy as f32 + 0.5, gz as f32 + 0.5);
                let m = if tr.limbs.iter().any(|l| in_limb(p, l)) {
                    tr.bark
                } else {
                    // La masa de follaje más "adentro" decide.
                    let mut best = f32::MAX;
                    let mut droop = false;
                    for b in &tr.blobs {
                        let d = p - b.0;
                        // Las ramas colgantes alargan la masa hacia abajo.
                        let iv = if b.3 && d.y < 0.0 { b.2 * 0.45 } else { b.2 };
                        let e = (d.x * d.x + d.z * d.z) * b.1 + d.y * d.y * iv;
                        if e < best {
                            best = e;
                            droop = b.3;
                        }
                    }
                    // Racimos de hojas: el contorno de la copa se abulta y se hunde según un
                    // ruido de ~0,8 m, y entre racimo y racimo quedan huecos por donde pasa la
                    // luz (no una cáscara lisa con agujeros sueltos).
                    if tr.clump_amp > 0.0 {
                        let c = vnoise3(p * tr.clump_k, tr.seed);
                        best += (0.5 - c) * 0.7 * tr.clump_amp;
                        if best > 0.4 && c < 0.34 * tr.clump_amp {
                            continue;
                        }
                    }
                    if best > 1.0 {
                        continue;
                    }
                    // Borde de la copa irregular: algunos vóxeles del contorno faltan. Las ramas
                    // colgantes se cortan por columnas (hebras verticales).
                    let hh = if droop {
                        (col_h ^ (gy.div_euclid(tr.strand) as u32).wrapping_mul(40_503)) % 100
                    } else {
                        (col_h ^ (gy as u32).wrapping_mul(40_503)) % 100
                    };
                    let inner = if droop { 0.35 } else { 0.55 };
                    if best >= inner && hh < tr.holes {
                        continue;
                    }
                    match tr.flower {
                        Some((f, pct)) if best > 0.5 && (hh * 7 + 13) % 100 < pct => f,
                        _ => tr.leaf,
                    }
                };
                pad[k] = m;
                n += 1;
            }
        }
    }
    n
}

/// Genera y malla una columna de chunks completa de un nivel.
pub fn build_column(t: &TerrainGen, store: &EditStore, spec: LevelSpec, col: IVec2, ring: &HashSet<IVec2>) -> Mesh {
    let t_start = std::time::Instant::now();
    let mut mesh_ns = 0u64;
    let out = build_column_inner(t, store, spec, col, ring, &mut mesh_ns);
    let total = t_start.elapsed().as_nanos() as u64;
    GEN_NS.fetch_add(total.saturating_sub(mesh_ns), AtOrd::Relaxed);
    MESH_NS.fetch_add(mesh_ns, AtOrd::Relaxed);
    out
}

fn build_column_inner(t: &TerrainGen, store: &EditStore, spec: LevelSpec, col: IVec2, ring: &HashSet<IVec2>, mesh_ns: &mut u64) -> Mesh {
    let vs = spec.voxel();
    // Niveles gruesos (entorno amplio): la malla baja `drop` metros (ver `lod::far_drop`).
    let drop = crate::lod::far_drop(spec.lod);
    let ls = &store.levels[spec.lod as usize];
    let p = N + 4;
    let base = col * N;
    // Alturas con dos vóxeles de margen (uno para el borde y otro para la pendiente).
    let th = std::time::Instant::now();
    let mut hs = vec![0f32; (p * p) as usize];
    let coast: Vec<f32> = (0..p).map(|i| t.coast_z(((base.x + i - 2) as f32 + 0.5) * vs)).collect();
    for j in 0..p {
        for i in 0..p {
            let x = ((base.x + i - 2) as f32 + 0.5) * vs;
            let z = ((base.y + j - 2) as f32 + 0.5) * vs;
            hs[(i + j * p) as usize] = t.height_with_coast(x, z, coast[i as usize]);
        }
    }
    HEIGHT_NS.fetch_add(th.elapsed().as_nanos() as u64, AtOrd::Relaxed);
    let tsf = std::time::Instant::now();
    // Superficie y material de cada columna de vóxeles, con un vóxel de borde.
    let present = |gx: i32, gz: i32| ring.contains(&IVec2::new(gx >> 5, gz >> 5));
    let mut top = vec![i32::MIN; (Q * Q) as usize];
    let mut surf = vec![AIR; (Q * Q) as usize];
    // Columnas de suelo natural (sin calle, plaza ni suelo nivelado): ahí crece la flora baja.
    let mut natural = vec![false; (Q * Q) as usize];
    let (mut tmin, mut tmax, mut pmin) = (i32::MAX, i32::MIN, i32::MAX);
    for j in 0..Q {
        for i in 0..Q {
            let (gx, gz) = (base.x + i - 1, base.y + j - 1);
            if !present(gx, gz) {
                pmin = i32::MIN;
                continue;
            }
            let hi = |a: i32, b: i32| hs[((i + 1 + a) + (j + 1 + b) * p) as usize];
            let h = hi(0, 0);
            let dx = hi(1, 0) - hi(-1, 0);
            let dz = hi(0, 1) - hi(0, -1);
            let slope = (dx * dx + dz * dz).sqrt() / (2.0 * vs);
            // Donde hay obras (calle, plaza), el suelo nivelado manda sobre el natural.
            let ty = ls.ground.get(&IVec2::new(gx, gz)).copied().unwrap_or_else(|| if drop > 0.0 { top_from_height_far(h, vs, drop) } else { top_from_height(h, vs) });
            let k = (i + j * Q) as usize;
            top[k] = ty;
            let x = (gx as f32 + 0.5) * vs;
            let z = (gz as f32 + 0.5) * vs;
            let painted = ls.surface.get(&IVec2::new(gx, gz)).copied();
            natural[k] = painted.is_none() && !ls.ground.contains_key(&IVec2::new(gx, gz));
            surf[k] = painted.unwrap_or_else(|| t.surface_material(x, z, h, slope));
            pmin = pmin.min(ty);
            if i >= 1 && i <= N && j >= 1 && j <= N {
                tmin = tmin.min(ty);
                tmax = tmax.max(ty);
            }
        }
    }
    // Pendiente continua de cada columna de la copia con borde (para la normal de las laderas).
    let mut grad = vec![0f32; (Q * Q * 2) as usize];
    for j in 0..Q {
        for i in 0..Q {
            let hi = |a: i32, b: i32| hs[((i + 1 + a) + (j + 1 + b) * p) as usize];
            let k = ((i + j * Q) * 2) as usize;
            grad[k] = (hi(1, 0) - hi(-1, 0)) / (2.0 * vs);
            grad[k + 1] = (hi(0, 1) - hi(0, -1)) / (2.0 * vs);
        }
    }
    SURF_NS.fetch_add(tsf.elapsed().as_nanos() as u64, AtOrd::Relaxed);
    let soil = ((2.0 / vs).ceil() as i32).max(1);
    let edit_top = ls.max_y.get(&col).copied().unwrap_or(i32::MIN);
    let trees = column_trees(t, ls, spec, col);
    let tree_top = trees.iter().map(|tr| tr.top).max().unwrap_or(i32::MIN);
    // Nivel de detalle del monte: en los niveles sin árboles sueltos (vóxel de más de 4 m) el
    // monte es una masa de copas sobre el suelo, con la altura del monte (5–9 m) y el borde
    // ralo, así no desaparece al alejar la cámara.
    let (canopy, canopy_mat) = forest_canopy(t, spec, base, &top, &surf, &natural);
    let tree_top = tree_top.max((0..(Q * Q) as usize).filter(|&k| canopy[k] > 0).map(|k| top[k] + canopy[k]).max().unwrap_or(i32::MIN));
    // Flora baja (matas, juncos, flores) de cada columna natural, por encima del agua: en
    // micro-vóxeles, aparte de la malla (ver `trees::plant_micro`).
    let mut detail: Vec<crate::trees::Micro> = Vec::new();
    if t.is_real() && vs <= crate::trees::PLANT_MAX_VOXEL {
        for j in 1..=N {
            for i in 1..=N {
                let k = (i + j * Q) as usize;
                if !natural[k] || top[k] == i32::MIN || (top[k] as f32 + 1.0) * vs < 0.4 {
                    continue;
                }
                let (gx, gz) = (base.x + i - 1, base.y + j - 1);
                if let Some(p) = crate::trees::ground_plant(surf[k], gx, gz, vs) {
                    // Sin plantas bajo la copa de un árbol que llega hasta el suelo ni en un vóxel ocupado.
                    crate::trees::plant_micro(&p, gx, gz, vs, (top[k] + 1) as f32 * vs, &mut detail);
                }
            }
        }
    }
    let y0 = (tmin - 1).div_euclid(N);
    let y1 = tmax.max(edit_top).max(tree_top).div_euclid(N);
    // Origen de la malla de la columna: esquina inferior del chunk más bajo.
    let mut out = Mesh::new(Vec3::new(base.x as f32, (y0 * N) as f32, base.y as f32) * vs - Vec3::Y * drop, vs);
    let mut pad = vec![AIR; (Q * Q * Q) as usize];
    for cy in y0..=y1 {
        let ck = IVec3::new(col.x, cy, col.y);
        let lo = IVec3::new(base.x - 1, cy * N - 1, base.y - 1);
        // ¿Hay ediciones en este chunk o en sus vecinos?
        let mut any_edit = false;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if ls.has_chunk(ck + IVec3::new(dx, dy, dz)) {
                        any_edit = true;
                    }
                }
            }
        }
        // Chunk enterrado y sin ediciones: todo macizo, no tiene caras visibles.
        if !any_edit && pmin != i32::MIN && cy * N + N <= pmin {
            continue;
        }
        // Terreno por capas.
        let mut solid = 0usize;
        for j in 0..Q {
            for i in 0..Q {
                let k = (i + j * Q) as usize;
                let ty = top[k];
                if ty == i32::MIN {
                    for y in 0..Q {
                        pad[(i + j * Q + y * Q * Q) as usize] = AIR;
                    }
                    continue;
                }
                for y in 0..Q {
                    let gy = lo.y + y;
                    let m = if gy > ty + canopy[k] {
                        AIR
                    } else if gy > ty {
                        canopy_mat[k]
                    } else {
                        layer(ty - gy, soil, surf[k])
                    };
                    solid += (m != AIR) as usize;
                    pad[(i + j * Q + y * Q * Q) as usize] = m;
                }
            }
        }
        // Árboles del paisaje (antes que las ediciones: una obra los tapa).
        for tr in &trees {
            solid += raster_tree(tr, lo, &top, &mut pad);
        }
        if any_edit {
            // Ediciones del propio chunk: se recorren todas.
            ls.for_each_in_chunk(ck, |idx, m| {
                let (lx, lz, ly) = ((idx & 31) as i32, ((idx >> 5) & 31) as i32, (idx >> 10) as i32);
                pad[((lx + 1) + (lz + 1) * Q + (ly + 1) * Q * Q) as usize] = m;
                solid += 1;
            });
            // Borde: consultas puntuales a los chunks vecinos.
            for y in 0..Q {
                for j in 0..Q {
                    for i in 0..Q {
                        if i != 0 && i != Q - 1 && j != 0 && j != Q - 1 && y != 0 && y != Q - 1 {
                            continue;
                        }
                        let g = lo + IVec3::new(i, y, j);
                        if top[(i + j * Q) as usize] == i32::MIN {
                            continue;
                        }
                        if let Some(m) = ls.get(g) {
                            pad[(i + j * Q + y * Q * Q) as usize] = m;
                            solid += 1;
                        }
                    }
                }
            }
        }
        if solid == 0 {
            continue;
        }
        // Hojas sueltas de 0,125 m en la superficie de las copas (solo en el nivel más fino).
        if !trees.is_empty() && vs <= crate::trees::PLANT_MAX_VOXEL {
            leaf_fuzz(&pad, lo, vs, &top, &mut detail);
        }
        CHUNKS.fetch_add(1, AtOrd::Relaxed);
        let tm = std::time::Instant::now();
        let m = mesher::mesh_chunk_grad(&pad, IVec3::new(0, (cy - y0) * N, 0), Some(&grad));
        out.append(&m);
        *mesh_ns += tm.elapsed().as_nanos() as u64;
    }
    out.detail = detail;
    out
}

/// Índice del vóxel de superficie del terreno en el nivel 0 (vóxel de 0,25 m).
pub fn terrain_top_l0(t: &TerrainGen, x: i32, z: i32) -> i32 {
    let vs = crate::lod::BASE_VOXEL;
    top_from_height(t.height((x as f32 + 0.5) * vs, (z as f32 + 0.5) * vs), vs)
}

#[cfg(test)]
mod tests {
    use super::top_from_height;

    #[test]
    fn la_tierra_firme_no_queda_bajo_el_agua_en_ningun_nivel() {
        // El agua del río es un plano en y = 0 (más la marea): la tierra con altura positiva
        // tiene que quedar arriba en todos los niveles de detalle, y el fondo, abajo; si no, el
        // agua se dibuja sobre la tierra (o la tierra asoma en el río) según dónde esté la cámara.
        for lod in 0..9 {
            let vs = crate::lod::BASE_VOXEL * (1u32 << lod) as f32;
            for k in 1..400 {
                let h = k as f32 * 0.05;
                assert!((top_from_height(h, vs) + 1) as f32 * vs > 0.0, "tierra a {h} m tapada con vóxel de {vs} m");
                assert!(((top_from_height(-h, vs) + 1) as f32 * vs) < 0.0, "fondo a -{h} m asoma con vóxel de {vs} m");
            }
        }
        // Niveles gruesos, con la malla bajada: tierra entre 3 m y medio vóxel sobre el agua, el
        // fondo bajo ella, y las alturas a menos de medio vóxel de la real.
        for lod in crate::lod::DROP_LOD..crate::lod::LEVELS + 1 {
            let vs = crate::lod::BASE_VOXEL * (1u32 << lod) as f32;
            let drop = crate::lod::far_drop(lod);
            let face = |k: i32| (k + 1) as f32 * vs - drop;
            for k in 1..4000 {
                let h = k as f32 * 0.25;
                let up = face(super::top_from_height_far(h, vs, drop));
                assert!(up >= 2.9, "tierra a {h} m tapada con vóxel de {vs} m ({up})");
                assert!(up - h <= vs * 0.5 + 3.0 && h - up <= vs * 0.5 + 0.01, "tierra a {h} m dibujada a {up} m");
                assert!(face(super::top_from_height_far(-h, vs, drop)) <= 0.0, "fondo a -{h} m asoma con vóxel de {vs} m");
            }
        }
    }
    use super::*;
    use crate::buildings::Edits;
    use std::time::{Duration, Instant};

    fn pump(s: &mut Streamer, secs: u64) -> Vec<Key> {
        let t0 = Instant::now();
        let mut order = Vec::new();
        while !s.idle() && t0.elapsed() < Duration::from_secs(secs) {
            for op in s.poll(usize::MAX) {
                if let Op::Set(k, _) = op {
                    order.push(k);
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        for op in s.poll(usize::MAX) {
            if let Op::Set(k, _) = op {
                order.push(k);
            }
        }
        order
    }

    #[test]
    fn primero_se_cargan_las_columnas_cercanas() {
        let t = Arc::new(TerrainGen::new(1726));
        let store = Arc::new(RwLock::new(EditStore::new(2)));
        let mut s = Streamer::new(t, store, 2, 1);
        let center = Vec2::new(13500.0, 16300.0);
        s.update(center);
        let order = pump(&mut s, 120);
        assert_eq!(order.len(), s.wanted_columns(), "faltan columnas");
        // Con un solo hilo el orden es exacto: la distancia de las primeras debe ser menor
        // que la de las últimas.
        let d = |k: &Key| {
            let cm = s.specs[k.0 as usize].chunk_m();
            Vec2::new((k.1.x as f32 + 0.5) * cm, (k.1.y as f32 + 0.5) * cm).distance(center)
        };
        let n = order.len() / 5;
        let first: f32 = order[..n].iter().map(d).sum::<f32>() / n as f32;
        let last: f32 = order[order.len() - n..].iter().map(d).sum::<f32>() / n as f32;
        assert!(first < last * 0.5, "las cercanas ({first:.0} m) no salieron antes que las lejanas ({last:.0} m)");
    }

    #[test]
    fn editar_re_malla_solo_las_columnas_tocadas() {
        let t = Arc::new(TerrainGen::new(1726));
        let store = Arc::new(RwLock::new(EditStore::new(3)));
        let mut s = Streamer::new(t.clone(), store, 3, 2);
        s.update(Vec2::new(13500.0, 16300.0));
        pump(&mut s, 120);
        let total = s.wanted_columns();
        // Un bloque de 3×3×3 vóxeles en medio de una columna: toca su columna en cada nivel.
        let v = IVec3::new(54020, terrain_top_l0(&t, 54020, 65220) + 1, 65220);
        let mut e = Edits::default();
        e.fill(v, v + IVec3::splat(3), CAL);
        let keys = s.edit(&e);
        assert!(!keys.is_empty() && keys.len() <= 3 * 4, "se encolaron {} columnas de {total}", keys.len());
        assert!(keys.contains(&(0, IVec2::new(54020 >> 5, 65220 >> 5))));
        let rebuilt = pump(&mut s, 60);
        assert_eq!(rebuilt.len(), keys.len(), "solo deben volver a subirse las columnas tocadas");
        // Y al quitarlo, lo mismo.
        let mut q = Edits::default();
        for (&p, _) in &e.voxels {
            q.set(p, AIR);
        }
        let keys2 = s.edit(&q);
        assert_eq!(keys2.len(), keys.len());
    }

    #[test]
    fn los_resultados_con_version_vieja_se_descartan() {
        let t = Arc::new(TerrainGen::new(1726));
        let store = Arc::new(RwLock::new(EditStore::new(2)));
        let mut s = Streamer::new(t.clone(), store, 2, 2);
        s.update(Vec2::new(13500.0, 16300.0));
        pump(&mut s, 120);
        // Dos ediciones seguidas sobre la misma columna, sin dar tiempo a que se suba la
        // primera: solo debe llegar a la GPU la malla con la última versión.
        let v = IVec3::new(54020, terrain_top_l0(&t, 54020, 65220) + 1, 65220);
        let mut e1 = Edits::default();
        e1.fill(v, v + IVec3::splat(2), CAL);
        let mut e2 = Edits::default();
        e2.fill(v + IVec3::Y * 2, v + IVec3::new(2, 6, 2), TILE);
        let k1 = s.edit(&e1);
        let k2 = s.edit(&e2);
        assert_eq!(k1, k2);
        let key = (0u32, IVec2::new(54020 >> 5, 65220 >> 5));
        let rebuilt = pump(&mut s, 60);
        let n = rebuilt.iter().filter(|k| **k == key).count();
        assert_eq!(n, 1, "la columna editada se subió {n} veces (debería subirse una sola, con la última versión)");
        assert_eq!(s.ready.get(&key), s.versions.get(&key));
    }

    #[test]
    fn las_columnas_viejas_se_quitan_apenas_su_zona_esta_cubierta() {
        let t = Arc::new(TerrainGen::new(1726));
        let store = Arc::new(RwLock::new(EditStore::new(3)));
        let mut s = Streamer::new(t, store, 3, 2);
        s.update(Vec2::new(13500.0, 16300.0));
        pump(&mut s, 120);
        // Se mueve la cámara: hay columnas retiradas. Mientras llegan las nuevas, ninguna
        // columna retirada puede seguir subida si su zona ya está cubierta.
        s.update(Vec2::new(13500.0 + 600.0, 16300.0));
        let t0 = Instant::now();
        while !s.idle() && t0.elapsed() < Duration::from_secs(120) {
            s.poll(20_000);
            for k in &s.retired {
                assert!(!s.covered(*k), "la columna {k:?} sigue subida con su zona ya cubierta");
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        s.poll(usize::MAX);
        assert!(s.retired.is_empty());
    }

    /// Lo que "ve" la GPU: claves de las columnas subidas, después de aplicar las órdenes.
    fn apply(shown: &mut HashSet<Key>, ops: Vec<Op>) {
        for op in ops {
            match op {
                Op::Set(k, _) => {
                    shown.insert(k);
                }
                Op::Remove(k) => {
                    shown.remove(&k);
                }
            }
        }
    }

    /// ¿Algún nivel tiene subida la columna que contiene el punto?
    fn point_in(s: &Streamer, keys: &HashSet<Key>, p: Vec2) -> bool {
        s.specs.iter().any(|sp| {
            let cm = sp.chunk_m();
            keys.contains(&(sp.lod, IVec2::new((p.x / cm).floor() as i32, (p.y / cm).floor() as i32)))
        })
    }

    /// ¿Algún nivel quiere la columna que contiene el punto (con la vista actual)?
    fn point_wanted(s: &Streamer, p: Vec2) -> bool {
        s.specs.iter().any(|sp| {
            let cm = sp.chunk_m();
            s.versions.contains_key(&(sp.lod, IVec2::new((p.x / cm).floor() as i32, (p.y / cm).floor() as i32)))
        })
    }

    /// Error visto en una placa real: al acercar/alejar o mover rápido la cámara desaparecían
    /// bloques rectangulares de terreno y se veía el río debajo. Un punto del suelo que se
    /// estaba dibujando no puede quedar sin ninguna columna mientras llegan las nuevas: ni
    /// con un salto grande del nivel más fino (zoom rápido, de 0 a 5) ni con varios
    /// recentrados seguidos sin esperar (viaje rápido).
    #[test]
    fn nunca_quedan_huecos_al_hacer_zoom_o_viajar_rapido() {
        let t = Arc::new(TerrainGen::new(1726));
        let store = Arc::new(RwLock::new(EditStore::new(7)));
        let mut s = Streamer::new(t, store, 7, 3);
        let c0 = Vec2::new(13500.0, 16300.0);
        s.update_with(c0, 0);
        let mut shown = HashSet::new();
        let t0 = Instant::now();
        while !s.idle() && t0.elapsed() < Duration::from_secs(120) {
            apply(&mut shown, s.poll(usize::MAX));
            std::thread::sleep(Duration::from_millis(2));
        }
        apply(&mut shown, s.poll(usize::MAX));
        // Puntos de muestra alrededor del centro (hasta 1,2 km).
        let pts: Vec<Vec2> = (-24..=24).flat_map(|j| (-24..=24).map(move |i| Vec2::new(i as f32 * 50.0 + 3.0, j as f32 * 50.0 + 7.0))).collect();
        let check = |s: &Streamer, shown: &HashSet<Key>, center: Vec2, what: &str| {
            for d in &pts {
                let p = center + *d;
                // (Todo el entorno está dentro del anillo más grueso: siempre se quiere.)
                assert!(point_wanted(s, p), "{what}: ningún nivel quiere ({:.0}, {:.0})", p.x, p.y);
                assert!(point_in(s, shown, p), "{what}: quedó un hueco en ({:.0}, {:.0})", p.x, p.y);
            }
        };
        check(&s, &shown, c0, "antes de moverse");
        // Zoom rápido hacia afuera (salto de nivel) y viaje rápido con recentrados seguidos.
        let moves: [(Vec2, u32); 5] = [(c0, 5), (c0 + Vec2::new(700.0, 0.0), 5), (c0 + Vec2::new(700.0, 500.0), 2), (c0 + Vec2::new(300.0, 900.0), 0), (c0, 0)];
        for (center, first) in moves {
            s.update_with(center, first);
            // Unas pocas entregas con presupuesto chico, sin esperar a que termine.
            for _ in 0..3 {
                apply(&mut shown, s.poll(4_000));
                check(&s, &shown, center, "mientras se mueve");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        let t0 = Instant::now();
        while !s.idle() && t0.elapsed() < Duration::from_secs(120) {
            apply(&mut shown, s.poll(20_000));
            check(&s, &shown, c0, "llegando");
            std::thread::sleep(Duration::from_millis(2));
        }
        apply(&mut shown, s.poll(usize::MAX));
        check(&s, &shown, c0, "al final");
        assert!(s.retired.is_empty());
    }
}
