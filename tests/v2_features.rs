//! Tests d'intégration des fonctionnalités v0.2 : migrations, contexte Git,
//! recherche filtrée et statistiques.
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

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

/// Initialise un dépôt Git minimal dans `dir` sur la branche `branch`.
/// Retourne `true` si Git est disponible et l'initialisation a réussi.
fn init_git_repo(dir: &Path, branch: &str) -> bool {
    let ok = |o: &Output| o.status.success();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
    };
    match git(&["init", "-b", branch]) {
        Ok(o) if ok(&o) => {}
        _ => return false,
    }
    let _ = git(&["config", "user.email", "test@example.com"]);
    let _ = git(&["config", "user.name", "Test"]);
    let _ = git(&[
        "remote",
        "add",
        "origin",
        "https://example.com/acme/demo.git",
    ]);
    true
}

#[test]
fn migrate_sur_base_neuve_est_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());

    // Premier migrate : la base est déjà à jour (init applique les migrations).
    let out = run(home, &["migrate"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("Schéma"), "sortie migrate inattendue : {s:?}");

    // Deuxième migrate : toujours OK, idempotent.
    let out2 = run(home, &["migrate"]);
    assert!(out2.status.success());
    assert!(stdout(&out2).contains("déjà à jour"));
}

#[test]
fn doctor_affiche_la_version_de_schema() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());
    let out = run(home, &["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    let s = stdout(&out);
    assert!(
        s.contains("Schéma SQLite"),
        "doctor doit afficher le schéma : {s:?}"
    );
    assert!(s.contains("v2"), "doctor doit afficher la version : {s:?}");
}

#[test]
fn add_hors_git_fonctionne() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());
    // /tmp n'est pas un dépôt Git : add doit réussir et la commande être trouvée.
    let add = run(home, &["add", "--cmd", "echo hors-git", "--cwd", "/tmp"]);
    assert!(add.status.success());

    let out = run(home, &["search", "hors-git", "--print"]);
    assert!(stdout(&out).contains("echo hors-git"));
}

#[test]
fn add_dans_git_remplit_le_contexte() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let repo = home.join("workspace").join("demo");
    std::fs::create_dir_all(&repo).unwrap();

    if !init_git_repo(&repo, "main") {
        eprintln!("git indisponible : test ignoré");
        return;
    }

    assert!(run(home, &["init"]).status.success());

    let repo_str = repo.to_str().unwrap();
    let add = run(
        home,
        &["add", "--cmd", "cargo build --release", "--cwd", repo_str],
    );
    assert!(add.status.success());

    // Filtre projet : doit retrouver la commande via le nom du dossier racine.
    let by_project = run(home, &["search", "cargo", "--print", "--project", "demo"]);
    assert!(
        stdout(&by_project).contains("cargo build --release"),
        "filtre --project doit retrouver la commande : {:?}",
        stdout(&by_project)
    );

    // Filtre branche : la branche du dépôt est `main`.
    let by_branch = run(home, &["search", "cargo", "--print", "--branch", "main"]);
    assert!(
        stdout(&by_branch).contains("cargo build --release"),
        "filtre --branch doit retrouver la commande : {:?}",
        stdout(&by_branch)
    );

    // Filtre projet inexistant : aucun résultat.
    let none = run(
        home,
        &["search", "cargo", "--print", "--project", "inexistant"],
    );
    assert!(!stdout(&none).contains("cargo build"));
}

#[test]
fn stats_sur_base_vide_et_remplie() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());

    // Base vide : stats doit fonctionner sans planter.
    let empty = run(home, &["stats"]);
    assert!(empty.status.success());
    let es = stdout(&empty);
    assert!(es.contains("Commandes enregistrées : 0"));

    // On ajoute quelques commandes, dont une en échec.
    for args in [
        vec!["add", "--cmd", "cargo build", "--cwd", "/tmp"],
        vec![
            "add",
            "--cmd",
            "cargo test",
            "--cwd",
            "/tmp",
            "--exit-code",
            "1",
        ],
        vec!["add", "--cmd", "git status", "--cwd", "/tmp"],
    ] {
        assert!(run(home, &args).status.success());
    }

    let filled = run(home, &["stats"]);
    assert!(filled.status.success());
    let fs = stdout(&filled);
    assert!(fs.contains("Commandes enregistrées : 3"), "{fs:?}");
    assert!(fs.contains("Commandes en échec     : 1"), "{fs:?}");
    assert!(fs.contains("Top commandes"));
    assert!(fs.contains("cargo"));
}
