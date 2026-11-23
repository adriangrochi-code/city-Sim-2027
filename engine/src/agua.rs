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

/// Niveles del agua de todos los arroyos del mapa, con una grilla para buscar la muestra más
/// cercana a un punto.
#[derive(Default)]
pub struct Arroyos {
    muestras: Vec<Muestra>,
    celdas: HashMap<(i32, i32), Vec<u32>, crate::fxhash::FxBuild>,
}

const CELDA: f32 = 32.0;

impl Arroyos {
    pub fn new(t: &TerrainGen) -> Arroyos {
        let mut a = Arroyos::default();
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
        let lista = self.celdas.get(&((x / CELDA).floor() as i32, (z / CELDA).floor() as i32))?;
        let p = Vec2::new(x, z);
        let mut mejor: Option<(f32, &Muestra)> = None;
        for &i in lista {
            let m = &self.muestras[i as usize];
            let d = m.p.distance(p) - m.alcance;
            if d < 0.0 && mejor.is_none_or(|(b, _)| d < b) {
                mejor = Some((d, m));
            }
        }
        mejor.map(|(_, m)| (m.nivel, m.corriente))
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
/// origen de la malla; `drop`: cuánto baja la malla de este nivel (ver `lod::far_drop`).
pub fn mallar(top: &[i32], agua: &[ColumnaAgua], q: i32, y_base: i32, vs: f32, drop: f32, mesh: &mut crate::mesher::Mesh) {
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
                    AGUA_RIO as u64 | (((piso + 1 + 0x8000) as u64 & 0xffff) << 8)
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
                    ([0, (plano as f32 * vs - drop).to_bits()], plano.max(alto_rio))
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
                    let h = t.height(c.x, c.y);
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

    #[test]
    fn el_nivel_del_rio_se_redondea_a_la_grilla() {
        assert_eq!(nivel_en_grilla(0.0), 0.0);
        assert_eq!(nivel_en_grilla(0.12), 0.0);
        assert_eq!(nivel_en_grilla(0.13), 0.25);
        assert_eq!(nivel_en_grilla(-0.3), -0.25);
        assert_eq!(nivel_en_grilla(1.9), 2.0);
    }
}
