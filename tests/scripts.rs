//! Tests d'intégration des scripts shell (`scripts/`).
//!
//! Ces tests vérifient :
//! - la syntaxe (`bash -n`) de install.sh / uninstall.sh / lib/bashrc.sh ;
//! - que le bloc `.bashrc` n'est pas ajouté deux fois (idempotence) ;
//! - qu'une sauvegarde est créée avant toute modification du `.bashrc`.
//!
//! La logique de manipulation du `.bashrc` est centralisée dans
//! `scripts/lib/bashrc.sh`, sourcée à la fois par les scripts et par ces tests.

use std::path::PathBuf;
use std::process::Command;

fn project_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn script(path: &str) -> PathBuf {
    project_dir().join("scripts").join(path)
}

/// Exécute `bash -n` sur un script et renvoie true si la syntaxe est valide.
fn bash_syntax_ok(rel: &str) -> bool {
    Command::new("bash")
        .arg("-n")
        .arg(script(rel))
        .status()
        .expect("bash doit être disponible")
        .success()
}

#[test]
fn install_script_syntaxe_valide() {
    assert!(bash_syntax_ok("install.sh"));
}

#[test]
fn uninstall_script_syntaxe_valide() {
    assert!(bash_syntax_ok("uninstall.sh"));
}

#[test]
fn lib_bashrc_syntaxe_valide() {
    assert!(bash_syntax_ok("lib/bashrc.sh"));
}

/// Lance un fragment bash qui source lib/bashrc.sh, en passant le chemin du
/// `.bashrc` factice via la variable d'environnement RC.
fn run_with_lib(rc: &std::path::Path, body: &str) -> std::process::Output {
    let lib = script("lib/bashrc.sh");
    let program = format!("set -euo pipefail\nsource '{}'\n{}", lib.display(), body);
    Command::new("bash")
        .arg("-c")
        .arg(program)
        .env("RC", rc)
        .output()
        .expect("exécution bash")
}

#[test]
fn bloc_bashrc_non_ajoute_deux_fois() {
    let dir = tempfile::tempdir().unwrap();
    let rc = dir.path().join(".bashrc");
    std::fs::write(&rc, "# bashrc existant\nexport FOO=1\n").unwrap();

    // Deux installations successives du même bloc.
    let out = run_with_lib(
        &rc,
        r#"
        mnemo_install_bashrc_block "$RC" "snippet ligne 1"
        mnemo_install_bashrc_block "$RC" "snippet ligne 1" || true
        "#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = std::fs::read_to_string(&rc).unwrap();
    let occurrences = content.matches("# >>> mnemo init >>>").count();
    assert_eq!(
        occurrences, 1,
        "le bloc mnemo doit être présent une seule fois"
    );
}

#[test]
fn bashrc_sauvegarde_avant_modification() {
    let dir = tempfile::tempdir().unwrap();
    let rc = dir.path().join(".bashrc");
    let original = "# contenu original\n";
    std::fs::write(&rc, original).unwrap();

    let out = run_with_lib(&rc, r#"mnemo_install_bashrc_block "$RC" "snippet""#);
    assert!(out.status.success());

    // Une sauvegarde .bashrc.mnemo.bak.* doit exister et contenir l'original.
    let backups: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains(".bashrc.mnemo.bak.")
        })
        .collect();
    assert_eq!(
        backups.len(),
        1,
        "exactement une sauvegarde doit être créée"
    );

    let backup_content = std::fs::read_to_string(backups[0].path()).unwrap();
    assert_eq!(
        backup_content, original,
        "la sauvegarde doit contenir l'original"
    );
}

#[test]
fn suppression_du_bloc_bashrc() {
    let dir = tempfile::tempdir().unwrap();
    let rc = dir.path().join(".bashrc");
    std::fs::write(&rc, "export FOO=1\n").unwrap();

    let out = run_with_lib(
        &rc,
        r#"
        mnemo_install_bashrc_block "$RC" "ma config mnemo"
        mnemo_remove_bashrc_block "$RC"
        "#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = std::fs::read_to_string(&rc).unwrap();
    assert!(!content.contains("# >>> mnemo init >>>"));
    assert!(!content.contains("ma config mnemo"));
    assert!(
        content.contains("export FOO=1"),
        "le reste doit être préservé"
    );
}
