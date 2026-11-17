//! Nivel del Río de la Plata frente a la ciudad: marea astronómica con el ciclo lunar, viento
//! (sudestadas que suben el agua y pampero que la baja) y caudal de los ríos Uruguay y Paraná
//! (crecientes en los años lluviosos, bajantes en las sequías). Ver
//! `docs/sistemas/agua-mareas-inundaciones.md`.
//!
//! Órdenes de magnitud (Montevideo): la marea astronómica es chica (decenas de centímetros, más
//! en las mareas vivas de luna nueva y llena); lo que mueve de verdad el agua es el viento: una
//! sudestada fuerte la sube más de un metro y medio durante uno o dos días, y un pampero la
//! baja y deja al descubierto los bancos de arena. El caudal de los ríos agrega o quita
//! algunos decímetros durante meses.
//!
//! Dos relojes, como en el resto del juego: la hora del día (el ciclo de día y noche) mueve la
//! marea de cada día; el calendario (los meses) mueve la luna, las estaciones y el caudal.

use crate::buildings::Rng;

/// Período de la marea principal (luna, semidiurna): 12 h 25 min.
const M2_HOURS: f32 = 12.42;
/// Período de la marea diurna (desigualdad entre las dos pleamares del día).
const K1_HOURS: f32 = 23.93;

/// Viento sobre el río.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wind {
    Calma,
    /// Sudestada: viento del sudeste que empuja el agua contra la costa (sube el río).
    Sudestada { peak: f32 },
    /// Pampero: viento del sudoeste, frío y seco, que vacía la costa (baja el río).
    Pampero { drop: f32 },
}

#[derive(Clone, Debug)]
pub struct WaterLevel {
    /// Horas que dura el temporal que está pasando, y las que lleva.
    event_hours: f32,
    event_elapsed: f32,
    pub wind: Wind,
    /// Anomalía del caudal de los ríos (m sobre el nivel medio), lenta.
    pub river: f32,
    /// Régimen de varios años: > 0 años lluviosos (crecientes), < 0 secos.
    regime: f32,
    regime_months: u32,
    rng: Rng,
    /// Nivel más alto del último temporal (para la gaceta y la ficha).
    pub last_peak: f32,
}

impl Default for WaterLevel {
    fn default() -> Self {
        WaterLevel { event_hours: 0.0, event_elapsed: 0.0, wind: Wind::Calma, river: 0.0, regime: 0.0, regime_months: 0, rng: Rng::new(1514), last_peak: 0.0 }
    }
}

/// Fase de la luna (0 = luna nueva, 0,5 = llena) a partir del calendario: un mes del juego es
/// una lunación.
pub fn moon_phase(month: u32, month_frac: f32) -> f32 {
    ((month as f32 + month_frac) * 1.0).fract()
}

/// Nombre de la fase para la interfaz.
pub fn moon_name(phase: f32) -> &'static str {
    match ((phase * 8.0 + 0.5).floor() as i32).rem_euclid(8) {
        0 => "luna nueva",
        1 => "luna creciente",
        2 => "cuarto creciente",
        3 => "gibosa creciente",
        4 => "luna llena",
        5 => "gibosa menguante",
        6 => "cuarto menguante",
        _ => "luna menguante",
    }
}

/// Iluminación de la luna (0 en luna nueva, 1 en luna llena).
pub fn moon_light(phase: f32) -> f32 {
    0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()
}

/// Marea astronómica (m) a una hora del día y fase de la luna. Vivas (luna nueva y llena):
/// ±0,3 m; muertas (cuartos): ±0,15 m.
pub fn astronomical_tide(hour: f32, phase: f32) -> f32 {
    let spring = 0.5 + 0.5 * (phase * 2.0 * std::f32::consts::TAU).cos(); // 1 en nueva y llena
    let amp = 0.15 + 0.15 * spring;
    let t = hour + phase * 24.0 * 29.53; // la hora de la pleamar se corre cada día
    amp * (t / M2_HOURS * std::f32::consts::TAU).cos() + 0.06 * (t / K1_HOURS * std::f32::consts::TAU).cos()
}

impl WaterLevel {
    /// Nivel del río (m sobre el nivel medio) en este momento.
    pub fn level(&self, hour: f32, phase: f32) -> f32 {
        astronomical_tide(hour, phase) + self.surge() + self.river
    }

    /// Lo que aporta el viento: sube y baja con forma de campana a lo largo del temporal.
    pub fn surge(&self) -> f32 {
        if self.event_hours <= 0.0 {
            return 0.0;
        }
        let x = (self.event_elapsed / self.event_hours).clamp(0.0, 1.0);
        let bell = (x * std::f32::consts::PI).sin().powf(1.5);
        match self.wind {
            Wind::Calma => 0.0,
            Wind::Sudestada { peak } => peak * bell,
            Wind::Pampero { drop } => -drop * bell,
        }
    }

    /// Avanza `hours` horas del reloj del día. `month` es el mes del calendario (0 = enero)
    /// para la estación de los temporales. Devuelve un aviso si empieza o termina un temporal.
    pub fn advance(&mut self, hours: f32, month: u32) -> Option<String> {
        if self.event_hours > 0.0 {
            self.event_elapsed += hours;
            self.last_peak = self.last_peak.max(self.surge());
            if self.event_elapsed >= self.event_hours {
                let msg = match self.wind {
                    Wind::Sudestada { .. } => Some("Amainó la sudestada: el río vuelve a su cauce".to_string()),
                    Wind::Pampero { .. } => Some("Pasó el pampero: el agua vuelve a la costa".to_string()),
                    Wind::Calma => None,
                };
                self.event_hours = 0.0;
                self.wind = Wind::Calma;
                return msg;
            }
            return None;
        }
        // Probabilidad por hora de que empiece un temporal: las sudestadas son más frecuentes
        // de otoño a primavera (abril a noviembre).
        let m = month % 12;
        let season = if (3..=10).contains(&m) { 1.0 } else { 0.5 };
        let p_sud = 0.0009 * season * hours;
        let p_pam = 0.0012 * hours;
        let r = self.rng.f();
        if r < p_sud {
            // Casi siempre medio metro a un metro; a veces una gran sudestada de casi dos metros.
            let big = self.rng.f() < 0.15;
            let peak = if big { 1.6 + self.rng.f() * 0.6 } else { 0.5 + self.rng.f() * 0.6 };
            self.wind = Wind::Sudestada { peak };
            self.event_hours = 24.0 + self.rng.f() * 36.0;
            self.event_elapsed = 0.0;
            self.last_peak = 0.0;
            return Some(if big {
                "¡Gran sudestada! El río sube con fuerza: los bajos de la costa se van a inundar".into()
            } else {
                "Sudestada: el viento del sudeste empuja el río contra la costa".into()
            });
        } else if r < p_sud + p_pam {
            let drop = 0.4 + self.rng.f() * 0.8;
            self.wind = Wind::Pampero { drop };
            self.event_hours = 12.0 + self.rng.f() * 24.0;
            self.event_elapsed = 0.0;
            return Some("Pampero: el río se retira y deja al descubierto los bancos de arena".into());
        }
        None
    }

    /// Una vez por mes del calendario: caudal de los ríos Uruguay y Paraná. Hay regímenes de
    /// varios años (lluviosos o secos, como los de El Niño y La Niña); la sequía de la
    /// campaña (`drought`) baja el río y los años lluviosos traen crecientes.
    pub fn month_tick(&mut self, drought: bool) -> Option<String> {
        if self.regime_months == 0 {
            self.regime_months = 12 + (self.rng.f() * 30.0) as u32;
            let r = self.rng.f();
            self.regime = if r < 0.25 { 1.0 } else if r < 0.5 { -1.0 } else { 0.0 };
        }
        self.regime_months -= 1;
        let target = if drought { -0.35 } else { self.regime * 0.3 };
        let before = self.river;
        self.river += (target - self.river) * 0.25;
        if before < 0.2 && self.river >= 0.2 {
            return Some("Crecientes del Uruguay y el Paraná: el río viene alto".into());
        }
        if before > -0.2 && self.river <= -0.2 {
            return Some("Bajante: el río está más bajo que de costumbre".into());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mareas_vivas_y_muertas() {
        // Rango de un día en luna llena (vivas) y en cuarto (muertas).
        let range = |phase: f32| {
            let v: Vec<f32> = (0..96).map(|k| astronomical_tide(k as f32 * 0.25, phase)).collect();
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!(range(0.5) > range(0.25) * 1.5, "vivas {} / muertas {}", range(0.5), range(0.25));
        assert!(range(0.5) < 0.9);
        // Dos pleamares por día.
        let v: Vec<f32> = (0..240).map(|k| astronomical_tide(k as f32 * 0.1, 0.0)).collect();
        let peaks = (1..v.len() - 1).filter(|&i| v[i] > v[i - 1] && v[i] >= v[i + 1]).count();
        assert!((2..=3).contains(&peaks), "{peaks} pleamares en 24 h");
        assert_eq!(moon_name(0.0), "luna nueva");
        assert_eq!(moon_name(0.5), "luna llena");
        assert!(moon_light(0.5) > 0.99 && moon_light(0.0) < 0.01);
    }

    #[test]
    fn sudestadas_y_pampero() {
        let mut w = WaterLevel::default();
        let (mut hi, mut lo, mut events) = (0f32, 0f32, 0);
        // Dos años de horas, mes por mes.
        for h in 0..(24 * 730) {
            if w.advance(1.0, (h / (24 * 30)) as u32).is_some() {
                events += 1;
            }
            hi = hi.max(w.surge());
            lo = lo.min(w.surge());
        }
        assert!(events >= 4, "{events} temporales en dos años");
        assert!(hi > 0.4 && hi < 2.4, "pico {hi}");
        assert!(lo < -0.2 && lo > -1.3, "bajante {lo}");
    }

    #[test]
    fn la_sequia_baja_el_rio() {
        let mut w = WaterLevel::default();
        for _ in 0..12 {
            w.month_tick(true);
        }
        assert!(w.river < -0.25, "río {}", w.river);
    }
}
