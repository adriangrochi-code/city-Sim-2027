//! [lod] Calidad de una sola perilla, recomendación por hardware y auto-ajuste por cuello de
//! botella (técnicas del mod LOD de Minecraft: `config/QualityPreset`,
//! `QualityPreset.recomendarPorHardware` y `core/BalanceadorCpuGpu` con la corrección del vsync
//! de la 0.26.38; ver `docs/sistemas/tecnicas-minecraft-lod.md` §2.7, §2.8 y §2.4).
//!
//! **Presets** ([`Calidad`]): fijan el techo de cada perilla (niveles, lado del anillo, sombras,
//! hilos de generación, subida por cuadro) y el FPS objetivo. `Personalizado` deja lo que el
//! jugador eligió a mano (y es lo que reciben las opciones guardadas antes de los presets).
//!
//! **Recomendación**: a diferencia del mod, el motor sabe qué adaptador hay (`wgpu::DeviceType`):
//! render por CPU → Mínimo, integrada → Bajo (Medio con 8 núcleos lógicos o más), dedicada →
//! Alto (Ultra con 6 núcleos y 12 GB de RAM o más).
//!
//! **Auto-ajuste** ([`Autoajuste`]): una decisión por ciclo de ~1 s, una perilla por vez y en
//! orden según qué está al límite:
//! - GPU al límite: sombras ↓ → anillo ↓ → niveles ↓.
//! - CPU al límite (el hilo principal ocupa casi todo el cuadro): hilos de generación ↓ →
//!   subida por cuadro ↓ → anillo ↓ → niveles ↓.
//! - Tirones con buen promedio: hilos de generación ↓.
//! - Con margen (2 ciclos seguidos debajo del 80 % del objetivo): se recupera en orden inverso
//!   (niveles, anillo, hilos, subida, sombras), de a un paso.
//!
//! Con vsync (la lección de la 0.26.38 del mod): el objetivo nunca es menor que el intervalo del
//! refresco, y con el cuadro pegado al tope el objetivo cuenta como cumplido (no se baja nada);
//! el margen para recuperar sale de la **carga** (GPU medida o trabajo del hilo principal, sin
//! las esperas), de a un paso, esperando 8 ciclos para ver si el cuadro sigue en el tope; si lo
//! pierde, no se recupera por carga durante 30 ciclos, el doble con cada falla seguida (hasta
//! 240). Los cuadros que no se dibujaron (la ventana no redibuja si nada cambió) y los del menú
//! no cuentan.

use serde::{Deserialize, Serialize};

// --- Presets ----------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calidad {
    Minimo,
    Bajo,
    Medio,
    Alto,
    Ultra,
    Personalizado,
}

pub const CALIDADES: [Calidad; 5] = [Calidad::Minimo, Calidad::Bajo, Calidad::Medio, Calidad::Alto, Calidad::Ultra];

/// Sombras posibles, de menos a más: (cascadas, resolución).
pub const SOMBRAS: [(u32, u32); 5] = [(2, 1024), (2, 2048), (3, 2048), (4, 2048), (4, 4096)];
/// Presupuestos de subida por cuadro posibles (ms).
pub const SUBIDAS: [f32; 6] = [0.5, 1.0, 1.5, 2.0, 3.0, 4.0];

/// Valores de un preset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub niveles: u32,
    pub anillo: i32,
    pub sombras: (u32, u32),
    pub msaa: u32,
    /// Hilos de generación: cuántos menos que los núcleos lógicos (None = 1 solo).
    pub hilos_menos: Option<usize>,
    pub subida_ms: f32,
    pub fps_objetivo: u32,
    pub cache_mb: u32,
    /// [render] Fundido entre niveles, SSAO lejano y escala de render (%, 100 = sin escalado).
    pub fundido: bool,
    pub ssao: bool,
    pub escala: u32,
}

impl Calidad {
    pub fn nombre(self) -> &'static str {
        match self {
            Calidad::Minimo => "Mínimo",
            Calidad::Bajo => "Bajo",
            Calidad::Medio => "Medio",
            Calidad::Alto => "Alto",
            Calidad::Ultra => "Ultra",
            Calidad::Personalizado => "Personalizado",
        }
    }

    /// La tabla de los presets (a ajustar con medidas en placas reales: acá solo hay lavapipe).
    pub fn preset(self) -> Option<Preset> {
        let p = |niveles, anillo, sombras, msaa, hilos_menos, subida_ms, fps_objetivo, cache_mb, (fundido, ssao, escala)| Some(Preset { niveles, anillo, sombras, msaa, hilos_menos, subida_ms, fps_objetivo, cache_mb, fundido, ssao, escala });
        // [render] Fundido desde Medio, SSAO lejano en Alto y Ultra (técnicas del mod LOD §2.5,
        // §2.13); el escalado al 75 % en Mínimo (la prueba "apagar solo si pierde" lo apaga si no
        // gana, §2.16).
        match self {
            Calidad::Minimo => p(9, 16, SOMBRAS[0], 1, None, 1.0, 30, 32, (false, false, 75)),
            Calidad::Bajo => p(9, 20, SOMBRAS[1], 1, Some(2), 1.5, 30, 64, (false, false, 100)),
            Calidad::Medio => p(10, 24, SOMBRAS[2], 4, Some(1), 2.0, 45, 128, (true, false, 100)),
            Calidad::Alto => p(11, 24, SOMBRAS[3], 4, Some(1), 3.0, 60, 192, (true, true, 100)),
            Calidad::Ultra => p(11, 32, SOMBRAS[4], 4, Some(1), 4.0, 60, 256, (true, true, 100)),
            Calidad::Personalizado => None,
        }
    }

    pub fn from_name(s: &str) -> Option<Calidad> {
        let s = s.to_lowercase();
        CALIDADES.into_iter().chain([Calidad::Personalizado]).find(|c| {
            let n = c.nombre().to_lowercase();
            n == s || n.replace('í', "i") == s
        })
    }
}

impl Preset {
    /// Hilos de generación con `nucleos` núcleos lógicos.
    pub fn hilos(&self, nucleos: usize) -> usize {
        self.hilos_menos.map_or(1, |m| nucleos.saturating_sub(m).max(1))
    }
}

// --- Recomendación por hardware ---------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adaptador {
    Cpu,
    Integrada,
    Dedicada,
    Otro,
}

#[derive(Clone, Copy, Debug)]
pub struct Hardware {
    pub adaptador: Adaptador,
    pub nucleos: usize,
    /// RAM total en GB (None si no se sabe).
    pub ram_gb: Option<f32>,
}

impl Adaptador {
    pub fn de_wgpu(t: wgpu::DeviceType) -> Adaptador {
        match t {
            wgpu::DeviceType::Cpu => Adaptador::Cpu,
            wgpu::DeviceType::IntegratedGpu => Adaptador::Integrada,
            wgpu::DeviceType::DiscreteGpu => Adaptador::Dedicada,
            _ => Adaptador::Otro,
        }
    }
}

/// RAM total del sistema (Linux: `/proc/meminfo`; en otros sistemas, None).
pub fn ram_total_gb() -> Option<f32> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let l = s.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: f32 = l.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0 / 1024.0)
}

pub fn nucleos() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

pub fn recomendar(h: &Hardware) -> Calidad {
    match h.adaptador {
        Adaptador::Cpu => Calidad::Minimo,
        Adaptador::Integrada | Adaptador::Otro => {
            if h.nucleos >= 8 && h.ram_gb.is_none_or(|r| r >= 8.0) {
                Calidad::Medio
            } else {
                Calidad::Bajo
            }
        }
        Adaptador::Dedicada => {
            if h.nucleos >= 6 && h.ram_gb.is_some_and(|r| r >= 12.0) {
                Calidad::Ultra
            } else {
                Calidad::Alto
            }
        }
    }
}

// --- Perillas ---------------------------------------------------------------------------------

/// Estado de las perillas, como escalones (índices en sus tablas) entre un piso y un techo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perillas {
    /// Índice en [`SOMBRAS`].
    pub sombras: usize,
    /// Índice en [`crate::lod::RINGS`].
    pub anillo: usize,
    pub niveles: u32,
    pub hilos: usize,
    /// Índice en [`SUBIDAS`].
    pub subida: usize,
    /// [render] SSAO lejano (0 apagado, 1 prendido) e índice en [`ESCALAS_PASOS`].
    pub ssao: usize,
    pub escala: usize,
}

/// [render] Escalas de render de menos a más (índices de la perilla `Escala`).
pub const ESCALAS_PASOS: [u32; 4] = [67, 75, 85, 100];

/// Cantidad de perillas.
const PERILLAS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perilla {
    Sombras,
    Anillo,
    Niveles,
    Hilos,
    Subida,
    /// [render]
    Ssao,
    Escala,
}

impl Perilla {
    pub fn nombre(self) -> &'static str {
        match self {
            Perilla::Sombras => "sombras",
            Perilla::Anillo => "anillo",
            Perilla::Niveles => "niveles",
            Perilla::Hilos => "hilos de generación",
            Perilla::Subida => "subida por cuadro",
            Perilla::Ssao => "oclusión lejana",
            Perilla::Escala => "escala de render",
        }
    }
}

fn indice_cercano<T: Copy>(tabla: &[T], dist: impl Fn(T) -> f32) -> usize {
    (0..tabla.len()).min_by(|&a, &b| dist(tabla[a]).total_cmp(&dist(tabla[b]))).unwrap_or(0)
}

impl Perillas {
    /// Las perillas que corresponden a unos valores (los de las opciones o un preset).
    pub fn desde(niveles: u32, anillo: i32, sombras: (u32, u32), hilos: usize, subida_ms: f32) -> Perillas {
        Perillas {
            sombras: indice_cercano(&SOMBRAS, |(c, r)| (c as f32 - sombras.0 as f32).abs() * 10.0 + (r as f32 - sombras.1 as f32).abs() / 1024.0),
            anillo: indice_cercano(&crate::lod::RINGS, |r| (r - anillo).abs() as f32),
            niveles,
            hilos: hilos.max(1),
            subida: indice_cercano(&SUBIDAS, |s| (s - subida_ms).abs()),
            ssao: 0,
            escala: ESCALAS_PASOS.len() - 1,
        }
    }

    /// [render] Con SSAO lejano y escala de render (% del lado).
    pub fn con_efectos(mut self, ssao: bool, escala: u32) -> Perillas {
        self.ssao = ssao as usize;
        self.escala = indice_cercano(&ESCALAS_PASOS, |e| (e as f32 - escala as f32).abs());
        self
    }

    /// [render] Escala de render (%).
    pub fn escala(&self) -> u32 {
        ESCALAS_PASOS[self.escala]
    }

    pub fn sombras(&self) -> (u32, u32) {
        SOMBRAS[self.sombras]
    }
    pub fn anillo(&self) -> i32 {
        crate::lod::RINGS[self.anillo]
    }
    pub fn subida_ms(&self) -> f32 {
        SUBIDAS[self.subida]
    }

    fn get(&self, p: Perilla) -> i64 {
        match p {
            Perilla::Sombras => self.sombras as i64,
            Perilla::Anillo => self.anillo as i64,
            Perilla::Niveles => self.niveles as i64,
            Perilla::Hilos => self.hilos as i64,
            Perilla::Subida => self.subida as i64,
            Perilla::Ssao => self.ssao as i64,
            Perilla::Escala => self.escala as i64,
        }
    }

    fn set(&mut self, p: Perilla, v: i64) {
        match p {
            Perilla::Sombras => self.sombras = v as usize,
            Perilla::Anillo => self.anillo = v as usize,
            Perilla::Niveles => self.niveles = v as u32,
            Perilla::Hilos => self.hilos = v as usize,
            Perilla::Subida => self.subida = v as usize,
            Perilla::Ssao => self.ssao = v as usize,
            Perilla::Escala => self.escala = v as usize,
        }
    }
}

// --- Auto-ajuste ------------------------------------------------------------------------------

/// Una medida de un cuadro de la ventana.
#[derive(Clone, Copy, Debug, Default)]
pub struct Muestra {
    /// Intervalo desde el cuadro dibujado anterior (ms), con las esperas del vsync.
    pub cuadro_ms: f64,
    /// Trabajo del hilo principal (ms), sin las esperas de la superficie (vsync).
    pub trabajo_ms: f64,
    /// Tiempo de GPU del pase principal (ms), si el adaptador tiene marcas de tiempo.
    pub gpu_ms: Option<f64>,
    /// Intervalo del tope (vsync o límite de FPS), 0 si no hay.
    pub tope_ms: f64,
    /// Se dibujó (la ventana no redibuja si nada cambió: esos intervalos no son cuadros).
    pub dibujado: bool,
    /// Con el menú abierto no se ajusta (mide el menú, no el juego).
    pub menu: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lado {
    Gpu,
    Cpu,
    /// Sin saber cuál (sin marcas de tiempo y sin evidencia clara): orden genérico.
    Incierto,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cambio {
    pub perilla: Perilla,
    pub sube: bool,
    pub motivo: &'static str,
}

/// Duración de un ciclo de decisión (ms de cuadros medidos).
pub const CICLO_MS: f64 = 1000.0;
/// Cuadro dentro de este margen sobre el tope = en el tope.
pub const TOLERANCIA_TOPE: f64 = 1.10;
/// Ciclos de prueba tras recuperar por carga, y espera tras una falla (se duplica, con tope).
pub const CICLOS_PRUEBA_CARGA: u32 = 8;
pub const ESPERA_FALLA: u32 = 30;
pub const ESPERA_FALLA_MAX: u32 = 240;
/// Ciclos sin tocar nada después de un cambio (las mallas nuevas tardan en llegar).
pub const CICLOS_ASENTAR: u32 = 2;
/// Una perilla que se subió y hubo que bajar algo antes de estos ciclos: la subida falló y esa
/// perilla no se vuelve a subir por [`ESPERA_FALLA`] ciclos (el doble con cada falla seguida,
/// hasta [`ESPERA_FALLA_MAX`]). Sin esto, con el cuadro justo en el borde, una perilla subía y
/// bajaba cada 4 o 5 s (con el SSAO o la escala, rehaciendo las texturas de la escena: un tirón
/// cada vez).
pub const CICLOS_FALLA: u32 = CICLOS_ASENTAR + 4;
/// El SSAO lejano y la escala de render rehacen los destinos de la escena al cambiar: después
/// de moverse no se vuelven a subir por lo menos en estos ciclos.
pub const CICLOS_QUIETA: u32 = 10;

pub struct Autoajuste {
    pub activo: bool,
    pub techo: Perillas,
    pub piso: Perillas,
    pub actual: Perillas,
    pub fps_objetivo: f64,
    muestras: Vec<Muestra>,
    acumulado: f64,
    ciclo: u32,
    margen_seguido: u32,
    asentar: u32,
    prueba_hasta: Option<u32>,
    bloqueo_hasta: u32,
    espera: u32,
    tirones: bool,
    /// [render] Por perilla: ciclo hasta el que no se sube y la espera de la última falla.
    bloqueo_perilla: [(u32, u32); PERILLAS],
    /// Última subida: perilla y ciclo.
    subida: Option<(Perilla, u32)>,
    /// Lo que se vio en el último ciclo (para el diagnóstico).
    pub lado: Option<Lado>,
    pub en_tope: bool,
    pub ultimo: Option<Cambio>,
    pub promedio_ms: f64,
    pub carga_ms: f64,
    pub cambios: u32,
}

impl Autoajuste {
    pub fn new(techo: Perillas, fps_objetivo: f64) -> Autoajuste {
        let piso = Perillas { sombras: 0, anillo: 0, niveles: crate::lod::MAP_LEVELS.min(techo.niveles), hilos: 1, subida: 0, ssao: 0, escala: piso_escala(&techo) };
        Autoajuste {
            activo: true,
            techo,
            piso,
            actual: techo,
            fps_objetivo,
            muestras: Vec::new(),
            acumulado: 0.0,
            ciclo: 0,
            margen_seguido: 0,
            asentar: 0,
            prueba_hasta: None,
            bloqueo_hasta: 0,
            espera: 0,
            tirones: false,
            bloqueo_perilla: [(0, 0); PERILLAS],
            subida: None,
            lado: None,
            en_tope: false,
            ultimo: None,
            promedio_ms: 0.0,
            carga_ms: 0.0,
            cambios: 0,
        }
    }

    /// Cambia el techo (otro preset u opciones): las perillas vuelven a él.
    pub fn set_techo(&mut self, techo: Perillas, fps_objetivo: f64) {
        self.techo = techo;
        self.piso.niveles = crate::lod::MAP_LEVELS.min(techo.niveles);
        self.piso.escala = piso_escala(&techo);
        self.actual = techo;
        self.fps_objetivo = fps_objetivo;
        self.muestras.clear();
        self.acumulado = 0.0;
        self.margen_seguido = 0;
        self.prueba_hasta = None;
        self.bloqueo_perilla = [(0, 0); PERILLAS];
        self.subida = None;
    }

    /// ¿Está todo en el piso? (el motor ya no puede ayudar más en este equipo).
    pub fn en_piso(&self) -> bool {
        self.actual == self.piso
    }

    /// Suma un cuadro; al completar un ciclo decide y devuelve el cambio de perilla, si hay.
    pub fn muestra(&mut self, m: Muestra) -> Option<Cambio> {
        if !self.activo || !m.dibujado || m.menu || !(m.cuadro_ms > 0.0) {
            return None;
        }
        self.acumulado += m.cuadro_ms;
        self.muestras.push(m);
        if self.acumulado < CICLO_MS {
            return None;
        }
        let ms = std::mem::take(&mut self.muestras);
        self.acumulado = 0.0;
        self.ciclo += 1;
        self.decidir(&ms)
    }

    fn decidir(&mut self, ms: &[Muestra]) -> Option<Cambio> {
        let n = ms.len() as f64;
        let prom = ms.iter().map(|m| m.cuadro_ms).sum::<f64>() / n;
        let trabajo = ms.iter().map(|m| m.trabajo_ms).sum::<f64>() / n;
        let gpus: Vec<f64> = ms.iter().filter_map(|m| m.gpu_ms).collect();
        let gpu = (!gpus.is_empty()).then(|| gpus.iter().sum::<f64>() / gpus.len() as f64);
        let tope = ms.iter().map(|m| m.tope_ms).fold(0.0, f64::max);
        let mut orden: Vec<f64> = ms.iter().map(|m| m.cuadro_ms).collect();
        orden.sort_by(f64::total_cmp);
        let peor = orden[((orden.len() - 1) as f64 * 0.98).round() as usize];
        // El objetivo nunca es menor que el intervalo del tope (no se puede ir más rápido).
        let objetivo = (1000.0 / self.fps_objetivo.max(1.0)).max(tope);
        let en_tope = tope > 0.0 && prom <= tope * TOLERANCIA_TOPE;
        let carga = gpu.unwrap_or(0.0).max(trabajo);
        self.promedio_ms = prom;
        self.carga_ms = carga;
        self.en_tope = en_tope;
        // Lado: con marcas de tiempo de GPU, el que ocupa casi todo el cuadro; sin ellas, si el
        // hilo principal no llega a ocupar el cuadro el que espera es él (manda la GPU).
        let lado = match gpu {
            Some(g) if g >= 0.85 * prom && g >= trabajo => Lado::Gpu,
            _ if trabajo >= 0.80 * prom => Lado::Cpu,
            Some(_) => Lado::Incierto,
            None if trabajo < 0.60 * prom => Lado::Gpu,
            None => Lado::Incierto,
        };
        self.lado = Some(lado);
        self.tirones = peor > 2.5 * objetivo && prom <= objetivo;
        if self.asentar > 0 {
            self.asentar -= 1;
            return None;
        }
        // Prueba de una recuperación por carga: si el cuadro perdió el tope, se espera más.
        if let Some(h) = self.prueba_hasta {
            if !en_tope && tope > 0.0 && prom > objetivo * TOLERANCIA_TOPE {
                self.espera = if self.espera == 0 { ESPERA_FALLA } else { (self.espera * 2).min(ESPERA_FALLA_MAX) };
                self.bloqueo_hasta = self.ciclo + self.espera;
                self.prueba_hasta = None;
            } else if self.ciclo >= h {
                self.prueba_hasta = None;
                self.espera = 0;
            }
        }
        // 1) Pasado del objetivo (y no por el tope): bajar.
        if !en_tope && prom > objetivo * 1.05 {
            self.margen_seguido = 0;
            // Si se acaba de subir algo, se deshace eso (y queda bloqueado un rato).
            if let Some(p) = self.subida_fallida() {
                if let Some(c) = self.mover(&[p], false, "subida que no se sostuvo") {
                    return Some(c);
                }
            }
            let ordenes: &[Perilla] = match lado {
                Lado::Gpu => &[Perilla::Ssao, Perilla::Sombras, Perilla::Escala, Perilla::Anillo, Perilla::Niveles],
                Lado::Cpu => &[Perilla::Hilos, Perilla::Subida, Perilla::Anillo, Perilla::Niveles],
                Lado::Incierto => &[Perilla::Ssao, Perilla::Sombras, Perilla::Hilos, Perilla::Anillo, Perilla::Niveles],
            };
            return self.mover(ordenes, false, match lado {
                Lado::Gpu => "GPU al límite",
                Lado::Cpu => "CPU al límite",
                Lado::Incierto => "cuadro lento",
            });
        }
        // 2) Tirones con buen promedio: menos generación a la vez.
        if self.tirones {
            self.margen_seguido = 0;
            let _ = self.subida_fallida();
            return self.mover(&[Perilla::Hilos], false, "tirones");
        }
        // 3) Margen: recuperar de a un paso.
        let recuperar = [Perilla::Niveles, Perilla::Anillo, Perilla::Hilos, Perilla::Subida, Perilla::Escala, Perilla::Sombras, Perilla::Ssao];
        if en_tope {
            // En el tope el cuadro no dice nada del margen: lo dice la carga.
            if carga > 0.0 && carga < 0.8 * tope && self.prueba_hasta.is_none() && self.ciclo >= self.bloqueo_hasta {
                self.margen_seguido += 1;
                if self.margen_seguido >= 2 {
                    self.margen_seguido = 0;
                    let c = self.mover(&recuperar, true, "margen de carga en el tope");
                    if c.is_some() {
                        self.prueba_hasta = Some(self.ciclo + CICLOS_PRUEBA_CARGA);
                    }
                    return c;
                }
            } else {
                self.margen_seguido = 0;
            }
            return None;
        }
        if prom < 0.8 * objetivo && self.ciclo >= self.bloqueo_hasta {
            self.margen_seguido += 1;
            if self.margen_seguido >= 2 {
                self.margen_seguido = 0;
                return self.mover(&recuperar, true, "margen");
            }
        } else {
            self.margen_seguido = 0;
        }
        None
    }

    /// Mueve la primera perilla del orden que pueda moverse un paso (hacia el techo o el piso).
    fn mover(&mut self, orden: &[Perilla], sube: bool, motivo: &'static str) -> Option<Cambio> {
        for &p in orden {
            // Con tirones no se suben los hilos de generación.
            if sube && p == Perilla::Hilos && self.tirones {
                continue;
            }
            // Bloqueada después de una subida fallida o de un cambio caro.
            if sube && self.ciclo < self.bloqueo_perilla[p as usize].0 {
                continue;
            }
            let (v, lim) = (self.actual.get(p), if sube { self.techo.get(p) } else { self.piso.get(p) });
            if (sube && v < lim) || (!sube && v > lim) {
                self.actual.set(p, v + if sube { 1 } else { -1 });
                self.asentar = CICLOS_ASENTAR;
                if sube {
                    self.subida = Some((p, self.ciclo));
                }
                if matches!(p, Perilla::Ssao | Perilla::Escala) {
                    let b = &mut self.bloqueo_perilla[p as usize].0;
                    *b = (*b).max(self.ciclo + CICLOS_QUIETA);
                }
                self.cambios += 1;
                let c = Cambio { perilla: p, sube, motivo };
                self.ultimo = Some(c);
                return Some(c);
            }
        }
        None
    }

    /// Si hay que bajar poco después de una subida, esa subida falló: la perilla queda
    /// bloqueada (el doble que la vez anterior si vuelve a fallar enseguida).
    fn subida_fallida(&mut self) -> Option<Perilla> {
        let (p, c) = self.subida.take()?;
        if self.ciclo > c + CICLOS_FALLA {
            return None;
        }
        let (hasta, espera) = &mut self.bloqueo_perilla[p as usize];
        // Fallas seguidas (dentro de la espera anterior más un margen): el doble.
        *espera = if *espera > 0 && self.ciclo <= *hasta + *espera { (*espera * 2).min(ESPERA_FALLA_MAX) } else { ESPERA_FALLA };
        *hasta = self.ciclo + *espera;
        Some(p)
    }

    /// [render] La prueba del escalado no ganó: la escala queda al 100 % sin tocar las demás
    /// perillas (antes se volvía al techo entero: un salto de calidad y un tirón).
    pub fn apagar_escala(&mut self) {
        let ultimo = ESCALAS_PASOS.len() - 1;
        self.techo.escala = ultimo;
        self.actual.escala = ultimo;
        self.piso.escala = piso_escala(&self.techo);
    }

    /// Una línea para el diagnóstico (contador de FPS y perfil).
    pub fn estado(&self) -> String {
        if !self.activo {
            return "auto-ajuste apagado".into();
        }
        let lado = match self.lado {
            Some(Lado::Gpu) => "manda la GPU",
            Some(Lado::Cpu) => "manda la CPU",
            Some(Lado::Incierto) | None => "midiendo",
        };
        let tope = if self.en_tope { " · en el tope" } else { "" };
        let ult = self.ultimo.map_or(String::new(), |c| format!(" · {} {} ({})", c.perilla.nombre(), if c.sube { "↑" } else { "↓" }, c.motivo));
        let piso = if self.en_piso() { " · en el piso" } else { "" };
        format!("auto-ajuste: {lado}{tope}, carga {:.1} ms de {:.1}{ult}{piso}", self.carga_ms, self.promedio_ms)
    }
}

/// [render] Piso de la escala de render: si el jugador eligió escalar, el auto-ajuste puede bajar
/// hasta el 67 %; si eligió el 100 %, la escala no es una perilla (no se baja la resolución
/// sin que lo pida).
fn piso_escala(techo: &Perillas) -> usize {
    if techo.escala + 1 < ESCALAS_PASOS.len() {
        0
    } else {
        techo.escala
    }
}

// --- [render] Prueba del escalado: "apagar si no gana" ---------------------------------------

/// Prueba del escalado de render (`PruebaEscalado` del mod, 0.26.27; técnicas del mod LOD
/// §2.16): con la escala prendida, se miden [`PruebaEscalado::CUADROS`] cuadros con escala y
/// otros tantos sin, dos veces alternadas, y se compara la mediana del tiempo de GPU (o del
/// cuadro, sin marcas de tiempo). Si el escalado no gana más de [`PruebaEscalado::GANANCIA`] se
/// apaga: en un equipo limitado por la CPU (o render por software) solo empeora la imagen.
#[derive(Clone, Debug, Default)]
pub struct PruebaEscalado {
    con: Vec<f64>,
    sin: Vec<f64>,
    cuadro: usize,
    pub resultado: Option<bool>,
}

impl PruebaEscalado {
    pub const CUADROS: usize = 30;
    pub const GANANCIA: f64 = 0.03;
    /// Cuadros que se descartan al cambiar de escala (destinos nuevos, cascadas).
    const DESCARTE: usize = 3;

    /// ¿Se dibuja este cuadro con la escala? (fases: con, sin, con, sin).
    pub fn con_escala(&self) -> bool {
        self.resultado.unwrap_or((self.cuadro / Self::CUADROS) % 2 == 0)
    }

    /// Suma el tiempo de un cuadro dibujado; al terminar devuelve la decisión (seguir o apagar).
    pub fn muestra(&mut self, ms: f64) -> Option<bool> {
        if self.resultado.is_some() {
            return None;
        }
        if self.cuadro % Self::CUADROS >= Self::DESCARTE && ms > 0.0 {
            if self.con_escala() { &mut self.con } else { &mut self.sin }.push(ms);
        }
        self.cuadro += 1;
        if self.cuadro >= Self::CUADROS * 4 {
            let r = Self::decidir(&self.sin, &self.con);
            self.resultado = Some(r);
            return Some(r);
        }
        None
    }

    /// Sigue si la mediana con escala es más de un 3 % menor que sin escala.
    pub fn decidir(sin: &[f64], con: &[f64]) -> bool {
        let mediana = |v: &[f64]| {
            let mut v = v.to_vec();
            v.sort_by(f64::total_cmp);
            if v.is_empty() { f64::NAN } else { v[v.len() / 2] }
        };
        let (s, c) = (mediana(sin), mediana(con));
        s.is_finite() && c.is_finite() && c < s * (1.0 - Self::GANANCIA)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_prueba_del_escalado_apaga_si_no_gana() {
        // Limitado por la GPU: con escala el cuadro baja un 40 %: sigue.
        let mut p = PruebaEscalado::default();
        let mut r = None;
        while r.is_none() {
            let ms = if p.con_escala() { 12.0 } else { 20.0 };
            r = p.muestra(ms);
        }
        assert_eq!(r, Some(true));
        assert!(p.con_escala());
        // Limitado por la CPU: casi igual (−2 %): se apaga.
        let mut p = PruebaEscalado::default();
        let mut r = None;
        while r.is_none() {
            let ms = if p.con_escala() { 19.6 } else { 20.0 };
            r = p.muestra(ms);
        }
        assert_eq!(r, Some(false));
        assert!(!p.con_escala());
    }

    #[test]
    fn la_escala_solo_baja_si_el_jugador_la_eligio() {
        let t = Perillas::desde(11, 24, (2, 1024), 3, 3.0).con_efectos(true, 100);
        let mut a = Autoajuste::new(t, 60.0);
        a.piso.sombras = 0;
        // GPU al límite: SSAO primero; la escala (100 %) no se toca.
        let gpu = Muestra { cuadro_ms: 40.0, trabajo_ms: 5.0, gpu_ms: Some(39.0), tope_ms: 0.0, dibujado: true, menu: false };
        let mut cambios = Vec::new();
        for _ in 0..60 {
            if let Some(c) = ciclo(&mut a, gpu) {
                cambios.push(c.perilla);
            }
        }
        assert_eq!(cambios.first(), Some(&Perilla::Ssao));
        assert!(!cambios.contains(&Perilla::Escala));
        // Con la escala elegida (85 %), baja hasta el 67 % después de las sombras.
        let t = Perillas::desde(11, 24, (2, 1024), 3, 3.0).con_efectos(false, 85);
        let mut a = Autoajuste::new(t, 60.0);
        let mut cambios = Vec::new();
        for _ in 0..60 {
            if let Some(c) = ciclo(&mut a, gpu) {
                cambios.push(c.perilla);
            }
        }
        let i = cambios.iter().position(|p| *p == Perilla::Escala).expect("la escala baja");
        assert!(cambios[..i].iter().all(|p| *p == Perilla::Sombras));
        assert_eq!(a.actual.escala().min(a.techo.escala()), 67);
    }

    fn techo() -> Perillas {
        Perillas::desde(11, 24, (4, 2048), 3, 3.0)
    }

    /// Un ciclo (1 s) de cuadros iguales.
    fn ciclo(a: &mut Autoajuste, m: Muestra) -> Option<Cambio> {
        let n = (CICLO_MS / m.cuadro_ms).ceil() as usize;
        let mut out = None;
        for _ in 0..n {
            if let Some(c) = a.muestra(m) {
                out = Some(c);
            }
        }
        out
    }

    fn vsync(trabajo: f64, gpu: f64) -> Muestra {
        Muestra { cuadro_ms: 16.8, trabajo_ms: trabajo, gpu_ms: Some(gpu), tope_ms: 1000.0 / 60.0, dibujado: true, menu: false }
    }

    #[test]
    fn con_vsync_a_60_hz_y_objetivo_60_no_baja_nada_si_la_carga_es_baja() {
        let mut a = Autoajuste::new(techo(), 60.0);
        for _ in 0..300 {
            assert_eq!(ciclo(&mut a, vsync(6.0, 9.0)), None, "bajó algo con el cuadro en el tope");
        }
        assert_eq!(a.actual, a.techo);
        // Tampoco con la carga cerca del tope (el cuadro sigue en el tope: el objetivo se cumple).
        for _ in 0..300 {
            ciclo(&mut a, vsync(12.0, 15.5));
        }
        assert_eq!(a.actual, a.techo, "con el cuadro en el tope no se baja");
    }

    #[test]
    fn con_tope_de_fps_nunca_llega_al_piso_y_recupera_por_carga_de_a_un_paso() {
        // Objetivo 75 FPS en un monitor de 60 Hz: imposible de cumplir; sin la corrección del
        // vsync bajaba todo, ciclo a ciclo, hasta el piso.
        let mut t = techo();
        t.niveles = 11;
        let mut a = Autoajuste::new(t, 75.0);
        a.actual = Perillas { sombras: 1, anillo: 0, niveles: 9, hilos: 1, subida: 1, ssao: 0, escala: ESCALAS_PASOS.len() - 1 };
        let mut cambios = Vec::new();
        for k in 0..400 {
            if let Some(c) = ciclo(&mut a, vsync(5.0, 7.0)) {
                assert!(c.sube, "bajó {:?} en el ciclo {k}", c.perilla);
                cambios.push(k);
            }
            assert!(!a.en_piso());
        }
        assert_eq!(a.actual, a.techo, "no recuperó todo con carga baja: {:?}", a.actual);
        // De a un paso, con la espera de prueba entre medio (≤ 1 cambio cada 8 ciclos).
        for w in cambios.windows(2) {
            assert!(w[1] - w[0] >= CICLOS_PRUEBA_CARGA, "dos recuperaciones a {} ciclos", w[1] - w[0]);
        }
    }

    #[test]
    fn si_recuperar_por_carga_hace_perder_el_tope_se_espera_cada_vez_mas() {
        let mut a = Autoajuste::new(techo(), 60.0);
        a.actual.anillo = 0;
        let mut recuperaciones = Vec::new();
        let mut k = 0;
        while k < 600 {
            // Cada vez que sube el anillo, el cuadro pierde el tope (25 ms) por unos ciclos.
            let c = ciclo(&mut a, vsync(5.0, 7.0));
            k += 1;
            if let Some(c) = c {
                if c.sube {
                    recuperaciones.push(k);
                    for _ in 0..3 {
                        ciclo(&mut a, Muestra { cuadro_ms: 25.0, trabajo_ms: 5.0, gpu_ms: Some(24.0), tope_ms: 1000.0 / 60.0, dibujado: true, menu: false });
                        k += 1;
                    }
                }
            }
        }
        assert!(recuperaciones.len() >= 3);
        let gaps: Vec<usize> = recuperaciones.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.windows(2).all(|g| g[1] >= g[0]), "la espera no crece: {gaps:?}");
        assert!(*gaps.last().unwrap() >= ESPERA_FALLA as usize * 2, "{gaps:?}");
    }

    #[test]
    fn con_la_gpu_al_limite_baja_primero_sombras_y_despues_el_anillo() {
        let mut a = Autoajuste::new(techo(), 60.0);
        let lento = Muestra { cuadro_ms: 28.0, trabajo_ms: 4.0, gpu_ms: Some(27.0), tope_ms: 0.0, dibujado: true, menu: false };
        let mut orden = Vec::new();
        for _ in 0..40 {
            if let Some(c) = ciclo(&mut a, lento) {
                assert!(!c.sube);
                orden.push(c.perilla);
            }
        }
        let primero_anillo = orden.iter().position(|p| *p == Perilla::Anillo).unwrap();
        assert!(orden[..primero_anillo].iter().all(|p| *p == Perilla::Sombras) && primero_anillo == 3, "{orden:?}");
        assert!(!orden.contains(&Perilla::Hilos), "con la GPU al límite no se tocan los hilos: {orden:?}");
        // Sin marcas de tiempo: el hilo principal no ocupa el cuadro → también es la GPU.
        let mut b = Autoajuste::new(techo(), 60.0);
        let c = ciclo(&mut b, Muestra { gpu_ms: None, ..lento }).or_else(|| ciclo(&mut b, Muestra { gpu_ms: None, ..lento }));
        assert_eq!(c.map(|c| c.perilla), Some(Perilla::Sombras));
    }

    #[test]
    fn con_la_cpu_al_limite_baja_primero_los_hilos_y_la_subida() {
        let mut a = Autoajuste::new(techo(), 60.0);
        let lento = Muestra { cuadro_ms: 30.0, trabajo_ms: 28.0, gpu_ms: Some(8.0), tope_ms: 0.0, dibujado: true, menu: false };
        let mut orden = Vec::new();
        for _ in 0..30 {
            if let Some(c) = ciclo(&mut a, lento) {
                orden.push(c.perilla);
            }
        }
        assert_eq!(&orden[..3], &[Perilla::Hilos, Perilla::Hilos, Perilla::Subida], "{orden:?}");
        assert!(!orden.contains(&Perilla::Sombras));
    }

    #[test]
    fn ignora_los_cuadros_no_dibujados_y_el_menu() {
        let mut a = Autoajuste::new(techo(), 60.0);
        for _ in 0..100 {
            assert_eq!(ciclo(&mut a, Muestra { cuadro_ms: 100.0, trabajo_ms: 1.0, gpu_ms: None, tope_ms: 0.0, dibujado: false, menu: false }), None);
            assert_eq!(ciclo(&mut a, Muestra { cuadro_ms: 100.0, trabajo_ms: 1.0, gpu_ms: None, tope_ms: 0.0, dibujado: true, menu: true }), None);
        }
        assert_eq!(a.actual, a.techo);
    }

    /// El SSAO lejano en el borde: prendido pasa el objetivo, apagado sobra margen. Antes se
    /// prendía y apagaba cada 4 o 5 s (rehaciendo las texturas de la escena cada vez); ahora la
    /// subida que no se sostiene se deshace y queda bloqueada cada vez más tiempo.
    #[test]
    fn el_ssao_en_el_borde_no_se_prende_y_apaga_en_bucle() {
        let costo = |p: &Perillas| 12.0 + 6.0 * p.ssao as f64;
        let mut a = Autoajuste::new(techo().con_efectos(true, 100), 60.0);
        let mut cambios = Vec::new();
        for k in 0..400 {
            let g = costo(&a.actual);
            if let Some(c) = ciclo(&mut a, Muestra { cuadro_ms: g, trabajo_ms: 3.0, gpu_ms: Some(g), tope_ms: 0.0, dibujado: true, menu: false }) {
                cambios.push((k, c.perilla, c.sube));
            }
        }
        let ssao = cambios.iter().filter(|c| c.1 == Perilla::Ssao).count();
        assert!(ssao <= 10, "{ssao} cambios del SSAO en 400 s: {cambios:?}");
        // Solo se tocó el SSAO (deshacer la subida fallida no baja otra perilla).
        assert!(cambios.iter().all(|c| c.1 == Perilla::Ssao), "{cambios:?}");
    }

    /// La escala de render en el borde tampoco oscila.
    #[test]
    fn la_escala_en_el_borde_no_oscila() {
        // Elegida al 75 %: al 67 % sobra margen, al 75 % pasa el objetivo.
        let costo = |p: &Perillas| if p.escala() < 75 { 12.0 } else { 18.0 };
        let mut t = techo().con_efectos(false, 75);
        t.sombras = 0;
        let mut a = Autoajuste::new(t, 60.0);
        a.piso.sombras = 0;
        let mut n = 0;
        for _ in 0..400 {
            let g = costo(&a.actual);
            if ciclo(&mut a, Muestra { cuadro_ms: g, trabajo_ms: 3.0, gpu_ms: Some(g), tope_ms: 0.0, dibujado: true, menu: false }).is_some_and(|c| c.perilla == Perilla::Escala) {
                n += 1;
            }
        }
        assert!(n <= 10, "{n} cambios de la escala en 400 s");
    }

    /// Un sistema simulado: el cuadro sale del costo de las perillas. En régimen no oscila.
    #[test]
    fn en_regimen_no_oscila() {
        let costo = |p: &Perillas| 6.0 + 2.5 * p.sombras as f64 + 3.0 * p.anillo as f64 + 2.0 * (p.niveles as f64 - 9.0);
        let mut a = Autoajuste::new(Perillas::desde(11, 32, (4, 4096), 3, 4.0), 60.0);
        let mut cambios = Vec::new();
        for k in 0..500 {
            let g = costo(&a.actual);
            if ciclo(&mut a, Muestra { cuadro_ms: g.max(1.0), trabajo_ms: 3.0, gpu_ms: Some(g), tope_ms: 0.0, dibujado: true, menu: false }).is_some() {
                cambios.push(k);
            }
        }
        let tarde = cambios.iter().filter(|&&k| k >= 100).count();
        assert!(tarde <= 400 / 8, "{tarde} cambios en 400 ciclos de régimen");
        assert!(costo(&a.actual) <= 1000.0 / 60.0 * 1.05, "quedó pasado del objetivo: {:?}", a.actual);
    }

    #[test]
    fn presets_y_recomendacion() {
        // Monótonos de Mínimo a Ultra.
        let ps: Vec<Preset> = CALIDADES.iter().map(|c| c.preset().unwrap()).collect();
        for w in ps.windows(2) {
            assert!(w[0].niveles <= w[1].niveles && w[0].anillo <= w[1].anillo && w[0].cache_mb <= w[1].cache_mb && w[0].subida_ms <= w[1].subida_ms);
            assert!(SOMBRAS.iter().position(|s| *s == w[0].sombras) <= SOMBRAS.iter().position(|s| *s == w[1].sombras));
        }
        assert!(Calidad::Personalizado.preset().is_none());
        assert!(ps.iter().all(|p| crate::lod::RINGS.contains(&p.anillo)));
        let hw = |adaptador, nucleos, ram| Hardware { adaptador, nucleos, ram_gb: ram };
        assert_eq!(recomendar(&hw(Adaptador::Cpu, 16, Some(64.0))), Calidad::Minimo);
        assert_eq!(recomendar(&hw(Adaptador::Integrada, 4, Some(12.0))), Calidad::Bajo);
        assert_eq!(recomendar(&hw(Adaptador::Integrada, 8, Some(16.0))), Calidad::Medio);
        assert_eq!(recomendar(&hw(Adaptador::Dedicada, 6, Some(16.0))), Calidad::Ultra);
        assert_eq!(recomendar(&hw(Adaptador::Dedicada, 4, Some(8.0))), Calidad::Alto);
        assert_eq!(Calidad::from_name("minimo"), Some(Calidad::Minimo));
        assert_eq!(Calidad::from_name("Ultra"), Some(Calidad::Ultra));
        assert_eq!(ps[0].hilos(4), 1);
        assert_eq!(ps[2].hilos(4), 3);
        // Las perillas de un preset vuelven a dar sus valores.
        for p in &ps {
            let k = Perillas::desde(p.niveles, p.anillo, p.sombras, p.hilos(4), p.subida_ms);
            assert_eq!((k.sombras(), k.anillo(), k.subida_ms()), (p.sombras, p.anillo, p.subida_ms));
        }
    }
}
