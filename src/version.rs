//! Commande `mnemo version` : affiche des informations détaillées sur le build.

/// Architecture cible (déduite à la compilation).
const TARGET_ARCH: &str = std::env::consts::ARCH;
/// Système d'exploitation cible.
const TARGET_OS: &str = std::env::consts::OS;

/// Profil de compilation : `release` ou `debug`.
fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

/// Chemin du binaire courant, si disponible.
fn binary_path() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
}

/// Affiche le rapport de version.
pub fn run() {
    println!("mnemo {}", env!("CARGO_PKG_VERSION"));
    println!("  cible   : {TARGET_OS}/{TARGET_ARCH}");
    println!("  profil  : {}", build_profile());
    match binary_path() {
        Some(p) => println!("  binaire : {p}"),
        None => println!("  binaire : (indisponible)"),
    }
}
