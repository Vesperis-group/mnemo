//! Tests d'intégration de la feature « search UX » (v1.5) : inspection et
//! récupération sûres d'une commande (`mnemo show`, `mnemo print`) et filtres
//! avancés de `mnemo search` (`--id-only`, `--since 24h`, `--until`, `--json`
//! sans `--print`).
//!
//! Chaque test s'exécute dans un HOME temporaire isolé, stdin fermé : aucune
//! commande de l'historique n'est jamais exécutée par mnemo.

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

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

fn db_path(home: &Path) -> PathBuf {
    home.join(".local/share/mnemo/history.db")
}

fn setup(home: &Path, commands: &[(&str, i64)]) {
    assert!(run(home, &["init"]).status.success());
    for (cmd, exit) in commands {
        let exit = exit.to_string();
        assert!(run(
            home,
            &["add", "--cmd", cmd, "--cwd", "/tmp", "--exit-code", &exit]
        )
        .status
        .success());
    }
}

/// Renvoie l'ID d'une commande à partir de son texte (lecture directe en base).
fn id_of(home: &Path, command: &str) -> i64 {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.query_row(
        "SELECT id FROM commands WHERE command = ?1",
        [command],
        |row| row.get(0),
    )
    .unwrap()
}

/// Force `created_at` (et éventuellement `git_root`) d'une commande.
fn tweak(home: &Path, command: &str, created_at: &str, git_root: Option<&str>) {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.execute(
        "UPDATE commands SET created_at = ?1, git_root = COALESCE(?2, git_root) WHERE command = ?3",
        rusqlite::params![created_at, git_root, command],
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// mnemo show
// ---------------------------------------------------------------------------

#[test]
fn show_affiche_une_commande_existante() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("cargo test --locked", 0)]);
    let id = id_of(home, "cargo test --locked");

    let out = run(home, &["show", &id.to_string()]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains(&format!("Commande {id}")));
    assert!(text.contains("Code retour : 0"));
    assert!(text.contains("Commande :\ncargo test --locked\n"));
}

#[test]
fn show_echoue_proprement_si_id_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("ls", 0)]);

    let out = run(home, &["show", "999999"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("999999"));
    // Aucune sortie « utile » sur stdout.
    assert!(stdout(&out).trim().is_empty());
}

#[test]
fn show_conserve_une_commande_redactee() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("deploy prod", 0)]);
    // Simule une commande déjà redactée en base (forme stockée).
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.execute(
        "UPDATE commands SET command = ?1 WHERE command = ?2",
        rusqlite::params!["curl -H 'Authorization: Bearer [REDACTED]'", "deploy prod"],
    )
    .unwrap();
    let id: i64 = conn
        .query_row(
            "SELECT id FROM commands WHERE command LIKE '%REDACTED%'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    let out = run(home, &["show", &id.to_string()]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("[REDACTED]"));
}

// ---------------------------------------------------------------------------
// mnemo print
// ---------------------------------------------------------------------------

#[test]
fn print_imprime_uniquement_la_commande() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("git status", 0)]);
    let id = id_of(home, "git status");

    let out = run(home, &["print", &id.to_string()]);
    assert!(out.status.success());
    // Exactement la commande suivie d'un saut de ligne, sans décor.
    assert_eq!(stdout(&out), "git status\n");
}

#[test]
fn print_echoue_proprement_si_id_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("ls", 0)]);

    let out = run(home, &["print", "424242"]);
    assert!(!out.status.success());
    assert!(stdout(&out).is_empty());
    assert!(stderr(&out).contains("424242"));
}

// ---------------------------------------------------------------------------
// mnemo search : nouvelles options
// ---------------------------------------------------------------------------

#[test]
fn search_id_only_affiche_uniquement_des_ids() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(
        home,
        &[("cargo build", 0), ("cargo test", 0), ("ls -la", 0)],
    );

    let out = run(home, &["search", "cargo", "--id-only"]);
    assert!(out.status.success());
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(!lines.is_empty());
    // Chaque ligne est un entier, et correspond à une commande « cargo ».
    let cargo_ids = [id_of(home, "cargo build"), id_of(home, "cargo test")];
    for line in &lines {
        let id: i64 = line.trim().parse().expect("ligne doit être un ID entier");
        assert!(cargo_ids.contains(&id), "ID inattendu : {id}");
    }
    // La commande non « cargo » ne doit pas apparaître.
    let ls_id = id_of(home, "ls -la").to_string();
    assert!(!lines.contains(&ls_id.as_str()));
}

#[test]
fn search_json_implique_le_mode_non_interactif() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("cargo build", 0)]);

    // Sans --print : --json doit malgré tout produire du JSON (pas de TUI).
    let out = run(home, &["search", "cargo", "--json"]);
    assert!(out.status.success());
    let text = stdout(&out);
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("JSON valide");
    assert!(parsed.is_array());
    assert!(text.contains("cargo build"));
}

#[test]
fn search_since_24h_filtre_correctement() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("commande_recente", 0), ("commande_ancienne", 0)]);
    // L'ancienne date à 2020, la récente reste à aujourd'hui.
    tweak(home, "commande_ancienne", "2020-01-01 10:00:00", None);

    let out = run(home, &["search", "--print", "--since", "24h"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("commande_recente"));
    assert!(!text.contains("commande_ancienne"));
}

#[test]
fn search_since_date_iso_filtre_correctement() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("recent_iso", 0), ("vieux_iso", 0)]);
    tweak(home, "recent_iso", "2026-05-01 10:00:00", None);
    tweak(home, "vieux_iso", "2020-01-01 10:00:00", None);

    let out = run(home, &["search", "--print", "--since", "2026-01-01"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("recent_iso"));
    assert!(!text.contains("vieux_iso"));
}

#[test]
fn search_until_alias_de_before() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("avant_borne", 0), ("apres_borne", 0)]);
    tweak(home, "avant_borne", "2020-01-01 10:00:00", None);
    tweak(home, "apres_borne", "2026-05-01 10:00:00", None);

    // --until est un alias de --before : ne garde que ce qui précède la date.
    let out = run(home, &["search", "--print", "--until", "2021-01-01"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("avant_borne"));
    assert!(!text.contains("apres_borne"));
}

#[test]
fn search_project_filtre_correctement() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("dans_projet", 0), ("hors_projet", 0)]);
    tweak(
        home,
        "dans_projet",
        "2026-05-01 10:00:00",
        Some("/home/u/projects/mnemo"),
    );

    let out = run(home, &["search", "--print", "--project", "mnemo"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("dans_projet"));
    assert!(!text.contains("hors_projet"));
}

#[test]
fn search_before_reste_compatible() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    setup(home, &[("ancienne_cmd", 0), ("recente_cmd", 0)]);
    tweak(home, "ancienne_cmd", "2020-01-01 10:00:00", None);
    tweak(home, "recente_cmd", "2026-05-01 10:00:00", None);

    // L'option historique --before doit continuer de fonctionner.
    let out = run(home, &["search", "--print", "--before", "2021-01-01"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("ancienne_cmd"));
    assert!(!text.contains("recente_cmd"));
}
