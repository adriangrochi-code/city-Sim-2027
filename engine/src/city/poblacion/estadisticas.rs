//! Estadísticas de la población: pirámide, orígenes, clases, desigualdad (Gini de la riqueza
//! y del ingreso, parte del 10 % más rico, pobreza y riqueza por origen y por clase) y
//! felicidad por grupo con sus motivos principales. Se recalculan cada mes; las fotos
//! mensuales y anuales quedan para los gráficos.

use super::*;
use serde::{Deserialize, Serialize};

/// Grupos de edad de la pirámide (de 5 años; el último, 80 y más).
pub const GRUPOS_PIRAMIDE: usize = 17;
pub const EDADES: [&str; 4] = ["0 a 14", "15 a 29", "30 a 49", "50 y más"];
pub const BARRIOS: [&str; 5] = ["Centro", "Intramuros", "Afueras", "Sin casa", "Campaña"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Estadisticas {
    pub habitantes: u32,
    pub hogares: u32,
    /// [grupo de edad][hombres, mujeres].
    pub piramide: [[u32; 2]; GRUPOS_PIRAMIDE],
    pub edad_media: f32,
    pub tam_hogar: f32,
    pub hombres_por_100_mujeres: f32,
    pub por_origen: Vec<(Origen, u32)>,
    pub por_clase: [u32; 4],
    pub por_raiz: [u32; 4],
    /// Adultos que saben leer (0 a 1).
    pub lectura: f32,
    pub sin_casa: u32,
    /// Personas que viven en las chacras y estancias de la campaña.
    pub campo: u32,
    // Desigualdad
    pub gini_riqueza: f32,
    pub gini_ingreso: f32,
    /// Parte de la riqueza en manos del 10 % de hogares más ricos.
    pub top10: f32,
    /// Personas en hogares pobres (0 a 1).
    pub pobreza: f32,
    /// (origen del hogar, pobreza, personas).
    pub pobreza_por_origen: Vec<(Origen, f32, u32)>,
    /// Riqueza media de los hogares por origen (pesos).
    pub riqueza_por_origen: Vec<(Origen, f32)>,
    // Felicidad
    pub felicidad: f32,
    /// Adultos con felicidad menor a 0,35 (0 a 1).
    pub descontentos: f32,
    pub fel_edad: [f32; 4],
    pub fel_sexo: [f32; 2],
    pub fel_clase: [f32; 4],
    pub fel_barrio: [f32; 5],
    pub fel_origen: Vec<(Origen, f32, u32)>,
    /// Motivos de queja más comunes (código, personas).
    pub motivos: Vec<(u8, u32)>,
    /// Personas con ascendencia africana y con ascendencia indígena (la raíz se sigue por
    /// padre y madre).
    pub afrodescendientes: u32,
    pub indigenas: u32,
}

/// Una foto para los gráficos (mes o año).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Foto {
    pub mes: i32,
    pub habitantes: u32,
    pub cuenta: Contadores,
    /// Tasas brutas por mil habitantes y por año.
    pub natalidad: f32,
    pub mortalidad: f32,
    pub gini_riqueza: f32,
    pub gini_ingreso: f32,
    pub top10: f32,
    pub pobreza: f32,
    pub felicidad: f32,
    #[serde(default)]
    pub afrodescendientes: u32,
}

impl Foto {
    /// Foto de un período: `c` son los contadores del mes (o del año, si es diciembre y se
    /// llama con los del año).
    pub fn de(mes: i32, s: &Estadisticas, c: &Contadores) -> Foto {
        let hab = s.habitantes.max(1) as f32;
        Foto {
            mes,
            habitantes: s.habitantes,
            cuenta: *c,
            natalidad: c.nacimientos as f32 / hab * 1000.0,
            mortalidad: c.muertes as f32 / hab * 1000.0,
            gini_riqueza: s.gini_riqueza,
            gini_ingreso: s.gini_ingreso,
            top10: s.top10,
            pobreza: s.pobreza,
            felicidad: s.felicidad,
            afrodescendientes: s.afrodescendientes,
        }
    }
}

/// Coeficiente de Gini (0 igualdad, 1 todo en una mano). Los valores negativos (deudas) cuentan
/// como cero.
pub fn gini(valores: &[f32]) -> f32 {
    let mut v: Vec<f32> = valores.iter().map(|x| x.max(0.0)).collect();
    if v.len() < 2 {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len() as f64;
    let total: f64 = v.iter().map(|&x| x as f64).sum();
    if total <= 0.0 {
        return 0.0;
    }
    let mut acc = 0.0f64;
    for (i, &x) in v.iter().enumerate() {
        acc += (2.0 * (i as f64 + 1.0) - n - 1.0) * x as f64;
    }
    (acc / (n * total)) as f32
}

/// Parte del total en manos del 10 % más alto.
pub fn parte_top10(valores: &[f32]) -> f32 {
    let mut v: Vec<f32> = valores.iter().map(|x| x.max(0.0)).collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| b.total_cmp(a));
    let total: f32 = v.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    let k = (v.len() as f32 * 0.1).ceil() as usize;
    v[..k].iter().sum::<f32>() / total
}

/// Ascendencia de cada persona registrada (por id): bit 1 africana, bit 2 indígena. Se sigue
/// por padre y madre (los padres siempre tienen un id menor que sus hijos).
pub const ASC_AFRICANA: u8 = 1;
pub const ASC_INDIGENA: u8 = 2;

fn media(a: (f32, f32)) -> f32 {
    if a.1 > 0.0 { a.0 / a.1 } else { 0.0 }
}

impl Poblacion {
    /// Ascendencia (bits `ASC_*`) de todas las personas registradas, en una pasada.
    pub fn ascendencias(&self) -> Vec<u8> {
        let bits = |r: Raiz| match r {
            Raiz::Africana => ASC_AFRICANA,
            Raiz::Indigena => ASC_INDIGENA,
            _ => 0,
        };
        let mut v = vec![0u8; self.personas.len()];
        for p in &self.personas {
            let mut a = bits(p.origen.raiz()) | bits(p.raiz);
            for x in [p.padre, p.madre].into_iter().flatten() {
                if (x as usize) < v.len() {
                    a |= v[x as usize];
                }
            }
            v[p.id as usize] = a;
        }
        v
    }

    pub fn estadisticas(&self) -> Estadisticas {
        let mes = self.mes;
        let mut s = Estadisticas { habitantes: self.habitantes(), ..Default::default() };
        let mut origen = [0u32; Origen::ALL.len()];
        let (mut edades, mut n, mut h, mut m, mut adultos, mut leen) = (0i64, 0u32, 0u32, 0u32, 0u32, 0u32);
        let mut fel = (0.0f32, 0.0f32);
        let mut desc = 0u32;
        let mut fel_edad = [(0.0f32, 0.0f32); 4];
        let mut fel_sexo = [(0.0f32, 0.0f32); 2];
        let mut fel_clase = [(0.0f32, 0.0f32); 4];
        let mut fel_barrio = [(0.0f32, 0.0f32); 5];
        let mut fel_origen = [(0.0f32, 0.0f32); Origen::ALL.len()];
        let mut motivos = [0u32; 256];
        for p in self.vivos() {
            let e = p.edad(mes);
            let g = (e / 5).min(GRUPOS_PIRAMIDE as i32 - 1) as usize;
            s.piramide[g][p.sexo as usize] += 1;
            edades += e as i64;
            n += 1;
            match p.sexo {
                Sexo::Hombre => h += 1,
                Sexo::Mujer => m += 1,
            }
            origen[p.origen as usize] += 1;
            s.por_raiz[p.raiz as usize] += 1;
            let hg = p.hogar.map(|x| &self.hogares[x as usize]);
            let clase = hg.map_or(Clase::Pobre, |x| x.clase);
            s.por_clase[clase as usize] += 1;
            if e >= 15 {
                adultos += 1;
                if p.educacion > 0 {
                    leen += 1;
                }
            }
            let f = p.felicidad;
            fel.0 += f;
            fel.1 += 1.0;
            let ge = match e {
                0..=14 => 0,
                15..=29 => 1,
                30..=49 => 2,
                _ => 3,
            };
            fel_edad[ge].0 += f;
            fel_edad[ge].1 += 1.0;
            fel_sexo[p.sexo as usize].0 += f;
            fel_sexo[p.sexo as usize].1 += 1.0;
            fel_clase[clase as usize].0 += f;
            fel_clase[clase as usize].1 += 1.0;
            let b = self.barrio_cache(p).unwrap_or(3);
            fel_barrio[b].0 += f;
            fel_barrio[b].1 += 1.0;
            let fo = &mut fel_origen[p.origen as usize];
            fo.0 += f;
            fo.1 += 1.0;
            if e >= 15 {
                if f < 0.35 {
                    desc += 1;
                }
                for &mo in &p.motivos {
                    if mo != felicidad::SIN_MOTIVO {
                        motivos[mo as usize] += 1;
                    }
                }
            }
            match hg.map(|x| x.alojamiento) {
                Some(Alojamiento::SinCasa) => s.sin_casa += 1,
                Some(Alojamiento::Campo) => s.campo += 1,
                _ => {}
            }
        }
        let asc = self.ascendencias();
        for p in self.vivos() {
            let a = asc.get(p.id as usize).copied().unwrap_or(0);
            s.afrodescendientes += (a & ASC_AFRICANA != 0) as u32;
            s.indigenas += (a & ASC_INDIGENA != 0) as u32;
        }
        s.edad_media = if n > 0 { edades as f32 / n as f32 } else { 0.0 };
        s.hombres_por_100_mujeres = if m > 0 { h as f32 / m as f32 * 100.0 } else { 0.0 };
        s.lectura = if adultos > 0 { leen as f32 / adultos as f32 } else { 0.0 };
        let mut po: Vec<(Origen, u32)> = Origen::ALL.iter().map(|&o| (o, origen[o as usize])).filter(|x| x.1 > 0).collect();
        po.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        s.por_origen = po;
        s.felicidad = media(fel);
        s.descontentos = if adultos > 0 { desc as f32 / adultos as f32 } else { 0.0 };
        s.fel_edad = fel_edad.map(media);
        s.fel_sexo = fel_sexo.map(media);
        s.fel_clase = fel_clase.map(media);
        s.fel_barrio = fel_barrio.map(media);
        let mut fo: Vec<(Origen, f32, u32)> = Origen::ALL.iter().map(|&o| (o, media(fel_origen[o as usize]), fel_origen[o as usize].1 as u32)).filter(|x| x.2 > 0).collect();
        fo.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        s.fel_origen = fo;
        let mut mv: Vec<(u8, u32)> = motivos.iter().enumerate().filter(|x| *x.1 > 0).map(|(k, &n)| (k as u8, n)).collect();
        mv.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        mv.truncate(6);
        s.motivos = mv;
        // Desigualdad entre hogares (sin el cuartel ni el campamento).
        let mut riqueza = Vec::new();
        let mut ingreso = Vec::new();
        let mut pob = (0.0f32, 0.0f32);
        let mut por_origen: BTreeMap<Origen, (f32, f32, f32)> = BTreeMap::new();
        let mut tam = (0.0f32, 0.0f32);
        for hg in self.hogares.iter().filter(|x| x.activo && !x.miembros.is_empty() && !Self::colectivo(x)) {
            let personas = hg.miembros.len() as f32;
            s.hogares += 1;
            tam.0 += hg.miembros.len() as f32;
            tam.1 += 1.0;
            let pat = self.patrimonio(hg);
            riqueza.push(pat);
            ingreso.push(hg.ingresos / self.equivalentes(hg));
            let pobre = (hg.clase == Clase::Pobre) as u8 as f32;
            pob.0 += pobre * personas;
            pob.1 += personas;
            let e = por_origen.entry(hg.origen).or_default();
            e.0 += pobre * personas;
            e.1 += personas;
            e.2 += pat;
        }
        s.tam_hogar = media(tam);
        s.gini_riqueza = gini(&riqueza);
        s.gini_ingreso = gini(&ingreso);
        s.top10 = parte_top10(&riqueza);
        s.pobreza = media(pob);
        let hogares_por_origen: BTreeMap<Origen, f32> = self.hogares.iter().filter(|x| x.activo && !x.miembros.is_empty() && !Self::colectivo(x)).fold(BTreeMap::new(), |mut acc, x| {
            *acc.entry(x.origen).or_default() += 1.0;
            acc
        });
        let mut ppo: Vec<(Origen, f32, u32)> = por_origen.iter().map(|(&o, &(p, n, _))| (o, if n > 0.0 { p / n } else { 0.0 }, n as u32)).collect();
        ppo.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        s.pobreza_por_origen = ppo;
        let mut rpo: Vec<(Origen, f32)> = por_origen.iter().map(|(&o, &(_, _, r))| (o, r / hogares_por_origen.get(&o).copied().unwrap_or(1.0).max(1.0))).collect();
        rpo.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        s.riqueza_por_origen = rpo;
        s
    }

    /// Barrio de una persona según la distancia de su casa a la plaza (si se conoce).
    fn barrio_cache(&self, p: &Persona) -> Option<usize> {
        let h = &self.hogares[p.hogar? as usize];
        if !h.vive_en_solar() {
            return Some(match h.alojamiento {
                Alojamiento::Cuartel | Alojamiento::Campamento => 1,
                Alojamiento::Campo => 4,
                _ => 3,
            });
        }
        let d = *self.distancias.get(&h.lote?)?;
        Some(match d {
            d if d < 250.0 => 0,
            d if d < 700.0 => 1,
            _ => 2,
        })
    }
}
