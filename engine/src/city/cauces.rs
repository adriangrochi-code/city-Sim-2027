//! Calles y solares que esquivan los arroyos (hoja de ruta: «las calles y solares deben esquivar
//! los arroyos»).
//!
//! - **Solares**: ninguno en el cauce ni en su orilla (una faja que crece con el ancho del
//!   arroyo), ni en un bañado ni en una laguna oficial (IDE): ahí la tierra no aguanta una casa.
//!   Lo usa `Lots::valid`.
//! - **Damero**: las calles del damero se cortan en las orillas de los arroyos en vez de cruzar
//!   con un puente cada una (la traza colonial paraba en los arroyos); un cruce suelto con
//!   puente se sigue haciendo con la calle recta o curva.
//! - **Aviso al trazar**: la calle que cruza un arroyo dice cuál y cuánto mide el puente, y la
//!   que pisa un bañado lo advierte.

use crate::terrain::{TerrainGen, BIO_BANADO, BIO_LAGUNA};
use glam::Vec2;

/// Orilla libre a cada lado del agua (m): 2,5 m más el medio ancho del cauce (hasta 10 m más),
/// así un arroyo de 10 m deja 7,5 m de barranca sin casas y una cañada de 2 m, 3,5 m.
pub fn orilla(medio_ancho: f32) -> f32 {
    2.5 + medio_ancho.min(10.0)
}

/// ¿Un solar puede pisar este punto? No en el cauce ni en su orilla, ni en un bañado o una
/// laguna.
pub fn punto_vedado(t: &TerrainGen, p: Vec2) -> bool {
    let (d, hw) = t.arroyo_query(p.x, p.y);
    if d < hw + orilla(hw) {
        return true;
    }
    matches!(t.official_biome(p.x, p.y), BIO_BANADO | BIO_LAGUNA)
}

/// Tramos de una calle del damero que quedan en tierra: la calle se corta a `margen` metros
/// de cada orilla del agua. Los pedazos de menos de 20 m se descartan.
pub fn cortar_en_arroyos(t: &TerrainGen, linea: &[Vec2]) -> Vec<Vec<Vec2>> {
    let spans = super::voxelize::water_spans(t, linea);
    if spans.is_empty() {
        return vec![linea.to_vec()];
    }
    let margen = 4.0;
    let total = super::roads::polyline_length(linea);
    // Intervalos en tierra (en metros a lo largo de la línea).
    let mut tierra = Vec::new();
    let mut desde = 0.0;
    for &(a, b) in &spans {
        tierra.push((desde, (a - margen).max(desde)));
        desde = b + margen;
    }
    tierra.push((desde, total));
    tierra.into_iter().filter(|(a, b)| b - a >= 20.0).map(|(a, b)| sub_linea(linea, a, b)).collect()
}

/// Pedazo de una polilínea entre dos distancias.
fn sub_linea(pts: &[Vec2], a: f32, b: f32) -> Vec<Vec2> {
    let mut out = Vec::new();
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let l = w[0].distance(w[1]);
        let (s0, s1) = (acc, acc + l);
        if s1 >= a && s0 <= b {
            let p0 = w[0].lerp(w[1], ((a - s0) / l.max(1e-6)).clamp(0.0, 1.0));
            let p1 = w[0].lerp(w[1], ((b - s0) / l.max(1e-6)).clamp(0.0, 1.0));
            if out.last().map_or(true, |q: &Vec2| q.distance(p0) > 0.01) {
                out.push(p0);
            }
            out.push(p1);
        }
        acc = s1;
    }
    out.dedup_by(|x, y| x.distance(*y) < 0.01);
    out
}

/// Nombre del arroyo más cercano a un punto (a menos de 60 m), si tiene.
pub fn nombre_arroyo(t: &TerrainGen, p: Vec2) -> Option<String> {
    let mut mejor: Option<(f32, usize)> = None;
    for (k, a) in t.arroyos.iter().enumerate() {
        for w in a.windows(2) {
            let (u, v) = (w[0], w[1]);
            let d = {
                let ab = v - u;
                let tt = ((p - u).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                (u + ab * tt).distance(p)
            };
            if d < 60.0 && mejor.map_or(true, |m| d < m.0) {
                mejor = Some((d, k));
            }
        }
    }
    mejor.and_then(|(_, k)| t.arroyo_nombres.get(k).cloned().flatten())
}

/// Aviso para el trazado de una calle: los arroyos que cruza (con el largo de cada puente) y
/// si pisa un bañado. `None` si no cruza nada.
pub fn aviso_trazado(t: &TerrainGen, pts: &[Vec2]) -> Option<String> {
    let spans = super::voxelize::water_spans(t, pts);
    let mut partes = Vec::new();
    for &(a, b) in spans.iter().take(3) {
        let m = sub_linea(pts, (a + b) * 0.5, (a + b) * 0.5 + 0.1);
        let nombre = m.first().and_then(|&p| nombre_arroyo(t, p)).map(|n| format!("el {n}")).unwrap_or_else(|| "un arroyo".into());
        partes.push(format!("cruza {nombre}: puente de {:.0} m", b - a));
    }
    if spans.len() > 3 {
        partes.push(format!("y {} cruces más", spans.len() - 3));
    }
    // Bañado: muestras cada 5 m.
    let total = super::roads::polyline_length(pts);
    let mut d = 0.0;
    let mut banado = 0.0;
    while d < total {
        if let Some(&p) = sub_linea(pts, d, d + 0.1).first() {
            if t.official_biome(p.x, p.y) == BIO_BANADO {
                banado += 5.0;
            }
        }
        d += 5.0;
    }
    if banado >= 15.0 {
        partes.push(format!("pasa {banado:.0} m por un bañado (sin solares)"));
    }
    (!partes.is_empty()).then(|| partes.join(" · "))
}

#[cfg(test)]
mod tests {
    use super::super::lots::{corners_of, Lots, D, W};
    use super::super::roads::{line, RoadKind, RoadNetwork};
    use super::*;

    /// Un tramo de arroyo del mapa real lejos del borde y del río, con su dirección.
    fn arroyo(t: &TerrainGen, ancho_min: f32) -> (Vec2, Vec2) {
        for a in &t.arroyos {
            for w in a.windows(2).step_by(5) {
                let m = (w[0] + w[1]) * 0.5;
                let (_, hw) = t.arroyo_query(m.x, m.y);
                if hw >= ancho_min && m.x > 3000.0 && m.y > 3000.0 && m.x < 21000.0 && m.y < 12000.0 && t.height(m.x, m.y + 300.0) > 3.0 && w[0].distance(w[1]) > 20.0 {
                    return (m, (w[1] - w[0]).normalize());
                }
            }
        }
        panic!("no hay arroyos");
    }

    #[test]
    fn los_solares_no_pisan_cauces_orillas_ni_banados() {
        let t = TerrainGen::montevideo();
        let (m, dir) = arroyo(&t, 1.0);
        let n = Vec2::new(-dir.y, dir.x);
        // Una calle que corre paralela al arroyo, a 18 m del eje: sus solares del lado del agua
        // no pueden caer en la orilla.
        let a = m - dir * 120.0 + n * 18.0;
        let b = m + dir * 120.0 + n * 18.0;
        let mut net = RoadNetwork::default();
        let new = net.add_road(&line(a, b), RoadKind::Dirt);
        let mut lots = Lots::default();
        lots.on_roads_changed(&net, &t, &new);
        for l in lots.lots.values() {
            let cs = corners_of(l.c, l.f, W, D);
            for p in cs.iter().chain(std::iter::once(&l.c)) {
                let (d, hw) = t.arroyo_query(p.x, p.y);
                assert!(d >= hw + 1.5, "solar {} a {:.1} m del eje de un arroyo de {:.1} m", l.id, d, hw * 2.0);
            }
            assert!(!punto_vedado(&t, l.c), "solar {} en terreno vedado", l.id);
        }
        // Los bañados oficiales: ningún punto de bañado sirve para un solar.
        let mut probados = 0;
        'b: for x in (0..24000).step_by(97) {
            for z in (0..24000).step_by(97) {
                let p = Vec2::new(x as f32, z as f32);
                if t.official_biome(p.x, p.y) == BIO_BANADO && t.height(p.x, p.y) > 1.0 {
                    assert!(punto_vedado(&t, p));
                    probados += 1;
                    if probados > 20 {
                        break 'b;
                    }
                }
            }
        }
        assert!(probados > 0, "no se encontraron bañados en el mapa");
    }

    #[test]
    fn el_damero_se_corta_en_los_arroyos_y_el_trazado_avisa() {
        let t = TerrainGen::montevideo();
        let (m, dir) = arroyo(&t, 1.0);
        let n = Vec2::new(-dir.y, dir.x);
        let calle = line(m - n * 150.0, m + n * 150.0);
        assert!(!super::super::voxelize::water_spans(&t, &calle).is_empty());
        let partes = cortar_en_arroyos(&t, &calle);
        assert!(partes.len() >= 2, "la calle se corta en dos: {}", partes.len());
        for p in &partes {
            assert!(super::super::voxelize::water_spans(&t, p).is_empty(), "un pedazo todavía cruza agua");
            assert!(super::super::roads::polyline_length(p) >= 20.0);
        }
        // El aviso del trazado nombra el cruce y el puente.
        let aviso = aviso_trazado(&t, &calle).expect("aviso");
        assert!(aviso.contains("puente de"), "{aviso}");
        // Una calle en seco no se toca ni avisa.
        let seca = line(m + n * 400.0, m + n * 400.0 + dir * 50.0);
        if super::super::voxelize::water_spans(&t, &seca).is_empty() {
            assert_eq!(cortar_en_arroyos(&t, &seca).len(), 1);
        }
    }

    #[test]
    fn un_damero_sobre_un_arroyo_no_hace_puentes() {
        let t = TerrainGen::montevideo();
        let mut city = super::super::City::default();
        city.money = 10_000_000;
        city.found_plaza(&t, t.founding_site(), 0.0).unwrap();
        let (m, dir) = arroyo(&t, 1.0);
        let n = Vec2::new(-dir.y, dir.x);
        let lineas = vec![line(m - n * 120.0, m + n * 120.0), line(m - n * 120.0 + dir * 86.6, m + n * 120.0 + dir * 86.6)];
        if city.build_grid(&t, &lineas, RoadKind::Dirt).is_ok() {
            for e in city.net.edges.values() {
                assert!(super::super::voxelize::water_spans(&t, &e.pts).is_empty(), "una calle del damero cruza el arroyo");
            }
            for l in city.lots.lots.values() {
                assert!(!punto_vedado(&t, l.c));
            }
        }
    }
}
