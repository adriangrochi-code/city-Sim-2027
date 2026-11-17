//! Materiales de construcción: cada edificio pide una cantidad de materiales y se levanta con
//! **lo que hay**. Si falta el material preferido se usa el mejor que haya a mano, y en el peor
//! caso barro y paja, que siempre están: construir nunca se prohíbe. El edificio guarda con qué
//! se hizo ([`Obra`]) y de eso dependen el fuego, lo que dura, la humedad (salud), el prestigio
//! (valor del suelo), cuántos pisos aguanta y el mantenimiento. También se ve en los vóxeles:
//! una casa sin cal queda de ladrillo sin revoque; una con solo madera, de tablas.
//!
//! El diseño y las fuentes están en `docs/sistemas/materiales-de-construccion.md`.
//!
//! Frontera con el catálogo de minerales (`recursos`, en otra rama): este módulo solo pregunta
//! dos cosas a través de [`Disponible`] y [`FuenteLocal`]: cuánto hay en depósito de cada
//! material y cuánto rinde el entorno por mes. Hoy salen de las existencias de la economía
//! (`Res::Ladrillos`, `Res::Piedra`, `Res::Cueros`), del [`Almacen`] propio de este módulo y de
//! las capas de arcilla, arena y piedra de los solares; mañana pueden salir de
//! `Recursos::materiales_cerca(p, radio)` usando la clave [`Material::clave`] (`construccion`).

pub mod ciudad;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

// --- Materiales ------------------------------------------------------------------------------

/// Un material de construcción. Las cantidades van en "unidades de obra" (las mismas que los
/// ladrillos de la economía: una carretada chica).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Material {
    /// Barro del solar: gratis y siempre a mano.
    Barro,
    /// Paja de los bañados y pajonales.
    Paja,
    /// Adobe: barro y paja moldeados y secados al sol.
    Adobe,
    /// Cuero vacuno (techos de ranchos, puertas, corrales).
    Cuero,
    /// Madera de obra: tirantes, tablas, puertas.
    Madera,
    /// Ladrillo cocido en horno.
    Ladrillo,
    /// Teja cocida en horno.
    Teja,
    /// Piedra de cantera.
    Piedra,
    /// Cal (mortero, revoque y blanqueo).
    Cal,
    /// Arena (mortero).
    Arena,
    /// Hierro: rejas, clavos, herrajes.
    Hierro,
    /// Vidrio de ventana.
    Vidrio,
}

pub const N: usize = 12;

impl Material {
    pub const TODOS: [Material; N] = [Material::Barro, Material::Paja, Material::Adobe, Material::Cuero, Material::Madera, Material::Ladrillo, Material::Teja, Material::Piedra, Material::Cal, Material::Arena, Material::Hierro, Material::Vidrio];

    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn nombre(self) -> &'static str {
        DATOS[self.idx()].nombre
    }

    /// Clave común con el catálogo de minerales (`construccion`): la frontera para
    /// conectarlo (ver el comentario del módulo).
    #[allow(dead_code)]
    pub fn clave(self) -> &'static str {
        DATOS[self.idx()].clave
    }

    #[allow(dead_code)]
    pub fn de_clave(s: &str) -> Option<Material> {
        Material::TODOS.into_iter().find(|m| m.clave() == s)
    }

    pub fn datos(self) -> &'static DatosMaterial {
        &DATOS[self.idx()]
    }

    /// Siempre se consigue en el lugar sin contarlo (el barro del solar). La paja se cuenta
    /// (si se acaba, los ranchos se techan con cuero), pero el techo de paja es el último
    /// recurso y nunca se niega.
    pub fn siempre_local(self) -> bool {
        matches!(self, Material::Barro)
    }

    /// Vive en las existencias de la economía (con su ficha propia en la barra).
    pub fn de_la_economia(self) -> Option<crate::city::economy::Res> {
        use crate::city::economy::Res;
        match self {
            Material::Ladrillo => Some(Res::Ladrillos),
            Material::Piedra => Some(Res::Piedra),
            Material::Cuero => Some(Res::Cueros),
            _ => None,
        }
    }

    /// Se trae por el puerto cuando falta.
    pub fn importable(self) -> bool {
        DATOS[self.idx()].importa.is_some()
    }

    /// Aviso de la gaceta cuando empieza a faltar.
    pub fn aviso_falta(self) -> &'static str {
        match self {
            Material::Cal => "Falta cal: las casas nuevas se levantan sin revoque y no hay azoteas",
            Material::Ladrillo => "Faltan ladrillos: las casas nuevas se levantan de adobe o de tablas",
            Material::Teja => "Falta teja: los techos nuevos son de tablas, de paja o de cuero",
            Material::Madera => "Falta madera: sin tirantes no hay techos de teja ni azoteas",
            Material::Piedra => "Falta piedra: las casas de altos se hacen de ladrillo o esperan",
            Material::Hierro => "Falta hierro: las ventanas nuevas no llevan rejas",
            Material::Adobe => "Falta adobe: los ranchos nuevos se levantan con barro y paja",
            Material::Cuero => "Faltan cueros para techar los ranchos",
            Material::Arena => "Falta arena para el mortero",
            Material::Vidrio => "Falta vidrio: las ventanas nuevas quedan con postigos",
            Material::Barro | Material::Paja => "",
        }
    }
}

/// Ficha de un material: precio, de dónde sale y si se importa.
pub struct DatosMaterial {
    pub nombre: &'static str,
    pub clave: &'static str,
    /// Precio de referencia por unidad de obra (pesos, el mundo en calma). Estimación.
    pub precio: f32,
    /// De dónde sale en la ciudad colonial.
    pub origen: &'static str,
    /// De dónde llega por el puerto (None = no se importa).
    pub importa: Option<&'static str>,
    /// Existencias de la fundación (lo que trajeron los primeros barcos). Estimación.
    pub inicial: f32,
}

/// Tabla de los materiales (los precios y cantidades son estimaciones de diseño; la base
/// histórica está en el documento).
pub const DATOS: [DatosMaterial; N] = [
    DatosMaterial { nombre: "barro", clave: "barro", precio: 0.0, origen: "El suelo del mismo solar.", importa: None, inicial: 0.0 },
    DatosMaterial { nombre: "paja", clave: "paja", precio: 0.1, origen: "Pajonales y bañados del ejido (paja brava, totora).", importa: None, inicial: 600.0 },
    DatosMaterial { nombre: "adobe", clave: "adobe", precio: 0.3, origen: "Barro con paja moldeado y secado al sol, junto a los arroyos; no seca con lluvia ni frío.", importa: None, inicial: 60.0 },
    DatosMaterial { nombre: "cuero", clave: "cuero", precio: 2.0, origen: "Las estancias (los mismos cueros que se exportan).", importa: None, inicial: 0.0 },
    DatosMaterial { nombre: "madera", clave: "madera", precio: 1.6, origen: "Casi no hay monte de obra cerca: algo de palos del monte criollo; los tirantes vienen del Paraguay y Corrientes por Buenos Aires, o del Brasil.", importa: Some("Paraguay y Corrientes (por Buenos Aires), Brasil"), inicial: 120.0 },
    DatosMaterial { nombre: "ladrillo", clave: "ladrillo", precio: 1.0, origen: "Hornos de ladrillos junto al barro de los arroyos y la costa.", importa: Some("Buenos Aires"), inicial: 0.0 },
    DatosMaterial { nombre: "teja", clave: "teja", precio: 1.2, origen: "Los mismos hornos de ladrillos (y los talleres).", importa: Some("Buenos Aires"), inicial: 0.0 },
    DatosMaterial { nombre: "piedra", clave: "piedra", precio: 0.6, origen: "Canteras sobre el basamento de granito y gneis.", importa: Some("Buenos Aires"), inicial: 0.0 },
    DatosMaterial { nombre: "cal", clave: "cal", precio: 2.0, origen: "No hay piedra caliza en el mapa: llega de las caleras de la Banda Oriental (Colonia) o de ultramar.", importa: Some("caleras de Colonia, Buenos Aires"), inicial: 30.0 },
    DatosMaterial { nombre: "arena", clave: "arena", precio: 0.15, origen: "Playas, médanos y orillas de los arroyos.", importa: None, inicial: 80.0 },
    DatosMaterial { nombre: "hierro", clave: "hierro", precio: 6.0, origen: "No se funde en el Río de la Plata: viene de España (Vizcaya) en los barcos de registro.", importa: Some("España"), inicial: 8.0 },
    DatosMaterial { nombre: "vidrio", clave: "vidrio", precio: 8.0, origen: "Se importa de Europa; en la colonia, raro y caro.", importa: Some("Europa"), inicial: 0.0 },
];

// --- Partes del edificio -----------------------------------------------------------------

/// Con qué se levantan las paredes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Pared {
    Piedra,
    Ladrillo,
    Adobe,
    Tablas,
    /// Barro y paja (terrón, chorizo o quincha): siempre se puede.
    Barro,
}

/// Con qué se techa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Techo {
    Teja,
    /// Azotea: tirantes de madera, ladrillos y cal (plana, transitable).
    Azotea,
    Tablas,
    /// Paja: siempre se puede.
    Paja,
    Cuero,
}

/// Propiedades de una parte (pared o techo). Todas son estimaciones de diseño.
#[derive(Clone, Copy, Debug)]
pub struct Cualidad {
    /// Multiplica la probabilidad de que se prenda (1 = normal).
    pub fuego: f32,
    /// Facilidad para que le salte el fuego de un vecino (0 a 1).
    pub contagio: f32,
    /// Años que dura sin arreglos.
    pub vida: f32,
    /// Humedad que deja adentro (0 a 1): enferma.
    pub humedad: f32,
    /// Prestigio (0 a 1): sube el valor del suelo y el alquiler.
    pub prestigio: f32,
}

impl Pared {
    pub const TODAS: [Pared; 5] = [Pared::Piedra, Pared::Ladrillo, Pared::Adobe, Pared::Tablas, Pared::Barro];

    pub fn nombre(self) -> &'static str {
        match self {
            Pared::Piedra => "piedra",
            Pared::Ladrillo => "ladrillo",
            Pared::Adobe => "adobe",
            Pared::Tablas => "tablas",
            Pared::Barro => "barro y paja",
        }
    }

    pub fn cualidad(self) -> Cualidad {
        match self {
            Pared::Piedra => Cualidad { fuego: 0.8, contagio: 0.0, vida: 150.0, humedad: 0.2, prestigio: 1.0 },
            Pared::Ladrillo => Cualidad { fuego: 0.9, contagio: 0.0, vida: 80.0, humedad: 0.15, prestigio: 0.7 },
            Pared::Adobe => Cualidad { fuego: 1.0, contagio: 0.0, vida: 30.0, humedad: 0.4, prestigio: 0.3 },
            Pared::Tablas => Cualidad { fuego: 1.8, contagio: 0.35, vida: 20.0, humedad: 0.25, prestigio: 0.25 },
            Pared::Barro => Cualidad { fuego: 1.1, contagio: 0.05, vida: 10.0, humedad: 0.6, prestigio: 0.0 },
        }
    }

    /// Pisos que aguanta: los altos piden ladrillo o piedra asentados con cal.
    pub fn pisos_max(self, con_cal: bool) -> u8 {
        match self {
            Pared::Piedra | Pared::Ladrillo if con_cal => 2,
            _ => 1,
        }
    }

    /// Lo que pide por módulo de edificio (un módulo ≈ una casa de una planta de 60 m²).
    pub fn receta(self) -> &'static [(Material, f32)] {
        match self {
            Pared::Piedra => &[(Material::Piedra, 30.0), (Material::Arena, 4.0)],
            Pared::Ladrillo => &[(Material::Ladrillo, 30.0), (Material::Arena, 3.0)],
            Pared::Adobe => &[(Material::Adobe, 30.0)],
            Pared::Tablas => &[(Material::Madera, 14.0)],
            Pared::Barro => &[(Material::Barro, 30.0), (Material::Paja, 4.0)],
        }
    }

    /// ¿Se la lleva el agua de una inundación?
    pub fn se_lava(self) -> bool {
        matches!(self, Pared::Barro | Pared::Adobe)
    }

    /// Admite revoque y blanqueo de cal.
    pub fn admite_revoque(self) -> bool {
        !matches!(self, Pared::Tablas)
    }
}

impl Techo {
    pub const TODOS: [Techo; 5] = [Techo::Teja, Techo::Azotea, Techo::Tablas, Techo::Paja, Techo::Cuero];

    pub fn nombre(self) -> &'static str {
        match self {
            Techo::Teja => "teja",
            Techo::Azotea => "azotea",
            Techo::Tablas => "tablas",
            Techo::Paja => "paja",
            Techo::Cuero => "cuero",
        }
    }

    pub fn cualidad(self) -> Cualidad {
        match self {
            Techo::Teja => Cualidad { fuego: 0.2, contagio: 0.25, vida: 50.0, humedad: 0.0, prestigio: 0.6 },
            Techo::Azotea => Cualidad { fuego: 0.07, contagio: 0.08, vida: 40.0, humedad: 0.05, prestigio: 0.8 },
            Techo::Tablas => Cualidad { fuego: 0.7, contagio: 0.6, vida: 15.0, humedad: 0.1, prestigio: 0.2 },
            Techo::Paja => Cualidad { fuego: 1.0, contagio: 1.0, vida: 6.0, humedad: 0.15, prestigio: 0.0 },
            Techo::Cuero => Cualidad { fuego: 0.45, contagio: 0.4, vida: 4.0, humedad: 0.2, prestigio: 0.05 },
        }
    }

    pub fn receta(self) -> &'static [(Material, f32)] {
        match self {
            Techo::Teja => &[(Material::Teja, 12.0), (Material::Madera, 3.0)],
            Techo::Azotea => &[(Material::Ladrillo, 12.0), (Material::Cal, 3.0), (Material::Madera, 4.0)],
            Techo::Tablas => &[(Material::Madera, 8.0)],
            Techo::Paja => &[(Material::Paja, 10.0)],
            Techo::Cuero => &[(Material::Cuero, 6.0)],
        }
    }
}

/// Cal del revoque y del mortero, por módulo.
pub const CAL_REVOQUE: f32 = 3.0;
/// Hierro de las rejas, por módulo.
pub const HIERRO_REJAS: f32 = 1.0;

// --- Planes y obras ------------------------------------------------------------------------

/// Lo que se quiere construir: tamaño y preferencias en orden.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Módulos de obra (1 = rancho; 2 = casa de una planta; 4 = casa de altos).
    pub modulos: f32,
    pub paredes: Vec<Pared>,
    pub techos: Vec<Techo>,
    /// Quiere revoque de cal.
    pub revoque: bool,
    /// Quiere rejas de hierro.
    pub rejas: bool,
    /// Pisos que tiene que aguantar.
    pub pisos: u8,
}

/// Con qué quedó hecho un edificio, cómo está y qué le faltó.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Obra {
    pub paredes: Pared,
    pub techo: Techo,
    /// Revocado y blanqueado con cal (o asentado con mortero de cal).
    pub revoque: bool,
    pub rejas: bool,
    /// Estado (1 = nuevo; con 0 se viene abajo).
    pub estado: f32,
    /// Materiales que faltaron al levantarlo (bits de [`Material`]).
    #[serde(default)]
    pub faltas: u16,
}

impl Obra {
    pub fn nueva(paredes: Pared, techo: Techo, revoque: bool, rejas: bool) -> Obra {
        Obra { paredes, techo, revoque, rejas, estado: 1.0, faltas: 0 }
    }

    pub fn falto(&self, m: Material) -> bool {
        self.faltas & (1 << m.idx()) != 0
    }

    pub fn faltantes(&self) -> Vec<Material> {
        Material::TODOS.into_iter().filter(|&m| self.falto(m)).collect()
    }

    /// Multiplicador de la probabilidad de incendio (techo y paredes).
    pub fn fuego(&self) -> f32 {
        self.techo.cualidad().fuego * self.paredes.cualidad().fuego
    }

    /// Facilidad para que le salte el fuego de un vecino.
    pub fn contagio(&self) -> f32 {
        (self.techo.cualidad().contagio + self.paredes.cualidad().contagio).min(1.0)
    }

    /// Años que dura sin arreglos (manda la parte más débil; el revoque protege el adobe y el
    /// barro de la lluvia).
    pub fn vida(&self) -> f32 {
        let mut p = self.paredes.cualidad().vida;
        if self.revoque && matches!(self.paredes, Pared::Adobe | Pared::Barro) {
            p *= 1.5;
        }
        // El techo se cambia más seguido que las paredes: pesa con dos veces y media su vida.
        p.min(self.techo.cualidad().vida * 2.5)
    }

    /// Humedad que deja adentro (0 a 1).
    pub fn humedad(&self) -> f32 {
        let mut h = self.paredes.cualidad().humedad + self.techo.cualidad().humedad;
        if self.revoque {
            h *= 0.75;
        }
        h.clamp(0.0, 1.0)
    }

    /// Prestigio (0 a 1).
    pub fn prestigio(&self) -> f32 {
        let mut p = 0.6 * self.paredes.cualidad().prestigio + 0.3 * self.techo.cualidad().prestigio;
        if self.revoque {
            p += 0.07;
        }
        if self.rejas {
            p += 0.03;
        }
        p.clamp(0.0, 1.0)
    }

    pub fn pisos_max(&self) -> u8 {
        self.paredes.pisos_max(self.revoque)
    }

    /// "Paredes de adobe, techo de paja".
    pub fn nombre(&self) -> String {
        let rev = if self.revoque && self.paredes.admite_revoque() {
            if self.paredes == Pared::Piedra { " asentada con cal" } else { " encalado" }
        } else if self.paredes == Pared::Ladrillo {
            " sin revoque"
        } else {
            ""
        };
        let techo = match self.techo {
            Techo::Azotea => "azotea".to_string(),
            t => format!("techo de {}", t.nombre()),
        };
        format!("Paredes de {}{rev}, {techo}{}", self.paredes.nombre(), if self.rejas { ", rejas de hierro" } else { "" })
    }

    /// Lo que eso significa: "arde fácil, húmeda; dura ~15 años".
    pub fn efectos(&self) -> String {
        let f = self.fuego();
        let fuego = if f >= 0.7 { "arde fácil" } else if f >= 0.15 { "arde poco" } else { "casi no arde" };
        let h = self.humedad();
        let hum = if h >= 0.5 { "muy húmeda" } else if h >= 0.3 { "húmeda" } else { "seca" };
        let años = (self.vida() / 5.0).round() * 5.0;
        format!("{fuego}, {hum}; dura ~{} años sin arreglos", años.max(5.0) as i32)
    }
}

// --- Existencias -----------------------------------------------------------------------------

/// Cuánto hay de cada material y de dónde se saca: la frontera con la economía y con el
/// catálogo de minerales. [`ciudad`] la implementa con las existencias de la ciudad.
pub trait Disponible {
    /// Existencias a mano ahora.
    fn hay(&self, m: Material) -> f32;
}

/// Lo que se puede sacar o traer por mes de cada material en el lugar.
pub trait FuenteLocal {
    /// Rinde local por mes (unidades de obra): extracción del entorno y producción de los
    /// hornos y canteras. Hoy se calcula con las existencias y las capas de los solares; el
    /// catálogo de minerales puede reemplazarlo con `Recursos::materiales_cerca`.
    fn disponibilidad_local(&self, m: Material) -> f32;
}

/// Un depósito que se puede consumir.
pub trait Almacenable: Disponible {
    /// Saca hasta `n` y devuelve lo sacado.
    fn tomar(&mut self, m: Material, n: f32) -> f32;
}

/// Existencias simples (para pruebas y para lo que no está en la economía).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Existencia {
    pub cantidad: f32,
    pub precio: f32,
    /// El último mes cerrado.
    pub producido: f32,
    pub importado: f32,
    pub usado: f32,
    /// Lo que pidieron las obras (lo haya habido o no).
    pub pedido: f32,
    /// Lo que se pidió y no había.
    pub faltante: f32,
    /// Obras que se hicieron con otra cosa por falta de este material.
    pub sustituciones: u32,
}

impl Disponible for [f32; N] {
    fn hay(&self, m: Material) -> f32 {
        if m == Material::Barro { f32::INFINITY } else { self[m.idx()] }
    }
}

impl Almacenable for [f32; N] {
    fn tomar(&mut self, m: Material, n: f32) -> f32 {
        if m == Material::Barro {
            return n;
        }
        let t = n.min(self[m.idx()]).max(0.0);
        self[m.idx()] -= t;
        t
    }
}

/// Elige con qué se hace un edificio según lo que hay: para paredes y techo, la primera
/// preferencia cuya receta entera esté en depósito (barro y paja siempre). No consume nada.
pub fn elegir(plan: &Plan, d: &dyn Disponible) -> Obra {
    let k = plan.modulos.max(0.1);
    let mut reservado = [0.0f32; N];
    let alcanza = |receta: &[(Material, f32)], reservado: &[f32; N]| receta.iter().all(|&(m, q)| m.siempre_local() || d.hay(m) - reservado[m.idx()] >= q * k - 1e-3);
    let reservar = |receta: &[(Material, f32)], reservado: &mut [f32; N]| {
        for &(m, q) in receta {
            reservado[m.idx()] += q * k;
        }
    };
    let mut faltas = 0u16;
    let marcar = |receta: &[(Material, f32)], reservado: &[f32; N], faltas: &mut u16| {
        for &(m, q) in receta {
            if !m.siempre_local() && d.hay(m) - reservado[m.idx()] < q * k - 1e-3 {
                *faltas |= 1 << m.idx();
            }
        }
    };
    // Revoque: hace falta cal para el mortero de los altos y para blanquear.
    let cal = plan.revoque && d.hay(Material::Cal) >= CAL_REVOQUE * k;
    if plan.revoque && !cal {
        faltas |= 1 << Material::Cal.idx();
    }
    // Lo que falta se anota solo de la primera preferencia (lo que de verdad se quería).
    let mut paredes = Pared::Barro;
    let mut primera = true;
    for &p in &plan.paredes {
        if p.pisos_max(cal) < plan.pisos {
            continue;
        }
        if p == Pared::Barro || alcanza(p.receta(), &reservado) {
            paredes = p;
            break;
        }
        if primera {
            marcar(p.receta(), &reservado, &mut faltas);
            primera = false;
        }
    }
    reservar(paredes.receta(), &mut reservado);
    let revoque = cal && paredes.admite_revoque();
    if revoque {
        reservado[Material::Cal.idx()] += CAL_REVOQUE * k;
    }
    // Si nada alcanza, paja igual (cortada a mano en el bañado): techar nunca se niega.
    let mut techo = Techo::Paja;
    for (i, &t) in plan.techos.iter().enumerate() {
        if alcanza(t.receta(), &reservado) {
            techo = t;
            break;
        }
        if i == 0 {
            marcar(t.receta(), &reservado, &mut faltas);
        }
    }
    reservar(techo.receta(), &mut reservado);
    let rejas = plan.rejas && d.hay(Material::Hierro) - reservado[Material::Hierro.idx()] >= HIERRO_REJAS * k;
    if plan.rejas && !rejas {
        faltas |= 1 << Material::Hierro.idx();
    }
    let mut o = Obra::nueva(paredes, techo, revoque, rejas);
    o.faltas = faltas;
    o
}

/// Todo lo que pide una obra ya elegida (para consumirlo o mostrarlo).
pub fn cantidades(o: &Obra, modulos: f32) -> Vec<(Material, f32)> {
    let mut v: Vec<(Material, f32)> = Vec::new();
    let mut sumar = |m: Material, q: f32| match v.iter_mut().find(|x| x.0 == m) {
        Some(x) => x.1 += q,
        None => v.push((m, q)),
    };
    for &(m, q) in o.paredes.receta().iter().chain(o.techo.receta()) {
        sumar(m, q * modulos);
    }
    if o.revoque {
        sumar(Material::Cal, CAL_REVOQUE * modulos);
    }
    if o.rejas {
        sumar(Material::Hierro, HIERRO_REJAS * modulos);
    }
    v
}

/// Lo que pediría el plan si hubiera de todo (la primera preferencia): la demanda.
pub fn pedido(plan: &Plan) -> Vec<(Material, f32)> {
    let ideal = Obra::nueva(plan.paredes.first().copied().unwrap_or(Pared::Barro), plan.techos.first().copied().unwrap_or(Techo::Paja), plan.revoque, plan.rejas);
    cantidades(&ideal, plan.modulos)
}

/// Saca del depósito lo que pide la obra (lo que no alcanza de barro y paja se junta a mano).
pub fn consumir(o: &Obra, modulos: f32, a: &mut dyn Almacenable) {
    for (m, q) in cantidades(o, modulos) {
        a.tomar(m, q);
    }
}
