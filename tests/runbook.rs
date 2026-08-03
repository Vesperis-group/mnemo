//! Tests d'intégration de `mnemo runbook`.

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

fn add_with_session(home: &Path, session: &str, cmd: &str) {
    let out = mnemo(home)
        .env("MNEMO_SESSION_ID", session)
        .args(["add", "--cmd", cmd, "--cwd", "/home/user/proj"])
        .output()
        .unwrap();
    assert!(out.status.success(), "add failed for {cmd}: {out:?}");
}

fn seed_project(home: &Path, cmd: &str, root: &str, created_at: &str) {
    assert!(run(home, &["add", "--cmd", cmd, "--cwd", root])
        .status
        .success());
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    let n = conn
        .execute(
            "UPDATE commands SET git_root = ?1, created_at = ?2 WHERE command = ?3",
            rusqlite::params![root, created_at, cmd],
        )
        .unwrap();
    assert!(n >= 1, "commande absente : {cmd}");
}

fn set_date(home: &Path, command: &str, date: &str) {
    let conn = rusqlite::Connection::open(db_path(home)).unwrap();
    let n = conn
        .execute(
            "UPDATE commands SET created_at = ?1 WHERE command = ?2",
            rusqlite::params![date, command],
        )
        .unwrap();
    assert!(n >= 1, "commande absente : {command}");
}

#[test]
fn last_retourne_les_commandes_de_la_derniere_session() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-a", "cargo build");
    add_with_session(home, "sess-a", "cargo test");
    set_date(home, "cargo build", "2026-06-20 10:00:00");
    set_date(home, "cargo test", "2026-06-20 10:05:00");

    let out = run(home, &["runbook", "--last"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(md.contains("# Runbook -"), "titre manquant");
    assert!(md.contains("## Metadata"), "section Metadata manquante");
    assert!(md.contains("## Commands"), "section Commands manquante");
    assert!(md.contains("cargo build"), "cargo build absent");
    assert!(md.contains("cargo test"), "cargo test absent");
}

#[test]
fn last_sans_session_echoue_proprement() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    assert!(run(home, &["add", "--cmd", "ls -la", "--cwd", "/tmp"])
        .status
        .success());

    let out = run(home, &["runbook", "--last"]);
    assert!(
        !out.status.success(),
        "--last sans session doit échouer proprement"
    );
}

#[test]
fn session_retourne_les_bonnes_commandes() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-x", "git status");
    add_with_session(home, "sess-x", "git diff");
    add_with_session(home, "sess-y", "npm install");
    set_date(home, "git status", "2026-06-21 09:00:00");
    set_date(home, "git diff", "2026-06-21 09:01:00");
    set_date(home, "npm install", "2026-06-21 10:00:00");

    let out = run(home, &["runbook", "--session", "sess-x"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(md.contains("git status"), "git status absent");
    assert!(md.contains("git diff"), "git diff absent");
    assert!(
        !md.contains("npm install"),
        "npm install ne doit pas apparaître"
    );
}

#[test]
fn session_inexistante_echoue_proprement() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add_with_session(home, "sess-a", "cargo build");

    let out = run(home, &["runbook", "--session", "inconnue"]);
    assert!(
        !out.status.success(),
        "session inexistante doit échouer proprement"
    );
}

#[test]
fn project_par_nom_retourne_les_commandes() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    seed_project(
        home,
        "make build",
        "/home/user/myproject",
        "2026-06-22 08:00:00",
    );
    seed_project(
        home,
        "make test",
        "/home/user/myproject",
        "2026-06-22 08:05:00",
    );
    seed_project(home, "npm start", "/home/user/other", "2026-06-22 09:00:00");

    let out = run(home, &["runbook", "--project", "myproject"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(md.contains("make build"), "make build absent");
    assert!(md.contains("make test"), "make test absent");
    assert!(
        !md.contains("npm start"),
        "npm start ne doit pas apparaître"
    );
}

#[test]
fn project_inexistant_echoue_proprement() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    seed_project(home, "ls", "/home/user/proj", "2026-06-22 08:00:00");

    let out = run(home, &["runbook", "--project", "inexistant"]);
    assert!(
        !out.status.success(),
        "projet inexistant doit échouer proprement"
    );
}

#[test]
fn output_cree_le_fichier() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add_with_session(home, "sess-a", "cargo build");

    let target = home.join("runbook.md");
    let out = mnemo(home)
        .args(["runbook", "--last", "--output"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(target.exists(), "le fichier de sortie doit exister");
    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("# Runbook -"), "contenu invalide");
    assert!(content.contains("cargo build"));
}

#[test]
fn output_refuse_d_ecraser_sans_force() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add_with_session(home, "sess-a", "cargo build");

    let target = home.join("runbook.md");
    std::fs::write(&target, "CONTENU EXISTANT").unwrap();

    let out = mnemo(home)
        .args(["runbook", "--last", "--output"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "écrasement sans --force doit échouer"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "CONTENU EXISTANT",
        "le fichier existant ne doit pas être modifié"
    );
}

#[test]
fn output_force_ecrase_le_fichier_existant() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add_with_session(home, "sess-a", "cargo build");

    let target = home.join("runbook.md");
    std::fs::write(&target, "CONTENU EXISTANT").unwrap();

    let out = mnemo(home)
        .args(["runbook", "--last", "--force", "--output"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("# Runbook -"));
    assert!(
        !content.contains("CONTENU EXISTANT"),
        "l'ancien contenu doit être remplacé"
    );
}

#[test]
fn limit_borne_le_nombre_de_commandes() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-a", "cmd-one");
    add_with_session(home, "sess-a", "cmd-two");
    add_with_session(home, "sess-a", "cmd-three");
    set_date(home, "cmd-one", "2026-06-20 10:00:00");
    set_date(home, "cmd-two", "2026-06-20 10:01:00");
    set_date(home, "cmd-three", "2026-06-20 10:02:00");

    let out = run(home, &["runbook", "--last", "--limit", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(md.contains("cmd-one"), "cmd-one doit être incluse");
    assert!(md.contains("cmd-two"), "cmd-two doit être incluse");
    assert!(
        !md.contains("cmd-three"),
        "--limit 2 ne doit pas inclure cmd-three"
    );
    assert!(md.contains("Commands: 2"), "Commands: 2 attendu");
}

#[test]
fn last_et_session_sont_mutuellement_exclusifs() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add_with_session(home, "sess-a", "cargo build");

    let out = run(home, &["runbook", "--last", "--session", "sess-a"]);
    assert!(
        !out.status.success(),
        "--last et --session doivent être mutuellement exclusifs"
    );
}

#[test]
fn last_et_project_sont_mutuellement_exclusifs() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    let out = run(home, &["runbook", "--last", "--project", "mon-projet"]);
    assert!(
        !out.status.success(),
        "--last et --project doivent être mutuellement exclusifs"
    );
}

#[test]
fn aucune_source_produit_une_erreur_claire() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    let out = run(home, &["runbook"]);
    assert!(
        !out.status.success(),
        "aucune source doit produire une erreur"
    );
    let err = stderr(&out);
    assert!(
        err.contains("--last") || err.contains("--session") || err.contains("--project"),
        "le message d'erreur doit mentionner les options disponibles : {err}"
    );
}

#[test]
fn format_json_produit_un_json_valide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-j", "cargo build");
    add_with_session(home, "sess-j", "cargo test");

    let out = run(home, &["runbook", "--last", "--format", "json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    let v: serde_json::Value = serde_json::from_str(&text).expect("JSON invalide");
    assert!(v["title"].is_string(), "champ title manquant");
    assert!(v["source"].is_string(), "champ source manquant");
    assert!(v["generated_at"].is_string(), "champ generated_at manquant");
    assert!(v["commands"].is_array(), "champ commands manquant");
    assert_eq!(
        v["commands"].as_array().unwrap().len(),
        2,
        "2 commandes attendues"
    );
}

#[test]
fn format_json_champ_command_present() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-j2", "git status");

    let out = run(home, &["runbook", "--last", "--format", "json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let cmd = &v["commands"][0];
    assert!(cmd["command"].is_string());
    assert!(cmd["cwd"].is_string());
    assert!(cmd["timestamp"].is_string());
    assert_eq!(cmd["n"], 1);
}

#[test]
fn redaction_activee_par_defaut() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(
        home,
        "sess-r",
        "curl https://user:Xk7abc@api.example.com/data",
    );

    let out = run(home, &["runbook", "--last"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(
        !md.contains("Xk7abc"),
        "le mot de passe brut ne doit pas apparaître dans la sortie redactée"
    );
    assert!(
        md.contains("[REDACTED]") || md.contains("[REDACTED COMMAND]"),
        "marque de redaction attendue"
    );
}

#[test]
fn no_redact_desactive_la_redaction() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(
        home,
        "sess-nr",
        "curl https://user:Xk7abc@api.example.com/data",
    );

    let out = run(home, &["runbook", "--last", "--no-redact"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(
        md.contains("Xk7abc"),
        "le mot de passe brut doit être conservé avec --no-redact"
    );
}

#[test]
fn group_by_none_liste_plate() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-g", "cmd-alpha");
    add_with_session(home, "sess-g", "cmd-beta");
    set_date(home, "cmd-alpha", "2026-06-25 10:00:00");
    set_date(home, "cmd-beta", "2026-06-25 10:01:00");

    let out = run(home, &["runbook", "--last", "--group-by", "none"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(md.contains("### 1."), "numérotation plate attendue");
    assert!(md.contains("### 2."), "numérotation plate attendue");
    assert!(md.contains("cmd-alpha"));
    assert!(md.contains("cmd-beta"));
}

#[test]
fn group_by_cwd_groupe_les_sections() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    let out = mnemo(home)
        .env("MNEMO_SESSION_ID", "sess-gcwd")
        .args(["add", "--cmd", "make build", "--cwd", "/home/user/foo"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = mnemo(home)
        .env("MNEMO_SESSION_ID", "sess-gcwd")
        .args(["add", "--cmd", "make test", "--cwd", "/home/user/foo"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = mnemo(home)
        .env("MNEMO_SESSION_ID", "sess-gcwd")
        .args(["add", "--cmd", "npm install", "--cwd", "/home/user/bar"])
        .output()
        .unwrap();
    assert!(out.status.success());
    set_date(home, "make build", "2026-06-25 10:00:00");
    set_date(home, "make test", "2026-06-25 10:01:00");
    set_date(home, "npm install", "2026-06-25 10:02:00");

    let out = run(home, &["runbook", "--last", "--group-by", "cwd"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(md.contains("## "), "sections de groupe attendues");
    assert!(md.contains("make build"));
    assert!(md.contains("make test"));
    assert!(md.contains("npm install"));
    let foo_pos = md.find("foo").unwrap_or(usize::MAX);
    let bar_pos = md.find("bar").unwrap_or(usize::MAX);
    assert!(foo_pos < usize::MAX, "groupe foo attendu");
    assert!(bar_pos < usize::MAX, "groupe bar attendu");
}

#[test]
fn group_by_json_ajoute_champ_group() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    let out = mnemo(home)
        .env("MNEMO_SESSION_ID", "sess-gjson")
        .args(["add", "--cmd", "cmd-a", "--cwd", "/home/user/pa"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = mnemo(home)
        .env("MNEMO_SESSION_ID", "sess-gjson")
        .args(["add", "--cmd", "cmd-b", "--cwd", "/home/user/pb"])
        .output()
        .unwrap();
    assert!(out.status.success());
    set_date(home, "cmd-a", "2026-06-25 10:00:00");
    set_date(home, "cmd-b", "2026-06-25 10:01:00");

    let out = run(
        home,
        &["runbook", "--last", "--format", "json", "--group-by", "cwd"],
    );
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let cmds = v["commands"].as_array().unwrap();
    for cmd in cmds {
        assert!(
            cmd["group"].is_string(),
            "champ group manquant dans la commande JSON"
        );
    }
}

#[test]
fn group_by_project_json_ajoute_champ_group() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    seed_project(
        home,
        "cmd-pa",
        "/home/user/project-a",
        "2026-06-25 10:00:00",
    );
    seed_project(
        home,
        "cmd-pb",
        "/home/user/project-a",
        "2026-06-25 10:01:00",
    );

    let out = run(
        home,
        &[
            "runbook",
            "--project",
            "project-a",
            "--format",
            "json",
            "--group-by",
            "project",
        ],
    );
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let cmds = v["commands"].as_array().unwrap();
    assert_eq!(cmds.len(), 2, "2 commandes attendues");
    for cmd in cmds {
        assert!(
            cmd["group"].is_string(),
            "champ group manquant dans la commande JSON"
        );
        assert!(
            cmd["group"].as_str().unwrap().contains("project-a"),
            "group attendu sur project-a"
        );
    }
}

#[test]
fn markdown_ordre_chronologique_croissant() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-a", "premier");
    add_with_session(home, "sess-a", "deuxieme");
    set_date(home, "premier", "2026-06-20 10:00:00");
    set_date(home, "deuxieme", "2026-06-20 10:05:00");

    let out = run(home, &["runbook", "--last"]);
    assert!(out.status.success());
    let md = stdout(&out);
    let pos_premier = md.find("premier").unwrap();
    let pos_deuxieme = md.find("deuxieme").unwrap();
    assert!(
        pos_premier < pos_deuxieme,
        "ordre chronologique croissant attendu"
    );
}

#[test]
fn markdown_contient_le_titre_personnalise() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add_with_session(home, "sess-a", "cargo build");

    let out = run(home, &["runbook", "--last", "--title", "Mon Super Runbook"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    assert!(
        md.contains("# Runbook - Mon Super Runbook"),
        "titre personnalisé absent"
    );
}

#[test]
fn aucune_commande_vide_dans_la_sortie() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);

    add_with_session(home, "sess-a", "cargo build");

    let out = run(home, &["runbook", "--last"]);
    assert!(out.status.success());
    let md = stdout(&out);
    // Aucun bloc de code bash vide (contient uniquement le fence).
    let lines: Vec<&str> = md.lines().collect();
    let mut in_block = false;
    let mut empty_block = false;
    for line in &lines {
        if line.starts_with("```bash") {
            in_block = true;
            empty_block = true;
        } else if in_block && line.starts_with("```") {
            if empty_block {
                panic!("bloc de code vide détecté dans le runbook");
            }
            in_block = false;
        } else if in_block && !line.trim().is_empty() {
            empty_block = false;
        }
    }
}

#[test]
fn caracteres_speciaux_ne_cassent_pas_le_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    add_with_session(home, "sess-a", "echo `date`");
    add_with_session(home, "sess-a", "grep -E 'a|b' fichier");

    let out = run(home, &["runbook", "--last"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let md = stdout(&out);
    // Les blocs de code doivent être équilibrés (nombre pair de lignes fence).
    let fences = md.lines().filter(|l| l.starts_with("```")).count();
    assert_eq!(fences % 2, 0, "blocs de code non équilibrés");
    assert!(md.contains("## Commands"), "section Commands manquante");
}
