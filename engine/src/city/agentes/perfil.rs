//! Perfil de conducta de cada vecino (`docs/sistemas/ia-y-transito.md` §3): de la persona de
//! la población (rasgos, edad, salud, riqueza, clase, oficio, felicidad, años en la ciudad)
//! salen unos pocos números que deciden cómo viaja: prisa, prudencia, cortesía, paciencia,
//! velocidad a pie, sociabilidad, devoción, conocimiento de la ciudad y medios propios.
//!
//! Nada se sortea por cuadro: lo que no sale de la persona sale de un azar **por contador**,
//! `azar(semilla, persona, día, motivo)`, así que es determinista y agregar un sorteo nuevo
//! no corre a los demás (D16).
//!
//! **Decisión de cuidado** (§3.2): no se usan el sexo ni el origen para el estilo de manejo ni
//! para la velocidad.

use super::datos::datos;
use crate::city::poblacion::modelo::{Clase, Oficio, Persona, TipoEvento};
use serde::{Deserialize, Serialize};

/// Motivos del azar por contador: cada sorteo tiene el suyo (nunca se reutiliza un número).
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum Motivo {
    Perfil = 1,
    Variacion,
    Horario,
    Compra,
    Recova,
    Pulperia,
    Misa,
    Paseo,
    Duracion,
    Fidelidad,
    Lluvia,
    Arreo,
    Revision,
    Empantanar,
    Visita,
    Rasgo,
}

/// Azar por contador: un número en [0, 1) que solo depende de sus cuatro argumentos
/// (*splitmix64* sobre la mezcla).
pub fn azar(semilla: u64, persona: u64, dia: u32, motivo: Motivo) -> f32 {
    azar_n(semilla, persona, dia, motivo as u32, 0)
}

/// Como [`azar`], con un índice más (varios sorteos del mismo motivo en el día).
pub fn azar_n(semilla: u64, persona: u64, dia: u32, motivo: u32, n: u32) -> f32 {
    let mut z = semilla ^ persona.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((dia as u64) << 32 | motivo as u64).wrapping_mul(0xD1B5_4A32_D192_ED03) ^ (n as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// Medios propios del hogar (bits de [`Perfil::medios`]).
pub const CABALLO: u8 = 1;

/// Perfil de 12 bytes (cada valor de 0 a 255 representa 0 a 1, salvo `v_pie` y `edad`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Perfil {
    pub prisa: u8,
    pub prudencia: u8,
    pub cortesia: u8,
    /// Segundos de espera (simulados) antes de achicar el hueco que acepta en un cruce.
    pub paciencia: u8,
    /// Velocidad a pie en cm/s / 2 (135 cm/s → 67).
    pub v_pie: u8,
    pub valor_tiempo: u8,
    pub flexibilidad: u8,
    pub social: u8,
    pub devocion: u8,
    pub conocimiento: u8,
    pub medios: u8,
    /// Edad en años (tope 255): los chicos no salen solos a caballo; los mayores no salen de
    /// noche ni con temporal.
    pub edad: u8,
}

fn b(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn f(x: u8) -> f32 {
    x as f32 / 255.0
}

impl Perfil {
    pub fn prisa(&self) -> f32 {
        f(self.prisa)
    }
    pub fn prudencia(&self) -> f32 {
        f(self.prudencia)
    }
    #[allow(dead_code)] // para el cambio de carril (MOBIL, épocas con carriles)
    pub fn cortesia(&self) -> f32 {
        f(self.cortesia)
    }
    pub fn social(&self) -> f32 {
        f(self.social)
    }
    pub fn devocion(&self) -> f32 {
        f(self.devocion)
    }
    #[allow(dead_code)] // para anticipar el carril (épocas con carriles)
    pub fn conocimiento(&self) -> f32 {
        f(self.conocimiento)
    }
    pub fn flexibilidad(&self) -> f32 {
        f(self.flexibilidad)
    }
    pub fn caballo(&self) -> bool {
        self.medios & CABALLO != 0 && self.edad >= 12
    }
    /// Velocidad a pie (m/s).
    pub fn velocidad_a_pie(&self) -> f32 {
        if self.v_pie == 0 {
            1.35
        } else {
            self.v_pie as f32 * 0.02
        }
    }

    /// Perfil de una persona de la población. `clase` y `ganado` son los de su hogar; `mes`,
    /// el mes absoluto; `semilla`, la de la partida.
    pub fn de_persona(p: &Persona, clase: Clase, ganado: u32, mes: i32, semilla: u64) -> Perfil {
        let r = &p.rasgos;
        let edad = p.edad(mes).clamp(0, 255);
        let infeliz = (0.5 - p.felicidad).max(0.0) * 2.0;
        // Prisa estructural: ambición, laboriosidad y oficios de horario rígido.
        let rigido = matches!(p.oficio, Oficio::Soldado | Oficio::Funcionario | Oficio::Pulpero | Oficio::Maestro);
        let prisa = 0.45 * r.ambicion() + 0.35 * r.laboriosidad() + if rigido { 0.2 } else { 0.0 };
        // Prudencia: el rasgo, un poco más con los años.
        let prudencia = r.prudencia() * 0.8 + (edad as f32 / 80.0).min(1.0) * 0.2;
        let cortesia = ((r.sociabilidad() + r.prudencia()) * 0.5 * (1.0 - 0.2 * infeliz)).clamp(0.0, 1.0);
        let paciencia = (8.0 + 40.0 * r.prudencia()) * (1.0 - 0.2 * infeliz);
        // Velocidad a pie (Weidmann 1993; Knoblauch et al. 1996): chicos ~1,1, adultos
        // 1,3-1,45, mayores 1,0-1,2; la salud la baja.
        let base = match edad {
            0..=11 => 1.1,
            12..=59 => 1.3 + 0.15 * azar(semilla, p.id as u64, 0, Motivo::Perfil),
            60..=74 => 1.15,
            _ => 1.0,
        };
        let v_pie = base * (0.7 + 0.3 * p.salud.clamp(0.0, 1.0));
        let valor_tiempo = match clase {
            Clase::Pobre => 0.15,
            Clase::Modesta => 0.35,
            Clase::Acomodada => 0.65,
            Clase::Principal => 0.9,
        };
        let flexible = datos().horario(&format!("{:?}", p.oficio)).flexible;
        let flexibilidad = if flexible { 0.7 } else { 0.25 } + 0.2 * azar(semilla, p.id as u64, 1, Motivo::Perfil);
        // Años en la ciudad: desde que llegó (o desde que nació, si es de acá).
        let llegada = p.historial.iter().find_map(|e| matches!(e.tipo, TipoEvento::Llego(_)).then_some(e.mes)).unwrap_or(p.nacimiento);
        let anios = ((mes - llegada) / 12).max(0) as f32;
        let conocimiento = (anios / 15.0).min(1.0);
        // Caballo propio: las familias acomodadas, y quienes trabajan en el campo (los peones,
        // si el hogar tiene ganado).
        let caballo = clase >= Clase::Acomodada || matches!(p.oficio, Oficio::Hacendado | Oficio::Labrador) || (ganado > 0 && p.oficio == Oficio::Peon);
        // Social: la sociabilidad, menos para los infelices (que igual van a la pulpería).
        let social = r.sociabilidad();
        Perfil {
            prisa: b(prisa),
            prudencia: b(prudencia),
            cortesia: b(cortesia),
            paciencia: paciencia.round().clamp(1.0, 255.0) as u8,
            v_pie: (v_pie / 0.02).round().clamp(1.0, 255.0) as u8,
            valor_tiempo: b(valor_tiempo),
            flexibilidad: b(flexibilidad),
            social: b(social),
            devocion: b(r.devocion()),
            conocimiento: b(conocimiento),
            medios: if caballo { CABALLO } else { 0 },
            edad: edad as u8,
        }
    }

    /// Perfil de un vecino sintético (sin población): sale de su temperamento (0 prudente,
    /// 1 tranquilo, 2 apurado), de su rol y del azar por contador de su clave.
    pub fn sintetico(clave: u64, temple: u8, edad: u8, jinete: bool, semilla: u64) -> Perfil {
        let x = |i: u32| azar_n(semilla, clave, 0, Motivo::Rasgo as u32, i);
        let prudencia = match temple % 3 {
            0 => 0.7 + 0.25 * x(0),
            1 => 0.4 + 0.25 * x(0),
            _ => 0.1 + 0.25 * x(0),
        };
        let prisa = match temple % 3 {
            2 => 0.65 + 0.3 * x(1),
            _ => 0.2 + 0.4 * x(1),
        };
        let social = x(2);
        Perfil {
            prisa: b(prisa),
            prudencia: b(prudencia),
            cortesia: b((social + prudencia) * 0.5),
            paciencia: (8.0 + 40.0 * prudencia).round() as u8,
            v_pie: (((if edad < 12 { 1.1 } else if edad >= 60 { 1.15 } else { 1.3 + 0.15 * x(3) }) / 0.02) as f32).round() as u8,
            valor_tiempo: b(if jinete { 0.7 } else { 0.3 }),
            flexibilidad: b(0.3 + 0.5 * x(4)),
            social: b(social),
            devocion: b(x(5)),
            conocimiento: b(0.3 + 0.7 * x(6)),
            medios: if jinete { CABALLO } else { 0 },
            edad,
        }
    }

    /// Factores del conductor (IDM, §3.2): velocidad deseada, separación `T`, distancia
    /// mínima `s0` y frenada cómoda `b`.
    pub fn factores_idm(&self) -> (f32, f32, f32, f32) {
        let p = self.prudencia();
        (0.95 + 0.1 * self.prisa(), 0.8 + 0.6 * p, 0.8 + 0.5 * p, 1.1 - 0.3 * p)
    }

    /// Hueco crítico (s) para entrar a un cruce desde la calle secundaria después de esperar
    /// `espera` segundos: crece con la prudencia (Raff y Hart 1950) y baja con la espera, de a
    /// `baja_por_espera_s` por cada `paciencia` segundos (Mahmassani y Sheffi 1981).
    pub fn hueco_critico(&self, espera: f32) -> f32 {
        let c = &datos().cruces;
        let base = c.hueco_base_s + c.hueco_por_prudencia_s * self.prudencia();
        let paciencia = (self.paciencia.max(1)) as f32;
        (base - c.baja_por_espera_s * (espera / paciencia).floor()).max(c.hueco_minimo_s)
    }

    /// Hueco (s) que pide un peatón para cruzar la calzada.
    pub fn hueco_peaton(&self) -> f32 {
        let c = &datos().cruces;
        c.peaton_hueco_base_s + c.peaton_hueco_por_prudencia_s * self.prudencia()
    }

    /// ¿Adelanta? Y con cuánto carril contrario libre (m).
    pub fn adelanta(&self) -> Option<f32> {
        let a = &datos().adelantamiento;
        let p = self.prudencia();
        (p < a.prudencia_maxima).then(|| a.libre_base_m + a.libre_por_prudencia_m * p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_azar_por_contador_no_se_corre() {
        // El mismo contador da lo mismo; otro motivo da otra cosa.
        let a = azar(7, 42, 3, Motivo::Compra);
        assert_eq!(a, azar(7, 42, 3, Motivo::Compra));
        assert_ne!(a, azar(7, 42, 3, Motivo::Recova));
        assert_ne!(a, azar(7, 42, 4, Motivo::Compra));
        // Uniforme: la media de muchos sorteos ronda 0,5.
        let m: f32 = (0..10_000).map(|i| azar(1, i, 0, Motivo::Paseo)).sum::<f32>() / 10_000.0;
        assert!((m - 0.5).abs() < 0.02, "{m}");
    }

    #[test]
    fn los_prudentes_aceptan_huecos_mas_largos() {
        let prudente = Perfil { prudencia: 230, paciencia: 40, ..Default::default() };
        let apurado = Perfil { prudencia: 30, paciencia: 10, ..Default::default() };
        assert!(prudente.hueco_critico(0.0) > apurado.hueco_critico(0.0) + 1.0);
        // La espera achica el hueco, nunca por debajo del mínimo.
        assert!(apurado.hueco_critico(30.0) < apurado.hueco_critico(0.0));
        assert!(apurado.hueco_critico(10_000.0) >= datos().cruces.hueco_minimo_s);
        // El prudente se impacienta más despacio.
        assert!(prudente.hueco_critico(25.0) > apurado.hueco_critico(25.0));
        assert!(prudente.hueco_peaton() > apurado.hueco_peaton());
        assert!(prudente.adelanta().is_none() && apurado.adelanta().is_some());
    }
}
