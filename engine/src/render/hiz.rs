//! Pirámide de profundidad jerárquica (Hi-Z) para descartar lo tapado.
//!
//! Profundidad con **Z invertida**: 1 = cerca, 0 = infinito (cielo). Cada texel de la
//! pirámide guarda lo **más lejano** (el mínimo) de lo que cubre; una caja está tapada si su
//! punto más cercano (el máximo de z) queda detrás de eso.
//!
//! - Nivel 0: la mitad de la resolución de la pantalla, mínimo de 2×2 píxeles y de todas las
//!   muestras del MSAA (`hiz.wgsl`, `reduce`).
//! - Niveles siguientes: los de wgpu (⌊anterior/2⌋, mínimo 1). El texel t lee los texels 2t
//!   y 2t+1 del nivel anterior; si ese nivel es impar, el **último** texel lee también el que
//!   sobra. Así el píxel p cae en el texel min(p >> (L+1), ancho_L − 1) del nivel L.
//! - Prueba (`cull.wgsl`, `occluded`): rectángulo de la caja en píxeles (+1 de margen), el
//!   nivel más chico donde el rectángulo toca a lo sumo 2×2 texels, y 4 lecturas.
//!
//! Esta es la referencia en la CPU de la misma lógica, para las pruebas.

/// Tamaños de los niveles de la pirámide para una pantalla de `w`×`h`.
pub fn level_sizes(w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut out = vec![(w.div_ceil(2).max(1), h.div_ceil(2).max(1))];
    while {
        let (a, b) = *out.last().unwrap();
        a > 1 || b > 1
    } {
        let (a, b) = *out.last().unwrap();
        out.push(((a / 2).max(1), (b / 2).max(1)));
    }
    out
}

/// Texels del nivel anterior que lee el texel `t` de un nivel de `dim` (el anterior mide `src`).
#[cfg(test)]
fn footprint(t: u32, dim: u32, src: u32) -> std::ops::Range<u32> {
    let end = if t + 1 == dim { src } else { (2 * t + 2).min(src) };
    (2 * t)..end
}

/// Pirámide en la CPU a partir de la profundidad de cada muestra (`samples` por píxel).
#[cfg(test)]
pub fn build(depth: &[f32], w: u32, h: u32, samples: u32) -> Vec<Vec<f32>> {
    let sizes = level_sizes(w, h);
    let mut levels: Vec<Vec<f32>> = Vec::new();
    for (l, &(lw, lh)) in sizes.iter().enumerate() {
        let mut out = vec![0.0f32; (lw * lh) as usize];
        for y in 0..lh {
            for x in 0..lw {
                let mut m = 1.0f32;
                if l == 0 {
                    for yy in footprint(y, lh, h) {
                        for xx in footprint(x, lw, w) {
                            for s in 0..samples {
                                m = m.min(depth[((yy * w + xx) * samples + s) as usize]);
                            }
                        }
                    }
                } else {
                    let (pw, ph) = sizes[l - 1];
                    let prev = &levels[l - 1];
                    for yy in footprint(y, lh, ph) {
                        for xx in footprint(x, lw, pw) {
                            m = m.min(prev[(yy * pw + xx) as usize]);
                        }
                    }
                }
                out[(y * lw + x) as usize] = m;
            }
        }
        levels.push(out);
    }
    levels
}

/// ¿Está tapado lo que ocupa los píxeles [x0, x1] × [y0, y1] (inclusive) y cuya profundidad
/// más cercana es `zmax`? Misma lógica que `occluded` en cull.wgsl.
#[cfg(test)]
pub fn occluded(levels: &[Vec<f32>], sizes: &[(u32, u32)], x0: u32, y0: u32, x1: u32, y1: u32, zmax: f32) -> bool {
    let n = levels.len() as u32;
    let tex = |p: u32, l: u32, dim: u32| (p >> (l + 1)).min(dim - 1);
    let mut l = 0u32;
    while l + 1 < n && {
        let (lw, lh) = sizes[l as usize];
        tex(x1, l, lw) - tex(x0, l, lw) > 1 || tex(y1, l, lh) - tex(y0, l, lh) > 1
    } {
        l += 1;
    }
    let (lw, lh) = sizes[l as usize];
    let lv = &levels[l as usize];
    let at = |x: u32, y: u32| lv[(tex(y, l, lh) * lw + tex(x, l, lw)) as usize];
    let d = at(x0, y0).min(at(x1, y0)).min(at(x0, y1)).min(at(x1, y1));
    zmax < d
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generador simple y determinista.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) as u32
        }
        fn f(&mut self) -> f32 {
            self.next() as f32 / (1u64 << 31) as f32
        }
    }

    #[test]
    fn los_niveles_terminan_en_un_texel() {
        assert_eq!(level_sizes(1280, 720)[0], (640, 360));
        assert_eq!(*level_sizes(1280, 720).last().unwrap(), (1, 1));
        assert_eq!(level_sizes(1281, 721)[0], (641, 361));
        assert_eq!(level_sizes(1281, 721)[1], (320, 180));
        // Los mismos tamaños que wgpu da a los niveles de una textura (⌊/2⌋), y no más niveles
        // de los que admite.
        let s = level_sizes(1280, 720);
        assert_eq!(s.len() as u32, 640f32.log2().floor() as u32 + 1);
        assert_eq!(level_sizes(1, 1), vec![(1, 1)]);
    }

    /// Lo central: la prueba es **conservadora**. Si dice "tapado", todas las muestras del
    /// rectángulo tienen algo más cerca que la caja. Pantallas con tamaños impares, MSAA y
    /// cajas al azar (chicas, grandes, en los bordes).
    #[test]
    fn la_prueba_nunca_descarta_algo_visible() {
        let mut r = Rng(7);
        let mut culled = 0;
        let mut total = 0;
        for &(w, h, s) in &[(37u32, 23u32, 4u32), (64, 64, 1), (101, 57, 4), (8, 5, 1)] {
            // Profundidad: "edificios" (bloques) a distintas distancias sobre un fondo lejano.
            let mut depth = vec![0.0f32; (w * h * s) as usize];
            for _ in 0..12 {
                let (bx, by) = (r.next() % w, r.next() % h);
                let (bw, bh) = (1 + r.next() % (w / 2 + 1), 1 + r.next() % (h / 2 + 1));
                let z = 0.2 + r.f() * 0.7;
                for y in by..(by + bh).min(h) {
                    for x in bx..(bx + bw).min(w) {
                        for k in 0..s {
                            // El MSAA: alguna muestra del borde puede no estar cubierta.
                            let d = &mut depth[((y * w + x) * s + k) as usize];
                            if !(x == bx && k == 0) {
                                *d = d.max(z);
                            }
                        }
                    }
                }
            }
            let levels = build(&depth, w, h, s);
            let sizes = level_sizes(w, h);
            for _ in 0..4000 {
                let (x0, y0) = (r.next() % w, r.next() % h);
                let x1 = (x0 + r.next() % (w - x0).max(1)).min(w - 1);
                let y1 = (y0 + r.next() % (h - y0).max(1)).min(h - 1);
                let zmax = r.f();
                total += 1;
                if occluded(&levels, &sizes, x0, y0, x1, y1, zmax) {
                    culled += 1;
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            for k in 0..s {
                                let d = depth[((y * w + x) * s + k) as usize];
                                assert!(d > zmax, "{w}×{h}×{s}: descartó una caja visible en ({x}, {y}): profundidad {d} y la caja {zmax}");
                            }
                        }
                    }
                }
            }
        }
        // Y que descarte algo (si no, la prueba no prueba nada).
        assert!(culled > total / 50, "descartó {culled} de {total}");
    }

    #[test]
    fn una_pared_tapa_lo_que_esta_detras() {
        let (w, h) = (128u32, 72u32);
        // Pared cercana (z = 0.8) en toda la mitad izquierda, cielo en la derecha.
        let depth: Vec<f32> = (0..w * h).map(|i| if i % w < 64 { 0.8 } else { 0.0 }).collect();
        let levels = build(&depth, w, h, 1);
        let sizes = level_sizes(w, h);
        assert!(occluded(&levels, &sizes, 10, 10, 50, 60, 0.5));
        assert!(!occluded(&levels, &sizes, 10, 10, 50, 60, 0.9), "algo más cerca que la pared se ve");
        assert!(!occluded(&levels, &sizes, 40, 10, 70, 20, 0.5), "lo que asoma por el cielo se ve");
    }
}
