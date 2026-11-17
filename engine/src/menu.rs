//! Menús del juego con el mismo papel y tinta que la interfaz: menú principal (nueva partida,
//! continuar, cargar, opciones, salir), menú de pausa con Esc (guardar, guardar como, cargar,
//! opciones, salir) y las pantallas de cargar, opciones, nombre de la ciudad y confirmación.
//!
//! El menú es una pila de pantallas: abajo de todo está el menú principal (sin partida) o el
//! de pausa (con partida); Esc vuelve a la pantalla anterior y, en la pausa, al juego.
//! Las acciones pesadas (empezar o cargar una partida) se hacen un cuadro después de pedirlas
//! para que llegue a verse el aviso «Cargando…».

use crate::city::game::{fmt_thousands, Game};
use crate::render::Renderer;
use crate::save::{self, SaveInfo, Settings};
use crate::session::Session;
use crate::ui::{theme, GameUi};
use egui::{pos2, vec2, Align2, Color32, FontId, RichText, Sense, Stroke};
use std::path::PathBuf;

/// Cámara del fondo del menú principal (la escena colonial vista desde la bahía).
pub const BACKGROUND_CAM: [f32; 5] = [13500.0, 16300.0, 560.0, 205.0, 27.0];
pub const BACKGROUND_HOUR: f32 = 16.5;
/// Grados por segundo que gira la cámara detrás del menú principal.
const ORBIT_SPEED: f32 = 2.5;
/// Cámara al fundar una ciudad nueva.
pub fn new_game_cam() -> [f32; 5] {
    let p = crate::city::game_demo::PLAZA;
    [p.x, p.y, 700.0, 200.0, 45.0]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    ToMenu,
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    /// Menú principal (sin partida en curso).
    Main,
    /// Nombre de la ciudad nueva.
    NewGame,
    Load,
    Options,
    /// Menú de pausa (con partida en curso).
    Pause,
    SaveAs,
    /// Hay cambios sin guardar: guardar, salir igual o cancelar.
    ConfirmLeave(Leave),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    NewGame(String),
    Load(PathBuf),
    /// F9: la partida actual o la más reciente.
    QuickLoad,
    Delete(PathBuf),
    Save,
    SaveAs(String),
    Resume,
    Leave(Leave),
    SaveAndLeave(Leave),
    ApplySettings(Settings),
}

impl MenuAction {
    /// Las que regeneran el mundo (se hacen un cuadro después, con el aviso de carga).
    pub fn heavy(&self) -> bool {
        matches!(self, MenuAction::NewGame(_) | MenuAction::Load(_) | MenuAction::QuickLoad | MenuAction::Leave(Leave::ToMenu))
    }
}

/// Qué tiene que hacer la ventana después de una acción del menú.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Quit,
    /// Cambiaron las opciones (sincronización vertical, escala…).
    Settings,
}

/// Huella de la partida para saber si hay cambios sin guardar.
pub fn fingerprint(g: &Game) -> u64 {
    let c = &g.city;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    mix(c.month as u64);
    mix(c.money as u64);
    mix(c.net.edges.len() as u64);
    mix(c.net.total_length().to_bits() as u64);
    mix(c.lots.lots.len() as u64);
    for l in c.lots.lots.values() {
        mix(((l.id as u64) << 16) | ((l.zone as u64) << 8) | l.level as u64);
    }
    for t in c.economy.taxes {
        mix(t as u64);
    }
    mix(c.economy.active.len() as u64);
    mix(c.plaza.is_some() as u64);
    h
}

pub struct Menu {
    /// Peor cuadro del último medio segundo (ms) y en qué se fue: para el contador de FPS.
    pub frame_worst: (f32, String),
    /// Pila de pantallas; vacía = jugando (menú cerrado).
    pub stack: Vec<Screen>,
    /// Carpeta de datos (opciones) y de partidas.
    pub base: PathBuf,
    pub saves_dir: PathBuf,
    pub settings: Settings,
    /// Niveles de detalle e hilos para las sesiones nuevas.
    pub levels: u32,
    pub threads: usize,
    /// Opciones mientras se editan (se aplican al aceptar).
    pub draft: Settings,
    pub name_input: String,
    pub saves: Vec<SaveInfo>,
    pub confirm_delete: Option<PathBuf>,
    /// Aviso en el menú (texto, ¿es un error?).
    pub status: Option<(String, bool)>,
    /// «Cargando…»: se muestra mientras se prepara la partida.
    pub loading: Option<String>,
    /// Archivo de la partida en curso y su huella al guardarla.
    pub current: Option<PathBuf>,
    pub saved_fp: Option<u64>,
    /// Velocidad y pausa del día antes de abrir el menú de pausa.
    paused: Option<(f32, bool)>,
    /// Mes del último autoguardado (o del comienzo).
    auto_mark: Option<u32>,
    focus_name: bool,
    /// Escena detrás del menú principal ("ciudad"; "vacia" en las pruebas).
    pub background_scene: &'static str,
}

impl Menu {
    pub fn new(base: PathBuf, settings: Settings, levels: u32, threads: usize, main: bool) -> Menu {
        let saves_dir = save::saves_dir(&base);
        let mut m = Menu {
            frame_worst: (0.0, String::new()),
            stack: Vec::new(),
            base,
            saves_dir,
            draft: settings.clone(),
            settings,
            levels,
            threads,
            name_input: String::new(),
            saves: Vec::new(),
            confirm_delete: None,
            status: None,
            loading: None,
            current: None,
            saved_fp: None,
            paused: None,
            auto_mark: None,
            focus_name: false,
            background_scene: "ciudad",
        };
        if main {
            m.push(Screen::Main);
        }
        m
    }

    pub fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    pub fn in_main_menu(&self) -> bool {
        self.stack.first() == Some(&Screen::Main)
    }

    pub fn screen(&self) -> Option<&Screen> {
        self.stack.last()
    }

    pub fn refresh_saves(&mut self) {
        self.saves = save::list(&self.saves_dir);
    }

    pub fn latest(&self) -> Option<&SaveInfo> {
        self.saves.iter().find(|s| s.error.is_none())
    }

    pub fn push(&mut self, s: Screen) {
        match &s {
            Screen::Main | Screen::Load | Screen::Pause => self.refresh_saves(),
            Screen::Options => self.draft = self.settings.clone(),
            Screen::NewGame | Screen::SaveAs => self.focus_name = true,
            Screen::ConfirmLeave(_) => {}
        }
        self.confirm_delete = None;
        self.stack.push(s);
    }

    pub fn open_pause(&mut self) {
        self.stack.clear();
        self.status = None;
        self.push(Screen::Pause);
    }

    /// Esc dentro del menú: vuelve a la pantalla anterior; en la pausa, vuelve al juego.
    pub fn escape(&mut self) -> Option<MenuAction> {
        if self.confirm_delete.take().is_some() {
            return None;
        }
        match self.stack.len() {
            0 => None,
            1 if self.stack[0] == Screen::Pause => {
                self.stack.clear();
                Some(MenuAction::Resume)
            }
            1 => None,
            _ => {
                self.stack.pop();
                None
            }
        }
    }

    /// Hay cambios sin guardar en la partida.
    pub fn dirty(&self, g: &Game) -> bool {
        self.saved_fp != Some(fingerprint(g))
    }

    /// Detiene la simulación y el día mientras el menú está abierto; los restituye al cerrarlo.
    pub fn sync_pause(&mut self, s: &mut Session) {
        let open = self.is_open();
        let Some(g) = s.game.as_mut() else {
            self.paused = None;
            return;
        };
        match (open, self.paused) {
            (true, None) => {
                self.paused = Some((g.speed, s.day_paused));
                g.speed = 0.0;
                s.day_paused = true;
            }
            (false, Some((speed, day))) => {
                g.speed = speed;
                s.day_paused = day;
                self.paused = None;
            }
            _ => {}
        }
    }

    /// Detrás del menú principal la cámara gira despacio alrededor de la plaza.
    pub fn animate_background(&self, s: &mut Session, dt: f32) {
        if self.in_main_menu() {
            s.cam.yaw = (s.cam.yaw + ORBIT_SPEED * dt).rem_euclid(360.0);
        }
    }

    /// Autoguardado cada `autoguardado_meses` meses de juego (en un archivo aparte).
    pub fn autosave(&mut self, s: &mut Session) {
        let n = self.settings.autoguardado_meses;
        let cam = s.cam_array();
        let hour = s.hour;
        let Some(g) = s.game.as_mut() else { return };
        if n == 0 || self.is_open() || g.city.plaza.is_none() {
            return;
        }
        let m = g.city.month;
        match self.auto_mark {
            Some(m0) if m >= m0 + n => {
                self.auto_mark = Some(m);
                let path = save::path_for(&self.saves_dir, &g.city_name, true);
                match save::write(&path, &save::snapshot(g, cam, hour, true)) {
                    Ok(()) => g.say(format!("Autoguardado: {}, {}", g.city_name, g.city.date())),
                    Err(e) => g.say(format!("No se pudo autoguardar: {e}")),
                }
            }
            None => self.auto_mark = Some(m),
            _ => {}
        }
    }

    // --- Acciones -------------------------------------------------------------------------

    fn replace_session(&mut self, s: &mut Session, new: Session, r: Option<&mut Renderer>, gui: &mut GameUi) {
        let view_h = s.view_h;
        *s = new;
        s.view_h = view_h;
        if let Some(r) = r {
            r.clear_columns();
        }
        *gui = GameUi::default();
        self.paused = None;
        self.auto_mark = s.game.as_ref().map(|g| g.city.month);
        self.loading = None;
    }

    /// Guarda la partida en curso en `path` (o en su archivo). Devuelve el aviso.
    fn save_to(&mut self, s: &mut Session, path: Option<PathBuf>) -> Result<String, String> {
        let cam = s.cam_array();
        let hour = s.hour;
        let g = s.game.as_mut().ok_or("no hay partida en curso")?;
        let path = path.or_else(|| self.current.clone()).unwrap_or_else(|| save::unique_path(&self.saves_dir, &g.city_name));
        let mut f = save::snapshot(g, cam, hour, false);
        // Desde el menú de pausa: se guarda la velocidad que tenía antes de pausar.
        if let Some((speed, _)) = self.paused {
            f.herramientas.velocidad = speed;
        }
        save::write(&path, &f)?;
        self.current = Some(path);
        self.saved_fp = Some(fingerprint(g));
        Ok(format!("Partida guardada: {}, {}", g.city_name, g.city.date()))
    }

    /// Cuenta un resultado en la gaceta (con partida) o en el menú.
    fn tell(&mut self, s: &mut Session, msg: String, error: bool) {
        match s.game.as_mut() {
            Some(g) if !self.is_open() => g.say(msg),
            _ => self.status = Some((msg, error)),
        }
    }

    pub fn load(&mut self, path: &std::path::Path, s: &mut Session, r: Option<&mut Renderer>, gui: &mut GameUi) -> Result<(), String> {
        let f = save::read(path)?;
        let t0 = std::time::Instant::now();
        // Las partidas se juegan sobre el mapa real (el fondo del menú usa el mapa ficticio).
        let real;
        let t = if s.terrain.is_real() {
            &s.terrain
        } else {
            real = std::sync::Arc::new(crate::terrain::TerrainGen::montevideo());
            &real
        };
        let (mut game, edits) = save::restore(&f, t)?;
        // El mensaje inicial de una partida nueva ("Funda la ciudad…") no va en una cargada.
        if game.city.plaza.is_some() {
            game.message = format!("Partida cargada: {}, {}", f.nombre, game.city.date());
        }
        let t_rebuild = t0.elapsed();
        let fp = fingerprint(&game);
        let new = Session::with_game(game, &edits, self.levels, self.threads, f.camara, f.hora);
        println!("Partida «{}» cargada en {:.2?} ({:.2?} regenerando {} vóxeles)", f.nombre, t0.elapsed(), t_rebuild, edits.voxel_count());
        self.replace_session(s, new, r, gui);
        // Un autoguardado se sigue guardando en la partida normal de esa ciudad.
        self.current = Some(if f.autoguardado { save::path_for(&self.saves_dir, &f.nombre, false) } else { path.to_path_buf() });
        self.saved_fp = Some(fp);
        self.stack.clear();
        self.status = None;
        Ok(())
    }

    pub fn perform(&mut self, a: MenuAction, s: &mut Session, r: Option<&mut Renderer>, gui: &mut GameUi) -> Outcome {
        match a {
            MenuAction::NewGame(name) => {
                let name = if name.trim().is_empty() { crate::ui::CITY_NAME.to_string() } else { name.trim().to_string() };
                let mut game = Game::default();
                game.city_name = name;
                let fp = fingerprint(&game);
                let new = Session::with_game(game, &crate::buildings::Edits::default(), self.levels, self.threads, new_game_cam(), 10.5);
                self.replace_session(s, new, r, gui);
                self.current = None;
                self.saved_fp = Some(fp);
                self.stack.clear();
                self.status = None;
            }
            MenuAction::Load(path) => {
                if let Err(e) = self.load(&path, s, r, gui) {
                    self.loading = None;
                    self.tell(s, format!("No se pudo cargar: {e}"), true);
                }
            }
            MenuAction::QuickLoad => {
                self.refresh_saves();
                let path = self.current.clone().filter(|p| p.exists()).or_else(|| self.latest().map(|s| s.path.clone()));
                match path {
                    Some(p) => return self.perform(MenuAction::Load(p), s, r, gui),
                    None => {
                        self.loading = None;
                        self.tell(s, "No hay partidas guardadas para cargar".into(), true);
                    }
                }
            }
            MenuAction::Delete(path) => {
                match save::delete(&path) {
                    Ok(()) => {
                        if self.current.as_ref() == Some(&path) {
                            self.current = None;
                            self.saved_fp = None;
                        }
                        self.status = None;
                    }
                    Err(e) => self.status = Some((e, true)),
                }
                self.confirm_delete = None;
                self.refresh_saves();
            }
            MenuAction::Save => {
                let r = self.save_to(s, None);
                self.refresh_saves();
                match r {
                    Ok(m) => self.tell(s, m, false),
                    Err(e) => self.tell(s, format!("No se pudo guardar: {e}"), true),
                }
            }
            MenuAction::SaveAs(name) => {
                let name = name.trim().to_string();
                if let Some(g) = s.game.as_mut() {
                    if !name.is_empty() {
                        g.city_name = name.clone();
                    }
                }
                let path = s.game.as_ref().map(|g| save::path_for(&self.saves_dir, &g.city_name, false));
                let r = self.save_to(s, path);
                self.refresh_saves();
                match r {
                    Ok(m) => {
                        // Vuelve al menú de pausa con el aviso.
                        self.stack.truncate(1);
                        self.tell(s, m, false)
                    }
                    Err(e) => self.tell(s, format!("No se pudo guardar: {e}"), true),
                }
            }
            MenuAction::Resume => {
                self.stack.clear();
                self.status = None;
            }
            MenuAction::SaveAndLeave(to) => match self.save_to(s, None) {
                Ok(_) => return self.perform(MenuAction::Leave(to), s, r, gui),
                Err(e) => self.status = Some((format!("No se pudo guardar: {e}"), true)),
            },
            MenuAction::Leave(Leave::Quit) => return Outcome::Quit,
            MenuAction::Leave(Leave::ToMenu) => {
                let new = Session::new(self.background_scene, self.levels, self.threads, BACKGROUND_CAM, BACKGROUND_HOUR);
                self.replace_session(s, new, r, gui);
                self.current = None;
                self.saved_fp = None;
                self.stack.clear();
                self.status = None;
                self.push(Screen::Main);
            }
            MenuAction::ApplySettings(new) => {
                self.settings = new.sanitized();
                self.draft = self.settings.clone();
                if let Err(e) = self.settings.store(&self.base) {
                    self.status = Some((e, true));
                } else {
                    self.status = None;
                }
                self.stack.pop();
                return Outcome::Settings;
            }
        }
        Outcome::None
    }

    // --- Dibujo ---------------------------------------------------------------------------

    /// Dibuja el menú abierto (y el contador de FPS si está activado). Devuelve lo pedido.
    pub fn draw(&mut self, ctx: &egui::Context, game: Option<&Game>, fps: f32) -> Vec<MenuAction> {
        let mut acts = Vec::new();
        if self.settings.mostrar_fps {
            egui::Area::new("fps".into()).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, vec2(-10.0, -10.0)).interactable(false).show(ctx, |ui| {
                theme::paper_frame().inner_margin(egui::Margin::symmetric(8.0, 2.0)).show(ui, |ui| {
                    let (worst, why) = &self.frame_worst;
                    let txt = if *worst > 0.0 { format!("{fps:.0} FPS · peor cuadro {worst:.0} ms{}", if why.is_empty() { String::new() } else { format!(" ({why})") }) } else { format!("{fps:.0} FPS") };
                    ui.label(RichText::new(txt).small());
                });
            });
        }
        let Some(screen) = self.screen().cloned() else { return acts };
        let main = self.in_main_menu();
        // Velo sobre el mundo: tapa los clics y oscurece un poco la escena.
        egui::Area::new("menu-velo".into()).order(egui::Order::Foreground).fixed_pos(pos2(0.0, 0.0)).show(ctx, |ui| {
            let r = ctx.screen_rect();
            ui.allocate_rect(r, Sense::click_and_drag());
            let p = ui.painter();
            if main {
                // Viñeta suave en sepia: más oscura arriba y abajo.
                p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(40, 26, 12, 40));
                let band = r.height() * 0.22;
                for k in 0..12 {
                    let a = ((12 - k) as f32 / 12.0 * 70.0) as u8;
                    let h = band / 12.0;
                    let col = Color32::from_rgba_unmultiplied(30, 20, 10, a);
                    p.rect_filled(egui::Rect::from_min_size(pos2(r.left(), r.top() + k as f32 * h), vec2(r.width(), h + 0.5)), 0.0, col);
                    p.rect_filled(egui::Rect::from_min_size(pos2(r.left(), r.bottom() - (k + 1) as f32 * h), vec2(r.width(), h + 0.5)), 0.0, col);
                }
            } else {
                p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(25, 16, 8, 165));
            }
        });
        if let Some(text) = self.loading.clone() {
            panel(ctx, "menu-cargando", 360.0, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new(text).size(theme::BODY * 1.2));
                    ui.add_space(4.0);
                    ui.add(egui::Spinner::new().size(22.0).color(theme::INK_SOFT));
                    ui.add_space(6.0);
                });
            });
            return acts;
        }
        if main {
            self.title_block(ctx);
            egui::Area::new("menu-pie".into()).order(egui::Order::Tooltip).anchor(Align2::LEFT_BOTTOM, vec2(14.0, -10.0)).interactable(false).show(ctx, |ui| {
                ui.label(RichText::new("Motor de vóxeles · versión 0.1").small().color(Color32::from_rgb(240, 232, 212)));
            });
        }
        match screen {
            Screen::Main => self.main_screen(ctx, &mut acts),
            Screen::Pause => self.pause_screen(ctx, game, &mut acts),
            Screen::NewGame => self.name_screen(ctx, false, game, &mut acts),
            Screen::SaveAs => self.name_screen(ctx, true, game, &mut acts),
            Screen::Load => self.load_screen(ctx, game, &mut acts),
            Screen::Options => self.options_screen(ctx, &mut acts),
            Screen::ConfirmLeave(to) => self.confirm_screen(ctx, to, &mut acts),
        }
        acts
    }

    fn title_block(&self, ctx: &egui::Context) {
        let h = ctx.screen_rect().height();
        egui::Area::new("menu-titulo".into()).order(egui::Order::Tooltip).anchor(Align2::CENTER_TOP, vec2(0.0, (h * 0.07).max(16.0))).interactable(false).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let light = Color32::from_rgb(246, 238, 216);
                // Sombra de la letra para que se lea sobre el cielo.
                let galley = ui.painter().layout_no_wrap("City Sim 2027".into(), FontId::proportional(64.0), light);
                let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                ui.painter().galley(rect.min + vec2(2.0, 3.0), ui.painter().layout_no_wrap("City Sim 2027".into(), FontId::proportional(64.0), Color32::from_black_alpha(140)), Color32::BLACK);
                ui.painter().galley(rect.min, galley, light);
                ornament(ui, 300.0, light);
                let sub = ui.painter().layout_no_wrap("San Felipe de Montevideo · 1726".into(), FontId::proportional(theme::BODY * 1.15), light);
                let (r2, _) = ui.allocate_exact_size(sub.size(), Sense::hover());
                ui.painter().galley(r2.min + vec2(1.0, 2.0), ui.painter().layout_no_wrap("San Felipe de Montevideo · 1726".into(), FontId::proportional(theme::BODY * 1.15), Color32::from_black_alpha(140)), Color32::BLACK);
                ui.painter().galley(r2.min, sub, light);
            });
        });
    }

    fn main_screen(&mut self, ctx: &egui::Context, acts: &mut Vec<MenuAction>) {
        let latest = self.latest().cloned();
        let mut next = None;
        panel(ctx, "menu-principal", 340.0, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                if menu_button(ui, "Nueva partida", Some("Fundar una ciudad"), true).clicked() {
                    next = Some(Screen::NewGame);
                }
                let sub = latest.as_ref().map(|s| format!("{} · {}", s.name, s.date)).unwrap_or_else(|| "No hay partidas guardadas".into());
                if menu_button(ui, "Continuar", Some(&sub), latest.is_some()).clicked() {
                    if let Some(s) = &latest {
                        acts.push(MenuAction::Load(s.path.clone()));
                    }
                }
                let n = self.saves.len();
                let sub = match n {
                    0 => "Ninguna todavía".to_string(),
                    1 => "1 partida guardada".to_string(),
                    n => format!("{n} partidas guardadas"),
                };
                if menu_button(ui, "Cargar partida", Some(&sub), true).clicked() {
                    next = Some(Screen::Load);
                }
                if menu_button(ui, "Opciones", None, true).clicked() {
                    next = Some(Screen::Options);
                }
                if menu_button(ui, "Salir", None, true).clicked() {
                    acts.push(MenuAction::Leave(Leave::Quit));
                }
                self.status_line(ui);
            });
        });
        if let Some(s) = next {
            self.status = None;
            if s == Screen::NewGame {
                self.name_input = crate::ui::CITY_NAME.to_string();
            }
            self.push(s);
        }
    }

    fn pause_screen(&mut self, ctx: &egui::Context, game: Option<&Game>, acts: &mut Vec<MenuAction>) {
        let mut next = None;
        let dirty = game.is_some_and(|g| self.dirty(g));
        panel(ctx, "menu-pausa", 320.0, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Pausa").size(theme::BODY * 1.6));
                if let Some(g) = game {
                    ui.label(RichText::new(format!("{} · {}", g.city_name, g.city.date())).color(theme::INK_SOFT));
                    let saved = if dirty { "Hay cambios sin guardar" } else { "Todo guardado" };
                    ui.label(RichText::new(saved).small().color(if dirty { theme::BAD } else { theme::GOOD }));
                }
                ornament(ui, 220.0, theme::BORDER);
                ui.spacing_mut().item_spacing.y = 7.0;
                if menu_button(ui, "Volver al juego", None, true).on_hover_text("Atajo: Esc").clicked() {
                    acts.push(MenuAction::Resume);
                }
                if menu_button(ui, "Guardar", None, game.is_some()).on_hover_text("Atajo: F5").clicked() {
                    acts.push(MenuAction::Save);
                }
                if menu_button(ui, "Guardar como…", None, game.is_some()).clicked() {
                    next = Some(Screen::SaveAs);
                }
                if menu_button(ui, "Cargar", None, true).on_hover_text("F9 carga la última partida").clicked() {
                    next = Some(Screen::Load);
                }
                if menu_button(ui, "Opciones", None, true).clicked() {
                    next = Some(Screen::Options);
                }
                if menu_button(ui, "Salir al menú principal", None, true).clicked() {
                    if dirty {
                        next = Some(Screen::ConfirmLeave(Leave::ToMenu));
                    } else {
                        acts.push(MenuAction::Leave(Leave::ToMenu));
                    }
                }
                if menu_button(ui, "Salir del juego", None, true).clicked() {
                    if dirty {
                        next = Some(Screen::ConfirmLeave(Leave::Quit));
                    } else {
                        acts.push(MenuAction::Leave(Leave::Quit));
                    }
                }
                self.status_line(ui);
            });
        });
        if let Some(s) = next {
            if s == Screen::SaveAs {
                self.name_input = game.map(|g| g.city_name.clone()).unwrap_or_default();
            }
            self.status = None;
            self.push(s);
        }
    }

    fn name_screen(&mut self, ctx: &egui::Context, save_as: bool, game: Option<&Game>, acts: &mut Vec<MenuAction>) {
        let mut back = false;
        let focus = std::mem::take(&mut self.focus_name);
        let exists = save_as && {
            let p = save::path_for(&self.saves_dir, &self.name_input, false);
            p.exists() && self.current.as_ref() != Some(&p)
        };
        panel(ctx, "menu-nombre", 420.0, |ui| {
            ui.label(RichText::new(if save_as { "Guardar como…" } else { "Nueva partida" }).size(theme::BODY * 1.5));
            ornament(ui, 396.0, theme::BORDER);
            ui.label(if save_as { "Nombre de la partida" } else { "Nombre de la ciudad" });
            let te = egui::TextEdit::singleline(&mut self.name_input).desired_width(396.0).char_limit(40).font(egui::TextStyle::Body).margin(vec2(8.0, 6.0));
            let resp = ui.add(te);
            if focus {
                resp.request_focus();
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if save_as {
                if exists {
                    ui.label(RichText::new("Ya hay una partida con ese nombre: se va a reemplazar.").small().color(theme::BAD));
                } else if let Some(g) = game {
                    ui.label(RichText::new(format!("Se guarda {} con la ciudad en {}.", g.city.date(), fmt_thousands(g.city.population() as i64) + " vecinos")).small().color(theme::INK_SOFT));
                }
            } else {
                ui.label(RichText::new("Empiezas en marzo de 1726 con 20.000 pesos del cabildo: elige un sitio firme y llano para la plaza mayor, traza las calles y reparte los solares.").small().color(theme::INK_SOFT));
            }
            ui.add_space(6.0);
            let ok = !self.name_input.trim().is_empty();
            ui.horizontal(|ui| {
                let label = if save_as { "Guardar" } else { "Fundar la ciudad" };
                if small_menu_button(ui, label, ok).clicked() || (enter && ok) {
                    let n = self.name_input.trim().to_string();
                    acts.push(if save_as { MenuAction::SaveAs(n) } else { MenuAction::NewGame(n) });
                }
                if small_menu_button(ui, "Volver", true).clicked() {
                    back = true;
                }
            });
            self.status_line(ui);
        });
        if back {
            self.stack.pop();
        }
    }

    fn load_screen(&mut self, ctx: &egui::Context, game: Option<&Game>, acts: &mut Vec<MenuAction>) {
        let mut back = false;
        let now = save::now_secs();
        let max_h = (ctx.screen_rect().height() * 0.62).max(160.0);
        let dirty = game.is_some_and(|g| self.dirty(g));
        panel(ctx, "menu-cargar", 640.0, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Cargar partida").size(theme::BODY * 1.5));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Volver · atajo: Esc").clicked() {
                        back = true;
                    }
                });
            });
            ornament(ui, 616.0, theme::BORDER);
            if dirty {
                ui.label(RichText::new("La partida en curso tiene cambios sin guardar: se pierden al cargar otra.").small().color(theme::BAD));
            }
            if self.saves.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("Todavía no hay partidas guardadas. Desde el juego, Esc → Guardar (o F5).").color(theme::INK_SOFT));
                ui.add_space(10.0);
            }
            let saves = self.saves.clone();
            egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                for (i, s) in saves.iter().enumerate() {
                    let current = self.current.as_ref() == Some(&s.path);
                    let fill = if current { theme::PAPER_LIGHT } else { theme::PAPER_DARK.gamma_multiply(0.45) };
                    egui::Frame::none().fill(fill).stroke(Stroke::new(1.0_f32, theme::BORDER.gamma_multiply(0.6))).rounding(4.0).inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&s.name).size(theme::BODY * 1.15).strong());
                                    if s.auto {
                                        ui.label(RichText::new("autoguardado").small().color(theme::INK_SOFT));
                                    }
                                    if current {
                                        ui.label(RichText::new("· en curso").small().color(theme::GOOD));
                                    }
                                });
                                match &s.error {
                                    Some(e) => {
                                        ui.label(RichText::new(e).small().color(theme::BAD));
                                    }
                                    None => {
                                        ui.label(RichText::new(format!("{} · {} vecinos · {} pesos", crate::ui::capitalize(&s.date), fmt_thousands(s.population as i64), fmt_thousands(s.money))).small());
                                    }
                                }
                                ui.label(RichText::new(format!("Guardada {} · {}", save::ago(now, s.saved), s.path.file_name().and_then(|f| f.to_str()).unwrap_or(""))).small().color(theme::INK_SOFT));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.confirm_delete.as_ref() == Some(&s.path) {
                                    if ui.button("No").clicked() {
                                        self.confirm_delete = None;
                                    }
                                    if ui.button(RichText::new("Sí, borrar").color(theme::BAD)).clicked() {
                                        acts.push(MenuAction::Delete(s.path.clone()));
                                    }
                                    ui.label(RichText::new("¿Borrar para siempre?").small().color(theme::BAD));
                                } else {
                                    if ui.button("Borrar").on_hover_text("Borrar el archivo de esta partida").clicked() {
                                        self.confirm_delete = Some(s.path.clone());
                                    }
                                    let load = ui.add_enabled(s.error.is_none(), egui::Button::new(RichText::new("Cargar").strong()));
                                    if load.clicked() {
                                        acts.push(MenuAction::Load(s.path.clone()));
                                    }
                                }
                            });
                        });
                    });
                    if i + 1 < saves.len() {
                        ui.add_space(4.0);
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if small_menu_button(ui, "Volver", true).clicked() {
                    back = true;
                }
                ui.add(egui::Label::new(RichText::new(format!("Carpeta: {}", self.saves_dir.display())).small().color(theme::INK_SOFT)).truncate()).on_hover_text(self.saves_dir.display().to_string());
            });
            self.status_line(ui);
        });
        if back {
            self.confirm_delete = None;
            self.stack.pop();
        }
    }

    fn options_screen(&mut self, ctx: &egui::Context, acts: &mut Vec<MenuAction>) {
        let mut back = false;
        let status = self.status.clone();
        let d = &mut self.draft;
        panel(ctx, "menu-opciones", 520.0, |ui| {
            ui.label(RichText::new("Opciones").size(theme::BODY * 1.5));
            ornament(ui, 496.0, theme::BORDER);
            ui.spacing_mut().slider_width = 200.0;
            egui::Grid::new("opciones").num_columns(2).spacing(vec2(18.0, 10.0)).show(ui, |ui| {
                ui.label("Tamaño de la interfaz");
                ui.add(egui::Slider::new(&mut d.escala_interfaz, 0.75..=1.5).step_by(0.05).custom_formatter(|v, _| format!("{} %", (v * 100.0).round() as i32)));
                ui.end_row();
                ui.label("Sincronización vertical");
                ui.checkbox(&mut d.sincronizacion_vertical, "Evita cortes en la imagen");
                ui.end_row();
                ui.label("Mostrar FPS");
                ui.checkbox(&mut d.mostrar_fps, "Cuadros por segundo en una esquina");
                ui.end_row();
                ui.label("Distancia de dibujo");
                ui.horizontal(|ui| {
                    let l = crate::lod::LEVELS;
                    for (v, name, tip) in [(l - 2, "Corta", "Menos detalle a lo lejos: más rápido"), (l - 1, "Media", "Un término medio"), (l, "Larga", "Se ve hasta el horizonte")] {
                        if ui.selectable_label(d.niveles == v, name).on_hover_text(tip).clicked() {
                            d.niveles = v;
                        }
                    }
                });
                ui.end_row();
                ui.label("Suavizado de bordes");
                ui.horizontal(|ui| {
                    for (v, name, tip) in [(0, "Automático", "4× con tarjeta gráfica; desactivado con render por CPU"), (1, "Desactivado", "Más rápido"), (4, "4×", "Bordes suaves (MSAA)")] {
                        if ui.selectable_label(d.msaa == v, name).on_hover_text(tip).clicked() {
                            d.msaa = v;
                        }
                    }
                });
                ui.end_row();
                ui.label("Sombras");
                ui.horizontal(|ui| {
                    // Cascadas × resolución: memoria de los mapas (Depth32: 4 bytes por texel).
                    for (c, r, name, tip) in [
                        (2, 1024, "Baja", "2 cascadas de 1024² (8 MB): sombras borrosas de cerca"),
                        (3, 2048, "Media", "3 cascadas de 2048² (48 MB)"),
                        (4, 2048, "Alta", "4 cascadas de 2048² (64 MB): nítidas de cerca y hasta los cerros"),
                        (4, 4096, "Muy alta", "4 cascadas de 4096² (256 MB): para pantallas grandes"),
                    ] {
                        if ui.selectable_label(d.sombras_cascadas == c && d.sombras_resolucion == r, name).on_hover_text(tip).clicked() {
                            (d.sombras_cascadas, d.sombras_resolucion) = (c, r);
                        }
                    }
                });
                ui.end_row();
                ui.label("Autoguardado");
                ui.horizontal(|ui| {
                    for (v, name) in [(0, "Nunca"), (6, "6 meses"), (12, "12 meses"), (24, "24 meses")] {
                        if ui.selectable_label(d.autoguardado_meses == v, name).on_hover_text("Cada tantos meses de juego, en un archivo aparte").clicked() {
                            d.autoguardado_meses = v;
                        }
                    }
                });
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.label(RichText::new("La distancia de dibujo se aplica al empezar o cargar una partida; el suavizado, al volver a abrir el juego.").small().color(theme::INK_SOFT));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if small_menu_button(ui, "Aceptar", true).clicked() {
                    acts.push(MenuAction::ApplySettings(d.clone()));
                }
                if small_menu_button(ui, "Cancelar", true).clicked() {
                    back = true;
                }
                if small_menu_button(ui, "Valores por omisión", true).clicked() {
                    *d = Settings::default();
                }
            });
            if let Some((m, err)) = &status {
                ui.label(RichText::new(m).small().color(if *err { theme::BAD } else { theme::GOOD }));
            }
        });
        if back {
            self.draft = self.settings.clone();
            self.stack.pop();
        }
    }

    fn confirm_screen(&mut self, ctx: &egui::Context, to: Leave, acts: &mut Vec<MenuAction>) {
        let mut back = false;
        panel(ctx, "menu-confirmar", 420.0, |ui| {
            ui.label(RichText::new("Hay cambios sin guardar").size(theme::BODY * 1.4));
            ornament(ui, 396.0, theme::BORDER);
            ui.label(if to == Leave::Quit { "Si sales del juego ahora, se pierde lo hecho desde la última vez que guardaste." } else { "Si vuelves al menú principal ahora, se pierde lo hecho desde la última vez que guardaste." });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if small_menu_button(ui, "Guardar y salir", true).clicked() {
                    acts.push(MenuAction::SaveAndLeave(to));
                }
                if small_menu_button(ui, "Salir sin guardar", true).clicked() {
                    acts.push(MenuAction::Leave(to));
                }
                if small_menu_button(ui, "Cancelar", true).clicked() {
                    back = true;
                }
            });
            self.status_line(ui);
        });
        if back {
            self.stack.pop();
        }
    }

    fn status_line(&self, ui: &mut egui::Ui) {
        if let Some((m, err)) = &self.status {
            ui.add_space(4.0);
            ui.label(RichText::new(m).small().color(if *err { theme::BAD } else { theme::GOOD }));
        }
    }

}

/// Esc en el juego, en orden: cancela el trazado, cierra paneles y cajones, abre el menú de
/// pausa; con el menú abierto, vuelve atrás (y de la pausa, al juego).
pub fn escape(game: Option<&mut Game>, gui: &mut GameUi, menu: &mut Menu) -> Option<MenuAction> {
    if menu.is_open() {
        return menu.escape();
    }
    let g = game?;
    // Esc: primero cancela el trazado, después cierra paneles, después suelta la herramienta y
    // recién entonces abre el menú de pausa.
    if g.cancel() || gui.close_one(g) || g.drop_tool() {
        return None;
    }
    menu.open_pause();
    None
}

/// Panel de papel centrado en la pantalla, por encima del velo.
fn panel(ctx: &egui::Context, id: &'static str, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Area::new(id.into()).order(egui::Order::Tooltip).anchor(Align2::CENTER_CENTER, vec2(0.0, ctx.screen_rect().height() * 0.06)).show(ctx, |ui| {
        theme::paper_frame().inner_margin(egui::Margin::symmetric(12.0, 12.0)).show(ui, |ui| {
            ui.set_width(width);
            add(ui);
        });
    });
}

/// Filete con un rombo al medio, como en las portadas de los libros de la época.
fn ornament(ui: &mut egui::Ui, width: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 14.0), Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let st = Stroke::new(1.2_f32, color);
    p.line_segment([pos2(rect.left() + 8.0, c.y), pos2(c.x - 12.0, c.y)], st);
    p.line_segment([pos2(c.x + 12.0, c.y), pos2(rect.right() - 8.0, c.y)], st);
    let d = 5.0;
    p.add(egui::Shape::convex_polygon(vec![pos2(c.x, c.y - d), pos2(c.x + d, c.y), pos2(c.x, c.y + d), pos2(c.x - d, c.y)], color, Stroke::NONE));
}

/// Botón grande del menú: una línea, o título y subtítulo.
fn menu_button(ui: &mut egui::Ui, text: &str, sub: Option<&str>, enabled: bool) -> egui::Response {
    let h = if sub.is_some() { 54.0 } else { 40.0 };
    let w = ui.available_width().min(300.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let p = ui.painter();
    let hovered = enabled && resp.hovered();
    let fill = if hovered { theme::PRESSED } else if enabled { theme::PAPER_DARK } else { theme::PAPER_DARK.gamma_multiply(0.45) };
    p.rect_filled(rect, 4.0, fill);
    p.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, if enabled { theme::BORDER } else { theme::BORDER.gamma_multiply(0.5) }));
    let ink = if hovered { theme::PRESSED_TEXT } else if enabled { theme::INK } else { theme::INK_SOFT.gamma_multiply(0.7) };
    let y = if sub.is_some() { rect.top() + 19.0 } else { rect.center().y };
    p.text(pos2(rect.center().x, y), Align2::CENTER_CENTER, text, FontId::proportional(theme::BODY * 1.15), ink);
    if let Some(s) = sub {
        let soft = if hovered { theme::PRESSED_TEXT } else { theme::INK_SOFT };
        let s = if s.chars().count() > 42 { format!("{}…", s.chars().take(41).collect::<String>()) } else { s.to_string() };
        p.text(pos2(rect.center().x, rect.bottom() - 14.0), Align2::CENTER_CENTER, s, FontId::proportional(theme::BODY * 0.74), soft);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

fn small_menu_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).size(theme::BODY)).min_size(vec2(96.0, 32.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Category;
    use glam::Vec2;

    fn menu(main: bool) -> Menu {
        let base = std::env::temp_dir().join(format!("citysim-menu-{}", std::process::id()));
        Menu::new(base, Settings::default(), 6, 1, main)
    }

    #[test]
    fn pantallas_y_escape_del_menu_principal() {
        let mut m = menu(true);
        assert!(m.is_open() && m.in_main_menu());
        assert_eq!(m.escape(), None, "Esc en el menú principal no hace nada");
        assert!(m.is_open());
        m.push(Screen::Options);
        m.draft.mostrar_fps = true;
        assert_eq!(m.screen(), Some(&Screen::Options));
        assert_eq!(m.escape(), None);
        assert_eq!(m.screen(), Some(&Screen::Main), "Esc vuelve al menú principal");
        m.push(Screen::Load);
        m.confirm_delete = Some(PathBuf::from("x.json"));
        m.escape();
        assert_eq!(m.screen(), Some(&Screen::Load), "Esc primero cancela el borrado");
        m.escape();
        assert_eq!(m.screen(), Some(&Screen::Main));
        assert!(MenuAction::NewGame("x".into()).heavy() && !MenuAction::Save.heavy());
    }

    #[test]
    fn escape_en_el_juego_en_orden() {
        let mut g = Game::default();
        let mut gui = GameUi::default();
        let mut m = menu(false);
        assert!(!m.is_open());
        // 1) cancela el trazado; 2) cierra el listado; 3) el cajón; 4) abre la pausa.
        g.stage = 1;
        gui.list.open = true;
        gui.open = Some(Category::Calles);
        escape(Some(&mut g), &mut gui, &mut m);
        assert_eq!(g.stage, 0);
        assert!(gui.list.open && !m.is_open());
        escape(Some(&mut g), &mut gui, &mut m);
        assert!(!gui.list.open);
        escape(Some(&mut g), &mut gui, &mut m);
        assert!(gui.open.is_none() && !m.is_open());
        escape(Some(&mut g), &mut gui, &mut m);
        assert_eq!(m.screen(), Some(&Screen::Pause), "sin nada más que cerrar, abre la pausa");
        // En la pausa: Opciones y Esc vuelve a la pausa; otra vez Esc vuelve al juego.
        m.push(Screen::Options);
        assert_eq!(escape(Some(&mut g), &mut gui, &mut m), None);
        assert_eq!(m.screen(), Some(&Screen::Pause));
        assert_eq!(escape(Some(&mut g), &mut gui, &mut m), Some(MenuAction::Resume));
        assert!(!m.is_open());
        // Sin partida (sandbox) Esc no abre nada.
        assert_eq!(escape(None, &mut gui, &mut m), None);
        assert!(!m.is_open());
    }

    #[test]
    fn la_pausa_detiene_y_restituye_la_simulacion() {
        let mut s = Session::new_game(4, 1, new_game_cam(), 10.0);
        let mut m = menu(false);
        s.game.as_mut().unwrap().speed = 2.0;
        s.day_paused = false;
        m.open_pause();
        m.sync_pause(&mut s);
        assert_eq!(s.game.as_ref().unwrap().speed, 0.0);
        assert!(s.day_paused);
        m.sync_pause(&mut s);
        assert_eq!(m.escape(), Some(MenuAction::Resume));
        m.sync_pause(&mut s);
        assert_eq!(s.game.as_ref().unwrap().speed, 2.0);
        assert!(!s.day_paused);
    }

    #[test]
    fn guardar_cargar_y_cambios_sin_guardar() {
        let base = std::env::temp_dir().join(format!("citysim-menu-guardar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mut m = Menu::new(base.clone(), Settings::default(), 5, 1, true);
        m.background_scene = "vacia";
        let mut s = Session::new_game(5, 1, new_game_cam(), 10.0);
        let mut gui = GameUi::default();
        m.perform(MenuAction::NewGame("  Colonia del Sacramento ".into()), &mut s, None, &mut gui);
        assert!(!m.is_open());
        let g = s.game.as_mut().unwrap();
        assert_eq!(g.city_name, "Colonia del Sacramento");
        assert!(!m.dirty(g), "una partida recién empezada no pide guardar");
        let t = s.terrain.clone();
        g.city.found_plaza(&t, crate::city::game_demo::PLAZA, 0.0).unwrap();
        assert!(m.dirty(s.game.as_ref().unwrap()));
        // F5 (Guardar) con el juego abierto: avisa en la gaceta y queda guardado.
        m.perform(MenuAction::Save, &mut s, None, &mut gui);
        let g = s.game.as_ref().unwrap();
        assert!(g.message.starts_with("Partida guardada"), "{}", g.message);
        assert!(!m.dirty(g));
        let path = m.current.clone().unwrap();
        assert!(path.ends_with("colonia-del-sacramento.json"));
        // Guardar como: otro archivo y otro nombre.
        m.open_pause();
        m.push(Screen::SaveAs);
        m.perform(MenuAction::SaveAs("Maldonado".into()), &mut s, None, &mut gui);
        assert_eq!(m.screen(), Some(&Screen::Pause));
        assert!(m.current.as_ref().unwrap().ends_with("maldonado.json"));
        assert_eq!(m.saves.len(), 2);
        // Cambios y F9: se vuelve a lo guardado.
        s.game.as_mut().unwrap().city.money = 1;
        m.stack.clear();
        m.perform(MenuAction::QuickLoad, &mut s, None, &mut gui);
        let g = s.game.as_ref().unwrap();
        assert_eq!(g.city_name, "Maldonado");
        assert_eq!(g.city.money, 20_000);
        assert_eq!(g.city.net.edges.len(), 4);
        assert!(!m.dirty(g));
        // Una partida que no existe deja el aviso y no cambia nada.
        m.perform(MenuAction::Load(base.join("no-existe.json")), &mut s, None, &mut gui);
        assert!(s.game.as_ref().unwrap().message.contains("No se pudo cargar"));
        // Volver al menú principal: sin partida y con el menú abierto; borrar una partida.
        m.perform(MenuAction::Leave(Leave::ToMenu), &mut s, None, &mut gui);
        assert!(s.game.is_none() && m.in_main_menu());
        assert_eq!(m.saves.len(), 2);
        m.perform(MenuAction::Delete(path), &mut s, None, &mut gui);
        assert_eq!(m.saves.len(), 1);
        assert_eq!(m.perform(MenuAction::Leave(Leave::Quit), &mut s, None, &mut gui), Outcome::Quit);
        // Opciones: se guardan en la carpeta de datos.
        m.push(Screen::Options);
        let mut o = m.settings.clone();
        o.escala_interfaz = 1.2;
        assert_eq!(m.perform(MenuAction::ApplySettings(o.clone()), &mut s, None, &mut gui), Outcome::Settings);
        assert_eq!(Settings::load(&base), o);
        assert_eq!(m.screen(), Some(&Screen::Main));
        let _ = Vec2::ZERO;
        let _ = std::fs::remove_dir_all(&base);
    }

}
