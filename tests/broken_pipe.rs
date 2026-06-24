//! Tests d'intégration : robustesse face à une sortie pipée et fermée en avance
//! (`mnemo … | head`). La sortie ne doit jamais provoquer de panic Rust
//! (`failed printing to stdout: Broken pipe`, code 101), mais se terminer
//! proprement, comme tout outil Unix standard.

#![cfg(unix)]

use std::process::Command;

/// Exécute `"<mnemo>" <args…> | head -n<lines>` via `sh` et renvoie stderr.
///
/// Le chemin du binaire est passé par variable d'environnement pour éviter tout
/// problème de quoting dans la commande `sh -c`.
fn run_piped_to_head(args: &str, lines: u32) -> String {
    let script = format!("\"$MNEMO_BIN\" {args} | head -n{lines}");
    let output = Command::new("sh")
        .env("MNEMO_BIN", env!("CARGO_BIN_EXE_mnemo"))
        .args(["-c", &script])
        .output()
        .expect("exécution de sh");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn completions_pipe_vers_head_ne_panique_pas() {
    // `completions bash` produit une sortie volumineuse : `head -n1` ferme le
    // tube après la première ligne. Avant correctif, `generate` paniquait.
    let stderr = run_piped_to_head("completions bash", 1);
    assert!(
        !stderr.contains("panicked"),
        "mnemo a paniqué sur un tube fermé : {stderr}"
    );
    assert!(
        !stderr.contains("Broken pipe"),
        "fuite d'un message BrokenPipe : {stderr}"
    );
}

#[test]
fn version_pipe_vers_head_ne_panique_pas() {
    // `version` écrit plusieurs lignes ; `head -n1` ferme le tube après la
    // première. Avant correctif, le second `println!` paniquait.
    let stderr = run_piped_to_head("version", 1);
    assert!(
        !stderr.contains("panicked"),
        "mnemo a paniqué sur un tube fermé : {stderr}"
    );
    assert!(
        !stderr.contains("Broken pipe"),
        "fuite d'un message BrokenPipe : {stderr}"
    );
}
