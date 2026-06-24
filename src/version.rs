//! Commande `mnemo version` : affiche des informations détaillées sur le build.

use std::io::{self, Write};

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
///
/// L'écriture passe par un stdout verrouillé : si la sortie est pipée vers
/// `head`/`less` et que le tube est fermé en avance, le `BrokenPipe` remonte
/// comme une erreur propre (interceptée dans `main`) au lieu de faire paniquer
/// `println!`.
pub fn run() -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    write_report(&mut out)
}

/// Écrit le rapport de version sur un writer quelconque (testable).
fn write_report<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(out, "mnemo {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(out, "  cible   : {TARGET_OS}/{TARGET_ARCH}")?;
    writeln!(out, "  profil  : {}", build_profile())?;
    match binary_path() {
        Some(p) => writeln!(out, "  binaire : {p}")?,
        None => writeln!(out, "  binaire : (indisponible)")?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writer qui échoue systématiquement avec `BrokenPipe`, pour simuler une
    /// sortie pipée vers un consommateur qui ferme le tube (`| head`).
    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn write_report_ecrit_la_version() {
        let mut buf: Vec<u8> = Vec::new();
        write_report(&mut buf).expect("écriture en mémoire");
        let rendu = String::from_utf8(buf).expect("utf8");
        assert!(rendu.starts_with("mnemo "));
        assert!(rendu.contains("cible"));
    }

    #[test]
    fn write_report_remonte_broken_pipe_sans_paniquer() {
        let mut writer = BrokenPipeWriter;
        let err = write_report(&mut writer).expect_err("doit retourner une erreur");
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }
}
