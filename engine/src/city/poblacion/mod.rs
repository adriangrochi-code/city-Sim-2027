//! Población: cada habitante es una persona con nombre, familia, personalidad e historia de
//! vida; los hogares tienen su propia economía. La simulación es mensual (nacimientos,
//! muertes, casamientos, herencias, migraciones, trabajo, vivienda y felicidad) y se conecta
//! con la ciudad por dos estructuras planas: [`Entorno`] (lo que la ciudad le cuenta cada mes:
//! solares, precios, agua, llegadas, ordenanzas) e [`InformeMes`] (lo que le devuelve: gente
//! sin casa, ánimo por sector, mejoras de casas, líneas del presupuesto y noticias).
//!
//! Diseño, calibración y la decisión de no representar la esclavitud:
//! `docs/sistemas/poblacion.md`.
//!
//! Para otros sistemas:
//! - agentes móviles: [`Poblacion::habitantes_de_lote`], [`Poblacion::lugar_de_trabajo`],
//!   [`Poblacion::persona`];
//! - servicios (agua, incendios, puerto, epidemias): [`Poblacion::choque_epidemia`],
//!   [`Poblacion::choque_incendio`], [`Poblacion::choque_inundacion`],
//!   [`Poblacion::choque_hambruna`].

// Parte de la API es para otros sistemas (agentes móviles, servicios) que todavía no la usan.
#![allow(dead_code)]

pub mod demografia;
pub mod economia;
pub mod estadisticas;
pub mod felicidad;
pub mod modelo;
pub mod nombres;
pub mod origenes;
pub mod politicas;
pub mod simulacion;
pub mod textos;

#[cfg(test)]
mod tests;

use super::economy::Ordinance;
use super::lots::ZoneKind;
use crate::buildings::Rng;
pub use estadisticas::{Estadisticas, Foto};
pub use modelo::*;
pub use origenes::Epoca;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// Mes absoluto (meses desde enero de 1726) → año.
pub fn año_de(mes: i32) -> i32 {
    super::START_YEAR as i32 + mes.div_euclid(12)
}

/// Un solar tal como lo ve la población.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoteInfo {
    pub id: LoteId,
    pub tipo: Option<ZoneKind>,
    pub nivel: u8,
    /// Habitantes que caben (casas) y empleos (comercios y talleres).
    pub capacidad: i32,
    pub empleos: i32,
    /// Valor del suelo (0 a 1) y distancia a la plaza (m).
    pub valor: f32,
    pub distancia: f32,
}

/// Solares del entorno indexados por id (búsqueda directa, sin tabla hash).
pub struct Lotes<'a> {
    por_id: Vec<Option<&'a LoteInfo>>,
}

impl<'a> Lotes<'a> {
    pub fn de(ent: &'a Entorno) -> Lotes<'a> {
        let max = ent.lotes.iter().map(|l| l.id as usize + 1).max().unwrap_or(0);
        let mut por_id = vec![None; max];
        for l in &ent.lotes {
            por_id[l.id as usize] = Some(l);
        }
        Lotes { por_id }
    }
    pub fn get(&self, id: &LoteId) -> Option<&'a LoteInfo> {
        self.por_id.get(*id as usize).copied().flatten()
    }
}

impl<'a> std::ops::Index<&LoteId> for Lotes<'a> {
    type Output = LoteInfo;
    fn index(&self, id: &LoteId) -> &LoteInfo {
        self.get(id).unwrap()
    }
}

/// Escuela, salud, insalubridad y delito de un solar (los calcula `services::barrio`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ServiciosLote {
    /// Acceso a una escuela (0 a 1: lugar por lo que rinde).
    pub escuela: f32,
    /// Cobertura de botica, médico y hospital (0 a 1).
    pub salud: f32,
    /// Agua mala, pozos negros y basurales (0 a 1).
    pub insalubre: f32,
    /// Riesgo de delito (0 a 1) y vigilancia (0 a 1).
    pub delito: f32,
    pub vigilancia: f32,
    /// Olor, ruido, humo y agua sucia de la industria cercana (0 a 1; ver `city::recursos`).
    pub molestia: f32,
}

/// Lo que la ciudad le cuenta a la población cada mes.
#[derive(Clone, Debug, Default)]
pub struct Entorno {
    pub mes: i32,
    pub lotes: Vec<LoteInfo>,
    pub precio_pan: f32,
    pub precio_carne: f32,
    /// Fracción de casas con agua (0 a 1).
    pub agua: f32,
    pub epidemia: bool,
    pub sequia: bool,
    /// Personas que llegan este mes (las decide la ciudad según su atractivo).
    pub llegadas: u32,
    /// Tasa de la contribución de los vecinos (%).
    pub contribucion: i32,
    pub ordenanzas: Vec<Ordinance>,
    /// ¿Hay iglesia matriz (plaza fundada)? Pesa en la felicidad de los devotos.
    pub iglesia: bool,
    /// Escuela, salud, insalubridad y delito de cada solar (por id de solar). Si está vacío,
    /// la población funciona sin edificios: las ordenanzas de escuelas y hospital actúan en
    /// toda la ciudad (la ciudad sintética de las pruebas).
    pub servicios: Vec<ServiciosLote>,
    /// ¿La ciudad tiene el modelo de edificios de escuelas, salud y seguridad?
    pub con_servicios: bool,
}

impl Entorno {
    pub fn vigente(&self, o: Ordinance) -> bool {
        self.ordenanzas.contains(&o)
    }

    /// Servicios del solar de un hogar (ceros si no vive en un solar).
    pub fn servicios_de(&self, lote: Option<LoteId>) -> ServiciosLote {
        lote.and_then(|l| self.servicios.get(l as usize)).copied().unwrap_or_default()
    }

    /// Servicios del solar donde vive un hogar.
    pub(crate) fn servicios_hogar(&self, h: Option<&Hogar>) -> ServiciosLote {
        self.servicios_de(h.filter(|h| h.vive_en_solar()).and_then(|h| h.lote))
    }
}

/// Lo que la población le devuelve a la ciudad cada mes.
#[derive(Clone, Debug, Default)]
pub struct InformeMes {
    pub noticias: Vec<String>,
    /// Personas sin casa más las que sobran en casas hacinadas: la demanda de vivienda.
    pub sin_casa: i32,
    /// Solares cuyos dueños juntaron para mejorar la casa (solar, nivel pedido).
    pub mejoras: Vec<(LoteId, u8)>,
    /// Felicidad media (0 a 1) de vecinos, comerciantes y hacendados.
    pub animo: [f32; 3],
    /// Líneas del presupuesto del cabildo (positivas = ingreso).
    pub lineas: Vec<(String, i64)>,
}

/// Contadores del mes y del año en curso.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Contadores {
    pub nacimientos: u32,
    pub muertes: u32,
    pub muertes_infantiles: u32,
    pub casamientos: u32,
    pub llegados: u32,
    pub emigrados: u32,
}

impl Contadores {
    fn sumar(&mut self, o: &Contadores) {
        self.nacimientos += o.nacimientos;
        self.muertes += o.muertes;
        self.muertes_infantiles += o.muertes_infantiles;
        self.casamientos += o.casamientos;
        self.llegados += o.llegados;
        self.emigrados += o.emigrados;
    }
}

/// Índice compacto de clave → lista de ids (claves ordenadas, búsqueda binaria; todo en tres
/// vectores, sin un `Vec` por clave): rápido de rehacer con cientos de miles de personas.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Indice {
    claves: Vec<u32>,
    inicio: Vec<u32>,
    valores: Vec<u32>,
}

impl Indice {
    /// Arma el índice con pares (clave, valor); los valores de una clave quedan en el orden
    /// en que llegaron.
    pub fn construir(mut pares: Vec<(u32, u32)>) -> Indice {
        pares.sort_by_key(|p| p.0);
        let mut ix = Indice { claves: Vec::new(), inicio: Vec::new(), valores: Vec::with_capacity(pares.len()) };
        for (k, v) in pares {
            if ix.claves.last() != Some(&k) {
                ix.claves.push(k);
                ix.inicio.push(ix.valores.len() as u32);
            }
            ix.valores.push(v);
        }
        ix
    }
    fn rango(&self, i: usize) -> &[u32] {
        let a = self.inicio[i] as usize;
        let b = self.inicio.get(i + 1).map_or(self.valores.len(), |&x| x as usize);
        &self.valores[a..b]
    }
    pub fn get(&self, k: &u32) -> Option<&[u32]> {
        self.claves.binary_search(k).ok().map(|i| self.rango(i))
    }
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &[u32])> {
        self.claves.iter().enumerate().map(|(i, k)| (k, self.rango(i)))
    }
    pub fn len(&self) -> usize {
        self.claves.len()
    }
    pub fn is_empty(&self) -> bool {
        self.claves.is_empty()
    }
}

impl<'a> IntoIterator for &'a Indice {
    type Item = (&'a u32, &'a [u32]);
    type IntoIter = Box<dyn Iterator<Item = (&'a u32, &'a [u32])> + 'a>;
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

/// Mapa denso por id de solar (los ids de los solares son chicos y consecutivos): un vector
/// en lugar de una tabla hash.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Densa<T> {
    v: Vec<Option<T>>,
}

impl<T> Default for Densa<T> {
    fn default() -> Self {
        Densa { v: Vec::new() }
    }
}

impl<T: Copy + Default> Densa<T> {
    pub fn get(&self, k: &u32) -> Option<&T> {
        self.v.get(*k as usize).and_then(|x| x.as_ref())
    }
    /// El valor de la clave (lo crea con el valor por defecto si no estaba).
    pub fn en(&mut self, k: u32) -> &mut T {
        let i = k as usize;
        if i >= self.v.len() {
            self.v.resize(i + 1, None);
        }
        self.v[i].get_or_insert_with(T::default)
    }
    pub fn insert(&mut self, k: u32, x: T) {
        *self.en(k) = x;
    }
}

impl<T: Copy + Default> FromIterator<(u32, T)> for Densa<T> {
    fn from_iter<I: IntoIterator<Item = (u32, T)>>(it: I) -> Self {
        let mut d = Densa::default();
        for (k, x) in it {
            d.insert(k, x);
        }
        d
    }
}

/// Índices derivados (no se guardan; se rehacen al cargar).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Indices {
    /// Personas vivas, en orden de id.
    pub vivos: Vec<PersonaId>,
    /// Hogares activos de cada solar residencial.
    pub por_lote: Indice,
    /// Personas que trabajan en cada solar.
    pub trabajadores: Indice,
    pub textos: HashMap<String, u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Poblacion {
    pub personas: Vec<Persona>,
    pub hogares: Vec<Hogar>,
    /// Registro de nombres y apellidos (las personas guardan índices).
    pub textos: Vec<String>,
    /// Dueño de cada solar (residencial, comercial o industrial) con edificio.
    pub duenos: BTreeMap<LoteId, HogarId>,
    pub azar: u64,
    pub mes: i32,
    pub fundada: bool,
    /// Modo "fiel a la historia": las llegadas históricas ocurren en sus fechas y la época
    /// sigue al calendario. Si no, depende del jugador.
    pub fiel_historia: bool,
    pub epoca: Epoca,
    /// Crónica de la población (mes, texto), las últimas entradas.
    pub cronica: VecDeque<(i32, String)>,
    /// Fotos mensuales (últimos 24 meses) y anuales (todas).
    pub historial: VecDeque<Foto>,
    pub anual: Vec<Foto>,
    pub mes_actual: Contadores,
    pub año_actual: Contadores,
    /// Lo que salió del patrimonio de las familias por legados (misas, obras pías) y por
    /// herencias vacantes: cierra la cuenta de la herencia (la riqueza se conserva).
    pub legados: f64,
    /// Préstamos del cabildo pendientes de cobro.
    pub prestamos: f32,
    /// Meses que lleva la obra del fuerte (las cuadrillas guaraníes vuelven al terminar).
    pub obra: u32,
    /// Población agregada (anónima) por edad simple: [hombres, mujeres]. Se usa cuando las
    /// personas individuales superan `limite_individuos` (épocas tardías).
    pub cohortes: Vec<[f32; 2]>,
    pub limite_individuos: u32,
    /// Nivel de cada solar y su distancia a la plaza según el último mes (para valuar los
    /// patrimonios y saber el barrio). Se guardan para que la partida cargada valúe igual.
    #[serde(default)]
    pub niveles: Densa<u8>,
    #[serde(default)]
    pub distancias: Densa<f32>,
    #[serde(skip)]
    pub idx: Indices,
    #[serde(skip)]
    pub stats: Estadisticas,
}

/// Dos poblaciones son iguales si guardan lo mismo (los índices y las estadísticas se
/// derivan del estado guardado).
impl PartialEq for Poblacion {
    fn eq(&self, o: &Self) -> bool {
        serde_json::to_string(self).ok() == serde_json::to_string(o).ok()
    }
}

/// Límite por defecto de personas simuladas una por una.
pub const LIMITE_INDIVIDUOS: u32 = 250_000;

impl Poblacion {
    pub fn nueva(semilla: u64) -> Poblacion {
        Poblacion { azar: Rng::new(semilla).state(), fiel_historia: true, epoca: Epoca::Colonial, limite_individuos: LIMITE_INDIVIDUOS, ..Default::default() }
    }

    // --- Consultas (API para los demás sistemas) ---------------------------------------

    pub fn persona(&self, id: PersonaId) -> Option<&Persona> {
        self.personas.get(id as usize)
    }

    pub fn hogar(&self, id: HogarId) -> Option<&Hogar> {
        self.hogares.get(id as usize)
    }

    /// Personas vivas.
    pub fn vivos(&self) -> impl Iterator<Item = &Persona> {
        self.idx.vivos.iter().map(|&i| &self.personas[i as usize])
    }

    /// Habitantes: personas individuales más la población agregada.
    pub fn habitantes(&self) -> u32 {
        self.idx.vivos.len() as u32 + self.cohortes.iter().map(|c| c[0] + c[1]).sum::<f32>().round() as u32
    }

    /// Hogares que viven en un solar.
    pub fn hogares_de_lote(&self, lote: LoteId) -> impl Iterator<Item = &Hogar> {
        self.idx.por_lote.get(&lote).into_iter().flatten().map(|&h| &self.hogares[h as usize])
    }

    /// Personas que viven en un solar (para los agentes que salen a la calle).
    pub fn habitantes_de_lote(&self, lote: LoteId) -> impl Iterator<Item = &Persona> {
        self.hogares_de_lote(lote).flat_map(|h| h.miembros.iter().map(|&p| &self.personas[p as usize]))
    }

    /// Personas que trabajan en un solar.
    pub fn trabajadores_de_lote(&self, lote: LoteId) -> impl Iterator<Item = &Persona> {
        self.idx.trabajadores.get(&lote).into_iter().flatten().map(|&p| &self.personas[p as usize])
    }

    /// Solar donde trabaja una persona (si trabaja en un comercio o taller de la ciudad).
    pub fn lugar_de_trabajo(&self, persona: PersonaId) -> Option<LoteId> {
        self.persona(persona).filter(|p| p.viva()).and_then(|p| p.trabajo)
    }

    /// Solar donde vive una persona.
    pub fn casa_de(&self, persona: PersonaId) -> Option<LoteId> {
        let p = self.persona(persona)?;
        self.hogar(p.hogar?).filter(|h| h.vive_en_solar()).and_then(|h| h.lote)
    }

    pub fn texto(&self, i: u32) -> &str {
        self.textos.get(i as usize).map_or("", |s| s.as_str())
    }

    /// "Juan Hernández Mesa" (o "Juan, nacido en Montevideo" si no tiene apellido).
    pub fn nombre_completo(&self, id: PersonaId) -> String {
        let Some(p) = self.persona(id) else { return "—".into() };
        let n = self.texto(p.nombre);
        match p.apellidos {
            [NADIE, NADIE] => format!("{n}, {} en {}", if p.sexo == Sexo::Mujer { "nacida" } else { "nacido" }, p.origen.corto()),
            [a, NADIE] | [NADIE, a] => format!("{n} {}", self.texto(a)),
            [a, b] => format!("{n} {} {}", self.texto(a), self.texto(b)),
        }
    }

    /// Nombre de pila.
    pub fn nombre(&self, id: PersonaId) -> &str {
        self.persona(id).map_or("—", |p| self.texto(p.nombre))
    }

    /// Apellido de la familia de un hogar (el primero del jefe).
    pub fn apellido_hogar(&self, h: HogarId) -> String {
        let Some(hg) = self.hogar(h) else { return String::new() };
        let j = &self.personas[hg.jefe as usize];
        match j.apellidos {
            [NADIE, NADIE] => format!("de {}", self.texto(j.nombre)),
            [a, NADIE] | [NADIE, a] => self.texto(a).to_string(),
            [a, b] => format!("{} {}", self.texto(a), self.texto(b)),
        }
    }

    /// Actualiza el nivel y la distancia a la plaza de los solares (al cargar una partida,
    /// antes del primer mes).
    pub fn conocer_lotes(&mut self, lotes: &[LoteInfo]) {
        self.niveles = lotes.iter().map(|l| (l.id, l.nivel)).collect();
        self.distancias = lotes.iter().map(|l| (l.id, l.distancia)).collect();
    }

    // --- Registro de textos ------------------------------------------------------------

    pub(crate) fn interna(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.idx.textos.get(s) {
            return i;
        }
        let i = self.textos.len() as u32;
        self.textos.push(s.to_string());
        self.idx.textos.insert(s.to_string(), i);
        i
    }

    // --- Índices ------------------------------------------------------------------------

    /// Rehace los índices derivados (al cargar una partida o después de cambios grandes).
    pub fn reconstruir(&mut self) {
        self.idx.textos = self.textos.iter().enumerate().map(|(i, s)| (s.clone(), i as u32)).collect();
        self.reindexar();
        self.stats = self.estadisticas();
    }

    pub(crate) fn reindexar(&mut self) {
        self.idx.vivos = self.personas.iter().filter(|p| p.viva()).map(|p| p.id).collect();
        self.idx.por_lote = Indice::construir(self.hogares.iter().filter(|h| h.activo && h.vive_en_solar()).map(|h| (h.lote.unwrap(), h.id)).collect());
        self.idx.trabajadores = Indice::construir(self.idx.vivos.iter().filter_map(|&p| self.personas[p as usize].trabajo.map(|l| (l, p))).collect());
    }

    // --- Creación -----------------------------------------------------------------------

    pub(crate) fn rasgos_al_azar(rng: &mut Rng) -> Rasgos {
        let mut r = [0u8; 5];
        for x in &mut r {
            *x = (((rng.f() + rng.f() + rng.f()) / 3.0) * 255.0) as u8;
        }
        Rasgos(r)
    }

    /// Crea una persona (sin hogar) y devuelve su id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn crear_persona(&mut self, rng: &mut Rng, sexo: Sexo, nacimiento: i32, origen: Origen, apellidos: [u32; 2], padre: Option<PersonaId>, madre: Option<PersonaId>) -> PersonaId {
        let lista = nombres::nombres(origen.cultura(), sexo);
        let nombre = self.interna(lista[(rng.next() % lista.len() as u64) as usize]);
        let raiz = match (padre.map(|p| self.personas[p as usize].raiz), madre.map(|m| self.personas[m as usize].raiz)) {
            (Some(a), Some(b)) if a == b => a,
            (Some(_), Some(_)) => Raiz::Mixta,
            _ => origen.raiz(),
        };
        let id = self.personas.len() as PersonaId;
        let mut p = Persona {
            id,
            nombre,
            apellidos,
            sexo,
            nacimiento,
            muerte: None,
            fuera: None,
            padre,
            madre,
            conyuge: None,
            union_formal: false,
            hijos: Vec::new(),
            hogar: None,
            origen,
            raiz,
            estamento: Estamento::Morador,
            oficio: Oficio::Ninguno,
            educacion: 0,
            salud: 0.75 + rng.f() * 0.25,
            riqueza: 0.0,
            rasgos: Self::rasgos_al_azar(rng),
            trabajo: None,
            felicidad: 0.6,
            motivos: [felicidad::SIN_MOTIVO; 2],
            duelo: 0,
            historial: Vec::new(),
        };
        p.evento(nacimiento, TipoEvento::Nacio, NADIE);
        self.personas.push(p);
        self.idx.vivos.push(id);
        id
    }

    /// Apellido al azar de una cultura (o `NADIE`).
    pub(crate) fn apellido_al_azar(&mut self, rng: &mut Rng, c: Cultura) -> u32 {
        let l = nombres::apellidos(c);
        if l.is_empty() {
            return NADIE;
        }
        // Los guaraníes de las misiones alternan apellidos de raíz guaraní y castellanos.
        let l = if c == Cultura::Guarani && rng.f() < 0.5 { nombres::HISPANO_AP } else { l };
        self.interna(l[(rng.next() % l.len() as u64) as usize])
    }

    pub(crate) fn crear_hogar(&mut self, jefe: PersonaId, origen: Origen, alojamiento: Alojamiento) -> HogarId {
        let id = self.hogares.len() as HogarId;
        self.hogares.push(Hogar {
            id,
            activo: true,
            lote: None,
            alojamiento,
            miembros: Vec::new(),
            jefe,
            origen,
            ingresos: 0.0,
            gastos: 0.0,
            ahorro: 0.0,
            deuda: 0.0,
            solares: Vec::new(),
            ganado: 0,
            clase: Clase::Modesta,
            detalle: [0.0; 6],
            meses_de_hambre: 0,
            desastre: None,
        });
        self.unir_a_hogar(jefe, id);
        id
    }

    /// Mueve a una persona a un hogar (sacándola del anterior).
    pub(crate) fn unir_a_hogar(&mut self, p: PersonaId, h: HogarId) {
        if let Some(prev) = self.personas[p as usize].hogar {
            if prev == h {
                return;
            }
            self.hogares[prev as usize].miembros.retain(|&m| m != p);
            // Si se va el jefe, queda al frente quien siga.
            if self.hogares[prev as usize].jefe == p {
                let nuevo = self.hogares[prev as usize].miembros.first().copied();
                let hp = &mut self.hogares[prev as usize];
                match nuevo {
                    Some(n) => hp.jefe = n,
                    None if hp.ahorro == 0.0 && hp.deuda == 0.0 && hp.solares.is_empty() && hp.ganado == 0 => hp.activo = false,
                    None => {}
                }
            }
        }
        self.personas[p as usize].hogar = Some(h);
        self.hogares[h as usize].miembros.push(p);
    }

    /// Edad al azar de un adulto que llega (inmigrantes jóvenes).
    fn edad_adulta(rng: &mut Rng, min: i32, max: i32) -> i32 {
        let t = (rng.f() + rng.f()) * 0.5;
        min + ((max - min) as f32 * t) as i32
    }

    /// Una familia que llega: jefe, cónyuge e hijos (o una persona sola). Devuelve el hogar.
    pub(crate) fn llegar_familia(&mut self, rng: &mut Rng, ll: &origenes::Llegada, mes: i32, tam: u32) -> HogarId {
        let o = ll.origen;
        let c = o.cultura();
        let ap_h = [self.apellido_al_azar(rng, c), self.apellido_al_azar(rng, c)];
        let edad_h = Self::edad_adulta(rng, 20, 45);
        let sexo_jefe = if tam == 1 && rng.f() < 0.15 { Sexo::Mujer } else { Sexo::Hombre };
        let jefe = {
            let nac = mes - edad_h * 12 - rng.range(0, 11);
            self.crear_persona(rng, sexo_jefe, nac, o, ap_h, None, None)
        };
        let h = self.crear_hogar(jefe, o, Alojamiento::SinCasa);
        let riqueza = ll.riqueza * (ll.dispersion * normal(rng)).exp();
        self.hogares[h as usize].ahorro = riqueza;
        let mut adultos = vec![jefe];
        if tam >= 2 {
            let ap_m = [self.apellido_al_azar(rng, c), self.apellido_al_azar(rng, c)];
            let edad_m = (edad_h - 3 - rng.range(0, 8)).max(17);
            let esposa = {
                let nac = mes - edad_m * 12 - rng.range(0, 11);
                self.crear_persona(rng, Sexo::Mujer, nac, o, ap_m, None, None)
            };
            self.casar(jefe, esposa, true, mes);
            self.unir_a_hogar(esposa, h);
            adultos.push(esposa);
            // Hijos nacidos antes de llegar.
            let max_hijo = (edad_m - 16).max(0);
            for _ in 2..tam {
                let e = rng.range(0, max_hijo.min(18));
                let sexo = if rng.f() < 0.51 { Sexo::Hombre } else { Sexo::Mujer };
                let hijo = {
                    let nac = mes - e * 12 - rng.range(0, 11);
                    self.crear_persona(rng, sexo, nac, o, [ap_h[0], ap_m[0]], Some(jefe), Some(esposa))
                };
                self.personas[jefe as usize].hijos.push(hijo);
                self.personas[esposa as usize].hijos.push(hijo);
                self.unir_a_hogar(hijo, h);
                if e >= 15 {
                    adultos.push(hijo);
                }
            }
        }
        for &a in &adultos {
            let p = &mut self.personas[a as usize];
            p.evento(mes, TipoEvento::Llego(o), NADIE);
            if rng.f() < ll.lectura {
                p.educacion = if rng.f() < 0.4 { 2 } else { 1 };
            }
        }
        // Oficio: el jefe trae el suyo; la esposa y las hijas, a veces.
        let total: f32 = ll.oficios.iter().map(|x| x.1).sum();
        for &a in &adultos {
            let mujer = self.personas[a as usize].sexo == Sexo::Mujer;
            if mujer && a != jefe && rng.f() > 0.35 {
                continue;
            }
            let mut r = rng.f() * total;
            let mut of = ll.oficios[0].0;
            for &(o2, w) in ll.oficios {
                if r < w {
                    of = o2;
                    break;
                }
                r -= w;
            }
            if mujer && matches!(of, Oficio::Hacendado | Oficio::Soldado | Oficio::Marinero | Oficio::Labrador) {
                of = Oficio::Servicio;
            }
            self.personas[a as usize].oficio = of;
        }
        if self.personas[jefe as usize].oficio == Oficio::Hacendado {
            self.hogares[h as usize].ganado = (riqueza / 4.0) as u32;
        }
        h
    }

    // --- Fundación ----------------------------------------------------------------------

    /// Los primeros pobladores al fundar la plaza (meses desde enero de 1726): seis familias de
    /// Buenos Aires (33 personas), la guarnición del fuerte y las cuadrillas guaraníes de la
    /// obra.
    pub fn fundar(&mut self, mes: i32) {
        if self.fundada {
            return;
        }
        self.fundada = true;
        self.mes = mes;
        let mut rng = Rng::from_state(self.azar);
        let bsas = origenes::COLONIAL.iter().find(|l| l.origen == Origen::BuenosAires).unwrap();
        for tam in [8, 7, 6, 5, 4, 3] {
            let h = self.llegar_familia(&mut rng, bsas, mes, tam);
            let jefe = self.hogares[h as usize].jefe;
            self.personas[jefe as usize].estamento = Estamento::Vecino;
            // Los pobladores fundadores recibían solares, chacras y algo de ganado.
            self.hogares[h as usize].ganado += 20 + rng.range(0, 40) as u32;
        }
        // Guarnición: cien soldados de distintos orígenes, en el cuartel del fuerte.
        let origenes_tropa = [Origen::Andalucia, Origen::Galicia, Origen::Cantabrico, Origen::BuenosAires, Origen::Canarias];
        let mut cuartel = None;
        for i in 0..origenes::GUARNICION_1728 {
            let o = origenes_tropa[i as usize % origenes_tropa.len()];
            let ap = [self.apellido_al_azar(&mut rng, o.cultura()), self.apellido_al_azar(&mut rng, o.cultura())];
            let edad = Self::edad_adulta(&mut rng, 17, 40);
            let p = {
                let nac = mes - edad * 12 - rng.range(0, 11);
                self.crear_persona(&mut rng, Sexo::Hombre, nac, o, ap, None, None)
            };
            let pp = &mut self.personas[p as usize];
            pp.oficio = Oficio::Soldado;
            pp.estamento = Estamento::Militar;
            pp.evento(mes, TipoEvento::Llego(o), NADIE);
            match cuartel {
                None => cuartel = Some(self.crear_hogar(p, o, Alojamiento::Cuartel)),
                Some(h) => self.unir_a_hogar(p, h),
            }
        }
        // Cuadrillas guaraníes de las misiones para la obra del fuerte (libres; vuelven a sus
        // pueblos al terminar la obra, algunos se quedan).
        let mis = origenes::COLONIAL.iter().find(|l| l.origen == Origen::Misiones).unwrap();
        let mut campamento = None;
        for _ in 0..origenes::CUADRILLA_GUARANI_1728 {
            let ap = [self.apellido_al_azar(&mut rng, Cultura::Guarani), self.apellido_al_azar(&mut rng, Cultura::Guarani)];
            let edad = Self::edad_adulta(&mut rng, 16, 40);
            let p = {
                let nac = mes - edad * 12 - rng.range(0, 11);
                self.crear_persona(&mut rng, Sexo::Hombre, nac, Origen::Misiones, ap, None, None)
            };
            let pp = &mut self.personas[p as usize];
            pp.oficio = if rng.f() < 0.25 { Oficio::Artesano } else { Oficio::Peon };
            pp.evento(mes, TipoEvento::Llego(Origen::Misiones), NADIE);
            if rng.f() < mis.lectura {
                pp.educacion = 1;
            }
            match campamento {
                None => campamento = Some(self.crear_hogar(p, Origen::Misiones, Alojamiento::Campamento)),
                Some(h) => self.unir_a_hogar(p, h),
            }
        }
        self.clasificar_hogares();
        self.azar = rng.state();
        self.mes_actual.llegados += 33 + origenes::GUARNICION_1728 + origenes::CUADRILLA_GUARANI_1728;
        self.anotar(mes, "Se funda la ciudad: seis familias de Buenos Aires, la guarnición del fuerte y las cuadrillas guaraníes de la obra".into());
        self.reindexar();
        self.stats = self.estadisticas();
    }

    /// Población inicial para una partida guardada antes de que existiera la población:
    /// la fundación más tantas personas como vivían en los solares.
    pub fn desde_partida_vieja(semilla: u64, mes: i32, habitantes: i32) -> Poblacion {
        let mut p = Poblacion::nueva(semilla);
        p.fundar(mes);
        let mut rng = Rng::from_state(p.azar);
        let mut n = 0;
        while n < habitantes.max(0) as u32 {
            let ll = p.elegir_llegada(&mut rng);
            let tam = p.tam_familia(&mut rng, ll);
            p.llegar_familia(&mut rng, ll, mes, tam);
            n += tam;
        }
        p.azar = rng.state();
        p.reindexar();
        p.stats = p.estadisticas();
        p
    }

    // --- Llegadas -----------------------------------------------------------------------

    pub(crate) fn tam_familia(&self, rng: &mut Rng, ll: &origenes::Llegada) -> u32 {
        if ll.familia <= 1.05 {
            return 1;
        }
        // Una parte llega sola; el resto, en familia de tamaño alrededor de la media.
        if rng.f() < 0.3 {
            return 1;
        }
        let m = (ll.familia - 1.0) / 0.7 + 1.0;
        (m + normal(rng) * 1.2).round().clamp(2.0, 9.0) as u32
    }

    /// Elige un origen de la tabla de la época según su peso.
    pub(crate) fn elegir_llegada(&self, rng: &mut Rng) -> &'static origenes::Llegada {
        let t = origenes::tabla(self.epoca);
        let año = año_de(self.mes);
        let peso = |l: &origenes::Llegada| l.peso * origenes::factor_origen(l.origen, año);
        let total: f32 = t.iter().map(peso).sum();
        let mut r = rng.f() * total;
        for l in t {
            let w = peso(l);
            if r < w {
                return l;
            }
            r -= w;
        }
        &t[0]
    }

    /// Las llegadas del mes: familias de los orígenes de la época hasta sumar `n` personas.
    fn llegadas(&mut self, rng: &mut Rng, ent: &Entorno, noticias: &mut Vec<String>) {
        let mut n = ent.llegadas;
        // Llegadas históricas fijas (los barcos de pobladores canarios).
        if self.fiel_historia {
            for &(m, o, familias, personas) in origenes::LLEGADAS_HISTORICAS {
                if m == ent.mes {
                    let ll = origenes::COLONIAL.iter().find(|l| l.origen == o).unwrap();
                    let mut hechas = 0;
                    for f in 0..familias {
                        let tam = (personas - hechas) / (familias - f);
                        let h = self.llegar_familia(rng, ll, ent.mes, tam.max(1));
                        let jefe = self.hogares[h as usize].jefe;
                        self.personas[jefe as usize].estamento = Estamento::Vecino;
                        hechas += tam.max(1);
                    }
                    self.mes_actual.llegados += hechas;
                    let t = format!("Llega un barco de pobladores {}: {familias} familias, {hechas} personas, a las que se les reparten solares", o.de());
                    noticias.push(t.clone());
                    self.anotar(ent.mes, t);
                }
            }
        }
        let mut notable: Option<HogarId> = None;
        while n > 0 {
            let ll = self.elegir_llegada(rng);
            let tam = self.tam_familia(rng, ll).min(n.max(1));
            let h = self.llegar_familia(rng, ll, ent.mes, tam);
            if tam >= 4 && notable.is_none() {
                notable = Some(h);
            }
            self.mes_actual.llegados += tam;
            n = n.saturating_sub(tam);
        }
        if let Some(h) = notable {
            let hg = &self.hogares[h as usize];
            let j = hg.jefe;
            let hijos = hg.miembros.len().saturating_sub(2);
            let t = format!("Llega {} la familia {}: {} con su familia y {} {}", hg.origen.de(), self.apellido_hogar(h), self.nombre(j), hijos, if hijos == 1 { "hijo" } else { "hijos" });
            self.anotar(ent.mes, t);
        }
    }

    // --- Crónica ------------------------------------------------------------------------

    pub(crate) fn anotar(&mut self, mes: i32, texto: String) {
        self.cronica.push_back((mes, texto));
        while self.cronica.len() > 200 {
            self.cronica.pop_front();
        }
    }

    // --- El mes -------------------------------------------------------------------------

    /// Avanza un mes de la población.
    pub fn mes(&mut self, ent: &Entorno) -> InformeMes {
        let mut inf = InformeMes::default();
        if !self.fundada {
            return inf;
        }
        if self.idx.textos.len() != self.textos.len() {
            self.reconstruir();
        }
        self.mes = ent.mes;
        if self.fiel_historia {
            self.epoca = self.epoca.max(Epoca::del_año(año_de(ent.mes)));
        }
        let mut rng = Rng::from_state(self.azar);
        let mut noticias = Vec::new();
        self.llegadas(&mut rng, ent, &mut noticias);
        self.vivienda(&mut rng, ent, &mut inf);
        self.empleo(&mut rng, ent);
        self.economia_hogares(&mut rng, ent, &mut inf);
        self.muertes(&mut rng, ent, &mut noticias);
        self.nacimientos(&mut rng, ent);
        self.casamientos(&mut rng, ent, &mut noticias);
        self.crecer(&mut rng, ent);
        self.migraciones(&mut rng, ent, &mut noticias);
        self.reindexar();
        self.felicidad(ent);
        self.clasificar_hogares();
        if self.idx.vivos.len() as u32 > self.limite_individuos {
            self.agregar_cohortes();
        }
        if ent.mes % 12 == 11 {
            self.envejecer_cohortes();
        }
        inf.sin_casa = self.demanda_vivienda(ent);
        inf.animo = self.animo_sectores();
        // Fotos para los gráficos.
        self.stats = self.estadisticas();
        let foto = Foto::de(ent.mes, &self.stats, &self.mes_actual);
        self.historial.push_back(foto);
        while self.historial.len() > 24 {
            self.historial.pop_front();
        }
        let mc = self.mes_actual;
        self.año_actual.sumar(&mc);
        if ent.mes % 12 == 11 {
            let fa = Foto::de(ent.mes, &self.stats, &self.año_actual);
            let t = format!(
                "Año {}: {} habitantes; nacieron {}, murieron {} ({} antes del año de vida), {} casamientos, llegaron {} y se fueron {}",
                año_de(ent.mes),
                self.habitantes(),
                self.año_actual.nacimientos,
                self.año_actual.muertes,
                self.año_actual.muertes_infantiles,
                self.año_actual.casamientos,
                self.año_actual.llegados,
                self.año_actual.emigrados
            );
            self.anotar(ent.mes, t);
            self.anual.push(fa);
            self.año_actual = Contadores::default();
            self.compactar();
        }
        self.mes_actual = Contadores::default();
        // Protestas cuando mucha gente lo pasa mal.
        if self.stats.descontentos > 0.35 && rng.f() < 0.35 {
            noticias.push(format!("Protesta en la plaza: {} de cada diez vecinos dicen que no se puede vivir así", (self.stats.descontentos * 10.0).round() as i32));
        }
        self.azar = rng.state();
        noticias.truncate(4);
        inf.noticias = noticias;
        inf
    }

    /// Las personas muertas hace mucho guardan solo los hechos principales de su vida (el
    /// árbol del linaje sigue completo).
    fn compactar(&mut self) {
        let corte = self.mes - 12 * 40;
        for p in &mut self.personas {
            if p.muerte.is_some_and(|m| m < corte) && p.historial.len() > 6 {
                p.historial.retain(|e| matches!(e.tipo, TipoEvento::Nacio | TipoEvento::Llego(_) | TipoEvento::Caso | TipoEvento::Union | TipoEvento::Murio(_)));
                p.historial.shrink_to_fit();
            }
        }
    }

    // --- Choques de mortalidad (para servicios, agua, puerto) -------------------------

    /// Una epidemia: `virulencia` es la probabilidad extra de morir de cada persona (por
    /// ejemplo 0,05); `lotes` limita el brote a esos solares (o a toda la ciudad). Los pobres,
    /// los niños y los viejos son los más expuestos. Devuelve cuántas personas murieron.
    pub fn choque_epidemia(&mut self, virulencia: f32, lotes: Option<&[LoteId]>) -> u32 {
        self.choque(Causa::Epidemia, virulencia, lotes)
    }

    /// Incendio en un solar: riesgo para quienes viven ahí; la casa se pierde (la ciudad borra
    /// el edificio) y el hogar queda sin casa.
    pub fn choque_incendio(&mut self, lote: LoteId) -> u32 {
        let n = self.choque(Causa::Incendio, 0.03, Some(&[lote]));
        self.desalojar(&[lote]);
        n
    }

    /// Inundación: riesgo para quienes viven en esos solares; los que perdieron la casa
    /// (ranchos) quedan sin casa.
    pub fn choque_inundacion(&mut self, lotes: &[LoteId], perdidos: &[LoteId]) -> u32 {
        let n = self.choque(Causa::Inundacion, 0.01, Some(lotes));
        self.desalojar(perdidos);
        n
    }

    /// Un robo en un solar (casa) o el asalto a un comercio (`comercio`): el hogar que vive ahí
    /// (o el dueño del comercio) pierde una parte de sus ahorros. Devuelve lo robado (pesos).
    pub fn choque_robo(&mut self, lote: LoteId, comercio: bool) -> f32 {
        let victima = if comercio {
            self.duenos.get(&lote).copied()
        } else {
            self.idx.por_lote.get(&lote).and_then(|hs| hs.iter().copied().max_by(|a, b| self.hogares[*a as usize].ahorro.total_cmp(&self.hogares[*b as usize].ahorro).then(b.cmp(a))))
        };
        let Some(h) = victima else { return 0.0 };
        let hg = &mut self.hogares[h as usize];
        let robo = (hg.ahorro * 0.2).min(if comercio { 80.0 } else { 40.0 }).max(0.0);
        hg.ahorro -= robo;
        robo
    }

    /// Hambruna o sequía larga: `severidad` 0 a 1. Pesa sobre los hogares sin reservas.
    pub fn choque_hambruna(&mut self, severidad: f32) -> u32 {
        self.choque(Causa::Hambre, 0.02 * severidad.clamp(0.0, 1.0), None)
    }

    fn choque(&mut self, causa: Causa, p_extra: f32, lotes: Option<&[LoteId]>) -> u32 {
        let mut rng = Rng::from_state(self.azar);
        let mes = self.mes;
        let mut victimas = Vec::new();
        for &id in &self.idx.vivos {
            let p = &self.personas[id as usize];
            let h = p.hogar.map(|h| &self.hogares[h as usize]);
            if let Some(ls) = lotes {
                if !h.and_then(|h| h.lote).is_some_and(|l| ls.contains(&l)) {
                    continue;
                }
            }
            let edad = p.edad(mes);
            let mut f = 1.0 + (1.0 - p.salud);
            if edad < 5 || edad > 60 {
                f *= 1.8;
            }
            if let Some(h) = h {
                f *= match h.clase {
                    Clase::Pobre => 1.5,
                    Clase::Modesta => 1.0,
                    Clase::Acomodada => 0.7,
                    Clase::Principal => 0.5,
                };
                if causa == Causa::Hambre && h.ahorro > 50.0 {
                    f *= 0.2;
                }
            }
            if rng.f() < p_extra * f {
                victimas.push(id);
            }
        }
        self.azar = rng.state();
        let n = victimas.len() as u32;
        let mut noticias = Vec::new();
        for v in victimas {
            self.fallecer(v, causa, &mut noticias);
        }
        for t in noticias {
            self.anotar(mes, t);
        }
        if n > 0 {
            let t = match causa {
                Causa::Epidemia => format!("La epidemia se lleva {n} vidas"),
                Causa::Incendio => format!("El incendio deja {n} muertos"),
                Causa::Inundacion => format!("La inundación deja {n} muertos"),
                Causa::Hambre => format!("El hambre se lleva {n} vidas"),
                _ => format!("{n} muertes"),
            };
            self.anotar(mes, t);
        }
        self.reindexar();
        n
    }

    /// Los hogares de esos solares quedan sin casa (por un incendio o una inundación).
    pub fn desalojar(&mut self, lotes: &[LoteId]) {
        let mes = self.mes;
        for h in &mut self.hogares {
            if h.activo && h.lote.is_some_and(|l| lotes.contains(&l)) {
                h.lote = None;
                h.alojamiento = Alojamiento::SinCasa;
                h.desastre = Some(mes);
            }
        }
        self.reindexar();
        // Las fotos, al día (los incendios pasan entre un mes y otro).
        self.stats = self.estadisticas();
    }
}

/// Normal estándar aproximada (suma de uniformes).
pub(crate) fn normal(rng: &mut Rng) -> f32 {
    (rng.f() + rng.f() + rng.f() + rng.f() - 2.0) * 1.732
}


