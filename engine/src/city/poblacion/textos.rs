//! Textos de la población para la interfaz y la crónica: la historia de vida de una persona,
//! su familia (árbol de unas pocas generaciones) y su linaje por línea paterna.

use super::*;

pub fn fecha(mes: i32) -> String {
    format!("{} de {}", crate::city::MONTHS[mes.rem_euclid(12) as usize], año_de(mes))
}

pub fn educacion(e: u8) -> &'static str {
    match e {
        0 => "no sabe leer",
        1 => "sabe leer",
        2 => "lee y escribe",
        _ => "tiene estudios",
    }
}

/// Parentescos para el árbol de la ficha.
pub struct Familia {
    pub abuelos: Vec<PersonaId>,
    pub padres: Vec<PersonaId>,
    pub conyuge: Option<PersonaId>,
    pub hermanos: Vec<PersonaId>,
    pub hijos: Vec<PersonaId>,
    pub nietos: Vec<PersonaId>,
}

impl Poblacion {
    /// Un hecho de la historia de vida, en una frase.
    pub fn texto_evento(&self, p: &Persona, e: &Evento) -> String {
        let a = |m: &str, f: &str| if p.sexo == Sexo::Mujer { f.to_string() } else { m.to_string() };
        let otro = || if e.con != NADIE { self.nombre_completo(e.con) } else { String::new() };
        match e.tipo {
            TipoEvento::Nacio => match p.padre {
                Some(_) => format!("{} en la ciudad", a("Nace", "Nace")),
                None => format!("Nace en {}", p.origen.nombre()),
            },
            TipoEvento::Llego(o) => format!("{} desde {}", a("Llega", "Llega"), o.nombre()),
            TipoEvento::LlegoForzada(o) => format!("Llega por la trata desde {}, {} contra su voluntad", o.nombre(), a("traído", "traída")),
            TipoEvento::Caso => format!("Se casa con {}", otro()),
            TipoEvento::Union => format!("Forma pareja con {}", otro()),
            TipoEvento::Enviudo => format!("{} de {}", a("Enviuda", "Enviuda"), otro()),
            TipoEvento::NacioHijo => format!("Nace su {} {}", if self.persona(e.con).is_some_and(|c| c.sexo == Sexo::Mujer) { "hija" } else { "hijo" }, self.nombre(e.con)),
            TipoEvento::MurioHijo => format!("Muere su {} {}", if self.persona(e.con).is_some_and(|c| c.sexo == Sexo::Mujer) { "hija" } else { "hijo" }, self.nombre(e.con)),
            TipoEvento::MurioPadre => format!("Muere su {} {}", if self.persona(e.con).is_some_and(|c| c.sexo == Sexo::Mujer) { "madre" } else { "padre" }, self.nombre(e.con)),
            TipoEvento::Libertad(v) => match v {
                ViaLibertad::Ahorro => "Obtiene su carta de libertad con sus ahorros".into(),
                ViaLibertad::Testamento => "Recibe la libertad por testamento".into(),
                ViaLibertad::Fondo => "Obtiene la libertad con ayuda del fondo del cabildo".into(),
                ViaLibertad::Abolicion => "Es libre por la ley de abolición".into(),
                ViaLibertad::Vientres => "Nace libre por la ley de libertad de vientres".into(),
            },
            TipoEvento::TomoApellido => format!("Toma el apellido {}", self.texto(p.apellidos[0])),
            TipoEvento::Aprendio => "Aprende sus primeras letras".into(),
            TipoEvento::Oficio(o) => format!("Empieza a trabajar de {}", o.nombre(p.sexo)),
            TipoEvento::SeMudo => "Se muda a una casa nueva".into(),
            TipoEvento::CompraSolar => "Su familia recibe o compra un solar".into(),
            TipoEvento::MejoroCasa(n) => format!("Mejora su casa: ahora es {}", ["", "un rancho", "una casa de azotea", "una casa de altos"][n.min(3) as usize]),
            TipoEvento::Heredo(x) => format!("Hereda {x} pesos de {}", otro()),
            TipoEvento::Pleito => format!("Pleitea por la herencia de {}", otro()),
            TipoEvento::Empobrecio => "Su familia pasa hambre".into(),
            TipoEvento::Emigro => "Se va de la ciudad".into(),
            TipoEvento::Murio(c) => format!("{} a los {} años, por {}", a("Muere", "Muere"), p.edad(e.mes), c.texto()),
        }
    }

    /// Abuelos, padres, cónyuge, hermanos, hijos y nietos.
    pub fn familia(&self, id: PersonaId) -> Familia {
        let p = &self.personas[id as usize];
        let padres: Vec<PersonaId> = [p.padre, p.madre].into_iter().flatten().collect();
        let abuelos: Vec<PersonaId> = padres.iter().flat_map(|&x| [self.personas[x as usize].padre, self.personas[x as usize].madre]).flatten().collect();
        let mut hermanos: Vec<PersonaId> = padres.iter().flat_map(|&x| self.personas[x as usize].hijos.iter().copied()).filter(|&h| h != id).collect();
        hermanos.sort();
        hermanos.dedup();
        let nietos: Vec<PersonaId> = p.hijos.iter().flat_map(|&h| self.personas[h as usize].hijos.iter().copied()).collect();
        Familia { abuelos, padres, conyuge: p.conyuge, hermanos, hijos: p.hijos.clone(), nietos }
    }

    /// Linaje por línea paterna (o materna si no hay padre): de la persona hacia atrás.
    pub fn linaje(&self, id: PersonaId) -> Vec<PersonaId> {
        let mut v = vec![id];
        let mut x = id;
        while let Some(a) = self.personas[x as usize].padre.or(self.personas[x as usize].madre) {
            v.push(a);
            x = a;
            if v.len() > 40 {
                break;
            }
        }
        v
    }

    /// Descendientes vivos de una persona (todas las generaciones).
    pub fn descendientes_vivos(&self, id: PersonaId) -> usize {
        let mut pila = self.personas[id as usize].hijos.clone();
        let mut n = 0;
        let mut vistos = std::collections::BTreeSet::new();
        while let Some(x) = pila.pop() {
            if !vistos.insert(x) {
                continue;
            }
            if self.personas[x as usize].viva() {
                n += 1;
            }
            pila.extend(self.personas[x as usize].hijos.iter().copied());
        }
        n
    }

    /// "Juan Hernández Mesa (45)" para los botones de la ficha.
    pub fn etiqueta(&self, id: PersonaId) -> String {
        let p = &self.personas[id as usize];
        let nacio = año_de(p.nacimiento);
        match p.muerte {
            Some(m) => format!("{} ({}–{})", self.nombre_completo(id), nacio, año_de(m)),
            None if p.fuera.is_some() => format!("{} (n. {}, se fue)", self.nombre_completo(id), nacio),
            None => format!("{} ({})", self.nombre_completo(id), p.edad(self.mes)),
        }
    }
}
