//! Tests d'intégration des fonctionnalités produit v0.9 : configuration
//! enrichie, recherche avancée, export compressé, mode projet, maintenance
//! automatique et statistiques.
//!
//! Chaque test s'exécute dans un HOME temporaire isolé (HOME + XDG_*), avec
//! stdin fermé : les opérations destructives sans `--yes` sont donc refusées
//! (mode non interactif).

use std::io::Read;
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

fn config_path(home: &Path) -> PathBuf {
    home.join(".config/mnemo/config.toml")
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

/// Force `created_at` et `shell` d'une commande (pour tester les filtres).
fn tweak(home: &Path, command: &str, date: &str, shell: &str) {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.execute(
        "UPDATE commands SET created_at = ?1, shell = ?2 WHERE command = ?3",
        rusqlite::params![date, shell, command],
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Tâche 1 - config
// ---------------------------------------------------------------------------

#[test]
fn config_show_path_validate() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    assert!(run(home, &["init"]).status.success());

    let show = run(home, &["config", "show"]);
    assert!(show.status.success());
    assert!(stdout(&show).contains("[maintenance]"));

    let path = run(home, &["config", "path"]);
    assert!(path.status.success());
    assert!(stdout(&path).contains("config.toml"));

    let validate = run(home, &["config", "validate"]);
    assert!(validate.status.success());
}

#[test]
fn config_validate_detecte_une_erreur() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    assert!(run(home, &["init"]).status.success());

    // search_limit invalide -> erreur de validation, code 1.
    std::fs::write(
        config_path(home),
        "search_limit = 0\n[maintenance]\nauto_prune_after = \"180d\"\n",
    )
    .unwrap();
    let validate = run(home, &["config", "validate"]);
    assert!(!validate.status.success());
}

// ---------------------------------------------------------------------------
// Tâche 3 - recherche avancée
// ---------------------------------------------------------------------------

#[test]
fn search_filtre_par_code_et_echec() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("ls -la", 0), ("make build", 1), ("cargo test", 2)]);

    let failed = run(home, &["search", "--print", "--failed"]);
    let txt = stdout(&failed);
    assert!(txt.contains("make build"));
    assert!(txt.contains("cargo test"));
    assert!(!txt.contains("ls -la"));

    let exact = run(home, &["search", "--print", "--exit-code", "1"]);
    let txt = stdout(&exact);
    assert!(txt.contains("make build"));
    assert!(!txt.contains("cargo test"));
}

#[test]
fn search_filtre_par_date_et_shell() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("vieux", 0), ("recent", 0)]);
    tweak(home, "vieux", "2020-01-01 10:00:00", "zsh");
    tweak(home, "recent", "2026-06-18 10:00:00", "bash");

    // --before exclut le récent.
    let before = run(home, &["search", "--print", "--before", "2021-01-01"]);
    let txt = stdout(&before);
    assert!(txt.contains("vieux"));
    assert!(!txt.contains("recent"));

    // --shell filtre par interpréteur.
    let shell = run(home, &["search", "--print", "--shell", "zsh"]);
    let txt = stdout(&shell);
    assert!(txt.contains("vieux"));
    assert!(!txt.contains("recent"));
}

#[test]
fn search_json_est_stable() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("echo hi", 0)]);

    let out = run(home, &["search", "--print", "--json"]);
    assert!(out.status.success());
    let txt = stdout(&out);
    let parsed: serde_json::Value = serde_json::from_str(&txt).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed[0]["command"], "echo hi");
}

#[test]
fn search_date_invalide_ne_panique_pas() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("echo hi", 0)]);

    let out = run(home, &["search", "--print", "--since", "pas-une-date"]);
    // Pas de panique ; la commande se termine proprement.
    assert!(out.status.success() || out.status.code() == Some(0));
}

// ---------------------------------------------------------------------------
// Tâche 4 - export compressé
// ---------------------------------------------------------------------------

#[test]
fn export_gzip_produit_un_fichier_valide() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("echo a", 0), ("echo b", 0)]);

    let dest = home.join("export.json");
    let out = run(
        home,
        &[
            "export",
            "--format",
            "json",
            "--gzip",
            "--output",
            dest.to_str().unwrap(),
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let gz = home.join("export.json.gz");
    assert!(gz.exists(), "le fichier .json.gz doit exister");

    let file = std::fs::File::open(&gz).unwrap();
    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut content = String::new();
    decoder.read_to_string(&mut content).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// Tâche 5 - mode projet
// ---------------------------------------------------------------------------

#[test]
fn project_current_et_list() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("echo a", 0)]);

    let current = run(home, &["project", "current"]);
    assert!(current.status.success());
    assert!(!stdout(&current).trim().is_empty());

    let list = run(home, &["project", "list"]);
    assert!(list.status.success());
}

// ---------------------------------------------------------------------------
// Tâche 6 - maintenance automatique
// ---------------------------------------------------------------------------

#[test]
fn maintenance_status_et_dry_run_sans_suppression() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("vieux", 0), ("recent", 0)]);
    tweak(home, "vieux", "2000-01-01 10:00:00", "bash");

    // Désactivé par défaut : status le signale, dry-run ne supprime rien.
    let status = run(home, &["maintenance", "status"]);
    assert!(status.status.success());

    let dry = run(home, &["maintenance", "run", "--dry-run"]);
    assert!(dry.status.success());

    let count = run(home, &["search", "--print", "--limit", "100"]);
    assert!(stdout(&count).contains("vieux"));
}

#[test]
fn maintenance_run_yes_supprime_les_anciennes() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("vieux", 0), ("recent", 0)]);
    tweak(home, "vieux", "2000-01-01 10:00:00", "bash");
    tweak(home, "recent", "2026-06-18 10:00:00", "bash");

    // Active le nettoyage automatique dans la config.
    let cfg = std::fs::read_to_string(config_path(home)).unwrap();
    let cfg = cfg.replace("auto_prune_enabled = false", "auto_prune_enabled = true");
    std::fs::write(config_path(home), cfg).unwrap();

    let run_yes = run(home, &["maintenance", "run", "--yes"]);
    assert!(
        run_yes.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run_yes.stderr)
    );

    let remaining = run(home, &["search", "--print", "--limit", "100"]);
    let txt = stdout(&remaining);
    assert!(
        !txt.contains("vieux"),
        "l'ancienne commande doit être supprimée"
    );
    assert!(txt.contains("recent"), "la récente doit être conservée");
}

// ---------------------------------------------------------------------------
// Tâche 7 - stats
// ---------------------------------------------------------------------------

#[test]
fn stats_json_contient_les_nouveaux_champs() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("ls", 0), ("false", 1)]);

    let out = run(home, &["stats", "--json"]);
    assert!(out.status.success());
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert!(parsed.get("failure_rate").is_some());
    assert!(parsed.get("top_shells").is_some());
    assert!(parsed.get("activity_last_7_days").is_some());
}

#[test]
fn stats_since_invalide_ne_panique_pas() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    setup(home, &[("ls", 0)]);

    let out = run(home, &["stats", "--since", "n'importe-quoi"]);
    assert!(out.status.success());
}
