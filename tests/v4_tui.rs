//! Tests d'intégration de la TUI v0.4.
//!
//! On ne teste pas le rendu Ratatui : on vérifie que la commande `tui` est bien
//! exposée (et son `--help`), et que les commandes non interactives existantes
//! (`search --print`, `list`, `export`, `stats`) restent inchangées.

use std::path::Path;
use std::process::{Command, Output, Stdio};

fn mnemo(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mnemo"));
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .stdin(Stdio::null());
    cmd
}

fn run(home: &Path, args: &[&str]) -> Output {
    mnemo(home).args(args).output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn setup(home: &Path, commands: &[&str]) {
    assert!(run(home, &["init"]).status.success());
    for c in commands {
        assert!(run(home, &["add", "--cmd", c, "--cwd", "/tmp"])
            .status
            .success());
    }
}

#[test]
fn tui_help_fonctionne() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), &["tui", "--help"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("tui") || text.to_lowercase().contains("interactive"));
    // Les filtres documentés sont exposés.
    assert!(text.contains("--project"));
    assert!(text.contains("--branch"));
    assert!(text.contains("--cwd"));
    assert!(text.contains("--failed"));
}

#[test]
fn tui_help_avec_filtre_fonctionne() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), &["tui", "--project", "mnemo", "--help"]);
    assert!(out.status.success());
}

#[test]
fn tui_sur_base_absente_propose_init() {
    let dir = tempfile::tempdir().unwrap();
    // Aucune init : la base n'existe pas, la TUI ne doit pas planter.
    let out = run(dir.path(), &["tui"]);
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("mnemo init"));
}

#[test]
fn search_print_reste_inchange() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["cargo build", "git status", "cargo test"]);

    let out = run(home, &["search", "--print", "cargo"]);
    assert!(out.status.success());
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| l.contains("cargo")));
}

#[test]
fn list_export_stats_restent_ok() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["alpha", "beta"]);

    assert!(run(home, &["list"]).status.success());
    assert!(run(home, &["export", "--format", "json"]).status.success());
    assert!(run(home, &["export", "--format", "csv"]).status.success());
    assert!(run(home, &["stats"]).status.success());
    assert!(run(home, &["stats", "--json"]).status.success());
}
