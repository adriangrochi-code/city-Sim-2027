//! Red de calles: grafo de nodos (cruces y extremos) y tramos (polilíneas). Porte de
//! `scripts/roads/road_network.gd` del prototipo de Godot, con la misma semántica:
//! las calles se dibujan como Bézier cuadráticas muestreadas cada 4 m, se enganchan a nodos
//! y tramos cercanos, y al cruzar otra calle se crea el cruce automáticamente.

use glam::{IVec2, Vec2};
use std::collections::{BTreeMap, HashMap};

pub const SNAP: f32 = 9.0;
pub const STEP: f32 = 4.0;
const CELL: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoadKind {
    Dirt,
    Stone,
}

impl RoadKind {
    pub fn width(self) -> f32 {
        match self {
            RoadKind::Dirt => 6.0,
            RoadKind::Stone => 8.0,
        }
    }
    pub fn min_radius(self) -> f32 {
        match self {
            RoadKind::Dirt => 14.0,
            RoadKind::Stone => 22.0,
        }
    }
    pub fn cost_per_m(self) -> f32 {
        match self {
            RoadKind::Dirt => 2.0,
            RoadKind::Stone => 7.0,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            RoadKind::Dirt => "camino de tierra",
            RoadKind::Stone => "calle empedrada",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub pts: Vec<Vec2>,
    pub kind: RoadKind,
    /// Largo acumulado en cada punto.
    pub cum: Vec<f32>,
}

impl Edge {
    fn new(a: u32, b: u32, pts: Vec<Vec2>, kind: RoadKind) -> Edge {
        let mut cum = vec![0.0];
        for i in 0..pts.len() - 1 {
            cum.push(cum[i] + pts[i].distance(pts[i + 1]));
        }
        Edge { a, b, pts, kind, cum }
    }
    pub fn len(&self) -> f32 {
        *self.cum.last().unwrap()
    }
    /// Punto y tangente a `s` metros desde el nodo `a`.
    pub fn point(&self, s: f32) -> (Vec2, Vec2) {
        let s = s.clamp(0.0, self.len());
        let i = match self.cum.binary_search_by(|c| c.partial_cmp(&s).unwrap()) {
            Ok(i) => i.min(self.pts.len() - 2),
            Err(i) => i.saturating_sub(1).min(self.pts.len() - 2),
        };
        let seg = (self.cum[i + 1] - self.cum[i]).max(1e-4);
        let tan = (self.pts[i + 1] - self.pts[i]) / seg;
        (self.pts[i].lerp(self.pts[i + 1], ((s - self.cum[i]) / seg).clamp(0.0, 1.0)), tan)
    }
}

/// Dónde engancharse cerca de un punto.
#[derive(Clone, Copy, Debug)]
pub struct Snap {
    pub pos: Vec2,
    pub node: Option<u32>,
    pub edge: Option<(u32, usize)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Nearest {
    pub edge: u32,
    pub seg: usize,
    pub point: Vec2,
    pub dist: f32,
}

// --- Geometría -------------------------------------------------------------------

pub fn bezier(p0: Vec2, c: Vec2, p1: Vec2) -> Vec<Vec2> {
    let approx = p0.distance(c) + c.distance(p1);
    let n = ((approx / STEP).ceil() as usize).max(1);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            p0 * (u * u) + c * (2.0 * u * t) + p1 * (t * t)
        })
        .collect()
}

pub fn line(p0: Vec2, p1: Vec2) -> Vec<Vec2> {
    bezier(p0, (p0 + p1) * 0.5, p1)
}

pub fn polyline_length(pts: &[Vec2]) -> f32 {
    pts.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// Radio de curvatura mínimo (círculo por cada terna de puntos).
pub fn min_radius(pts: &[Vec2]) -> f32 {
    let mut best = f32::INFINITY;
    for w in pts.windows(3) {
        let (a, b, c) = (w[0], w[1], w[2]);
        let area2 = (b - a).perp_dot(c - a).abs();
        if area2 < 1e-3 {
            continue;
        }
        best = best.min(a.distance(b) * b.distance(c) * c.distance(a) / (2.0 * area2));
    }
    best
}

pub fn point_at(pts: &[Vec2], t: f32) -> Vec2 {
    let i = (t.floor() as isize).clamp(0, pts.len() as isize - 2) as usize;
    pts[i].lerp(pts[i + 1], (t - i as f32).clamp(0.0, 1.0))
}

pub fn sub_polyline(pts: &[Vec2], t0: f32, t1: f32) -> Vec<Vec2> {
    let mut out = vec![point_at(pts, t0)];
    for i in (t0.floor() as isize + 1)..(t1.ceil() as isize) {
        if i >= 0 && (i as usize) < pts.len() && pts[i as usize].distance(*out.last().unwrap()) > 0.05 {
            out.push(pts[i as usize]);
        }
    }
    let last = point_at(pts, t1);
    if last.distance(*out.last().unwrap()) > 0.05 {
        out.push(last);
    }
    out
}

pub fn closest_on_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let l2 = ab.length_squared();
    if l2 < 1e-9 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
}

/// Intersección de dos segmentos (incluye los extremos).
pub fn segment_intersection(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
    let r = a2 - a1;
    let s = b2 - b1;
    let den = r.perp_dot(s);
    if den.abs() < 1e-9 {
        return None;
    }
    let t = (b1 - a1).perp_dot(s) / den;
    let u = (b1 - a1).perp_dot(r) / den;
    if (-1e-6..=1.0 + 1e-6).contains(&t) && (-1e-6..=1.0 + 1e-6).contains(&u) {
        Some(a1 + r * t)
    } else {
        None
    }
}

// --- Red ---------------------------------------------------------------------------

#[derive(Default)]
pub struct RoadNetwork {
    pub nodes: BTreeMap<u32, Vec2>,
    pub node_edges: BTreeMap<u32, Vec<u32>>,
    pub edges: BTreeMap<u32, Edge>,
    next_node: u32,
    next_edge: u32,
    hash: HashMap<IVec2, Vec<(u32, usize)>>,
}

fn cell(p: Vec2) -> IVec2 {
    IVec2::new((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

fn cells_of(a: Vec2, b: Vec2) -> impl Iterator<Item = IVec2> {
    let mn = cell(a.min(b));
    let mx = cell(a.max(b));
    (mn.y..=mx.y).flat_map(move |y| (mn.x..=mx.x).map(move |x| IVec2::new(x, y)))
}

impl RoadNetwork {
    #[cfg(test)]
    pub fn degree(&self, nid: u32) -> usize {
        self.node_edges.get(&nid).map_or(0, |v| v.len())
    }

    pub fn snap(&self, p: Vec2) -> Snap {
        let mut best: Option<(u32, f32)> = None;
        for (&id, &q) in &self.nodes {
            let d = q.distance(p);
            if d < SNAP && best.is_none_or(|b| d < b.1) {
                best = Some((id, d));
            }
        }
        if let Some((id, _)) = best {
            return Snap { pos: self.nodes[&id], node: Some(id), edge: None };
        }
        if let Some(e) = self.nearest_edge(p, SNAP) {
            return Snap { pos: e.point, node: None, edge: Some((e.edge, e.seg)) };
        }
        Snap { pos: p, node: None, edge: None }
    }

    fn segments_near(&self, p: Vec2, maxd: f32) -> impl Iterator<Item = (u32, usize)> + '_ {
        let c = cell(p);
        let r = (maxd / CELL).ceil() as i32;
        (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| c + IVec2::new(dx, dy))).filter_map(|k| self.hash.get(&k)).flatten().copied()
    }

    pub fn nearest_edge(&self, p: Vec2, maxd: f32) -> Option<Nearest> {
        let mut best: Option<Nearest> = None;
        for (eid, si) in self.segments_near(p, maxd) {
            let pts = &self.edges[&eid].pts;
            let q = closest_on_segment(p, pts[si], pts[si + 1]);
            let d = p.distance(q);
            if d < maxd && best.is_none_or(|b| d < b.dist || (d == b.dist && eid < b.edge)) {
                best = Some(Nearest { edge: eid, seg: si, point: q, dist: d });
            }
        }
        best
    }

    /// Distancia del punto al borde de la calzada más cercana (negativa si está encima).
    pub fn clearance(&self, p: Vec2, maxd: f32) -> f32 {
        let mut best = f32::INFINITY;
        for (eid, si) in self.segments_near(p, maxd) {
            let e = &self.edges[&eid];
            let q = closest_on_segment(p, e.pts[si], e.pts[si + 1]);
            best = best.min(p.distance(q) - e.kind.width() * 0.5);
        }
        best
    }

    pub fn total_length(&self) -> f32 {
        self.edges.values().map(|e| e.len()).sum()
    }

    /// Agrega una calle. Devuelve los ids de los tramos nuevos (incluidos los trozos de
    /// calles existentes que se partieron en los cruces).
    pub fn add_road(&mut self, pts_in: &[Vec2], kind: RoadKind) -> Vec<u32> {
        if pts_in.len() < 2 {
            return Vec::new();
        }
        let before: Vec<u32> = self.edges.keys().copied().collect();
        let mut path = pts_in.to_vec();
        let start = self.resolve_endpoint(path[0]);
        let end = self.resolve_endpoint(*path.last().unwrap());
        if start == end {
            return self.new_since(&before);
        }
        let n = path.len();
        path[0] = self.nodes[&start];
        path[n - 1] = self.nodes[&end];
        let mut cuts: Vec<(f32, u32)> = vec![(0.0, start), ((n - 1) as f32, end)];
        let mut hits = self.find_intersections(&path);
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (t, pt) in hits {
            if pt.distance(self.nodes[&start]) < SNAP || pt.distance(self.nodes[&end]) < SNAP {
                continue;
            }
            let nid = self.resolve_endpoint(pt);
            if !cuts.iter().any(|c| c.1 == nid) {
                cuts.push((t, nid));
            }
        }
        cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for w in cuts.windows(2) {
            let ((t0, n0), (t1, n1)) = (w[0], w[1]);
            if n0 == n1 || self.edge_between(n0, n1).is_some() {
                continue;
            }
            let mut sub = sub_polyline(&path, t0, t1);
            if sub.len() < 2 {
                continue;
            }
            sub[0] = self.nodes[&n0];
            let l = sub.len();
            sub[l - 1] = self.nodes[&n1];
            if polyline_length(&sub) < 2.0 {
                continue;
            }
            self.add_edge(n0, n1, sub, kind);
        }
        self.rebuild_hash();
        self.new_since(&before)
    }

    fn new_since(&self, before: &[u32]) -> Vec<u32> {
        self.edges.keys().copied().filter(|k| before.binary_search(k).is_err()).collect()
    }

    pub fn remove_edge(&mut self, eid: u32) -> Option<Edge> {
        let e = self.remove_edge_internal(eid)?;
        for nid in [e.a, e.b] {
            if self.node_edges.get(&nid).is_some_and(|v| v.is_empty()) {
                self.nodes.remove(&nid);
                self.node_edges.remove(&nid);
            }
        }
        self.rebuild_hash();
        Some(e)
    }

    fn resolve_endpoint(&mut self, p: Vec2) -> u32 {
        let s = self.snap(p);
        if let Some(n) = s.node {
            return n;
        }
        if let Some((eid, si)) = s.edge {
            return self.split_edge(eid, si, s.pos);
        }
        self.add_node(p)
    }

    fn split_edge(&mut self, eid: u32, si: usize, pt: Vec2) -> u32 {
        let e = self.edges[&eid].clone();
        if pt.distance(self.nodes[&e.a]) < 1.0 {
            return e.a;
        }
        if pt.distance(self.nodes[&e.b]) < 1.0 {
            return e.b;
        }
        let mut first: Vec<Vec2> = e.pts[..=si].to_vec();
        if first.last().unwrap().distance(pt) > 0.05 {
            first.push(pt);
        }
        let mut second = vec![pt];
        for &q in &e.pts[si + 1..] {
            if q.distance(*second.last().unwrap()) > 0.05 {
                second.push(q);
            }
        }
        self.remove_edge_internal(eid);
        let nid = self.add_node(pt);
        if first.len() >= 2 {
            self.add_edge(e.a, nid, first, e.kind);
        }
        if second.len() >= 2 {
            self.add_edge(nid, e.b, second, e.kind);
        }
        self.rebuild_hash();
        nid
    }

    fn find_intersections(&self, path: &[Vec2]) -> Vec<(f32, Vec2)> {
        let mut hits: Vec<(f32, Vec2)> = Vec::new();
        for i in 0..path.len() - 1 {
            let (a1, a2) = (path[i], path[i + 1]);
            let seg_len = a1.distance(a2);
            if seg_len < 1e-3 {
                continue;
            }
            let mut checked: Vec<(u32, usize)> = Vec::new();
            for k in cells_of(a1, a2) {
                let Some(list) = self.hash.get(&k) else { continue };
                for &(eid, si) in list {
                    if checked.contains(&(eid, si)) {
                        continue;
                    }
                    checked.push((eid, si));
                    let pts = &self.edges[&eid].pts;
                    if let Some(pt) = segment_intersection(a1, a2, pts[si], pts[si + 1]) {
                        if !hits.iter().any(|h| h.1.distance(pt) < 1.0) {
                            hits.push((i as f32 + a1.distance(pt) / seg_len, pt));
                        }
                    }
                }
            }
        }
        hits
    }

    pub fn edge_between(&self, a: u32, b: u32) -> Option<u32> {
        self.node_edges.get(&a)?.iter().copied().find(|eid| {
            let e = &self.edges[eid];
            (e.a == a && e.b == b) || (e.a == b && e.b == a)
        })
    }

    fn add_node(&mut self, p: Vec2) -> u32 {
        self.next_node += 1;
        let id = self.next_node;
        self.nodes.insert(id, p);
        self.node_edges.insert(id, Vec::new());
        id
    }

    fn add_edge(&mut self, a: u32, b: u32, pts: Vec<Vec2>, kind: RoadKind) -> u32 {
        self.next_edge += 1;
        let id = self.next_edge;
        self.edges.insert(id, Edge::new(a, b, pts, kind));
        self.node_edges.get_mut(&a).unwrap().push(id);
        self.node_edges.get_mut(&b).unwrap().push(id);
        id
    }

    fn remove_edge_internal(&mut self, eid: u32) -> Option<Edge> {
        let e = self.edges.remove(&eid)?;
        for n in [e.a, e.b] {
            if let Some(v) = self.node_edges.get_mut(&n) {
                v.retain(|&x| x != eid);
            }
        }
        Some(e)
    }

    fn rebuild_hash(&mut self) {
        self.hash.clear();
        for (&eid, e) in &self.edges {
            for si in 0..e.pts.len() - 1 {
                for k in cells_of(e.pts[si], e.pts[si + 1]) {
                    self.hash.entry(k).or_default().push((eid, si));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_calles_que_se_cruzan_crean_un_cruce() {
        let mut net = RoadNetwork::default();
        net.add_road(&line(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)), RoadKind::Stone);
        assert_eq!(net.edges.len(), 1);
        let new = net.add_road(&line(Vec2::new(50.0, -50.0), Vec2::new(50.0, 50.0)), RoadKind::Dirt);
        // La calle vieja se parte en dos y la nueva también: 4 tramos y un nodo de grado 4.
        assert_eq!(net.edges.len(), 4);
        assert_eq!(new.len(), 4);
        let center = net.nodes.iter().find(|(_, p)| p.distance(Vec2::new(50.0, 0.0)) < 0.5).map(|(&id, _)| id).unwrap();
        assert_eq!(net.degree(center), 4);
        // El largo total se conserva.
        assert!((net.total_length() - 200.0).abs() < 0.5);
    }

    #[test]
    fn los_extremos_se_enganchan_a_nodos_y_tramos() {
        let mut net = RoadNetwork::default();
        net.add_road(&line(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)), RoadKind::Stone);
        // Termina a 5 m del extremo: se engancha al nodo existente.
        net.add_road(&line(Vec2::new(103.0, 50.0), Vec2::new(103.0, 4.0)), RoadKind::Stone);
        assert_eq!(net.nodes.len(), 3);
        // Termina a 6 m del medio de un tramo: lo parte y se conecta (T).
        net.add_road(&line(Vec2::new(40.0, -60.0), Vec2::new(40.0, -6.0)), RoadKind::Dirt);
        let t = net.nodes.values().any(|p| p.distance(Vec2::new(40.0, 0.0)) < 0.5);
        assert!(t, "debe haber un nodo en la T");
        assert_eq!(net.edges.len(), 4);
    }

    #[test]
    fn radio_minimo_de_curvas() {
        // Curva amplia: radio grande; curva cerrada: radio chico.
        let amplia = bezier(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0), Vec2::new(100.0, 100.0));
        let cerrada = bezier(Vec2::new(0.0, 0.0), Vec2::new(20.0, 0.0), Vec2::new(20.0, 20.0));
        assert!(min_radius(&amplia) > RoadKind::Stone.min_radius());
        assert!(min_radius(&cerrada) < RoadKind::Stone.min_radius());
        assert!(min_radius(&line(Vec2::ZERO, Vec2::new(50.0, 3.0))).is_infinite());
    }

    #[test]
    fn la_distancia_al_borde_de_la_calzada() {
        let mut net = RoadNetwork::default();
        net.add_road(&line(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)), RoadKind::Stone);
        assert!((net.clearance(Vec2::new(50.0, 10.0), 30.0) - 6.0).abs() < 1e-3);
        assert!(net.clearance(Vec2::new(50.0, 1.0), 30.0) < 0.0);
        let (p, tan) = net.edges.values().next().unwrap().point(25.0);
        assert!((p - Vec2::new(25.0, 0.0)).length() < 1e-3 && (tan - Vec2::X).length() < 1e-3);
    }
}
