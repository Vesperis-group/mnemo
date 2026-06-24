//! Tests d'intégration de la feature « project » (v1.6) : navigation par projet
//! (`mnemo project list|show`) et rapports d'activité réutilisables
//! (`mnemo project report`), en Markdown et JSON.
//!
//! Chaque test s'exécute dans un HOME temporaire isolé, stdin fermé : aucune
//! commande de l'historique n'est jamais exécutée par mnemo (un test dédié le
//! vérifie explicitement).

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

fn init(home: &Path) {
    assert!(run(home, &["init"]).status.success());
}

/// Enregistre une commande puis force son contexte Git (racine, branche,
/// session) et son horodatage directement en base, pour des scénarios
/// déterministes.
#[allow(clippy::too_many_arguments)]
fn seed(
    home: &Path,
    cmd: &str,
    exit: i64,
    root: &str,
    branch: Option<&str>,
    session: Option<&str>,
    created_at: &str,
) {
    let exit_s = exit.to_string();
    assert!(run(
        home,
        &["add", "--cmd", cmd, "--cwd", "/tmp", "--exit-code", &exit_s]
    )
    .status
    .success());
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.execute(
        "UPDATE commands
         SET git_root = ?1, git_branch = ?2, session_id = ?3, created_at = ?4
         WHERE command = ?5",
        rusqlite::params![root, branch, session, created_at, cmd],
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// project list
// ---------------------------------------------------------------------------

#[test]
fn list_affiche_les_projets_connus() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd-a1",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "cmd-b1",
        0,
        "/home/u/beta",
        Some("dev"),
        Some("s2"),
        "2026-06-15 10:00:00",
    );

    let out = run(home, &["project", "list"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("alpha"));
    assert!(text.contains("beta"));
    assert!(text.contains("SESSIONS"));
    assert!(text.contains("BRANCHES"));
}

#[test]
fn list_json_expose_les_compteurs() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd-1",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "cmd-2",
        1,
        "/home/u/alpha",
        Some("feat"),
        Some("s1"),
        "2026-06-14 10:05:00",
    );
    seed(
        home,
        "cmd-3",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s2"),
        "2026-06-15 09:00:00",
    );

    let out = run(home, &["project", "list", "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "alpha");
    assert_eq!(arr[0]["command_count"], 3);
    assert_eq!(arr[0]["session_count"], 2);
    let branches = arr[0]["branches"].as_array().unwrap();
    assert_eq!(branches.len(), 2);
}

#[test]
fn list_respecte_la_limite() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd-a",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "cmd-b",
        0,
        "/home/u/beta",
        Some("main"),
        Some("s2"),
        "2026-06-15 10:00:00",
    );

    let out = run(home, &["project", "list", "--json", "--limit", "1"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    // Le plus récemment actif vient en tête.
    assert_eq!(v[0]["name"], "beta");
}

// ---------------------------------------------------------------------------
// project show
// ---------------------------------------------------------------------------

#[test]
fn show_par_nom_affiche_les_metadonnees() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cargo build",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "cargo test",
        1,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:05:00",
    );

    let out = run(home, &["project", "show", "alpha"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("Projet  : alpha"));
    assert!(text.contains("Commandes : 2"));
    assert!(text.contains("Commandes récentes"));
    assert!(text.contains("cargo build"));
    assert!(text.contains("Derniers échecs"));
    assert!(text.contains("cargo test (exit 1)"));
}

#[test]
fn show_json_separe_recents_et_echecs() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "ok-cmd",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "ko-cmd",
        2,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:05:00",
    );

    let out = run(home, &["project", "show", "alpha", "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["project"]["name"], "alpha");
    assert_eq!(v["recent"].as_array().unwrap().len(), 2);
    assert_eq!(v["recent_failures"].as_array().unwrap().len(), 1);
    assert_eq!(v["recent_failures"][0]["command"], "ko-cmd");
}

#[test]
fn show_projet_inconnu_echoue_clairement() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );

    let out = run(home, &["project", "show", "inexistant"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("Aucun projet ne correspond"));
}

#[test]
fn show_ambigu_refuse_explicitement() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd-1",
        0,
        "/home/u/app",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "cmd-2",
        0,
        "/srv/app",
        Some("main"),
        Some("s2"),
        "2026-06-15 10:00:00",
    );

    let out = run(home, &["project", "show", "app"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("Plusieurs projets correspondent"));
}

#[test]
fn show_current_hors_historique_echoue() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );

    // Le répertoire courant (HOME temporaire) n'est rattaché à aucun projet de
    // l'historique : la résolution doit échouer proprement.
    let out = mnemo(home)
        .args(["project", "show", "--current"])
        .current_dir(home)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("absent de l'historique"));
}

// ---------------------------------------------------------------------------
// project report
// ---------------------------------------------------------------------------

#[test]
fn report_markdown_contient_titre_et_tableau() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cargo build",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "cargo test",
        1,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:05:00",
    );

    let out = run(home, &["project", "report", "alpha"]);
    assert!(out.status.success());
    let md = stdout(&out);
    assert!(md.contains("# Rapport projet — alpha"));
    assert!(md.contains("## Commandes"));
    assert!(md.contains("## Détail chronologique"));
    assert!(md.contains("| Date | Code retour | Branche | Commande |"));
    assert!(md.contains("## Échecs"));
    assert!(md.contains("cargo build"));
}

#[test]
fn report_markdown_echappe_pipes_et_backticks() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "grep -E 'a|b' && echo `date`",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );

    let out = run(home, &["project", "report", "alpha"]);
    assert!(out.status.success());
    let md = stdout(&out);
    // Le pipe est échappé dans la cellule de tableau.
    assert!(md.contains("a\\|b"));
    // Le bloc de code est ouvert par une clôture d'au moins trois backticks.
    assert!(md.contains("```bash"));
    // Le backtick interne de la commande est préservé en code en ligne.
    assert!(md.contains("echo `date`"));
}

#[test]
fn report_json_expose_les_agregats_de_periode() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "ok",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    seed(
        home,
        "ko",
        1,
        "/home/u/alpha",
        Some("feat"),
        Some("s2"),
        "2026-06-14 10:05:00",
    );

    let out = run(home, &["project", "report", "alpha", "--format", "json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["project"]["name"], "alpha");
    assert_eq!(v["period"]["command_count"], 2);
    assert_eq!(v["period"]["failure_count"], 1);
    assert_eq!(v["period"]["session_count"], 2);
    assert_eq!(v["commands"].as_array().unwrap().len(), 2);
}

#[test]
fn report_since_filtre_la_periode() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "vieux",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2020-01-01 10:00:00",
    );
    seed(
        home,
        "recent",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2099-01-01 10:00:00",
    );

    let out = run(
        home,
        &[
            "project",
            "report",
            "alpha",
            "--since",
            "2026-01-01",
            "--format",
            "json",
        ],
    );
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["period"]["command_count"], 1);
    assert_eq!(v["commands"][0]["command"], "recent");
}

#[test]
fn report_until_invalide_echoue() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );

    let out = run(
        home,
        &["project", "report", "alpha", "--until", "pas-une-date"],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--until invalide"));
}

#[test]
fn report_ecrit_un_fichier_et_protege_contre_lecrasement() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "cmd",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );
    let target = home.join("rapport.md");
    let target_s = target.to_str().unwrap();

    let out = run(home, &["project", "report", "alpha", "--output", target_s]);
    assert!(out.status.success());
    assert!(target.exists());

    // Sans --force, un fichier existant n'est pas écrasé.
    let out2 = run(home, &["project", "report", "alpha", "--output", target_s]);
    assert!(!out2.status.success());
    assert!(stderr(&out2).contains("--force"));

    // Avec --force, l'écrasement est autorisé.
    let out3 = run(
        home,
        &[
            "project", "report", "alpha", "--output", target_s, "--force",
        ],
    );
    assert!(out3.status.success());
}

#[test]
fn report_preserve_les_commandes_redactees() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    seed(
        home,
        "curl -H 'Authorization: [REDACTED]' https://api",
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );

    let out = run(home, &["project", "report", "alpha"]);
    assert!(out.status.success());
    let md = stdout(&out);
    // La commande redactée est restituée telle quelle, sans nouvelle analyse.
    assert!(md.contains("[REDACTED]"));
}

#[test]
fn aucune_commande_de_lhistorique_nest_executee() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    init(home);
    let sentinel = home.join("sentinel.txt");
    let sentinel_s = sentinel.to_str().unwrap();
    let dangerous = format!("touch {sentinel_s}");
    seed(
        home,
        &dangerous,
        0,
        "/home/u/alpha",
        Some("main"),
        Some("s1"),
        "2026-06-14 10:00:00",
    );

    assert!(run(home, &["project", "show", "alpha"]).status.success());
    assert!(run(home, &["project", "report", "alpha"]).status.success());

    // Le fichier sentinelle ne doit jamais avoir été créé.
    assert!(!sentinel.exists());
}
