//! Íconos de la interfaz dibujados con trazos (sin imágenes de terceros): líneas de tinta
//! sobre papel, al estilo de los signos de un plano antiguo.

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    // Categorías
    Calles,
    Zonas,
    Servicios,
    Produccion,
    Puerto,
    Ordenanzas,
    Capas,
    Demoler,
    // Herramientas y tarjetas
    Plaza,
    Recta,
    Curva,
    Damero,
    Mejorar,
    QuitarZona,
    Quintas,
    Aljibe,
    Sereno,
    Bomba,
    Lazareto,
    Iglesia,
    Atahona,
    Cantera,
    Saladero,
    Muralla,
    Aduana,
    Presupuesto,
    Lupa,
    Valor,
    Agua,
    Opinion,
    Ladrillo,
    Piedra,
    Pan,
    Vecinos,
    Comercio,
    Hacienda,
}

/// Dibuja el ícono dentro de `r` con el color `c`.
pub fn paint(p: &Painter, r: Rect, icon: Icon, c: Color32) {
    let s = r.width().min(r.height());
    let o = r.center();
    // Coordenadas en una grilla de -1..1 alrededor del centro.
    let at = |x: f32, y: f32| -> Pos2 { pos2(o.x + x * s * 0.5, o.y + y * s * 0.5) };
    let w = (s / 14.0).max(1.2);
    let st = Stroke::new(w, c);
    let thin = Stroke::new(w * 0.6, c);
    let line = |a: (f32, f32), b: (f32, f32)| p.line_segment([at(a.0, a.1), at(b.0, b.1)], st);
    let poly = |pts: &[(f32, f32)], closed: bool, stroke: Stroke| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| at(x, y)).collect();
        if closed {
            p.add(Shape::closed_line(v, stroke));
        } else {
            p.add(Shape::line(v, stroke));
        }
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| p.rect_stroke(Rect::from_two_pos(at(x0, y0), at(x1, y1)), 1.0, st);
    let circle = |x: f32, y: f32, rad: f32| p.circle_stroke(at(x, y), rad * s * 0.5, st);
    let dot = |x: f32, y: f32, rad: f32| p.circle_filled(at(x, y), rad * s * 0.5, c);
    let arc = |cx: f32, cy: f32, rad: f32, a0: f32, a1: f32, stroke: Stroke| {
        let n = 16;
        let v: Vec<Pos2> = (0..=n).map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            at(cx + rad * a.cos(), cy + rad * a.sin())
        }).collect();
        p.add(Shape::line(v, stroke));
    };
    let house = |x: f32, y: f32, k: f32| {
        poly(&[(x - 0.4 * k, y + 0.4 * k), (x - 0.4 * k, y - 0.1 * k), (x, y - 0.45 * k), (x + 0.4 * k, y - 0.1 * k), (x + 0.4 * k, y + 0.4 * k)], true, st);
    };
    use std::f32::consts::PI;
    match icon {
        Icon::Calles | Icon::Recta => {
            line((-0.75, 0.8), (-0.25, -0.8));
            line((0.75, 0.8), (0.25, -0.8));
            for k in 0..3 {
                let y = 0.55 - k as f32 * 0.55;
                line((0.0, y), (0.0, y - 0.25));
            }
        }
        Icon::Curva => {
            arc(0.85, 0.85, 1.45, PI, PI * 1.5, st);
            arc(0.85, 0.85, 0.75, PI, PI * 1.5, st);
        }
        Icon::Damero => {
            rect(-0.8, -0.8, 0.8, 0.8);
            line((-0.8, -0.27), (0.8, -0.27));
            line((-0.8, 0.27), (0.8, 0.27));
            line((-0.27, -0.8), (-0.27, 0.8));
            line((0.27, -0.8), (0.27, 0.8));
        }
        Icon::Mejorar => {
            for (x, y) in [(-0.45, -0.3), (0.15, -0.3), (-0.15, 0.25), (0.45, 0.25)] {
                p.rect_stroke(Rect::from_center_size(at(x, y), vec2(0.5 * s * 0.5, 0.4 * s * 0.5)), 1.0, thin);
            }
            line((-0.8, 0.75), (0.8, 0.75));
            line((0.55, -0.85), (0.55, -0.45));
            line((0.35, -0.65), (0.75, -0.65));
        }
        Icon::Zonas => {
            rect(-0.8, -0.8, 0.8, 0.8);
            for d in [-0.8, -0.27, 0.27] {
                line((d, 0.8), (0.8, d));
                line((-0.8, -d), (-d, -0.8));
            }
        }
        Icon::QuitarZona => {
            rect(-0.8, -0.8, 0.8, 0.8);
            line((-0.5, -0.5), (0.5, 0.5));
            line((0.5, -0.5), (-0.5, 0.5));
        }
        Icon::Quintas => {
            rect(-0.8, -0.6, 0.8, 0.8);
            for x in [-0.45, 0.0, 0.45] {
                circle(x, 0.1, 0.17);
                line((x, 0.27), (x, 0.55));
            }
        }
        Icon::Servicios | Icon::Aljibe => {
            poly(&[(-0.6, -0.2), (-0.6, 0.75), (0.6, 0.75), (0.6, -0.2)], false, st);
            line((-0.75, -0.2), (0.75, -0.2));
            line((-0.45, -0.2), (-0.45, -0.75));
            line((0.45, -0.2), (0.45, -0.75));
            line((-0.6, -0.75), (0.6, -0.75));
            arc(0.0, 0.35, 0.25, PI, 2.0 * PI, thin);
        }
        Icon::Sereno => {
            rect(-0.35, -0.4, 0.35, 0.45);
            poly(&[(-0.45, -0.4), (0.0, -0.8), (0.45, -0.4)], false, st);
            dot(0.0, 0.05, 0.16);
            line((-0.2, 0.45), (-0.25, 0.8));
            line((0.2, 0.45), (0.25, 0.8));
        }
        Icon::Bomba => {
            poly(&[(0.0, -0.85), (0.45, -0.15), (0.35, 0.45), (0.0, 0.8), (-0.35, 0.45), (-0.45, -0.15)], true, st);
            poly(&[(0.0, -0.1), (0.2, 0.3), (0.0, 0.55), (-0.2, 0.3)], true, thin);
        }
        Icon::Lazareto => {
            rect(-0.7, -0.4, 0.7, 0.75);
            line((0.0, -0.85), (0.0, -0.45));
            line((-0.2, -0.65), (0.2, -0.65));
            line((-0.25, 0.15), (0.25, 0.15));
            line((0.0, -0.1), (0.0, 0.4));
        }
        Icon::Iglesia => {
            rect(-0.7, -0.1, 0.7, 0.8);
            poly(&[(-0.25, -0.1), (-0.25, -0.55), (0.25, -0.55), (0.25, -0.1)], false, st);
            line((0.0, -0.55), (0.0, -0.9));
            line((-0.15, -0.75), (0.15, -0.75));
            arc(0.0, 0.8, 0.22, PI, 2.0 * PI, thin);
        }
        Icon::Produccion => {
            poly(&[(-0.8, 0.75), (-0.8, -0.05), (-0.3, -0.35), (-0.3, -0.05), (0.2, -0.35), (0.2, -0.05), (0.5, -0.05), (0.5, -0.8), (0.75, -0.8), (0.75, 0.75)], true, st);
        }
        Icon::Atahona => {
            circle(0.0, 0.0, 0.7);
            circle(0.0, 0.0, 0.18);
            for k in 0..6 {
                let a = k as f32 * PI / 3.0;
                line((0.18 * a.cos(), 0.18 * a.sin()), (0.7 * a.cos(), 0.7 * a.sin()));
            }
        }
        Icon::Cantera | Icon::Piedra => {
            poly(&[(-0.8, 0.6), (-0.55, -0.1), (-0.1, -0.25), (0.05, 0.6)], true, st);
            poly(&[(0.05, 0.6), (0.2, -0.05), (0.6, -0.5), (0.8, 0.6)], true, st);
        }
        Icon::Saladero => {
            poly(&[(-0.8, 0.0), (-0.4, -0.45), (0.5, -0.45), (0.8, -0.1), (0.5, 0.35), (-0.4, 0.35)], true, st);
            line((-0.35, 0.35), (-0.35, 0.8));
            line((0.4, 0.35), (0.4, 0.8));
            dot(0.6, -0.2, 0.06);
        }
        Icon::Puerto => {
            circle(0.0, -0.65, 0.15);
            line((0.0, -0.5), (0.0, 0.8));
            line((-0.35, -0.25), (0.35, -0.25));
            arc(0.0, 0.15, 0.65, 0.15 * PI, 0.85 * PI, st);
        }
        Icon::Muralla => {
            poly(&[(-0.85, 0.75), (-0.85, -0.35), (-0.55, -0.35), (-0.55, -0.6), (-0.25, -0.6), (-0.25, -0.35), (0.25, -0.35), (0.25, -0.6), (0.55, -0.6), (0.55, -0.35), (0.85, -0.35), (0.85, 0.75)], true, st);
            arc(0.0, 0.75, 0.3, PI, 2.0 * PI, st);
        }
        Icon::Aduana => {
            house(0.0, 0.2, 1.4);
            line((-0.25, 0.1), (0.25, 0.1));
            line((0.0, -0.05), (0.0, 0.5));
        }
        Icon::Ordenanzas => {
            rect(-0.55, -0.75, 0.55, 0.75);
            for k in 0..4 {
                let y = -0.45 + k as f32 * 0.28;
                line((-0.35, y), (0.35, y));
            }
            dot(0.3, 0.55, 0.12);
        }
        Icon::Presupuesto => {
            circle(0.0, 0.0, 0.7);
            line((0.0, -0.5), (0.0, 0.5));
            arc(0.0, -0.12, 0.25, 0.0, PI * 1.5, thin);
            arc(0.0, 0.14, 0.25, PI, PI * 2.5, thin);
        }
        Icon::Capas => {
            for k in 0..3 {
                let y = -0.4 + k as f32 * 0.4;
                poly(&[(-0.8, y), (0.0, y - 0.3), (0.8, y), (0.0, y + 0.3)], true, if k == 0 { st } else { thin });
            }
        }
        Icon::Lupa => {
            circle(-0.15, -0.15, 0.5);
            line((0.22, 0.22), (0.75, 0.75));
        }
        Icon::Valor => {
            line((-0.8, 0.75), (0.8, 0.75));
            for (k, h) in [0.4, 0.8, 1.2].iter().enumerate() {
                let x = -0.5 + k as f32 * 0.5;
                rect(x - 0.15, 0.75 - h, x + 0.15, 0.75);
            }
        }
        Icon::Agua => {
            poly(&[(0.0, -0.8), (0.45, 0.05), (0.45, 0.3), (0.2, 0.7), (-0.2, 0.7), (-0.45, 0.3), (-0.45, 0.05)], true, st);
        }
        Icon::Opinion | Icon::Vecinos => {
            circle(0.0, -0.4, 0.3);
            arc(0.0, 0.8, 0.65, PI, 2.0 * PI, st);
        }
        Icon::Comercio => {
            line((-0.7, -0.5), (0.7, -0.5));
            line((0.0, -0.75), (0.0, 0.7));
            line((-0.35, 0.7), (0.35, 0.7));
            poly(&[(-0.7, -0.5), (-0.95, 0.1), (-0.45, 0.1)], true, thin);
            poly(&[(0.7, -0.5), (0.45, 0.1), (0.95, 0.1)], true, thin);
        }
        Icon::Hacienda => {
            poly(&[(-0.7, 0.2), (-0.4, -0.25), (0.5, -0.25), (0.75, 0.0), (0.5, 0.25), (-0.4, 0.25)], true, st);
            line((-0.4, 0.25), (-0.4, 0.7));
            line((0.45, 0.25), (0.45, 0.7));
            line((0.55, -0.25), (0.7, -0.55));
        }
        Icon::Ladrillo => {
            rect(-0.8, -0.35, 0.8, 0.35);
            line((-0.8, 0.0), (0.8, 0.0));
            line((0.0, -0.35), (0.0, 0.0));
            line((-0.4, 0.0), (-0.4, 0.35));
            line((0.4, 0.0), (0.4, 0.35));
        }
        Icon::Pan => {
            arc(0.0, 0.3, 0.75, PI, 2.0 * PI, st);
            line((-0.75, 0.3), (0.75, 0.3));
            line((-0.3, -0.25), (-0.15, 0.0));
            line((0.1, -0.35), (0.25, -0.1));
        }
        Icon::Plaza => {
            rect(-0.8, -0.8, 0.8, 0.8);
            rect(-0.45, -0.45, 0.45, 0.45);
            dot(0.0, 0.0, 0.1);
        }
        Icon::Demoler => {
            line((-0.7, 0.75), (0.35, -0.3));
            poly(&[(0.05, -0.75), (0.75, -0.05)], false, st);
            arc(0.4, -0.4, 0.45, PI * 0.75, PI * 1.75, st);
        }
    }
}

/// Ícono como widget: reserva un cuadrado del tamaño dado y lo dibuja.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, c: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    paint(ui.painter(), r, icon, c);
    resp
}
