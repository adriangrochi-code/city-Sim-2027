//! Desglose del cuadro por sección (hilo principal): cuánto tarda cada parte del trabajo de un
//! cuadro del juego, para encontrar qué se lleva el tiempo y qué da tirones.
//!
//! Cada parte suma su tiempo con [`sumar`] o [`medir`] en un acumulador del hilo; al terminar el
//! cuadro, [`Muestras::cerrar_cuadro`] lo vacía y lo guarda como una fila. Con las filas se
//! arma la tabla de mediana, p95, p99 y peor por sección ([`Muestras::tabla`]), que escriben el
//! banco del juego (`MOTOR_BANCO_JUEGO=1 motor --demo-ciudad carpeta`) y `medir-tirones`
//! (`CITYSIM_PERFIL=1`, en `tirones.txt`). Medir cuesta dos lecturas del reloj por sección.

use std::cell::Cell;
use std::fmt::Write as _;
use std::time::Instant;

/// Las secciones del cuadro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sec {
    /// `Session::update`: la simulación de la ciudad y de los vecinos, el pincel y las ediciones.
    Simulacion,
    /// Cajas de los vecinos y vehículos en camino (`Agentes::cajas_con`).
    Vecinos,
    /// Escenas de los oficios (corrillos, baldes, sentados, aguadores, serenos).
    Escenas,
    /// Rampas de los escalones de las calles.
    Rampas,
    /// Animales, botes, detalles fijos y de muestra.
    Fauna,
    /// Rejas, faroles de fachada, macetas y llamadores.
    Fachadas,
    /// Subida de las cajas de los agentes a la GPU (`Renderer::set_agents`).
    SubirAgentes,
    /// Luces de los faroles cercanos.
    Luces,
    /// Subida de las mallas del streaming (`upload_ops`).
    Streaming,
    /// Selección bajo el cursor y resaltado.
    Seleccion,
    /// Interfaz (egui, teselado y pintado).
    Interfaz,
    /// CPU del dibujo: plan de las cascadas, tramos, codificación y envío.
    Dibujo,
    /// Parte del dibujo en armar los tramos del fundido entre niveles.
    Fundido,
    /// Espera de la superficie y entrega (vsync o GPU ocupada).
    Espera,
    /// Auto-ajuste, perillas y prueba del escalado.
    Autoajuste,
}

pub const N: usize = 15;

impl Sec {
    pub const TODAS: [Sec; N] = [
        Sec::Simulacion,
        Sec::Vecinos,
        Sec::Escenas,
        Sec::Rampas,
        Sec::Fauna,
        Sec::Fachadas,
        Sec::SubirAgentes,
        Sec::Luces,
        Sec::Streaming,
        Sec::Seleccion,
        Sec::Interfaz,
        Sec::Dibujo,
        Sec::Fundido,
        Sec::Espera,
        Sec::Autoajuste,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Sec::Simulacion => "simulación",
            Sec::Vecinos => "cajas: vecinos",
            Sec::Escenas => "cajas: escenas",
            Sec::Rampas => "cajas: rampas",
            Sec::Fauna => "cajas: fauna y detalles",
            Sec::Fachadas => "cajas: fachadas",
            Sec::SubirAgentes => "subir agentes",
            Sec::Luces => "luces",
            Sec::Streaming => "streaming (subir mallas)",
            Sec::Seleccion => "selección",
            Sec::Interfaz => "interfaz",
            Sec::Dibujo => "dibujo (CPU)",
            Sec::Fundido => "  de eso, fundido",
            Sec::Espera => "espera (vsync/GPU)",
            Sec::Autoajuste => "auto-ajuste",
        }
    }
}

thread_local! {
    static ACUM: Cell<[f64; N]> = const { Cell::new([0.0; N]) };
}

/// Suma a la sección el tiempo desde `t0` (ms).
pub fn sumar(s: Sec, t0: Instant) {
    sumar_ms(s, t0.elapsed().as_secs_f64() * 1000.0);
}

pub fn sumar_ms(s: Sec, ms: f64) {
    ACUM.with(|a| {
        let mut v = a.get();
        v[s as usize] += ms;
        a.set(v);
    });
}

/// Corre `f` y suma su tiempo a la sección.
pub fn medir<R>(s: Sec, f: impl FnOnce() -> R) -> R {
    let t0 = Instant::now();
    let r = f();
    sumar(s, t0);
    r
}

/// Lo acumulado en este hilo desde la última vez (y lo pone en cero).
pub fn tomar() -> [f64; N] {
    ACUM.with(|a| a.replace([0.0; N]))
}

/// Una fila por cuadro: las secciones, el total del cuadro (intervalo entre cuadros) y el GPU.
#[derive(Default)]
pub struct Muestras {
    filas: Vec<([f64; N], f64, Option<f64>)>,
}

impl Muestras {
    /// Cierra el cuadro: guarda lo acumulado con el intervalo del cuadro (ms) y el GPU (ms).
    pub fn cerrar_cuadro(&mut self, cuadro_ms: f64, gpu_ms: Option<f64>) {
        self.filas.push((tomar(), cuadro_ms, gpu_ms));
    }

    /// Las secciones más largas del último cuadro, para la línea de un cuadro lento.
    pub fn mayores_del_ultimo(&self, n: usize) -> String {
        let Some((f, _, _)) = self.filas.last() else { return String::new() };
        let mut v: Vec<(Sec, f64)> = Sec::TODAS.iter().map(|s| (*s, f[*s as usize])).filter(|x| x.0 != Sec::Espera && x.1 >= 0.05).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v.iter().take(n).map(|(s, ms)| format!("{} {ms:.1}", s.nombre().trim())).collect::<Vec<_>>().join(", ")
    }

    pub fn len(&self) -> usize {
        self.filas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.filas.is_empty()
    }

    pub fn vaciar(&mut self) {
        self.filas.clear();
    }

    /// Mediana, p95, p99, peor y media de cada sección y del total. `(nombre, [p50, p95, p99, peor, media])`.
    pub fn resumen(&self) -> Vec<(&'static str, [f64; 5])> {
        let est = |mut v: Vec<f64>| {
            if v.is_empty() {
                return [0.0; 5];
            }
            v.sort_by(f64::total_cmp);
            let p = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
            [p(0.5), p(0.95), p(0.99), v[v.len() - 1], v.iter().sum::<f64>() / v.len() as f64]
        };
        let mut out: Vec<(&'static str, [f64; 5])> = Sec::TODAS.iter().map(|s| (s.nombre(), est(self.filas.iter().map(|f| f.0[*s as usize]).collect()))).collect();
        let trabajo = |f: &([f64; N], f64, Option<f64>)| f.0.iter().enumerate().filter(|(i, _)| *i != Sec::Fundido as usize && *i != Sec::Espera as usize).map(|(_, x)| x).sum::<f64>();
        out.push(("trabajo del hilo principal", est(self.filas.iter().map(trabajo).collect())));
        out.push(("cuadro (entre cuadros)", est(self.filas.iter().map(|f| f.1).collect())));
        let gpu: Vec<f64> = self.filas.iter().filter_map(|f| f.2).collect();
        if !gpu.is_empty() {
            out.push(("GPU (marcas de tiempo)", est(gpu)));
        }
        out
    }

    /// Tabla en texto (Markdown) del resumen.
    pub fn tabla(&self, titulo: &str) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{titulo} · {} cuadros", self.filas.len());
        let _ = writeln!(s, "| Sección | Mediana | p95 | p99 | Peor | Media |");
        let _ = writeln!(s, "|---|---|---|---|---|---|");
        for (n, v) in self.resumen() {
            let _ = writeln!(s, "| {n} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |", v[0], v[1], v[2], v[3], v[4]);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_resumen_toma_las_secciones_de_cada_cuadro() {
        let mut m = Muestras::default();
        for k in 0..100 {
            sumar_ms(Sec::Rampas, k as f64);
            sumar_ms(Sec::Rampas, 1.0);
            sumar_ms(Sec::Espera, 5.0);
            m.cerrar_cuadro(16.0, None);
        }
        let r = m.resumen();
        let rampas = r.iter().find(|x| x.0 == Sec::Rampas.nombre()).unwrap().1;
        assert_eq!(rampas[3], 100.0);
        assert!((rampas[0] - 51.0).abs() <= 1.0);
        // El trabajo no cuenta la espera.
        let trabajo = r.iter().find(|x| x.0 == "trabajo del hilo principal").unwrap().1;
        assert_eq!(trabajo[3], 100.0);
        assert!(tomar().iter().all(|x| *x == 0.0));
    }
}
