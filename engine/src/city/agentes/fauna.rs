//! Animales y detalles fijos de la ciudad y el campo: el rodeo de cada estancia en su
//! pastura, la yunta de la chacra, los caballos atados al palenque de las pulperías, perros en
//! las calles, gallinas en el fondo de los ranchos, botes amarrados al muelle, carretas paradas
//! y letreros de pulpería.
//!
//! No se guarda nada: todo se deriva de la ciudad (solares, servicios, clima, año) y del
//! reloj. Cada animal se mueve con una fórmula determinista: en cada tramo de tiempo camina
//! de un punto de su área al siguiente (puntos al azar con su semilla) y después pasta, así
//! nunca sale del área (un disco: la recta entre dos puntos queda adentro) y no hace falta
//! simularlo ni guardarlo. El tamaño del rodeo sale de lo mismo que la producción de la
//! estancia (rinde del sitio × pasturas del clima).

use super::super::lots::{side_of, ZoneKind, D as LOT_D, W as LOT_W};
use super::super::services::Kind;
use super::super::City;
use crate::detalle::ropa::mezcla;
use crate::detalle::{anim, biblioteca, emitir, modelos, sombra, Caja, Detalles, Id};
use crate::lod::BASE_VOXEL;
use crate::terrain::TerrainGen;
use glam::{Vec2, Vec3};

/// Hasta dónde se dibujan los animales (m).
pub const RADIO_FAUNA: f32 = 160.0;

/// Qué hace un grupo de animales.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    /// Camina un poco y pasta (vacunos, ovejas, caballos sueltos).
    Pasta,
    /// Atado: parado en su lugar, mira alrededor (caballos en el palenque).
    Atado,
    /// Anda de acá para allá (perros).
    Deambula,
    /// Picotea (gallinas).
    Picotea,
}

/// Un grupo de animales de una especie en un área (disco).
#[derive(Clone, Debug, PartialEq)]
pub struct Rebano {
    pub especie: Id,
    /// Otras especies mezcladas (por ejemplo, vacas overas y bueyes en el rodeo).
    pub mezcla: &'static [Id],
    pub centro: Vec2,
    pub radio: f32,
    pub n: u32,
    pub semilla: u32,
    pub modo: Modo,
    /// Rumbo fijo de los atados (rad).
    pub giro: f32,
}

/// Qué está haciendo un animal en un momento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accion {
    /// Caminando: metros recorridos en este tramo (para la fase de las patas).
    Camina(f32),
    Pasta,
    Quieto,
}

/// Parámetros de marcha por modo: (segundos por tramo, velocidad m/s).
fn marcha(m: Modo) -> (f32, f32) {
    match m {
        Modo::Pasta => (45.0, 0.6),
        Modo::Deambula => (14.0, 1.1),
        Modo::Picotea => (6.0, 0.35),
        Modo::Atado => (1.0, 0.0),
    }
}

fn punto(r: &Rebano, i: u32, k: i64) -> Vec2 {
    let h = mezcla(((r.semilla as u64) << 32) ^ ((i as u64) << 20) ^ (k as u64).wrapping_mul(0x9E37_79B9));
    let a = (h & 0xFFFF) as f32 / 65536.0 * std::f32::consts::TAU;
    let d = ((h >> 16) & 0xFFFF) as f32 / 65536.0;
    r.centro + Vec2::new(a.sin(), a.cos()) * d.sqrt() * r.radio
}

/// Especie del animal `i` del grupo.
pub fn especie(r: &Rebano, i: u32) -> Id {
    if r.mezcla.is_empty() {
        return r.especie;
    }
    let h = mezcla(((r.semilla as u64) << 8) ^ i as u64 ^ 0xE5);
    if h % 4 == 0 {
        r.mezcla[(h / 4 % r.mezcla.len() as u64) as usize]
    } else {
        r.especie
    }
}

/// Posición, rumbo y acción del animal `i` del grupo en el segundo `t`.
pub fn estado(r: &Rebano, i: u32, t: f32) -> (Vec2, f32, Accion) {
    if r.modo == Modo::Atado {
        // En fila a lo largo del palenque, mirándolo.
        let lado = Vec2::new(r.giro.cos(), -r.giro.sin());
        let x = (i as f32 - (r.n as f32 - 1.0) * 0.5) * 1.3;
        return (r.centro + lado * x, r.giro + ((mezcla(r.semilla as u64 ^ i as u64) % 100) as f32 - 50.0) * 0.002, Accion::Quieto);
    }
    let (tramo, vel) = marcha(r.modo);
    // Cada animal arranca su tramo en otro momento.
    let off = (mezcla(r.semilla as u64 ^ (i as u64).wrapping_mul(77)) % 10_000) as f32 / 10_000.0 * tramo;
    let tt = t + off;
    let k = (tt / tramo).floor() as i64;
    let en = tt - k as f32 * tramo;
    let (a, b) = (punto(r, i, k), punto(r, i, k + 1));
    let d = b - a;
    let largo = d.length();
    let dur = (largo / vel).min(tramo * 0.6);
    let rumbo = |d: Vec2| d.x.atan2(d.y);
    if largo < 1e-3 {
        return (a, 0.0, if r.modo == Modo::Picotea { Accion::Pasta } else { Accion::Quieto });
    }
    if en < dur {
        let u = en / dur;
        (a + d * u, rumbo(d), Accion::Camina(largo * u))
    } else {
        (b, rumbo(d), if r.modo == Modo::Deambula { Accion::Quieto } else { Accion::Pasta })
    }
}

/// Bote amarrado: posición en el agua y rumbo (la altura es la del río en el momento).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bote {
    pub pos: Vec2,
    pub giro: f32,
    pub semilla: u32,
}

/// Todo lo que se deriva de la ciudad para dibujar.
#[derive(Clone, Debug, Default)]
pub struct Escena {
    pub rebanos: Vec<Rebano>,
    pub fijos: Detalles,
    pub botes: Vec<Bote>,
}

/// Altura del suelo de vóxeles en un punto (la cara de arriba del vóxel más alto).
pub fn suelo(t: &TerrainGen, p: Vec2) -> f32 {
    ((t.height(p.x, p.y) / BASE_VOXEL - 0.5).floor() + 1.0) * BASE_VOXEL
}

/// Centro y radio de la pastura de una estancia (detrás del casco).
pub fn pastura_de(c: Vec2, f: Vec2) -> (Vec2, f32) {
    (c - f * 58.0 + side_of(f) * 10.0, 30.0)
}

/// Firma de lo que cambia la escena (se rehace cuando cambia).
pub fn firma(c: &City) -> u64 {
    let mut h: u64 = c.lots.lots.len() as u64;
    for l in c.lots.lots.values() {
        h = mezcla(h ^ ((l.id as u64) << 16) ^ ((l.level as u64) << 8) ^ l.zone as u64);
    }
    for b in c.services.buildings.values() {
        h = mezcla(h ^ b.id as u64 ^ ((b.kind as u64) << 40) ^ (b.rinde * 100.0) as i64 as u64);
    }
    h = mezcla(h ^ c.services.port.pier.as_ref().map_or(0, |p| p.len as u64 + 1));
    mezcla(h ^ ((c.month / 12) as u64) << 3 ^ (c.climate.livestock_factor() * 10.0) as u64)
}

/// Arma la escena de la ciudad.
pub fn armar(c: &City, t: &TerrainGen) -> Escena {
    let mut e = Escena::default();
    let anio = super::super::START_YEAR as i32 + (c.month / 12) as i32;
    let pastos = c.climate.livestock_factor();
    for b in c.services.of_kind(Kind::Estancia) {
        let rinde = if b.rinde < 0.0 { 1.0 } else { b.rinde };
        let (centro, radio) = pastura_de(b.c, b.f);
        // El rodeo crece con lo que rinde la estancia (como la carne, los cueros y el sebo).
        let n = (8.0 + 14.0 * rinde * pastos).round().clamp(4.0, 30.0) as u32;
        e.rebanos.push(Rebano { especie: Id::Vaca, mezcla: &[Id::VacaOvera, Id::Buey], centro, radio, n, semilla: b.id * 31 + 1, modo: Modo::Pasta, giro: 0.0 });
        // La tropilla.
        e.rebanos.push(Rebano { especie: Id::Caballo, mezcla: &[], centro: centro + side_of(b.f) * 18.0, radio: 12.0, n: 3, semilla: b.id * 31 + 2, modo: Modo::Pasta, giro: 0.0 });
        // Ovejas, desde que el lanar se hizo negocio (mediados del siglo XIX).
        if anio >= 1840 {
            e.rebanos.push(Rebano { especie: Id::Oveja, mezcla: &[], centro: centro - side_of(b.f) * 45.0, radio: 22.0, n: (n * 2).min(40), semilla: b.id * 31 + 3, modo: Modo::Pasta, giro: 0.0 });
        }
        // Carreta parada frente al casco.
        let p = b.c + b.f * 28.0 + side_of(b.f) * 16.0;
        e.fijos.poner(Id::CarretaParada, Vec3::new(p.x, suelo(t, p), p.y), side_of(b.f).x.atan2(side_of(b.f).y), b.id);
        // El perro de la estancia.
        e.rebanos.push(Rebano { especie: Id::Perro, mezcla: &[], centro: b.c + b.f * 30.0, radio: 8.0, n: 2, semilla: b.id * 31 + 4, modo: Modo::Deambula, giro: 0.0 });
    }
    for b in c.services.of_kind(Kind::Chacra) {
        e.rebanos.push(Rebano { especie: Id::Buey, mezcla: &[], centro: b.c - b.f * 52.0, radio: 10.0, n: 2, semilla: b.id * 37 + 1, modo: Modo::Pasta, giro: 0.0 });
        e.rebanos.push(Rebano { especie: Id::Gallina, mezcla: &[], centro: b.c + b.f * 44.0 + side_of(b.f) * 12.0, radio: 4.0, n: 6, semilla: b.id * 37 + 2, modo: Modo::Picotea, giro: 0.0 });
    }
    let mut casas = 0u32;
    for l in c.lots.lots.values() {
        let Some(k) = l.kind() else { continue };
        let lado = side_of(l.f);
        let frente = l.c + l.f * (LOT_D * 0.5);
        let mira_calle = l.f.x.atan2(l.f.y);
        let s = mezcla(l.seed ^ 0x9A);
        if l.level == 0 || l.voxels.is_empty() {
            continue;
        }
        match (k, l.level) {
            // Pulpería: palenque con caballos atados, el letrero y un tonel en la puerta.
            (ZoneKind::Comercial, 1) => {
                let pal = frente + l.f * 1.6;
                e.fijos.poner(Id::Palenque, Vec3::new(pal.x, suelo(t, pal), pal.y), mira_calle, l.id);
                let n = 1 + (s % 3) as u32;
                let caballos = frente + l.f * 3.0;
                e.rebanos.push(Rebano { especie: Id::CaballoEnsillado, mezcla: &[], centro: caballos, radio: 0.0, n, semilla: l.id * 13 + 5, modo: Modo::Atado, giro: (-l.f).x.atan2((-l.f).y) });
                let esquina = if l.seed % 2 == 0 { 1.0 } else { -1.0 };
                let le = frente + l.f * 0.3 + lado * esquina * (LOT_W * 0.5 - 0.8);
                e.fijos.poner(Id::Letrero, Vec3::new(le.x, suelo(t, le), le.y), mira_calle, l.id);
                let to = frente + l.f * 0.5 - lado * esquina * 2.5;
                e.fijos.poner(if s % 2 == 0 { Id::Tonel } else { Id::Barrica }, Vec3::new(to.x, suelo(t, to), to.y), mira_calle + 0.4, l.id);
            }
            // Gallinas en el fondo de los ranchos (uno de cada dos).
            (ZoneKind::Residencial, 1) if s % 2 == 0 => {
                let fondo = l.c + l.f * 2.0;
                e.rebanos.push(Rebano { especie: Id::Gallina, mezcla: &[], centro: fondo, radio: 3.5, n: 3 + (s % 3) as u32, semilla: l.id * 13 + 6, modo: Modo::Picotea, giro: 0.0 });
            }
            _ => {}
        }
        if k == ZoneKind::Residencial {
            casas += 1;
            // Un perro cada diez casas, en la calle de enfrente.
            if casas % 10 == 3 && e.rebanos.iter().filter(|r| r.especie == Id::Perro).count() < 40 {
                e.rebanos.push(Rebano { especie: Id::Perro, mezcla: &[], centro: frente + l.f * 4.0, radio: 6.0, n: 1, semilla: l.id * 13 + 7, modo: Modo::Deambula, giro: 0.0 });
            }
        }
    }
    // Botes amarrados a los costados del muelle.
    if let Some(p) = &c.services.port.pier {
        let lado = side_of(p.dir);
        let rumbo = p.dir.x.atan2(p.dir.y);
        for (k, (u, s)) in [(0.55, 1.0), (0.8, -1.0), (0.35, -1.0)].into_iter().enumerate() {
            let q = p.land + p.dir * (p.len * u) + lado * s * 3.4;
            e.botes.push(Bote { pos: q, giro: rumbo + if k == 1 { std::f32::consts::PI } else { 0.0 }, semilla: k as u32 + 11 });
        }
    }
    // Carretas paradas en la plaza (el mercado).
    if let Some((pc, ang)) = c.plaza {
        let (f, s) = super::super::plaza_frame(ang);
        for (k, d) in [-1.0f32, 1.0].into_iter().enumerate() {
            let p = pc + f * 34.0 + s * (d * 9.0);
            e.fijos.poner(Id::CarretaParada, Vec3::new(p.x, suelo(t, p), p.y), s.x.atan2(s.y) + d * 0.2, 900 + k as u32);
        }
    }
    e
}

/// Cajas de los animales cerca de `centro` en el segundo `t`.
pub fn cajas_fauna(e: &Escena, t_suelo: &TerrainGen, centro: Vec2, t: f32, out: &mut Vec<Caja>) {
    let b = biblioteca();
    for r in &e.rebanos {
        if r.centro.distance(centro) > RADIO_FAUNA + r.radio {
            continue;
        }
        for i in 0..r.n {
            let (p, giro, acc) = estado(r, i, t);
            if p.distance(centro) > RADIO_FAUNA {
                continue;
            }
            let id = especie(r, i);
            let m = b.objeto(id);
            let semilla = r.semilla.wrapping_mul(7919).wrapping_add(i);
            let ciclo = match id {
                Id::Caballo | Id::CaballoEnsillado => anim::CICLO_CABALLO,
                Id::Vaca | Id::VacaOvera | Id::Buey => anim::CICLO_VACUNO,
                _ => anim::CICLO_CHICO,
            };
            let pose = match (acc, id) {
                (Accion::Camina(d), Id::Gallina) => {
                    let mut q = anim::picotear(t, semilla);
                    let s = (anim::fase(d, ciclo) * std::f32::consts::TAU).sin() * 0.5;
                    q.set(crate::detalle::Parte::PataDI, 0.0, s);
                    q.set(crate::detalle::Parte::PataDD, 0.0, -s);
                    q
                }
                (Accion::Camina(d), _) => anim::cuadrupedo(anim::fase(d, ciclo), 1.0),
                (Accion::Pasta, Id::Gallina) => anim::picotear(t, semilla),
                (Accion::Pasta, _) => anim::pastar(t, semilla, modelos::cabeza_pastando(id)),
                (Accion::Quieto, _) => anim::quieto(t, semilla),
            };
            let base = Vec3::new(p.x, suelo(t_suelo, p), p.y);
            emitir(out, m, &modelos::paleta(id, semilla), base, giro, &pose, semilla);
            sombra(out, base, giro, [m.huella[0] * 0.75, m.huella[1] * 0.8], 120);
        }
    }
}

/// Cajas de los botes, flotando en el nivel del río.
pub fn cajas_botes(e: &Escena, t_suelo: &TerrainGen, centro: Vec2, nivel: f32, t: f32, out: &mut Vec<Caja>) {
    let m = biblioteca().objeto(Id::Bote);
    for (k, bo) in e.botes.iter().enumerate() {
        if bo.pos.distance(centro) > RADIO_FAUNA {
            continue;
        }
        // Un balanceo lento; calado de 0,3 m. Con la bajante quedan varados en el barro.
        let fondo = suelo(t_suelo, bo.pos);
        let y = if nivel - 0.3 > fondo { nivel - 0.3 + (t * 0.8 + k as f32).sin() * 0.03 } else { fondo - 0.05 };
        let mut pose = crate::detalle::Pose::default();
        pose.set(crate::detalle::Parte::Fijo, 0.0, 0.0);
        emitir(out, m, &modelos::paleta(Id::Bote, bo.semilla), Vec3::new(bo.pos.x, y, bo.pos.y), bo.giro + (t * 0.3 + k as f32).sin() * 0.03, &pose, bo.semilla);
    }
}

/// Todo lo que se dibuja con el pipeline de los agentes cerca de `centro`: vecinos y
/// vehículos (vestidos según su persona y el año), animales, botes y detalles fijos.
/// `hora`: la del reloj del día (para el nivel del río).
pub fn cajas_ciudad(c: &City, t: &TerrainGen, centro: Vec2, hora: f32, out: &mut Vec<Caja>) {
    out.clear();
    let ag = &c.agentes;
    // Segundos "reales" (el reloj de los agentes corre a ESCALA_TIEMPO): los animales y las
    // miradas se mueven a su ritmo natural.
    let segs = ((ag.e.reloj % 1.0e5) as f32 + ag.alfa() * super::PASO) / super::ESCALA_TIEMPO;
    let anio = super::super::START_YEAR as i32 + (c.month / 12) as i32;
    let v = super::dibujo::Vestuario { poblacion: Some(&c.poblacion), lots: Some(&c.lots), anio, t: segs };
    ag.cajas_con(&c.net, centro, &v, out);
    ag.cajas_oficios(c, t, centro, &v, out); // [ia]
    ag.cajas_rampas(c, t, centro, out); // [ia]
    let esc = escena(c, t);
    cajas_fauna(&esc, t, centro, segs, out);
    cajas_botes(&esc, t, centro, c.water_level(hora), segs, out);
    esc.fijos.cajas(centro, out);
    ag.detalles.cajas(centro, out);
}

/// La escena de la ciudad, armada de nuevo solo si cambió su firma.
pub fn escena(c: &City, t: &TerrainGen) -> std::sync::Arc<Escena> {
    let f = firma(c);
    let mut g = c.agentes.escena.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, e)) = g.as_ref() {
        if *k == f {
            return e.clone();
        }
    }
    let e = std::sync::Arc::new(armar(c, t));
    *g = Some((f, e.clone()));
    e
}

impl super::Agentes {
    /// Pone una fila de modelos de detalle de muestra junto a `plaza` (sobre la calle más
    /// cercana), para mirarlos de cerca. Devuelve cuántos puso.
    pub fn poner_muestra(&mut self, t: &TerrainGen, net: &super::super::roads::RoadNetwork, plaza: Vec2) -> usize {
        let Some(n) = net.nearest_edge(plaza + Vec2::new(70.0, 0.0), 120.0) else { return 0 };
        let e = &net.edges[&n.edge];
        let (p0, dir) = e.point(e.len() * 0.5);
        let lado = side_of(dir);
        // Al costado de la calle, en fila a lo largo.
        let c0 = p0 + lado * 4.0;
        let fila = [
            Id::FarolAceite,
            Id::FarolGas,
            Id::Tonel,
            Id::Barrica,
            Id::Banco,
            Id::BrocalAljibe,
            Id::PozoRoldana,
            Id::Cruz,
            Id::Veleta,
            Id::Letrero,
            Id::Palenque,
            Id::CarretaParada,
            Id::Perro,
            Id::Gallina,
            Id::Oveja,
            Id::Vaca,
            Id::VacaOvera,
            Id::Caballo,
        ];
        let mut x = -12.0f32;
        let mira = (-lado).x.atan2((-lado).y);
        self.detalles.quitar(|_| true);
        for (k, id) in fila.into_iter().enumerate() {
            let m = biblioteca().objeto(id);
            let ancho = (m.max[0] - m.min[0]).max(m.max[2] - m.min[2]) as f32 * crate::detalle::VOX;
            let ancho = if matches!(id, Id::CarretaParada) { 3.0 } else { ancho.min(3.0) };
            x += ancho * 0.5 + 0.4;
            let p = c0 + dir * x;
            let giro = if matches!(id, Id::CarretaParada | Id::Vaca | Id::VacaOvera | Id::Caballo | Id::Perro | Id::Oveja | Id::Gallina | Id::Palenque | Id::Banco) { dir.x.atan2(dir.y) + std::f32::consts::FRAC_PI_2 * if matches!(id, Id::Palenque | Id::Banco) { 1.0 } else { 0.0 } } else { mira };
            self.detalles.poner(id, Vec3::new(p.x, suelo(t, p), p.y), giro, 40 + k as u32);
            x += ancho * 0.5;
        }
        self.muestra = Some((c0 + dir * -12.0, dir, x + 12.0));
        self.detalles.len()
    }

    /// Comienzo, dirección y largo de la fila de muestra de detalles, si se puso.
    pub fn escena_muestra(&self) -> Option<(Vec2, Vec2, f32)> {
        self.muestra
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::services::Building;

    fn rebano(modo: Modo, radio: f32) -> Rebano {
        Rebano { especie: Id::Vaca, mezcla: &[Id::Buey], centro: Vec2::new(500.0, -300.0), radio, n: 12, semilla: 77, modo, giro: 0.4 }
    }

    #[test]
    fn los_animales_no_salen_de_su_area_y_son_deterministas() {
        for modo in [Modo::Pasta, Modo::Deambula, Modo::Picotea] {
            let r = rebano(modo, 20.0);
            let mut camina = 0;
            let mut t = 0.0;
            while t < 6000.0 {
                for i in 0..r.n {
                    let (p, g, a) = estado(&r, i, t);
                    assert!(p.distance(r.centro) <= r.radio + 1e-3, "{modo:?}: {p:?} fuera del área");
                    assert!(g.is_finite());
                    assert_eq!((p, g, a), estado(&r, i, t), "la misma hora da lo mismo");
                    if matches!(a, Accion::Camina(_)) {
                        camina += 1;
                    }
                }
                t += 3.7;
            }
            assert!(camina > 0, "{modo:?}: alguno camina");
        }
        // Atados: siempre en el mismo lugar, en fila junto al palenque.
        let r = rebano(Modo::Atado, 0.0);
        assert_eq!(estado(&r, 3, 10.0), estado(&r, 3, 999.0));
        assert!(estado(&r, 0, 1.0).0.distance(estado(&r, 1, 1.0).0) > 1.0);
        // El rodeo mezcla especies.
        assert!((0..40).any(|i| especie(&rebano(Modo::Pasta, 5.0), i) == Id::Buey));
    }

    #[test]
    fn el_rodeo_crece_con_la_estancia_y_no_cambia_la_partida() {
        let t = TerrainGen::new(1726);
        let mut c = City::default();
        let rodeo = |c: &City| armar(c, &t).rebanos.iter().filter(|r| r.especie == Id::Vaca).map(|r| r.n).sum::<u32>();
        assert_eq!(rodeo(&c), 0);
        let est = |rinde: f32| Building { id: 1, kind: Kind::Estancia, c: Vec2::new(12000.0, 9000.0), f: Vec2::new(0.0, 1.0), served: 0, contaminacion: 0.0, rinde, voxels: Vec::new() };
        c.services.buildings.insert(1, est(0.4));
        let pobre = rodeo(&c);
        let f1 = firma(&c);
        c.services.buildings.insert(1, est(1.6));
        let rica = rodeo(&c);
        assert!(rica > pobre && pobre >= 4, "rodeo {pobre} → {rica}");
        assert_ne!(firma(&c), f1, "la escena se rehace cuando cambia la estancia");
        // Dibujar no toca el estado que se guarda.
        let antes = serde_json::to_string(&c.agentes.e).unwrap();
        let mut out = Vec::new();
        cajas_ciudad(&c, &t, Vec2::new(12000.0, 9000.0 - 58.0), 10.0, &mut out);
        assert!(out.len() > 100, "{} cajas", out.len());
        assert!(out.iter().any(|x| x.es_sombra()));
        assert_eq!(antes, serde_json::to_string(&c.agentes.e).unwrap());
        // Todas las vacas paradas sobre el suelo de vóxeles.
        let e = escena(&c, &t);
        for r in &e.rebanos {
            for i in 0..r.n {
                let (p, _, _) = estado(r, i, 50.0);
                assert!((suelo(&t, p) - t.height(p.x, p.y)).abs() < 0.5);
            }
        }
    }
}
