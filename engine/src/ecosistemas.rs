//! [atlas] Ecosistemas con la leyenda del «Atlas ecológico e histórico del Uruguay» (v1.0).
//!
//! Los datos salen de `tools/mapa_real/atlas_ecologico.py` (ver
//! `docs/sistemas/atlas-ecologico.md`): una grilla por escala (el país a ~1 km, el entorno de la
//! ciudad a 100 m y el entorno amplio a 500 m), con una capa completa por período. Cada celda
//! guarda el código primario y el secundario (mosaico), la densidad arbórea, la altura del
//! dosel (solo si hay fuente), la evidencia (documentado / inferido / desconocido), la
//! confianza y la fuente. Los números de código son estables: no cambian nunca.

#![allow(dead_code)] // la leyenda completa es parte del formato aunque el motor no use todos los códigos

/// Una clase de la leyenda.
#[derive(Clone, Copy, Debug)]
pub struct Clase {
    pub n: u8,
    pub codigo: &'static str,
    pub nombre: &'static str,
    pub color: [u8; 3],
    /// `false`: extensión del juego (no está en el atlas).
    pub atlas: bool,
}

const fn cl(n: u8, codigo: &'static str, nombre: &'static str, color: [u8; 3], atlas: bool) -> Clase {
    Clase { n, codigo, nombre, color, atlas }
}

/// La leyenda del atlas (18 clases) y las extensiones del juego. Mismo orden y números que
/// `LEYENDA` y `EXTENSIONES` de `atlas_ecologico.py`.
pub const LEYENDA: [Clase; 21] = [
    cl(1, "PR01", "Pradera natural", [176, 190, 112], true),
    cl(2, "PR02", "Pastizal ralo sobre suelos superficiales", [206, 196, 132], true),
    cl(3, "PR03", "Pastizal denso", [140, 176, 96], true),
    cl(4, "PR04", "Pastizal alto y denso", [112, 156, 84], true),
    cl(5, "SH01", "Chircal y matorral", [160, 138, 92], true),
    cl(6, "FO01", "Monte ribereño/fluvial", [40, 104, 56], true),
    cl(7, "FO02", "Monte de quebrada", [22, 78, 46], true),
    cl(8, "FO03", "Monte serrano", [84, 112, 66], true),
    cl(9, "FO04", "Bosque parque / espinal", [148, 128, 70], true),
    cl(10, "FO05", "Monte psamófilo/costero", [92, 132, 104], true),
    cl(11, "PA01", "Palmar de butiá", [196, 150, 52], true),
    cl(12, "PA02", "Palmar de yatay", [210, 120, 60], true),
    cl(13, "HU01", "Bañado / pastizal inundable", [84, 140, 136], true),
    cl(14, "AQ01", "Vegetación acuática y de orilla", [120, 168, 186], true),
    cl(15, "CO01", "Arenales, dunas y costa abierta", [232, 218, 172], true),
    cl(16, "SA01", "Blanqueales/suelos alcalinos", [222, 214, 200], true),
    cl(17, "AN01", "Sabana arbolada / arbolado disperso", [164, 150, 86], true),
    cl(18, "AN02", "Ecotono bosque-pradera", [110, 140, 76], true),
    cl(40, "PL01", "Plantación forestal (pinos, acacias, eucaliptos)", [150, 60, 130], false),
    cl(41, "UR01", "Área urbana", [120, 110, 110], false),
    cl(42, "TR01", "Cobertura transformada (sin dato por celda)", [200, 180, 170], false),
];

pub const PR01: u8 = 1;
pub const PR02: u8 = 2;
pub const PR03: u8 = 3;
pub const PR04: u8 = 4;
pub const SH01: u8 = 5;
pub const FO01: u8 = 6;
pub const FO02: u8 = 7;
pub const FO03: u8 = 8;
pub const FO04: u8 = 9;
pub const FO05: u8 = 10;
pub const PA01: u8 = 11;
pub const PA02: u8 = 12;
pub const HU01: u8 = 13;
pub const AQ01: u8 = 14;
pub const CO01: u8 = 15;
pub const SA01: u8 = 16;
pub const AN01: u8 = 17;
pub const AN02: u8 = 18;
pub const PL01: u8 = 40;
pub const UR01: u8 = 41;
pub const TR01: u8 = 42;

/// Clase de un número de código (`None` para 0: agua o fuera del alcance del atlas).
pub fn clase(n: u8) -> Option<&'static Clase> {
    LEYENDA.iter().find(|c| c.n == n)
}

pub const DENSIDAD: [&str; 6] = ["ausente", "muy baja", "baja", "media", "alta", "sin dato"];
pub const EVIDENCIA: [&str; 3] = ["desconocido", "inferencia ecológica", "documentado"];
pub const CONFIANZA: [&str; 3] = ["baja", "media", "alta"];

/// Una celda (6 bytes en el archivo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Celda {
    pub prim: u8,
    pub sec: u8,
    /// 0 ausente, 1 muy baja, 2 baja, 3 media, 4 alta, 5 sin dato.
    pub densidad: u8,
    /// Decímetros; 0 = sin fuente.
    pub dosel_dm: u8,
    /// 0 desconocido, 1 inferencia ecológica, 2 documentado.
    pub evidencia: u8,
    /// 0 baja, 1 media, 2 alta.
    pub confianza: u8,
    /// Índice en `fuentes` de `ecosistemas.json`.
    pub fuente: u8,
}

impl Celda {
    /// ¿El código primario o el secundario es `n`?
    pub fn tiene(&self, n: u8) -> bool {
        self.prim == n || self.sec == n
    }

    /// Altura del dosel en metros, si hay fuente.
    pub fn dosel_m(&self) -> Option<f32> {
        (self.dosel_dm > 0).then(|| self.dosel_dm as f32 / 10.0)
    }
}

/// Una capa (período) de una grilla.
pub struct Capa {
    pub desde: i32,
    pub hasta: i32,
    datos: Vec<u8>,
}

/// Una grilla con sus capas por período.
pub struct Grilla {
    pub ancho: usize,
    pub alto: usize,
    pub capas: Vec<Capa>,
}

impl Grilla {
    /// Lee el formato `ECO1` (ver `atlas_ecologico.py`).
    pub fn leer(b: &[u8]) -> Result<Grilla, String> {
        if b.len() < 10 || &b[0..4] != b"ECO1" {
            return Err("no es un archivo ECO1".into());
        }
        let ancho = u16::from_le_bytes([b[4], b[5]]) as usize;
        let alto = u16::from_le_bytes([b[6], b[7]]) as usize;
        if b[8] != 6 {
            return Err(format!("{} bytes por celda (se esperaban 6)", b[8]));
        }
        let n = b[9] as usize;
        let mut o = 10;
        let mut capas = Vec::with_capacity(n);
        for _ in 0..n {
            if o + 8 > b.len() {
                return Err("archivo cortado".into());
            }
            let desde = i16::from_le_bytes([b[o], b[o + 1]]) as i32;
            let hasta = i16::from_le_bytes([b[o + 2], b[o + 3]]) as i32;
            let len = u32::from_le_bytes([b[o + 4], b[o + 5], b[o + 6], b[o + 7]]) as usize;
            o += 8;
            let z = b.get(o..o + len).ok_or("archivo cortado")?;
            let datos = miniz_oxide::inflate::decompress_to_vec_zlib(z).map_err(|e| format!("zlib: {e:?}"))?;
            if datos.len() != ancho * alto * 6 {
                return Err(format!("capa {desde}: {} bytes para {ancho} × {alto}", datos.len()));
            }
            capas.push(Capa { desde, hasta, datos });
            o += len;
        }
        if capas.is_empty() {
            return Err("sin capas".into());
        }
        Ok(Grilla { ancho, alto, capas })
    }

    /// Índice de la capa que rige en un año (la primera antes de la base, la última después).
    pub fn capa_de(&self, anio: i32) -> usize {
        self.capas.iter().rposition(|c| c.desde <= anio).unwrap_or(0)
    }

    /// Celda (columna `x`, fila `z`) de una capa; `None` fuera de la grilla.
    pub fn celda(&self, capa: usize, x: i64, z: i64) -> Option<Celda> {
        if x < 0 || z < 0 || x as usize >= self.ancho || z as usize >= self.alto {
            return None;
        }
        let k = (z as usize * self.ancho + x as usize) * 6;
        let d = &self.capas[capa].datos[k..k + 6];
        Some(Celda { prim: d[0], sec: d[1], densidad: d[2], dosel_dm: d[3], evidencia: d[4] & 15, confianza: d[4] >> 4, fuente: d[5] })
    }
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct Fuente {
    pub id: String,
    pub cita: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct CapaInfo {
    pub desde: i32,
    pub hasta: i32,
    pub periodo: String,
    pub nota: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct LeyendaJson {
    pub n: u8,
    pub codigo: String,
    pub nombre: String,
    pub atlas: bool,
}

/// `ecosistemas.json`: leyenda, fuentes, capas y estadísticas de MapBiomas por departamento.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct Info {
    pub leyenda: Vec<LeyendaJson>,
    pub fuentes: Vec<Fuente>,
    pub capas: Vec<CapaInfo>,
    #[serde(default)]
    pub mapbiomas_departamentos_km2: Option<std::collections::BTreeMap<String, std::collections::BTreeMap<String, std::collections::BTreeMap<String, f32>>>>,
}

pub const INFO_JSON: &str = include_str!("../../assets/mapas/uruguay/ecosistemas.json");

pub fn info() -> &'static Info {
    static I: std::sync::OnceLock<Info> = std::sync::OnceLock::new();
    I.get_or_init(|| serde_json::from_str(INFO_JSON).expect("ecosistemas.json dañado"))
}

/// Texto de la fuente de un índice.
pub fn fuente(k: u8) -> &'static str {
    info().fuentes.get(k as usize).map_or("?", |f| f.cita.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAIS: &[u8] = include_bytes!("../../assets/mapas/uruguay/ecosistemas.bin");
    const ENTORNO: &[u8] = include_bytes!("../../assets/mapas/montevideo/ecosistemas.bin");
    const AMPLIO: &[u8] = include_bytes!("../../assets/mapas/montevideo/ecosistemas_amplio.bin");

    #[test]
    fn codigos_estables_y_leyenda_completa() {
        // Los 18 códigos del atlas, con sus números fijos, más las extensiones del juego.
        let atlas = ["PR01", "PR02", "PR03", "PR04", "SH01", "FO01", "FO02", "FO03", "FO04", "FO05", "PA01", "PA02", "HU01", "AQ01", "CO01", "SA01", "AN01", "AN02"];
        for (k, c) in atlas.iter().enumerate() {
            let cl = clase(k as u8 + 1).unwrap();
            assert_eq!(cl.codigo, *c);
            assert!(cl.atlas);
        }
        assert_eq!(LEYENDA.iter().filter(|c| c.atlas).count(), 18);
        assert_eq!((clase(PL01).unwrap().codigo, clase(UR01).unwrap().codigo, clase(TR01).unwrap().codigo), ("PL01", "UR01", "TR01"));
        // El JSON del constructor tiene la misma leyenda.
        let i = info();
        assert_eq!(i.leyenda.len(), LEYENDA.len());
        for (j, c) in i.leyenda.iter().zip(LEYENDA.iter()) {
            assert_eq!((j.n, j.codigo.as_str(), j.nombre.as_str(), j.atlas), (c.n, c.codigo, c.nombre, c.atlas));
        }
        // Números sin repetir.
        let mut ns: Vec<u8> = LEYENDA.iter().map(|c| c.n).collect();
        ns.sort();
        ns.dedup();
        assert_eq!(ns.len(), LEYENDA.len());
        // Cada capa del JSON está en el archivo, con los mismos años.
        let g = Grilla::leer(PAIS).unwrap();
        assert_eq!(g.capas.len(), i.capas.len());
        for (c, ci) in g.capas.iter().zip(&i.capas) {
            assert_eq!((c.desde, c.hasta), (ci.desde, ci.hasta));
        }
    }

    /// Todas las celdas de todas las capas de las tres grillas.
    fn todas(g: &Grilla) -> impl Iterator<Item = (usize, i64, i64, Celda)> + '_ {
        (0..g.capas.len()).flat_map(move |k| (0..g.alto as i64).flat_map(move |z| (0..g.ancho as i64).map(move |x| (k, x, z, g.celda(k, x, z).unwrap()))))
    }

    #[test]
    fn reglas_de_integridad() {
        let nf = info().fuentes.len() as u8;
        for (nombre, b) in [("país", PAIS), ("entorno", ENTORNO), ("amplio", AMPLIO)] {
            let g = Grilla::leer(b).unwrap();
            let base = g.capa_de(1750);
            assert_eq!(g.capa_de(1726), base, "{nombre}: el juego empieza con la base");
            let mut plantadas = 0;
            for (k, _, _, c) in todas(&g) {
                // Evidencia y confianza siempre presentes y válidas; fuente conocida.
                assert!(c.evidencia <= 2 && c.confianza <= 2 && c.fuente < nf && c.densidad <= 5, "{nombre}: {c:?}");
                assert!(c.prim == 0 || clase(c.prim).is_some(), "{nombre}: código {}", c.prim);
                assert!(c.sec == 0 || clase(c.sec).is_some(), "{nombre}: secundario {}", c.sec);
                let periodo = &g.capas[k];
                // Ninguna plantación (ni área urbana ni cobertura transformada) antes de 1900.
                if periodo.desde < 1900 {
                    assert!(!matches!(c.prim, PL01 | UR01 | TR01) && !matches!(c.sec, PL01 | UR01 | TR01), "{nombre}: {c:?} en {}", periodo.desde);
                }
                // La sabana arbolada (AN01) no se asigna sin una fuente ecológica específica.
                assert!(!c.tiene(AN01));
                // Un desconocido nunca es de confianza alta; lo documentado tiene fuente.
                assert!(!(c.evidencia == 0 && c.confianza == 2));
                if c.evidencia == 2 {
                    assert!(c.fuente > 0);
                }
                if c.prim == PL01 && periodo.desde >= 1900 {
                    plantadas += 1;
                }
                // Lo plantado no se mezcla con lo nativo: una celda plantada no tiene dosel de
                // monte nativo.
                if c.prim == PL01 {
                    assert_eq!(c.dosel_dm, 0);
                }
            }
            if nombre != "entorno" {
                assert!(plantadas > 0, "{nombre}: la forestación de Canelones está desde 1900");
            }
        }
    }

    #[test]
    fn forestacion_de_canelones_solo_desde_su_periodo() {
        let g = Grilla::leer(PAIS).unwrap();
        let cuenta = |anio: i32| {
            let k = g.capa_de(anio);
            (0..g.alto as i64).flat_map(|z| (0..g.ancho as i64).map(move |x| (x, z))).filter(|&(x, z)| g.celda(k, x, z).unwrap().prim == PL01).count()
        };
        assert_eq!(cuenta(1750), 0);
        assert_eq!(cuenta(1899), 0);
        // Más de 800 ha en el entorno de Las Toscas y Parque del Plata (celdas de ~1 km²).
        let n = cuenta(1910);
        assert!((8..=20).contains(&n), "{n} celdas plantadas");
        assert_eq!(cuenta(1960), n);
        // Todas en la costa de Canelones, entre Atlántida y La Floresta.
        let k = g.capa_de(1910);
        for z in 0..g.alto as i64 {
            for x in 0..g.ancho as i64 {
                if g.celda(k, x, z).unwrap().prim == PL01 {
                    let (lon, lat) = (-58.5 + x as f64 * 0.0107, -30.05 - z as f64 * 0.009);
                    assert!((-55.80..-55.65).contains(&lon) && (-34.80..-34.72).contains(&lat), "{lon} {lat}");
                }
            }
        }
    }

    #[test]
    fn palmares_en_sus_regiones() {
        let g = Grilla::leer(PAIS).unwrap();
        let geo = crate::pais::datos::geo();
        let base = g.capa_de(1750);
        let (mut butia, mut yatay) = (0, 0);
        for z in 0..g.alto as i64 {
            for x in 0..g.ancho as i64 {
                let c = g.celda(base, x, z).unwrap();
                let depto = geo.nombre_depto(geo.depto(x as f32, z as f32)).unwrap_or("");
                if c.tiene(PA01) {
                    butia += (c.prim == PA01) as usize;
                    // Butiá: este (Rocha, cuenca de la Laguna Merín).
                    assert_eq!(depto, "Rocha", "butiá en {x},{z}");
                }
                if c.tiene(PA02) {
                    yatay += (c.prim == PA02) as usize;
                    // Yatay: litoral noroeste (Paysandú y Río Negro).
                    assert!(matches!(depto, "Paysandú" | "Río Negro"), "yatay en {depto}");
                }
            }
        }
        // ~70.000 ha de butiá (Castillos y San Luis); el yatay ocupa pocos km² (27 km² en 2013).
        assert!((600..=800).contains(&butia), "{butia} celdas de butiá");
        assert!((10..=40).contains(&yatay), "{yatay} celdas de yatay");
        // Las quebradas documentadas (Lunarejo, Valle Edén, Quebrada de los Cuervos) tienen
        // monte de quebrada cerca.
        for (lon, lat) in [(-55.90, -31.13), (-56.16, -31.82), (-54.46, -32.93)] {
            let (cx, cz) = (((lon + 58.5) / 0.0107f64).round() as i64, ((-30.05 - lat) / 0.009f64).round() as i64);
            let hay = (-4..=4).any(|dz| (-4..=4).any(|dx| g.celda(base, cx + dx, cz + dz).unwrap().prim == FO02));
            assert!(hay, "quebrada en {lon} {lat}");
        }
    }

    #[test]
    fn la_lectura_es_determinista() {
        let a = Grilla::leer(PAIS).unwrap();
        let b = Grilla::leer(PAIS).unwrap();
        for k in 0..a.capas.len() {
            assert_eq!(a.capas[k].datos, b.capas[k].datos);
        }
        // Mismo archivo, mismo resumen (huella simple de todas las celdas de la base).
        let h = |g: &Grilla| g.capas[0].datos.iter().fold(0u64, |h, &v| h.wrapping_mul(1_000_003).wrapping_add(v as u64));
        assert_eq!(h(&a), h(&b));
    }
}
