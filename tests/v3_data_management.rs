//! Tests d'intégration des fonctionnalités v0.3 : sauvegarde, restauration,
//! export, list, delete et prune.
//!
//! Chaque test s'exécute dans un HOME temporaire isolé (HOME + XDG_*), avec
//! stdin fermé : les opérations destructives sans `--yes` doivent donc être
//! refusées (mode non interactif).

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

fn backups_dir(home: &Path) -> PathBuf {
    home.join(".local/share/mnemo/backups")
}

/// Compte les archives `.tar.gz` présentes dans un dossier (0 si absent).
fn count_archives(dir: &Path) -> usize {
    match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tar.gz"))
            .count(),
        Err(_) => 0,
    }
}

/// Force la date `created_at` d'une commande (pour tester `prune`).
fn backdate(home: &Path, id: i64, date: &str) {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    conn.execute(
        "UPDATE commands SET created_at = ?1 WHERE id = ?2",
        rusqlite::params![date, id],
    )
    .unwrap();
}

/// Prépare un HOME initialisé avec quelques commandes.
fn setup(home: &Path, commands: &[&str]) {
    assert!(run(home, &["init"]).status.success());
    for c in commands {
        assert!(run(home, &["add", "--cmd", c, "--cwd", "/tmp"])
            .status
            .success());
    }
}

/// Initialise un dépôt Git minimal (repris des tests v0.2).
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
    true
}

// ---------------------------------------------------------------------------
// backup
// ---------------------------------------------------------------------------

#[test]
fn backup_cree_une_archive_valide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b", "c"]);

    let out = run(home, &["backup"]);
    assert!(out.status.success());
    assert_eq!(count_archives(&backups_dir(home)), 1);

    // L'archive est exploitable : restore --dry-run la valide et lit son contenu.
    let archive = std::fs::read_dir(backups_dir(home))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let dry = run(home, &["restore", archive.to_str().unwrap(), "--dry-run"]);
    assert!(dry.status.success());
    let s = stdout(&dry);
    assert!(s.contains("Commandes        : 3"), "{s}");
    assert!(s.contains("[dry-run]"), "{s}");
}

#[cfg(unix)]
#[test]
fn backup_cree_une_archive_en_600() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b"]);

    assert!(run(home, &["backup"]).status.success());

    let archive = std::fs::read_dir(backups_dir(home))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mode = std::fs::metadata(&archive).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "archive {} en {:o}", archive.display(), mode);
}

#[test]
fn backup_json_valide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b"]);

    let out = run(home, &["backup", "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert!(v["backup_path"].is_string());
    assert_eq!(v["metadata"]["command_count"], 2);
    assert_eq!(v["metadata"]["schema_version"], 2);
    assert!(v["metadata"]["mnemo_version"].is_string());
}

// ---------------------------------------------------------------------------
// restore
// ---------------------------------------------------------------------------

#[test]
fn restore_dry_run_ne_modifie_rien() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b", "c"]);

    let bk = run(
        home,
        &["backup", "--output", home.join("manual").to_str().unwrap()],
    );
    assert!(bk.status.success());
    let archive = std::fs::read_dir(home.join("manual"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    // Supprime tout puis tente une restauration en dry-run : rien ne change.
    assert!(run(home, &["delete", "1", "--yes"]).status.success());
    let before = run(home, &["list"]);
    let dry = run(home, &["restore", archive.to_str().unwrap(), "--dry-run"]);
    assert!(dry.status.success());
    let after = run(home, &["list"]);
    assert_eq!(stdout(&before), stdout(&after));
}

#[test]
fn restore_refuse_une_archive_invalide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a"]);

    let bad = home.join("bad.tar.gz");
    std::fs::write(&bad, b"ceci n'est pas une archive").unwrap();
    let out = run(home, &["restore", bad.to_str().unwrap(), "--yes"]);
    assert!(!out.status.success());
}

/// Construit une archive `.tar.gz` contenant une entrée dont le nom est écrit
/// directement dans le header (pour simuler une archive malveillante que le
/// builder tar refuserait normalement de produire).
fn evil_archive(path: &Path, entry_name: &str) {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let data = b"evil";
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    {
        let gnu = header.as_gnu_mut().unwrap();
        let b = entry_name.as_bytes();
        gnu.name[..b.len()].copy_from_slice(b);
    }
    header.set_cksum();
    let enc = GzEncoder::new(std::fs::File::create(path).unwrap(), Compression::default());
    let mut builder = tar::Builder::new(enc);
    builder.append(&header, &data[..]).unwrap();
    builder.into_inner().unwrap().finish().unwrap();
}

#[test]
fn restore_refuse_path_traversal_parent() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a"]);

    let evil = home.join("evil.tar.gz");
    evil_archive(&evil, "../evil.txt");
    let out = run(home, &["restore", evil.to_str().unwrap(), "--yes"]);
    assert!(
        !out.status.success(),
        "restore doit refuser une archive avec `..`"
    );
    // Aucune écriture hors du dossier d'extraction.
    assert!(!std::env::temp_dir().join("evil.txt").exists());
}

#[test]
fn restore_refuse_chemin_absolu() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a"]);

    let evil = home.join("evil-abs.tar.gz");
    evil_archive(&evil, "/tmp/mnemo-evil-abs.txt");
    let out = run(home, &["restore", evil.to_str().unwrap(), "--yes"]);
    assert!(
        !out.status.success(),
        "restore doit refuser une archive à chemin absolu"
    );
    assert!(!Path::new("/tmp/mnemo-evil-abs.txt").exists());
}

#[test]
fn restore_cree_un_backup_avant_remplacement() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b"]);

    // Sauvegarde manuelle dans un dossier séparé.
    let manual = home.join("manual");
    assert!(run(home, &["backup", "--output", manual.to_str().unwrap()])
        .status
        .success());
    let archive = std::fs::read_dir(&manual)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    // Le dossier de backups par défaut est vide avant restauration.
    assert_eq!(count_archives(&backups_dir(home)), 0);

    // Modifie l'état courant puis restaure.
    assert!(run(home, &["add", "--cmd", "ajout", "--cwd", "/tmp"])
        .status
        .success());
    let out = run(home, &["restore", archive.to_str().unwrap(), "--yes"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Sauvegarde de sécurité créée"));

    // Une sauvegarde de sécurité a bien été créée automatiquement.
    assert_eq!(count_archives(&backups_dir(home)), 1);
}

// ---------------------------------------------------------------------------
// export
// ---------------------------------------------------------------------------

#[test]
fn export_json_valide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["cargo build", "git status"]);

    let out_file = home.join("export.json");
    let out = run(
        home,
        &[
            "export",
            "--format",
            "json",
            "--output",
            out_file.to_str().unwrap(),
        ],
    );
    assert!(out.status.success());
    let raw = std::fs::read_to_string(&out_file).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr[0]["command"].is_string());
    assert!(arr[0]["id"].is_number());
    assert!(arr[0].get("git_branch").is_some());
}

#[test]
fn export_csv_echappement_correct() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["echo hello, \"world\""]);

    let out = run(home, &["export", "--format", "csv"]);
    assert!(out.status.success());
    let s = stdout(&out);
    let mut lines = s.lines();
    assert_eq!(
        lines.next().unwrap(),
        "id,command,cwd,shell,hostname,exit_code,created_at,git_root,git_branch,git_remote,session_id"
    );
    // La virgule et les guillemets sont correctement échappés (RFC 4180).
    assert!(s.contains("\"echo hello, \"\"world\"\"\""), "{s}");
}

#[test]
fn export_project_filtre() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let repo = home.join("workspace").join("demo");
    std::fs::create_dir_all(&repo).unwrap();
    let has_git = init_git_repo(&repo, "main");
    if !has_git {
        return; // Git indisponible : test ignoré silencieusement.
    }

    assert!(run(home, &["init"]).status.success());
    let repo_str = repo.to_str().unwrap();
    assert!(
        run(home, &["add", "--cmd", "cargo build", "--cwd", repo_str])
            .status
            .success()
    );
    assert!(
        run(home, &["add", "--cmd", "ls hors projet", "--cwd", "/tmp"])
            .status
            .success()
    );

    let out = run(home, &["export", "--format", "json", "--project", "demo"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["command"], "cargo build");
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

#[test]
fn list_affiche_les_ids() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["un", "deux", "trois"]);

    let out = run(home, &["list", "--limit", "20"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains(" 1 ") || s.contains("     1"), "{s}");
    assert!(s.contains("trois"), "{s}");
}

#[test]
fn list_json_valide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["un", "deux"]);

    let out = run(home, &["list", "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr[0]["id"].is_number());
    assert!(arr[0]["command"].is_string());
}

// ---------------------------------------------------------------------------
// delete
// ---------------------------------------------------------------------------

#[test]
fn delete_dry_run_ne_supprime_rien() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b", "c"]);

    let out = run(home, &["delete", "2", "--dry-run"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("[dry-run]"));

    // La commande 2 est toujours là.
    let list = run(home, &["list", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&list)).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 3);
}

#[test]
fn delete_yes_supprime_uniquement_la_cible() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b", "c"]);

    let out = run(home, &["delete", "2", "--yes"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("commande supprimée"));

    let list = run(home, &["list", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&list)).unwrap();
    let ids: Vec<i64> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert!(!ids.contains(&2));
    assert!(ids.contains(&1));
    assert!(ids.contains(&3));
}

#[test]
fn delete_id_inexistant_message_propre() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a"]);

    let out = run(home, &["delete", "9999", "--yes"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Aucune commande avec l'ID 9999"));
}

#[test]
fn delete_sans_yes_en_mode_non_interactif_ne_supprime_pas() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a", "b"]);

    // stdin fermé (Stdio::null) → mode non interactif → refus.
    let out = run(home, &["delete", "1"]);
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    assert!(stderr.contains("entrée non interactive"), "{stderr}");
    assert!(stdout(&out).contains("Suppression annulée"));

    let list = run(home, &["list", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&list)).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// prune
// ---------------------------------------------------------------------------

#[test]
fn prune_dry_run_ne_supprime_rien() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["vieux", "recent"]);
    backdate(home, 1, "2020-01-01 00:00:00");

    let out = run(home, &["prune", "--older-than", "30d", "--dry-run"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("seront supprimées"), "{s}");
    assert!(s.contains("[dry-run]"), "{s}");

    let list = run(home, &["list", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&list)).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[test]
fn prune_yes_supprime_les_bonnes_lignes() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["vieux", "recent"]);
    backdate(home, 1, "2020-01-01 00:00:00");

    let out = run(home, &["prune", "--older-than", "30d", "--yes"]);
    assert!(out.status.success());

    let list = run(home, &["list", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&list)).unwrap();
    let ids: Vec<i64> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![2]); // seule la commande récente subsiste.
}

#[test]
fn prune_respecte_le_filtre_projet() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let repo = home.join("workspace").join("demo");
    std::fs::create_dir_all(&repo).unwrap();
    let has_git = init_git_repo(&repo, "main");
    if !has_git {
        return;
    }

    assert!(run(home, &["init"]).status.success());
    let repo_str = repo.to_str().unwrap();
    assert!(
        run(home, &["add", "--cmd", "old projet", "--cwd", repo_str])
            .status
            .success()
    );
    assert!(
        run(home, &["add", "--cmd", "old hors projet", "--cwd", "/tmp"])
            .status
            .success()
    );
    // Les deux sont anciennes.
    backdate(home, 1, "2020-01-01 00:00:00");
    backdate(home, 2, "2020-01-01 00:00:00");

    let out = run(
        home,
        &["prune", "--project", "demo", "--older-than", "30d", "--yes"],
    );
    assert!(out.status.success());

    let list = run(home, &["list", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&list)).unwrap();
    let ids: Vec<i64> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    // Seule la commande du projet "demo" (id 1) a été supprimée.
    assert_eq!(ids, vec![2]);
}

#[test]
fn prune_sans_resultat_message_propre() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["recent"]);

    let out = run(home, &["prune", "--older-than", "365d", "--yes"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Aucune commande antérieure"));
}

#[test]
fn prune_duree_invalide_echoue() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    setup(home, &["a"]);

    let out = run(home, &["prune", "--older-than", "30x", "--yes"]);
    assert!(!out.status.success());
}

// ---------------------------------------------------------------------------
// broken pipe (v0.3.1)
// ---------------------------------------------------------------------------

/// Exécute `mnemo <args>` et redirige sa sortie standard vers `head -n <lines>`,
/// puis renvoie `(code de sortie de mnemo, stderr de mnemo)`.
///
/// `head` ferme le tube après avoir lu assez de lignes : mnemo doit alors
/// s'arrêter silencieusement (code 0) sans afficher « Broken pipe ».
fn pipe_through_head(home: &Path, args: &[&str], lines: usize) -> (i32, String) {
    let mut child = mnemo(home)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mnemo_stdout: Stdio = child.stdout.take().unwrap().into();
    let head = Command::new("head")
        .args(["-n", &lines.to_string()])
        .stdin(mnemo_stdout)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();

    // On attend la fin de `head` d'abord (il ferme le tube), puis celle de mnemo.
    let _ = head.wait_with_output().unwrap();
    let out = child.wait_with_output().unwrap();
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stderr)
}

#[test]
fn export_json_pipe_vers_head_ne_casse_pas() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let commands: Vec<String> = (1..=200).map(|i| format!("commande numero {i}")).collect();
    let refs: Vec<&str> = commands.iter().map(|s| s.as_str()).collect();
    setup(home, &refs);

    let (code, stderr) = pipe_through_head(home, &["export", "--format", "json"], 20);
    assert_eq!(code, 0, "mnemo doit sortir proprement (stderr: {stderr})");
    assert!(
        !stderr.to_lowercase().contains("broken pipe"),
        "stderr ne doit pas mentionner broken pipe: {stderr}"
    );
}

#[test]
fn export_csv_pipe_vers_head_ne_casse_pas() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let commands: Vec<String> = (1..=200).map(|i| format!("commande numero {i}")).collect();
    let refs: Vec<&str> = commands.iter().map(|s| s.as_str()).collect();
    setup(home, &refs);

    let (code, stderr) = pipe_through_head(home, &["export", "--format", "csv"], 5);
    assert_eq!(code, 0, "mnemo doit sortir proprement (stderr: {stderr})");
    assert!(!stderr.to_lowercase().contains("broken pipe"), "{stderr}");
}

#[test]
fn list_pipe_vers_head_ne_casse_pas() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let commands: Vec<String> = (1..=200).map(|i| format!("commande numero {i}")).collect();
    let refs: Vec<&str> = commands.iter().map(|s| s.as_str()).collect();
    setup(home, &refs);

    let (code, stderr) = pipe_through_head(home, &["list", "--limit", "200"], 5);
    assert_eq!(code, 0, "mnemo doit sortir proprement (stderr: {stderr})");
    assert!(!stderr.to_lowercase().contains("broken pipe"), "{stderr}");
}
