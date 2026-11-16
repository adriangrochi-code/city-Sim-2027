//! Estado de una sesión interactiva del motor (con o sin ventana): mundo en streaming,
//! cámara, hora del día, herramienta de construcción y selección con el ratón.

use crate::buildings::Edits;
use crate::camera::OrbitCamera;
use crate::lod::BASE_VOXEL;
use crate::raycast::{raycast, Hit};
use crate::render::{overlay_box, OverlayVertex, Renderer};
use crate::sandbox::{self, CATALOG};
use crate::store::EditStore;
use crate::stream::{Op, Streamer};
use crate::structures;
use crate::terrain::TerrainGen;
use glam::{IVec2, Vec2, Vec3};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cámara inicial sugerida por escena: (x, z, distancia, giro, inclinación).
pub fn scene_camera(scene: &str) -> [f32; 5] {
    match scene {
        "catalogo" => [13585.0, 14960.0, 330.0, 15.0, 36.0],
        _ => [13500.0, 16300.0, 420.0, 200.0, 38.0],
    }
}

pub struct Session {
    pub terrain: Arc<TerrainGen>,
    pub streamer: Streamer,
    pub cam: OrbitCamera,
    pub hour: f32,
    pub day_paused: bool,
    pub tool: usize,
    pub rot: u32,
    pub hit: Option<Hit>,
    pub scene_ready: Option<Duration>,
    pub started: Instant,
    pub first_frame: Option<Duration>,
    seed: u64,
    scene_rx: Option<Receiver<EditStore>>,
    /// Alto de la imagen en píxeles (para elegir el nivel de detalle por tamaño en pantalla).
    pub view_h: f32,
}

impl Session {
    pub fn new(scene: &str, levels: u32, threads: usize, cam: [f32; 5], hour: f32) -> Session {
        let started = Instant::now();
        let terrain = Arc::new(TerrainGen::new(1726));
        let store = Arc::new(RwLock::new(EditStore::new(levels)));
        let streamer = Streamer::new(terrain.clone(), store, levels, threads);
        // La escena (miles de edificios) se arma en otro hilo: mientras tanto ya se ve el terreno.
        let (tx, rx) = channel();
        let scene_name = scene.to_string();
        let t = terrain.clone();
        std::thread::spawn(move || {
            let plaza = Vec2::new(13500.0, 16300.0);
            let edits = match scene_name.as_str() {
                "vacia" => Edits::default(),
                "catalogo" => structures::scenes::catalogo(&t).edits,
                _ => structures::scenes::ciudad(&t, plaza, 3, 7).edits,
            };
            let mut store = EditStore::new(levels);
            store.apply(&edits, false);
            let _ = tx.send(store);
        });
        let ground = terrain.height(cam[0], cam[1]).max(0.0);
        let cam = OrbitCamera { target: Vec3::new(cam[0], ground, cam[1]), yaw: cam[3], pitch: cam[4], distance: cam[2], fov_y: 50.0 };
        Session { terrain, streamer, cam, hour, day_paused: true, tool: 2, rot: 0, hit: None, scene_ready: None, started, first_frame: None, seed: 1, scene_rx: Some(rx), view_h: 900.0 }
    }

    /// Avanza el estado: la cámara sigue el terreno, el día corre y el mundo se recentra.
    pub fn update(&mut self, dt: f32) {
        let ground = self.terrain.height(self.cam.target.x, self.cam.target.z).max(0.0);
        self.cam.target.y += (ground - self.cam.target.y) * (dt * 6.0).min(1.0);
        self.cam.target.x = self.cam.target.x.clamp(0.0, 24000.0);
        self.cam.target.z = self.cam.target.z.clamp(0.0, 24000.0);
        if !self.day_paused {
            // Un día de juego dura 8 minutos reales.
            self.hour = (self.hour + dt * 24.0 / 480.0).rem_euclid(24.0);
        }
        if let Some(rx) = &self.scene_rx {
            if let Ok(store) = rx.try_recv() {
                let n = self.streamer.replace_store(store);
                self.scene_ready = Some(self.started.elapsed());
                println!("Escena lista en {:.2?}: {} columnas a regenerar", self.started.elapsed(), n);
                self.scene_rx = None;
            }
        }
        let eye_h = (self.cam.eye().y - self.cam.target.y).max(1.0);
        self.streamer.update_view(Vec2::new(self.cam.target.x, self.cam.target.z), eye_h, self.cam.fov_y, self.view_h);
    }

    pub fn scene_loaded(&self) -> bool {
        self.scene_rx.is_none()
    }

    /// Sube a la GPU lo que terminaron los hilos de trabajo (con un presupuesto por cuadro).
    pub fn apply_ops(&mut self, r: &mut Renderer, vertex_budget: usize) -> usize {
        let ops = self.streamer.poll(vertex_budget);
        let n = ops.len();
        r.maintain();
        for op in ops {
            match op {
                Op::Set(k, m) => r.set_column(k, &m),
                Op::Remove(k) => r.remove_column(k),
            }
        }
        if self.first_frame.is_none() && r.column_count() > 0 {
            self.first_frame = Some(self.started.elapsed());
        }
        n
    }

    pub fn pick(&mut self, origin: Vec3, dir: Vec3) -> Option<Hit> {
        let store = self.streamer.store.read().unwrap();
        self.hit = raycast(&self.terrain, &store.levels[0], origin, dir, 40000.0);
        self.hit
    }

    /// Selecciona lo que hay en un punto del mapa mirando desde arriba.
    pub fn pick_xz(&mut self, x: f32, z: f32) -> Option<Hit> {
        self.pick(Vec3::new(x, 600.0, z), Vec3::NEG_Y)
    }

    pub fn tool_name(&self) -> &'static str {
        CATALOG[self.tool].name
    }

    fn frame(&self, hit: &Hit) -> structures::Frame {
        sandbox::frame_at(&self.terrain, &CATALOG[self.tool], IVec2::new(hit.voxel.x, hit.voxel.z), self.rot, self.seed)
    }

    /// Coloca la estructura elegida donde apunta el ratón. Devuelve cuántas columnas se re-mallan.
    pub fn place(&mut self) -> Option<usize> {
        let hit = self.hit?;
        let f = self.frame(&hit);
        let edits = sandbox::build(&self.terrain, &CATALOG[self.tool], &f);
        self.seed += 1;
        Some(self.streamer.edit(&edits).len())
    }

    pub fn dig(&mut self, r: i32) -> Option<usize> {
        let hit = self.hit?;
        let edits = {
            let store = self.streamer.store.read().unwrap();
            sandbox::dig(&self.terrain, &store.levels[0], hit.voxel, r)
        };
        Some(self.streamer.edit(&edits).len())
    }

    pub fn fill(&mut self, r: i32) -> Option<usize> {
        let hit = self.hit?;
        let edits = {
            let store = self.streamer.store.read().unwrap();
            sandbox::fill(&self.terrain, &store.levels[0], hit.voxel + hit.normal * r, r)
        };
        Some(self.streamer.edit(&edits).len())
    }

    /// Resaltado: el vóxel bajo el cursor y el contorno de la estructura a colocar.
    pub fn overlay(&self) -> Vec<OverlayVertex> {
        let mut out = Vec::new();
        if let Some(h) = self.hit {
            let vs = BASE_VOXEL;
            let a = h.voxel.as_vec3() * vs - Vec3::splat(0.03);
            overlay_box(&mut out, a, a + Vec3::splat(vs + 0.06), [1.0, 0.95, 0.4, 0.45]);
            let f = self.frame(&h);
            let (lo, hi) = sandbox::footprint(&f);
            overlay_box(&mut out, lo.as_vec3() * vs, hi.as_vec3() * vs, [0.35, 0.95, 0.45, 0.22]);
        }
        out
    }

    pub fn title(&self, fps: f32, r: &Renderer) -> String {
        format!(
            "Motor de vóxeles · {fps:.0} FPS · {:.2} M triángulos ({:.2} M caras, {:.0} MB) · dibujo {} · {} + {} llamadas · subido {:.1} MB · {} columnas · pendientes {} · {:02}:{:02}{} · herramienta: {} (giro {})",
            r.triangles() as f32 / 1e6,
            r.quad_count() as f32 / 1e6,
            r.mesh_mb(),
            r.draw_mode.name(),
            r.last_draws.0,
            r.last_draws.1,
            r.upload_frame as f32 / 1048576.0,
            r.column_count(),
            self.streamer.pending(),
            self.hour as u32,
            ((self.hour.fract()) * 60.0) as u32,
            if self.day_paused { " (pausa)" } else { "" },
            self.tool_name(),
            self.rot * 90
        )
    }
}

#[cfg(target_os = "linux")]
extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

/// Devuelve al sistema la memoria libre que el asignador de glibc retiene (las mallas de las
/// columnas reemplazadas dejan huecos en el montículo que de otro modo no se devuelven).
pub fn trim_memory() {
    #[cfg(target_os = "linux")]
    unsafe {
        malloc_trim(0);
    }
}

/// Memoria del proceso (Linux): (actual, máxima) en MB.
pub fn memory_mb() -> (f64, f64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let get = |k: &str| status.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 1024.0;
    (get("VmRSS:"), get("VmHWM:"))
}
