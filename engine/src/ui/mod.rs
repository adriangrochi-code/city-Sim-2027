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
pub mod services;
pub mod poblacion;
pub mod theme;

use crate::city::game::fmt_thousands;
use crate::city::game::{stage_name, Game, Inspect, InspectKind, Tool, TOOLS};
use crate::city::economy::{Economy, Res, Sector, Tax, ORDINANCES, TAX_MAX, TOGGLE_COST};
use crate::city::roads::RoadKind;
use crate::city::lots::{Demand, Density, ZoneKind};
use crate::terrain::TerrainGen;
use egui::{vec2, Align2, Color32, RichText, Sense};
use icons::Icon;
use std::collections::VecDeque;

/// Nombre de la ciudad en la barra superior.
pub const CITY_NAME: &str = "Montevideo";
/// Velocidades de la simulación (pausa, normal, rápida, muy rápida) y su atajo.
pub const SPEEDS: [(f32, &str, &str); 4] = [(0.0, "II", "Pausa · atajo: Espacio"), (1.0, ">", "Velocidad normal · atajo: 0 cambia"), (2.0, ">>", "Rápido (×2) · atajo: 0 cambia"), (4.0, ">>>", "Muy rápido (×4) · atajo: 0 cambia")];
/// Alto de la barra superior (en puntos de la interfaz).
pub const TOP_BAR_H: f32 = 40.0;
/// Segundos que se ve un aviso de la gaceta (el último segundo y medio se desvanece).
pub const TOAST_SECS: f64 = 6.0;
/// Motivo de las tarjetas de lo que el motor todavía no tiene.
pub const SOON: &str = "Próximamente";

/// Nombre corto y atajo de cada herramienta (en el orden de `TOOLS`).
pub fn tool_short(i: usize) -> &'static str {
    ["Plaza mayor", "Calle recta", "Calle curva", "Damero", "Zonificar", "Quitar zona", "Demoler calle", "Inspeccionar"][i.min(7)]
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
            Category::Servicios => "Agua, educación, salud, seguridad, saneamiento e iglesia",
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
            Tool::Service => Category::Servicios,
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
    fn zone(game: &Game, k: ZoneKind, icon: Icon, name: &'static str, tip: &str) -> Card {
        let mut c = Card::tool(game, 4, icon, String::new(), tip);
        let d = game.city.demand().of(k);
        c.name = name;
        c.cost = format!("demanda {}{} %", if d >= 0.0 { "+" } else { "" }, (d * 100.0).round() as i32);
        c.action = Some(UiAction::Zone(k));
        c.selected = game.tool == Tool::Zone && game.zone_kind == k;
        c
    }
    /// Tarjeta de una herramienta (para los cajones armados fuera de este módulo).
    pub fn tool_pub(game: &Game, i: usize, icon: Icon, cost: String, tip: &str) -> Card {
        Card::tool(game, i, icon, cost, tip)
    }
    fn soon(icon: Icon, name: &'static str, tip: &str) -> Card {
        Card { icon, name, cost: String::new(), tip: format!("{tip} (todavía no está en el motor)"), action: None, disabled: Some(SOON.to_string()), selected: false }
    }
}

/// Tarjetas del cajón de cada categoría.
pub fn cards(cat: Category, game: &Game, list_open: bool, budget_open: bool) -> Vec<Card> {
    let per_m = format!("{} pesos/m", game.kind.cost_per_m());
    if let Some(c) = services::cards(cat, game) {
        return c;
    }
    match cat {
        Category::Calles => vec![
            Card::tool(game, 0, Icon::Plaza, "gratis".into(), "Fundar la plaza mayor (R la gira)"),
            Card::tool(game, 1, Icon::Recta, per_m.clone(), "Calle recta por tramos encadenados"),
            Card::tool(game, 2, Icon::Curva, per_m.clone(), "Calle curva: inicio, control y final"),
            Card::tool(game, 3, Icon::Damero, per_m, "Damero: un lado, el otro y la extensión"),
            Card::soon(Icon::Mejorar, "Mejorar calle", "Empedrar o poner faroles sin demoler"),
            Card::tool(game, 6, Icon::Demoler, "20 pesos".into(), "Demoler un tramo de calle"),
        ],
        Category::Zonas => vec![
            Card::zone(game, ZoneKind::Residencial, Icon::Vecinos, "Residencial", "Casas: rancho, casa de azotea y casa de altos. Traen pobladores"),
            Card::zone(game, ZoneKind::Comercial, Icon::Comercio, "Comercial", "Pulperías, tiendas y almacenes: empleo y alcabala"),
            Card::zone(game, ZoneKind::Industrial, Icon::Produccion, "Industrial", "Talleres, curtiembres y barracas: empleo, ladrillos, sebo y cueros. Molestan a los vecinos"),
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
            Card { icon: Icon::Ordenanzas, name: "Ordenanzas", cost: format!("{} vigentes", game.city.economy.active.len()), tip: "Abrir las ordenanzas del cabildo".into(), action: Some(UiAction::ToggleList), disabled: None, selected: list_open },
            Card { icon: Icon::Presupuesto, name: "Presupuesto", cost: "impuestos".into(), tip: "Ingresos, gastos e impuestos del cabildo".into(), action: Some(UiAction::ToggleBudget), disabled: None, selected: budget_open },
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
    /// Pestañas del cajón de Servicios (agua, educación, salud, seguridad, saneamiento).
    Grupos,
}

pub fn options_for(cat: Category) -> Options {
    match cat {
        Category::Calles => Options::RoadKind,
        Category::Zonas => Options::Brush,
        Category::Servicios => Options::Grupos,
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
        population: fmt_thousands(game.city.habitantes() as i64),
        waiting: fmt_thousands(game.city.pool as i64),
    }
}

/// Coma decimal, a la uruguaya: 1,25.
pub fn fmt_dec(v: f32, decimals: usize) -> String {
    format!("{v:.decimals$}").replace('.', ",")
}

/// Fichas de recursos y opinión de la derecha: (ícono, valor, ayuda). Salen de la economía
/// de la ciudad (existencias, precio del pan y opinión de los tres sectores).
pub fn chips(game: &Game) -> Vec<(Icon, String, String)> {
    let e = &game.city.economy;
    let st = |r: Res| e.stock(r);
    let opinion = |i: usize| {
        let v = e.opinion[i].round() as i32;
        let mood = if v >= 65 { "contentos" } else if v >= 40 { "conformes" } else { "enojados" };
        (format!("{v}"), format!("Opinión de los {}: {v} de 100 ({mood})", Sector::ALL[i].name().to_lowercase()))
    };
    let stock_tip = |r: Res| {
        let s = st(r);
        format!("{}: {} en depósito (caben {}) · precio {} · el mes pasado: +{} producidos, +{} traídos, -{} usados", r.name(), s.amount.round() as i64, s.capacity.round() as i64, fmt_dec(s.price, 2), s.produced.round() as i64, s.imported.round() as i64, s.consumed.round() as i64)
    };
    let (v0, t0) = opinion(0);
    let (v1, t1) = opinion(1);
    let (v2, t2) = opinion(2);
    vec![
        weather_chip(game),
        (Icon::Ladrillo, fmt_thousands(st(Res::Ladrillos).amount.round() as i64), stock_tip(Res::Ladrillos)),
        (Icon::Piedra, fmt_thousands(st(Res::Piedra).amount.round() as i64), stock_tip(Res::Piedra)),
        (Icon::Pan, fmt_dec(e.price(Res::Pan), 2), format!("Precio del pan (1,00 es el normal). {}", stock_tip(Res::Pan))),
        (Icon::Vecinos, v0, t0),
        (Icon::Comercio, v1, t1),
        (Icon::Hacienda, v2, t2),
        poblacion::chip(game),
    ]
}

/// Ficha del clima: estación y temperatura; el globo cuenta el mes, el río, la luna y qué
/// se puede sembrar.
pub fn weather_chip(game: &Game) -> (Icon, String, String) {
    use crate::city::{climate, water};
    let c = &game.city;
    let m = (c.month % 12) as usize;
    // Antes de cerrar el primer mes se muestran las normales del mes.
    let normal = climate::Weather {
        month: m,
        temp: climate::T_MEDIA[m] + climate::EPOCA_C,
        tmin: climate::T_MIN_MEDIA[m] + climate::EPOCA_C,
        rain: climate::LLUVIA_MM[m],
        soil: c.climate.soil,
        ..Default::default()
    };
    let w = if c.climate.last.temp == 0.0 && c.climate.last.rain == 0.0 { &normal } else { &c.climate.last };
    let phase = c.moon_phase();
    let mut tip = format!(
        "{} de {} · {} °C (mínimas de {} °C) · lluvia del mes {} mm (normal {} mm) · agua en el suelo {} % · heladas: {} días",
        capitalize(climate::season(m)),
        climate::MESES[m],
        fmt_dec(w.temp, 1),
        fmt_dec(w.tmin, 1),
        w.rain.round() as i32,
        climate::LLUVIA_MM[m] as i32,
        (w.soil / climate::SUELO_MAX * 100.0).round() as i32,
        w.frost_days.round() as i32
    );
    tip += match c.climate.regime {
        climate::Regime::Lluvioso => " · años lluviosos",
        climate::Regime::Seco => " · años secos",
        climate::Regime::Normal => "",
    };
    if w.drought {
        tip += " · SEQUÍA";
    }
    tip += &format!("\nRío: {} m sobre el nivel medio", fmt_dec(c.water_level(12.0), 2));
    match c.water.wind {
        water::Wind::Sudestada { .. } => tip += " · sudestada",
        water::Wind::Pampero { .. } => tip += " · pampero",
        water::Wind::Calma => {}
    }
    tip += &format!(
        "\nViento: {}, {} km/h ({}) · media de {} km/h en {}",
        water::wind_name(c.water.wind_dir),
        (c.water.wind_speed * 3.6).round() as i32,
        water::wind_strength(c.water.wind_speed),
        (water::VIENTO_MEDIO[m] * 3.6).round() as i32,
        climate::MESES[m]
    );
    tip += &format!(" · {}", water::moon_name(phase));
    tip += &format!("\nPasturas al {} %", (c.climate.pasture * 100.0).round() as i32);
    for line in c.climate.sowing_advice(c.month) {
        tip += &format!("\n{}", capitalize(&line));
    }
    tip += "\nDatos: normales 1991–2020 del INUMET (estación Prado), algo más frescas por la época.";
    // Rumbo de donde viene el viento, como en los partes meteorológicos.
    const RUMBOS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SO", "O", "NO"];
    let arrow = RUMBOS[(((c.water.wind_dir.rem_euclid(360.0) + 22.5) / 45.0) as usize) % 8];
    let value = format!("{} °C · {} {} km/h", w.temp.round() as i32, arrow, (c.water.wind_speed * 3.6).round() as i32);
    (Icon::Agua, value, tip)
}

/// Balance del último mes cerrado según el presupuesto de la economía.
pub fn month_balance(game: &Game) -> Option<i64> {
    let last = &game.city.economy.last;
    (!last.lines.is_empty()).then(|| last.balance())
}

pub fn capitalize(s: &str) -> String {
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
    /// Si los ítems son las ordenanzas de la economía (se rehacen cada cuadro y el botón las
    /// aprueba o deroga de verdad); si no, es un listado suelto (el ejemplo de las pruebas).
    pub ordinances: bool,
    pub title: String,
    pub intro: String,
    pub items: Vec<ListItem>,
}

impl ListPanel {
    /// Las ordenanzas del cabildo, leídas de la economía de la ciudad.
    pub fn ordinances(e: &Economy, money: i64, open: bool) -> ListPanel {
        let sectors = ["vecinos", "comerciantes", "hacendados"];
        let items = ORDINANCES
            .iter()
            .map(|o| {
                let mut effects: Vec<String> = o.opinion.iter().zip(sectors).filter(|(v, _)| **v != 0).map(|(v, s)| format!("{}{} opinión de los {s}", if *v > 0 { "+ " } else { "- " }, v.abs())).collect();
                if o.monthly > 0 {
                    effects.push(format!("- {} pesos al mes", o.monthly));
                }
                ListItem { title: o.name.into(), desc: o.desc.into(), cost: (o.monthly > 0).then_some(o.monthly), effects, active: e.is_on(o.id), enabled: money >= TOGGLE_COST, blocked: format!("Faltan pesos (el cambio cuesta {TOGGLE_COST})") }
            })
            .collect();
        ListPanel {
            open,
            ordinances: true,
            title: "Ordenanzas del cabildo".into(),
            intro: format!("Aprobar o derogar una ordenanza cuesta {TOGGLE_COST} pesos de papeleo. Hay {} vigentes.", e.active.len()),
            items,
        }
    }

    /// Ejemplo suelto para las pruebas del panel genérico.
    #[cfg(test)]
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
            ordinances: false,
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
    /// Pincel de zonas: tipo (elige también la herramienta) y densidad.
    Zone(ZoneKind),
    Density(Density),
    Brush(f32),
    Speed(f32),
    Hour(f32),
    CloseInspect,
    ToggleList,
    ToggleItem(usize),
    ToggleBudget,
    /// Cambia un impuesto (índice en `Tax::ALL`) en tantos puntos.
    Tax(usize, i32),
    /// Servicios: colocar, capas de datos y órdenes (ver `ui::services`).
    Servicio(services::Act),
    /// Abre un panel de detalle (por ejemplo, los vecinos de una casa).
    Abrir(Detail),
}

/// Estado propio de la interfaz entre cuadros.
pub struct GameUi {
    pub list: ListPanel,
    /// Panel de presupuesto abierto.
    pub budget: bool,
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
    /// ¿Estaba abierto el cajón de zonas el cuadro anterior? (al cerrarlo se deja de zonificar)
    zones_were_open: bool,
    /// Panel de detalle abierto desde una ficha de la barra (recurso, opinión o clima).
    pub detail: Option<Detail>,
}

impl Default for GameUi {
    fn default() -> Self {
        GameUi { zones_were_open: false, detail: None, list: ListPanel::ordinances(&Economy::default(), 0, false), budget: false, open: None, toasts: VecDeque::new(), last_seq: 0, time: 0.0, month_mark: None, balance: None, bar_h: 74.0, drawer_h: 0.0 }
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

    /// Cierra lo último que se abrió (listado, presupuesto, ficha, cajón). `false` si no había nada.
    pub fn close_one(&mut self, game: &mut Game) -> bool {
        if self.list.open {
            self.list.open = false;
        } else if self.budget {
            self.budget = false;
        } else if game.inspected.is_some() {
            game.inspected = None;
        } else if self.open.is_some() {
            self.open = None;
        } else {
            return false;
        }
        true
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
        if self.list.ordinances {
            self.list = ListPanel::ordinances(&game.city.economy, game.city.money, self.list.open);
        }
        self.top_bar(ctx, game, hour, extra_status, &mut acts);
        self.category_bar(ctx, game);
        // Al cerrar el cajón de zonas se deja la herramienta de zonas (como en el género): así
        // el plano de colores de las zonas desaparece del mapa.
        let zones_open = self.open == Some(Category::Zonas);
        if self.zones_were_open && !zones_open && matches!(game.tool, Tool::Zone | Tool::Dezone) {
            let inspect = TOOLS.iter().position(|t| t.0 == Tool::Inspect).unwrap();
            acts.push(UiAction::Tool(inspect));
        }
        self.zones_were_open = zones_open;
        match self.open {
            Some(cat) => self.drawer(ctx, cat, game, &mut acts),
            None => self.drawer_h = 0.0,
        }
        self.hint(ctx, game, terrain);
        // Junto al cursor, mientras se traza una calle: largo, ángulo respecto del damero y costo.
        if let (Some(txt), Some(pos)) = (game.draft_label(terrain), ctx.pointer_hover_pos()) {
            egui::Area::new(egui::Id::new("trazado"))
                .fixed_pos(pos + vec2(18.0, 14.0))
                .order(egui::Order::Tooltip)
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new(txt).small());
                    });
                });
        }
        if let Some(info) = game.inspect_info_en(terrain) {
            inspect_panel(ctx, &info, &mut acts);
        }
        // Paneles pedidos desde la ficha (los vecinos de una casa).
        acts.retain(|a| match a {
            UiAction::Abrir(d) => {
                self.detail = Some(*d);
                self.budget = false;
                false
            }
            _ => true,
        });
        self.gazette(ctx);
        if let Some(d) = self.detail {
            let reserved = 8.0 + self.bar_h + if self.drawer_h > 0.0 { self.drawer_h + 6.0 } else { 0.0 } + 10.0;
            if matches!(d, Detail::Poblacion(_) | Detail::Hogares(_) | Detail::Persona(_)) {
                if let Some(nav) = poblacion::panel(ctx, game, d, reserved, &mut acts) {
                    self.detail = nav;
                }
            } else if detail_panel(ctx, game, d, reserved) {
                self.detail = None;
            }
        }
        if self.budget {
            let reserved = 8.0 + self.bar_h + if self.drawer_h > 0.0 { self.drawer_h + 6.0 } else { 0.0 } + 10.0;
            if budget_panel(ctx, game, reserved, &mut acts) {
                self.budget = false;
            }
        }
        if self.list.open {
            let reserved = 8.0 + self.bar_h + if self.drawer_h > 0.0 { self.drawer_h + 6.0 } else { 0.0 } + 10.0;
            list_panel(ctx, &self.list, reserved, &mut acts);
        }
        acts
    }

    fn top_bar(&mut self, ctx: &egui::Context, game: &Game, hour: f32, extra: &str, acts: &mut Vec<UiAction>) {
        let st = stats(game, hour, month_balance(game).or(self.balance));
        // Una franja de papel con tres grupos: ciudad y tiempo · tesoro y gente · fichas.
        let frame = egui::Frame::none().fill(theme::PAPER).stroke(egui::Stroke::new(1.5_f32, theme::BORDER)).inner_margin(egui::Margin::symmetric(12.0, 4.0));
        egui::TopBottomPanel::top("barra").exact_height(TOP_BAR_H).frame(frame).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(&game.city_name).size(theme::BODY * 1.15).strong()).on_hover_text(format!("Etapa: {}", st.stage));
                ui.label(RichText::new(&st.date).color(theme::INK_SOFT));
                ui.label(RichText::new(&st.hour).color(theme::INK_SOFT)).on_hover_text("Hora del día · atajos: , y .");
                for (v, label, tip) in SPEEDS {
                    if ui.selectable_label(game.speed == v, label).on_hover_text(tip).clicked() {
                        acts.push(UiAction::Speed(v));
                    }
                }
                if ui.small_button("−").on_hover_text("Atrasar la hora · atajo: ,").clicked() {
                    acts.push(UiAction::Hour(-0.5));
                }
                if ui.small_button("+").on_hover_text("Adelantar la hora · atajo: .").clicked() {
                    acts.push(UiAction::Hour(0.5));
                }
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(6.0);
                let bal_col = if month_balance(game).or(self.balance).is_some_and(|b| b < 0) { theme::BAD } else { theme::GOOD };
                ui.label(RichText::new(&st.money).strong()).on_hover_text("Tesoro del cabildo");
                ui.label(RichText::new(&st.balance).color(bal_col)).on_hover_text("Balance del último mes cerrado (detalle en Ordenanzas → Presupuesto)");
                ui.add_space(10.0);
                icons::show(ui, Icon::Vecinos, 18.0, theme::INK);
                // Clic en la población: el panel de vecinos en la calle (tránsito).
                let en_calle: u32 = game.city.agentes.en_la_calle().iter().sum();
                let pop = ui.add(egui::Label::new(&st.population).sense(Sense::click())).on_hover_text(format!("Población · etapa: {} · {en_calle} en la calle\n(clic: vecinos y tránsito)", st.stage));
                if pop.clicked() {
                    self.detail = if self.detail == Some(Detail::Transito) { None } else { Some(Detail::Transito) };
                    self.budget = false;
                }
                ui.add_space(6.0);
                icons::show(ui, Icon::Aduana, 18.0, theme::INK_SOFT);
                ui.label(&st.waiting).on_hover_text("Pobladores que esperan casa");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    // De derecha a izquierda: se invierten para leerlas en orden.
                    let targets = chip_details();
                    for (i, (icon, value, tip)) in chips(game).into_iter().enumerate().rev() {
                        let d = targets.get(i).copied();
                        let selected = d.is_some() && self.detail == d;
                        if chip(ui, icon, &value, selected).on_hover_text(format!("{tip}\n(clic: más información)")).clicked() {
                            self.detail = if selected { None } else { d };
                            if self.detail.is_some() {
                                self.budget = false;
                            }
                        }
                    }
                    if !extra.is_empty() {
                        ui.label(RichText::new(extra).small().color(theme::INK_SOFT));
                    }
                });
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
                            ui.label(RichText::new("Densidad:").small());
                            for d in Density::ALL {
                                let tip = match d {
                                    Density::Baja => "Una planta: ranchos, pulperías y talleres",
                                    Density::Media => "Hasta dos plantas (casas de altos, almacenes, barracas)",
                                    Density::Alta => "Empieza en edificios de material y crece hasta dos plantas con más gente. Necesita calle empedrada",
                                };
                                if ui.selectable_label(game.density == d, RichText::new(capitalize(d.name())).small()).on_hover_text(tip).clicked() {
                                    acts.push(UiAction::Density(d));
                                }
                            }
                            ui.add_space(12.0);
                            demand_meter(ui, game.city.demand());
                            ui.add_space(12.0);
                            ui.label(RichText::new("Pincel:").small()).on_hover_text("Atajos: [ y ]");
                            let mut b = game.brush;
                            if ui.add(egui::Slider::new(&mut b, 10.0..=150.0).suffix(" m").integer()).on_hover_text("Atajos: [ y ]").changed() {
                                acts.push(UiAction::Brush(b));
                            }
                        }
                        Options::Grupos => {
                            ui.add_space(16.0);
                            for g in crate::city::services::Grupo::PESTAÑAS {
                                if ui.selectable_label(game.grupo == g, RichText::new(g.nombre()).small()).on_hover_text(format!("Servicios de {}", g.nombre().to_lowercase())).clicked() {
                                    acts.push(UiAction::Servicio(services::Act::Grupo(g)));
                                }
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
                    for c in cards(cat, game, self.list.open, self.budget) {
                        let resp = card_button(ui, &c);
                        let tip = match &c.disabled {
                            Some(why) if why != SOON => format!("{} · no disponible: {why}", c.tip),
                            _ => c.tip.clone(),
                        };
                        if resp.on_hover_text(tip).clicked() && c.disabled.is_none() {
                            match c.action.clone() {
                                Some(UiAction::ToggleBudget) => self.budget = !self.budget,
                                Some(a) => acts.push(a),
                                None => {}
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
                if let Some(d) = game.draft_label(terrain) {
                    ui.label(RichText::new(d).small().strong().color(theme::INK));
                }
                if matches!(game.tool, Tool::Straight | Tool::Curve | Tool::Grid) {
                    ui.label(RichText::new("Se engancha a 45° y 90° de la calle de arranque y del damero; Shift: libre").small().color(theme::INK_SOFT));
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

/// Ficha pequeña de la barra superior: ícono y valor (dibujada a mano para que no dependa
/// de la dirección del renglón).
fn chip(ui: &mut egui::Ui, icon: Icon, value: &str, selected: bool) -> egui::Response {
    let font = egui::FontId::proportional(theme::BODY * 0.8);
    let galley = ui.painter().layout_no_wrap(value.to_string(), font, theme::INK);
    let (icon_w, pad, gap) = (16.0, 8.0, 4.0);
    let size = vec2(pad + icon_w + gap + galley.size().x + pad, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let bg = if selected { theme::PRESSED.gamma_multiply(0.35) } else if resp.hovered() { theme::PAPER } else { theme::PAPER_DARK };
    p.rect_filled(rect, 11.0, bg);
    let ir = egui::Rect::from_center_size(egui::pos2(rect.left() + pad + icon_w / 2.0, rect.center().y), vec2(icon_w, icon_w));
    icons::paint(p, ir, icon, theme::INK);
    p.galley(egui::pos2(ir.right() + gap, rect.center().y - galley.size().y / 2.0), galley, theme::INK);
    resp
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
                        InspectKind::Service(_) => Icon::Servicios,
                        InspectKind::Church => Icon::Iglesia,
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
                    InspectKind::Lot => &[("Zonificar", "Pintar la zona elegida alrededor · atajo: 5", 4), ("Quitar zona", "Quitar la zona · atajo: 6", 5)],
                    InspectKind::Road => &[("Demoler tramo", "Elegir la herramienta de demoler · atajo: 7", 6), ("Calle nueva", "Trazar una calle recta desde aquí · atajo: 2", 1)],
                    InspectKind::Terrain => &[("Calle recta", "Trazar una calle · atajo: 2", 1)],
                    InspectKind::Service(_) | InspectKind::Church => &[],
                };
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if let Some(l) = info.lot.filter(|_| info.vecinos > 0) {
                        if ui.button(RichText::new("Vecinos").small()).on_hover_text("Quiénes viven en esta casa: familias, economía e historias").clicked() {
                            acts.push(UiAction::Abrir(Detail::Hogares(l)));
                        }
                    }
                    for (label, tip, tool) in actions {
                        if ui.button(RichText::new(*label).small()).on_hover_text(*tip).clicked() {
                            acts.push(UiAction::Tool(*tool));
                        }
                    }
                    for (label, tip, act) in services::inspect_buttons(info.kind) {
                        if ui.button(RichText::new(label).small()).on_hover_text(tip).clicked() {
                            acts.push(act);
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

/// Paneles de detalle que se abren desde las fichas de la barra superior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Clima,
    Recurso(Res),
    Opinion(usize),
    /// Vecinos en la calle, viajes y demoras (se abre con un clic en la población).
    Transito,
    /// Panel de la población (pestaña: resumen, desigualdad, felicidad, crónica).
    Poblacion(u8),
    /// Los hogares de una casa (id del solar).
    Hogares(u32),
    /// La ficha de una persona.
    Persona(u32),
}

/// A qué panel lleva cada ficha de `chips()` (mismo orden).
pub fn chip_details() -> [Detail; 8] {
    [Detail::Clima, Detail::Recurso(Res::Ladrillos), Detail::Recurso(Res::Piedra), Detail::Recurso(Res::Pan), Detail::Opinion(0), Detail::Opinion(1), Detail::Opinion(2), Detail::Poblacion(0)]
}

/// Gráfico de barras simple (los últimos meses, el más reciente a la derecha). `marks` dibuja
/// una raya por barra (por ejemplo, la lluvia normal de ese mes).
fn bars(ui: &mut egui::Ui, values: &[f32], marks: Option<&[f32]>, color: Color32, label: &str) {
    let w = ui.available_width().min(340.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, theme::PAPER_DARK.gamma_multiply(0.5));
    if values.is_empty() {
        p.text(rect.center(), Align2::CENTER_CENTER, "Sin datos todavía", egui::FontId::proportional(11.0), theme::INK_SOFT);
        return;
    }
    let top = values.iter().chain(marks.unwrap_or(&[]).iter()).cloned().fold(1e-6f32, f32::max);
    let n = values.len().max(12) as f32;
    let bw = rect.width() / n;
    let off = (n as usize - values.len()) as f32 * bw;
    for (i, v) in values.iter().enumerate() {
        let x = rect.left() + off + i as f32 * bw;
        let h = (v / top).clamp(0.0, 1.0) * (rect.height() - 14.0);
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(x + 1.0, rect.bottom() - 12.0 - h), egui::pos2(x + bw - 1.0, rect.bottom() - 12.0)), 1.0, color);
        if let Some(m) = marks.and_then(|m| m.get(i)) {
            let y = rect.bottom() - 12.0 - (m / top).clamp(0.0, 1.0) * (rect.height() - 14.0);
            p.line_segment([egui::pos2(x, y), egui::pos2(x + bw, y)], egui::Stroke::new(1.5_f32, theme::INK));
        }
    }
    p.text(egui::pos2(rect.left() + 3.0, rect.bottom() - 1.0), Align2::LEFT_BOTTOM, label, egui::FontId::proportional(10.0), theme::INK_SOFT);
    p.text(egui::pos2(rect.right() - 3.0, rect.top() + 1.0), Align2::RIGHT_TOP, format!("máx. {}", fmt_dec(top, if top < 10.0 { 2 } else { 0 })), egui::FontId::proportional(10.0), theme::INK_SOFT);
    resp.on_hover_text(values.iter().map(|v| fmt_dec(*v, if top < 10.0 { 2 } else { 0 })).collect::<Vec<_>>().join(" · "));
}

/// Una fila "nombre: valor" en una grilla de dos columnas.
fn kv(ui: &mut egui::Ui, k: &str, v: String, col: Color32) {
    ui.label(RichText::new(k).small());
    ui.label(RichText::new(v).small().color(col));
    ui.end_row();
}

/// Panel de detalle de una ficha. Devuelve `true` si se pidió cerrarlo.
fn detail_panel(ctx: &egui::Context, game: &Game, d: Detail, reserved_bottom: f32) -> bool {
    use crate::city::{climate, water};
    let c = &game.city;
    let e = &c.economy;
    let mut close = false;
    egui::Area::new("detalle".into()).anchor(Align2::LEFT_TOP, vec2(10.0, 52.0)).show(ctx, |ui| {
        theme::paper_frame().show(ui, |ui| {
            ui.set_width(360.0);
            let (icon, title) = match d {
                Detail::Clima => (Icon::Agua, "Clima, río y campo".to_string()),
                Detail::Recurso(r) => (match r { Res::Pan => Icon::Pan, Res::Piedra => Icon::Piedra, _ => Icon::Ladrillo }, r.name().to_string()),
                Detail::Opinion(i) => ([Icon::Vecinos, Icon::Comercio, Icon::Hacienda][i], format!("Opinión de los {}", Sector::ALL[i].name().to_lowercase())),
                Detail::Transito => (Icon::Vecinos, "Vecinos y tránsito".to_string()),
                // Los de la población se dibujan en `poblacion::panel`.
                Detail::Poblacion(_) | Detail::Hogares(_) | Detail::Persona(_) => (Icon::Familia, "Población".to_string()),
            };
            ui.horizontal(|ui| {
                icons::show(ui, icon, 22.0, theme::INK);
                ui.heading(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Cerrar").clicked() {
                        close = true;
                    }
                });
            });
            let max_h = (ctx.screen_rect().height() - 52.0 - reserved_bottom - 24.0).max(160.0);
            egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| match d {
                Detail::Recurso(r) => {
                    let s = e.stock(r);
                    egui::Grid::new("rec").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                        kv(ui, "En depósito", format!("{} (caben {})", fmt_thousands(s.amount.round() as i64), fmt_thousands(s.capacity.round() as i64)), theme::INK);
                        let pc = if s.price > 1.3 { theme::BAD } else if s.price < 0.9 { theme::GOOD } else { theme::INK };
                        kv(ui, "Precio", format!("{} (1,00 es el normal)", fmt_dec(s.price, 2)), pc);
                    });
                    section(ui, "Último mes");
                    egui::Grid::new("mes").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                        kv(ui, "Producido", format!("+{}", fmt_thousands(s.produced.round() as i64)), theme::GOOD);
                        kv(ui, "Traído por el puerto", format!("+{}", fmt_thousands(s.imported.round() as i64)), theme::GOOD);
                        kv(ui, "Usado", format!("−{}", fmt_thousands(s.consumed.round() as i64)), theme::BAD);
                        kv(ui, "Embarcado", format!("−{}", fmt_thousands(s.exported.round() as i64)), theme::BAD);
                    });
                    section(ui, "De dónde sale y para qué se usa");
                    let text = match r {
                        Res::Pan => format!(
                            "Las chacras del ejido: el trigo se siembra en otoño y se cosecha en diciembre (y se trilla en enero); el maíz, en marzo y abril. La última cosecha de trigo rindió {} % y la de maíz {} %. Cada vecino come una ración por mes: el grano de la cosecha tiene que durar hasta la próxima.{}",
                            (c.climate.trigo.last_yield * 100.0).round() as i32,
                            (c.climate.maiz.last_yield * 100.0).round() as i32,
                            if e.is_on(crate::city::economy::Ordinance::Posito) { format!(" El pósito guarda {} raciones para los años malos.", fmt_thousands(e.posito.round() as i64)) } else { " Con el pósito (Ordenanzas) se guarda grano para los años malos.".into() }
                        ),
                        Res::Ladrillos => format!("Los hornos del ejido y los talleres de las zonas industriales. Con lluvia y frío el adobe no seca: este mes rinden al {} %. Los piden las casas de azotea y de altos.", (c.climate.brick_factor() * 100.0).round() as i32),
                        Res::Piedra => "Por ahora llega por el puerto (las canteras todavía no están en el motor). La usan las calles empedradas y, más adelante, la muralla.".into(),
                        Res::Carne => format!("Las estancias del campo: dependen de las pasturas, hoy al {} %.", (c.climate.pasture * 100.0).round() as i32),
                        _ => String::new(),
                    };
                    ui.label(RichText::new(text).small());
                    section(ui, "Últimos meses");
                    let k = r as usize;
                    let amounts: Vec<f32> = e.history.iter().rev().take(12).rev().map(|h| h.stocks[k][0]).collect();
                    let prices: Vec<f32> = e.history.iter().rev().take(12).rev().map(|h| h.stocks[k][1]).collect();
                    bars(ui, &amounts, None, theme::GOOD, "En depósito");
                    bars(ui, &prices, Some(&vec![1.0; prices.len()]), theme::PRESSED, "Precio (raya: el normal)");
                }
                Detail::Opinion(i) => {
                    let v = e.opinion[i];
                    let mood = if v >= 65.0 { "contentos" } else if v >= 40.0 { "conformes" } else { "enojados" };
                    ui.label(RichText::new(format!("{} de 100: {mood}", v.round() as i32)).strong());
                    section(ui, "Qué la mueve");
                    egui::Grid::new("op").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                        for &o in &e.active {
                            let info = crate::city::economy::ord_info(o);
                            let x = info.opinion[i];
                            if x != 0 {
                                kv(ui, info.name, format!("{x:+}"), if x > 0 { theme::GOOD } else { theme::BAD });
                            }
                        }
                        for t in Tax::ALL {
                            let over = e.tax(t) - t.normal();
                            if over != 0 {
                                let w = if t.sector() as usize == i { 2.5 } else { 0.5 };
                                let x = -(over as f32) * w;
                                kv(ui, &format!("{} al {} %", t.name(), e.tax(t)), format!("{:+.0}", x), if x > 0.0 { theme::GOOD } else { theme::BAD });
                            }
                        }
                        if c.poblacion.fundada {
                            let a = c.poblacion.animo_sectores()[i];
                            let x = ((a - 0.55) * 40.0).clamp(-15.0, 15.0);
                            kv(ui, &format!("Cómo viven (felicidad {} %)", (a * 100.0).round() as i32), format!("{x:+.0}"), if x >= 0.0 { theme::GOOD } else { theme::BAD });
                        }
                        match i {
                            0 => {
                                let pan = -(e.price(Res::Pan) - 1.0) * 25.0;
                                kv(ui, "Precio del pan", format!("{pan:+.0}"), if pan >= 0.0 { theme::GOOD } else { theme::BAD });
                                let carne = -(e.price(Res::Carne) - 1.0) * 10.0;
                                kv(ui, "Precio de la carne", format!("{carne:+.0}"), if carne >= 0.0 { theme::GOOD } else { theme::BAD });
                            }
                            1 => kv(ui, "Barcos y comercio", "más barcos, más conformes".into(), theme::INK),
                            _ => {
                                if c.climate.last.drought {
                                    kv(ui, "Sequía", "−10".into(), theme::BAD);
                                }
                                kv(ui, "Precio de los cueros en el mundo", "sube con las guerras lejanas".into(), theme::INK);
                            }
                        }
                    });
                    ui.label(RichText::new("La opinión se acerca de a poco a lo que piden estas causas, mes a mes.").small().color(theme::INK_SOFT));
                    section(ui, "Últimos meses");
                    let h: Vec<f32> = e.history.iter().rev().take(12).rev().map(|h| h.opinion[i]).collect();
                    bars(ui, &h, Some(&vec![50.0; h.len()]), theme::PRESSED, "Opinión (raya: 50)");
                }
                Detail::Transito => {
                    let a = &c.agentes;
                    ui.label(RichText::new(format!("{}, {}", capitalize(a.dia_de_semana()), stats(game, a.e.hora, None).hour)).strong());
                    egui::Grid::new("tr").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                        for (k, v, malo) in crate::city::agentes::resumen(a) {
                            kv(ui, &k, v, if malo { theme::BAD } else { theme::INK });
                        }
                    });
                    ui.label(RichText::new("Si los viajes en carreta o a caballo tardan mucho más de lo previsto, los vecinos se quejan. Inspeccioná una calle para ver su tránsito.").small().color(theme::INK_SOFT));
                }
                Detail::Poblacion(_) | Detail::Hogares(_) | Detail::Persona(_) => {}
                Detail::Clima => {
                    let m = (c.month % 12) as usize;
                    let w = &c.climate.last;
                    egui::Grid::new("cl").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                        kv(ui, "Estación", format!("{} ({})", capitalize(climate::season(m)), climate::MESES[m]), theme::INK);
                        kv(ui, "Temperatura media", format!("{} °C (normal {} °C)", fmt_dec(w.temp, 1), fmt_dec(climate::T_MEDIA[m] + climate::EPOCA_C, 1)), theme::INK);
                        kv(ui, "Lluvia del mes", format!("{} mm (normal {} mm)", w.rain.round() as i32, climate::LLUVIA_MM[m] as i32), theme::INK);
                        kv(ui, "Agua en el suelo", format!("{} %", (c.climate.soil / climate::SUELO_MAX * 100.0).round() as i32), if c.climate.soil < 30.0 { theme::BAD } else { theme::GOOD });
                        kv(ui, "Heladas", format!("{} días", w.frost_days.round() as i32), theme::INK);
                        kv(ui, "Años", match c.climate.regime { climate::Regime::Lluvioso => "lluviosos", climate::Regime::Seco => "secos", climate::Regime::Normal => "normales" }.into(), theme::INK);
                        kv(ui, "Pasturas", format!("{} %", (c.climate.pasture * 100.0).round() as i32), theme::INK);
                    });
                    section(ui, "Río, viento y luna");
                    egui::Grid::new("rio").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                        kv(ui, "Nivel del río", format!("{} m", fmt_dec(c.water_level(12.0), 2)), theme::INK);
                        kv(ui, "Viento", format!("{}, {} km/h", water::wind_name(c.water.wind_dir), (c.water.wind_speed * 3.6).round() as i32), theme::INK);
                        kv(ui, "Luna", water::moon_name(c.moon_phase()).into(), theme::INK);
                    });
                    section(ui, "El campo: qué se puede sembrar");
                    for line in c.climate.sowing_advice(c.month) {
                        ui.label(RichText::new(capitalize(&line)).small());
                    }
                    section(ui, "Lluvia de los últimos meses");
                    let rain: Vec<f32> = c.climate.history.iter().map(|w| w.rain).collect();
                    let normal: Vec<f32> = c.climate.history.iter().map(|w| climate::LLUVIA_MM[w.month]).collect();
                    bars(ui, &rain, Some(&normal), Color32::from_rgb(90, 120, 160), "mm (raya: normal del INUMET)");
                    ui.label(RichText::new("Normales 1991–2020 de la estación Prado (INUMET), algo más frescas por la época.").small().color(theme::INK_SOFT));
                }
            });
        });
    });
    close
}

/// Panel de presupuesto: impuestos (con − y +) y el detalle del último mes cerrado.
/// Devuelve `true` si se pidió cerrarlo.
fn budget_panel(ctx: &egui::Context, game: &Game, reserved_bottom: f32, acts: &mut Vec<UiAction>) -> bool {
    let e = &game.city.economy;
    let mut close = false;
    egui::Area::new("presupuesto".into()).anchor(Align2::LEFT_TOP, vec2(10.0, 52.0)).show(ctx, |ui| {
        theme::paper_frame().show(ui, |ui| {
            ui.set_width(360.0);
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Presupuesto, 22.0, theme::INK);
                ui.heading("Presupuesto del cabildo");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Cerrar").clicked() {
                        close = true;
                    }
                });
            });
            section(ui, "Impuestos");
            egui::Grid::new("impuestos").num_columns(3).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
                for (i, t) in Tax::ALL.iter().enumerate() {
                    let rate = e.tax(*t);
                    ui.label(RichText::new(t.name()).small()).on_hover_text(format!("Lo habitual es {} %; más arriba molesta a los {}", t.normal(), t.sector().name().to_lowercase()));
                    let col = if rate > t.normal() { theme::BAD } else if rate < t.normal() { theme::GOOD } else { theme::INK };
                    ui.label(RichText::new(format!("{rate} %")).small().color(col));
                    ui.horizontal(|ui| {
                        if ui.add_enabled(rate > 0, egui::Button::new(RichText::new("−").small())).on_hover_text("Bajar un punto").clicked() {
                            acts.push(UiAction::Tax(i, -1));
                        }
                        if ui.add_enabled(rate < TAX_MAX, egui::Button::new(RichText::new("+").small())).on_hover_text(format!("Subir un punto (hasta {TAX_MAX} %)")).clicked() {
                            acts.push(UiAction::Tax(i, 1));
                        }
                    });
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            section(ui, "Último mes");
            let last = &e.last;
            if last.lines.is_empty() {
                ui.label(RichText::new("Todavía no cerró ningún mes.").small().color(theme::INK_SOFT));
            } else {
                egui::ScrollArea::vertical().max_height((ctx.screen_rect().height() - 52.0 - reserved_bottom - 230.0).max(80.0)).show(ui, |ui| {
                    egui::Grid::new("lineas").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                        for l in &last.lines {
                            ui.label(RichText::new(&l.concept).small());
                            let col = if l.amount < 0 { theme::BAD } else { theme::GOOD };
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(RichText::new(fmt_thousands(l.amount)).small().color(col)));
                            ui.end_row();
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Ingresos {}", fmt_thousands(last.income()))).small().color(theme::GOOD));
                    ui.label(RichText::new(format!("Gastos {}", fmt_thousands(last.expenses()))).small().color(theme::BAD));
                    let b = last.balance();
                    ui.label(RichText::new(format!("Balance {}{}", if b >= 0 { "+" } else { "" }, fmt_thousands(b))).small().strong());
                });
            }
        });
    });
    close
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
                    if ui.small_button("×").on_hover_text("Cerrar").clicked() {
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
/// Medidor de demanda residencial, comercial e industrial: tres barras que suben (verde) o
/// bajan (rojo) desde el centro.
pub fn demand_meter(ui: &mut egui::Ui, d: Demand) {
    let (rect, resp) = ui.allocate_exact_size(vec2(84.0, 36.0), egui::Sense::hover());
    let p = ui.painter();
    let mid = rect.top() + 13.0;
    for (k, (v, label)) in [(d.r, "R"), (d.c, "C"), (d.i, "I")].into_iter().enumerate() {
        let x = rect.left() + 6.0 + k as f32 * 27.0;
        let h = v.clamp(-1.0, 1.0) * 11.0;
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(x, mid - 11.0), egui::pos2(x + 16.0, mid + 11.0)), 2.0, theme::INK_SOFT.gamma_multiply(0.15));
        let bar = egui::Rect::from_two_pos(egui::pos2(x, mid), egui::pos2(x + 16.0, mid - h));
        p.rect_filled(bar, 1.0, if v >= 0.0 { theme::GOOD } else { theme::BAD });
        p.line_segment([egui::pos2(x - 1.0, mid), egui::pos2(x + 17.0, mid)], egui::Stroke::new(1.0_f32, theme::INK_SOFT));
        p.text(egui::pos2(x + 8.0, rect.bottom()), Align2::CENTER_BOTTOM, label, egui::FontId::proportional(11.0), theme::INK);
    }
    let pct = |v: f32| format!("{}{} %", if v >= 0.0 { "+" } else { "" }, (v * 100.0).round() as i32);
    resp.on_hover_text(format!("Demanda · residencial {} · comercial {} · industrial {}", pct(d.r), pct(d.c), pct(d.i)));
}

/// Aplica las acciones de la interfaz al juego (la hora vive en la sesión: se devuelve el
/// cambio para que el llamador la ajuste).
pub fn apply(acts: &[UiAction], game: &mut Game, list: &mut ListPanel) -> f32 {
    let mut hour_delta = 0.0;
    for a in acts {
        match a {
            UiAction::Tool(i) => game.set_tool(*i),
            UiAction::Kind(k) => game.kind = *k,
            UiAction::Zone(k) => {
                game.set_tool(4);
                game.zone_kind = *k;
            }
            UiAction::Density(d) => game.density = *d,
            UiAction::Brush(b) => game.brush = b.clamp(10.0, 150.0),
            UiAction::Speed(v) => game.speed = *v,
            UiAction::Hour(d) => hour_delta += d,
            UiAction::CloseInspect => game.inspected = None,
            UiAction::ToggleList => list.open = !list.open,
            UiAction::ToggleBudget | UiAction::Abrir(_) => {}
            UiAction::Servicio(a) => services::apply(*a, game),
            UiAction::Tax(i, d) => {
                if let Some(t) = Tax::ALL.get(*i) {
                    let e = &mut game.city.economy;
                    let now = e.tax(*t);
                    e.set_tax(*t, now + d);
                }
            }
            UiAction::ToggleItem(i) if list.ordinances => {
                if let Some(o) = ORDINANCES.get(*i) {
                    let c = &mut game.city;
                    match c.economy.toggle(o.id, &mut c.money) {
                        Ok(m) | Err(m) => game.say(m),
                    }
                }
            }
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
            let found = CATEGORIES.iter().flat_map(|c| cards(*c, &g, false, false)).find(|c| c.action == Some(UiAction::Tool(i)) || (i == 4 && c.action == Some(UiAction::Zone(ZoneKind::Residencial)))).expect("la herramienta tiene tarjeta");
            assert!(found.tip.contains(&format!("atajo: {}", i + 1)), "{}", found.tip);
            assert!(!found.name.starts_with(char::is_numeric), "el atajo no va en el botón");
        }
        // Antes de fundar, las calles están desactivadas con el motivo; la plaza no.
        let calles = cards(Category::Calles, &g, false, false);
        assert!(calles[0].disabled.is_none());
        assert_eq!(calles[1].disabled.as_deref(), Some("Falta la plaza"));
        assert_eq!(calles[4].disabled.as_deref(), Some(SOON), "lo que falta en el motor sale como próximamente");
        g.city.plaza = Some((Vec2::ZERO, 0.0));
        let calles = cards(Category::Calles, &g, false, false);
        assert_eq!(calles[0].disabled.as_deref(), Some("Ya fundada"));
        assert!(calles[1].disabled.is_none());
        g.city.money = 10;
        assert_eq!(cards(Category::Calles, &g, false, false)[1].disabled.as_deref(), Some("Sin fondos"));
        assert_eq!(options_for(Category::Calles), Options::RoadKind);
        assert_eq!(options_for(Category::Zonas), Options::Brush);
        assert_eq!(options_for(Category::Servicios), Options::Grupos);
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
        let ch = chips(&g);
        assert_eq!(ch.len(), 8);
        assert!(ch[0].2.contains("INUMET") && ch[0].2.contains("Trigo"), "{}", ch[0].2);
        assert!(ch[0].1.starts_with("21 °C"), "marzo, con la normal del INUMET: {}", ch[0].1);
        assert!(ch[0].2.starts_with("Otoño de marzo"), "{}", ch[0].2);
        assert!(ch[0].2.contains("Viento:"), "{}", ch[0].2);
        assert_eq!(ch[1].1, "200", "ladrillos de la fundación");
        assert_eq!(ch[3].1, "1,00", "precio normal del pan");
        assert!(ch[4].2.contains("vecinos"), "{}", ch[4].2);
        assert_eq!(month_balance(&g), None, "sin mes cerrado todavía");
    }

    #[test]
    fn escape_cierra_en_orden() {
        let mut g = Game::default();
        let mut ui = GameUi::default();
        ui.open = Some(Category::Calles);
        ui.list.open = true;
        g.inspected = Some(Vec2::ZERO);
        assert!(ui.close_one(&mut g) && !ui.list.open);
        assert!(ui.close_one(&mut g) && g.inspected.is_none());
        assert!(ui.close_one(&mut g) && ui.open.is_none());
        assert!(!ui.close_one(&mut g), "nada más que cerrar: Esc sale");
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
        // Con las ordenanzas de verdad: el botón aprueba en la economía y cobra el papeleo.
        let money = g.city.money;
        let mut o = ListPanel::ordinances(&g.city.economy, g.city.money, true);
        assert_eq!(o.items.len(), ORDINANCES.len());
        apply(&[UiAction::ToggleItem(1)], &mut g, &mut o);
        assert!(g.city.economy.is_on(ORDINANCES[1].id));
        assert_eq!(g.city.money, money - TOGGLE_COST);
        assert!(g.message.contains("aprueba"), "{}", g.message);
        let mut o = ListPanel::ordinances(&g.city.economy, 50, true);
        assert!(o.items[1].active);
        assert!(!o.items[0].enabled, "sin plata para el papeleo");
        // Impuestos: se mueven de a un punto y no pasan del tope.
        let before = g.city.economy.tax(Tax::ALL[0]);
        apply(&[UiAction::Tax(0, 1), UiAction::Tax(0, 1)], &mut g, &mut o);
        assert_eq!(g.city.economy.tax(Tax::ALL[0]), before + 2);
        apply(&vec![UiAction::Tax(0, 1); 40], &mut g, &mut o);
        assert_eq!(g.city.economy.tax(Tax::ALL[0]), TAX_MAX);
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
