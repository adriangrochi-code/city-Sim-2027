//! Recorrido automático para probar el motor sin intervención: mueve la cámara, construye,
//! excava, viaja lejos (streaming) y guarda capturas en puntos clave con métricas.
//! Funciona igual con ventana o sin pantalla.

use crate::render::Renderer;
use crate::session::{memory_mb, Session};
use glam::Vec2;
use std::time::{Duration, Instant};

pub enum Act {
    /// Cámara: x, z, distancia, giro, inclinación.
    Cam([f32; 5]),
    Hour(f32),
    Smooth(bool),
    WaitIdle(&'static str),
    Shot(&'static str),
    /// Coloca una estructura del catálogo (índice, giro) en x, z.
    Place(usize, u32, f32, f32),
    /// Coloca usando el rayo del centro de la pantalla (como un clic).
    PlaceCenter(usize, u32),
    Dig(f32, f32, i32),
    Fill(f32, f32, i32),
    /// Viaja con la cámara hasta x, z en N cuadros (el mundo se va cargando en el camino).
    Travel(f32, f32, u32),
}

pub struct Tour {
    steps: Vec<Act>,
    i: usize,
    dir: String,
    width: u32,
    height: u32,
    step_start: Instant,
    travel: Option<(Vec2, Vec2, u32, u32)>,
    pub report: Vec<String>,
}

pub fn default_steps() -> Vec<Act> {
    use Act::*;
    let site = (13600.0, 15800.0);
    vec![
        Cam([13500.0, 16300.0, 1600.0, 200.0, 42.0]),
        WaitIdle("vista general"),
        Shot("motor-vista-general"),
        Cam([13200.0, 15000.0, 4200.0, 190.0, 28.0]),
        Smooth(false),
        WaitIdle("vista lejana"),
        Shot("motor-escalones-antes"),
        Smooth(true),
        Shot("motor-escalones-despues"),
        Cam([13500.0, 16300.0, 170.0, 205.0, 30.0]),
        WaitIdle("plaza de cerca"),
        Shot("motor-plaza"),
        Cam([site.0, site.1, 150.0, 200.0, 38.0]),
        WaitIdle("terreno para construir"),
        PlaceCenter(2, 0),
        Place(1, 0, site.0 + 16.0, site.1),
        Place(3, 0, site.0 - 16.0, site.1),
        Place(4, 0, site.0, site.1 + 20.0),
        Place(5, 0, site.0 - 30.0, site.1 + 25.0),
        Place(0, 1, site.0 + 40.0, site.1 + 22.0),
        WaitIdle("construcción"),
        Shot("motor-construir"),
        Cam([site.0, site.1 + 40.0, 70.0, 200.0, 40.0]),
        Dig(site.0 + 10.0, site.1 + 45.0, 9),
        Dig(site.0 + 14.0, site.1 + 45.0, 9),
        Dig(site.0 + 18.0, site.1 + 45.0, 9),
        Fill(site.0 - 12.0, site.1 + 45.0, 7),
        Fill(site.0 - 16.0, site.1 + 45.0, 7),
        WaitIdle("excavar y rellenar"),
        Place(2, 2, site.0 + 2.0, site.1 + 30.0),
        WaitIdle("otra casa, girada"),
        Shot("motor-excavar"),
        Cam([site.0, site.1, 700.0, 200.0, 35.0]),
        Travel(10400.0, 14900.0, 150),
        Cam([10000.0, 15100.0, 1100.0, 200.0, 22.0]),
        WaitIdle("viaje al cerro (3 km)"),
        Shot("motor-cerro"),
        Hour(20.5),
        Cam([13500.0, 16300.0, 450.0, 200.0, 35.0]),
        WaitIdle("vuelta a la plaza de noche"),
        Shot("motor-noche"),
    ]
}

impl Tour {
    pub fn new(dir: &str, width: u32, height: u32) -> Tour {
        let _ = std::fs::create_dir_all(dir);
        Tour { steps: default_steps(), i: 0, dir: dir.to_string(), width, height, step_start: Instant::now(), travel: None, report: Vec::new() }
    }

    pub fn done(&self) -> bool {
        self.i >= self.steps.len()
    }

    fn next(&mut self) {
        self.i += 1;
        self.step_start = Instant::now();
    }

    /// Ejecuta los pasos que se puedan en este cuadro. Devuelve `true` al terminar.
    pub fn step(&mut self, s: &mut Session, r: &mut Renderer) -> bool {
        while !self.done() {
            match &self.steps[self.i] {
                Act::Cam(c) => {
                    s.cam.target.x = c[0];
                    s.cam.target.z = c[1];
                    s.cam.target.y = s.terrain.height(c[0], c[1]).max(0.0);
                    s.cam.distance = c[2];
                    s.cam.yaw = c[3];
                    s.cam.pitch = c[4];
                    s.update(0.0);
                    self.next();
                }
                Act::Hour(h) => {
                    s.hour = *h;
                    self.next();
                }
                Act::Smooth(on) => {
                    r.smooth_terrain = *on;
                    self.next();
                }
                Act::WaitIdle(label) => {
                    // La escena tiene que estar cargada y todo lo visible generado.
                    if !(s.scene_loaded() && s.streamer.idle()) {
                        if self.step_start.elapsed() > Duration::from_secs(300) {
                            self.report.push(format!("{label}: ¡tiempo agotado esperando el streaming! ({} pendientes)", s.streamer.pending()));
                            self.next();
                        }
                        return false;
                    }
                    crate::session::trim_memory();
                    let (rss, peak) = memory_mb();
                    let line = format!(
                        "{label}: listo en {:.2?} · {} columnas pedidas, {} con malla · {:.2} M triángulos · mallas {:.0} MB · memoria {rss:.0} MB (máx. {peak:.0} MB)",
                        self.step_start.elapsed(),
                        s.streamer.wanted_columns(),
                        r.column_count(),
                        r.triangles() as f32 / 1e6,
                        r.mesh_mb()
                    );
                    println!("{line}");
                    self.report.push(line);
                    self.next();
                }
                Act::Shot(name) => {
                    r.set_overlay(&s.overlay());
                    let path = format!("{}/{}.png", self.dir, name);
                    let t0 = Instant::now();
                    match r.capture(&s.cam, s.hour, self.width, self.height, &path) {
                        Ok(()) => println!("Captura {path} ({:.2?})", t0.elapsed()),
                        Err(e) => eprintln!("Error en la captura {path}: {e}"),
                    }
                    self.next();
                }
                Act::Place(kind, rot, x, z) => {
                    s.tool = *kind;
                    s.rot = *rot;
                    s.pick_xz(*x, *z);
                    let n = s.place();
                    self.report.push(format!("colocar {} en ({x:.0}, {z:.0}): {:?} columnas re-malladas", s.tool_name(), n));
                    self.next();
                }
                Act::PlaceCenter(kind, rot) => {
                    s.tool = *kind;
                    s.rot = *rot;
                    let (o, d) = s.cam.ray(self.width as f32 * 0.5, self.height as f32 * 0.5, self.width as f32, self.height as f32);
                    let hit = s.pick(o, d);
                    let n = s.place();
                    self.report.push(format!("colocar {} con el rayo del centro de pantalla (vóxel {:?}): {:?} columnas re-malladas", s.tool_name(), hit.map(|h| h.voxel), n));
                    self.next();
                }
                Act::Dig(x, z, rad) => {
                    s.pick_xz(*x, *z);
                    let n = s.dig(*rad);
                    self.report.push(format!("excavar en ({x:.0}, {z:.0}) radio {rad}: {:?} columnas re-malladas", n));
                    self.next();
                }
                Act::Fill(x, z, rad) => {
                    s.pick_xz(*x, *z);
                    let n = s.fill(*rad);
                    self.report.push(format!("rellenar en ({x:.0}, {z:.0}) radio {rad}: {:?} columnas re-malladas", n));
                    self.next();
                }
                Act::Travel(x, z, frames) => {
                    let (from, to, k, n) = *self.travel.get_or_insert((Vec2::new(s.cam.target.x, s.cam.target.z), Vec2::new(*x, *z), 0, *frames));
                    let p = from.lerp(to, (k + 1) as f32 / n as f32);
                    s.cam.target.x = p.x;
                    s.cam.target.z = p.y;
                    s.update(1.0 / 30.0);
                    if k + 1 >= n {
                        self.travel = None;
                        self.report.push(format!("viaje de {:.0} m en {n} cuadros: {:.2?}", from.distance(to), self.step_start.elapsed()));
                        self.next();
                    } else {
                        self.travel = Some((from, to, k + 1, n));
                    }
                    return false;
                }
            }
        }
        true
    }
}
