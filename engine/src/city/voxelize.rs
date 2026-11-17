//! Del plano de la ciudad a vóxeles: calzadas que aplanan suavemente el terreno, la plaza
//! mayor y edificios del catálogo girados a cualquier ángulo para mirar a su calle.

use super::roads::{closest_on_segment, Edge, RoadKind, RoadNetwork};
use crate::buildings::{top_at, Edits};
use crate::lod::BASE_VOXEL;
use crate::palette::*;
use crate::structures::Frame;
use crate::terrain::TerrainGen;
use glam::{IVec2, IVec3, Vec2};
use std::collections::HashMap;

/// Banquina: franja a cada lado de la calzada donde el terreno pasa suavemente de la altura
/// de la calle a la natural (sin cortes bruscos).
pub const SHOULDER: f32 = 3.0;
/// Lado de la plaza mayor (m).
/// Lado de la plaza: el hueco entre las calles que la rodean (cuadra − calle empedrada) más medio
/// metro por lado, que queda debajo de la calzada (sin franja de pasto entre ambas).
pub const PLAZA_SIZE: f32 = 86.6 - 10.4 + 1.0;

fn vox_center(x: i32, z: i32) -> Vec2 {
    Vec2::new((x as f32 + 0.5) * BASE_VOXEL, (z as f32 + 0.5) * BASE_VOXEL)
}

fn top_index(h: f32) -> i32 {
    (h / BASE_VOXEL - 0.5).floor() as i32
}

/// Altura de la calzada en un punto: el terreno promediado en un disco de 6 m. Depende solo
/// del lugar (no de la calle), así dos calles que se cruzan quedan a la misma altura.
pub fn road_height(t: &TerrainGen, p: Vec2) -> f32 {
    let mut sum = t.height(p.x, p.y);
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        let q = p + Vec2::new(a.cos(), a.sin()) * 6.0;
        sum += t.height(q.x, q.y);
    }
    (sum / 9.0).max(0.4)
}

/// Lleva una columna de su altura natural `g` a `top`, con `mat` arriba.
fn set_column(e: &mut Edits, x: i32, z: i32, g: i32, top: i32, mat: Mat) {
    e.ground.insert(IVec2::new(x, z), top);
    for y in (top + 1)..=g {
        e.set(IVec3::new(x, y, z), AIR);
    }
    for y in (g + 1)..top {
        e.set(IVec3::new(x, y, z), DIRT);
    }
    e.set(IVec3::new(x, top, z), mat);
}

/// Deshace `set_column` (vuelve a la altura natural, con pasto arriba).
fn restore_column(e: &mut Edits, x: i32, z: i32, g: i32, top: i32) {
    e.ground.insert(IVec2::new(x, z), g);
    for y in (g + 1)..=top.max(g) {
        e.set(IVec3::new(x, y, z), AIR);
    }
    for y in (top + 1)..g {
        e.set(IVec3::new(x, y, z), DIRT);
    }
    e.set(IVec3::new(x, g, z), GRASS);
}

/// Columnas cercanas a un tramo con su distancia al eje y el punto del eje más cercano.
/// También devuelve `s`: metros a lo largo del tramo hasta ese punto del eje.
fn edge_columns(e: &Edge, reach: f32) -> HashMap<IVec2, (f32, Vec2, f32)> {
    let mut out: HashMap<IVec2, (f32, Vec2, f32)> = HashMap::new();
    let vs = BASE_VOXEL;
    for (i, w) in e.pts.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let lo = a.min(b) - Vec2::splat(reach);
        let hi = a.max(b) + Vec2::splat(reach);
        for z in (lo.y / vs).floor() as i32..=(hi.y / vs).floor() as i32 {
            for x in (lo.x / vs).floor() as i32..=(hi.x / vs).floor() as i32 {
                let q = vox_center(x, z);
                let p = closest_on_segment(q, a, b);
                let d = q.distance(p);
                if d > reach {
                    continue;
                }
                let entry = out.entry(IVec2::new(x, z)).or_insert((f32::INFINITY, p, 0.0));
                if d < entry.0 {
                    *entry = (d, p, e.cum[i] + a.distance(p));
                }
            }
        }
    }
    out
}

/// Altura (índice de vóxel) de la superficie en una columna de un tramo: calzada o banquina.
fn road_top(t: &TerrainGen, kind: RoadKind, d: f32, p: Vec2, g: i32) -> Option<i32> {
    let hw = kind.width() * 0.5;
    let yr = top_index(road_height(t, p));
    if d <= hw {
        Some(yr)
    } else if d <= hw + SHOULDER {
        let k = (d - hw) / SHOULDER;
        let yb = (yr as f32 + (g - yr) as f32 * k).round() as i32;
        (yb != g).then_some(yb)
    } else {
        None
    }
}

/// Vóxeles de un tramo de calle: calzada (empedrado con veredas de piedra, o tierra) y
/// banquinas que empalman con el terreno.
/// Largo máximo de un puente en la época colonial (madera o piedra, con pilares).
pub const MAX_BRIDGE: f32 = 80.0;
/// Estribos: la obra del puente se extiende estos metros sobre cada orilla.
const ABUTMENT: f32 = 3.0;

/// Tramos de agua que cruza una calle (río o arroyo), como (desde, hasta) en metros a lo
/// largo de la polilínea, con los estribos incluidos. Ahí la calle es un puente.
pub fn water_spans(t: &TerrainGen, pts: &[Vec2]) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::new();
    let mut s = 0.0;
    let mut start: Option<f32> = None;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        let n = (len / 1.0).ceil().max(1.0) as i32;
        for k in 0..n {
            let u = k as f32 / n as f32;
            let p = a.lerp(b, u);
            let here = s + len * u;
            let wet = t.is_water(p.x, p.y);
            match (wet, start) {
                (true, None) => start = Some(here),
                (false, Some(s0)) => {
                    out.push((s0, here));
                    start = None;
                }
                _ => {}
            }
        }
        s += len;
    }
    if let Some(s0) = start {
        out.push((s0, s));
    }
    // Estribos y unión de cruces muy cercanos (un solo puente).
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (a, b) in out {
        let (a, b) = ((a - ABUTMENT).max(0.0), (b + ABUTMENT).min(s));
        match merged.last_mut() {
            Some(last) if a <= last.1 + 2.0 => last.1 = b,
            _ => merged.push((a, b)),
        }
    }
    merged
}

/// Altura del tablero de un puente (índice de vóxel): por encima de las dos orillas y al
/// menos 2,5 m sobre el agua.
fn deck_top(t: &TerrainGen, edge: &Edge, span: (f32, f32)) -> i32 {
    let (pa, _) = edge.point(span.0);
    let (pb, _) = edge.point(span.1);
    let banks = road_height(t, pa).max(road_height(t, pb));
    let mut water = 0.0f32;
    let n = ((span.1 - span.0) / 2.0).ceil().max(1.0) as i32;
    for k in 0..=n {
        let (p, _) = edge.point(span.0 + (span.1 - span.0) * k as f32 / n as f32);
        if t.is_water(p.x, p.y) {
            water = water.max(t.height(p.x, p.y).max(0.0));
        }
    }
    top_index(banks.max(water + 2.5) + 0.3)
}

/// Vóxeles de un tramo de calle: calzada (empedrado con veredas de piedra, o tierra) y
/// puentes donde cruza agua (el cauce no se toca: el agua sigue corriendo).
pub fn road_edits(t: &TerrainGen, net: &RoadNetwork, eid: u32, e: &mut Edits) {
    let edge = &net.edges[&eid];
    let hw = edge.kind.width() * 0.5;
    let spans = water_spans(t, &edge.pts);
    let decks: Vec<i32> = spans.iter().map(|&sp| deck_top(t, edge, sp)).collect();
    for (c, (d, p, s)) in edge_columns(edge, hw + SHOULDER) {
        if let Some(k) = spans.iter().position(|&(a, b)| s >= a && s <= b) {
            bridge_column(e, edge.kind, c, d, hw, s - spans[k].0, spans[k].1 - spans[k].0, decks[k], top_at(t, c.x, c.y), false);
            continue;
        }
        let q = vox_center(c.x, c.y);
        let g = top_at(t, c.x, c.y);
        let Some(top) = road_top(t, edge.kind, d, p, g) else { continue };
        // ¿Está dentro de la calzada de otra calle (por ejemplo, en un cruce)?
        let inside_any = net.clearance(q, 20.0);
        if d <= hw {
            let mat = match edge.kind {
                RoadKind::Dirt => DIRT_ROAD,
                // Vereda de piedra en el metro y medio de afuera, salvo dentro de otra calzada.
                RoadKind::Stone if d > hw - 1.5 && inside_any >= -1.5 => PLAZA,
                RoadKind::Stone => COBBLE,
            };
            set_column(e, c.x, c.y, g, top, mat);
        } else if inside_any > 0.0 {
            set_column(e, c.x, c.y, g, top, GRASS);
        }
    }
}

/// Una columna de puente: tablero, pretiles y, cada 8 m, un pilar hasta el fondo. Con
/// `remove`, borra lo mismo (al demoler).
#[allow(clippy::too_many_arguments)]
fn bridge_column(e: &mut Edits, kind: RoadKind, c: IVec2, d: f32, hw: f32, along: f32, span: f32, deck: i32, ground: i32, remove: bool) {
    if d > hw {
        return;
    }
    let (deck_m, rail_m, pier_m) = match kind {
        RoadKind::Stone => (STONE, STONE, STONE_DARK),
        RoadKind::Dirt => (WOOD, WOOD_DARK, WOOD_DARK),
    };
    let mut put = |y: i32, m: Mat| e.set(IVec3::new(c.x, y, c.y), if remove { AIR } else { m });
    put(deck, deck_m);
    put(deck - 1, deck_m);
    if d > hw - 0.6 {
        put(deck + 1, rail_m);
        if kind == RoadKind::Stone {
            put(deck + 2, rail_m);
        }
    }
    // Pilares cada 8 m (no en los estribos), del fondo al tablero.
    let pier = along > 4.0 && along < span - 4.0 && (along % 8.0) < 1.0 && d < hw - 0.5;
    if pier {
        for y in (ground + 1).min(deck - 2)..deck - 1 {
            put(y, pier_m);
        }
    }
}

/// Deshace los vóxeles de un tramo demolido (el terreno vuelve a su forma natural).
pub fn road_restore(t: &TerrainGen, edge: &Edge, e: &mut Edits) {
    let hw = edge.kind.width() * 0.5;
    let spans = water_spans(t, &edge.pts);
    let decks: Vec<i32> = spans.iter().map(|&sp| deck_top(t, edge, sp)).collect();
    for (c, (d, p, s)) in edge_columns(edge, hw + SHOULDER) {
        if let Some(k) = spans.iter().position(|&(a, b)| s >= a && s <= b) {
            bridge_column(e, edge.kind, c, d, hw, s - spans[k].0, spans[k].1 - spans[k].0, decks[k], top_at(t, c.x, c.y), true);
            continue;
        }
        let g = top_at(t, c.x, c.y);
        if let Some(top) = road_top(t, edge.kind, d, p, g) {
            restore_column(e, c.x, c.y, g, top);
        }
    }
}

/// Plaza mayor empedrada (cuadrada, girada `f`), con el rollo en el centro.
pub fn plaza_edits(t: &TerrainGen, c: Vec2, f: Vec2, e: &mut Edits) {
    let s = super::lots::side_of(f);
    let half = PLAZA_SIZE * 0.5;
    let reach = half + SHOULDER;
    let yp = top_index(road_height(t, c));
    let r = reach * std::f32::consts::SQRT_2;
    let vs = BASE_VOXEL;
    for z in ((c.y - r) / vs).floor() as i32..=((c.y + r) / vs).floor() as i32 {
        for x in ((c.x - r) / vs).floor() as i32..=((c.x + r) / vs).floor() as i32 {
            let q = vox_center(x, z) - c;
            let m = q.dot(s).abs().max(q.dot(f).abs());
            if m > reach {
                continue;
            }
            let g = top_at(t, x, z);
            if m <= half {
                set_column(e, x, z, g, yp, PLAZA);
            } else {
                let k = (m - half) / SHOULDER;
                let yb = (yp as f32 + (g - yp) as f32 * k).round() as i32;
                if yb != g {
                    set_column(e, x, z, g, yb, GRASS);
                }
            }
        }
    }
    // Rollo (columna de la justicia) en el centro.
    let cx = (c.x / vs).floor() as i32;
    let cz = (c.y / vs).floor() as i32;
    e.fill(IVec3::new(cx - 2, yp + 1, cz - 2), IVec3::new(cx + 2, yp + 2, cz + 2), STONE);
    e.fill(IVec3::new(cx - 1, yp + 2, cz - 1), IVec3::new(cx + 1, yp + 11, cz + 1), CAL);
}

/// Resultado de construir una estructura girada.
pub struct Built {
    pub edits: Edits,
    pub voxels: Vec<IVec3>,
}

/// Construye una estructura del catálogo en un solar con centro `center` (m) y frente hacia
/// `front` (vector unitario hacia la calle), con cualquier ángulo.
///
/// Se arma primero alineada a los ejes y después se gira re-muestreando su grilla de vóxeles
/// (vecino más cercano) alrededor del centro del solar. Los cimientos se rehacen columna por
/// columna con el terreno real. Si el ángulo está a menos de 1,5° de uno cardinal, se usa el
/// cardinal exacto (sin re-muestreo, sin dientes de sierra).
pub fn build_rotated(t: &TerrainGen, center: Vec2, front: Vec2, width: i32, depth: i32, seed: u64, builder: &dyn Fn(&TerrainGen, &mut Edits, &Frame)) -> Built {
    let mut f = front.normalize_or(Vec2::NEG_Y);
    let ang = f.y.atan2(f.x);
    let quarter = std::f32::consts::FRAC_PI_2;
    let snapped = (ang / quarter).round() * quarter;
    if (ang - snapped).abs() < 1.5f32.to_radians() {
        f = Vec2::new(snapped.cos().round(), snapped.sin().round());
    }
    // Ejes del mundo para los ejes locales: u (a lo largo del frente) → s; w (hacia el fondo) → −f.
    let s = Vec2::new(-f.y, f.x);
    let bw = -f;
    let vs = BASE_VOXEL;
    let cv = IVec2::new((center.x / vs).round() as i32, (center.y / vs).round() as i32);
    let cp = cv.as_vec2();
    let to_world = |lu: f32, lw: f32| cp + s * lu + bw * lw;
    // Base: sobre lo más alto del terreno bajo la huella girada.
    let mut base = i32::MIN;
    let mut w = -depth / 2;
    while w <= depth / 2 {
        let mut u = -width / 2;
        while u <= width / 2 {
            let p = to_world(u as f32, w as f32);
            base = base.max(top_at(t, p.x.floor() as i32, p.y.floor() as i32) + 1);
            u += 4;
        }
        w += 4;
    }
    // 1. Alineada a los ejes: u → +x, w → +z, centrada en `cv`.
    let origin = cv - IVec2::new(width / 2, depth / 2);
    let frame = Frame { origin, du: IVec2::X, dw: IVec2::Y, base, width, depth, seed };
    let mut tmp = Edits::default();
    builder(t, &mut tmp, &frame);
    let mut cols: HashMap<IVec2, Vec<(i32, Mat)>> = HashMap::new();
    let (mut lo, mut hi) = (IVec2::splat(i32::MAX), IVec2::splat(i32::MIN));
    for (&v, &m) in &tmp.voxels {
        if v.y < base {
            continue; // los cimientos se rehacen abajo con el terreno real
        }
        let k = IVec2::new(v.x, v.z);
        cols.entry(k).or_default().push((v.y, m));
        lo = lo.min(k);
        hi = hi.max(k);
    }
    for &k in tmp.surface.keys() {
        lo = lo.min(k);
        hi = hi.max(k);
    }
    let mut out = Built { edits: Edits::default(), voxels: Vec::new() };
    if lo.x > hi.x {
        return out;
    }
    // 2. Caja del resultado en el mundo y re-muestreo inverso.
    let corners = [lo, IVec2::new(hi.x + 1, lo.y), IVec2::new(lo.x, hi.y + 1), hi + IVec2::ONE];
    let (mut wlo, mut whi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
    for c in corners {
        let d = c.as_vec2() - cp;
        let p = to_world(d.x, d.y);
        wlo = wlo.min(p);
        whi = whi.max(p);
    }
    for z in wlo.y.floor() as i32 - 1..=whi.y.ceil() as i32 + 1 {
        for x in wlo.x.floor() as i32 - 1..=whi.x.ceil() as i32 + 1 {
            let q = Vec2::new(x as f32 + 0.5, z as f32 + 0.5) - cp;
            let src = IVec2::new((cp.x + q.dot(s)).floor() as i32, (cp.y + q.dot(bw)).floor() as i32);
            if let Some(m) = tmp.surface.get(&src) {
                out.edits.surface.insert(IVec2::new(x, z), *m);
            }
            let Some(list) = cols.get(&src) else { continue };
            for &(y, m) in list {
                let v = IVec3::new(x, y, z);
                out.edits.set(v, m);
                out.voxels.push(v);
                if y == base {
                    // Cimientos hasta el terreno (unos vóxeles más abajo cubren los cortes de
                    // las banquinas de la calle).
                    let g = top_at(t, x, z);
                    for yy in (g - 4).min(base - 1)..base {
                        let fv = IVec3::new(x, yy, z);
                        out.edits.set(fv, m);
                        out.voxels.push(fv);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structures::colonial::{casa_azotea, LOT_D, LOT_W};

    #[test]
    fn un_edificio_girado_queda_dentro_de_su_solar() {
        let t = TerrainGen::new(1726);
        let c = Vec2::new(13600.0, 16100.0);
        for deg in [0.0f32, 17.0, 33.0, 90.0, 141.0, 225.0] {
            let a = deg.to_radians();
            let f = Vec2::new(a.cos(), a.sin());
            let b = build_rotated(&t, c, f, LOT_W, LOT_D, 7, &casa_azotea);
            assert!(b.voxels.len() > 1000, "{deg}°: {} vóxeles", b.voxels.len());
            // Todo cae dentro del rectángulo del solar girado (12 × 38 m, con 1 m de margen
            // para rejas voladas y el redondeo del re-muestreo).
            let s = Vec2::new(-f.y, f.x);
            for v in &b.voxels {
                let q = Vec2::new((v.x as f32 + 0.5) * BASE_VOXEL, (v.z as f32 + 0.5) * BASE_VOXEL) - c;
                assert!(q.dot(s).abs() <= 6.0 + 1.0, "{deg}°: fuera a lo ancho {}", q.dot(s));
                assert!(q.dot(f).abs() <= super::super::lots::D * 0.5 + 1.0, "{deg}°: fuera a lo hondo {}", q.dot(f));
            }
            // La fachada (los vóxeles más cercanos a la calle) está del lado del frente.
            let front = b.voxels.iter().map(|v| (Vec2::new(v.x as f32 + 0.5, v.z as f32 + 0.5) * BASE_VOXEL - c).dot(f)).fold(f32::NEG_INFINITY, f32::max);
            assert!(front > super::super::lots::D * 0.5 - 2.0, "{deg}°: la fachada no mira a la calle ({front})");
        }
    }

    #[test]
    fn el_angulo_cardinal_da_el_mismo_edificio_que_el_constructor() {
        // Frente al sur (+z... aquí f = (0, -1) es el norte): sin re-muestreo, misma cantidad
        // de vóxeles sobre la base que el constructor directo.
        let t = TerrainGen::new(1726);
        let c = Vec2::new(13600.0, 16100.0);
        let b = build_rotated(&t, c, Vec2::new(0.0, -1.0), LOT_W, LOT_D, 7, &casa_azotea);
        let b2 = build_rotated(&t, c, Vec2::new(0.0, -1.0).rotate(Vec2::from_angle(0.01)), LOT_W, LOT_D, 7, &casa_azotea);
        let mut a: Vec<IVec3> = b.voxels.clone();
        let mut c2: Vec<IVec3> = b2.voxels.clone();
        a.sort_by_key(|v| (v.x, v.y, v.z));
        c2.sort_by_key(|v| (v.x, v.y, v.z));
        assert_eq!(a, c2, "casi cardinal debe ajustarse al cardinal exacto");
    }

    #[test]
    fn la_calle_aplana_y_pinta_la_calzada() {
        let t = TerrainGen::new(1726);
        let mut net = RoadNetwork::default();
        let p = Vec2::new(13500.0, 16100.0);
        let ids = net.add_road(&super::super::roads::line(p, p + Vec2::new(60.0, 0.0)), RoadKind::Stone);
        let mut e = Edits::default();
        road_edits(&t, &net, ids[0], &mut e);
        let vs = BASE_VOXEL;
        let mid = IVec2::new(((p.x + 30.0) / vs) as i32, (p.y / vs) as i32);
        let top = (0..400).rev().find(|&y| e.voxels.get(&IVec3::new(mid.x, y, mid.y)).is_some_and(|&m| m != AIR)).unwrap();
        assert_eq!(e.voxels[&IVec3::new(mid.x, top, mid.y)], COBBLE);
        // Vereda en el borde y calzada a la misma altura que en el eje (la calle es plana de lado a lado).
        let edge = IVec2::new(mid.x, mid.y + 7);
        assert_eq!(e.voxels.get(&IVec3::new(edge.x, top, edge.y)).copied(), Some(PLAZA));
    }
}
