//! Demografía: muertes (con la herencia), nacimientos, casamientos y uniones, crecimiento
//! (escuela, oficio, salud), migraciones y la población agregada de las épocas tardías.
//!
//! Calibración (régimen pretransicional de Montevideo, 1724–1870; Pollero y González Demuro,
//! *Población y sociedad de Montevideo*, 2024): defunciones de 25 a 35 por mil al año; un
//! quinto de los nacidos moría antes del año; esperanza de vida al nacer de 33 a 41 años;
//! nacimientos por encima de 40 por mil; unos cinco hijos por mujer; diferencia media de edad
//! entre esposos de unos nueve años; nacimientos de uniones libres entre 20 y 40 %. Las tablas
//! por edad son ESTIMACIONES (modelo propio) ajustadas a esos valores; la prueba
//! `esperanza_de_vida_de_la_tabla` lo verifica.

use super::*;
use crate::city::economy::Ordinance;

/// Probabilidad anual de morir por edad (antes de los factores de cada persona).
pub fn q_anual(edad: i32) -> f32 {
    match edad {
        0 => 0.19,
        1..=4 => 0.035,
        5..=9 => 0.010,
        10..=14 => 0.006,
        15..=24 => 0.008,
        25..=34 => 0.011,
        35..=44 => 0.014,
        45..=54 => 0.020,
        55..=64 => 0.035,
        65..=74 => 0.07,
        75..=84 => 0.14,
        _ => 0.28,
    }
}

/// Fecundidad anual de las mujeres en pareja, por edad.
pub fn fecundidad(edad: i32) -> f32 {
    match edad {
        15..=19 => 0.32,
        20..=24 => 0.42,
        25..=29 => 0.40,
        30..=34 => 0.35,
        35..=39 => 0.27,
        40..=44 => 0.13,
        45..=49 => 0.03,
        _ => 0.0,
    }
}

/// Probabilidad anual de casarse o unirse (mujeres solteras o viudas), por edad.
fn nupcialidad(edad: i32) -> f32 {
    match edad {
        15..=17 => 0.10,
        18..=24 => 0.30,
        25..=29 => 0.25,
        30..=39 => 0.12,
        40..=45 => 0.04,
        _ => 0.0,
    }
}

/// Esperanza de vida al nacer de la tabla `q_anual` (sin factores): para la calibración.
pub fn esperanza_de_vida() -> f32 {
    let mut l = 1.0f32;
    let mut e = 0.0f32;
    for edad in 0..110 {
        let q = q_anual(edad);
        e += l * (1.0 - q * 0.5);
        l *= 1.0 - q;
    }
    e
}

impl Poblacion {
    pub(crate) fn sola(&self, p: &Persona) -> bool {
        p.conyuge.is_none_or(|c| !self.personas[c as usize].viva())
    }

    pub(crate) fn casar(&mut self, a: PersonaId, b: PersonaId, formal: bool, mes: i32) {
        let tipo = if formal { TipoEvento::Caso } else { TipoEvento::Union };
        for (x, y) in [(a, b), (b, a)] {
            let p = &mut self.personas[x as usize];
            p.conyuge = Some(y);
            p.union_formal = formal;
            p.evento(mes, tipo, y);
        }
    }

    // --- Muertes ------------------------------------------------------------------------

    pub(crate) fn muertes(&mut self, rng: &mut Rng, ent: &Entorno, noticias: &mut Vec<String>) {
        let hospital = ent.vigente(Ordinance::HospitalCaridad);
        let mes = ent.mes;
        let mut muertos = Vec::new();
        for &id in &self.idx.vivos {
            let p = &self.personas[id as usize];
            let edad = p.edad(mes);
            let mut f = 1.75 - p.salud * 0.7;
            let h = p.hogar.map(|h| &self.hogares[h as usize]);
            if let Some(h) = h {
                f *= match h.clase {
                    Clase::Pobre => 1.25,
                    Clase::Modesta => 1.0,
                    Clase::Acomodada => 0.85,
                    Clase::Principal => 0.75,
                };
                if h.alojamiento == Alojamiento::SinCasa {
                    f *= 1.2;
                }
                if h.meses_de_hambre >= 3 {
                    f *= 1.5;
                }
                if hospital && h.clase <= Clase::Modesta && !ent.con_servicios {
                    f *= 0.85;
                }
            }
            if ent.con_servicios {
                // Botica, médico y hospital cerca (los pobres, si el cabildo les paga la cama);
                // el agua mala, los pozos negros y la basura: diarreas y fiebres, sobre todo en
                // los chicos.
                let sv = ent.servicios_hogar(h);
                let acceso = match h.map_or(Clase::Pobre, |h| h.clase) {
                    Clase::Pobre | Clase::Modesta if !hospital => 0.5,
                    _ => 1.0,
                };
                f *= 1.0 - 0.3 * sv.salud * acceso;
                f *= 1.0 + sv.insalubre * if edad < 5 { 1.3 } else { 0.35 };
            }
            if p.condicion == Condicion::Esclavizada {
                f *= 1.3;
            }
            if ent.epidemia {
                f *= if edad < 5 || edad > 60 { 2.2 } else { 1.6 };
            }
            let q = (q_anual(edad) * f).min(0.95);
            let mensual = 1.0 - (1.0 - q).powf(1.0 / 12.0);
            if rng.f() < mensual {
                let hambre = h.is_some_and(|h| h.meses_de_hambre >= 3);
                muertos.push((id, if ent.epidemia && rng.f() < 0.5 { Causa::Epidemia } else if hambre && rng.f() < 0.4 { Causa::Hambre } else { Causa::Natural }));
            }
        }
        for (id, c) in muertos {
            self.fallecer(id, c, noticias);
        }
        self.depurar_vivos();
    }

    /// Muerte de una persona: duelo, viudez y la herencia según el derecho castellano (ver
    /// [`Poblacion::heredar`]).
    pub fn morir(&mut self, id: PersonaId, causa: Causa, noticias: &mut Vec<String>) {
        self.fallecer(id, causa, noticias);
        self.depurar_vivos();
    }

    /// Como `morir`, sin depurar la lista de vivos (para las fases que matan a muchos).
    pub(crate) fn fallecer(&mut self, id: PersonaId, causa: Causa, noticias: &mut Vec<String>) {
        let mes = self.mes;
        if !self.personas[id as usize].viva() {
            return;
        }
        let edad = self.personas[id as usize].edad(mes);
        {
            let p = &mut self.personas[id as usize];
            p.muerte = Some(mes);
            p.trabajo = None;
            p.evento(mes, TipoEvento::Murio(causa), NADIE);
        }
        self.mes_actual.muertes += 1;
        if edad == 0 {
            self.mes_actual.muertes_infantiles += 1;
        }
        // Duelo y viudez.
        let p = self.personas[id as usize].clone();
        if let Some(c) = p.conyuge.filter(|&c| self.personas[c as usize].viva() && self.personas[c as usize].conyuge == Some(id)) {
            let cp = &mut self.personas[c as usize];
            cp.evento(mes, TipoEvento::Enviudo, id);
            cp.duelo = 8;
        }
        for &h in &p.hijos {
            let hp = &mut self.personas[h as usize];
            if hp.viva() {
                hp.evento(mes, TipoEvento::MurioPadre, id);
                hp.duelo = hp.duelo.max(6);
            }
        }
        for pa in [p.padre, p.madre].into_iter().flatten() {
            let pp = &mut self.personas[pa as usize];
            if pp.viva() {
                pp.evento(mes, TipoEvento::MurioHijo, id);
                pp.duelo = pp.duelo.max(if edad < 5 { 4 } else { 8 });
            }
        }
        // Sale del hogar; si era el jefe, se hereda.
        if let Some(h) = p.hogar {
            self.hogares[h as usize].miembros.retain(|&m| m != id);
            let r = std::mem::take(&mut self.personas[id as usize].riqueza);
            let colectivo = Self::colectivo(&self.hogares[h as usize]);
            if colectivo {
                // Lo que guardaba un soldado o un peón de la obra: a sus herederos o, si no
                // tiene, a la Iglesia.
                self.repartir_efectivo(id, r, noticias);
                if self.hogares[h as usize].jefe == id {
                    if let Some(&nuevo) = self.hogares[h as usize].miembros.first() {
                        self.hogares[h as usize].jefe = nuevo;
                    } else {
                        self.hogares[h as usize].activo = false;
                    }
                }
            } else {
                self.hogares[h as usize].ahorro += r;
                if self.hogares[h as usize].jefe == id {
                    self.heredar(id, h, noticias);
                } else if self.hogares[h as usize].miembros.iter().all(|&m| self.personas[m as usize].condicion == Condicion::Esclavizada) {
                    self.heredar(id, h, noticias);
                }
            }
        }
        // `idx.vivos` se depura después de cada fase (ver `depurar_vivos`), no acá: sacar de
        // a uno es lento con cientos de miles de personas.
    }

    pub(crate) fn depurar_vivos(&mut self) {
        let personas = &self.personas;
        self.idx.vivos.retain(|&v| personas[v as usize].viva());
    }

    /// Herederos forzosos: hijos vivos; si no hay, el cónyuge; si no, los padres; si no, los
    /// hermanos. Vacío si no tiene a nadie.
    pub fn herederos(&self, id: PersonaId) -> Vec<PersonaId> {
        let p = &self.personas[id as usize];
        let vivos = |v: &[PersonaId]| v.iter().copied().filter(|&x| self.personas[x as usize].viva() && x != id).collect::<Vec<_>>();
        let hijos = vivos(&p.hijos);
        if !hijos.is_empty() {
            return hijos;
        }
        if let Some(c) = p.conyuge.filter(|&c| self.personas[c as usize].viva()) {
            return vec![c];
        }
        let padres = vivos(&[p.padre, p.madre].into_iter().flatten().collect::<Vec<_>>());
        if !padres.is_empty() {
            return padres;
        }
        let mut hermanos: Vec<PersonaId> = [p.padre, p.madre].into_iter().flatten().flat_map(|x| self.personas[x as usize].hijos.clone()).collect();
        hermanos.sort();
        hermanos.dedup();
        vivos(&hermanos)
    }

    /// Lleva dinero de un hogar a otro (exacto: lo que falta en efectivo se vuelve deuda).
    fn transferir(&mut self, de: HogarId, a: HogarId, monto: f32) {
        if de == a || monto <= 0.0 {
            return;
        }
        let d = &mut self.hogares[de as usize];
        let efectivo = d.ahorro.min(monto);
        d.ahorro -= efectivo;
        d.deuda += monto - efectivo;
        self.hogares[a as usize].ahorro += monto;
    }

    /// Reparte dinero suelto (de alguien sin hogar propio) entre sus herederos.
    fn repartir_efectivo(&mut self, id: PersonaId, monto: f32, _noticias: &mut Vec<String>) {
        if monto <= 0.0 {
            return;
        }
        let hs: Vec<HogarId> = self.herederos(id).into_iter().filter_map(|x| self.personas[x as usize].hogar).filter(|&h| self.hogares[h as usize].activo).collect();
        if hs.is_empty() {
            self.legados += monto as f64;
            return;
        }
        let parte = monto / hs.len() as f32;
        for h in hs {
            self.hogares[h as usize].ahorro += parte;
        }
    }

    /// Herencia al morir el jefe del hogar `h`. Derecho castellano: si vive el cónyuge, la
    /// mitad de los gananciales es suya y sigue al frente de la casa; la otra mitad es la
    /// herencia. De ella, cuatro quintos son la legítima de los hijos, por partes iguales, y el
    /// quinto de libre disposición se deja en misas y obras pías si el difunto era devoto (o
    /// se suma a la legítima). Los herederos que viven en otra casa cobran en efectivo (lo que
    /// falta se vuelve deuda: de ahí los pleitos); los que viven en la casa, conservan su parte
    /// en ella. Si no queda nadie libre en la casa, el hogar se disuelve y todo se reparte:
    /// solares, ganado y dinero. La riqueza se conserva exactamente (prueba
    /// `la_herencia_conserva_la_riqueza`).
    pub(crate) fn heredar(&mut self, id: PersonaId, h: HogarId, noticias: &mut Vec<String>) {
        let mes = self.mes;
        let devoto = self.personas[id as usize].rasgos.devocion() > 0.55;
        let conyuge = self.personas[id as usize].conyuge.filter(|&c| self.personas[c as usize].viva() && self.personas[c as usize].hogar == Some(h));
        let herederos = self.herederos(id);
        // Libertad por testamento de quienes vivían sometidos en la casa.
        let sometidas: Vec<PersonaId> = self.hogares[h as usize].miembros.iter().copied().filter(|&m| self.personas[m as usize].condicion == Condicion::Esclavizada).collect();
        let p_testamento = 0.08 + 0.35 * self.personas[id as usize].rasgos.devocion();
        let mut rng = Rng::from_state(self.azar ^ 0x5eed ^ id as u64);
        for &s in &sometidas {
            if rng.f() < p_testamento {
                self.liberar(s, ViaLibertad::Testamento, noticias);
            }
        }
        // Quienes quedan libres en la casa (contando a quienes se acaba de liberar y siguen en
        // ella, como los niños).
        let libres: Vec<PersonaId> = self.hogares[h as usize].miembros.iter().copied().filter(|&m| self.personas[m as usize].libre()).collect();
        let continua = !libres.is_empty();
        if continua {
            // La casa sigue: nuevo jefe y la parte de los herederos de afuera.
            let nuevo = conyuge.unwrap_or_else(|| *libres.iter().max_by_key(|&&m| (self.personas[m as usize].edad(mes), std::cmp::Reverse(m))).unwrap());
            self.hogares[h as usize].jefe = nuevo;
            let pat = self.hogares[h as usize].ahorro - self.hogares[h as usize].deuda;
            // Se reparte el efectivo neto y el ganado (la casa queda en la familia, que compensa
            // a los de afuera en dinero).
            let herencia = (pat.max(0.0) + self.hogares[h as usize].ganado as f32 * economia::PRECIO_GANADO) * if conyuge.is_some() { 0.5 } else { 1.0 };
            if herencia <= 0.0 || herederos.is_empty() {
                return;
            }
            let quinto = herencia * 0.2;
            let legitima = if devoto { herencia - quinto } else { herencia };
            if devoto {
                let d = &mut self.hogares[h as usize];
                let efectivo = d.ahorro.min(quinto);
                d.ahorro -= efectivo;
                d.deuda += quinto - efectivo;
                self.legados += quinto as f64;
            }
            let parte = legitima / herederos.len() as f32;
            let mut afuera = 0;
            for &x in &herederos {
                let hx = self.personas[x as usize].hogar;
                if hx == Some(h) {
                    continue;
                }
                if let Some(hx) = hx.filter(|&hx| self.hogares[hx as usize].activo) {
                    self.transferir(h, hx, parte);
                    afuera += 1;
                    self.personas[x as usize].evento(mes, TipoEvento::Heredo(parte as u32), id);
                }
            }
            if afuera >= 2 && self.hogares[h as usize].deuda > self.hogares[h as usize].ahorro + 50.0 {
                for &x in &herederos {
                    self.personas[x as usize].evento(mes, TipoEvento::Pleito, id);
                }
                let t = format!("Pleito por la herencia de {}: {} hijos reclaman su parte y la casa no tiene con qué pagar", self.nombre_completo(id), herederos.len());
                noticias.push(t.clone());
                self.anotar(mes, t);
            }
            return;
        }
        // Nadie libre en la casa: se disuelve y se reparte todo.
        let destinos: Vec<HogarId> = herederos.iter().filter_map(|&x| self.personas[x as usize].hogar).filter(|&hx| hx != h && self.hogares[hx as usize].activo).collect();
        let hg = std::mem::replace(&mut self.hogares[h as usize].solares, Vec::new());
        let ganado = std::mem::take(&mut self.hogares[h as usize].ganado);
        let neto = self.hogares[h as usize].ahorro - self.hogares[h as usize].deuda;
        self.hogares[h as usize].ahorro = 0.0;
        self.hogares[h as usize].deuda = 0.0;
        self.hogares[h as usize].activo = false;
        if destinos.is_empty() {
            // Herencia vacante: los solares vuelven al cabildo y el resto a obras pías.
            for l in hg {
                self.duenos.remove(&l);
                self.legados_solar(l);
            }
            self.legados += (neto + ganado as f32 * economia::PRECIO_GANADO) as f64;
            for s in sometidas {
                // Sin herederos, quedan libres.
                self.liberar(s, ViaLibertad::Testamento, noticias);
            }
            // Los niños liberados siguen juntos en la casa, sin bienes.
            if let Some(&mayor) = self.hogares[h as usize].miembros.iter().max_by_key(|&&m| (self.personas[m as usize].edad(mes), std::cmp::Reverse(m))) {
                let hg = &mut self.hogares[h as usize];
                hg.activo = true;
                hg.jefe = mayor;
            }
            return;
        }
        let n = destinos.len();
        let (legado, neto) = if devoto && neto > 0.0 { (neto * 0.2, neto * 0.8) } else { (0.0, neto) };
        self.legados += legado as f64;
        for (i, &d) in destinos.iter().enumerate() {
            let parte = neto / n as f32;
            if parte >= 0.0 {
                self.hogares[d as usize].ahorro += parte;
            } else {
                self.hogares[d as usize].deuda -= parte;
            }
            self.hogares[d as usize].ganado += ganado / n as u32 + if i == 0 { ganado % n as u32 } else { 0 };
        }
        for (i, l) in hg.into_iter().enumerate() {
            let d = destinos[i % n];
            self.hogares[d as usize].solares.push(l);
            self.duenos.insert(l, d);
        }
        for &x in &herederos {
            self.personas[x as usize].evento(mes, TipoEvento::Heredo((neto / n as f32).max(0.0) as u32), id);
        }
        // Quienes seguían sometidas pasan a la casa de los herederos: la injusticia sigue.
        for s in sometidas {
            if self.personas[s as usize].condicion == Condicion::Esclavizada {
                let d = destinos[0];
                self.unir_a_hogar(s, d);
                let t = format!("Tras la muerte de {}, {} sigue sin libertad y pasa a vivir con la familia {}", self.nombre_completo(id), self.nombre_completo(s), self.apellido_hogar(d));
                self.anotar(mes, t);
            }
        }
    }

    /// El valor de un solar que vuelve al cabildo sale del patrimonio de las familias.
    fn legados_solar(&mut self, l: LoteId) {
        let nivel = *self.niveles.get(&l).unwrap_or(&0) as usize;
        self.legados += economia::VALOR_SOLAR[nivel.min(3)] as f64;
    }

    // --- Nacimientos --------------------------------------------------------------------

    pub(crate) fn nacimientos(&mut self, rng: &mut Rng, ent: &Entorno) {
        let mes = ent.mes;
        let vientres = self.leyes.vientres.is_some() || self.leyes.abolicion.is_some();
        let madres: Vec<PersonaId> = self
            .idx
            .vivos
            .iter()
            .copied()
            .filter(|&m| {
                let p = &self.personas[m as usize];
                p.sexo == Sexo::Mujer && (15..50).contains(&p.edad(mes)) && p.conyuge.is_some_and(|c| self.personas[c as usize].viva())
            })
            .collect();
        for m in madres {
            let p = &self.personas[m as usize];
            // Embarazo y lactancia: un año como mínimo entre partos.
            if p.hijos.last().is_some_and(|&h| self.personas[h as usize].nacimiento > mes - 15) {
                continue;
            }
            let padre = p.conyuge.unwrap();
            let juntos = self.personas[padre as usize].hogar == p.hogar;
            let f = fecundidad(p.edad(mes)) / 12.0 * (0.55 + 0.55 * p.salud) * (0.85 + 0.3 * p.felicidad) * if juntos { 1.0 } else { 0.6 };
            if rng.f() >= f {
                continue;
            }
            let sexo = if rng.f() < 0.512 { Sexo::Hombre } else { Sexo::Mujer };
            let ap_p = self.personas[padre as usize].apellidos[0];
            let ap_m = p.apellidos[0];
            let apellidos = if ap_p != NADIE { [ap_p, ap_m] } else { [ap_m, NADIE] };
            let esclava = p.condicion == Condicion::Esclavizada;
            let condicion = if esclava && !vientres { Condicion::Esclavizada } else { Condicion::Libre };
            let hogar = p.hogar;
            let c = self.crear_persona(rng, sexo, mes, Origen::Montevideo, condicion, apellidos, Some(padre), Some(m));
            if esclava && vientres {
                self.personas[c as usize].evento(mes, TipoEvento::Libertad(ViaLibertad::Vientres), NADIE);
            }
            self.personas[m as usize].hijos.push(c);
            self.personas[padre as usize].hijos.push(c);
            self.personas[m as usize].evento(mes, TipoEvento::NacioHijo, c);
            self.personas[padre as usize].evento(mes, TipoEvento::NacioHijo, c);
            if let Some(h) = hogar {
                self.unir_a_hogar(c, h);
            }
            self.mes_actual.nacimientos += 1;
            // Mortalidad materna (estimación: alrededor del 1 % de los partos).
            if rng.f() < 0.008 * (1.6 - self.personas[m as usize].salud) {
                let mut basura = Vec::new();
                self.fallecer(m, Causa::Parto, &mut basura);
            }
        }
        self.depurar_vivos();
    }

    // --- Casamientos ----------------------------------------------------------------------

    fn puntaje(&self, mujer: &Persona, hombre: &Persona, mes: i32) -> f32 {
        let d = (hombre.edad(mes) - mujer.edad(mes)) as f32;
        if d < -4.0 {
            return 0.0;
        }
        // Parientes cercanos no.
        let comparten = |a: Option<PersonaId>, b: Option<PersonaId>| a.is_some() && a == b;
        if comparten(mujer.padre, hombre.padre) || comparten(mujer.madre, hombre.madre) {
            return 0.0;
        }
        let mut s = (-((d - 8.0) / 7.0).powi(2) * 0.5).exp();
        if mujer.origen.grupo() == hombre.origen.grupo() {
            s *= 2.0;
        }
        if mujer.raiz == hombre.raiz {
            s *= 1.5;
        }
        let ch = |p: &Persona| p.hogar.map_or(Clase::Pobre, |h| self.hogares[h as usize].clase) as i32;
        s *= match (ch(mujer) - ch(hombre)).abs() {
            0 => 1.8,
            1 => 1.0,
            _ => 0.35,
        };
        let esc = |p: &Persona| p.condicion == Condicion::Esclavizada;
        s *= match (esc(mujer), esc(hombre)) {
            (true, true) => 1.5,
            (false, false) => 1.0,
            _ => 0.15,
        };
        if hombre.oficio != Oficio::Ninguno {
            s *= 1.5;
        }
        if hombre.hogar.is_some_and(|h| self.hogares[h as usize].jefe == hombre.id && self.hogares[h as usize].vive_en_solar()) {
            s *= 1.3;
        }
        s * (0.6 + 0.8 * hombre.rasgos.sociabilidad())
    }

    pub(crate) fn casamientos(&mut self, rng: &mut Rng, ent: &Entorno, noticias: &mut Vec<String>) {
        let mes = ent.mes;
        let mut hombres: Vec<PersonaId> = self
            .idx
            .vivos
            .iter()
            .copied()
            .filter(|&x| {
                let p = &self.personas[x as usize];
                p.sexo == Sexo::Hombre && (18..66).contains(&p.edad(mes)) && p.oficio != Oficio::Clerigo && self.sola(p) && p.duelo == 0
            })
            .collect();
        if hombres.is_empty() {
            return;
        }
        let mujeres: Vec<PersonaId> = self
            .idx
            .vivos
            .iter()
            .copied()
            .filter(|&x| {
                let p = &self.personas[x as usize];
                p.sexo == Sexo::Mujer && (15..46).contains(&p.edad(mes)) && p.oficio != Oficio::Clerigo && self.sola(p) && p.duelo == 0
            })
            .collect();
        for m in mujeres {
            if hombres.is_empty() {
                break;
            }
            let p = &self.personas[m as usize];
            let viuda = p.conyuge.is_some();
            let haz = nupcialidad(p.edad(mes)) / 12.0 * (0.7 + 0.6 * p.rasgos.sociabilidad()) * if viuda { 0.6 } else { 1.0 };
            if rng.f() >= haz {
                continue;
            }
            let mut mejor: Option<(usize, f32)> = None;
            for _ in 0..16.min(hombres.len() * 2) {
                let i = (rng.next() % hombres.len() as u64) as usize;
                let s = self.puntaje(p, &self.personas[hombres[i] as usize], mes);
                if s > mejor.map_or(0.05, |b| b.1) {
                    mejor = Some((i, s));
                }
            }
            let Some((i, _)) = mejor else { continue };
            let h = hombres.swap_remove(i);
            self.formar_pareja(rng, m, h, mes, noticias);
        }
    }

    fn formar_pareja(&mut self, rng: &mut Rng, mujer: PersonaId, hombre: PersonaId, mes: i32, noticias: &mut Vec<String>) {
        let clase = |s: &Self, p: PersonaId| s.personas[p as usize].hogar.map_or(Clase::Pobre, |h| s.hogares[h as usize].clase);
        let cl = clase(self, hombre).max(clase(self, mujer));
        let alguna_esclava = !self.personas[mujer as usize].libre() || !self.personas[hombre as usize].libre();
        let p_formal = if alguna_esclava {
            0.5
        } else {
            match cl {
                Clase::Principal | Clase::Acomodada => 0.97,
                Clase::Modesta => 0.8,
                Clase::Pobre => 0.6,
            }
        };
        let formal = rng.f() < p_formal;
        self.casar(mujer, hombre, formal, mes);
        self.mes_actual.casamientos += 1;
        let hm = self.personas[mujer as usize].hogar;
        let hh = self.personas[hombre as usize].hogar;
        // Las personas esclavizadas no pueden mudarse por su cuenta: la pareja vive separada
        // hasta que las dos sean libres (salvo que vivan en la misma casa).
        if alguna_esclava {
            return;
        }
        let jefe_de = |s: &Self, p: PersonaId, h: Option<HogarId>| h.filter(|&h| s.hogares[h as usize].jefe == p && !Self::colectivo(&s.hogares[h as usize]));
        let destino = match (jefe_de(self, hombre, hh), jefe_de(self, mujer, hm)) {
            (Some(a), Some(b)) => {
                // Se juntan dos casas: todo pasa a la del marido.
                let miembros = self.hogares[b as usize].miembros.clone();
                for x in miembros {
                    self.unir_a_hogar(x, a);
                }
                let (ah, de, so, ga) = {
                    let hb = &mut self.hogares[b as usize];
                    (std::mem::take(&mut hb.ahorro), std::mem::take(&mut hb.deuda), std::mem::take(&mut hb.solares), std::mem::take(&mut hb.ganado))
                };
                self.hogares[b as usize].activo = false;
                let ha = &mut self.hogares[a as usize];
                ha.ahorro += ah;
                ha.deuda += de;
                ha.ganado += ga;
                for l in so {
                    ha.solares.push(l);
                    self.duenos.insert(l, a);
                }
                a
            }
            (Some(a), None) => a,
            (None, Some(b)) => {
                self.hogares[b as usize].jefe = hombre;
                b
            }
            (None, None) => {
                let o = self.personas[hombre as usize].origen;
                self.crear_hogar(hombre, o, Alojamiento::SinCasa)
            }
        };
        self.unir_a_hogar(hombre, destino);
        let antes = self.personas[mujer as usize].hogar;
        self.unir_a_hogar(mujer, destino);
        // Dote: la familia de la novia aporta parte de sus ahorros (se conserva la riqueza).
        if let Some(a) = antes.filter(|&a| a != destino && self.hogares[a as usize].activo) {
            let dote = self.hogares[a as usize].ahorro * if cl >= Clase::Acomodada { 0.2 } else { 0.08 };
            if dote > 0.0 {
                self.hogares[a as usize].ahorro -= dote;
                self.hogares[destino as usize].ahorro += dote;
            }
            // Los hijos chicos de una viuda van con ella.
            for c in self.personas[mujer as usize].hijos.clone() {
                let cp = &self.personas[c as usize];
                if cp.viva() && cp.edad(mes) < 14 && cp.hogar == Some(a) && cp.libre() {
                    self.unir_a_hogar(c, destino);
                }
            }
        }
        // Casamiento entre familias principales: noticia.
        let principal = |s: &Self, h: Option<HogarId>| h.is_some_and(|h| s.hogares[h as usize].clase >= Clase::Acomodada);
        let alguna_principal = [hh, antes].iter().any(|h| h.is_some_and(|h| self.hogares[h as usize].clase == Clase::Principal));
        if formal && principal(self, hh) && principal(self, antes) && alguna_principal && antes != hh {
            let t = format!("Se casan {} y {}: se unen las familias {} y {}", self.nombre_completo(hombre), self.nombre_completo(mujer), self.texto(self.personas[hombre as usize].apellidos[0]), self.texto(self.personas[mujer as usize].apellidos[0]));
            noticias.push(t.clone());
            self.anotar(mes, t);
        }
    }

    // --- Crecer: escuela, oficio, salud, duelo -------------------------------------------

    pub(crate) fn crecer(&mut self, rng: &mut Rng, ent: &Entorno) {
        let mes = ent.mes;
        let escuelas = ent.vigente(Ordinance::EscuelasPrimerasLetras);
        let hospital = ent.vigente(Ordinance::HospitalCaridad);
        let tardia = self.epoca >= Epoca::Inmigracion;
        let vivos = self.idx.vivos.clone();
        for id in vivos {
            let (edad, cumple, hogar) = {
                let p = &self.personas[id as usize];
                (p.edad(mes), (mes - p.nacimiento).rem_euclid(12) == 0, p.hogar)
            };
            let (clase, hambre, sin_casa) = hogar.map_or((Clase::Pobre, 0, true), |h| {
                let h = &self.hogares[h as usize];
                (h.clase, h.meses_de_hambre, h.alojamiento == Alojamiento::SinCasa)
            });
            let sv = ent.servicios_hogar(hogar.map(|h| &self.hogares[h as usize]));
            let padre_oficio = self.personas[id as usize].padre.map(|f| self.personas[f as usize].oficio);
            let p = &mut self.personas[id as usize];
            p.duelo = p.duelo.saturating_sub(1);
            // Salud: tiende a un valor según la edad, la comida y la casa.
            let mut meta = 0.9 - (edad - 40).max(0) as f32 * 0.008;
            if hambre >= 2 {
                meta -= 0.25;
            }
            if sin_casa {
                meta -= 0.1;
            }
            if clase == Clase::Pobre {
                meta -= 0.08;
            }
            if ent.con_servicios {
                let acceso = if hospital || clase >= Clase::Acomodada { 1.0 } else { 0.5 };
                meta += 0.08 * sv.salud * acceso - 0.15 * sv.insalubre;
            } else if hospital && clase <= Clase::Modesta {
                meta += 0.05;
            }
            if p.condicion == Condicion::Esclavizada {
                meta -= 0.08;
            }
            p.salud = (p.salud + (meta.clamp(0.1, 1.0) - p.salud) * 0.08).clamp(0.05, 1.0);
            if !cumple {
                continue;
            }
            // Escuela de primeras letras (6 a 12 años).
            if (6..=12).contains(&edad) && p.educacion < 2 {
                let mut chance = match clase {
                    Clase::Principal => 0.25,
                    Clase::Acomodada => 0.15,
                    Clase::Modesta => 0.05,
                    Clase::Pobre => 0.015,
                };
                if ent.con_servicios {
                    // La escuela del barrio: hay que tener lugar en una; los pobres y modestos
                    // van si el cabildo paga al maestro (si no, pocos pueden pagarle).
                    let paga = match clase {
                        Clase::Pobre if !escuelas => 0.25,
                        Clase::Modesta if !escuelas => 0.6,
                        _ => 1.0,
                    };
                    chance += 0.4 * sv.escuela * paga;
                } else if escuelas {
                    chance += 0.2;
                }
                if tardia {
                    chance += 0.3;
                }
                if p.condicion == Condicion::Esclavizada {
                    // Restricción de la época: casi sin acceso a la escuela.
                    chance = if escuelas && (!ent.con_servicios || sv.escuela > 0.0) { 0.04 } else { 0.0 };
                }
                if rng.f() < chance {
                    p.educacion += 1;
                    p.evento(mes, TipoEvento::Aprendio, NADIE);
                }
            }
            // A los 15, un oficio.
            if edad == 15 && p.oficio == Oficio::Ninguno {
                if p.sexo == Sexo::Mujer && rng.f() > 0.4 {
                    continue;
                }
                let of = elegir_oficio(rng, p, padre_oficio, clase, tardia);
                p.oficio = of;
                p.evento(mes, TipoEvento::Oficio(of), NADIE);
            }
            // Movilidad: un adulto ambicioso que sabe leer puede cambiar de oficio.
            if (20..45).contains(&edad) && p.libre() && p.educacion >= 2 && matches!(p.oficio, Oficio::Peon | Oficio::Servicio | Oficio::Artesano | Oficio::Marinero) && rng.f() < 0.03 * p.rasgos.ambicion() {
                let of = if rng.f() < 0.6 { Oficio::Pulpero } else if tardia { Oficio::Empleado } else { Oficio::Comerciante };
                p.oficio = of;
                p.trabajo = None;
                p.evento(mes, TipoEvento::Oficio(of), NADIE);
            }
        }
    }

    // --- Migraciones --------------------------------------------------------------------

    pub(crate) fn migraciones(&mut self, rng: &mut Rng, ent: &Entorno, noticias: &mut Vec<String>) {
        let mes = ent.mes;
        self.obra += 1;
        let mut se_van: Vec<PersonaId> = Vec::new();
        let mut se_quedan: Vec<PersonaId> = Vec::new();
        let mut al_campo: Vec<HogarId> = Vec::new();
        for h in 0..self.hogares.len() {
            let hg = &self.hogares[h];
            if !hg.activo || hg.miembros.is_empty() {
                continue;
            }
            match hg.alojamiento {
                Alojamiento::Campamento => {
                    // Las cuadrillas guaraníes vuelven a sus pueblos; algunos se quedan.
                    let p = if self.obra < 48 { 0.02 } else { 0.08 };
                    for &m in &hg.miembros {
                        if rng.f() < p {
                            if rng.f() < 0.12 {
                                se_quedan.push(m);
                            } else {
                                se_van.push(m);
                            }
                        }
                    }
                }
                Alojamiento::Cuartel => {
                    // La tropa rota; los casados dejan el cuartel.
                    for &m in &hg.miembros {
                        let pm = &self.personas[m as usize];
                        if !self.sola(pm) {
                            se_quedan.push(m);
                        } else if rng.f() < 0.006 {
                            se_van.push(m);
                        }
                    }
                }
                Alojamiento::SinCasa => {
                    // Sin casa en la ciudad: los de oficios del campo se van a las chacras y
                    // estancias de la campaña (siguen siendo vecinos de la jurisdicción); los
                    // demás, después de un tiempo, se van.
                    if hg.miembros.iter().any(|&m| self.personas[m as usize].condicion == Condicion::Esclavizada) {
                        continue;
                    }
                    let campo = matches!(self.personas[hg.jefe as usize].oficio, Oficio::Labrador | Oficio::Hacendado | Oficio::Peon);
                    if campo && rng.f() < 0.06 {
                        al_campo.push(hg.id);
                    } else if rng.f() < 0.025 {
                        se_van.extend(hg.miembros.iter().copied());
                    }
                }
                Alojamiento::Campo => {
                    if rng.f() < 0.0008 {
                        se_van.extend(hg.miembros.iter().copied());
                    }
                }
                _ => {
                    // Emigran hogares libres descontentos (los que tienen bienes, menos).
                    if hg.miembros.iter().any(|&m| self.personas[m as usize].condicion == Condicion::Esclavizada) {
                        continue;
                    }
                    let fel = hg.miembros.iter().map(|&m| self.personas[m as usize].felicidad).sum::<f32>() / hg.miembros.len() as f32;
                    let jefe = &self.personas[hg.jefe as usize];
                    let mut p = 0.0006 + (0.4 - fel).max(0.0) * 0.03 * (1.3 - jefe.rasgos.prudencia());
                    if !hg.solares.is_empty() {
                        p *= 0.3;
                    }
                    if rng.f() < p {
                        se_van.extend(hg.miembros.iter().copied());
                    }
                }
            }
        }
        for h in al_campo {
            self.hogares[h as usize].alojamiento = Alojamiento::Campo;
        }
        for m in se_quedan {
            let o = self.personas[m as usize].origen;
            self.crear_hogar(m, o, Alojamiento::SinCasa);
            if self.personas[m as usize].oficio == Oficio::Soldado && rng.f() < 0.5 {
                self.personas[m as usize].oficio = Oficio::Labrador;
            }
            // Si está en pareja, la pareja (libre) se suma a su casa.
            if let Some(c) = self.personas[m as usize].conyuge.filter(|&c| self.personas[c as usize].viva() && self.personas[c as usize].libre()) {
                let hm = self.personas[m as usize].hogar.unwrap();
                let hc = self.personas[c as usize].hogar;
                if hc.is_none_or(|x| self.hogares[x as usize].jefe != c) {
                    self.unir_a_hogar(c, hm);
                }
            }
        }
        let n = se_van.len() as u32;
        let mut hogares_que_se_van = std::collections::BTreeSet::new();
        for m in se_van {
            let p = &mut self.personas[m as usize];
            if !p.viva() {
                continue;
            }
            p.fuera = Some(mes);
            p.trabajo = None;
            p.evento(mes, TipoEvento::Emigro, NADIE);
            if let Some(h) = p.hogar {
                self.hogares[h as usize].miembros.retain(|&x| x != m);
                hogares_que_se_van.insert(h);
            }
        }
        for h in hogares_que_se_van {
            let hg = &mut self.hogares[h as usize];
            if hg.miembros.is_empty() {
                // Los que se van venden sus solares (vuelven al mercado del cabildo).
                hg.activo = false;
                for l in std::mem::take(&mut hg.solares) {
                    self.duenos.remove(&l);
                }
            } else if !hg.miembros.contains(&hg.jefe) {
                hg.jefe = hg.miembros[0];
            }
        }
        self.mes_actual.emigrados += n;
        if n >= 15 {
            noticias.push(format!("Este mes se fueron de la ciudad {n} personas"));
        }
        // La guarnición se completa con soldados nuevos.
        let tropa: u32 = self.hogares.iter().filter(|h| h.activo && h.alojamiento == Alojamiento::Cuartel).map(|h| h.miembros.len() as u32).sum();
        let meta = origenes::GUARNICION_1728 + self.habitantes() / 50;
        if tropa < meta && self.epoca <= Epoca::CiudadNueva {
            let cuartel = self.hogares.iter().find(|h| h.activo && h.alojamiento == Alojamiento::Cuartel).map(|h| h.id);
            for _ in 0..(meta - tropa).min(5) {
                let o = [Origen::Andalucia, Origen::Galicia, Origen::BuenosAires, Origen::Cantabrico][rng.range(0, 3) as usize];
                let ap = [self.apellido_al_azar(rng, o.cultura()), self.apellido_al_azar(rng, o.cultura())];
                let edad = 18 + rng.range(0, 14);
                let p = self.crear_persona(rng, Sexo::Hombre, mes - edad * 12, o, Condicion::Libre, ap, None, None);
                let pp = &mut self.personas[p as usize];
                pp.oficio = Oficio::Soldado;
                pp.estamento = Estamento::Militar;
                pp.evento(mes, TipoEvento::Llego(o), NADIE);
                match cuartel {
                    Some(c) => self.unir_a_hogar(p, c),
                    None => {
                        self.crear_hogar(p, o, Alojamiento::Cuartel);
                    }
                }
                self.mes_actual.llegados += 1;
            }
        }
    }

    // --- Población agregada (épocas tardías) ---------------------------------------------

    /// Pasa a la población agregada a las personas que superan el límite de individuos: las
    /// más recientes llegadas solas, sin familia en la ciudad (las familias siguen
    /// individuales para conservar los linajes).
    pub(crate) fn agregar_cohortes(&mut self) {
        let sobra = self.idx.vivos.len().saturating_sub(self.limite_individuos as usize);
        if sobra == 0 {
            return;
        }
        if self.cohortes.is_empty() {
            self.cohortes = vec![[0.0; 2]; 101];
        }
        let mes = self.mes;
        let mut quitados = 0;
        for i in (0..self.idx.vivos.len()).rev() {
            if quitados >= sobra {
                break;
            }
            let id = self.idx.vivos[i];
            let p = &self.personas[id as usize];
            let solo = p.hogar.is_some_and(|h| self.hogares[h as usize].miembros.len() == 1) && p.hijos.is_empty() && p.padre.is_none() && p.condicion != Condicion::Esclavizada;
            if !solo {
                continue;
            }
            let e = p.edad(mes).clamp(0, 100) as usize;
            self.cohortes[e][p.sexo as usize] += 1.0;
            let h = p.hogar.unwrap();
            self.hogares[h as usize].activo = false;
            self.hogares[h as usize].miembros.clear();
            let p = &mut self.personas[id as usize];
            p.fuera = Some(mes);
            quitados += 1;
        }
        self.reindexar();
    }

    /// Un año de la población agregada: muertes y nacimientos esperados, y un año más.
    pub(crate) fn envejecer_cohortes(&mut self) {
        if self.cohortes.is_empty() {
            return;
        }
        let mujeres_fertiles: f32 = (15..50).map(|e| self.cohortes[e][1] * fecundidad(e as i32) * 0.55).sum();
        let mut nuevo = vec![[0.0f32; 2]; 101];
        for e in 0..101 {
            let q = q_anual(e as i32);
            let sig = (e + 1).min(100);
            for s in 0..2 {
                nuevo[sig][s] += self.cohortes[e][s] * (1.0 - q);
            }
        }
        nuevo[0][0] = mujeres_fertiles * 0.512;
        nuevo[0][1] = mujeres_fertiles * 0.488;
        self.cohortes = nuevo;
    }
}

/// Oficio al llegar a los 15: casi siempre el del padre o uno parecido; la escuela y la
/// ambición abren otros caminos (la movilidad social).
fn elegir_oficio(rng: &mut Rng, p: &Persona, padre: Option<Oficio>, clase: Clase, tardia: bool) -> Oficio {
    let esclava = p.condicion == Condicion::Esclavizada;
    if esclava {
        return if p.sexo == Sexo::Mujer || rng.f() < 0.4 { Oficio::Servicio } else if rng.f() < 0.5 { Oficio::Peon } else { Oficio::Artesano };
    }
    if p.sexo == Sexo::Mujer {
        return match rng.range(0, 9) {
            0..=3 => Oficio::Servicio,
            4..=6 => Oficio::Artesano,
            7 => Oficio::Pulpero,
            _ if p.educacion >= 2 => Oficio::Maestro,
            _ => Oficio::Servicio,
        };
    }
    if let Some(o) = padre.filter(|&o| o != Oficio::Ninguno && o != Oficio::Soldado) {
        if rng.f() < 0.55 {
            return o;
        }
    }
    let letrado = p.educacion >= 2;
    let amb = p.rasgos.ambicion();
    if letrado && rng.f() < 0.25 + 0.4 * amb {
        return match rng.range(0, 9) {
            0..=3 => if tardia { Oficio::Empleado } else { Oficio::Comerciante },
            4..=5 => Oficio::Funcionario,
            6 => Oficio::Maestro,
            7 if p.rasgos.devocion() > 0.6 => Oficio::Clerigo,
            _ => if tardia { Oficio::Profesional } else { Oficio::Pulpero },
        };
    }
    match (rng.range(0, 9), clase) {
        (0..=2, _) => if tardia { Oficio::Obrero } else { Oficio::Peon },
        (3..=5, _) => Oficio::Artesano,
        (6, _) => Oficio::Marinero,
        (7, Clase::Acomodada | Clase::Principal) => Oficio::Comerciante,
        (7, _) => Oficio::Labrador,
        _ => Oficio::Soldado,
    }
}
