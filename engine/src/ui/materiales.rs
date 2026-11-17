//! Ficha de la barra y panel de los materiales de construcción: existencias, precio, lo que
//! produce el lugar, lo que trae el puerto, lo que piden las obras y lo que se está
//! sustituyendo (ver `city::materiales`).

use super::icons::Icon;
use super::{bars, fmt_dec, kv, section, theme};
use crate::city::game::{fmt_thousands, Game};
use crate::city::materiales::{Material, Pared, Techo};
use egui::{vec2, RichText};

/// Materiales que se muestran (el barro no se cuenta: está en todos lados).
pub const MOSTRADOS: [Material; 11] = [Material::Madera, Material::Cal, Material::Teja, Material::Ladrillo, Material::Piedra, Material::Adobe, Material::Paja, Material::Arena, Material::Cuero, Material::Hierro, Material::Vidrio];

/// El material que más falta ahora (el que se está sustituyendo con más obras).
pub fn peor_falta(game: &Game) -> Option<Material> {
    let m = &game.city.materiales;
    MOSTRADOS.into_iter().filter(|x| m.ex[x.idx()].sustituciones > 0 || m.avisadas & (1 << x.idx()) != 0).max_by_key(|x| (m.ex[x.idx()].sustituciones, std::cmp::Reverse(x.idx())))
}

/// Nombre corto para la ficha de la barra.
fn corto(m: Material) -> &'static str {
    match m {
        Material::Ladrillo => "lad.",
        Material::Madera => "mad.",
        Material::Piedra => "pie.",
        Material::Hierro => "hie.",
        Material::Vidrio => "vid.",
        o => o.nombre(),
    }
}

/// Ficha de la barra superior: qué falta (o que todo está a mano) y las existencias.
pub fn chip(game: &Game) -> (Icon, String, String) {
    let c = &game.city;
    // Corto: la barra ya está llena (el detalle va en el globo y en el panel).
    let valor = match peor_falta(game) {
        Some(m) => format!("sin {}", corto(m)),
        None => "al día".into(),
    };
    let lista = MOSTRADOS.iter().map(|&m| format!("{} {}", m.nombre(), fmt_thousands(c.hay_material(m).round() as i64))).collect::<Vec<_>>().join(" · ");
    let falta = peor_falta(game).map_or(String::new(), |m| format!("{}. ", m.aviso_falta()));
    (Icon::Materiales, valor, format!("{falta}Materiales de construcción: {lista}. Las obras usan lo que hay: si falta lo mejor, se sustituye."))
}

/// Contenido del panel de materiales.
pub fn panel(ui: &mut egui::Ui, game: &Game) {
    let c = &game.city;
    let m = &c.materiales;
    let ent = c.entorno_materiales();
    ui.label(RichText::new("Cada edificio se levanta con lo que hay. Si falta lo mejor, se usa lo que sigue; en el peor caso, barro y paja.").small().color(theme::INK_SOFT));
    // Lo que falta ahora (lo sustituido el último mes y lo que la gaceta ya avisó).
    let faltas: Vec<Material> = MOSTRADOS.into_iter().filter(|x| m.ex[x.idx()].sustituciones > 0 || m.avisadas & (1 << x.idx()) != 0).collect();
    if faltas.is_empty() {
        ui.label(RichText::new("No falta nada: las obras tienen lo que piden.").small().color(theme::GOOD));
    }
    for f in faltas {
        ui.label(RichText::new(format!("• {} ({} en depósito)", f.aviso_falta(), fmt_thousands(c.hay_material(f).round() as i64))).small().color(theme::BAD));
    }
    let (n, s) = m.obras_ultimo;
    let (arr, caidas) = m.arreglos_ultimo;
    ui.label(RichText::new(format!("Último mes: {n} obras ({s} con sustitutos), {arr} arreglos, {caidas} derrumbes.")).small().color(theme::INK_SOFT));
    section(ui, "Con qué está hecha la ciudad");
    let mut paredes = [0usize; 5];
    let mut techos = [0usize; 5];
    let mut humedas = 0;
    let mut total = 0;
    for l in c.lots.lots.values() {
        if let Some(o) = c.obra_de(l) {
            paredes[Pared::TODAS.iter().position(|&p| p == o.paredes).unwrap()] += 1;
            techos[Techo::TODOS.iter().position(|&t| t == o.techo).unwrap()] += 1;
            humedas += (o.humedad() >= 0.3) as usize;
            total += 1;
        }
    }
    egui::Grid::new("ciudad_mat").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        let pct = |x: usize| if total == 0 { 0 } else { (x * 100 + total / 2) / total };
        kv(ui, "Paredes", Pared::TODAS.iter().zip(paredes).filter(|x| x.1 > 0).map(|(p, k)| format!("{} {} %", p.nombre(), pct(k))).collect::<Vec<_>>().join(" · "), theme::INK);
        kv(ui, "Techos", Techo::TODOS.iter().zip(techos).filter(|x| x.1 > 0).map(|(t, k)| format!("{} {} %", t.nombre(), pct(k))).collect::<Vec<_>>().join(" · "), theme::INK);
        kv(ui, "Casas húmedas", format!("{} %", pct(humedas)), if pct(humedas) > 40 { theme::BAD } else { theme::INK });
    });
    section(ui, "Existencias y el último mes");
    egui::Grid::new("materiales").num_columns(7).spacing(vec2(8.0, 2.0)).striped(true).show(ui, |ui| {
        for h in ["", "Hay", "Precio", "Lugar", "Puerto", "Pedido", "Faltó"] {
            ui.label(RichText::new(h).small().strong());
        }
        ui.end_row();
        for mat in MOSTRADOS {
            let ex = &m.ex[mat.idx()];
            let (producido, importado) = match mat.de_la_economia() {
                Some(r) => (c.economy.stock(r).produced, c.economy.stock(r).imported),
                None => (ex.producido, ex.importado),
            };
            let precio = c.precio_material(mat);
            let falta = ex.sustituciones > 0;
            ui.label(RichText::new(crate::ui::capitalize(mat.nombre())).small().color(if falta { theme::BAD } else { theme::INK })).on_hover_text(format!("{} Precio de referencia: {} pesos la unidad.{}", mat.datos().origen, fmt_dec(mat.datos().precio, 2), mat.datos().importa.map_or(String::new(), |o| format!(" Se importa de: {o}."))));
            ui.label(RichText::new(fmt_thousands(c.hay_material(mat).round() as i64)).small());
            ui.label(RichText::new(fmt_dec(precio, 2)).small().color(if precio > 1.4 { theme::BAD } else if precio < 0.9 { theme::GOOD } else { theme::INK }));
            ui.label(RichText::new(format!("+{}", producido.round() as i64)).small().color(theme::GOOD));
            ui.label(RichText::new(if importado > 0.5 { format!("+{}", importado.round() as i64) } else { "—".into() }).small());
            ui.label(RichText::new(ex.pedido.round().to_string()).small());
            ui.label(RichText::new(if ex.faltante > 0.5 { format!("{} ({} obras)", ex.faltante.round() as i64, ex.sustituciones) } else { "—".into() }).small().color(if falta { theme::BAD } else { theme::INK_SOFT }));
            ui.end_row();
        }
    });
    section(ui, "Lo que da el lugar por mes");
    use crate::city::materiales::FuenteLocal;
    ui.label(RichText::new(format!(
        "Adobe {} (se seca al sol: este mes al {} %) · paja {} · arena {} · madera del monte {} · tejas de los hornos {}. La cal, el hierro y el vidrio solo llegan por el puerto.",
        ent.disponibilidad_local(Material::Adobe).round() as i64,
        (ent.clima * 100.0).round() as i32,
        ent.disponibilidad_local(Material::Paja).round() as i64,
        ent.disponibilidad_local(Material::Arena).round() as i64,
        ent.disponibilidad_local(Material::Madera).round() as i64,
        ent.disponibilidad_local(Material::Teja).round() as i64,
    )).small());
    section(ui, "Madera y cal en depósito (últimos meses)");
    let madera: Vec<f32> = m.historial.iter().map(|h| h[Material::Madera.idx()]).collect();
    let cal: Vec<f32> = m.historial.iter().map(|h| h[Material::Cal.idx()]).collect();
    bars(ui, &madera, None, theme::GOOD, "Madera");
    bars(ui, &cal, None, theme::PRESSED, "Cal");
}
