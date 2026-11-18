//! Datos del mapa del país (escala país): el Uruguay a ~1 km, armado por
//! `tools/mapa_real/construir_pais.py` con el MDT de la IDE, la hidrografía, los departamentos,
//! las vías y los puertos del MTOP, los bañados de la IDE y el monte nativo de la DGF, más la
//! historia (pueblos con su año de fundación, caminos, puertos, estancias y saladeros) de
//! `historia_pais.py`. Todo va embebido en el ejecutable y se lee una sola vez.
//!
//! Coordenadas: **celdas** de la grilla (columna, fila), con decimales. La celda (c, f) es el
//! punto lon = −58,5 + c · 0,0107, lat = −30,05 − f · 0,009 (la misma grilla que la geología del
//! país, `assets/datos/uruguay/geologia_pais.bin`). Una celda mide ~1 km de lado (`km_x`, `km_y`).

use serde::Deserialize;
use std::sync::OnceLock;

const RELIEVE_BIN: &[u8] = include_bytes!("../../../assets/mapas/uruguay/relieve.bin");
const CAPAS_BIN: &[u8] = include_bytes!("../../../assets/mapas/uruguay/capas.bin");
const MAPA_JSON: &str = include_str!("../../../assets/mapas/uruguay/mapa.json");
const HISTORIA_JSON: &str = include_str!("../../../assets/mapas/uruguay/historia.json");
const GEOLOGIA_BIN: &[u8] = include_bytes!("../../../assets/datos/uruguay/geologia_pais.bin");
const GEOLOGIA_JSON: &str = include_str!("../../../assets/datos/uruguay/pais.json");
const TITULOS_JSON: &str = include_str!("../../../assets/datos/uruguay/titulos_mineros.json");

/// Canales de `capas.bin` (8 bytes por celda).
pub const C_AGUA: usize = 0;
pub const C_EMBALSE: usize = 1;
pub const C_DEPTO: usize = 2;
pub const C_BANADO: usize = 3;
pub const C_MONTE: usize = 4;
pub const C_RIO: usize = 5;
pub const C_PENDIENTE: usize = 6;

#[derive(Deserialize, Clone, Debug)]
pub struct Grilla {
    pub lon_oeste: f64,
    pub lat_norte: f64,
    pub dlon: f64,
    pub dlat: f64,
    pub columnas: usize,
    pub filas: usize,
    pub km_x: f32,
    pub km_y: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Departamento {
    pub id: u8,
    pub nombre: String,
    pub celdas: u32,
    pub centro: [f32; 2],
    pub contorno: Vec<Vec<[f32; 2]>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Embalse {
    pub indice: u8,
    pub nombre: String,
    pub anio: i32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rio {
    pub nombre: String,
    pub navegable: bool,
    pub largo_km: f32,
    pub lineas: Vec<Vec<[f32; 2]>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Laguna {
    pub nombre: String,
    pub pos: [f32; 2],
    pub km2: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Via {
    pub linea: String,
    pub activa: bool,
    pub anio: i32,
    pub nota: String,
    pub puntos: Vec<[f32; 2]>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Estacion {
    pub nombre: String,
    pub activa: bool,
    pub pos: [f32; 2],
}

#[derive(Deserialize, Clone, Debug)]
pub struct Ciudad {
    pub celda_no: [f32; 2],
    pub celda_se: [f32; 2],
}

#[derive(Deserialize, Clone, Debug)]
struct MapaJson {
    grilla: Grilla,
    ciudad: Ciudad,
    departamentos: Vec<Departamento>,
    embalses: Vec<Embalse>,
    rios: Vec<Rio>,
    lagunas: Vec<Laguna>,
    vias: Vec<Via>,
    estaciones: Vec<Estacion>,
    atribucion: String,
}

/// Un pueblo de la historia (con su año de fundación y la fuente).
#[derive(Deserialize, Clone, Debug)]
pub struct PuebloHist {
    pub id: String,
    pub nombre: String,
    pub anio: i32,
    pub tipo: String,
    pub pos: [f32; 2],
    pub departamento: Option<String>,
    pub poblacion_inicial: f32,
    pub estado: String,
    pub fuente: Option<String>,
    pub nota: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CaminoHist {
    pub desde: String,
    pub hasta: String,
    pub nombre: String,
    pub anio: i32,
    pub puntos: Vec<[f32; 2]>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct PuertoHist {
    pub pueblo: String,
    pub desde: i32,
    pub tipo: String,
    pub estado: String,
    pub nota: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LugarHist {
    pub id: String,
    pub nombre: String,
    pub anio: i32,
    pub pos: [f32; 2],
    pub estado: String,
    pub nota: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DeptoHist {
    pub nombre: String,
    pub creado: Option<i32>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Historia {
    pub pueblos: Vec<PuebloHist>,
    pub caminos: Vec<CaminoHist>,
    pub puertos: Vec<PuertoHist>,
    pub estancias: Vec<LugarHist>,
    pub saladeros: Vec<LugarHist>,
    pub departamentos: Vec<DeptoHist>,
}

#[derive(Deserialize)]
struct UnidadJson {
    codigo: u32,
    unidad: String,
    #[serde(default)]
    materiales: std::collections::BTreeMap<String, f32>,
}

#[derive(Deserialize)]
struct GeologiaJson {
    unidades: Vec<UnidadJson>,
}

#[derive(Deserialize)]
struct TituloJson {
    lon: f64,
    lat: f64,
    #[serde(default)]
    materiales: Vec<String>,
}

#[derive(Deserialize)]
struct TitulosJson {
    titulos: Vec<TituloJson>,
}

/// Unidad geológica (DINAMIGE) con el material principal que da (del catálogo de recursos).
#[derive(Clone, Debug)]
pub struct Unidad {
    pub nombre: String,
    /// (material, peso 0..1), el de más peso primero.
    pub materiales: Vec<(String, f32)>,
}

/// Título minero (catastro de DINAMIGE): dónde y qué sustancia.
#[derive(Clone, Debug)]
pub struct Titulo {
    pub pos: [f32; 2],
    pub material: Option<String>,
}

pub struct Geografia {
    pub g: Grilla,
    /// Alturas (m) por celda, filas de norte a sur; agua < 0.
    pub alt: Vec<f32>,
    pub capas: Vec<[u8; 8]>,
    pub departamentos: Vec<Departamento>,
    pub embalses: Vec<Embalse>,
    pub rios: Vec<Rio>,
    pub lagunas: Vec<Laguna>,
    pub vias: Vec<Via>,
    pub estaciones: Vec<Estacion>,
    pub ciudad: Ciudad,
    pub atribucion: String,
    pub historia: Historia,
    /// Código de unidad geológica + 1 por celda (0 = fuera o agua).
    pub geologia: Vec<u8>,
    pub unidades: std::collections::HashMap<u32, Unidad>,
    pub titulos: Vec<Titulo>,
}

static DATOS: OnceLock<Geografia> = OnceLock::new();

/// Los datos del país (se leen la primera vez).
pub fn geo() -> &'static Geografia {
    DATOS.get_or_init(|| Geografia::cargar().expect("datos del país dañados"))
}

impl Geografia {
    fn cargar() -> Result<Geografia, String> {
        let m: MapaJson = serde_json::from_str(MAPA_JSON).map_err(|e| format!("mapa.json: {e}"))?;
        let historia: Historia = serde_json::from_str(HISTORIA_JSON).map_err(|e| format!("historia.json: {e}"))?;
        let n = m.grilla.columnas * m.grilla.filas;
        if RELIEVE_BIN.len() != n * 2 || CAPAS_BIN.len() != n * 8 {
            return Err(format!("tamaños: relieve {} capas {} para {n} celdas", RELIEVE_BIN.len(), CAPAS_BIN.len()));
        }
        let alt = RELIEVE_BIN.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]]) as f32 / 10.0 - 100.0).collect();
        let capas = CAPAS_BIN.chunks_exact(8).map(|b| b.try_into().unwrap()).collect();
        let geologia = if GEOLOGIA_BIN.len() == n { GEOLOGIA_BIN.to_vec() } else { vec![0; n] };
        let gj: GeologiaJson = serde_json::from_str(GEOLOGIA_JSON).map_err(|e| format!("pais.json: {e}"))?;
        let unidades = gj
            .unidades
            .into_iter()
            .map(|u| {
                let mut mats: Vec<(String, f32)> = u.materiales.into_iter().collect();
                mats.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
                (u.codigo, Unidad { nombre: u.unidad, materiales: mats })
            })
            .collect();
        let tj: TitulosJson = serde_json::from_str(TITULOS_JSON).map_err(|e| format!("titulos_mineros.json: {e}"))?;
        let g = m.grilla;
        let titulos = tj
            .titulos
            .into_iter()
            .map(|t| Titulo { pos: [((t.lon - g.lon_oeste) / g.dlon) as f32, ((g.lat_norte - t.lat) / g.dlat) as f32], material: t.materiales.into_iter().next() })
            .collect();
        Ok(Geografia {
            g,
            alt,
            capas,
            departamentos: m.departamentos,
            embalses: m.embalses,
            rios: m.rios,
            lagunas: m.lagunas,
            vias: m.vias,
            estaciones: m.estaciones,
            ciudad: m.ciudad,
            atribucion: m.atribucion,
            historia,
            geologia,
            unidades,
            titulos,
        })
    }

    pub fn ancho(&self) -> usize {
        self.g.columnas
    }

    pub fn alto(&self) -> usize {
        self.g.filas
    }

    fn idx(&self, c: f32, f: f32) -> Option<usize> {
        let (j, i) = (c.round() as i64, f.round() as i64);
        (j >= 0 && i >= 0 && (j as usize) < self.g.columnas && (i as usize) < self.g.filas).then(|| i as usize * self.g.columnas + j as usize)
    }

    /// Byte de un canal de `capas.bin` en la celda más cercana (0 fuera de la grilla).
    pub fn capa(&self, c: f32, f: f32, canal: usize) -> u8 {
        self.idx(c, f).map_or(0, |k| self.capas[k][canal])
    }

    /// Altura (m) interpolada.
    pub fn altura(&self, c: f32, f: f32) -> f32 {
        let w = self.g.columnas;
        let (c, f) = (c.clamp(0.0, (w - 1) as f32), f.clamp(0.0, (self.g.filas - 1) as f32));
        let (j, i) = (c.floor() as usize, f.floor() as usize);
        let (j1, i1) = ((j + 1).min(w - 1), (i + 1).min(self.g.filas - 1));
        let (tx, ty) = (c - j as f32, f - i as f32);
        let a = |i: usize, j: usize| self.alt[i * w + j];
        (a(i, j) * (1.0 - tx) + a(i, j1) * tx) * (1.0 - ty) + (a(i1, j) * (1.0 - tx) + a(i1, j1) * tx) * ty
    }

    /// ¿Agua en ese año? El agua natural siempre; un embalse solo desde el año de su represa.
    pub fn es_agua(&self, c: f32, f: f32, anio: i32) -> bool {
        let Some(k) = self.idx(c, f) else { return true };
        let b = self.capas[k];
        b[C_AGUA] >= 128 || (b[C_EMBALSE] > 0 && self.embalse_anio(b[C_EMBALSE]).is_some_and(|y| anio >= y))
    }

    pub fn embalse_anio(&self, indice: u8) -> Option<i32> {
        self.embalses.iter().find(|e| e.indice == indice).map(|e| e.anio)
    }

    /// Departamento (1..19) o 0 fuera del país.
    pub fn depto(&self, c: f32, f: f32) -> u8 {
        self.capa(c, f, C_DEPTO)
    }

    pub fn nombre_depto(&self, id: u8) -> Option<&str> {
        self.departamentos.iter().find(|d| d.id == id).map(|d| d.nombre.as_str())
    }

    /// Tierra del país (no agua del año, no fuera de frontera).
    pub fn es_tierra(&self, c: f32, f: f32, anio: i32) -> bool {
        self.depto(c, f) > 0 && !self.es_agua(c, f, anio)
    }

    /// Calidad de la pastura (0..1) para el ganado: pradera abierta y llana es lo mejor; el
    /// bañado, el monte cerrado y las sierras empinadas rinden menos. ESTIMADO.
    pub fn pastura(&self, c: f32, f: f32) -> f32 {
        let Some(k) = self.idx(c, f) else { return 0.0 };
        let b = self.capas[k];
        if b[C_DEPTO] == 0 || b[C_AGUA] >= 128 {
            return 0.0;
        }
        let ban = b[C_BANADO] as f32 / 255.0;
        let mon = b[C_MONTE] as f32 / 255.0;
        let pend = b[C_PENDIENTE] as f32 / 1000.0;
        (1.0 - 0.55 * ban - 0.4 * mon - (pend * 12.0).min(0.5)).clamp(0.1, 1.0)
    }

    /// Kilómetros entre dos puntos (en celdas).
    pub fn km(&self, a: [f32; 2], b: [f32; 2]) -> f32 {
        (((a[0] - b[0]) * self.g.km_x).powi(2) + ((a[1] - b[1]) * self.g.km_y).powi(2)).sqrt()
    }

    /// Celda de un punto geográfico.
    pub fn celda(&self, lon: f64, lat: f64) -> [f32; 2] {
        [((lon - self.g.lon_oeste) / self.g.dlon) as f32, ((self.g.lat_norte - lat) / self.g.dlat) as f32]
    }

    /// Longitud y latitud de una celda.
    pub fn lonlat(&self, p: [f32; 2]) -> (f64, f64) {
        (self.g.lon_oeste + p[0] as f64 * self.g.dlon, self.g.lat_norte - p[1] as f64 * self.g.dlat)
    }

    /// Unidad geológica de la celda.
    pub fn unidad(&self, c: f32, f: f32) -> Option<&Unidad> {
        let k = self.idx(c, f)?;
        let u = self.geologia[k];
        (u > 0).then(|| self.unidades.get(&(u as u32 - 1))).flatten()
    }

    /// Río con nombre que pasa por la celda (el más cercano a menos de `radio` celdas).
    pub fn rio_cerca(&self, p: [f32; 2], radio: f32) -> Option<(&Rio, f32)> {
        let mut mejor: Option<(&Rio, f32)> = None;
        for r in &self.rios {
            for l in &r.lineas {
                for w in l.windows(2) {
                    let d = dist_seg(p, w[0], w[1]);
                    if d < radio && mejor.map_or(true, |m| d < m.1) {
                        mejor = Some((r, d));
                    }
                }
            }
        }
        mejor
    }

    /// Pueblo de la historia por id.
    pub fn pueblo_hist(&self, id: &str) -> Option<&PuebloHist> {
        self.historia.pueblos.iter().find(|p| p.id == id)
    }

    /// Centro del rectángulo del mapa de la ciudad (Montevideo, escala ciudad).
    pub fn centro_ciudad(&self) -> [f32; 2] {
        [(self.ciudad.celda_no[0] + self.ciudad.celda_se[0]) * 0.5, (self.ciudad.celda_no[1] + self.ciudad.celda_se[1]) * 0.5]
    }
}

/// Distancia de un punto a un segmento (en celdas).
pub fn dist_seg(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = vx * vx + vy * vy;
    let t = if l2 < 1e-9 { 0.0 } else { (((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / l2).clamp(0.0, 1.0) };
    let (x, y) = (a[0] + vx * t - p[0], a[1] + vy * t - p[1]);
    (x * x + y * y).sqrt()
}

/// Largo de una polilínea (en celdas).
pub fn largo(pts: &[[f32; 2]]) -> f32 {
    pts.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum()
}
