//! Recorrido automático para probar el motor sin intervención: mueve la cámara, construye,
//! excava, viaja lejos (streaming) y guarda capturas en puntos clave con métricas.
//! Funciona igual con ventana o sin pantalla.

use crate::render::Renderer;
use crate::ui::{self, paint::UiPainter, GameUi};
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
    /// Una acción del modo juego (fundar, trazar, zonificar, crecer); devuelve lo que pasó.
    Game(&'static str, fn(&mut Session) -> String),
    /// Captura con la interfaz del juego encima, después de prepararla (herramienta, ficha,
    /// avisos, panel) con la función dada.
    UiShot(&'static str, fn(&mut Session, &mut GameUi)),
}

/// Interfaz para las capturas sin pantalla: contexto de egui, pintor y estado del juego.
struct TourUi {
    ctx: egui::Context,
    painter: UiPainter,
    game_ui: GameUi,
    ms: Vec<f64>,
}

/// Recorrido del modo juego: funda la plaza, traza un damero y una calle curva, zonifica,
/// hace crecer la ciudad y saca capturas (general, de cerca sobre la curva y de noche).
pub fn city_steps() -> Vec<Act> {
    use crate::city::game_demo as d;
    use Act::*;
    let c = d::PLAZA;
    vec![
        Cam([c.x, c.y, 700.0, 200.0, 45.0]),
        Hour(10.5),
        WaitIdle("terreno virgen"),
        Game("fundar la plaza", d::found),
        Game("damero", d::grid),
        Game("calle curva", d::curve),
        Game("zonificar", d::zone),
        WaitIdle("trazado"),
        Shot("motor-ciudad-trazado"),
        Game("crecer 60 meses", d::grow),
        WaitIdle("ciudad crecida"),
        Game("un mes al ritmo del juego", d::month_realtime),
        WaitIdle("re-mallado de un mes"),
        Cam([c.x + 60.0, c.y + 40.0, 620.0, 215.0, 42.0]),
        WaitIdle("vista general"),
        Shot("motor-ciudad-general"),
        Cam([d::CURVE_VIEW.x, d::CURVE_VIEW.y, 110.0, 160.0, 38.0]),
        WaitIdle("curva de cerca"),
        Shot("motor-ciudad-curva"),
        Cam([c.x, c.y, 150.0, 200.0, 32.0]),
        WaitIdle("plaza de cerca"),
        Shot("motor-ciudad-plaza"),
        Cam([c.x + 60.0, c.y + 40.0, 520.0, 215.0, 40.0]),
        Hour(20.5),
        WaitIdle("noche"),
        Shot("motor-ciudad-noche"),
        Hour(10.5),
        Cam([c.x + 40.0, c.y + 30.0, 330.0, 205.0, 40.0]),
        WaitIdle("vista para la interfaz"),
        UiShot("motor-ui-barra", ui_toolbar),
        UiShot("motor-ui-cajon-servicios", ui_services),
        UiShot("motor-ui-ficha", ui_inspect),
        UiShot("motor-ui-gaceta", ui_gazette),
        UiShot("motor-ui-panel", ui_panel),
    ]
}

/// Barra de categorías con el cajón de calles abierto (calle recta elegida).
fn ui_toolbar(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(1);
    u.open = Some(ui::Category::Calles);
    g.hover = Some(crate::city::game_demo::PLAZA + Vec2::new(60.0, 90.0));
    u.sync_news(g);
    u.time += 30.0; // sin avisos en esta captura
}

/// Cajón de servicios: tarjetas de lo que llega más adelante, desactivadas con el motivo.
fn ui_services(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(4);
    g.hover = Some(crate::city::game_demo::PLAZA + Vec2::new(60.0, 90.0));
    u.open = Some(ui::Category::Servicios);
}

/// Ficha de un solar que todavía puede crecer (el de nivel más bajo cerca de la plaza).
fn ui_inspect(s: &mut Session, u: &mut GameUi) {
    u.open = None;
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    let c = crate::city::game_demo::PLAZA;
    let lot = g.city.lots.lots.values().filter(|l| l.level == 1).min_by_key(|l| l.c.distance(c + Vec2::new(-40.0, -120.0)) as i32).map(|l| l.c);
    g.inspected = lot;
    g.hover = lot;
}

/// Un aviso de la gaceta y una calle recta empezada que no se puede trazar (cruza el río).
fn ui_gazette(s: &mut Session, u: &mut GameUi) {
    let terrain = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    g.inspected = None;
    g.set_tool(1);
    let c = crate::city::game_demo::PLAZA;
    let _ = g.press(&terrain, c + Vec2::new(0.0, 160.0));
    g.hover = Some(Vec2::new(c.x, 19000.0));
    g.say("Llegaron 40 pobladores en la fragata «Santa Rosa»; piden solares cerca del puerto.");
    u.sync_news(g);
}

/// El panel genérico de decisiones con el ejemplo de ordenanzas.
fn ui_panel(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.cancel();
    g.set_tool(7);
    g.hover = None;
    u.time += 30.0;
    u.list.open = true;
    u.open = Some(ui::Category::Ordenanzas);
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
    ui: Option<TourUi>,
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
    pub fn with_steps(dir: &str, width: u32, height: u32, steps: Vec<Act>) -> Tour {
        let _ = std::fs::create_dir_all(dir);
        Tour { steps, i: 0, dir: dir.to_string(), width, height, step_start: Instant::now(), travel: None, report: Vec::new(), ui: None }
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
                Act::Game(label, f) => {
                    let t0 = Instant::now();
                    let out = f(s);
                    let line = format!("{label}: {out} ({:.2?})", t0.elapsed());
                    println!("{line}");
                    self.report.push(line);
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
                Act::UiShot(name, setup) => {
                    let u = self.ui.get_or_insert_with(|| {
                        let ctx = egui::Context::default();
                        ui::theme::install(&ctx);
                        TourUi { ctx, painter: UiPainter::new(r.device(), r.format()), game_ui: GameUi::default(), ms: Vec::new() }
                    });
                    setup(s, &mut u.game_ui);
                    r.set_overlay(&s.overlay());
                    let ppp = ui::zoom_for(self.height, 1.0);
                    let (w, h) = (self.width, self.height);
                    let terrain = s.terrain.clone();
                    let hour = s.hour;
                    let g = s.game.as_ref().unwrap();
                    // Varias pasadas: egui mide las áreas en la primera y las muestra en la siguiente.
                    // Las texturas (la fuente) de las pasadas descartadas se suben en orden.
                    let mut full = None;
                    let t0 = Instant::now();
                    for k in 0..3 {
                        u.game_ui.time += 0.05;
                        let mut out = u.ctx.run(ui::headless_input(w, h, ppp, u.game_ui.time), |ctx| {
                            let _ = u.game_ui.draw(ctx, g, &terrain, hour, "");
                        });
                        if k == 2 {
                            full = Some(out);
                        } else {
                            u.painter.update_textures(r.device(), r.queue(), &std::mem::take(&mut out.textures_delta));
                        }
                    }
                    let full = full.unwrap();
                    let ms_run = t0.elapsed().as_secs_f64() * 1000.0 / 3.0;
                    let path = format!("{}/{}.png", self.dir, name);
                    let (ctx, painter) = (&u.ctx, &mut u.painter);
                    let mut paint_ms = 0.0;
                    let mut out = Some(full);
                    let mut hook = |d: &wgpu::Device, q: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size: (u32, u32)| {
                        if let Some(f) = out.take() {
                            let t = Instant::now();
                            painter.paint(d, q, enc, view, size, ctx, f);
                            paint_ms = t.elapsed().as_secs_f64() * 1000.0;
                        }
                    };
                    match r.capture_with(&s.cam, s.hour, w, h, &path, Some(&mut hook)) {
                        Ok(()) => {
                            u.ms.push(ms_run + paint_ms);
                            let line = format!("Captura con interfaz {path}: interfaz {:.2} ms de CPU (egui {ms_run:.2} + pintar {paint_ms:.2})", ms_run + paint_ms);
                            println!("{line}");
                            self.report.push(line);
                        }
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
