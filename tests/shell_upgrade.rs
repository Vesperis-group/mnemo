//! Tests d'intégration pour la mise à niveau de l'intégration Bash
//! (`mnemo shell upgrade`) et sa détection par `mnemo doctor`.
//!
//! Comme les autres suites, l'exécution est isolée dans un HOME temporaire via
//! HOME + XDG_CONFIG_HOME / XDG_DATA_HOME. La commande n'est jamais lancée comme
//! un wizard : aucune interaction n'est requise.

use std::path::{Path, PathBuf};
use std::process::Command;

const BLOCK_BEGIN: &str = "# >>> mnemo init >>>";

fn mnemo(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mnemo"));
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"));
    cmd
}

fn bashrc(home: &Path) -> PathBuf {
    home.join(".bashrc")
}

fn count_blocks(content: &str) -> usize {
    content.lines().filter(|l| l.trim() == BLOCK_BEGIN).count()
}

/// Bloc « legacy » : marqueurs présents, raccourci Ctrl+R présent, mais aucune
/// capture de `MNEMO_SESSION_ID` ni marqueur de version.
fn legacy_block() -> String {
    "# >>> mnemo init >>>\n\
     # >>> mnemo >>>\n\
     __mnemo_record() { :; }\n\
     bind -x '\"\\C-r\": __mnemo_search'\n\
     # <<< mnemo <<<\n\
     # <<< mnemo init <<<\n"
        .to_string()
}

fn backups(home: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(home) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".bashrc.mnemo.bak.") {
                found.push(entry.path());
            }
        }
    }
    found
}

#[test]
fn upgrade_met_a_niveau_un_bloc_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let before = format!("export FOO=1\n{}export BAR=2\n", legacy_block());
    std::fs::write(bashrc(home), &before).unwrap();

    let out = mnemo(home).args(["shell", "upgrade"]).output().unwrap();
    assert!(
        out.status.success(),
        "la mise à niveau doit réussir : {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("obsolète"), "doit annoncer l'état obsolète");
    assert!(stdout.contains("mis à niveau"));
    assert!(stdout.contains("source ~/.bashrc"));

    let after = std::fs::read_to_string(bashrc(home)).unwrap();
    assert!(
        after.contains("MNEMO_SESSION_ID"),
        "le bloc à jour doit capturer MNEMO_SESSION_ID"
    );
    assert!(after.contains("integration version: 2"));
    assert_eq!(count_blocks(&after), 1, "jamais de doublon");
    assert!(
        after.contains("export FOO=1"),
        "contenu utilisateur préservé"
    );
    assert!(
        after.contains("export BAR=2"),
        "contenu utilisateur préservé"
    );

    assert_eq!(backups(home).len(), 1, "une sauvegarde doit être créée");
    let backup_content = std::fs::read_to_string(&backups(home)[0]).unwrap();
    assert_eq!(
        backup_content, before,
        "la sauvegarde doit refléter l'avant"
    );
}

#[test]
fn upgrade_est_idempotent_si_deja_a_jour() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // Premier passage : installe un bloc à jour.
    std::fs::write(bashrc(home), legacy_block()).unwrap();
    assert!(mnemo(home)
        .args(["shell", "upgrade"])
        .output()
        .unwrap()
        .status
        .success());
    let after_first = std::fs::read_to_string(bashrc(home)).unwrap();
    let backups_after_first = backups(home).len();

    // Second passage : aucun changement, aucune nouvelle sauvegarde.
    let out = mnemo(home).args(["shell", "upgrade"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("déjà à jour"));

    assert_eq!(std::fs::read_to_string(bashrc(home)).unwrap(), after_first);
    assert_eq!(
        backups(home).len(),
        backups_after_first,
        "pas de sauvegarde superflue"
    );
}

#[test]
fn upgrade_sans_bloc_propose_init() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    std::fs::write(bashrc(home), "export FOO=1\n").unwrap();

    let out = mnemo(home).args(["shell", "upgrade"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Aucune intégration"));
    assert!(stdout.contains("mnemo init"));

    // Aucun bloc créé, fichier inchangé, aucune sauvegarde.
    assert_eq!(
        std::fs::read_to_string(bashrc(home)).unwrap(),
        "export FOO=1\n"
    );
    assert_eq!(
        count_blocks(&std::fs::read_to_string(bashrc(home)).unwrap()),
        0
    );
    assert!(backups(home).is_empty());
}

#[test]
fn doctor_signale_un_bloc_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    std::fs::write(bashrc(home), legacy_block()).unwrap();

    let out = mnemo(home).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("obsolète") && stdout.contains("mnemo shell upgrade"),
        "doctor doit signaler le bloc obsolète et proposer la correction"
    );
}

#[test]
fn doctor_fix_met_a_niveau_un_bloc_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    std::fs::write(bashrc(home), legacy_block()).unwrap();

    let out = mnemo(home).args(["doctor", "--fix"]).output().unwrap();
    assert!(out.status.success());

    let after = std::fs::read_to_string(bashrc(home)).unwrap();
    assert!(after.contains("MNEMO_SESSION_ID"));
    assert!(after.contains("integration version: 2"));
    assert_eq!(count_blocks(&after), 1);
}
