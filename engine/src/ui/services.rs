//! Tarjetas y acciones de los servicios en la interfaz. El cajón de Servicios tiene pestañas:
//! agua, educación, salud, seguridad, saneamiento e iglesia; cada una con sus edificios, las
//! ordenanzas que le tocan y, desactivados, los de las épocas siguientes («Época N»).
//! Además: hornos, canteras, chacras y estancias (Producción); muelle, aduana y muralla
//! (Puerto y muralla); y las capas de datos (agua, serenos, fuego, educación, salud,
//! saneamiento y delito).

use super::icons::Icon;
use super::{Card, Category, UiAction};
use crate::city::economy::{ord_info, Ordinance, Res};
use crate::city::game::{fmt_thousands, Game, InspectKind, Tool};
use crate::city::services::barrio::{numero_epoca, FUTUROS};
use crate::city::services::{self, port, works, Grupo, Kind, Layer, Order, Placeable};

/// Lo que se pide desde las tarjetas y la ficha de los servicios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Place(Placeable),
    /// Muestra u oculta una capa de datos.
    Layer(Layer),
    Order(Order),
    /// Elige la pestaña del cajón de Servicios.
    Grupo(Grupo),
    /// Aprueba o deroga una ordenanza desde su tarjeta.
    Ordenanza(Ordinance),
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
        Kind::Escuela => Icon::Escuela,
        Kind::Convento => Icon::Iglesia,
        Kind::Botica => Icon::Botica,
        Kind::Medico | Kind::Hospital => Icon::Hospital,
        Kind::Alcalde => Icon::Alcalde,
        Kind::Carcel => Icon::Carcel,
        Kind::Basurero => Icon::Basura,
        Kind::EscuelaOficios => Icon::Produccion,
        Kind::EscuelaNautica => Icon::Puerto,
        Kind::Saladero => Icon::Saladero,
        Kind::Molino => Icon::Atahona,
        Kind::Astillero => Icon::Puerto,
        Kind::Imprenta => Icon::Escuela,
    }
}

/// Nombre corto para la tarjeta (el completo va en el globo de ayuda).
fn nombre_corto(k: Kind) -> &'static str {
    match k {
        Kind::Escuela => "Escuela",
        Kind::Convento => "Convento",
        Kind::Medico => "Médico",
        Kind::Hospital => "Hospital de caridad",
        Kind::Alcalde => "Alcalde de barrio",
        Kind::Carcel => "Cárcel del Cabildo",
        Kind::Basurero => "Basurero",
        Kind::EscuelaOficios => "Escuela de oficios",
        Kind::Astillero => "Astillero",
        _ => services::info(k).name,
    }
}

fn ordenanza_corta(o: Ordinance) -> &'static str {
    match o {
        Ordinance::EscuelasPrimerasLetras => "Maestro del cabildo",
        Ordinance::HospitalCaridad => "Asistencia a pobres",
        Ordinance::RecoleccionBasura => "Recolección",
        Ordinance::PozosNegrosLejos => "Pozos negros lejos",
        Ordinance::Limpieza => "Limpieza de calles",
        Ordinance::TurnosAgua => "Turnos de agua",
        _ => ord_info(o).name,
    }
}

fn grupo_icon(g: Grupo) -> Icon {
    match g {
        Grupo::Agua => Icon::Agua,
        Grupo::Educacion => Icon::Escuela,
        Grupo::Salud => Icon::Hospital,
        Grupo::Seguridad => Icon::Sereno,
        Grupo::Saneamiento => Icon::Basura,
        Grupo::Otros => Icon::Iglesia,
    }
}

/// Tarjeta de una ordenanza que se aprueba o deroga desde el cajón.
fn ordinance_card(game: &Game, o: Ordinance, icon: Icon) -> Card {
    let i = ord_info(o);
    let on = game.city.economy.is_on(o);
    Card {
        icon,
        name: ordenanza_corta(o),
        cost: if on { "vigente · derogar".into() } else { "ordenanza".into() },
        tip: format!("Ordenanza «{}»: {} Aprobar o derogar cuesta {} pesos de papeleo.", i.name, i.desc, crate::city::economy::TOGGLE_COST),
        action: Some(UiAction::Servicio(Act::Ordenanza(o))),
        disabled: if game.city.plaza.is_none() { Some("Falta la plaza".into()) } else if game.city.money < crate::city::economy::TOGGLE_COST { Some("Sin fondos".into()) } else { None },
        selected: on,
    }
}

/// Tarjetas de los servicios de las épocas siguientes de un grupo (desactivadas).
fn future_cards(g: Grupo) -> Vec<Card> {
    FUTUROS
        .iter()
        .filter(|f| f.grupo == g)
        .map(|f| Card {
            icon: grupo_icon(g),
            name: f.nombre,
            cost: String::new(),
            tip: format!("{} Llega con la época «{}»{}.", f.efecto, f.desde.nombre(), f.fecha.map_or(String::new(), |y| format!(" (en Montevideo, desde {y})"))),
            action: None,
            disabled: Some(format!("Época {}", numero_epoca(f.desde))),
            selected: false,
        })
        .collect()
}

/// Tarjetas de una pestaña del cajón de Servicios.
pub fn group_cards(g: Grupo, game: &Game) -> Vec<Card> {
    let mut v = match g {
        Grupo::Agua => vec![place_card(game, Kind::Pozo), place_card(game, Kind::Aljibe), place_card(game, Kind::Aguadores), ordinance_card(game, Ordinance::TurnosAgua, Icon::Agua)],
        Grupo::Educacion => vec![place_card(game, Kind::Escuela), place_card(game, Kind::Convento), ordinance_card(game, Ordinance::EscuelasPrimerasLetras, Icon::Escuela), place_card(game, Kind::EscuelaOficios), place_card(game, Kind::EscuelaNautica)],
        Grupo::Salud => vec![place_card(game, Kind::Botica), place_card(game, Kind::Medico), place_card(game, Kind::Hospital), place_card(game, Kind::Lazareto), ordinance_card(game, Ordinance::HospitalCaridad, Icon::Hospital)],
        Grupo::Seguridad => vec![place_card(game, Kind::Serenos), place_card(game, Kind::Alcalde), place_card(game, Kind::Carcel)],
        Grupo::Saneamiento => vec![
            place_card(game, Kind::Basurero),
            ordinance_card(game, Ordinance::RecoleccionBasura, Icon::Basura),
            ordinance_card(game, Ordinance::PozosNegrosLejos, Icon::Aljibe),
            ordinance_card(game, Ordinance::Limpieza, Icon::Basura),
        ],
        Grupo::Otros => vec![church_card(game)],
    };
    v.extend(future_cards(g));
    v
}

/// Tarjeta para colocar un edificio (para los cajones armados fuera de este módulo).
pub fn place_card_pub(game: &Game, k: Kind) -> Card {
    place_card(game, k)
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
    } else if !game.city.desarrollo.habilitado(k) {
        Some("Falta tecnología")
    } else if matches!(k, Kind::Imprenta | Kind::EscuelaNautica) && game.city.services.count(k) > 0 {
        Some("Ya hay una")
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
    } else if matches!(k, Kind::Convento | Kind::Carcel) && game.city.services.count(k) > 0 {
        Some("Ya hay uno")
    } else {
        None
    };
    let p = Placeable::Building(k);
    Card {
        icon: icon_of(k),
        name: nombre_corto(k),
        cost,
        tip: format!(
            "{}: {}{}{} Mantenimiento: {} pesos al mes.",
            i.name,
            i.desc,
            if i.radius > 0.0 { format!(" Alcance: {} m.", i.radius as i32) } else { String::new() },
            if i.plazas > 0 { format!(" Capacidad: {}.", fmt_thousands(i.plazas as i64)) } else { String::new() },
            i.upkeep
        ),
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
        Category::Servicios => group_cards(game.grupo, game),
        Category::Produccion => vec![
            place_card(game, Kind::Chacra),
            place_card(game, Kind::Estancia),
            place_card(game, Kind::Horno),
            place_card(game, Kind::Cantera),
            place_card(game, Kind::Saladero),
            place_card(game, Kind::Molino),
            place_card(game, Kind::Astillero),
            place_card(game, Kind::Imprenta),
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
            layer_card(game, Layer::Educacion, Icon::Escuela, "Educación", "Casas con lugar en una escuela (verde) o sin escuela cerca (rojo) y el alcance de escuelas y conventos"),
            layer_card(game, Layer::Salud, Icon::Hospital, "Salud", "Quién tiene botica, médico u hospital cerca"),
            layer_card(game, Layer::Saneamiento, Icon::Basura, "Saneamiento", "Insalubridad de cada casa (agua mala, pozos negros, basura), los basurales y los pozos sucios"),
            layer_card(game, Layer::Delito, Icon::Delito, "Delito", "Mapa de calor del riesgo de robos y asaltos, y la ronda de serenos, alcaldes y guardia"),
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
        Act::Grupo(g) => game.grupo = g,
        Act::Ordenanza(o) => {
            let c = &mut game.city;
            match c.economy.toggle(o, &mut c.money) {
                Ok(m) | Err(m) => game.say(m),
            }
            game.city.refresh_barrio();
        }
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
        // Pestañas: educación con la escuela, el convento, la ordenanza y lo que viene.
        apply(Act::Grupo(Grupo::Educacion), &mut g);
        let ed = cards(Category::Servicios, &g).unwrap();
        assert!(ed.iter().any(|c| c.name == "Escuela" && c.disabled.is_none()));
        assert!(ed.iter().any(|c| c.name == "Escuela pública" && c.disabled.as_deref() == Some("Época 3")), "las de otras épocas, desactivadas");
        apply(Act::Grupo(Grupo::Saneamiento), &mut g);
        let san = cards(Category::Servicios, &g).unwrap();
        let rec = san.iter().find(|c| c.name == "Recolección").unwrap().clone();
        assert!(!rec.selected);
        apply(Act::Ordenanza(Ordinance::RecoleccionBasura), &mut g);
        assert!(g.city.economy.is_on(Ordinance::RecoleccionBasura), "la tarjeta aprueba la ordenanza");
        for gr in Grupo::PESTAÑAS {
            apply(Act::Grupo(gr), &mut g);
            assert!(!cards(Category::Servicios, &g).unwrap().is_empty(), "{gr:?}");
        }
        apply(Act::Grupo(Grupo::Agua), &mut g);
        let capas = cards(Category::Capas, &g).unwrap();
        assert!(capas.iter().any(|c| c.name == "Delito") && capas.iter().any(|c| c.name == "Saneamiento"));
        assert!(pozo.disabled.is_none(), "{:?}", pozo.disabled);
        g.grupo = Grupo::Otros;
        let ig = cards(Category::Servicios, &g).unwrap();
        assert_eq!(ig.iter().find(|c| c.name == "Iglesia matriz").unwrap().disabled.as_deref(), Some("Falta gente"));
        g.grupo = Grupo::Agua;
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
