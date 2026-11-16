//! Interfaz del juego dentro del motor (egui con tema de plano colonial), con la estructura de
//! los constructores de ciudades modernos:
//! - barra superior compacta: ciudad, fecha y velocidad · tesoro, población y pobladores
//!   esperando · fichas de recursos y opinión;
//! - barra inferior de categorías con ícono y nombre; al elegir una se abre encima un cajón de
//!   tarjetas (ícono, nombre, costo; desactivadas con el motivo);
//! - pista compacta de la herramienta abajo a la izquierda, ficha de información a la derecha,
//!   avisos de la gaceta arriba al centro y un panel de listado genérico (políticas u ordenanzas).
//! Los atajos de teclado se muestran en los globos de ayuda, no en los botones.

pub mod icons;
pub mod paint;
pub mod theme;

use crate::city::game::fmt_thousands;
use crate::city::game::{stage_name, Game, Inspect, InspectKind, Tool, TOOLS};
use crate::city::roads::RoadKind;
use crate::terrain::TerrainGen;
use egui::{vec2, Align2, Color32, RichText, Sense};
use icons::Icon;
use std::collections::VecDeque;

/// Nombre de la ciudad en la barra superior.
pub const CITY_NAME: &str = "Montevideo";
/// Velocidades de la simulación (pausa, normal, rápida, muy rápida) y su atajo.
pub const SPEEDS: [(f32, &str, &str); 4] = [(0.0, "II", "Pausa · atajo: Espacio"), (1.0, ">", "Velocidad normal"), (2.0, ">>", "Rápido"), (4.0, ">>>", "Muy rápido")];
/// Segundos que se ve un aviso de la gaceta (el último segundo y medio se desvanece).
pub const TOAST_SECS: f64 = 6.0;
/// Motivo de las tarjetas de lo que el motor todavía no tiene.
pub const SOON: &str = "Próximamente";

/// Nombre corto y atajo de cada herramienta (en el orden de `TOOLS`).
pub fn tool_short(i: usize) -> &'static str {
    ["Plaza mayor", "Calle recta", "Calle curva", "Damero", "Zona mixta", "Quitar zona", "Demoler calle", "Inspeccionar"][i.min(7)]
}

pub fn tool_shortcut(i: usize) -> String {
    format!("atajo: {}", i + 1)
}

// --- Categorías y tarjetas ------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Calles,
    Zonas,
    Servicios,
    Produccion,
    Puerto,
    Ordenanzas,
    Capas,
    Demoler,
}

pub const CATEGORIES: [Category; 8] = [Category::Calles, Category::Zonas, Category::Servicios, Category::Produccion, Category::Puerto, Category::Ordenanzas, Category::Capas, Category::Demoler];

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Calles => "Calles",
            Category::Zonas => "Zonas",
            Category::Servicios => "Servicios",
            Category::Produccion => "Producción",
            Category::Puerto => "Puerto y muralla",
            Category::Ordenanzas => "Ordenanzas",
            Category::Capas => "Capas de datos",
            Category::Demoler => "Demoler",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Category::Calles => Icon::Calles,
            Category::Zonas => Icon::Zonas,
            Category::Servicios => Icon::Servicios,
            Category::Produccion => Icon::Produccion,
            Category::Puerto => Icon::Puerto,
            Category::Ordenanzas => Icon::Ordenanzas,
            Category::Capas => Icon::Capas,
            Category::Demoler => Icon::Demoler,
        }
    }
    pub fn tip(self) -> &'static str {
        match self {
            Category::Calles => "Plaza, calles rectas, curvas y damero · atajos 1 a 4, Tab cambia el tipo",
            Category::Zonas => "Pintar o quitar zona · atajos 5 y 6, [ y ] cambian el pincel",
            Category::Servicios => "Aljibes, serenos, bombas de incendio, lazareto, iglesia",
            Category::Produccion => "Hornos de ladrillos, atahonas, canteras, saladeros",
            Category::Puerto => "Muelle, muralla, portones y aduana",
            Category::Ordenanzas => "Decisiones del cabildo y presupuesto",
            Category::Capas => "Inspeccionar (atajo 8) y mapas de datos",
            Category::Demoler => "Demoler calles (atajo 7) y edificios",
        }
    }
    /// Categoría a la que pertenece una herramienta (para resaltarla).
    pub fn of_tool(t: Tool) -> Category {
        match t {
            Tool::Plaza | Tool::Straight | Tool::Curve | Tool::Grid => Category::Calles,
            Tool::Zone | Tool::Dezone => Category::Zonas,
            Tool::Demolish => Category::Demoler,
            Tool::Inspect => Category::Capas,
        }
    }
}

/// Tarjeta del cajón de una categoría.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub icon: Icon,
    pub name: &'static str,
    /// Costo visible ("2 pesos/m", "gratis").
    pub cost: String,
    /// Globo de ayuda (qué hace y su atajo).
    pub tip: String,
    pub action: Option<UiAction>,
    /// Motivo por el que está desactivada (corto, en la tarjeta; el globo lo explica).
    pub disabled: Option<String>,
    pub selected: bool,
}

impl Card {
    fn tool(game: &Game, i: usize, icon: Icon, cost: String, tip: &str) -> Card {
        let tool = TOOLS[i].0;
        let disabled = if tool == Tool::Plaza && game.city.plaza.is_some() {
            Some("Ya fundada".to_string())
        } else if game.city.plaza.is_none() && !matches!(tool, Tool::Plaza | Tool::Inspect) {
            Some("Falta la plaza".to_string())
        } else if matches!(tool, Tool::Straight | Tool::Curve | Tool::Grid) && (game.city.money as f32) < game.kind.cost_per_m() * 20.0 {
            Some("Sin fondos".to_string())
        } else {
            None
        };
        Card { icon, name: tool_short(i), cost, tip: format!("{tip} · {}", tool_shortcut(i)), action: Some(UiAction::Tool(i)), disabled, selected: game.tool == tool }
    }
    fn soon(icon: Icon, name: &'static str, tip: &str) -> Card {
        Card { icon, name, cost: String::new(), tip: format!("{tip} (todavía no está en el motor)"), action: None, disabled: Some(SOON.to_string()), selected: false }
    }
}

/// Tarjetas del cajón de cada categoría.
pub fn cards(cat: Category, game: &Game, list_open: bool) -> Vec<Card> {
    let per_m = format!("{} pesos/m", game.kind.cost_per_m());
    match cat {
        Category::Calles => vec![
            Card::tool(game, 0, Icon::Plaza, "gratis".into(), "Fundar la plaza mayor (Q y E la giran)"),
            Card::tool(game, 1, Icon::Recta, per_m.clone(), "Calle recta por tramos encadenados"),
            Card::tool(game, 2, Icon::Curva, per_m.clone(), "Calle curva: inicio, control y final"),
            Card::tool(game, 3, Icon::Damero, per_m, "Damero: un lado, el otro y la extensión"),
            Card::soon(Icon::Mejorar, "Mejorar calle", "Empedrar o poner faroles sin demoler"),
            Card::tool(game, 6, Icon::Demoler, "20 pesos".into(), "Demoler un tramo de calle"),
        ],
        Category::Zonas => vec![
            Card::tool(game, 4, Icon::Zonas, "gratis".into(), "Pintar zona mixta: vivienda, tienda y taller"),
            Card::tool(game, 5, Icon::QuitarZona, "gratis".into(), "Quitar la zona de los solares"),
            Card::soon(Icon::Quintas, "Quintas", "Chacras y quintas en el ejido"),
        ],
        Category::Servicios => vec![
            Card::soon(Icon::Aljibe, "Aljibe", "Agua de lluvia para la manzana"),
            Card::soon(Icon::Sereno, "Sereno", "Vigila y enciende los faroles"),
            Card::soon(Icon::Bomba, "Bomba de incendios", "Apaga los fuegos cercanos"),
            Card::soon(Icon::Lazareto, "Lazareto", "Cuarentena de los barcos"),
            Card::soon(Icon::Iglesia, "Iglesia matriz", "Se construye por fases"),
        ],
        Category::Produccion => vec![
            Card::soon(Icon::Produccion, "Horno de ladrillos", "Ladrillos para azoteas e iglesia"),
            Card::soon(Icon::Atahona, "Atahona", "Muele el trigo: harina para el pan"),
            Card::soon(Icon::Cantera, "Cantera", "Piedra para empedrar"),
            Card::soon(Icon::Saladero, "Saladero", "Carne salada y cueros"),
        ],
        Category::Puerto => vec![
            Card::soon(Icon::Puerto, "Muelle", "Barcos con pobladores y mercaderías"),
            Card::soon(Icon::Muralla, "Muralla", "Defensa de la plaza fuerte"),
            Card::soon(Icon::Aduana, "Aduana", "Cobra derechos al comercio"),
        ],
        Category::Ordenanzas => vec![
            Card { icon: Icon::Ordenanzas, name: "Ordenanzas", cost: "panel".into(), tip: "Abrir las ordenanzas del cabildo".into(), action: Some(UiAction::ToggleList), disabled: None, selected: list_open },
            Card::soon(Icon::Presupuesto, "Presupuesto", "Impuestos y gastos del cabildo"),
        ],
        Category::Capas => vec![
            Card::tool(game, 7, Icon::Lupa, "gratis".into(), "Ver la ficha de un solar, una calle o el terreno"),
            Card::soon(Icon::Valor, "Valor del suelo", "Mapa de calor del valor"),
            Card::soon(Icon::Agua, "Agua", "Alcance de aljibes y pozos"),
            Card::soon(Icon::Opinion, "Opinión", "Qué piensa cada barrio"),
        ],
        Category::Demoler => vec![
            Card::tool(game, 6, Icon::Demoler, "20 pesos".into(), "Demoler un tramo de calle"),
            Card::soon(Icon::QuitarZona, "Demoler edificio", "Derribar una casa para rehacer el solar"),
        ],
    }
}

/// Opciones que acompañan al cajón (interruptor de tipo de calle o pincel).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Options {
    None,
    RoadKind,
    Brush,
}

pub fn options_for(cat: Category) -> Options {
    match cat {
        Category::Calles => Options::RoadKind,
        Category::Zonas => Options::Brush,
        _ => Options::None,
    }
}

// --- Barra superior -------------------------------------------------------------------------

/// Textos de la barra superior (también usados en pruebas).
pub struct TopStats {
    pub date: String,
    pub hour: String,
    pub stage: &'static str,
    pub money: String,
    pub balance: String,
    pub population: String,
    pub waiting: String,
}

pub fn stats(game: &Game, hour: f32, balance: Option<i64>) -> TopStats {
    let pop = game.city.population();
    TopStats {
        date: capitalize(&game.city.date()),
        hour: format!("{:02}:{:02}", hour as u32 % 24, (hour.fract() * 60.0) as u32),
        stage: stage_name(pop),
        money: format!("{} pesos", fmt_thousands(game.city.money)),
        balance: match balance {
            Some(b) if b >= 0 => format!("+{}/mes", fmt_thousands(b)),
            Some(b) => format!("{}/mes", fmt_thousands(b)),
            None => "—/mes".into(),
        },
        population: fmt_thousands(pop as i64),
        waiting: fmt_thousands(game.city.pool as i64),
    }
}

/// Fichas de recursos y opinión de la derecha: (ícono, nombre, valor, ayuda).
/// El motor todavía no simula recursos ni opinión: se muestran con "—" y lo dicen.
pub fn chips() -> Vec<(Icon, &'static str, String, &'static str)> {
    let soon = "Todavía no se simula en el motor (llega con los recursos y las ordenanzas)";
    vec![
        (Icon::Ladrillo, "Ladrillos", "—".into(), soon),
        (Icon::Piedra, "Piedra", "—".into(), soon),
        (Icon::Pan, "Pan", "—".into(), soon),
        (Icon::Vecinos, "Vecinos", "—".into(), soon),
        (Icon::Comercio, "Comerciantes", "—".into(), soon),
        (Icon::Hacienda, "Hacendados", "—".into(), soon),
    ]
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
    /// Categoría con el cajón abierto.
    pub open: Option<Category>,
    toasts: VecDeque<(String, f64)>,
    last_seq: u64,
    /// Tiempo de la interfaz (segundos) para el desvanecido de los avisos.
    pub time: f64,
    /// Mes y tesoro al empezar el mes en curso, y la variación del último mes cerrado.
    month_mark: Option<(u32, i64)>,
    pub balance: Option<i64>,
    /// Alto de la barra de categorías del cuadro anterior (para apoyar el cajón encima).
    bar_h: f32,
    /// Alto del cajón abierto en el cuadro anterior (0 si está cerrado).
    drawer_h: f32,
}

impl Default for GameUi {
    fn default() -> Self {
        GameUi { list: ListPanel::example(), open: None, toasts: VecDeque::new(), last_seq: 0, time: 0.0, month_mark: None, balance: None, bar_h: 74.0, drawer_h: 0.0 }
    }
}

impl GameUi {
    /// Agrega a la gaceta los avisos nuevos del juego y lleva el balance mensual del tesoro.
    pub fn sync_news(&mut self, game: &Game) {
        if game.message_seq != self.last_seq {
            self.last_seq = game.message_seq;
            self.push_toast(game.message.clone());
        }
        let (m, money) = (game.city.month, game.city.money);
        match self.month_mark {
            Some((m0, start)) if m0 != m => {
                self.balance = Some(money - start);
                self.month_mark = Some((m, money));
            }
            None => self.month_mark = Some((m, money)),
            _ => {}
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
        self.category_bar(ctx, game);
        match self.open {
            Some(cat) => self.drawer(ctx, cat, game, &mut acts),
            None => self.drawer_h = 0.0,
        }
        self.hint(ctx, game, terrain);
        if let Some(info) = game.inspect_info() {
            inspect_panel(ctx, &info, &mut acts);
        }
        self.gazette(ctx);
        if self.list.open {
            let reserved = 8.0 + self.bar_h + if self.drawer_h > 0.0 { self.drawer_h + 6.0 } else { 0.0 } + 10.0;
            list_panel(ctx, &self.list, reserved, &mut acts);
        }
        acts
    }

    fn top_bar(&self, ctx: &egui::Context, game: &Game, hour: f32, extra: &str, acts: &mut Vec<UiAction>) {
        let st = stats(game, hour, self.balance);
        // Fondo: una franja de papel; encima, tres grupos (izquierda, centro, derecha).
        egui::TopBottomPanel::top("barra").exact_height(40.0).frame(egui::Frame::none().fill(theme::PAPER).stroke(egui::Stroke::new(1.5_f32, theme::BORDER))).show(ctx, |_| {});
        let y = 4.0;
        egui::Area::new("barra-izq".into()).anchor(Align2::LEFT_TOP, vec2(12.0, y)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(CITY_NAME).size(theme::BODY * 1.15).strong()).on_hover_text(format!("Etapa: {}", st.stage));
                ui.label(RichText::new(&st.date).color(theme::INK_SOFT));
                ui.label(RichText::new(&st.hour).color(theme::INK_SOFT)).on_hover_text("Hora del día · + y - la cambian");
                ui.add_space(4.0);
                for (v, label, tip) in SPEEDS {
                    if ui.selectable_label(game.speed == v, label).on_hover_text(tip).clicked() {
                        acts.push(UiAction::Speed(v));
                    }
                }
                ui.add_space(2.0);
                if ui.small_button("−").on_hover_text("Atrasar la hora · atajo: -").clicked() {
                    acts.push(UiAction::Hour(-0.5));
                }
                if ui.small_button("+").on_hover_text("Adelantar la hora · atajo: +").clicked() {
                    acts.push(UiAction::Hour(0.5));
                }
            });
        });
        egui::Area::new("barra-centro".into()).anchor(Align2::CENTER_TOP, vec2(-40.0, y)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let bal_col = if self.balance.is_some_and(|b| b < 0) { theme::BAD } else { theme::GOOD };
                ui.label(RichText::new(&st.money).strong()).on_hover_text("Tesoro del cabildo");
                ui.label(RichText::new(&st.balance).color(bal_col)).on_hover_text("Variación del tesoro en el último mes (se calcula al cerrar cada mes)");
                ui.add_space(14.0);
                icons::show(ui, Icon::Vecinos, 18.0, theme::INK);
                ui.label(&st.population).on_hover_text(format!("Población · etapa: {}", st.stage));
                ui.add_space(10.0);
                icons::show(ui, Icon::Aduana, 18.0, theme::INK_SOFT);
                ui.label(&st.waiting).on_hover_text("Pobladores que esperan casa");
            });
        });
        egui::Area::new("barra-der".into()).anchor(Align2::RIGHT_TOP, vec2(-10.0, y + 3.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if !extra.is_empty() {
                    ui.label(RichText::new(extra).small().color(theme::INK_SOFT));
                }
                for (icon, name, value, tip) in chips() {
                    chip(ui, icon, &value).on_hover_text(format!("{name}: {tip}"));
                }
            });
        });
    }

    fn category_bar(&mut self, ctx: &egui::Context, game: &Game) {
        let current = Category::of_tool(game.tool);
        let r = egui::Area::new("categorias".into()).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -8.0)).show(ctx, |ui| {
            theme::paper_frame().inner_margin(egui::Margin::symmetric(8.0, 6.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for cat in CATEGORIES {
                        let open = self.open == Some(cat);
                        let resp = category_button(ui, cat, open, cat == current).on_hover_text(cat.tip());
                        if resp.clicked() {
                            self.open = if open { None } else { Some(cat) };
                        }
                    }
                });
            });
        });
        self.bar_h = r.response.rect.height();
    }

    fn drawer(&mut self, ctx: &egui::Context, cat: Category, game: &Game, acts: &mut Vec<UiAction>) {
        let r = egui::Area::new("cajon".into()).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -8.0 - self.bar_h - 6.0)).show(ctx, |ui| {
            theme::paper_frame().inner_margin(egui::Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    icons::show(ui, cat.icon(), 18.0, theme::INK_SOFT);
                    ui.label(RichText::new(cat.name().to_uppercase()).small().color(theme::INK_SOFT));
                    match options_for(cat) {
                        Options::RoadKind => {
                            ui.add_space(16.0);
                            ui.label(RichText::new("Tipo:").small());
                            for k in [RoadKind::Dirt, RoadKind::Stone] {
                                let tip = format!("{} · {} pesos/m · Tab alterna", k.name(), k.cost_per_m());
                                if ui.selectable_label(game.kind == k, RichText::new(capitalize(k.name())).small()).on_hover_text(tip).clicked() {
                                    acts.push(UiAction::Kind(k));
                                }
                            }
                        }
                        Options::Brush => {
                            ui.add_space(16.0);
                            ui.label(RichText::new("Pincel:").small()).on_hover_text("Atajos: [ y ]");
                            let mut b = game.brush;
                            if ui.add(egui::Slider::new(&mut b, 10.0..=150.0).suffix(" m").integer()).on_hover_text("Atajos: [ y ]").changed() {
                                acts.push(UiAction::Brush(b));
                            }
                        }
                        Options::None => {}
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").on_hover_text("Cerrar el cajón").clicked() {
                            self.open = None;
                        }
                    });
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for c in cards(cat, game, self.list.open) {
                        let resp = card_button(ui, &c);
                        let tip = match &c.disabled {
                            Some(why) if why != SOON => format!("{} · no disponible: {why}", c.tip),
                            _ => c.tip.clone(),
                        };
                        if resp.on_hover_text(tip).clicked() && c.disabled.is_none() {
                            if let Some(a) = c.action.clone() {
                                acts.push(a);
                            }
                        }
                    }
                });
            });
        });
        self.drawer_h = r.response.rect.height();
    }

    fn hint(&self, ctx: &egui::Context, game: &Game, terrain: &TerrainGen) {
        let (help, err) = game.hint(terrain);
        // Primera línea: qué hace; el resto (costo, tipo) va aparte y más chico.
        let mut lines = help.lines();
        let first = lines.next().unwrap_or("").to_string();
        let rest: Vec<&str> = lines.collect();
        egui::Area::new("pista".into()).anchor(Align2::LEFT_BOTTOM, vec2(10.0, -10.0)).interactable(false).show(ctx, |ui| {
            theme::paper_frame().inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
                ui.set_max_width(300.0);
                ui.label(RichText::new(game.tool_name().to_uppercase()).small().color(theme::INK_SOFT));
                ui.label(RichText::new(first).small());
                for l in rest {
                    ui.label(RichText::new(l).small().strong());
                }
                if let Some(e) = err {
                    ui.label(RichText::new(e).small().color(theme::BAD));
                }
            });
        });
    }

    fn gazette(&self, ctx: &egui::Context) {
        let toasts = self.toasts();
        if toasts.is_empty() {
            return;
        }
        egui::Area::new("gaceta".into()).anchor(Align2::CENTER_TOP, vec2(0.0, 50.0)).interactable(false).show(ctx, |ui| {
            for (text, alpha) in toasts {
                let fill = Color32::from_rgba_unmultiplied(240, 232, 212, (240.0 * alpha) as u8);
                let ink = Color32::from_rgba_unmultiplied(61, 43, 26, (255.0 * alpha) as u8);
                let soft = Color32::from_rgba_unmultiplied(110, 86, 60, (255.0 * alpha) as u8);
                egui::Frame::none().fill(fill).stroke(egui::Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(138, 111, 77, (255.0 * alpha) as u8))).rounding(6.0).inner_margin(egui::Margin::symmetric(14.0, 6.0)).show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("GACETA").small().color(soft));
                        ui.label(RichText::new(text).color(ink));
                    });
                });
                ui.add_space(4.0);
            }
        });
    }
}

/// Botón de categoría: ícono arriba y nombre abajo.
fn category_button(ui: &mut egui::Ui, cat: Category, open: bool, current: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(86.0, 56.0), Sense::click());
    let p = ui.painter();
    let fill = if open { theme::PRESSED } else if resp.hovered() { theme::PAPER_LIGHT } else { Color32::TRANSPARENT };
    p.rect_filled(rect, 5.0, fill);
    if current && !open {
        p.line_segment([rect.left_bottom() + vec2(18.0, -2.0), rect.right_bottom() + vec2(-18.0, -2.0)], egui::Stroke::new(2.5_f32, theme::PRESSED));
    }
    let ink = if open { theme::PRESSED_TEXT } else { theme::INK };
    icons::paint(p, egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 18.0), vec2(26.0, 26.0)), cat.icon(), ink);
    p.text(rect.center_bottom() + vec2(0.0, -9.0), Align2::CENTER_CENTER, cat.name(), egui::FontId::proportional(theme::BODY * 0.72), ink);
    resp
}

/// Tarjeta del cajón: ícono, nombre y costo (o el motivo si está desactivada).
fn card_button(ui: &mut egui::Ui, c: &Card) -> egui::Response {
    let sense = if c.disabled.is_none() { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(120.0, 92.0), sense);
    let p = ui.painter();
    let off = c.disabled.is_some();
    let fill = if c.selected { theme::PRESSED } else if resp.hovered() && !off { theme::PAPER_LIGHT } else { theme::PAPER_DARK.gamma_multiply(if off { 0.55 } else { 0.85 }) };
    p.rect_filled(rect, 6.0, fill);
    p.rect_stroke(rect, 6.0, egui::Stroke::new(1.0_f32, if off { theme::BORDER.gamma_multiply(0.5) } else { theme::BORDER }));
    let ink = if c.selected { theme::PRESSED_TEXT } else if off { theme::INK_SOFT.gamma_multiply(0.7) } else { theme::INK };
    icons::paint(p, egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 24.0), vec2(32.0, 32.0)), c.icon, ink);
    p.text(rect.center_top() + vec2(0.0, 54.0), Align2::CENTER_CENTER, c.name, egui::FontId::proportional(theme::BODY * 0.78), ink);
    let (sub, col) = match &c.disabled {
        Some(why) => (why.as_str(), if why == SOON { ink } else { theme::BAD }),
        None => (c.cost.as_str(), if c.selected { theme::PRESSED_TEXT } else { theme::INK_SOFT }),
    };
    p.text(rect.center_top() + vec2(0.0, 75.0), Align2::CENTER_CENTER, sub, egui::FontId::proportional(theme::BODY * 0.66), col);
    resp
}

/// Ficha pequeña de la barra superior: ícono y valor.
fn chip(ui: &mut egui::Ui, icon: Icon, value: &str) -> egui::Response {
    egui::Frame::none().fill(theme::PAPER_DARK).rounding(10.0).inner_margin(egui::Margin::symmetric(7.0, 1.0)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.horizontal(|ui| {
            icons::show(ui, icon, 16.0, theme::INK);
            ui.label(RichText::new(value).small());
        });
    })
    .response
    .interact(Sense::hover())
}

/// Ficha de información a la derecha: cabecera, secciones y botones de acción.
fn inspect_panel(ctx: &egui::Context, info: &Inspect, acts: &mut Vec<UiAction>) {
    egui::Area::new("ficha".into()).anchor(Align2::RIGHT_TOP, vec2(-10.0, 52.0)).show(ctx, |ui| {
        theme::paper_frame().inner_margin(egui::Margin::same(0.0)).show(ui, |ui| {
            ui.set_width(310.0);
            // Cabecera en tinta sobre papel oscuro.
            egui::Frame::none().fill(theme::PAPER_DARK).rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }).inner_margin(egui::Margin::symmetric(12.0, 6.0)).show(ui, |ui| {
                ui.set_width(310.0 - 24.0);
                ui.horizontal(|ui| {
                    let icon = match info.kind {
                        InspectKind::Lot => Icon::Aduana,
                        InspectKind::Road => Icon::Recta,
                        InspectKind::Terrain => Icon::Lupa,
                    };
                    icons::show(ui, icon, 22.0, theme::INK);
                    ui.label(RichText::new(&info.title).size(theme::BODY * 1.2));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").on_hover_text("Cerrar · atajo: Esc").clicked() {
                            acts.push(UiAction::CloseInspect);
                        }
                    });
                });
            });
            egui::Frame::none().inner_margin(egui::Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
                section(ui, "Datos");
                egui::Grid::new("datos").num_columns(2).spacing(vec2(12.0, 3.0)).show(ui, |ui| {
                    for l in &info.lines {
                        match l.split_once(": ") {
                            Some((k, v)) => {
                                ui.label(RichText::new(k).small().color(theme::INK_SOFT));
                                ui.label(RichText::new(v).small());
                            }
                            None => {
                                ui.label(RichText::new(l).small());
                                ui.label("");
                            }
                        }
                        ui.end_row();
                    }
                });
                if !info.why.is_empty() {
                    ui.add_space(4.0);
                    section(ui, "¿Por qué no crece?");
                    for w in &info.why {
                        ui.label(RichText::new(format!("· {w}")).small());
                    }
                }
                let actions: &[(&str, &str, usize)] = match info.kind {
                    InspectKind::Lot => &[("Zonificar", "Pintar zona mixta alrededor · atajo: 5", 4), ("Quitar zona", "Quitar la zona · atajo: 6", 5)],
                    InspectKind::Road => &[("Demoler tramo", "Elegir la herramienta de demoler · atajo: 7", 6), ("Calle nueva", "Trazar una calle recta desde aquí · atajo: 2", 1)],
                    InspectKind::Terrain => &[("Calle recta", "Trazar una calle · atajo: 2", 1)],
                };
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for (label, tip, tool) in actions {
                        if ui.button(RichText::new(*label).small()).on_hover_text(*tip).clicked() {
                            acts.push(UiAction::Tool(*tool));
                        }
                    }
                });
            });
        });
    });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title.to_uppercase()).small().color(theme::INK_SOFT));
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.top() - 2.0, egui::Stroke::new(1.0_f32, theme::BORDER.gamma_multiply(0.5)));
    ui.add_space(2.0);
}

/// Panel de listado genérico (políticas, ordenanzas…) centrado en la pantalla.
/// `reserved_bottom` es el alto ocupado abajo (barra y cajón) que el panel no debe tapar.
pub fn list_panel(ctx: &egui::Context, panel: &ListPanel, reserved_bottom: f32, acts: &mut Vec<UiAction>) {
    egui::Area::new("listado".into()).anchor(Align2::CENTER_TOP, egui::vec2(0.0, 52.0)).show(ctx, |ui| {
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
            egui::ScrollArea::vertical().max_height((ctx.screen_rect().height() - 52.0 - reserved_bottom - 110.0).max(120.0)).show(ui, |ui| {
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
                            if !it.enabled && !it.blocked.is_empty() {
                                ui.label(RichText::new(&it.blocked).color(theme::BAD));
                            } else if let Some(c) = it.cost {
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
/// Escala de la interfaz según el alto de la ventana en píxeles físicos: 800 px de alto es la
/// medida de diseño; más chica se achica (hasta 0,75) y más grande crece (hasta 2).
pub fn zoom_for(height_px: u32, scale_factor: f32) -> f32 {
    (height_px as f32 / 800.0 / scale_factor.max(0.5)).clamp(0.75, 2.0)
}

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
    fn escala_con_la_ventana() {
        assert_eq!(zoom_for(800, 1.0), 1.0);
        assert!(zoom_for(1440, 1.0) > 1.7);
        assert_eq!(zoom_for(400, 1.0), 0.75);
        assert_eq!(zoom_for(4000, 1.0), 2.0);
        // En una pantalla de densidad doble el sistema ya duplica: no se vuelve a agrandar.
        assert_eq!(zoom_for(1600, 2.0), 1.0);
        let i = headless_input(1280, 720, 1.5, 0.0);
        assert_eq!(i.screen_rect.unwrap().width(), 1280.0 / 1.5);
    }

    #[test]
    fn categorias_y_tarjetas() {
        let mut g = Game::default();
        assert_eq!(CATEGORIES.len(), 8);
        // Cada herramienta del juego aparece en alguna tarjeta, con su atajo en el globo.
        for (i, _) in TOOLS.iter().enumerate() {
            let found = CATEGORIES.iter().flat_map(|c| cards(*c, &g, false)).find(|c| c.action == Some(UiAction::Tool(i))).expect("la herramienta tiene tarjeta");
            assert!(found.tip.contains(&format!("atajo: {}", i + 1)), "{}", found.tip);
            assert!(!found.name.starts_with(char::is_numeric), "el atajo no va en el botón");
        }
        // Antes de fundar, las calles están desactivadas con el motivo; la plaza no.
        let calles = cards(Category::Calles, &g, false);
        assert!(calles[0].disabled.is_none());
        assert_eq!(calles[1].disabled.as_deref(), Some("Falta la plaza"));
        assert_eq!(calles[4].disabled.as_deref(), Some(SOON), "lo que falta en el motor sale como próximamente");
        g.city.plaza = Some((Vec2::ZERO, 0.0));
        let calles = cards(Category::Calles, &g, false);
        assert_eq!(calles[0].disabled.as_deref(), Some("Ya fundada"));
        assert!(calles[1].disabled.is_none());
        g.city.money = 10;
        assert_eq!(cards(Category::Calles, &g, false)[1].disabled.as_deref(), Some("Sin fondos"));
        assert_eq!(options_for(Category::Calles), Options::RoadKind);
        assert_eq!(options_for(Category::Zonas), Options::Brush);
        assert_eq!(options_for(Category::Servicios), Options::None);
        assert_eq!(Category::of_tool(Tool::Inspect), Category::Capas);
    }

    #[test]
    fn textos_de_la_barra() {
        assert_eq!(fmt_thousands(20000), "20.000");
        assert_eq!(fmt_thousands(-1234567), "-1.234.567");
        assert_eq!(fmt_thousands(999), "999");
        let g = Game::default();
        let st = stats(&g, 9.5, None);
        assert!(st.date.starts_with(char::is_uppercase), "{}", st.date);
        assert!(st.money.ends_with("pesos"));
        assert_eq!(st.stage, "Fundación");
        assert_eq!(st.hour, "09:30");
        assert_eq!(stats(&g, 9.5, Some(-1500)).balance, "-1.500/mes");
        assert_eq!(stats(&g, 9.5, Some(320)).balance, "+320/mes");
        assert!(chips().iter().all(|c| c.2 == "—"), "sin datos inventados");
    }

    #[test]
    fn balance_mensual() {
        let mut g = Game::default();
        let mut ui = GameUi::default();
        ui.sync_news(&g);
        assert_eq!(ui.balance, None);
        g.city.money -= 700;
        ui.sync_news(&g);
        assert_eq!(ui.balance, None, "el mes no cerró");
        g.city.month += 1;
        ui.sync_news(&g);
        assert_eq!(ui.balance, Some(-700));
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
