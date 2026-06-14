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

#[test]
fn stats_ne_montre_plus_les_tokens_parasites() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());
    for args in [
        vec!["add", "--cmd", "sudo apt update", "--cwd", "/tmp"],
        vec![
            "add",
            "--cmd",
            "env RUST_LOG=debug cargo test",
            "--cwd",
            "/tmp",
        ],
        vec!["add", "--cmd", "/usr/bin/git status", "--cwd", "/tmp"],
    ] {
        assert!(run(home, &args).status.success());
    }

    let out = run(home, &["stats"]);
    assert!(out.status.success());
    let s = stdout(&out);
    // Les noms normalisés apparaissent…
    assert!(s.contains("apt"), "{s:?}");
    assert!(s.contains("cargo"), "{s:?}");
    assert!(s.contains("git"), "{s:?}");
    // …et aucun token parasite ne pollue le Top commandes.
    for junk in ["  sudo", "  env", "  |", "  -\n", "/usr/bin/git"] {
        assert!(
            !s.contains(junk),
            "token parasite trouvé ({junk:?}) : {s:?}"
        );
    }
}

#[test]
fn stats_json_est_valide_et_filtre() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let repo = home.join("workspace").join("demo");
    std::fs::create_dir_all(&repo).unwrap();

    let has_git = init_git_repo(&repo, "main");

    assert!(run(home, &["init"]).status.success());
    let repo_str = repo.to_str().unwrap();
    assert!(
        run(home, &["add", "--cmd", "cargo build", "--cwd", repo_str])
            .status
            .success()
    );

    // JSON valide et bien structuré.
    let js = run(home, &["stats", "--json"]);
    assert!(js.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&js)).unwrap();
    assert!(v["total_commands"].is_number());
    assert!(v["top_commands"].is_array());
    assert!(v["filters"].is_object());

    if has_git {
        // Filtre projet en texte.
        let by_proj = run(home, &["stats", "--project", "demo"]);
        assert!(by_proj.status.success());
        assert!(stdout(&by_proj).contains("cargo"));

        // Filtre projet en JSON.
        let pj = run(home, &["stats", "--project", "demo", "--json"]);
        let pv: serde_json::Value = serde_json::from_str(&stdout(&pj)).unwrap();
        assert_eq!(pv["filters"]["project"], "demo");

        // Filtre branche.
        let by_branch = run(home, &["stats", "--branch", "main"]);
        assert!(by_branch.status.success());
        assert!(stdout(&by_branch).contains("cargo"));

        // Filtre sans résultat : message propre.
        let none = run(home, &["stats", "--project", "inexistant"]);
        assert!(none.status.success());
        assert!(stdout(&none).contains("Aucune commande trouvée pour ces filtres."));
    }
}

#[test]
fn init_cree_la_section_stats() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());
    let cfg = std::fs::read_to_string(home.join(".config/mnemo/config.toml")).unwrap();
    assert!(cfg.contains("[stats]"), "{cfg:?}");
    assert!(cfg.contains("ignored_commands"), "{cfg:?}");
}

#[test]
fn config_sans_section_stats_reste_compatible() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // Écrit une config « ancienne » sans section [stats].
    let cfg_dir = home.join(".config/mnemo");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(
        cfg_dir.join("config.toml"),
        "sensitive_keywords = [\"password\"]\nignore_prefixes = [\"mnemo\"]\nsearch_limit = 5000\n",
    )
    .unwrap();

    // stats et list doivent fonctionner sans erreur.
    let out = run(home, &["config", "stats-ignore", "list"]);
    assert!(out.status.success(), "{:?}", out);
    assert!(stdout(&out).contains("Aucune commande ignorée configurée."));
}

#[test]
fn config_stats_ignore_add_remove_list() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());

    // Liste vide au départ.
    let list0 = run(home, &["config", "stats-ignore", "list"]);
    assert!(stdout(&list0).contains("Aucune commande ignorée configurée."));

    // Ajout.
    let add = run(home, &["config", "stats-ignore", "add", "create_dir"]);
    assert!(add.status.success());
    assert!(stdout(&add).contains("Commande ignorée ajoutée : create_dir"));

    // Ajout idempotent (pas de doublon).
    let add2 = run(home, &["config", "stats-ignore", "add", "create_dir"]);
    assert!(stdout(&add2).contains("Commande déjà présente : create_dir"));

    // Liste contient la commande.
    let list1 = run(home, &["config", "stats-ignore", "list"]);
    assert!(stdout(&list1).contains("create_dir"));

    // La config ne contient qu'une seule occurrence.
    let cfg = std::fs::read_to_string(home.join(".config/mnemo/config.toml")).unwrap();
    assert_eq!(cfg.matches("create_dir").count(), 1, "{cfg:?}");

    // Retrait.
    let rm = run(home, &["config", "stats-ignore", "remove", "create_dir"]);
    assert!(stdout(&rm).contains("Commande retirée : create_dir"));

    // Retrait d'une commande absente.
    let rm2 = run(home, &["config", "stats-ignore", "remove", "create_dir"]);
    assert!(stdout(&rm2).contains("Commande absente : create_dir"));
}

#[test]
fn stats_respecte_la_config_ignored_commands() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    assert!(run(home, &["init"]).status.success());
    for args in [
        vec!["add", "--cmd", "create_dir foo", "--cwd", "/tmp"],
        vec!["add", "--cmd", "create_dir bar", "--cwd", "/tmp"],
        vec!["add", "--cmd", "cargo build", "--cwd", "/tmp"],
    ] {
        assert!(run(home, &args).status.success());
    }

    // Avant config : create_dir apparaît dans le Top.
    assert!(stdout(&run(home, &["stats"])).contains("create_dir"));

    // On ignore create_dir.
    assert!(run(home, &["config", "stats-ignore", "add", "create_dir"])
        .status
        .success());

    let out = run(home, &["stats"]);
    assert!(out.status.success());
    let s = stdout(&out);
    // create_dir n'apparaît plus dans le Top commandes…
    assert!(!s.contains("  create_dir"), "{s:?}");
    // …mais cargo reste présent et le total est inchangé.
    assert!(s.contains("cargo"), "{s:?}");
    assert!(s.contains("Commandes enregistrées : 3"), "{s:?}");
    assert!(
        s.contains("Entrées ignorées dans le Top commandes : 2"),
        "{s:?}"
    );

    // JSON expose la config.
    let js = run(home, &["stats", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&js)).unwrap();
    assert_eq!(v["ignored_commands_config"][0], "create_dir");
    assert_eq!(v["ignored_for_top_commands"], 2);

    // doctor signale la commande ignorée.
    let doc = run(home, &["doctor"]);
    assert!(stdout(&doc).contains("Commandes ignorées dans stats : create_dir"));
}
