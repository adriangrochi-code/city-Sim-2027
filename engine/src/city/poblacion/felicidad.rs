//! Felicidad de cada persona (0 a 1), como en los juegos de ciudad pero con causas que se
//! pueden leer: sale de sus condiciones (comida, casa, agua, salud, trabajo, familia, libertad,
//! cómo vive frente a sus vecinos, la fe y las fiestas, las ordenanzas) y su personalidad
//! decide cuánto pesa cada cosa. Alimenta la opinión de los sectores de la economía (no la
//! reemplaza), la emigración, la natalidad, la productividad y las protestas.

use super::*;
use crate::city::economy::Ordinance;

/// Factores de la felicidad (el orden de los motivos).
pub const FACTORES: [&str; 12] = ["Comida", "Vivienda", "Agua", "Salud", "Trabajo", "Familia", "Libertad", "Comparación con los vecinos", "Fe y fiestas", "Ordenanzas", "Seguridad", "Educación"];

/// Pesos base de cada factor (la personalidad los mueve).
const PESOS: [f32; 12] = [1.6, 1.0, 0.6, 0.8, 0.9, 0.8, 2.5, 0.5, 0.5, 0.4, 0.6, 0.6];

/// Sin motivo de queja.
pub const SIN_MOTIVO: u8 = 255;

/// Motivos que se muestran ("le preocupa el precio del pan"). Código = factor × 4 + variante.
pub fn motivo_texto(codigo: u8) -> &'static str {
    match codigo {
        SIN_MOTIVO => "nada en particular: está conforme",
        0 => "le cuesta llenar la olla",
        1 => "le preocupa el precio del pan",
        2 => "no le alcanza para comer",
        3 => "pasa hambre",
        4 => "vive en un rancho",
        5 => "vive hacinado",
        6 => "no tiene casa",
        7 => "vive en un cuartel o campamento",
        8 => "falta agua en el barrio",
        12 => "está enfermo",
        13 => "está débil por la edad",
        14 => "no hay médico ni botica cerca",
        15 => "el olor, el humo y el ruido de los talleres del barrio",
        16 => "no tiene trabajo",
        17 => "trabaja por su cuenta, sin taller ni mostrador",
        20 => "está de duelo",
        21 => "enviudó",
        22 => "está solo, sin familia",
        24 => "vive sin libertad",
        28 => "ve que sus vecinos viven mejor",
        32 => "extraña la misa y las fiestas",
        36 => "le molestan las ordenanzas del cabildo",
        40 => "teme por su seguridad",
        41 => "perdió su casa en un incendio o una inundación",
        42 => "teme a los ladrones del barrio",
        44 => "no hay escuela para sus hijos",
        45 => "no puede pagar la escuela de sus hijos",
        _ => "—",
    }
}

/// Felicidad desglosada de una persona (para la ficha).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Desglose {
    pub valores: [f32; 12],
    pub pesos: [f32; 12],
    pub total: f32,
    pub motivos: [u8; 2],
}

impl Poblacion {
    /// Felicidad del mes de todas las personas.
    pub(crate) fn felicidad(&mut self, ent: &Entorno) {
        let info = Lotes::de(ent);
        // Ingreso por adulto de cada hogar y medias por barrio y por origen (comparación).
        let n = self.hogares.len();
        let mut ing = vec![0.0f32; n];
        let mut barrio = [(0.0f32, 0.0f32); 5];
        let mut origen = [(0.0f32, 0.0f32); 8];
        for h in self.hogares.iter().filter(|h| h.activo && !h.miembros.is_empty() && !Self::colectivo(h)) {
            let v = h.ingresos / self.equivalentes(h);
            ing[h.id as usize] = v;
            let b = self.barrio_de(h, &info);
            let e = &mut barrio[b as usize];
            e.0 += v;
            e.1 += 1.0;
            let e = &mut origen[h.origen.grupo() as usize];
            e.0 += v;
            e.1 += 1.0;
        }
        let vivos = self.idx.vivos.clone();
        for id in vivos {
            let d = self.desglose_con(id, ent, &info, &ing, &barrio, &origen);
            let p = &mut self.personas[id as usize];
            // La felicidad cambia de a poco (memoria de los meses anteriores).
            p.felicidad += (d.total - p.felicidad) * 0.35;
            p.motivos = d.motivos;
        }
    }

    /// Barrio de un hogar: anillos según la distancia a la plaza (0 centro, 1 intramuros,
    /// 2 afueras, 3 sin casa, 4 la campaña).
    pub(crate) fn barrio_de(&self, h: &Hogar, info: &Lotes) -> i32 {
        match h.lote.and_then(|l| info.get(&l)) {
            Some(i) if h.vive_en_solar() => match i.distancia {
                d if d < 250.0 => 0,
                d if d < 700.0 => 1,
                _ => 2,
            },
            _ if h.alojamiento == Alojamiento::Campo => 4,
            _ => 3,
        }
    }

    /// Desglose de la felicidad de una persona con el entorno dado (para la ficha).
    pub fn desglose(&self, id: PersonaId, ent: &Entorno) -> Desglose {
        let info = Lotes::de(ent);
        let ing: Vec<f32> = self.hogares.iter().map(|h| if h.activo && !h.miembros.is_empty() { h.ingresos / self.equivalentes(h) } else { 0.0 }).collect();
        let mut barrio = [(0.0f32, 0.0f32); 5];
        let mut origen = [(0.0f32, 0.0f32); 8];
        for h in self.hogares.iter().filter(|h| h.activo && !h.miembros.is_empty() && !Self::colectivo(h)) {
            let v = ing[h.id as usize];
            let e = &mut barrio[self.barrio_de(h, &info) as usize];
            e.0 += v;
            e.1 += 1.0;
            let e = &mut origen[h.origen.grupo() as usize];
            e.0 += v;
            e.1 += 1.0;
        }
        self.desglose_con(id, ent, &info, &ing, &barrio, &origen)
    }

    fn desglose_con(&self, id: PersonaId, ent: &Entorno, info: &Lotes, ing: &[f32], barrio: &[(f32, f32); 5], origen: &[(f32, f32); 8]) -> Desglose {
        let mes = ent.mes;
        let p = &self.personas[id as usize];
        let edad = p.edad(mes);
        let h = p.hogar.map(|h| &self.hogares[h as usize]);
        let r = &p.rasgos;
        let mut v = [0.7f32; 12];
        let mut var = [0u8; 12];
        // 0 Comida: lo que gana el hogar frente a lo que cuesta comer.
        if let Some(h) = h {
            if Self::colectivo(h) {
                v[0] = 0.65;
            } else {
                let comida = h.detalle[2].max(0.1);
                let cubre = ((h.ingresos + h.ahorro * 0.1) / comida).min(3.0);
                v[0] = (0.25 + 0.3 * cubre).clamp(0.0, 1.0);
                if h.meses_de_hambre >= 2 {
                    v[0] = 0.05;
                    var[0] = 3;
                } else if cubre < 1.0 {
                    var[0] = 2;
                } else if ent.precio_pan > 1.25 {
                    var[0] = 1;
                }
                v[0] -= ((ent.precio_pan - 1.0).max(0.0) * 0.25).min(0.3) * if h.clase <= Clase::Modesta { 1.0 } else { 0.3 };
            }
        }
        // 1 Vivienda: calidad y hacinamiento.
        if let Some(h) = h {
            match h.alojamiento {
                Alojamiento::SinCasa => {
                    v[1] = 0.12;
                    var[1] = 2;
                }
                Alojamiento::Cuartel | Alojamiento::Campamento => {
                    v[1] = 0.5;
                    var[1] = 3;
                }
                Alojamiento::Campo => v[1] = 0.6,
                Alojamiento::Solar => {
                    let i = h.lote.and_then(|l| info.get(&l));
                    let nivel = i.map_or(1, |i| i.nivel);
                    v[1] = [0.4, 0.45, 0.72, 0.92][nivel.min(3) as usize];
                    if nivel <= 1 {
                        var[1] = 0;
                    }
                    // El olor, el ruido y el humo de la industria cercana.
                    let m = ent.servicios_hogar(Some(h)).molestia;
                    if m > 0.05 {
                        v[1] -= (m * 0.35).min(0.3);
                    }
                    if let Some(i) = i {
                        let hac = self.hacinamiento(i.id, i.capacidad);
                        if hac > 1.0 {
                            v[1] -= ((hac - 1.0) * 0.6).min(0.35);
                            var[1] = 1;
                        }
                    }
                }
            }
        }
        // 2 Agua.
        v[2] = (0.3 + ent.agua * 0.7).clamp(0.0, 1.0);
        // 3 Salud: la propia y, con el modelo de servicios, tener quién lo atienda.
        let sv = ent.servicios_hogar(h);
        v[3] = p.salud;
        if p.salud < 0.5 {
            var[3] = if edad > 60 { 1 } else { 0 };
        }
        if ent.con_servicios {
            v[3] = p.salud * 0.75 + 0.25 * (0.35 + 0.65 * sv.salud);
            if p.salud >= 0.5 && sv.salud < 0.1 {
                var[3] = 2;
            }
            if sv.molestia > 0.35 {
                v[3] -= 0.1 * sv.molestia;
                var[3] = 3;
            }
        }
        // 4 Trabajo.
        v[4] = if !(16..=60).contains(&edad) {
            0.75
        } else if p.oficio == Oficio::Ninguno {
            if p.sexo == Sexo::Mujer { 0.65 } else { 0.25 }
        } else if economia::zona_de_trabajo(p.oficio).is_some() && p.trabajo.is_none() {
            var[4] = 1;
            0.55
        } else {
            0.8
        };
        // 5 Familia.
        v[5] = if p.duelo > 0 {
            var[5] = 0;
            0.2
        } else if edad < 16 {
            if p.padre.is_some_and(|x| self.personas[x as usize].viva()) || p.madre.is_some_and(|x| self.personas[x as usize].viva()) { 0.8 } else { 0.4 }
        } else if !self.sola(p) {
            0.75 + if p.hijos.iter().any(|&c| self.personas[c as usize].viva()) { 0.15 } else { 0.0 }
        } else if p.conyuge.is_some() {
            var[5] = 1;
            0.4
        } else if edad > 30 {
            var[5] = 2;
            0.45
        } else {
            0.65
        };
        // 6 Libertad: la esclavitud pesa como ninguna otra cosa.
        v[6] = match p.condicion {
            Condicion::Esclavizada => 0.03,
            Condicion::Liberta => 0.9,
            Condicion::Libre => 0.8,
        };
        // 7 Comparación con el barrio y con los de su mismo origen.
        if let Some(h) = h.filter(|h| !Self::colectivo(h)) {
            let mio = ing[h.id as usize].max(0.1);
            let media = |x: (f32, f32)| if x.1 > 0.0 { x.0 / x.1 } else { mio };
            let mb = media(barrio[self.barrio_de(h, info) as usize]).max(0.1);
            let mo = media(origen[h.origen.grupo() as usize]).max(0.1);
            let rel = (mio / mb).ln() * 0.6 + (mio / mo).ln() * 0.4;
            v[7] = (0.66 + 0.2 * rel).clamp(0.0, 1.0);
        }
        // 8 Fe y fiestas.
        v[8] = r.devocion() * if ent.iglesia { 0.85 } else { 0.35 } + (1.0 - r.devocion()) * 0.65;
        // 9 Ordenanzas que lo tocan.
        let clase = h.map_or(Clase::Pobre, |h| h.clase);
        let mut o = 0.65f32;
        let vig = |x: Ordinance| ent.vigente(x);
        if vig(Ordinance::PanDeLosPobres) && clase == Clase::Pobre {
            o += 0.2;
        }
        if vig(Ordinance::HospitalCaridad) && clase <= Clase::Modesta {
            o += 0.1;
        }
        if vig(Ordinance::EscuelasPrimerasLetras) && p.hijos.iter().any(|&c| (5..13).contains(&self.personas[c as usize].edad(mes))) {
            o += 0.15;
        }
        if vig(Ordinance::RepartoSolares) && clase <= Clase::Modesta {
            o += 0.1;
        }
        if vig(Ordinance::ContribucionProgresiva) {
            o += match clase {
                Clase::Pobre => 0.1,
                Clase::Principal => -0.3,
                Clase::Acomodada => -0.1,
                _ => 0.0,
            };
        }
        if vig(Ordinance::PrestamosCabildo) && clase <= Clase::Modesta && h.is_some_and(|h| h.deuda > 0.0) {
            o += 0.1;
        }
        if vig(Ordinance::FondoManumision) && p.condicion == Condicion::Esclavizada {
            o += 0.25;
        }
        if vig(Ordinance::FarolesVecinos) {
            o -= 0.08;
        }
        if vig(Ordinance::SinPaja) && h.is_some_and(|h| h.vive_en_solar()) && clase <= Clase::Modesta {
            o -= 0.08;
        }
        v[9] = o.clamp(0.0, 1.0);
        // 10 Seguridad: el barrio (el centro, con serenos y vecinos, es más seguro que las
        // afueras), no tener techo y haber perdido la casa en un desastre.
        if let Some(h) = h {
            v[10] = [0.8, 0.72, 0.55, 0.3, 0.6][self.barrio_de(h, info) as usize];
            if ent.con_servicios && h.vive_en_solar() {
                // El delito del barrio (pobreza, oscuridad, baldíos) y quién lo vigila.
                v[10] = (0.85 - 0.6 * sv.delito + 0.1 * sv.vigilancia).clamp(0.0, 1.0);
                if sv.delito > 0.3 {
                    var[10] = 2;
                }
            }
            if h.desastre.is_some_and(|m| mes - m < 12) {
                v[10] = 0.25;
                var[10] = 1;
            }
        }
        // 11 Educación: para los padres con hijos de 5 a 12 años y para los chicos de esa edad,
        // tener escuela en el barrio (con el modelo de servicios).
        let en_edad = |e: i32| (5..=12).contains(&e);
        let con_hijos = en_edad(edad) || p.hijos.iter().any(|&c| self.personas[c as usize].viva() && en_edad(self.personas[c as usize].edad(mes)));
        let mut pesa_educacion = false;
        if ent.con_servicios && con_hijos && p.libre() {
            pesa_educacion = true;
            let paga = vig(Ordinance::EscuelasPrimerasLetras) || clase >= Clase::Acomodada;
            v[11] = if sv.escuela <= 0.0 { 0.35 } else if paga { 0.55 + 0.4 * sv.escuela } else { 0.5 };
            if sv.escuela <= 0.0 {
                var[11] = 0;
            } else if !paga {
                var[11] = 1;
            }
        }
        // Pesos según la personalidad.
        let mut w = PESOS;
        w[0] *= 0.8 + 0.4 * r.prudencia();
        w[1] *= 0.8 + 0.4 * r.prudencia();
        w[4] *= 0.6 + 0.6 * r.laboriosidad() + 0.2 * r.ambicion();
        w[5] *= 0.6 + 0.8 * r.sociabilidad();
        w[7] *= 0.4 + 1.2 * r.ambicion();
        w[8] *= 0.3 + 1.4 * r.devocion();
        w[10] *= 0.7 + 0.6 * r.prudencia();
        if !pesa_educacion {
            w[11] = 0.0;
        }
        if edad < 14 {
            w[4] = 0.0;
            w[7] *= 0.3;
            w[9] *= 0.3;
        }
        for x in &mut v {
            *x = x.clamp(0.0, 1.0);
        }
        // La media ponderada, más el peso de lo peor: una sola cosa grave (no tener libertad,
        // pasar hambre, no tener casa) no se compensa con lo demás.
        let media = v.iter().zip(&w).map(|(a, b)| a * b).sum::<f32>() / w.iter().sum::<f32>();
        let peor = v.iter().zip(&w).filter(|(_, &b)| b >= 0.5).map(|(a, _)| *a).fold(1.0f32, f32::min);
        let total = 0.65 * media + 0.35 * peor;
        // Motivos: lo que más le pesa (por debajo de lo aceptable).
        let mut peor: Vec<(usize, f32)> = (0..12).map(|i| (i, w[i] * (0.62 - v[i]))).filter(|x| x.1 > 0.0).collect();
        peor.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let cod = |i: usize| (i * 4) as u8 + var[i];
        let motivos = match peor.len() {
            0 => [SIN_MOTIVO, SIN_MOTIVO],
            1 => [cod(peor[0].0), SIN_MOTIVO],
            _ => [cod(peor[0].0), cod(peor[1].0)],
        };
        Desglose { valores: v, pesos: w, total: total.clamp(0.0, 1.0), motivos }
    }

    /// Felicidad media de los adultos de cada sector (vecinos, comerciantes, hacendados),
    /// según el oficio del jefe de su hogar.
    pub fn animo_sectores(&self) -> [f32; 3] {
        let mut s = [(0.0f32, 0.0f32); 3];
        for p in self.vivos() {
            if p.edad(self.mes) < 15 {
                continue;
            }
            let sector = p.hogar.map_or(0, |h| self.personas[self.hogares[h as usize].jefe as usize].oficio.sector());
            s[sector].0 += p.felicidad;
            s[sector].1 += 1.0;
        }
        s.map(|(a, n)| if n > 0.0 { a / n } else { 0.55 })
    }
}
