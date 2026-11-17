//! Sombras en cascadas (CSM): varias capas de un mapa de sombras, cada una cubre un tramo de la
//! vista. La primera llega un poco más allá del punto que mira la cámara (ahí está la acción
//! del constructor de ciudades, con vóxeles de 0,25 m), las siguientes van creciendo hasta la
//! distancia de sombra (cerros y montes lejanos).
//!
//! Estabilidad (que las sombras no "titilen" al mover la cámara):
//! - La rotación de la luz depende solo del sol; el centro de cada cascada se ajusta a la
//!   grilla de texels y el lado se elige de una escalera fija (cada escalón ×2^¼).
//! - **Histéresis**: si el lugar anterior de la cascada todavía cubre lo que hace falta (con el
//!   mismo escalón o uno más), no se mueve. Así, con la cámara quieta o moviéndose poco, la
//!   matriz no cambia y el mapa de esa cascada no se vuelve a dibujar (caché por cascada).
//!
//! Todo en esta parte es matemática pura, con pruebas.

use glam::{Mat4, Vec3, Vec4};

pub const MAX_CASCADES: usize = 4;
/// Lado mínimo de la escalera (medio lado, en metros) y escalones por octava.
const STEP_BASE: f32 = 1.0;
const STEPS_PER_OCTAVE: f32 = 4.0;
/// Texels de margen en cada borde: el filtro PCF 3×3 lee uno más allá y la mezcla entre
/// cascadas necesita un poco de lugar.
const EDGE_TEXELS: f32 = 3.0;

/// Calidad de las sombras (opciones gráficas): cantidad de cascadas y resolución de cada una.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowQuality {
    pub cascades: u32,
    pub resolution: u32,
}

impl ShadowQuality {
    /// 4 cascadas de 2048²: 64 MB, lo mismo que el mapa único de 4096² de antes.
    pub const DEFAULT: ShadowQuality = ShadowQuality { cascades: 4, resolution: 2048 };

    pub fn sanitized(self) -> ShadowQuality {
        ShadowQuality { cascades: self.cascades.clamp(2, MAX_CASCADES as u32), resolution: if matches!(self.resolution, 1024 | 2048 | 4096) { self.resolution } else { 2048 } }
    }

    /// Memoria de los mapas (profundidad de 32 bits).
    pub fn bytes(&self) -> u64 {
        self.cascades as u64 * self.resolution as u64 * self.resolution as u64 * 4
    }
}

/// Rotación de la luz: mira desde el sol (sin traslación, así no depende de la cámara).
pub fn light_rotation(sun: Vec3) -> Mat4 {
    let up = if sun.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
    Mat4::look_at_rh(Vec3::ZERO, -sun, up)
}

/// Lugar de una cascada en el espacio de la luz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Escalón de la escalera de tamaños: medio lado = STEP_BASE · 2^(step/4).
    pub step: i32,
    pub cx: f32,
    pub cy: f32,
    /// Rango de profundidad del ortográfico (metros, como en `Mat4::orthographic_rh`).
    pub z_near: f32,
    pub z_far: f32,
}

pub fn step_half(step: i32) -> f32 {
    STEP_BASE * 2f32.powf(step as f32 / STEPS_PER_OCTAVE)
}

impl Placement {
    pub fn half(&self) -> f32 {
        step_half(self.step)
    }

    /// Lado de un texel en metros.
    pub fn texel(&self, res: u32) -> f32 {
        2.0 * self.half() / res as f32
    }

    pub fn matrix(&self, rot: Mat4) -> Mat4 {
        let h = self.half();
        Mat4::orthographic_rh(self.cx - h, self.cx + h, self.cy - h, self.cy + h, self.z_near, self.z_far) * rot
    }
}

/// Lo que una cascada tiene que cubrir, en el espacio de la luz.
#[derive(Clone, Copy, Debug)]
pub struct Need {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    /// Rango de profundidad necesario (como `z_near`/`z_far`).
    pub z_near: f32,
    pub z_far: f32,
}

impl Need {
    /// Caja de unos puntos ya pasados al espacio de la luz; `reach` alarga la caja hacia el sol
    /// (los que dan sombra pueden estar más arriba que lo que la recibe).
    pub fn from_points(pts: &[Vec3], reach: f32) -> Option<Need> {
        if pts.is_empty() {
            return None;
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for p in pts {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        // En la vista de la luz se mira hacia −z: lo más cercano al sol tiene la z más grande.
        Some(Need { x0: lo.x, x1: hi.x, y0: lo.y, y1: hi.y, z_near: -hi.z - reach, z_far: -lo.z + 1.0 })
    }
}

/// Elige el lugar de la cascada: reutiliza el anterior si todavía sirve (histéresis); si no,
/// uno nuevo del tamaño justo de la escalera, con el centro en la grilla de texels.
pub fn fit(prev: Option<Placement>, need: &Need, res: u32) -> Placement {
    let usable = 1.0 - 2.0 * EDGE_TEXELS / res as f32;
    let half_req = ((need.x1 - need.x0).max(need.y1 - need.y0) * 0.5).max(0.5) / usable;
    let step = (STEPS_PER_OCTAVE * (half_req / STEP_BASE).log2()).ceil() as i32;
    if let Some(p) = prev {
        let h = p.half() * usable;
        let inside = p.cx - h <= need.x0 && p.cx + h >= need.x1 && p.cy - h <= need.y0 && p.cy + h >= need.y1;
        if (step..=step + 1).contains(&p.step) && inside && p.z_near <= need.z_near && p.z_far >= need.z_far {
            return p;
        }
    }
    let h = step_half(step);
    let texel = 2.0 * h / res as f32;
    let snap = |v: f32| (v / texel).round() * texel;
    // Margen de profundidad: moverse un poco no obliga a rehacer el mapa.
    let zm = h * 0.25;
    Placement { step, cx: snap((need.x0 + need.x1) * 0.5), cy: snap((need.y0 + need.y1) * 0.5), z_near: need.z_near - zm, z_far: need.z_far + zm }
}

/// Distancias de corte (profundidad de vista): la primera cascada va de `n0` a `f0` (un poco
/// más allá del objetivo de la cámara) y las demás crecen en progresión geométrica hasta `far`.
pub fn splits(n0: f32, f0: f32, far: f32, count: usize) -> [f32; MAX_CASCADES + 1] {
    let count = count.clamp(1, MAX_CASCADES);
    let n0 = n0.max(0.05);
    let f0 = f0.max(n0 * 1.5);
    let far = far.max(f0 * 1.5);
    let mut s = [far; MAX_CASCADES + 1];
    s[0] = n0;
    if count == 1 {
        s[1] = far;
        return s;
    }
    s[1] = f0;
    for i in 2..count {
        s[i] = f0 * (far / f0).powf((i - 1) as f32 / (count - 1) as f32);
    }
    s[count] = far;
    s
}

/// Ejes de la cámara para armar los tramos de la vista.
#[derive(Clone, Copy, Debug)]
pub struct ViewFrame {
    pub eye: Vec3,
    pub fwd: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub tan_y: f32,
    pub aspect: f32,
}

impl ViewFrame {
    pub fn new(eye: Vec3, target: Vec3, fov_y_deg: f32, aspect: f32) -> ViewFrame {
        let fwd = (target - eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        ViewFrame { eye, fwd, right, up, tan_y: (fov_y_deg.to_radians() * 0.5).tan(), aspect }
    }

    /// Las 8 esquinas del tramo [n, f] de la vista (4 cerca, 4 lejos).
    pub fn corners(&self, n: f32, f: f32) -> [Vec3; 8] {
        let mut c = [Vec3::ZERO; 8];
        for (k, s) in [n, f].into_iter().enumerate() {
            for (i, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].into_iter().enumerate() {
                c[k * 4 + i] = self.eye + self.fwd * s + self.right * (sx * s * self.tan_y * self.aspect) + self.up * (sy * s * self.tan_y);
            }
        }
        c
    }
}

/// Vértices del tramo [n, f] de la vista recortado a la franja de alturas [ymin, ymax]: solo
/// ahí puede haber algo que reciba sombra (debajo del río o encima del techo más alto, no).
/// Los planos son paralelos, así que los vértices del poliedro recortado son las esquinas
/// adentro de la franja y los cruces de las aristas con cada plano.
pub fn slice_points(v: &ViewFrame, n: f32, f: f32, ymin: f32, ymax: f32) -> Vec<Vec3> {
    let c = v.corners(n, f);
    let inside = |p: Vec3| p.y >= ymin && p.y <= ymax;
    let mut out: Vec<Vec3> = c.iter().copied().filter(|p| inside(*p)).collect();
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in edges {
        let (a, b) = (c[a], c[b]);
        for (y, lo, hi) in [(ymin, f32::NEG_INFINITY, ymax), (ymax, ymin, f32::INFINITY)] {
            if (a.y - y) * (b.y - y) < 0.0 {
                let q = a.lerp(b, (y - a.y) / (b.y - a.y));
                if q.y >= lo - 1e-3 && q.y <= hi + 1e-3 {
                    out.push(Vec3::new(q.x, y, q.z));
                }
            }
        }
    }
    out
}

/// Datos de una cascada para el shader (ver `Sombras` en sombras.wgsl).
pub fn shader_info(p: &Placement, res: u32) -> Vec4 {
    Vec4::new(p.texel(res), 1.0 / (p.z_far - p.z_near), 1.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn need_for(rot: Mat4, pts: &[Vec3]) -> Need {
        let lp: Vec<Vec3> = pts.iter().map(|p| rot.transform_point3(*p)).collect();
        Need::from_points(&lp, 200.0).unwrap()
    }

    #[test]
    fn los_cortes_crecen_y_la_primera_cascada_pasa_el_objetivo() {
        for count in 2..=4 {
            let s = splits(30.0, 180.0, 1500.0, count);
            assert_eq!(s[0], 30.0);
            assert_eq!(s[1], 180.0);
            assert_eq!(s[count], 1500.0);
            for i in 0..count {
                assert!(s[i + 1] > s[i], "{count} cascadas: {s:?}");
            }
            if count == 4 {
                // Progresión geométrica entre f0 y far.
                let r1 = s[2] / s[1];
                let r2 = s[3] / s[2];
                assert!((r1 - r2).abs() < 1e-3);
            }
        }
        // Valores raros: nada se cruza.
        let s = splits(500.0, 100.0, 50.0, 4);
        for i in 0..4 {
            assert!(s[i + 1] > s[i]);
        }
    }

    #[test]
    fn el_tramo_recortado_queda_en_la_franja_y_dentro_del_tramo() {
        let v = ViewFrame::new(Vec3::new(0.0, 80.0, 120.0), Vec3::new(0.0, 0.0, 0.0), 50.0, 16.0 / 9.0);
        let pts = slice_points(&v, 40.0, 200.0, -5.0, 30.0);
        assert!(pts.len() >= 4);
        for p in &pts {
            assert!(p.y >= -5.001 && p.y <= 30.001, "{p:?}");
            let depth = (*p - v.eye).dot(v.fwd);
            assert!(depth >= 40.0 - 0.01 && depth <= 200.0 + 0.01, "profundidad {depth}");
        }
        // La caja recortada es más chica que la del tramo entero.
        let full = v.corners(40.0, 200.0);
        let ext = |ps: &[Vec3]| ps.iter().fold(f32::NEG_INFINITY, |m, p| m.max(p.y)) - ps.iter().fold(f32::INFINITY, |m, p| m.min(p.y));
        assert!(ext(&pts) < ext(&full));
        // Mirando al cielo, por encima de la franja: no queda nada.
        let up = ViewFrame::new(Vec3::new(0.0, 500.0, 0.0), Vec3::new(0.0, 900.0, -100.0), 50.0, 1.5);
        assert!(slice_points(&up, 10.0, 100.0, -5.0, 200.0).is_empty());
    }

    #[test]
    fn la_cascada_cubre_lo_pedido_con_margen() {
        let sun = Vec3::new(0.3, 0.7, -0.4).normalize();
        let rot = light_rotation(sun);
        let v = ViewFrame::new(Vec3::new(13000.0, 90.0, 16000.0), Vec3::new(13050.0, 10.0, 15880.0), 50.0, 16.0 / 9.0);
        let pts = slice_points(&v, 20.0, 180.0, -6.0, 60.0);
        let need = need_for(rot, &pts);
        let p = fit(None, &need, 2048);
        let m = p.matrix(rot);
        for q in &pts {
            let c = m * q.extend(1.0);
            assert!(c.x.abs() <= 1.0 - 2.0 * EDGE_TEXELS / 2048.0 + 1e-4 && c.y.abs() <= 1.0 - 2.0 * EDGE_TEXELS / 2048.0 + 1e-4, "{c:?}");
            assert!(c.z >= 0.0 && c.z <= 1.0, "profundidad {c:?}");
        }
        // Un poco más arriba (hacia el sol) del punto también entra: allí están los que dan sombra.
        let caster = pts[0] + sun * 150.0;
        let c = m * caster.extend(1.0);
        assert!(c.z >= 0.0 && c.z <= 1.0);
    }

    /// Ajuste a la grilla de texels: al mover la cámara (y la cascada se mueve), la grilla del
    /// mapa sigue alineada con el mundo: un punto fijo cae siempre en la misma fracción de texel.
    #[test]
    fn el_centro_se_ajusta_a_la_grilla_de_texels() {
        let sun = Vec3::new(-0.2, 0.8, -0.5).normalize();
        let rot = light_rotation(sun);
        let res = 2048;
        let world = Vec3::new(13020.3, 12.7, 15990.1);
        let mut frac = None;
        for k in 0..40 {
            let shift = k as f32 * 7.31;
            let need = Need { x0: -300.0 + shift, x1: 300.0 + shift, y0: 1000.0 - shift, y1: 1500.0 - shift, z_near: -5000.0, z_far: 5000.0 };
            let p = fit(None, &need, res);
            assert_eq!(p.step, fit(None, &need, res).step);
            let c = p.matrix(rot) * world.extend(1.0);
            // Coordenada en texels del punto fijo.
            let t = (c.x * 0.5 + 0.5) * res as f32;
            let f = t - t.floor();
            if let Some(f0) = frac {
                let d: f32 = f - f0;
                let d = d.abs().min(1.0 - d.abs());
                assert!(d < 0.02, "el punto se corrió {d} texel con la cascada en otro lugar");
            }
            frac = Some(f);
        }
    }

    #[test]
    fn con_movimientos_chicos_la_cascada_no_cambia() {
        let sun = Vec3::new(0.3, 0.7, -0.4).normalize();
        let rot = light_rotation(sun);
        let mut prev = None;
        let mut changes = 0;
        // La cámara se desplaza 30 cm por cuadro durante 100 cuadros (30 m en total) y gira un poco.
        for k in 0..100 {
            let t = Vec3::new(13000.0 + k as f32 * 0.3, 10.0, 16000.0);
            let yaw = (k as f32 * 0.05).to_radians();
            let eye = t + Vec3::new(yaw.sin() * 120.0, 80.0, yaw.cos() * 120.0);
            let v = ViewFrame::new(eye, t, 50.0, 16.0 / 9.0);
            let need = need_for(rot, &slice_points(&v, 20.0, 180.0, -6.0, 60.0));
            let p = fit(prev, &need, 2048);
            if prev != Some(p) {
                changes += 1;
            }
            prev = Some(p);
        }
        // Sin histéresis se movería en cada cuadro (el centro cambia 30 cm = varios texels).
        assert!(changes <= 15, "la cascada cambió {changes} veces en 100 cuadros");
        assert!(changes >= 1);
    }

    #[test]
    fn calidad_saneada() {
        assert_eq!(ShadowQuality { cascades: 9, resolution: 3000 }.sanitized(), ShadowQuality { cascades: 4, resolution: 2048 });
        assert_eq!(ShadowQuality { cascades: 1, resolution: 1024 }.sanitized(), ShadowQuality { cascades: 2, resolution: 1024 });
        assert_eq!(ShadowQuality::DEFAULT.bytes(), 64 << 20);
    }
}
