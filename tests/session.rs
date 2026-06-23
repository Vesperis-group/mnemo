//! Tests d'intégration de `mnemo session` (list / show / export).
//!
//! Chaque test s'exécute dans un HOME temporaire isolé (HOME + XDG_*), stdin
//! fermé. Les commandes sont semées avec un `MNEMO_SESSION_ID` explicite pour
//! reproduire la capture faite par l'intégration shell, puis leurs horodatages
//! sont fixés via SQLite pour rendre l'ordre déterministe.

use std::path::{Path, PathBuf};
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

fn db_path(home: &Path) -> PathBuf {
    home.join(".local/share/mnemo/history.db")
}

fn init(home: &Path) {
    assert!(run(home, &["init"]).status.success());
}

/// Ajoute une commande rattachée à une session (via `MNEMO_SESSION_ID`).
fn add(home: &Path, session: &str, cmd: &str) {
    let out = mnemo(home)
        .env("MNEMO_SESSION_ID", session)
        .args(["add", "--cmd", cmd, "--cwd", "/tmp"])
        .output()
        .unwrap();
    assert!(out.status.success());
}

/// Fixe l'horodatage d'une commande pour contrôler l'ordre chronologique.
fn set_date(home: &Path, command: &str, date: &str) {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    let n = conn
        .execute(
            "UPDATE commands SET created_at = ?1 WHERE command = ?2",
            rusqlite::params![date, command],
        )
        .unwrap();
    assert!(n >= 1, "commande absente : {command}");
}

#[test]
fn list_affiche_plusieurs_sessions_recentes_en_premier() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add(home, "sess-a", "cargo build");
    add(home, "sess-a", "cargo test");
    add(home, "sess-b", "git status");
    set_date(home, "cargo build", "2026-06-20 10:00:00");
    set_date(home, "cargo test", "2026-06-20 10:05:00");
    set_date(home, "git status", "2026-06-21 09:00:00");

    let out = run(home, &["session", "list"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("Sessions récentes"));
    assert!(text.contains("sess-a"));
    assert!(text.contains("sess-b"));
    let pos_b = text.find("sess-b").unwrap();
    let pos_a = text.find("sess-a").unwrap();
    assert!(
        pos_b < pos_a,
        "la session la plus récente doit venir en premier"
    );
}

#[test]
fn list_limit_borne_le_nombre_de_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add(home, "sess-a", "cargo build");
    add(home, "sess-b", "git status");
    set_date(home, "cargo build", "2026-06-20 10:00:00");
    set_date(home, "git status", "2026-06-21 09:00:00");

    let out = run(home, &["session", "list", "--limit", "1"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("sess-b"));
    assert!(
        !text.contains("sess-a"),
        "--limit 1 ne doit montrer qu'une session"
    );
}

#[test]
fn show_affiche_les_commandes_dans_l_ordre() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add(home, "sess-a", "cargo build");
    add(home, "sess-a", "cargo test");
    set_date(home, "cargo build", "2026-06-20 10:00:00");
    set_date(home, "cargo test", "2026-06-20 10:05:00");

    let out = run(home, &["session", "show", "sess-a"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("Session sess-a"));
    assert!(text.contains("Commandes : 2"));
    let pos_build = text.find("cargo build").unwrap();
    let pos_test = text.find("cargo test").unwrap();
    assert!(pos_build < pos_test, "ordre chronologique attendu");
}

#[test]
fn show_session_inexistante_echoue_proprement() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "cargo build");

    let out = run(home, &["session", "show", "inconnue"]);
    assert!(
        !out.status.success(),
        "une session inexistante doit échouer"
    );
}

#[test]
fn export_markdown_produit_un_document_propre() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "cargo build");
    add(home, "sess-a", "git status");

    let out = run(home, &["session", "export", "sess-a"]);
    assert!(out.status.success());
    let md = stdout(&out);
    assert!(md.contains("# Session mnemo"));
    assert!(md.contains("## Commandes"));
    assert!(md.contains("## Détail chronologique"));
    assert!(md.contains("cargo build"));
    assert!(md.contains("git status"));
}

#[test]
fn export_json_est_valide_et_complet() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "cargo build");
    add(home, "sess-a", "git status");

    let out = run(home, &["session", "export", "sess-a", "--format", "json"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(value["session_id"], "sess-a");
    assert_eq!(value["command_count"], 2);
    assert_eq!(value["commands"].as_array().unwrap().len(), 2);
    assert_eq!(value["commands"][0]["command"], "cargo build");
}

#[test]
fn export_last_cible_la_session_la_plus_recente() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "cargo build");
    add(home, "sess-b", "git status");
    set_date(home, "cargo build", "2026-06-20 10:00:00");
    set_date(home, "git status", "2026-06-21 09:00:00");

    let out = run(home, &["session", "export", "--last", "--format", "json"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(value["session_id"], "sess-b");
}

#[test]
fn export_output_cree_le_fichier() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "cargo build");

    let target = home.join("session.md");
    let out = mnemo(home)
        .args(["session", "export", "sess-a", "--output"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(target.exists());
    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("# Session mnemo"));
}

#[test]
fn export_output_refuse_d_ecraser_sans_force() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "cargo build");

    let target = home.join("session.md");
    std::fs::write(&target, "CONTENU EXISTANT").unwrap();

    let out = mnemo(home)
        .args(["session", "export", "sess-a", "--output"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "écrasement sans --force doit échouer"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "CONTENU EXISTANT"
    );
}

#[test]
fn export_output_force_ecrase_explicitement() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "cargo build");

    let target = home.join("session.md");
    std::fs::write(&target, "CONTENU EXISTANT").unwrap();

    let out = mnemo(home)
        .args(["session", "export", "sess-a", "--force", "--output"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(out.status.success());
    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("# Session mnemo"));
    assert!(!content.contains("CONTENU EXISTANT"));
}

#[test]
fn caracteres_speciaux_ne_cassent_pas_le_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add(home, "sess-a", "echo `date`");
    add(home, "sess-a", "grep -E 'a|b' fichier");

    let out = run(home, &["session", "export", "sess-a"]);
    assert!(out.status.success());
    let md = stdout(&out);
    // La structure doit rester intacte malgré les backticks et pipes.
    assert!(md.contains("## Commandes"));
    assert!(md.contains("## Détail chronologique"));
    // Le pipe dans la cellule de tableau doit être échappé.
    assert!(md.contains("a\\|b"));
    // La clôture du bloc de code doit rester équilibrée (nombre pair de lignes
    // de clôture commençant par au moins trois backticks).
    let fences = md.lines().filter(|l| l.starts_with("```")).count();
    assert_eq!(fences % 2, 0, "blocs de code non équilibrés");
}

#[test]
fn anciennes_commandes_sans_session_ne_plantent_pas() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    // Commandes ajoutées sans MNEMO_SESSION_ID (cas des imports anciens).
    assert!(run(home, &["add", "--cmd", "ls -la", "--cwd", "/tmp"])
        .status
        .success());

    let list = run(home, &["session", "list"]);
    assert!(list.status.success());
    assert!(stdout(&list).contains("Aucune session enregistrée"));

    let export = run(home, &["session", "export", "--last"]);
    assert!(
        !export.status.success(),
        "--last sans session doit échouer proprement"
    );
}
