//! Formas de los árboles nativos del paisaje (ver `docs/sistemas/vegetacion-historica.md`).
//!
//! [`TerrainGen::trees_in`](crate::terrain::TerrainGen::trees_in) decide dónde hay un árbol, de
//! qué especie y de qué tamaño; acá se arma su forma: tronco y ramas (cilindros que se afinan) y
//! una copa hecha de varias masas de follaje (elipsoides) en vez de una sola esfera. Cada
//! especie tiene cuatro variantes (inclinado, de tronco partido, viejo y asimétrico, joven…)
//! y dentro de cada variante el azar del árbol cambia ángulos y medidas: el monte no se repite.
//! Todo es determinista: el mismo árbol da la misma forma en todos los niveles de detalle.

use crate::palette::*;
use glam::Vec3;

/// Especies del monte nativo de la campaña de Montevideo hacia 1726.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Species {
    /// Sauce criollo (*Salix humboldtiana*): en la orilla, alto, de ramas colgantes y verde claro.
    Sauce,
    /// Sarandí (*Cephalanthus*, *Phyllanthus*, *Sebastiania*): arbusto de varios tallos al borde del agua.
    Sarandi,
    /// Ceibo (*Erythrina crista-galli*): tronco torcido, copa abierta y flores rojas.
    Ceibo,
    /// Coronilla (*Scutia buxifolia*): copa densa y oscura, espinosa.
    Coronilla,
    /// Tala (*Celtis tala*): copa irregular y extendida, verde grisáceo.
    Tala,
    /// Canelón (*Myrsine laetevirens*): tronco recto y copa redondeada, en el monte cerrado.
    Canelon,
    /// Espinillo (*Vachellia caven*): arbolito de copa rala en sombrilla, en los bordes secos.
    Espinillo,
    /// Ombú (*Phytolacca dioica*): solitario en la pradera, tronco enorme y copa muy ancha.
    Ombu,
    /// Molle (*Schinus longifolius*) y arbustos del matorral costero y serrano.
    Molle,
}

/// Variantes de forma por especie.
pub const VARIANTS: u8 = 4;

/// Un tramo de tronco o rama: cilindro con puntas redondas que se afina de `r0` a `r1` (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limb {
    pub a: Vec3,
    pub b: Vec3,
    pub r0: f32,
    pub r1: f32,
    /// Tronco principal: se dibuja en todos los niveles (las ramas finas solo de cerca).
    pub main: bool,
}

/// Una masa de follaje (elipsoide). `droop`: ramas colgantes (hebras verticales, como el sauce).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blob {
    pub c: Vec3,
    pub rh: f32,
    pub rv: f32,
    pub droop: bool,
}

/// Forma de un árbol en metros, con el pie del tronco en el origen (x al este, y arriba, z al sur).
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub limbs: Vec<Limb>,
    pub blobs: Vec<Blob>,
    pub leaf: Mat,
    pub bark: Mat,
    /// Porcentaje de vóxeles del borde de la copa que faltan (copa rala o tupida).
    pub holes: u32,
    /// Flores sobre la copa: material y porcentaje de los vóxeles de afuera.
    pub flower: Option<(Mat, u32)>,
}

/// Azar determinista a partir de la semilla del árbol.
struct Rng(u32);

impl Rng {
    fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let mut h = self.0;
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        (h & 0xffff) as f32 / 65535.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    /// Dirección horizontal al azar.
    fn dir(&mut self) -> Vec3 {
        let a = self.f() * std::f32::consts::TAU;
        Vec3::new(a.cos(), 0.0, a.sin())
    }
}

/// Medidas de un árbol: especie, variante, altura total y radio de copa (m), radio del tronco (m).
#[derive(Clone, Copy, Debug)]
pub struct TreeSpec {
    pub species: Species,
    pub variant: u8,
    pub height: f32,
    pub radius: f32,
    pub trunk: f32,
    pub seed: u32,
}

/// Hojas de cada especie.
pub fn leaf_of(s: Species) -> Mat {
    match s {
        Species::Sauce => LEAF_SAUCE,
        Species::Sarandi => LEAF,
        Species::Ceibo => LEAF,
        Species::Coronilla | Species::Molle => LEAF_DARK,
        Species::Tala => LEAF_TALA,
        Species::Canelon => LEAF,
        Species::Espinillo => LEAF_ESPINILLO,
        Species::Ombu => LEAF_OMBU,
    }
}

/// Tronco que sube desde `base` inclinado según `lean` (desplazamiento horizontal por metro)
/// hasta la altura `h`, con una curva suave a mitad de camino. Devuelve la punta.
fn trunk(out: &mut Vec<Limb>, base: Vec3, lean: Vec3, h: f32, r0: f32, r1: f32, kink: Vec3) -> Vec3 {
    let mid = base + Vec3::new(lean.x * h * 0.5, h * 0.5, lean.z * h * 0.5) + kink;
    let top = base + Vec3::new(lean.x * h, h, lean.z * h) + kink * 0.5;
    let rm = (r0 + r1) * 0.5;
    out.push(Limb { a: base, b: mid, r0, r1: rm, main: true });
    out.push(Limb { a: mid, b: top, r0: rm, r1, main: true });
    top
}

/// Ramas desde `from` hasta cada masa de follaje (a 70 % del camino).
fn branches(out: &mut Vec<Limb>, from: Vec3, blobs: &[Blob], r: f32) {
    for b in blobs {
        let to = from + (b.c - from) * 0.7;
        if (to - from).length() > 0.6 {
            out.push(Limb { a: from, b: to, r0: r, r1: r * 0.55, main: false });
        }
    }
}

/// Copa de `n` masas alrededor de `c`: una central y las demás corridas hacia afuera
/// (`spread` del radio) y a distintas alturas. `flat` < 1 aplasta las masas.
#[allow(clippy::too_many_arguments)]
fn crown(rng: &mut Rng, c: Vec3, r: f32, n: usize, spread: f32, flat: f32, skew: Vec3, droop: bool) -> Vec<Blob> {
    let mut v = vec![Blob { c: c + skew * r * 0.3, rh: r * 0.72, rv: r * 0.72 * flat, droop }];
    let a0 = rng.f() * std::f32::consts::TAU;
    for k in 1..n {
        let a = a0 + k as f32 * std::f32::consts::TAU / (n - 1) as f32 + rng.range(-0.5, 0.5);
        let d = r * spread * rng.range(0.7, 1.1);
        let off = Vec3::new(a.cos() * d, r * flat * rng.range(-0.35, 0.3), a.sin() * d) + skew * r * 0.5;
        let s = rng.range(0.45, 0.65);
        v.push(Blob { c: c + off, rh: r * s, rv: r * s * flat * rng.range(0.85, 1.15), droop });
    }
    v
}

/// Arma la forma de un árbol.
pub fn shape(t: &TreeSpec) -> Shape {
    let mut rng = Rng(t.seed ^ 0x9e37_79b9);
    let (h, r, tr) = (t.height, t.radius, t.trunk);
    let v = t.variant % VARIANTS;
    let o = Vec3::ZERO;
    let mut limbs = Vec::new();
    let mut blobs;
    let holes;
    let mut flower = None;
    let mut bark = TRUNK;
    let lean_dir = rng.dir();
    let kink = rng.dir() * rng.range(0.1, 0.35) * h * 0.1;
    match t.species {
        Species::Sauce => {
            // Tronco corto que se abre en ramas que suben y una copa alta de ramas colgantes.
            // 0: derecho; 1: inclinado sobre el agua; 2: tronco partido en V; 3: viejo, ancho y bajo.
            bark = if v == 3 { TRUNK_GRAY } else { TRUNK };
            let lean = lean_dir * if v == 1 { 0.32 } else { 0.06 };
            let hb = h * if v == 3 { 0.24 } else { 0.32 };
            let top = trunk(&mut limbs, o, lean, hb, tr, tr * 0.75, kink);
            let rc = if v == 3 { r * 1.25 } else { r };
            let c = top + Vec3::new(lean.x * h * 0.3, (h - hb) * 0.52, lean.z * h * 0.3);
            let flat = (h - hb) * 0.62 / rc;
            blobs = crown(&mut rng, c, rc, if v == 3 { 5 } else { 4 }, 0.55, flat, lean * 1.5, true);
            if v == 2 {
                // Dos troncos: cada mitad de la copa sobre el suyo.
                let side = Vec3::new(-lean_dir.z, 0.0, lean_dir.x) * rc * 0.45;
                let a = top + side + Vec3::Y * hb * 0.6;
                let b = top - side + Vec3::Y * hb * 0.5;
                limbs.push(Limb { a: top, b: a, r0: tr * 0.7, r1: tr * 0.5, main: true });
                limbs.push(Limb { a: top, b, r0: tr * 0.7, r1: tr * 0.5, main: true });
                blobs[0].c += side * 0.6;
            }
            branches(&mut limbs, top, &blobs, tr * 0.4);
            holes = 34;
        }
        Species::Sarandi => {
            // Mata de varios tallos desde el suelo, abierta en abanico (sin tronco).
            // 0: tres tallos; 1: inclinada sobre el agua; 2: tupida; 3: baja y desparramada.
            let stems = [3, 4, 5, 4][v as usize];
            let lean = lean_dir * if v == 1 { 0.35 } else { 0.0 };
            let (spread, rise) = if v == 3 { (1.5, 0.7) } else { (1.0, 1.0) };
            blobs = Vec::new();
            for k in 0..stems {
                let a = rng.f() * std::f32::consts::TAU + k as f32 * 2.1;
                let d = r * rng.range(0.25, 0.6) * spread;
                let tip = Vec3::new(a.cos() * d + lean.x * h, h * rng.range(0.6, 0.85) * rise, a.sin() * d + lean.z * h);
                limbs.push(Limb { a: Vec3::new(a.cos() * 0.15, 0.0, a.sin() * 0.15), b: tip, r0: tr, r1: tr * 0.5, main: k == 0 });
                let s = r * rng.range(0.5, 0.7);
                blobs.push(Blob { c: tip + Vec3::Y * s * 0.2, rh: s, rv: s * 0.75, droop: false });
            }
            blobs.push(Blob { c: Vec3::new(lean.x * h * 0.5, h * 0.35, lean.z * h * 0.5), rh: r * 0.6, rv: h * 0.3, droop: false });
            holes = 28;
        }
        Species::Ceibo => {
            // Tronco torcido y copa abierta en varias masas; flores rojas.
            // 0: torcido; 1: muy inclinado; 2: dos troncos desde el pie; 3: joven.
            let lean = lean_dir * match v {
                1 => 0.4,
                3 => 0.05,
                _ => 0.15,
            };
            let hb = h * if v == 3 { 0.5 } else { 0.42 };
            let top = trunk(&mut limbs, o, lean, hb, tr, tr * 0.7, kink * 2.5);
            if v == 2 {
                let d2 = Vec3::new(-lean.z, 0.0, lean.x) * 2.0 + lean * -0.5;
                let top2 = trunk(&mut limbs, o + d2 * 0.1, d2 * 0.6, hb * 0.85, tr * 0.8, tr * 0.55, kink * -1.5);
                let c2 = top2 + Vec3::Y * (h - hb) * 0.45;
                blobs = crown(&mut rng, c2, r * 0.7, 2, 0.6, 0.55, Vec3::ZERO, false);
                branches(&mut limbs, top2, &blobs, tr * 0.4);
            } else {
                blobs = Vec::new();
            }
            let c = top + Vec3::new(lean.x * h * 0.2, (h - hb) * 0.45, lean.z * h * 0.2);
            let main = if v == 3 {
                crown(&mut rng, c, r * 0.8, 3, 0.55, 0.7, Vec3::ZERO, false)
            } else {
                crown(&mut rng, c, r, 4, 0.75, 0.55, lean, false)
            };
            branches(&mut limbs, top, &main, tr * 0.45);
            blobs.extend(main);
            holes = 40;
            flower = Some((CEIBO_FLOR, 9));
        }
        Species::Coronilla | Species::Molle => {
            // Copa densa y oscura, casi desde abajo. 0: redonda; 1: ladeada; 2: dos troncos;
            // 3: baja y ancha (matorral).
            let lean = lean_dir * if v == 1 { 0.22 } else { 0.05 };
            let hb = h * if v == 3 { 0.18 } else { 0.28 };
            let top = trunk(&mut limbs, o, lean, hb, tr, tr * 0.7, kink);
            if v == 2 {
                let d2 = Vec3::new(-lean_dir.z, 0.0, lean_dir.x) * 0.25;
                trunk(&mut limbs, o, d2, hb * 1.1, tr * 0.8, tr * 0.6, Vec3::ZERO);
            }
            let flat = if v == 3 { 0.6 } else { 0.85 };
            let rc = if v == 3 { r * 1.15 } else { r };
            let c = top + Vec3::new(lean.x * h * 0.25, (h - hb) * 0.5, lean.z * h * 0.25);
            blobs = crown(&mut rng, c, rc, 4, 0.45, flat, lean_dir * if v == 1 { 0.6 } else { 0.15 }, false);
            branches(&mut limbs, top, &blobs, tr * 0.4);
            holes = if t.species == Species::Molle { 26 } else { 16 };
        }
        Species::Tala => {
            // Tronco algo torcido y copa irregular extendida. 0: común; 1: inclinado;
            // 2: tronco que se abre en dos ramas grandes; 3: viejo, asimétrico (una rama larga).
            let lean = lean_dir * if v == 1 { 0.3 } else { 0.1 };
            let hb = h * 0.36;
            let top = trunk(&mut limbs, o, lean, hb, tr, tr * 0.7, kink * 2.0);
            let skew = if v == 3 { lean_dir * 1.2 } else { lean * 0.8 };
            let c = top + Vec3::new(0.0, (h - hb) * 0.45, 0.0);
            blobs = crown(&mut rng, c, r, 5, 0.7, 0.55, skew, false);
            if v == 2 {
                let side = Vec3::new(-lean_dir.z, 0.0, lean_dir.x) * r * 0.55;
                for s in [side, -side] {
                    limbs.push(Limb { a: top, b: top + s + Vec3::Y * (h - hb) * 0.35, r0: tr * 0.65, r1: tr * 0.4, main: true });
                }
            }
            branches(&mut limbs, top, &blobs, tr * 0.42);
            holes = 32;
        }
        Species::Canelon => {
            // Tronco recto y copa ovalada. 0: común; 1: alto y angosto (busca luz); 2: dos
            // troncos; 3: copa corrida hacia un claro.
            let lean = lean_dir * 0.04;
            let hb = h * if v == 1 { 0.5 } else { 0.4 };
            let top = trunk(&mut limbs, o, lean, hb, tr, tr * 0.75, kink * 0.5);
            if v == 2 {
                trunk(&mut limbs, o, lean_dir * -0.15, hb * 0.9, tr * 0.8, tr * 0.6, Vec3::ZERO);
            }
            let rc = if v == 1 { r * 0.8 } else { r };
            let c = top + Vec3::new(0.0, (h - hb) * 0.5, 0.0);
            let skew = if v == 3 { lean_dir * 0.9 } else { Vec3::ZERO };
            blobs = crown(&mut rng, c, rc, 3, 0.4, (h - hb) * 0.55 / rc, skew, false);
            branches(&mut limbs, top, &blobs, tr * 0.4);
            holes = 24;
        }
        Species::Espinillo => {
            // Arbolito de tronco inclinado o bifurcado y copa rala, chata, en sombrilla.
            // 0: sombrilla; 1: muy inclinado; 2: horqueta; 3: arbusto de varios tallos.
            let lean = lean_dir * if v == 1 { 0.45 } else { 0.2 };
            let hb = h * if v == 3 { 0.25 } else { 0.55 };
            let top = trunk(&mut limbs, o, lean, hb, tr, tr * 0.7, kink * 2.0);
            if v == 2 || v == 3 {
                let d2 = Vec3::new(-lean.z, 0.0, lean.x) * 1.4;
                trunk(&mut limbs, o, d2, hb * 0.9, tr * 0.8, tr * 0.6, Vec3::ZERO);
            }
            let c = top + Vec3::new(lean.x * h * 0.2, (h - hb) * 0.35, lean.z * h * 0.2);
            blobs = crown(&mut rng, c, r, 3, 0.55, 0.38, lean, false);
            branches(&mut limbs, top, &blobs, tr * 0.5);
            holes = 52;
        }
        Species::Ombu => {
            // Tronco enorme y ensanchado al pie, ramas gruesas casi horizontales y copa muy
            // ancha y baja. 0: redondo; 1: asimétrico (una rama enorme); 2: varios troncos
            // soldados; 3: joven.
            bark = TRUNK_GRAY;
            let hb = h * 0.3;
            let top = trunk(&mut limbs, o, lean_dir * 0.03, hb, tr, tr * 0.6, Vec3::ZERO);
            // Pie ensanchado.
            limbs.push(Limb { a: o, b: Vec3::Y * hb * 0.25, r0: tr * 1.35, r1: tr, main: true });
            if v == 2 {
                for k in 0..3 {
                    let d = Vec3::new((k as f32 * 2.1).cos(), 0.0, (k as f32 * 2.1).sin()) * tr * 0.7;
                    limbs.push(Limb { a: d, b: top + d * 0.6, r0: tr * 0.65, r1: tr * 0.45, main: true });
                }
            }
            let skew = if v == 1 { lean_dir * 0.5 } else { Vec3::ZERO };
            let c = top + Vec3::new(0.0, (h - hb) * 0.42, 0.0);
            blobs = if v == 3 {
                // Joven: copa más alta, redonda y angosta.
                crown(&mut rng, c + Vec3::Y * h * 0.08, r * 0.75, 4, 0.5, (h - hb) * 0.5 / (r * 0.75), Vec3::ZERO, false)
            } else {
                crown(&mut rng, c, r, 6, 0.62, (h - hb) * 0.42 / r, skew, false)
            };
            for b in &blobs[1..] {
                // Ramas gruesas casi horizontales hacia cada masa.
                let to = top + (b.c - top) * Vec3::new(0.8, 0.55, 0.8);
                limbs.push(Limb { a: top, b: to, r0: tr * 0.45, r1: tr * 0.2, main: true });
            }
            holes = 22;
        }
    }
    Shape { limbs, blobs, leaf: leaf_of(t.species), bark, holes, flower }
}

/// Alcance horizontal y altura máxima de una forma (m), para el recorte por columnas.
pub fn extent(s: &Shape) -> (f32, f32) {
    let mut reach: f32 = 0.0;
    let mut top: f32 = 0.0;
    for b in &s.blobs {
        reach = reach.max(Vec3::new(b.c.x, 0.0, b.c.z).length() + b.rh);
        top = top.max(b.c.y + b.rv);
    }
    for l in &s.limbs {
        for p in [l.a, l.b] {
            reach = reach.max(Vec3::new(p.x, 0.0, p.z).length() + l.r0);
            top = top.max(p.y + l.r1);
        }
    }
    (reach, top)
}

// --- Flora baja ---------------------------------------------------------------------------

/// Lado del micro-vóxel de la flora baja (m): la mitad del vóxel del terreno. Con él una hoja
/// de paja o un junco es una tira fina y no un poste.
pub const MICRO: f32 = 0.125;
/// La flora baja se arma solo en el nivel más fino (vóxel de 0,25 m), cerca de la cámara; más
/// lejos la pradera se ve por el color del suelo.
pub const PLANT_MAX_VOXEL: f32 = 0.26;
/// Altura máxima de una planta (m): los juncos y totoras más altos.
pub const PLANT_MAX_HEIGHT: f32 = 2.4;

fn hash_u(x: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0x9e37_79b9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

fn value_noise(x: f32, z: f32, salt: u32) -> f32 {
    let (i, j) = (x.floor() as i32, z.floor() as i32);
    let (tx, tz) = (x - i as f32, z - j as f32);
    let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let a = hash_u(i, j, salt) + (hash_u(i + 1, j, salt) - hash_u(i, j, salt)) * sx;
    let b = hash_u(i, j + 1, salt) + (hash_u(i + 1, j + 1, salt) - hash_u(i, j + 1, salt)) * sx;
    a + (b - a) * sz
}

/// Qué crece en una columna.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlantKind {
    /// Macollo de flechilla o paja (con espiga clara en el pasto seco).
    Macollo { espiga: bool, verde: bool },
    Flor(Mat),
    Cardo,
    /// Junco (verde) o totora (con la espiga parda).
    Junco { totora: bool },
    PajaBrava,
    Camalote { flor: bool },
    Espartillo,
    Senecio { flor: bool },
    Redondita,
    Helecho,
    Caraguata { flor: bool },
    Gramilla,
    MataSerrana,
    Tuna,
}

/// Una planta: qué es, su altura (m) y el centro de su mata (m), hacia donde no se abren las hojas.
#[derive(Clone, Copy, Debug)]
pub struct Plant {
    pub kind: PlantKind,
    pub height: f32,
    pub center: (f32, f32),
}

/// Flora baja nativa sobre una columna del terreno natural, según la zona (el material del
/// suelo): matas de flechilla y paja, cardos y flores (margaritas, macachines) en la pradera;
/// juncos, totoras y paja brava en los bañados; camalotes y redondita de agua en las orillas;
/// espartillo y senecio en los médanos; helechos y caraguatá en el monte; tunas y matas entre
/// las piedras. `gx`, `gz`: columna de vóxeles del nivel; `vs`: lado del vóxel (m). Barato: un
/// par de hashes por columna, sin consultar el terreno.
pub fn ground_plant(surf: Mat, gx: i32, gz: i32, vs: f32) -> Option<Plant> {
    if vs > PLANT_MAX_VOXEL {
        return None;
    }
    // Las matas crecen en celdas de 1,5 m: cada celda tiene o no una mata, con su centro en
    // algún lugar de la celda, y la mata ocupa las columnas a menos de su radio. Las manchas
    // (juncales, parches de flores) se miden en metros.
    let (fx, fz) = ((gx as f32 + 0.5) * vs, (gz as f32 + 0.5) * vs);
    let (cx, cz) = ((fx / 1.5).floor() as i32, (fz / 1.5).floor() as i32);
    let cell = hash_u(cx, cz, 1);
    let center = ((cx as f32 + 0.25 + 0.5 * hash_u(cx, cz, 8)) * 1.5, (cz as f32 + 0.25 + 0.5 * hash_u(cx, cz, 9)) * 1.5);
    let dc = ((fx - center.0).powi(2) + (fz - center.1).powi(2)).sqrt();
    let size = 0.2 + 0.25 * hash_u(cx, cz, 10);
    let u = hash_u(gx, gz, 6);
    let clump = value_noise(fx / 7.0, fz / 7.0, 2) * 0.75 + value_noise(fx / 2.5, fz / 2.5, 7) * 0.25;
    let patch = value_noise(fx / 15.0, fz / 15.0, 3);
    let v = hash_u(gx, gz, 4);
    let hv = 0.7 + 0.6 * hash_u(cx, cz, 5);
    // ¿Esta columna es parte de la mata de su celda?
    let mata = |p: f32, r: f32| cell < p && dc < r * size / 0.3;
    let flor = |p: f32| u < p && cell > 0.5;
    let pf = |m: Mat| match (patch * 3.0) as u32 {
        0 => FLOR_BLANCA,
        1 => FLOR_VIOLETA,
        _ => m,
    };
    use PlantKind::*;
    let (kind, height) = match surf {
        GRASS | GRASS_LUSH | GRASS_DRY => {
            let tuft = if surf == GRASS_DRY { 0.5 } else { 0.3 } * (0.2 + 1.6 * clump * clump);
            // Flores en parches: la mayoría del campo casi sin flores, algún parche florido.
            let fl = if surf == GRASS_LUSH { 0.05 } else { 0.02 } * patch * patch * patch * 3.0;
            if mata(tuft, 0.3) {
                (Macollo { espiga: surf == GRASS_DRY && v < 0.5, verde: surf != GRASS_DRY }, 0.55 * hv)
            } else if flor(fl) {
                (Flor(pf(FLOR_AMARILLA)), 0.25 + 0.15 * v)
            } else if cell < 0.004 && dc < 0.2 {
                (Cardo, 1.0 * hv)
            } else {
                return None;
            }
        }
        MARSH => {
            // Juncales y totorales cerrados en manchas, y matas de paja brava entre ellos.
            if clump > 0.58 && u < 0.3 + (clump - 0.58) * 2.0 {
                (Junco { totora: patch > 0.55 }, 0.8 + 1.6 * hash_u(gx, gz, 5).powi(2))
            } else if mata(0.15, 0.45) {
                (PajaBrava, 1.2 * hv)
            } else {
                return None;
            }
        }
        // Orillas: camalotes (con flor lila) y redondita de agua.
        MUD if mata(0.3 * (0.3 + clump), 0.45) => (Camalote { flor: v < 0.25 }, 0.4),
        DUNE | SAND => {
            let k = if surf == DUNE { 1.0 } else { 0.25 };
            if mata(0.35 * k * (0.3 + clump), 0.3) {
                (Espartillo, 0.6 * hv)
            } else if cell > 0.97 - 0.03 * k && dc < 0.35 {
                (Senecio { flor: patch > 0.4 }, 0.7 * hv)
            } else if cell > 0.9 && cell < 0.9 + 0.03 * k && dc < 0.3 {
                (Redondita, 0.1)
            } else {
                return None;
            }
        }
        FOREST_FLOOR => {
            if mata(0.25 * (0.3 + clump), 0.35) {
                (Helecho, 0.5 * hv)
            } else if cell > 0.95 && dc < 0.35 {
                // Caraguatá: roseta espinosa, con el centro rojo cuando florece.
                (Caraguata { flor: hash_u(cx, cz, 11) < 0.3 }, 0.9 * hv)
            } else {
                return None;
            }
        }
        ROCK | COAST_ROCK => {
            if mata(0.3 * clump, 0.3) {
                (Gramilla, 0.25) // gramilla entre las piedras
            } else if cell > 0.95 && dc < 0.35 {
                (MataSerrana, 0.7 * hv)
            } else if cell > 0.93 && cell <= 0.95 && dc < 0.3 {
                (Tuna, 0.9 * hv)
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(Plant { kind, height: height.min(PLANT_MAX_HEIGHT), center })
}

/// Una pieza de la flora baja: una caja de micro-vóxeles de [`MICRO`] m (posición de la esquina
/// en metros, alineada a la grilla de 0,125 m; tamaño en micro-vóxeles; material y matiz).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Micro {
    pub pos: [f32; 3],
    /// x, y, z en micro-vóxeles; w libre.
    pub size: [u8; 4],
    /// Material, matiz (0..255: más oscuro a más claro), altura de la base sobre el suelo
    /// (micro-vóxeles), libre.
    pub info: [u8; 4],
}

/// Una caja de micro-vóxeles con la base `k` micro-vóxeles por encima del suelo (`y0`, m).
#[allow(clippy::too_many_arguments)]
fn micro(out: &mut Vec<Micro>, x: i32, y0: f32, k: i32, z: i32, s: [u8; 3], mat: Mat, shade: u8) {
    out.push(Micro { pos: [x as f32 * MICRO, y0 + k as f32 * MICRO, z as f32 * MICRO], size: [s[0], s[1], s[2], 0], info: [mat, shade, k.clamp(0, 255) as u8, 0] });
}

/// Una hoja, tallo o junco: tira de un micro-vóxel de lado y `h` de alto que se inclina hacia
/// `dir` (curva: casi recta abajo, más tumbada arriba). Los últimos `tip` micro-vóxeles son de
/// `tip_mat` (espiga, flor). Se arma con un tramo por cada escalón de la inclinación.
#[allow(clippy::too_many_arguments)]
fn blade(out: &mut Vec<Micro>, x: i32, y0: f32, z: i32, dir: (f32, f32), h: i32, bend: f32, mat: Mat, tip: i32, tip_mat: Mat, shade: u8) {
    let mut k = 0;
    while k < h {
        let t = k as f32 / h.max(1) as f32;
        let o = bend * t * t * h as f32 * 0.45;
        let (ox, oz) = ((dir.0 * o).round() as i32, (dir.1 * o).round() as i32);
        let m = if k >= h - tip { tip_mat } else { mat };
        // Hasta dónde sigue el mismo escalón y el mismo material.
        let mut e = k + 1;
        while e < h {
            let te = e as f32 / h as f32;
            let oe = bend * te * te * h as f32 * 0.45;
            let me = if e >= h - tip { tip_mat } else { mat };
            if (dir.0 * oe).round() as i32 != ox || (dir.1 * oe).round() as i32 != oz || me != m {
                break;
            }
            e += 1;
        }
        // Una hoja que se tumba también baja: el tramo se corre un poco hacia abajo.
        micro(out, x + ox, y0, k, z + oz, [1, (e - k) as u8, 1], m, shade);
        k = e;
    }
}

/// Arma con micro-vóxeles la planta de la columna (`gx`, `gz`) de lado `vs`, con el suelo a
/// `y0` m. Determinista.
pub fn plant_micro(p: &Plant, gx: i32, gz: i32, vs: f32, y0: f32, out: &mut Vec<Micro>) {
    use PlantKind::*;
    let n = (vs / MICRO).round().max(1.0) as i32;
    let (mx, mz) = (gx * n, gz * n);
    let hm = |m: f32| ((m / MICRO).round() as i32).max(1);
    let r = |k: u32| hash_u(gx, gz, 40 + k);
    // Hacia afuera de la mata (con algo de azar).
    let out_dir = |sx: i32, sz: i32, k: u32| {
        let (px, pz) = ((mx + sx) as f32 * MICRO, (mz + sz) as f32 * MICRO);
        let (mut dx, mut dz) = (px - p.center.0, pz - p.center.1);
        let a = r(k) * std::f32::consts::TAU;
        dx += a.cos() * 0.15;
        dz += a.sin() * 0.15;
        let l = (dx * dx + dz * dz).sqrt().max(1e-3);
        (dx / l, dz / l)
    };
    let shade = |k: u32| (60.0 + 140.0 * r(k)) as u8;
    match p.kind {
        Macollo { espiga, verde } => {
            // Varias hojas finas por columna, de largo desparejo, abiertas hacia afuera.
            for s in 0..(n * n) {
                let (sx, sz) = (s % n, s / n);
                if s > 0 && r(s as u32) > 0.75 {
                    continue;
                }
                let h = hm(p.height * (0.55 + 0.45 * r(10 + s as u32)));
                let tip = if espiga && r(20 + s as u32) < 0.5 { 2 } else { 0 };
                // En el pasto verde, hojas verdes con alguna seca; en el seco, color paja.
                let m = if verde && r(50 + s as u32) < 0.7 { GRASS_LUSH } else { PASTO_MATA };
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), h, 0.6 + 0.6 * r(30 + s as u32), m, tip, GRASS_DRY, shade(s as u32));
            }
        }
        Flor(m) => {
            let (sx, sz) = ((r(1) * n as f32) as i32, (r(2) * n as f32) as i32);
            let h = hm(p.height);
            blade(out, mx + sx, y0, mz + sz, (0.0, 0.0), h, 0.0, GRASS_LUSH, 1, m, shade(3));
            // Dos hojitas en la base.
            micro(out, mx + sx - 1, y0, 0, mz + sz, [1, 1, 1], GRASS_LUSH, shade(4));
            micro(out, mx + sx + 1, y0, 0, mz + sz, [1, 1, 1], GRASS_LUSH, shade(5));
        }
        Cardo => {
            let (sx, sz) = (0, 0);
            let h = hm(p.height);
            blade(out, mx + sx, y0, mz + sz, (0.0, 0.0), h - 2, 0.0, LEAF_TALA, 0, LEAF_TALA, shade(1));
            // Roseta de hojas espinosas abajo y la cabeza violeta arriba.
            for (k, d) in [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().enumerate() {
                blade(out, mx + sx + d.0, y0, mz + sz + d.1, (d.0 as f32, d.1 as f32), 3, 1.6, LEAF_TALA, 0, LEAF_TALA, shade(2 + k as u32));
            }
            micro(out, mx + sx, y0, h - 2, mz + sz, [1, 2, 1], FLOR_VIOLETA, shade(9));
        }
        Junco { totora } => {
            // Uno o dos tallos finos por columna, casi rectos; la totora con su espiga parda.
            for s in 0..(n * n) {
                if s > 0 && r(s as u32) > 0.45 {
                    continue;
                }
                let (sx, sz) = (s % n, s / n);
                let h = hm(p.height * (0.7 + 0.3 * r(10 + s as u32)));
                let tip = if totora && r(20 + s as u32) < 0.6 { 2 } else { 0 };
                let a = r(30 + s as u32) * std::f32::consts::TAU;
                blade(out, mx + sx, y0, mz + sz, (a.cos(), a.sin()), h, 0.15, JUNCO, tip, TRUNK, shade(s as u32));
            }
        }
        PajaBrava => {
            for s in 0..(n * n) {
                let (sx, sz) = (s % n, s / n);
                let h = hm(p.height * (0.6 + 0.4 * r(10 + s as u32)));
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), h, 1.0 + 0.5 * r(30 + s as u32), PASTO_MATA, if r(20 + s as u32) < 0.3 { 3 } else { 0 }, GRASS_DRY, shade(s as u32));
            }
        }
        Camalote { flor } => {
            // Hojas redondas y anchas, paradas, y a veces la flor lila.
            micro(out, mx, y0, 0, mz, [2, 2, 1], LEAF_SAUCE, shade(1));
            micro(out, mx + (r(2) * 2.0) as i32, y0, 0, mz + 1, [1, 2, 2], LEAF_SAUCE, shade(2));
            if flor {
                blade(out, mx + 1, y0, mz + 1, (0.0, 0.0), 4, 0.0, LEAF_SAUCE, 1, FLOR_VIOLETA, shade(3));
            }
        }
        Espartillo => {
            for s in 0..(n * n) {
                if s > 0 && r(s as u32) > 0.8 {
                    continue;
                }
                let (sx, sz) = (s % n, s / n);
                let h = hm(p.height * (0.5 + 0.5 * r(10 + s as u32)));
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), h, 0.9, PASTO_MATA, 2, GRASS_DRY, shade(s as u32));
            }
        }
        Senecio { flor } => {
            for s in 0..(n * n) {
                let (sx, sz) = (s % n, s / n);
                let h = hm(p.height * (0.6 + 0.4 * r(10 + s as u32)));
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), h, 0.5, LEAF_ESPINILLO, if flor { 1 } else { 0 }, FLOR_AMARILLA, shade(s as u32));
            }
        }
        Redondita => micro(out, mx, y0, 0, mz, [2, 1, 2], LEAF, shade(1)),
        Helecho => {
            // Frondas que salen del centro y se arquean hacia afuera.
            for s in 0..(n * n) {
                let (sx, sz) = (s % n, s / n);
                let h = hm(p.height * (0.6 + 0.4 * r(10 + s as u32)));
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), h, 2.2, LEAF, 0, LEAF, shade(s as u32));
            }
        }
        Caraguata { flor } => {
            for s in 0..(n * n) {
                let (sx, sz) = (s % n, s / n);
                let h = hm(p.height * (0.7 + 0.3 * r(10 + s as u32)));
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), h, 2.0, LEAF_DARK, 0, LEAF_DARK, shade(s as u32));
            }
            if flor {
                micro(out, mx, y0, 0, mz, [n as u8, 2, n as u8], CEIBO_FLOR, shade(9));
            }
        }
        Gramilla => {
            for s in 0..(n * n) {
                if s > 0 && r(s as u32) > 0.6 {
                    continue;
                }
                let (sx, sz) = (s % n, s / n);
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), hm(p.height), 0.8, GRASS, 0, GRASS, shade(s as u32));
            }
        }
        MataSerrana => {
            for s in 0..(n * n) {
                let (sx, sz) = (s % n, s / n);
                let h = hm(p.height * (0.6 + 0.4 * r(10 + s as u32)));
                blade(out, mx + sx, y0, mz + sz, out_dir(sx, sz, s as u32), h, 0.8, LEAF_DARK, 0, LEAF_DARK, shade(s as u32));
            }
        }
        Tuna => {
            // Paletas chatas encimadas y giradas, a veces con una flor amarilla.
            let h = hm(p.height);
            let mut k = 0;
            while k * 2 < h && k < 4 {
                let ox = (r(k as u32) * 3.0) as i32 - 1;
                if k % 2 == 0 {
                    micro(out, mx + ox, y0, k * 2, mz, [3, 3, 1], LEAF_TALA, shade(k as u32));
                } else {
                    micro(out, mx, y0, k * 2, mz + ox, [1, 3, 3], LEAF_TALA, shade(k as u32));
                }
                k += 1;
            }
            if r(9) < 0.3 {
                micro(out, mx, y0, k * 2 + 1, mz, [1, 1, 1], FLOR_AMARILLA, 200);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(species: Species, variant: u8, seed: u32) -> TreeSpec {
        TreeSpec { species, variant, height: 8.0, radius: 3.5, trunk: 0.4, seed }
    }

    #[test]
    fn cada_especie_tiene_variantes_distintas() {
        for sp in [Species::Sauce, Species::Sarandi, Species::Ceibo, Species::Coronilla, Species::Tala, Species::Canelon, Species::Espinillo, Species::Ombu, Species::Molle] {
            let shapes: Vec<Shape> = (0..VARIANTS).map(|v| shape(&spec(sp, v, 77))).collect();
            for a in 0..shapes.len() {
                for b in a + 1..shapes.len() {
                    assert_ne!(shapes[a], shapes[b], "{sp:?}: variantes {a} y {b} iguales");
                }
            }
            // La misma variante con otra semilla también cambia (ángulos, medidas).
            assert_ne!(shape(&spec(sp, 0, 1)), shape(&spec(sp, 0, 2)), "{sp:?}: la semilla no cambia nada");
            // Y es determinista.
            assert_eq!(shape(&spec(sp, 1, 5)), shape(&spec(sp, 1, 5)));
            // Copa de varias masas, no una esfera sobre un palo.
            assert!(shapes.iter().all(|s| s.blobs.len() >= 3), "{sp:?}: copa de una sola masa");
        }
    }

    #[test]
    fn flora_baja_por_zona() {
        // Cuántas columnas de cada zona tienen plantas, y que no haya plantas de lejos.
        let vs = crate::lod::BASE_VOXEL;
        let cuenta = |surf: Mat, vs: f32| (0..200 * 200).filter(|k| ground_plant(surf, k % 200, k / 200, vs).is_some()).count();
        let pradera = cuenta(GRASS, vs);
        let banado = cuenta(MARSH, vs);
        assert!(pradera > 100 && pradera < 6000, "pradera {pradera} de 40000");
        assert!(banado > pradera, "bañado {banado} / pradera {pradera}");
        assert!(cuenta(DUNE, vs) > 200, "médanos {}", cuenta(DUNE, vs));
        assert_eq!(cuenta(GRASS, 0.5), 0);
        assert_eq!(cuenta(STREAM, vs), 0);
        assert_eq!(cuenta(DIRT_ROAD, vs), 0);
        // Los juncos llegan a 1,5–2,4 m; las flores son bajas y de colores.
        let alto = (0..20000).filter_map(|k| ground_plant(MARSH, k % 100, k / 100, vs)).map(|p| p.height).fold(0.0, f32::max);
        assert!((1.5..=PLANT_MAX_HEIGHT).contains(&alto), "juncos de {alto} m");
        let flores = (0..40000).filter(|k| matches!(ground_plant(GRASS_LUSH, k % 200, k / 200, vs), Some(Plant { kind: PlantKind::Flor(_), .. }))).count();
        assert!(flores > 20, "{flores} flores");
    }

    #[test]
    fn la_flora_es_fina_y_esta_sobre_el_suelo() {
        // Las piezas son de un micro-vóxel de lado (o pocas), no columnas de 0,25 m, y nacen
        // en la columna o al lado (las hojas tumbadas salen un poco).
        let vs = crate::lod::BASE_VOXEL;
        let mut out = Vec::new();
        let mut plantas = 0;
        for surf in [GRASS, GRASS_DRY, GRASS_LUSH, MARSH, MUD, DUNE, FOREST_FLOOR, ROCK] {
            for k in 0..40000 {
                let (gx, gz) = (k % 200, k / 200);
                if let Some(p) = ground_plant(surf, gx, gz, vs) {
                    plantas += 1;
                    out.clear();
                    plant_micro(&p, gx, gz, vs, 10.0, &mut out);
                    assert!(!out.is_empty(), "{:?} sin piezas", p.kind);
                    for m in &out {
                        assert!(m.pos[1] >= 10.0 - 1e-4, "{:?} bajo el suelo", p.kind);
                        assert!(m.size[0] <= 3 && m.size[2] <= 3, "{:?} pieza ancha {:?}", p.kind, m.size);
                        let top = m.pos[1] + m.size[1] as f32 * MICRO - 10.0;
                        assert!(top <= PLANT_MAX_HEIGHT + 0.3, "{:?} de {top} m", p.kind);
                        let (cx, cz) = ((gx as f32 + 0.5) * vs, (gz as f32 + 0.5) * vs);
                        assert!((m.pos[0] - cx).abs() < 1.5 && (m.pos[2] - cz).abs() < 1.5, "{:?} lejos de su columna", p.kind);
                    }
                }
            }
        }
        assert!(plantas > 1000);
    }

    #[test]
    fn la_copa_empieza_abajo_y_no_se_va_lejos() {
        for sp in [Species::Sauce, Species::Coronilla, Species::Tala, Species::Ombu, Species::Espinillo] {
            for v in 0..VARIANTS {
                let s = shape(&spec(sp, v, 9));
                let low = s.blobs.iter().map(|b| b.c.y - b.rv).fold(f32::MAX, f32::min);
                assert!(low < 8.0 * 0.6, "{sp:?}/{v}: copa empieza a {low} m (chupetín)");
                let (reach, top) = extent(&s);
                assert!(reach < 3.5 * 2.2 && top < 8.0 * 1.6, "{sp:?}/{v}: alcance {reach}, altura {top}");
            }
        }
    }
}
