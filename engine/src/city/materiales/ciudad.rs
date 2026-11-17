//! Los materiales en la ciudad: el depósito (lo que no guarda la economía), la producción del
//! lugar, lo que trae el puerto, los precios, las obras de cada casa y servicio, el desgaste y
//! los arreglos, y cómo se ven los materiales en los vóxeles.

use super::*;
use crate::buildings::Edits;
use crate::city::economy::{Economy, Ordinance};
use crate::city::lots::{zone_of, Density, Lot, ZoneKind};
use crate::city::services::{self, Kind};
use crate::city::City;
use crate::palette::*;
use crate::structures::{aplicar_variante, Variante};
use crate::terrain::TerrainGen;
use std::collections::BTreeMap;

/// Tejas por mes de un horno de ladrillos (además de sus ladrillos). Estimación.
pub const HORNO_TEJAS: f32 = 15.0;
/// Lo que trae cada barco de materiales de obra (unidades). Estimación.
pub const BODEGA_MATERIALES: f32 = 90.0;
/// Bajo este estado el dueño lo manda arreglar.
pub const ESTADO_ARREGLO: f32 = 0.4;
/// Parte de la receta que piden los arreglos.
pub const PARTE_ARREGLO: f32 = 0.25;

/// Estado de los materiales de la ciudad (se guarda con la partida).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Materiales {
    /// Existencias y cuentas del último mes cerrado. El ladrillo, la piedra y el cuero viven
    /// en la economía: acá solo llevan la cuenta de lo pedido, lo que faltó y lo sustituido.
    pub ex: [Existencia; N],
    /// Cuentas del mes en curso: pedido, faltante, usado y sustituciones.
    pub mes_pedido: [f32; N],
    pub mes_faltante: [f32; N],
    pub mes_usado: [f32; N],
    pub mes_sust: [u32; N],
    /// Demanda media de las obras (suavizada mes a mes): manda los encargos al puerto y los
    /// precios.
    pub demanda: [f32; N],
    /// Faltas ya avisadas en la gaceta (bits de [`Material`]).
    pub avisadas: u16,
    /// Con qué se hizo cada edificio de servicio (id → obra).
    pub servicios: BTreeMap<u32, Obra>,
    /// Obras levantadas el mes en curso (y cuántas con sustitutos), y las del último mes.
    pub obras_mes: (u32, u32),
    pub obras_ultimo: (u32, u32),
    /// Arreglos y derrumbes del último mes.
    pub arreglos_ultimo: (u32, u32),
    /// Últimos 12 meses: existencias de cada material (para el panel).
    pub historial: Vec<[f32; N]>,
}

impl Default for Materiales {
    fn default() -> Self {
        let mut ex: [Existencia; N] = Default::default();
        for m in Material::TODOS {
            ex[m.idx()].cantidad = m.datos().inicial;
            ex[m.idx()].precio = 1.0;
        }
        Materiales { ex, mes_pedido: [0.0; N], mes_faltante: [0.0; N], mes_usado: [0.0; N], mes_sust: [0; N], demanda: [0.0; N], avisadas: 0, servicios: BTreeMap::new(), obras_mes: (0, 0), obras_ultimo: (0, 0), arreglos_ultimo: (0, 0), historial: Vec::new() }
    }
}

impl Materiales {
    /// Demanda de ladrillos, piedra y cueros de las obras, para la economía (precio y puerto).
    pub fn pedidos_economia(&self) -> [f32; 6] {
        let mut p = [0.0; 6];
        for m in [Material::Ladrillo, Material::Piedra, Material::Cuero] {
            if let Some(r) = m.de_la_economia() {
                p[r as usize] = self.demanda[m.idx()];
            }
        }
        p
    }
}

impl Materiales {
    /// Cierra el mes del depósito: lo que da el lugar (`ent`), lo que trae el puerto si hay
    /// comercio (hasta `bodega` por material), precios, avisos de la gaceta y las cuentas.
    /// `eco` da las existencias de lo que guarda la economía (ladrillo, piedra, cuero).
    pub fn cerrar_mes(&mut self, ent: &dyn FuenteLocal, comercio: bool, bodega: f32, eco: &dyn Fn(Material) -> f32) -> Vec<String> {
        let mat = self;
        let mut news = Vec::new();
        let mut llego = Vec::new();
        for m in Material::TODOS {
            let i = m.idx();
            mat.demanda[i] = mat.demanda[i] * 0.6 + mat.mes_pedido[i] * 0.4;
            let s = &mut mat.ex[i];
            s.pedido = mat.mes_pedido[i];
            s.faltante = mat.mes_faltante[i];
            s.usado = mat.mes_usado[i];
            s.sustituciones = mat.mes_sust[i];
            if m == Material::Barro || m.de_la_economia().is_some() {
                continue;
            }
            let p = ent.disponibilidad_local(m);
            s.producido = p;
            s.cantidad += p;
            s.importado = 0.0;
            if comercio && m.importable() {
                let reserva = match m {
                    Material::Madera => 40.0,
                    Material::Cal => 40.0,
                    Material::Hierro => 4.0,
                    _ => 0.0,
                };
                let quiere = (mat.demanda[i] * 1.5 + reserva - s.cantidad).max(0.0).min(bodega);
                if quiere > 0.5 {
                    s.cantidad += quiere;
                    s.importado = quiere;
                    if mat.avisadas & (1 << i) != 0 {
                        llego.push(m);
                    }
                }
            }
            // Precio: meses de reserva; lo importado nunca baja de lo que cuesta traerlo.
            let need = mat.demanda[i];
            let cover = if need > 0.5 { s.cantidad / need } else { 3.0 };
            let piso = if p < 1.0 && m.importable() { 1.0 } else { 0.6 };
            let target = (1.5 / cover.max(0.2)).powf(0.8).clamp(piso, 3.0);
            s.precio += (target - s.precio) * 0.5;
            s.cantidad = s.cantidad.min(4000.0);
        }
        // 3. Avisos: una vez por falta, y cuando llega por el puerto.
        for m in Material::TODOS {
            let i = m.idx();
            let bit = 1u16 << i;
            if mat.mes_sust[i] > 0 && mat.avisadas & bit == 0 && !m.aviso_falta().is_empty() {
                mat.avisadas |= bit;
                news.push(m.aviso_falta().to_string());
            } else if mat.mes_sust[i] == 0 && mat.avisadas & bit != 0 && (llego.contains(&m) || (if m.de_la_economia().is_some() { eco(m) } else { mat.ex[i].cantidad }) > mat.demanda[i]) {
                mat.avisadas &= !bit;
                if llego.contains(&m) {
                    news.push(format!("Llegó {} por el puerto para las obras", m.nombre()));
                }
            }
        }
        mat.obras_ultimo = mat.obras_mes;
        mat.obras_mes = (0, 0);
        mat.mes_pedido = [0.0; N];
        mat.mes_faltante = [0.0; N];
        mat.mes_usado = [0.0; N];
        mat.mes_sust = [0; N];
        news
    }
}

/// Vista de solo lectura de todo lo que hay (economía + depósito propio).
pub struct Vista<'a> {
    pub eco: &'a Economy,
    pub mat: &'a Materiales,
}

impl Disponible for Vista<'_> {
    fn hay(&self, m: Material) -> f32 {
        match (m, m.de_la_economia()) {
            (Material::Barro, _) => f32::INFINITY,
            (_, Some(r)) => self.eco.stock(r).amount,
            _ => self.mat.ex[m.idx()].cantidad,
        }
    }
}

/// El depósito de la ciudad para consumir (economía + depósito propio).
pub struct Deposito<'a> {
    pub eco: &'a mut Economy,
    pub mat: &'a mut Materiales,
}

impl Disponible for Deposito<'_> {
    fn hay(&self, m: Material) -> f32 {
        Vista { eco: self.eco, mat: self.mat }.hay(m)
    }
}

impl Almacenable for Deposito<'_> {
    fn tomar(&mut self, m: Material, n: f32) -> f32 {
        if m == Material::Barro || n <= 0.0 {
            return n.max(0.0);
        }
        let t = match m.de_la_economia() {
            Some(r) => {
                let s = &mut self.eco.stocks[r as usize];
                let t = n.min(s.amount).max(0.0);
                s.amount -= t;
                t
            }
            None => {
                let s = &mut self.mat.ex[m.idx()];
                let t = n.min(s.cantidad).max(0.0);
                s.cantidad -= t;
                t
            }
        };
        self.mat.mes_usado[m.idx()] += t;
        t
    }
}

/// El entorno de la ciudad como fuente de materiales (hoy: población, hornos, canteras,
/// clima y las capas de arcilla y arena de los solares).
pub struct EntornoMateriales {
    pub pop: f32,
    /// Hornos y canteras (suma de su rinde).
    pub hornos: f32,
    pub canteras: f32,
    /// Empleos de los talleres (también cuecen tejas).
    pub talleres: f32,
    /// Factor del clima para secar adobes y cocer (0,3 a 1).
    pub clima: f32,
    /// Arcilla y arena medias de los solares (0 a 1).
    pub arcilla: f32,
    pub arena: f32,
    /// Cal que dieron las caleras este mes (caliza o conchilla de su sitio).
    pub cal: f32,
}

impl FuenteLocal for EntornoMateriales {
    fn disponibilidad_local(&self, m: Material) -> f32 {
        match m {
            Material::Barro => f32::INFINITY,
            Material::Paja => 400.0 + 0.3 * self.pop,
            Material::Adobe => (50.0 + 0.08 * self.pop) * self.clima * (0.5 + self.arcilla),
            Material::Arena => 30.0 + 120.0 * self.arena,
            // Palos y leña del monte criollo: no hay madera de obra cerca.
            Material::Madera => 6.0 + 0.004 * self.pop,
            Material::Teja => (HORNO_TEJAS * self.hornos + 0.4 * self.talleres) * self.clima,
            Material::Ladrillo => (services::HORNO_LADRILLOS * self.hornos + 1.5 * self.talleres) * self.clima,
            Material::Piedra => services::CANTERA_PIEDRA * self.canteras,
            // La de las caleras (caliza o conchilla); en Montevideo casi no hay: la trae el puerto.
            Material::Cal => self.cal,
            Material::Hierro | Material::Vidrio | Material::Cuero => 0.0,
        }
    }
}

// --- Planes de cada edificio ----------------------------------------------------------------

/// Cómo dibuja el catálogo cada tipo de solar: (paredes, revoque, techo, rejas). Con esto, una
/// partida vieja queda con los materiales de su dibujo y los vóxeles no cambian.
pub fn nominal(kind: ZoneKind, density: Density, level: u8, seed: u64, techo: Option<Techo>) -> Obra {
    let pick = (seed >> 7) % 100;
    let ladrillo = |t: Techo| Obra::nueva(Pared::Ladrillo, t, true, true);
    match (kind, level) {
        (ZoneKind::Residencial, 1) => Obra::nueva(Pared::Adobe, Techo::Paja, false, false),
        (ZoneKind::Residencial, 2) if density == Density::Alta => ladrillo(Techo::Azotea),
        // Casa de una planta: si se sabe el techo, el dibujo lo sigue (azotea plana o techo a
        // dos aguas); si no, la semilla elige como siempre.
        (ZoneKind::Residencial, 2) => match techo {
            Some(Techo::Azotea) => ladrillo(Techo::Azotea),
            Some(_) => ladrillo(Techo::Teja),
            None if pick < 45 => ladrillo(Techo::Teja),
            None => ladrillo(Techo::Azotea),
        },
        (ZoneKind::Residencial, _) => ladrillo(Techo::Azotea),
        (ZoneKind::Comercial, 1) => ladrillo(Techo::Azotea),
        (ZoneKind::Comercial, 2) if pick < 40 => ladrillo(Techo::Teja),
        (ZoneKind::Comercial, 2) => ladrillo(Techo::Azotea),
        (ZoneKind::Comercial, _) if density == Density::Alta || pick < 45 => ladrillo(Techo::Azotea),
        (ZoneKind::Comercial, _) => ladrillo(Techo::Teja),
        (ZoneKind::Industrial, 1) if pick < 45 => ladrillo(Techo::Azotea),
        (ZoneKind::Industrial, _) => ladrillo(Techo::Teja),
    }
}

/// Lo que quiere construir el dueño de un solar, con sus preferencias en orden.
pub fn plan_solar(kind: ZoneKind, density: Density, level: u8, seed: u64, sin_paja: bool) -> Plan {
    let alta = density == Density::Alta && level >= 2;
    let modulos = match (kind, level) {
        (_, 1) => 1.0,
        (ZoneKind::Industrial, 2) => 2.5,
        (_, 2) => 2.0,
        _ => 4.0,
    } * if alta { 1.3 } else { 1.0 };
    let nom = nominal(kind, density, level, seed, None);
    let mut techos = vec![nom.techo];
    let resto: &[Techo] = if level == 1 && kind == ZoneKind::Residencial {
        // El rancho se techa con paja; con la ordenanza (cerca de la plaza) no se puede.
        if sin_paja { &[Techo::Teja, Techo::Tablas, Techo::Cuero, Techo::Paja] } else { &[Techo::Paja, Techo::Cuero] }
    } else {
        &[Techo::Teja, Techo::Azotea, Techo::Tablas, Techo::Paja, Techo::Cuero]
    };
    if sin_paja && level == 1 && kind == ZoneKind::Residencial {
        techos.clear();
    }
    for &t in resto {
        if !techos.contains(&t) {
            techos.push(t);
        }
    }
    let altos = level >= 3 && kind != ZoneKind::Industrial;
    let paredes = match level {
        1 => vec![Pared::Adobe, Pared::Tablas, Pared::Barro],
        2 => vec![Pared::Ladrillo, Pared::Piedra, Pared::Adobe, Pared::Tablas, Pared::Barro],
        // Los altos: piedra antes que ladrillo (más prestigio y no la quema nada).
        _ if altos => vec![Pared::Piedra, Pared::Ladrillo, Pared::Adobe, Pared::Tablas, Pared::Barro],
        _ => vec![Pared::Ladrillo, Pared::Piedra, Pared::Adobe, Pared::Tablas, Pared::Barro],
    };
    let paredes = if level == 1 && kind != ZoneKind::Residencial { vec![Pared::Ladrillo, Pared::Adobe, Pared::Tablas, Pared::Barro] } else { paredes };
    Plan { modulos, paredes, techos, revoque: level >= 2 || kind != ZoneKind::Residencial, rejas: level >= 2, pisos: if altos { 2 } else { 1 } }
}

/// Plan de un edificio de servicio: los ladrillos y la piedra de su ficha, y si faltan, adobe
/// o tablas.
pub fn plan_servicio(k: Kind) -> Plan {
    let i = services::info(k);
    // Un módulo pide 30 de pared y 12 de techo de ladrillo (ver `Pared::receta`).
    let modulos = ((i.ladrillos + i.piedra) / 42.0).max(0.5);
    let paredes = if i.piedra > i.ladrillos * 1.5 { vec![Pared::Piedra, Pared::Ladrillo, Pared::Adobe, Pared::Tablas, Pared::Barro] } else { vec![Pared::Ladrillo, Pared::Piedra, Pared::Adobe, Pared::Tablas, Pared::Barro] };
    Plan { modulos, paredes, techos: vec![Techo::Teja, Techo::Azotea, Techo::Tablas, Techo::Paja, Techo::Cuero], revoque: true, rejas: false, pisos: 1 }
}

/// Lo que dibuja el catálogo para un servicio.
pub fn nominal_servicio() -> Obra {
    Obra::nueva(Pared::Ladrillo, Techo::Teja, true, true)
}

/// Servicios sin obra propia (el campo, el basurero): no piden materiales de obra.
pub fn servicio_sin_obra(k: Kind) -> bool {
    let i = services::info(k);
    i.ladrillos <= 0.0 && i.piedra <= 0.0
}

// --- Vóxeles ---------------------------------------------------------------------------------

/// La variante de dibujo de una obra: con qué materiales se pinta el edificio que el catálogo
/// dibujó con los suyos de siempre (`nominal`). `None` si son los mismos (no se toca nada).
/// Determinista: depende solo de la obra y de la semilla del solar.
pub fn variante(real: &Obra, nominal: &Obra, seed: u64) -> Option<Variante> {
    let rev = real.revoque && real.paredes.admite_revoque();
    let cambia_paredes = (real.paredes, rev) != (nominal.paredes, nominal.revoque);
    let cambia_techo = real.techo != nominal.techo;
    let sin_rejas = nominal.rejas && !real.rejas;
    if !cambia_paredes && !cambia_techo && !sin_rejas {
        return None;
    }
    let paredes = cambia_paredes.then(|| match (real.paredes, rev) {
        (Pared::Piedra, true) => (STONE, STONE_DARK),
        (Pared::Piedra, false) => (MAMPOSTERIA, MAMPOSTERIA_DARK),
        (Pared::Ladrillo, false) => (BRICK, BRICK_DARK),
        (Pared::Adobe, false) => (ADOBE, ADOBE_DARK),
        (Pared::Tablas, _) => (TABLAS, TABLAS_GRIS),
        (Pared::Barro, false) => (QUINCHA, QUINCHA_DARK),
        // Revocado y blanqueado: la cal del catálogo queda; lo demás se encala.
        (_, true) => (CAL, CAL_SUCIA),
    });
    let techo = cambia_techo.then(|| match real.techo {
        Techo::Teja => (TILE, TILE_DARK),
        Techo::Azotea => (AZOTEA, AZOTEA),
        Techo::Tablas => (TABLAS, TABLAS_GRIS),
        Techo::Paja => (THATCH, THATCH_DARK),
        Techo::Cuero => (HIDE, HIDE),
    });
    Some(Variante { paredes, revoque: rev && !matches!(real.paredes, Pared::Piedra), ladrillo_visto: real.paredes == Pared::Ladrillo && rev, techo, rejas: !sin_rejas, seed, cal: None, estilo: None })
}

/// Lleva el dibujo del catálogo (hecho con los materiales `nominal`) a los materiales reales
/// de la obra (ver [`variante`] y `structures::aplicar_variante`).
pub fn remapear(e: &mut Edits, real: &Obra, nominal: &Obra, seed: u64) {
    remapear_con_cal(e, real, nominal, seed, None);
}

/// Como [`remapear`], y además pasa los revoques de cal del dibujo al color `cal` (el de la
/// época y la familia; ver `city::arquitectura`).
pub fn remapear_con_cal(e: &mut Edits, real: &Obra, nominal: &Obra, seed: u64, cal: Option<Mat>) {
    let v = variante(real, nominal, seed);
    if v.is_none() && cal.is_none() {
        return;
    }
    let mut v = v.unwrap_or(Variante { paredes: None, revoque: true, ladrillo_visto: false, techo: None, rejas: true, seed, cal: None, estilo: None });
    v.cal = cal;
    aplicar_variante(e, &v);
}

// --- La ciudad -------------------------------------------------------------------------------

impl City {
    /// Vista de las existencias de todos los materiales.
    pub fn vista_materiales(&self) -> Vista<'_> {
        Vista { eco: &self.economy, mat: &self.materiales }
    }

    /// Cuánto hay de un material ahora.
    pub fn hay_material(&self, m: Material) -> f32 {
        self.vista_materiales().hay(m)
    }

    /// Precio relativo de un material (1 = normal).
    pub fn precio_material(&self, m: Material) -> f32 {
        match m.de_la_economia() {
            Some(r) => self.economy.price(r),
            None => self.materiales.ex[m.idx()].precio,
        }
    }

    /// El entorno de la ciudad como fuente de materiales.
    pub fn entorno_materiales(&self) -> EntornoMateriales {
        let s = &self.services;
        let n = |k: Kind| s.of_kind(k).map(|b| if b.rinde < 0.0 { 1.0 } else { b.rinde }).sum::<f32>();
        let est = &self.suelo.estatico;
        let (arcilla, arena) = if est.is_empty() {
            (0.5, 0.3)
        } else {
            let k = est.len() as f32;
            (est.values().map(|e| e.recursos.arcilla).sum::<f32>() / k, est.values().map(|e| e.recursos.arena).sum::<f32>() / k)
        };
        EntornoMateriales { pop: self.population() as f32, hornos: n(Kind::Horno), canteras: n(Kind::Cantera), talleres: self.lots.jobs().1 as f32, clima: self.climate.brick_factor(), arcilla, arena, cal: self.productos_de_sitios()[crate::city::desarrollo::Prod::Cal.i()] }
    }

    /// ¿La ordenanza sin paja alcanza a este punto?
    fn sin_paja_en(&self, p: glam::Vec2) -> bool {
        self.economy.is_on(Ordinance::SinPaja) && self.lots.plaza_center.is_some_and(|c| c.distance(p) < 400.0)
    }

    /// Plan de lo que se construye en un solar a ese nivel.
    pub fn plan_de_solar(&self, l: &Lot, level: u8) -> Option<Plan> {
        let (k, d) = zone_of(l.zone)?;
        Some(self.plan_con_preferencias(l, level, plan_solar(k, d, level, l.seed, self.sin_paja_en(l.c))))
    }

    /// Elige los materiales con lo que hay, los saca del depósito y anota la demanda: así se
    /// construye todo edificio. Nunca falla (en el peor caso, barro y paja).
    pub fn construir_obra(&mut self, plan: &Plan) -> Obra {
        let obra = elegir(plan, &self.vista_materiales());
        let mat = &mut self.materiales;
        for (m, q) in pedido(plan) {
            mat.mes_pedido[m.idx()] += q;
        }
        for m in obra.faltantes() {
            mat.mes_sust[m.idx()] += 1;
            let q = pedido(plan).iter().find(|x| x.0 == m).map_or(0.0, |x| x.1);
            mat.mes_faltante[m.idx()] += q;
        }
        mat.obras_mes.0 += 1;
        if obra.faltas != 0 {
            mat.obras_mes.1 += 1;
        }
        consumir(&obra, plan.modulos, &mut Deposito { eco: &mut self.economy, mat: &mut self.materiales });
        obra
    }

    /// ¿Alcanzan hoy los materiales para levantar una casa de altos?
    pub fn altos_posibles(&self) -> bool {
        let plan = plan_solar(ZoneKind::Residencial, Density::Media, 3, 0, false);
        elegir(&plan, &self.vista_materiales()).pisos_max() >= 2
    }

    /// Obra de un solar edificado (la guardada o la de su tipo).
    pub fn obra_de(&self, l: &Lot) -> Option<Obra> {
        if l.level == 0 {
            return None;
        }
        l.obra.or_else(|| zone_of(l.zone).map(|(k, d)| nominal(k, d, l.level, l.seed, None)))
    }

    /// Partidas de antes de los materiales: cada edificio queda con los de su dibujo.
    pub fn inferir_obras(&mut self) {
        for l in self.lots.lots.values_mut() {
            if l.level > 0 && l.obra.is_none() {
                l.obra = zone_of(l.zone).map(|(k, d)| nominal(k, d, l.level, l.seed, None));
            }
        }
    }

    /// El mes de los materiales (antes de la economía): desgaste y arreglos, lo que produce el
    /// lugar, lo que trae el puerto, precios y avisos de la gaceta.
    pub fn materiales_mes(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        let mut news = Vec::new();
        // 1. Desgaste, arreglos y derrumbes.
        let ids: Vec<u32> = self.lots.lots.values().filter(|l| l.level > 0 && zone_of(l.zone).is_some()).map(|l| l.id).collect();
        let (mut arreglos, mut caidas) = (0u32, Vec::new());
        for id in ids {
            let l = &self.lots.lots[&id];
            let Some(mut o) = self.obra_de(l) else { continue };
            let Some(plan) = self.plan_de_solar(l, l.level) else { continue };
            o.estado -= 1.0 / (o.vida() * 12.0);
            if o.estado < ESTADO_ARREGLO {
                // El dueño manda arreglar con los mismos materiales, si los hay.
                let pide: Vec<(Material, f32)> = cantidades(&o, plan.modulos).into_iter().map(|(m, q)| (m, q * PARTE_ARREGLO)).collect();
                for &(m, q) in &pide {
                    self.materiales.mes_pedido[m.idx()] += q;
                }
                let v = self.vista_materiales();
                if pide.iter().all(|&(m, q)| m.siempre_local() || v.hay(m) >= q) {
                    let mut dep = Deposito { eco: &mut self.economy, mat: &mut self.materiales };
                    for &(m, q) in &pide {
                        dep.tomar(m, q);
                    }
                    o.estado = 1.0;
                    arreglos += 1;
                } else if o.estado <= 0.0 {
                    caidas.push(id);
                    o.estado = 0.0;
                }
            }
            self.lots.lots.get_mut(&id).unwrap().obra = Some(o);
        }
        // Lo que se vino abajo se levanta de nuevo en el mismo nivel con lo que haya (las
        // casas de altos sin materiales para otros altos quedan apuntaladas).
        let mut levantadas = 0;
        for &id in &caidas {
            let l = &self.lots.lots[&id];
            let Some(plan) = self.plan_de_solar(l, l.level) else { continue };
            if plan.pisos >= 2 && elegir(&plan, &self.vista_materiales()).pisos_max() < 2 {
                self.lots.lots.get_mut(&id).unwrap().obra.as_mut().unwrap().estado = 0.05;
                continue;
            }
            let nivel = self.lots.lots[&id].level;
            let o = self.obra_de_solar(id, nivel, &plan);
            self.lots.lots.get_mut(&id).unwrap().obra = Some(o);
            self.revoxelizar(t, id, e);
            levantadas += 1;
        }
        if levantadas > 0 {
            news.push(format!("Se {} abajo {levantadas} {} de materiales pobres: sus dueños las levantan de nuevo con lo que hay", if levantadas == 1 { "vino" } else { "vinieron" }, if levantadas == 1 { "casa" } else { "casas" }));
        }
        self.materiales.arreglos_ultimo = (arreglos, levantadas);
        // 2. Producción del lugar, puerto y precios.
        let inp = self.month_input();
        let ent = self.entorno_materiales();
        let comercio = inp.port && inp.ships > 0 && !inp.blockade;
        let bodega = inp.ships.max(0) as f32 * BODEGA_MATERIALES;
        let eco: [f32; N] = std::array::from_fn(|i| Material::TODOS[i].de_la_economia().map_or(0.0, |r| self.economy.stock(r).amount));
        news.extend(self.materiales.cerrar_mes(&ent, comercio, bodega, &|m| eco[m.idx()]));
        let foto: [f32; N] = std::array::from_fn(|i| {
            let m = Material::TODOS[i];
            match m.de_la_economia() {
                Some(r) => self.economy.stock(r).amount,
                None => self.materiales.ex[i].cantidad,
            }
        });
        let mat = &mut self.materiales;
        mat.historial.push(foto);
        if mat.historial.len() > 12 {
            mat.historial.remove(0);
        }
        news
    }

    // --- Textos ---------------------------------------------------------------------------

    /// Líneas de la ficha de un edificio: con qué está hecho y qué significa.
    pub fn lineas_obra(&self, o: &Obra) -> Vec<String> {
        let mut v = vec![o.nombre(), format!("{} · estado {} %", capitalizar(&o.efectos()), (o.estado * 100.0).round() as i32)];
        let f = o.faltantes();
        if !f.is_empty() {
            v.push(format!("Se levantó con lo que había: faltaba {}", lista(&f)));
        }
        v
    }

    /// Qué se usaría hoy para un servicio y qué se sustituye (tarjetas y pista al colocar).
    pub fn vista_previa_servicio(&self, k: Kind) -> Option<(Obra, String)> {
        if servicio_sin_obra(k) {
            return None;
        }
        let plan = plan_servicio(k);
        let o = elegir(&plan, &self.vista_materiales());
        let pide = pedido(&plan).iter().map(|(m, q)| format!("{} {}", q.round() as i32, m.nombre())).collect::<Vec<_>>().join(", ");
        let f = o.faltantes();
        let txt = if f.is_empty() { format!("Materiales: {pide}") } else { format!("Materiales: {pide}\nFalta {}: se hará con {}", lista(&f), o.nombre().to_lowercase()) };
        Some((o, txt))
    }
}

/// "ladrillo, cal y hierro".
fn lista(v: &[Material]) -> String {
    let n: Vec<&str> = v.iter().map(|m| m.nombre()).collect();
    match n.len() {
        0 => String::new(),
        1 => n[0].to_string(),
        k => format!("{} y {}", n[..k - 1].join(", "), n[k - 1]),
    }
}

/// Nombre del edificio de un solar según sus materiales («rancho de adobe», «casa de teja»,
/// «casa de tablas»).
pub fn nombre_con_obra(l: &Lot, o: &Obra) -> Option<String> {
    let (k, d) = zone_of(l.zone)?;
    match (k, l.level) {
        (ZoneKind::Residencial, 1) => Some(format!("rancho de {}", o.paredes.nombre())),
        (ZoneKind::Residencial, 2) if d != Density::Alta => Some(match (o.techo, o.paredes) {
            (Techo::Azotea, _) => "casa de azotea".to_string(),
            (_, Pared::Tablas) => "casa de tablas".to_string(),
            (_, Pared::Barro) => "casa de barro y paja".to_string(),
            (Techo::Teja, _) => "casa de teja".to_string(),
            (t, p) => format!("casa de {} con techo de {}", p.nombre(), t.nombre()),
        }),
        _ => None,
    }
}

fn capitalizar(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}
