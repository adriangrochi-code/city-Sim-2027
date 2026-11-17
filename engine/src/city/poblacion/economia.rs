//! Economía de cada hogar: vivienda (dónde vive, si es dueño o inquilino), trabajo (solares
//! comerciales e industriales, el campo, el puerto, el servicio), ingresos y gastos (comida al
//! precio del mercado, alquiler, impuestos), ahorro, deuda, compra de solares y mejoras de la
//! casa y clase social.
//!
//! Montos en pesos de plata por mes. Son ESTIMACIONES de orden de magnitud para el Río de la
//! Plata colonial (ver la tabla de calibración en `docs/sistemas/poblacion.md`).

use super::*;
use crate::city::economy::Ordinance;

/// Costo de vida básico de un adulto por mes a precios normales (comida, leña, vela, ropa).
pub const SUBSISTENCIA: f32 = 2.0;
/// Valor de una cabeza de ganado (para el patrimonio).
pub const PRECIO_GANADO: f32 = 2.0;

/// Valor de un solar con su edificio, por nivel (vacío, rancho, azotea, altos).
pub const VALOR_SOLAR: [f32; 4] = [20.0, 40.0, 450.0, 1500.0];
/// Alquiler mensual de una casa por nivel.
pub const ALQUILER: [f32; 4] = [0.0, 1.0, 4.0, 10.0];
/// Lo que cuesta mejorar la casa al nivel dado (ladrillos, cal, mano de obra).
pub const COSTO_MEJORA: [f32; 4] = [0.0, 30.0, 400.0, 1100.0];

/// Salario o ganancia mensual de un oficio (hombre adulto; las mujeres cobraban bastante
/// menos: ×0,6, estimación).
pub fn salario(o: Oficio) -> f32 {
    match o {
        Oficio::Ninguno => 0.0,
        Oficio::Peon => 7.0,
        Oficio::Labrador => 9.0,
        Oficio::Hacendado => 15.0,
        Oficio::Artesano => 12.0,
        Oficio::Pulpero => 14.0,
        Oficio::Comerciante => 32.0,
        Oficio::Marinero => 9.0,
        Oficio::Soldado => 8.0,
        Oficio::Servicio => 4.0,
        Oficio::Clerigo => 15.0,
        Oficio::Funcionario => 25.0,
        Oficio::Maestro => 12.0,
        Oficio::Obrero => 10.0,
        Oficio::Empleado => 18.0,
        Oficio::Profesional => 40.0,
    }
}

/// Tipo de solar donde se trabaja en cada oficio (si no hay vacante: por cuenta propia).
pub fn zona_de_trabajo(o: Oficio) -> Option<ZoneKind> {
    match o {
        Oficio::Comerciante | Oficio::Pulpero | Oficio::Empleado => Some(ZoneKind::Comercial),
        Oficio::Artesano | Oficio::Obrero | Oficio::Peon => Some(ZoneKind::Industrial),
        _ => None,
    }
}

impl Poblacion {
    /// Patrimonio de un hogar: ahorro menos deuda, más solares y ganado.
    pub fn patrimonio(&self, h: &Hogar) -> f32 {
        let solares: f32 = h.solares.iter().map(|l| VALOR_SOLAR[(*self.niveles.get(l).unwrap_or(&0) as usize).min(3)]).sum();
        h.ahorro - h.deuda + solares + h.ganado as f32 * PRECIO_GANADO
    }

    /// Riqueza total de la ciudad: patrimonio de los hogares, riqueza propia de las personas y
    /// lo que salió en legados. Se conserva en las herencias, dotes y mudanzas.
    pub fn riqueza_total(&self) -> f64 {
        let hogares: f64 = self.hogares.iter().filter(|h| h.activo).map(|h| self.patrimonio(h) as f64).sum();
        let personas: f64 = self.personas.iter().map(|p| p.riqueza as f64).sum();
        hogares + personas + self.legados
    }

    pub(crate) fn colectivo(h: &Hogar) -> bool {
        matches!(h.alojamiento, Alojamiento::Cuartel | Alojamiento::Campamento)
    }

    /// Adultos equivalentes de un hogar (los niños comen menos).
    pub(crate) fn equivalentes(&self, h: &Hogar) -> f32 {
        h.miembros.iter().map(|&m| if self.personas[m as usize].edad(self.mes) >= 14 { 1.0 } else { 0.6 }).sum::<f32>().max(0.6)
    }

    // --- Vivienda -----------------------------------------------------------------------

    pub(crate) fn vivienda(&mut self, rng: &mut Rng, ent: &Entorno, inf: &mut InformeMes) {
        self.conocer_lotes(&ent.lotes);
        let info = Lotes::de(ent);
        let es_casa = |l: Option<LoteId>| l.and_then(|l| info.get(&l)).is_some_and(|i| i.tipo == Some(ZoneKind::Residencial) && i.capacidad > 0);
        // Desalojos: la casa se demolió, se inundó o se rezonificó.
        for h in &mut self.hogares {
            if h.activo && h.alojamiento == Alojamiento::Solar && !es_casa(h.lote) {
                h.lote = None;
                h.alojamiento = Alojamiento::SinCasa;
            }
        }
        // Propiedades que ya no existen.
        let perdidas: Vec<LoteId> = self.duenos.keys().copied().filter(|l| info.get(l).is_none_or(|i| i.nivel == 0)).collect();
        for l in perdidas {
            if let Some(h) = self.duenos.remove(&l) {
                self.hogares[h as usize].solares.retain(|&x| x != l);
            }
        }
        self.reindexar();
        // Ocupación de cada casa.
        let mut ocupado: Densa<i32> = Densa::default();
        for (l, hs) in &self.idx.por_lote {
            ocupado.insert(*l, hs.iter().map(|&h| self.hogares[h as usize].miembros.len() as i32).sum());
        }
        let mut casas: Vec<&LoteInfo> = ent.lotes.iter().filter(|l| l.tipo == Some(ZoneKind::Residencial) && l.capacidad > 0).collect();
        casas.sort_by(|a, b| a.valor.total_cmp(&b.valor).then(a.id.cmp(&b.id)));
        // Solo las que tienen lugar (aunque sea hacinado): las llenas no se miran.
        let todas = casas.clone();
        casas.retain(|c| *ocupado.get(&c.id).unwrap_or(&0) < (c.capacidad as f32 * 1.6) as i32);
        let mut vacias: std::collections::VecDeque<LoteId> = casas.iter().filter(|c| *ocupado.get(&c.id).unwrap_or(&0) == 0).map(|c| c.id).collect();
        let reparto = ent.vigente(Ordinance::RepartoSolares);
        let primeros_años = self.mes < 12 * 6;
        let mut ventas = 0.0f32;
        let mut sin_casa: Vec<HogarId> = self.hogares.iter().filter(|h| h.activo && h.alojamiento == Alojamiento::SinCasa && !h.miembros.is_empty()).map(|h| h.id).collect();
        let n_sin_casa = sin_casa.len();
        // Los inquilinos de casas hacinadas se mudan si encuentran lugar de verdad.
        // (Unas pocas por mes: mudarse lleva tiempo.)
        for (l, hs) in &self.idx.por_lote {
            let cap = info.get(l).map_or(0, |i| i.capacidad);
            if *ocupado.get(l).unwrap_or(&0) > cap {
                if let Some(&h) = hs.iter().rev().find(|&&h| self.duenos.get(l) != Some(&h)) {
                    sin_casa.push(h);
                    if sin_casa.len() >= n_sin_casa + 150 {
                        break;
                    }
                }
            }
        }
        for (k, h) in sin_casa.into_iter().enumerate() {
            let hacinado = k >= n_sin_casa;
            let n = self.hogares[h as usize].miembros.len() as i32;
            let rico = self.hogares[h as usize].ahorro > 300.0;
            // Los ricos buscan el mejor suelo; los pobres, el más barato. Primero una casa con
            // lugar (se miran pocas), si no una vacía.
            let actual = self.hogares[h as usize].lote;
            let cabe = |c: &&&LoteInfo| {
                let occ = *ocupado.get(&c.id).unwrap_or(&0);
                occ > 0 && occ + n <= c.capacidad && Some(c.id) != actual
            };
            let mut elegido = if rico { casas.iter().rev().take(48).find(cabe) } else { casas.iter().take(48).find(cabe) }.map(|c| c.id);
            if elegido.is_none() {
                elegido = if rico { vacias.pop_back() } else { vacias.pop_front() };
            }
            // Sin lugar: algunos pobres se agregan a una casa ajena (hacinamiento).
            if elegido.is_none() && !hacinado && rng.f() < 0.4 {
                elegido = casas.iter().take(200).find(|c| *ocupado.get(&c.id).unwrap_or(&0) + n <= (c.capacidad as f32 * 1.6) as i32).map(|c| c.id);
            }
            let Some(l) = elegido else { continue };
            if let Some(antes) = self.hogares[h as usize].lote.filter(|_| hacinado) {
                *ocupado.en(antes) -= n;
            }
            *ocupado.en(l) += n;
            let hg = &mut self.hogares[h as usize];
            hg.lote = Some(l);
            hg.alojamiento = Alojamiento::Solar;
            for m in hg.miembros.clone() {
                self.personas[m as usize].evento(ent.mes, TipoEvento::SeMudo, l);
            }
            // ¿De quién es la casa?
            if !self.duenos.contains_key(&l) {
                let nivel = info[&l].nivel as usize;
                let precio = VALOR_SOLAR[nivel];
                let hg = &mut self.hogares[h as usize];
                let jefe = hg.jefe;
                let fundador = primeros_años && self.personas[jefe as usize].estamento == Estamento::Vecino;
                let pobre = hg.clase <= Clase::Modesta && hg.ahorro < precio;
                if fundador || (reparto && pobre && nivel <= 1) {
                    // Merced de solar (fundadores) o reparto a familias pobres.
                    self.duenos.insert(l, h);
                    hg.solares.push(l);
                    self.personas[jefe as usize].evento(ent.mes, TipoEvento::CompraSolar, l);
                } else if hg.ahorro >= precio * 1.2 {
                    hg.ahorro -= precio;
                    ventas += precio;
                    self.duenos.insert(l, h);
                    hg.solares.push(l);
                    self.personas[jefe as usize].evento(ent.mes, TipoEvento::CompraSolar, l);
                }
            }
        }
        // Inversión: un hogar rico compra una casa del cabildo para alquilarla (una por mes).
        let libre = todas.iter().find(|c| !self.duenos.contains_key(&c.id)).map(|c| (c.id, c.nivel));
        if let Some((l, nivel)) = libre {
            let precio = VALOR_SOLAR[nivel as usize];
            let comprador = self.hogares.iter().filter(|h| h.activo && !Self::colectivo(h) && h.ahorro > precio * 2.0 && self.personas[h.jefe as usize].rasgos.ambicion() > 0.45).max_by(|a, b| a.ahorro.total_cmp(&b.ahorro).then(b.id.cmp(&a.id))).map(|h| h.id);
            if let Some(c) = comprador {
                let hg = &mut self.hogares[c as usize];
                hg.ahorro -= precio;
                hg.solares.push(l);
                ventas += precio;
                self.duenos.insert(l, c);
            }
        }
        // Comercios y talleres: el dueño es un comerciante o artesano con recursos, que no
        // tenga ya un negocio (los candidatos se ordenan una vez por mes, del más rico al menos).
        let tiene_negocio = |_: &Self, h: &Hogar| h.solares.iter().any(|x| info.get(x).is_some_and(|i| i.tipo != Some(ZoneKind::Residencial)));
        let mut candidatos: [Vec<HogarId>; 2] = Default::default();
        for h in self.hogares.iter().filter(|h| h.activo && !Self::colectivo(h) && h.ahorro >= 10.0) {
            let k = match self.personas[h.jefe as usize].oficio {
                Oficio::Comerciante | Oficio::Pulpero => 0,
                Oficio::Artesano | Oficio::Hacendado => 1,
                _ => continue,
            };
            if !tiene_negocio(self, h) {
                candidatos[k].push(h.id);
            }
        }
        for v in &mut candidatos {
            v.sort_by(|a, b| self.hogares[*b as usize].ahorro.total_cmp(&self.hogares[*a as usize].ahorro).then(a.cmp(b)));
            v.reverse();
        }
        for c in ent.lotes.iter().filter(|l| matches!(l.tipo, Some(ZoneKind::Comercial | ZoneKind::Industrial)) && l.nivel > 0) {
            if self.duenos.contains_key(&c.id) {
                continue;
            }
            let k = if c.tipo == Some(ZoneKind::Comercial) { 0 } else { 1 };
            let precio = VALOR_SOLAR[c.nivel as usize] * 0.5;
            // El más rico que quede (están al final).
            let Some(&h) = candidatos[k].last() else { continue };
            if self.hogares[h as usize].ahorro < precio {
                continue;
            }
            candidatos[k].pop();
            let hg = &mut self.hogares[h as usize];
            hg.ahorro -= precio;
            hg.solares.push(c.id);
            ventas += precio;
            self.duenos.insert(c.id, h);
        }
        if ventas >= 1.0 {
            inf.lineas.push(("Venta de solares".into(), ventas as i64));
        }
        self.reindexar();
    }

    /// Demanda de vivienda: personas sin casa más las que sobran en casas hacinadas.
    pub(crate) fn demanda_vivienda(&self, ent: &Entorno) -> i32 {
        let sin: usize = self.hogares.iter().filter(|h| h.activo && h.alojamiento == Alojamiento::SinCasa).map(|h| h.miembros.len()).sum();
        let mut sobra = 0;
        for l in &ent.lotes {
            if let Some(hs) = self.idx.por_lote.get(&l.id) {
                let occ: i32 = hs.iter().map(|&h| self.hogares[h as usize].miembros.len() as i32).sum();
                sobra += (occ - l.capacidad).max(0);
            }
        }
        sin as i32 + sobra
    }

    /// Hacinamiento de una casa: personas / capacidad.
    pub fn hacinamiento(&self, lote: LoteId, capacidad: i32) -> f32 {
        let occ: usize = self.hogares_de_lote(lote).map(|h| h.miembros.len()).sum();
        occ as f32 / capacidad.max(1) as f32
    }

    // --- Trabajo ------------------------------------------------------------------------

    pub(crate) fn empleo(&mut self, _rng: &mut Rng, ent: &Entorno) {
        let info = Lotes::de(ent);
        // Puestos ocupados y vacantes.
        let mut ocupados: Densa<i32> = Densa::default();
        let vivos = self.idx.vivos.clone();
        for &id in &vivos {
            let p = &mut self.personas[id as usize];
            if let Some(l) = p.trabajo {
                let ok = info.get(&l).is_some_and(|i| i.empleos > *ocupados.get(&l).unwrap_or(&0) && Some(i.tipo.unwrap_or(ZoneKind::Residencial)) == zona_de_trabajo(p.oficio)) && p.edad(ent.mes) < 66;
                if ok {
                    *ocupados.en(l) += 1;
                } else {
                    p.trabajo = None;
                }
            }
        }
        let mut vacantes: [Vec<LoteId>; 3] = Default::default();
        for l in &ent.lotes {
            if let Some(k) = l.tipo {
                for _ in *ocupados.get(&l.id).unwrap_or(&0)..l.empleos {
                    vacantes[k as usize].push(l.id);
                }
            }
        }
        for v in &mut vacantes {
            v.reverse();
        }
        for &id in &vivos {
            let p = &self.personas[id as usize];
            if p.trabajo.is_some() || !(15..66).contains(&p.edad(ent.mes)) {
                continue;
            }
            let Some(k) = zona_de_trabajo(p.oficio) else { continue };
            if let Some(l) = vacantes[k as usize].pop() {
                *ocupados.en(l) += 1;
                self.personas[id as usize].trabajo = Some(l);
            }
        }
    }

    // --- Ingresos y gastos ----------------------------------------------------------------

    /// Rinde más quien está contento y es laborioso.
    pub(crate) fn productividad(p: &Persona) -> f32 {
        0.8 + 0.25 * p.felicidad + 0.15 * p.rasgos.laboriosidad()
    }

    pub(crate) fn economia_hogares(&mut self, rng: &mut Rng, ent: &Entorno, inf: &mut InformeMes) {
        let _ = rng;
        let info = Lotes::de(ent);
        let n = self.hogares.len();
        // Alquileres: cada inquilino paga al dueño (o al cabildo) su parte de la casa.
        let mut renta = vec![0.0f32; n];
        let mut alquiler = vec![0.0f32; n];
        let mut al_cabildo = 0.0f32;
        for (l, hs) in &self.idx.por_lote {
            let Some(i) = info.get(l) else { continue };
            // Los alquileres siguen al valor del suelo (0,7 a 1,3 veces el de lista).
            let total = ALQUILER[i.nivel as usize] * (0.7 + 0.6 * i.valor.clamp(0.0, 1.0));
            let dueno = self.duenos.get(l).copied();
            for &h in hs {
                if Some(h) == dueno {
                    continue;
                }
                let parte = total / hs.len() as f32;
                alquiler[h as usize] += parte;
                match dueno {
                    Some(d) => renta[d as usize] += parte,
                    None => al_cabildo += parte,
                }
            }
        }
        // Ganancia de los dueños de comercios y talleres: un margen por cada empleado.
        for (l, ws) in &self.idx.trabajadores {
            if let Some(&d) = self.duenos.get(l) {
                renta[d as usize] += ws.len() as f32 * 2.5;
            }
        }
        let pan = ent.precio_pan.max(0.3);
        let carne = ent.precio_carne.max(0.3);
        let pan_barato = ent.vigente(Ordinance::PanDeLosPobres);
        let progresiva = ent.vigente(Ordinance::ContribucionProgresiva);
        let prestamos = ent.vigente(Ordinance::PrestamosCabildo);
        let tasa = ent.contribucion.max(0) as f32 / 100.0;
        let (mut subsidio_pan, mut recargo, mut exencion, mut prestado) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mes = ent.mes;
        for hi in 0..n {
            if !self.hogares[hi].activo || self.hogares[hi].miembros.is_empty() {
                continue;
            }
            let colectivo = Self::colectivo(&self.hogares[hi]);
            let clase = self.hogares[hi].clase;
            let miembros = self.hogares[hi].miembros.clone();
            let mut salarios = 0.0f32;
            let mut eq = 0.0f32;
            for &m in &miembros {
                let p = &mut self.personas[m as usize];
                let edad = p.edad(mes);
                eq += if edad >= 14 { 1.0 } else { 0.6 };
                if !(14..=70).contains(&edad) || p.oficio == Oficio::Ninguno {
                    continue;
                }
                let mut w = salario(p.oficio) * Self::productividad(p);
                if p.sexo == Sexo::Mujer {
                    w *= 0.6;
                }
                if zona_de_trabajo(p.oficio).is_some() && p.trabajo.is_none() {
                    w *= 0.75; // por cuenta propia, sin taller ni mostrador
                }
                match p.oficio {
                    Oficio::Labrador if ent.sequia => w *= 0.6,
                    Oficio::Hacendado if ent.sequia => w *= 0.7,
                    _ => {}
                }
                if colectivo {
                    // En el cuartel o el campamento cada uno guarda lo suyo.
                    p.riqueza += (w - SUBSISTENCIA * (0.45 * pan + 0.15 * carne + 0.4)).max(0.0) * 0.3;
                    continue;
                }
                salarios += w;
            }
            if colectivo {
                let h = &mut self.hogares[hi];
                h.ingresos = 0.0;
                h.gastos = 0.0;
                continue;
            }
            let h = &mut self.hogares[hi];
            // Ganado: cría y venta (las sequías lo achican).
            if h.ganado > 0 {
                // Crecimiento logístico: el campo de una familia no da para más de ~2.000 cabezas.
                let crece = if ent.sequia { -0.015 } else { 0.008 * (1.0 - h.ganado as f32 / 2000.0) };
                h.ganado = ((h.ganado as f32) * (1.0 + crece)).round().max(0.0) as u32;
            }
            let rentas = renta[hi] + h.ganado as f32 * 0.12 * if ent.sequia { 0.6 } else { 1.0 };
            let ingresos = salarios + rentas;
            // Comida al precio del mercado (el pan pesa casi la mitad).
            let pan_hogar = if pan_barato && (clase == Clase::Pobre || h.meses_de_hambre > 0) { pan.min(1.0) } else { pan };
            subsidio_pan += eq * SUBSISTENCIA * 0.45 * (pan - pan_hogar);
            let comida = eq * SUBSISTENCIA * (0.45 * pan_hogar + 0.15 * carne + 0.4);
            // Contribución: proporcional al ingreso; con la progresiva, los pobres no pagan y
            // los principales pagan más.
            let mut impuesto = tasa * ingresos * 0.5;
            if progresiva {
                match clase {
                    Clase::Pobre => {
                        exencion += impuesto;
                        impuesto = 0.0;
                    }
                    Clase::Principal => {
                        recargo += impuesto * 1.5;
                        impuesto *= 2.5;
                    }
                    Clase::Acomodada => {
                        recargo += impuesto * 0.5;
                        impuesto *= 1.5;
                    }
                    _ => {}
                }
            }
            let fijos = comida + alquiler[hi] + impuesto;
            let prudencia = self.personas[h.jefe as usize].rasgos.prudencia();
            let base = match clase {
                Clase::Pobre => 0.2,
                Clase::Modesta => 0.6,
                Clase::Acomodada => 2.5,
                Clase::Principal => 7.0,
            } * eq;
            let otros = base + 0.75 * (ingresos - fijos - base).max(0.0) * (1.0 - 0.4 * prudencia);
            let gastos = fijos + otros;
            // Intereses de la deuda (el cabildo presta más barato a los pobres y modestos).
            let barato = prestamos && clase <= Clase::Modesta;
            let interes = h.deuda * if barato { 0.002 } else { 0.006 };
            let balance = ingresos - gastos - interes;
            h.ingresos = ingresos;
            h.gastos = gastos + interes;
            h.detalle = [salarios, rentas, comida, alquiler[hi], impuesto, otros + interes];
            h.ahorro += balance;
            if h.ahorro < 0.0 {
                let falta = -h.ahorro;
                h.ahorro = 0.0;
                // ¿Quién le presta? El límite de deuda depende de lo que gana y tiene.
                let limite = (ingresos * 8.0).max(20.0) + h.solares.len() as f32 * 60.0 + if barato { 60.0 } else { 0.0 };
                let puede = (limite - h.deuda).max(0.0).min(falta);
                h.deuda += puede;
                if barato {
                    prestado += puede;
                }
                // Lo que no consiguió: comió menos.
                let cubierto = if comida > 0.0 { ((comida - (falta - puede)) / comida).clamp(0.0, 1.0) } else { 1.0 };
                h.meses_de_hambre = if cubierto < 0.8 { h.meses_de_hambre.saturating_add(1) } else { 0 };
            } else {
                h.meses_de_hambre = 0;
                if h.deuda > 0.0 {
                    let paga = h.deuda.min(h.ahorro * 0.5);
                    h.deuda -= paga;
                    h.ahorro -= paga;
                }
            }
            if h.meses_de_hambre == 3 {
                let j = h.jefe;
                self.personas[j as usize].evento(mes, TipoEvento::Empobrecio, NADIE);
            }
            // Mejora de la casa propia: los hogares que juntaron lo suficiente.
            let h = &self.hogares[hi];
            if let (Some(l), true) = (h.lote, h.vive_en_solar()) {
                if self.duenos.get(&l) == Some(&h.id) {
                    if let Some(i) = info.get(&l) {
                        let sig = (i.nivel + 1) as usize;
                        let ambicion = self.personas[h.jefe as usize].rasgos.ambicion();
                        if sig <= 3 && h.ahorro > COSTO_MEJORA[sig] * (1.7 - ambicion) {
                            inf.mejoras.push((l, sig as u8));
                        }
                    }
                }
            }
        }
        if al_cabildo >= 1.0 {
            inf.lineas.push(("Alquileres de casas del cabildo".into(), al_cabildo as i64));
        }
        if pan_barato && subsidio_pan >= 1.0 {
            inf.lineas.push(("Pan barato para los pobres".into(), -(subsidio_pan as i64)));
        }
        if progresiva {
            inf.lineas.push(("Recargo de la contribución progresiva".into(), recargo as i64));
            inf.lineas.push(("Exención de la contribución a los pobres".into(), -(exencion as i64)));
        }
        if prestamos {
            self.prestamos += prestado;
            // El cabildo cobra un interés bajo y pierde parte de lo prestado.
            let costo = (self.prestamos * 0.004) as i64 + 10;
            inf.lineas.push(("Préstamos del cabildo (costo)".into(), -costo));
            self.prestamos *= 0.99;
        }
        let pob = self.habitantes() as f32;
        if ent.vigente(Ordinance::HospitalCaridad) {
            inf.lineas.push(("Hospital de caridad".into(), -((20.0 + pob * 0.03) as i64)));
        }
        if ent.vigente(Ordinance::EscuelasPrimerasLetras) {
            let niños = self.vivos().filter(|p| (6..=12).contains(&p.edad(mes))).count() as f32;
            inf.lineas.push(("Escuelas de primeras letras".into(), -((15.0 + niños * 0.25) as i64)));
        }
        if ent.vigente(Ordinance::RepartoSolares) {
            inf.lineas.push(("Reparto de solares (mensura y escrituras)".into(), -25));
        }
    }

    /// El cabildo aplicó la mejora de la casa: el dueño la paga.
    pub fn mejora_hecha(&mut self, lote: LoteId, nivel: u8) {
        let Some(&h) = self.duenos.get(&lote) else { return };
        let costo = COSTO_MEJORA[(nivel as usize).min(3)];
        let hg = &mut self.hogares[h as usize];
        hg.ahorro -= costo;
        if hg.ahorro < 0.0 {
            hg.deuda += -hg.ahorro;
            hg.ahorro = 0.0;
        }
        let j = hg.jefe;
        let mes = self.mes;
        self.personas[j as usize].evento(mes, TipoEvento::MejoroCasa(nivel), lote);
    }

    // --- Clases -------------------------------------------------------------------------

    pub(crate) fn clasificar_hogares(&mut self) {
        for i in 0..self.hogares.len() {
            let h = &self.hogares[i];
            if !h.activo || h.miembros.is_empty() {
                continue;
            }
            if Self::colectivo(h) {
                self.hogares[i].clase = Clase::Modesta;
                continue;
            }
            let pat = self.patrimonio(h);
            let eq = self.equivalentes(h);
            let ing = h.ingresos / eq;
            let clase = if pat >= 3000.0 || ing >= 25.0 {
                Clase::Principal
            } else if pat >= 400.0 || ing >= 8.0 {
                Clase::Acomodada
            } else if ing < SUBSISTENCIA && pat < 60.0 {
                Clase::Pobre
            } else {
                Clase::Modesta
            };
            let jefe = h.jefe;
            let vecino = h.vive_en_solar() && h.lote.is_some_and(|l| self.duenos.get(&l) == Some(&h.id));
            let en_solar = h.vive_en_solar();
            self.hogares[i].clase = clase;
            let p = &mut self.personas[jefe as usize];
            if p.estamento != Estamento::Militar && p.estamento != Estamento::Clero {
                p.estamento = if vecino || p.estamento == Estamento::Vecino && en_solar { Estamento::Vecino } else { Estamento::Morador };
            }
        }
    }
}
