//! Oficios y sucesos de la ciudad colonial (`docs/sistemas/ia-y-transito.md` §4.1, los
//! pendientes de la v0.12.0):
//!
//! - **Aguadores**: cada puesto de aguadores larga sus carros con la pipa por la mañana; cada
//!   carro llena en la aguada (el pozo más cercano, o la orilla) y recorre unas casas de las
//!   que el puesto abastece (las que no tienen aljibe ni pozo propio), parando en cada puerta.
//! - **Serenos**: al anochecer encienden los faroles de sebo de su puesto (paran en cada uno)
//!   y después rondan la noche cantando la hora; llevan el farol de mano y el chuzo.
//! - **Incendios**: cuando un solar se prende, los vecinos de alrededor corren a mirar y, si
//!   hay un pozo o un aljibe que alcanza, se forma la cadena de baldes desde ahí hasta la casa.
//! - **Carretas empantanadas**: ya no salen solas a los pocos minutos: esperan a que alguien
//!   que pasa (a pie o a caballo, con el lazo) se pare a ayudar; sin ayuda esperan mucho.
//! - **Salida de misa y corrillos**: la misa mayor del domingo termina a la misma hora para
//!   todos (ver `agenda`) y la gente se queda conversando en grupos en el atrio; en la recova
//!   se juntan corrillos y el domingo a la tarde se da la vuelta a la plaza. Los grupos se
//!   acomodan con una **fuerza social** (Helbing y Molnár, 1995: repulsión exponencial entre
//!   personas, atracción al centro de su grupo y la fachada como pared), calculada una vez por
//!   corrillo: con pocos agentes la plaza se llena.
//! - **Gente sentada**: en la pulpería, en el banco de la puerta; en la vereda, a la tardecita,
//!   en el poyo de la casa o en una silla de paja.
//!
//! Los números son de diseño (`assets/datos/transito_colonial.json`, sección `oficios`). Nada
//! de esto cambia la economía: son agentes y escenas que muestran lo que la ciudad ya calcula.

use super::datos::datos;
use super::perfil::{azar_n, Motivo};
use super::{frente, pose, Agentes, Lugar, Marcha, Programado, Proposito, Quien, Tipo, APELLIDOS};
use crate::city::lots::{side_of, Lots};
use crate::city::roads::RoadNetwork;
use crate::city::services::{Kind, Services};
use crate::detalle::ropa::{mezcla, vestir};
use crate::detalle::{anim, biblioteca, emitir, modelos, sombra, Caja, Id, Parte, Pose};
use crate::lod::{BASE_VOXEL, DESIGN_VOXEL};
use crate::terrain::TerrainGen;
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Un incendio que se ve: la casa, desde y hasta cuándo (reloj simulado) y la fuente de agua
/// de la cadena de baldes (si hay una que alcanza).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Incendio {
    pub lote: u32,
    pub pos: [f32; 2],
    pub fuente: Option<[f32; 2]>,
    pub desde: f64,
    pub hasta: f64,
}

/// Un puesto (de aguadores o de serenos): su lugar y sus puntos (las casas que abastece o los
/// faroles que enciende).
#[derive(Clone, Debug, Default)]
pub struct Puesto {
    pub id: u32,
    pub pos: Vec2,
    pub puntos: Vec<Vec2>,
    /// Dónde llena la pipa (aguadores).
    pub aguada: Option<Vec2>,
}

/// Los puestos de la ciudad (derivado de los servicios; se rehace cuando cambia la firma).
#[derive(Clone, Debug, Default)]
pub struct Oficios {
    firma: u64,
    pub aguadores: Vec<Puesto>,
    pub serenos: Vec<Puesto>,
}

fn firma(s: &Services) -> u64 {
    let mut h = (s.buildings.len() as u64) ^ (s.water.len() as u64) << 20 ^ (s.lamps.len() as u64) << 40;
    for (l, w) in &s.water {
        h = h.wrapping_mul(0x100_0000_01B3) ^ ((*l as u64) << 24 | w.1 as u64);
    }
    for b in s.buildings.values() {
        h = h.wrapping_mul(0x100_0000_01B3) ^ (b.id as u64) ^ (b.kind as u64) << 32;
    }
    h | 1
}

impl Oficios {
    pub fn armar(s: &Services, lots: &Lots) -> Oficios {
        let d = &datos().oficios;
        let mut o = Oficios { firma: firma(s), ..Default::default() };
        let pozos: Vec<Vec2> = s.of_kind(Kind::Pozo).map(|b| b.c).collect();
        for b in s.of_kind(Kind::Aguadores) {
            let mut casas: Vec<(u32, Vec2)> = s.water.iter().filter(|(_, w)| w.1 == b.id).filter_map(|(l, _)| lots.lots.get(l).map(|x| (*l, frente(x)))).collect();
            // Alrededor del puesto (cada viaje se lleva un tramo de la vuelta).
            casas.sort_by(|x, y| (x.1 - b.c).to_angle().total_cmp(&(y.1 - b.c).to_angle()).then(x.0.cmp(&y.0)));
            let aguada = pozos.iter().copied().filter(|p| p.distance(b.c) < d.aguada_max_m).min_by(|x, y| x.distance(b.c).total_cmp(&y.distance(b.c)));
            o.aguadores.push(Puesto { id: b.id, pos: b.c, puntos: casas.into_iter().map(|c| c.1).collect(), aguada });
        }
        for b in s.of_kind(Kind::Serenos) {
            let r = b.info().radius;
            let mut f: Vec<Vec2> = s.lamps.iter().map(|(v, _)| (v.as_vec2() + Vec2::splat(0.5)) * DESIGN_VOXEL).filter(|p| p.distance(b.c) <= r).collect();
            f.sort_by(|x, y| (*x - b.c).to_angle().total_cmp(&(*y - b.c).to_angle()));
            o.serenos.push(Puesto { id: b.id, pos: b.c, puntos: f, aguada: None });
        }
        o
    }
}

/// Recorrido: de `desde`, siempre al punto más cercano que falta.
fn mas_cercano_cada_vez(desde: Vec2, mut resto: Vec<Vec2>) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(resto.len());
    let mut at = desde;
    while !resto.is_empty() {
        let i = (0..resto.len()).min_by(|&a, &b| resto[a].distance(at).total_cmp(&resto[b].distance(at))).unwrap();
        at = resto.swap_remove(i);
        out.push(at);
    }
    out
}

impl Agentes {
    /// Los puestos de aguadores y serenos de la ciudad (cuando cambian los servicios).
    pub(super) fn actualizar_oficios(&mut self, s: &Services, lots: &Lots) {
        if self.oficios.firma != firma(s) {
            self.oficios = Oficios::armar(s, lots);
        }
    }

    /// Los viajes del día de aguadores (no los domingos) y serenos.
    pub(super) fn programar_oficios(&self, ag: &mut Vec<Programado>, domingo: bool) {
        let d = &datos().oficios;
        let (sem, dia) = (self.e.semilla, self.e.dia);
        if !domingo {
            for p in &self.oficios.aguadores {
                if p.puntos.is_empty() {
                    continue;
                }
                let n = (p.puntos.len() as u32).div_ceil(d.aguador_casas_por_viaje.max(1)).clamp(1, d.aguador_viajes_max.max(1));
                for j in 0..n {
                    let x = azar_n(sem, p.id as u64 | 1 << 40, dia, Motivo::Oficio as u32, j);
                    let h = d.aguador_hora[0] + (d.aguador_hora[1] - d.aguador_hora[0]) * (j as f32 + x) / n as f32;
                    ag.push(Programado { hora: h, quien: Quien::Aguador(p.id, j as u8), que: Proposito::Aguador });
                }
            }
        }
        for p in &self.oficios.serenos {
            if p.puntos.is_empty() {
                continue;
            }
            let n = (p.puntos.len() as u32).div_ceil(d.sereno_faroles_por_sereno.max(1)).clamp(1, d.serenos_max.max(1));
            for j in 0..n {
                let jit = 0.2 * azar_n(sem, p.id as u64 | 2 << 40, dia, Motivo::Oficio as u32, j);
                ag.push(Programado { hora: d.sereno_enciende_hora + jit, quien: Quien::Sereno(p.id, j as u8, true), que: Proposito::EnciendeFaroles });
                for &h in &d.sereno_rondas {
                    ag.push(Programado { hora: (h + jit).rem_euclid(24.0), quien: Quien::Sereno(p.id, j as u8, false), que: Proposito::Ronda });
                }
            }
        }
    }

    /// Larga un viaje con paradas. Devuelve si salió (si no salió porque había otro vehículo
    /// encima, no queda la vuelta pendiente sin sus paradas).
    #[allow(clippy::too_many_arguments)]
    fn lanzar_recorrido(&mut self, net: &RoadNetwork, tipo: Tipo, desde: Vec2, primera: Vec2, resto: Vec<Vec2>, que: Proposito, parada_seg: f32, en_parada: Marcha) -> bool {
        let n = self.e.pendientes.len();
        let c = (self.azar_u(12) as u8, self.azar_u(APELLIDOS.len() as u32) as u8);
        let sem = self.azar_u(100_000);
        if !self.lanzar(net, tipo, desde, primera, None, que, sem, 1, c) {
            self.e.pendientes.truncate(n);
            return false;
        }
        let a = self.e.agentes.last_mut().unwrap();
        a.paradas = resto.into_iter().map(Into::into).collect();
        a.parada_seg = parada_seg;
        a.en_parada = Some(en_parada);
        true
    }

    /// Un carro de aguador: del puesto a la aguada, por las casas de su tramo y de vuelta.
    pub(super) fn lanzar_aguador(&mut self, net: &RoadNetwork, sid: u32, j: u8) -> bool {
        let d = &datos().oficios;
        let Some(p) = self.oficios.aguadores.iter().find(|p| p.id == sid).cloned() else { return false };
        let n = p.puntos.len();
        if n == 0 {
            return false;
        }
        let k = (d.aguador_casas_por_viaje.max(1) as usize).min(n);
        let viajes = n.div_ceil(k);
        // Cada viaje, un tramo de la vuelta alrededor del puesto (rota con los días).
        let desde = ((j as usize + self.e.dia as usize) % viajes) * k;
        let casas: Vec<Vec2> = p.puntos.iter().cycle().skip(desde).take(k).copied().collect();
        let aguada = p.aguada.or(self.lugares.puerto).unwrap_or(p.pos);
        let mut paradas = mas_cercano_cada_vez(aguada, casas);
        paradas.push(p.pos);
        // La primera parada (la aguada) es la de llenar la pipa.
        if self.lanzar_recorrido(net, Tipo::Carro, p.pos, aguada, paradas, Proposito::Aguador, d.aguador_llenar_s, Marcha::DejaAgua) {
            self.e.stats.aguadores_hoy += 1;
            return true;
        }
        false
    }

    /// Un sereno: enciende los faroles de su parte (al anochecer) o ronda cantando la hora.
    pub(super) fn lanzar_sereno(&mut self, net: &RoadNetwork, sid: u32, j: u8, enciende: bool) -> bool {
        let d = &datos().oficios;
        let Some(p) = self.oficios.serenos.iter().find(|p| p.id == sid).cloned() else { return false };
        if p.puntos.is_empty() {
            return false;
        }
        let n = (p.puntos.len()).div_ceil(d.sereno_faroles_por_sereno.max(1) as usize).clamp(1, d.serenos_max.max(1) as usize);
        let mios: Vec<Vec2> = p.puntos.iter().enumerate().filter(|(i, _)| i % n == j as usize % n).map(|(_, q)| *q).collect();
        if mios.is_empty() {
            return false;
        }
        // Las paradas: repartidas entre sus faroles (al encender) o la mitad (en la ronda).
        let tope = if enciende { d.sereno_paradas_max } else { (d.sereno_paradas_max / 2).max(2) }.max(1);
        let paso = mios.len().div_ceil(tope).max(1);
        let elegidos: Vec<Vec2> = mios.iter().step_by(paso).copied().collect();
        let mut paradas = mas_cercano_cada_vez(p.pos, elegidos);
        let primera = paradas.remove(0);
        paradas.push(p.pos);
        let (que, seg) = if enciende { (Proposito::EnciendeFaroles, d.sereno_parada_enciende_s) } else { (Proposito::Ronda, d.sereno_parada_ronda_s) };
        if self.lanzar_recorrido(net, Tipo::APie, p.pos, primera, paradas, que, seg, Marcha::CantaLaHora) {
            self.e.stats.rondas_hoy += 1;
            return true;
        }
        false
    }

    /// Llegó a una parada: se queda ahí lo que dura la parada y sigue hacia la siguiente.
    /// Devuelve `false` si no le quedan paradas a las que pueda ir (entonces llegó).
    pub(super) fn siguiente_parada(&mut self, net: &RoadNetwork, i: usize) -> bool {
        let d = &datos().oficios;
        let reloj = self.e.reloj;
        // Lo que hizo en esta parada.
        {
            let a = &self.e.agentes[i];
            match a.proposito {
                Proposito::Aguador if a.parada_seg == d.aguador_parada_s => self.e.stats.casas_con_agua_hoy += 1,
                Proposito::EnciendeFaroles => self.e.stats.faroles_hoy += 1,
                _ => {}
            }
        }
        let espera = self.e.agentes[i].parada_seg;
        loop {
            let a = &self.e.agentes[i];
            let Some(&sig) = a.paradas.first() else { return false };
            let aqui = pose(net, a).0;
            let (tipo, sem) = (a.tipo, a.id);
            let dest = Vec2::from(sig);
            let patas = if aqui.distance(dest) < 3.0 { None } else { self.planificar(net, aqui, dest, tipo.vehiculo().then_some(sem)) };
            self.e.agentes[i].paradas.remove(0);
            let Some(patas) = patas else { continue };
            let portones = self.cruces_de_portones(net, &patas);
            let a = &mut self.e.agentes[i];
            a.s = patas[0].desde;
            a.patas = patas;
            a.li = 0;
            a.v = 0.0;
            a.portones = portones;
            a.porton_i = 0;
            a.revision = 0.0;
            a.en_cola = 0.0;
            a.llegada = -1.0;
            a.adelantando = None;
            a.antes = None;
            a.destino = sig;
            a.detenido_hasta = reloj + espera as f64;
            a.t_carril = reloj + espera as f64;
            a.marcha = a.en_parada.unwrap_or(Marcha::EsperandoEnLaFila);
            // Después de llenar la pipa, las paradas son en las casas.
            if a.proposito == Proposito::Aguador {
                a.parada_seg = d.aguador_parada_s;
            }
            return true;
        }
    }

    /// Los incendios que empezaron (los solares que arden y todavía no se vieron): se anotan
    /// con su fuente de agua y los vecinos de alrededor corren a mirar.
    pub(super) fn revisar_incendios(&mut self, net: &RoadNetwork, lots: &Lots, s: &Services) {
        let d = &datos().oficios;
        let reloj = self.e.reloj;
        self.e.incendios.retain(|i| i.hasta > reloj);
        let nuevos: Vec<u32> = s.fires.burning.keys().copied().filter(|l| !self.e.incendios.iter().any(|i| i.lote == *l)).collect();
        for lote in nuevos {
            let Some(l) = lots.lots.get(&lote) else { continue };
            let pos = frente(l);
            let fuente = s.fire_source(l.c).and_then(|id| s.buildings.get(&id)).map(|b| b.c);
            self.e.incendios.push(Incendio { lote, pos: pos.into(), fuente: fuente.map(Into::into), desde: reloj, hasta: reloj + d.incendio_dura_s as f64 });
            // Los curiosos: de las casas de alrededor (las más cercanas, salteadas).
            let mut cerca: Vec<(f32, u32, Vec2)> = lots.lots.values().filter(|x| x.level > 0 && x.id != lote).map(|x| (frente(x).distance(pos), x.id, frente(x))).filter(|c| c.0 > 12.0 && c.0 < d.incendio_radio_m).collect();
            cerca.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            let mut n = 0;
            for (k, &(_, id, origen)) in cerca.iter().enumerate() {
                if n >= d.incendio_curiosos_max {
                    break;
                }
                if azar_n(self.e.semilla, id as u64, lote, Motivo::Curioso as u32, k as u32) > 0.55 {
                    continue;
                }
                // Del otro lado de la calle, desparramados frente a la casa.
                let x = azar_n(self.e.semilla, id as u64, lote, Motivo::Curioso as u32, 1000 + k as u32);
                let lado = side_of(l.f);
                let dest = pos + l.f * (5.0 + 4.0 * x) + lado * ((x * 7.3).fract() - 0.5) * 22.0;
                let mirar = d.incendio_mirar_s[0] + (d.incendio_mirar_s[1] - d.incendio_mirar_s[0]) * (x * 3.1).fract();
                if self.lanzar_recorrido(net, Tipo::APie, origen, dest, vec![origen], Proposito::AlIncendio, mirar, Marcha::MiraElFuego) {
                    n += 1;
                    self.e.stats.curiosos_hoy += 1;
                }
            }
        }
    }

    /// Las carretas empantanadas esperan ayuda: alguien que pasa cerca por la misma calle (a
    /// pie o a caballo) se para, ayuda unos minutos y la carreta sale.
    pub(super) fn ayudar_empantanadas(&mut self, net: &RoadNetwork) {
        let reloj = self.e.reloj;
        let ags = &self.e.agentes;
        let atascadas: Vec<usize> = (0..ags.len()).filter(|&i| ags[i].tipo == Tipo::Carreta && ags[i].ayuda.is_none() && reloj < ags[i].detenido_hasta && ags[i].en_parada.is_none()).collect();
        if atascadas.is_empty() {
            return;
        }
        let d = &datos().oficios;
        for ci in atascadas {
            let ags = &self.e.agentes;
            let c = &ags[ci];
            let tramo = c.pata().tramo;
            let pc = pose(net, c).0;
            let ayudante = (0..ags.len())
                .filter(|&j| j != ci)
                .filter(|&j| {
                    let b = &ags[j];
                    matches!(b.tipo, Tipo::APie | Tipo::Jinete) && b.ayuda.is_none() && b.paradas.is_empty() && reloj >= b.detenido_hasta && b.pata().tramo == tramo && (b.perfil.edad == 0 || (14..70).contains(&b.perfil.edad))
                })
                .map(|j| (pose(net, &ags[j]).0.distance(pc), j))
                .filter(|x| x.0 < d.barro_radio_ayuda_m)
                .min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            let Some((_, j)) = ayudante else { continue };
            let x = azar_n(self.e.semilla, c.id as u64, self.e.dia, Motivo::Ayuda as u32, 0);
            let seg = (d.barro_ayuda_min[0] + (d.barro_ayuda_min[1] - d.barro_ayuda_min[0]) * x) * 60.0;
            let (cid, bid) = (c.id, ags[j].id);
            let b = &mut self.e.agentes[j];
            b.ayuda = Some(cid);
            b.detenido_hasta = reloj + seg as f64;
            b.en_parada = Some(Marcha::Ayudando);
            b.v = 0.0;
            let c = &mut self.e.agentes[ci];
            c.ayuda = Some(bid);
            c.detenido_hasta = c.detenido_hasta.min(reloj + seg as f64);
            self.e.stats.ayudas_hoy += 1;
        }
    }
}

// --- Corrillos: grupos de gente parada, con fuerza social -----------------------------------

/// Lugares donde se juntan corrillos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Corro {
    /// El atrio de la iglesia matriz, a la salida de misa.
    Atrio,
    /// La recova (el mercado de la plaza).
    Recova,
}

/// Un corrillo armado: los lugares (con su giro, en el marco del lugar: x al costado, y hacia
/// adentro de la plaza) y quién ocupa cada uno. Nadie cambia de lugar mientras se queda; el
/// que se va deja el suyo libre.
#[derive(Clone, Debug, Default)]
struct Corrillo {
    clave: u64,
    lugares: Arc<Vec<(Vec2, f32)>>,
    ocupa: HashMap<u64, usize>,
}

/// Los corrillos de la ciudad (no se guardan: salen de quién está en cada lugar).
#[derive(Debug, Default)]
pub struct Corrillos {
    por_lugar: Mutex<HashMap<Corro, Corrillo>>,
}

impl Clone for Corrillos {
    fn clone(&self) -> Self {
        Corrillos::default()
    }
}

/// Lugares para `n` personas en grupos de 2 a 6 frente a una pared (y = 0), relajados con una
/// fuerza social: repulsión exponencial entre personas (A·e^((r−d)/B), Helbing y Molnár 1995,
/// con A = 2, B = 0,3 m y r = 0,55 m), atracción de cada uno al círculo de su grupo y la pared.
/// Cada persona mira al centro de su grupo. Determinista (sale de la semilla).
pub fn corrillo(n: usize, radio: f32, semilla: u64) -> Vec<(Vec2, f32)> {
    let mut grupos: Vec<(usize, usize, f32)> = Vec::new(); // (desde, cuántos, radio del grupo)
    let mut p: Vec<Vec2> = Vec::with_capacity(n);
    let mut de: Vec<usize> = Vec::with_capacity(n);
    let mut g = 0u64;
    let total_grupos = (n as f32 / 3.5).max(1.0);
    while p.len() < n {
        let t = (2 + (mezcla(semilla ^ g.wrapping_mul(0x9E37)) % 5) as usize).min(n - p.len());
        // Centros en la espiral de Vogel sobre el semidisco frente a la pared.
        let a = g as f32 * 2.399_963;
        let r = radio * ((g as f32 + 0.5) / total_grupos).sqrt().min(1.0);
        let c = Vec2::new(r * a.cos(), (r * a.sin()).abs() + 1.2);
        let rg = 0.38 + 0.13 * t as f32;
        let a0 = (mezcla(semilla ^ g ^ 0xABC) % 628) as f32 / 100.0;
        grupos.push((p.len(), t, rg));
        for m in 0..t {
            let ang = a0 + m as f32 / t as f32 * std::f32::consts::TAU;
            p.push(c + Vec2::from_angle(ang) * rg);
            de.push(grupos.len() - 1);
        }
        g += 1;
    }
    let (fa, fb, fr) = (2.0f32, 0.3f32, 0.55f32);
    let mut celdas: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for _ in 0..60 {
        let mut f = vec![Vec2::ZERO; n];
        // Repulsión entre personas (solo las de celdas vecinas de 2 m).
        celdas.clear();
        for (i, q) in p.iter().enumerate() {
            celdas.entry(((q.x / 2.0).floor() as i32, (q.y / 2.0).floor() as i32)).or_default().push(i);
        }
        for (i, q) in p.iter().enumerate() {
            let (cx, cy) = ((q.x / 2.0).floor() as i32, (q.y / 2.0).floor() as i32);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(v) = celdas.get(&(cx + dx, cy + dy)) else { continue };
                    for &j in v {
                        if j == i {
                            continue;
                        }
                        let d = *q - p[j];
                        let dist = d.length().max(0.01);
                        if dist < 2.0 {
                            f[i] += d / dist * fa * ((fr - dist) / fb).exp();
                        }
                    }
                }
            }
        }
        // Cada uno al círculo de su grupo (alrededor del centro actual del grupo).
        for &(desde, t, rg) in &grupos {
            let c = p[desde..desde + t].iter().copied().sum::<Vec2>() / t as f32;
            for i in desde..desde + t {
                let to = c - p[i];
                let dist = to.length().max(0.01);
                f[i] += to / dist * 3.0 * (dist - rg);
            }
        }
        // La pared (la fachada de la iglesia o el borde de la recova).
        for i in 0..n {
            if p[i].y < 0.6 {
                f[i].y += 4.0 * (0.6 - p[i].y);
            }
            p[i] += f[i].clamp_length_max(3.0) * 0.08;
        }
    }
    (0..n)
        .map(|i| {
            let (desde, t, _) = grupos[de[i]];
            let c = p[desde..desde + t].iter().copied().sum::<Vec2>() / t as f32;
            let to = c - p[i];
            (p[i], to.x.atan2(to.y))
        })
        .collect()
}

impl Corrillos {
    /// Quién ocupa cada lugar de un corrillo: los que siguen, en el suyo; los nuevos, en el
    /// primero libre. `presentes` ordenados; `capacidad` para la primera vez (se arma una vez
    /// por `clave`: el día, por ejemplo).
    fn ocupar(&self, corro: Corro, clave: u64, presentes: &[u64], capacidad: usize, radio: f32) -> (Arc<Vec<(Vec2, f32)>>, Vec<(u64, usize)>) {
        let mut g = self.por_lugar.lock().unwrap_or_else(|e| e.into_inner());
        let c = g.entry(corro).or_default();
        if c.clave != clave || c.lugares.is_empty() {
            let n = capacidad.max(presentes.len() + presentes.len() / 8 + 8).min(900);
            *c = Corrillo { clave, lugares: Arc::new(corrillo(n, radio, clave)), ocupa: HashMap::new() };
        }
        let quedan: std::collections::HashSet<u64> = presentes.iter().copied().collect();
        c.ocupa.retain(|k, _| quedan.contains(k));
        let mut usados = vec![false; c.lugares.len()];
        for &i in c.ocupa.values() {
            usados[i] = true;
        }
        let mut libre = 0;
        for &k in presentes {
            if c.ocupa.contains_key(&k) {
                continue;
            }
            while libre < usados.len() && usados[libre] {
                libre += 1;
            }
            if libre == usados.len() {
                break;
            }
            usados[libre] = true;
            c.ocupa.insert(k, libre);
        }
        let mut out: Vec<(u64, usize)> = c.ocupa.iter().map(|(&k, &i)| (k, i)).collect();
        out.sort_unstable();
        (c.lugares.clone(), out)
    }
}

// --- Dibujo de las escenas ------------------------------------------------------------------

/// Altura de la superficie de la plaza (como `voxelize::plaza_edits`).
fn suelo_plaza(t: &TerrainGen, pc: Vec2) -> f32 {
    ((crate::city::voxelize::road_height(t, pc) / BASE_VOXEL - 0.5).floor() + 1.0) * BASE_VOXEL
}

/// Una persona parada (conversando: mira a los otros, mueve los brazos de a ratos).
fn parada(out: &mut Vec<Caja>, q: &crate::detalle::ropa::Quien, anio: i32, base: Vec3, giro: f32, t: f32, id: u32) {
    let b = biblioteca();
    let (at, pal) = vestir(q, anio);
    let mut pose = anim::persona_quieta(t, id);
    // Gesticula de a ratos.
    let g = ((t * 0.9 + id as f32 * 1.3).sin() * 0.5 + 0.5).powi(8);
    pose.set(Parte::BrazoD, 0.0, -0.6 * g);
    emitir(out, b.persona(at, false), &pal, base, giro, &pose, id);
    sombra(out, base, giro, [0.27, 0.2], 130);
}

/// Una persona sentada en un asiento de 0,5 m (`asiento`: el punto del asiento).
fn sentada(out: &mut Vec<Caja>, q: &crate::detalle::ropa::Quien, anio: i32, asiento: Vec3, giro: f32, t: f32, id: u32) {
    let b = biblioteca();
    let (at, pal) = vestir(q, anio);
    let mut pose = anim::persona_quieta(t, id);
    pose.set(Parte::BrazoI, 0.0, -0.35);
    pose.set(Parte::BrazoD, 0.0, -0.35 - 0.4 * ((t * 0.5 + id as f32).sin() * 0.5 + 0.5).powi(10));
    emitir(out, b.sentada(at), &pal, asiento, giro, &pose, id);
    sombra(out, asiento - Vec3::Y * 0.5, giro, [0.27, 0.3], 120);
}

/// Distancia máxima (m) a la que se dibujan las escenas (como los detalles fijos).
const RADIO_ESCENAS: f32 = 140.0;

impl Agentes {
    /// Las escenas de los oficios y sucesos cerca de `centro`: la cadena de baldes de los
    /// incendios, los corrillos del atrio y de la recova, la vuelta de la plaza, la gente
    /// sentada en las pulperías y en las veredas.
    pub fn cajas_oficios(&self, c: &crate::city::City, t: &TerrainGen, centro: Vec2, v: &super::dibujo::Vestuario, out: &mut Vec<Caja>) {
        let segs = v.t;
        self.cajas_incendios(c, t, centro, v, out);
        let Some(pc) = self.lugares.plaza else { return };
        let iglesia = self.lugares.iglesia.unwrap_or(pc);
        let f = (pc - iglesia).normalize_or(Vec2::Y);
        let lado = side_of(f);
        let y_plaza = suelo_plaza(t, pc);
        let hora = self.e.hora;
        let dia = self.e.dia;
        let domingo = dia % 7 == 6;
        let d = &datos().oficios;
        // Quiénes están en la plaza o en la iglesia (sin agente: llegaron y se quedan).
        let mut atrio: Vec<u64> = Vec::new();
        let mut recova: Vec<u64> = Vec::new();
        let mut paseo: Vec<u64> = Vec::new();
        let mut pulperia: HashMap<u32, Vec<u64>> = HashMap::new();
        let mut vereda: HashMap<u32, Vec<u64>> = HashMap::new();
        let en_vereda = !self.e.lluvia && hora >= d.vereda_hora[0] && hora <= d.vereda_hora[1];
        for (&k, vc) in &self.e.vecinos {
            if vc.en_camino.is_some() {
                continue;
            }
            let ultima = vc.paso_plan.checked_sub(1).and_then(|i| vc.plan.get(i as usize)).map(|a| a.que);
            match vc.lugar {
                Lugar::Iglesia if domingo && hora >= d.misa_fin_domingo - 0.01 => atrio.push(k),
                Lugar::Plaza if ultima == Some(Proposito::APaseo) => paseo.push(k),
                Lugar::Plaza => recova.push(k),
                Lugar::Solar(l) if matches!(ultima, Some(Proposito::APulperia | Proposito::ACompra)) => pulperia.entry(l).or_default().push(k),
                Lugar::Casa if en_vereda && (vc.perfil.edad >= 35 || vc.perfil.edad == 0) && super::perfil::azar(self.e.semilla, k, dia, Motivo::Vereda) < d.vereda_fraccion => vereda.entry(vc.casa).or_default().push(k),
                _ => {}
            }
        }
        let persona_de = |k: u64| self.e.vecinos.get(&k).and_then(|vc| vc.persona);
        let quien = |k: u64| super::dibujo::quien_de(self, persona_de(k), Some(k), k, v);
        // Atrio: frente a la iglesia, del lado de la plaza.
        let origen_atrio = pc - f * (crate::city::voxelize::PLAZA_SIZE * 0.5 - 1.0);
        if !atrio.is_empty() && origen_atrio.distance(centro) < RADIO_ESCENAS + 30.0 {
            let (lugares, quien_va) = self.corrillos.ocupar(Corro::Atrio, dia as u64 * 7 + 1, &atrio, atrio.len(), 6.0 + (atrio.len() as f32).sqrt() * 0.85);
            for (k, i) in quien_va {
                let (q, g) = lugares[i];
                let p = origen_atrio + lado * q.x + f * q.y;
                let giro = g + f.x.atan2(f.y);
                parada(out, &quien(k), v.anio, Vec3::new(p.x, y_plaza, p.y), giro, segs, k as u32);
            }
        }
        // La recova: corrillos alrededor del mercado, en el medio de la plaza.
        let origen_recova = pc + lado * 9.0 + f * 4.0;
        if !recova.is_empty() && origen_recova.distance(centro) < RADIO_ESCENAS + 30.0 {
            let (lugares, quien_va) = self.corrillos.ocupar(Corro::Recova, dia as u64 * 7 + 2, &recova, 160, 14.0);
            for (k, i) in quien_va {
                let (q, g) = lugares[i];
                let p = origen_recova + lado * q.x + f * q.y;
                parada(out, &quien(k), v.anio, Vec3::new(p.x, y_plaza, p.y), g + f.x.atan2(f.y), segs, k as u32);
            }
        }
        // La vuelta de la plaza (el domingo a la tarde): de a dos, despacio, alrededor.
        if !paseo.is_empty() && pc.distance(centro) < RADIO_ESCENAS + 40.0 {
            let b = biblioteca();
            for (n, &k) in paseo.iter().enumerate() {
                let s = mezcla(k ^ 0x9A5E0);
                let r = 27.0 + (s % 7) as f32 * 1.1 + if n % 2 == 1 { 0.65 } else { 0.0 };
                let v_m = 0.75 + (s >> 8 & 15) as f32 * 0.01;
                let a0 = ((n / 2) as f32) * 0.37 + (s >> 12 & 63) as f32 * 0.002;
                let ang = a0 + segs * v_m / r;
                let p = pc + Vec2::from_angle(ang) * r;
                let dir = Vec2::from_angle(ang + std::f32::consts::FRAC_PI_2);
                let giro = dir.x.atan2(dir.y);
                let (at, pal) = vestir(&quien(k), v.anio);
                let pose = anim::caminar(anim::fase(ang * r, anim::CICLO_PERSONA), 0.8, 0.0);
                let base = Vec3::new(p.x, y_plaza, p.y);
                emitir(out, b.persona(at, false), &pal, base, giro, &pose, k as u32);
                sombra(out, base, giro, [0.27, 0.2], 130);
            }
        }
        // Pulperías: los dos primeros en el banco de la puerta, los demás parados en el
        // mostrador; en la vereda, el poyo de la casa o una silla.
        let b = biblioteca();
        for (lote, ks) in pulperia {
            let Some(l) = c.lots.lots.get(&lote) else { continue };
            let fr = frente(l);
            if fr.distance(centro) > RADIO_ESCENAS {
                continue;
            }
            let lado = side_of(l.f);
            let esquina = if l.seed % 2 == 0 { 1.0 } else { -1.0 };
            let y = self.altura_cerca(c, t, fr + l.f * 0.6);
            let giro = l.f.x.atan2(l.f.y);
            let banco = fr + l.f * 0.6 - lado * esquina * 0.9;
            emitir(out, b.objeto(Id::Banco), &modelos::paleta(Id::Banco, lote), Vec3::new(banco.x, y, banco.y), giro + std::f32::consts::FRAC_PI_2, &Pose::default(), lote);
            for (n, &k) in ks.iter().enumerate() {
                if n < 2 {
                    // El banco mira a la calle (se sientan de espaldas a la pared).
                    let p = banco + lado * esquina * (if n == 0 { -0.45 } else { 0.45 });
                    sentada(out, &quien(k), v.anio, Vec3::new(p.x, y + 0.5, p.y), giro, segs, k as u32);
                } else {
                    let a = n as f32 * 1.9;
                    let p = fr + l.f * 1.4 + lado * esquina * 1.5 + Vec2::from_angle(a) * 0.9;
                    let to = fr + lado * esquina * 1.5 - p;
                    parada(out, &quien(k), v.anio, Vec3::new(p.x, y, p.y), to.x.atan2(to.y), segs, k as u32);
                }
            }
        }
        for (casa, ks) in vereda {
            let Some(l) = c.lots.lots.get(&casa) else { continue };
            if l.level == 0 || frente(l).distance(centro) > RADIO_ESCENAS * 0.7 {
                continue;
            }
            let lado = side_of(l.f);
            let giro = l.f.x.atan2(l.f.y);
            // El poyo de la casa, si tiene; si no, sillas sacadas a la vereda.
            let poyos: Vec<(Vec3, f32)> = c.fachadas.de_solar(casa, &c.lots).iter().filter(|p| p.id == Id::BancoPiedra).map(|p| (p.pos + crate::detalle::girar(Vec3::new(0.0, 0.5, 0.22), p.giro), p.giro)).collect();
            for (n, &k) in ks.iter().take(2).enumerate() {
                let (asiento, g) = match poyos.first() {
                    Some(&(p, g)) => (p + crate::detalle::girar(Vec3::new(if n == 0 { -0.3 } else { 0.3 }, 0.0, 0.0), g), g),
                    None => {
                        let q = frente(l) + l.f * 0.7 + lado * (if n == 0 { 1.6 } else { 2.4 });
                        let y = self.altura_cerca(c, t, q);
                        emitir(out, b.objeto(Id::Silla), &modelos::paleta(Id::Silla, k as u32), Vec3::new(q.x, y, q.y), giro, &Pose::default(), k as u32);
                        (Vec3::new(q.x, y + 0.5, q.y), giro)
                    }
                };
                sentada(out, &quien(k), v.anio, asiento, g, segs, k as u32);
            }
        }
    }

    /// Altura de la calle más cercana a `p` (la de los agentes), o la del terreno.
    fn altura_cerca(&self, c: &crate::city::City, t: &TerrainGen, p: Vec2) -> f32 {
        if let Some(n) = c.net.nearest_edge(p, 14.0) {
            if let Some(pf) = self.perfiles.get(&n.edge) {
                let e = &c.net.edges[&n.edge];
                let s = e.cum[n.seg] + e.pts[n.seg].distance(n.point);
                return super::dibujo::altura_en(pf, s);
            }
        }
        super::fauna::suelo(t, p)
    }

    /// La cadena de baldes de cada incendio, del pozo o aljibe a la casa: gente en fila que se
    /// pasa los baldes (uno de cada tres tiene uno en la mano, y el balde avanza).
    fn cajas_incendios(&self, c: &crate::city::City, t: &TerrainGen, centro: Vec2, v: &super::dibujo::Vestuario, out: &mut Vec<Caja>) {
        let d = &datos().oficios;
        let b = biblioteca();
        let segs = v.t;
        for inc in &self.e.incendios {
            let (Some(fuente), pos) = (inc.fuente, Vec2::from(inc.pos)) else { continue };
            let fuente = Vec2::from(fuente);
            if pos.distance(centro) > RADIO_ESCENAS + 60.0 {
                continue;
            }
            let largo = fuente.distance(pos);
            let n = ((largo / d.cadena_separacion_m) as usize).clamp(2, d.cadena_max);
            let dir = (pos - fuente).normalize_or(Vec2::X);
            let normal = Vec2::new(-dir.y, dir.x);
            let paso_balde = (segs * 1.6) as i64;
            for i in 0..n {
                let u = (i as f32 + 0.5) / n as f32;
                let zig = if i % 2 == 0 { 0.35 } else { -0.35 };
                let p = fuente.lerp(pos, u) + normal * zig;
                if p.distance(centro) > RADIO_ESCENAS {
                    continue;
                }
                let y = self.altura_cerca(c, t, p);
                let s = mezcla(inc.lote as u64 * 1000 + i as u64);
                let q = crate::detalle::ropa::Quien { mujer: s % 3 == 0, edad: 18 + (s % 40) as u8, oficio: crate::city::poblacion::modelo::Oficio::Peon, principal: false, raiz: [crate::city::poblacion::modelo::Raiz::Europea, crate::city::poblacion::modelo::Raiz::Mixta, crate::city::poblacion::modelo::Raiz::Indigena][(s % 3) as usize], semilla: s };
                let (at, pal) = vestir(&q, v.anio);
                // Giran de un vecino al otro para pasar el balde.
                let vaiven = (segs * 2.2 + i as f32 * 0.8).sin();
                let cara = -normal * zig.signum();
                let giro = cara.x.atan2(cara.y) + vaiven * 0.7;
                let con_balde = (i as i64 + paso_balde).rem_euclid(3) == 0;
                let mut pose = Pose::default();
                pose.set(Parte::BrazoI, 0.0, -0.7 - 0.2 * vaiven);
                pose.set(Parte::BrazoD, 0.0, if con_balde { -0.55 } else { -0.8 - 0.2 * vaiven });
                pose.set(Parte::Cabeza, vaiven * 0.3, 0.0);
                let base = Vec3::new(p.x, y, p.y);
                emitir(out, b.persona(at, false), &pal, base, giro, &pose, s as u32);
                if con_balde {
                    emitir(out, b.objeto(Id::BaldeEnMano), &modelos::paleta(Id::BaldeEnMano, s as u32), base, giro, &pose, s as u32);
                }
                sombra(out, base, giro, [0.27, 0.2], 130);
            }
        }
    }
}

// --- Rampas de los escalones de las calles (ver `voxelize::escalones`) ---------------------

/// Las rampas de los escalones de cada tramo, armadas la primera vez que el tramo queda cerca
/// de la vista (no se guardan).
#[derive(Debug, Default)]
pub struct Rampas {
    por_tramo: Mutex<HashMap<u32, (u64, Arc<Vec<Caja>>)>>,
}

impl Clone for Rampas {
    fn clone(&self) -> Self {
        Rampas::default()
    }
}

/// Sin rampas (para las capturas de antes y después del recorrido).
pub static SIN_RAMPAS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Tramos nuevos que se arman por cuadro (para no trabar un cuadro al llegar a un barrio).
const RAMPAS_POR_CUADRO: usize = 3;

impl Agentes {
    /// Las rampas de los escalones de las calles a menos de 140 m de `centro`.
    pub fn cajas_rampas(&self, c: &crate::city::City, t: &TerrainGen, centro: Vec2, out: &mut Vec<Caja>) {
        if SIN_RAMPAS.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        self.armar_rampas(c, t, centro, RAMPAS_POR_CUADRO, Some(out));
    }

    /// Arma las rampas de los tramos cerca de `centro` que faltan (todas, sin el tope por
    /// cuadro): para las capturas.
    pub fn precalentar_rampas(&self, c: &crate::city::City, t: &TerrainGen, centro: Vec2) {
        self.armar_rampas(c, t, centro, usize::MAX, None);
    }

    /// Las rampas de los tramos cerca de `centro`, del más cercano al más lejano; arma hasta
    /// `tope` tramos que falten.
    fn armar_rampas(&self, c: &crate::city::City, t: &TerrainGen, centro: Vec2, tope: usize, mut out: Option<&mut Vec<Caja>>) {
        let mut g = self.rampas.por_tramo.lock().unwrap_or_else(|e| e.into_inner());
        g.retain(|id, _| c.net.edges.contains_key(id));
        let plaza = c.plaza_square();
        let mut cerca: Vec<(f32, u32)> = c
            .net
            .edges
            .iter()
            .map(|(&id, e)| (e.pts.windows(2).map(|w| crate::city::roads::closest_on_segment(centro, w[0], w[1]).distance(centro)).fold(f32::INFINITY, f32::min), id))
            .filter(|x| x.0 < RADIO_ESCENAS)
            .collect();
        cerca.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut nuevos = 0;
        for (_, id) in cerca {
            let e = &c.net.edges[&id];
            let firma = (e.pts.len() as u64) << 40 ^ e.len().to_bits() as u64 ^ (e.kind as u64) << 32 ^ (plaza.map_or(0, |p| p.0.x.to_bits() as u64) << 20);
            let vigente = g.get(&id).is_some_and(|x| x.0 == firma);
            if !vigente {
                if nuevos >= tope {
                    continue;
                }
                nuevos += 1;
                g.insert(id, (firma, Arc::new(crate::city::voxelize::escalones(t, &c.net, id, plaza))));
            }
            if let Some(out) = out.as_deref_mut() {
                out.extend(g[&id].1.iter().filter(|k| Vec2::new(k.pos[0], k.pos[2]).distance(centro) < RADIO_ESCENAS));
            }
        }
    }
}
