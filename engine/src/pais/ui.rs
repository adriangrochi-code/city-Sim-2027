//! Modo «Mapa del país»: el Uruguay entero como un mapa estilizado (no en vóxeles).
//!
//! Por qué un mapa y no el motor de vóxeles: a escala país una celda mide ~1 km; un vóxel por
//! celda serían cubos de un kilómetro, y el mundo en streaming del motor (niveles de detalle,
//! columnas, sombras) está atado al marco de la ciudad (24 km más el entorno). En cambio el
//! jugador necesita leer el país: pueblos, caminos, pasos, flujos de carretas, departamentos y
//! recursos, con rótulos y capas, como en el plano de un agrimensor. El relieve se pinta «en
//! terrazas» (escalones de 15 m con su luz y su sombra, como vóxeles vistos desde arriba) para
//! que conserve la estética del juego, y la textura se arma una sola vez por modo y época.

use super::datos::{self, geo};
use super::{nivel_epoca, permitido, Accion, Obra, Pais, TipoEvento, NIVELES};
use crate::city::game::{fmt_thousands, Game};
use crate::ui::theme;
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke};

/// Herramientas del mapa del país.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Herr {
    Pueblo,
    Estancia,
    Saladero,
    Camino,
    Mejorar,
    Balsa,
    Puente,
    Puerto,
}

impl Herr {
    pub const ALL: [Herr; 8] = [Herr::Estancia, Herr::Saladero, Herr::Pueblo, Herr::Camino, Herr::Mejorar, Herr::Balsa, Herr::Puente, Herr::Puerto];

    pub fn nombre(self) -> &'static str {
        match self {
            Herr::Pueblo => "Fundar pueblo",
            Herr::Estancia => "Otorgar estancia",
            Herr::Saladero => "Saladero",
            Herr::Camino => "Trazar camino",
            Herr::Mejorar => "Mejorar camino",
            Herr::Balsa => "Balsa en un paso",
            Herr::Puente => "Puente en un paso",
            Herr::Puerto => "Puerto fluvial",
        }
    }

    pub fn ayuda(self) -> &'static str {
        match self {
            Herr::Pueblo => "Clic en tierra firme, a más de 15 km de otro pueblo",
            Herr::Estancia => "Clic en buena pradera (no bañado ni sierra), a más de 6 km de otra estancia",
            Herr::Saladero => "Clic junto a un río o la costa: compra el ganado de las estancias cercanas",
            Herr::Camino => "Clic en un pueblo y después en otro: el camino busca las cuchillas y los pasos angostos",
            Herr::Mejorar => "Clic en un camino: más rápido para las carretas",
            Herr::Balsa => "Clic en un paso de río (círculo azul): las crecientes ya no lo cortan",
            Herr::Puente => "Clic en un paso de río: sin demoras ni con el río crecido",
            Herr::Puerto => "Clic en un pueblo sobre un río navegable o la costa",
        }
    }

    /// Acción de ejemplo para preguntar si la época la permite.
    fn ejemplo(self) -> Accion {
        match self {
            Herr::Pueblo => Accion::FundarPueblo([0.0; 2]),
            Herr::Estancia => Accion::Estancia([0.0; 2]),
            Herr::Saladero => Accion::Saladero([0.0; 2]),
            Herr::Camino => Accion::Camino(String::new(), String::new()),
            Herr::Mejorar => Accion::MejorarCamino(0),
            Herr::Balsa => Accion::Paso(0, 0, Obra::Balsa),
            Herr::Puente => Accion::Paso(0, 0, Obra::Puente),
            Herr::Puerto => Accion::Puerto(String::new()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Sel {
    Pueblo(String),
    Estancia(u32),
    Saladero(u32),
    Camino(u32),
    Celda([f32; 2]),
}

/// Capas visibles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capas {
    pub departamentos: bool,
    pub recursos: bool,
    pub cobertura: bool,
    /// [atlas] Ecosistemas del atlas (capa del período del año).
    pub ecosistemas: bool,
    pub pueblos: bool,
    pub caminos: bool,
    pub ferrocarril: bool,
    pub flujos: bool,
    pub rotulos: bool,
}

impl Default for Capas {
    fn default() -> Self {
        Capas { departamentos: false, recursos: false, cobertura: true, ecosistemas: false, pueblos: true, caminos: true, ferrocarril: true, flujos: true, rotulos: true }
    }
}

pub struct PaisUi {
    pub abierto: bool,
    /// Centro de la vista (celdas) y escala (puntos por celda).
    pub centro: [f32; 2],
    pub zoom: f32,
    pub capas: Capas,
    pub herr: Option<Herr>,
    pub camino_desde: Option<String>,
    pub sel: Option<Sel>,
    pub hover: Option<[f32; 2]>,
    textura: Option<((bool, bool, u8, u8), egui::TextureHandle)>,
    /// La vista se ajusta al país la primera vez que se abre.
    ajustada: bool,
}

impl Default for PaisUi {
    fn default() -> Self {
        PaisUi { abierto: false, centro: [255.0, 278.0], zoom: 1.5, capas: Capas::default(), herr: None, camino_desde: None, sel: None, hover: None, textura: None, ajustada: false }
    }
}

const PANEL_W: f32 = 330.0;

impl PaisUi {
    pub fn alternar(&mut self) {
        self.abierto = !self.abierto;
        self.herr = None;
        self.camino_desde = None;
    }

    /// Esc: suelta la herramienta; sin herramienta, vuelve a la ciudad.
    pub fn escape(&mut self) {
        if self.herr.is_some() || self.camino_desde.is_some() {
            self.herr = None;
            self.camino_desde = None;
        } else if self.sel.is_some() {
            self.sel = None;
        } else {
            self.abierto = false;
        }
    }

    /// Centra la vista en un punto con una escala dada.
    pub fn mirar(&mut self, centro: [f32; 2], zoom: f32) {
        self.centro = centro;
        self.zoom = zoom;
        self.ajustada = true;
    }

    /// Botón «Mapa del país» sobre la vista de la ciudad.
    pub fn boton(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("boton-mapa-pais"))
            .anchor(Align2::RIGHT_TOP, vec2(-12.0, crate::ui::TOP_BAR_H + 8.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                theme::paper_frame().inner_margin(egui::Margin::symmetric(6.0, 4.0)).show(ui, |ui| {
                    if ui.button("Mapa del país").on_hover_text("El Uruguay entero: pueblos, caminos, estancias y saladeros · atajo: M").clicked() {
                        self.alternar();
                    }
                });
            });
    }

    /// Dibuja el mapa del país. Devuelve las acciones del jugador.
    pub fn draw(&mut self, ctx: &egui::Context, game: &Game) -> Vec<Accion> {
        let mut acts = Vec::new();
        let p = &game.pais;
        let anio = p.anio();
        let epoca = p.epoca();
        // Barra superior.
        egui::TopBottomPanel::top("pais-barra").exact_height(crate::ui::TOP_BAR_H).frame(egui::Frame::none().fill(theme::PAPER).stroke(st(1.5, theme::BORDER)).inner_margin(egui::Margin::symmetric(12.0, 6.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new("Mapa del país").size(theme::BODY * 1.25));
                ui.separator();
                ui.label(format!("{} · {}", game.city.date(), epoca.nombre()));
                ui.separator();
                ui.label(format!("{} pesos", fmt_thousands(game.city.money)));
                ui.separator();
                ui.label(RichText::new(&game.message).small().color(theme::INK_SOFT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Volver a la ciudad").on_hover_text("Atajo: M o Esc").clicked() {
                        self.abierto = false;
                    }
                });
            });
        });
        egui::SidePanel::right("pais-panel").exact_width(PANEL_W).resizable(false).frame(egui::Frame::none().fill(theme::PAPER).stroke(st(1.5, theme::BORDER)).inner_margin(egui::Margin::same(10.0))).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.panel(ui, game, &mut acts));
        });
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_rgb(52, 78, 110))).show(ctx, |ui| {
            let rect = ui.max_rect();
            if !self.ajustada {
                let g = geo();
                self.zoom = (rect.height() / g.alto() as f32).min(rect.width() / g.ancho() as f32) * 0.96;
                self.centro = [g.ancho() as f32 * 0.5, g.alto() as f32 * 0.5];
                self.ajustada = true;
            }
            let resp = ui.allocate_rect(rect, Sense::click_and_drag());
            self.entrada(ui, &resp, rect);
            let painter = ui.painter_at(rect);
            self.mapa(ctx, &painter, rect, p, anio);
            if let Some(a) = self.clic(&resp, rect, p) {
                acts.push(a);
            }
            // Leyenda de la herramienta.
            if let Some(h) = self.herr {
                let t = match (h, &self.camino_desde) {
                    (Herr::Camino, Some(a)) => format!("Trazar camino desde {}: clic en el otro pueblo", p.pueblo(a).map_or(a.as_str(), |x| x.nombre.as_str())),
                    _ => h.ayuda().to_string(),
                };
                let r = painter.text(rect.left_bottom() + vec2(14.0, -14.0), Align2::LEFT_BOTTOM, &t, FontId::proportional(16.0), theme::INK);
                painter.rect_filled(r.expand(6.0), 3.0, theme::PAPER);
                painter.text(rect.left_bottom() + vec2(14.0, -14.0), Align2::LEFT_BOTTOM, t, FontId::proportional(16.0), theme::INK);
            }
            let _ = nivel_epoca(epoca);
        });
        acts
    }

    fn a_pantalla(&self, rect: Rect, c: [f32; 2]) -> Pos2 {
        rect.center() + vec2((c[0] - self.centro[0]) * self.zoom, (c[1] - self.centro[1]) * self.zoom)
    }

    fn a_celda(&self, rect: Rect, s: Pos2) -> [f32; 2] {
        let d = s - rect.center();
        [self.centro[0] + d.x / self.zoom, self.centro[1] + d.y / self.zoom]
    }

    /// Arrastrar, rueda y teclado.
    fn entrada(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: Rect) {
        let g = geo();
        let arrastra = resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) || (self.herr.is_none() && resp.dragged_by(egui::PointerButton::Primary));
        if arrastra {
            let d = resp.drag_delta();
            self.centro[0] -= d.x / self.zoom;
            self.centro[1] -= d.y / self.zoom;
        }
        if let Some(h) = resp.hover_pos() {
            self.hover = Some(self.a_celda(rect, h));
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let antes = self.a_celda(rect, h);
                self.zoom = (self.zoom * (scroll / 200.0).exp()).clamp(0.8, 60.0);
                let despues = self.a_celda(rect, h);
                self.centro[0] += antes[0] - despues[0];
                self.centro[1] += antes[1] - despues[1];
            }
        } else {
            self.hover = None;
        }
        let (dt, teclas) = ui.input(|i| {
            let k = |k: egui::Key| i.key_down(k) as i32 as f32;
            (i.stable_dt.min(0.1), [k(egui::Key::D) + k(egui::Key::ArrowRight) - k(egui::Key::A) - k(egui::Key::ArrowLeft), k(egui::Key::S) + k(egui::Key::ArrowDown) - k(egui::Key::W) - k(egui::Key::ArrowUp), k(egui::Key::Plus) + k(egui::Key::Equals) - k(egui::Key::Minus)])
        });
        let v = 500.0 / self.zoom * dt;
        self.centro[0] += teclas[0] * v;
        self.centro[1] += teclas[1] * v;
        self.zoom = (self.zoom * (teclas[2] * dt * 2.0).exp()).clamp(0.8, 60.0);
        self.centro[0] = self.centro[0].clamp(0.0, g.ancho() as f32);
        self.centro[1] = self.centro[1].clamp(0.0, g.alto() as f32);
        if teclas.iter().any(|t| *t != 0.0) {
            ui.ctx().request_repaint();
        }
    }

    /// Lo que hay cerca de un punto de la pantalla.
    fn elegir(&self, rect: Rect, s: Pos2, p: &Pais) -> Option<Sel> {
        let anio = p.anio();
        let d = |c: [f32; 2]| (self.a_pantalla(rect, c) - s).length();
        if let Some(q) = p.pueblos.iter().filter(|q| q.anio <= anio).min_by(|a, b| d(a.pos).partial_cmp(&d(b.pos)).unwrap()).filter(|q| d(q.pos) < 12.0) {
            return Some(Sel::Pueblo(q.id.clone()));
        }
        if let Some(x) = p.saladeros.iter().min_by(|a, b| d(a.pos).partial_cmp(&d(b.pos)).unwrap()).filter(|x| d(x.pos) < 8.0) {
            return Some(Sel::Saladero(x.id));
        }
        if let Some(x) = p.estancias.iter().min_by(|a, b| d(a.pos).partial_cmp(&d(b.pos)).unwrap()).filter(|x| d(x.pos) < 7.0) {
            return Some(Sel::Estancia(x.id));
        }
        let mut mejor: Option<(f32, u32)> = None;
        for c in &p.caminos {
            for w in c.puntos.windows(2) {
                let (a, b) = (self.a_pantalla(rect, w[0]), self.a_pantalla(rect, w[1]));
                let dd = datos::dist_seg([s.x, s.y], [a.x, a.y], [b.x, b.y]);
                if dd < 6.0 && mejor.map_or(true, |m| dd < m.0) {
                    mejor = Some((dd, c.id));
                }
            }
        }
        if let Some((_, id)) = mejor {
            return Some(Sel::Camino(id));
        }
        Some(Sel::Celda(self.a_celda(rect, s)))
    }

    fn clic(&mut self, resp: &egui::Response, rect: Rect, p: &Pais) -> Option<Accion> {
        if !resp.clicked() {
            return None;
        }
        let s = resp.interact_pointer_pos()?;
        let celda = self.a_celda(rect, s);
        let elegido = self.elegir(rect, s, p);
        match self.herr {
            None => {
                self.sel = elegido;
                None
            }
            Some(Herr::Pueblo) => Some(Accion::FundarPueblo(celda)),
            Some(Herr::Estancia) => Some(Accion::Estancia(celda)),
            Some(Herr::Saladero) => Some(Accion::Saladero(celda)),
            Some(Herr::Camino) => match elegido {
                Some(Sel::Pueblo(id)) => match self.camino_desde.take() {
                    Some(a) if a != id => Some(Accion::Camino(a, id)),
                    _ => {
                        self.camino_desde = Some(id);
                        None
                    }
                },
                _ => None,
            },
            Some(Herr::Mejorar) => match elegido {
                Some(Sel::Camino(id)) => Some(Accion::MejorarCamino(id)),
                _ => None,
            },
            Some(Herr::Balsa) | Some(Herr::Puente) => {
                let obra = if self.herr == Some(Herr::Puente) { Obra::Puente } else { Obra::Balsa };
                let mut mejor: Option<(f32, u32, usize)> = None;
                for c in &p.caminos {
                    for (k, ps) in c.pasos.iter().enumerate() {
                        let dd = (self.a_pantalla(rect, ps.pos) - s).length();
                        if dd < 10.0 && mejor.map_or(true, |m| dd < m.0) {
                            mejor = Some((dd, c.id, k));
                        }
                    }
                }
                mejor.map(|(_, id, k)| Accion::Paso(id, k, obra))
            }
            Some(Herr::Puerto) => match elegido {
                Some(Sel::Pueblo(id)) => Some(Accion::Puerto(id)),
                _ => None,
            },
        }
    }

    /// Textura del relieve en terrazas (o de los recursos), por modo y por qué embalses hay.
    fn textura(&mut self, ctx: &egui::Context, anio: i32) -> egui::TextureId {
        let g = geo();
        let emb = g.embalses.iter().enumerate().fold(0u8, |m, (k, e)| if anio >= e.anio { m | (1 << k) } else { m });
        // [atlas] La capa de ecosistemas cambia con el período (255: apagada).
        let eco = if self.capas.ecosistemas { g.eco.capa_de(anio) as u8 } else { 255 };
        let clave = (self.capas.recursos, self.capas.cobertura, emb, eco);
        if let Some((k, t)) = &self.textura {
            if *k == clave {
                return t.id();
            }
        }
        let img = imagen_capas(anio, self.capas.recursos, self.capas.cobertura, self.capas.ecosistemas);
        let t = ctx.load_texture("mapa-pais", img, egui::TextureOptions::LINEAR);
        let id = t.id();
        self.textura = Some((clave, t));
        id
    }

    fn mapa(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, p: &Pais, anio: i32) {
        let g = geo();
        let tex = self.textura(ctx, anio);
        let r = Rect::from_min_max(self.a_pantalla(rect, [-0.5, -0.5]), self.a_pantalla(rect, [g.ancho() as f32 - 0.5, g.alto() as f32 - 0.5]));
        painter.image(tex, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let z = self.zoom;
        let pts = |v: &[[f32; 2]]| v.iter().map(|c| self.a_pantalla(rect, *c)).collect::<Vec<_>>();
        let tinta = Color32::from_rgb(70, 48, 30);
        // Eventos: sequías (mancha ocre) y crecientes (río en rojo).
        for e in &p.eventos {
            match e.tipo {
                TipoEvento::Sequia => {
                    painter.circle(self.a_pantalla(rect, e.centro), e.radio / g.g.km_x * z, Color32::from_rgba_unmultiplied(200, 140, 40, 45), st(1.5, Color32::from_rgba_unmultiplied(170, 100, 20, 160)));
                }
                TipoEvento::Creciente => {
                    if let Some(rio) = g.rios.iter().find(|r| r.nombre == e.zona) {
                        for l in &rio.lineas {
                            painter.add(Shape::line(pts(l), st((z * 0.6).clamp(2.0, 6.0), Color32::from_rgba_unmultiplied(190, 40, 30, 170))));
                        }
                    }
                }
            }
        }
        // Departamentos.
        if self.capas.departamentos {
            for d in &g.departamentos {
                for c in &d.contorno {
                    let mut v = pts(c);
                    if let Some(f) = v.first().copied() {
                        v.push(f);
                    }
                    painter.extend(Shape::dashed_line(&v, st(2.4, Color32::from_rgb(120, 30, 70)), 7.0, 4.0));
                }
                let creado = g.historia.departamentos.iter().find(|h| h.nombre == d.nombre).and_then(|h| h.creado);
                let txt = match creado {
                    Some(y) if y > anio => format!("({})", d.nombre),
                    _ => d.nombre.to_uppercase(),
                };
                let (q, f) = (self.a_pantalla(rect, d.centro), FontId::proportional((z * 7.0).clamp(12.0, 22.0)));
                for o in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
                    painter.text(q + o, Align2::CENTER_CENTER, &txt, f.clone(), Color32::from_rgba_unmultiplied(250, 245, 230, 220));
                }
                painter.text(q, Align2::CENTER_CENTER, txt, f, Color32::from_rgb(120, 30, 70));
            }
        }
        // [ciudad] Islas con su contorno real (islas.json: Flores, Gorriti, Lobos, las Ratas,
        // San Gabriel, Farallón): a 1 km casi no ocupan una celda del relieve, así que se dibujan
        // encima, con su nombre al acercarse.
        for (nombre, partes) in crate::islas::contornos_geo() {
            let mut centro = None;
            for parte in partes {
                let v: Vec<Pos2> = parte.iter().map(|q| self.a_pantalla(rect, g.celda(q[0], q[1]))).collect();
                centro.get_or_insert(v[0]);
                painter.add(Shape::convex_polygon(v, Color32::from_rgb(150, 160, 104), st(1.0, Color32::from_rgb(90, 84, 70))));
            }
            if let (Some(c), true) = (centro, self.capas.rotulos && z > 4.0) {
                painter.text(c + vec2(0.0, 4.0), Align2::CENTER_TOP, nombre, FontId::proportional(11.0), Color32::from_rgb(220, 235, 250));
            }
        }
        // Rótulos de ríos y lagunas.
        if self.capas.rotulos && z > 2.5 {
            for r in g.rios.iter().filter(|r| r.largo_km > 120.0) {
                if let Some(l) = r.lineas.iter().max_by_key(|l| l.len()) {
                    let m = l[l.len() / 2];
                    painter.text(self.a_pantalla(rect, m), Align2::CENTER_BOTTOM, &r.nombre, FontId::proportional(12.0), Color32::from_rgb(35, 70, 120));
                }
            }
        }
        if self.capas.rotulos {
            for l in g.lagunas.iter().filter(|l| l.km2 > 40.0) {
                painter.text(self.a_pantalla(rect, l.pos), Align2::CENTER_CENTER, &l.nombre, FontId::proportional(12.0), Color32::from_rgb(220, 235, 250));
            }
        }
        // Ríos con nombre (líneas, más anchas las navegables y al acercarse).
        for r in &g.rios {
            let w = ((if r.navegable { 0.35 } else { 0.22 }) * z).clamp(1.0, 4.5);
            for l in &r.lineas {
                painter.add(Shape::line(pts(l), st(w, Color32::from_rgb(62, 108, 165))));
            }
        }
        // Ferrocarril del año.
        if self.capas.ferrocarril {
            for v in g.vias.iter().filter(|v| v.anio <= anio) {
                let s = pts(&v.puntos);
                painter.add(Shape::line(s.clone(), st(3.0, Color32::from_rgb(30, 30, 30))));
                painter.extend(Shape::dashed_line(&s, st(1.4, Color32::from_rgb(235, 230, 215)), 4.0, 4.0));
            }
        }
        // Caminos y sus pasos.
        if self.capas.caminos {
            for c in &p.caminos {
                let s = pts(&c.puntos);
                let sel = self.sel == Some(Sel::Camino(c.id));
                let (w, col) = match c.nivel {
                    0 => (1.4f32, Color32::from_rgb(150, 110, 70)),
                    1 => (2.2, Color32::from_rgb(120, 72, 36)),
                    2 => (3.0, Color32::from_rgb(95, 50, 25)),
                    _ => (3.6, Color32::from_rgb(150, 40, 30)),
                };
                if sel {
                    painter.add(Shape::line(s.clone(), st(w + 4.0, Color32::from_rgba_unmultiplied(255, 230, 120, 200))));
                }
                if c.nivel == 0 {
                    painter.extend(Shape::dashed_line(&s, st(w, col), 5.0, 3.0));
                } else {
                    painter.add(Shape::line(s, st(w, col)));
                }
                if z > 2.0 {
                    for ps in &c.pasos {
                        let q = self.a_pantalla(rect, ps.pos);
                        let roto = ps.obra == Obra::Vado && p.crecido(&ps.rio);
                        let col = if roto { theme::BAD } else { Color32::from_rgb(40, 80, 150) };
                        match ps.obra {
                            Obra::Vado => {
                                painter.circle(q, 3.5, Color32::from_rgb(230, 240, 250), st(1.5, col));
                            }
                            Obra::Balsa => {
                                painter.rect(Rect::from_center_size(q, vec2(7.0, 7.0)), 1.0, Color32::from_rgb(200, 220, 240), st(1.5, col));
                            }
                            Obra::Puente => {
                                painter.rect_filled(Rect::from_center_size(q, vec2(9.0, 4.0)), 1.0, tinta);
                            }
                        }
                    }
                }
            }
        }
        // Flujos de carretas del último mes.
        if self.capas.flujos {
            for f in &p.flujos {
                let col = match f.destino {
                    1 => Color32::from_rgba_unmultiplied(200, 40, 30, 120),
                    2 => Color32::from_rgba_unmultiplied(40, 110, 200, 110),
                    3 => Color32::from_rgba_unmultiplied(60, 60, 60, 110),
                    _ => Color32::from_rgba_unmultiplied(230, 170, 40, 110),
                };
                let w = (f.carga.sqrt() * 0.9).clamp(1.0, 7.0);
                painter.add(Shape::line(pts(&f.puntos), st(w, col)));
            }
        }
        // Estancias y saladeros.
        for e in &p.estancias {
            let q = self.a_pantalla(rect, e.pos);
            if !rect.contains(q) {
                continue;
            }
            let s = (2.0 + (e.ganado / 1500.0).sqrt() * 1.5).min(6.0) * (z / 3.0).clamp(0.7, 1.6);
            let col = if e.jugador { Color32::from_rgb(160, 60, 30) } else { Color32::from_rgb(120, 85, 45) };
            painter.rect(Rect::from_center_size(q, vec2(s, s)), 0.0, col, st(0.8, Color32::from_rgb(250, 240, 220)));
            if self.sel == Some(Sel::Estancia(e.id)) {
                painter.circle_stroke(q, s + 4.0, st(2.0, Color32::YELLOW));
            }
        }
        for sd in &p.saladeros {
            let q = self.a_pantalla(rect, sd.pos);
            let s = 6.0 * (z / 3.0).clamp(0.8, 1.6);
            painter.add(Shape::convex_polygon(vec![q + vec2(0.0, -s), q + vec2(s, s * 0.8), q + vec2(-s, s * 0.8)], Color32::from_rgb(150, 30, 30), st(1.0, Color32::WHITE)));
            if self.sel == Some(Sel::Saladero(sd.id)) {
                painter.circle_stroke(q, s + 4.0, st(2.0, Color32::YELLOW));
            }
        }
        // El mapa de la ciudad (escala ciudad).
        let (a, b) = (self.a_pantalla(rect, g.ciudad.celda_no), self.a_pantalla(rect, g.ciudad.celda_se));
        let rr = Rect::from_two_pos(a, b);
        painter.rect_stroke(rr, 0.0, st(1.5, Color32::from_rgb(250, 240, 200)));
        if z > 6.0 {
            painter.text(rr.left_top() + vec2(3.0, 2.0), Align2::LEFT_TOP, "mapa de la ciudad (24 km)", FontId::proportional(12.0), Color32::from_rgb(250, 240, 200));
        }
        // Pueblos.
        if self.capas.pueblos {
            for q in &p.pueblos {
                let s = self.a_pantalla(rect, q.pos);
                let capital = q.tipo == "capital";
                let r = if capital { 7.0 } else { (2.5 + (q.poblacion / 150.0).sqrt() * 1.2).min(8.0) };
                let sel = self.sel == Some(Sel::Pueblo(q.id.clone())) || self.camino_desde.as_deref() == Some(q.id.as_str());
                if sel {
                    painter.circle_stroke(s, r + 4.0, st(2.5, Color32::YELLOW));
                }
                let fill = if q.tipo == "fuerte" { Color32::from_rgb(120, 120, 120) } else if q.jugador { Color32::from_rgb(255, 230, 170) } else { Color32::WHITE };
                painter.circle(s, r, fill, st(2.0, tinta));
                if capital {
                    painter.circle_filled(s, 3.0, tinta);
                }
                if p.puertos.iter().any(|x| x.pueblo == q.id && x.desde <= p.anio()) {
                    painter.text(s + vec2(-r - 2.0, 0.0), Align2::RIGHT_CENTER, "⚓", FontId::proportional(13.0), Color32::from_rgb(30, 60, 120));
                }
                let importante = capital || q.poblacion > 600.0 || matches!(q.tipo.as_str(), "ciudad" | "villa");
                if self.capas.rotulos && (z > 2.6 || importante) {
                    let t = if z > 4.0 { format!("{} · {}", q.nombre, fmt_thousands(q.poblacion as i64)) } else { q.nombre.clone() };
                    let pos = s + vec2(r + 3.0, 0.0);
                    let f = FontId::proportional(if capital { 15.0 } else { 13.0 });
                    for o in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
                        painter.text(pos + o, Align2::LEFT_CENTER, &t, f.clone(), Color32::from_rgba_unmultiplied(250, 245, 230, 200));
                    }
                    painter.text(pos, Align2::LEFT_CENTER, t, f, tinta);
                }
            }
        }
        // Escala gráfica.
        let km = [10.0, 25.0, 50.0, 100.0].into_iter().find(|k| k / g.g.km_x * z > 70.0).unwrap_or(100.0);
        let l = km / g.g.km_x * z;
        let o = rect.right_bottom() + vec2(-l - 20.0, -20.0);
        painter.rect_filled(Rect::from_min_size(o - vec2(6.0, 20.0), vec2(l + 12.0, 28.0)), 3.0, Color32::from_rgba_unmultiplied(240, 232, 212, 220));
        painter.line_segment([o, o + vec2(l, 0.0)], st(3.0, tinta));
        painter.text(o + vec2(l * 0.5, -3.0), Align2::CENTER_BOTTOM, format!("{km:.0} km"), FontId::proportional(13.0), tinta);
    }

    fn panel(&mut self, ui: &mut egui::Ui, game: &Game, acts: &mut Vec<Accion>) {
        let _ = acts;
        let p = &game.pais;
        let g = geo();
        let anio = p.anio();
        let epoca = p.epoca();
        ui.label(RichText::new("Uruguay").size(theme::BODY * 1.4));
        ui.label(RichText::new(match nivel_epoca(epoca) {
            0 => "Época colonial: el país vive solo (estancias, villas y caminos reales). El cabildo puede otorgar estancias, abrir saladeros, poner balsas y componer los caminos.",
            1 => "Ciudad Nueva: se funda el país. Pueblos, caminos con puentes y puertos del litoral.",
            _ => "El país conectado: ferrocarril, puertos del interior y caminos afirmados.",
        }).small().color(theme::INK_SOFT));
        ui.add_space(4.0);
        ui.label(RichText::new("Capas").strong());
        let c = &mut self.capas;
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut c.departamentos, "Departamentos");
            ui.checkbox(&mut c.recursos, "Recursos");
            ui.checkbox(&mut c.cobertura, "Monte y bañados");
            // [atlas]
            if ui.checkbox(&mut c.ecosistemas, "Ecosistemas").on_hover_text("Leyenda del Atlas ecológico e histórico del Uruguay, con la capa del período del año").changed() && c.ecosistemas {
                c.recursos = false;
            }
            ui.checkbox(&mut c.pueblos, "Pueblos");
            ui.checkbox(&mut c.caminos, "Caminos");
            ui.checkbox(&mut c.ferrocarril, "Ferrocarril");
            ui.checkbox(&mut c.flujos, "Carretas");
            ui.checkbox(&mut c.rotulos, "Rótulos");
        });
        if c.recursos && c.ecosistemas {
            c.ecosistemas = false;
        }
        if c.ecosistemas {
            leyenda_ecosistemas(ui, anio);
        }
        if c.recursos {
            // Leyenda de la capa de recursos (material principal de la unidad geológica).
            ui.horizontal_wrapped(|ui| {
                for (m, n) in [("basalto", "basalto"), ("granito", "granito"), ("gneis", "gneis"), ("arenisca", "arenisca y arena"), ("arcilla_roja", "arcilla roja"), ("arcilla_gris", "arcilla gris"), ("caliza", "caliza"), ("balasto", "balasto y tosca"), ("amatista", "ágata y amatista")] {
                    let k = color_material(m);
                    let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, Color32::from_rgb(k[0] as u8, k[1] as u8, k[2] as u8));
                    ui.label(RichText::new(n).small());
                }
            });
            ui.label(RichText::new("Puntos: títulos mineros (DINAMIGE). Geología 1:500.000; materiales ESTIMADOS por unidad.").small().color(theme::INK_SOFT));
        }
        ui.separator();
        ui.label(RichText::new("Obras del cabildo").strong());
        ui.horizontal_wrapped(|ui| {
            for h in Herr::ALL {
                let ok = permitido(&h.ejemplo(), epoca, anio);
                let activo = self.herr == Some(h);
                let b = ui.add_enabled(ok.is_ok(), egui::SelectableLabel::new(activo, h.nombre()));
                let b = match &ok {
                    Ok(()) => b.on_hover_text(format!("{} · {}", h.ayuda(), costo_txt(p, h))),
                    Err(e) => b.on_disabled_hover_text(e),
                };
                if b.clicked() {
                    self.herr = if activo { None } else { Some(h) };
                    self.camino_desde = None;
                }
            }
        });
        if let Some(h) = Herr::ALL.iter().find(|h| permitido(&h.ejemplo(), epoca, anio).is_err()) {
            let _ = h;
            ui.label(RichText::new("Algunas obras esperan a la época siguiente (pasa el cursor para ver por qué).").small().color(theme::INK_SOFT));
        }
        ui.separator();
        self.ficha(ui, game);
        ui.separator();
        let a = &p.ultimo;
        ui.label(RichText::new("El último mes").strong());
        ui.label(format!("{} carretas llegaron a Montevideo", a.carretas));
        ui.label(format!("Cueros {:.0} · sebo {:.0} · tasajo {:.0} arrobas · carne {:.0} raciones", a.cueros, a.sebo, a.tasajo, a.carne));
        ui.label(format!("Derechos {} y comercio {} pesos", a.derechos, a.comercio));
        if a.contrabando > 1.0 {
            ui.label(RichText::new(format!("Cueros de contrabando por la frontera: {:.0}", a.contrabando)).color(theme::BAD));
        }
        ui.label(RichText::new(format!(
            "{} pueblos · {} habitantes en el interior · {} estancias con {} cabezas · {} saladeros · {} km de caminos",
            p.pueblos.iter().filter(|q| q.tipo != "capital").count(),
            fmt_thousands(p.poblacion_interior() as i64),
            p.estancias.len(),
            fmt_thousands(p.ganado() as i64),
            p.saladeros.len(),
            fmt_thousands(p.caminos.iter().map(|c| c.km()).sum::<f32>() as i64)
        )).small());
        for e in &p.eventos {
            let t = match e.tipo {
                TipoEvento::Sequia => format!("Seca en {} ({} meses más)", e.zona, e.meses),
                TipoEvento::Creciente => format!("Creciente del {}: los vados no dan paso", e.zona),
            };
            ui.label(RichText::new(t).color(theme::BAD));
        }
        ui.separator();
        ui.label(RichText::new("Crónica del país").strong());
        for (m, t) in p.cronica.iter().rev().take(6) {
            ui.label(RichText::new(format!("{}: {t}", super::anio_de(*m))).small());
        }
        ui.separator();
        ui.label(RichText::new(&g.atribucion).size(11.0).color(theme::INK_SOFT));
    }

    /// Ficha de lo elegido (o de la celda bajo el cursor).
    fn ficha(&self, ui: &mut egui::Ui, game: &Game) {
        let p = &game.pais;
        let g = geo();
        let anio = p.anio();
        match &self.sel {
            Some(Sel::Pueblo(id)) => {
                let Some(q) = p.pueblo(id) else { return };
                ui.label(RichText::new(&q.nombre).strong());
                if q.tipo == "capital" {
                    ui.label(format!("La ciudad: {} vecinos. Se juega en la escala ciudad (botón «Volver a la ciudad»).", fmt_thousands(game.city.population() as i64)));
                } else {
                    ui.label(format!("{} · {} habitantes", q.tipo, fmt_thousands(q.poblacion as i64)));
                }
                if let Some(h) = g.pueblo_hist(id) {
                    ui.label(RichText::new(format!("Fundación: {} ({}). {}", h.anio, h.estado, h.nota)).small());
                    if let Some(f) = &h.fuente {
                        ui.label(RichText::new(format!("Fuente: {f}")).size(11.0).color(theme::INK_SOFT));
                    }
                } else if q.jugador {
                    ui.label(RichText::new(format!("Fundado por el cabildo en {}", q.anio)).small());
                }
                let red = super::rutas::Red::armar(p, anio);
                if let Some(k) = p.pueblos.iter().position(|x| x.id == *id) {
                    let d = red.dias_pueblo(k);
                    ui.label(if d.is_finite() { format!("A {d:.1} días de carreta del puerto") } else { "Sin camino hasta el puerto".to_string() });
                }
            }
            Some(Sel::Estancia(id)) => {
                let Some(e) = p.estancias.iter().find(|e| e.id == *id) else { return };
                ui.label(RichText::new(&e.nombre).strong());
                ui.label(format!("{} cabezas · pradera {:.0} % · desde {}", fmt_thousands(e.ganado as i64), g.pastura(e.pos[0], e.pos[1]) * 100.0, e.anio));
            }
            Some(Sel::Saladero(id)) => {
                let Some(s) = p.saladeros.iter().find(|e| e.id == *id) else { return };
                ui.label(RichText::new(&s.nombre).strong());
                ui.label(format!("Faenó {:.0} de {:.0} cabezas el último mes · desde {}", s.faena, s.capacidad, s.anio));
            }
            Some(Sel::Camino(id)) => {
                let Some(c) = p.caminos.iter().find(|c| c.id == *id) else { return };
                ui.label(RichText::new(&c.nombre).strong());
                ui.label(format!("{:.0} km · {} · {:.0} km por día · desde {}", c.km(), NIVELES[c.nivel as usize], c.velocidad(), c.anio));
                for ps in &c.pasos {
                    ui.label(RichText::new(format!("Paso del {} ({:.1} km): {}{}", ps.rio, ps.ancho, ps.obra.nombre(), if p.crecido(&ps.rio) { " · crecido" } else { "" })).small());
                }
            }
            Some(Sel::Celda(c)) => celda_info(ui, *c, anio),
            None => match self.hover {
                Some(c) => celda_info(ui, c, anio),
                None => {
                    ui.label(RichText::new("Clic en un pueblo, una estancia, un saladero o un camino para ver su ficha.").small());
                }
            },
        }
    }
}

fn costo_txt(p: &Pais, h: Herr) -> String {
    match h {
        Herr::Camino => format!("{} pesos el km", super::COSTO_KM_CAMINO),
        Herr::Mejorar => "10 pesos el km por nivel".into(),
        Herr::Puente => "1.500 pesos más el ancho del río".into(),
        _ => format!("{} pesos", p.costo(&h.ejemplo())),
    }
}

fn celda_info(ui: &mut egui::Ui, c: [f32; 2], anio: i32) {
    let g = geo();
    let (lon, lat) = g.lonlat(c);
    let d = g.depto(c[0], c[1]);
    if d == 0 {
        ui.label(if g.es_agua(c[0], c[1], anio) { "Agua" } else { "Fuera del país" });
        return;
    }
    ui.label(RichText::new(g.nombre_depto(d).unwrap_or("")).strong());
    let mut t = format!("{:.2}° S, {:.2}° O · {:.0} m", -lat, -lon, g.altura(c[0], c[1]));
    if g.es_agua(c[0], c[1], anio) {
        t.push_str(" · agua");
    }
    ui.label(RichText::new(t).small());
    // [atlas]
    ficha_ecosistema(ui, c, anio);
    ui.label(RichText::new(format!(
        "Pradera {:.0} % · monte {:.0} % · bañado {:.0} %",
        g.pastura(c[0], c[1]) * 100.0,
        g.capa(c[0], c[1], datos::C_MONTE) as f32 / 2.55,
        g.capa(c[0], c[1], datos::C_BANADO) as f32 / 2.55
    )).small());
    if let Some(u) = g.unidad(c[0], c[1]) {
        let mats: Vec<String> = u.materiales.iter().take(3).map(|(m, w)| format!("{m} {:.0} %", w * 100.0)).collect();
        ui.label(RichText::new(format!("Geología: {}{}", u.nombre, if mats.is_empty() { String::new() } else { format!(" · {}", mats.join(", ")) })).small());
    }
    let tit = g.titulos.iter().filter(|t| ((t.pos[0] - c[0]).powi(2) + (t.pos[1] - c[1]).powi(2)) < 25.0).count();
    if tit > 0 {
        ui.label(RichText::new(format!("{tit} títulos mineros a menos de 5 km (DINAMIGE)")).small());
    }
    if let Some((r, _)) = g.rio_cerca(c, 2.0) {
        ui.label(RichText::new(format!("{}{}", r.nombre, if r.navegable { " (navegable)" } else { "" })).small());
    }
}

/// [atlas] Ecosistema de la celda: código, nombre, mosaico, densidad, evidencia, confianza y fuente
/// (de la capa del período del año).
fn ficha_ecosistema(ui: &mut egui::Ui, c: [f32; 2], anio: i32) {
    use crate::ecosistemas as eco;
    let g = geo();
    let Some(e) = g.ecosistema(c[0], c[1], anio) else { return };
    let Some(cl) = eco::clase(e.prim) else { return };
    let capa = &eco::info().capas[g.eco.capa_de(anio)];
    ui.label(RichText::new(format!("{} · {}", cl.codigo, cl.nombre)).strong());
    if let Some(s) = eco::clase(e.sec) {
        ui.label(RichText::new(format!("Mosaico con {} ({})", s.nombre.to_lowercase(), s.codigo)).small());
    }
    let mut t = format!("Árboles: {}", eco::DENSIDAD[e.densidad.min(5) as usize]);
    if let Some(h) = e.dosel_m() {
        t.push_str(&format!(" · dosel ~{h:.0} m"));
    }
    ui.label(RichText::new(t).small());
    let col = match e.evidencia {
        2 => theme::INK,
        1 => theme::INK_SOFT,
        _ => theme::BAD,
    };
    ui.label(RichText::new(format!("Evidencia: {} · confianza {}", eco::EVIDENCIA[e.evidencia.min(2) as usize], eco::CONFIANZA[e.confianza.min(2) as usize])).small().color(col));
    ui.label(RichText::new(format!("Capa {} ({}–{})", capa.periodo, capa.desde, capa.hasta.min(2025))).small());
    ui.label(RichText::new(format!("Fuente: {}", eco::fuente(e.fuente))).size(11.0).color(theme::INK_SOFT));
    // Desde 1985, las cifras de MapBiomas del departamento (sin compararlas con las históricas).
    if anio >= 1985 {
        let dep = g.nombre_depto(g.depto(c[0], c[1])).unwrap_or("");
        if let Some(m) = eco::info().mapbiomas_departamentos_km2.as_ref().and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(dep) || quitar_tildes(k) == quitar_tildes(dep)).map(|(_, v)| v)) {
            let v = |cl: &str| m.get(cl).map(|a| (a.get("1985").copied().unwrap_or(0.0), a.get("2025").copied().unwrap_or(0.0)));
            let mut partes = Vec::new();
            for (k, n) in [("Bosque Natural", "bosque natural"), ("Pastizal natural", "pastizal"), ("Agricultura", "agricultura")] {
                if let Some((a, b)) = v(k) {
                    partes.push(format!("{n} {a:.0} → {b:.0}"));
                }
            }
            let plant = v("Pino").map_or(0.0, |x| x.1) + v("Eucaliptus").map_or(0.0, |x| x.1);
            ui.label(RichText::new(format!("{dep}, MapBiomas col. 4 (km², 1985 → 2025): {} · plantaciones 2025: {plant:.0}", partes.join(" · "))).size(11.0).color(theme::INK_SOFT));
        }
    }
}

fn quitar_tildes(s: &str) -> String {
    s.to_lowercase().chars().map(|c| match c {
        'á' => 'a',
        'é' => 'e',
        'í' => 'i',
        'ó' => 'o',
        'ú' => 'u',
        c => c,
    }).collect()
}

/// [atlas] Leyenda de la capa de ecosistemas: las clases que hay en la capa del período.
fn leyenda_ecosistemas(ui: &mut egui::Ui, anio: i32) {
    use crate::ecosistemas as eco;
    let g = geo();
    let k = g.eco.capa_de(anio);
    let capa = &eco::info().capas[k];
    ui.label(RichText::new(format!("Capa {} · {}", capa.periodo, capa.nota)).small().color(theme::INK_SOFT));
    let mut hay = [false; 64];
    for f in (0..g.alto()).step_by(2) {
        for c in (0..g.ancho()).step_by(2) {
            if let Some(e) = g.eco.celda(k, c as i64, f as i64) {
                hay[(e.prim as usize).min(63)] = true;
                hay[(e.sec as usize).min(63)] = true;
            }
        }
    }
    ui.horizontal_wrapped(|ui| {
        for cl in eco::LEYENDA.iter().filter(|cl| hay[cl.n as usize]) {
            // Cuadradito de color y código en un solo rótulo (no se separan al cortar la línea).
            let mut job = egui::text::LayoutJob::default();
            let f = FontId::proportional(13.0);
            job.append("■", 0.0, egui::TextFormat { font_id: FontId::proportional(15.0), color: Color32::from_rgb(cl.color[0], cl.color[1], cl.color[2]), ..Default::default() });
            job.append(cl.codigo, 2.0, egui::TextFormat { font_id: f, color: theme::INK, ..Default::default() });
            ui.label(job).on_hover_text(format!("{}{}", cl.nombre, if cl.atlas { "" } else { " (extensión del juego, no está en el atlas)" }));
        }
    });
    ui.label(RichText::new("Puntos: mosaico (código secundario). Rayado: evidencia desconocida. Atlas v1.0 + reglas por gradientes (ver ficha de cada celda).").small().color(theme::INK_SOFT));
}

/// Color de un material del catálogo (capa de recursos).
fn color_material(m: &str) -> [f32; 3] {
    match m {
        "granito" => [200.0, 90.0, 110.0],
        "gneis" => [170.0, 110.0, 150.0],
        "basalto" => [80.0, 80.0, 90.0],
        "arenisca" | "arena" => [230.0, 200.0, 110.0],
        "caliza" | "conchilla" | "dolomita" | "marmol" => [235.0, 235.0, 220.0],
        "arcilla_roja" => [200.0, 100.0, 60.0],
        "arcilla_gris" => [150.0, 150.0, 140.0],
        "balasto" | "canto_rodado" | "tosca" => [190.0, 160.0, 120.0],
        "amatista" | "agata" => [150.0, 80.0, 190.0],
        "oro" | "hierro" => [220.0, 170.0, 40.0],
        "turba" => [90.0, 70.0, 50.0],
        _ => [180.0, 170.0, 150.0],
    }
}

/// Imagen del mapa sin la capa de ecosistemas (la usan las pruebas y el resto del código).
pub fn imagen(anio: i32, recursos: bool, cobertura: bool) -> egui::ColorImage {
    imagen_capas(anio, recursos, cobertura, false)
}

/// Imagen del mapa: relieve en terrazas (escalones de 15 m con luz del noroeste), agua, monte,
/// bañados y lo de afuera apagado (los ríos van encima, como líneas). Tres píxeles por celda.
pub fn imagen_capas(anio: i32, recursos: bool, cobertura: bool, ecosistemas: bool) -> egui::ColorImage {
    let g = geo();
    let capa_eco = g.eco.capa_de(anio);
    const S: usize = 3;
    let (w, h) = (g.ancho() * S, g.alto() * S);
    let mut px = vec![0u8; w * h * 4];
    let paso_m = 15.0;
    let terraza = |c: f32, f: f32| (g.altura(c, f).max(0.0) / paso_m).floor();
    for y in 0..h {
        for x in 0..w {
            let c = (x as f32 + 0.5) / S as f32 - 0.5;
            let f = (y as f32 + 0.5) / S as f32 - 0.5;
            let alt = g.altura(c, f);
            let k = (y * w + x) * 4;
            let ci = (c.round().max(0.0) as usize).min(g.ancho() - 1);
            let fi = (f.round().max(0.0) as usize).min(g.alto() - 1);
            let b = g.capas[fi * g.ancho() + ci];
            let emb = b[datos::C_EMBALSE] > 0 && g.embalse_anio(b[datos::C_EMBALSE]).is_some_and(|y| anio >= y);
            let col: [f32; 3] = if b[datos::C_AGUA] >= 128 {
                let t = (-alt / 20.0).clamp(0.0, 1.0);
                [70.0 - 25.0 * t, 112.0 - 30.0 * t, 152.0 - 20.0 * t]
            } else if emb {
                [92.0, 138.0, 178.0]
            } else {
                // Hipsometría suave: pradera verde en lo bajo, ocre en las cuchillas, gris en
                // las sierras.
                let t = (alt / 320.0).clamp(0.0, 1.0);
                let mut col = [118.0 + 52.0 * t, 146.0 + 18.0 * t - 20.0 * t * t, 86.0 + 38.0 * t];
                // Luz: pendiente de la superficie suave más el borde de cada terraza.
                let (dx, dy) = (g.altura(c + 0.5, f) - g.altura(c - 0.5, f), g.altura(c, f + 0.5) - g.altura(c, f - 0.5));
                let mut luz = 1.0 - (dx - dy) * 0.012;
                let q = terraza(c, f);
                if terraza(c + 0.5, f + 0.5) < q {
                    luz -= 0.16; // borde en sombra (al sudeste del escalón)
                } else if terraza(c - 0.5, f - 0.5) < q {
                    luz += 0.12; // borde iluminado
                }
                luz += (q % 2.0) * 0.025;
                for v in col.iter_mut() {
                    *v *= luz.clamp(0.55, 1.15);
                }
                if b[datos::C_DEPTO] == 0 {
                    // Fuera del país (Argentina, Brasil): llano y apagado, sin relieve.
                    col = [168.0, 164.0, 150.0];
                } else if ecosistemas {
                    // [atlas] Color de la clase con la luz del relieve; el secundario en un punto
                    // de cada celda (mosaico) y la evidencia desconocida, rayada.
                    if let Some(e) = g.eco.celda(capa_eco, ci as i64, fi as i64) {
                        let luz = (col[0] + col[1] + col[2]) / (118.0 + 146.0 + 86.0 + 0.001) * 1.0;
                        let (sx, sy) = (x % S, y % S);
                        let n = if sx == 1 && sy == 1 && e.sec != 0 { e.sec } else { e.prim };
                        if let Some(cl) = crate::ecosistemas::clase(n) {
                            let k = (0.75 + 0.25 * luz).clamp(0.6, 1.2);
                            col = [cl.color[0] as f32 * k, cl.color[1] as f32 * k, cl.color[2] as f32 * k];
                        }
                        if e.evidencia == 0 && (x + y) % 4 == 0 {
                            col = [col[0] * 0.8 + 40.0, col[1] * 0.8 + 40.0, col[2] * 0.8 + 40.0];
                        }
                    }
                } else if recursos {
                    let m = (col[0] + col[1] + col[2]) / 3.0;
                    let gris = [m * 0.75 + 40.0; 3];
                    col = match g.unidad(c, f).and_then(|u| u.materiales.first()) {
                        Some((mat, peso)) => {
                            let mc = color_material(mat);
                            let a = 0.35 + 0.45 * peso;
                            [gris[0] * (1.0 - a) + mc[0] * a * m / 150.0, gris[1] * (1.0 - a) + mc[1] * a * m / 150.0, gris[2] * (1.0 - a) + mc[2] * a * m / 150.0]
                        }
                        None => gris,
                    };
                } else if cobertura {
                    let mon = b[datos::C_MONTE] as f32 / 255.0;
                    let ban = b[datos::C_BANADO] as f32 / 255.0;
                    // Monte: verde oscuro en bloques de una celda (como copas de vóxeles);
                    // bañado: verde agua con rayas.
                    let bloque = ((ci * 7 + fi * 13) % 3) as f32 * 0.08;
                    let a = (mon * (0.75 + bloque)).min(0.9);
                    col = [col[0] * (1.0 - a) + 48.0 * a, col[1] * (1.0 - a) + 88.0 * a, col[2] * (1.0 - a) + 42.0 * a];
                    let raya = (y % 3 == 0) as i32 as f32;
                    let a = (ban * 0.55 + raya * ban * 0.3).min(0.85);
                    col = [col[0] * (1.0 - a) + 96.0 * a, col[1] * (1.0 - a) + 140.0 * a, col[2] * (1.0 - a) + 128.0 * a];
                }
                col
            };
            px[k] = col[0].clamp(0.0, 255.0) as u8;
            px[k + 1] = col[1].clamp(0.0, 255.0) as u8;
            px[k + 2] = col[2].clamp(0.0, 255.0) as u8;
            px[k + 3] = 255;
        }
    }
    // Títulos mineros (capa de recursos): puntos.
    if recursos {
        for t in &g.titulos {
            let (x, y) = (((t.pos[0] + 0.5) * S as f32) as i64, ((t.pos[1] + 0.5) * S as f32) as i64);
            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                continue;
            }
            let mc = t.material.as_deref().map_or([60.0, 40.0, 30.0], color_material);
            let k = (y as usize * w + x as usize) * 4;
            px[k] = (mc[0] * 0.6) as u8;
            px[k + 1] = (mc[1] * 0.6) as u8;
            px[k + 2] = (mc[2] * 0.6) as u8;
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([w, h], &px)
}

/// Trazo (el ancho siempre en f32).
fn st(w: f32, c: Color32) -> Stroke {
    Stroke::new(w, c)
}
