//! Tests d'intégration pour l'onboarding (`mnemo init [--wizard]`), les
//! complétions shell (`mnemo completions`) et la page de manuel.
//!
//! Comme pour `tests/cli.rs`, l'exécution est isolée dans un HOME temporaire via
//! HOME + XDG_CONFIG_HOME / XDG_DATA_HOME. Invoqué via `Command::output()`,
//! stdin n'est pas un terminal : le wizard est donc en mode non interactif.

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

#[test]
fn init_simple_cree_la_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let out = mnemo(home).arg("init").output().unwrap();
    assert!(out.status.success());

    let cfg = home.join(".config/mnemo/config.toml");
    assert!(cfg.exists(), "la configuration doit être créée");
}

#[test]
fn wizard_non_interactif_sans_yes_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // Pré-remplit le .bashrc pour vérifier qu'il n'est pas modifié.
    std::fs::write(bashrc(home), "export FOO=1\n").unwrap();

    let out = mnemo(home).args(["init", "--wizard"]).output().unwrap();
    assert!(
        !out.status.success(),
        "le wizard doit refuser de s'exécuter sans terminal ni --yes"
    );

    let content = std::fs::read_to_string(bashrc(home)).unwrap();
    assert_eq!(
        content, "export FOO=1\n",
        "le .bashrc ne doit pas être touché"
    );
    assert_eq!(count_blocks(&content), 0);
}

#[test]
fn wizard_yes_est_idempotent_et_non_destructif() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // Contenu utilisateur existant qui doit survivre.
    std::fs::write(bashrc(home), "export FOO=1\n").unwrap();

    for _ in 0..2 {
        let out = mnemo(home)
            .args(["init", "--wizard", "--yes"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "le wizard --yes doit réussir : {:?}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let content = std::fs::read_to_string(bashrc(home)).unwrap();
    assert!(
        content.contains("export FOO=1"),
        "le contenu utilisateur existant doit être préservé"
    );
    assert_eq!(
        count_blocks(&content),
        1,
        "le bloc d'intégration ne doit jamais être dupliqué"
    );
}

#[test]
fn completions_bash_contient_le_binaire() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let out = mnemo(home).args(["completions", "bash"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("mnemo"),
        "le script bash doit mentionner mnemo"
    );
}

#[test]
fn completions_zsh_contient_la_fonction() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let out = mnemo(home).args(["completions", "zsh"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("_mnemo"),
        "le script zsh doit définir _mnemo"
    );
}

#[test]
fn completions_fish_est_valide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let out = mnemo(home).args(["completions", "fish"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("complete"));
    assert!(stdout.contains("mnemo"));
}

#[test]
fn completions_shell_inconnu_echoue() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let out = mnemo(home)
        .args(["completions", "powershell"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "un shell non supporté doit produire une erreur"
    );
}

#[test]
fn page_de_manuel_presente() {
    let man = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/man/mnemo.1");
    assert!(
        man.exists(),
        "la page de manuel docs/man/mnemo.1 doit exister"
    );
}
