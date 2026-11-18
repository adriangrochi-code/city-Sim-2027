//! Personas: figuras de micro-vóxeles por atuendo (hombre, mujer, niño y niña; ropa de cada
//! época, clase y oficio) y la elección del atuendo y sus colores para cada vecino.
//!
//! La elección sale de la persona de la demografía (sexo, edad, oficio, estamento, riqueza y
//! raíz) o, para los vecinos sintéticos, de su nombre, oficio y casa; es una función pura de
//! esos datos, de la semilla de la persona y del año (siempre el mismo atuendo para la misma
//! persona en la misma época). Las reglas por época son datos ([`REGLAS`]).
//!
//! Sin caricaturas: todas las figuras comparten las mismas proporciones; la piel tiene tonos
//! naturales y lo que distingue a cada uno es la ropa de su oficio y su época.

use super::{r, Lienzo, Modelo, Paleta, Parte};
use crate::city::poblacion::modelo::{Oficio, Raiz};

/// Atuendos (cada uno implica sexo y edad: la figura se arma con sus proporciones).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Atuendo {
    /// Hombre de campo: poncho, chiripá, calzoncillo cribado, botas de potro y sombrero.
    Paisano,
    /// Peón: camisa, chiripá, faja y vincha.
    Peon,
    /// Artesano: camisa, chaleco, calzón corto, medias, zapatos y gorro.
    Artesano,
    /// Pulpero o tendero: camisa arremangada, chaleco y delantal.
    Pulpero,
    /// Vecino principal: casaca, chupa, calzón, medias y tricornio.
    Principal,
    /// Soldado de la guarnición: casaca azul con vueltas, correaje y tricornio.
    Soldado,
    /// Fraile: hábito con capucha y cordón.
    Fraile,
    /// Marinero o lanchero: camisa a rayas, calzón ancho y gorro de lana.
    Marinero,
    /// Señora: vestido largo y mantilla.
    Senora,
    /// Mujer del pueblo: falda, blusa, rebozo y trenza.
    MujerPueblo,
    /// Lavandera: pañuelo en la cabeza, delantal y el atado de ropa.
    Lavandera,
    Nino,
    Nina,
    // --- Épocas siguientes ---
    /// Levita, pantalón largo y galera (1815–1880).
    Levita,
    /// Señora de la Patria Vieja: vestido amplio, peinetón y mantilla (1815–1860).
    DamaPeineton,
    /// Obrero: saco corto, pantalón y gorra (1880–).
    Obrero,
    /// Traje, corbata y sombrero de fieltro (1890–).
    Traje,
    /// Señorita del novecientos: blusa, falda larga y sombrero.
    Senorita,
    /// Vestido a la rodilla (1940–).
    MujerModerna,
}

impl Atuendo {
    pub const TODOS: [Atuendo; 19] = [
        Atuendo::Paisano,
        Atuendo::Peon,
        Atuendo::Artesano,
        Atuendo::Pulpero,
        Atuendo::Principal,
        Atuendo::Soldado,
        Atuendo::Fraile,
        Atuendo::Marinero,
        Atuendo::Senora,
        Atuendo::MujerPueblo,
        Atuendo::Lavandera,
        Atuendo::Nino,
        Atuendo::Nina,
        Atuendo::Levita,
        Atuendo::DamaPeineton,
        Atuendo::Obrero,
        Atuendo::Traje,
        Atuendo::Senorita,
        Atuendo::MujerModerna,
    ];
    pub fn mujer(self) -> bool {
        matches!(self, Atuendo::Senora | Atuendo::MujerPueblo | Atuendo::Lavandera | Atuendo::Nina | Atuendo::DamaPeineton | Atuendo::Senorita | Atuendo::MujerModerna)
    }
    pub fn nino(self) -> bool {
        matches!(self, Atuendo::Nino | Atuendo::Nina)
    }
    pub fn nombre(self) -> &'static str {
        match self {
            Atuendo::Paisano => "poncho y chiripá",
            Atuendo::Peon => "ropa de peón",
            Atuendo::Artesano => "chaleco y calzón corto",
            Atuendo::Pulpero => "chaleco y delantal",
            Atuendo::Principal => "casaca y tricornio",
            Atuendo::Soldado => "uniforme de la guarnición",
            Atuendo::Fraile => "hábito",
            Atuendo::Marinero => "ropa de marinero",
            Atuendo::Senora => "vestido largo y mantilla",
            Atuendo::MujerPueblo => "falda y rebozo",
            Atuendo::Lavandera => "pañuelo y delantal de lavandera",
            Atuendo::Nino => "camisa y calzón",
            Atuendo::Nina => "vestido",
            Atuendo::Levita => "levita y galera",
            Atuendo::DamaPeineton => "vestido y peinetón",
            Atuendo::Obrero => "saco y gorra",
            Atuendo::Traje => "traje y sombrero",
            Atuendo::Senorita => "blusa, falda larga y sombrero",
            Atuendo::MujerModerna => "vestido",
        }
    }
    /// Lleva poncho o hábito que tapa los brazos (no se balancean).
    fn sin_brazos(self) -> bool {
        matches!(self, Atuendo::Paisano)
    }
}

/// Medidas de una figura (micro-vóxeles): cadera, hombro (alto del torso), cabeza.
struct Medidas {
    cadera: i32,
    hombro: i32,
    /// Medio ancho del torso y ancho de los brazos.
    torso: i32,
    brazo: i32,
    cabeza: i32,
}

fn medidas(a: Atuendo) -> Medidas {
    if a.nino() {
        Medidas { cadera: 7, hombro: 13, torso: 2, brazo: 1, cabeza: 4 }
    } else if a.mujer() {
        Medidas { cadera: 11, hombro: 20, torso: 3, brazo: 2, cabeza: 5 }
    } else {
        Medidas { cadera: 12, hombro: 21, torso: 3, brazo: 2, cabeza: 5 }
    }
}

/// Arma la figura de un atuendo, de pie (origen: el suelo entre los pies) o montada (origen:
/// el asiento; las piernas abiertas a los costados del caballo).
pub fn figura(a: Atuendo, montado: bool) -> Modelo {
    use Atuendo::*;
    use Parte::*;
    let m = medidas(a);
    let (ca, ho, t, br) = (m.cadera, m.hombro, m.torso, m.brazo);
    // De pie, el origen es el suelo; montado, el asiento (todo baja `ca`).
    let dy = if montado { -ca } else { 0 };
    let y = |v: i32| v + dy;
    let mut l = Lienzo::new([-8, -20, -6], [8, 40, 6]);
    let cab0 = ho + 1;
    let cab1 = cab0 + m.cabeza;
    l.pivote(Cabeza, [0.0, y(ho) as f32, 0.0]);
    l.pivote(BrazoI, [-(t as f32 + br as f32 * 0.5), y(ho) as f32 - 0.5, 0.0]);
    l.pivote(BrazoD, [t as f32 + br as f32 * 0.5, y(ho) as f32 - 0.5, 0.0]);
    l.pivote(PiernaI, [-1.5, y(ca) as f32, 0.5]);
    l.pivote(PiernaD, [1.5, y(ca) as f32, 0.5]);

    // --- Piernas (de pie) ---
    let ancho_p = if a.nino() { 2 } else { 3 };
    if !montado {
        // Calzado, y la pierna del color del calzón (lo que cambia va encima).
        l.par((PiernaI, PiernaD), r::CALZON, [0, ancho_p], [1, ca], [-1, 2]);
        l.par((PiernaI, PiernaD), r::CALZADO, [0, ancho_p], [0, 1], [-1, 3]);
    }
    // --- Torso, cuello, cabeza y pelo ---
    l.centrada(Fijo, r::ROPA, t, [y(ca), y(ho)], [-2, 2]);
    l.caja(Cabeza, r::PIEL, [-1, y(ho), -1], [1, y(cab0), 1]);
    l.caja(Cabeza, r::PIEL, [-2, y(cab0), -2], [2, y(cab1), 2]);
    // Pelo: arriba, atrás y en las sienes (la cara queda limpia).
    l.caja(Cabeza, r::PELO, [-2, y(cab1 - 1), -2], [2, y(cab1), 2]);
    l.caja(Cabeza, r::PELO, [-2, y(cab0 + 1), -2], [2, y(cab1), -1]);
    l.caja(Cabeza, r::PELO, [-2, y(cab1 - 2), -1], [-1, y(cab1 - 1), 1]);
    l.caja(Cabeza, r::PELO, [1, y(cab1 - 2), -1], [2, y(cab1 - 1), 1]);
    // --- Brazos con las manos ---
    let largo_b = if a.nino() { 6 } else { 9 };
    let mano = if a.nino() { 1 } else { 2 };
    if !a.sin_brazos() {
        l.par((BrazoI, BrazoD), r::ROPA, [t, t + br], [y(ho - largo_b), y(ho)], [-1, 1]);
        l.par((BrazoI, BrazoD), r::PIEL, [t, t + br], [y(ho - largo_b), y(ho - largo_b + mano)], [-1, 1]);
    }
    let pierna = |l: &mut Lienzo, ranura: u8, y0: i32, y1: i32| {
        if !montado {
            l.par((PiernaI, PiernaD), ranura, [0, ancho_p], [y0, y1], [-1, 2]);
        }
    };
    let manga = |l: &mut Lienzo, ranura: u8, y0: i32, y1: i32| l.par((BrazoI, BrazoD), ranura, [t, t + br], [y(y0), y(y1)], [-1, 1]);
    let sombrero_ala = |l: &mut Lienzo, ala: i32, copa: i32| {
        l.centrada(Cabeza, r::SOMBRERO, ala, [y(cab1), y(cab1 + 1)], [-ala, ala]);
        l.centrada(Cabeza, r::SOMBRERO, 2, [y(cab1 + 1), y(cab1 + 1 + copa)], [-2, 2]);
    };
    match a {
        Paisano => {
            // Botas de potro, calzoncillo cribado y chiripá; el poncho tapa brazos y torso.
            pierna(&mut l, r::CALZADO, 0, 3);
            pierna(&mut l, r::CLARO, 3, 7);
            l.centrada(Fijo, r::CALZON, t, [y(7), y(ca)], [-2, 2]);
            l.centrada(Fijo, r::CAPA, t + 2, [y(8), y(ho)], [-3, 3]);
            l.centrada(Fijo, r::CAPA, t + 1, [y(ho), y(ho + 1)], [-2, 2]);
            // Guardas del poncho.
            l.centrada(Fijo, r::ACENTO, t + 2, [y(9), y(10)], [-3, 3]);
            l.centrada(Fijo, r::ACENTO, t + 2, [y(ho - 3), y(ho - 2)], [-3, 3]);
            l.caja(Cabeza, r::ACENTO, [-1, y(ho), 1], [1, y(ho + 1), 2]);
            sombrero_ala(&mut l, 4, 2);
        }
        Peon => {
            pierna(&mut l, r::PIEL, 0, 1);
            pierna(&mut l, r::CLARO, 1, 7);
            l.centrada(Fijo, r::CALZON, t, [y(7), y(ca + 1)], [-2, 2]);
            l.centrada(Fijo, r::ACENTO, t, [y(ca + 1), y(ca + 2)], [-2, 2]);
            // Vincha.
            l.caja(Cabeza, r::ACENTO, [-2, y(cab1 - 2), -2], [2, y(cab1 - 1), 2]);
            manga(&mut l, r::PIEL, ho - largo_b, ho - 5);
        }
        Artesano => {
            pierna(&mut l, r::CLARO, 1, 6);
            l.centrada(Fijo, r::CAPA, t, [y(ca), y(ho)], [-2, 2]);
            l.caja(Fijo, r::ROPA, [-1, y(ho - 4), 1], [1, y(ho), 2]);
            // Gorro.
            l.caja(Cabeza, r::SOMBRERO, [-2, y(cab1 - 1), -2], [2, y(cab1 + 1), 2]);
        }
        Pulpero => {
            l.centrada(Fijo, r::CAPA, t, [y(ca), y(ho)], [-2, 2]);
            l.caja(Fijo, r::ROPA, [-1, y(ho - 4), 1], [1, y(ho), 2]);
            l.caja(Fijo, r::CLARO, [-2, y(5), 2], [2, y(ho - 4), 3]);
            manga(&mut l, r::PIEL, ho - largo_b, ho - 4);
        }
        Principal | Soldado => {
            let soldado = a == Soldado;
            pierna(&mut l, if soldado { r::CALZADO } else { r::CLARO }, 1, 6);
            // Casaca hasta las rodillas (faldones), abierta adelante sobre la chupa.
            l.centrada(Fijo, r::CAPA, t, [y(ca), y(ho)], [-2, 2]);
            l.centrada(Fijo, r::CAPA, t, [y(7), y(ca)], [-2, -1]);
            l.caja(Fijo, r::CAPA, [-t, y(7), -1], [-t + 1, y(ca), 2]);
            l.caja(Fijo, r::CAPA, [t - 1, y(7), -1], [t, y(ca), 2]);
            l.caja(Fijo, if soldado { r::CLARO } else { r::ROPA }, [-1, y(ca), 1], [1, y(ho - 1), 2]);
            manga(&mut l, r::CAPA, ho - largo_b + 2, ho);
            manga(&mut l, r::ACENTO, ho - largo_b + 2, ho - largo_b + 4);
            if soldado {
                // Cuello y vueltas encarnadas, correaje blanco cruzado.
                l.caja(Fijo, r::ACENTO, [-t, y(ho - 1), 1], [t, y(ho), 2]);
                for i in 0..(ho - ca) {
                    let x = -t + (i * 2 * t) / (ho - ca);
                    l.caja(Fijo, r::CLARO, [x, y(ho - 1 - i), 1], [x + 1, y(ho - i), 2]);
                    l.caja(Fijo, r::CLARO, [x, y(ho - 1 - i), -2], [x + 1, y(ho - i), -1]);
                }
            }
            // Coleta y tricornio (ala de tres picos: el frente y los costados levantados).
            l.caja(Cabeza, r::PELO, [-1, y(ho + 1), -3], [1, y(cab0 + 2), -2]);
            l.centrada(Cabeza, r::SOMBRERO, 3, [y(cab1), y(cab1 + 1)], [-3, 3]);
            l.centrada(Cabeza, r::SOMBRERO, 2, [y(cab1 + 1), y(cab1 + 2)], [-2, 2]);
            l.caja(Cabeza, r::SOMBRERO, [-3, y(cab1 + 1), -3], [-2, y(cab1 + 2), 2]);
            l.caja(Cabeza, r::SOMBRERO, [2, y(cab1 + 1), -3], [3, y(cab1 + 2), 2]);
            l.caja(Cabeza, r::SOMBRERO, [-2, y(cab1 + 1), -3], [2, y(cab1 + 2), -2]);
            if soldado {
                l.caja(Cabeza, r::ACENTO, [-1, y(cab1 + 1), 2], [1, y(cab1 + 2), 3]);
            }
        }
        Fraile => {
            // Hábito hasta los tobillos, cordón y capucha caída sobre la espalda; tonsura.
            l.centrada(Fijo, r::CAPA, t, [y(1), y(ho)], [-2, 2]);
            l.centrada(Fijo, r::CAPA, t + 1, [y(1), y(4)], [-2, 2]);
            l.centrada(Fijo, r::CLARO, t, [y(ca), y(ca + 1)], [-2, 2]);
            l.caja(Fijo, r::CLARO, [1, y(5), 2], [2, y(ca), 3]);
            l.centrada(Fijo, r::CAPA, t, [y(ho - 3), y(ho + 1)], [-3, -2]);
            manga(&mut l, r::CAPA, ho - largo_b + 2, ho);
            l.caja(Cabeza, r::PIEL, [-1, y(cab1 - 1), -1], [1, y(cab1), 1]);
        }
        Marinero => {
            pierna(&mut l, r::PIEL, 0, 1);
            l.centrada(Fijo, r::CALZON, t, [y(4), y(ca)], [-2, 2]);
            for k in 0..((ho - ca) / 2) {
                l.centrada(Fijo, r::ACENTO, t, [y(ca + 1 + k * 2), y(ca + 2 + k * 2)], [-2, 2]);
            }
            l.caja(Cabeza, r::SOMBRERO, [-2, y(cab1 - 1), -2], [2, y(cab1 + 2), 2]);
            manga(&mut l, r::PIEL, ho - largo_b, ho - 5);
        }
        Senora | DamaPeineton | MujerPueblo | Lavandera | Senorita => {
            // Falda larga hasta el suelo (más amplia abajo).
            let amplia = if a == DamaPeineton { 2 } else { 1 };
            l.centrada(Fijo, r::CALZON, t + amplia, [y(0), y(4)], [-2 - amplia, 2 + amplia]);
            l.centrada(Fijo, r::CALZON, t, [y(4), y(ca + 1)], [-2 - (amplia - 1), 2 + (amplia - 1)]);
            match a {
                Senora | DamaPeineton => {
                    if a == DamaPeineton {
                        // Peinetón de carey detrás de la cabeza, bajo la mantilla.
                        l.centrada(Cabeza, r::ACENTO, 4, [y(cab1 - 1), y(cab1 + 5)], [-3, -2]);
                    }
                    // Mantilla sobre la cabeza y los hombros (la cara al descubierto).
                    l.centrada(Cabeza, r::CAPA, 3, [y(cab0), y(cab1 + 1)], [-3, 1]);
                    l.centrada(Fijo, r::CAPA, t + 1, [y(ho - 4), y(ho + 1)], [-3, 2]);
                }
                MujerPueblo | Lavandera => {
                    // Rebozo sobre los hombros y trenza.
                    l.centrada(Fijo, r::CAPA, t + 1, [y(ho - 3), y(ho + 1)], [-3, 2]);
                    l.caja(Cabeza, r::PELO, [-1, y(ho - 3), -3], [1, y(cab0 + 2), -2]);
                    if a == Lavandera {
                        manga(&mut l, r::PIEL, ho - largo_b, ho - 5);
                        l.caja(Fijo, r::CLARO, [-2, y(3), 2], [2, y(ca + 1), 3]);
                        // Pañuelo y el atado de ropa sobre la cabeza.
                        l.caja(Cabeza, r::ACENTO, [-2, y(cab1 - 2), -2], [2, y(cab1), 2]);
                        l.centrada(Cabeza, r::CLARO, 3, [y(cab1), y(cab1 + 3)], [-3, 3]);
                    }
                }
                _ => {
                    // Señorita: rodete y sombrero de ala.
                    l.caja(Cabeza, r::PELO, [-1, y(cab1 - 2), -3], [1, y(cab1), -2]);
                    sombrero_ala(&mut l, 4, 1);
                    l.caja(Cabeza, r::ACENTO, [-2, y(cab1 + 1), -2], [2, y(cab1 + 2), 2]);
                }
            }
        }
        Nino => {
            pierna(&mut l, r::PIEL, 1, 3);
        }
        Nina => {
            l.centrada(Fijo, r::CALZON, t + 1, [y(2), y(ca + 1)], [-2, 2]);
            l.caja(Cabeza, r::PELO, [-1, y(ho - 2), -3], [1, y(cab0 + 2), -2]);
        }
        Levita => {
            l.centrada(Fijo, r::CAPA, t, [y(ca), y(ho)], [-2, 2]);
            l.centrada(Fijo, r::CAPA, t, [y(6), y(ca)], [-2, -1]);
            l.caja(Fijo, r::CAPA, [-t, y(6), -1], [-t + 1, y(ca), 2]);
            l.caja(Fijo, r::CAPA, [t - 1, y(6), -1], [t, y(ca), 2]);
            l.caja(Fijo, r::CLARO, [-1, y(ho - 3), 1], [1, y(ho), 2]);
            l.caja(Fijo, r::ACENTO, [-1, y(ho - 2), 1], [1, y(ho - 1), 2]);
            manga(&mut l, r::CAPA, ho - largo_b + 2, ho);
            // Galera.
            l.centrada(Cabeza, r::SOMBRERO, 3, [y(cab1), y(cab1 + 1)], [-3, 3]);
            l.centrada(Cabeza, r::SOMBRERO, 2, [y(cab1 + 1), y(cab1 + 5)], [-2, 2]);
        }
        Obrero => {
            l.centrada(Fijo, r::CAPA, t, [y(ca - 2), y(ho)], [-2, 2]);
            manga(&mut l, r::CAPA, ho - largo_b + 2, ho);
            // Gorra con visera.
            l.caja(Cabeza, r::SOMBRERO, [-2, y(cab1 - 1), -2], [2, y(cab1 + 1), 2]);
            l.caja(Cabeza, r::SOMBRERO, [-2, y(cab1 - 1), 2], [2, y(cab1), 4]);
        }
        Traje => {
            l.centrada(Fijo, r::CAPA, t, [y(ca - 3), y(ho)], [-2, 2]);
            l.caja(Fijo, r::CLARO, [-1, y(ho - 5), 1], [1, y(ho), 2]);
            l.caja(Fijo, r::ACENTO, [0, y(ho - 5), 1], [1, y(ho), 2]);
            manga(&mut l, r::CAPA, ho - largo_b + 2, ho);
            sombrero_ala(&mut l, 3, 2);
            l.caja(Cabeza, r::ACENTO, [-2, y(cab1 + 1), -2], [2, y(cab1 + 2), 2]);
        }
        MujerModerna => {
            pierna(&mut l, r::PIEL, 1, 6);
            l.centrada(Fijo, r::ROPA, t, [y(6), y(ca + 1)], [-2, 2]);
        }
    }
    if montado {
        // Lo que cuelga bajo el asiento queda dentro del caballo: fuera.
        l.borrar([-4, -20, -6], [4, 0, 6]);
        // Piernas abiertas a los costados del caballo: muslos hacia adelante y canillas abajo.
        let (pa, pc) = if a.nino() { (2, 5) } else { (3, 9) };
        let calzon = if matches!(a, Paisano | Peon) { r::CLARO } else { r::CALZON };
        for s in [-1, 1] {
            let (x0, x1) = if s < 0 { (-4 - pa, -4) } else { (4, 4 + pa) };
            l.caja(Fijo, r::CALZON, [x0, 0, -2], [x1, 3, 5]);
            l.caja(Fijo, calzon, [x0, -pc, 3], [x1, 0, 6]);
            l.caja(Fijo, r::CALZADO, [x0, -pc - 1, 3], [x1, -pc, 7]);
        }
    }
    l.armar(if montado { "persona montada" } else { "persona" })
}

// --- Colores --------------------------------------------------------------------------------

/// Tonos de piel naturales, de claro a oscuro.
pub const PIELES: [[u8; 3]; 6] = [[226, 186, 156], [212, 166, 128], [190, 142, 104], [160, 112, 78], [124, 84, 58], [92, 62, 44]];
const PELOS: [[u8; 3]; 5] = [[34, 26, 22], [52, 38, 28], [78, 56, 38], [110, 82, 52], [44, 34, 30]];
const CANAS: [u8; 3] = [168, 164, 158];

/// Opciones de color de cada prenda de un atuendo (tintes de la época).
struct Colores {
    ropa: &'static [[u8; 3]],
    calzon: &'static [[u8; 3]],
    sombrero: &'static [[u8; 3]],
    capa: &'static [[u8; 3]],
    calzado: &'static [[u8; 3]],
    acento: &'static [[u8; 3]],
}

const BLANCOS: &[[u8; 3]] = &[[222, 214, 196], [206, 198, 178], [228, 222, 210]];
const CRUDOS: &[[u8; 3]] = &[[206, 192, 160], [190, 174, 140], [214, 204, 180]];
const OSCUROS: &[[u8; 3]] = &[[40, 36, 34], [52, 44, 38], [36, 40, 52]];
const PARDOS: &[[u8; 3]] = &[[96, 74, 52], [116, 90, 62], [78, 62, 48]];
const CUEROS: &[[u8; 3]] = &[[150, 112, 76], [128, 92, 60]];
const NEGROS: &[[u8; 3]] = &[[30, 28, 28], [40, 34, 30]];

fn colores(a: Atuendo) -> Colores {
    use Atuendo::*;
    match a {
        Paisano => Colores {
            ropa: BLANCOS,
            // Chiripá: azul, negro, gris o pardo.
            calzon: &[[44, 52, 84], [40, 36, 34], [92, 88, 82], [96, 74, 52]],
            sombrero: &[[40, 34, 30], [84, 66, 48], [30, 28, 28]],
            // Ponchos de lana: vicuña, azul, pardo, colorado oscuro, gris.
            capa: &[[150, 118, 82], [52, 58, 92], [110, 80, 54], [128, 52, 40], [112, 108, 100]],
            calzado: &[[196, 168, 132], [182, 150, 112]],
            acento: &[[160, 48, 38], [214, 196, 160], [40, 36, 34], [70, 84, 120]],
        },
        Peon => Colores { ropa: CRUDOS, calzon: &[[44, 52, 84], [92, 88, 82], [96, 74, 52], [64, 60, 56]], sombrero: OSCUROS, capa: PARDOS, calzado: CUEROS, acento: &[[168, 50, 40], [44, 52, 84], [214, 196, 160]] },
        Artesano => Colores {
            ropa: BLANCOS,
            calzon: &[[84, 66, 48], [52, 46, 42], [60, 64, 84]],
            sombrero: &[[96, 40, 36], [52, 58, 92], [84, 66, 48]],
            capa: &[[110, 80, 54], [62, 80, 66], [96, 50, 44], [70, 66, 60]],
            calzado: NEGROS,
            acento: OSCUROS,
        },
        Pulpero => Colores { ropa: BLANCOS, calzon: &[[60, 54, 50], [84, 66, 48], [52, 58, 80]], sombrero: OSCUROS, capa: &[[52, 46, 42], [96, 50, 44], [62, 80, 66]], calzado: NEGROS, acento: OSCUROS },
        Principal => Colores {
            ropa: &[[196, 170, 120], [210, 200, 180], [150, 60, 50]],
            calzon: &[[40, 36, 34], [60, 50, 44], [52, 58, 92]],
            sombrero: NEGROS,
            capa: &[[38, 46, 84], [92, 36, 40], [42, 64, 50], [36, 34, 34], [96, 74, 52]],
            calzado: NEGROS,
            acento: &[[196, 170, 120], [150, 60, 50], [210, 200, 180]],
        },
        Soldado => Colores { ropa: &[[214, 204, 180]], calzon: &[[44, 52, 92]], sombrero: NEGROS, capa: &[[40, 50, 104]], calzado: NEGROS, acento: &[[176, 40, 36]] },
        Fraile => Colores { ropa: &[[110, 96, 80]], calzon: &[[110, 96, 80]], sombrero: NEGROS, capa: &[[108, 92, 74], [96, 90, 84]], calzado: CUEROS, acento: BLANCOS },
        Marinero => Colores { ropa: BLANCOS, calzon: &[[196, 186, 160], [44, 52, 84], [92, 88, 82]], sombrero: &[[150, 44, 40], [44, 52, 84], [60, 56, 52]], capa: PARDOS, calzado: CUEROS, acento: &[[44, 52, 104], [150, 44, 40]] },
        Senora => Colores {
            ropa: &[[40, 36, 40], [70, 40, 46], [44, 52, 80], [60, 72, 58], [96, 74, 60]],
            calzon: &[[40, 36, 40], [70, 40, 46], [44, 52, 80], [60, 72, 58], [110, 92, 70]],
            sombrero: NEGROS,
            capa: &[[24, 22, 24], [224, 218, 206], [34, 30, 32]],
            calzado: NEGROS,
            acento: &[[90, 60, 40]],
        },
        DamaPeineton => Colores {
            ropa: &[[150, 60, 64], [60, 84, 120], [210, 196, 170], [70, 100, 76]],
            calzon: &[[150, 60, 64], [60, 84, 120], [210, 196, 170], [70, 100, 76]],
            sombrero: NEGROS,
            capa: &[[24, 22, 24], [224, 218, 206]],
            calzado: NEGROS,
            acento: &[[96, 62, 36]],
        },
        MujerPueblo | Lavandera => Colores {
            ropa: BLANCOS,
            calzon: &[[150, 120, 70], [140, 56, 44], [52, 64, 104], [96, 74, 52], [86, 96, 70]],
            sombrero: OSCUROS,
            capa: &[[110, 50, 44], [64, 70, 104], [140, 116, 80], [84, 80, 74], [52, 46, 42]],
            calzado: CUEROS,
            acento: &[[222, 214, 196], [170, 64, 50], [210, 170, 70]],
        },
        Nino => Colores { ropa: BLANCOS, calzon: &[[84, 66, 48], [52, 58, 92], [92, 88, 82]], sombrero: OSCUROS, capa: PARDOS, calzado: CUEROS, acento: OSCUROS },
        Nina => Colores { ropa: BLANCOS, calzon: &[[150, 120, 70], [140, 56, 44], [52, 64, 104], [86, 96, 70]], sombrero: OSCUROS, capa: PARDOS, calzado: CUEROS, acento: OSCUROS },
        Levita => Colores { ropa: BLANCOS, calzon: &[[60, 56, 52], [96, 90, 80], [40, 36, 34]], sombrero: NEGROS, capa: &[[30, 30, 34], [44, 52, 80], [60, 48, 40]], calzado: NEGROS, acento: NEGROS },
        Obrero => Colores { ropa: CRUDOS, calzon: &[[60, 56, 52], [52, 58, 80], [84, 76, 64]], sombrero: &[[50, 48, 46], [44, 52, 80]], capa: &[[70, 66, 60], [60, 52, 44], [52, 58, 80]], calzado: NEGROS, acento: OSCUROS },
        Traje => Colores { ropa: BLANCOS, calzon: &[[54, 54, 58], [70, 62, 52], [44, 48, 64]], sombrero: &[[60, 56, 52], [40, 36, 34], [110, 96, 76]], capa: &[[54, 54, 58], [70, 62, 52], [44, 48, 64]], calzado: NEGROS, acento: &[[120, 40, 40], [40, 48, 90], [40, 36, 34]] },
        Senorita => Colores {
            ropa: BLANCOS,
            calzon: &[[40, 40, 52], [96, 74, 60], [70, 84, 70], [60, 40, 46]],
            sombrero: &[[214, 196, 150], [40, 36, 34]],
            capa: PARDOS,
            calzado: NEGROS,
            acento: &[[150, 60, 64], [60, 84, 120]],
        },
        MujerModerna => Colores {
            ropa: &[[150, 60, 64], [60, 84, 120], [190, 170, 120], [70, 100, 76], [200, 196, 190]],
            calzon: OSCUROS,
            sombrero: NEGROS,
            capa: PARDOS,
            calzado: NEGROS,
            acento: OSCUROS,
        },
    }
}

fn pick(v: &[[u8; 3]], s: u64) -> [u8; 3] {
    v[(s % v.len() as u64) as usize]
}

/// Mezcla de bits (splitmix64), para sacar varias elecciones independientes de una semilla.
pub fn mezcla(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Paleta de una persona: los colores de su atuendo (elegidos con su semilla), su piel y su
/// pelo (canoso desde los 55).
pub fn paleta(a: Atuendo, semilla: u64, piel: u8, edad: u8) -> Paleta {
    let c = colores(a);
    let s = |k: u64| mezcla(semilla ^ k.wrapping_mul(0x1000_0001));
    let mut p: Paleta = super::modelos::BASE;
    p[r::PIEL as usize] = PIELES[piel as usize % PIELES.len()];
    p[r::PELO as usize] = if edad >= 55 { CANAS } else { pick(&PELOS, s(1)) };
    p[r::ROPA as usize] = pick(c.ropa, s(2));
    p[r::CALZON as usize] = pick(c.calzon, s(3));
    p[r::SOMBRERO as usize] = pick(c.sombrero, s(4));
    p[r::CAPA as usize] = pick(c.capa, s(5));
    p[r::CALZADO as usize] = pick(c.calzado, s(6));
    p[r::ACENTO as usize] = pick(c.acento, s(7));
    p[r::CLARO as usize] = [222, 216, 202];
    // El vestido de una pieza: falda y cuerpo del mismo paño.
    if matches!(a, Atuendo::Senora | Atuendo::DamaPeineton) {
        p[r::ROPA as usize] = p[r::CALZON as usize];
    }
    p
}

// --- Elección del atuendo -------------------------------------------------------------------

/// Lo que importa de una persona para vestirla.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quien {
    pub mujer: bool,
    pub edad: u8,
    pub oficio: Oficio,
    /// Vecino principal, rico o militar de grado: viste de casaca o de señora.
    pub principal: bool,
    pub raiz: Raiz,
    pub semilla: u64,
}

/// Grupos de oficio para las reglas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grupo {
    Nino,
    Campo,
    Oficio,
    Comercio,
    Principal,
    Militar,
    Clero,
    Mar,
    Servicio,
    Obrero,
    Otro,
}

pub fn grupo(q: &Quien) -> Grupo {
    if q.edad < 13 {
        return Grupo::Nino;
    }
    if q.principal && !matches!(q.oficio, Oficio::Clerigo | Oficio::Soldado) {
        return Grupo::Principal;
    }
    match q.oficio {
        Oficio::Peon | Oficio::Labrador | Oficio::Hacendado => Grupo::Campo,
        Oficio::Artesano => Grupo::Oficio,
        Oficio::Pulpero | Oficio::Comerciante => Grupo::Comercio,
        Oficio::Marinero => Grupo::Mar,
        Oficio::Soldado => Grupo::Militar,
        Oficio::Servicio => Grupo::Servicio,
        Oficio::Clerigo => Grupo::Clero,
        Oficio::Funcionario | Oficio::Maestro | Oficio::Profesional | Oficio::Empleado => Grupo::Principal,
        Oficio::Obrero => Grupo::Obrero,
        Oficio::Ninguno => Grupo::Otro,
    }
}

/// Regla: en esos años, para ese sexo y grupo, los atuendos posibles con su peso.
pub struct Regla {
    pub desde: i32,
    pub hasta: i32,
    pub mujer: bool,
    pub grupos: &'static [Grupo],
    pub opciones: &'static [(Atuendo, u8)],
}

use Atuendo as A;
use Grupo as G;

/// Reglas de vestir por época (la primera que corresponde gana; la última de cada época y
/// sexo es la de todos los demás).
pub const REGLAS: &[Regla] = &[
    // Niños, en todas las épocas.
    Regla { desde: 0, hasta: 9999, mujer: false, grupos: &[G::Nino], opciones: &[(A::Nino, 1)] },
    Regla { desde: 0, hasta: 9999, mujer: true, grupos: &[G::Nino], opciones: &[(A::Nina, 1)] },
    // Colonia (hasta 1814).
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Campo], opciones: &[(A::Paisano, 3), (A::Peon, 2)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Oficio, G::Obrero], opciones: &[(A::Artesano, 4), (A::Peon, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Comercio], opciones: &[(A::Pulpero, 3), (A::Artesano, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Principal], opciones: &[(A::Principal, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Militar], opciones: &[(A::Soldado, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Clero], opciones: &[(A::Fraile, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Mar], opciones: &[(A::Marinero, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[G::Servicio], opciones: &[(A::Peon, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: false, grupos: &[], opciones: &[(A::Paisano, 2), (A::Artesano, 1), (A::Peon, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: true, grupos: &[G::Principal], opciones: &[(A::Senora, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: true, grupos: &[G::Servicio], opciones: &[(A::Lavandera, 2), (A::MujerPueblo, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: true, grupos: &[G::Clero], opciones: &[(A::Senora, 1)] },
    Regla { desde: 0, hasta: 1814, mujer: true, grupos: &[], opciones: &[(A::MujerPueblo, 3), (A::Senora, 1)] },
    // Patria Vieja y siglo XIX (1815–1879).
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[G::Campo], opciones: &[(A::Paisano, 4), (A::Peon, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[G::Principal], opciones: &[(A::Levita, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[G::Comercio], opciones: &[(A::Pulpero, 2), (A::Levita, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[G::Oficio, G::Obrero, G::Servicio], opciones: &[(A::Artesano, 3), (A::Peon, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[G::Militar], opciones: &[(A::Soldado, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[G::Clero], opciones: &[(A::Fraile, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[G::Mar], opciones: &[(A::Marinero, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: false, grupos: &[], opciones: &[(A::Paisano, 2), (A::Artesano, 1), (A::Levita, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: true, grupos: &[G::Principal], opciones: &[(A::DamaPeineton, 2), (A::Senora, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: true, grupos: &[G::Servicio], opciones: &[(A::Lavandera, 2), (A::MujerPueblo, 1)] },
    Regla { desde: 1815, hasta: 1879, mujer: true, grupos: &[], opciones: &[(A::MujerPueblo, 3), (A::Senora, 1)] },
    // Modernización e inmigración (1880–1939).
    Regla { desde: 1880, hasta: 1939, mujer: false, grupos: &[G::Campo], opciones: &[(A::Paisano, 2), (A::Obrero, 1)] },
    Regla { desde: 1880, hasta: 1939, mujer: false, grupos: &[G::Principal, G::Comercio], opciones: &[(A::Traje, 1)] },
    Regla { desde: 1880, hasta: 1939, mujer: false, grupos: &[G::Clero], opciones: &[(A::Fraile, 1)] },
    Regla { desde: 1880, hasta: 1939, mujer: false, grupos: &[G::Militar], opciones: &[(A::Soldado, 1)] },
    Regla { desde: 1880, hasta: 1939, mujer: false, grupos: &[G::Mar], opciones: &[(A::Marinero, 1)] },
    Regla { desde: 1880, hasta: 1939, mujer: false, grupos: &[], opciones: &[(A::Obrero, 3), (A::Traje, 1)] },
    Regla { desde: 1880, hasta: 1939, mujer: true, grupos: &[G::Servicio], opciones: &[(A::Lavandera, 1), (A::Senorita, 1)] },
    Regla { desde: 1880, hasta: 1939, mujer: true, grupos: &[], opciones: &[(A::Senorita, 1)] },
    // Desde 1940.
    Regla { desde: 1940, hasta: 9999, mujer: false, grupos: &[G::Principal, G::Comercio], opciones: &[(A::Traje, 1)] },
    Regla { desde: 1940, hasta: 9999, mujer: false, grupos: &[], opciones: &[(A::Obrero, 2), (A::Traje, 1)] },
    Regla { desde: 1940, hasta: 9999, mujer: true, grupos: &[], opciones: &[(A::MujerModerna, 1)] },
];

/// Atuendo de una persona en un año (determinista: misma persona y año, mismo atuendo).
pub fn atuendo(q: &Quien, anio: i32) -> Atuendo {
    let g = grupo(q);
    let regla = REGLAS
        .iter()
        .find(|r| r.mujer == q.mujer && anio >= r.desde && anio <= r.hasta && (r.grupos.is_empty() || r.grupos.contains(&g)))
        .expect("hay una regla general para cada época y sexo");
    let total: u32 = regla.opciones.iter().map(|o| o.1 as u32).sum();
    let mut x = (mezcla(q.semilla ^ 0xA7) % total as u64) as u32;
    for &(a, w) in regla.opciones {
        if x < w as u32 {
            return a;
        }
        x -= w as u32;
    }
    regla.opciones[0].0
}

/// Tono de piel según la raíz familiar (con variación propia de cada uno).
pub fn piel(raiz: Raiz, semilla: u64) -> u8 {
    let (a, n) = match raiz {
        Raiz::Europea => (0, 3),
        Raiz::Mixta => (1, 4),
        Raiz::Indigena => (2, 3),
        Raiz::Africana => (3, 3),
    };
    (a + mezcla(semilla ^ 0x51) % n) as u8
}

/// Atuendo y paleta de una persona en un año.
pub fn vestir(q: &Quien, anio: i32) -> (Atuendo, Paleta) {
    let a = atuendo(q, anio);
    (a, paleta(a, q.semilla, piel(q.raiz, q.semilla), q.edad))
}
