//! El desarrollo enganchado a la ciudad. Tres momentos de cada mes:
//! 1. [`City::desarrollo_month`] (después de los servicios, antes de la economía): arma el
//!    [`Contexto`], cierra el mes del desarrollo, compra el ganado de los saladeros y levanta el
//!    edificio de un inversor privado si alguno se decidió;
//! 2. [`City::desarrollo_input`] (dentro de `month_input`): producción, presupuesto y precio
//!    de los cueros para la economía;
//! 3. [`City::desarrollo_tras_poblacion`] (después del mes de la población): ganancias a los
//!    hogares dueños, salarios a los peones, escuelas y aprendizaje (personas que pasan a ser
//!    oficiales o pilotos), contratación y los saberes de la población.

use super::*;
use crate::buildings::Edits;
use crate::city::economy::{MonthInput, Ordinance, Tax};
use crate::city::lots::{ZoneKind, JOBS_I};
use crate::city::poblacion::economia::salario;
use crate::city::poblacion::{Condicion, Oficio, Sexo, TipoEvento, NADIE};
use crate::city::services::{self, barrio::Barrio, near_coast, port};
use crate::city::City;
use crate::terrain::TerrainGen;
use glam::Vec2;

/// ¿Es un edificio del desarrollo (institución o industria)?
pub fn es_de_desarrollo(k: Kind) -> bool {
    matches!(k, Kind::EscuelaOficios | Kind::EscuelaNautica | Kind::Saladero | Kind::Molino | Kind::Astillero | Kind::Imprenta)
}

/// Rinde de una industria en un sitio (1,0 = normal), según el recurso del suelo que declara
/// su perfil (pasturas, tierra de cultivo, arcilla, piedra; ver `recursos`). Amortiguado: el
/// sitio pesa, pero no decide solo (entre 0,84 y 1,34 para los recursos más comunes).
pub fn rinde_del_sitio(ind: Ind, c: Vec2, t: &TerrainGen) -> f32 {
    let r = super::super::recursos::Recursos::de(t).en_punto(c);
    let base = super::super::recursos::rinde_por_clave(ind_info(ind).recurso, &r);
    0.75 + 0.25 * base
}

/// Distancias a la plaza (mínima y máxima, m) donde un inversor busca sitio para cada edificio.
fn distancias(k: Kind) -> (f32, f32) {
    match k {
        Kind::Saladero => (620.0, 3500.0),
        Kind::Astillero => (200.0, 3500.0),
        Kind::Molino => (260.0, 2500.0),
        Kind::Imprenta => (0.0, 480.0),
        _ => (0.0, 2000.0),
    }
}

/// Plazas de alumnos y meses de curso de las escuelas del desarrollo.
const CURSO_MESES: f32 = 24.0;
/// Aprendices por mes por cada empleo de los talleres (oficiales que salen del taller).
const APRENDIZAJE: f32 = 0.0005;
/// Oficiales que el aprendizaje en los talleres sostiene sin escuela de oficios: alcanza para
/// mejorar un oficio, no para industrializar (eso pide formar gente en la escuela).
pub(crate) const TOPE_APRENDIZAJE: usize = 3;

impl City {
    /// Cuántos edificios de cada tipo hay (para los requisitos).
    fn edificios_por_tipo(&self) -> Vec<(Kind, u32)> {
        let mut v: Vec<(Kind, u32)> = Vec::new();
        for b in self.services.buildings.values() {
            match v.iter_mut().find(|x| x.0 == b.kind) {
                Some(x) => x.1 += 1,
                None => v.push((b.kind, 1)),
            }
        }
        v
    }

    /// Personas que podrían tomar un puesto de peón (libres, en edad, sin trabajo).
    fn candidatos_peon(&self) -> Vec<u32> {
        let p = &self.poblacion;
        if !p.fundada {
            return Vec::new();
        }
        let ocupados: std::collections::HashSet<u32> = self.desarrollo.establecimientos.iter().flat_map(|e| e.empleados.iter().copied()).collect();
        p.idx
            .vivos
            .iter()
            .copied()
            .filter(|&id| {
                let q = &p.personas[id as usize];
                (15..=60).contains(&q.edad(p.mes)) && q.condicion != Condicion::Esclavizada && q.trabajo.is_none() && matches!(q.oficio, Oficio::Peon | Oficio::Ninguno) && q.hogar.is_some() && !ocupados.contains(&id)
            })
            .collect()
    }

    /// Lo que la ciudad le cuenta al desarrollo (también para la interfaz).
    pub fn desarrollo_contexto(&self) -> Contexto {
        use crate::city::economy::Res;
        let eco = &self.economy;
        let s = eco.stock(Res::Carne);
        let contraband = if self.services.count(Kind::Aduana) > 0 { port::CONTRABAND_CUSTOMS } else { port::CONTRABAND_NO_CUSTOMS };
        let (_, jobs_i) = self.lots.jobs();
        let brick = self.climate.brick_factor();
        let mut inversores: Vec<(u32, f32)> = self
            .poblacion
            .hogares
            .iter()
            .filter(|h| h.activo && !h.miembros.is_empty() && h.clase >= crate::city::poblacion::Clase::Acomodada)
            .map(|h| (h.id, h.ahorro))
            .collect();
        inversores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        inversores.truncate(5);
        Contexto {
            mes: self.month as i32,
            epoca: self.poblacion.epoca,
            saberes: self.desarrollo.saberes,
            edificios: self.edificios_por_tipo(),
            muelle: self.services.port.pier.is_some(),
            bloqueo: self.services.port.mult(|e| e.ships) <= 0.0,
            tasa_aduana: eco.tax(Tax::Aduana) as f32 / 100.0 * eco.duty_factor() * (1.0 - contraband),
            // Lo que sobra de la producción de cada mes (más un poco de lo guardado): lo que se
            // puede comprar sin vaciar las existencias.
            carne_disponible: (s.amount * 0.1 + (s.produced - s.consumed).max(0.0)).max(0.0),
            precios: Res::ALL.map(|r| eco.price(r)),
            world_hides: self.services.port.mult(|e| e.hides),
            dinero: self.money,
            ladrillos: eco.stock(Res::Ladrillos).amount,
            jobs_i,
            ladrillos_base: (services::HORNO_LADRILLOS * self.services.count(Kind::Horno) as f32 + jobs_i as f32 * 1.5) * brick,
            habitantes: self.habitantes(),
            delito: Barrio::media(&self.services.barrio.delito),
            opinion: eco.opinion,
            inversores,
            desocupados: self.candidatos_peon().len() as u32,
            exportado: Res::ALL.map(|r| eco.stock(r).exported),
            importado: Res::ALL.map(|r| eco.stock(r).imported),
        }
    }

    /// Empieza a invertir en una tecnología (desde el panel de Desarrollo).
    pub fn desarrollo_iniciar(&mut self, t: Tec) -> Result<String, String> {
        let ctx = self.desarrollo_contexto();
        self.desarrollo.iniciar(t, &ctx)
    }

    /// Prende o apaga un incentivo (cuesta el papeleo de una ordenanza).
    pub fn desarrollo_alternar(&mut self, i: Incentivo) -> Result<String, String> {
        if self.money < crate::city::economy::TOGGLE_COST {
            return Err(format!("Cambiar un incentivo cuesta {} pesos de papeleo", crate::city::economy::TOGGLE_COST));
        }
        let m = self.desarrollo.alternar(i, self.economy.is_on(Ordinance::LibreComercio))?;
        self.money -= crate::city::economy::TOGGLE_COST;
        Ok(m)
    }

    /// Requisitos de sitio de los edificios del desarrollo (`None` si se puede).
    pub fn desarrollo_problema_sitio(&self, t: &TerrainGen, k: Kind, c: Vec2) -> Option<String> {
        if !es_de_desarrollo(k) {
            return None;
        }
        if let Some(tec) = Desarrollo::tec_del_edificio(k) {
            if !self.desarrollo.hecha(tec) {
                return Some(format!("Falta la tecnología «{}» (panel Desarrollo)", tec_info(tec).nombre));
            }
        }
        let d = self.lots.plaza_center.map_or(f32::INFINITY, |p| p.distance(c));
        match k {
            Kind::Saladero if d < 600.0 => Some("El saladero huele: a más de 600 m de la plaza".into()),
            Kind::Saladero if t.arroyo_distance(c.x, c.y) > 300.0 && !near_coast(t, c, 300.0) => Some("El saladero necesita agua: un arroyo o la costa a menos de 300 m".into()),
            Kind::Molino if d < 250.0 => Some("El molino va en las afueras, a más de 250 m de la plaza".into()),
            Kind::Astillero if d < 200.0 => Some("La carpintería de ribera va fuera del centro (a más de 200 m de la plaza)".into()),
            Kind::Astillero if !near_coast(t, c, 60.0) => Some("La carpintería de ribera va sobre la costa (a menos de 60 m del agua)".into()),
            Kind::Imprenta if d > 500.0 => Some("La imprenta va cerca de la plaza (a menos de 500 m)".into()),
            Kind::Imprenta | Kind::EscuelaNautica if self.services.count(k) > 0 => Some("La ciudad ya tiene una".into()),
            _ => None,
        }
    }

    /// Un sitio para el edificio de un inversor: junto a una calle, a la distancia que pide
    /// cada industria. Recorre los nudos de la red de calles en orden.
    fn sitio_para_inversor(&mut self, t: &TerrainGen, k: Kind) -> Option<(Vec2, Vec2)> {
        let plaza = self.lots.plaza_center?;
        let (dmin, dmax) = distancias(k);
        let mut nudos: Vec<Vec2> = self.net.nodes.values().copied().filter(|p| (dmin..dmax).contains(&p.distance(plaza))).collect();
        // Primero los más cercanos a la distancia mínima (el inversor no quiere ir lejos).
        nudos.sort_by(|a, b| a.distance(plaza).total_cmp(&b.distance(plaza)).then(a.x.total_cmp(&b.x)));
        nudos.truncate(250);
        let guardado = self.money;
        self.money = i64::MAX / 4;
        let mut sitio = None;
        'b: for n in nudos {
            for a in 0..8 {
                let p = n + Vec2::from_angle(a as f32 / 8.0 * std::f32::consts::TAU) * 28.0;
                let (c, f) = self.service_site(k, p);
                if self.service_problem(t, k, c, f).is_none() {
                    sitio = Some((c, f));
                    break 'b;
                }
            }
        }
        self.money = guardado;
        sitio
    }

    /// Los edificios de industria que el jugador levantó con la herramienta son del cabildo;
    /// los demolidos se van de la lista.
    fn sincronizar_establecimientos(&mut self, t: &TerrainGen) {
        let edificios = &self.services.buildings;
        self.desarrollo.establecimientos.retain(|e| edificios.get(&e.edificio).is_some_and(|b| ind_de_edificio(b.kind) == Some(e.ind)));
        let nuevos: Vec<(u32, Ind)> = edificios.values().filter_map(|b| ind_de_edificio(b.kind).map(|i| (b.id, i))).filter(|(id, _)| !self.desarrollo.establecimientos.iter().any(|e| e.edificio == *id)).collect();
        for (id, ind) in nuevos {
            self.desarrollo.registrar(ind, id, Dueno::Cabildo, self.month as i32, false);
        }
        // El rinde de cada sitio (recursos del suelo).
        for e in &mut self.desarrollo.establecimientos {
            e.sitio = rinde_del_sitio(e.ind, self.services.buildings[&e.edificio].c, t);
        }
    }

    /// El mes del desarrollo (antes de la economía). Devuelve las noticias.
    pub(crate) fn desarrollo_month(&mut self, t: &TerrainGen, e: &mut Edits) -> Vec<String> {
        use crate::city::economy::Res;
        self.sincronizar_establecimientos(t);
        let ctx = self.desarrollo_contexto();
        let mut r = self.desarrollo.mes(&ctx);
        // El ganado que compran los saladeros sale de las existencias de carne.
        let s = &mut self.economy.stocks[Res::Carne as usize];
        let de_stock = r.carne_tomada.min(s.amount);
        s.amount -= de_stock;
        r.produccion[Res::Carne as usize] -= r.carne_tomada - de_stock;
        self.economy.take(Res::Ladrillos, r.ladrillos_tomados.min(self.economy.stock(Res::Ladrillos).amount));
        let mut news = std::mem::take(&mut r.noticias);
        // Un inversor se decidió: busca sitio y levanta el edificio con su plata.
        if let Some((ind, dueno)) = r.inversion {
            let k = ind_info(ind).edificio.unwrap();
            if let Some((c, f)) = self.sitio_para_inversor(t, k) {
                let cost = services::info(k).cost;
                self.money += cost;
                match self.place_service(t, k, c, f) {
                    Ok(ed) => {
                        e.extend(ed);
                        let id = self.services.next;
                        let credito = dueno != Dueno::Forastero && self.desarrollo.incentivo(Incentivo::Credito);
                        let propio = self.desarrollo.capital_propio(ind, dueno);
                        if let Dueno::Hogar(h) = dueno {
                            if let Some(hg) = self.poblacion.hogares.get_mut(h as usize) {
                                hg.ahorro = (hg.ahorro - propio).max(0.0);
                            }
                        }
                        r.lineas.extend(self.desarrollo.registrar(ind, id, dueno, self.month as i32, credito));
                        self.desarrollo.anotar_inversion(ind_info(ind).capital as f32);
                        let quien = match dueno {
                            Dueno::Hogar(h) => format!("La familia {}", self.poblacion.apellido_hogar(h)),
                            _ => "Un comerciante de Buenos Aires".to_string(),
                        };
                        news.push(format!("Gaceta: {quien} invierte {} pesos y abre un {}{}", ind_info(ind).capital, ind_info(ind).nombre.to_lowercase(), if credito { " con el crédito de fomento" } else { "" }));
                    }
                    Err(_) => self.money -= cost,
                }
            }
        }
        self.desarrollo.pendiente = r;
        news
    }

    /// Lo que el desarrollo le pasa a la economía este mes.
    pub(crate) fn desarrollo_input(&self, inp: &mut MonthInput) {
        let r = &self.desarrollo.pendiente;
        for k in 0..6 {
            inp.production[k] += r.produccion[k];
        }
        inp.other.extend(r.lineas.iter().cloned());
        inp.world_hides *= r.factor_cueros;
        inp.jobs_i += self.desarrollo.establecimientos.iter().map(|e| e.empleados.len() as i32).sum::<i32>();
    }

    /// Empujón del mes a la opinión de los sectores (después de cerrar la economía).
    pub(crate) fn desarrollo_opinion(&mut self) {
        let o = self.desarrollo.pendiente.opinion;
        for k in 0..3 {
            self.economy.opinion[k] = (self.economy.opinion[k] + o[k]).clamp(0.0, 100.0);
        }
    }

    /// Después del mes de la población: plata a los hogares, escuelas, contratación y saberes.
    pub(crate) fn desarrollo_tras_poblacion(&mut self) {
        if !self.poblacion.fundada {
            self.desarrollo.pendiente = Resultado::default();
            return;
        }
        let mes = self.poblacion.mes;
        let r = std::mem::take(&mut self.desarrollo.pendiente);
        let pob = &mut self.poblacion;
        let pagar = |pob: &mut crate::city::poblacion::Poblacion, h: u32, monto: f32, renta: bool| {
            let Some(hg) = pob.hogares.get_mut(h as usize) else { return };
            if !hg.activo {
                return;
            }
            hg.ingresos += monto;
            hg.detalle[if renta { 1 } else { 0 }] += monto;
            hg.ahorro += monto;
            if hg.ahorro < 0.0 {
                hg.deuda -= hg.ahorro;
                hg.ahorro = 0.0;
            }
        };
        // 1. Ganancias de los dueños y renta de la suela para los dueños de los talleres.
        for &(h, g) in &r.ganancias {
            pagar(pob, h, g, true);
        }
        if r.renta_talleres > 0.0 {
            let talleres: Vec<(u32, i32)> = self.lots.lots.values().filter(|l| l.kind() == Some(ZoneKind::Industrial) && l.level > 0).filter_map(|l| pob.duenos.get(&l.id).map(|&d| (d, JOBS_I[l.level as usize]))).collect();
            let total: i32 = talleres.iter().map(|x| x.1).sum();
            for (d, j) in talleres {
                pagar(pob, d, r.renta_talleres * j as f32 / total.max(1) as f32, true);
            }
        }
        // 2. Salarios: el establecimiento completa el jornal de sus peones.
        for est in &self.desarrollo.establecimientos {
            let w = est.info().salario;
            for &id in &est.empleados {
                let Some(p) = pob.personas.get(id as usize) else { continue };
                if !p.viva() {
                    continue;
                }
                let mujer = if p.sexo == Sexo::Mujer { 0.6 } else { 1.0 };
                let extra = ((w - 0.75 * salario(p.oficio)) * mujer).max(0.0);
                if let Some(h) = p.hogar {
                    pagar(pob, h, extra, false);
                }
            }
        }
        // 3. Escuelas y aprendizaje.
        let d = &mut self.desarrollo;
        d.formados.retain(|(id, _)| pob.personas.get(*id as usize).is_some_and(|p| p.viva()));
        let n_ofi = self.services.count(Kind::EscuelaOficios) as f32;
        let n_nau = self.services.count(Kind::EscuelaNautica) as f32;
        let (_, jobs_i) = self.lots.jobs();
        let plazas_ofi = services::info(Kind::EscuelaOficios).plazas as f32;
        let plazas_nau = services::info(Kind::EscuelaNautica).plazas as f32;
        d.acumulado[Saber::Oficiales.i()] += n_ofi * plazas_ofi / CURSO_MESES;
        d.acumulado[Saber::Artesanos.i()] += jobs_i as f32 * APRENDIZAJE;
        d.acumulado[Saber::Pilotos.i()] += n_nau * plazas_nau / CURSO_MESES;
        let ya: std::collections::HashSet<u32> = d.formados.iter().map(|x| x.0).collect();
        let vivos = pob.idx.vivos.clone();
        let inicio = if vivos.is_empty() { 0 } else { (mes.unsigned_abs() as usize * 7919) % vivos.len() };
        let mut sin_alumnos = 0.0;
        // (acumulador, saber que da, lectura mínima, solo varones, oficio que toma)
        for (acc, saber, lectura, varones, oficio) in [(Saber::Oficiales, Saber::Oficiales, 1u8, false, Oficio::Artesano), (Saber::Artesanos, Saber::Oficiales, 0, false, Oficio::Artesano), (Saber::Pilotos, Saber::Pilotos, 2, true, Oficio::Marinero)] {
            while d.acumulado[acc.i()] >= 1.0 {
                // Sin escuela de oficios, el taller no forma más que unos pocos oficiales.
                if acc == Saber::Artesanos && n_ofi == 0.0 && d.formados.iter().filter(|x| x.1 == Saber::Oficiales).count() >= TOPE_APRENDIZAJE {
                    d.acumulado[acc.i()] = 0.0;
                    break;
                }
                d.acumulado[acc.i()] -= 1.0;
                let elegido = (0..vivos.len()).map(|k| vivos[(inicio + k) % vivos.len()]).find(|&id| {
                    let p = &pob.personas[id as usize];
                    let edad = p.edad(mes);
                    let oficio_ok = if acc == Saber::Artesanos { matches!(p.oficio, Oficio::Artesano | Oficio::Peon) && p.trabajo.is_some() } else { matches!(p.oficio, Oficio::Ninguno | Oficio::Peon | Oficio::Servicio | Oficio::Artesano | Oficio::Labrador | Oficio::Marinero) };
                    (14..=30).contains(&edad) && p.educacion >= lectura && p.condicion != Condicion::Esclavizada && (!varones || p.sexo == Sexo::Hombre) && oficio_ok && !ya.contains(&id) && !d.formados.iter().any(|x| x.0 == id)
                });
                match elegido {
                    Some(id) => {
                        let p = &mut pob.personas[id as usize];
                        if p.oficio != oficio {
                            p.oficio = oficio;
                            p.evento(mes, TipoEvento::Oficio(oficio), NADIE);
                        }
                        p.evento(mes, TipoEvento::Aprendio, NADIE);
                        d.formados.push((id, saber));
                    }
                    None => {
                        // Plaza vacía: no hay alumnos que cumplan (lectura, edad, oficio).
                        sin_alumnos += 1.0;
                        d.acumulado[acc.i()] = 0.0;
                        break;
                    }
                }
            }
        }
        d.sin_alumnos = sin_alumnos;
        // 4. Contratación: se van los que murieron, consiguieron otro trabajo o envejecieron.
        for est in &mut d.establecimientos {
            est.empleados.retain(|&id| pob.personas.get(id as usize).is_some_and(|p| p.viva() && p.trabajo.is_none() && p.edad(mes) <= 65));
        }
        let mut cands = {
            let ocupados: std::collections::HashSet<u32> = d.establecimientos.iter().flat_map(|e| e.empleados.iter().copied()).collect();
            vivos
                .iter()
                .copied()
                .filter(|&id| {
                    let q = &pob.personas[id as usize];
                    (15..=60).contains(&q.edad(mes)) && q.condicion != Condicion::Esclavizada && q.trabajo.is_none() && matches!(q.oficio, Oficio::Peon | Oficio::Ninguno) && q.hogar.is_some() && !ocupados.contains(&id)
                })
                .collect::<Vec<u32>>()
        };
        cands.reverse();
        for est in &mut d.establecimientos {
            let n = est.info().empleos as usize;
            while est.empleados.len() < n {
                let Some(id) = cands.pop() else { break };
                let p = &mut pob.personas[id as usize];
                if p.oficio == Oficio::Ninguno {
                    p.oficio = Oficio::Peon;
                    p.evento(mes, TipoEvento::Oficio(Oficio::Peon), NADIE);
                }
                est.empleados.push(id);
            }
        }
        // 5. Saberes de la población.
        let mut s = [0u32; 6];
        for &id in &pob.idx.vivos {
            let p = &pob.personas[id as usize];
            if p.edad(mes) >= 15 {
                if p.educacion >= 1 {
                    s[Saber::Alfabetizados.i()] += 1;
                }
                if p.oficio == Oficio::Artesano {
                    s[Saber::Artesanos.i()] += 1;
                }
            }
        }
        for &(_, sb) in &d.formados {
            s[sb.i()] += 1;
        }
        d.saberes = s;
        if let Some(f) = d.historia.back_mut() {
            f.saberes = s;
        }
        d.pendiente.opinion = r.opinion;
        // Las estadísticas de la población, al día con la plata y los oficios nuevos.
        pob.stats = pob.estadisticas();
    }

    /// Ficha de un edificio del desarrollo.
    pub(crate) fn desarrollo_lines(&self, id: u32, out: &mut Vec<String>) {
        let b = &self.services.buildings[&id];
        let d = &self.desarrollo;
        match b.kind {
            Kind::EscuelaOficios | Kind::EscuelaNautica => {
                let inf = services::info(b.kind);
                let saber = if b.kind == Kind::EscuelaOficios { Saber::Oficiales } else { Saber::Pilotos };
                out.push(format!("Alumnos: {} · egresan unos {} por año", inf.plazas, (inf.plazas as f32 / CURSO_MESES * 12.0).round() as i32));
                out.push(format!("{} en la ciudad: {}", saber.nombre(), d.saber(saber)));
                out.push(if b.kind == Kind::EscuelaOficios { "Piden saber leer (escuelas de primeras letras)".into() } else { "Piden leer y escribir; muchachos de 14 a 30 años".into() });
                if d.sin_alumnos > 0.0 {
                    out.push("El último mes quedaron plazas vacías: faltan jóvenes que lean".into());
                }
            }
            _ => {
                let Some(e) = d.establecimientos.iter().find(|e| e.edificio == id) else {
                    out.push("Abre el mes que viene".into());
                    return;
                };
                let inf = e.info();
                out.push(match e.dueno {
                    Dueno::Cabildo => "Dueño: el cabildo (la ganancia va al presupuesto)".into(),
                    Dueno::Hogar(h) => format!("Dueña: la familia {}", self.poblacion.apellido_hogar(h)),
                    Dueno::Forastero => "Dueño: un comerciante de afuera (la ganancia se va)".into(),
                });
                out.push(format!("Peones: {} de {}{}", e.empleados.len(), inf.empleos, inf.saber.map_or(String::new(), |(s, n)| format!(" · {}: {}", s.nombre().to_lowercase(), n))));
                out.push(format!("Marcha: {} % · ventas {} · ganancia {} pesos al mes", (e.marcha * 100.0).round() as i32, e.ventas as i64, e.ganancia as i64));
                if !e.falta.is_empty() {
                    out.push(format!("No rinde a pleno: {}", e.falta));
                }
                if e.deuda > 1.0 {
                    out.push(format!("Debe al crédito de fomento: {} pesos", e.deuda as i64));
                }
                if d.incentivo(Incentivo::Exencion) && (self.month as i32 - e.desde) < MESES_EXENCION {
                    out.push(format!("Exenta de derechos por {} meses más", MESES_EXENCION - (self.month as i32 - e.desde)));
                }
            }
        }
    }
}
