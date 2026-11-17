//! Clima y estaciones de Montevideo con datos reales, y el calendario del campo.
//!
//! Base: normales 1991–2020 de la estación Prado del INUMET (`assets/clima/`), menos 0,5 °C
//! por la época (Pequeña Edad de Hielo). Cada mes del juego sale de esas normales con la
//! variabilidad real del Uruguay: la lluvia de un mes puede ser la mitad o el doble de la
//! normal, y hay regímenes de varios años lluviosos o secos (como los de El Niño y La Niña).
//! El agua del suelo se lleva con un balance simple (lluvia menos evapotranspiración de
//! Thornthwaite): por eso los veranos suelen quedar secos aunque llueva lo mismo todo el año.
//!
//! El campo sigue el calendario de la época: el trigo se siembra en otoño (mayo a julio) y se
//! cosecha en diciembre; el maíz se siembra en primavera y se cosecha en marzo y abril; las
//! huertas dan en verano; las pasturas del ganado crecen en primavera y otoño. Cada cultivo
//! depende de la lluvia, las heladas y la sequía de su temporada. Ver
//! `docs/sistemas/clima-y-estaciones.md`.

use crate::buildings::Rng;

/// Normales INUMET 1991–2020, estación Prado (enero a diciembre).
pub const T_MEDIA: [f32; 12] = [23.3, 22.8, 21.2, 18.1, 14.8, 11.9, 11.0, 12.6, 13.9, 16.5, 19.2, 21.8];
pub const T_MIN_MEDIA: [f32; 12] = [18.8, 18.6, 17.1, 14.1, 11.0, 8.2, 7.3, 8.5, 9.8, 12.4, 14.7, 17.1];
pub const T_MAX_MEDIA: [f32; 12] = [27.8, 27.0, 25.3, 22.0, 18.5, 15.6, 14.7, 16.7, 17.9, 20.7, 23.7, 26.4];
pub const LLUVIA_MM: [f32; 12] = [95.0, 94.0, 106.0, 111.0, 83.0, 89.0, 93.0, 90.0, 92.0, 102.0, 96.0, 91.0];
pub const DIAS_LLUVIA: [f32; 12] = [6.0, 6.0, 6.0, 7.0, 6.0, 7.0, 6.0, 7.0, 7.0, 7.0, 7.0, 7.0];
/// Corrección de la época (siglo XVIII, algo más fresco).
pub const EPOCA_C: f32 = -0.5;
/// Horas de luz a 35° S (para la evapotranspiración) y días de cada mes.
const LUZ_H: [f32; 12] = [14.2, 13.3, 12.2, 11.1, 10.2, 9.7, 9.9, 10.7, 11.8, 12.9, 13.9, 14.5];
const DIAS_MES: [f32; 12] = [31.0, 28.0, 31.0, 30.0, 31.0, 30.0, 31.0, 31.0, 30.0, 31.0, 30.0, 31.0];
/// Agua útil que guarda el suelo de la pradera (mm).
pub const SUELO_MAX: f32 = 120.0;

pub const MESES: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

pub fn season(m: usize) -> &'static str {
    match m % 12 {
        11 | 0 | 1 => "verano",
        2..=4 => "otoño",
        5..=7 => "invierno",
        _ => "primavera",
    }
}

/// Régimen de varios años.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Regime {
    Normal,
    /// Años lluviosos (como El Niño): más lluvia en primavera y verano, crecientes.
    Lluvioso,
    /// Años secos (como La Niña): sequías de verano.
    Seco,
}

/// El tiempo de un mes.
#[derive(Clone, Debug, Default)]
pub struct Weather {
    pub month: usize,
    pub temp: f32,
    pub tmin: f32,
    pub rain: f32,
    pub rain_days: f32,
    pub frost_days: f32,
    pub pet: f32,
    /// Agua en el suelo al terminar el mes (0 a SUELO_MAX mm).
    pub soil: f32,
    pub drought: bool,
    /// Lluvias muy por encima de lo normal: crecen los arroyos.
    pub deluge: bool,
}

/// Un cultivo de la campaña: dónde está en su ciclo y cómo viene.
#[derive(Clone, Debug)]
pub struct Crop {
    pub name: &'static str,
    /// Meses de siembra, de crecimiento y de cosecha (0 = enero).
    pub sow: &'static [usize],
    pub grow: &'static [usize],
    pub harvest: &'static [usize],
    /// Sembrado esta temporada y cómo viene (1,0 = año normal).
    pub planted: bool,
    pub health: f32,
    /// Rendimiento de la última cosecha (1,0 = normal).
    pub last_yield: f32,
}

impl Crop {
    fn new(name: &'static str, sow: &'static [usize], grow: &'static [usize], harvest: &'static [usize]) -> Crop {
        Crop { name, sow, grow, harvest, planted: false, health: 1.0, last_yield: 1.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Climate {
    pub regime: Regime,
    regime_months: u32,
    temp_anom: f32,
    pub soil: f32,
    dry_months: u32,
    pub last: Weather,
    pub trigo: Crop,
    pub maiz: Crop,
    pub huerta: Crop,
    /// Estado de las pasturas (0 a 1,2): de ellas comen las vacas.
    pub pasture: f32,
    rng: Rng,
    /// Evapotranspiración potencial normal de cada mes (Thornthwaite).
    pet_norm: [f32; 12],
    /// Últimos 12 meses (el más reciente al final), para el panel del clima.
    pub history: std::collections::VecDeque<Weather>,
}

/// Evapotranspiración potencial mensual (mm) de Thornthwaite para las temperaturas normales.
fn thornthwaite() -> ([f32; 12], f32, f32) {
    let i: f32 = T_MEDIA.iter().map(|t| (t.max(0.0) / 5.0).powf(1.514)).sum();
    let a = 6.75e-7 * i.powi(3) - 7.71e-5 * i * i + 1.792e-2 * i + 0.49239;
    let mut out = [0f32; 12];
    for m in 0..12 {
        out[m] = 16.0 * (10.0 * T_MEDIA[m].max(0.0) / i).powf(a) * (LUZ_H[m] / 12.0) * (DIAS_MES[m] / 30.0);
    }
    (out, i, a)
}

impl Default for Climate {
    fn default() -> Self {
        Climate {
            regime: Regime::Normal,
            regime_months: 0,
            temp_anom: 0.0,
            soil: 90.0,
            dry_months: 0,
            last: Weather::default(),
            trigo: Crop::new("trigo", &[4, 5, 6], &[6, 7, 8, 9, 10], &[11]),
            maiz: Crop::new("maíz", &[9, 10], &[11, 0, 1], &[2, 3]),
            huerta: Crop::new("huertas (zapallo, porotos, hortalizas)", &[8, 9, 10], &[10, 11], &[11, 0, 1, 2]),
            pasture: 0.8,
            rng: Rng::new(1724),
            pet_norm: thornthwaite().0,
            history: std::collections::VecDeque::new(),
        }
    }
}

impl Climate {
    /// Estado para guardar la partida: valores y el estado del azar.
    pub fn to_state(&self) -> (Vec<f32>, u64) {
        let mut v = vec![self.regime as u8 as f32, self.regime_months as f32, self.temp_anom, self.soil, self.dry_months as f32, self.pasture];
        for c in [&self.trigo, &self.maiz, &self.huerta] {
            v.extend([c.planted as u8 as f32, c.health, c.last_yield]);
        }
        let w = &self.last;
        v.extend([w.month as f32, w.temp, w.tmin, w.rain, w.rain_days, w.frost_days, w.pet, w.soil, w.drought as u8 as f32, w.deluge as u8 as f32]);
        (v, self.rng.state())
    }

    pub fn from_state(v: &[f32], azar: u64) -> Climate {
        let mut c = Climate::default();
        if v.len() >= 15 {
            c.regime = match v[0] as u8 { 1 => Regime::Lluvioso, 2 => Regime::Seco, _ => Regime::Normal };
            c.regime_months = v[1] as u32;
            c.temp_anom = v[2];
            c.soil = v[3];
            c.dry_months = v[4] as u32;
            c.pasture = v[5];
            for (k, cr) in [&mut c.trigo, &mut c.maiz, &mut c.huerta].into_iter().enumerate() {
                cr.planted = v[6 + k * 3] > 0.5;
                cr.health = v[7 + k * 3];
                cr.last_yield = v[8 + k * 3];
            }
            if v.len() >= 25 {
                let w = &v[15..25];
                c.last = Weather { month: w[0] as usize, temp: w[1], tmin: w[2], rain: w[3], rain_days: w[4], frost_days: w[5], pet: w[6], soil: w[7], drought: w[8] > 0.5, deluge: w[9] > 0.5 };
            }
            c.rng = Rng::from_state(azar);
        }
        c
    }

    fn gauss(&mut self) -> f32 {
        let u1 = self.rng.f().max(1e-6);
        let u2 = self.rng.f();
        (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
    }

    /// Avanza un mes (`month` = mes del calendario, 0 = enero) y devuelve el tiempo y avisos.
    pub fn month_tick(&mut self, month: u32) -> (Weather, Vec<String>) {
        let m = (month % 12) as usize;
        let mut news = Vec::new();
        // Régimen de varios años.
        if self.regime_months == 0 {
            self.regime_months = 12 + (self.rng.f() * 30.0) as u32;
            let r = self.rng.f();
            let new = if r < 0.22 { Regime::Lluvioso } else if r < 0.44 { Regime::Seco } else { Regime::Normal };
            if new != self.regime {
                news.push(match new {
                    Regime::Lluvioso => "Vienen años lluviosos: buenas pasturas, pero crecientes y trigo con roya".to_string(),
                    Regime::Seco => "Vienen años secos: hay que cuidar el agua y guardar grano".to_string(),
                    Regime::Normal => "El tiempo vuelve a lo de siempre".to_string(),
                });
            }
            self.regime = new;
        }
        self.regime_months -= 1;
        // Temperatura: anomalía que persiste de un mes al otro.
        self.temp_anom = self.temp_anom * 0.5 + self.gauss() * 0.9;
        let temp = T_MEDIA[m] + EPOCA_C + self.temp_anom;
        let tmin = T_MIN_MEDIA[m] + EPOCA_C + self.temp_anom;
        // Lluvia: lognormal con la media de la normal (coeficiente de variación ≈ 0,6).
        let sigma = 0.55;
        let mut rain = LLUVIA_MM[m] * (sigma * self.gauss() - sigma * sigma / 2.0).exp();
        let warm = matches!(m, 9 | 10 | 11 | 0 | 1);
        rain *= match self.regime {
            Regime::Lluvioso => if warm { 1.35 } else { 1.1 },
            Regime::Seco => if warm { 0.6 } else { 0.85 },
            Regime::Normal => 1.0,
        };
        let rain_days = (DIAS_LLUVIA[m] * (rain / LLUVIA_MM[m]).sqrt()).clamp(0.0, 20.0);
        // Heladas: con mínimas medias bajo 9 °C; Montevideo tiene pocas (la costa abriga).
        let frost_days = if tmin < 9.0 { ((9.0 - tmin) * 0.8 * (0.5 + self.rng.f())).clamp(0.0, 14.0) } else { 0.0 };
        // Balance de agua del suelo.
        let pet = self.pet_norm[m] * (1.0 + self.temp_anom * 0.07);
        self.soil = (self.soil + rain - pet).clamp(0.0, SUELO_MAX);
        if self.soil < 25.0 {
            self.dry_months += 1;
        } else {
            self.dry_months = 0;
        }
        let drought = self.dry_months >= 2;
        let deluge = rain > LLUVIA_MM[m] * 2.0 && rain > 160.0;
        if self.dry_months == 2 {
            news.push("Sequía: los campos están secos y las cañadas cortadas".into());
        }
        if deluge {
            news.push(format!("Temporal de lluvias: {} mm en {}; crecen los arroyos", rain as i32, MESES[m]));
        }
        if frost_days >= 4.0 {
            news.push(format!("Heladas fuertes en {}: se queman las huertas y el pasto", MESES[m]));
        }
        let w = Weather { month: m, temp, tmin, rain, rain_days, frost_days, pet, soil: self.soil, drought, deluge };
        self.crops_tick(&w, &mut news);
        self.last = w.clone();
        self.history.push_back(w.clone());
        while self.history.len() > 12 {
            self.history.pop_front();
        }
        (w, news)
    }

    fn crops_tick(&mut self, w: &Weather, news: &mut Vec<String>) {
        let m = w.month;
        let wet = w.soil / SUELO_MAX;
        // Pasturas del campo natural: crecen con temperatura templada y agua en el suelo.
        let tf = if w.temp < 8.0 { 0.35 } else if w.temp < 12.0 { 0.65 } else if w.temp <= 24.0 { 1.0 } else { 0.85 };
        let growth = tf * (0.25 + 0.95 * wet.min(1.0)) - w.frost_days * 0.03;
        self.pasture = (self.pasture + (growth.clamp(0.1, 1.2) - self.pasture) * 0.5).clamp(0.1, 1.2);
        // Trigo: siembra de otoño con el suelo húmedo; heladas tardías, seca y exceso de lluvia
        // en primavera (roya) lo castigan; se cosecha en diciembre.
        let (trigo, maiz, huerta) = (&mut self.trigo, &mut self.maiz, &mut self.huerta);
        if trigo.sow.contains(&m) && !trigo.planted && w.soil > 35.0 {
            trigo.planted = true;
            trigo.health = 1.0;
        }
        if trigo.planted && trigo.grow.contains(&m) {
            if matches!(m, 8 | 9) {
                trigo.health -= w.frost_days * 0.04; // helada en la espigazón
            }
            if matches!(m, 8..=10) && w.soil < 30.0 {
                trigo.health -= 0.18;
            }
            if matches!(m, 9 | 10) && w.rain > LLUVIA_MM[m] * 1.7 {
                trigo.health -= 0.12; // roya y vuelco
            }
            if wet > 0.6 && w.rain < LLUVIA_MM[m] * 1.4 {
                trigo.health += 0.04;
            }
        }
        // Maíz: siembra de primavera con el suelo tibio; sufre la seca de verano.
        if maiz.sow.contains(&m) && !maiz.planted && w.soil > 35.0 && w.temp > 14.0 {
            maiz.planted = true;
            maiz.health = 1.0;
        }
        if maiz.planted && maiz.grow.contains(&m) {
            if w.soil < 35.0 {
                maiz.health -= 0.22;
            } else if wet > 0.6 {
                maiz.health += 0.05;
            }
        }
        // Huertas: necesitan agua y no aguantan heladas.
        if huerta.sow.contains(&m) && !huerta.planted && w.frost_days < 1.0 {
            huerta.planted = true;
            huerta.health = 1.0;
        }
        if huerta.planted && (huerta.grow.contains(&m) || huerta.harvest.contains(&m)) {
            if w.soil < 30.0 {
                huerta.health -= 0.15;
            }
            huerta.health -= w.frost_days * 0.1;
        }
        for c in [&mut self.trigo, &mut self.maiz, &mut self.huerta] {
            c.health = c.health.clamp(0.05, 1.3);
        }
        // Cosechas.
        for c in [&mut self.trigo, &mut self.maiz] {
            if c.harvest.last() == Some(&m) {
                c.last_yield = if c.planted { c.health } else { 0.0 };
                if c.planted {
                    let how = if c.last_yield >= 1.1 { "excelente" } else if c.last_yield >= 0.85 { "buena" } else if c.last_yield >= 0.5 { "pobre" } else { "perdida" };
                    news.push(format!("Cosecha de {} {}: rinde {} %", c.name, how, (c.last_yield * 100.0).round() as i32));
                } else {
                    news.push(format!("No hubo cosecha de {}: no se pudo sembrar a tiempo", c.name));
                }
                c.planted = false;
            }
        }
        if self.huerta.harvest.last() == Some(&m) {
            self.huerta.last_yield = self.huerta.health;
            self.huerta.planted = false;
        }
    }

    /// Granos (raciones) que entran este mes de las chacras, para una cosecha anual normal de
    /// `annual` raciones: el trigo en diciembre (y lo que se trilla en enero), el maíz en
    /// marzo y abril, las huertas en verano. Más un poco de molienda y comercio cada mes.
    pub fn grain_this_month(&self, month: u32, annual: f32) -> f32 {
        let m = (month % 12) as usize;
        let mut g = annual * 0.12 / 12.0; // harina que llega de a poco del interior
        if m == 11 {
            g += annual * 0.65 * 0.6 * self.trigo.last_yield;
        }
        if m == 0 {
            g += annual * 0.65 * 0.4 * self.trigo.last_yield;
        }
        if matches!(m, 2 | 3) && self.maiz.harvest.contains(&m) {
            g += annual * 0.18 * 0.5 * self.maiz.last_yield.max(if self.maiz.planted { self.maiz.health } else { 0.0 });
        }
        if self.huerta.harvest.contains(&m) && self.huerta.planted {
            g += annual * 0.05 / 4.0 * self.huerta.health;
        }
        g
    }

    /// Factor de producción del campo ganadero (carne, cueros y sebo).
    pub fn livestock_factor(&self) -> f32 {
        0.55 + 0.5 * self.pasture
    }

    /// Hornos de ladrillos: el adobe no seca con lluvia ni con frío.
    pub fn brick_factor(&self) -> f32 {
        let w = &self.last;
        ((1.0 - w.rain_days / 14.0) * if w.temp < 10.0 { 0.75 } else { 1.0 }).clamp(0.3, 1.0)
    }

    /// Qué se puede sembrar este mes y en qué condiciones (para la ficha del clima).
    pub fn sowing_advice(&self, month: u32) -> Vec<String> {
        let m = (month % 12) as usize;
        let w = &self.last;
        let mut out = Vec::new();
        for c in [&self.trigo, &self.maiz, &self.huerta] {
            let line = if c.sow.contains(&m) {
                if c.planted {
                    format!("{}: ya sembrado, viene al {} %", c.name, (c.health * 100.0).round() as i32)
                } else if w.soil <= 35.0 {
                    format!("{}: es tiempo de sembrar, pero la tierra está seca", c.name)
                } else {
                    format!("{}: es tiempo de sembrar", c.name)
                }
            } else if c.planted {
                format!("{}: creciendo, viene al {} %", c.name, (c.health * 100.0).round() as i32)
            } else {
                let next = (1..=12).map(|k| (m + k) % 12).find(|x| c.sow.contains(x)).unwrap_or(m);
                format!("{}: no es temporada (se siembra desde {})", c.name, MESES[next])
            };
            out.push(line);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normales_y_evapotranspiracion() {
        let lluvia: f32 = LLUVIA_MM.iter().sum();
        assert!((lluvia - 1142.0).abs() < 2.0, "lluvia anual {lluvia}");
        let (pet, _, _) = thornthwaite();
        // Verano con más evapotranspiración que lluvia; invierno al revés.
        assert!(pet[0] > LLUVIA_MM[0] && pet[6] < LLUVIA_MM[6], "{pet:?}");
        let anual: f32 = pet.iter().sum();
        assert!((700.0..1000.0).contains(&anual), "ETP anual {anual}");
    }

    #[test]
    fn veinte_años_de_clima() {
        let mut c = Climate::default();
        let (mut rain, mut droughts, mut frosts, mut harvests) = (0.0, 0, 0.0, Vec::new());
        for k in 0..240u32 {
            let (w, _) = c.month_tick(k);
            rain += w.rain;
            droughts += w.drought as i32;
            frosts += w.frost_days;
            if w.month == 11 {
                harvests.push(c.trigo.last_yield);
            }
        }
        let anual = rain / 20.0;
        assert!((850.0..1450.0).contains(&anual), "lluvia media anual {anual}");
        assert!(droughts > 0, "en 20 años hay alguna sequía");
        assert!(frosts / 20.0 < 15.0, "{} heladas por año (Montevideo tiene pocas)", frosts / 20.0);
        // Hay años buenos y malos.
        let (lo, hi) = harvests.iter().fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
        assert!(hi - lo > 0.3, "cosechas de trigo entre {lo} y {hi}");
    }

    #[test]
    fn calendario_de_siembra() {
        let mut c = Climate::default();
        c.month_tick(4); // mayo
        let advice = c.sowing_advice(4).join(" / ");
        assert!(advice.contains("trigo: ya sembrado") || advice.contains("trigo: es tiempo"), "{advice}");
        assert!(advice.contains("maíz: no es temporada"), "{advice}");
        assert_eq!(season(6), "invierno");
        assert_eq!(season(0), "verano");
    }
}
