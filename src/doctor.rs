//! Commande `mnemo doctor` : diagnostic de l'installation locale.
//!
//! En mode simple, `doctor` ne modifie **jamais** le système : il se contente
//! d'inspecter la configuration, la base, le `PATH` et le `.bashrc`. Le mode
//! `--fix` répare les éléments manquants de façon non destructive (création de
//! la config / base, ajout du bloc `.bashrc` avec sauvegarde) ; il ne supprime
//! jamais de données.

use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::{config, db, migrations, shell};

/// Niveau de chaque contrôle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Error,
    Info,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Warn => "WARN",
            Status::Error => "ERROR",
            Status::Info => "INFO",
        }
    }
}

/// Résultat d'un contrôle individuel.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub message: String,
}

impl Check {
    fn new(name: &str, status: Status, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status,
            message: message.into(),
        }
    }
}

/// Rapport complet (liste de contrôles).
#[derive(Debug, Default)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    fn push(&mut self, name: &str, status: Status, message: impl Into<String>) {
        self.checks.push(Check::new(name, status, message));
    }

    fn count(&self, status: Status) -> usize {
        self.checks.iter().filter(|c| c.status == status).count()
    }

    /// Code de sortie : 1 si au moins une erreur bloquante, sinon 0.
    pub fn exit_code(&self) -> i32 {
        if self.count(Status::Error) > 0 {
            1
        } else {
            0
        }
    }
}

/// Point d'entrée de la commande. Retourne le code de sortie attendu.
pub fn run(fix: bool, json: bool) -> Result<i32> {
    let mut report = Report::default();

    if fix {
        apply_fixes(&mut report)?;
    }

    collect_checks(&mut report)?;

    if json {
        println!("{}", render_json(&report));
    } else {
        render_text(&report);
    }

    Ok(report.exit_code())
}

// ---------------------------------------------------------------------------
// Mode --fix (non destructif).
// ---------------------------------------------------------------------------

fn apply_fixes(report: &mut Report) -> Result<()> {
    // 1. Config.
    let cfg_path = config::config_path()?;
    if cfg_path.exists() {
        report.push("fix.config", Status::Info, "Configuration déjà présente");
    } else {
        config::Config::default().save(&cfg_path)?;
        report.push(
            "fix.config",
            Status::Ok,
            format!("Configuration créée : {}", cfg_path.display()),
        );
    }

    // 2. Base de données (création du schéma si absente).
    let db_path = config::db_path()?;
    let db_existait = db_path.exists();
    db::open(&db_path)?;
    if db_existait {
        report.push("fix.db", Status::Info, "Base de données déjà présente");
    } else {
        report.push(
            "fix.db",
            Status::Ok,
            format!("Base de données créée : {}", db_path.display()),
        );
    }

    // 3. Bloc .bashrc (ajout avec sauvegarde, sans doublon).
    if let Some(bashrc) = bashrc_path() {
        match shell::install_block(&bashrc) {
            Ok(true) => report.push(
                "fix.bashrc",
                Status::Ok,
                "Bloc mnemo ajouté au .bashrc (sauvegarde créée)",
            ),
            Ok(false) => report.push(
                "fix.bashrc",
                Status::Info,
                "Bloc mnemo déjà présent (aucune modification)",
            ),
            Err(e) => report.push(
                "fix.bashrc",
                Status::Warn,
                format!("Impossible de modifier le .bashrc : {e}"),
            ),
        }
    }

    // 4. PATH : message clair, jamais de modification automatique.
    if let Some(local_bin) = local_bin_dir() {
        if !path_contains(&local_bin) {
            report.push(
                "fix.path",
                Status::Warn,
                format!(
                    "{} n'est pas dans le PATH. Ajoutez à votre ~/.bashrc : export PATH=\"$HOME/.local/bin:$PATH\"",
                    local_bin.display()
                ),
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Contrôles de diagnostic (lecture seule).
// ---------------------------------------------------------------------------

fn collect_checks(report: &mut Report) -> Result<()> {
    check_binary(report);
    check_local_bin_path(report);
    check_config(report)?;
    check_database(report)?;
    check_bashrc(report);
    check_shell(report);
    check_histtimeformat(report);
    Ok(())
}

fn check_binary(report: &mut Report) {
    report.push(
        "binary.version",
        Status::Info,
        format!("mnemo version {}", env!("CARGO_PKG_VERSION")),
    );

    match find_in_path("mnemo") {
        Some(p) => report.push(
            "binary.path",
            Status::Ok,
            format!("Binaire trouvé dans le PATH : {}", p.display()),
        ),
        None => report.push(
            "binary.path",
            Status::Warn,
            "Binaire mnemo introuvable dans le PATH (installez-le dans ~/.local/bin)",
        ),
    }
}

fn check_local_bin_path(report: &mut Report) {
    if let Some(local_bin) = local_bin_dir() {
        if path_contains(&local_bin) {
            report.push(
                "path.local_bin",
                Status::Ok,
                format!("{} est dans le PATH", local_bin.display()),
            );
        } else {
            report.push(
                "path.local_bin",
                Status::Warn,
                format!("{} n'est pas dans le PATH", local_bin.display()),
            );
        }
    }
}

fn check_config(report: &mut Report) -> Result<()> {
    let cfg_path = config::config_path()?;
    if cfg_path.exists() {
        report.push(
            "config.file",
            Status::Ok,
            format!("Configuration présente : {}", cfg_path.display()),
        );
        check_permissions(report, "config.perms", &cfg_path);
    } else {
        report.push(
            "config.file",
            Status::Warn,
            format!(
                "Configuration absente : {} (lancez `mnemo init` ou `mnemo doctor --fix`)",
                cfg_path.display()
            ),
        );
    }

    let cfg = config::Config::load()?;
    if cfg.stats.ignored_commands.is_empty() {
        report.push(
            "config.stats_ignore",
            Status::Info,
            "Aucune commande ignorée dans stats".to_string(),
        );
    } else {
        report.push(
            "config.stats_ignore",
            Status::Info,
            format!(
                "Commandes ignorées dans stats : {}",
                cfg.stats.ignored_commands.join(", ")
            ),
        );
    }
    Ok(())
}

fn check_database(report: &mut Report) -> Result<()> {
    let db_path = config::db_path()?;
    if !db_path.exists() {
        report.push(
            "db.file",
            Status::Warn,
            format!(
                "Base absente : {} (lancez `mnemo import` ou `mnemo doctor --fix`)",
                db_path.display()
            ),
        );
        return Ok(());
    }

    report.push(
        "db.file",
        Status::Ok,
        format!("Base présente : {}", db_path.display()),
    );
    check_permissions(report, "db.perms", &db_path);

    // Ouverture en lecture seule (ne modifie jamais la base).
    let conn = match db::open_readonly(&db_path) {
        Ok(c) => c,
        Err(e) => {
            report.push(
                "db.open",
                Status::Error,
                format!("Base illisible / corrompue : {e}"),
            );
            return Ok(());
        }
    };

    match db::table_exists(&conn, "commands") {
        Ok(true) => {
            report.push("db.table", Status::Ok, "Table `commands` présente");
            check_schema_version(report, &conn);
            match db::count(&conn) {
                Ok(n) => report.push(
                    "db.count",
                    Status::Info,
                    format!("{n} commande(s) enregistrée(s)"),
                ),
                Err(e) => report.push(
                    "db.count",
                    Status::Error,
                    format!("Lecture du nombre de commandes impossible : {e}"),
                ),
            }
        }
        Ok(false) => report.push(
            "db.table",
            Status::Error,
            "Table `commands` absente (base invalide)",
        ),
        Err(e) => report.push(
            "db.open",
            Status::Error,
            format!("Base illisible / corrompue : {e}"),
        ),
    }

    Ok(())
}

/// Vérifie la version du schéma SQLite (`PRAGMA user_version`) sans jamais
/// migrer : `doctor` reste en lecture seule. Signale si une migration est
/// nécessaire, ou si la base provient d'une version plus récente de mnemo.
fn check_schema_version(report: &mut Report, conn: &rusqlite::Connection) {
    let expected = migrations::SCHEMA_VERSION;
    match migrations::schema_version(conn) {
        Ok(current) => {
            report.push(
                "db.schema",
                Status::Info,
                format!("Schéma SQLite : v{current} (attendu v{expected})"),
            );
            if current < expected {
                report.push(
                    "db.schema.migration",
                    Status::Warn,
                    "Migration nécessaire : lancez `mnemo migrate` (ou toute commande mnemo l'applique automatiquement)",
                );
            } else if current > expected {
                report.push(
                    "db.schema.migration",
                    Status::Error,
                    format!(
                        "Base créée par une version plus récente (schéma v{current} > v{expected}) : mettez mnemo à jour"
                    ),
                );
            } else {
                report.push(
                    "db.schema.migration",
                    Status::Ok,
                    "Schéma à jour, aucune migration nécessaire",
                );
            }
        }
        Err(e) => report.push(
            "db.schema",
            Status::Error,
            format!("Lecture de la version de schéma impossible : {e}"),
        ),
    }
}

fn check_bashrc(report: &mut Report) {
    let Some(bashrc) = bashrc_path() else {
        return;
    };

    if !bashrc.exists() {
        report.push(
            "bashrc.file",
            Status::Warn,
            format!("{} introuvable", bashrc.display()),
        );
        return;
    }
    report.push(
        "bashrc.file",
        Status::Ok,
        format!("{} présent", bashrc.display()),
    );

    let content = std::fs::read_to_string(&bashrc).unwrap_or_default();

    if shell::has_block(&content) {
        report.push(
            "bashrc.block",
            Status::Ok,
            "Bloc d'intégration mnemo présent",
        );
        let n = shell::count_blocks(&content);
        if n > 1 {
            report.push(
                "bashrc.duplicate",
                Status::Warn,
                format!("Bloc mnemo dupliqué {n} fois (gardez-en un seul)"),
            );
        } else {
            report.push("bashrc.duplicate", Status::Ok, "Bloc mnemo unique");
        }

        if shell::has_ctrl_r_bind(&content) {
            report.push("bashrc.ctrl_r", Status::Ok, "Raccourci Ctrl+R configuré");
        } else {
            report.push(
                "bashrc.ctrl_r",
                Status::Warn,
                "Raccourci Ctrl+R absent du bloc mnemo",
            );
        }
    } else {
        report.push(
            "bashrc.block",
            Status::Warn,
            "Bloc d'intégration mnemo absent (lancez `mnemo doctor --fix`)",
        );
    }
}

fn check_shell(report: &mut Report) {
    match std::env::var("SHELL") {
        Ok(sh) if sh.ends_with("bash") => {
            report.push("shell.current", Status::Ok, format!("Shell courant : {sh}"))
        }
        Ok(sh) => report.push(
            "shell.current",
            Status::Warn,
            format!("Shell courant : {sh} (mnemo ne supporte que Bash pour l'instant)"),
        ),
        Err(_) => report.push("shell.current", Status::Warn, "Variable $SHELL non définie"),
    }
}

fn check_histtimeformat(report: &mut Report) {
    match std::env::var("HISTTIMEFORMAT") {
        Ok(v) if !v.trim().is_empty() => {
            report.push("shell.histtime", Status::Ok, "HISTTIMEFORMAT est configuré")
        }
        _ => report.push(
            "shell.histtime",
            Status::Info,
            "HISTTIMEFORMAT non configuré : les horodatages d'import seront approximatifs",
        ),
    }
}

/// Vérifie que le fichier n'est pas modifiable par le groupe ou les autres.
fn check_permissions(report: &mut Report, name: &str, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(meta) => {
                let mode = meta.permissions().mode() & 0o777;
                if mode & 0o022 != 0 {
                    report.push(
                        name,
                        Status::Warn,
                        format!(
                            "{} est modifiable par le groupe/les autres ({:o})",
                            path.display(),
                            mode
                        ),
                    );
                } else {
                    report.push(
                        name,
                        Status::Ok,
                        format!("Permissions correctes ({:o})", mode),
                    );
                }
            }
            Err(e) => report.push(name, Status::Warn, format!("Permissions illisibles : {e}")),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path,);
        report.push(name, Status::Info, "Vérification des permissions ignorée");
    }
}

// ---------------------------------------------------------------------------
// Helpers PATH / chemins.
// ---------------------------------------------------------------------------

fn bashrc_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".bashrc"))
}

fn local_bin_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".local").join("bin"))
}

fn path_contains(dir: &Path) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p == dir))
        .unwrap_or(false)
}

fn find_in_path(exe: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(exe);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

// ---------------------------------------------------------------------------
// Rendu.
// ---------------------------------------------------------------------------

fn render_text(report: &Report) {
    println!("mnemo doctor — rapport de diagnostic");
    println!("------------------------------------");
    for c in &report.checks {
        println!("[{:^5}] {}", c.status.label(), c.message);
    }
    println!("------------------------------------");
    println!(
        "Résumé : {} OK, {} WARN, {} ERROR, {} INFO",
        report.count(Status::Ok),
        report.count(Status::Warn),
        report.count(Status::Error),
        report.count(Status::Info),
    );
    if report.exit_code() == 0 {
        println!("État global : sain (code 0)");
    } else {
        println!("État global : erreurs détectées (code 1)");
    }
}

fn render_json(report: &Report) -> String {
    #[derive(Serialize)]
    struct Summary {
        ok: usize,
        warn: usize,
        error: usize,
        info: usize,
        exit_code: i32,
    }
    #[derive(Serialize)]
    struct Output<'a> {
        summary: Summary,
        checks: &'a [Check],
    }

    let output = Output {
        summary: Summary {
            ok: report.count(Status::Ok),
            warn: report.count(Status::Warn),
            error: report.count(Status::Error),
            info: report.count(Status::Info),
            exit_code: report.exit_code(),
        },
        checks: &report.checks,
    };

    // La sérialisation ne peut échouer pour ces types simples ; on retombe sur
    // un objet vide par prudence plutôt que de paniquer.
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_depend_des_erreurs() {
        let mut r = Report::default();
        r.push("a", Status::Ok, "ok");
        r.push("b", Status::Warn, "warn");
        assert_eq!(r.exit_code(), 0);
        r.push("c", Status::Error, "boom");
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn json_echappe_les_caracteres_speciaux() {
        let mut r = Report::default();
        r.push("x", Status::Ok, "a\"b\\c\nfin");
        let s = render_json(&r);
        // serde_json doit produire un JSON parseable et correctement échappé.
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["checks"][0]["message"], "a\"b\\c\nfin");
    }

    #[test]
    fn json_est_bien_forme() {
        let mut r = Report::default();
        r.push("x", Status::Ok, "tout va bien");
        let s = render_json(&r);
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["summary"]["ok"], 1);
        assert_eq!(parsed["summary"]["exit_code"], 0);
        assert_eq!(parsed["checks"][0]["status"], "ok");
        assert_eq!(parsed["checks"][0]["name"], "x");
    }
}
