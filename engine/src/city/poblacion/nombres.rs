//! Nombres de pila y apellidos comunes de cada época y origen. Todas las personas son
//! ficticias: los nombres se combinan al azar a partir de listas de nombres frecuentes, no se
//! recrea a ninguna persona real.
//!
//! Notas de representación:
//! - Las personas africanas llegadas por la trata recibían en el bautismo un nombre cristiano
//!   impuesto y se las registraba con su "nación" (Congo, Angola, Benguela, Mina, Calabarí,
//!   Mozambique…: nombres de puertos y regiones de embarque, no de pueblos) y el nombre de
//!   quien las sometía ("Rosa, de nación Benguela, esclava de D. …"); el apellido, cuando lo
//!   tenían, solía ser el de esa casa (Frega et al., 2020, cap. 7). Aquí se muestran como
//!   "Rosa, de nación Benguela" (sin el nombre de nadie que las posea) y toman apellido al
//!   obtener la libertad: muchas veces el de la casa donde vivieron, a veces otro. Queda en su
//!   historia.
//! - Los guaraníes de las misiones tenían nombre cristiano y, muchas veces, apellido de raíz
//!   guaraní; con los años muchos tomaron apellidos castellanos. La lista guaraní es corta y
//!   orientativa (estimación, ver `docs/sistemas/poblacion.md`).

use super::modelo::{Cultura, Sexo};

pub const HISPANO_H: &[&str] = &[
    "Juan", "José", "Francisco", "Antonio", "Pedro", "Manuel", "Domingo", "Bartolomé", "Miguel", "Luis", "Andrés", "Diego", "Bernardo", "Felipe", "Ignacio", "Tomás", "Joaquín", "Agustín", "Lorenzo", "Sebastián", "Cristóbal", "Gregorio", "Matías", "Pablo", "Vicente", "Ramón", "Santiago", "Blas", "Fermín", "Mateo", "Nicolás", "Juan José", "Juan Antonio", "José Antonio", "Francisco Javier",
];
pub const HISPANO_M: &[&str] = &[
    "María", "Josefa", "Juana", "Francisca", "Ana", "Catalina", "Isabel", "Antonia", "Manuela", "Teresa", "Rosa", "Petrona", "Bernarda", "Gregoria", "Margarita", "Lucía", "Inés", "Leonor", "Ignacia", "Tomasa", "Feliciana", "Agustina", "Clara", "Micaela", "Ramona", "Juliana", "María Antonia", "María Josefa", "María Rosa", "Pascuala",
];
pub const HISPANO_AP: &[&str] = &[
    "García", "Fernández", "González", "Rodríguez", "López", "Martínez", "Sánchez", "Pérez", "Gómez", "Díaz", "Álvarez", "Romero", "Ruiz", "Moreno", "Navarro", "Torres", "Domínguez", "Vázquez", "Ramos", "Gil", "Serrano", "Molina", "Ortiz", "Castillo", "Herrera", "Medina", "Aguilar", "Vega", "Ríos", "Benítez", "Sosa", "Acosta", "Correa", "Silveira", "Maciel", "Barrios", "Villalba", "Ledesma", "Olivera", "Chaves",
];
pub const CANARIO_AP: &[&str] = &[
    "Hernández", "González", "Rodríguez", "Pérez", "Díaz", "Martín", "Suárez", "Cabrera", "Delgado", "Morales", "Perdomo", "Mesa", "Medina", "Padrón", "Quintana", "Umpiérrez", "Santana", "Ramos", "Afonso", "Melián", "Bello", "Trujillo", "Acosta", "Toledo", "Viera", "Socas", "Curbelo", "Betancor", "Brito", "Arbelo",
];
pub const GALLEGO_AP: &[&str] = &["Fernández", "López", "Otero", "Castro", "Vázquez", "Pereira", "Rey", "Fraga", "Varela", "Souto", "Lema", "Seoane", "Iglesias", "Novoa", "Paz", "Rivas", "Mosquera", "Losada"];
pub const VASCO_AP: &[&str] = &["Echeverría", "Aguirre", "Zubillaga", "Iturralde", "Goñi", "Arrieta", "Elizalde", "Garmendia", "Olaechea", "Lasarte", "Urrutia", "Zabaleta", "Arrillaga", "Ibarra", "Etcheverry", "Larre"];
pub const CATALAN_AP: &[&str] = &["Puig", "Vidal", "Ferrer", "Soler", "Roca", "Pons", "Riera", "Font", "Mas", "Sala", "Camps", "Prats"];
pub const PORTUGUES_H: &[&str] = &["João", "Manoel", "Antônio", "Francisco", "José", "Domingos", "Bento", "Luís", "Pedro", "Joaquim", "Inácio", "Caetano"];
pub const PORTUGUES_M: &[&str] = &["Maria", "Ana", "Joana", "Rosa", "Luzia", "Teresa", "Josefa", "Francisca", "Inês", "Quitéria", "Rita"];
pub const PORTUGUES_AP: &[&str] = &["Silva", "Souza", "Oliveira", "Pereira", "Costa", "Almeida", "Carvalho", "Ferreira", "Machado", "Mendes", "Teixeira", "Brito", "Rodrigues", "Gonçalves", "Nunes", "Fagundes"];
/// Apellidos de raíz guaraní (orientativos) que se alternan con apellidos castellanos.
pub const GUARANI_AP: &[&str] = &["Arapí", "Yaparí", "Guarapí", "Caraí", "Tupayú", "Mbatará", "Aracú", "Yaguarú"];
pub const ITALIANO_H: &[&str] = &["Giuseppe", "Giovanni", "Antonio", "Francesco", "Luigi", "Pietro", "Carlo", "Angelo", "Domenico", "Vincenzo", "Salvatore", "Battista"];
pub const ITALIANO_M: &[&str] = &["Maria", "Giuseppina", "Rosa", "Angela", "Teresa", "Anna", "Lucia", "Caterina", "Carmela", "Assunta", "Concetta"];
pub const ITALIANO_AP: &[&str] = &["Rossi", "Russo", "Ferrari", "Esposito", "Bianchi", "Romano", "Colombo", "Ricci", "Marino", "Greco", "Bruno", "Gallo", "Conti", "De Luca", "Lombardi", "Moretti", "Barbieri", "Fontana", "Santoro", "Mariani", "Rinaldi", "Caruso", "Ferrara", "Galli", "Canepa", "Parodi"];
pub const FRANCES_H: &[&str] = &["Jean", "Pierre", "Louis", "Jacques", "Michel", "Bernard", "Étienne", "Joseph", "Pascal"];
pub const FRANCES_M: &[&str] = &["Marie", "Jeanne", "Anne", "Catherine", "Marguerite", "Louise", "Madeleine", "Françoise"];
pub const FRANCES_AP: &[&str] = &["Martin", "Bernard", "Dubois", "Durand", "Lefebvre", "Moreau", "Laurent", "Simon", "Michel", "Garnier", "Roux", "Fournier", "Iribarne", "Larrouy"];
pub const BRITANICO_H: &[&str] = &["John", "William", "James", "Thomas", "George", "Robert", "Henry", "Patrick"];
pub const BRITANICO_M: &[&str] = &["Mary", "Elizabeth", "Anne", "Sarah", "Margaret", "Jane", "Catherine"];
pub const BRITANICO_AP: &[&str] = &["Smith", "Brown", "Wilson", "Taylor", "Young", "Campbell", "Stewart", "Murphy", "Kelly", "Hughes"];
pub const ALEMAN_H: &[&str] = &["Johann", "Friedrich", "Heinrich", "Karl", "Wilhelm", "Hans", "Jakob"];
pub const ALEMAN_M: &[&str] = &["Anna", "Maria", "Elisabeth", "Katharina", "Margarethe", "Sophie"];
pub const ALEMAN_AP: &[&str] = &["Müller", "Schmidt", "Schneider", "Fischer", "Weber", "Meyer", "Wagner", "Becker", "Hoffmann", "Keller"];
pub const LEVANTINO_H: &[&str] = &["Elías", "Jorge", "José", "Miguel", "Nicolás", "Antonio", "Salim", "Jalil", "Nagib"];
pub const LEVANTINO_M: &[&str] = &["María", "Elena", "Rosa", "Salma", "Laila", "Nadia", "Jamile", "Amelia"];
pub const LEVANTINO_AP: &[&str] = &["Haddad", "Khoury", "Saade", "Nader", "Salem", "Atie", "Hanna", "Karam", "Daher", "Abdala"];
pub const JUDIO_H: &[&str] = &["Abraham", "Isaac", "Jacobo", "Moisés", "Samuel", "David", "Salomón", "León", "Bernardo", "Mauricio"];
pub const JUDIO_M: &[&str] = &["Ana", "Rebeca", "Sara", "Raquel", "Esther", "Clara", "Dora", "Rosa", "Fanny", "Ida"];
pub const JUDIO_AP: &[&str] = &["Goldberg", "Rosenberg", "Weiss", "Friedman", "Kaplan", "Levin", "Shapiro", "Zylberberg", "Rubinstein", "Grinberg", "Liberman", "Fainstein", "Benarroch", "Cohen"];
pub const ARMENIO_H: &[&str] = &["Hagop", "Garabed", "Krikor", "Ohannes", "Aram", "Sarkis", "Vartan", "Levon"];
pub const ARMENIO_M: &[&str] = &["Anahid", "Siranush", "Arpi", "Mari", "Zabel", "Nvart", "Takuhi"];
pub const ARMENIO_AP: &[&str] = &["Kasparian", "Sarkissian", "Boyadjian", "Hovsepian", "Manukian", "Avedissian", "Kalustian", "Der Bedrossian"];

/// "Naciones" africanas registradas en Montevideo, por región de embarque (orientativo:
/// Andrews, 2007, nombra Congo, Mina, Benguela y Calabarí; Frega et al., 2020, Angola y
/// Mozambique; las salas de nación de la década de 1830 eran de África occidental, del Congo y
/// Angola y de África oriental).
pub const NACIONES_CENTRO: &[&str] = &["Congo", "Angola", "Benguela"];
pub const NACIONES_OCCIDENTE: &[&str] = &["Mina", "Calabarí"];
pub const NACIONES_ORIENTE: &[&str] = &["Mozambique"];

/// La "nación" con que se registraba a una persona africana (determinista por su id: no
/// necesita guardarse). `None` si no nació en África.
pub fn nacion(o: super::modelo::Origen, id: u32) -> Option<&'static str> {
    use super::modelo::Origen;
    let l = match o {
        Origen::AfricaCentral => NACIONES_CENTRO,
        Origen::AfricaOccidental => NACIONES_OCCIDENTE,
        Origen::AfricaOriental => NACIONES_ORIENTE,
        _ => return None,
    };
    Some(l[(id as usize).wrapping_mul(2654435761) % l.len()])
}

pub fn nombres(c: Cultura, s: Sexo) -> &'static [&'static str] {
    use Cultura::*;
    match (c, s) {
        (Portuguesa, Sexo::Hombre) => PORTUGUES_H,
        (Portuguesa, Sexo::Mujer) => PORTUGUES_M,
        (Italiana, Sexo::Hombre) => ITALIANO_H,
        (Italiana, Sexo::Mujer) => ITALIANO_M,
        (Francesa, Sexo::Hombre) => FRANCES_H,
        (Francesa, Sexo::Mujer) => FRANCES_M,
        (Britanica, Sexo::Hombre) => BRITANICO_H,
        (Britanica, Sexo::Mujer) => BRITANICO_M,
        (Alemana, Sexo::Hombre) => ALEMAN_H,
        (Alemana, Sexo::Mujer) => ALEMAN_M,
        (Levantina, Sexo::Hombre) => LEVANTINO_H,
        (Levantina, Sexo::Mujer) => LEVANTINO_M,
        (Judia, Sexo::Hombre) => JUDIO_H,
        (Judia, Sexo::Mujer) => JUDIO_M,
        (Armenia, Sexo::Hombre) => ARMENIO_H,
        (Armenia, Sexo::Mujer) => ARMENIO_M,
        (_, Sexo::Hombre) => HISPANO_H,
        (_, Sexo::Mujer) => HISPANO_M,
    }
}

/// Apellidos de una cultura. Las personas africanas llegan sin apellido (lista vacía).
pub fn apellidos(c: Cultura) -> &'static [&'static str] {
    use Cultura::*;
    match c {
        Canaria => CANARIO_AP,
        Gallega => GALLEGO_AP,
        Vasca => VASCO_AP,
        Catalana => CATALAN_AP,
        Portuguesa => PORTUGUES_AP,
        Guarani => GUARANI_AP,
        Africana => &[],
        Italiana => ITALIANO_AP,
        Francesa => FRANCES_AP,
        Britanica => BRITANICO_AP,
        Alemana => ALEMAN_AP,
        Levantina => LEVANTINO_AP,
        Judia => JUDIO_AP,
        Armenia => ARMENIO_AP,
        Hispana => HISPANO_AP,
    }
}
