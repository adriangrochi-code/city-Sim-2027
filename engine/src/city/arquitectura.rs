//! La arquitectura de cada solar: de la obra (materiales reales), el hogar que la levanta
//! (clase y origen), la época y el lugar al [`Estilo`] con que se dibuja la casa (ver
//! `structures::estilo`, `structures::casas` y `docs/sistemas/arquitectura-colonial.md`).
//!
//! El contexto se toma una sola vez, al levantar la obra, y se guarda con ella: así una casa
//! no cambia de cara porque se mude otra familia, y la partida cargada dibuja lo mismo.

use super::lots::{zone_of, Density, Lot, ZoneKind, D, W};
use super::materiales::{Obra, Pared, Plan, Techo};
use super::poblacion::{Clase, Epoca, Origen};
use super::{City, START_YEAR};
use crate::buildings::Rng;
use crate::structures::estilo::{self, Contexto, Cubierta, Estilo, MaterialesObra, Muro, Tradicion, Uso};
use glam::Vec2;

/// Tradición constructiva de cada origen (los rasgos están en `structures::estilo::RASGOS`).
/// Sin estilo propio documentado: los africanos y afrodescendientes, los pueblos de la región,
/// franceses, británicos y los demás orígenes de épocas posteriores.
pub fn tradicion_de(o: Origen) -> Tradicion {
    match o {
        Origen::Montevideo | Origen::BuenosAires | Origen::Interior => Tradicion::Criolla,
        Origen::Canarias => Tradicion::Canaria,
        Origen::Galicia => Tradicion::Gallega,
        Origen::Andalucia => Tradicion::Andaluza,
        Origen::Cantabrico | Origen::PaisVasco | Origen::Cataluna => Tradicion::NorteEspana,
        Origen::Portugal | Origen::Brasil => Tradicion::Lusa,
        Origen::Misiones => Tradicion::Guarani,
        Origen::Italia => Tradicion::Italiana,
        Origen::Francia => Tradicion::Francesa,
        _ => Tradicion::Ninguna,
    }
}

/// Los materiales de una obra como los ve el estilo.
pub fn materiales_obra(o: &Obra) -> MaterialesObra {
    MaterialesObra {
        muro: match o.paredes {
            Pared::Piedra => Muro::Piedra,
            Pared::Ladrillo => Muro::Ladrillo,
            Pared::Adobe => Muro::Adobe,
            Pared::Tablas => Muro::Tablas,
            Pared::Barro => Muro::Barro,
        },
        cubierta: match o.techo {
            Techo::Teja => Cubierta::Teja,
            Techo::Azotea => Cubierta::Azotea,
            Techo::Tablas => Cubierta::Tablas,
            Techo::Paja => Cubierta::Paja,
            Techo::Cuero => Cubierta::Cuero,
        },
        revoque: o.revoque && o.paredes.admite_revoque(),
        hierro: o.rejas,
    }
}

fn pick(seed: u64) -> u64 {
    (seed >> 7) % 100
}

/// ¿El solar se dibuja con un edificio del catálogo de siempre (fonda, casa de comercio con
/// recova, depósito del puerto, talleres e industrias) en lugar de una casa con estilo?
pub fn legado(kind: ZoneKind, density: Density, level: u8, seed: u64, ctx: &Contexto) -> bool {
    match (kind, level) {
        (ZoneKind::Industrial, _) => true,
        // [epoca3] En la época 3 el comercio va en casas y edificios de renta (el almacén y bar
        // de esquina, el comercio en la planta baja); quedan los depósitos del puerto.
        (ZoneKind::Comercial, 2) if ctx.epoca >= 2 => false,
        (ZoneKind::Comercial, l) if l >= 3 && ctx.epoca >= 2 => ctx.puerto && pick(seed) < 25,
        (ZoneKind::Comercial, 2) => pick(seed) < 15,
        (ZoneKind::Comercial, l) if l >= 3 => (density == Density::Alta && pick(seed) < 50) || (ctx.puerto && pick(seed) < 40),
        _ => false,
    }
}

/// Estilo de la casa de un solar con su obra; `None` si va un edificio del catálogo.
pub fn estilo_de(kind: ZoneKind, density: Density, level: u8, seed: u64, o: &Obra) -> Option<Estilo> {
    if legado(kind, density, level, seed, &o.contexto) {
        return None;
    }
    let uso = if kind == ZoneKind::Comercial { Uso::Comercio } else { Uso::Vivienda };
    Some(estilo::estilo(&o.contexto, &materiales_obra(o), uso, level, density == Density::Alta, seed))
}

/// Color de la cal de un edificio del catálogo (fonda, taller): el de la época y la familia.
pub fn cal_de_legado(level: u8, seed: u64, o: &Obra) -> Option<crate::palette::Mat> {
    let m = materiales_obra(o);
    if !m.revoque {
        return None;
    }
    let es = estilo::estilo(&o.contexto, &MaterialesObra { cubierta: Cubierta::Teja, ..m }, Uso::Comercio, level.min(2), false, seed);
    estilo::es_cal(es.muro).then_some(es.muro)
}

impl City {
    /// Año de juego.
    pub fn anio(&self) -> u16 {
        (START_YEAR + self.month / 12) as u16
    }

    /// Barrio del solar por la distancia a la plaza: 0 centro, 1 intramuros, 2 orillas,
    /// 3 extramuros (la península amurallada medía ~1 km [E]).
    pub fn lugar_de(&self, p: Vec2) -> u8 {
        // Las manzanas del ensanche son ciudad, no extramuros (época 2).
        if self.epoca.ensanche.contiene(p) {
            return 1;
        }
        match self.lots.plaza_center.map(|c| c.distance(p)) {
            Some(d) if d < 200.0 => 0,
            Some(d) if d < 550.0 => 1,
            Some(d) if d < 850.0 => 2,
            _ => 3,
        }
    }

    /// Esquina: ¿pasa una calle junto a un costado del solar, cerca del frente?
    /// 1 del lado u = 0 del marco del edificio, 2 del otro.
    pub fn esquina_de(&self, l: &Lot) -> u8 {
        // El eje u del marco del edificio es (−f.y, f.x) (ver `voxelize::build_rotated`).
        let s = Vec2::new(-l.f.y, l.f.x);
        // Los solares de esquina quedan a unos metros de la calle lateral (los solares no
        // pisan el cruce): se busca la calzada hasta 8 m del costado, cerca del frente.
        let cerca = l.c + l.f * (D * 0.5 - 4.0);
        let mira = |lado: f32| [1.0f32, 3.0, 5.0, 8.0].iter().any(|&k| self.net.clearance(cerca + s * lado * (W * 0.5 + k), 20.0) < 0.5);
        if mira(-1.0) {
            1
        } else if mira(1.0) {
            2
        } else {
            0
        }
    }

    /// Contexto de un solar al levantar su obra: año, hogar principal (clase y origen; si no
    /// vive nadie, la clase probable y un origen sorteado con el peso de cada origen en la
    /// época), esquina, barrio y puerto. La tirada contra la repetición se elige aparte.
    pub fn contexto_solar(&self, id: u32, level: u8) -> Contexto {
        let Some(l) = self.lots.lots.get(&id) else { return Contexto::default() };
        let anio = self.anio();
        let hogar = if self.poblacion.fundada {
            self.poblacion.hogares_de_lote(id).filter(|h| h.activo && !h.miembros.is_empty()).max_by(|a, b| (a.clase as u8, a.ingresos).partial_cmp(&(b.clase as u8, b.ingresos)).unwrap_or(std::cmp::Ordering::Equal))
        } else {
            None
        };
        let valor = self.lots.valor.get(&id).map_or(0.5, |v| v.valor);
        let (clase, origen) = match hogar {
            Some(h) => (Clase::ALL.iter().position(|&c| c == h.clase).unwrap_or(1) as u8, Some(h.origen)),
            None => (estilo::clase_probable(level, valor, l.seed), None),
        };
        let origen = origen.unwrap_or_else(|| origen_probable_en(self.epoca.actual, anio, l.seed));
        let puerto = self.services.port.pier.as_ref().is_some_and(|p| p.land.distance(l.c) < 350.0);
        // Lo que se levanta desde la Ciudad Nueva usa el kit de la época 2.
        let epoca = u8::from(self.en_epoca(Epoca::CiudadNueva)) + u8::from(self.en_epoca(Epoca::Inmigracion)); // [epoca3]
        Contexto { anio, clase, esquina: self.esquina_de(l), lugar: self.lugar_de(l.c), puerto, giro: 0, tradicion: tradicion_de(origen).indice(), epoca, reforma: 0, antes: 0, conventillo: 0 }
    }

    /// Estilos de las casas vecinas sobre la misma calle (a un solar de distancia).
    fn estilos_vecinos(&self, id: u32) -> Vec<Estilo> {
        let Some(l) = self.lots.lots.get(&id) else { return Vec::new() };
        let s = Vec2::new(-l.f.y, l.f.x);
        self.lots
            .nearby(l.c)
            .into_iter()
            .filter(|&o| o != id)
            .filter_map(|o| self.lots.lots.get(&o))
            .filter(|o| o.level > 0 && o.f.dot(l.f) > 0.9 && (o.c - l.c).dot(l.f).abs() < 4.0 && ((o.c - l.c).dot(s).abs() - W).abs() < 3.0)
            .filter_map(|o| {
                let (k, d) = zone_of(o.zone)?;
                estilo_de(k, d, o.level, o.seed, o.obra.as_ref()?)
            })
            .collect()
    }

    /// Levanta la obra de un solar (con lo que hay) y le pone su contexto, con la tirada que
    /// respeta la regla anti-monotonía con los vecinos (guía §6.6).
    pub fn obra_de_solar(&mut self, id: u32, level: u8, plan: &Plan) -> Obra {
        let mut o = self.construir_obra(plan);
        self.estampar_contexto(id, level, &mut o);
        o
    }

    /// Pone el contexto (y la tirada anti-monotonía) a una obra recién levantada.
    pub fn estampar_contexto(&self, id: u32, level: u8, o: &mut Obra) {
        o.contexto = self.contexto_solar(id, level);
        self.elegir_giro(id, level, o);
    }

    /// Elige la tirada del estilo que no repite a los vecinos ya edificados: no comparten más
    /// de 2 de los 4 atributos (tipología, color, vanos, puerta) y la cornisa difiere.
    pub fn elegir_giro(&self, id: u32, level: u8, o: &mut Obra) {
        let Some(l) = self.lots.lots.get(&id) else { return };
        let Some((k, d)) = zone_of(l.zone) else { return };
        o.contexto.giro = 0;
        let vecinos = self.estilos_vecinos(id);
        if vecinos.is_empty() {
            return;
        }
        let mut mejor = (u32::MAX, 0u8);
        for giro in 0..12u8 {
            o.contexto.giro = giro;
            let Some(es) = estilo_de(k, d, level, l.seed, o) else { return };
            let malos: u32 = vecinos.iter().map(|v| u32::from(!es.distinto_de(v))).sum();
            if malos < mejor.0 {
                mejor = (malos, giro);
            }
            if malos == 0 {
                break;
            }
        }
        o.contexto.giro = mejor.1;
    }

    /// Plan de lo que se construye en un solar, con el techo que prefiere el dueño según la
    /// época, el barrio y su tradición (la azotea llega a fines del XVIII; el cuero de los
    /// primeros ranchos se va con la paja) — sigue siendo el sistema de materiales el que
    /// decide con lo que hay.
    pub fn plan_con_preferencias(&self, l: &Lot, level: u8, mut plan: Plan) -> Plan {
        let Some((kind, density)) = zone_of(l.zone) else { return plan };
        let ctx = self.contexto_solar(l.id, level);
        let t = Tradicion::de_indice(ctx.tradicion);
        let x = Rng::new(l.seed ^ 0x7EC40).f();
        let primero = if ctx.epoca >= 1 {
            // Ciudad Nueva: azotea casi siempre (la teja queda para las quintas y las orillas) [E].
            match (kind, level) {
                (ZoneKind::Residencial | ZoneKind::Comercial, 2..) => Some(if x < if ctx.lugar >= 3 { 0.55 } else { 0.9 } { Techo::Azotea } else { Techo::Teja }),
                _ => None,
            }
        } else {
            None
        };
        let primero = if primero.is_some() { primero } else { match (kind, level) {
            (ZoneKind::Residencial, 1) if plan.techos.contains(&Techo::Paja) => Some(if x < estilo::prob_cuero(ctx.anio) { Techo::Cuero } else { Techo::Paja }),
            (ZoneKind::Residencial, 2) if density == Density::Alta => None,
            (ZoneKind::Residencial | ZoneKind::Comercial, 2) | (ZoneKind::Comercial, 1) => {
                Some(if x < estilo::prob_azotea(ctx.anio, ctx.lugar, t) { Techo::Azotea } else { Techo::Teja })
            }
            (ZoneKind::Residencial, _) if ctx.anio < 1780 => Some(if x < (estilo::prob_azotea(ctx.anio, ctx.lugar, t) * 1.5).min(0.9) { Techo::Azotea } else { Techo::Teja }),
            _ => None,
        } };
        if let Some(p) = primero {
            if let Some(i) = plan.techos.iter().position(|&x| x == p) {
                let v = plan.techos.remove(i);
                plan.techos.insert(0, v);
            }
        }
        plan
    }

    /// Estilo con que se dibuja la casa de un solar (`None` si es un edificio del catálogo).
    pub fn estilo_de_solar(&self, id: u32) -> Option<Estilo> {
        let l = self.lots.lots.get(&id)?;
        let (k, d) = zone_of(l.zone)?;
        estilo_de(k, d, l.level, l.seed, &self.obra_de(l)?)
    }

    /// Líneas de la ficha sobre la casa: su tipo, el año y el origen de la familia.
    pub fn lineas_estilo(&self, l: &Lot) -> Vec<String> {
        let Some((k, d)) = zone_of(l.zone) else { return Vec::new() };
        let Some(o) = self.obra_de(l) else { return Vec::new() };
        let Some(es) = estilo_de(k, d, l.level, l.seed, &o) else { return Vec::new() };
        let mut v = Vec::new();
        let nombre = es.nombre();
        let mut cap = nombre.chars();
        let nombre = cap.next().map(|c| c.to_uppercase().collect::<String>() + cap.as_str()).unwrap_or_default();
        if o.contexto.anio > 0 {
            v.push(format!("{nombre}, levantada en {}", o.contexto.anio));
        } else {
            v.push(nombre);
        }
        if es.reforma > 0 {
            v.push(format!("Fachada modernizada en {}: misma planta, cara de la Ciudad Nueva", es.reforma));
        }
        if let Some(t) = es.linea_origen() {
            v.push(t);
        }
        v
    }
}

/// Origen más probable de quien levanta una casa en ese año, sorteado (con la semilla del
/// solar) con el peso de cada origen en las llegadas de la época.
pub fn origen_probable(anio: u16, seed: u64) -> Origen {
    origen_probable_en(Epoca::del_año(anio as i32), anio, seed)
}

/// Como [`origen_probable`], con la tabla de llegadas de una época dada (la de la ciudad).
pub fn origen_probable_en(epoca: Epoca, anio: u16, seed: u64) -> Origen {
    let tabla = super::poblacion::origenes::tabla(epoca);
    let total: f32 = tabla.iter().map(|l| l.peso).sum::<f32>() + 20.0;
    // Los nacidos en la ciudad pesan cada vez más.
    let criollos = 20.0 + (anio.saturating_sub(1726) as f32) * 0.3;
    let mut x = Rng::new(seed ^ 0x0216E).f() * (total - 20.0 + criollos);
    if x < criollos {
        return Origen::Montevideo;
    }
    x -= criollos;
    for l in tabla {
        if x < l.peso {
            return l.origen;
        }
        x -= l.peso;
    }
    Origen::Montevideo
}
