//! Modo ventana: winit + superficie wgpu, cámara interactiva y construcción con el ratón.
//!
//! Controles:
//!   WASD / flechas  mover          Q / E    girar          rueda   acercar / alejar
//!   botón central   orbitar        botón derecho: arrastrar = mover el mapa, clic = excavar
//!   clic izquierdo  colocar        1–9 / Tab  elegir estructura    R  girarla
//!   F  rellenar terreno            [ / ]  hora del día      P  pausar el día
//!   F12  captura PNG               Esc  salir

use crate::render::Renderer;
use crate::session::{memory_mb, Session};
use crate::sandbox::CATALOG;
use crate::tour::Tour;
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
}

/// Con la escena quieta solo se redibuja para animar el agua, a este intervalo.
const IDLE_REDRAW: std::time::Duration = std::time::Duration::from_millis(100);

pub fn run(session: Session, opts: Options) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("no se pudo abrir el sistema de ventanas: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let tour = opts.tour_dir.as_ref().map(|d| Tour::new(d, opts.size.0, opts.size.1));
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
        let same = self.last_key == Some(key) && self.tour.is_none() && s.streamer.pending() == 0;
        self.idle = same;
        if same && self.last_draw.elapsed() < IDLE_REDRAW {
            self.skipped += 1;
            return;
        }
        self.last_key = Some(key);
        self.last_draw = Instant::now();
        if let Err(e) = gpu.renderer.render_surface(&gpu.surface, &gpu.config, &s.cam, s.hour) {
            eprintln!("{e}");
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

    fn key(&mut self, code: KeyCode, el: &ActiveEventLoop) {
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
                self.gpu = Some(Gpu { window, renderer, surface, config });
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
        }
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.renderer.configure(&gpu.surface, &gpu.config);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
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
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.session.cam.zoom(0.88f32.powf(y));
            }
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) => {
                    if let Some(n) = self.session.place() {
                        println!("Colocada: {} ({n} columnas re-malladas)", self.session.tool_name());
                    }
                }
                (MouseButton::Middle, st) => self.middle = st == ElementState::Pressed,
                (MouseButton::Right, ElementState::Pressed) => {
                    self.right = Some(self.mouse);
                    self.right_moved = 0.0;
                }
                (MouseButton::Right, ElementState::Released) => {
                    // Un clic derecho sin arrastrar excava.
                    if self.right_moved < 5.0 {
                        self.session.dig(3);
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
