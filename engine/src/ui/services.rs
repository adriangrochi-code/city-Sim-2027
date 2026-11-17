//! Tarjetas y acciones de los servicios en la interfaz: agua, serenos, lazareto e iglesia
//! (Servicios); hornos, canteras, chacras y estancias (Producción); muelle, aduana y muralla
//! (Puerto y muralla); y las capas de datos de agua, serenos y fuego.

use super::icons::Icon;
use super::{Card, Category, UiAction};
use crate::city::economy::Res;
use crate::city::game::{fmt_thousands, Game, InspectKind, Tool};
use crate::city::services::{self, port, works, Kind, Layer, Order, Placeable};

/// Lo que se pide desde las tarjetas y la ficha de los servicios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Place(Placeable),
    /// Muestra u oculta una capa de datos.
    Layer(Layer),
    Order(Order),
}

fn icon_of(k: Kind) -> Icon {
    match k {
        Kind::Pozo | Kind::Aljibe => Icon::Aljibe,
        Kind::Aguadores => Icon::Agua,
        Kind::Serenos => Icon::Sereno,
        Kind::Lazareto => Icon::Lazareto,
        Kind::Chacra => Icon::Quintas,
        Kind::Estancia => Icon::Hacienda,
        Kind::Horno => Icon::Ladrillo,
        Kind::Cantera => Icon::Cantera,
        Kind::Aduana => Icon::Aduana,
    }
}

fn place_card(game: &Game, k: Kind) -> Card {
    let i = services::info(k);
    let eco = &game.city.economy;
    let mut cost = format!("{} pesos", fmt_thousands(i.cost));
    if i.ladrillos > 0.0 {
        cost += &format!(" · {} lad.", i.ladrillos as i32);
    }
    let disabled = if game.city.plaza.is_none() {
        Some("Falta la plaza")
    } else if game.city.money < i.cost {
        Some("Sin fondos")
    } else if eco.stock(Res::Ladrillos).amount < i.ladrillos {
        Some("Faltan ladrillos")
    } else if eco.stock(Res::Piedra).amount < i.piedra {
        Some("Falta piedra")
    } else if k == Kind::Aduana && game.city.services.port.pier.is_none() {
        Some("Falta el muelle")
    } else if k == Kind::Aduana && game.city.services.count(Kind::Aduana) > 0 {
        Some("Ya construida")
    } else {
        None
    };
    let p = Placeable::Building(k);
    Card {
        icon: icon_of(k),
        name: i.name,
        cost,
        tip: format!("{}{}", i.desc, if i.radius > 0.0 { format!(" Alcance: {} m.", i.radius as i32) } else { String::new() }),
        action: Some(UiAction::Servicio(Act::Place(p))),
        disabled: disabled.map(str::to_string),
        selected: game.tool == Tool::Service && game.service == p,
    }
}

fn church_card(game: &Game) -> Card {
    let ch = &game.city.services.church;
    let (cost, disabled) = match ch.next() {
        None => ("terminada".to_string(), Some("Terminada".to_string())),
        Some(n) if ch.building => (format!("en obra: {}", n.name.to_lowercase()), Some(format!("Faltan {} meses", ch.months_left))),
        Some(n) => {
            let short = game.city.church_problem().map(|p| if p.contains("habitantes") { "Falta gente".to_string() } else if p.contains("pesos") { "Sin fondos".to_string() } else { p });
            (format!("{}: {} pesos", n.name, fmt_thousands(n.cost)), short)
        }
    };
    let phases: Vec<&str> = works::CHURCH.iter().map(|p| p.name).collect();
    Card {
        icon: Icon::Iglesia,
        name: "Iglesia matriz",
        cost,
        tip: format!("El gran proyecto de la época, por fases: {}. Cada fase consume ladrillos y piedra mes a mes y atrae más pobladores. Hace falta una ciudad de {} habitantes.", phases.join(", "), works::CHURCH_MIN_POP),
        action: Some(UiAction::Servicio(Act::Order(Order::FundChurch))),
        disabled,
        selected: false,
    }
}

fn layer_card(game: &Game, l: Layer, icon: Icon, name: &'static str, tip: &str) -> Card {
    Card { icon, name, cost: if game.layer == Some(l) { "visible".into() } else { "gratis".into() }, tip: tip.into(), action: Some(UiAction::Servicio(Act::Layer(l))), disabled: None, selected: game.layer == Some(l) }
}

fn soon(icon: Icon, name: &'static str, tip: &str) -> Card {
    Card { icon, name, cost: String::new(), tip: format!("{tip} (todavía no está en el motor)"), action: None, disabled: Some(super::SOON.to_string()), selected: false }
}

/// Tarjetas de los servicios para una categoría (`None` si la categoría no es de servicios).
pub fn cards(cat: Category, game: &Game) -> Option<Vec<Card>> {
    Some(match cat {
        Category::Servicios => vec![
            place_card(game, Kind::Pozo),
            place_card(game, Kind::Aljibe),
            place_card(game, Kind::Aguadores),
            place_card(game, Kind::Serenos),
            place_card(game, Kind::Lazareto),
            church_card(game),
        ],
        Category::Produccion => vec![
            place_card(game, Kind::Chacra),
            place_card(game, Kind::Estancia),
            place_card(game, Kind::Horno),
            place_card(game, Kind::Cantera),
            soon(Icon::Atahona, "Atahona", "Muele el trigo: harina para el pan"),
            soon(Icon::Saladero, "Saladero", "Carne salada y cueros"),
        ],
        Category::Puerto => {
            let has = game.city.services.port.pier.is_some();
            let pier = Card {
                icon: Icon::Puerto,
                name: "Muelle",
                cost: format!("{} pesos", fmt_thousands(port::PIER_COST)),
                tip: format!(
                    "Muelle de madera con barraca de cueros: los barcos atracan, traen pobladores y mercaderías y se llevan los cueros. Sin muelle, desembarcan en la playa y no hay comercio.{}",
                    game.city.services.port.event_name().map_or(String::new(), |e| format!(" Ahora: {e}."))
                ),
                action: Some(UiAction::Servicio(Act::Place(Placeable::Muelle))),
                disabled: if has { Some("Ya construido".into()) } else if game.city.money < port::PIER_COST { Some("Sin fondos".into()) } else if game.city.plaza.is_none() { Some("Falta la plaza".into()) } else { None },
                selected: game.tool == Tool::Service && game.service == Placeable::Muelle,
            };
            let wall = Card {
                icon: Icon::Muralla,
                name: "Muralla",
                cost: match game.city.services.walls.total_length() {
                    l if l > 0.0 => format!("{} de {} m", game.city.services.walls.tramos.iter().map(|t| t.built).sum::<f32>() as i32, l as i32),
                    _ => format!("{} pesos/m", works::WALL_COST_PER_M),
                },
                tip: format!("Por tramos: se paga al trazarlo y se levanta con {} de piedra por metro, mes a mes. No es defensiva: marca la ciudad y sus portones cobran derechos.", works::WALL_STONE_PER_M as i32),
                action: Some(UiAction::Servicio(Act::Place(Placeable::Muralla))),
                disabled: if game.city.plaza.is_none() { Some("Falta la plaza".into()) } else { None },
                selected: game.tool == Tool::Service && game.service == Placeable::Muralla,
            };
            vec![pier, place_card(game, Kind::Aduana), wall]
        }
        Category::Capas => vec![
            super::Card::tool_pub(game, 7, Icon::Lupa, "gratis".into(), "Ver la ficha de un solar, una calle, un servicio o el terreno"),
            layer_card(game, Layer::Agua, Icon::Agua, "Agua", "Qué casas tienen agua (buena, regular o mala) y el alcance de pozos, aljibes y aguadores"),
            layer_card(game, Layer::Serenos, Icon::Sereno, "Serenos", "Calles con faroles y vigiladas de noche"),
            layer_card(game, Layer::Fuego, Icon::Bomba, "Fuego", "Riesgo de incendio por el techo y el agua cercana; lo que arde y las ruinas"),
            soon(Icon::Valor, "Valor del suelo", "Mapa de calor del valor"),
        ],
        _ => return None,
    })
}

/// Botones de la ficha de un servicio o de la iglesia: (texto, ayuda, acción).
pub fn inspect_buttons(kind: InspectKind) -> Vec<(&'static str, &'static str, UiAction)> {
    match kind {
        InspectKind::Service(id) => vec![("Demoler", "Derribar este servicio (20 pesos; no se devuelve lo gastado)", UiAction::Servicio(Act::Order(Order::Demolish(id))))],
        InspectKind::Church => vec![("Financiar la fase", "Pagar y empezar la fase siguiente de la iglesia", UiAction::Servicio(Act::Order(Order::FundChurch)))],
        _ => Vec::new(),
    }
}

/// Aplica una acción de los servicios al juego.
pub fn apply(a: Act, game: &mut Game) {
    match a {
        Act::Place(p) => game.set_service(p),
        Act::Layer(l) => game.layer = if game.layer == Some(l) { None } else { Some(l) },
        Act::Order(o) => game.orders.push(o),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainGen;
    use glam::Vec2;

    #[test]
    fn tarjetas_de_servicios_y_capas() {
        let t = TerrainGen::new(1726);
        let mut g = Game::default();
        let sv = cards(Category::Servicios, &g).unwrap();
        assert!(sv.iter().all(|c| c.disabled.is_some()), "sin plaza no se construye nada");
        g.city.found_plaza(&t, Vec2::new(13500.0, 16300.0), 0.0).unwrap();
        g.city.money = 100_000;
        let sv = cards(Category::Servicios, &g).unwrap();
        let pozo = sv.iter().find(|c| c.name == "Pozo de balde").unwrap();
        assert!(pozo.disabled.is_none(), "{:?}", pozo.disabled);
        assert_eq!(sv.iter().find(|c| c.name == "Iglesia matriz").unwrap().disabled.as_deref(), Some("Falta gente"));
        // Elegir la tarjeta elige la herramienta; la capa se prende y se apaga.
        apply(Act::Place(Placeable::Building(Kind::Pozo)), &mut g);
        assert_eq!(g.tool, Tool::Service);
        assert!(cards(Category::Servicios, &g).unwrap().iter().any(|c| c.selected && c.name == "Pozo de balde"));
        apply(Act::Layer(Layer::Agua), &mut g);
        assert_eq!(g.layer, Some(Layer::Agua));
        apply(Act::Layer(Layer::Agua), &mut g);
        assert_eq!(g.layer, None);
        let puerto = cards(Category::Puerto, &g).unwrap();
        assert_eq!(puerto.iter().find(|c| c.name == "Aduana").unwrap().disabled.as_deref(), Some("Falta el muelle"));
        assert!(cards(Category::Calles, &g).is_none());
        // El clic coloca el pozo junto a la calle y la ficha lo muestra con su botón.
        let p = Vec2::new(13500.0, 16300.0) + Vec2::new(0.0, 70.0);
        let e = g.press(&t, p);
        assert!(e.is_some(), "{}", g.message);
        let id = *g.city.services.buildings.keys().next().unwrap();
        g.inspected = Some(g.city.services.buildings[&id].c);
        let info = g.inspect_info().unwrap();
        assert_eq!(info.kind, InspectKind::Service(id));
        assert!(info.lines.iter().any(|l| l.starts_with("Abastece")));
        assert_eq!(inspect_buttons(info.kind).len(), 1);
        // Demoler desde la ficha.
        apply(Act::Order(Order::Demolish(id)), &mut g);
        assert!(g.update(&t, 0.0).is_some());
        assert!(g.city.services.buildings.is_empty());
    }
}
