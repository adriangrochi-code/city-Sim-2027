//! Interfaz del juego dentro del motor (egui con tema de plano colonial):
//! barra superior de estadísticas, barra de herramientas con fila de opciones, pista de la
//! herramienta con costo y error en vivo, ficha de inspección, avisos de la gaceta y un panel
//! de listado genérico (para políticas u ordenanzas).

pub mod paint;
pub mod theme;

use crate::city::game::{stage_name, Game, Inspect, Tool, TOOLS};
use crate::city::roads::RoadKind;
use crate::terrain::TerrainGen;
use egui::{Align2, Color32, RichText};
use std::collections::VecDeque;

/// Velocidades de la simulación (pausa, normal, rápida, muy rápida).
pub const SPEEDS: [(f32, &str); 4] = [(0.0, "II"), (1.0, ">"), (2.0, ">>"), (4.0, ">>>")];
/// Segundos que se ve un aviso de la gaceta (el último segundo y medio se desvanece).
pub const TOAST_SECS: f64 = 6.0;

/// Atajo y nombre visible de cada herramienta (en el orden de `TOOLS`).
pub fn tool_label(i: usize) -> String {
    let short = ["Plaza", "Recta", "Curva", "Damero", "Zona", "Quitar zona", "Demoler", "Inspeccionar"];
    format!("{}  {}", i + 1, short[i.min(short.len() - 1)])
}

/// ¿Qué fila de opciones lleva la herramienta?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Options {
    None,
    RoadKind,
    Brush,
}

pub fn options_for(tool: Tool) -> Options {
    match tool {
        Tool::Straight | Tool::Curve | Tool::Grid => Options::RoadKind,
        Tool::Zone | Tool::Dezone => Options::Brush,
        _ => Options::None,
    }
}

/// Texto de la barra superior (también usado en pruebas).
pub fn stats(game: &Game, hour: f32) -> Vec<String> {
    let pop = game.city.population();
    vec![
        capitalize(&game.city.date()),
        format!("{} pesos", fmt_thousands(game.city.money)),
        format!("Población: {}", fmt_thousands(pop as i64)),
        format!("Esperan casa: {}", game.city.pool),
        format!("Etapa: {}", stage_name(pop)),
        format!("{:02}:{:02}", hour as u32 % 24, (hour.fract() * 60.0) as u32),
    ]
}

pub fn fmt_thousands(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

// --- Panel de listado genérico -----------------------------------------------------------

/// Un ítem del panel genérico (por ejemplo, una política u ordenanza).
#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub title: String,
    pub desc: String,
    /// Costo mensual (o único) en pesos; `None` si no tiene.
    pub cost: Option<i64>,
    pub effects: Vec<String>,
    pub active: bool,
    /// Si se puede activar/desactivar ahora (si no, el botón aparece desactivado).
    pub enabled: bool,
    /// Motivo por el que no se puede (se muestra si `enabled` es falso).
    pub blocked: String,
}

/// Panel de listado reutilizable: título, introducción e ítems con botón de activar.
#[derive(Clone, Debug, Default)]
pub struct ListPanel {
    pub open: bool,
    pub title: String,
    pub intro: String,
    pub items: Vec<ListItem>,
}

impl ListPanel {
    /// Ejemplo de prueba (ordenanzas coloniales); el sistema de políticas real lo reemplaza.
    pub fn example() -> ListPanel {
        let item = |t: &str, d: &str, c: i64, e: &[&str], active: bool, enabled: bool, blocked: &str| ListItem {
            title: t.into(),
            desc: d.into(),
            cost: Some(c),
            effects: e.iter().map(|s| s.to_string()).collect(),
            active,
            enabled,
            blocked: blocked.into(),
        };
        ListPanel {
            open: false,
            title: "Ordenanzas del cabildo (ejemplo)".into(),
            intro: "Panel de prueba: así se van a mostrar las políticas de comida, agua y recursos.".into(),
            items: vec![
                item("Alumbrado público", "Los serenos encienden faroles de sebo en las calles principales.", 120, &["+ vida nocturna", "- 120 pesos al mes"], false, true, ""),
                item("Abasto controlado", "El cabildo fija el precio del pan y de la carne en la recova.", 60, &["+ felicidad de los pobres", "- ganancias de los pulperos"], true, true, ""),
                item("Empedrado obligatorio", "Los vecinos de la plaza deben empedrar su frente.", 0, &["+ valor del suelo", "- crecimiento más lento"], false, false, "Requiere 1.000 habitantes"),
            ],
        }
    }
}

// --- Estado y acciones ----------------------------------------------------------------------

/// Lo que el jugador pidió desde la interfaz (se aplica al juego después de dibujar).
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    Tool(usize),
    Kind(RoadKind),
    Brush(f32),
    Speed(f32),
    Hour(f32),
    CloseInspect,
    ToggleList,
    ToggleItem(usize),
}

/// Estado propio de la interfaz entre cuadros.
pub struct GameUi {
    pub list: ListPanel,
    toasts: VecDeque<(String, f64)>,
    last_seq: u64,
    /// Tiempo de la interfaz (segundos) para el desvanecido de los avisos.
    pub time: f64,
}

impl Default for GameUi {
    fn default() -> Self {
        GameUi { list: ListPanel::example(), toasts: VecDeque::new(), last_seq: 0, time: 0.0 }
    }
}

impl GameUi {
    /// Agrega a la gaceta los avisos nuevos del juego.
    pub fn sync_news(&mut self, game: &Game) {
        if game.message_seq != self.last_seq {
            self.last_seq = game.message_seq;
            self.push_toast(game.message.clone());
        }
    }

    pub fn push_toast(&mut self, text: String) {
        self.toasts.push_back((text, self.time));
        while self.toasts.len() > 3 {
            self.toasts.pop_front();
        }
    }

    pub fn toasts(&self) -> Vec<(String, f32)> {
        self.toasts
            .iter()
            .filter_map(|(t, at)| {
                let age = self.time - at;
                (age < TOAST_SECS).then(|| (t.clone(), (((TOAST_SECS - age) / 1.5).min(1.0)) as f32))
            })
            .collect()
    }

    /// Dibuja toda la interfaz del juego y devuelve las acciones pedidas.
    pub fn draw(&mut self, ctx: &egui::Context, game: &Game, terrain: &TerrainGen, hour: f32, extra_status: &str) -> Vec<UiAction> {
        let mut acts = Vec::new();
        self.top_bar(ctx, game, hour, extra_status, &mut acts);
        self.toolbar(ctx, game, &mut acts);
        self.hint(ctx, game, terrain);
        if let Some(info) = game.inspect_info() {
            inspect_panel(ctx, &info, &mut acts);
        }
        self.gazette(ctx);
        if self.list.open {
            list_panel(ctx, &self.list, &mut acts);
        }
        acts
    }

    fn top_bar(&self, ctx: &egui::Context, game: &Game, hour: f32, extra: &str, acts: &mut Vec<UiAction>) {
        egui::TopBottomPanel::top("barra").frame(theme::paper_frame().rounding(0.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let st = stats(game, hour);
                for s in &st[..5] {
                    ui.label(s);
                    ui.add_space(10.0);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for (v, label) in SPEEDS.iter().rev() {
                        if ui.selectable_label(game.speed == *v, *label).clicked() {
                            acts.push(UiAction::Speed(*v));
                        }
                    }
                    ui.add_space(6.0);
                    if ui.small_button("+").on_hover_text("Adelantar la hora").clicked() {
                        acts.push(UiAction::Hour(0.5));
                    }
                    ui.label(&st[5]);
                    if ui.small_button("-").on_hover_text("Atrasar la hora").clicked() {
                        acts.push(UiAction::Hour(-0.5));
                    }
                    ui.add_space(10.0);
                    if ui.selectable_label(self.list.open, "Ordenanzas").clicked() {
                        acts.push(UiAction::ToggleList);
                    }
                    if !extra.is_empty() {
                        ui.add_space(10.0);
                        ui.label(RichText::new(extra).small().color(theme::INK_SOFT));
                    }
                });
            });
        });
    }

    fn toolbar(&self, ctx: &egui::Context, game: &Game, acts: &mut Vec<UiAction>) {
        egui::Area::new("herramientas".into()).anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0)).show(ctx, |ui| {
            theme::paper_frame().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        for (i, (tool, _)) in TOOLS.iter().enumerate() {
                            let locked = game.city.plaza.is_none() && !matches!(tool, Tool::Plaza | Tool::Inspect);
                            let b = ui.add_enabled(!locked, egui::SelectableLabel::new(game.tool == *tool, tool_label(i)));
                            if b.clicked() {
                                acts.push(UiAction::Tool(i));
                            }
                        }
                    });
                    match options_for(game.tool) {
                        Options::RoadKind => {
                            ui.horizontal(|ui| {
                                ui.label("Tipo de calle (Tab):");
                                for k in [RoadKind::Dirt, RoadKind::Stone] {
                                    if ui.selectable_label(game.kind == k, k.name()).clicked() {
                                        acts.push(UiAction::Kind(k));
                                    }
                                }
                            });
                        }
                        Options::Brush => {
                            ui.horizontal(|ui| {
                                ui.label("Pincel ([ ]):");
                                let mut b = game.brush;
                                if ui.add(egui::Slider::new(&mut b, 10.0..=150.0).suffix(" m").integer()).changed() {
                                    acts.push(UiAction::Brush(b));
                                }
                            });
                        }
                        Options::None => {}
                    }
                });
            });
        });
    }

    fn hint(&self, ctx: &egui::Context, game: &Game, terrain: &TerrainGen) {
        let (help, err) = game.hint(terrain);
        egui::Area::new("pista".into()).anchor(Align2::LEFT_BOTTOM, egui::vec2(12.0, -110.0)).show(ctx, |ui| {
            theme::paper_frame().show(ui, |ui| {
                ui.set_max_width(400.0);
                ui.label(RichText::new(game.tool_name().to_uppercase()).small().color(theme::INK_SOFT));
                ui.label(help);
                if let Some(e) = err {
                    ui.label(RichText::new(e).color(theme::BAD));
                }
            });
        });
    }

    fn gazette(&self, ctx: &egui::Context) {
        let toasts = self.toasts();
        if toasts.is_empty() {
            return;
        }
        egui::Area::new("gaceta".into()).anchor(Align2::CENTER_TOP, egui::vec2(0.0, 58.0)).interactable(false).show(ctx, |ui| {
            for (text, alpha) in toasts {
                let fill = Color32::from_rgba_unmultiplied(232, 222, 197, (235.0 * alpha) as u8);
                let ink = Color32::from_rgba_unmultiplied(61, 43, 26, (255.0 * alpha) as u8);
                egui::Frame::none().fill(fill).stroke(egui::Stroke::new(1.5, Color32::from_rgba_unmultiplied(138, 111, 77, (255.0 * alpha) as u8))).rounding(3.0).inner_margin(egui::Margin::symmetric(14.0, 6.0)).show(ui, |ui| {
                    ui.label(RichText::new(format!("Gaceta · {text}")).size(theme::BODY * 1.05).color(ink));
                });
                ui.add_space(4.0);
            }
        });
    }
}

fn inspect_panel(ctx: &egui::Context, info: &Inspect, acts: &mut Vec<UiAction>) {
    egui::Area::new("ficha".into()).anchor(Align2::RIGHT_TOP, egui::vec2(-12.0, 60.0)).show(ctx, |ui| {
        theme::paper_frame().show(ui, |ui| {
            ui.set_width(330.0);
            ui.horizontal(|ui| {
                ui.heading(&info.title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("x").on_hover_text("Cerrar").clicked() {
                        acts.push(UiAction::CloseInspect);
                    }
                });
            });
            ui.separator();
            for l in &info.lines {
                ui.label(l);
            }
            if !info.why.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new("¿Por qué no crece?").strong());
                for w in &info.why {
                    ui.label(format!("· {w}"));
                }
            }
        });
    });
}

/// Panel de listado genérico (políticas, ordenanzas…) centrado en la pantalla.
pub fn list_panel(ctx: &egui::Context, panel: &ListPanel, acts: &mut Vec<UiAction>) {
    egui::Area::new("listado".into()).anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
        theme::paper_frame().show(ui, |ui| {
            ui.set_width(560.0);
            ui.horizontal(|ui| {
                ui.heading(&panel.title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("x").on_hover_text("Cerrar").clicked() {
                        acts.push(UiAction::ToggleList);
                    }
                });
            });
            if !panel.intro.is_empty() {
                ui.label(RichText::new(&panel.intro).color(theme::INK_SOFT));
            }
            ui.separator();
            egui::ScrollArea::vertical().max_height(ctx.screen_rect().height() * 0.6).show(ui, |ui| {
                for (i, it) in panel.items.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&it.title).size(theme::BODY * 1.15));
                        if it.active {
                            ui.label(RichText::new("· vigente").color(theme::GOOD));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let label = if it.active { "Derogar" } else { "Aprobar" };
                            let r = ui.add_enabled(it.enabled, egui::Button::new(label));
                            let r = if !it.enabled && !it.blocked.is_empty() { r.on_disabled_hover_text(&it.blocked) } else { r };
                            if r.clicked() {
                                acts.push(UiAction::ToggleItem(i));
                            }
                            if let Some(c) = it.cost {
                                if c > 0 {
                                    ui.label(format!("{} pesos/mes", fmt_thousands(c)));
                                }
                            }
                        });
                    });
                    ui.label(&it.desc);
                    for e in &it.effects {
                        let col = if e.starts_with('+') { theme::GOOD } else if e.starts_with('-') { theme::BAD } else { theme::INK };
                        ui.label(RichText::new(e).small().color(col));
                    }
                    if !it.enabled && !it.blocked.is_empty() {
                        ui.label(RichText::new(&it.blocked).small().color(theme::BAD));
                    }
                    ui.separator();
                }
            });
        });
    });
}

/// Aplica las acciones de la interfaz al juego (la hora vive en la sesión: se devuelve el
/// cambio para que el llamador la ajuste).
pub fn apply(acts: &[UiAction], game: &mut Game, list: &mut ListPanel) -> f32 {
    let mut hour_delta = 0.0;
    for a in acts {
        match a {
            UiAction::Tool(i) => game.set_tool(*i),
            UiAction::Kind(k) => game.kind = *k,
            UiAction::Brush(b) => game.brush = b.clamp(10.0, 150.0),
            UiAction::Speed(v) => game.speed = *v,
            UiAction::Hour(d) => hour_delta += d,
            UiAction::CloseInspect => game.inspected = None,
            UiAction::ToggleList => list.open = !list.open,
            UiAction::ToggleItem(i) => {
                if let Some(it) = list.items.get_mut(*i) {
                    if it.enabled {
                        it.active = !it.active;
                    }
                }
            }
        }
    }
    hour_delta
}

/// Entrada de egui para dibujar sin ventana (capturas): solo el tamaño de pantalla.
pub fn headless_input(width: u32, height: u32, ppp: f32, time: f64) -> egui::RawInput {
    let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32 / ppp, height as f32 / ppp))), time: Some(time), ..Default::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    input
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn herramientas_y_opciones() {
        assert_eq!(tool_label(0), "1  Plaza");
        assert_eq!(tool_label(7), "8  Inspeccionar");
        assert_eq!(TOOLS.len(), 8);
        assert_eq!(options_for(Tool::Curve), Options::RoadKind);
        assert_eq!(options_for(Tool::Zone), Options::Brush);
        assert_eq!(options_for(Tool::Inspect), Options::None);
    }

    #[test]
    fn textos_de_la_barra() {
        assert_eq!(fmt_thousands(20000), "20.000");
        assert_eq!(fmt_thousands(-1234567), "-1.234.567");
        assert_eq!(fmt_thousands(999), "999");
        let g = Game::default();
        let st = stats(&g, 9.5);
        assert!(st[0].starts_with(char::is_uppercase), "{}", st[0]);
        assert!(st[1].ends_with("pesos"));
        assert_eq!(st[4], "Etapa: Fundación");
        assert_eq!(st[5], "09:30");
    }

    #[test]
    fn acciones_y_panel_generico() {
        let mut g = Game::default();
        let mut l = ListPanel::example();
        let d = apply(&[UiAction::Speed(4.0), UiAction::Brush(500.0), UiAction::Hour(0.5), UiAction::ToggleList, UiAction::ToggleItem(0), UiAction::ToggleItem(2)], &mut g, &mut l);
        assert_eq!(g.speed, 4.0);
        assert_eq!(g.brush, 150.0);
        assert_eq!(d, 0.5);
        assert!(l.open);
        assert!(l.items[0].active, "el ítem habilitado se activa");
        assert!(!l.items[2].active, "el bloqueado no cambia");
        // Antes de fundar la plaza, las herramientas de calle se rechazan.
        apply(&[UiAction::Tool(1)], &mut g, &mut l);
        assert_eq!(g.tool, Tool::Plaza);
    }

    #[test]
    fn gaceta_y_ficha() {
        let t = TerrainGen::new(1726);
        let mut g = Game::default();
        let mut ui = GameUi::default();
        ui.sync_news(&g);
        assert_eq!(ui.toasts().len(), 1);
        g.say("Llega un barco");
        ui.sync_news(&g);
        assert_eq!(ui.toasts().len(), 2);
        ui.time += TOAST_SECS + 1.0;
        assert!(ui.toasts().is_empty(), "los avisos viejos se van");
        // Ficha de un solar y pista con error en vivo.
        let c = Vec2::new(13500.0, 16300.0);
        g.press(&t, c).expect("funda la plaza");
        g.set_tool(7);
        g.press(&t, c + Vec2::new(42.0, 70.0));
        let info = g.inspect_info().expect("hay ficha");
        assert!(!info.title.is_empty());
        g.set_tool(1);
        g.hover = Some(c + Vec2::new(42.0, -42.0));
        g.press(&t, c + Vec2::new(42.0, -42.0));
        g.hover = Some(Vec2::new(13500.0, 19000.0)); // en el río
        let (_, err) = g.hint(&t);
        assert!(err.is_some_and(|e| e.contains("agua")), "la pista avisa el motivo");
    }
}
