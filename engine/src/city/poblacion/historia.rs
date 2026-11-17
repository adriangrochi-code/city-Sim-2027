//! Hechos históricos de la esclavitud, de la libertad y de los pueblos originarios en
//! Montevideo, con sus fechas y su contexto. En el modo fiel a la historia ocurren en sus
//! fechas; en la historia alternativa, los que tienen sentido fuera de su fecha (cofradías,
//! salas de nación, el caserío de la cuarentena) llegan cuando la ciudad los hace posibles.
//!
//! Fuentes verificadas (detalle en `docs/sistemas/esclavitud-y-pueblos-originarios.md`):
//! Frega, Duffau, Chagas y Stalla (coords.), *Historia de la población africana y
//! afrodescendiente en Uruguay* (FHCE-MIDES, 2020), en particular la tabla "Disposiciones
//! legales sobre tráfico de esclavizados y proceso de abolición (1812-1862)" (pp. 136-137);
//! Borucki, "¿Es posible integrar la esclavitud al relato de la historia económica uruguaya
//! previa a 1860?", Boletín AUDHE n.º 4 (2005); Pollero y González Demuro, *Población y
//! sociedad de Montevideo* (2024).
//!
//! Representación cuidadosa: nada de esto es una decisión del jugador ni mueve dinero hacia
//! nadie por tener personas sometidas. Son restricciones y hechos del mundo que la crónica
//! cuenta con respeto.

use super::*;
use crate::city::economy::Ordinance;

/// Mes absoluto (meses desde enero de 1726) de un año y un mes (1 a 12).
pub const fn mes_de(año: i32, mes: i32) -> i32 {
    (año - 1726) * 12 + mes - 1
}

// --- Fechas (modo fiel a la historia) ----------------------------------------------------

/// Cofradía de San Benito de Palermo (africanos y afrodescendientes), "desde 1773", en el
/// convento de San Bernardino (Frega et al., 2020, cap. 7). Mes: estimación.
pub const COFRADIA_SAN_BENITO: i32 = mes_de(1773, 1);
/// Cofradía de San Baltasar, en la Iglesia Matriz "desde 1787". Mes: estimación.
pub const COFRADIA_SAN_BALTASAR: i32 = mes_de(1787, 1);
/// Primeros arribos de la Real Compañía de Filipinas a Montevideo (1788) y uso del "Caserío de
/// Negros" para la cuarentena (1788–1810). Mes: estimación.
pub const CASERIO: i32 = mes_de(1788, 1);
/// Salas de nación: "desde fines del siglo XVIII". Año: estimación.
pub const SALAS_DE_NACION: i32 = mes_de(1790, 1);
/// Ante la invasión portuguesa, padrón y leva de esclavizados en Montevideo (1816).
/// Mes: estimación (la invasión empezó en agosto de 1816).
pub const LEVA_1816: i32 = mes_de(1816, 9);
/// Ley de la Sala de Representantes en la Florida: nadie nacerá esclavo y se prohíbe el
/// tráfico "de país extranjero" (7 de setiembre de 1825). No rigió en Montevideo, en manos del
/// Imperio del Brasil.
pub const LEY_FLORIDA: i32 = mes_de(1825, 9);
/// La Asamblea General Constituyente extiende a todo el territorio la ley de 1825 (20 de enero
/// de 1830): desde aquí rige en Montevideo.
pub const VIENTRES_MONTEVIDEO: i32 = mes_de(1830, 1);
/// Jura de la Constitución (18 de julio de 1830), art. 131.
pub const CONSTITUCION: i32 = mes_de(1830, 7);
/// Llegada a Montevideo de los prisioneros de Salsipuedes (matanza del 11 de abril de 1831;
/// orden de reparto de los menores del 29 de abril). Mes: estimación.
pub const SALSIPUEDES: i32 = mes_de(1831, 5);
/// Primer contrato para la introducción de "colonos" africanos (12 de noviembre de 1832).
pub const COLONOS_DESDE: i32 = mes_de(1832, 11);
/// El gobierno de Oribe prohíbe la introducción de colonos (marzo de 1835).
pub const COLONOS_HASTA: i32 = mes_de(1835, 3);
/// Ley de reglamentación del tráfico (14 de junio de 1837): los introducidos son libres, bajo
/// patronato (tres años los mayores; hasta los 25 años los menores).
pub const LEY_1837: i32 = mes_de(1837, 6);
/// Ley de abolición del Gobierno de la Defensa, en Montevideo (12 de diciembre de 1842).
pub const ABOLICION_DEFENSA: i32 = mes_de(1842, 12);
/// Ley de abolición del Gobierno del Cerrito (28 de octubre de 1846), en el territorio que
/// controlaba (no en Montevideo, Maldonado ni Colonia).
pub const ABOLICION_CERRITO: i32 = mes_de(1846, 10);
/// Ley que elimina el patronato sobre los hijos de los emancipados (2 de mayo de 1853).
pub const FIN_PATRONATO: i32 = mes_de(1853, 5);

/// Años de patronato de los mayores de 25 (reglamento de 1837, aplicado tras la abolición).
pub const PATRONATO_ADULTOS: i32 = 3;
/// Edad hasta la que servían los menores bajo patronato.
pub const PATRONATO_MENORES: i32 = 25;

// --- Hitos de una sola vez ----------------------------------------------------------------

pub const HITO_COFRADIAS: u32 = 1 << 0;
pub const HITO_SAN_BALTASAR: u32 = 1 << 1;
pub const HITO_CASERIO: u32 = 1 << 2;
pub const HITO_SALAS: u32 = 1 << 3;
pub const HITO_LEVA_1816: u32 = 1 << 4;
pub const HITO_LEY_FLORIDA: u32 = 1 << 5;
pub const HITO_CONSTITUCION: u32 = 1 << 6;
pub const HITO_SALSIPUEDES: u32 = 1 << 7;
pub const HITO_COLONOS: u32 = 1 << 8;
pub const HITO_FIN_COLONOS: u32 = 1 << 9;
pub const HITO_LEY_1837: u32 = 1 << 10;
pub const HITO_CERRITO: u32 = 1 << 11;
pub const HITO_FIN_PATRONATO: u32 = 1 << 12;

/// Ascendencia de cada persona registrada (por id): bit 1 africana, bit 2 indígena. Se sigue
/// por padre y madre (los padres siempre tienen un id menor que sus hijos).
pub const ASC_AFRICANA: u8 = 1;
pub const ASC_INDIGENA: u8 = 2;

impl Poblacion {
    pub(crate) fn hito(&self, h: u32) -> bool {
        self.hitos & h != 0
    }

    /// Ascendencia (bits `ASC_*`) de todas las personas registradas, en una pasada.
    pub fn ascendencias(&self) -> Vec<u8> {
        let mut v = vec![0u8; self.personas.len()];
        for p in &self.personas {
            let mut a = match p.origen.raiz() {
                Raiz::Africana => ASC_AFRICANA,
                Raiz::Indigena => ASC_INDIGENA,
                _ => 0,
            };
            a |= match p.raiz {
                Raiz::Africana => ASC_AFRICANA,
                Raiz::Indigena => ASC_INDIGENA,
                _ => 0,
            };
            for x in [p.padre, p.madre].into_iter().flatten() {
                if (x as usize) < v.len() {
                    a |= v[x as usize];
                }
            }
            v[p.id as usize] = a;
        }
        v
    }

    /// ¿Tiene ascendencia africana? (sigue a padres y abuelos hasta seis generaciones).
    pub fn afrodescendiente(&self, id: PersonaId) -> bool {
        let mut pila = vec![(id, 0u8)];
        while let Some((x, g)) = pila.pop() {
            let Some(p) = self.persona(x) else { continue };
            if p.origen.raiz() == Raiz::Africana || p.raiz == Raiz::Africana {
                return true;
            }
            if g < 6 {
                pila.extend([p.padre, p.madre].into_iter().flatten().map(|a| (a, g + 1)));
            }
        }
        false
    }

    /// ¿Existen ya las cofradías de africanos y afrodescendientes? (sociabilidad, ayuda mutua y
    /// colectas para la libertad).
    pub fn hay_cofradias(&self) -> bool {
        self.hito(HITO_COFRADIAS)
    }

    /// Hogares que reciben a quienes llegan sometidos (trata, colonos, prisioneros): los de la
    /// ciudad con más recursos, como en la época. Peso: raíz del ahorro.
    pub(crate) fn hogares_receptores(&self) -> Vec<(HogarId, f32)> {
        let mut ricos: Vec<(HogarId, f32)> = self.hogares.iter().filter(|h| h.activo && h.alojamiento != Alojamiento::Cuartel && h.alojamiento != Alojamiento::Campamento && h.clase >= Clase::Acomodada).map(|h| (h.id, h.ahorro.max(1.0).sqrt())).collect();
        if ricos.is_empty() {
            ricos = self.hogares.iter().filter(|h| h.activo && h.alojamiento == Alojamiento::Solar).map(|h| (h.id, 1.0)).collect();
        }
        ricos
    }

    pub(crate) fn elegir_receptor(rng: &mut Rng, ricos: &[(HogarId, f32)]) -> HogarId {
        let total: f32 = ricos.iter().map(|x| x.1).sum();
        let mut r = rng.f() * total;
        for &(h, w) in ricos {
            if r < w {
                return h;
            }
            r -= w;
        }
        ricos[0].0
    }

    /// El cuartel de la guarnición (o uno nuevo con esta persona).
    fn cuartel_para(&mut self, p: PersonaId) -> HogarId {
        match self.hogares.iter().find(|h| h.activo && h.alojamiento == Alojamiento::Cuartel).map(|h| h.id) {
            Some(h) => h,
            None => {
                let o = self.personas[p as usize].origen;
                self.crear_hogar(p, o, Alojamiento::Cuartel)
            }
        }
    }

    /// Libertad por alistarse: pasa a ser soldado y vive en el cuartel.
    pub(crate) fn alistar(&mut self, p: PersonaId, noticias: &mut Vec<String>) {
        if self.personas[p as usize].condicion != Condicion::Esclavizada {
            return;
        }
        self.liberar_con(p, ViaLibertad::Armas, noticias, None);
        let h = self.cuartel_para(p);
        self.unir_a_hogar(p, h);
        let pp = &mut self.personas[p as usize];
        pp.oficio = Oficio::Soldado;
        pp.estamento = Estamento::Militar;
        pp.trabajo = None;
    }

    /// Termina el patronato de una persona: si es adulta, arma su hogar o se reúne con su
    /// familia (como al obtener la libertad).
    pub(crate) fn terminar_patronato(&mut self, p: PersonaId) {
        let mes = self.mes;
        if self.personas[p as usize].patronato.take().is_none() {
            return;
        }
        self.personas[p as usize].evento(mes, TipoEvento::FinPatronato, NADIE);
        if self.personas[p as usize].edad(mes) >= 16 {
            let antes = self.personas[p as usize].hogar;
            self.armar_hogar_propio(p, antes);
        }
    }

    /// Los patronatos que vencen este mes (o todos, por ley).
    fn vencer_patronatos(&mut self, todos: bool) -> usize {
        let mes = self.mes;
        let ids: Vec<PersonaId> = self.idx.vivos.iter().copied().filter(|&p| self.personas[p as usize].patronato.is_some_and(|m| todos || m <= mes)).collect();
        let n = ids.len();
        for p in ids {
            self.terminar_patronato(p);
        }
        n
    }

    /// Personas que llegan como "colonos" (1832–1835): niñas, niños y jóvenes de hasta 16 años
    /// (los varones no más del 60 %), obligados a servir doce años desde los 12 en las casas
    /// que los reciben. Una forma de burlar la prohibición del tráfico: nadie paga ni cobra en
    /// el modelo.
    pub(crate) fn llegada_colonos(&mut self, rng: &mut Rng, mes: i32, n: u32) -> u32 {
        let ricos = self.hogares_receptores();
        if ricos.is_empty() {
            return 0;
        }
        for _ in 0..n {
            let destino = Self::elegir_receptor(rng, &ricos);
            let sexo = if rng.f() < 0.58 { Sexo::Hombre } else { Sexo::Mujer };
            let edad = 8 + rng.range(0, 8);
            let o = if rng.f() < 0.75 { Origen::AfricaCentral } else { Origen::AfricaOriental };
            let nac = mes - edad * 12 - rng.range(0, 11);
            let p = self.crear_persona(rng, sexo, nac, o, Condicion::Liberta, [NADIE, NADIE], None, None);
            let pp = &mut self.personas[p as usize];
            pp.evento(mes, TipoEvento::LlegoColono(o), NADIE);
            pp.patronato = Some(nac + 24 * 12);
            pp.evento(mes, TipoEvento::Patronato, NADIE);
            pp.oficio = if edad < 12 { Oficio::Ninguno } else if sexo == Sexo::Mujer || rng.f() < 0.5 { Oficio::Servicio } else { Oficio::Peon };
            pp.salud = 0.45 + rng.f() * 0.3;
            self.unir_a_hogar(p, destino);
        }
        n
    }

    /// Los prisioneros de Salsipuedes: mujeres, niñas y niños charrúas repartidos entre
    /// familias de la ciudad como sirvientes; muchos niños, separados de sus madres.
    fn llegada_prisioneros(&mut self, rng: &mut Rng, mes: i32, n: u32) -> u32 {
        let ricos = self.hogares_receptores();
        if ricos.is_empty() {
            return 0;
        }
        let mut usados: Vec<HogarId> = Vec::new();
        for _ in 0..n {
            // Cada persona a una casa distinta (si se puede): así los repartieron.
            let mut destino = Self::elegir_receptor(rng, &ricos);
            for _ in 0..6 {
                if !usados.contains(&destino) {
                    break;
                }
                destino = Self::elegir_receptor(rng, &ricos);
            }
            usados.push(destino);
            let niño = rng.f() < 0.6;
            let sexo = if !niño || rng.f() < 0.5 { Sexo::Mujer } else { Sexo::Hombre };
            let edad = if niño { rng.range(0, 11) } else { 15 + rng.range(0, 25) };
            let nac = mes - edad * 12 - rng.range(0, 11);
            // Bautizados en la Matriz con nombre y apellido castellanos.
            let ap = [self.apellido_al_azar(rng, Cultura::Hispana), NADIE];
            let p = self.crear_persona(rng, sexo, nac, Origen::PueblosRegion, Condicion::Libre, ap, None, None);
            let pp = &mut self.personas[p as usize];
            pp.evento(mes, TipoEvento::LlegoPrisionera, NADIE);
            pp.patronato = Some(if edad < PATRONATO_MENORES { nac + PATRONATO_MENORES * 12 } else { mes + PATRONATO_ADULTOS * 12 });
            pp.evento(mes, TipoEvento::Patronato, NADIE);
            pp.oficio = if edad < 12 { Oficio::Ninguno } else { Oficio::Servicio };
            pp.salud = 0.4 + rng.f() * 0.3;
            self.unir_a_hogar(p, destino);
        }
        n
    }

    fn contar(&mut self, mes: i32, t: String, noticias: &mut Vec<String>) {
        noticias.push(t.clone());
        self.anotar(mes, t);
    }

    /// Los hechos históricos del mes (después de las leyes de libertad).
    pub(crate) fn hechos_historicos(&mut self, rng: &mut Rng, ent: &Entorno, noticias: &mut Vec<String>) {
        let mes = ent.mes;
        // Los patronatos que vencen.
        self.vencer_patronatos(false);
        let fiel = self.fiel_historia;
        let colonial = self.epoca <= Epoca::CiudadNueva;
        // Cofradías, salas de nación y el caserío de la cuarentena: en sus fechas o, en la
        // historia alternativa, cuando la ciudad tiene una comunidad africana numerosa (se
        // cuenta una vez por año).
        let (mut afro_adultos, mut africanos) = (u32::MAX, u32::MAX);
        if !fiel && colonial && mes % 12 == 0 && self.hitos & (HITO_COFRADIAS | HITO_SALAS | HITO_CASERIO) != (HITO_COFRADIAS | HITO_SALAS | HITO_CASERIO) {
            let asc = self.ascendencias();
            afro_adultos = self.vivos().filter(|p| asc[p.id as usize] & ASC_AFRICANA != 0 && p.edad(mes) >= 16).count() as u32;
            africanos = self.personas.iter().filter(|p| p.origen.raiz() == Raiz::Africana).count() as u32;
        }
        let toca = |fecha: i32, umbral: u32, cuenta: u32| if fiel { mes >= fecha } else { colonial && cuenta != u32::MAX && cuenta >= umbral };
        if !self.hito(HITO_COFRADIAS) && toca(COFRADIA_SAN_BENITO, 40, afro_adultos) {
            self.hitos |= HITO_COFRADIAS;
            let t = if fiel {
                "Africanos y afrodescendientes, libres y esclavizados, fundan la cofradía de San Benito de Palermo en el convento de San Bernardino: se cuidan en la enfermedad, se acompañan en la muerte y juntan limosnas, a veces para comprar una libertad"
            } else {
                "Africanos y afrodescendientes, libres y esclavizados, fundan su cofradía: se cuidan en la enfermedad, se acompañan en la muerte y juntan limosnas, a veces para comprar una libertad"
            };
            self.contar(mes, t.into(), noticias);
        }
        if fiel && !self.hito(HITO_SAN_BALTASAR) && mes >= COFRADIA_SAN_BALTASAR {
            self.hitos |= HITO_SAN_BALTASAR;
            self.anotar(mes, "Se funda la cofradía de San Baltasar en la Iglesia Matriz".into());
        }
        if !self.hito(HITO_CASERIO) && toca(CASERIO, 300, africanos) {
            self.hitos |= HITO_CASERIO;
            let t = if fiel {
                "La Real Compañía de Filipinas levanta junto al arroyo Miguelete el «Caserío de los Negros»: allí desembarcan y pasan la cuarentena las personas traídas por la trata. Muchas mueren de disentería días después de llegar; la mayoría sigue viaje a Buenos Aires, Chile, el Alto Perú y Lima"
            } else {
                "Junto a un arroyo de la bahía se levanta un caserío para la cuarentena de las personas traídas por la trata. Muchas mueren de disentería días después de llegar; la mayoría sigue viaje a otros puertos"
            };
            self.contar(mes, t.into(), noticias);
        }
        if !self.hito(HITO_SALAS) && toca(SALAS_DE_NACION, 80, afro_adultos) {
            self.hitos |= HITO_SALAS;
            self.contar(mes, "Las naciones africanas —Congo, Angola, Benguela, Mina, Mozambique…— abren sus salas: allí se baila, se canta y se ayudan entre sí, y cada 6 de enero, día de San Baltasar, eligen a sus reyes".into(), noticias);
        }
        // Día de Reyes en la crónica (sin noticia).
        if self.hito(HITO_SALAS) && colonial && mes % 12 == 0 {
            self.anotar(mes, "Día de Reyes: las naciones africanas coronan a sus reyes y reinas, van en comitiva a misa a la Matriz y, de vuelta en sus salas, empiezan los candombes".into());
        }
        if !fiel {
            return;
        }
        // Padrón y leva de 1816, ante la invasión portuguesa.
        if !self.hito(HITO_LEVA_1816) && mes >= LEVA_1816 {
            self.hitos |= HITO_LEVA_1816;
            let hombres: Vec<PersonaId> = self.idx.vivos.iter().copied().filter(|&p| {
                let q = &self.personas[p as usize];
                q.condicion == Condicion::Esclavizada && q.sexo == Sexo::Hombre && (16..=40).contains(&q.edad(mes))
            }).collect();
            let mut n = 0;
            let mut basura = Vec::new();
            for p in hombres {
                // Muchos amos escondieron o sacaron de la ciudad a los suyos para evitar la leva.
                if rng.f() < 0.2 {
                    self.alistar(p, &mut basura);
                    n += 1;
                }
            }
            if n > 0 {
                self.contar(mes, format!("Ante la invasión portuguesa, el gobierno artiguista empadrona a las personas esclavizadas de la ciudad y alista a {n} varones: quienes pasan a las armas obtienen la libertad. Muchos amos esconden a los suyos para evitar la leva"), noticias);
            }
            self.reindexar();
        }
        if !self.hito(HITO_LEY_FLORIDA) && mes >= LEY_FLORIDA {
            self.hitos |= HITO_LEY_FLORIDA;
            self.contar(mes, "En la Florida, la Sala de Representantes de la Provincia Oriental declara que nadie nacerá esclavo y prohíbe el tráfico de esclavizados (7 de setiembre de 1825). Montevideo sigue en manos del Imperio del Brasil: en la ciudad la ley no rige".into(), noticias);
        }
        if !self.hito(HITO_CONSTITUCION) && mes >= CONSTITUCION {
            self.hitos |= HITO_CONSTITUCION;
            self.contar(mes, "Se jura la Constitución: «En el territorio del Estado nadie nacerá ya esclavo; queda prohibido para siempre su tráfico e introducción en la República» (art. 131). La esclavitud de quienes ya lo eran sigue siendo legal".into(), noticias);
        }
        if !self.hito(HITO_SALSIPUEDES) && mes >= SALSIPUEDES {
            self.hitos |= HITO_SALSIPUEDES;
            if mes < SALSIPUEDES + 12 {
                // Unos 150 llegaron a una ciudad de unos 14.000 a 20.000 habitantes: la misma
                // proporción en esta ciudad (estimación).
                let n = ((self.habitantes() as f32 * 0.008) as u32).clamp(3, 150);
                let n = self.llegada_prisioneros(rng, mes, n);
                self.mes_actual.llegados += n;
                self.mes_actual.llegados_forzados += n;
                self.contar(mes, format!("Tras la matanza de Salsipuedes (11 de abril de 1831), en la que el ejército del Estado atacó a los charrúas, llegan a pie a la ciudad {n} prisioneras y prisioneros, casi todos mujeres, niñas y niños. El gobierno los reparte entre familias como sirvientes y separa a muchos niños de sus madres"), noticias);
                self.reindexar();
            }
        }
        // Los "colonos" africanos (1832–1835): la trata disfrazada de contrato.
        if (COLONOS_DESDE..COLONOS_HASTA).contains(&mes) && !ent.vigente(Ordinance::Abolicion) {
            if !self.hito(HITO_COLONOS) {
                self.hitos |= HITO_COLONOS;
                self.contar(mes, "El gobierno firma el primer contrato para traer «colonos» africanos: niñas, niños y jóvenes comprados en África, que deberán servir doce años en las casas que paguen su contrato. Es la trata con otro nombre".into(), noticias);
            }
            // Al menos mil en Montevideo y Maldonado en algo más de dos años, para una ciudad de
            // unos 20.000 habitantes (Borucki): la misma proporción por mes (estimación).
            let esperado = self.habitantes() as f32 * 0.0015;
            let n = (esperado * (0.5 + rng.f())).round() as u32;
            if n > 0 {
                let n = self.llegada_colonos(rng, mes, n);
                self.mes_actual.llegados += n;
                self.mes_actual.llegados_forzados += n;
                if n >= 2 {
                    self.anotar(mes, format!("Llegan en un barco {n} niñas, niños y jóvenes africanos como «colonos»: libres en el papel, obligados a servir en las casas que los reciben"));
                }
            }
        }
        if !self.hito(HITO_FIN_COLONOS) && mes >= COLONOS_HASTA {
            self.hitos |= HITO_FIN_COLONOS;
            self.contar(mes, "El gobierno de Manuel Oribe prohíbe la introducción de colonos africanos. El contrabando de personas sigue, en pequeña escala, por el puerto y por la frontera con el Brasil".into(), noticias);
        }
        if !self.hito(HITO_LEY_1837) && mes >= LEY_1837 {
            self.hitos |= HITO_LEY_1837;
            self.contar(mes, "Una ley reglamenta la prohibición del tráfico: quien sea introducido como esclavo o como colono será libre, pero bajo patronato, sirviendo en una casa a cambio de techo, comida y ropa: tres años los mayores, hasta los 25 años los menores".into(), noticias);
        }
        if !self.hito(HITO_CERRITO) && mes >= ABOLICION_CERRITO {
            self.hitos |= HITO_CERRITO;
            self.contar(mes, "En el Cerrito, el gobierno de Manuel Oribe decreta la abolición de la esclavitud en el territorio que controla y crea comisiones que clasifican a los liberados para el ejército. Montevideo, sitiada, la había declarado en 1842".into(), noticias);
        }
        if !self.hito(HITO_FIN_PATRONATO) && mes >= FIN_PATRONATO {
            self.hitos |= HITO_FIN_PATRONATO;
            let n = self.vencer_patronatos(true);
            self.contar(mes, format!("Una ley termina con el patronato sobre los hijos de quienes fueron esclavizados: vuelven con sus familias ({n} personas en la ciudad). A los huérfanos y a los hijos de familias pobres los alcanza el «pupilaje», que en muchas casas sigue como servicio sin paga"), noticias);
            self.reindexar();
        }
    }

    /// Abolición del Gobierno de la Defensa (12 de diciembre de 1842), en plena guerra: los
    /// varones aptos pasan al ejército; las mujeres, los menores y los no aptos quedan bajo el
    /// patronato de sus antiguos amos (los menores hasta los 25 años; los demás, tres años más).
    pub(crate) fn abolicion_de_1842(&mut self, noticias: &mut Vec<String>) -> (usize, usize) {
        let mes = self.mes;
        let todas: Vec<PersonaId> = self.idx.vivos.iter().copied().filter(|&p| self.personas[p as usize].condicion == Condicion::Esclavizada).collect();
        let (mut armas, mut patronato) = (0, 0);
        let mut basura = Vec::new();
        for p in todas {
            let q = &self.personas[p as usize];
            let edad = q.edad(mes);
            if q.sexo == Sexo::Hombre && (15..=45).contains(&edad) && q.salud >= 0.35 {
                self.alistar(p, &mut basura);
                armas += 1;
            } else {
                let hasta = if edad < PATRONATO_MENORES { q.nacimiento + PATRONATO_MENORES * 12 } else { mes + PATRONATO_ADULTOS * 12 };
                self.liberar_con(p, ViaLibertad::Abolicion, &mut basura, Some(hasta));
                patronato += 1;
            }
        }
        let t = format!("Ley de abolición del Gobierno de la Defensa: no hay más personas esclavizadas en la República. En plena guerra, {armas} varones aptos pasan al ejército; {patronato} mujeres, menores y mayores quedan bajo patronato de sus antiguos amos, los menores hasta los 25 años y los demás tres años más");
        noticias.push(t.clone());
        self.anotar(mes, t);
        (armas, patronato)
    }
}
