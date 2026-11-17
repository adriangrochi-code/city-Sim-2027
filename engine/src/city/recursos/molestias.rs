//! Molestias de la industria y de algunos servicios: olor (saladeros, curtiembres, basurales),
//! ruido y polvo (canteras, herrerías, astilleros, barracas), humo (hornos de ladrillos,
//! velerías) y agua sucia (curtiembres y saladeros que tiran al arroyo: baja por el cauce,
//! aguas abajo, y se va diluyendo).
//!
//! Cada fuente tiene un [`Perfil`] (intensidad 0 a 1 de cada molestia y su alcance); la
//! molestia en un punto es la suma, que baja con la distancia como (1 − d/alcance)^1,5. Los
//! valores son **estimaciones de diseño** (ver `docs/sistemas/recursos-del-suelo.md`).

use super::super::lots::{zone_of, Lot, ZoneKind};
use super::super::services::Kind;
use crate::terrain::TerrainGen;
use glam::{IVec2, Vec2};
use std::collections::HashMap;

/// Olor, ruido, humo y agua sucia en un punto (0 a 1 cada una).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Molestia {
    pub olor: f32,
    pub ruido: f32,
    pub humo: f32,
    pub agua: f32,
}

impl Molestia {
    /// Molestia total (0 a 1): se combinan como probabilidades independientes; el agua sucia
    /// pesa menos (molesta a quien vive junto al arroyo, no a todo el barrio).
    pub fn total(&self) -> f32 {
        1.0 - (1.0 - self.olor) * (1.0 - self.ruido * 0.8) * (1.0 - self.humo * 0.9) * (1.0 - self.agua * 0.6)
    }

    /// La que más pesa, para las fichas.
    pub fn principal(&self) -> &'static str {
        let v = [(self.olor, "olor"), (self.ruido * 0.8, "ruido"), (self.humo * 0.9, "humo"), (self.agua * 0.6, "agua sucia")];
        v.iter().copied().max_by(|a, b| a.0.total_cmp(&b.0)).map_or("—", |x| if x.0 < 0.03 { "—" } else { x.1 })
    }

    fn sumar(&mut self, o: Molestia) {
        self.olor = (self.olor + o.olor).min(1.0);
        self.ruido = (self.ruido + o.ruido).min(1.0);
        self.humo = (self.humo + o.humo).min(1.0);
        self.agua = (self.agua + o.agua).min(1.0);
    }
}

/// Perfil de molestias de un tipo de industria o servicio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perfil {
    pub nombre: &'static str,
    pub olor: f32,
    pub ruido: f32,
    pub humo: f32,
    /// Alcance (m) de olor, ruido y humo.
    pub radio: f32,
    /// Lo que tira al arroyo (0 a 1) si está a menos de [`RADIO_VERTIDO`] de un cauce.
    pub vertido: f32,
}

impl Perfil {
    const fn new(nombre: &'static str, olor: f32, ruido: f32, humo: f32, radio: f32, vertido: f32) -> Perfil {
        Perfil { nombre, olor, ruido, humo, radio, vertido }
    }

    /// Perfil genérico a partir de una molestia única (0 a 1), para los perfiles de industria
    /// que solo traen `molestia: f32`: mitad olor, mitad ruido, con más alcance cuanto más
    /// molesta.
    #[allow(dead_code)] // API para el sistema de industrias (todavía sin enchufar)
    pub fn desde_molestia(m: f32) -> Perfil {
        let m = m.clamp(0.0, 1.0);
        Perfil::new("industria", m * 0.7, m * 0.6, m * 0.3, 80.0 + 250.0 * m, 0.0)
    }

    fn en(&self, d: f32, peso: f32) -> Molestia {
        if d >= self.radio {
            return Molestia::default();
        }
        let k = (1.0 - d / self.radio).powf(1.5) * peso;
        Molestia { olor: self.olor * k, ruido: self.ruido * k, humo: self.humo * k, agua: 0.0 }
    }
}

/// Un taller tira al arroyo si lo tiene a menos de esto (m).
pub const RADIO_VERTIDO: f32 = 150.0;
/// Hasta dónde baja el agua sucia por el cauce (m) y cómo se diluye.
pub const LARGO_AGUA_SUCIA: f32 = 5000.0;
pub const DILUCION_M: f32 = 1800.0;
/// Un solar junto al cauce sucio lo sufre hasta esta distancia (m).
pub const ORILLA: f32 = 90.0;

/// Perfiles de la época colonial (los de las épocas siguientes y los saladeros los agrega el
/// sistema de industrias con [`Perfil::desde_molestia`] o con [`perfil_por_nombre`]).
pub const PERFILES: [Perfil; 14] = [
    Perfil::new("panadería", 0.0, 0.05, 0.18, 50.0, 0.0),
    Perfil::new("herrería y taller", 0.0, 0.45, 0.12, 80.0, 0.0),
    Perfil::new("velería y jabonería", 0.45, 0.05, 0.3, 120.0, 0.25),
    Perfil::new("curtiembre", 0.9, 0.1, 0.05, 180.0, 0.9),
    Perfil::new("barraca y depósito", 0.1, 0.35, 0.0, 80.0, 0.0),
    Perfil::new("horno de ladrillos", 0.05, 0.1, 0.85, 260.0, 0.0),
    Perfil::new("cantera", 0.0, 0.85, 0.2, 320.0, 0.1),
    Perfil::new("basurero", 0.9, 0.05, 0.15, 300.0, 0.35),
    Perfil::new("estancia (corrales)", 0.25, 0.05, 0.0, 130.0, 0.15),
    Perfil::new("muelle", 0.05, 0.2, 0.0, 120.0, 0.0),
    Perfil::new("lazareto", 0.2, 0.0, 0.05, 150.0, 0.2),
    // Para el sistema de industrias de exportación (saladeros, graserías, astilleros).
    Perfil::new("saladero", 1.0, 0.25, 0.3, 450.0, 1.0),
    Perfil::new("grasería", 0.85, 0.1, 0.5, 300.0, 0.6),
    Perfil::new("astillero", 0.05, 0.6, 0.1, 200.0, 0.1),
];

pub fn perfil_por_nombre(n: &str) -> Option<&'static Perfil> {
    PERFILES.iter().find(|p| p.nombre == n)
}

/// Perfil del edificio de un solar industrial, según el edificio que se ve (las mismas reglas
/// que el catálogo de vóxeles: `builder_for`).
pub fn perfil_de_lote(l: &Lot) -> Option<&'static Perfil> {
    let (k, _) = zone_of(l.zone)?;
    if k != ZoneKind::Industrial || l.level == 0 {
        return None;
    }
    let pick = (l.seed >> 7) % 100;
    perfil_por_nombre(match (l.level, pick < 45) {
        (1, true) => "panadería",
        (1, false) => "herrería y taller",
        (2, true) => "velería y jabonería",
        (2, false) => "curtiembre",
        (_, _) => "barraca y depósito",
    })
}

/// Perfil de un edificio de servicio (los que molestan).
pub fn perfil_de_servicio(k: Kind) -> Option<&'static Perfil> {
    perfil_por_nombre(match k {
        Kind::Horno => "horno de ladrillos",
        Kind::Cantera => "cantera",
        Kind::Basurero => "basurero",
        Kind::Estancia => "estancia (corrales)",
        Kind::Lazareto => "lazareto",
        // Industrias del sistema de desarrollo.
        Kind::Saladero => "saladero",
        Kind::Astillero => "astillero",
        _ => return None,
    })
}

/// Una fuente de molestias.
#[derive(Clone, Copy, Debug)]
pub struct Fuente {
    pub pos: Vec2,
    pub perfil: Perfil,
    /// Escala de la fuente (un taller grande molesta más).
    pub peso: f32,
}

// --- Cuencas: el agua baja por los arroyos -------------------------------------------------

/// Un cauce orientado aguas abajo y adónde desemboca.
#[derive(Clone, Debug)]
pub struct Cauce {
    pub pts: Vec<Vec2>,
    /// Cauce y punto donde desagua (`None`: en el río o fuera del mapa).
    pub sale_a: Option<(usize, usize)>,
}

/// Los arroyos del mapa como red que corre aguas abajo.
#[derive(Clone, Debug, Default)]
pub struct Cuencas {
    pub cauces: Vec<Cauce>,
    celdas: HashMap<IVec2, Vec<(u32, u32)>>,
}

const CELDA: f32 = 60.0;

fn celda(p: Vec2) -> IVec2 {
    (p / CELDA).floor().as_ivec2()
}

impl Cuencas {
    /// Orienta cada arroyo (de su punta alta a la baja; si empatan, hacia el mar) y busca dónde
    /// desemboca: el punto más cercano de otro cauce a menos de 40 m de su punta baja.
    pub fn de(t: &TerrainGen) -> Cuencas {
        let mut cauces: Vec<Cauce> = t
            .arroyos
            .iter()
            .filter(|a| a.len() >= 2)
            .map(|a| {
                let (p0, p1) = (a[0], *a.last().unwrap());
                let (h0, h1) = (t.height(p0.x, p0.y), t.height(p1.x, p1.y));
                let al_reves = if (h0 - h1).abs() > 0.3 { h0 < h1 } else { t.sea_distance(p0.x, p0.y) < t.sea_distance(p1.x, p1.y) };
                let mut pts = a.clone();
                if al_reves {
                    pts.reverse();
                }
                Cauce { pts, sale_a: None }
            })
            .collect();
        let mut celdas: HashMap<IVec2, Vec<(u32, u32)>> = HashMap::new();
        for (ci, c) in cauces.iter().enumerate() {
            for (pi, p) in c.pts.iter().enumerate() {
                celdas.entry(celda(*p)).or_default().push((ci as u32, pi as u32));
            }
        }
        let mut cu = Cuencas { cauces: Vec::new(), celdas };
        let salidas: Vec<Option<(usize, usize)>> = cauces
            .iter()
            .enumerate()
            .map(|(ci, c)| {
                let fin = *c.pts.last().unwrap();
                let mut best: Option<(f32, usize, usize)> = None;
                for (cj, pj, d) in cu.cerca_de(&cauces, fin, 40.0) {
                    // Ni sí mismo ni la punta baja de otro cauce (sería un callejón).
                    if cj == ci || pj + 1 >= cauces[cj].pts.len() {
                        continue;
                    }
                    if best.is_none_or(|b| d < b.0 || (d == b.0 && (cj, pj) < (b.1, b.2))) {
                        best = Some((d, cj, pj));
                    }
                }
                best.map(|b| (b.1, b.2))
            })
            .collect();
        for (c, s) in cauces.iter_mut().zip(salidas) {
            c.sale_a = s;
        }
        cu.cauces = cauces;
        cu
    }

    fn cerca_de(&self, cauces: &[Cauce], p: Vec2, r: f32) -> Vec<(usize, usize, f32)> {
        let c = celda(p);
        let k = (r / CELDA).ceil() as i32;
        let mut out = Vec::new();
        for dz in -k..=k {
            for dx in -k..=k {
                if let Some(v) = self.celdas.get(&(c + IVec2::new(dx, dz))) {
                    for &(ci, pi) in v {
                        let d = cauces[ci as usize].pts[pi as usize].distance(p);
                        if d <= r {
                            out.push((ci as usize, pi as usize, d));
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.2.total_cmp(&b.2).then((a.0, a.1).cmp(&(b.0, b.1))));
        out
    }

    /// Punto de cauce más cercano a `p` (a menos de `r` m): (cauce, punto, distancia).
    pub fn cercano(&self, p: Vec2, r: f32) -> Option<(usize, usize, f32)> {
        self.cerca_de(&self.cauces, p, r).into_iter().next()
    }

    /// El agua sucia que entra en `desde` con `fuerza` baja por el cauce (y por los que siguen
    /// aguas abajo): puntos del cauce ((cauce, punto), posición) con lo sucio que llega (se
    /// diluye con la distancia).
    pub fn aguas_abajo(&self, desde: (usize, usize), fuerza: f32) -> Vec<((usize, usize), Vec2, f32)> {
        let mut out = Vec::new();
        let (mut ci, mut pi) = desde;
        let mut recorrido = 0.0f32;
        let mut prev = self.cauces[ci].pts[pi];
        let mut saltos = 0;
        loop {
            let c = &self.cauces[ci];
            while pi < c.pts.len() {
                let p = c.pts[pi];
                recorrido += p.distance(prev);
                prev = p;
                let v = fuerza * (-recorrido / DILUCION_M).exp();
                if recorrido > LARGO_AGUA_SUCIA || v < 0.03 {
                    return out;
                }
                out.push(((ci, pi), p, v));
                pi += 1;
            }
            match c.sale_a {
                Some((cj, pj)) if saltos < 64 => {
                    ci = cj;
                    pi = pj;
                    saltos += 1;
                }
                _ => return out,
            }
        }
    }
}

// --- Campo de molestias ----------------------------------------------------------------------

/// Molestias de todas las fuentes, listas para consultar en cualquier punto.
#[derive(Clone, Debug, Default)]
pub struct Campo {
    fuentes: Vec<Fuente>,
    celdas: HashMap<IVec2, Vec<u32>>,
    /// Puntos de cauce con agua sucia (y cuánta).
    pub agua_sucia: Vec<(Vec2, f32)>,
    celdas_agua: HashMap<IVec2, Vec<u32>>,
}

const CELDA_CAMPO: f32 = 100.0;

impl Campo {
    pub fn new(fuentes: Vec<Fuente>, cuencas: Option<&Cuencas>) -> Campo {
        let mut c = Campo::default();
        for (i, f) in fuentes.iter().enumerate() {
            let r = (f.perfil.radio / CELDA_CAMPO).ceil() as i32;
            let base = (f.pos / CELDA_CAMPO).floor().as_ivec2();
            for dz in -r..=r {
                for dx in -r..=r {
                    c.celdas.entry(base + IVec2::new(dx, dz)).or_default().push(i as u32);
                }
            }
        }
        // Vertidos: cada fuente que tira al arroyo ensucia aguas abajo; en cada punto del
        // cauce se queda lo más sucio que le llega (los vertidos de varias fuentes se suman).
        if let Some(cu) = cuencas {
            let mut sucio: HashMap<(usize, usize), (Vec2, f32)> = HashMap::new();
            for f in &fuentes {
                let v = f.perfil.vertido * f.peso;
                if v < 0.05 {
                    continue;
                }
                let Some((ci, pi, _)) = cu.cercano(f.pos, RADIO_VERTIDO) else { continue };
                for (clave, p, x) in cu.aguas_abajo((ci, pi), v) {
                    let e = sucio.entry(clave).or_insert((p, 0.0));
                    e.1 = (e.1 + x).min(1.0);
                }
            }
            let mut v: Vec<((usize, usize), (Vec2, f32))> = sucio.into_iter().collect();
            v.sort_by_key(|x| x.0);
            c.agua_sucia = v.into_iter().map(|x| x.1).collect();
        }
        for (i, (p, _)) in c.agua_sucia.iter().enumerate() {
            c.celdas_agua.entry((*p / CELDA_CAMPO).floor().as_ivec2()).or_default().push(i as u32);
        }
        c.fuentes = fuentes;
        c
    }

    /// Molestias en un punto (sin contar la fuente que está en `salvo`, si la hay: un taller
    /// no se molesta a sí mismo).
    pub fn en(&self, p: Vec2, salvo: Option<Vec2>) -> Molestia {
        let mut m = Molestia::default();
        let k = (p / CELDA_CAMPO).floor().as_ivec2();
        if let Some(v) = self.celdas.get(&k) {
            for &i in v {
                let f = &self.fuentes[i as usize];
                if salvo.is_some_and(|s| s.distance(f.pos) < 1.0) {
                    continue;
                }
                m.sumar(f.perfil.en(f.pos.distance(p), f.peso));
            }
        }
        // Agua sucia del arroyo de al lado.
        let mut agua = 0.0f32;
        for dz in -1..=1 {
            for dx in -1..=1 {
                if let Some(v) = self.celdas_agua.get(&(k + IVec2::new(dx, dz))) {
                    for &i in v {
                        let (q, x) = self.agua_sucia[i as usize];
                        let d = q.distance(p);
                        if d < ORILLA {
                            agua = agua.max(x * (1.0 - d / ORILLA));
                        }
                    }
                }
            }
        }
        m.agua = agua.min(1.0);
        m
    }

    pub fn fuentes(&self) -> &[Fuente] {
        &self.fuentes
    }
}
