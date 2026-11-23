//! Portones de la muralla con aduana (`docs/sistemas/ia-y-transito.md` §4.1): cada portón
//! terminado es un **punto de servicio** sobre la calle que cruza la muralla. Los vehículos
//! hacen cola por sentido (la fila del carril es FIFO), el guarda revisa al primero un tiempo
//! que depende de la carga (una carreta de cueros mucho más que un jinete; a pie casi nada),
//! cobra el derecho y lo deja pasar. De noche el portón está cerrado: los que llegan tarde
//! esperan afuera hasta que abre. Los contrabandistas buscan caminos sin portones.
//!
//! Los portones los publica la ciudad (posiciones de los terminados, ver
//! `City::portones_terminados`); acá se ubican sobre la red y se guarda lo que pasó en cada uno
//! (cola, esperas, recaudación, contrabando), que se ve en su ficha y en la capa «cola en
//! portones».

use super::datos::datos;
use super::idm::Tipo;
use crate::city::roads::RoadNetwork;
use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Un portón ubicado sobre la red.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Porton {
    pub clave: u64,
    pub pos: Vec2,
    pub tramo: u32,
    /// Posición sobre el tramo (m desde su extremo `a`).
    pub s: f32,
    /// Sentido del tramo que va hacia adentro (hacia la plaza).
    pub hacia_adentro: u8,
}

/// Lo que pasó en un portón (se guarda con la partida).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StatsPorton {
    pub pasados_hoy: u32,
    pub pasados_ayer: u32,
    pub reales_hoy: f32,
    pub reales_ayer: f32,
    pub reales_total: f64,
    /// Suma de las esperas (s simulados) y cuántas, hoy y ayer.
    pub espera_hoy: f32,
    pub esperas_hoy: u32,
    pub espera_ayer: f32,
    pub esperas_ayer: u32,
    pub cola_max_hoy: u16,
    /// Cola de ahora por sentido: [entrando, saliendo] (se recalcula en cada paso).
    pub cola: [u16; 2],
    /// Vehículos que esperan a que abra (de noche).
    pub cerrado_esperan: u16,
    /// Carretas que evitaron los portones este mes (estimado: se reparte entre todos).
    pub contrabando_mes: f32,
}

impl StatsPorton {
    pub fn nuevo_dia(&mut self) {
        self.pasados_ayer = self.pasados_hoy;
        self.reales_ayer = self.reales_hoy;
        self.espera_ayer = self.espera_hoy;
        self.esperas_ayer = self.esperas_hoy;
        self.pasados_hoy = 0;
        self.reales_hoy = 0.0;
        self.espera_hoy = 0.0;
        self.esperas_hoy = 0;
        self.cola_max_hoy = 0;
    }
    /// Espera media (s simulados) hoy, o ayer si hoy todavía no pasó nadie.
    pub fn espera_media(&self) -> Option<f32> {
        if self.esperas_hoy > 0 {
            Some(self.espera_hoy / self.esperas_hoy as f32)
        } else if self.esperas_ayer > 0 {
            Some(self.espera_ayer / self.esperas_ayer as f32)
        } else {
            None
        }
    }
}

/// Clave estable de un portón: su posición redondeada al metro.
pub fn clave(p: Vec2) -> u64 {
    ((p.x.round() as i32 as u32 as u64) << 32) | (p.y.round() as i32 as u32 as u64)
}

/// ¿Está abierto a esa hora?
pub fn abierto(hora: f32) -> bool {
    let p = &datos().porton;
    let h = hora.rem_euclid(24.0);
    h >= p.abre && h < p.cierra
}

/// Ubica los portones sobre la red: el tramo que pasa a menos de 8 m y la posición sobre él.
pub fn ubicar(net: &RoadNetwork, posiciones: &[Vec2], plaza: Option<Vec2>) -> Vec<Porton> {
    let mut out: Vec<Porton> = Vec::new();
    for &p in posiciones {
        let Some(n) = net.nearest_edge(p, 8.0) else { continue };
        let e = &net.edges[&n.edge];
        let s = e.cum[n.seg] + e.pts[n.seg].distance(n.point);
        // Hacia adentro: el sentido que acerca a la plaza.
        let hacia_adentro = match plaza {
            Some(c) if e.pts[0].distance(c) < e.pts.last().unwrap().distance(c) => 1,
            _ => 0,
        };
        let c = clave(p);
        if !out.iter().any(|q| q.clave == c) {
            out.push(Porton { clave: c, pos: n.point, tramo: n.edge, s, hacia_adentro });
        }
    }
    out.sort_by_key(|g| g.clave);
    out
}

/// Tiempo de revisión (s simulados) y derecho (reales) de un vehículo: `cargada` para las
/// carretas que traen carga; `cabezas` para los arreos; `sorteo` en [0, 1) para el tiempo de
/// las carretas cargadas.
pub fn revision(tipo: Tipo, cargada: bool, cabezas: u16, sorteo: f32) -> (f32, f32) {
    let p = &datos().porton;
    let (r, d) = (&p.revision_minutos, &p.derecho_reales);
    let (min, reales) = match tipo {
        Tipo::APie => (r.a_pie, d.a_pie),
        Tipo::Jinete => (r.jinete, d.jinete),
        Tipo::Carro => (r.carro, d.carro),
        Tipo::Carreta if cargada => (r.carreta_cargada_min + (r.carreta_cargada_max - r.carreta_cargada_min) * sorteo, d.carreta_cargada),
        Tipo::Carreta => (r.carreta_vacia, d.carreta_vacia),
        Tipo::Arreo => (r.arreo_por_cabeza * cabezas as f32, d.arreo_por_cabeza * cabezas as f32),
    };
    (min * 60.0, reales)
}

/// Fracción de carretas que intentan evitar los portones (crece con el derecho).
pub fn fraccion_contrabando() -> f32 {
    let c = &datos().contrabando;
    (c.fraccion_base + c.fraccion_por_real_de_derecho * datos().porton.derecho_reales.carreta_cargada).min(c.fraccion_maxima)
}
