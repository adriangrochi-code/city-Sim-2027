// Identifica la build: número de versión (Cargo.toml), commit de git y fecha de compilación.
// Así cada paquete de prueba dice exactamente de qué código salió.
use std::process::Command;

fn main() {
    let git = |args: &[&str]| Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let commit = git(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "sin-git".into());
    let sucio = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    let fecha = Command::new("date").args(["-u", "+%Y-%m-%d"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    println!("cargo:rustc-env=CITYSIM_COMMIT={commit}{}", if sucio { "+cambios" } else { "" });
    println!("cargo:rustc-env=CITYSIM_FECHA={fecha}");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/heads");
    println!("cargo:rerun-if-changed=../.git/index");
}
