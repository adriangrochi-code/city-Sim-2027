//! [ciudad] Islas con su contorno real: la de Flores, la Gorriti, la de Lobos y las de frente a
//! Colonia (`assets/mapas/montevideo/islas.json`, armado por `tools/mapa_real/construir_islas.py`
//! con la capa de islas de la IDE y las alturas de Copernicus GLO-90; ver
//! `docs/sistemas/mapa-real.md#islas`).
//!
//! Las grillas del relieve no alcanzan para islas tan chicas (en la del entorno amplio, de
//! 500 m, ni una celda). Cada isla guarda la distancia con signo a su contorno en una grilla
//! fina propia (5 m), y el terreno la levanta sobre el agua en cualquier nivel de detalle: un
//! domo que sube desde la orilla (0,3 m en el contorno) hasta su altura, y afuera un fondo que
//! baja al río. Las que la grilla del entorno ya tenía engrosadas (`recortar`, la de Flores) se
//! recortan a su contorno.

use glam::Vec2;

/// El archivo de las islas (contornos, alturas y nombres).
pub const ISLAS_JSON: &str = include_str!("../../assets/mapas/montevideo/islas.json");

/// Lado de las celdas de la distancia al contorno (m).
const PASO: f32 = 5.0;
/// Margen alrededor del contorno (m): hasta dónde la isla toca el fondo del río (y hasta dónde
/// se recorta lo que la grilla gruesa engrosó).
const MARGEN: f32 = 400.0;
/// Altura mínima de una isla (m): Copernicus (90 m) no ve las más chicas [E].
const ALTO_MIN: f32 = 2.5;

#[derive(serde::Deserialize)]
struct Parte {
    alto_m: f32,
    poligono: Vec<[f32; 2]>,
    /// El contorno en (lon, lat), más simplificado (para el mapa del país).
    #[serde(default)]
    geo: Vec<[f64; 2]>,
}

#[derive(serde::Deserialize)]
struct IslaJson {
    nombre: String,
    #[serde(default)]
    recortar: bool,
    /// `false`: solo para el mapa del país (la grilla del terreno ya la tiene, o queda fuera).
    #[serde(default = "si")]
    terreno: bool,
    partes: Vec<Parte>,
}

fn si() -> bool {
    true
}

#[derive(serde::Deserialize)]
struct Doc {
    islas: Vec<IslaJson>,
}

/// Una isla: nombre, caja, altura y la distancia con signo a su contorno (+ adentro).
pub struct Isla {
    pub nombre: String,
    pub recortar: bool,
    pub alto: f32,
    /// Distancia máxima adentro (m): el centro de la isla.
    pub hondo: f32,
    lo: Vec2,
    nx: usize,
    nz: usize,
    dist: Vec<f32>,
}

/// Lo que la isla dice de un punto.
#[derive(Clone, Copy, Debug)]
pub struct EnIsla {
    /// Distancia con signo al contorno (m; positiva adentro).
    pub d: f32,
    pub alto: f32,
    pub hondo: f32,
    pub recortar: bool,
}

fn dist_segmento(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn dentro(p: Vec2, pts: &[Vec2]) -> bool {
    let mut c = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

impl Isla {
    fn new(j: IslaJson) -> Isla {
        let partes: Vec<Vec<Vec2>> = j.partes.iter().map(|p| p.poligono.iter().map(|q| Vec2::new(q[0], q[1])).collect()).filter(|v: &Vec<Vec2>| v.len() >= 3).collect();
        let alto = j.partes.iter().map(|p| p.alto_m).fold(0.0f32, f32::max).max(ALTO_MIN);
        let (mut lo, mut hi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
        for p in partes.iter().flatten() {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        lo -= Vec2::splat(MARGEN);
        hi += Vec2::splat(MARGEN);
        let nx = ((hi.x - lo.x) / PASO).ceil() as usize + 1;
        let nz = ((hi.y - lo.y) / PASO).ceil() as usize + 1;
        let mut dist = vec![0.0f32; nx * nz];
        let mut hondo = 0.0f32;
        for k in 0..nz {
            for i in 0..nx {
                let p = lo + Vec2::new(i as f32, k as f32) * PASO;
                let mut d = f32::INFINITY;
                let mut adentro = false;
                for pts in &partes {
                    for w in 0..pts.len() {
                        d = d.min(dist_segmento(p, pts[w], pts[(w + 1) % pts.len()]));
                    }
                    adentro |= dentro(p, pts);
                }
                let s = if adentro { d } else { -d };
                hondo = hondo.max(s);
                dist[i + k * nx] = s;
            }
        }
        Isla { nombre: j.nombre, recortar: j.recortar, alto, hondo: hondo.max(PASO), lo, nx, nz, dist }
    }

    /// Distancia con signo al contorno (bilineal), si el punto cae en la caja de la isla.
    fn distancia(&self, x: f32, z: f32) -> Option<f32> {
        let fx = (x - self.lo.x) / PASO;
        let fz = (z - self.lo.y) / PASO;
        if fx < 0.0 || fz < 0.0 || fx >= (self.nx - 1) as f32 || fz >= (self.nz - 1) as f32 {
            return None;
        }
        let (i, k) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - i as f32, fz - k as f32);
        let d = |i: usize, k: usize| self.dist[i + k * self.nx];
        let a = d(i, k) + (d(i + 1, k) - d(i, k)) * tx;
        let b = d(i, k + 1) + (d(i + 1, k + 1) - d(i, k + 1)) * tx;
        Some(a + (b - a) * tz)
    }
}

/// Islas para el mapa del país: nombre y contornos en (lon, lat).
pub fn contornos_geo() -> &'static [(String, Vec<Vec<[f64; 2]>>)] {
    static C: std::sync::OnceLock<Vec<(String, Vec<Vec<[f64; 2]>>)>> = std::sync::OnceLock::new();
    C.get_or_init(|| serde_json::from_str::<Doc>(ISLAS_JSON).map_or(Vec::new(), |d| d.islas.into_iter().map(|i| (i.nombre, i.partes.into_iter().map(|p| p.geo).filter(|g| g.len() >= 3).collect())).collect()))
}

/// Las islas del archivo que el terreno levanta.
#[derive(Default)]
pub struct Islas {
    pub islas: Vec<Isla>,
}

impl Islas {
    pub fn cargar(json: &str) -> Islas {
        match serde_json::from_str::<Doc>(json) {
            Ok(d) => Islas { islas: d.islas.into_iter().filter(|i| i.terreno).map(Isla::new).collect() },
            Err(e) => {
                eprintln!("islas.json: {e}");
                Islas::default()
            }
        }
    }

    /// La isla que contiene (o casi) el punto.
    pub fn en(&self, x: f32, z: f32) -> Option<EnIsla> {
        self.islas.iter().find_map(|i| i.distancia(x, z).map(|d| EnIsla { d, alto: i.alto, hondo: i.hondo, recortar: i.recortar }))
    }

    /// Nombre de la isla que contiene el punto (adentro de su contorno).
    pub fn nombre_en(&self, x: f32, z: f32) -> Option<&str> {
        self.islas.iter().find(|i| i.distancia(x, z).is_some_and(|d| d >= 0.0)).map(|i| i.nombre.as_str())
    }

    /// Altura del terreno con las islas: adentro, un domo que sube desde 0,3 m en la orilla
    /// hasta la altura de la isla (alcanzada a un 60 % del camino al centro); afuera, un fondo
    /// que baja 8 cm por metro. `bump` (0..1) da el relieve fino de la piedra.
    pub fn altura(&self, h: f32, x: f32, z: f32, bump: f32) -> f32 {
        let Some(e) = self.en(x, z) else { return h };
        let perfil = if e.d >= 0.0 {
            let t = (e.d / (e.hondo * 0.6)).min(1.0);
            let s = t * t * (3.0 - 2.0 * t);
            0.3 + (e.alto - 0.3) * s * (0.85 + 0.15 * bump)
        } else {
            0.3 + e.d * 0.08
        };
        if e.d >= 0.0 {
            h.max(perfil)
        } else if e.recortar {
            // La grilla engrosó la isla: lo que sobra fuera del contorno vuelve al agua (y el
            // recorte se apaga antes del borde de la caja, sin escalón).
            let w = 1.0 - ((-e.d - MARGEN * 0.6) / (MARGEN * 0.35)).clamp(0.0, 1.0);
            h + (h.min(perfil.max(-4.0)) - h) * w
        } else {
            h.max(perfil)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::terrain::TerrainGen;

    /// Cada isla del archivo sale del agua con su forma: la tierra seca ocupa más o menos el
    /// área del contorno oficial, llega cerca de su altura y a 150 m de la orilla hay agua.
    #[test]
    fn las_islas_salen_del_agua_con_su_contorno() {
        let t = TerrainGen::montevideo();
        let doc: super::Doc = serde_json::from_str(super::ISLAS_JSON).unwrap();
        assert!(doc.islas.iter().filter(|i| i.terreno).count() >= 3);
        // Las del mapa del país, con su contorno en (lon, lat) dentro del Río de la Plata.
        let geo = super::contornos_geo();
        assert!(geo.len() >= 6 && geo.iter().all(|(_, p)| !p.is_empty() && p.iter().flatten().all(|q| (-58.5..-54.0).contains(&q[0]) && (-35.2..-34.3).contains(&q[1]))));
        for isla in doc.islas.iter().filter(|i| i.terreno) {
            let area: f32 = isla.partes.iter().map(|p| {
                let v = &p.poligono;
                (0..v.len()).map(|i| v[i][0] * v[(i + 1) % v.len()][1] - v[(i + 1) % v.len()][0] * v[i][1]).sum::<f32>().abs() * 0.5
            }).sum();
            let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
            for q in isla.partes.iter().flat_map(|p| &p.poligono) {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            let paso = 5.0;
            let (mut seca, mut alto) = (0.0f32, f32::NEG_INFINITY);
            let mut z = lo[1] - 150.0;
            while z <= hi[1] + 150.0 {
                let mut x = lo[0] - 150.0;
                while x <= hi[0] + 150.0 {
                    let h = t.height(x, z);
                    alto = alto.max(h);
                    if h > 0.25 {
                        seca += paso * paso;
                    }
                    x += paso;
                }
                z += paso;
            }
            let pedido = isla.partes.iter().map(|p| p.alto_m).fold(0.0f32, f32::max).max(super::ALTO_MIN);
            assert!(seca > area * 0.6 && seca < area * 1.6, "{}: {:.1} ha secas, contorno {:.1} ha", isla.nombre, seca / 1e4, area / 1e4);
            assert!(alto > pedido * 0.75, "{}: alto {alto:.1} m de {pedido} m", isla.nombre);
            // Agua alrededor, a 150 m de la caja.
            for (x, z) in [(lo[0] - 150.0, (lo[1] + hi[1]) * 0.5), (hi[0] + 150.0, (lo[1] + hi[1]) * 0.5)] {
                assert!(t.height(x, z) < 0.0, "{}: tierra a 150 m de la orilla ({x}, {z}): {}", isla.nombre, t.height(x, z));
            }
            // El nombre, adentro de su contorno.
            let mut con_nombre = false;
            let mut z = lo[1];
            while z <= hi[1] && !con_nombre {
                let mut x = lo[0];
                while x <= hi[0] && !con_nombre {
                    con_nombre = t.isla(x, z) == Some(isla.nombre.as_str());
                    x += 10.0;
                }
                z += 10.0;
            }
            assert!(con_nombre, "{}", isla.nombre);
        }
    }
}
