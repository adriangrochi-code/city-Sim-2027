//! [lod] Horizonte y oclusión por relieve **antes de generar** (técnica del mod LOD de Minecraft:
//! `render/OclusionRelieve` y el horizonte real de `core/HorizonteCurvo`; ver
//! `docs/sistemas/tecnicas-minecraft-lod.md` §2.9 y §2.10).
//!
//! El Hi-Z descarta en la GPU lo tapado que ya se generó y se subió; acá se evita generarlo. Con
//! la cámara baja (calle, rasante) el relieve cercano y la curvatura de la Tierra tapan buena
//! parte de los anillos gruesos: esas columnas no se generan hasta que se vean.
//!
//! Dos piezas:
//! - [`Relieve`]: cotas (mínimo y máximo) del terreno por zona, en una pirámide de celdas
//!   muestreadas una vez por mapa (16 m en el mapa y 3 km alrededor, 512 m en el entorno amplio),
//!   con margen. Las cotas son **conservadoras**: el máximo nunca queda debajo del terreno y el
//!   mínimo nunca encima (lo comprueba un test contra el terreno muestreado fino).
//! - [`ocultas`]: barrido por sectores angulares desde el ojo, de cerca hacia afuera. Una caja
//!   está oculta si, en todos los sectores que toca, su elevación máxima posible queda debajo
//!   del horizonte armado con el **mínimo** de las cajas más cercanas. La elevación se mide como
//!   tangente con la bajada de la curvatura (`d²/2R`, la misma de `voxel.wgsl::curved`), con las
//!   cotas de distancia que la hacen conservadora en las dos puntas.

use crate::terrain::TerrainGen;
use glam::Vec2;
use std::sync::{Arc, Mutex, Weak};

/// 1 / 2R con R = 6371 km (la curvatura de `voxel.wgsl::curved`).
pub const K_CURVA: f32 = 1.0 / 12_742_000.0;

/// Margen de las cotas de la grilla fina sobre lo muestreado: médanos, afloramientos y piedras
/// de la orilla miden menos que una celda de 16 m; los cauces de los arroyos bajan el terreno.
const MARGEN_FINO_ALTO: f32 = 5.0;
const MARGEN_FINO_BAJO: f32 = 9.0;
/// En el entorno amplio (celdas de 512 m) el relieve es suave, pero la celda es grande.
const MARGEN_AMPLIO: f32 = 14.0;

struct Mip {
    nx: usize,
    nz: usize,
    lo: Vec<f32>,
    hi: Vec<f32>,
}

/// Cotas por celda de una grilla regular, con niveles cada vez más gruesos (mínimo y máximo de
/// 2×2 celdas del nivel anterior).
pub struct Grilla {
    x0: f32,
    z0: f32,
    celda: f32,
    mips: Vec<Mip>,
}

impl Grilla {
    /// Muestrea el terreno en las esquinas de las celdas (en paralelo) y arma la pirámide.
    fn build(f: &(dyn Fn(f32, f32) -> f32 + Sync), x0: f32, z0: f32, celda: f32, nx: usize, nz: usize, alto: f32, bajo: f32) -> Grilla {
        let sx = nx + 1;
        let mut esq = vec![0f32; sx * (nz + 1)];
        let hilos = std::thread::available_parallelism().map_or(2, |n| n.get()).max(1);
        let filas = (nz + 1).div_ceil(hilos);
        std::thread::scope(|sc| {
            for (k, parte) in esq.chunks_mut(filas * sx).enumerate() {
                sc.spawn(move || {
                    for (r, fila) in parte.chunks_mut(sx).enumerate() {
                        let z = z0 + (k * filas + r) as f32 * celda;
                        for (i, v) in fila.iter_mut().enumerate() {
                            *v = f(x0 + i as f32 * celda, z);
                        }
                    }
                });
            }
        });
        let (mut lo, mut hi) = (vec![0f32; nx * nz], vec![0f32; nx * nz]);
        for j in 0..nz {
            for i in 0..nx {
                let c = [esq[i + j * sx], esq[i + 1 + j * sx], esq[i + (j + 1) * sx], esq[i + 1 + (j + 1) * sx]];
                let (mn, mx) = c.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
                // Entre esquinas el terreno puede subir o bajar más que ellas: la mitad del
                // desnivel de la celda más un margen fijo.
                let m = 0.5 * (mx - mn);
                lo[i + j * nx] = mn - m - bajo;
                hi[i + j * nx] = mx + m + alto;
            }
        }
        let mut mips = vec![Mip { nx, nz, lo, hi }];
        while mips.last().is_some_and(|m| m.nx > 1 || m.nz > 1) {
            let p = mips.last().unwrap();
            let (nx2, nz2) = (p.nx.div_ceil(2), p.nz.div_ceil(2));
            let (mut lo, mut hi) = (vec![f32::MAX; nx2 * nz2], vec![f32::MIN; nx2 * nz2]);
            for j in 0..p.nz {
                for i in 0..p.nx {
                    let k = i / 2 + (j / 2) * nx2;
                    lo[k] = lo[k].min(p.lo[i + j * p.nx]);
                    hi[k] = hi[k].max(p.hi[i + j * p.nx]);
                }
            }
            mips.push(Mip { nx: nx2, nz: nz2, lo, hi });
        }
        Grilla { x0, z0, celda, mips }
    }

    fn x1(&self) -> f32 {
        self.x0 + self.mips[0].nx as f32 * self.celda
    }
    fn z1(&self) -> f32 {
        self.z0 + self.mips[0].nz as f32 * self.celda
    }

    /// Cotas del rectángulo recortado a la grilla (None si no la toca).
    fn cotas(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> Option<(f32, f32)> {
        let (x0, z0, x1, z1) = (x0.max(self.x0), z0.max(self.z0), x1.min(self.x1()), z1.min(self.z1()));
        if x0 >= x1 || z0 >= z1 {
            return None;
        }
        // Nivel de la pirámide con a lo sumo ~4 celdas por lado.
        let lado = (x1 - x0).max(z1 - z0);
        let mut k = 0;
        while k + 1 < self.mips.len() && self.celda * (1u32 << k) as f32 * 4.0 < lado {
            k += 1;
        }
        let m = &self.mips[k];
        let c = self.celda * (1u32 << k) as f32;
        let i0 = (((x0 - self.x0) / c).floor() as usize).min(m.nx - 1);
        let i1 = ((((x1 - self.x0) / c).ceil() as usize).max(i0 + 1)).min(m.nx);
        let j0 = (((z0 - self.z0) / c).floor() as usize).min(m.nz - 1);
        let j1 = ((((z1 - self.z0) / c).ceil() as usize).max(j0 + 1)).min(m.nz);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for j in j0..j1 {
            for i in i0..i1 {
                lo = lo.min(m.lo[i + j * m.nx]);
                hi = hi.max(m.hi[i + j * m.nx]);
            }
        }
        Some((lo, hi))
    }

    fn contiene(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> bool {
        x0 >= self.x0 && z0 >= self.z0 && x1 <= self.x1() && z1 <= self.z1()
    }
}

/// Cotas del terreno de un mapa: grilla fina sobre el mapa jugable (y 3 km alrededor) y gruesa
/// sobre el entorno amplio.
pub struct Relieve {
    fina: Grilla,
    amplia: Grilla,
    /// Densidad del monte (máximo por celda, 64 m y 512 m): dónde puede haber copas del monte en
    /// los niveles gruesos (`stream::forest_canopy`, un vóxel entero sobre el suelo).
    monte_fina: Grilla,
    monte_amplia: Grilla,
}

impl Relieve {
    pub fn build(t: &TerrainGen) -> Relieve {
        let m = crate::terrain::MAP_SIZE;
        let (f0, fc) = (-3072.0, 16.0);
        let nf = ((m + 2.0 * 3072.0) / fc).ceil() as usize;
        let h = |x: f32, z: f32| t.height(x, z);
        let fina = Grilla::build(&h, f0, f0, fc, nf, nf, MARGEN_FINO_ALTO, MARGEN_FINO_BAJO);
        // El último anillo llega a ~197 km de lado: ±180 km alrededor del centro del mapa.
        let (a0, ac) = (m * 0.5 - 180_000.0, 512.0);
        let na = (360_000.0 / ac) as usize;
        let amplia = Grilla::build(&h, a0, a0, ac, na, na, MARGEN_AMPLIO, MARGEN_AMPLIO);
        let d = |x: f32, z: f32| if t.is_real() { t.forest_density(x, z) } else { 0.0 };
        let monte_fina = Grilla::build(&d, f0, f0, 64.0, nf / 4, nf / 4, 0.0, 0.0);
        let monte_amplia = Grilla::build(&d, a0, a0, ac, na, na, 0.0, 0.0);
        Relieve { fina, amplia, monte_fina, monte_amplia }
    }

    /// ¿Puede haber copas del monte en el rectángulo? (con una celda de margen alrededor: el
    /// monte de galería de los arroyos es más angosto que la celda).
    pub fn monte(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> bool {
        let g = if self.monte_fina.contiene(x0, z0, x1, z1) { &self.monte_fina } else { &self.monte_amplia };
        let c = g.celda;
        g.cotas(x0 - c, z0 - c, x1 + c, z1 + c).is_none_or(|(_, hi)| hi > 0.1)
    }

    /// Cotas (mínimo, máximo) del terreno en el rectángulo; None si sale de los datos. Donde la
    /// grilla fina no alcanza se combina con la amplia (la combinación sigue siendo conservadora:
    /// cada parte la cubre una grilla que vale para ella).
    pub fn cotas(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> Option<(f32, f32)> {
        if self.fina.contiene(x0, z0, x1, z1) {
            return self.fina.cotas(x0, z0, x1, z1);
        }
        if !self.amplia.contiene(x0, z0, x1, z1) {
            return None;
        }
        let a = self.amplia.cotas(x0, z0, x1, z1)?;
        Some(match self.fina.cotas(x0, z0, x1, z1) {
            Some(f) => (a.0.min(f.0), a.1.max(f.1)),
            None => a,
        })
    }
}

/// Pirámides ya armadas, una por terreno (los bancos y las partidas crean varios `Streamer`).
static CACHE: Mutex<Vec<(Weak<TerrainGen>, Arc<Relieve>)>> = Mutex::new(Vec::new());

/// La pirámide del terreno (la arma si hace falta; tarda ~1 s, se llama desde un hilo aparte).
pub fn obtener(t: &Arc<TerrainGen>) -> Arc<Relieve> {
    {
        let mut c = CACHE.lock().unwrap();
        c.retain(|(w, _)| w.strong_count() > 0);
        if let Some((_, r)) = c.iter().find(|(w, _)| w.as_ptr() == Arc::as_ptr(t)) {
            return r.clone();
        }
    }
    let r = Arc::new(Relieve::build(t));
    CACHE.lock().unwrap().push((Arc::downgrade(t), r.clone()));
    r
}

/// Una zona (columna de chunks) para la oclusión: rectángulo, cotas de lo que se va a dibujar y
/// si puede tapar a otras.
#[derive(Clone, Copy, Debug)]
pub struct Caja {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
    /// Lo más bajo que puede quedar su superficie dibujada (para tapar).
    pub lo: f32,
    /// Lo más alto que puede llegar algo dibujado en ella (para ser tapada).
    pub hi: f32,
    pub oclusor: bool,
}

/// Sectores angulares del horizonte (0,18° cada uno).
pub const SECTORES: usize = 2048;

/// ¿Qué cajas quedan ocultas desde el ojo (`ojo` en planta, a la altura `e`)? Ninguna a menos de
/// `d_oculta` del ojo, y solo tapan las que empiezan a `d_oclusor` o más.
pub fn ocultas(ojo: Vec2, e: f32, cajas: &[Caja], d_oculta: f32, d_oclusor: f32) -> Vec<bool> {
    use std::f32::consts::{PI, TAU};
    let n = cajas.len();
    let delta = TAU / SECTORES as f32;
    // Distancia mínima y máxima al ojo y arco de ángulos de cada caja.
    struct Geo {
        dn: f32,
        df: f32,
        a0: f32,
        a1: f32,
    }
    let geo: Vec<Option<Geo>> = cajas
        .iter()
        .map(|c| {
            let dx = (c.x0 - ojo.x).max(0.0).max(ojo.x - c.x1);
            let dz = (c.z0 - ojo.y).max(0.0).max(ojo.y - c.z1);
            let dn = (dx * dx + dz * dz).sqrt();
            if dn < 1.0 {
                return None; // el ojo está encima: no tapa ni se tapa
            }
            let esq = [(c.x0, c.z0), (c.x1, c.z0), (c.x0, c.z1), (c.x1, c.z1)];
            let df = esq.iter().map(|&(x, z)| Vec2::new(x - ojo.x, z - ojo.y).length()).fold(0.0, f32::max);
            let ac = ((c.z0 + c.z1) * 0.5 - ojo.y).atan2((c.x0 + c.x1) * 0.5 - ojo.x);
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for &(x, z) in &esq {
                let mut d = (z - ojo.y).atan2(x - ojo.x) - ac;
                if d > PI {
                    d -= TAU;
                } else if d < -PI {
                    d += TAU;
                }
                lo = lo.min(d);
                hi = hi.max(d);
            }
            let a0 = (ac + lo).rem_euclid(TAU);
            Some(Geo { dn, df, a0, a1: a0 + (hi - lo) })
        })
        .collect();
    let mut oclusores: Vec<usize> = (0..n).filter(|&i| cajas[i].oclusor && geo[i].as_ref().is_some_and(|g| g.dn >= d_oclusor)).collect();
    oclusores.sort_by(|&a, &b| geo[a].as_ref().unwrap().df.total_cmp(&geo[b].as_ref().unwrap().df));
    let mut candidatas: Vec<usize> = (0..n).filter(|&i| geo[i].as_ref().is_some_and(|g| g.dn >= d_oculta)).collect();
    candidatas.sort_by(|&a, &b| geo[a].as_ref().unwrap().dn.total_cmp(&geo[b].as_ref().unwrap().dn));
    let mut horizonte = vec![f32::MIN; SECTORES];
    let mut out = vec![false; n];
    let mut k = 0;
    for &i in &candidatas {
        let g = geo[i].as_ref().unwrap();
        // Se suman al horizonte las cajas que terminan antes de que empiece esta.
        while k < oclusores.len() && geo[oclusores[k]].as_ref().unwrap().df < g.dn {
            let o = oclusores[k];
            let go = geo[o].as_ref().unwrap();
            // Tangente mínima posible de la superficie del oclusor (cota inferior).
            let l = cajas[o].lo - e;
            let t = if l > 0.0 { l / go.df } else { l / go.dn } - K_CURVA * go.df;
            // Solo los sectores enteros dentro de su arco.
            let s0 = (go.a0 / delta).ceil() as i64;
            let s1 = (go.a1 / delta).floor() as i64;
            for s in s0..s1 {
                let h = &mut horizonte[s.rem_euclid(SECTORES as i64) as usize];
                *h = h.max(t);
            }
            k += 1;
        }
        // Tangente máxima posible de lo que se dibuje en la caja (cota superior).
        let hh = cajas[i].hi - e;
        let t = if hh > 0.0 { hh / g.dn } else { hh / g.df } - K_CURVA * g.dn;
        let s0 = (g.a0 / delta).floor() as i64;
        let s1 = (g.a1 / delta).floor() as i64;
        out[i] = (s0..=s1).all(|s| horizonte[s.rem_euclid(SECTORES as i64) as usize] > t + 1e-6);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caja(x0: f32, z0: f32, lado: f32, lo: f32, hi: f32) -> Caja {
        Caja { x0, z0, x1: x0 + lado, z1: z0 + lado, lo, hi, oclusor: true }
    }

    #[test]
    fn una_loma_tapa_lo_bajo_de_atras_y_no_lo_alto() {
        // Ojo a 2 m sobre un llano; una loma de 30 m a 400 m al norte (-z), de 200 m de ancho.
        let ojo = Vec2::ZERO;
        let mut c = vec![caja(-100.0, -440.0, 200.0, 30.0, 35.0)];
        // Detrás de la loma, a 1 km: un bajo (5 m), una torre de 120 m y algo fuera del arco.
        c.push(caja(-20.0, -1040.0, 40.0, 0.0, 5.0));
        c.push(caja(-20.0, -1100.0, 40.0, 0.0, 120.0));
        c.push(caja(400.0, -1040.0, 40.0, 0.0, 5.0));
        // Delante de la loma: nunca oculta.
        c.push(caja(-20.0, -300.0, 40.0, 0.0, 1.0));
        let o = ocultas(ojo, 2.0, &c, 150.0, 100.0);
        assert_eq!(o, vec![false, true, false, false, false]);
    }

    #[test]
    fn la_curvatura_esconde_el_llano_lejano_pero_no_las_sierras() {
        // Ojo a 3 m sobre el agua; anillos de cajas a nivel del mar cada 2 km hasta 100 km. Con
        // el agua como piso (cota 0) el horizonte está a ~6 km: el llano a 40 km se esconde, una
        // sierra de 300 m a 40 km no (su horizonte propio está a ~62 km).
        let ojo = Vec2::ZERO;
        let mut c = Vec::new();
        for r in 1..50 {
            let d = r as f32 * 2000.0;
            c.push(caja(-1000.0, -d - 2000.0, 2000.0, 0.0, 0.5));
        }
        let llano = c.len();
        c.push(caja(-1000.0, -42_000.0, 2000.0, 0.0, 0.5));
        let sierra = c.len();
        c.push(caja(-1000.0, -42_000.0, 2000.0, 0.0, 300.0));
        let o = ocultas(ojo, 3.0, &c, 300.0, 150.0);
        assert!(o[llano], "el llano a 40 km debería quedar bajo el horizonte");
        assert!(!o[sierra], "la sierra a 40 km se ve");
        assert!(!o[1], "lo de 4 km se ve (antes del horizonte)");
    }

    /// Las cotas de la pirámide son conservadoras contra el terreno muestreado fino (cada 2 m en
    /// zonas chicas, cada 16 m en las grandes), en el mapa real y alrededor.
    #[test]
    fn las_cotas_del_relieve_nunca_quedan_dentro_del_terreno() {
        let t = Arc::new(TerrainGen::montevideo());
        let r = obtener(&t);
        let mut h = 0x1234_5678u32;
        let mut rnd = || {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            h as f32 / u32::MAX as f32
        };
        for k in 0..900 {
            let (lado, paso, alcance) = match k % 3 {
                0 => (8.0, 1.0, 26000.0),
                1 => (64.0, 4.0, 30000.0),
                _ => (2048.0, 32.0, 120_000.0),
            };
            let x0 = 12000.0 + (rnd() * 2.0 - 1.0) * alcance;
            let z0 = 12000.0 + (rnd() * 2.0 - 1.0) * alcance;
            let Some((lo, hi)) = r.cotas(x0, z0, x0 + lado, z0 + lado) else { continue };
            let n = (lado / paso) as i32;
            for j in 0..=n {
                for i in 0..=n {
                    let (x, z) = (x0 + i as f32 * paso, z0 + j as f32 * paso);
                    let v = t.height(x, z);
                    assert!(v <= hi && v >= lo, "en ({x:.0}, {z:.0}) la altura {v:.2} sale de las cotas [{lo:.2}, {hi:.2}] (zona de {lado} m)");
                }
            }
        }
    }
}
