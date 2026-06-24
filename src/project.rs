//! Détection du « projet » courant et inventaire des projets connus.
//!
//! Stratégie de détection (du plus fiable au plus approximatif) :
//! 1. **Racine Git** (`git rev-parse --show-toplevel`), priorité absolue ;
//! 2. **Fichier marqueur** (`package.json`, `Cargo.toml`, `pyproject.toml`,
//!    `go.mod`, `composer.json`) trouvé en remontant l'arborescence ;
//! 3. **Nom du dossier courant** en dernier recours.
//!
//! Cette logique n'altère jamais le champ historique `git_root` : elle ne sert
//! qu'à *résoudre* un nom de projet pour les filtres et l'affichage.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use serde::Serialize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cli::SessionFormat;
use crate::db::{self, CommandRecord, ProjectSummary};
use crate::gitctx;
use crate::mdfmt::{
    display_home, md_code_block, md_table_cell_code, md_table_cell_text, opt, short_datetime,
};

/// Origine de la détection d'un projet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSource {
    /// Détecté via la racine d'un dépôt Git.
    Git,
    /// Détecté via un fichier marqueur d'écosystème.
    Marker(&'static str),
    /// Détecté via le simple nom du dossier courant.
    Directory,
}

impl ProjectSource {
    /// Libellé lisible de la source.
    pub fn label(&self) -> String {
        match self {
            ProjectSource::Git => "dépôt git".to_string(),
            ProjectSource::Marker(f) => format!("marqueur {f}"),
            ProjectSource::Directory => "dossier courant".to_string(),
        }
    }
}

/// Projet détecté pour un répertoire de travail.
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    /// Nom court du projet (dernier segment de la racine).
    pub name: String,
    /// Racine du projet si connue (racine git ou dossier du marqueur).
    pub root: Option<PathBuf>,
    /// Comment le projet a été détecté.
    pub source: ProjectSource,
}

/// Fichiers marqueurs reconnus, par ordre de priorité.
const MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "composer.json",
];

/// Détecte le projet associé à `cwd`. Ne renvoie jamais d'erreur : au pire on
/// retombe sur le nom du dossier courant.
pub fn detect(cwd: &Path) -> ProjectInfo {
    // 1. Racine Git prioritaire.
    let git = gitctx::detect(cwd);
    if let Some(root) = git.root {
        let path = PathBuf::from(&root);
        return ProjectInfo {
            name: base_name(&path).unwrap_or(root),
            root: Some(path),
            source: ProjectSource::Git,
        };
    }

    // 2. Fichier marqueur en remontant l'arborescence.
    if let Some((dir, marker)) = find_marker(cwd) {
        return ProjectInfo {
            name: base_name(&dir).unwrap_or_else(|| dir.display().to_string()),
            root: Some(dir),
            source: ProjectSource::Marker(marker),
        };
    }

    // 3. Nom du dossier courant.
    ProjectInfo {
        name: base_name(cwd).unwrap_or_else(|| "(inconnu)".to_string()),
        root: Some(cwd.to_path_buf()),
        source: ProjectSource::Directory,
    }
}

/// Nom du projet courant (résolution de `--project current`).
pub fn current_name() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    Some(detect(&cwd).name)
}

/// Cherche le premier dossier marqueur en remontant depuis `start`.
fn find_marker(start: &Path) -> Option<(PathBuf, &'static str)> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        for marker in MARKERS {
            if d.join(marker).is_file() {
                return Some((d.to_path_buf(), marker));
            }
        }
        dir = d.parent();
    }
    None
}

/// Dernier segment d'un chemin (nom de dossier), si présent.
fn base_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Affiche le projet courant (commande `mnemo project current`).
pub fn run_current() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let info = detect(&cwd);
    println!("Projet  : {}", info.name);
    if let Some(root) = &info.root {
        println!("Racine  : {}", root.display());
    }
    println!("Source  : {}", info.source.label());
    Ok(())
}

/// Affiche les projets connus de l'historique (commande `mnemo project list`).
pub fn run_list(limit: Option<usize>, json: bool) -> Result<()> {
    let conn = db::open(&crate::config::db_path()?)?;
    let projects = db::project_summaries(&conn, limit)?;

    if json {
        let rows: Vec<ProjectListJson> = projects.iter().map(ProjectListJson::from).collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if projects.is_empty() {
        println!("Aucun projet Git enregistré dans l'historique.");
        return Ok(());
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "Projets connus ({})", projects.len())?;
    writeln!(out)?;
    writeln!(
        out,
        "{:<24}  {:>9}  {:>8}  {:<16}  BRANCHES",
        "PROJET", "COMMANDES", "SESSIONS", "DERNIÈRE ACTIVITÉ"
    )?;
    for p in &projects {
        writeln!(
            out,
            "{:<24}  {:>9}  {:>8}  {:<16}  {}",
            short_name(&p.root),
            p.command_count,
            p.session_count,
            short_datetime(&p.last_activity),
            branches_label(&p.branches)
        )?;
    }
    Ok(())
}

/// Affiche le détail d'un projet (commande `mnemo project show`).
pub fn run_show(
    project: Option<String>,
    current: bool,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let conn = db::open(&crate::config::db_path()?)?;
    let root = resolve_root(&conn, project, current)?;
    let summary = db::project_summary(&conn, &root)?
        .with_context(|| format!("projet absent de l'historique : {}", display_home(&root)))?;

    let recent_limit = limit.unwrap_or(20);
    let recent = db::project_records(&conn, &root, None, None, false, Some(recent_limit))?;
    let failures = db::project_records(&conn, &root, None, None, true, Some(recent_limit))?;

    if json {
        let doc = ProjectShowJson {
            project: ProjectMetaJson::from(&summary),
            recent: recent.iter().map(RecordJson::from).collect(),
            recent_failures: failures.iter().map(RecordJson::from).collect(),
        };
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "Projet  : {}", short_name(&summary.root))?;
    writeln!(out, "Racine  : {}", display_home(&summary.root))?;
    writeln!(out, "Remote  : {}", opt(&summary.remote))?;
    writeln!(out, "Commandes : {}", summary.command_count)?;
    writeln!(out, "Sessions  : {}", summary.session_count)?;
    writeln!(
        out,
        "Activité  : {} → {}",
        short_datetime(&summary.first_activity),
        short_datetime(&summary.last_activity)
    )?;
    writeln!(out, "Branches  : {}", branches_label(&summary.branches))?;

    writeln!(out)?;
    writeln!(out, "Commandes récentes ({})", recent.len())?;
    for c in &recent {
        write_command_line(&mut out, c)?;
    }

    if !failures.is_empty() {
        writeln!(out)?;
        writeln!(out, "Derniers échecs ({})", failures.len())?;
        for c in &failures {
            write_command_line(&mut out, c)?;
        }
    }
    Ok(())
}

/// Génère un rapport d'activité d'un projet (commande `mnemo project report`).
#[allow(clippy::too_many_arguments)]
pub fn run_report(
    project: Option<String>,
    current: bool,
    since: Option<String>,
    until: Option<String>,
    format: SessionFormat,
    output: Option<PathBuf>,
    force: bool,
    limit: Option<usize>,
) -> Result<()> {
    let conn = db::open(&crate::config::db_path()?)?;
    let root = resolve_root(&conn, project, current)?;
    let summary = db::project_summary(&conn, &root)?
        .with_context(|| format!("projet absent de l'historique : {}", display_home(&root)))?;

    let since_ts = resolve_bound(since.as_deref(), db::resolve_since, "--since")?;
    let before_ts = resolve_bound(until.as_deref(), db::resolve_before, "--until")?;

    // Toutes les commandes de la période (les plus récentes d'abord), bornées
    // ensuite à `limit` pour la section détaillée.
    let records = db::project_records(
        &conn,
        &root,
        since_ts.as_deref(),
        before_ts.as_deref(),
        false,
        None,
    )?;
    let report = ReportData::build(
        &summary,
        &records,
        since.as_deref(),
        until.as_deref(),
        limit,
    );

    let content = match format {
        SessionFormat::Markdown => report.render_markdown(),
        SessionFormat::Json => serde_json::to_string_pretty(&report.as_json())?,
    };

    match output {
        Some(path) => {
            if path.exists() && !force {
                bail!(
                    "Le fichier {} existe déjà. Utilisez --force pour l'écraser.",
                    path.display()
                );
            }
            std::fs::write(&path, content.as_bytes())
                .with_context(|| format!("écriture du rapport {}", path.display()))?;
            eprintln!(
                "Rapport du projet {} écrit dans {} ({} commandes).",
                short_name(&summary.root),
                path.display(),
                report.period_count
            );
        }
        None => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            out.write_all(content.as_bytes())?;
        }
    }
    Ok(())
}

/// Résout la racine Git ciblée à partir d'un argument explicite ou `--current`.
///
/// Pour un argument explicite, accepte la racine complète ou le nom court du
/// projet (suffixe). Une correspondance ambiguë est refusée explicitement.
fn resolve_root(conn: &Connection, project: Option<String>, current: bool) -> Result<String> {
    if current {
        let cwd = std::env::current_dir().context("répertoire courant introuvable")?;
        let info = detect(&cwd);
        let root = info
            .root
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .context("racine du projet courant indéterminée")?;
        return Ok(root);
    }

    let needle = match project {
        Some(p) => expand_tilde(&p),
        None => bail!("Préciser un projet (racine ou nom) ou utiliser --current."),
    };

    let matches = db::match_project_roots(conn, &needle)?;
    match matches.len() {
        1 => Ok(matches.into_iter().next().unwrap()),
        0 => bail!("Aucun projet ne correspond à « {needle} ». Voir `mnemo project list`."),
        _ => {
            let listed = matches
                .iter()
                .map(|r| display_home(r))
                .collect::<Vec<_>>()
                .join(", ");
            bail!("Plusieurs projets correspondent à « {needle} » : {listed}. Préciser la racine complète.")
        }
    }
}

/// Remplace un préfixe `~` ou `~/` par le répertoire personnel.
fn expand_tilde(path: &str) -> String {
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().into_owned();
        }
    } else if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

/// Résout une borne temporelle, en échouant proprement si la spec est invalide.
fn resolve_bound(
    spec: Option<&str>,
    resolver: fn(&str) -> Option<String>,
    flag: &str,
) -> Result<Option<String>> {
    match spec {
        None => Ok(None),
        Some(s) => match resolver(s) {
            Some(ts) => Ok(Some(ts)),
            None => bail!("Valeur {flag} invalide : « {s} » (durée ou date AAAA-MM-JJ attendue)."),
        },
    }
}

/// Nom court d'un projet (dernier segment de la racine).
fn short_name(root: &str) -> String {
    base_name(Path::new(root)).unwrap_or_else(|| root.to_string())
}

/// Libellé compact d'une liste de branches, ou `-` si vide.
fn branches_label(branches: &[String]) -> String {
    if branches.is_empty() {
        "-".to_string()
    } else {
        branches.join(", ")
    }
}

/// Écrit une commande sur une ligne, en signalant les échecs par leur code.
fn write_command_line<W: Write>(out: &mut W, c: &CommandRecord) -> io::Result<()> {
    match c.exit_code {
        Some(code) if code != 0 => writeln!(
            out,
            "[{}] {} (exit {code})",
            short_datetime(&c.created_at),
            c.command
        ),
        _ => writeln!(out, "[{}] {}", short_datetime(&c.created_at), c.command),
    }
}

/// Données agrégées d'un rapport de projet, pour une période donnée.
struct ReportData<'a> {
    summary: &'a ProjectSummary,
    since_spec: Option<&'a str>,
    until_spec: Option<&'a str>,
    period_count: usize,
    failure_count: usize,
    session_count: usize,
    period_branches: Vec<String>,
    period_start: Option<String>,
    period_end: Option<String>,
    /// Commandes détaillées (ordre chronologique croissant, bornées à `limit`).
    detail: Vec<&'a CommandRecord>,
    /// Échecs de la période (les plus récents d'abord).
    failures: Vec<&'a CommandRecord>,
}

impl<'a> ReportData<'a> {
    /// Construit les agrégats à partir des commandes de la période (triées de la
    /// plus récente à la plus ancienne par `project_records`).
    fn build(
        summary: &'a ProjectSummary,
        records: &'a [CommandRecord],
        since_spec: Option<&'a str>,
        until_spec: Option<&'a str>,
        limit: Option<usize>,
    ) -> Self {
        let mut sessions = std::collections::BTreeSet::new();
        let mut branches = std::collections::BTreeSet::new();
        let mut failure_count = 0usize;
        for c in records {
            if let Some(sid) = c.session_id.as_deref().filter(|s| !s.trim().is_empty()) {
                sessions.insert(sid.to_string());
            }
            if let Some(b) = c.git_branch.as_deref().filter(|s| !s.is_empty()) {
                branches.insert(b.to_string());
            }
            if matches!(c.exit_code, Some(code) if code != 0) {
                failure_count += 1;
            }
        }

        // `records` est décroissant : le dernier élément est le plus ancien.
        let period_end = records.first().map(|c| c.created_at.clone());
        let period_start = records.last().map(|c| c.created_at.clone());

        let failures: Vec<&CommandRecord> = records
            .iter()
            .filter(|c| matches!(c.exit_code, Some(code) if code != 0))
            .take(limit.unwrap_or(20))
            .collect();

        // Détail chronologique : les `limit` plus récentes, réordonnées en
        // ordre croissant pour une lecture naturelle.
        let mut detail: Vec<&CommandRecord> = match limit {
            Some(n) => records.iter().take(n).collect(),
            None => records.iter().collect(),
        };
        detail.reverse();

        ReportData {
            summary,
            since_spec,
            until_spec,
            period_count: records.len(),
            failure_count,
            session_count: sessions.len(),
            period_branches: branches.into_iter().collect(),
            period_start,
            period_end,
            detail,
            failures,
        }
    }

    /// Rendu Markdown réutilisable du rapport.
    fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Rapport projet — {}\n\n",
            short_name(&self.summary.root)
        ));

        out.push_str(&format!(
            "- Racine : {}\n",
            display_home(&self.summary.root)
        ));
        out.push_str(&format!("- Remote : {}\n", opt(&self.summary.remote)));
        out.push_str(&format!(
            "- Période : {} → {}\n",
            self.since_spec.unwrap_or("(début)"),
            self.until_spec.unwrap_or("(maintenant)")
        ));
        out.push_str(&format!("- Commandes : {}\n", self.period_count));
        out.push_str(&format!("- Échecs : {}\n", self.failure_count));
        out.push_str(&format!("- Sessions : {}\n", self.session_count));
        let start = self
            .period_start
            .as_deref()
            .map(short_datetime)
            .unwrap_or("-");
        let end = self
            .period_end
            .as_deref()
            .map(short_datetime)
            .unwrap_or("-");
        out.push_str(&format!("- Première activité : {start}\n"));
        out.push_str(&format!("- Dernière activité : {end}\n"));
        out.push_str(&format!(
            "- Branches : {}\n",
            branches_label(&self.period_branches)
        ));
        out.push('\n');

        out.push_str("## Commandes\n\n");
        if self.detail.is_empty() {
            out.push_str("_Aucune commande sur la période._\n\n");
        } else {
            let commands: Vec<String> = self.detail.iter().map(|c| c.command.clone()).collect();
            out.push_str(&md_code_block(&commands));
            out.push('\n');
        }

        out.push_str("## Détail chronologique\n\n");
        out.push_str("| Date | Code retour | Branche | Commande |\n");
        out.push_str("| --- | ---: | --- | --- |\n");
        for c in &self.detail {
            let code = c
                .exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string());
            let branch = c
                .git_branch
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("-");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                short_datetime(&c.created_at),
                code,
                md_table_cell_text(branch),
                md_table_cell_code(&c.command)
            ));
        }

        if !self.failures.is_empty() {
            out.push('\n');
            out.push_str("## Échecs\n\n");
            out.push_str("| Date | Code retour | Commande |\n");
            out.push_str("| --- | ---: | --- |\n");
            for c in &self.failures {
                let code = c
                    .exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".to_string());
                out.push_str(&format!(
                    "| {} | {} | {} |\n",
                    short_datetime(&c.created_at),
                    code,
                    md_table_cell_code(&c.command)
                ));
            }
        }
        out
    }

    /// Document JSON stable du rapport.
    fn as_json(&self) -> ReportJson<'_> {
        ReportJson {
            project: ProjectMetaJson::from(self.summary),
            period: PeriodJson {
                since: self.since_spec,
                until: self.until_spec,
                command_count: self.period_count,
                failure_count: self.failure_count,
                session_count: self.session_count,
                first_activity: self.period_start.clone(),
                last_activity: self.period_end.clone(),
                branches: self.period_branches.clone(),
            },
            commands: self.detail.iter().map(|c| RecordJson::from(*c)).collect(),
            failures: self.failures.iter().map(|c| RecordJson::from(*c)).collect(),
        }
    }
}

/// Projet sérialisé pour `mnemo project list --json`.
#[derive(Serialize)]
struct ProjectListJson {
    name: String,
    root: String,
    command_count: i64,
    session_count: i64,
    last_activity: String,
    branches: Vec<String>,
}

impl From<&ProjectSummary> for ProjectListJson {
    fn from(s: &ProjectSummary) -> Self {
        ProjectListJson {
            name: short_name(&s.root),
            root: s.root.clone(),
            command_count: s.command_count,
            session_count: s.session_count,
            last_activity: s.last_activity.clone(),
            branches: s.branches.clone(),
        }
    }
}

/// Métadonnées d'un projet sérialisées (show / report).
#[derive(Serialize)]
struct ProjectMetaJson {
    name: String,
    root: String,
    remote: Option<String>,
    command_count: i64,
    session_count: i64,
    first_activity: String,
    last_activity: String,
    branches: Vec<String>,
}

impl From<&ProjectSummary> for ProjectMetaJson {
    fn from(s: &ProjectSummary) -> Self {
        ProjectMetaJson {
            name: short_name(&s.root),
            root: s.root.clone(),
            remote: s.remote.clone(),
            command_count: s.command_count,
            session_count: s.session_count,
            first_activity: s.first_activity.clone(),
            last_activity: s.last_activity.clone(),
            branches: s.branches.clone(),
        }
    }
}

/// Commande sérialisée (show / report).
#[derive(Serialize)]
struct RecordJson<'a> {
    created_at: &'a str,
    exit_code: Option<i64>,
    git_branch: Option<&'a str>,
    command: &'a str,
}

impl<'a> From<&'a CommandRecord> for RecordJson<'a> {
    fn from(c: &'a CommandRecord) -> Self {
        RecordJson {
            created_at: &c.created_at,
            exit_code: c.exit_code,
            git_branch: c.git_branch.as_deref(),
            command: &c.command,
        }
    }
}

/// Document JSON de `mnemo project show`.
#[derive(Serialize)]
struct ProjectShowJson<'a> {
    project: ProjectMetaJson,
    recent: Vec<RecordJson<'a>>,
    recent_failures: Vec<RecordJson<'a>>,
}

/// Document JSON de `mnemo project report`.
#[derive(Serialize)]
struct ReportJson<'a> {
    project: ProjectMetaJson,
    period: PeriodJson<'a>,
    commands: Vec<RecordJson<'a>>,
    failures: Vec<RecordJson<'a>>,
}

/// Agrégats de la période dans l'export JSON.
#[derive(Serialize)]
struct PeriodJson<'a> {
    since: Option<&'a str>,
    until: Option<&'a str>,
    command_count: usize,
    failure_count: usize,
    session_count: usize,
    first_activity: Option<String>,
    last_activity: Option<String>,
    branches: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detecte_un_marqueur_cargo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("Cargo.toml"), "[package]\n").unwrap();
        let sub = root.join("src/inner");
        fs::create_dir_all(&sub).unwrap();

        let info = detect(&sub);
        assert!(matches!(info.source, ProjectSource::Marker("Cargo.toml")));
        assert_eq!(info.root.as_deref(), Some(root));
    }

    #[test]
    fn retombe_sur_le_nom_du_dossier() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("projet-sans-marqueur");
        fs::create_dir_all(&sub).unwrap();

        let info = detect(&sub);
        assert_eq!(info.source, ProjectSource::Directory);
        assert_eq!(info.name, "projet-sans-marqueur");
    }

    #[test]
    fn summaries_agregent_sessions_et_branches() {
        let conn = db::open_in_memory().unwrap();
        let rows = [
            (
                "a",
                "/home/u/proj-a",
                "main",
                Some("s1"),
                "2026-06-14 10:00:00",
                0,
            ),
            (
                "b",
                "/home/u/proj-a",
                "feat",
                Some("s1"),
                "2026-06-14 10:05:00",
                1,
            ),
            (
                "c",
                "/home/u/proj-a",
                "main",
                Some("s2"),
                "2026-06-15 09:00:00",
                0,
            ),
            (
                "d",
                "/home/u/proj-b",
                "main",
                None,
                "2026-06-13 08:00:00",
                0,
            ),
        ];
        for (cmd, root, branch, sid, at, code) in rows {
            db::insert_command(
                &conn,
                &db::NewCommand {
                    command: cmd.into(),
                    cwd: Some(root.into()),
                    shell: Some("bash".into()),
                    hostname: Some("h".into()),
                    exit_code: Some(code),
                    created_at: at.into(),
                    git_root: Some(root.into()),
                    git_branch: Some(branch.into()),
                    git_remote: None,
                    session_id: sid.map(|s| s.to_string()),
                },
            )
            .unwrap();
        }

        let summary = db::project_summary(&conn, "/home/u/proj-a")
            .unwrap()
            .unwrap();
        assert_eq!(summary.command_count, 3);
        assert_eq!(summary.session_count, 2);
        assert_eq!(
            summary.branches,
            vec!["feat".to_string(), "main".to_string()]
        );
        assert_eq!(summary.last_activity, "2026-06-15 09:00:00");

        // Le plus récemment actif vient en tête.
        let all = db::project_summaries(&conn, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(short_name(&all[0].root), "proj-a");
    }

    #[test]
    fn resolve_bound_echoue_sur_spec_invalide() {
        assert!(resolve_bound(Some("pas-une-date"), db::resolve_since, "--since").is_err());
        assert!(resolve_bound(None, db::resolve_since, "--since")
            .unwrap()
            .is_none());
        assert!(resolve_bound(Some("7d"), db::resolve_since, "--since")
            .unwrap()
            .is_some());
    }

    #[test]
    fn branches_label_compacte() {
        assert_eq!(branches_label(&[]), "-");
        assert_eq!(
            branches_label(&["main".to_string(), "dev".to_string()]),
            "main, dev"
        );
    }
}
