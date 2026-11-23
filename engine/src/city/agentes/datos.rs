//! Datos del tránsito colonial (`assets/datos/transito_colonial.json`): portones, arreos,
//! horarios por oficio, recados, clima, cruces y adelantamiento. Se leen una vez; cada número
//! lleva su estado (ESTIMADO o DOCUMENTADO) y su base en el archivo.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const JSON: &str = include_str!("../../../../assets/datos/transito_colonial.json");

#[derive(Deserialize, Debug, Clone)]
pub struct Datos {
    pub porton: Porton,
    pub contrabando: Contrabando,
    pub tropas: Tropas,
    pub horarios: Horarios,
    pub recados: Recados,
    pub clima: Clima,
    pub cruces: Cruces,
    pub adelantamiento: Adelantamiento,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Porton {
    pub abre: f32,
    pub cierra: f32,
    pub revision_minutos: Revision,
    pub derecho_reales: Derecho,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Revision {
    pub a_pie: f32,
    pub jinete: f32,
    pub carro: f32,
    pub carreta_vacia: f32,
    pub carreta_cargada_min: f32,
    pub carreta_cargada_max: f32,
    pub arreo_por_cabeza: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Derecho {
    pub a_pie: f32,
    pub jinete: f32,
    pub carro: f32,
    pub carreta_vacia: f32,
    pub carreta_cargada: f32,
    pub arreo_por_cabeza: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Contrabando {
    pub fraccion_base: f32,
    pub fraccion_por_real_de_derecho: f32,
    pub fraccion_maxima: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tropas {
    pub por_semana: u32,
    pub cabezas_min: u32,
    pub cabezas_max: u32,
    pub metros_por_cabeza: f32,
    pub velocidad_kmh: f32,
    pub cabezas_por_tropero: u32,
    pub hora_min: f32,
    pub hora_max: f32,
    pub radio_intramuros_m: f32,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Horario {
    pub entrada: f32,
    pub salida: f32,
    pub desvio: f32,
    pub flexible: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Horarios {
    pub oficios: BTreeMap<String, Horario>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Recados {
    pub compra_por_hogar: f32,
    pub recova_por_clase: [f32; 4],
    pub pulperia_tras_trabajo: f32,
    pub pulperia_minutos: [f32; 2],
    pub recova_minutos: [f32; 2],
    pub compra_minutos: [f32; 2],
    pub misa_domingo: f32,
    pub misa_semana_devocion_minima: f32,
    pub misa_semana: f32,
    pub paseo_domingo: f32,
    pub paseo_hora: [f32; 2],
    pub paseo_minutos: [f32; 2],
    pub visita_pariente_domingo: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Clima {
    pub optativos_con_lluvia: f32,
    pub optativos_con_lluvia_prudencia: f32,
    pub barro_dias: u32,
    pub carreta_en_barro: f32,
    pub pie_con_lluvia: f32,
    pub empantanarse: f32,
    pub empantanado_minutos: [f32; 2],
}

#[derive(Deserialize, Debug, Clone)]
pub struct Cruces {
    pub hueco_base_s: f32,
    pub hueco_por_prudencia_s: f32,
    pub baja_por_espera_s: f32,
    pub hueco_minimo_s: f32,
    pub peaton_hueco_base_s: f32,
    pub peaton_hueco_por_prudencia_s: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Adelantamiento {
    pub prudencia_maxima: f32,
    pub libre_base_m: f32,
    pub libre_por_prudencia_m: f32,
}

/// Los datos (se leen la primera vez que se piden).
pub fn datos() -> &'static Datos {
    static D: OnceLock<Datos> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(JSON).expect("assets/datos/transito_colonial.json mal formado"))
}

impl Datos {
    /// Horario habitual de un oficio (el de artesano si no figura).
    pub fn horario(&self, oficio: &str) -> Horario {
        self.horarios.oficios.get(oficio).or_else(|| self.horarios.oficios.get("Artesano")).copied().unwrap_or(Horario { entrada: 6.5, salida: 17.5, desvio: 0.5, flexible: true })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn los_datos_se_leen_y_tienen_sentido() {
        let d = super::datos();
        assert!(d.porton.abre < d.porton.cierra);
        assert!(d.tropas.cabezas_min <= d.tropas.cabezas_max);
        assert!(d.horarios.oficios.len() >= 12);
        for (o, h) in &d.horarios.oficios {
            assert!(h.entrada < h.salida, "{o}");
        }
    }
}
