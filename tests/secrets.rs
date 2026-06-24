//! Tests d'intégration de `mnemo secrets` (scan / redact).
//!
//! Chaque test s'exécute dans un HOME temporaire isolé (HOME + XDG_*), stdin
//! fermé. Les commandes sensibles sont semées directement en base (le chemin
//! d'enregistrement normal `mnemo add` refuse les commandes sensibles), ce qui
//! reproduit un historique antérieur au filtrage.

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

/// Insère une commande arbitraire en base (contourne le filtrage d'ajout).
fn seed(home: &Path, command: &str, date: &str) {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.execute(
        "INSERT INTO commands (command, cwd, created_at, hash)
         VALUES (?1, '/tmp', ?2, ?3)",
        rusqlite::params![command, date, command],
    )
    .unwrap();
}

/// Lit le texte de commande stocké pour une date donnée.
fn stored_command(home: &Path, date: &str) -> String {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.query_row(
        "SELECT command FROM commands WHERE created_at = ?1",
        rusqlite::params![date],
        |row| row.get::<_, String>(0),
    )
    .unwrap()
}

#[test]
fn scan_affiche_redacte_et_jamais_le_secret_en_clair() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    seed(
        home,
        "export DB_PASSWORD=s3cr3tvalue",
        "2026-06-20 10:00:00",
    );
    seed(home, "ls -la", "2026-06-20 10:05:00");

    let out = run(home, &["secrets", "scan"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("[REDACTED]"));
    assert!(
        !text.contains("s3cr3tvalue"),
        "le secret ne doit jamais apparaître en clair : {text}"
    );
    // La commande non sensible n'est pas listée.
    assert!(!text.contains("ls -la"));
}

#[test]
fn scan_json_valide_sans_secret() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    seed(
        home,
        // Jeton factice de test, pas un vrai secret ; le marqueur évite un faux
        // positif du scanner gitleaks (règle curl-auth-header).
        "curl -H 'Authorization: Bearer tok3nABC' h", // gitleaks:allow
        "2026-06-20 10:00:00",
    );

    let out = run(home, &["secrets", "scan", "--json"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(
        !text.contains("tok3nABC"),
        "le JSON ne doit pas contenir de secret : {text}"
    );
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["suspected"], 1);
    let cats = value["results"][0]["categories"].as_array().unwrap();
    assert!(cats.iter().any(|c| c == "bearer_token"));
}

#[test]
fn redact_dry_run_ne_modifie_pas_la_base() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    let date = "2026-06-20 10:00:00";
    seed(home, "export TOKEN=abc123", date);

    let out = run(home, &["secrets", "redact"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("[dry-run]"));
    // La base est intacte.
    assert_eq!(stored_command(home, date), "export TOKEN=abc123");
}

#[test]
fn redact_apply_modifie_uniquement_la_colonne_command() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    let date = "2026-06-20 10:00:00";
    seed(home, "export TOKEN=abc123secret", date);

    let out = run(home, &["secrets", "redact", "--apply", "--yes"]);
    assert!(out.status.success(), "{:?}", out);
    let text = stdout(&out);
    assert!(text.contains("Sauvegarde"));
    assert!(text.contains("Commandes modifiées: 1"));

    // Seule la commande est redactée ; la date (autres colonnes) est conservée.
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    let (command, cwd): (String, String) = conn
        .query_row(
            "SELECT command, cwd FROM commands WHERE created_at = ?1",
            rusqlite::params![date],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(command, "export TOKEN=[REDACTED]");
    assert_eq!(cwd, "/tmp");
    assert!(!command.contains("abc123secret"));
}

#[test]
fn redact_apply_cree_une_sauvegarde() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    seed(home, "export PASSWORD=hunter2pass", "2026-06-20 10:00:00");

    let backups_before = backup_count(home);
    let out = run(home, &["secrets", "redact", "--apply", "--yes"]);
    assert!(out.status.success());
    assert!(backup_count(home) > backups_before);
}

#[test]
fn redact_apply_non_interactif_sans_yes_est_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    let date = "2026-06-20 10:00:00";
    seed(home, "export TOKEN=abc123secret", date);

    // stdin fermé + pas de --yes : la confirmation est refusée proprement.
    let out = run(home, &["secrets", "redact", "--apply"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("annulée"));
    // La base ne doit pas avoir été modifiée.
    assert_eq!(stored_command(home, date), "export TOKEN=abc123secret");
}

#[test]
fn scan_sans_secret_indique_rien() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    seed(home, "git status", "2026-06-20 10:00:00");

    let out = run(home, &["secrets", "scan"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Aucune commande"));
}

/// Compte les archives de sauvegarde présentes dans le répertoire de données.
fn backup_count(home: &Path) -> usize {
    let dir = home.join(".local/share/mnemo/backups");
    match std::fs::read_dir(&dir) {
        Ok(entries) => entries.count(),
        Err(_) => 0,
    }
}
