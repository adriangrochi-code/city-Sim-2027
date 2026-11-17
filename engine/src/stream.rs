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
use crate::store::{chunk_of, local_index, EditStore};
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
            for k in l.voxels.keys() {
                add(IVec2::new(k.x, k.z));
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
        // rápido se veían mallas de dos niveles superpuestas). Si se acumula mucho, se borra.
        if !self.retired.is_empty() {
            let flush = self.idle_now() || self.retired.len() > 3000;
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
        for k in (l - 3).max(0)..=(l + 3).min(self.specs.len() as i32 - 1) {
            if k == l {
                continue;
            }
            if k < l {
                let f = 1 << (l - k);
                for dz in 0..f {
                    for dx in 0..f {
                        let kk = (k as u32, c * f + IVec2::new(dx, dz));
                        if !self.is_current(kk) {
                            return false;
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
        (h / vs - 0.5).floor() as i32
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

/// Un árbol ya llevado a vóxeles del nivel: tronco y copa (elipsoide).
struct VoxTree {
    /// Centro del tronco en vóxeles (x, z), suelo (y del primer vóxel de tronco).
    cx: f32,
    cz: f32,
    ground: i32,
    trunk_top: i32,
    trunk_r: f32,
    /// Centro y radios de la copa en vóxeles.
    cy: f32,
    rh: f32,
    rv: f32,
    top: i32,
    leaf: Mat,
    seed: u32,
}

/// Niveles con árboles: de 0,5 m a 4 m por vóxel (más lejos, una copa sería un solo vóxel).
const TREE_MAX_LOD: u32 = 3;

/// Árboles cuyo tronco o copa caen en la columna, salvo donde hay obras (calles, plaza o
/// edificios): ahí el monte se taló.
fn column_trees(t: &TerrainGen, ls: &crate::store::LevelStore, spec: LevelSpec, col: IVec2) -> Vec<VoxTree> {
    let mut out = Vec::new();
    if spec.lod > TREE_MAX_LOD || !t.is_real() {
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
        // ¿Obras cerca del tronco? (superficie pintada o vóxeles editados en el chunk).
        let cleared = (-1..=1).any(|d: i32| {
            let g = IVec2::new(gx + d * (3.0 / vs).ceil() as i32, gz);
            let g2 = IVec2::new(gx, gz + d * (3.0 / vs).ceil() as i32);
            ls.surface.contains_key(&g) || ls.surface.contains_key(&g2)
        });
        if cleared {
            continue;
        }
        let ground = top_from_height(t.height(tr.x, tr.z), vs) + 1;
        let ck = IVec2::new(gx >> 5, gz >> 5);
        let cy = ground >> 5;
        if (-1..=1).any(|dy| ls.voxels.contains_key(&IVec3::new(ck.x, cy + dy, ck.y))) {
            continue;
        }
        let rh = (tr.radius / vs).max(0.8);
        let rv = (tr.radius * 0.72 / vs).max(0.7);
        let c_y = ground as f32 + (tr.height - tr.radius * 0.72) / vs;
        out.push(VoxTree {
            cx: tr.x / vs,
            cz: tr.z / vs,
            ground,
            trunk_top: c_y as i32,
            trunk_r: (tr.trunk / vs).max(0.55),
            cy: c_y,
            rh,
            rv,
            top: (c_y + rv).ceil() as i32,
            leaf: tr.leaf,
            seed: (gx as u32).wrapping_mul(73_856_093) ^ (gz as u32).wrapping_mul(19_349_663),
        });
    }
    out
}

/// Escribe un árbol en el bloque `pad` (Q³ desde `lo`); solo ocupa aire y columnas presentes.
/// Devuelve los vóxeles puestos.
fn raster_tree(tr: &VoxTree, lo: IVec3, top: &[i32], pad: &mut [Mat]) -> usize {
    let mut n = 0;
    let (u0, u1) = (((tr.cx - tr.rh).floor() as i32 - lo.x).max(0), ((tr.cx + tr.rh).ceil() as i32 - lo.x).min(Q - 1));
    let (w0, w1) = (((tr.cz - tr.rh).floor() as i32 - lo.z).max(0), ((tr.cz + tr.rh).ceil() as i32 - lo.z).min(Q - 1));
    let (y0, y1) = ((tr.ground - lo.y).max(0), (tr.top - lo.y).min(Q - 1));
    if u0 > u1 || w0 > w1 || y0 > y1 {
        return 0;
    }
    for w in w0..=w1 {
        for u in u0..=u1 {
            if top[(u + w * Q) as usize] == i32::MIN {
                continue;
            }
            let (dx, dz) = ((u + lo.x) as f32 + 0.5 - tr.cx, (w + lo.z) as f32 + 0.5 - tr.cz);
            let d2 = dx * dx + dz * dz;
            for y in y0..=y1 {
                let gy = y + lo.y;
                let k = (u + w * Q + y * Q * Q) as usize;
                if pad[k] != AIR {
                    continue;
                }
                let m = if gy <= tr.trunk_top && d2 <= tr.trunk_r * tr.trunk_r {
                    TRUNK
                } else {
                    let dy = gy as f32 + 0.5 - tr.cy;
                    let e = d2 / (tr.rh * tr.rh) + dy * dy / (tr.rv * tr.rv);
                    // Borde de la copa irregular: algunos vóxeles del contorno faltan.
                    let hh = (tr.seed ^ ((u + lo.x) as u32).wrapping_mul(2_654_435_761) ^ (gy as u32).wrapping_mul(40_503) ^ ((w + lo.z) as u32).wrapping_mul(97_331)) % 100;
                    if e <= 1.0 && (e < 0.6 || hh > 35) {
                        tr.leaf
                    } else {
                        continue;
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
            let ty = top_from_height(h, vs);
            let k = (i + j * Q) as usize;
            top[k] = ty;
            let x = (gx as f32 + 0.5) * vs;
            let z = (gz as f32 + 0.5) * vs;
            surf[k] = ls.surface.get(&IVec2::new(gx, gz)).copied().unwrap_or_else(|| t.surface_material(x, z, h, slope));
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
    let y0 = (tmin - 1).div_euclid(N);
    let y1 = tmax.max(edit_top).max(tree_top).div_euclid(N);
    // Origen de la malla de la columna: esquina inferior del chunk más bajo.
    let mut out = Mesh::new(Vec3::new(base.x as f32, (y0 * N) as f32, base.y as f32) * vs, vs);
    let mut pad = vec![AIR; (Q * Q * Q) as usize];
    for cy in y0..=y1 {
        let ck = IVec3::new(col.x, cy, col.y);
        let lo = IVec3::new(base.x - 1, cy * N - 1, base.y - 1);
        // ¿Hay ediciones en este chunk o en sus vecinos?
        let mut any_edit = false;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if ls.voxels.contains_key(&(ck + IVec3::new(dx, dy, dz))) {
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
                    let m = if gy > ty { AIR } else { layer(ty - gy, soil, surf[k]) };
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
            if let Some(map) = ls.voxels.get(&ck) {
                for (idx, m) in map.iter() {
                    let (lx, lz, ly) = ((idx & 31) as i32, ((idx >> 5) & 31) as i32, (idx >> 10) as i32);
                    pad[((lx + 1) + (lz + 1) * Q + (ly + 1) * Q * Q) as usize] = m;
                    solid += 1;
                }
            }
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
                        if let Some(map) = ls.voxels.get(&chunk_of(g)) {
                            if let Some(m) = map.get(local_index(g)) {
                                pad[(i + j * Q + y * Q * Q) as usize] = m;
                                solid += 1;
                            }
                        }
                    }
                }
            }
        }
        if solid == 0 {
            continue;
        }
        CHUNKS.fetch_add(1, AtOrd::Relaxed);
        let tm = std::time::Instant::now();
        let m = mesher::mesh_chunk_grad(&pad, IVec3::new(0, (cy - y0) * N, 0), Some(&grad));
        out.append(&m);
        *mesh_ns += tm.elapsed().as_nanos() as u64;
    }
    out
}

/// Índice del vóxel de superficie del terreno en el nivel 0 (vóxel de 0,5 m).
pub fn terrain_top_l0(t: &TerrainGen, x: i32, z: i32) -> i32 {
    let vs = crate::lod::BASE_VOXEL;
    top_from_height(t.height((x as f32 + 0.5) * vs, (z as f32 + 0.5) * vs), vs)
}

#[cfg(test)]
mod tests {
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
        let v = IVec3::new(27010, terrain_top_l0(&t, 27010, 32610) + 1, 32610);
        let mut e = Edits::default();
        e.fill(v, v + IVec3::splat(3), CAL);
        let keys = s.edit(&e);
        assert!(!keys.is_empty() && keys.len() <= 3 * 4, "se encolaron {} columnas de {total}", keys.len());
        assert!(keys.contains(&(0, IVec2::new(27010 >> 5, 32610 >> 5))));
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
        let v = IVec3::new(27010, terrain_top_l0(&t, 27010, 32610) + 1, 32610);
        let mut e1 = Edits::default();
        e1.fill(v, v + IVec3::splat(2), CAL);
        let mut e2 = Edits::default();
        e2.fill(v + IVec3::Y * 2, v + IVec3::new(2, 6, 2), TILE);
        let k1 = s.edit(&e1);
        let k2 = s.edit(&e2);
        assert_eq!(k1, k2);
        let key = (0u32, IVec2::new(27010 >> 5, 32610 >> 5));
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
}
