//! Panel de Desarrollo (industria y conocimiento): árbol de tecnologías por época con sus
//! requisitos, saberes de la población, industrias (perfiles, establecimientos y lo que
//! miran los inversores), incentivos del cabildo como tarjetas y el comercio exterior por
//! producto (exportaciones, importaciones, balanza, precios y diversificación) con gráficos.
//! El modelo está en `city::desarrollo`.

use super::{bars, fmt_dec, icons, section, theme, Card, Detail, Icon, UiAction};
use crate::city::desarrollo::{self as dv, Desarrollo, Dueno, Efecto, EstadoTec, Incentivo, Prod, Saber, Tec, INCENTIVOS, INDUSTRIAS, NPROD, PRODUCTOS, TECNOLOGIAS};
use crate::city::economy::{ord_info, Ordinance};
use crate::city::game::{fmt_thousands, Game};
use crate::city::poblacion::Epoca;
use crate::city::services::{self, barrio::numero_epoca};
use egui::{vec2, Align2, Color32, RichText, Sense};

pub const PESTAÑAS: [&str; 5] = ["Tecnologías", "Saberes", "Industria", "Fomento", "Comercio"];

/// Lo que se pide desde el panel de Desarrollo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// Empezar a invertir en una tecnología.
    Iniciar(Tec),
    /// Disponer o levantar un incentivo.
    Incentivo(Incentivo),
    /// Aprobar o derogar el libre comercio (es una ordenanza).
    LibreComercio,
}

/// Aplica una acción del panel.
pub fn apply(a: Act, game: &mut Game) {
    let r = match a {
        Act::Iniciar(t) => game.city.desarrollo_iniciar(t),
        Act::Incentivo(i) => game.city.desarrollo_alternar(i),
        Act::LibreComercio => {
            let c = &mut game.city;
            if !c.economy.is_on(Ordinance::LibreComercio) && c.desarrollo.incentivo(Incentivo::Arancel) {
                Err("Con aranceles protectores no hay libre comercio: primero hay que levantarlos".to_string())
            } else {
                c.economy.toggle(Ordinance::LibreComercio, &mut c.money)
            }
        }
    };
    match r {
        Ok(m) | Err(m) => game.say(m),
    }
}

/// Tarjetas del cajón de Desarrollo: las pestañas del panel y los edificios.
pub fn cards(game: &Game, abierto: Option<Detail>) -> Vec<Card> {
    let d = &game.city.desarrollo;
    let ctx = game.city.desarrollo_contexto();
    let disponibles = TECNOLOGIAS.iter().filter(|t| d.estado(t.id, &ctx) == EstadoTec::Disponible).count();
    let ult = d.historia.back();
    let tab = |i: u8, icon: Icon, name: &'static str, cost: String, tip: &str| Card {
        icon,
        name,
        cost,
        tip: tip.into(),
        action: Some(UiAction::Abrir(Detail::Desarrollo(i))),
        disabled: if game.city.plaza.is_none() { Some("Falta la plaza".into()) } else { None },
        selected: abierto == Some(Detail::Desarrollo(i)),
    };
    let mut v = vec![
        tab(0, Icon::Desarrollo, "Tecnologías", format!("{} lograda{} · {} disponible{}", d.hechas.len(), if d.hechas.len() == 1 { "" } else { "s" }, disponibles, if disponibles == 1 { "" } else { "s" }), "Árbol de tecnologías por época: requisitos, inversión del cabildo y lo que desbloquea cada una"),
        tab(1, Icon::Escuela, "Saberes", format!("{} oficiales", d.saber(Saber::Oficiales)), "Gente que lee, artesanos, oficiales formados y pilotos: de dónde salen y para qué sirven"),
        tab(2, Icon::Produccion, "Industria", format!("{} establecimientos", d.establecimientos.len()), "Perfiles de cada industria (empleo, capital, exportación), los establecimientos y lo que miran los inversores"),
        tab(3, Icon::Presupuesto, "Fomento", format!("{} incentivos", d.incentivos.len()), "Exención, crédito de fomento, aranceles, privilegios y maestros de ultramar: costos y efectos"),
        tab(4, Icon::Puerto, "Comercio exterior", ult.map_or("sin datos".into(), |f| format!("{} pesos/mes", fmt_thousands(f.exportaciones() as i64))), "Exportaciones e importaciones por producto, balanza, precios del mundo y diversificación"),
    ];
    for k in [services::Kind::EscuelaOficios, services::Kind::EscuelaNautica] {
        v.push(super::services::place_card_pub(game, k));
    }
    v
}

fn ok_color(ok: bool) -> Color32 {
    if ok { theme::GOOD } else { theme::BAD }
}

fn marca(ok: bool) -> &'static str {
    if ok { "•" } else { "×" }
}

fn pct(x: f32) -> String {
    format!("{} %", (x * 100.0).round() as i32)
}

/// Barras con signo (balanza comercial): verde arriba, rojo abajo.
fn barras_signo(ui: &mut egui::Ui, values: &[f32], label: &str) {
    let w = ui.available_width().min(460.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 60.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, theme::PAPER_DARK.gamma_multiply(0.5));
    if values.is_empty() {
        p.text(rect.center(), Align2::CENTER_CENTER, "Sin datos todavía", egui::FontId::proportional(11.0), theme::INK_SOFT);
        return;
    }
    let top = values.iter().map(|v| v.abs()).fold(1e-6f32, f32::max);
    let mid = rect.center().y - 5.0;
    let half = rect.height() * 0.5 - 8.0;
    let n = values.len().max(12) as f32;
    let bw = rect.width() / n;
    let off = (n as usize - values.len()) as f32 * bw;
    for (i, v) in values.iter().enumerate() {
        let x = rect.left() + off + i as f32 * bw;
        let h = v / top * half;
        p.rect_filled(egui::Rect::from_two_pos(egui::pos2(x + 1.0, mid), egui::pos2(x + bw - 1.0, mid - h)), 1.0, if *v >= 0.0 { theme::GOOD } else { theme::BAD });
    }
    p.line_segment([egui::pos2(rect.left(), mid), egui::pos2(rect.right(), mid)], egui::Stroke::new(1.0_f32, theme::INK_SOFT));
    p.text(egui::pos2(rect.left() + 3.0, rect.bottom() - 1.0), Align2::LEFT_BOTTOM, label, egui::FontId::proportional(10.0), theme::INK_SOFT);
    resp.on_hover_text(values.iter().map(|v| fmt_thousands(*v as i64)).collect::<Vec<_>>().join(" · "));
}

/// Colores de los productos en los gráficos apilados.
const COLORES: [Color32; NPROD] = [
    Color32::from_rgb(140, 92, 50),
    Color32::from_rgb(214, 186, 120),
    Color32::from_rgb(170, 70, 60),
    Color32::from_rgb(200, 160, 60),
    Color32::from_rgb(120, 40, 40),
    Color32::from_rgb(90, 60, 40),
    Color32::from_rgb(70, 100, 130),
    Color32::from_rgb(90, 90, 90),
    // Materiales: adoquines, sillería, arena, tejas, cal.
    Color32::from_rgb(200, 120, 140),
    Color32::from_rgb(150, 140, 160),
    Color32::from_rgb(235, 210, 110),
    Color32::from_rgb(190, 90, 50),
    Color32::from_rgb(235, 235, 220),
];

/// Exportaciones apiladas por producto (24 meses) con su leyenda.
fn apiladas(ui: &mut egui::Ui, d: &Desarrollo) {
    let w = ui.available_width().min(460.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 70.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, theme::PAPER_DARK.gamma_multiply(0.5));
    let fotos: Vec<&dv::Foto> = d.historia.iter().collect();
    if fotos.is_empty() {
        p.text(rect.center(), Align2::CENTER_CENTER, "Sin datos todavía", egui::FontId::proportional(11.0), theme::INK_SOFT);
        return;
    }
    let top = fotos.iter().map(|f| f.exportaciones()).fold(1e-6f32, f32::max);
    let n = fotos.len().max(12) as f32;
    let bw = rect.width() / n;
    let off = (n as usize - fotos.len()) as f32 * bw;
    let base = rect.bottom() - 12.0;
    for (i, f) in fotos.iter().enumerate() {
        let x = rect.left() + off + i as f32 * bw;
        let mut y = base;
        for k in 0..NPROD {
            let h = f.exportado[k] / top * (rect.height() - 14.0);
            if h > 0.2 {
                p.rect_filled(egui::Rect::from_min_max(egui::pos2(x + 1.0, y - h), egui::pos2(x + bw - 1.0, y)), 0.0, COLORES[k]);
                y -= h;
            }
        }
    }
    p.text(egui::pos2(rect.left() + 3.0, rect.bottom() - 1.0), Align2::LEFT_BOTTOM, "Exportaciones por mes, por producto (24 meses)", egui::FontId::proportional(10.0), theme::INK_SOFT);
    p.text(egui::pos2(rect.right() - 3.0, rect.top() + 1.0), Align2::RIGHT_TOP, format!("máx. {}", fmt_thousands(top as i64)), egui::FontId::proportional(10.0), theme::INK_SOFT);
    resp.on_hover_text(fotos.iter().map(|f| fmt_thousands(f.exportaciones() as i64)).collect::<Vec<_>>().join(" · "));
    ui.horizontal_wrapped(|ui| {
        for k in 0..NPROD {
            let (r, _) = ui.allocate_exact_size(vec2(9.0, 9.0), Sense::hover());
            ui.painter().rect_filled(r, 1.0, COLORES[k]);
            ui.label(RichText::new(PRODUCTOS[k].nombre).small());
        }
    });
}

/// Panel de Desarrollo. Devuelve `Some(None)` para cerrarlo y `Some(Some(d))` para cambiar de
/// pestaña.
pub fn panel(ctx: &egui::Context, game: &Game, tab: u8, reserved_bottom: f32, acts: &mut Vec<UiAction>) -> Option<Option<Detail>> {
    let city = &game.city;
    let d = &city.desarrollo;
    let cx = city.desarrollo_contexto();
    let mut nav = None;
    egui::Area::new("desarrollo".into()).anchor(Align2::LEFT_TOP, vec2(10.0, 52.0)).show(ctx, |ui| {
        theme::paper_frame().show(ui, |ui| {
            ui.set_width(500.0);
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Desarrollo, 22.0, theme::INK);
                ui.heading("Desarrollo: industria y conocimiento");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Cerrar").clicked() {
                        nav = Some(None);
                    }
                });
            });
            ui.horizontal(|ui| {
                for (i, t) in PESTAÑAS.iter().enumerate() {
                    if ui.selectable_label(tab as usize == i, *t).clicked() {
                        nav = Some(Some(Detail::Desarrollo(i as u8)));
                    }
                }
            });
            let max_h = (ctx.screen_rect().height() - 52.0 - reserved_bottom - 70.0).max(160.0);
            egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| match tab {
                0 => tecnologias(ui, game, &cx, acts),
                1 => saberes(ui, game),
                2 => industria(ui, game),
                3 => fomento(ui, game, acts),
                _ => comercio(ui, d),
            });
        });
    });
    nav
}

fn tecnologias(ui: &mut egui::Ui, game: &Game, cx: &dv::Contexto, acts: &mut Vec<UiAction>) {
    let d = &game.city.desarrollo;
    ui.label(RichText::new("Cada tecnología pide saberes, edificios u otras tecnologías; después el cabildo la paga en cuotas mensuales (con ladrillos para las obras). Sin plata o sin ladrillos, la obra se para.").small().color(theme::INK_SOFT));
    for e in Epoca::ALL {
        let techs: Vec<&dv::TecInfo> = TECNOLOGIAS.iter().filter(|t| t.epoca == e).collect();
        if techs.is_empty() {
            continue;
        }
        if e > cx.epoca {
            egui::CollapsingHeader::new(RichText::new(format!("Época {}: {} (todavía no se juega)", numero_epoca(e), e.nombre())).small()).id_salt(format!("epoca{}", numero_epoca(e))).show(ui, |ui| {
                for t in techs {
                    ui.label(RichText::new(format!("• {} — {}", t.nombre, t.desc)).small().color(theme::INK_SOFT));
                    ui.label(RichText::new(format!("   {}", t.historia)).small().italics().color(theme::INK_SOFT));
                }
            });
            continue;
        }
        section(ui, &format!("Época {}: {}", numero_epoca(e), e.nombre()));
        for t in techs {
            let st = d.estado(t.id, cx);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.nombre).strong());
                let (txt, col) = match &st {
                    EstadoTec::Hecha => ("lograda".to_string(), theme::GOOD),
                    EstadoTec::EnObra { hechos, meses, parado } => (format!("en marcha {hechos} de {meses} meses{}", if *parado { " · PARADA" } else { "" }), if *parado { theme::BAD } else { theme::INK }),
                    EstadoTec::Disponible => ("disponible".to_string(), theme::GOOD),
                    EstadoTec::Bloqueada(_) => ("faltan requisitos".to_string(), theme::BAD),
                    EstadoTec::OtraEpoca(_) => ("otra época".to_string(), theme::INK_SOFT),
                };
                ui.label(RichText::new(txt).small().color(col));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cuota = Desarrollo::cuota(t.id);
                    let en = st == EstadoTec::Disponible && game.city.money >= cuota;
                    let r = ui.add_enabled(en, egui::Button::new(RichText::new("Invertir").small()));
                    if r.on_hover_text(format!("{} pesos en {} meses: {} por mes{}", fmt_thousands(t.costo), t.meses, fmt_thousands(cuota), if t.ladrillos_mes > 0.0 { format!(" y {} ladrillos", t.ladrillos_mes as i32) } else { String::new() })).clicked() {
                        acts.push(UiAction::Desarrollo(Act::Iniciar(t.id)));
                    }
                });
            });
            if let EstadoTec::EnObra { hechos, meses, .. } = st {
                let frac = hechos as f32 / meses.max(1) as f32;
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().min(460.0), 6.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, theme::PAPER_DARK.gamma_multiply(0.6));
                ui.painter().rect_filled(egui::Rect::from_min_size(r.min, vec2(r.width() * frac, r.height())), 2.0, theme::GOOD);
            }
            ui.label(RichText::new(t.desc).small());
            if st != EstadoTec::Hecha {
                ui.horizontal_wrapped(|ui| {
                    for (txt, ok) in d.requisitos(t.id, cx) {
                        ui.label(RichText::new(format!("{} {txt}", marca(ok))).small().color(ok_color(ok)));
                    }
                });
            }
            let da: Vec<String> = t
                .efectos
                .iter()
                .map(|e| match e {
                    Efecto::Edificio(k) => format!("edificio: {}", services::info(*k).name.to_lowercase()),
                    Efecto::Productividad(i, m) => format!("{} rinde +{} %", dv::ind_info(*i).nombre.to_lowercase(), ((m - 1.0) * 100.0).round() as i32),
                    Efecto::Producto(p) => format!("producto para exportar: {}", dv::prod_info(*p).nombre.to_lowercase()),
                    Efecto::Texto(s) => s.to_lowercase(),
                })
                .collect();
            ui.label(RichText::new(format!("Da: {} · costo {} pesos en {} meses", da.join("; "), fmt_thousands(t.costo), t.meses)).small().color(theme::INK));
            ui.label(RichText::new(t.historia).small().italics().color(theme::INK_SOFT));
            ui.add_space(4.0);
        }
    }
}

fn saberes(ui: &mut egui::Ui, game: &Game) {
    let c = &game.city;
    let d = &c.desarrollo;
    let max = d.saberes.iter().copied().max().unwrap_or(1).max(1) as f32;
    section(ui, "La gente y lo que sabe");
    for s in Saber::ALL {
        let n = d.saber(s);
        let futuro = matches!(s, Saber::Tecnicos | Saber::Ingenieros);
        ui.horizontal(|ui| {
            ui.add_sized(vec2(140.0, 16.0), egui::Label::new(RichText::new(s.nombre()).small().color(if futuro { theme::INK_SOFT } else { theme::INK })));
            let (r, _) = ui.allocate_exact_size(vec2(180.0, 11.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, theme::PAPER_DARK.gamma_multiply(0.5));
            ui.painter().rect_filled(egui::Rect::from_min_size(r.min, vec2(r.width() * n as f32 / max, r.height())), 2.0, theme::GOOD);
            ui.label(RichText::new(if futuro { "épocas siguientes".into() } else { fmt_thousands(n as i64) }).small());
        })
        .response
        .on_hover_text(s.desc());
    }
    section(ui, "De dónde salen");
    let (_, jobs_i) = c.lots.jobs();
    let n_ofi = c.services.count(services::Kind::EscuelaOficios);
    let n_nau = c.services.count(services::Kind::EscuelaNautica);
    let lineas = [
        (format!("Escuelas de primeras letras: {} · el cabildo paga al maestro: {}", c.services.count(services::Kind::Escuela), if c.economy.is_on(Ordinance::EscuelasPrimerasLetras) { "sí" } else { "no" }), true),
        (format!("Escuelas de oficios: {} (unos {} oficiales por año)", n_ofi, (n_ofi as f32 * 15.0).round() as i32), n_ofi > 0),
        (format!("Aprendices en los talleres: {} empleos industriales (unos {} oficiales por año)", jobs_i, (jobs_i as f32 * 0.0005 * 12.0).round() as i32), jobs_i > 0),
        (format!("Escuela de náutica: {} (unos {} pilotos por año)", n_nau, (n_nau as f32 * 7.5).round() as i32), n_nau > 0),
        (format!("Maestros de ultramar: {}", if d.incentivo(Incentivo::Maestros) { "contratados (unos 4 oficiales por año)" } else { "no" }), d.incentivo(Incentivo::Maestros)),
    ];
    for (t, ok) in lineas {
        ui.label(RichText::new(format!("{} {t}", marca(ok))).small().color(if ok { theme::INK } else { theme::INK_SOFT }));
    }
    if d.sin_alumnos > 0.0 {
        ui.label(RichText::new("Quedaron plazas vacías en las escuelas: faltan jóvenes que sepan leer (más escuelas de primeras letras).").small().color(theme::BAD));
    }
    let usados: u32 = d.establecimientos.iter().filter_map(|e| e.info().saber).filter(|s| s.0 == Saber::Oficiales).map(|s| s.1).sum();
    ui.label(RichText::new(format!("Oficiales trabajando en las industrias: {} de {}", usados.min(d.saber(Saber::Oficiales)), d.saber(Saber::Oficiales))).small());
    let serie: Vec<f32> = d.historia.iter().map(|f| f.saberes[Saber::Oficiales.i()] as f32).collect();
    bars(ui, &serie, None, theme::GOOD, "Oficiales formados (24 meses)");
    let serie: Vec<f32> = d.historia.iter().map(|f| f.saberes[Saber::Alfabetizados.i()] as f32).collect();
    bars(ui, &serie, None, theme::INK_SOFT, "Adultos que leen (24 meses)");
}

fn industria(ui: &mut egui::Ui, game: &Game) {
    let c = &game.city;
    let d = &c.desarrollo;
    section(ui, "Perfiles: empleo, capital y exportación");
    egui::Grid::new("perfiles").num_columns(5).spacing(vec2(10.0, 2.0)).striped(true).show(ui, |ui| {
        for h in ["Industria", "Empleos", "Capital", "Produce por obrero", "Exporta"] {
            ui.label(RichText::new(h).small().strong());
        }
        ui.end_row();
        for inf in INDUSTRIAS {
            let valor: f32 = inf
                .produce
                .iter()
                .map(|(b, q)| {
                    q * match b {
                        dv::Bien::Res(r) => r.base_price(),
                        dv::Bien::Prod(p) => dv::prod_info(*p).precio,
                    }
                })
                .sum();
            let gente = inf.empleos + inf.saber.map_or(0, |s| s.1);
            let col = if inf.epoca == Epoca::Colonial { theme::INK } else { theme::INK_SOFT };
            ui.label(RichText::new(format!("{}{}", inf.nombre, if inf.epoca == Epoca::Colonial { String::new() } else { format!(" (época {})", numero_epoca(inf.epoca)) })).small().color(col))
                .on_hover_text(format!("{}{} Molestia para los vecinos: {}.", inf.desc, inf.recurso.map_or(String::new(), |r| format!(" Necesita sitio con: {r}.")), pct(inf.molestia)));
            ui.label(RichText::new(format!("{}", gente)).small().color(col));
            ui.label(RichText::new(fmt_thousands(inf.capital)).small().color(col));
            ui.label(RichText::new(format!("{} pesos/mes", fmt_dec(valor / gente.max(1) as f32, 1))).small().color(col));
            ui.label(RichText::new(pct(inf.exporta)).small().color(col));
            ui.end_row();
        }
    });
    ui.label(RichText::new("Las industrias de capital (saladero, carpintería de ribera) producen mucho por obrero y traen plata de afuera; los talleres dan más empleo por peso invertido.").small().color(theme::INK_SOFT));
    section(ui, "Establecimientos");
    if d.establecimientos.is_empty() {
        ui.label(RichText::new("Todavía no hay ninguno. Se levantan desde Producción (los paga el cabildo) o los abre un inversor cuando le conviene.").small().color(theme::INK_SOFT));
    }
    for e in &d.establecimientos {
        let dueno = match e.dueno {
            Dueno::Cabildo => "del cabildo".to_string(),
            Dueno::Hogar(h) => format!("de la familia {}", c.poblacion.apellido_hogar(h)),
            Dueno::Forastero => "de un comerciante de afuera".to_string(),
        };
        ui.label(RichText::new(format!("{} {} · {} peones · marcha {} · ganancia {} pesos/mes", e.info().nombre, dueno, e.empleados.len(), pct(e.marcha), fmt_thousands(e.ganancia as i64))).small().color(if e.ganancia >= 0.0 { theme::INK } else { theme::BAD }));
        if !e.falta.is_empty() {
            ui.label(RichText::new(format!("   no rinde a pleno: {}", e.falta)).small().color(theme::INK_SOFT));
        }
    }
    ui.label(RichText::new(format!("Inversión privada acumulada: {} pesos · ganancias que se van a Buenos Aires: {} pesos/mes", fmt_thousands(d.inversion_total as i64), fmt_thousands(d.remesas() as i64))).small());
    section(ui, "Lo que miran los inversores");
    if d.evaluaciones.is_empty() {
        ui.label(RichText::new("Ninguna industria habilitada todavía: primero las tecnologías.").small().color(theme::INK_SOFT));
    }
    for ev in &d.evaluaciones {
        let ok = ev.conviene();
        ui.label(RichText::new(format!("{} · rinde esperado {} al año · {}", dv::ind_info(ev.ind).nombre, pct(ev.rinde), if ok { "alguien va a invertir" } else { "nadie invierte todavía" })).small().strong().color(ok_color(ok)));
        ui.horizontal_wrapped(|ui| {
            for (m, ok) in &ev.motivos {
                ui.label(RichText::new(format!("{} {m}", marca(*ok))).small().color(ok_color(*ok)));
            }
        });
    }
}

fn fomento(ui: &mut egui::Ui, game: &Game, acts: &mut Vec<UiAction>) {
    let c = &game.city;
    let d = &c.desarrollo;
    ui.label(RichText::new("Incentivos del cabildo para que los vecinos inviertan en industrias. Ninguno es gratis: cuestan plata, impuestos que no se cobran u opinión. Cambiar uno cuesta 100 pesos de papeleo.").small().color(theme::INK_SOFT));
    for inf in INCENTIVOS.iter() {
        let on = d.incentivo(inf.id);
        ui.horizontal(|ui| {
            ui.label(RichText::new(inf.nombre).strong());
            if on {
                ui.label(RichText::new("· vigente").small().color(theme::GOOD));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(c.plaza.is_some(), egui::Button::new(RichText::new(if on { "Levantar" } else { "Disponer" }).small())).clicked() {
                    acts.push(UiAction::Desarrollo(Act::Incentivo(inf.id)));
                }
            });
        });
        ui.label(RichText::new(inf.desc).small());
        let op: Vec<String> = ["vecinos", "comerciantes", "hacendados"].iter().zip(inf.opinion).filter(|x| x.1 != 0.0).map(|(n, v)| format!("{}{} {n}", if v > 0.0 { "+" } else { "" }, v as i32)).collect();
        ui.label(RichText::new(format!("{}Opinión: {}", if inf.mensual > 0 { format!("{} pesos por mes · ", inf.mensual) } else { String::new() }, op.join(", "))).small().color(theme::INK_SOFT));
        ui.label(RichText::new(inf.historia).small().italics().color(theme::INK_SOFT));
        ui.add_space(3.0);
    }
    section(ui, "Comercio con ultramar (ordenanza)");
    let lc = c.economy.is_on(Ordinance::LibreComercio);
    ui.horizontal(|ui| {
        ui.label(RichText::new(ord_info(Ordinance::LibreComercio).name).strong());
        if lc {
            ui.label(RichText::new("· vigente").small().color(theme::GOOD));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(c.plaza.is_some(), egui::Button::new(RichText::new(if lc { "Derogar" } else { "Aprobar" }).small())).clicked() {
                acts.push(UiAction::Desarrollo(Act::LibreComercio));
            }
        });
    });
    ui.label(RichText::new(format!("{} En la historia, el Reglamento de comercio libre de 1778 abrió el comercio de los puertos del Río de la Plata con España. Es lo contrario de los aranceles protectores.", ord_info(Ordinance::LibreComercio).desc)).small());
    section(ui, "Lo que cuesta");
    let ult = d.historia.back();
    ui.label(RichText::new(format!("Renuncia fiscal por la exención: {} pesos el último mes, {} en total", fmt_thousands(ult.map_or(0.0, |f| f.renuncia) as i64), fmt_thousands(d.renuncia_total as i64))).small());
    let deuda: f32 = d.establecimientos.iter().map(|e| e.deuda).sum();
    ui.label(RichText::new(format!("Préstamos de fomento pendientes de cobro: {} pesos", fmt_thousands(deuda as i64))).small());
    let serie: Vec<f32> = d.historia.iter().map(|f| f.inversion).collect();
    bars(ui, &serie, None, theme::GOOD, "Inversión privada por mes (24 meses)");
}

fn comercio(ui: &mut egui::Ui, d: &Desarrollo) {
    let x12 = d.exportado_12();
    let total: f32 = x12.iter().sum();
    let imp12: f32 = d.historia.iter().rev().take(12).map(|f| f.importado).sum();
    section(ui, "Último año");
    egui::Grid::new("comercio").num_columns(4).spacing(vec2(12.0, 2.0)).striped(true).show(ui, |ui| {
        for h in ["Producto", "Exportado", "Parte", "Precio afuera"] {
            ui.label(RichText::new(h).small().strong());
        }
        ui.end_row();
        for p in Prod::ALL {
            let v = x12[p.i()];
            let precio = d.precios[p.i()];
            let col = if precio > 1.1 { theme::GOOD } else if precio < 0.9 { theme::BAD } else { theme::INK };
            ui.label(RichText::new(dv::prod_info(p).nombre).small());
            ui.label(RichText::new(format!("{} pesos", fmt_thousands(v as i64))).small());
            ui.label(RichText::new(if total > 0.0 { pct(v / total) } else { "—".into() }).small());
            ui.label(RichText::new(format!("{} {}", fmt_dec(precio, 2), if precio > 1.05 { "sube" } else if precio < 0.95 { "baja" } else { "" })).small().color(col));
            ui.end_row();
        }
    });
    let bal = total - imp12;
    ui.label(RichText::new(format!("Exportaciones {} · importaciones {} · balanza {}{}", fmt_thousands(total as i64), fmt_thousands(imp12 as i64), if bal >= 0.0 { "+" } else { "" }, fmt_thousands(bal as i64))).small().strong().color(ok_color(bal >= 0.0)));
    let h = d.concentracion();
    let (nivel, col) = if h > 0.6 { ("muy concentrada: un choque de precios golpea fuerte", theme::BAD) } else if h > 0.35 { ("concentrada", theme::INK) } else { ("diversificada: aguanta mejor los choques", theme::GOOD) };
    ui.label(RichText::new(format!("Concentración de las exportaciones (índice de Herfindahl): {} — {nivel}", fmt_dec(h, 2))).small().color(col))
        .on_hover_text("Suma de los cuadrados de la parte de cada producto: 1 si se vende uno solo, cerca de 0 si se venden muchos por partes iguales.");
    if let Some((i, m)) = d.choque {
        let ch = &dv::CHOQUES[i];
        ui.label(RichText::new(format!("Choque en curso: «{}» ({} ×{}) por {} meses más", ch.nombre, dv::prod_info(ch.prod).nombre.to_lowercase(), fmt_dec(ch.factor, 2), m)).small().color(theme::BAD));
    }
    if d.bono_pilotos() > 1.001 {
        ui.label(RichText::new(format!("Pilotos propios: lo que se embarca se vende {} más caro", pct(d.bono_pilotos() - 1.0))).small().color(theme::GOOD));
    }
    section(ui, "Historia");
    apiladas(ui, d);
    let bal: Vec<f32> = d.historia.iter().map(|f| f.exportaciones() - f.importado).collect();
    barras_signo(ui, &bal, "Balanza comercial por mes (exportado menos importado)");
    let hh: Vec<f32> = d.historia.iter().map(|f| f.hhi).collect();
    bars(ui, &hh, None, theme::INK_SOFT, "Concentración de las exportaciones (24 meses)");
}
