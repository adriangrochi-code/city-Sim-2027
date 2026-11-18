//! Caminos y rutas del país: trazado de caminos sobre el relieve (A*), pasos de río y la red
//! por la que las carretas llevan las cargas a Montevideo (Dijkstra sobre los pueblos), con los
//! puertos del río y, desde la época 3, el ferrocarril.

use super::datos::{self, geo};
use super::{Obra, Paso, Pais};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::OnceLock;

/// Costo de andar con carretas por una celda (como `historia_pais._costo`): llano 1, las
/// pendientes, los ríos, los bañados y el monte cuestan más; el agua grande y el extranjero se
/// evitan.
fn costo_celda(c: usize, f: usize, anio: i32) -> f32 {
    let g = geo();
    let (x, y) = (c as f32, f as f32);
    if g.es_agua(x, y, anio) {
        return 60.0;
    }
    let b = g.capas[f * g.ancho() + c];
    let mut k = 1.0 + 0.4 * b[datos::C_PENDIENTE] as f32;
    if b[datos::C_DEPTO] == 0 {
        k += 30.0;
    }
    k += b[datos::C_BANADO] as f32 / 255.0 * 3.0 + b[datos::C_MONTE] as f32 / 255.0 * 1.0;
    // Cruzar un río cuesta: el camino lo cruza de frente y no corre por el cauce.
    if b[datos::C_RIO] > 40 {
        k += 25.0;
    }
    k
}

#[derive(Copy, Clone, PartialEq)]
struct Nodo(f32, usize);

impl Eq for Nodo {}
impl Ord for Nodo {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal).then(o.1.cmp(&self.1))
    }
}
impl PartialOrd for Nodo {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Traza un camino entre dos puntos (celdas) por donde es más fácil andar: A* con 8 vecinos y
/// la polilínea simplificada.
pub fn trazar(a: [f32; 2], b: [f32; 2], anio: i32) -> Vec<[f32; 2]> {
    let g = geo();
    let (w, h) = (g.ancho(), g.alto());
    let cl = |p: [f32; 2]| ((p[0].round() as usize).min(w - 1), (p[1].round() as usize).min(h - 1));
    let (s, t) = (cl(a), cl(b));
    let (si, ti) = (s.1 * w + s.0, t.1 * w + t.0);
    let mut dist = vec![f32::INFINITY; w * h];
    let mut prev = vec![u32::MAX; w * h];
    let mut costo = vec![f32::NAN; w * h];
    let mut cost_at = |k: usize| {
        if costo[k].is_nan() {
            costo[k] = costo_celda(k % w, k / w, anio);
        }
        costo[k]
    };
    let heur = |k: usize| (((k % w) as f32 - t.0 as f32).powi(2) + ((k / w) as f32 - t.1 as f32).powi(2)).sqrt();
    let mut pq = BinaryHeap::new();
    dist[si] = 0.0;
    pq.push(Nodo(heur(si), si));
    const V: [(i32, i32, f32); 8] = [(1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414)];
    while let Some(Nodo(_, k)) = pq.pop() {
        if k == ti {
            break;
        }
        let gk = dist[k];
        let ck = cost_at(k);
        let (x, y) = ((k % w) as i32, (k / w) as i32);
        for (dx, dy, l) in V {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let q = ny as usize * w + nx as usize;
            let nd = gk + l * 0.5 * (ck + cost_at(q));
            if nd < dist[q] {
                dist[q] = nd;
                prev[q] = k as u32;
                pq.push(Nodo(nd + heur(q), q));
            }
        }
    }
    if !dist[ti].is_finite() {
        return vec![a, b];
    }
    let mut celdas = vec![ti];
    while *celdas.last().unwrap() != si {
        let p = prev[*celdas.last().unwrap()];
        if p == u32::MAX {
            break;
        }
        celdas.push(p as usize);
    }
    celdas.reverse();
    let mut pts: Vec<[f32; 2]> = vec![a];
    pts.extend(celdas.iter().skip(1).take(celdas.len().saturating_sub(2)).map(|&k| [(k % w) as f32, (k / w) as f32]));
    pts.push(b);
    rdp(&pts, 0.7)
}

/// Simplificación de Ramer–Douglas–Peucker.
pub fn rdp(pts: &[[f32; 2]], tol: f32) -> Vec<[f32; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (mut k, mut dmax) = (0, -1.0);
    for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = datos::dist_seg(*p, a, b);
        if d > dmax {
            k = i;
            dmax = d;
        }
    }
    if dmax > tol {
        let mut l = rdp(&pts[..=k], tol);
        l.pop();
        l.extend(rdp(&pts[k..], tol));
        l
    } else {
        vec![a, b]
    }
}

/// Pasos de río de un trazado: los tramos donde cruza un río con nombre o agua. Los ríos
/// navegables y los cruces anchos empiezan con balsa; los demás, vado.
pub fn pasos_de(pts: &[[f32; 2]], anio: i32) -> Vec<Paso> {
    let g = geo();
    let mut out: Vec<Paso> = Vec::new();
    let mut run: Option<([f32; 2], [f32; 2], f32)> = None;
    let paso = 0.25;
    let cerrar = |run: &mut Option<([f32; 2], [f32; 2], f32)>, out: &mut Vec<Paso>| {
        if let Some((a, b, n)) = run.take() {
            let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
            let (rio, nav) = g.rio_cerca(mid, 2.5).map_or(("arroyo".to_string(), false), |(r, _)| (r.nombre.clone(), r.navegable));
            let ancho = (n * paso * g.g.km_x).max(0.05);
            let obra = if nav || ancho > 0.8 { Obra::Balsa } else { Obra::Vado };
            // Dos cruces seguidos del mismo río (meandro) cuentan como uno.
            if let Some(u) = out.last() {
                if u.rio == rio && g.km(u.pos, mid) < 5.0 {
                    return;
                }
            }
            out.push(Paso { pos: mid, rio, ancho, obra });
        }
    };
    for w in pts.windows(2) {
        let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        let n = (l / paso).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = k as f32 / n as f32;
            let p = [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
            let mojado = g.capa(p[0], p[1], datos::C_RIO) > 50 || g.es_agua(p[0], p[1], anio);
            match (&mut run, mojado) {
                (Some(r), true) => {
                    r.1 = p;
                    r.2 += 1.0;
                }
                (None, true) => run = Some((p, p, 1.0)),
                (Some(_), false) => cerrar(&mut run, &mut out),
                _ => {}
            }
        }
    }
    cerrar(&mut run, &mut out);
    out
}

/// Distancia (km) a la frontera con el Brasil y el punto de la frontera más cercano.
struct Frontera {
    km: Vec<f32>,
    cerca: Vec<u32>,
}

fn frontera() -> &'static Frontera {
    static F: OnceLock<Frontera> = OnceLock::new();
    F.get_or_init(|| {
        let g = geo();
        let (w, h) = (g.ancho(), g.alto());
        // Tierra extranjera al norte y al este: el Brasil (al oeste, cruzando el Río Uruguay,
        // está la Argentina).
        let brasil = |c: usize, f: usize| {
            let b = g.capas[f * w + c];
            b[datos::C_DEPTO] == 0 && b[datos::C_AGUA] < 128 && b[datos::C_EMBALSE] == 0 && (f < 45 || c > 200)
        };
        let mut km = vec![f32::INFINITY; w * h];
        let mut cerca = vec![u32::MAX; w * h];
        let mut q = std::collections::VecDeque::new();
        for f in 0..h {
            for c in 0..w {
                if brasil(c, f) {
                    km[f * w + c] = 0.0;
                    cerca[f * w + c] = (f * w + c) as u32;
                    q.push_back(f * w + c);
                }
            }
        }
        while let Some(k) = q.pop_front() {
            let (x, y) = ((k % w) as i32, (k / w) as i32);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let n = ny as usize * w + nx as usize;
                if km[n] > km[k] + 1.0 {
                    km[n] = km[k] + 1.0;
                    cerca[n] = cerca[k];
                    q.push_back(n);
                }
            }
        }
        Frontera { km, cerca }
    })
}

pub fn km_a_la_frontera(p: [f32; 2]) -> f32 {
    let g = geo();
    let (c, f) = ((p[0].round().max(0.0) as usize).min(g.ancho() - 1), (p[1].round().max(0.0) as usize).min(g.alto() - 1));
    frontera().km[f * g.ancho() + c]
}

pub fn punto_frontera(p: [f32; 2]) -> [f32; 2] {
    let g = geo();
    let (c, f) = ((p[0].round().max(0.0) as usize).min(g.ancho() - 1), (p[1].round().max(0.0) as usize).min(g.alto() - 1));
    let k = frontera().cerca[f * g.ancho() + c];
    if k == u32::MAX {
        return p;
    }
    [(k as usize % g.ancho()) as f32, (k as usize / g.ancho()) as f32]
}

/// Ruta de un productor a la capital.
#[derive(Clone, Debug)]
pub struct Ruta {
    /// Días de viaje de una carreta.
    pub dias: f32,
    /// Polilínea (celdas) del productor a Montevideo (o al puerto del río).
    pub puntos: Vec<[f32; 2]>,
    /// La carga sale por un puerto del río.
    pub por_rio: bool,
    /// La carga sale por el ferrocarril.
    pub por_tren: bool,
}

/// Red del mes: los pueblos, los caminos que se pueden andar y los días hasta la capital.
pub struct Red {
    pos: Vec<[f32; 2]>,
    dias: Vec<f32>,
    /// (pueblo anterior, camino, invertido) en el camino más corto; `None` en una fuente.
    prev: Vec<Option<(usize, usize, bool)>>,
    /// Fuente: 0 capital, 1 puerto del río, 2 estación del ferrocarril.
    fuente: Vec<u8>,
    caminos: Vec<Vec<[f32; 2]>>,
    capital: [f32; 2],
}

/// Velocidad de los barcos del río y del tren (km por día). ESTIMADO.
const KM_DIA_RIO: f32 = 100.0;
const KM_DIA_TREN: f32 = 300.0;
/// Carretas a campo traviesa (sin camino).
const KM_DIA_CAMPO: f32 = 8.0;

impl Red {
    pub fn armar(p: &Pais, anio: i32) -> Red {
        let g = geo();
        let n = p.pueblos.len();
        let pos: Vec<[f32; 2]> = p.pueblos.iter().map(|x| x.pos).collect();
        let cap = p.pueblos.iter().position(|x| x.tipo == "capital");
        let capital = cap.map_or(g.centro_ciudad(), |k| pos[k]);
        let mut dias = vec![f32::INFINITY; n];
        let mut fuente = vec![0u8; n];
        if let Some(k) = cap {
            dias[k] = 0.0;
        }
        // Puertos del río: embarcan a Montevideo (Colonia es portuguesa hasta 1777).
        for pt in &p.puertos {
            if pt.desde > anio || (pt.pueblo == "colonia" && anio < 1777) {
                continue;
            }
            if let Some(k) = p.pueblos.iter().position(|x| x.id == pt.pueblo) {
                let d = 1.0 + g.km(pos[k], capital) * 1.3 / KM_DIA_RIO;
                if d < dias[k] {
                    dias[k] = d;
                    fuente[k] = 1;
                }
            }
        }
        // Ferrocarril (desde la época 3): los pueblos junto a una vía ya abierta.
        if super::nivel_epoca(super::epoca_de(anio)) >= 2 {
            for k in 0..n {
                let junto = g.vias.iter().filter(|v| v.anio <= anio).any(|v| v.puntos.windows(2).any(|w| datos::dist_seg(pos[k], w[0], w[1]) < 4.0));
                if junto {
                    let d = 0.5 + g.km(pos[k], capital) * 1.25 / KM_DIA_TREN;
                    if d < dias[k] {
                        dias[k] = d;
                        fuente[k] = 2;
                    }
                }
            }
        }
        // Aristas: caminos con sus pasos (un vado no da paso con el río crecido).
        let idx = |id: &str| p.pueblos.iter().position(|x| x.id == id);
        let mut ady: Vec<Vec<(usize, f32, usize, bool)>> = vec![Vec::new(); n];
        let mut caminos = Vec::new();
        for c in &p.caminos {
            let (Some(a), Some(b)) = (idx(&c.desde), idx(&c.hasta)) else { continue };
            let mut d = c.km() / c.velocidad();
            let mut cortado = false;
            for ps in &c.pasos {
                let crecido = p.crecido(&ps.rio);
                d += match (ps.obra, crecido) {
                    (Obra::Vado, true) => {
                        cortado = true;
                        0.0
                    }
                    (Obra::Vado, false) => 0.3,
                    (Obra::Balsa, true) => 2.0,
                    (Obra::Balsa, false) => 0.5,
                    (Obra::Puente, _) => 0.0,
                };
            }
            let ci = caminos.len();
            caminos.push(c.puntos.clone());
            if cortado {
                continue;
            }
            ady[a].push((b, d, ci, false));
            ady[b].push((a, d, ci, true));
        }
        // Dijkstra desde todas las fuentes.
        let mut prev = vec![None; n];
        let mut pq = BinaryHeap::new();
        for k in 0..n {
            if dias[k].is_finite() {
                pq.push(Nodo(dias[k], k));
            }
        }
        while let Some(Nodo(d, k)) = pq.pop() {
            if d > dias[k] {
                continue;
            }
            for &(q, w, ci, inv) in &ady[k] {
                if d + w < dias[q] {
                    dias[q] = d + w;
                    // Desde q se vuelve a k por el camino ci (al revés de como se llegó).
                    prev[q] = Some((k, ci, !inv));
                    fuente[q] = fuente[k];
                    pq.push(Nodo(dias[q], q));
                }
            }
        }
        Red { pos, dias, prev, fuente, caminos, capital }
    }

    /// Días desde un pueblo (índice) a la capital.
    pub fn dias_pueblo(&self, k: usize) -> f32 {
        self.dias[k]
    }

    /// La mejor ruta de un punto a la capital: a campo traviesa hasta un pueblo de la red y
    /// de ahí por los caminos (o directo a campo traviesa si no hay pueblos cerca).
    pub fn ruta(&self, p: [f32; 2]) -> Option<Ruta> {
        let g = geo();
        let directo = g.km(p, self.capital) * 1.3 / KM_DIA_CAMPO;
        let mut mejor: Option<(f32, usize)> = None;
        for k in 0..self.pos.len() {
            if !self.dias[k].is_finite() {
                continue;
            }
            let km = g.km(p, self.pos[k]);
            if km > 90.0 {
                continue;
            }
            let d = km * 1.3 / KM_DIA_CAMPO + self.dias[k];
            if mejor.map_or(true, |m| d < m.0) {
                mejor = Some((d, k));
            }
        }
        match mejor {
            Some((d, k)) if d < directo => {
                let mut puntos = vec![p, self.pos[k]];
                let mut cur = k;
                let mut guard = 0;
                while let Some((q, ci, inv)) = self.prev[cur] {
                    let mut seg = self.caminos[ci].clone();
                    if inv {
                        seg.reverse();
                    }
                    puntos.extend(seg.into_iter().skip(1));
                    cur = q;
                    guard += 1;
                    if guard > 200 {
                        break;
                    }
                }
                // Del puerto o la estación sigue por el río o por la vía: la polilínea termina
                // ahí (el mapa dibuja las vías aparte).
                let (por_rio, por_tren) = (self.fuente[cur] == 1, self.fuente[cur] == 2);
                Some(Ruta { dias: d, puntos, por_rio, por_tren })
            }
            _ => Some(Ruta { dias: directo, puntos: vec![p, self.capital], por_rio: false, por_tren: false }),
        }
    }
}
