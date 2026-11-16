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
}

/// Con la escena quieta solo se redibuja para animar el agua, a este intervalo.
const IDLE_REDRAW: std::time::Duration = std::time::Duration::from_millis(100);

pub fn run(session: Session, opts: Options) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("no se pudo abrir el sistema de ventanas: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let steps = || if session.game.is_some() { crate::tour::city_steps() } else { crate::tour::default_steps() };
    let tour = opts.tour_dir.as_ref().map(|d| Tour::with_steps(d, opts.size.0, opts.size.1, steps()));
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
    };
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
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let s = &mut self.session;
        // Cámara con el teclado.
        let speed = s.cam.distance * 0.9 * dt;
        let k = |c| self.keys.contains(&c) as i32 as f32;
        let fwd = k(KeyCode::KeyW) + k(KeyCode::ArrowUp) - k(KeyCode::KeyS) - k(KeyCode::ArrowDown);
        let right = k(KeyCode::KeyD) + k(KeyCode::ArrowRight) - k(KeyCode::KeyA) - k(KeyCode::ArrowLeft);
        s.cam.pan(right * speed, fwd * speed);
        s.cam.yaw += (k(KeyCode::KeyQ) - k(KeyCode::KeyE)) * 80.0 * dt;
        s.update(dt);
        let Some(gpu) = self.gpu.as_mut() else { return };
        s.view_h = gpu.config.height as f32;
        s.apply_ops(&mut gpu.renderer, 400_000);
        gpu.renderer.time += dt;
        // Selección bajo el cursor (no durante el recorrido automático).
        if self.tour.is_none() {
            let (w, h) = (gpu.config.width as f32, gpu.config.height as f32);
            let (o, d) = s.cam.ray(self.mouse.x as f32, self.mouse.y as f32, w, h);
            s.pick(o, d);
        }
        gpu.renderer.set_overlay(&s.overlay());
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
            return;
        }
        self.last_key = Some(key);
        self.last_draw = Instant::now();
        // Interfaz del juego (solo en el modo juego): se arma, se aplican sus acciones y se
        // pinta encima de la escena en la misma entrega.
        let t_ui = Instant::now();
        let mut ui_out = None;
        if let Some(g) = s.game.as_mut() {
            self.ui.time += dt as f64;
            self.ui.sync_news(g);
            let raw = gpu.egui.take_egui_input(&gpu.window);
            let ctx = gpu.egui.egui_ctx().clone();
            let terrain = s.terrain.clone();
            let hour = s.hour;
            let mut acts = Vec::new();
            let ui_state = &mut self.ui;
            let full = ctx.run(raw, |ctx| {
                acts = ui_state.draw(ctx, g, &terrain, hour, "");
            });
            let delta = ui::apply(&acts, g, &mut self.ui.list);
            s.hour = (s.hour + delta).rem_euclid(24.0);
            let repaint = full.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.repaint_delay < std::time::Duration::from_millis(500));
            self.ui_repaint = repaint || !self.ui.toasts().is_empty() || !acts.is_empty();
            gpu.egui.handle_platform_output(&gpu.window, full.platform_output.clone());
            ui_out = Some((ctx, full));
        }
        let Gpu { renderer, surface, config, painter, .. } = gpu;
        let mut cpu_ui = t_ui.elapsed().as_secs_f64() * 1000.0;
        let mut hook = |d: &wgpu::Device, q: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size: (u32, u32)| {
            if let Some((ctx, full)) = ui_out.take() {
                let t0 = Instant::now();
                painter.paint(d, q, enc, view, size, &ctx, full);
                cpu_ui += t0.elapsed().as_secs_f64() * 1000.0;
            }
        };
        let has_ui = s.game.is_some();
        if let Err(e) = renderer.render_surface(surface, config, &s.cam, s.hour, has_ui.then_some(&mut hook as crate::render::UiHook)) {
            eprintln!("{e}");
        }
        drop(hook);
        if has_ui {
            self.ui_ms.push(cpu_ui);
        }
        self.frames += 1;
        self.fps_frames += 1;
        if self.fps_time.elapsed().as_secs_f32() >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_time.elapsed().as_secs_f32();
            self.fps_samples.push(self.fps);
            self.fps_frames = 0;
            self.fps_time = Instant::now();
            gpu.window.set_title(&s.title(self.fps, &gpu.renderer));
        }
        if self.opts.frames.is_some_and(|n| self.frames >= n) {
            el.exit();
        }
    }

    /// Teclas del modo juego (como en el prototipo).
    fn game_key(&mut self, code: KeyCode, el: &ActiveEventLoop) -> bool {
        let s = &mut self.session;
        let Some(g) = s.game.as_mut() else { return false };
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8];
        if let Some(n) = digits.iter().position(|c| *c == code) {
            g.set_tool(n);
            return true;
        }
        match code {
            KeyCode::Escape => {
                if !g.cancel() {
                    el.exit();
                }
            }
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
        if self.game_key(code, el) {
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
        let attrs = Window::default_attributes().with_title("Motor de vóxeles").with_inner_size(LogicalSize::new(self.opts.size.0, self.opts.size.1));
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("no se pudo crear la ventana: {e}"));
                el.exit();
                return;
            }
        };
        match Renderer::for_window(window.clone(), self.opts.msaa) {
            Ok((renderer, surface, config)) => {
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
        let (ui_pointer, ui_keys) = match (self.gpu.as_mut(), self.session.game.is_some()) {
            (Some(gpu), true) => {
                let _ = gpu.egui.on_window_event(&gpu.window, &event);
                let ctx = gpu.egui.egui_ctx();
                (ctx.is_pointer_over_area() || ctx.wants_pointer_input(), ctx.wants_keyboard_input())
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
                self.session.cam.zoom(0.88f32.powf(y));
            }
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) if ui_pointer => {}
                (MouseButton::Left, ElementState::Pressed) => {
                    if self.session.game.is_some() {
                        self.session.game_press();
                    } else if let Some(n) = self.session.place() {
                        println!("Colocada: {} ({n} columnas re-malladas)", self.session.tool_name());
                    }
                }
                (MouseButton::Left, ElementState::Released) => self.session.game_release(),
                (MouseButton::Middle, st) => self.middle = st == ElementState::Pressed,
                (MouseButton::Right, ElementState::Pressed) | (MouseButton::Middle, ElementState::Pressed) if ui_pointer => {}
                (MouseButton::Right, ElementState::Pressed) => {
                    self.right = Some(self.mouse);
                    self.right_moved = 0.0;
                }
                (MouseButton::Right, ElementState::Released) => {
                    // Un clic derecho sin arrastrar excava (en el modo juego, cancela el trazado).
                    if self.right_moved < 5.0 {
                        if let Some(g) = self.session.game.as_mut() {
                            g.cancel();
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
