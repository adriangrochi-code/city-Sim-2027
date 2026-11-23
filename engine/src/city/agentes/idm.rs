//! Modelo de conductor inteligente (IDM, *Intelligent Driver Model*, Treiber, Hennecke y
//! Helbing, 2000): la aceleración de un vehículo según su velocidad, la deseada y la
//! distancia y velocidad del que va adelante. Distancia de seguridad que crece con la
//! velocidad, aceleración y frenada suaves, frenada fuerte solo en emergencias.

use serde::{Deserialize, Serialize};

/// Cómo se mueve cada tipo de agente. Las épocas siguientes agregan los suyos (diligencia,
/// tranvía, automóvil, ómnibus) con sus parámetros: el resto del sistema no cambia.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Tipo {
    /// Gente a pie (por las veredas: no sigue a nadie).
    APie,
    /// Jinete a caballo.
    Jinete,
    /// Carreta de bueyes (cueros, trigo, leña): la más lenta y larga.
    Carreta,
    /// Carro de dos ruedas tirado por un caballo (reparto de mercancías).
    Carro,
    /// Arreo de ganado (una tropa con sus troperos a caballo): ocupa toda la calzada, nadie lo
    /// adelanta; su largo depende de las cabezas (ver `Agente::largo`).
    Arreo,
}

/// Parámetros del IDM: velocidad deseada `v0` (m/s), aceleración `a` y frenada cómoda `b`
/// (m/s²), tiempo de reacción o separación `t` (s), distancia mínima `s0` (m) y largo (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub v0: f32,
    pub a: f32,
    pub b: f32,
    pub t: f32,
    pub s0: f32,
    pub largo: f32,
}

impl Tipo {
    pub const TODOS: [Tipo; 5] = [Tipo::APie, Tipo::Jinete, Tipo::Carreta, Tipo::Carro, Tipo::Arreo];

    /// Valores de la época colonial (tabla de `docs/sistemas/agentes-y-conduccion.md`).
    pub fn params(self) -> Params {
        match self {
            Tipo::APie => Params { v0: 1.35, a: 1.0, b: 1.5, t: 0.8, s0: 0.8, largo: 0.6 },
            Tipo::Jinete => Params { v0: 3.8, a: 1.3, b: 2.2, t: 1.3, s0: 2.0, largo: 2.6 },
            Tipo::Carreta => Params { v0: 1.1, a: 0.25, b: 0.6, t: 2.4, s0: 3.0, largo: 7.5 },
            Tipo::Carro => Params { v0: 2.0, a: 0.6, b: 1.2, t: 1.8, s0: 2.5, largo: 5.0 },
            // 3 km/h (datos de tropas); el largo es el de la punta: el resto lo pone cada tropa.
            Tipo::Arreo => Params { v0: 0.83, a: 0.3, b: 0.8, t: 2.0, s0: 3.0, largo: 6.0 },
        }
    }
    pub fn vehiculo(self) -> bool {
        self != Tipo::APie
    }
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::APie => "a pie",
            Tipo::Jinete => "a caballo",
            Tipo::Carreta => "en carreta de bueyes",
            Tipo::Carro => "en carro",
            Tipo::Arreo => "arreando la tropa",
        }
    }
    /// "3 carretas", "1 jinete", "5 a pie".
    pub fn cuantos(self, n: u32) -> String {
        let (uno, varios) = match self {
            Tipo::APie => ("a pie", "a pie"),
            Tipo::Jinete => ("jinete", "jinetes"),
            Tipo::Carreta => ("carreta", "carretas"),
            Tipo::Carro => ("carro", "carros"),
            Tipo::Arreo => ("arreo", "arreos"),
        };
        format!("{n} {}", if n == 1 { uno } else { varios })
    }
}

/// Aceleración del IDM. `hueco`: distancia libre hasta el de adelante (o hasta el punto de
/// detención), `INFINITY` si no hay nadie; `v_lider`: su velocidad.
pub fn aceleracion(p: &Params, v0: f32, t: f32, v: f32, hueco: f32, v_lider: f32) -> f32 {
    let libre = 1.0 - (v / v0.max(0.1)).powi(4);
    let acc = if hueco.is_finite() {
        let deseado = p.s0 + (v * t + v * (v - v_lider) / (2.0 * (p.a * p.b).sqrt())).max(0.0);
        p.a * (libre - (deseado / hueco.max(0.05)).powi(2))
    } else {
        p.a * libre
    };
    acc.clamp(-4.0 * p.b, p.a)
}

/// Un paso de `dt` segundos: devuelve (avance, velocidad nueva). Nunca avanza más que el
/// hueco (menos 0,2 m): dos vehículos del mismo carril no se superponen nunca.
pub fn paso(p: &Params, v0: f32, t: f32, v: f32, hueco: f32, v_lider: f32, dt: f32) -> (f32, f32) {
    let acc = aceleracion(p, v0, t, v, hueco, v_lider);
    let v1 = (v + acc * dt).max(0.0);
    let mut ds = v1 * dt;
    if hueco.is_finite() {
        ds = ds.min((hueco - 0.2).max(0.0));
    }
    (ds, ds / dt)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fila de vehículos en un carril recto: posición del frente y velocidad; el primero es
    /// el de más adelante. `parado`: el primero no se mueve.
    fn simular(tipo: Tipo, n: usize, segundos: f32, parado: bool) -> Vec<(f32, f32)> {
        let p = tipo.params();
        let dt = 0.25;
        let mut fila: Vec<(f32, f32)> = (0..n).map(|i| (200.0 - i as f32 * (p.largo + p.s0 + 1.0), 0.0)).collect();
        let pasos = (segundos / dt) as usize;
        for _ in 0..pasos {
            let antes = fila.clone();
            for i in 0..n {
                if i == 0 && parado {
                    continue;
                }
                let (hueco, vl) = if i == 0 { (f32::INFINITY, 0.0) } else { (antes[i - 1].0 - p.largo - antes[i].0, antes[i - 1].1) };
                let (ds, v) = paso(&p, p.v0, p.t, antes[i].1, hueco, vl, dt);
                fila[i] = (antes[i].0 + ds, v);
            }
            for i in 1..n {
                assert!(fila[i - 1].0 - p.largo - fila[i].0 > 0.0, "{tipo:?}: el vehículo {i} se montó sobre el de adelante");
            }
        }
        fila
    }

    #[test]
    fn idm_llega_a_la_velocidad_deseada() {
        for tipo in [Tipo::Jinete, Tipo::Carreta, Tipo::Carro] {
            let p = tipo.params();
            let f = simular(tipo, 1, 120.0, false);
            assert!((f[0].1 - p.v0).abs() < 0.05 * p.v0, "{tipo:?}: {} m/s (deseada {})", f[0].1, p.v0);
            // Nunca la supera.
            assert!(f[0].1 <= p.v0 + 1e-3);
        }
    }

    #[test]
    fn idm_frena_detras_de_un_vehiculo_parado_sin_chocar() {
        for tipo in [Tipo::Jinete, Tipo::Carreta, Tipo::Carro] {
            let p = tipo.params();
            // Una fila de 6 que arranca detrás de uno parado: todos terminan quietos, con al
            // menos la distancia mínima (menos un margen) entre cada uno.
            let f = simular(tipo, 6, 400.0, true);
            for i in 1..f.len() {
                let hueco = f[i - 1].0 - p.largo - f[i].0;
                assert!(f[i].1 < 0.05, "{tipo:?}: el {i} sigue andando a {} m/s", f[i].1);
                assert!(hueco > p.s0 * 0.7 && hueco < p.s0 * 1.6, "{tipo:?}: hueco {hueco} m (mínimo {})", p.s0);
            }
        }
    }

    #[test]
    fn idm_sin_choques_en_una_fila_que_arranca_y_frena() {
        // Un jinete que va libre alcanza a una carreta lenta y se queda detrás.
        let pj = Tipo::Jinete.params();
        let pc = Tipo::Carreta.params();
        let (mut sc, mut vc) = (60.0f32, pc.v0);
        let (mut sj, mut vj) = (0.0f32, pj.v0);
        let mut min_hueco = f32::INFINITY;
        for _ in 0..2400 {
            let (dc, nvc) = paso(&pc, pc.v0, pc.t, vc, f32::INFINITY, 0.0, 0.25);
            let hueco = sc - pc.largo - sj;
            let (dj, nvj) = paso(&pj, pj.v0, pj.t, vj, hueco, vc, 0.25);
            sc += dc;
            vc = nvc;
            sj += dj;
            vj = nvj;
            min_hueco = min_hueco.min(sc - pc.largo - sj);
        }
        assert!(min_hueco > 1.0, "hueco mínimo {min_hueco}");
        assert!((vj - vc).abs() < 0.05, "el jinete sigue a la carreta: {vj} vs {vc}");
    }
}
