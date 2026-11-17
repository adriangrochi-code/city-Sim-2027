//! Paneles de la población: resumen (pirámide, nacimientos y muertes, orígenes, clases),
//! desigualdad (Gini, el 10 % más rico, pobreza por origen y condición, políticas), felicidad
//! por grupo con sus motivos, la crónica, los hogares de una casa y la ficha de cada persona
//! (personalidad, felicidad, familia, linaje e historia de vida).

use super::{bars, capitalize, fmt_dec, icons, kv, section, theme, Detail, Icon, UiAction};
use crate::city::economy::{ord_info, Sector};
use crate::city::game::{fmt_thousands, Game};
use crate::city::poblacion::estadisticas::{BARRIOS, EDADES, GRUPOS_PIRAMIDE};
use crate::city::poblacion::felicidad::{motivo_texto, FACTORES, SIN_MOTIVO};
use crate::city::poblacion::politicas::{FUTURAS, SOCIALES};
use crate::city::poblacion::textos::{educacion, fecha};
use crate::city::poblacion::{año_de, Clase, Condicion, Estamento, Oficio, PersonaId, Poblacion, Rasgos, Sexo, DETALLE};
use egui::{vec2, Align2, Color32, RichText, Sense};

const HOMBRES: Color32 = Color32::from_rgb(88, 116, 150);
const MUJERES: Color32 = Color32::from_rgb(166, 92, 70);
const PESTAÑAS: [&str; 4] = ["Resumen", "Desigualdad", "Felicidad", "Crónica"];

/// Ficha de la barra superior: habitantes y felicidad.
pub fn chip(game: &Game) -> (Icon, String, String) {
    let p = &game.city.poblacion;
    if !p.fundada {
        return (Icon::Familia, "—".into(), "Población: todavía no se fundó la ciudad".into());
    }
    let s = &p.stats;
    let ult = p.historial.back();
    let tip = format!(
        "Población: {} habitantes en {} hogares · felicidad {} %{}\nEl mes pasado: {} nacimientos, {} muertes, {} casamientos, {} llegados",
        fmt_thousands(s.habitantes as i64),
        fmt_thousands(s.hogares as i64),
        (s.felicidad * 100.0).round() as i32,
        s.motivos.first().map_or(String::new(), |m| format!(" · lo que más preocupa: {}", motivo_texto(m.0))),
        ult.map_or(0, |f| f.cuenta.nacimientos),
        ult.map_or(0, |f| f.cuenta.muertes),
        ult.map_or(0, |f| f.cuenta.casamientos),
        ult.map_or(0, |f| f.cuenta.llegados)
    );
    (Icon::Familia, format!("{} · {} %", fmt_thousands(s.habitantes as i64), (s.felicidad * 100.0).round() as i32), tip)
}

fn pct(x: f32) -> String {
    format!("{} %", (x * 100.0).round() as i32)
}

fn color_fel(f: f32) -> Color32 {
    if f >= 0.6 {
        theme::GOOD
    } else if f >= 0.42 {
        theme::INK
    } else {
        theme::BAD
    }
}

/// Barra horizontal con etiqueta y valor (fracción 0 a 1 del ancho).
fn hbar(ui: &mut egui::Ui, label: &str, frac: f32, valor: String, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(130.0, 16.0), egui::Label::new(RichText::new(label).small()).truncate());
        let (r, _) = ui.allocate_exact_size(vec2(150.0, 11.0), Sense::hover());
        ui.painter().rect_filled(r, 2.0, theme::PAPER_DARK.gamma_multiply(0.5));
        let w = r.width() * frac.clamp(0.0, 1.0);
        ui.painter().rect_filled(egui::Rect::from_min_size(r.min, vec2(w, r.height())), 2.0, color);
        ui.label(RichText::new(valor).small());
    });
}

/// Pirámide de población: hombres a la izquierda, mujeres a la derecha.
fn piramide(ui: &mut egui::Ui, p: &[[u32; 2]; GRUPOS_PIRAMIDE]) {
    let alto = 9.0;
    let w = ui.available_width().min(380.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, alto * GRUPOS_PIRAMIDE as f32 + 14.0), Sense::hover());
    let pt = ui.painter();
    let total: u32 = p.iter().map(|x| x[0] + x[1]).sum::<u32>().max(1);
    let max = p.iter().flat_map(|x| x.iter()).copied().max().unwrap_or(1).max(1) as f32;
    let mid = rect.center().x;
    let medio = w * 0.5 - 26.0;
    for (g, x) in p.iter().enumerate() {
        let y = rect.top() + (GRUPOS_PIRAMIDE - 1 - g) as f32 * alto;
        let hw = x[0] as f32 / max * medio;
        let mw = x[1] as f32 / max * medio;
        pt.rect_filled(egui::Rect::from_min_max(egui::pos2(mid - 18.0 - hw, y + 1.0), egui::pos2(mid - 18.0, y + alto - 1.0)), 1.0, HOMBRES);
        pt.rect_filled(egui::Rect::from_min_max(egui::pos2(mid + 18.0, y + 1.0), egui::pos2(mid + 18.0 + mw, y + alto - 1.0)), 1.0, MUJERES);
        if g % 2 == 0 {
            let t = if g == GRUPOS_PIRAMIDE - 1 { "80+".to_string() } else { format!("{}", g * 5) };
            pt.text(egui::pos2(mid, y + alto * 0.5), Align2::CENTER_CENTER, t, egui::FontId::proportional(9.5), theme::INK_SOFT);
        }
    }
    let y = rect.bottom() - 6.0;
    pt.text(egui::pos2(rect.left() + 2.0, y), Align2::LEFT_CENTER, "Hombres", egui::FontId::proportional(10.0), HOMBRES);
    pt.text(egui::pos2(rect.right() - 2.0, y), Align2::RIGHT_CENTER, "Mujeres", egui::FontId::proportional(10.0), MUJERES);
    resp.on_hover_text(format!("Pirámide por grupos de cinco años ({} personas)", fmt_thousands(total as i64)));
}

/// Botón con el nombre de una persona (lleva a su ficha).
fn persona_btn(ui: &mut egui::Ui, p: &Poblacion, id: PersonaId, nav: &mut Option<Option<Detail>>) {
    let viva = p.personas[id as usize].viva();
    let t = RichText::new(p.etiqueta(id)).small().color(if viva { theme::INK } else { theme::INK_SOFT });
    if ui.add(egui::Button::new(t).frame(false)).on_hover_text("Ver su ficha").clicked() {
        *nav = Some(Some(Detail::Persona(id)));
    }
}

/// Panel de la población, de una casa o de una persona. Devuelve `Some(None)` para cerrarlo,
/// `Some(Some(d))` para ir a otro panel y `None` si no cambia.
pub fn panel(ctx: &egui::Context, game: &Game, d: Detail, reserved_bottom: f32, acts: &mut Vec<UiAction>) -> Option<Option<Detail>> {
    let p = &game.city.poblacion;
    let mut nav = None;
    egui::Area::new("poblacion".into()).anchor(Align2::LEFT_TOP, vec2(10.0, 52.0)).show(ctx, |ui| {
        theme::paper_frame().show(ui, |ui| {
            ui.set_width(if matches!(d, Detail::Persona(_)) { 820.0 } else { 430.0 });
            let titulo = match d {
                Detail::Hogares(l) => format!("Casa n.º {l}: quiénes viven"),
                Detail::Persona(id) if p.persona(id).is_some() => p.nombre_completo(id),
                _ => "Población".into(),
            };
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Familia, 22.0, theme::INK);
                ui.heading(titulo);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Cerrar").clicked() {
                        nav = Some(None);
                    }
                    if !matches!(d, Detail::Poblacion(_)) && ui.small_button("Población").on_hover_text("Volver al panel de la población").clicked() {
                        nav = Some(Some(Detail::Poblacion(0)));
                    }
                });
            });
            if !p.fundada {
                ui.label("Todavía no se fundó la ciudad.");
                return;
            }
            if let Detail::Poblacion(tab) = d {
                ui.horizontal(|ui| {
                    for (i, t) in PESTAÑAS.iter().enumerate() {
                        if ui.selectable_label(tab as usize == i, *t).clicked() {
                            nav = Some(Some(Detail::Poblacion(i as u8)));
                        }
                    }
                });
            }
            let max_h = (ctx.screen_rect().height() - 52.0 - reserved_bottom - 70.0).max(160.0);
            egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| match d {
                Detail::Poblacion(0) => resumen(ui, p),
                Detail::Poblacion(1) => desigualdad(ui, game, acts),
                Detail::Poblacion(2) => felicidad(ui, p),
                Detail::Poblacion(_) => cronica(ui, p),
                Detail::Hogares(l) => hogares(ui, p, l, &mut nav),
                Detail::Persona(id) => persona(ui, game, id, &mut nav),
                _ => {}
            });
        });
    });
    nav
}

fn resumen(ui: &mut egui::Ui, p: &Poblacion) {
    let s = &p.stats;
    let año = p.anual.last();
    egui::Grid::new("pob").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        kv(ui, "Habitantes", format!("{} en {} hogares ({} por hogar)", fmt_thousands(s.habitantes as i64), fmt_thousands(s.hogares as i64), fmt_dec(s.tam_hogar, 1)), theme::INK);
        kv(ui, "Edad media", format!("{} años · {} hombres cada 100 mujeres", fmt_dec(s.edad_media, 1), s.hombres_por_100_mujeres.round() as i32), theme::INK);
        kv(ui, "Saben leer", pct(s.lectura), theme::INK);
        kv(ui, "Sin casa", format!("{} personas", fmt_thousands(s.sin_casa as i64)), if s.sin_casa > 0 { theme::BAD } else { theme::INK });
        kv(ui, "En la campaña", format!("{} personas en chacras y estancias", fmt_thousands(s.campo as i64)), theme::INK);
        if let Some(a) = año {
            kv(ui, &format!("Año {}", año_de(a.mes)), format!("{} nacimientos ({}‰), {} muertes ({}‰)", a.cuenta.nacimientos, fmt_dec(a.natalidad, 1), a.cuenta.muertes, fmt_dec(a.mortalidad, 1)), theme::INK);
            let tmi = if a.cuenta.nacimientos > 0 { a.cuenta.muertes_infantiles as f32 / a.cuenta.nacimientos as f32 * 1000.0 } else { 0.0 };
            kv(ui, "", format!("{} casamientos · {} llegados · {} se fueron · mortalidad infantil {}‰", a.cuenta.casamientos, a.cuenta.llegados, a.cuenta.emigrados, tmi.round() as i32), theme::INK_SOFT);
        }
    });
    section(ui, "Pirámide de edades");
    piramide(ui, &s.piramide);
    section(ui, "Últimos años");
    let hab: Vec<f32> = p.anual.iter().rev().take(30).rev().map(|f| f.habitantes as f32).collect();
    bars(ui, &hab, None, theme::GOOD, "Habitantes por año");
    let nat: Vec<f32> = p.anual.iter().rev().take(30).rev().map(|f| f.natalidad).collect();
    let mor: Vec<f32> = p.anual.iter().rev().take(30).rev().map(|f| f.mortalidad).collect();
    bars(ui, &nat, Some(&mor), HOMBRES, "Nacimientos por mil (raya: muertes por mil)");
    section(ui, "Dónde nacieron");
    let max = s.por_origen.first().map_or(1, |x| x.1).max(1) as f32;
    for (o, n) in s.por_origen.iter().take(9) {
        hbar(ui, o.corto(), *n as f32 / max, fmt_thousands(*n as i64), theme::PRESSED);
    }
    section(ui, "Clases y condición legal");
    let tot = s.habitantes.max(1) as f32;
    for c in Clase::ALL {
        hbar(ui, &capitalize(c.nombre()), s.por_clase[c as usize] as f32 / tot, fmt_thousands(s.por_clase[c as usize] as i64), theme::PRESSED);
    }
    for c in Condicion::ALL {
        hbar(ui, c.plural(), s.por_condicion[c as usize] as f32 / tot, fmt_thousands(s.por_condicion[c as usize] as i64), if c == Condicion::Esclavizada { theme::BAD } else { theme::GOOD });
    }
    if let Some(m) = p.leyes.vientres {
        ui.label(RichText::new(format!("Libertad de vientres desde {}.", fecha(m))).small().color(theme::INK_SOFT));
    }
    if let Some(m) = p.leyes.abolicion {
        ui.label(RichText::new(format!("Esclavitud abolida desde {}.", fecha(m))).small().color(theme::INK_SOFT));
    }
}

fn desigualdad(ui: &mut egui::Ui, game: &Game, acts: &mut Vec<UiAction>) {
    let p = &game.city.poblacion;
    let s = &p.stats;
    egui::Grid::new("des").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        kv(ui, "Gini de la riqueza", fmt_dec(s.gini_riqueza, 2), if s.gini_riqueza > 0.75 { theme::BAD } else { theme::INK });
        kv(ui, "Gini del ingreso", fmt_dec(s.gini_ingreso, 2), if s.gini_ingreso > 0.5 { theme::BAD } else { theme::INK });
        kv(ui, "El 10 % más rico tiene", format!("{} de la riqueza", pct(s.top10)), theme::INK);
        kv(ui, "En hogares pobres", pct(s.pobreza), if s.pobreza > 0.2 { theme::BAD } else { theme::INK });
    });
    ui.label(RichText::new("Gini: 0 si todos tienen lo mismo, 1 si una sola familia tiene todo.").small().color(theme::INK_SOFT));
    section(ui, "Últimos años");
    let g: Vec<f32> = p.anual.iter().rev().take(30).rev().map(|f| f.gini_riqueza).collect();
    bars(ui, &g, None, theme::PRESSED, "Gini de la riqueza por año");
    let po: Vec<f32> = p.anual.iter().rev().take(30).rev().map(|f| f.pobreza * 100.0).collect();
    bars(ui, &po, None, theme::BAD, "Pobreza (%) por año");
    section(ui, "Pobreza por condición legal");
    for c in Condicion::ALL {
        let v = s.pobreza_por_condicion[c as usize];
        let txt = if c == Condicion::Esclavizada { "sin derecho a bienes".to_string() } else { pct(v) };
        hbar(ui, c.plural(), v, txt, theme::BAD);
    }
    section(ui, "Pobreza y riqueza por origen del hogar");
    for (o, v, n) in s.pobreza_por_origen.iter().take(8) {
        let r = s.riqueza_por_origen.iter().find(|x| x.0 == *o).map_or(0.0, |x| x.1);
        hbar(ui, o.corto(), *v, format!("{} pobres · {} pesos por hogar · {} personas", pct(*v), fmt_thousands(r.round() as i64), n), theme::BAD);
    }
    section(ui, "Políticas del cabildo contra la desigualdad");
    let e = &game.city.economy;
    for (o, efecto) in SOCIALES {
        let info = ord_info(o);
        ui.horizontal(|ui| {
            ui.label(RichText::new(info.name).small().strong());
            if e.is_on(o) {
                ui.label(RichText::new("· vigente").small().color(theme::GOOD));
            }
        });
        ui.label(RichText::new(efecto).small().color(theme::INK_SOFT));
    }
    if ui.button("Abrir las ordenanzas").on_hover_text("Aprobar o derogar políticas").clicked() {
        acts.push(UiAction::ToggleList);
    }
    section(ui, "En las épocas siguientes");
    for f in FUTURAS {
        ui.label(RichText::new(format!("{} ({}): {}", f.nombre, f.epoca.nombre(), f.efecto)).small().color(theme::INK_SOFT));
    }
}

fn felicidad(ui: &mut egui::Ui, p: &Poblacion) {
    let s = &p.stats;
    egui::Grid::new("fel").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        kv(ui, "Felicidad media", pct(s.felicidad), color_fel(s.felicidad));
        kv(ui, "Adultos descontentos", pct(s.descontentos), if s.descontentos > 0.3 { theme::BAD } else { theme::INK });
    });
    let animo = p.animo_sectores();
    ui.label(RichText::new(format!("Pesa en la opinión del cabildo: {}.", Sector::ALL.iter().zip(animo).map(|(s, a)| format!("{} {}", s.name().to_lowercase(), pct(a))).collect::<Vec<_>>().join(", "))).small().color(theme::INK_SOFT));
    section(ui, "Lo que más preocupa");
    let max = s.motivos.first().map_or(1, |m| m.1).max(1) as f32;
    for (m, n) in &s.motivos {
        hbar(ui, &capitalize(motivo_texto(*m)), *n as f32 / max, format!("{} personas", fmt_thousands(*n as i64)), theme::BAD);
    }
    let grupo = |ui: &mut egui::Ui, titulo: &str, filas: Vec<(String, f32)>| {
        section(ui, titulo);
        for (k, v) in filas {
            hbar(ui, &k, v, pct(v), color_fel(v));
        }
    };
    grupo(ui, "Por edad", EDADES.iter().zip(s.fel_edad).map(|(a, b)| (a.to_string(), b)).collect());
    grupo(ui, "Por sexo", vec![("Hombres".into(), s.fel_sexo[0]), ("Mujeres".into(), s.fel_sexo[1])]);
    grupo(ui, "Por clase", Clase::ALL.iter().map(|c| (capitalize(c.nombre()), s.fel_clase[*c as usize])).filter(|x| x.1 > 0.0).collect());
    grupo(ui, "Por condición legal", Condicion::ALL.iter().map(|c| (c.plural().to_string(), s.fel_condicion[*c as usize])).filter(|x| x.1 > 0.0).collect());
    grupo(ui, "Por barrio", BARRIOS.iter().zip(s.fel_barrio).filter(|x| x.1 > 0.0).map(|(a, b)| (a.to_string(), b)).collect());
    grupo(ui, "Por lugar de nacimiento", s.fel_origen.iter().take(8).map(|(o, f, _)| (o.corto().to_string(), *f)).collect());
    section(ui, "Últimos meses");
    let h: Vec<f32> = p.historial.iter().map(|f| f.felicidad * 100.0).collect();
    bars(ui, &h, Some(&vec![50.0; h.len()]), theme::PRESSED, "Felicidad (%) · raya: 50");
}

fn cronica(ui: &mut egui::Ui, p: &Poblacion) {
    ui.label(RichText::new("La crónica de la ciudad: llegadas, casamientos, herencias, libertades y el balance de cada año.").small().color(theme::INK_SOFT));
    for (m, t) in p.cronica.iter().rev().take(60) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(fecha(*m)).small().color(theme::INK_SOFT));
            ui.label(RichText::new(t).small());
        });
    }
}

fn hogares(ui: &mut egui::Ui, p: &Poblacion, lote: u32, nav: &mut Option<Option<Detail>>) {
    let hs: Vec<_> = p.hogares_de_lote(lote).collect();
    if hs.is_empty() {
        ui.label("Nadie vive en esta casa todavía.");
        return;
    }
    for h in hs {
        let dueno = p.duenos.get(&lote) == Some(&h.id);
        section(ui, &format!("Familia {} · {} · {}", p.apellido_hogar(h.id), h.clase.nombre(), if dueno { "dueña de la casa" } else { "inquilina" }));
        egui::Grid::new(("hogar", h.id)).num_columns(2).spacing(vec2(12.0, 1.0)).show(ui, |ui| {
            kv(ui, "Llegó de", h.origen.corto().into(), theme::INK);
            kv(ui, "Ingresos del mes", format!("{} pesos", fmt_dec(h.ingresos, 1)), theme::GOOD);
            kv(ui, "Gastos del mes", format!("{} pesos", fmt_dec(h.gastos, 1)), theme::BAD);
            for (k, v) in DETALLE.iter().zip(h.detalle) {
                if v.abs() >= 0.05 {
                    kv(ui, &format!("  {k}"), fmt_dec(v, 1), theme::INK_SOFT);
                }
            }
            kv(ui, "Ahorro y deuda", format!("{} / {} pesos", fmt_thousands(h.ahorro.round() as i64), fmt_thousands(h.deuda.round() as i64)), if h.deuda > h.ahorro { theme::BAD } else { theme::INK });
            if h.ganado > 0 {
                kv(ui, "Ganado", format!("{} cabezas", fmt_thousands(h.ganado as i64)), theme::INK);
            }
            kv(ui, "Solares propios", format!("{}", h.solares.len()), theme::INK);
        });
        for &m in &h.miembros {
            let q = &p.personas[m as usize];
            ui.horizontal(|ui| {
                persona_btn(ui, p, m, nav);
                let rol = if m == h.jefe { "jefe del hogar".to_string() } else { q.oficio.nombre(q.sexo).to_string() };
                let cond = if q.condicion != Condicion::Libre { format!(" · {}", q.condicion.nombre(q.sexo)) } else { String::new() };
                ui.label(RichText::new(format!("{rol}{cond}")).small().color(theme::INK_SOFT));
            });
        }
    }
}

fn rasgo(ui: &mut egui::Ui, r: &Rasgos, i: usize) {
    hbar(ui, &capitalize(Rasgos::NOMBRES[i]), r.get(i), pct(r.get(i)), theme::PRESSED);
}

fn persona(ui: &mut egui::Ui, game: &Game, id: PersonaId, nav: &mut Option<Option<Detail>>) {
    let p = &game.city.poblacion;
    let Some(q) = p.persona(id) else {
        ui.label("No existe esa persona.");
        return;
    };
    let mes = p.mes;
    egui::Grid::new("per").num_columns(2).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        let estado = if q.muerte.is_some() { format!("murió en {}", fecha(q.muerte.unwrap())) } else if q.fuera.is_some() { "se fue de la ciudad".into() } else { format!("{} años", q.edad(mes)) };
        kv(ui, "Edad", format!("{} · nació en {}", estado, año_de(q.nacimiento)), theme::INK);
        kv(ui, "Lugar de nacimiento", capitalize(q.origen.nombre()), theme::INK);
        kv(ui, "Condición", capitalize(q.condicion.nombre(q.sexo)), if q.condicion == Condicion::Esclavizada { theme::BAD } else { theme::INK });
        let est = if q.estamento == Estamento::Vecino { "vecino con casa poblada" } else { q.estamento.nombre() };
        kv(ui, "Oficio", format!("{} · {}", capitalize(q.oficio.nombre(q.sexo)), est), theme::INK);
        kv(ui, "Educación", capitalize(educacion(q.educacion)), theme::INK);
        kv(ui, "Salud", pct(q.salud), color_fel(q.salud));
        if q.riqueza >= 1.0 {
            let k = if q.condicion == Condicion::Esclavizada { "Peculio ahorrado" } else { "Bienes propios" };
            kv(ui, k, format!("{} pesos", fmt_thousands(q.riqueza.round() as i64)), theme::INK);
        }
        if let Some(l) = p.lugar_de_trabajo(id) {
            kv(ui, "Trabaja en", format!("el solar n.º {l}"), theme::INK);
        }
        if let Some(h) = q.hogar.and_then(|h| p.hogar(h)) {
            kv(ui, "Hogar", format!("familia {} ({})", p.apellido_hogar(h.id), h.clase.nombre()), theme::INK);
        }
    });
    if q.condicion == Condicion::Esclavizada {
        ui.label(RichText::new("La ley de la época le niega la libertad: no puede tener casa ni elegir dónde trabajar, y su trabajo beneficia al hogar donde vive. Puede ahorrar para su carta de libertad, formar familia y ser libre por testamento, por el fondo del cabildo o por la ley.").small().color(theme::INK_SOFT));
    }
    if let Some(h) = q.hogar.and_then(|h| p.hogar(h)).and_then(|h| h.lote.filter(|_| h.vive_en_solar())) {
        if ui.small_button("Ver su casa").clicked() {
            *nav = Some(Some(Detail::Hogares(h)));
        }
    }
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        if q.viva() {
            section(ui, &format!("Felicidad: {}", pct(q.felicidad)));
            let ent = game.city.entorno_actual();
            let d = p.desglose(id, &ent);
            for (i, f) in FACTORES.iter().enumerate() {
                if d.pesos[i] > 0.05 {
                    hbar(ui, f, d.valores[i], format!("{} · pesa {}", pct(d.valores[i]), fmt_dec(d.pesos[i], 1)), color_fel(d.valores[i]));
                }
            }
            let motivos: Vec<&str> = q.motivos.iter().filter(|&&m| m != SIN_MOTIVO).map(|&m| motivo_texto(m)).collect();
            let txt = if motivos.is_empty() { "Está conforme con su vida.".to_string() } else { format!("Le preocupa: {}.", motivos.join("; ")) };
            ui.label(RichText::new(txt).small());
        }
        section(ui, "Personalidad");
        for i in 0..5 {
            rasgo(ui, &q.rasgos, i);
        }
        familia_e_historia(&mut cols[1], p, id, nav);
    });
}

fn familia_e_historia(ui: &mut egui::Ui, p: &Poblacion, id: PersonaId, nav: &mut Option<Option<Detail>>) {
    let q = &p.personas[id as usize];
    let mes = p.mes;
    section(ui, "Familia");
    let f = p.familia(id);
    let fila = |ui: &mut egui::Ui, titulo: &str, ids: &[PersonaId], nav: &mut Option<Option<Detail>>| {
        if ids.is_empty() {
            return;
        }
        ui.label(RichText::new(titulo).small().color(theme::INK_SOFT));
        ui.horizontal_wrapped(|ui| {
            for &x in ids.iter().take(14) {
                persona_btn(ui, p, x, nav);
            }
            if ids.len() > 14 {
                ui.label(RichText::new(format!("y {} más", ids.len() - 14)).small());
            }
        });
    };
    fila(ui, "Abuelos", &f.abuelos, nav);
    fila(ui, "Padres", &f.padres, nav);
    if let Some(c) = f.conyuge {
        fila(ui, if q.union_formal { "Cónyuge" } else { "Pareja" }, &[c], nav);
    }
    fila(ui, "Hermanos", &f.hermanos, nav);
    fila(ui, "Hijos", &f.hijos, nav);
    fila(ui, "Nietos", &f.nietos, nav);
    let lin = p.linaje(id);
    if lin.len() > 1 {
        let raiz = *lin.last().unwrap();
        let r = &p.personas[raiz as usize];
        section(ui, &format!("Linaje: {} generaciones desde {}", lin.len(), p.nombre_completo(raiz)));
        ui.label(RichText::new(format!("{} llegó de {} hacia {}; hoy viven {} de sus descendientes.", p.nombre_completo(raiz), r.origen.nombre(), año_de(r.nacimiento).max(1726), p.descendientes_vivos(raiz))).small());
        ui.horizontal_wrapped(|ui| {
            for (i, &x) in lin.iter().rev().enumerate() {
                if i > 0 {
                    ui.label(RichText::new("→").small().color(theme::INK_SOFT));
                }
                persona_btn(ui, p, x, nav);
            }
        });
    }
    section(ui, "Historia de vida");
    for e in q.historial.iter().rev().take(30) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(fecha(e.mes)).small().color(theme::INK_SOFT));
            ui.label(RichText::new(p.texto_evento(q, e)).small());
        });
    }
    if q.oficio == Oficio::Ninguno && q.sexo == Sexo::Mujer && q.edad(mes) >= 15 {
        ui.label(RichText::new("Trabaja en la casa (no cobra salario).").small().color(theme::INK_SOFT));
    }
}
