//! Versión de la build (ver `build.rs` y `docs/VERSIONES.md`).

/// Número de versión del juego (Cargo.toml).
pub const NUMERO: &str = env!("CARGO_PKG_VERSION");
/// Commit de git del que salió la build (con "+cambios" si había cambios sin commitear).
pub const COMMIT: &str = env!("CITYSIM_COMMIT");
/// Fecha de compilación (UTC).
pub const FECHA: &str = env!("CITYSIM_FECHA");

/// "v0.9.0 (abc1234, 2026-11-18)": para el menú, el título, los informes y los avisos.
pub fn completa() -> String {
    format!("v{NUMERO} ({COMMIT}, {FECHA})")
}
