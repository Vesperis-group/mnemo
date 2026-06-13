//! Tests d'intégration du binaire `mnemo` (mode non interactif).
//!
//! On isole entièrement l'exécution dans un HOME temporaire via HOME +
//! XDG_CONFIG_HOME / XDG_DATA_HOME, afin de ne jamais toucher aux données
//! réelles de l'utilisateur.

use std::path::Path;
use std::process::Command;

fn mnemo(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mnemo"));
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"));
    cmd
}

#[test]
fn search_print_non_interactif() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // init
    assert!(mnemo(home).arg("init").output().unwrap().status.success());

    // add plusieurs commandes, dont une sensible et la commande mnemo elle-même
    for args in [
        vec!["add", "--cmd", "cargo build --release", "--cwd", "/tmp"],
        vec!["add", "--cmd", "cargo test", "--cwd", "/tmp"],
        vec!["add", "--cmd", "git status", "--cwd", "/tmp"],
        vec!["add", "--cmd", "export TOKEN=secret123", "--cwd", "/tmp"],
        vec!["add", "--cmd", "mnemo search", "--cwd", "/tmp"],
    ] {
        assert!(mnemo(home).args(&args).output().unwrap().status.success());
    }

    // search --print sur "cargo"
    let out = mnemo(home)
        .args(["search", "cargo", "--print"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();

    assert_eq!(
        lines.len(),
        2,
        "deux commandes cargo attendues : {stdout:?}"
    );
    assert!(lines.iter().all(|l| l.contains("cargo")));

    // Le secret ne doit jamais apparaître.
    let all = mnemo(home).args(["search", "--print"]).output().unwrap();
    let all_out = String::from_utf8(all.stdout).unwrap();
    assert!(
        !all_out.contains("TOKEN"),
        "les secrets doivent être filtrés"
    );
    // La commande mnemo elle-même ne doit pas avoir été enregistrée.
    assert!(
        !all_out.lines().any(|l| l == "mnemo search"),
        "mnemo ne doit pas s'enregistrer lui-même"
    );
}

#[test]
fn search_query_option_equivaut_au_positionnel() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(mnemo(home).arg("init").output().unwrap().status.success());
    assert!(mnemo(home)
        .args(["add", "--cmd", "docker ps", "--cwd", "/tmp"])
        .output()
        .unwrap()
        .status
        .success());

    let out = mnemo(home)
        .args(["search", "--query", "docker", "--print"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("docker ps"));
}

#[test]
fn version_affiche_les_infos_de_build() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let out = mnemo(home).arg("version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
    assert!(stdout.contains("cible"));
    assert!(stdout.contains("profil"));
    assert!(stdout.contains("binaire"));
}
