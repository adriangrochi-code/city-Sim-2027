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

/// Niveles de detalle con flora baja: los de vóxel de hasta 0,5 m (0,25 y 0,5 m); más lejos
/// no se distingue y costaría muchos triángulos.
pub const PLANT_MAX_VOXEL: f32 = 0.5;
/// Altura máxima de una planta (m): los juncos y totoras más altos.
pub const PLANT_MAX_HEIGHT: f32 = 2.4;
/// Vóxeles de alto que puede tener una planta en el nivel más fino.
pub const PLANT_SLOTS: usize = (PLANT_MAX_HEIGHT / crate::lod::BASE_VOXEL + 0.25) as usize;

fn hash_u(x: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0x9e37_79b9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// Ruido de valores (hash en los vértices de una grilla, interpolado suave): manchas de
/// bordes redondeados, no cuadradas.
fn value_noise(x: f32, z: f32, salt: u32) -> f32 {
    let (i, j) = (x.floor() as i32, z.floor() as i32);
    let (tx, tz) = (x - i as f32, z - j as f32);
    let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let a = hash_u(i, j, salt) + (hash_u(i + 1, j, salt) - hash_u(i, j, salt)) * sx;
    let b = hash_u(i, j + 1, salt) + (hash_u(i + 1, j + 1, salt) - hash_u(i, j + 1, salt)) * sx;
    a + (b - a) * sz
}

/// Una planta: materiales de abajo hacia arriba (el último es la flor o la espiga) y altura (m).
struct Plant {
    stem: Mat,
    top: Mat,
    height: f32,
}

/// Flora baja nativa sobre una columna del terreno natural, según la zona (el material del
/// suelo): matas de flechilla y paja, cardos y flores (margaritas, macachines) en la pradera;
/// juncos, totoras y paja brava en los bañados; camalotes y redondita de agua en las orillas;
/// espartillo y senecio en los médanos; helechos y caraguatá en el monte; tunas y matas entre
/// las piedras. Escribe los materiales en `out` (de abajo hacia arriba) y devuelve cuántos son
/// (0: nada). `gx`, `gz`: columna de vóxeles del nivel; `vs`: lado del vóxel (m). Barato: un
/// par de hashes por columna, sin consultar el terreno.
pub fn ground_plant(surf: Mat, gx: i32, gz: i32, vs: f32, out: &mut [Mat; PLANT_SLOTS]) -> usize {
    if vs > PLANT_MAX_VOXEL {
        return 0;
    }
    // Las plantas crecen en macollos: celdas de 1,5 m (6 × 6 vóxeles de cerca) que tienen o no
    // una mata, y dentro de la celda algunos vóxeles. Las manchas (juncales, parches de flores)
    // se miden en metros, así caen igual en todos los niveles.
    let (fx, fz) = (gx as f32 * vs, gz as f32 * vs);
    let (cx, cz) = ((fx / 1.5).floor() as i32, (fz / 1.5).floor() as i32);
    let cell = hash_u(cx, cz, 1);
    let u = hash_u(gx, gz, 6);
    let clump = value_noise(fx / 7.0, fz / 7.0, 2) * 0.75 + value_noise(fx / 2.5, fz / 2.5, 7) * 0.25;
    let patch = value_noise(fx / 15.0, fz / 15.0, 3);
    let v = hash_u(gx, gz, 4);
    let hv = 0.7 + 0.6 * hash_u(gx, gz, 5);
    // ¿Este vóxel es parte de una mata? (celda con mata y el vóxel dentro de ella)
    let mata = |p: f32, fill: f32| cell < p && u < fill;
    let flor = |p: f32| u < p && cell > 0.5;
    let pf = |m: Mat| match (patch * 3.0) as u32 {
        0 => FLOR_BLANCA,
        1 => FLOR_VIOLETA,
        _ => m,
    };
    let p: Option<Plant> = match surf {
        GRASS | GRASS_LUSH | GRASS_DRY => {
            let tuft = if surf == GRASS_DRY { 0.1 } else { 0.05 } * (0.2 + 1.6 * clump * clump);
            // Flores en parches: la mayoría del campo casi sin flores, algún parche florido.
            let fl = if surf == GRASS_LUSH { 0.02 } else { 0.008 } * patch * patch * patch * 3.0;
            if mata(tuft, 0.55) {
                // Flechilla y paja; en el pasto seco, cola de zorro con la espiga clara.
                let top = if surf == GRASS_DRY && v < 0.3 { GRASS_DRY } else { PASTO_MATA };
                Some(Plant { stem: PASTO_MATA, top, height: 0.75 * hv })
            } else if flor(fl) {
                let m = pf(FLOR_AMARILLA);
                Some(Plant { stem: m, top: m, height: 0.4 })
            } else if cell < 0.0015 && u < 0.4 {
                Some(Plant { stem: LEAF_TALA, top: FLOR_VIOLETA, height: 1.1 * hv }) // cardo
            } else {
                None
            }
        }
        MARSH => {
            // Juncales y totorales cerrados en manchas, y matas de paja brava entre ellos.
            if clump > 0.58 && u < 0.14 + (clump - 0.58) * 1.2 {
                let top = if v < 0.1 { TRUNK } else { JUNCO };
                Some(Plant { stem: JUNCO, top, height: 0.6 + 1.8 * hv * hv })
            } else if mata(0.12, 0.6) {
                Some(Plant { stem: PASTO_MATA, top: PASTO_MATA, height: 1.1 * hv }) // paja brava
            } else {
                None
            }
        }
        MUD => {
            // Orillas: camalotes (con flor lila) y redondita de agua.
            if mata(0.15 * (0.3 + clump), 0.6) {
                let top = if v < 0.2 { FLOR_VIOLETA } else { LEAF_SAUCE };
                Some(Plant { stem: LEAF_SAUCE, top, height: 0.5 })
            } else {
                None
            }
        }
        DUNE | SAND => {
            let k = if surf == DUNE { 1.0 } else { 0.25 };
            if mata(0.16 * k * (0.3 + clump), 0.5) {
                Some(Plant { stem: PASTO_MATA, top: PASTO_MATA, height: 0.6 * hv }) // espartillo
            } else if cell > 0.97 - 0.03 * k && u < 0.5 {
                Some(Plant { stem: LEAF_ESPINILLO, top: if v < 0.4 { FLOR_AMARILLA } else { LEAF_ESPINILLO }, height: 0.8 * hv }) // senecio
            } else if cell > 0.9 && cell < 0.9 + 0.03 * k && u < 0.7 {
                Some(Plant { stem: LEAF, top: LEAF, height: 0.3 }) // redondita de agua
            } else {
                None
            }
        }
        FOREST_FLOOR => {
            if mata(0.1 * (0.3 + clump), 0.5) {
                Some(Plant { stem: LEAF, top: LEAF, height: 0.5 }) // helechos
            } else if cell > 0.95 && u < 0.6 {
                // Caraguatá: roseta espinosa, con el centro rojo cuando florece.
                Some(Plant { stem: LEAF_DARK, top: if v < 0.15 { CEIBO_FLOR } else { LEAF_DARK }, height: 0.9 * hv })
            } else {
                None
            }
        }
        ROCK | COAST_ROCK => {
            if mata(0.15 * clump, 0.5) {
                Some(Plant { stem: GRASS, top: GRASS, height: 0.3 }) // gramilla entre las piedras
            } else if cell > 0.95 && u < 0.5 {
                Some(Plant { stem: LEAF_DARK, top: LEAF_DARK, height: 0.8 * hv }) // matas
            } else if cell > 0.93 && cell <= 0.95 && u < 0.4 {
                Some(Plant { stem: LEAF_TALA, top: LEAF_TALA, height: 1.0 * hv }) // tuna
            } else {
                None
            }
        }
        _ => None,
    };
    let Some(p) = p else { return 0 };
    let n = ((p.height.min(PLANT_MAX_HEIGHT) / vs + 0.25) as usize).min(out.len());
    if n == 0 {
        return 0;
    }
    for m in out.iter_mut().take(n - 1) {
        *m = p.stem;
    }
    out[n - 1] = p.top;
    n
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
        let cuenta = |surf: Mat, vs: f32| {
            let mut out = [AIR; PLANT_SLOTS];
            (0..200 * 200).filter(|k| ground_plant(surf, k % 200, k / 200, vs, &mut out) > 0).count()
        };
        let pradera = cuenta(GRASS, 0.5);
        let banado = cuenta(MARSH, 0.5);
        assert!(pradera > 100 && pradera < 1500, "pradera {pradera} de 40000");
        assert!(banado > pradera, "bañado {banado} / pradera {pradera}");
        assert!(cuenta(DUNE, 0.5) > 200, "médanos {}", cuenta(DUNE, 0.5));
        assert_eq!(cuenta(GRASS, 1.0), 0);
        assert_eq!(cuenta(STREAM, 0.5), 0);
        assert_eq!(cuenta(DIRT_ROAD, 0.5), 0);
        // Los juncos son más altos que las flores (de 1,5 m a 2,4 m en el nivel más fino).
        let mut out = [AIR; PLANT_SLOTS];
        let alto = (0..20000).map(|k| ground_plant(MARSH, k % 100, k / 100, 0.5, &mut out)).max().unwrap();
        assert!(alto >= 3, "juncos de {alto} vóxeles");
        let vs = crate::lod::BASE_VOXEL;
        let alto = (0..20000).map(|k| ground_plant(MARSH, k % 100, k / 100, vs, &mut out)).max().unwrap();
        assert!(alto as f32 * vs >= 1.5 && alto as f32 * vs <= PLANT_MAX_HEIGHT + vs, "juncos de {alto} vóxeles de {vs} m");
        assert!(cuenta(MARSH, vs) > 0);
        // Hay flores de colores en la pradera.
        let flores = (0..40000).filter(|k| ground_plant(GRASS_LUSH, k % 200, k / 200, 0.5, &mut out) > 0 && matches!(out[0], FLOR_AMARILLA | FLOR_VIOLETA | FLOR_BLANCA)).count();
        assert!(flores > 20, "{flores} flores");
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
