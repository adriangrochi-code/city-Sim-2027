//! Las épocas en la interfaz: la ficha de la época en la barra superior, el panel con los
//! requisitos y la decisión de la muralla, la pantalla de resumen al cambiar de época, la
//! tarjeta del ensanche y los botones de rehabilitación en la ficha de una casa.

use super::icons::{self, Icon};
use super::{theme, Card, UiAction};
use crate::city::epocas::rehabilitar::Reforma;
use crate::city::epocas::Muralla;
use crate::city::game::{fmt_thousands, Game, Tool};
use crate::city::poblacion::Epoca;
use crate::city::services::Order;
use egui::{vec2, Align2, RichText};

/// Lo que se pide desde la interfaz de las épocas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Pasar(Muralla),
    Ensanche,
    Reforma(u32, Reforma),
    ResumenVisto,
}

pub fn apply(a: Act, game: &mut Game) {
    match a {
        Act::Pasar(m) => game.orders.push(Order::PasarEpoca(m)),
        Act::Ensanche => {
            if game.city.en_epoca(Epoca::CiudadNueva) {
                game.tool = Tool::Ensanche;
                game.stage = 0;
            } else {
                game.say("El ensanche llega con la época «Ciudad Nueva»");
            }
        }
        Act::Reforma(id, r) => game.orders.push(Order::Reforma(id, r)),
        Act::ResumenVisto => game.city.epoca.mostrar_resumen = false,
    }
}

/// Tarjeta del ensanche (cajón de Calles).
pub fn card_ensanche(game: &Game) -> Card {
    let ok = game.city.en_epoca(Epoca::CiudadNueva);
    Card {
        icon: Icon::Damero,
        name: "Ensanche",
        cost: if ok { "calles, bulevar y plazas".into() } else { String::new() },
        tip: "Ensanche planificado sobre el ejido, fuera de la ciudad vieja: damero de calles empedradas, un bulevar arbolado hacia el interior y una plaza cada tres manzanas (como la Ciudad Nueva de José María Reyes, 1829).".into(),
        action: Some(UiAction::Epoca(Act::Ensanche)),
        disabled: if !ok { Some("Época 2".into()) } else if game.city.plaza.is_none() { Some("Falta la plaza".into()) } else { None },
        selected: game.tool == Tool::Ensanche,
    }
}

/// Ficha de la época en la barra superior (resaltada si ya se puede pasar).
pub fn chip(ui: &mut egui::Ui, game: &Game, acts: &mut Vec<UiAction>) {
    let c = &game.city;
    let listo = c.puede_pasar_epoca();
    let txt = RichText::new(c.epoca.actual.nombre()).small();
    let txt = if listo { txt.color(theme::GOOD).strong() } else { txt.color(theme::INK_SOFT) };
    let tip = if listo { "¡Se puede pasar a la época siguiente! Clic: decidir qué hacer con la muralla".to_string() } else { "Clic: la época, sus requisitos y lo que trae la siguiente".to_string() };
    if ui.add(egui::Label::new(txt).sense(egui::Sense::click())).on_hover_text(tip).clicked() {
        acts.push(UiAction::Abrir(super::Detail::Epoca(0)));
    }
}

/// Panel de la época (pestaña 0: la época y el paso a la siguiente; 1: resumen de la época
/// terminada). Devuelve `Some(nuevo)` si hay que cambiar o cerrar el panel.
pub fn panel(ctx: &egui::Context, game: &Game, tab: u8, reserved_bottom: f32, acts: &mut Vec<UiAction>) -> Option<Option<super::Detail>> {
    let c = &game.city;
    let mut nav = None;
    egui::Area::new("epoca".into()).anchor(Align2::LEFT_TOP, vec2(10.0, 52.0)).show(ctx, |ui| {
        theme::paper_frame().show(ui, |ui| {
            ui.set_width(400.0);
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Valor, 22.0, theme::INK);
                ui.heading(if tab == 1 { "Fin de una época" } else { "La época" });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Cerrar").clicked() {
                        nav = Some(None);
                        if tab == 1 {
                            acts.push(UiAction::Epoca(Act::ResumenVisto));
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.selectable_label(tab == 0, "Época actual").clicked() {
                    nav = Some(Some(super::Detail::Epoca(0)));
                }
                if !c.epoca.resumenes.is_empty() && ui.selectable_label(tab == 1, "Resumen").clicked() {
                    nav = Some(Some(super::Detail::Epoca(1)));
                }
            });
            let max_h = (ctx.screen_rect().height() - 52.0 - reserved_bottom - 60.0).max(160.0);
            egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
                if tab == 1 {
                    resumen(ui, game, acts, &mut nav);
                } else {
                    actual(ui, game, acts);
                }
            });
        });
    });
    nav
}

fn actual(ui: &mut egui::Ui, game: &Game, acts: &mut Vec<UiAction>) {
    let c = &game.city;
    let e = c.epoca.actual;
    ui.label(RichText::new(e.nombre()).size(theme::BODY * 1.3).strong());
    let desde = crate::city::START_YEAR + c.epoca.desde_mes / 12;
    ui.label(RichText::new(format!("Desde {desde}{}", if c.poblacion.fiel_historia { " · modo fiel a la historia" } else { " · historia alternativa" })).small().color(theme::INK_SOFT));
    ui.add_space(4.0);
    match e {
        Epoca::Colonial => {
            ui.label(RichText::new("La plaza fuerte crece apretada dentro de la muralla. Para abrir la ciudad al país hacen falta:").small());
        }
        _ => {
            ui.label(RichText::new("La muralla cae y la ciudad respira: ensanche sobre el ejido, gas, teatro, agua corriente y colectores. Las casas viejas se modernizan de a poco.").small());
            ui.add_space(4.0);
            super::section(ui, "Lo de esta época");
            egui::Grid::new("ep2").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                super::kv(ui, "Ensanches trazados", format!("{} ({} plazas, {} árboles)", c.epoca.ensanche.trazados.len(), c.epoca.ensanche.plazas.len(), c.epoca.ensanche.arboles.len()), theme::INK);
                super::kv(ui, "Faroles de gas", format!("{} (carbón: {} pesos/mes)", c.epoca.faroles_gas.len(), c.costo_carbon()), theme::INK);
                super::kv(ui, "Faroles de sebo", format!("{}", c.services.lamps.len()), theme::INK);
                super::kv(ui, "Solares con agua corriente", format!("{}", c.epoca.agua_red.len()), theme::INK);
                super::kv(ui, "Solares con colector", format!("{}", c.epoca.cloaca.len()), theme::INK);
                super::kv(ui, "Casas modernizadas", format!("{}", c.epoca.rehabilitadas), theme::INK);
                let (viejas, nuevas) = c.lots.lots.values().filter(|l| l.level > 0).filter_map(|l| l.obra).fold((0, 0), |(v, n), o| if o.contexto.epoca >= 1 || o.contexto.reforma > 0 { (v, n + 1) } else { (v + 1, n) });
                super::kv(ui, "Casas coloniales / de la época", format!("{viejas} / {nuevas}"), theme::INK);
            });
            if let Some(m) = c.epoca.muralla {
                ui.label(RichText::new(format!("La muralla: {}.", m.nombre().to_lowercase())).small().color(theme::INK_SOFT));
            }
        }
    }
    if c.epoca_siguiente().is_none() {
        ui.add_space(4.0);
        ui.label(RichText::new("La época siguiente (inmigración y Belle Époque) todavía no se juega en el motor.").small().color(theme::INK_SOFT));
        return;
    }
    super::section(ui, "Requisitos para la Ciudad Nueva");
    for r in c.requisitos_epoca() {
        ui.label(RichText::new(format!("{} {}", if r.cumplido { "✔" } else { "·" }, r.texto)).small().color(if r.cumplido { theme::GOOD } else { theme::INK }));
    }
    ui.add_space(4.0);
    super::section(ui, "La decisión de la muralla");
    let listo = c.puede_pasar_epoca();
    for m in Muralla::ALL {
        ui.horizontal_wrapped(|ui| {
            let b = ui.add_enabled(listo, egui::Button::new(RichText::new(m.nombre()).small()));
            if b.on_hover_text(m.desc()).on_disabled_hover_text("Faltan requisitos").clicked() {
                acts.push(UiAction::Epoca(Act::Pasar(m)));
            }
        });
    }
    ui.label(RichText::new("Al pasar: tablas de llegadas de la época 2 (franceses, vascos, italianos), tecnologías de la época, casas neoclásicas, ensanche, usina de gas, teatro, agua corriente, colectores y la ordenanza de fachadas.").small().color(theme::INK_SOFT));
}

fn resumen(ui: &mut egui::Ui, game: &Game, acts: &mut Vec<UiAction>, nav: &mut Option<Option<super::Detail>>) {
    let c = &game.city;
    for r in c.epoca.resumenes.iter().rev() {
        ui.label(RichText::new(format!("{} ({}–{})", r.epoca.nombre(), r.desde, r.hasta)).size(theme::BODY * 1.15).strong());
        for l in &r.lineas {
            ui.label(RichText::new(format!("· {l}")).small());
        }
        ui.add_space(6.0);
    }
    ui.label(RichText::new(format!("Comienza «{}». Tesoro: {} pesos.", c.epoca.actual.nombre(), fmt_thousands(c.money))).small().color(theme::INK_SOFT));
    if c.epoca.mostrar_resumen && ui.button(RichText::new("Seguir").small()).clicked() {
        acts.push(UiAction::Epoca(Act::ResumenVisto));
        *nav = Some(Some(super::Detail::Epoca(0)));
    }
}

/// Botones de rehabilitación en la ficha de una casa: (texto, ayuda, acción).
pub fn botones_reforma(game: &Game, lot: u32) -> Vec<(String, String, UiAction)> {
    let c = &game.city;
    if !c.en_epoca(Epoca::CiudadNueva) {
        return Vec::new();
    }
    Reforma::ALL
        .iter()
        .filter_map(|&r| {
            let costo = c.reforma_posible(lot, r).ok()?;
            Some((format!("{} ({} $)", r.nombre(), fmt_thousands(costo)), r.desc().to_string(), UiAction::Epoca(Act::Reforma(lot, r))))
        })
        .collect()
}
