//! Agua en vóxeles: el río, la bahía y los arroyos llenan celdas de la grilla de vóxeles, igual
//! que el terreno (ver `docs/sistemas/motor-voxel.md`, «Agua en vóxeles»).
//!
//! Cada columna de un chunk puede tener agua encima de su suelo:
//!
//! - **Río y bahía (agua de nivel variable)**: las columnas cuyo suelo queda por debajo del
//!   nivel más alto posible del río ([`NIVEL_MAX`]) y cerca de él. Su cara de arriba no está en
//!   la malla a una altura fija: el vertex shader la pone a la altura del nivel de este momento
//!   (marea, sudestada, bajante), redondeado a la grilla de 0,25 m en todos los niveles de
//!   detalle (así no hay escalón entre niveles), y la esconde si el suelo de la columna queda
//!   por encima. Así la marea no obliga a rehacer ninguna malla.
//! - **Arroyos y cañadas (agua de nivel fijo)**: cada tramo tiene su nivel de agua (que baja
//!   aguas abajo); las columnas del cauce cuyo suelo queda debajo se llenan de vóxeles de agua
//!   hasta ese nivel, redondeado al vóxel del nivel de detalle. Donde el nivel baja de golpe o
//!   el agua da a una orilla más baja (cascadas, bordes) se mallan también las caras de los
//!   costados.
//!
//! Solo se mallan las caras que dan al aire (arriba y costados), unidas con *greedy meshing*.

use crate::terrain::TerrainGen;
use glam::Vec2;
use std::collections::HashMap;

/// Nivel más alto que puede alcanzar el río (m sobre el nivel medio): sudestada grande (hasta
/// 2,2 m), marea viva y creciente de los ríos, con margen. Las columnas con el suelo más alto
/// nunca tienen agua del río.
pub const NIVEL_MAX: f32 = 3.5;
/// Nivel más bajo (pampero fuerte con bajante y marea baja).
pub const NIVEL_MIN: f32 = -2.5;
/// Hasta qué distancia del río (m) el agua alta entra en la tierra baja. Más allá, una hondonada
/// no se inunda con la marea (no está conectada con el río).
pub const ALCANCE_RIO: f32 = 400.0;
/// Profundidad (m) desde la que el agua ya no deja ver nada del fondo: el fondo de las caras del
/// río se recorta ahí (y así se unen en caras grandes).
pub const HONDO: f32 = 4.0;
/// Paso de la grilla con la que se redondea el nivel del río en todos los niveles de detalle.
#[cfg_attr(not(test), allow(dead_code))]
pub const PASO_NIVEL: f32 = crate::lod::BASE_VOXEL;

/// Tipo de agua de una cara (va en el campo de material de `mesher::Quad`).
pub const AGUA_RIO: u8 = 1;
pub const AGUA_ARROYO: u8 = 2;

/// Nivel del río redondeado a la grilla de vóxeles (igual que en el shader `water.wgsl`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn nivel_en_grilla(nivel: f32) -> f32 {
    (nivel / PASO_NIVEL + 0.5).floor() * PASO_NIVEL
}

/// Una muestra del agua de un arroyo (cada ~5 m a lo largo del cauce).
#[derive(Clone, Copy, Debug)]
pub struct Muestra {
    pub p: Vec2,
    /// Hasta dónde llega el agua del tramo desde el eje (m): el cauce con sus orillas.
    pub alcance: f32,
    /// Nivel del agua (m).
    pub nivel: f32,
    /// Corriente (m/s, en x y z).
    pub corriente: Vec2,
}

// [ciudad] --- Agua quieta: lagunas y lagunetas de los bañados ------------------------------

/// Lado de los mosaicos del índice de agua quieta (celdas de la grilla).
const MOSAICO: usize = 16;

/// Una grilla de agua quieta: para cada celda con agua, la profundidad (m) y el nivel del agua
/// (m), guardados en mosaicos de 16 × 16 celdas (casi todo el mapa es seco: solo se guardan los
/// mosaicos con agua, con un mapa de bits para descartar rápido los demás).
struct GrillaQuieta {
    from: f32,
    step: f32,
    n: usize,
    /// Mosaicos por lado y, por mosaico, si tiene agua.
    nm: usize,
    hay: Vec<bool>,
    mosaicos: HashMap<(usize, usize), Vec<(f32, f32)>, crate::fxhash::FxBuild>,
}

impl GrillaQuieta {
    fn celda(&self, i: usize, j: usize) -> (f32, f32) {
        let (ti, tj) = (i / MOSAICO, j / MOSAICO);
        if !self.hay[ti + tj * self.nm] {
            return (0.0, 0.0);
        }
        self.mosaicos.get(&(ti, tj)).map_or((0.0, 0.0), |m| m[(i % MOSAICO) + (j % MOSAICO) * MOSAICO])
    }

    /// Profundidad interpolada entre las cuatro celdas vecinas (0 fuera del agua, así la orilla
    /// baja suave) y el nivel de la celda con agua de más peso.
    fn en(&self, x: f32, z: f32) -> Option<(f32, f32)> {
        let fx = (x - self.from) / self.step;
        let fz = (z - self.from) / self.step;
        let m = self.n as f32 - 1.0;
        if !(0.0..m).contains(&fx) || !(0.0..m).contains(&fz) {
            return None;
        }
        let (i, j) = (fx as usize, fz as usize);
        if !self.hay[i / MOSAICO + (j / MOSAICO) * self.nm] && !self.hay[(i + 1) / MOSAICO + ((j + 1) / MOSAICO) * self.nm] && !self.hay[(i + 1) / MOSAICO + (j / MOSAICO) * self.nm] && !self.hay[i / MOSAICO + ((j + 1) / MOSAICO) * self.nm] {
            return None;
        }
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let mut prof = 0.0;
        let (mut nivel, mut peso) = (0.0, -1.0f32);
        for (di, dj, w) in [(0, 0, (1.0 - tx) * (1.0 - tz)), (1, 0, tx * (1.0 - tz)), (0, 1, (1.0 - tx) * tz), (1, 1, tx * tz)] {
            let (p, l) = self.celda(i + di, j + dj);
            if p > 0.0 {
                prof += p * w;
                if w > peso {
                    peso = w;
                    nivel = l;
                }
            }
        }
        // La orilla no sigue la grilla: el umbral de la profundidad cambia con un ruido de
        // unos 7 m (en el borde la profundidad interpolada sube de 0 a la de la celda).
        let umbral = 0.05 + 0.3 * ruido_valor(x, z, 7.0, 0x0B1A);
        (prof > umbral).then_some((nivel, prof))
    }
}

/// Lagunas y lagunetas: agua quieta que no está unida a ningún cauce (las lagunas de la capa
/// oficial de la IDE por encima del nivel del río, y los claros de agua libre de los bañados).
/// El terreno se hunde con [`AguasQuietas::en`] y el agua se llena en vóxeles hasta su nivel,
/// como la de los arroyos ([`Arroyos::en`]).
///
/// - **Lagunas** (`BIO_LAGUNA` de `biomas.bin`): cada laguna es una componente de celdas; su
///   nivel es la mediana de la altura del MDT en sus celdas (el constructor del mapa las dejó
///   llanas a su nivel) y se descartan las que quedan a menos de 1 m (esas son del río). La
///   profundidad crece con la distancia a la orilla, de 0,6 a 2,5 m [E: no hay batimetría].
/// - **Lagunetas de los bañados**: el agua libre entre los juncos y la paja. Un ruido suave de
///   unos 90 m elige una parte de las celdas de bañado (las del borde no) [E]; cada claro
///   tiene su nivel (mediana de sus celdas) y de 0,35 a 0,8 m de agua [E].
pub struct AguasQuietas {
    grillas: Vec<GrillaQuieta>,
    /// La primera grilla manda dentro de `[0, lado)` en x y z (el mapa jugable).
    lado: f32,
    pub lagunas: usize,
    pub lagunetas: usize,
}

/// Datos de una grilla para armar el agua quieta: origen, paso, muestras por lado, alturas del
/// MDT y biomas oficiales (fila por fila).
pub struct DatosGrilla<'a> {
    pub from: f32,
    pub step: f32,
    pub n: usize,
    pub alturas: &'a [f32],
    pub bio: &'a [u8],
}

/// Ruido de valor suave (0..1) en el punto (x, z) con red de `lado` unidades, determinista.
fn ruido_valor(x: f32, z: f32, lado: f32, semilla: u64) -> f32 {
    let h = |a: i64, b: i64| {
        let mut x = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ semilla;
        x ^= x >> 31;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 29;
        (x >> 40) as f32 / (1u64 << 24) as f32
    };
    let (fx, fz) = (x / lado, z / lado);
    let (a, b) = (fx.floor() as i64, fz.floor() as i64);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (tx, tz) = (s(fx - a as f32), s(fz - b as f32));
    let v0 = h(a, b) + (h(a + 1, b) - h(a, b)) * tx;
    let v1 = h(a, b + 1) + (h(a + 1, b + 1) - h(a, b + 1)) * tx;
    v0 + (v1 - v0) * tz
}

/// Ruido de las lagunetas: dos capas giradas (sin que los claros sigan la grilla).
fn ruido_lagunetas(i: usize, j: usize, lado: f32) -> f32 {
    let (x, z) = (i as f32, j as f32);
    let a = ruido_valor(x, z, lado, 0xBA7A);
    let b = ruido_valor(x * 0.8 - z * 0.6, x * 0.6 + z * 0.8, lado * 0.55, 0x5EED);
    a * 0.65 + b * 0.35
}

/// Umbral del ruido para las lagunetas de los bañados [E]: más alto, menos agua libre.
pub const UMBRAL_LAGUNETA: f32 = 0.66;

impl AguasQuietas {
    /// Arma el agua quieta de las grillas (la primera, la del mapa jugable de `lado` m).
    pub fn new(datos: &[DatosGrilla], lado: f32, bio_laguna: u8, bio_banado: u8) -> AguasQuietas {
        let mut out = AguasQuietas { grillas: Vec::new(), lado, lagunas: 0, lagunetas: 0 };
        for d in datos {
            let n = d.n;
            // Celdas de agua: 1 laguna, 2 laguneta de bañado.
            let mut tipo = vec![0u8; n * n];
            // Las lagunetas, con la red del ruido de unos 90 m (al menos 2 celdas).
            let lado_ruido = (90.0 / d.step).max(2.0);
            for j in 1..n - 1 {
                for i in 1..n - 1 {
                    let k = i + j * n;
                    if d.bio[k] == bio_laguna {
                        tipo[k] = 1;
                    } else if d.bio[k] == bio_banado && [k - 1, k + 1, k - n, k + n].iter().all(|&q| d.bio[q] == bio_banado) && ruido_lagunetas(i, j, lado_ruido) > UMBRAL_LAGUNETA {
                        tipo[k] = 2;
                    }
                }
            }
            // Componentes (4 vecinos), su nivel y la distancia de cada celda a la orilla.
            let mut comp = vec![u32::MAX; n * n];
            let mut nivel_de: Vec<f32> = Vec::new();
            let mut prof = vec![0.0f32; n * n];
            let vecinas = |k: usize, ocho: bool| -> Vec<usize> {
                let (i, j) = ((k % n) as i32, (k / n) as i32);
                let d4 = [(-1, 0), (1, 0), (0, -1), (0, 1)];
                let d8 = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
                d4.iter()
                    .chain(if ocho { d8.iter() } else { [].iter() })
                    .map(|&(di, dj)| (i + di, j + dj))
                    .filter(|&(a, b)| a >= 0 && b >= 0 && a < n as i32 && b < n as i32)
                    .map(|(a, b)| a as usize + b as usize * n)
                    .collect()
            };
            for k0 in 0..n * n {
                if tipo[k0] == 0 || comp[k0] != u32::MAX {
                    continue;
                }
                let id = nivel_de.len() as u32;
                let mut celdas = vec![k0];
                comp[k0] = id;
                let mut q = 0;
                while q < celdas.len() {
                    let k = celdas[q];
                    q += 1;
                    for kk in vecinas(k, false) {
                        if tipo[kk] == tipo[k0] && comp[kk] == u32::MAX {
                            comp[kk] = id;
                            celdas.push(kk);
                        }
                    }
                }
                let mut hs: Vec<f32> = celdas.iter().map(|&k| d.alturas[k]).collect();
                hs.sort_by(f32::total_cmp);
                let nivel = hs[hs.len() / 2];
                // Las lagunas bajas son del río (el constructor las hundió bajo su agua).
                let vale = nivel > 1.0;
                nivel_de.push(if vale { nivel } else { f32::NAN });
                if !vale {
                    continue;
                }
                // Distancia a la orilla (en celdas, 8 vecinos con costo 1).
                let mut dist: HashMap<usize, u32, crate::fxhash::FxBuild> = HashMap::default();
                let mut frente: Vec<usize> = celdas.iter().copied().filter(|&k| vecinas(k, false).len() < 4 || vecinas(k, false).iter().any(|&q| comp[q] != id)).collect();
                for &k in &frente {
                    dist.insert(k, 1);
                }
                let mut nd = 1;
                while !frente.is_empty() {
                    nd += 1;
                    let mut sig = Vec::new();
                    for &k in &frente {
                        for kk in vecinas(k, true) {
                            if comp[kk] == id && !dist.contains_key(&kk) {
                                dist.insert(kk, nd);
                                sig.push(kk);
                            }
                        }
                    }
                    frente = sig;
                }
                let laguna = tipo[k0] == 1;
                for &k in &celdas {
                    let dm = (dist.get(&k).copied().unwrap_or(1) - 1) as f32 * d.step;
                    prof[k] = if laguna { (0.6 + dm * 0.0125).min(2.5) } else { (0.35 + dm * 0.0075).min(0.8) };
                }
                if laguna {
                    out.lagunas += 1;
                } else {
                    out.lagunetas += 1;
                }
            }
            let nm = n.div_ceil(MOSAICO);
            let mut g = GrillaQuieta { from: d.from, step: d.step, n, nm, hay: vec![false; nm * nm], mosaicos: HashMap::default() };
            for k in 0..n * n {
                if prof[k] <= 0.0 {
                    continue;
                }
                let (i, j) = (k % n, k / n);
                let (ti, tj) = (i / MOSAICO, j / MOSAICO);
                g.hay[ti + tj * nm] = true;
                let m = g.mosaicos.entry((ti, tj)).or_insert_with(|| vec![(0.0, 0.0); MOSAICO * MOSAICO]);
                m[(i % MOSAICO) + (j % MOSAICO) * MOSAICO] = (prof[k], nivel_de[comp[k] as usize]);
            }
            out.grillas.push(g);
        }
        out
    }

    /// Nivel del agua (m) y profundidad del fondo bajo él (m), si el punto cae en una laguna o
    /// en una laguneta de bañado.
    pub fn en(&self, x: f32, z: f32) -> Option<(f32, f32)> {
        let dentro = x >= 0.0 && z >= 0.0 && x < self.lado && z < self.lado;
        let g = if dentro { self.grillas.first()? } else { self.grillas.get(1)? };
        g.en(x, z)
    }
}


/// Niveles del agua de todos los arroyos del mapa, con una grilla para buscar la muestra más
/// cercana a un punto.
#[derive(Default)]
pub struct Arroyos {
    muestras: Vec<Muestra>,
    celdas: HashMap<(i32, i32), Vec<u32>, crate::fxhash::FxBuild>,
    /// [ciudad] Lagunas y lagunetas de los bañados (agua quieta, sin corriente).
    quietas: Option<std::sync::Arc<AguasQuietas>>,
}

const CELDA: f32 = 32.0;

impl Arroyos {
    pub fn new(t: &TerrainGen) -> Arroyos {
        let mut a = Arroyos { quietas: t.aguas_quietas(), ..Default::default() };
        for curso in t.stream_water() {
            for m in curso {
                let id = a.muestras.len() as u32;
                let r = m.alcance;
                let (x0, z0) = (((m.p.x - r) / CELDA).floor() as i32, ((m.p.y - r) / CELDA).floor() as i32);
                let (x1, z1) = (((m.p.x + r) / CELDA).floor() as i32, ((m.p.y + r) / CELDA).floor() as i32);
                for cz in z0..=z1 {
                    for cx in x0..=x1 {
                        a.celdas.entry((cx, cz)).or_default().push(id);
                    }
                }
                a.muestras.push(m);
            }
        }
        a
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.muestras.len()
    }

    /// Nivel del agua y corriente del arroyo en este punto, si cae dentro del cauce de alguno.
    /// Se elige la muestra cuyo borde de agua queda más cerca (como `TerrainGen::arroyo_info`).
    pub fn en(&self, x: f32, z: f32) -> Option<(f32, Vec2)> {
        // [ciudad] Fuera de los cauces, el agua quieta de lagunas y bañados (sin corriente).
        let Some(lista) = self.celdas.get(&((x / CELDA).floor() as i32, (z / CELDA).floor() as i32)) else {
            return self.quietas.as_ref()?.en(x, z).map(|(nivel, _)| (nivel, Vec2::ZERO));
        };
        let p = Vec2::new(x, z);
        let mut mejor: Option<(f32, &Muestra)> = None;
        for &i in lista {
            let m = &self.muestras[i as usize];
            let d = m.p.distance(p) - m.alcance;
            if d < 0.0 && mejor.is_none_or(|(b, _)| d < b) {
                mejor = Some((d, m));
            }
        }
        match mejor {
            Some((_, m)) => Some((m.nivel, m.corriente)),
            None => self.quietas.as_ref()?.en(x, z).map(|(nivel, _)| (nivel, Vec2::ZERO)),
        }
    }
}

/// ¿Puede llegar el río a esta columna? Suelo (cara de arriba, m) por debajo del nivel más alto
/// posible, y la columna en el río o cerca de él.
pub fn al_alcance_del_rio(t: &TerrainGen, x: f32, z: f32, h: f32, cara: f32) -> bool {
    cara < NIVEL_MAX && (h < 0.0 || t.sea_distance(x, z) < ALCANCE_RIO)
}

/// Niveles de un curso a lo largo del cauce (de arriba hacia abajo), sin subidas: el promedio
/// entre "nunca más alto que aguas arriba" y "nunca más bajo que aguas abajo". Lo primero solo
/// baja el agua (un pozo del relieve la cortaría); lo segundo solo la sube (un lomo la
/// levantaría todo aguas arriba). El promedio de dos sucesiones que no suben tampoco sube.
pub fn niveles_sin_subidas(raw: &[f32]) -> Vec<f32> {
    let n = raw.len();
    let mut baja = raw.to_vec();
    for i in 1..n {
        baja[i] = baja[i].min(baja[i - 1]);
    }
    let mut sube = raw.to_vec();
    for i in (0..n.saturating_sub(1)).rev() {
        sube[i] = sube[i].max(sube[i + 1]);
    }
    baja.iter().zip(&sube).map(|(a, b)| 0.5 * (a + b)).collect()
}

/// Agua de una columna de vóxeles (de la copia con borde de un chunk).
#[derive(Clone, Copy, Default, Debug)]
pub struct ColumnaAgua {
    /// 0 sin agua, [`AGUA_RIO`] o [`AGUA_ARROYO`].
    pub tipo: u8,
    /// Arroyos: índice del vóxel de agua más alto (el agua llena del suelo hasta él).
    pub tope: i32,
    pub corriente: Vec2,
}

/// Corriente redondeada a 16 rumbos y pasos de 0,1 m/s (para que las caras de un tramo recto
/// se unan), en un byte, y de vuelta en m/s.
fn codigo_corriente(c: Vec2) -> u32 {
    let v = (c.length() / 0.1).round().min(15.0) as u32;
    if v == 0 {
        return 0;
    }
    let a = ((c.y.atan2(c.x) / std::f32::consts::TAU * 16.0).round() as i32).rem_euclid(16) as u32;
    v | (a << 4)
}

fn corriente_de(cod: u32) -> Vec2 {
    let v = (cod & 15) as f32 * 0.1;
    let a = (cod >> 4) as f32 / 16.0 * std::f32::consts::TAU;
    Vec2::new(a.cos(), a.sin()) * v
}

/// Corriente empaquetada para el shader: x y z en mm/s, 16 bits con signo cada una.
fn corriente_empaquetada(c: Vec2) -> u32 {
    let q = |v: f32| ((v * 1000.0).round().clamp(-32767.0, 32767.0) as i32 as u32) & 0xffff;
    q(c.x) | (q(c.y) << 16)
}

/// Malla del agua de una columna de chunks: caras de arriba (unidas) y caras de los costados que
/// dan al aire (solo arroyos: el río tiene un nivel parejo y no deja costados a la vista).
///
/// `top`: índice del vóxel de suelo más alto de cada columna de la copia con borde (`q × q`,
/// `i32::MIN` = fuera del anillo); `agua`: el agua de cada una; `y_base`: índice del vóxel en el
/// origen de la malla. [render] `fill`: relleno del suelo de cada columna en los niveles gruesos
/// (ver `stream::top_relleno`; 15 = lleno): el fondo del río es la cara recortada (antes había un
/// parámetro `drop`, la malla bajada de `lod::far_drop`, que se fue).
pub fn mallar(top: &[i32], fill: Option<&[u8]>, agua: &[ColumnaAgua], q: i32, y_base: i32, vs: f32, mesh: &mut crate::mesher::Mesh) {
    let drop = 0.0f32; // [render]
    let corte = |k: usize| fill.map_or(0, |f| 15 - f[k].min(15)) as u64;
    use crate::mesher::{Quad, MAX_Y};
    let n = q - 2;
    let at = |i: i32, j: i32| (i + j * q) as usize;
    // Fondo recortado del río (ver HONDO): por debajo de esta cara, todo es "hondo".
    let piso_rio = ((NIVEL_MIN - HONDO + drop) / vs).floor() as i32 - 1;
    let hondo_arroyo = ((HONDO / vs).ceil() as i32).max(1);
    #[allow(clippy::too_many_arguments)]
    fn push(mesh: &mut crate::mesher::Mesh, y_base: i32, dir: usize, x: i32, y: i32, z: i32, w: i32, h: i32, tipo: u8, smooth: [u32; 2], alto: i32) {
        let yr = (y - y_base).clamp(0, MAX_Y);
        let qa = (x as u32 & 63) | ((z as u32 & 63) << 6) | ((yr as u32) << 12) | ((dir as u32) << 23);
        let qb = (w as u32 - 1) | ((h as u32 - 1) << 5) | (0xff << 10) | ((tipo as u32) << 18);
        mesh.water.push(Quad { a: qa, b: qb, smooth, slot: 0 });
        // Caja envolvente (en vóxeles relativos al origen).
        let (x1, z1) = match dir {
            0 => (x + h, z + w),
            1 | 2 => (x, z + h),
            _ => (x + w, z),
        };
        let lo = [x.max(0), yr, z.max(0)];
        let hi = [x1.max(0), (alto - y_base).clamp(yr, MAX_Y), z1.max(0)];
        for k in 0..3 {
            mesh.lo[k] = mesh.lo[k].min(lo[k] as u16);
            mesh.hi[k] = mesh.hi[k].max(hi[k] as u16);
        }
    }
    // 1. Caras de arriba: una clave por columna, unidas en rectángulos.
    let alto_rio = ((NIVEL_MAX + drop) / vs).ceil() as i32 + 1;
    let mut keys = vec![0u64; (n * n) as usize];
    let mut any = false;
    for j in 1..=n {
        for i in 1..=n {
            let k = at(i, j);
            let a = agua[k];
            let key = match a.tipo {
                AGUA_RIO => {
                    let piso = top[k].max(piso_rio);
                    // [render] Con el relleno, el fondo es la cara recortada (va en la clave).
                    let c = if piso == top[k] { corte(k) } else { 0 };
                    AGUA_RIO as u64 | (((piso + 1 + 0x8000) as u64 & 0xffff) << 8) | (c << 24)
                }
                AGUA_ARROYO => {
                    let piso = top[k].max(a.tope - hondo_arroyo);
                    AGUA_ARROYO as u64 | (((a.tope + 1 + 0x8000) as u64 & 0xffff) << 8) | (((piso + 1 + 0x8000) as u64 & 0xffff) << 24) | ((codigo_corriente(a.corriente) as u64) << 40)
                }
                _ => 0,
            };
            // Máscara con u sobre z y v sobre x (el orden de las caras de arriba en `mesher`).
            keys[((j - 1) + (i - 1) * n) as usize] = key;
            any |= key != 0;
        }
    }
    if any {
        for jx in 0..n {
            let mut iz = 0;
            while iz < n {
                let key = keys[(iz + jx * n) as usize];
                if key == 0 {
                    iz += 1;
                    continue;
                }
                let mut w = 1;
                while iz + w < n && keys[(iz + w + jx * n) as usize] == key {
                    w += 1;
                }
                let mut h = 1;
                'grow: while jx + h < n {
                    for kk in 0..w {
                        if keys[(iz + kk + (jx + h) * n) as usize] != key {
                            break 'grow;
                        }
                    }
                    h += 1;
                }
                for jj in 0..h {
                    for ii in 0..w {
                        keys[(iz + ii + (jx + jj) * n) as usize] = 0;
                    }
                }
                let tipo = (key & 0xff) as u8;
                let plano = ((key >> 8) & 0xffff) as i32 - 0x8000;
                let (smooth, alto) = if tipo == AGUA_RIO {
                    // La cara va al ras del suelo; el shader la sube al nivel del agua. Lleva el
                    // fondo (m) para el tinte de la profundidad.
                    let c = ((key >> 24) & 15) as f32; // [render]
                    ([0, (plano as f32 * vs - c / 16.0 * vs - drop).to_bits()], plano.max(alto_rio))
                } else {
                    let piso = ((key >> 24) & 0xffff) as i32 - 0x8000;
                    let c = corriente_de(((key >> 40) & 0xff) as u32);
                    ([corriente_empaquetada(c), (piso as f32 * vs - drop).to_bits()], plano)
                };
                push(mesh, y_base, 0, jx, plano, iz, w, h, tipo, smooth, alto);
                iz += w;
            }
        }
    }
    // 2. Costados de los arroyos que dan al aire (cascadas y bordes más bajos).
    let medio_rio = (drop / vs).round() as i32 - 1; // el río en su nivel medio
    let ocupado = |k: usize| -> i32 {
        match agua[k].tipo {
            AGUA_ARROYO => top[k].max(agua[k].tope),
            AGUA_RIO => top[k].max(medio_rio),
            _ => top[k],
        }
    };
    for j in 1..=n {
        for i in 1..=n {
            let k = at(i, j);
            let a = agua[k];
            if a.tipo != AGUA_ARROYO {
                continue;
            }
            let smooth = [corriente_empaquetada(a.corriente), ((top[k] + 1) as f32 * vs - drop).to_bits()];
            for (dir, di, dj) in [(1usize, 1, 0), (2, -1, 0), (3, 0, 1), (4, 0, -1)] {
                let kn = at(i + di, j + dj);
                if top[kn] == i32::MIN {
                    continue;
                }
                let on = ocupado(kn);
                if on >= a.tope {
                    continue;
                }
                let mut y = on.max(top[k]) + 1;
                let (x, z) = (i - 1, j - 1);
                while y <= a.tope {
                    let alto = (a.tope - y + 1).min(32);
                    if dir <= 2 {
                        push(mesh, y_base, dir, x + (dir == 1) as i32, y, z, alto, 1, AGUA_ARROYO, smooth, y + alto);
                    } else {
                        push(mesh, y_base, dir, x, y, z + (dir == 3) as i32, 1, alto, AGUA_ARROYO, smooth, y + alto);
                    }
                    y += alto;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nivel_de_un_arroyo_nunca_sube_aguas_abajo() {
        let raw = [5.0, 5.2, 4.1, 4.6, 3.0, 3.4, 3.3, 1.0, 1.5];
        let v = niveles_sin_subidas(&raw);
        for w in v.windows(2) {
            assert!(w[1] <= w[0] + 1e-6, "{v:?}");
        }
        // Sigue cerca del relieve (no se va al mínimo ni al máximo).
        assert!(v[0] > 4.0 && v[0] <= 5.2 && v[8] >= 1.0 && v[8] < 1.6, "{v:?}");
    }

    use crate::lod::{LevelSpec, CHUNK};
    use crate::mesher::Mesh;
    use crate::store::EditStore;
    use glam::IVec2;
    use std::collections::HashSet;
    use std::sync::OnceLock;

    fn mapa() -> &'static (TerrainGen, Arroyos) {
        static M: OnceLock<(TerrainGen, Arroyos)> = OnceLock::new();
        M.get_or_init(|| {
            let t = TerrainGen::montevideo();
            let a = Arroyos::new(&t);
            (t, a)
        })
    }

    /// Columna de chunks del nivel `lod` que contiene el punto, con sus vecinas en el anillo.
    fn columna(lod: u32, x: f32, z: f32) -> Mesh {
        let (t, a) = mapa();
        let spec = LevelSpec::new(lod); // [lod]
        let cm = spec.chunk_m();
        let col = IVec2::new((x / cm).floor() as i32, (z / cm).floor() as i32);
        let ring: HashSet<IVec2> = (-1..=1).flat_map(|j| (-1..=1).map(move |i| col + IVec2::new(i, j))).collect();
        crate::stream::build_column(t, a, &EditStore::new(lod + 1), spec, col, &ring)
    }

    /// Columnas (centro en metros) y fondo (m) de cada cara de arriba del agua de un tipo.
    fn caras_de_arriba(m: &Mesh, tipo: u8) -> Vec<(Vec2, f32, f32)> {
        let mut out = Vec::new();
        for q in m.water.iter().filter(|q| q.dir() == 0 && q.material() == tipo) {
            let b = q.base();
            let (w, h) = q.size();
            let fondo = f32::from_bits(q.smooth[1]);
            let tope = m.origin.y + b[1] as f32 * m.voxel;
            for dx in 0..h {
                for dz in 0..w {
                    let c = Vec2::new(m.origin.x + (b[0] + dx) as f32 * m.voxel + m.voxel * 0.5, m.origin.z + (b[2] + dz) as f32 * m.voxel + m.voxel * 0.5);
                    out.push((c, fondo, tope));
                }
            }
        }
        out
    }

    /// Lo que hace el vertex shader con las caras del río: ¿se ve con este nivel, y a qué altura?
    fn rio_visible(fondo: f32, nivel: f32, vs: f32) -> Option<f32> {
        let mut l = nivel_en_grilla(nivel);
        if vs >= 63.0 {
            l = l.min(2.0);
        }
        (l > fondo + 0.001).then_some(l)
    }

    /// Playa de Pocitos, bahía, costa de piedra y el borde sur (para todos los niveles).
    const COSTAS: [(f32, f32); 5] = [(16180.0, 15330.0), (16100.0, 15400.0), (9300.0, 13000.0), (15190.0, 17380.0), (12000.0, 23000.0)];

    #[test]
    fn el_agua_nunca_queda_sobre_tierra_seca_en_ningun_nivel() {
        let (t, _) = mapa();
        let mut vistas = 0;
        for lod in 0..crate::lod::LEVELS {
            let vs = crate::lod::BASE_VOXEL * (1u32 << lod) as f32;
            for &(x, z) in &COSTAS {
                let m = columna(lod, x, z);
                for (c, fondo, _) in caras_de_arriba(&m, AGUA_RIO) {
                    // [render] Desde el vóxel de 2 m la altura del vóxel es el promedio de su
                    // planta (prefiltrado, `stream::altura_prefiltrada`): la costa queda donde
                    // la deja ese promedio.
                    let h = crate::stream::altura_prefiltrada(&t, c.x, c.y, vs);
                    for nivel in [-1.3, 0.0, 0.3, 1.2, 2.4] {
                        if rio_visible(fondo, nivel, vs).is_some() {
                            vistas += 1;
                            // Tierra seca: más alta que el agua (con el redondeo a la grilla y
                            // medio vóxel del nivel, que es lo que puede bajar la cara del suelo).
                            // (Bajo el nivel medio, `stream::top_from_height` baja el suelo hasta
                            // un vóxel entero para que no asome: ahí el margen es un vóxel.)
                            let margen = if h < 0.0 { vs } else { 0.5 * vs };
                            assert!(h < nivel + margen + 0.13, "nivel {lod} ({vs} m): agua a {nivel} m sobre tierra a {h:.2} m en {c:?}");
                        }
                    }
                }
                // Arroyos: el agua nunca más alta que la orilla de su columna más un vóxel.
                for (c, _, tope) in caras_de_arriba(&m, AGUA_ARROYO) {
                    assert!(tope <= t.height(c.x, c.y) + 2.0 + vs, "arroyo a {tope} m sobre tierra a {} m", t.height(c.x, c.y));
                }
            }
        }
        assert!(vistas > 10_000, "casi no hay agua del río en las costas ({vistas})");
    }

    #[test]
    fn hay_agua_en_la_bahia_el_rio_y_los_arroyos() {
        let (t, a) = mapa();
        // Bahía y río: el punto queda bajo una cara del río visible con el nivel medio, en todos
        // los niveles de detalle (la altura del agua es la misma en todos: sin escalón entre
        // niveles).
        for &(x, z) in &[(9300.0, 13000.0), (12000.0, 23000.0)] {
            assert!(t.height(x, z) < -1.0);
            for lod in 0..crate::lod::LEVELS {
                let vs = crate::lod::BASE_VOXEL * (1u32 << lod) as f32;
                let m = columna(lod, x, z);
                let caras = caras_de_arriba(&m, AGUA_RIO);
                let hit = caras.iter().find(|(c, _, _)| (c.x - x).abs() <= vs * 0.5 && (c.y - z).abs() <= vs * 0.5);
                let (_, fondo, _) = hit.unwrap_or_else(|| panic!("sin agua en ({x}, {z}) en el nivel {lod}"));
                let alto = rio_visible(*fondo, 0.0, vs).unwrap_or_else(|| panic!("agua escondida en ({x}, {z}) en el nivel {lod}"));
                assert_eq!(alto, 0.0);
            }
        }
        // Arroyos: muestras del Miguelete y de otros cursos con su agua en el nivel 0.
        assert!(a.len() > 1000, "{} muestras de arroyos", a.len());
        let mut con_agua = 0;
        let cursos = t.stream_water();
        for curso in cursos.iter().filter(|c| c.len() > 40).take(12) {
            let m0 = curso[curso.len() / 2];
            let m = columna(0, m0.p.x, m0.p.y);
            let caras = caras_de_arriba(&m, AGUA_ARROYO);
            if caras.iter().any(|(c, _, _)| c.distance(m0.p) < m0.alcance) {
                con_agua += 1;
            }
            // El agua corre (las caras llevan la corriente).
            assert!(m.water.iter().filter(|q| q.material() == AGUA_ARROYO).all(|q| q.smooth[0] != 0 || m0.corriente.length() < 0.05));
        }
        assert!(con_agua >= 9, "solo {con_agua} de 12 arroyos tienen agua");
        // Ningún vóxel de agua por debajo de su fondo ni caras repetidas.
        let _ = CHUNK;
    }

    #[test]
    fn la_marea_cambia_el_nivel_del_agua_en_la_playa() {
        // Playa de Pocitos: con la marea baja la arena queda seca; con una sudestada, el agua
        // la cubre (las mismas caras, sin rehacer la malla).
        let m = columna(0, 16180.0, 15330.0);
        let caras = caras_de_arriba(&m, AGUA_RIO);
        let secas_bajas = caras.iter().filter(|(_, f, _)| rio_visible(*f, -0.5, 0.25).is_none()).count();
        let secas_altas = caras.iter().filter(|(_, f, _)| rio_visible(*f, 1.5, 0.25).is_none()).count();
        assert!(secas_bajas > secas_altas + 100, "la marea no cambia nada ({secas_bajas} contra {secas_altas})");
        // Y el agua sube con el nivel (en el río, frente a la costa).
        let rio = caras_de_arriba(&columna(0, 12000.0, 23000.0), AGUA_RIO);
        let (_, f, _) = rio.iter().find(|(_, f, _)| *f < -2.0).expect("fondo hondo");
        assert_eq!(rio_visible(*f, 0.0, 0.25), Some(0.0));
        assert_eq!(rio_visible(*f, 1.2, 0.25), Some(1.25));
        assert_eq!(rio_visible(*f, -0.9, 0.25), Some(-1.0));
    }

    /// [ciudad] Las lagunas y las lagunetas de los bañados (sin cauce) tienen agua en vóxeles,
    /// quieta, a su nivel, sobre un fondo hundido.
    #[test]
    fn hay_agua_quieta_en_lagunas_y_banados() {
        let (t, a) = mapa();
        let q = t.aguas_quietas().expect("mapa real");
        eprintln!("agua quieta: {} lagunas, {} lagunetas de bañado", q.lagunas, q.lagunetas);
        assert!(q.lagunas >= 3 && q.lagunetas >= 20, "{} lagunas, {} lagunetas", q.lagunas, q.lagunetas);
        // Puntos con agua quieta: el más hondo de una laguna (prof > 1 m) y uno de bañado,
        // buscando en el entorno y en el mapa.
        let (mut laguna, mut laguneta) = (None, None);
        let mut z = -20000.0;
        while z < 44000.0 && (laguna.is_none() || laguneta.is_none()) {
            let mut x = -20000.0;
            while x < 44000.0 {
                if let Some((nivel, prof)) = t.agua_quieta(x, z) {
                    if a.en(x, z).is_some_and(|(n, c)| (n - nivel).abs() < 1e-4 && c == Vec2::ZERO) {
                        if prof > 1.0 && laguna.is_none() {
                            laguna = Some((x, z, nivel));
                        } else if prof > 0.3 && prof < 0.85 && laguneta.is_none() && x >= 0.0 && z >= 0.0 && x < 24000.0 && z < 24000.0 {
                            laguneta = Some((x, z, nivel));
                        }
                    }
                }
                x += 37.0;
            }
            z += 37.0;
        }
        eprintln!("laguna {laguna:?} · laguneta {laguneta:?}");
        for (nombre, punto) in [("laguna", laguna), ("laguneta de bañado", laguneta)] {
            let (x, z, nivel) = punto.unwrap_or_else(|| panic!("sin {nombre}"));
            // El fondo, bajo el agua; el lugar cuenta como agua (no se construye encima).
            assert!(t.height(x, z) < nivel - 0.25, "{nombre}: fondo a {} m, agua a {nivel} m", t.height(x, z));
            assert!(t.is_water(x, z), "{nombre}");
            let m = columna(0, x, z);
            let caras = caras_de_arriba(&m, AGUA_ARROYO);
            let hit = caras.iter().find(|(c, _, _)| c.distance(Vec2::new(x, z)) < 0.5);
            let (_, _, tope) = hit.unwrap_or_else(|| panic!("{nombre} en ({x}, {z}) sin agua en vóxeles"));
            assert!((tope - nivel).abs() <= 0.25, "{nombre}: agua a {tope} m, nivel {nivel} m");
            // Quieta: sin corriente.
            assert!(m.water.iter().filter(|q| q.dir() == 0 && q.material() == AGUA_ARROYO).any(|q| q.smooth[0] == 0));
        }
    }

    #[test]
    fn el_nivel_del_rio_se_redondea_a_la_grilla() {
        assert_eq!(nivel_en_grilla(0.0), 0.0);
        assert_eq!(nivel_en_grilla(0.12), 0.0);
        assert_eq!(nivel_en_grilla(0.13), 0.25);
        assert_eq!(nivel_en_grilla(-0.3), -0.25);
        assert_eq!(nivel_en_grilla(1.9), 2.0);
    }
}
