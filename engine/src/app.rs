//! Modo ventana: winit + superficie wgpu, cámara interactiva y construcción con el ratón.
//!
//! Controles:
//!   WASD / flechas  mover          Q / E    girar          rueda   acercar / alejar
//!   botón central   orbitar        botón derecho: arrastrar = mover el mapa, clic = excavar
//!   clic izquierdo  colocar        1–9 / Tab  elegir estructura    R  girarla
//!   F  rellenar terreno            [ / ]  hora del día      P  pausar el día
//!   F12  captura PNG               Esc  salir
//!
//! Modo juego (`motor --juego`, por omisión): 1 plaza · 2 recta · 3 curva · 4 damero ·
//!   5 zona · 6 quitar zona · 7 demoler · 8 inspeccionar · Tab tipo de calle · R girar la
//!   plaza · [ ] pincel · , . hora · Espacio pausa · 0 velocidad · clic derecho/Esc cancelar
//!   · Esc (sin nada que cancelar) menú de pausa · F5 guardar · F9 cargar la última partida
//!   · M mapa del país (ver `pais::ui`)
//!
//! Sin argumentos el juego abre en el menú principal (ver `menu.rs`).

use crate::menu::{self, Menu, MenuAction, Outcome};
use crate::render::Renderer;
use crate::session::{memory_mb, Session};
use crate::sandbox::CATALOG;
use crate::tour::Tour;
use crate::ui::{self, paint::UiPainter, GameUi};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub struct Options {
    pub size: (u32, u32),
    pub msaa: u32,
    pub frames: Option<u64>,
    pub tour_dir: Option<String>,
}

struct Gpu {
    window: Arc<Window>,
    renderer: Renderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    egui: egui_winit::State,
    painter: UiPainter,
}

struct App {
    opts: Options,
    session: Session,
    gpu: Option<Gpu>,
    tour: Option<Tour>,
    keys: HashSet<KeyCode>,
    mouse: PhysicalPosition<f64>,
    middle: bool,
    right: Option<PhysicalPosition<f64>>,
    right_moved: f64,
    last: Instant,
    frames: u64,
    fps_frames: u32,
    fps_time: Instant,
    fps: f32,
    /// Perfil del cuadro: el peor del último medio segundo y su desglose.
    worst: (f32, String),
    perfil: bool,
    /// Distancia de cámara a la que va el zoom suave.
    zoom_goal: Option<f32>,
    fps_samples: Vec<f32>,
    error: Option<String>,
    /// Clave del último cuadro dibujado y cuándo se dibujó (para no redibujar lo mismo).
    last_key: Option<[u64; 4]>,
    last_draw: Instant,
    /// La escena está quieta: el bucle duerme en vez de girar (la CPU baja casi a cero).
    idle: bool,
    pub skipped: u64,
    ui: GameUi,
    /// La interfaz pidió redibujar (animación, avisos, cursor sobre un botón…).
    ui_repaint: bool,
    /// Milisegundos de CPU de la interfaz por cuadro (egui + teselado + subida).
    ui_ms: Vec<f64>,
    /// Menú principal y de pausa, partidas guardadas y opciones.
    menu: Menu,
    /// Acciones del menú de este cuadro, y la pesada que espera un cuadro (aviso «Cargando…»).
    menu_acts: Vec<MenuAction>,
    pending: Option<MenuAction>,
    /// [lod] Auto-ajuste de las perillas (ver `autoajuste.rs`), cuándo se dibujó el último
    /// cuadro, si hubo cuadros sin dibujar desde entonces, y el intervalo del vsync (ms).
    tuner: crate::autoajuste::Autoajuste,
    last_drawn: Option<Instant>,
    skipped_since: bool,
    tope_ms: f64,
}

/// Con la escena quieta solo se redibuja para animar el agua, a este intervalo.
const IDLE_REDRAW: std::time::Duration = std::time::Duration::from_millis(100);

pub fn run(session: Session, opts: Options, menu: Menu) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("no se pudo abrir el sistema de ventanas: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let steps = || if session.game.is_some() { crate::tour::city_steps() } else { crate::tour::default_steps() };
    let tour = opts.tour_dir.as_ref().map(|d| Tour::with_steps(d, opts.size.0, opts.size.1, steps()));
    let techo = menu.settings.techo(session.streamer.threads());
    let mut app = App {
        opts,
        session,
        gpu: None,
        tour,
        keys: HashSet::new(),
        mouse: PhysicalPosition::new(0.0, 0.0),
        middle: false,
        right: None,
        right_moved: 0.0,
        last: Instant::now(),
        frames: 0,
        fps_frames: 0,
        fps_time: Instant::now(),
        worst: (0.0, String::new()),
        perfil: std::env::var("CITYSIM_PERFIL").is_ok(),
        zoom_goal: None,
        fps: 0.0,
        fps_samples: Vec::new(),
        error: None,
        last_key: None,
        last_draw: Instant::now(),
        idle: false,
        skipped: 0,
        ui: GameUi::default(),
        ui_repaint: true,
        ui_ms: Vec::new(),
        tuner: crate::autoajuste::Autoajuste::new(techo, menu.settings.fps_objetivo as f64),
        menu,
        menu_acts: Vec::new(),
        pending: None,
        last_drawn: None,
        skipped_since: false,
        tope_ms: 0.0,
    };
    app.tuner.activo = app.menu.settings.autoajuste;
    event_loop.run_app(&mut app).map_err(|e| format!("error en el bucle de eventos: {e}"))?;
    if let Some(e) = app.error {
        return Err(e);
    }
    let (rss, peak) = memory_mb();
    let avg = if app.fps_samples.is_empty() { 0.0 } else { app.fps_samples.iter().sum::<f32>() / app.fps_samples.len() as f32 };
    println!(
        "Ventana cerrada tras {} cuadros · FPS medio {avg:.1} · primer cuadro con terreno a los {:.2?} · memoria {rss:.0} MB (máx. {peak:.0} MB)",
        app.frames,
        app.session.first_frame.unwrap_or_default()
    );
    if !app.ui_ms.is_empty() {
        let mut v = app.ui_ms.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("Interfaz: {:.2} ms de CPU por cuadro de media (mediana {:.2}, peor {:.2}) en {} cuadros", v.iter().sum::<f64>() / v.len() as f64, v[v.len() / 2], v[v.len() - 1], v.len());
    }
    if let Some(t) = &app.tour {
        for l in &t.report {
            println!("  {l}");
        }
    }
    Ok(())
}

impl App {
    fn frame(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        // Tras un reposo (sin cambios, se redibuja cada 100 ms) el intervalo no es un tirón.
        let frame_gap = if self.idle { 0.0 } else { (now - self.last).as_secs_f32() * 1000.0 };
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let t_frame = now;
        let s = &mut self.session;
        // El menú detiene la simulación (pausa) o gira la cámara de fondo (menú principal).
        self.menu.sync_pause(s);
        self.menu.animate_background(s, dt);
        self.menu.autosave(s);
        // [mapa del país] Con el mapa del país abierto, el ratón y la cámara son del mapa.
        let menu_open = self.menu.is_open() || self.ui.pais.abierto;
        // Cámara con el teclado (no con el menú abierto).
        let speed = s.cam.distance * 0.9 * dt;
        let k = |c| (self.keys.contains(&c) && !menu_open) as i32 as f32;
        let fwd = k(KeyCode::KeyW) + k(KeyCode::ArrowUp) - k(KeyCode::KeyS) - k(KeyCode::ArrowDown);
        let right = k(KeyCode::KeyD) + k(KeyCode::ArrowRight) - k(KeyCode::KeyA) - k(KeyCode::ArrowLeft);
        s.cam.pan(right * speed, fwd * speed);
        s.cam.yaw += (k(KeyCode::KeyQ) - k(KeyCode::KeyE)) * 80.0 * dt;
        if let Some(goal) = self.zoom_goal {
            let d = s.cam.distance;
            let step = 1.0 - (-dt * 14.0).exp();
            let next = d * (goal / d).powf(step);
            s.cam.zoom(next / d);
            if (goal / s.cam.distance - 1.0).abs() < 0.002 {
                s.cam.zoom(goal / s.cam.distance);
                self.zoom_goal = None;
            }
        }
        let t0 = Instant::now();
        s.update(dt);
        let ms_sim = t0.elapsed().as_secs_f32() * 1000.0;
        let Some(gpu) = self.gpu.as_mut() else { return };
        s.view_h = gpu.config.height as f32;
        let t0 = Instant::now();
        // Tope de vértices subidos por cuadro: repartir la carga entre cuadros evita tirones.
        s.apply_ops(&mut gpu.renderer, crate::session::WINDOW_VERTEX_BUDGET);
        let ms_upload = t0.elapsed().as_secs_f32() * 1000.0;
        let t0 = Instant::now();
        gpu.renderer.time += dt;
        // Selección bajo el cursor (no durante el recorrido automático ni con el menú abierto).
        if self.tour.is_none() && !menu_open {
            let (w, h) = (gpu.config.width as f32, gpu.config.height as f32);
            let (o, d) = s.cam.ray(self.mouse.x as f32, self.mouse.y as f32, w, h);
            s.pick(o, d);
        }
        gpu.renderer.set_overlay(&if self.menu.in_main_menu() { Vec::new() } else { s.overlay() });
        let ms_pick = t0.elapsed().as_secs_f32() * 1000.0;
        if let Some(t) = self.tour.as_mut() {
            if t.step(s, &mut gpu.renderer) && self.opts.frames.is_none() {
                el.exit();
            }
        }
        // Clave del cuadro: si nada cambió (y no hay recorrido automático), no se dibuja; solo
        // se redibuja cada IDLE_REDRAW para que el agua siga moviéndose.
        let key = gpu.renderer.frame_key(&s.cam, s.hour, (gpu.config.width, gpu.config.height));
        let same = self.last_key == Some(key) && self.tour.is_none() && s.streamer.pending() == 0 && !self.ui_repaint;
        self.idle = same;
        if same && self.last_draw.elapsed() < IDLE_REDRAW {
            self.skipped += 1;
            self.skipped_since = true;
            return;
        }
        if same {
            self.skipped_since = true;
        }
        self.last_key = Some(key);
        self.last_draw = Instant::now();
        // Interfaz del juego (solo en el modo juego): se arma, se aplican sus acciones y se
        // pinta encima de la escena en la misma entrega.
        let t_ui = Instant::now();
        let mut ui_out = None;
        if s.game.is_some() || menu_open || self.menu.settings.mostrar_fps {
            self.ui.time += dt as f64;
            if let Some(g) = s.game.as_ref() {
                self.ui.sync_news(g);
            }
            let raw = gpu.egui.take_egui_input(&gpu.window);
            let ctx = gpu.egui.egui_ctx().clone();
            ctx.set_zoom_factor(ui::zoom_for(gpu.config.height, gpu.window.scale_factor() as f32) * self.menu.settings.escala_interfaz);
            let terrain = s.terrain.clone();
            let hour = s.hour;
            let mut acts = Vec::new();
            let mut macts = Vec::new();
            let ui_state = &mut self.ui;
            let menu = &mut self.menu;
            let fps = self.fps;
            let full = ctx.run(raw, |ctx| {
                // La interfaz del juego queda detrás del menú de pausa; no del principal.
                if let Some(g) = s.game.as_ref().filter(|_| !menu.in_main_menu()) {
                    acts = ui_state.draw(ctx, g, &terrain, hour, "");
                }
                macts = menu.draw(ctx, s.game.as_ref(), fps);
            });
            if let Some(g) = s.game.as_mut() {
                let delta = ui::apply(&acts, g, &mut self.ui.list);
                s.hour = (s.hour + delta).rem_euclid(24.0);
            }
            self.menu_acts.extend(macts);
            let repaint = full.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.repaint_delay < std::time::Duration::from_millis(500));
            self.ui_repaint = repaint || !self.ui.toasts().is_empty() || !acts.is_empty() || !self.menu_acts.is_empty() || self.pending.is_some();
            gpu.egui.handle_platform_output(&gpu.window, full.platform_output.clone());
            ui_out = Some((ctx, full));
        }
        let has_ui = ui_out.is_some();
        let Gpu { renderer, surface, config, painter, .. } = gpu;
        let mut cpu_ui = t_ui.elapsed().as_secs_f64() * 1000.0;
        let mut hook = |d: &wgpu::Device, q: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size: (u32, u32)| {
            if let Some((ctx, full)) = ui_out.take() {
                let t0 = Instant::now();
                painter.paint(d, q, enc, view, size, &ctx, full);
                cpu_ui += t0.elapsed().as_secs_f64() * 1000.0;
            }
        };
        let t0 = Instant::now();
        if let Err(e) = renderer.render_surface(surface, config, &s.cam, s.hour, has_ui.then_some(&mut hook as crate::render::UiHook)) {
            eprintln!("{e}");
        }
        drop(hook);
        let ms_render = t0.elapsed().as_secs_f32() * 1000.0 - cpu_ui as f32;
        // Perfil: el cuadro más lento del medio segundo y dónde se fue el tiempo.
        let total = t_frame.elapsed().as_secs_f32() * 1000.0;
        // [lod] Auto-ajuste: un cuadro dibujado seguido del anterior (sin reposo entre medio).
        let drawn = Instant::now();
        let interval = self.last_drawn.map(|t| (drawn - t).as_secs_f64() * 1000.0);
        let muestra = crate::autoajuste::Muestra {
            cuadro_ms: interval.unwrap_or(0.0),
            trabajo_ms: (total as f64 - renderer.last_wait_ms).max(0.0),
            gpu_ms: renderer.last_gpu_ms,
            tope_ms: self.tope_ms,
            dibujado: interval.is_some() && !self.skipped_since,
            menu: menu_open || self.tour.is_some(),
        };
        self.last_drawn = Some(drawn);
        self.skipped_since = false;
        if let Some(c) = self.tuner.muestra(muestra) {
            if self.perfil {
                println!("Auto-ajuste: {} {} ({})", c.perilla.nombre(), if c.sube { "↑" } else { "↓" }, c.motivo);
            }
        }
        apply_knobs(&self.tuner.actual, &mut s.streamer, renderer, self.menu.settings.cache_mb);
        let parts = [("simulación", ms_sim), ("subir mallas", ms_upload), ("selección", ms_pick), ("interfaz", cpu_ui as f32), ("dibujo", ms_render)];
        let (who, _) = parts.iter().cloned().fold(("", 0.0f32), |a, b| if b.1 > a.1 { b } else { a });
        if frame_gap > self.worst.0 {
            self.worst = (frame_gap, if total > 12.0 { who.to_string() } else { String::new() });
        }
        if self.perfil && frame_gap > 25.0 {
            println!(
                "Cuadro lento: {frame_gap:.1} ms entre cuadros · trabajo {total:.1} ms (simulación {ms_sim:.1}, subir mallas {ms_upload:.1}, selección {ms_pick:.1}, interfaz {:.1}, dibujo {ms_render:.1}) · pendientes {} · cascadas de sombra dibujadas {} · Hi-Z {}",
                cpu_ui,
                s.streamer.pending(),
                renderer.last_shadow_passes,
                if renderer.hiz && renderer.draw_mode == crate::render::DrawMode::GpuCull { "sí" } else { "no" }
            );
        }
        if has_ui {
            self.ui_ms.push(cpu_ui);
        }
        self.frames += 1;
        self.fps_frames += 1;
        if self.fps_time.elapsed().as_secs_f32() >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_time.elapsed().as_secs_f32();
            self.fps_samples.push(self.fps);
            self.menu.frame_worst = std::mem::take(&mut self.worst);
            self.menu.autoajuste = self.tuner.estado();
            // [lod] Intervalo del vsync (el objetivo nunca puede ser menor).
            self.tope_ms = if self.menu.settings.sincronizacion_vertical { gpu.window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).map_or(1000.0 / 60.0, |mhz| 1.0e6 / mhz as f64) } else { 0.0 };
            self.fps_frames = 0;
            self.fps_time = Instant::now();
            let title = if s.game.is_none() && self.menu.is_open() { format!("City Sim 2027 · {:.0} FPS", self.fps) } else { s.title(self.fps, &gpu.renderer) };
            gpu.window.set_title(&title);
        }
        if self.opts.frames.is_some_and(|n| self.frames >= n) {
            el.exit();
        }
        self.run_menu_actions(el);
    }

    /// Hace lo que pidió el menú. Empezar o cargar una partida se deja para el cuadro
    /// siguiente, así se llega a ver el aviso «Cargando…».
    fn run_menu_actions(&mut self, el: &ActiveEventLoop) {
        let ready = self.pending.take();
        for a in std::mem::take(&mut self.menu_acts) {
            if a.heavy() && ready.is_none() {
                self.menu.loading = Some(match &a {
                    MenuAction::NewGame(n) => format!("Fundando {n}…"),
                    MenuAction::Leave(_) => "Volviendo al menú principal…".into(),
                    _ => "Cargando partida…".into(),
                });
                self.pending = Some(a);
                self.ui_repaint = true;
            } else {
                self.menu_action(a, el);
            }
        }
        if let Some(a) = ready {
            self.menu_action(a, el);
        }
    }

    fn menu_action(&mut self, a: MenuAction, el: &ActiveEventLoop) {
        let renderer = self.gpu.as_mut().map(|g| &mut g.renderer);
        match self.menu.perform(a, &mut self.session, renderer, &mut self.ui) {
            Outcome::Quit => el.exit(),
            Outcome::Settings => {
                // [lod] "Valores por omisión" deja la calidad sin elegir: se vuelve a recomendar.
                if self.menu.settings.calidad.is_none() {
                    if let Some(c) = self.menu.recomendada {
                        self.menu.settings.aplicar_calidad(c, crate::autoajuste::nucleos());
                        let _ = self.menu.settings.store(&self.menu.base);
                        self.menu.draft = self.menu.settings.clone();
                    }
                }
                self.tuner.set_techo(self.menu.settings.techo(self.session.streamer.threads()), self.menu.settings.fps_objetivo as f64);
                self.tuner.activo = self.menu.settings.autoajuste;
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.renderer.set_shadow_quality(self.menu.settings.shadow_quality());
                    gpu.renderer.voxel_terrain = self.menu.settings.terreno_voxeles;
                    let mode = if self.menu.settings.sincronizacion_vertical { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
                    if gpu.config.present_mode != mode {
                        gpu.config.present_mode = mode;
                        gpu.renderer.configure(&gpu.surface, &gpu.config);
                    }
                }
            }
            Outcome::None => {}
        }
        self.last_key = None;
        self.ui_repaint = true;
    }

    /// Esc, F5 y F9 con partida o con el menú abierto. Con el menú abierto se traga las
    /// demás teclas (no mueven la cámara ni eligen herramientas).
    fn menu_key(&mut self, code: KeyCode) -> bool {
        let has_game = self.session.game.is_some();
        if !has_game && !self.menu.is_open() {
            return false;
        }
        match code {
            KeyCode::Escape => {
                if let Some(a) = menu::escape(self.session.game.as_mut(), &mut self.ui, &mut self.menu) {
                    self.menu_acts.push(a);
                }
            }
            KeyCode::F5 if has_game && !self.menu.in_main_menu() => self.menu_acts.push(MenuAction::Save),
            KeyCode::F9 if self.pending.is_none() => self.menu_acts.push(MenuAction::QuickLoad),
            _ => return self.menu.is_open(),
        }
        self.ui_repaint = true;
        true
    }

    /// Teclas del modo juego (como en el prototipo).
    fn game_key(&mut self, code: KeyCode) -> bool {
        let s = &mut self.session;
        let Some(g) = s.game.as_mut() else { return false };
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8];
        if let Some(n) = digits.iter().position(|c| *c == code) {
            g.set_tool(n);
            return true;
        }
        match code {
            KeyCode::Tab => {
                g.kind = if g.kind == crate::city::roads::RoadKind::Dirt { crate::city::roads::RoadKind::Stone } else { crate::city::roads::RoadKind::Dirt };
            }
            KeyCode::KeyR => g.plaza_angle += 15f32.to_radians(),
            KeyCode::BracketLeft => g.brush = (g.brush - 10.0).max(10.0),
            KeyCode::BracketRight => g.brush = (g.brush + 10.0).min(150.0),
            KeyCode::Space => g.speed = if g.speed > 0.0 { 0.0 } else { 1.0 },
            KeyCode::Digit0 => g.speed = if g.speed >= 4.0 { 1.0 } else { g.speed * 2.0 }.max(1.0),
            KeyCode::Comma => s.hour = (s.hour - 0.5).rem_euclid(24.0),
            KeyCode::Period => s.hour = (s.hour + 0.5).rem_euclid(24.0),
            _ => return false,
        }
        true
    }

    fn key(&mut self, code: KeyCode, el: &ActiveEventLoop) {
        // [mapa del país] M abre y cierra el mapa del país; abierto, Esc vuelve a la ciudad y las
        // demás teclas son del mapa (las lee egui).
        if self.session.game.is_some() && !self.menu.is_open() {
            if code == KeyCode::KeyM {
                self.ui.pais.alternar();
                self.ui_repaint = true;
                return;
            }
            if self.ui.pais.abierto {
                if code == KeyCode::Escape {
                    self.ui.pais.escape();
                }
                self.ui_repaint = true;
                return;
            }
        }
        if self.menu_key(code) || self.game_key(code) {
            return;
        }
        let s = &mut self.session;
        match code {
            KeyCode::Escape => el.exit(),
            KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 | KeyCode::Digit4 | KeyCode::Digit5 | KeyCode::Digit6 | KeyCode::Digit7 | KeyCode::Digit8 | KeyCode::Digit9 => {
                let n = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9].iter().position(|c| *c == code).unwrap();
                s.tool = n.min(CATALOG.len() - 1);
            }
            KeyCode::Tab => s.tool = (s.tool + 1) % CATALOG.len(),
            KeyCode::KeyR => s.rot = (s.rot + 1) % 4,
            KeyCode::KeyF => {
                s.fill(4);
            }
            KeyCode::BracketLeft => s.hour = (s.hour - 0.5).rem_euclid(24.0),
            KeyCode::BracketRight => s.hour = (s.hour + 0.5).rem_euclid(24.0),
            KeyCode::KeyP => s.day_paused = !s.day_paused,
            KeyCode::F12 => {
                if let Some(gpu) = &self.gpu {
                    let path = format!("captura-{}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
                    match gpu.renderer.capture(&s.cam, s.hour, gpu.config.width, gpu.config.height, &path) {
                        Ok(()) => println!("Captura guardada en {path}"),
                        Err(e) => eprintln!("{e}"),
                    }
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title(format!("{} · {}", if self.menu.is_open() { "City Sim 2027" } else { "Motor de vóxeles" }, crate::version::completa())).with_inner_size(LogicalSize::new(self.opts.size.0, self.opts.size.1));
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("no se pudo crear la ventana: {e}"));
                el.exit();
                return;
            }
        };
        // Sin --msaa, el suavizado de las opciones (0 = automático).
        let msaa = if self.opts.msaa == 0 { self.menu.settings.msaa } else { self.opts.msaa };
        match Renderer::for_window(window.clone(), msaa) {
            Ok((mut renderer, surface, mut config)) => {
                // [lod] Calidad recomendada para este equipo; la primera vez se aplica y se avisa.
                let hw = crate::autoajuste::Hardware { adaptador: crate::autoajuste::Adaptador::de_wgpu(renderer.device_type), nucleos: crate::autoajuste::nucleos(), ram_gb: crate::autoajuste::ram_total_gb() };
                let rec = crate::autoajuste::recomendar(&hw);
                self.menu.recomendada = Some(rec);
                if self.menu.settings.calidad.is_none() {
                    self.menu.settings.aplicar_calidad(rec, hw.nucleos);
                    let _ = self.menu.settings.store(&self.menu.base);
                    self.menu.draft = self.menu.settings.clone();
                    self.menu.status = Some((format!("Calidad «{}», recomendada para este equipo (se cambia en Opciones)", rec.nombre()), false));
                    println!("Calidad recomendada para este equipo ({:?}, {} núcleos): {}", hw.adaptador, hw.nucleos, rec.nombre());
                }
                self.tuner.set_techo(self.menu.settings.techo(self.session.streamer.threads()), self.menu.settings.fps_objetivo as f64);
                self.tuner.activo = self.menu.settings.autoajuste;
                // En la ventana, a lo sumo 2 cascadas de sombra por cuadro (las lejanas esperan).
                renderer.shadow_budget = crate::render::SHADOW_BUDGET_WINDOW;
                renderer.set_shadow_quality(self.menu.settings.shadow_quality());
                renderer.voxel_terrain = self.menu.settings.terreno_voxeles;
                if !self.menu.settings.sincronizacion_vertical {
                    config.present_mode = wgpu::PresentMode::AutoNoVsync;
                    renderer.configure(&surface, &config);
                }
                println!("Ventana {}×{} · adaptador: {} · formato {:?}", config.width, config.height, renderer.adapter_name, config.format);
                let ctx = egui::Context::default();
                ui::theme::install(&ctx);
                let max_tex = renderer.device().limits().max_texture_dimension_2d as usize;
                let egui = egui_winit::State::new(ctx, egui::ViewportId::ROOT, &window, Some(window.scale_factor() as f32), None, Some(max_tex));
                let painter = UiPainter::new(renderer.device(), renderer.format());
                self.gpu = Some(Gpu { window, renderer, surface, config, egui, painter });
            }
            Err(e) => {
                self.error = Some(e);
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Cualquier entrada saca al bucle del reposo (el resaltado sigue al cursor, etc.).
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.idle = false;
            self.ui_repaint = true;
        }
        // La interfaz ve el evento primero; si el cursor está sobre un panel, el mundo no.
        // [mapa del país] Con el mapa del país abierto, el ratón y la cámara son del mapa.
        let menu_open = self.menu.is_open() || self.ui.pais.abierto;
        let (ui_pointer, ui_keys) = match (self.gpu.as_mut(), self.session.game.is_some() || menu_open || self.menu.settings.mostrar_fps) {
            (Some(gpu), true) => {
                let _ = gpu.egui.on_window_event(&gpu.window, &event);
                let ctx = gpu.egui.egui_ctx();
                // Se mira la posición actual del cursor contra las áreas del último cuadro: egui
                // todavía no procesó este evento, y un clic puede llegar sin cuadros entre medio.
                let ppp = ctx.pixels_per_point();
                let at = egui::pos2(self.mouse.x as f32 / ppp, self.mouse.y as f32 / ppp);
                // La capa de fondo cubre toda la pantalla: solo cuenta dentro de la barra superior.
                let over = ctx.layer_id_at(at).is_some_and(|l| l.order != egui::Order::Background || at.y <= ui::TOP_BAR_H);
                (over || ctx.is_using_pointer() || menu_open, ctx.wants_keyboard_input())
            }
            _ => (false, false),
        };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.renderer.configure(&gpu.surface, &gpu.config);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if !ui_keys => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if !event.repeat {
                            self.key(code, el);
                        }
                        self.keys.insert(code);
                    } else {
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                // Shift: trazar las calles libres, sin enganches.
                if let Some(g) = self.session.game.as_mut() {
                    g.free_draw = m.state().shift_key();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (dx, dy) = ((position.x - self.mouse.x) as f32, (position.y - self.mouse.y) as f32);
                self.mouse = position;
                let s = &mut self.session;
                if self.middle {
                    s.cam.orbit(-dx * 0.3, dy * 0.2);
                }
                if self.right.is_some() {
                    self.right_moved += (dx.abs() + dy.abs()) as f64;
                    let k = s.cam.distance * 0.0015;
                    s.cam.pan(-dx * k, dy * k);
                }
            }
            WindowEvent::MouseWheel { delta, .. } if !ui_pointer => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                // Zoom suave: la rueda fija la distancia a la que se quiere llegar y la cámara
                // la alcanza en unos 150 ms (de a saltos se veía como tirones).
                let goal = self.zoom_goal.unwrap_or(self.session.cam.distance);
                self.zoom_goal = Some((goal * 0.88f32.powf(y)).clamp(15.0, 15000.0));
            }
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) if ui_pointer => {}
                (MouseButton::Left, ElementState::Pressed) => {
                    if self.session.game.is_some() {
                        // El punto del mundo bajo el cursor de este instante (no el del último cuadro).
                        if let Some(gpu) = self.gpu.as_ref() {
                            let (w, h) = (gpu.config.width as f32, gpu.config.height as f32);
                            let (o, d) = self.session.cam.ray(self.mouse.x as f32, self.mouse.y as f32, w, h);
                            let hit = self.session.pick(o, d);
                            if let Some(g) = self.session.game.as_mut() {
                                g.hover = hit.map(|h| glam::Vec2::new(h.point.x, h.point.z));
                            }
                        }
                        self.session.game_press();
                    } else if let Some(n) = self.session.place() {
                        println!("Colocada: {} ({n} columnas re-malladas)", self.session.tool_name());
                    }
                }
                (MouseButton::Left, ElementState::Released) => self.session.game_release(),
                (MouseButton::Right, ElementState::Pressed) | (MouseButton::Middle, ElementState::Pressed) if ui_pointer => {}
                (MouseButton::Middle, st) => self.middle = st == ElementState::Pressed,
                (MouseButton::Right, ElementState::Pressed) => {
                    self.right = Some(self.mouse);
                    self.right_moved = 0.0;
                }
                (MouseButton::Right, ElementState::Released) => {
                    // Un clic derecho sin arrastrar excava (en el modo juego, cancela el trazado).
                    if self.right_moved < 5.0 {
                        if let Some(g) = self.session.game.as_mut() {
                            // Clic derecho: cancela el trazado; si no había, suelta la herramienta.
                            if !g.cancel() {
                                g.drop_tool();
                            }
                        } else {
                            self.session.dig(3);
                        }
                    }
                    self.right = None;
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => self.frame(el),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let Some(gpu) = &self.gpu else { return };
        if self.idle && self.keys.is_empty() && !self.middle && self.right.is_none() {
            // Reposo: se duerme hasta el próximo redibujo del agua (o hasta un evento).
            let next = self.last_draw + IDLE_REDRAW;
            el.set_control_flow(ControlFlow::WaitUntil(next));
            if Instant::now() >= next {
                gpu.window.request_redraw();
            }
        } else {
            el.set_control_flow(ControlFlow::Poll);
            gpu.window.request_redraw();
        }
    }
}

/// [lod] Lleva las perillas del auto-ajuste (o las de las opciones, con el auto-ajuste apagado)
/// al streamer y al renderer, solo si cambiaron: sirve también cuando se crea otra partida.
pub fn apply_knobs(k: &crate::autoajuste::Perillas, streamer: &mut crate::stream::Streamer, r: &mut Renderer, cache_mb: u32) {
    let (c, res) = k.sombras();
    let q = crate::render::ShadowQuality { cascades: c, resolution: res };
    if r.shadow_quality() != q {
        r.set_shadow_quality(q);
    }
    streamer.set_ring(k.anillo());
    streamer.set_active_levels(k.niveles);
    if streamer.concurrency() != k.hilos.min(streamer.threads()).max(1) {
        streamer.set_concurrency(k.hilos);
    }
    streamer.ritmo.ms = k.subida_ms() as f64;
    let bytes = (cache_mb as usize) << 20;
    if streamer.cache.presupuesto != bytes {
        streamer.cache.set_presupuesto(bytes);
    }
}
