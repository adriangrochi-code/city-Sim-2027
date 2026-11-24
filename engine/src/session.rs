//! Estado de una sesión interactiva del motor (con o sin ventana): mundo en streaming,
//! cámara, hora del día, herramienta de construcción y selección con el ratón.

use crate::buildings::Edits;
use crate::city::game::Game;
use crate::camera::OrbitCamera;
use crate::lod::{BASE_VOXEL, DESIGN_SCALE, DESIGN_VOXEL};
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

/// [lod] Tope de vértices subidos por cuadro en la ventana (repartir la carga entre cuadros
/// evita tirones).
pub const WINDOW_VERTEX_BUDGET: usize = 160_000;

/// [lod] Lo que subió un cuadro: órdenes aplicadas, bytes de mallas y tiempo de CPU (ms).
#[derive(Clone, Copy, Default, Debug)]
pub struct UploadFrame {
    pub ops: usize,
    pub bytes: u64,
    pub ms: f64,
    /// Parte del tiempo en recibir y elegir (`poll`) y en `maintain` (ms).
    pub poll_ms: f64,
    pub maintain_ms: f64,
}

/// [lod] Sube a la GPU lo que terminaron los hilos de trabajo, con el presupuesto del cuadro.
/// La usan la ventana ([`Session::apply_ops`]) y el banco (vuelo), así miden lo mismo.
pub fn upload_ops(streamer: &mut Streamer, r: &mut Renderer, vertex_budget: usize) -> UploadFrame {
    let t0 = Instant::now();
    let b0 = r.uploaded_bytes;
    // Tope de vértices y de bytes (lo que entra en el presupuesto de tiempo de subida).
    let byte_budget = if vertex_budget == usize::MAX { usize::MAX } else { streamer.ritmo.bytes() };
    streamer.fundido = r.fundir; // [render]
    let ops = streamer.poll_budget(vertex_budget, byte_budget);
    let poll_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let n = ops.len();
    let tm = Instant::now();
    r.maintain();
    let maintain_ms = tm.elapsed().as_secs_f64() * 1000.0;
    // [render] El envío de este cuadro es el de lo que se subió en el anterior.
    streamer.ritmo.medir_envio(maintain_ms);
    let tu = Instant::now();
    for op in ops {
        match op {
            Op::Set(k, m) => {
                r.set_column(k, &m);
                streamer.give_back(k, m);
            }
            Op::Remove(k) => r.remove_column(k),
            // [render] Fundido entre niveles (técnicas del mod LOD §2.5).
            Op::Entra(k, m) => {
                r.set_column_fade(k, &m);
                streamer.give_back(k, m);
            }
            Op::Sale(k) => r.fade_out_column(k),
        }
    }
    let bytes = r.uploaded_bytes - b0;
    streamer.ritmo.ultima = (bytes, tu.elapsed().as_secs_f64() * 1000.0);
    UploadFrame { ops: n, bytes, ms: t0.elapsed().as_secs_f64() * 1000.0, poll_ms, maintain_ms }
}

/// [lod] Le pasa la cámara al streamer como la ventana: centro de los anillos, altura del ojo
/// sobre el terreno (nivel más fino) y, para la prioridad y la oclusión, el ojo y la mirada.
pub fn view_to_streamer(streamer: &mut Streamer, cam: &OrbitCamera, view_h: f32) {
    let eye = cam.eye();
    streamer.set_view(eye, Vec2::new(cam.target.x - eye.x, cam.target.z - eye.z));
    let eye_h = (eye.y - cam.target.y).max(1.0);
    streamer.update_view(Vec2::new(cam.target.x, cam.target.z), eye_h, cam.fov_y, view_h);
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
    /// Modo juego: la ciudad colonial (calles, solares, crecimiento). `None` = sandbox.
    pub game: Option<Game>,
}

impl Session {
    pub fn new(scene: &str, levels: u32, threads: usize, cam: [f32; 5], hour: f32) -> Session {
        Session::with_terrain(scene, Arc::new(TerrainGen::new(1726)), levels, threads, cam, hour)
    }

    pub fn with_terrain(scene: &str, terrain: Arc<TerrainGen>, levels: u32, threads: usize, cam: [f32; 5], hour: f32) -> Session {
        let started = Instant::now();
        let store = Arc::new(RwLock::new(EditStore::new(levels)));
        let mut streamer = Streamer::new(terrain.clone(), store, levels, threads);
        streamer.prewarm_relief(); // [lod]
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
        Session { terrain, streamer, cam, hour, day_paused: true, tool: 2, rot: 0, hit: None, scene_ready: None, started, first_frame: None, seed: 1, scene_rx: Some(rx), view_h: 900.0, game: None }
    }

    /// Sesión del modo juego: terreno virgen y la ciudad por fundar.
    pub fn new_game(levels: u32, threads: usize, cam: [f32; 5], hour: f32) -> Session {
        Session::with_game(Game::default(), &Edits::default(), levels, threads, cam, hour)
    }

    /// Sesión del modo juego con una partida ya armada (nueva o cargada) y los vóxeles de su
    /// ciudad: el almacén de ediciones se llena antes de empezar a generar columnas, así que
    /// el mundo aparece directamente como se guardó.
    pub fn with_game(game: Game, edits: &Edits, levels: u32, threads: usize, cam: [f32; 5], hour: f32) -> Session {
        let started = Instant::now();
        // El juego se juega sobre la geografía real de Montevideo.
        let terrain = Arc::new(TerrainGen::montevideo());
        let mut store = EditStore::new(levels);
        store.apply(edits, false);
        let store = Arc::new(RwLock::new(store));
        let mut streamer = Streamer::new(terrain.clone(), store, levels, threads);
        streamer.prewarm_relief(); // [lod]
        let ground = terrain.height(cam[0], cam[1]).max(0.0);
        let cam = OrbitCamera { target: Vec3::new(cam[0], ground, cam[1]), yaw: cam[3], pitch: cam[4], distance: cam[2], fov_y: 50.0 };
        Session { terrain, streamer, cam, hour, day_paused: false, tool: 2, rot: 0, hit: None, scene_ready: Some(Duration::ZERO), started, first_frame: None, seed: 1, scene_rx: None, view_h: 900.0, game: Some(game) }
    }

    /// Cámara actual como en `--cam`: x, z, distancia, giro, inclinación.
    pub fn cam_array(&self) -> [f32; 5] {
        [self.cam.target.x, self.cam.target.z, self.cam.distance, self.cam.yaw, self.cam.pitch]
    }

    /// Aplica ediciones al mundo (re-malla solo las columnas tocadas).
    pub fn apply_edits(&mut self, e: &Edits) -> usize {
        if e.is_empty() {
            return 0;
        }
        self.streamer.edit(e).len()
    }

    /// Clic izquierdo del modo juego donde apunta el ratón.
    pub fn game_press(&mut self) {
        let t = self.terrain.clone();
        let Some(g) = self.game.as_mut() else { return };
        let Some(h) = g.hover else { return };
        if let Some(e) = g.press(&t, h) {
            let t0 = Instant::now();
            let n = self.apply_edits(&e);
            println!("{} vóxeles editados, {n} columnas a re-mallar ({:.1?})", e.voxel_count(), t0.elapsed());
        }
    }

    pub fn game_release(&mut self) {
        if let Some(g) = self.game.as_mut() {
            g.release();
        }
    }

    /// Avanza el estado: la cámara sigue el terreno, el día corre y el mundo se recentra.
    pub fn update(&mut self, dt: f32) {
        let ground = self.terrain.height(self.cam.target.x, self.cam.target.z).max(0.0);
        self.cam.target.y += (ground - self.cam.target.y) * (dt * 6.0).min(1.0);
        // [atlas] En el juego la cámara no sale del mapa; sin partida (paisaje del mapa real) puede
        // recorrer el entorno amplio (sierras de Maldonado, Colonia).
        let (lo, hi) = if self.game.is_some() || !self.terrain.is_real() { (0.0, 24000.0) } else { (-130000.0, 154000.0) };
        self.cam.target.x = self.cam.target.x.clamp(lo, hi);
        self.cam.target.z = self.cam.target.z.clamp(lo, hi);
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
        // Modo juego: el cursor, el pincel de zonas y la simulación (crecimiento de la ciudad).
        if self.game.is_some() {
            let t = self.terrain.clone();
            let hover = self.hit.map(|h| Vec2::new(h.point.x, h.point.z));
            let g = self.game.as_mut().unwrap();
            g.hover = hover;
            // Los vecinos siguen el reloj del día; cerca de la vista se simulan con detalle.
            g.city.agentes.set_hora(self.hour);
            g.city.agentes.foco = Some(Vec2::new(self.cam.target.x, self.cam.target.z));
            let mut batch = Edits::default();
            if let Some(h) = hover {
                if let Some(e) = g.paint_at(h) {
                    batch.extend(e);
                }
            }
            if let Some(e) = g.update(&t, dt) {
                batch.extend(e);
            }
            self.apply_edits(&batch);
        }
        view_to_streamer(&mut self.streamer, &self.cam, self.view_h);
    }

    pub fn scene_loaded(&self) -> bool {
        self.scene_rx.is_none()
    }

    /// Sube a la GPU lo que terminaron los hilos de trabajo (con un presupuesto por cuadro).
    pub fn apply_ops(&mut self, r: &mut Renderer, vertex_budget: usize) -> usize {
        // [entorno amplio] Hasta dónde llega el terreno: más allá, niebla (ver `Renderer::reach`).
        let (c, reach) = self.streamer.reach();
        r.reach = [c.x, c.y, reach];
        // [render] Niebla de lo que falta (ver `Streamer::sectores_faltan`).
        r.set_faltan(&self.streamer.sectores_faltan());
        // Río y luna del juego (sin partida: río en su nivel medio y media luna).
        match &self.game {
            Some(g) if g.city.plaza.is_some() => {
                r.water_level = g.city.water_level(self.hour);
                r.moon = crate::city::water::moon_light(g.city.moon_phase());
                r.wind = (g.city.water.wind_dir, g.city.water.wind_speed);
            }
            _ => {
                // [agua en vóxeles] `MOTOR_MAREA=1.2` fija el nivel del río sin partida (capturas).
                r.water_level = std::env::var("MOTOR_MAREA").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                r.moon = 0.5;
                r.wind = (70.0, 4.0);
            }
        }
        // Vecinos y vehículos cerca de la vista (posiciones interpoladas).
        match &self.game {
            Some(g) => {
                let mut cajas = Vec::new();
                // [personajes] vecinos, animales y detalles de micro-vóxeles
                crate::city::agentes::fauna::cajas_ciudad(&g.city, &self.terrain, Vec2::new(self.cam.target.x, self.cam.target.z), self.hour, &mut cajas);
                // [ciudad] rejas, faroles de fachada, macetas y llamadores de las casas
                g.city.fachadas.vigentes(&g.city.lots).cajas(Vec2::new(self.cam.target.x, self.cam.target.z), &mut cajas);
                r.set_agents(&cajas);
            }
            None => r.set_agents(&[]),
        }
        // Luces de los faroles cerca de la vista (gas y sebo; ver `render::luces`).
        let centro = Vec2::new(self.cam.target.x, self.cam.target.z);
        let luces = self.game.as_ref().map_or(Vec::new(), |g| g.city.luces_cerca(centro, 300.0, crate::render::luces::MAX));
        r.set_lights(centro, &luces);
        let n = upload_ops(&mut self.streamer, r, vertex_budget).ops;
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

    /// Excava una esfera de radio `r` vóxeles de diseño (0,5 m) donde apunta el ratón.
    pub fn dig(&mut self, r: i32) -> Option<usize> {
        let r = r * DESIGN_SCALE;
        let hit = self.hit?;
        let edits = {
            let store = self.streamer.store.read().unwrap();
            sandbox::dig(&self.terrain, &store.levels[0], hit.voxel, r)
        };
        Some(self.streamer.edit(&edits).len())
    }

    /// Rellena una esfera de radio `r` vóxeles de diseño (0,5 m) donde apunta el ratón.
    pub fn fill(&mut self, r: i32) -> Option<usize> {
        let r = r * DESIGN_SCALE;
        let hit = self.hit?;
        let edits = {
            let store = self.streamer.store.read().unwrap();
            sandbox::fill(&self.terrain, &store.levels[0], hit.voxel + hit.normal * r, r)
        };
        Some(self.streamer.edit(&edits).len())
    }

    /// Resaltado: el vóxel bajo el cursor y el contorno de la estructura a colocar.
    pub fn overlay(&self) -> Vec<OverlayVertex> {
        if let Some(g) = &self.game {
            return g.overlay(&self.terrain);
        }
        let mut out = Vec::new();
        if let Some(h) = self.hit {
            let vs = BASE_VOXEL;
            let a = h.voxel.as_vec3() * vs - Vec3::splat(0.03);
            overlay_box(&mut out, a, a + Vec3::splat(vs + 0.06), [1.0, 0.95, 0.4, 0.45]);
            let f = self.frame(&h);
            let (lo, hi) = sandbox::footprint(&f);
            overlay_box(&mut out, lo.as_vec3() * DESIGN_VOXEL, hi.as_vec3() * DESIGN_VOXEL, [0.35, 0.95, 0.45, 0.22]);
        }
        out
    }

    pub fn title(&self, fps: f32, r: &Renderer) -> String {
        if let Some(g) = &self.game {
            return format!("{} · {fps:.0} FPS · {:.2} M triángulos · {} columnas · pendientes {} · {:02}:{:02}", g.status(), r.triangles() as f32 / 1e6, r.column_count(), self.streamer.pending(), self.hour as u32, (self.hour.fract() * 60.0) as u32);
        }
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
