//! Tests d'intégration de la commande `mnemo doctor`.
//!
//! Chaque test s'exécute dans un HOME temporaire isolé (HOME + XDG_*), afin de
//! ne jamais toucher aux données réelles de l'utilisateur.

use std::path::Path;
use std::process::{Command, Output};

fn mnemo(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mnemo"));
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"));
    cmd
}

fn run(home: &Path, args: &[&str]) -> Output {
    mnemo(home).args(args).output().unwrap()
}

#[test]
fn doctor_home_sain_retourne_0() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());

    let out = run(home, &["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Configuration présente"));
    assert!(stdout.contains("Table `commands` présente"));
}

#[test]
fn doctor_config_absente_signale_warning_sans_erreur() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // Pas d'init : ni config ni base.
    let out = run(home, &["doctor"]);
    assert_eq!(out.status.code(), Some(0)); // warnings, pas d'erreur bloquante
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Configuration absente"));
}

#[test]
fn doctor_db_absente_signale_warning() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // Crée seulement la config via init puis supprime la base.
    assert!(run(home, &["init"]).status.success());
    let db = home.join(".local/share/mnemo/history.db");
    std::fs::remove_file(&db).unwrap();

    let out = run(home, &["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Base absente"));
}

#[test]
fn doctor_db_corrompue_retourne_1() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());
    // Remplace la base par un fichier non-SQLite.
    let db = home.join(".local/share/mnemo/history.db");
    std::fs::write(&db, b"ceci n'est pas une base sqlite valide").unwrap();

    let out = run(home, &["doctor"]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn doctor_fix_cree_config_et_db() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let out = run(home, &["doctor", "--fix"]);
    assert_eq!(out.status.code(), Some(0));

    assert!(home.join(".config/mnemo/config.toml").exists());
    assert!(home.join(".local/share/mnemo/history.db").exists());
}

#[test]
fn doctor_fix_ajoute_le_bloc_bashrc_sans_doublon() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let bashrc = home.join(".bashrc");
    std::fs::write(&bashrc, "export FOO=1\n").unwrap();

    // Premier --fix : ajoute le bloc.
    assert!(run(home, &["doctor", "--fix"]).status.success());
    let content1 = std::fs::read_to_string(&bashrc).unwrap();
    assert!(content1.contains("__mnemo_record"));
    assert_eq!(content1.matches("# >>> mnemo >>>").count(), 1);

    // Une sauvegarde doit avoir été créée.
    let backups = std::fs::read_dir(home)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains(".bashrc.mnemo.bak.")
        })
        .count();
    assert_eq!(backups, 1);

    // Second --fix : ne duplique pas le bloc.
    assert!(run(home, &["doctor", "--fix"]).status.success());
    let content2 = std::fs::read_to_string(&bashrc).unwrap();
    assert_eq!(content2.matches("# >>> mnemo >>>").count(), 1);
}

#[test]
fn doctor_json_est_valide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert!(run(home, &["init"]).status.success());

    let out = run(home, &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();

    // Validation structurelle via serde_json.
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("la sortie --json doit être un JSON valide");

    assert!(parsed["summary"].is_object());
    assert!(parsed["summary"]["exit_code"].is_number());
    assert!(parsed["checks"].is_array());

    let checks = parsed["checks"].as_array().unwrap();
    assert!(!checks.is_empty());
    for c in checks {
        assert!(c["name"].is_string());
        assert!(c["status"].is_string());
        assert!(c["message"].is_string());
    }
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(unix)]
fn chmod(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms).unwrap();
}

#[cfg(unix)]
#[test]
fn doctor_permissions_600_sont_correctes() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert!(run(home, &["init"]).status.success());

    let config = home.join(".config/mnemo/config.toml");
    let db = home.join(".local/share/mnemo/history.db");
    chmod(&config, 0o600);
    chmod(&db, 0o600);

    let out = run(home, &["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Permissions correctes (600)"));
    assert!(!stdout.contains("Permissions trop ouvertes"));
}

#[cfg(unix)]
#[test]
fn doctor_signale_config_trop_ouverte() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert!(run(home, &["init"]).status.success());

    let config = home.join(".config/mnemo/config.toml");
    chmod(&config, 0o644);

    let out = run(home, &["doctor"]);
    // Permissions trop ouvertes = WARN, pas une erreur bloquante.
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Permissions trop ouvertes"));
    assert!(stdout.contains("actuel 644, attendu 600"));
}

#[cfg(unix)]
#[test]
fn doctor_signale_db_trop_ouverte() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert!(run(home, &["init"]).status.success());

    let db = home.join(".local/share/mnemo/history.db");
    chmod(&db, 0o644);

    let out = run(home, &["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Permissions trop ouvertes"));
    assert!(stdout.contains("history.db"));
}

#[cfg(unix)]
#[test]
fn doctor_fix_resserre_config_a_600() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert!(run(home, &["init"]).status.success());

    let config = home.join(".config/mnemo/config.toml");
    chmod(&config, 0o644);

    let out = run(home, &["doctor", "--fix"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Permissions corrigées"));
    assert!(stdout.contains("→ 600"));
    assert_eq!(mode_of(&config), 0o600);
}

#[cfg(unix)]
#[test]
fn doctor_fix_resserre_db_a_600() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert!(run(home, &["init"]).status.success());

    let db = home.join(".local/share/mnemo/history.db");
    chmod(&db, 0o644);

    assert!(run(home, &["doctor", "--fix"]).status.success());
    assert_eq!(mode_of(&db), 0o600);
}

#[cfg(unix)]
#[test]
fn init_cree_config_et_db_en_600() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert!(run(home, &["init"]).status.success());

    let config = home.join(".config/mnemo/config.toml");
    let db = home.join(".local/share/mnemo/history.db");
    assert_eq!(mode_of(&config), 0o600);
    assert_eq!(mode_of(&db), 0o600);
}
