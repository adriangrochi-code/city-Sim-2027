//! Partidas guardadas y opciones del juego, en la carpeta de datos del usuario:
//! - Windows: `%APPDATA%\CitySim2027\` · Linux y otros: `~/.local/share/city-sim-2027/`
//!   (o `$XDG_DATA_HOME/city-sim-2027/`); la variable `CITYSIM_DATOS` la reemplaza.
//! - `partidas/<ciudad>.json`: una partida por archivo (JSON con número de formato).
//! - `opciones.json`: escala de la interfaz, sincronización vertical, FPS, calidad, autoguardado.
//!
//! Una partida guarda el estado de la ciudad ([`CityData`]), las herramientas que importan y la
//! cámara; los vóxeles se regeneran al cargar ([`crate::city::City::rebuild_edits`]).

use crate::buildings::Edits;
use crate::city::game::{Game, Tool, TOOLS};
use crate::city::lots::{Density, ZoneKind};
use crate::city::save::{kind_from, kind_key, CityData};
use crate::terrain::TerrainGen;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Versión del formato de las partidas (sube si cambia de forma incompatible).
pub const FORMAT: u32 = 1;
pub const GAME_ID: &str = "City Sim 2027";
const EXT: &str = "json";
const AUTO_SUFFIX: &str = "-autoguardado";

// --- Carpetas ---------------------------------------------------------------------------------

/// Carpeta de datos del usuario (sin crearla).
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("CITYSIM_DATOS").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA").filter(|d| !d.is_empty()) {
            return PathBuf::from(a).join("CitySim2027");
        }
    }
    if let Some(x) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(x).join("city-sim-2027");
    }
    if let Some(h) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|d| !d.is_empty()) {
        return PathBuf::from(h).join(".local").join("share").join("city-sim-2027");
    }
    PathBuf::from("datos-city-sim-2027")
}

pub fn saves_dir(base: &Path) -> PathBuf {
    base.join("partidas")
}

/// Nombre de archivo a partir del nombre de la ciudad: minúsculas, sin acentos, guiones.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars().flat_map(|c| c.to_lowercase()) {
        let c = match ch {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        };
        if c == '-' && (out.is_empty() || out.ends_with('-')) {
            continue;
        }
        out.push(c);
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(60);
    if out.is_empty() {
        "partida".into()
    } else {
        out
    }
}

pub fn path_for(dir: &Path, name: &str, auto: bool) -> PathBuf {
    dir.join(format!("{}{}.{EXT}", slug(name), if auto { AUTO_SUFFIX } else { "" }))
}

/// Un archivo libre para una partida nueva con ese nombre ("montevideo-2.json" si ya hay una).
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let base = slug(name);
    let mut p = dir.join(format!("{base}.{EXT}"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{base}-{n}.{EXT}"));
        n += 1;
    }
    p
}

// --- Partidas ---------------------------------------------------------------------------------

/// Herramientas y preferencias del juego que vale la pena conservar.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ToolsData {
    pub tipo_de_calle: String,
    pub pincel: f32,
    pub zona: u8,
    pub densidad: u8,
    pub angulo_plaza: f32,
    pub velocidad: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SaveFile {
    pub formato: u32,
    pub juego: String,
    /// Versión del motor que guardó la partida (ver `version`); vacía en las partidas viejas.
    #[serde(default)]
    pub version: String,
    pub nombre: String,
    /// Segundos desde 1970 (UTC) en que se guardó.
    pub guardada: u64,
    #[serde(default)]
    pub autoguardado: bool,
    /// Resumen para el listado (también está dentro de `ciudad`).
    pub fecha: String,
    pub poblacion: i32,
    pub dinero: i64,
    /// Cámara: x, z, distancia, giro, inclinación.
    pub camara: [f32; 5],
    pub hora: f32,
    pub herramientas: ToolsData,
    pub ciudad: CityData,
}

/// Solo la cabecera (para el listado; los campos de más se ignoran).
#[derive(Deserialize)]
struct Header {
    formato: u32,
    nombre: String,
    #[serde(default)]
    guardada: u64,
    #[serde(default)]
    autoguardado: bool,
    #[serde(default)]
    fecha: String,
    #[serde(default)]
    poblacion: i32,
    #[serde(default)]
    dinero: i64,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Copia de la partida en curso para escribirla.
pub fn snapshot(game: &Game, cam: [f32; 5], hour: f32, auto: bool) -> SaveFile {
    SaveFile {
        formato: FORMAT,
        juego: GAME_ID.into(),
        version: crate::version::completa(),
        nombre: game.city_name.clone(),
        guardada: now_secs(),
        autoguardado: auto,
        fecha: game.city.date(),
        poblacion: game.city.population(),
        dinero: game.city.money,
        camara: cam,
        hora: hour,
        herramientas: ToolsData { tipo_de_calle: kind_key(game.kind).into(), pincel: game.brush, zona: game.zone_kind as u8, densidad: game.density as u8, angulo_plaza: game.plaza_angle, velocidad: game.speed },
        ciudad: CityData::from_city(&game.city),
    }
}

/// Escribe la partida (primero a un archivo temporal y después lo renombra: un corte de luz
/// no deja una partida a medias).
pub fn write(path: &Path, f: &SaveFile) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("no se pudo crear la carpeta {}: {e}", d.display()))?;
    }
    let json = serde_json::to_vec(f).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("no se pudo escribir {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("no se pudo guardar {}: {e}", path.display()))
}

pub fn read(path: &Path) -> Result<SaveFile, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
    let h: Header = serde_json::from_slice(&bytes).map_err(|e| format!("no es una partida válida: {e}"))?;
    if h.formato > FORMAT {
        return Err(format!("la partida es de una versión más nueva del juego (formato {})", h.formato));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("la partida está dañada: {e}"))
}

/// Rehace el juego guardado y los vóxeles de la ciudad.
pub fn restore(f: &SaveFile, t: &TerrainGen) -> Result<(Game, Edits), String> {
    let mut g = Game::default();
    g.city_name = f.nombre.clone();
    g.city = f.ciudad.to_city()?;
    let h = &f.herramientas;
    g.kind = kind_from(&h.tipo_de_calle)?;
    g.brush = h.pincel.clamp(10.0, 150.0);
    g.zone_kind = ZoneKind::ALL[(h.zona as usize).min(2)];
    g.density = Density::ALL[(h.densidad as usize).min(2)];
    g.plaza_angle = h.angulo_plaza;
    g.speed = h.velocidad.clamp(0.0, 4.0);
    // Fundada la ciudad, se empieza inspeccionando (sin trazar nada por accidente).
    let inspect = TOOLS.iter().position(|t| t.0 == Tool::Inspect).unwrap();
    if g.city.plaza.is_some() {
        g.set_tool(inspect);
    }
    let e = g.city.rebuild_edits(t);
    g.say(format!("Partida cargada: {}, {}", f.nombre, g.city.date()));
    Ok((g, e))
}

/// Una partida en el listado de «Cargar partida».
#[derive(Clone, Debug, PartialEq)]
pub struct SaveInfo {
    pub path: PathBuf,
    pub name: String,
    pub date: String,
    pub population: i32,
    pub money: i64,
    pub saved: u64,
    pub auto: bool,
    /// Si no se puede leer (versión nueva, archivo dañado), el motivo.
    pub error: Option<String>,
}

/// Partidas de la carpeta, la más reciente primero (las que no se pueden leer, al final).
pub fn list(dir: &Path) -> Vec<SaveInfo> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
            continue;
        }
        let mtime = ent.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let info = match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Header>(&b).map_err(|e| e.to_string())) {
            Ok(h) => SaveInfo {
                error: (h.formato > FORMAT).then(|| "De una versión más nueva del juego".to_string()),
                path,
                name: h.nombre,
                date: h.fecha,
                population: h.poblacion,
                money: h.dinero,
                saved: if h.guardada > 0 { h.guardada } else { mtime },
                auto: h.autoguardado,
            },
            Err(_) => SaveInfo { path, name: stem, date: String::new(), population: 0, money: 0, saved: mtime, auto: false, error: Some("Archivo dañado o de otro programa".into()) },
        };
        out.push(info);
    }
    // Las que se pueden cargar primero, de la más reciente a la más vieja; las dañadas al final.
    out.sort_by(|a, b| a.error.is_some().cmp(&b.error.is_some()).then(b.saved.cmp(&a.saved)).then_with(|| a.name.cmp(&b.name)));
    out
}

/// La partida más reciente que se puede cargar.
#[cfg(test)]
pub fn latest(dir: &Path) -> Option<SaveInfo> {
    list(dir).into_iter().find(|s| s.error.is_none())
}

pub fn delete(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| format!("no se pudo borrar {}: {e}", path.display()))
}

/// Cuánto hace que se guardó: «hace 5 minutos», «ayer», o la fecha (UTC) si pasó una semana.
pub fn ago(now: u64, then: u64) -> String {
    let d = now.saturating_sub(then);
    let plural = |n: u64, one: &str, many: &str| format!("hace {n} {}", if n == 1 { one } else { many });
    match d {
        0..=59 => "hace un momento".into(),
        60..=3599 => plural(d / 60, "minuto", "minutos"),
        3600..=86_399 => plural(d / 3600, "hora", "horas"),
        86_400..=172_799 => "ayer".into(),
        172_800..=604_799 => plural(d / 86_400, "día", "días"),
        _ => {
            let (y, m, day) = civil_from_days((then / 86_400) as i64);
            format!("el {day}/{m}/{y}")
        }
    }
}

/// Fecha del calendario a partir de los días desde 1970 (algoritmo de H. Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

// --- Opciones -------------------------------------------------------------------------------

/// Opciones del juego (se guardan al aceptar en el menú de opciones).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Multiplica la escala automática de la interfaz (0,75 a 1,5).
    pub escala_interfaz: f32,
    pub sincronizacion_vertical: bool,
    pub mostrar_fps: bool,
    /// Niveles de detalle del mundo (9 = vista corta: el mapa; 11 = larga: el entorno amplio
    /// hasta el horizonte). Se aplica al empezar o cargar una partida.
    pub niveles: u32,
    /// Suavizado de bordes: 0 = automático, 1 = desactivado, 4 = 4×. Se aplica al reiniciar.
    pub msaa: u32,
    /// Autoguardado cada tantos meses de juego (0 = nunca).
    pub autoguardado_meses: u32,
    /// Lado del vóxel base (cm) con el que se eligieron los `niveles`. Las opciones guardadas
    /// antes del vóxel de 25 cm no lo tienen (eran de 50 cm): ahí hace falta un nivel más
    /// para ver igual de lejos.
    #[serde(default = "voxel_cm_viejo")]
    pub voxel_cm: u32,
    /// Niveles de detalle del motor ("larga") cuando se eligieron los `niveles`. Con el entorno
    /// amplio pasaron de 9 a 11: quien tenía "larga" sigue teniendo "larga" (las opciones de
    /// antes no tienen este campo: eran de 9).
    #[serde(default = "niveles_max_viejo")]
    pub niveles_max: u32,
    /// Sombras: cantidad de cascadas (2 a 4) y resolución de cada una (1024, 2048 o 4096).
    /// Se aplican enseguida.
    pub sombras_cascadas: u32,
    pub sombras_resolucion: u32,
}

fn voxel_cm_viejo() -> u32 {
    50
}

fn niveles_max_viejo() -> u32 {
    9
}

fn voxel_cm() -> u32 {
    (crate::lod::BASE_VOXEL * 100.0).round() as u32
}

impl Default for Settings {
    fn default() -> Self {
        let q = crate::render::ShadowQuality::DEFAULT;
        Settings { escala_interfaz: 1.0, sincronizacion_vertical: true, mostrar_fps: false, niveles: crate::lod::LEVELS, msaa: 0, autoguardado_meses: 12, voxel_cm: voxel_cm(), niveles_max: crate::lod::LEVELS, sombras_cascadas: q.cascades, sombras_resolucion: q.resolution }
    }
}

impl Settings {
    /// Corrige valores fuera de rango (un archivo editado a mano).
    pub fn sanitized(mut self) -> Settings {
        self.escala_interfaz = if self.escala_interfaz.is_finite() { self.escala_interfaz.clamp(0.75, 1.5) } else { 1.0 };
        // Opciones de antes de achicar el vóxel: la misma distancia de dibujo lleva más niveles.
        let mut cm = self.voxel_cm.max(1);
        while cm > voxel_cm() {
            self.niveles += 1;
            cm /= 2;
        }
        while cm < voxel_cm() && self.niveles > 0 {
            self.niveles -= 1;
            cm *= 2;
        }
        self.voxel_cm = voxel_cm();
        // Más niveles en el motor: la misma elección (corta, media, larga) sube con ellos.
        if self.niveles_max < crate::lod::LEVELS {
            self.niveles += crate::lod::LEVELS - self.niveles_max;
        }
        self.niveles_max = crate::lod::LEVELS;
        let max = crate::lod::LEVELS;
        self.niveles = self.niveles.clamp(max - 3, max);
        if !matches!(self.msaa, 0 | 1 | 4) {
            self.msaa = 0;
        }
        let q = self.shadow_quality();
        (self.sombras_cascadas, self.sombras_resolucion) = (q.cascades, q.resolution);
        self
    }

    pub fn shadow_quality(&self) -> crate::render::ShadowQuality {
        crate::render::ShadowQuality { cascades: self.sombras_cascadas, resolution: self.sombras_resolucion }.sanitized()
    }

    pub fn load(base: &Path) -> Settings {
        std::fs::read(base.join("opciones.json")).ok().and_then(|b| serde_json::from_slice::<Settings>(&b).ok()).unwrap_or_default().sanitized()
    }

    pub fn store(&self, base: &Path) -> Result<(), String> {
        std::fs::create_dir_all(base).map_err(|e| format!("no se pudo crear {}: {e}", base.display()))?;
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(base.join("opciones.json"), json).map_err(|e| format!("no se pudieron guardar las opciones: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("citysim-prueba-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn nombres_de_archivo() {
        assert_eq!(slug("San Felipe de Montevideo"), "san-felipe-de-montevideo");
        assert_eq!(slug("  Ñandú / Colonia  "), "nandu-colonia");
        assert_eq!(slug("¿?"), "partida");
        let d = tmp_dir("nombres");
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(unique_path(&d, "Montevideo").file_name().unwrap(), "montevideo.json");
        std::fs::write(d.join("montevideo.json"), "{}").unwrap();
        assert_eq!(unique_path(&d, "Montevideo").file_name().unwrap(), "montevideo-2.json");
        assert_eq!(path_for(&d, "Montevideo", true).file_name().unwrap(), "montevideo-autoguardado.json");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn fechas_relativas() {
        assert_eq!(ago(1000, 990), "hace un momento");
        assert_eq!(ago(10_000, 10_000 - 120), "hace 2 minutos");
        assert_eq!(ago(10_000, 10_000 - 3700), "hace 1 hora");
        assert_eq!(ago(1_000_000, 1_000_000 - 90_000), "ayer");
        // 17 de noviembre de 2026 a mediodía (UTC).
        assert_eq!(ago(1_900_000_000, 1_794_916_800), "el 17/11/2026");
    }

    #[test]
    fn guardar_listar_cargar_y_borrar() {
        let t = TerrainGen::new(1726);
        let d = tmp_dir("partidas");
        let mut g = Game::default();
        g.city_name = "Villa de Prueba".into();
        g.city.money = 500_000;
        g.city.found_plaza(&t, Vec2::new(13500.0, 16300.0), 0.3).unwrap();
        g.kind = crate::city::roads::RoadKind::Stone;
        g.brush = 70.0;
        g.city.fast_forward(&t, 3);
        let path = unique_path(&d, &g.city_name);
        write(&path, &snapshot(&g, [1.0, 2.0, 300.0, 10.0, 40.0], 9.0, false)).unwrap();
        let mut old = snapshot(&g, [0.0; 5], 9.0, true);
        old.guardada -= 3600;
        old.nombre = "Otra".into();
        write(&path_for(&d, "Otra", true), &old).unwrap();
        std::fs::write(d.join("basura.json"), "no soy una partida").unwrap();
        let l = list(&d);
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].name, "Villa de Prueba", "la más reciente primero");
        assert_eq!(l[0].date, g.city.date());
        assert!(l[1].auto);
        assert!(l[2].error.is_some(), "la basura se lista con el motivo");
        assert_eq!(latest(&d).unwrap().path, path);
        let f = read(&path).unwrap();
        assert_eq!(f.camara[2], 300.0);
        let (g2, e) = restore(&f, &t).unwrap();
        assert_eq!(g2.city_name, "Villa de Prueba");
        assert_eq!(g2.kind, crate::city::roads::RoadKind::Stone);
        assert_eq!(g2.brush, 70.0);
        assert_eq!(g2.city.money, g.city.money);
        assert_eq!(g2.city.net.edges.len(), 4);
        assert_eq!(g2.tool, Tool::Inspect);
        assert!(e.voxel_count() > 10_000, "plaza, iglesia y calles regeneradas");
        // Una partida de un formato futuro no se carga.
        let mut fut = f.clone();
        fut.formato = FORMAT + 1;
        write(&d.join("futura.json"), &fut).unwrap();
        assert!(read(&d.join("futura.json")).unwrap_err().contains("más nueva"));
        delete(&path).unwrap();
        assert!(list(&d).iter().all(|s| s.path != path));
        // Opciones: valores por omisión, ida y vuelta, y corrección de lo que está fuera de rango.
        assert_eq!(Settings::load(&d), Settings::default());
        let s = Settings { escala_interfaz: 1.25, mostrar_fps: true, niveles: crate::lod::LEVELS - 3, ..Default::default() };
        s.store(&d).unwrap();
        assert_eq!(Settings::load(&d), s);
        std::fs::write(d.join("opciones.json"), r#"{"escala_interfaz": 9, "msaa": 3, "sombras_cascadas": 7, "sombras_resolucion": 100}"#).unwrap();
        let s = Settings::load(&d);
        assert_eq!((s.escala_interfaz, s.msaa, s.niveles), (1.5, 0, crate::lod::LEVELS));
        assert_eq!((s.sombras_cascadas, s.sombras_resolucion), (4, 2048));
        // Opciones de antes del vóxel de 25 cm: "media" (7 niveles de 0,5 m) sigue siendo media.
        std::fs::write(d.join("opciones.json"), r#"{"niveles": 7}"#).unwrap();
        assert_eq!(Settings::load(&d).niveles, crate::lod::LEVELS - 1);
        // Opciones de antes del entorno amplio (9 niveles = "larga"): siguen en "larga".
        std::fs::write(d.join("opciones.json"), r#"{"niveles": 9, "voxel_cm": 25}"#).unwrap();
        assert_eq!(Settings::load(&d).niveles, crate::lod::LEVELS);
        std::fs::write(d.join("opciones.json"), r#"{"niveles": 7, "voxel_cm": 25}"#).unwrap();
        assert_eq!(Settings::load(&d).niveles, crate::lod::MAP_LEVELS);
        let _ = std::fs::remove_dir_all(&d);
    }
}
