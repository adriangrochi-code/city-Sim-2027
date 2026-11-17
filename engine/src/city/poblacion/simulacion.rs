//! Una ciudad sintética para probar y medir la población sin el resto del motor: agrega
//! casas, comercios y talleres a medida que la gente los necesita, aplica las mejoras que
//! pagan los dueños y trae inmigrantes a un ritmo histórico (≈2 % anual más un goteo). La
//! usan las pruebas de calibración y el banco de pruebas de 100 años.

use super::*;
use crate::city::economy::Ordinance;
use crate::city::lots::{CAP, JOBS_C, JOBS_I};

pub struct CiudadSintetica {
    pub p: Poblacion,
    pub lotes: Vec<LoteInfo>,
    pub mes: i32,
    pub ordenanzas: Vec<Ordinance>,
    /// Personas que llegan por año por cada habitante (inmigración).
    pub inmigracion: f32,
    pub precio_pan: f32,
    /// Una epidemia de un mes cada tantos años (crisis de mortalidad; 0 = nunca).
    pub epidemia_cada: i32,
    /// Escuela, salud, insalubridad y delito iguales en todos los solares (para probar el
    /// modelo de servicios; `None` = sin edificios, como antes).
    pub servicios: Option<ServiciosLote>,
}

impl CiudadSintetica {
    pub fn nueva(semilla: u64) -> CiudadSintetica {
        let mut p = Poblacion::nueva(semilla);
        p.fundar(2);
        CiudadSintetica { p, lotes: Vec::new(), mes: 2, ordenanzas: Vec::new(), inmigracion: 0.02, precio_pan: 1.0, epidemia_cada: 7, servicios: None }
    }

    fn agregar(&mut self, tipo: ZoneKind, nivel: u8) {
        let id = self.lotes.len() as u32;
        let distancia = 60.0 + (id as f32).sqrt() * 40.0;
        let (capacidad, empleos) = match tipo {
            ZoneKind::Residencial => (CAP[nivel as usize], 0),
            ZoneKind::Comercial => (0, JOBS_C[nivel as usize]),
            ZoneKind::Industrial => (0, JOBS_I[nivel as usize]),
        };
        self.lotes.push(LoteInfo { id, tipo: Some(tipo), nivel, capacidad, empleos, valor: (-distancia / 650.0).exp(), distancia });
    }

    pub fn entorno(&self, llegadas: u32) -> Entorno {
        let año = año_de(self.mes) - 1726;
        let epidemia = self.epidemia_cada > 0 && año > 0 && año % self.epidemia_cada == 0 && self.mes % 12 == 6;
        Entorno {
            mes: self.mes,
            lotes: self.lotes.clone(),
            precio_pan: self.precio_pan,
            precio_carne: 1.0,
            agua: 0.6,
            epidemia,
            sequia: false,
            llegadas,
            contribucion: 6,
            ordenanzas: self.ordenanzas.clone(),
            iglesia: true,
            servicios: self.servicios.map_or(Vec::new(), |s| vec![s; self.lotes.len()]),
            con_servicios: self.servicios.is_some(),
        }
    }

    /// Un mes: la ciudad crece lo que la gente pide y la población avanza.
    pub fn paso(&mut self) -> InformeMes {
        self.mes += 1;
        let hab = self.p.habitantes() as f32;
        // Casas: tantas como pida la gente sin casa (como el `pool` de la ciudad).
        let mut falta = self.p.demanda_vivienda(&self.entorno(0));
        while falta > 0 {
            self.agregar(ZoneKind::Residencial, 1);
            falta -= CAP[1];
        }
        // Empleos: comercios y talleres en proporción a la población.
        let (jc, ji): (i32, i32) = self.lotes.iter().fold((0, 0), |a, l| match l.tipo {
            Some(ZoneKind::Comercial) => (a.0 + l.empleos, a.1),
            Some(ZoneKind::Industrial) => (a.0, a.1 + l.empleos),
            _ => a,
        });
        if (jc as f32) < hab * 0.06 {
            self.agregar(ZoneKind::Comercial, 2);
        }
        if (ji as f32) < hab * 0.12 {
            self.agregar(ZoneKind::Industrial, 2);
        }
        let llegadas = (hab * self.inmigracion / 12.0 + 1.0) as u32;
        let ent = self.entorno(llegadas);
        let inf = self.p.mes(&ent);
        for &(l, nivel) in &inf.mejoras {
            let lt = &mut self.lotes[l as usize];
            if lt.nivel + 1 == nivel {
                lt.nivel = nivel;
                lt.capacidad = CAP[nivel as usize];
                self.p.mejora_hecha(l, nivel);
            }
        }
        if !inf.mejoras.is_empty() {
            self.p.stats = self.p.estadisticas();
        }
        inf
    }
}
