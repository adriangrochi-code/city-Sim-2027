//! Modelos de detalle: objetos chicos que quedan mucho mejor con vóxeles finos que con los de
//! la obra (terreno de 0,25 m, edificios en módulos de 0,5 m). Personas, animales, carretas,
//! faroles, toneles, brocales, botes… se arman con micro-vóxeles de [`VOX`] = 1/16 m.
//!
//! - **Biblioteca** ([`modelos`]): cada modelo se pinta por datos en una grilla 3D chica
//!   ([`Lienzo`]: cajas que se pisan unas a otras, cilindros, piezas giradas) con colores por
//!   *ranura* de paleta; al armarse, la grilla se funde en cajas grandes (fusión voraz por
//!   parte y color), así una persona son ~25 cajas y no ~2000 vóxeles.
//! - **Paletas**: el modelo no tiene colores propios sino ranuras ([`r`]): piel, pelo, ropa,
//!   pelaje, madera…; cada instancia trae su [`Paleta`] (la ropa de cada vecino según su
//!   persona y su época, el pelaje de cada animal: ver [`ropa`] y [`modelos::paleta`]).
//! - **Partes animadas** ([`Parte`]): cabeza, brazos, piernas, patas, cola y ruedas giran
//!   alrededor de su pivote. La pose ([`Pose`]) la calcula [`anim`] cada cuadro (caminar, trote,
//!   pastar, ruedas que giran) y el vertex shader la aplica (`shaders/agentes.wgsl`).
//! - **Dibujo**: una instancia de 48 bytes por caja ([`Caja`]), en el mismo pipeline que los
//!   vecinos; luz, sombra recibida, niebla y curva tonal como el resto, más una sombra de
//!   contacto suave bajo cada modelo ([`sombra`]).
//! - **Detalles fijos** ([`Detalles`]): lista de instancias por zona que cualquier sistema
//!   puede llenar con [`Detalles::poner`]; se dibujan cerca de la cámara.
//!
//! Diseño y medidas en `docs/sistemas/modelos-de-detalle.md` y
//! `docs/sistemas/personajes-y-animales.md`.

pub mod anim;
pub mod modelos;
pub mod ropa;
#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Lado del micro-vóxel de los modelos de detalle (m): 1/16 m = 6,25 cm. Cuatro por vóxel de
/// terreno (0,25 m), dos por micro-vóxel de la flora (0,125 m): todo queda en la misma grilla.
pub const VOX: f32 = 1.0 / 16.0;

/// Ranuras de la paleta de un modelo.
pub mod r {
    pub const PIEL: u8 = 0;
    pub const PELO: u8 = 1;
    /// Camisa, blusa, casaca: la prenda del torso.
    pub const ROPA: u8 = 2;
    /// Calzón, chiripá, pantalón, falda.
    pub const CALZON: u8 = 3;
    pub const SOMBRERO: u8 = 4;
    /// Poncho, mantilla, rebozo, hábito, chaleco: la prenda de encima.
    pub const CAPA: u8 = 5;
    pub const CALZADO: u8 = 6;
    /// Faja, vueltas del uniforme, cinta, cuernos, cresta.
    pub const ACENTO: u8 = 7;
    pub const PELAJE: u8 = 8;
    pub const PELAJE_OSC: u8 = 9;
    pub const CRIN: u8 = 10;
    pub const MADERA: u8 = 11;
    pub const MADERA_OSC: u8 = 12;
    pub const CUERO: u8 = 13;
    pub const HIERRO: u8 = 14;
    /// Blanco: medias, lana, vidrio, cal.
    pub const CLARO: u8 = 15;
    pub const N: usize = 16;
}

/// Colores sRGB de las ranuras de un modelo.
pub type Paleta = [[u8; 3]; r::N];

/// Partes que se mueven (cada una gira alrededor de su pivote).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Parte {
    Fijo = 0,
    /// Cabeza (de los animales, con el cuello).
    Cabeza,
    BrazoI,
    BrazoD,
    PiernaI,
    PiernaD,
    /// Patas: delantera izquierda y derecha, trasera izquierda y derecha.
    PataDI,
    PataDD,
    PataTI,
    PataTD,
    Cola,
    RuedaI,
    RuedaD,
}

pub const N_PARTES: usize = 13;

impl Parte {
    pub const TODAS: [Parte; N_PARTES] =
        [Parte::Fijo, Parte::Cabeza, Parte::BrazoI, Parte::BrazoD, Parte::PiernaI, Parte::PiernaD, Parte::PataDI, Parte::PataDD, Parte::PataTI, Parte::PataTD, Parte::Cola, Parte::RuedaI, Parte::RuedaD];
}

/// Una caja del modelo, en micro-vóxeles: esquina mínima `a`, máxima `b` (exclusiva), ranura
/// de color, parte y un giro fijo propio alrededor del eje x del pivote de su parte (rad; para
/// los rayos y las llantas de las ruedas, la lanza apoyada en el suelo…).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CajaM {
    pub a: [i16; 3],
    pub b: [i16; 3],
    pub ranura: u8,
    pub parte: Parte,
    pub rot: f32,
}

/// Un modelo armado: cajas, pivotes de las partes (micro-vóxeles) y límites.
#[derive(Clone, Debug)]
pub struct Modelo {
    pub nombre: &'static str,
    pub cajas: Vec<CajaM>,
    pub pivotes: [[f32; 3]; N_PARTES],
    pub min: [i16; 3],
    pub max: [i16; 3],
    /// Medio ancho y medio largo de la huella (m), para la sombra de contacto.
    pub huella: [f32; 2],
}

/// Ángulos de cada parte este cuadro: (giro alrededor de y, inclinación alrededor de x), rad.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose(pub [(f32, f32); N_PARTES]);

impl Pose {
    pub fn set(&mut self, p: Parte, giro: f32, inclina: f32) {
        self.0[p as usize] = (giro, inclina);
    }
    pub fn get(&self, p: Parte) -> (f32, f32) {
        self.0[p as usize]
    }
}

/// Una caja para el renderer (48 bytes): pivote (m, mundo), giro alrededor de y; desde el
/// pivote hasta el centro del piso de la caja (m, en el marco del modelo), inclinación
/// alrededor de x; tamaño (m) y color sRGB. `color[3]`: 0 = caja lisa (lejos), otro valor =
/// micro-vóxeles (semilla del matiz de cada vóxel). Con `tam[1] == 0` es una sombra de
/// contacto (elipse en el suelo de semiejes `tam[0]`, `tam[2]`; `color[3]` = cuánto oscurece).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Caja {
    pub pos: [f32; 3],
    pub giro: f32,
    pub desde: [f32; 3],
    pub inclina: f32,
    pub tam: [f32; 3],
    pub color: [u8; 4],
}

impl Caja {
    /// Caja lisa sin partes (los modelos de lejos).
    pub fn lisa(pos: Vec3, giro: f32, tam: [f32; 3], color: [u8; 3]) -> Caja {
        Caja { pos: pos.into(), giro, desde: [0.0; 3], inclina: 0.0, tam, color: [color[0], color[1], color[2], 0] }
    }
    pub fn es_sombra(&self) -> bool {
        self.tam[1] <= 0.0
    }
}

/// Gira `v` alrededor de y como el shader: (0, 0, 1) va a (sen, 0, cos).
pub fn girar(v: Vec3, giro: f32) -> Vec3 {
    let (s, c) = giro.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}

/// Sombra de contacto bajo un modelo: elipse de semiejes (ancho, largo) m en el suelo.
pub fn sombra(out: &mut Vec<Caja>, suelo: Vec3, giro: f32, semi: [f32; 2], fuerza: u8) {
    out.push(Caja { pos: (suelo + Vec3::Y * 0.02).into(), giro, desde: [0.0; 3], inclina: 0.0, tam: [semi[0], 0.0, semi[1]], color: [0, 0, 0, fuerza] });
}

/// Agrega las cajas de un modelo: `base` es el suelo bajo el origen del modelo, `giro` su
/// rumbo (0 = mirando a +z), `escala` (1 = tamaño real), la pose de sus partes y la semilla
/// del matiz de sus vóxeles.
pub fn emitir(out: &mut Vec<Caja>, m: &Modelo, pal: &Paleta, base: Vec3, giro: f32, pose: &Pose, semilla: u32) {
    let k = (semilla % 254 + 1) as u8;
    for c in &m.cajas {
        let a = Vec3::new(c.a[0] as f32, c.a[1] as f32, c.a[2] as f32) * VOX;
        let b = Vec3::new(c.b[0] as f32, c.b[1] as f32, c.b[2] as f32) * VOX;
        let tam = b - a;
        let piso = Vec3::new((a.x + b.x) * 0.5, a.y, (a.z + b.z) * 0.5);
        let col = pal[c.ranura as usize];
        let color = [col[0], col[1], col[2], k];
        let (gy, gx) = pose.get(c.parte);
        if c.parte == Parte::Fijo && c.rot == 0.0 {
            out.push(Caja { pos: (base + girar(piso, giro)).into(), giro, desde: [0.0; 3], inclina: 0.0, tam: tam.into(), color });
        } else {
            let p = Vec3::from(m.pivotes[c.parte as usize]) * VOX;
            out.push(Caja { pos: (base + girar(p, giro)).into(), giro: giro + gy, desde: (piso - p).into(), inclina: gx + c.rot, tam: tam.into(), color });
        }
    }
}

// --- Lienzo: grilla para pintar los modelos -------------------------------------------------

/// Grilla 3D de micro-vóxeles donde se pinta un modelo (lo último pintado pisa lo anterior).
/// Al terminar, [`Lienzo::armar`] la funde en cajas.
pub struct Lienzo {
    min: [i32; 3],
    tam: [i32; 3],
    /// 0 = vacío; si no, (parte << 8) | (ranura + 1).
    celdas: Vec<u16>,
    giradas: Vec<CajaM>,
    pivotes: [[f32; 3]; N_PARTES],
}

impl Lienzo {
    /// Lienzo que cubre de `min` a `max` (exclusivo), en micro-vóxeles.
    pub fn new(min: [i32; 3], max: [i32; 3]) -> Lienzo {
        let tam = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        Lienzo { min, tam, celdas: vec![0; (tam[0] * tam[1] * tam[2]) as usize], giradas: Vec::new(), pivotes: [[0.0; 3]; N_PARTES] }
    }

    fn idx(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (i, j, k) = (x - self.min[0], y - self.min[1], z - self.min[2]);
        (i >= 0 && j >= 0 && k >= 0 && i < self.tam[0] && j < self.tam[1] && k < self.tam[2]).then(|| ((k * self.tam[1] + j) * self.tam[0] + i) as usize)
    }

    pub fn pivote(&mut self, p: Parte, at: [f32; 3]) {
        self.pivotes[p as usize] = at;
    }

    pub fn punto(&mut self, parte: Parte, ranura: u8, x: i32, y: i32, z: i32) {
        if let Some(i) = self.idx(x, y, z) {
            self.celdas[i] = ((parte as u16) << 8) | (ranura as u16 + 1);
        }
    }

    /// Caja de `a` a `b` (exclusivo).
    pub fn caja(&mut self, parte: Parte, ranura: u8, a: [i32; 3], b: [i32; 3]) {
        for z in a[2]..b[2] {
            for y in a[1]..b[1] {
                for x in a[0]..b[0] {
                    self.punto(parte, ranura, x, y, z);
                }
            }
        }
    }

    /// Caja simétrica en x: de `-ax` a `ax` (centrada), y y z como en [`caja`](Self::caja).
    pub fn centrada(&mut self, parte: Parte, ranura: u8, ax: i32, y: [i32; 2], z: [i32; 2]) {
        self.caja(parte, ranura, [-ax, y[0], z[0]], [ax, y[1], z[1]]);
    }

    /// La misma caja a los dos lados: de x0 a x1 y de −x1 a −x0 (partes izquierda y derecha).
    pub fn par(&mut self, partes: (Parte, Parte), ranura: u8, x: [i32; 2], y: [i32; 2], z: [i32; 2]) {
        self.caja(partes.0, ranura, [-x[1], y[0], z[0]], [-x[0], y[1], z[1]]);
        self.caja(partes.1, ranura, [x[0], y[0], z[0]], [x[1], y[1], z[1]]);
    }

    pub fn borrar(&mut self, a: [i32; 3], b: [i32; 3]) {
        for z in a[2]..b[2] {
            for y in a[1]..b[1] {
                for x in a[0]..b[0] {
                    if let Some(i) = self.idx(x, y, z) {
                        self.celdas[i] = 0;
                    }
                }
            }
        }
    }

    /// Cilindro parado (eje y) de radio `rad` centrado en (cx, cz) (centro de celda en
    /// cx − ½ si el diámetro es par: se usan coordenadas dobles).
    pub fn cilindro_y(&mut self, parte: Parte, ranura: u8, c2: [i32; 2], rad: f32, y: [i32; 2]) {
        let rr = (rad + 0.5).ceil() as i32 + 1;
        for z in (c2[1] / 2 - rr)..=(c2[1] / 2 + rr) {
            for x in (c2[0] / 2 - rr)..=(c2[0] / 2 + rr) {
                let dx = (2 * x + 1 - c2[0]) as f32 * 0.5;
                let dz = (2 * z + 1 - c2[1]) as f32 * 0.5;
                if dx * dx + dz * dz <= rad * rad {
                    for yy in y[0]..y[1] {
                        self.punto(parte, ranura, x, yy, z);
                    }
                }
            }
        }
    }

    /// Cilindro acostado (eje x) de radio `rad` con centro (cy, cz) en coordenadas dobles.
    pub fn cilindro_x(&mut self, parte: Parte, ranura: u8, c2: [i32; 2], rad: f32, x: [i32; 2]) {
        let rr = (rad + 0.5).ceil() as i32 + 1;
        for z in (c2[1] / 2 - rr)..=(c2[1] / 2 + rr) {
            for y in (c2[0] / 2 - rr)..=(c2[0] / 2 + rr) {
                let dy = (2 * y + 1 - c2[0]) as f32 * 0.5;
                let dz = (2 * z + 1 - c2[1]) as f32 * 0.5;
                if dy * dy + dz * dz <= rad * rad {
                    for xx in x[0]..x[1] {
                        self.punto(parte, ranura, xx, y, z);
                    }
                }
            }
        }
    }

    /// Caja girada `rot` rad alrededor del eje x que pasa por el pivote de `parte` (no entra en
    /// la grilla: se agrega tal cual).
    pub fn girada(&mut self, parte: Parte, ranura: u8, a: [i32; 3], b: [i32; 3], rot: f32) {
        let c = |v: [i32; 3]| [v[0] as i16, v[1] as i16, v[2] as i16];
        self.giradas.push(CajaM { a: c(a), b: c(b), ranura, parte, rot });
    }

    /// Rueda en el plano yz, centrada en el pivote de `parte`: llanta de `n` tramos (de radio
    /// exterior `rad` y `grueso` de ancho), `rayos` rayos y maza; de x0 a x1.
    #[allow(clippy::too_many_arguments)]
    pub fn rueda(&mut self, parte: Parte, ranura: u8, ranura_maza: u8, x: [i32; 2], rad: i32, grueso: i32, n: u32, rayos: u32) {
        let p = self.pivotes[parte as usize];
        let (py, pz) = (p[1].round() as i32, p[2].round() as i32);
        let paso = std::f32::consts::TAU / n as f32;
        // Largo de cada tramo: la cuerda del radio medio, un poco más para que no se abran.
        let lt = ((2.0 * (rad as f32 - grueso as f32 * 0.5) * (paso * 0.5).sin()) + 1.5).ceil() as i32;
        for k in 0..n {
            self.girada(parte, ranura, [x[0], py + rad - grueso, pz - lt / 2], [x[1], py + rad, pz - lt / 2 + lt], k as f32 * paso);
        }
        let medio = rayos.max(1) / 2;
        for k in 0..medio.max(1) {
            // Cada rayo cruza la maza de lado a lado.
            let ang = k as f32 * std::f32::consts::PI / medio.max(1) as f32;
            self.girada(parte, ranura, [x[0] + 1, py - rad + grueso, pz - 1], [x[1] - 1, py + rad - grueso, pz + 1], ang);
        }
        let m = (rad / 6).max(2);
        self.girada(parte, ranura_maza, [x[0] - 1, py - m, pz - m], [x[1] + 1, py + m, pz + m], 0.0);
    }

    /// Funde la grilla en cajas (voraz: a lo largo de x, después y, después z, por parte y
    /// color) y arma el modelo.
    pub fn armar(self, nombre: &'static str) -> Modelo {
        let [tx, ty, tz] = self.tam;
        let mut usada = vec![false; self.celdas.len()];
        let at = |x: i32, y: i32, z: i32| ((z * ty + y) * tx + x) as usize;
        let mut cajas = Vec::new();
        for z in 0..tz {
            for y in 0..ty {
                for x in 0..tx {
                    let i = at(x, y, z);
                    let k = self.celdas[i];
                    if k == 0 || usada[i] {
                        continue;
                    }
                    let libre = |x: i32, y: i32, z: i32, usada: &Vec<bool>| {
                        let j = at(x, y, z);
                        self.celdas[j] == k && !usada[j]
                    };
                    let mut x1 = x + 1;
                    while x1 < tx && libre(x1, y, z, &usada) {
                        x1 += 1;
                    }
                    let mut y1 = y + 1;
                    while y1 < ty && (x..x1).all(|xx| libre(xx, y1, z, &usada)) {
                        y1 += 1;
                    }
                    let mut z1 = z + 1;
                    while z1 < tz && (y..y1).all(|yy| (x..x1).all(|xx| libre(xx, yy, z1, &usada))) {
                        z1 += 1;
                    }
                    for zz in z..z1 {
                        for yy in y..y1 {
                            for xx in x..x1 {
                                usada[at(xx, yy, zz)] = true;
                            }
                        }
                    }
                    let parte = Parte::TODAS[(k >> 8) as usize];
                    let m = self.min;
                    cajas.push(CajaM {
                        a: [(x + m[0]) as i16, (y + m[1]) as i16, (z + m[2]) as i16],
                        b: [(x1 + m[0]) as i16, (y1 + m[1]) as i16, (z1 + m[2]) as i16],
                        ranura: (k & 0xFF) as u8 - 1,
                        parte,
                        rot: 0.0,
                    });
                }
            }
        }
        cajas.extend(self.giradas);
        let mut min = [i16::MAX; 3];
        let mut max = [i16::MIN; 3];
        for c in &cajas {
            for e in 0..3 {
                min[e] = min[e].min(c.a[e]);
                max[e] = max[e].max(c.b[e]);
            }
        }
        if cajas.is_empty() {
            (min, max) = ([0; 3], [0; 3]);
        }
        let huella = [(max[0] - min[0]) as f32 * VOX * 0.5, (max[2] - min[2]) as f32 * VOX * 0.5];
        Modelo { nombre, cajas, pivotes: self.pivotes, min, max, huella }
    }
}

// --- Biblioteca -----------------------------------------------------------------------------

/// Modelos de detalle (todo menos las personas, que se arman por atuendo: [`ropa::Atuendo`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Id {
    Caballo,
    CaballoEnsillado,
    Vaca,
    VacaOvera,
    Buey,
    Oveja,
    Perro,
    Gallina,
    /// Carreta de bueyes en marcha (las ruedas giran), con su lanza y el yugo.
    Carreta,
    /// Carreta estacionada: sin yunta, la lanza apoyada en el suelo.
    CarretaParada,
    Carro,
    Bote,
    FarolAceite,
    FarolGas,
    Tonel,
    Barrica,
    Banco,
    BrocalAljibe,
    PozoRoldana,
    Cruz,
    Veleta,
    Letrero,
    Palenque,
    // [ciudad] Detalles de las fachadas (los piden los constructores de `structures/`).
    /// Reja de hierro forjado embutida en el vano de una ventana de 1 m × 2 m (y de 2,5 m).
    RejaPlana,
    RejaPlanaAlta,
    /// Reja volada (jaula que sale medio metro de la fachada), 1 m × 2 m (y de 2,5 m).
    RejaVolada,
    RejaVoladaAlta,
    /// Farol de aceite de fachada, colgado de un brazo de hierro.
    FarolBrazo,
    /// Farol de gas de fachada (época 2), de brazo con voluta.
    FarolGasBrazo,
    /// Maceta de barro con un geranio (balcones).
    Maceta,
    /// Aldaba o llamador de la puerta de calle.
    Llamador,
}

impl Id {
    pub const TODOS: [Id; 31] = [
        Id::Caballo,
        Id::CaballoEnsillado,
        Id::Vaca,
        Id::VacaOvera,
        Id::Buey,
        Id::Oveja,
        Id::Perro,
        Id::Gallina,
        Id::Carreta,
        Id::CarretaParada,
        Id::Carro,
        Id::Bote,
        Id::FarolAceite,
        Id::FarolGas,
        Id::Tonel,
        Id::Barrica,
        Id::Banco,
        Id::BrocalAljibe,
        Id::PozoRoldana,
        Id::Cruz,
        Id::Veleta,
        Id::Letrero,
        Id::Palenque,
        Id::RejaPlana,
        Id::RejaPlanaAlta,
        Id::RejaVolada,
        Id::RejaVoladaAlta,
        Id::FarolBrazo,
        Id::FarolGasBrazo,
        Id::Maceta,
        Id::Llamador,
    ];
}

/// Todos los modelos, armados una vez.
pub struct Biblioteca {
    pub objetos: Vec<Modelo>,
    /// Personas: `ropa::Atuendo as usize * 2 + montado`.
    pub personas: Vec<Modelo>,
}

impl Biblioteca {
    pub fn objeto(&self, id: Id) -> &Modelo {
        &self.objetos[id as usize]
    }
    pub fn persona(&self, a: ropa::Atuendo, montado: bool) -> &Modelo {
        &self.personas[a as usize * 2 + montado as usize]
    }
    /// Cajas y bytes de memoria de toda la biblioteca.
    pub fn medida(&self) -> (usize, usize) {
        let n: usize = self.objetos.iter().chain(&self.personas).map(|m| m.cajas.len()).sum();
        (n, n * std::mem::size_of::<CajaM>())
    }
}

pub fn biblioteca() -> &'static Biblioteca {
    static B: OnceLock<Biblioteca> = OnceLock::new();
    B.get_or_init(|| Biblioteca {
        objetos: Id::TODOS.iter().map(|&i| modelos::armar(i)).collect(),
        personas: ropa::Atuendo::TODOS.iter().flat_map(|&a| [false, true].map(|m| ropa::figura(a, m))).collect(),
    })
}

// --- Detalles fijos por zona ----------------------------------------------------------------

/// Lado de las zonas de [`Detalles`] (m).
pub const ZONA: f32 = 64.0;
/// Hasta dónde se dibujan los detalles fijos (m).
pub const RADIO_DETALLES: f32 = 140.0;

/// Un detalle puesto en el mundo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instancia {
    pub modelo: Id,
    /// Suelo bajo el origen del modelo (m).
    pub pos: Vec3,
    pub giro: f32,
    /// Elige la variante de colores y el matiz de los vóxeles.
    pub semilla: u32,
}

/// Detalles fijos (faroles, toneles, carretas paradas, botes…) por zona de [`ZONA`] m. Lo
/// llena cualquier sistema con [`poner`](Self::poner); el dibujo toma los cercanos.
#[derive(Clone, Debug, Default)]
pub struct Detalles {
    zonas: HashMap<(i32, i32), Vec<Instancia>, crate::fxhash::FxBuild>,
    n: usize,
}

impl Detalles {
    fn zona(p: Vec2) -> (i32, i32) {
        ((p.x / ZONA).floor() as i32, (p.y / ZONA).floor() as i32)
    }

    /// Pone un modelo con su base en `pos` (el suelo), mirando hacia `giro` (rad, 0 = +z).
    pub fn poner(&mut self, modelo: Id, pos: Vec3, giro: f32, semilla: u32) {
        self.zonas.entry(Self::zona(Vec2::new(pos.x, pos.z))).or_default().push(Instancia { modelo, pos, giro, semilla });
        self.n += 1;
    }

    /// Saca los detalles que cumplen `f` (por ejemplo, los de un solar que se demolió).
    pub fn quitar(&mut self, f: impl Fn(&Instancia) -> bool) {
        for v in self.zonas.values_mut() {
            v.retain(|i| !f(i));
        }
        self.zonas.retain(|_, v| !v.is_empty());
        self.n = self.zonas.values().map(|v| v.len()).sum();
    }

    pub fn limpiar(&mut self) {
        self.zonas.clear();
        self.n = 0;
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Los detalles a menos de `radio` m de `centro`.
    pub fn cerca(&self, centro: Vec2, radio: f32) -> impl Iterator<Item = &Instancia> {
        let (a, b) = (Self::zona(centro - Vec2::splat(radio)), Self::zona(centro + Vec2::splat(radio)));
        (a.1..=b.1).flat_map(move |z| (a.0..=b.0).map(move |x| (x, z))).filter_map(|k| self.zonas.get(&k)).flatten().filter(move |i| Vec2::new(i.pos.x, i.pos.z).distance(centro) < radio)
    }

    /// Cajas de los detalles cercanos a `centro`, con su sombra de contacto.
    pub fn cajas(&self, centro: Vec2, out: &mut Vec<Caja>) {
        let b = biblioteca();
        for i in self.cerca(centro, RADIO_DETALLES) {
            let m = b.objeto(i.modelo);
            emitir(out, m, &modelos::paleta(i.modelo, i.semilla), i.pos, i.giro, &Pose::default(), i.semilla);
            if modelos::con_sombra(i.modelo) {
                sombra(out, i.pos, i.giro, [m.huella[0] * 0.9, m.huella[1] * 0.9], 110);
            }
        }
    }
}
