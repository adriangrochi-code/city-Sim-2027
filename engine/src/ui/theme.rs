//! Tema de plano colonial para egui: papel envejecido, tinta sepia y bordes marrones,
//! con la tipografía EB Garamond (OFL) que cubre acentos, ñ, ¿ ¡ « ».

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, TextStyle};

pub const PAPER: Color32 = Color32::from_rgba_premultiplied(232, 222, 197, 244);
pub const PAPER_DARK: Color32 = Color32::from_rgb(210, 195, 162);
pub const PAPER_LIGHT: Color32 = Color32::from_rgb(240, 232, 212);
pub const INK: Color32 = Color32::from_rgb(61, 43, 26);
pub const INK_SOFT: Color32 = Color32::from_rgb(110, 86, 60);
pub const BORDER: Color32 = Color32::from_rgb(138, 111, 77);
pub const PRESSED: Color32 = Color32::from_rgb(140, 107, 72);
pub const PRESSED_TEXT: Color32 = Color32::from_rgb(250, 241, 220);
pub const BAD: Color32 = Color32::from_rgb(150, 40, 25);
pub const GOOD: Color32 = Color32::from_rgb(55, 100, 40);

/// Tamaño base de letra (Garamond tiene la x baja: se usa un poco más grande).
pub const BODY: f32 = 18.0;

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("garamond".into(), FontData::from_static(include_bytes!("../../assets/fonts/EBGaramond12-Regular.ttf")));
    // Garamond primero; las fuentes por omisión de egui quedan de respaldo (símbolos).
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "garamond".into());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(BODY * 0.8)),
        (TextStyle::Body, FontId::proportional(BODY)),
        (TextStyle::Button, FontId::proportional(BODY)),
        (TextStyle::Heading, FontId::proportional(BODY * 1.35)),
        (TextStyle::Monospace, FontId::monospace(BODY * 0.8)),
    ]
    .into();
    style.spacing.button_padding = egui::vec2(9.0, 4.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(10.0);
    let v = &mut style.visuals;
    *v = egui::Visuals::light();
    v.override_text_color = Some(INK);
    v.panel_fill = PAPER;
    v.window_fill = PAPER;
    v.window_stroke = Stroke::new(2.0_f32, BORDER);
    v.window_rounding = Rounding::same(4.0);
    v.window_shadow = egui::epaint::Shadow { offset: egui::vec2(2.0, 3.0), blur: 8.0, spread: 0.0, color: Color32::from_black_alpha(60) };
    v.popup_shadow = v.window_shadow;
    v.extreme_bg_color = PAPER_LIGHT;
    v.faint_bg_color = PAPER_LIGHT;
    v.selection.bg_fill = PRESSED;
    v.selection.stroke = Stroke::new(1.0_f32, PRESSED_TEXT);
    let w = &mut v.widgets;
    for (st, bg) in [(&mut w.inactive, PAPER_DARK), (&mut w.hovered, PAPER_LIGHT), (&mut w.active, PRESSED), (&mut w.open, PAPER_DARK)] {
        st.bg_fill = bg;
        st.weak_bg_fill = bg;
        st.bg_stroke = Stroke::new(1.0_f32, BORDER);
        st.fg_stroke = Stroke::new(1.5_f32, INK);
        st.rounding = Rounding::same(3.0);
    }
    w.noninteractive.bg_fill = PAPER;
    w.noninteractive.weak_bg_fill = PAPER;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, INK);
    ctx.set_style(style);
}

/// Marco de papel para los paneles flotantes.
pub fn paper_frame() -> egui::Frame {
    egui::Frame::none().fill(PAPER).stroke(Stroke::new(2.0_f32, BORDER)).rounding(4.0).inner_margin(egui::Margin::symmetric(12.0, 8.0)).shadow(egui::epaint::Shadow {
        offset: egui::vec2(2.0, 3.0),
        blur: 8.0,
        spread: 0.0,
        color: Color32::from_black_alpha(55),
    })
}
