//! El país (escala país): el Uruguay entero como simulación agregada, alrededor de la ciudad
//! del jugador (ver docs/GDD.md §4.9 y docs/sistemas/mapa-pais.md).
//!
//! - **Datos** (`datos`): relieve, agua, departamentos, ríos, monte, bañados, geología y la
//!   historia (pueblos con su año, caminos reales, puertos, estancias, saladeros, vías).
//! - **Estado** ([`Pais`]): pueblos con población, estancias con ganado, saladeros, caminos con
//!   sus pasos (vado, balsa, puente), puertos, sequías y crecientes. Se guarda aparte de la
//!   ciudad (`pais` en la partida).
//! - **Mes** ([`Pais::mes_tick`]): el país vive solo (los pueblos y caminos de la historia
//!   aparecen en su año, las estancias crecen alrededor de los pueblos), el ganado crece, los
//!   saladeros faenan, y las carretas llevan cueros, sebo, tasajo y ganado en pie al puerto de
//!   Montevideo por la red de caminos ([`rutas`]). Lo que llega es el [`Aporte`] que la ciudad
//!   suma a su economía el mes siguiente; lo que queda lejos se va de contrabando a la frontera.
//! - **Acciones** ([`Accion`]): fundar pueblos, otorgar estancias, saladeros, trazar y mejorar
//!   caminos, balsas y puentes en los pasos, puertos fluviales; cada una con su época
//!   ([`permitido`]).
//! - **Mapa** (`ui`): el modo «Mapa del país», un mapa estilizado con relieve en terrazas.

pub mod datos;
pub mod rutas;
pub mod tour;
pub mod ui;
#[cfg(test)]
mod tests;

use crate::buildings::Rng;
use crate::city::poblacion::origenes::Epoca;
use datos::{geo, largo};
use serde::{Deserialize, Serialize};

/// Año del mes `mes` del calendario de la ciudad (0 = enero de 1726).
pub fn anio_de(mes: u32) -> i32 {
    crate::city::START_YEAR as i32 + (mes / 12) as i32
}

/// Época del país en un año (la de la ciudad, en el modo fiel a la historia).
pub fn epoca_de(anio: i32) -> Epoca {
    Epoca::del_año(anio)
}

/// Orden de las épocas (para comparar).
pub fn nivel_epoca(e: Epoca) -> u8 {
    Epoca::ALL.iter().position(|x| *x == e).unwrap_or(0) as u8
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pueblo {
    pub id: String,
    pub nombre: String,
    pub pos: [f32; 2],
    pub anio: i32,
    /// capital, ciudad, villa, pueblo, fuerte, reduccion, colonia.
    pub tipo: String,
    pub poblacion: f32,
    #[serde(default)]
    pub jugador: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Estancia {
    pub id: u32,
    pub nombre: String,
    pub pos: [f32; 2],
    pub anio: i32,
    /// Cabezas de ganado vacuno.
    pub ganado: f32,
    #[serde(default)]
    pub jugador: bool,
    /// Id en la historia (las de muestra), para no repetirlas.
    #[serde(default)]
    pub hist: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Saladero {
    pub id: u32,
    pub nombre: String,
    pub pos: [f32; 2],
    pub anio: i32,
    /// Cabezas que puede faenar por mes.
    pub capacidad: f32,
    /// Cabezas faenadas el último mes.
    #[serde(default)]
    pub faena: f32,
    #[serde(default)]
    pub jugador: bool,
    #[serde(default)]
    pub hist: Option<String>,
}

/// Obra de un paso de río: vado (se corta con las crecientes), balsa o puente.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Obra {
    Vado,
    Balsa,
    Puente,
}

impl Obra {
    pub fn nombre(self) -> &'static str {
        match self {
            Obra::Vado => "vado",
            Obra::Balsa => "balsa",
            Obra::Puente => "puente",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Paso {
    pub pos: [f32; 2],
    pub rio: String,
    /// Ancho del cruce (km).
    pub ancho: f32,
    pub obra: Obra,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Camino {
    pub id: u32,
    pub nombre: String,
    pub desde: String,
    pub hasta: String,
    pub puntos: Vec<[f32; 2]>,
    pub anio: i32,
    /// 0 huella, 1 camino de carretas (camino real), 2 camino con puentes y alcantarillas,
    /// 3 camino afirmado (empedrado o macadam).
    pub nivel: u8,
    pub pasos: Vec<Paso>,
    #[serde(default)]
    pub jugador: bool,
}

pub const NIVELES: [&str; 4] = ["huella", "camino real (carretas)", "camino con alcantarillas", "camino afirmado"];

impl Camino {
    pub fn km(&self) -> f32 {
        let g = geo();
        self.puntos.windows(2).map(|w| g.km(w[0], w[1])).sum()
    }

    /// Velocidad de una carreta (km por día) según el nivel. ESTIMADO: las carretas de bueyes
    /// hacían unas 3 a 5 leguas por día en buen camino.
    pub fn velocidad(&self) -> f32 {
        [10.0, 16.0, 22.0, 30.0][self.nivel.min(3) as usize]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Puerto {
    pub pueblo: String,
    pub desde: i32,
    #[serde(default)]
    pub jugador: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoEvento {
    Sequia,
    Creciente,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Evento {
    pub tipo: TipoEvento,
    /// Departamento (sequía) o río (creciente).
    pub zona: String,
    pub centro: [f32; 2],
    /// Radio en celdas (sequía).
    pub radio: f32,
    pub meses: u32,
}

/// Lo que el interior le dejó a la capital en el último mes (la ciudad lo suma a su economía
/// el mes siguiente: las carretas tardan en llegar).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Aporte {
    /// Cueros (unidades de la economía de la ciudad).
    pub cueros: f32,
    pub sebo: f32,
    /// Carne (raciones): ganado en pie para el abasto.
    pub carne: f32,
    /// Tasajo embarcado (arrobas), paga derechos de aduana.
    pub tasajo: f32,
    /// Derechos del tasajo y comercio con los pueblos (pesos).
    pub derechos: i64,
    pub comercio: i64,
    /// Cueros que se fueron de contrabando por la frontera (no llegaron).
    pub contrabando: f32,
    /// Carretas del mes (para la gaceta y el mapa).
    pub carretas: u32,
}

impl Aporte {
    /// Suma el aporte del interior a la entrada del mes de la economía de la ciudad.
    pub fn aplicar(&self, inp: &mut crate::city::economy::MonthInput) {
        use crate::city::economy::Res;
        inp.production[Res::Cueros as usize] += self.cueros;
        inp.production[Res::Sebo as usize] += self.sebo;
        inp.production[Res::Carne as usize] += self.carne;
        if self.derechos > 0 {
            inp.other.push(("Derechos del tasajo del interior".into(), self.derechos));
        }
        if self.comercio > 0 {
            inp.other.push(("Comercio con los pueblos del interior".into(), self.comercio));
        }
    }
}

/// Carga que viajó por una ruta el último mes (para dibujar los flujos).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Flujo {
    pub puntos: Vec<[f32; 2]>,
    pub carga: f32,
    /// 0 a Montevideo, 1 de contrabando a la frontera, 2 a un puerto del río, 3 a una
    /// estación del ferrocarril.
    pub destino: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pais {
    /// Mes del calendario (el mismo que el de la ciudad).
    pub mes: u32,
    pub pueblos: Vec<Pueblo>,
    pub estancias: Vec<Estancia>,
    pub saladeros: Vec<Saladero>,
    pub caminos: Vec<Camino>,
    pub puertos: Vec<Puerto>,
    pub eventos: Vec<Evento>,
    pub azar: u64,
    pub ultimo: Aporte,
    pub flujos: Vec<Flujo>,
    /// Últimas noticias del país: (mes, texto).
    pub cronica: Vec<(u32, String)>,
    pub proximo: u32,
}

impl Default for Pais {
    fn default() -> Self {
        Pais::nuevo(2)
    }
}

/// Acciones del jugador en el mapa del país.
#[derive(Clone, Debug, PartialEq)]
pub enum Accion {
    FundarPueblo([f32; 2]),
    Estancia([f32; 2]),
    Saladero([f32; 2]),
    /// Camino nuevo entre dos lugares (ids de pueblos).
    Camino(String, String),
    MejorarCamino(u32),
    /// Obra en un paso: camino, índice del paso, obra.
    Paso(u32, usize, Obra),
    Puerto(String),
}

impl Accion {
    pub fn nombre(&self) -> &'static str {
        match self {
            Accion::FundarPueblo(_) => "Fundar un pueblo",
            Accion::Estancia(_) => "Otorgar una estancia",
            Accion::Saladero(_) => "Saladero",
            Accion::Camino(..) => "Trazar un camino",
            Accion::MejorarCamino(_) => "Mejorar el camino",
            Accion::Paso(_, _, Obra::Puente) => "Puente",
            Accion::Paso(..) => "Balsa",
            Accion::Puerto(_) => "Puerto fluvial",
        }
    }
}

/// Costos de las obras (pesos del cabildo). ESTIMADO.
pub const COSTO_PUEBLO: i64 = 3000;
pub const COSTO_ESTANCIA: i64 = 400;
pub const COSTO_SALADERO: i64 = 2500;
pub const COSTO_KM_CAMINO: f32 = 6.0;
pub const COSTO_BALSA: i64 = 300;
pub const COSTO_PUERTO: i64 = 4000;
/// Primer saladero de la Banda Oriental (Colla, 1781).
pub const ANIO_SALADERO: i32 = 1781;

/// ¿La época permite esta acción? Si no, el motivo (para la interfaz).
pub fn permitido(a: &Accion, epoca: Epoca, anio: i32) -> Result<(), String> {
    let n = nivel_epoca(epoca);
    match a {
        Accion::FundarPueblo(_) if n < 1 => Err("En la época colonial los pueblos los funda la Corona: fundar pueblos se desbloquea en la Ciudad Nueva (época 2)".into()),
        Accion::Camino(..) if n < 1 => Err("En la época colonial los caminos los abre el uso de las carretas: trazar caminos se desbloquea en la Ciudad Nueva (época 2)".into()),
        Accion::Paso(_, _, Obra::Puente) if n < 1 => Err("Los puentes del interior llegan con la Ciudad Nueva (época 2); en la colonia, balsas".into()),
        Accion::Puerto(_) if n < 1 => Err("Los puertos del litoral se abren con los vapores (época 2); en la colonia el comercio legal sale por Montevideo".into()),
        Accion::Saladero(_) if anio < ANIO_SALADERO => Err(format!("Todavía no se conoce la salazón de carnes (el primer saladero oriental es de {ANIO_SALADERO})")),
        _ => Ok(()),
    }
}

/// Nivel máximo de los caminos en una época.
pub fn nivel_max(epoca: Epoca) -> u8 {
    match nivel_epoca(epoca) {
        0 => 1,
        1 => 2,
        _ => 3,
    }
}

const APELLIDOS: [&str; 24] = [
    "Durán", "Artigas", "Haedo", "Alzáibar", "Viana", "Zabala", "Larrañaga", "Pereira", "Mancebo", "Sosa", "Medina", "Fernández", "Silva", "Correa", "Ortiz", "Burgues", "Soria", "Gadea", "Villagrán", "Rodríguez", "Oribe", "Lavalleja", "Muñoz", "Chain",
];
const NOMBRES_PUEBLO: [&str; 16] = [
    "San Gregorio", "Santa Clara", "Cerro Chato", "Paso del Medio", "Villa del Carmen", "San Antonio", "Santa Isabel", "Cuchilla Alta", "Paso Hondo", "Villa Nueva", "San Ramón", "Pueblo del Arroyo", "Santa Ana", "Las Cañas", "Tres Árboles", "Puntas del Yí",
];

impl Pais {
    /// El país al empezar el mes `mes` (por omisión, marzo de 1726): lo que ya existía.
    pub fn nuevo(mes: u32) -> Pais {
        let mut p = Pais { mes, pueblos: Vec::new(), estancias: Vec::new(), saladeros: Vec::new(), caminos: Vec::new(), puertos: Vec::new(), eventos: Vec::new(), azar: 0x5EED_1726, ultimo: Aporte::default(), flujos: Vec::new(), cronica: Vec::new(), proximo: 1 };
        p.historia(true);
        p
    }

    /// El país simulado mes a mes hasta `mes` (para las partidas viejas, que no lo traen, y
    /// para las pruebas). Determinista.
    pub fn simulado_hasta(mes: u32) -> Pais {
        let mut p = Pais::nuevo(mes.min(2));
        while p.mes < mes {
            let pob = poblacion_capital_estimada(anio_de(p.mes));
            p.paso(pob, false);
        }
        p
    }

    pub fn anio(&self) -> i32 {
        anio_de(self.mes)
    }

    pub fn epoca(&self) -> Epoca {
        epoca_de(self.anio())
    }

    fn id(&mut self) -> u32 {
        self.proximo += 1;
        self.proximo
    }

    pub fn pueblo(&self, id: &str) -> Option<&Pueblo> {
        self.pueblos.iter().find(|p| p.id == id)
    }

    fn noticia(&mut self, s: String) {
        self.cronica.push((self.mes, s));
        if self.cronica.len() > 40 {
            self.cronica.remove(0);
        }
    }

    /// Lo que la historia trae hasta el año en curso: pueblos, caminos, puertos, estancias y
    /// saladeros de muestra. `silencio`: sin noticias (al armar el país inicial).
    fn historia(&mut self, silencio: bool) -> Vec<String> {
        let g = geo();
        let anio = self.anio();
        let mut nuevas = Vec::new();
        for h in &g.historia.pueblos {
            if h.anio <= anio && self.pueblo(&h.id).is_none() {
                // Los pueblos viejos arrancan con su población crecida (ESTIMADO).
                let años = (anio - h.anio).max(0) as f32;
                let pob = if h.tipo == "capital" { 50.0 } else { h.poblacion_inicial * (1.0 + 0.02 * años.min(40.0)) };
                self.pueblos.push(Pueblo { id: h.id.clone(), nombre: h.nombre.clone(), pos: h.pos, anio: h.anio, tipo: h.tipo.clone(), poblacion: pob, jugador: false });
                if !silencio && h.tipo != "capital" {
                    nuevas.push(format!("Se funda {} ({})", h.nombre, h.departamento.clone().unwrap_or_default()));
                }
            }
        }
        for c in &g.historia.caminos {
            let ya = self.caminos.iter().any(|x| !x.jugador && x.desde == c.desde && x.hasta == c.hasta);
            if c.anio <= anio && !ya && self.pueblo(&c.desde).is_some() && self.pueblo(&c.hasta).is_some() {
                let id = self.id();
                let pasos = rutas::pasos_de(&c.puntos, anio);
                self.caminos.push(Camino { id, nombre: c.nombre.clone(), desde: c.desde.clone(), hasta: c.hasta.clone(), puntos: c.puntos.clone(), anio: c.anio, nivel: 1, pasos, jugador: false });
                if !silencio {
                    nuevas.push(format!("Las carretas abren el {}", c.nombre));
                }
            }
        }
        for h in &g.historia.puertos {
            if h.desde <= anio && !self.puertos.iter().any(|p| p.pueblo == h.pueblo) && self.pueblo(&h.pueblo).is_some() {
                self.puertos.push(Puerto { pueblo: h.pueblo.clone(), desde: h.desde, jugador: false });
            }
        }
        for h in &g.historia.estancias {
            if h.anio <= anio && !self.estancias.iter().any(|e| e.hist.as_deref() == Some(h.id.as_str())) {
                let id = self.id();
                self.estancias.push(Estancia { id, nombre: h.nombre.clone(), pos: h.pos, anio: h.anio, ganado: 1500.0, jugador: false, hist: Some(h.id.clone()) });
            }
        }
        for h in &g.historia.saladeros {
            if h.anio <= anio && !self.saladeros.iter().any(|e| e.hist.as_deref() == Some(h.id.as_str())) {
                let id = self.id();
                self.saladeros.push(Saladero { id, nombre: h.nombre.clone(), pos: h.pos, anio: h.anio, capacidad: 250.0, faena: 0.0, jugador: false, hist: Some(h.id.clone()) });
                if !silencio {
                    nuevas.push(format!("Abre el {}", h.nombre));
                }
            }
        }
        nuevas
    }

    /// Avanza el país hasta el mes de la ciudad y devuelve las noticias. `pob_capital`: los
    /// vecinos de la ciudad; `sequia_capital`: la ciudad está en sequía (la región sur también).
    pub fn mes_tick(&mut self, mes_ciudad: u32, pob_capital: i32, sequia_capital: bool) -> Vec<String> {
        let mut news = Vec::new();
        while self.mes < mes_ciudad {
            news.extend(self.paso(pob_capital as f32, sequia_capital));
        }
        news
    }

    /// Un mes del país.
    fn paso(&mut self, pob_capital: f32, sequia_capital: bool) -> Vec<String> {
        self.mes += 1;
        let mut news = Vec::new();
        let anio = self.anio();
        let mut rng = Rng::from_state(self.azar);
        if self.mes % 12 == 0 {
            news.extend(self.historia(false));
            news.extend(self.estancias_nuevas(&mut rng));
        }
        if let Some(cap) = self.pueblos.iter_mut().find(|p| p.tipo == "capital") {
            cap.poblacion = pob_capital.max(50.0);
        }
        news.extend(self.eventos_del_mes(&mut rng, sequia_capital));
        // Ganado: crece con la pastura (menos en la sequía) hasta lo que da el campo.
        let g = geo();
        let sequia_en = |eventos: &[Evento], p: [f32; 2]| eventos.iter().any(|e| e.tipo == TipoEvento::Sequia && g.km(e.centro, p) < e.radio);
        for e in self.estancias.iter_mut() {
            let pasto = g.pastura(e.pos[0], e.pos[1]);
            let seca = if sequia_en(&self.eventos, e.pos) { 0.55 } else { 1.0 };
            let cap = 5000.0 * pasto * seca;
            let crece = e.ganado * 0.022 * pasto * seca * (1.0 - e.ganado / cap.max(100.0));
            e.ganado = (e.ganado + crece).max(50.0);
        }
        // Rutas a la capital (carretas, puertos del río y, desde la época 3, el ferrocarril).
        let red = rutas::Red::armar(self, anio);
        let mut ap = Aporte::default();
        let mut flujos: Vec<Flujo> = Vec::new();
        let frontera = |p: [f32; 2]| rutas::km_a_la_frontera(p);
        let colonial = nivel_epoca(epoca_de(anio)) == 0;
        // Saladeros: compran el ganado de las estancias cercanas (hasta su capacidad).
        let mut vendido = vec![0.0f32; self.estancias.len()];
        for s in self.saladeros.iter_mut() {
            let mut falta = s.capacidad;
            let mut idx: Vec<usize> = (0..self.estancias.len()).filter(|&k| g.km(self.estancias[k].pos, s.pos) < 120.0).collect();
            idx.sort_by(|&a, &b| g.km(self.estancias[a].pos, s.pos).partial_cmp(&g.km(self.estancias[b].pos, s.pos)).unwrap().then(a.cmp(&b)));
            for k in idx {
                let disp = self.estancias[k].ganado * 0.01 - vendido[k];
                let t = disp.min(falta).max(0.0);
                vendido[k] += t;
                falta -= t;
                if falta <= 0.0 {
                    break;
                }
            }
            s.faena = s.capacidad - falta.max(0.0);
        }
        for (k, e) in self.estancias.iter_mut().enumerate() {
            e.ganado -= vendido[k];
        }
        // Lo que sale de cada productor: (posición, cueros, sebo, tasajo, carne en pie).
        let mut cargas: Vec<([f32; 2], f32, f32, f32, f32)> = Vec::new();
        for (k, e) in self.estancias.iter_mut().enumerate() {
            // Faena en la estancia (cueros y sebo, la carne se pierde en la colonia) y arreos.
            let faena = e.ganado * 0.008;
            e.ganado -= faena;
            let arreo = e.ganado * 0.003;
            let _ = k;
            cargas.push((e.pos, faena * 0.25, faena * 0.08, 0.0, arreo));
        }
        for s in &self.saladeros {
            cargas.push((s.pos, s.faena * 0.25, s.faena * 0.12, s.faena * 4.0, 0.0));
        }
        for (pos, cueros, sebo, tasajo, en_pie) in cargas {
            let Some(r) = red.ruta(pos) else { continue };
            // Contrabando: lejos de Montevideo y cerca de la frontera con los portugueses, una
            // parte de los cueros se va por la frontera (más en la colonia). ESTIMADO.
            let km_f = frontera(pos);
            let lejos = ((r.dias - 8.0) / 20.0).clamp(0.0, 1.0);
            let cerca_frontera = (1.0 - km_f / 120.0).clamp(0.0, 1.0);
            let contra = (lejos * (0.3 + 0.5 * cerca_frontera) * if colonial { 1.0 } else { 0.4 }).clamp(0.0, 0.85);
            // Se pierde algo en el camino (1 % por día de viaje, hasta la mitad).
            let llega = (1.0 - contra) * (1.0 - (r.dias * 0.01).min(0.5));
            ap.cueros += cueros * llega;
            ap.sebo += sebo * llega;
            ap.tasajo += tasajo * llega;
            // El ganado en pie solo llega al abasto desde menos de 12 días de arreo.
            if r.dias < 12.0 {
                ap.carne += en_pie * 20.0 * llega;
            }
            ap.contrabando += cueros * contra;
            ap.carretas += ((cueros + sebo + tasajo * 0.05) / 8.0).ceil() as u32;
            let carga = cueros + sebo + tasajo * 0.05 + en_pie * 0.2;
            if carga > 0.5 {
                flujos.push(Flujo { puntos: r.puntos.clone(), carga: carga * llega, destino: if r.por_rio { 2 } else if r.por_tren { 3 } else { 0 } });
                if contra > 0.05 {
                    flujos.push(Flujo { puntos: vec![pos, rutas::punto_frontera(pos)], carga: carga * contra, destino: 1 });
                }
            }
        }
        // Pueblos: crecen con la conexión a la capital y con las estancias de alrededor.
        let mut comercio = 0.0;
        for i in 0..self.pueblos.len() {
            let p = &self.pueblos[i];
            if p.tipo == "capital" {
                continue;
            }
            let dias = red.ruta(p.pos).map_or(60.0, |r| r.dias);
            let conexion = (1.0 - dias / 25.0).clamp(0.0, 1.0);
            let est = self.estancias.iter().filter(|e| g.km(e.pos, p.pos) < 40.0).count() as f32;
            let seca = if sequia_en(&self.eventos, p.pos) { 0.3 } else { 1.0 };
            // Cada pueblo es su pago entero (el pueblo y la campaña de alrededor). La capacidad
            // y el crecimiento suben con cada época (inmigración, ferrocarril): ESTIMADO,
            // calibrado con el orden de los censos (interior ~85.000 en 1852, ~700.000 en 1908).
            let ep = nivel_epoca(epoca_de(anio)) as usize;
            let escala = [1.6, 4.0, 14.0, 18.0, 22.0, 24.0][ep.min(5)];
            let extra = [0.0, 0.012, 0.035, 0.01, 0.0, 0.0][ep.min(5)];
            let r = (0.012 + 0.025 * conexion + 0.01 * (est / 5.0).min(1.0) + extra) * seca;
            let k = (250.0 + 2500.0 * conexion + 300.0 * est.min(10.0) + if self.puertos.iter().any(|x| x.pueblo == p.id && x.desde <= anio) { 1500.0 } else { 0.0 }) * escala;
            let pob = p.poblacion + p.poblacion * r / 12.0 * (1.0 - p.poblacion / k);
            comercio += pob * 0.004 * conexion;
            self.pueblos[i].poblacion = pob.max(20.0);
        }
        ap.comercio = comercio as i64;
        // Derechos de aduana del tasajo embarcado (precio 1 peso la arroba, 8 %). ESTIMADO.
        ap.derechos = (ap.tasajo * 0.08) as i64;
        if ap.contrabando > 20.0 && self.mes % 12 == 6 {
            news.push(format!("Cueros del interior que se van de contrabando por la frontera: {:.0} este mes", ap.contrabando));
        }
        flujos.sort_by(|a, b| b.carga.partial_cmp(&a.carga).unwrap());
        flujos.truncate(80);
        self.flujos = flujos;
        self.ultimo = ap;
        self.azar = rng.state();
        for n in &news {
            self.noticia(n.clone());
        }
        news
    }

    /// Estancias nuevas alrededor de los pueblos (el campo se puebla solo, una vez por año).
    fn estancias_nuevas(&mut self, rng: &mut Rng) -> Vec<String> {
        let g = geo();
        let anio = self.anio();
        let mut nuevas = 0;
        let pueblos: Vec<([f32; 2], f32)> = self.pueblos.iter().map(|p| (p.pos, p.poblacion)).collect();
        for (pos, pob) in pueblos {
            let cerca = self.estancias.iter().filter(|e| g.km(e.pos, pos) < 45.0).count();
            let quiere = ((pob / 150.0) as usize).clamp(1, 8);
            if cerca >= quiere || self.estancias.len() >= 400 {
                continue;
            }
            for _ in 0..12 {
                let ang = rng.f() * std::f32::consts::TAU;
                let r = 12.0 + rng.f() * 30.0;
                let p = [pos[0] + ang.cos() * r / g.g.km_x, pos[1] + ang.sin() * r / g.g.km_y];
                if g.es_tierra(p[0], p[1], anio) && g.pastura(p[0], p[1]) > 0.6 && !self.estancias.iter().any(|e| g.km(e.pos, p) < 9.0) {
                    let id = self.id();
                    let nombre = format!("Estancia de {}", APELLIDOS[(rng.next() % APELLIDOS.len() as u64) as usize]);
                    self.estancias.push(Estancia { id, nombre, pos: p, anio, ganado: 400.0, jugador: false, hist: None });
                    nuevas += 1;
                    break;
                }
            }
        }
        if nuevas > 0 {
            vec![format!("{nuevas} estancias nuevas en el campo ({} en todo el país)", self.estancias.len())]
        } else {
            Vec::new()
        }
    }

    /// Sequías y crecientes (ESTIMADO: frecuencias de diseño, más sequías en verano y más
    /// crecientes en otoño e invierno).
    fn eventos_del_mes(&mut self, rng: &mut Rng, sequia_capital: bool) -> Vec<String> {
        let g = geo();
        let mut news = Vec::new();
        for e in self.eventos.iter_mut() {
            e.meses = e.meses.saturating_sub(1);
        }
        let terminados: Vec<String> = self.eventos.iter().filter(|e| e.meses == 0).map(|e| match e.tipo {
            TipoEvento::Sequia => format!("Llovió en {}: termina la seca", e.zona),
            TipoEvento::Creciente => format!("Bajó el {}: los vados vuelven a dar paso", e.zona),
        }).collect();
        news.extend(terminados);
        self.eventos.retain(|e| e.meses > 0);
        let m = self.mes % 12;
        let verano = matches!(m, 0 | 1 | 11);
        let hay_seca = self.eventos.iter().any(|e| e.tipo == TipoEvento::Sequia);
        if !hay_seca && (rng.f() < if verano { 0.03 } else { 0.01 } || (sequia_capital && rng.f() < 0.3)) {
            // Región: un departamento al azar (con la capital en sequía, el sur).
            let d = if sequia_capital { g.departamentos.iter().find(|d| d.nombre == "Canelones").unwrap_or(&g.departamentos[0]) } else { &g.departamentos[(rng.next() % g.departamentos.len() as u64) as usize] };
            let meses = 4 + (rng.next() % 6) as u32;
            self.eventos.push(Evento { tipo: TipoEvento::Sequia, zona: d.nombre.clone(), centro: d.centro, radio: 110.0, meses });
            news.push(format!("Seca en {} y alrededores: se secan las aguadas y el ganado enflaquece", d.nombre));
        }
        let otoño = (3..=8).contains(&m);
        if !self.eventos.iter().any(|e| e.tipo == TipoEvento::Creciente) && rng.f() < if otoño { 0.06 } else { 0.02 } {
            const RIOS: [&str; 8] = ["Río Negro", "Río Santa Lucía", "Río Yí", "Río Uruguay", "Río Cebollatí", "Río Tacuarembó", "Río San José", "Río Queguay Grande"];
            let rio = RIOS[(rng.next() % RIOS.len() as u64) as usize];
            let centro = g.rios.iter().find(|r| r.nombre == rio).and_then(|r| r.lineas.first()).map(|l| l[l.len() / 2]).unwrap_or([250.0, 300.0]);
            let meses = 1 + (rng.next() % 2) as u32;
            self.eventos.push(Evento { tipo: TipoEvento::Creciente, zona: rio.to_string(), centro, radio: 0.0, meses });
            news.push(format!("Creciente del {rio}: los vados no dan paso y las carretas esperan"));
        }
        news
    }

    /// ¿El río está crecido este mes?
    pub fn crecido(&self, rio: &str) -> bool {
        self.eventos.iter().any(|e| e.tipo == TipoEvento::Creciente && e.zona == rio)
    }

    /// Costo de una acción (pesos), si se puede calcular.
    pub fn costo(&self, a: &Accion) -> i64 {
        match a {
            Accion::FundarPueblo(_) => COSTO_PUEBLO,
            Accion::Estancia(_) => COSTO_ESTANCIA,
            Accion::Saladero(_) => COSTO_SALADERO,
            Accion::Camino(a, b) => match (self.pueblo(a), self.pueblo(b)) {
                (Some(pa), Some(pb)) => (geo().km(pa.pos, pb.pos) * 1.25 * COSTO_KM_CAMINO) as i64,
                _ => 0,
            },
            Accion::MejorarCamino(id) => self.caminos.iter().find(|c| c.id == *id).map_or(0, |c| (c.km() * 10.0 * (c.nivel as f32 + 1.0)) as i64),
            Accion::Paso(id, k, obra) => self.caminos.iter().find(|c| c.id == *id).and_then(|c| c.pasos.get(*k)).map_or(0, |p| match obra {
                Obra::Puente => 1500 + (p.ancho * 1200.0) as i64,
                _ => COSTO_BALSA,
            }),
            Accion::Puerto(_) => COSTO_PUERTO,
        }
    }

    /// Hace una acción del jugador pagando con el tesoro de la ciudad. Devuelve el aviso.
    pub fn aplicar(&mut self, a: &Accion, dinero: &mut i64) -> Result<String, String> {
        let g = geo();
        let anio = self.anio();
        let epoca = self.epoca();
        permitido(a, epoca, anio)?;
        let costo = self.costo(a);
        if costo > *dinero {
            return Err(format!("{}: faltan fondos (cuesta {costo} pesos)", a.nombre()));
        }
        let msg = match a {
            Accion::FundarPueblo(p) => {
                if !g.es_tierra(p[0], p[1], anio) {
                    return Err("Ahí no hay tierra firme del país".into());
                }
                if let Some(o) = self.pueblos.iter().find(|q| g.km(q.pos, *p) < 15.0) {
                    return Err(format!("Muy cerca de {} (a menos de 15 km)", o.nombre));
                }
                let n = self.pueblos.iter().filter(|q| q.jugador).count();
                let nombre = NOMBRES_PUEBLO[n % NOMBRES_PUEBLO.len()].to_string();
                let id = format!("j{}", self.id());
                self.pueblos.push(Pueblo { id, nombre: nombre.clone(), pos: *p, anio, tipo: "pueblo".into(), poblacion: 150.0, jugador: true });
                format!("Se funda el pueblo de {nombre}")
            }
            Accion::Estancia(p) => {
                if !g.es_tierra(p[0], p[1], anio) {
                    return Err("Ahí no hay tierra firme del país".into());
                }
                if g.pastura(p[0], p[1]) < 0.35 {
                    return Err("Mal campo para el ganado (bañado, monte cerrado o sierra)".into());
                }
                if self.estancias.iter().any(|e| g.km(e.pos, *p) < 6.0) {
                    return Err("Ya hay una estancia a menos de 6 km".into());
                }
                let id = self.id();
                let nombre = format!("Estancia de {}", APELLIDOS[(id as usize * 7) % APELLIDOS.len()]);
                self.estancias.push(Estancia { id, nombre: nombre.clone(), pos: *p, anio, ganado: 500.0, jugador: true, hist: None });
                format!("Merced de tierras: {nombre} (una legua cuadrada de campo)")
            }
            Accion::Saladero(p) => {
                if !g.es_tierra(p[0], p[1], anio) {
                    return Err("Ahí no hay tierra firme del país".into());
                }
                // Cerca del agua: arroyo o río para lavar y embarcar.
                let agua = (-3..=3).any(|dx| (-3..=3).any(|dz| {
                    let (c, f) = (p[0] + dx as f32, p[1] + dz as f32);
                    g.es_agua(c, f, anio) || g.capa(c, f, datos::C_RIO) > 40
                }));
                if !agua {
                    return Err("El saladero necesita un río o la costa a menos de 3 km".into());
                }
                let id = self.id();
                let nombre = format!("Saladero {}", APELLIDOS[(id as usize * 5) % APELLIDOS.len()]);
                self.saladeros.push(Saladero { id, nombre: nombre.clone(), pos: *p, anio, capacidad: 250.0, faena: 0.0, jugador: true, hist: None });
                format!("Abre el {nombre}")
            }
            Accion::Camino(a, b) => {
                let (Some(pa), Some(pb)) = (self.pueblo(a), self.pueblo(b)) else { return Err("Elige dos pueblos".into()) };
                if a == b {
                    return Err("Elige dos pueblos distintos".into());
                }
                if self.caminos.iter().any(|c| (c.desde == *a && c.hasta == *b) || (c.desde == *b && c.hasta == *a)) {
                    return Err("Ya hay un camino entre esos pueblos".into());
                }
                let nombre = format!("Camino de {} a {}", pa.nombre, pb.nombre);
                let puntos = rutas::trazar(pa.pos, pb.pos, anio);
                if puntos.len() < 2 {
                    return Err("No se encontró un trazado".into());
                }
                let pasos = rutas::pasos_de(&puntos, anio);
                let id = self.id();
                let km = largo(&puntos) * g.g.km_x;
                let np = pasos.len();
                self.caminos.push(Camino { id, nombre: nombre.clone(), desde: a.clone(), hasta: b.clone(), puntos, anio, nivel: 1, pasos, jugador: true });
                format!("{nombre}: {km:.0} km, {np} pasos de río")
            }
            Accion::MejorarCamino(id) => {
                let max = nivel_max(epoca);
                let c = self.caminos.iter_mut().find(|c| c.id == *id).ok_or("No hay tal camino")?;
                if c.nivel >= max {
                    return Err(format!("En esta época el camino no se puede mejorar más ({})", NIVELES[c.nivel as usize]));
                }
                c.nivel += 1;
                format!("{}: ahora {}", c.nombre, NIVELES[c.nivel as usize])
            }
            Accion::Paso(id, k, obra) => {
                let c = self.caminos.iter_mut().find(|c| c.id == *id).ok_or("No hay tal camino")?;
                let p = c.pasos.get_mut(*k).ok_or("No hay tal paso")?;
                if p.obra == *obra || (p.obra == Obra::Puente) {
                    return Err(format!("El paso del {} ya tiene {}", p.rio, p.obra.nombre()));
                }
                p.obra = *obra;
                format!("{} en el paso del {}", if *obra == Obra::Puente { "Puente" } else { "Balsa" }, p.rio)
            }
            Accion::Puerto(id) => {
                let p = self.pueblo(id).ok_or("No hay tal pueblo")?;
                if self.puertos.iter().any(|x| x.pueblo == *id) {
                    return Err(format!("{} ya tiene puerto", p.nombre));
                }
                let navegable = g.rio_cerca(p.pos, 4.0).is_some_and(|(r, _)| r.navegable) || (-4..=4).any(|dx| (-4..=4).any(|dz| g.capa(p.pos[0] + dx as f32, p.pos[1] + dz as f32, datos::C_AGUA) > 200));
                if !navegable {
                    return Err(format!("{} no está sobre un río navegable ni sobre la costa", p.nombre));
                }
                let nombre = p.nombre.clone();
                self.puertos.push(Puerto { pueblo: id.clone(), desde: anio, jugador: true });
                format!("Puerto de {nombre}: los barcos llevan las cargas a Montevideo")
            }
        };
        *dinero -= costo;
        self.noticia(msg.clone());
        Ok(msg)
    }

    /// Población del interior (sin la capital).
    pub fn poblacion_interior(&self) -> f32 {
        self.pueblos.iter().filter(|p| p.tipo != "capital").map(|p| p.poblacion).sum()
    }

    pub fn ganado(&self) -> f32 {
        self.estancias.iter().map(|e| e.ganado).sum()
    }
}

/// Población de la capital para simular el país sin la ciudad (partidas viejas y pruebas):
/// del orden de los padrones de Montevideo (ESTIMADO).
pub fn poblacion_capital_estimada(anio: i32) -> f32 {
    let t = (anio - 1726).max(0) as f32;
    (300.0 * (1.0 + t * 0.06)).min(60000.0)
}
