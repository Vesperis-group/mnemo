//! Affichage des dernières commandes (`mnemo list`).
//!
//! Pratique pour repérer l'ID d'une commande à supprimer avec `mnemo delete`.

use anyhow::Result;
use serde::Serialize;
use std::path::Path;

use crate::config;
use crate::db::{self, CommandRecord, SearchFilter};

/// Nombre de commandes affichées par défaut.
const DEFAULT_LIMIT: usize = 20;

/// Ligne JSON pour `mnemo list --json`.
#[derive(Serialize)]
struct JsonRow<'a> {
    id: i64,
    created_at: &'a str,
    cwd: Option<&'a str>,
    git_root: Option<&'a str>,
    exit_code: Option<i64>,
    command: &'a str,
}

/// Point d'entrée de `mnemo list`.
pub fn run(
    limit: Option<usize>,
    project: Option<String>,
    branch: Option<String>,
    json: bool,
) -> Result<()> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let conn = db::open(&config::db_path()?)?;
    let filter = SearchFilter { project, branch };
    let records = db::fetch_filtered(&conn, &filter, limit)?;

    if json {
        let rows: Vec<JsonRow> = records
            .iter()
            .map(|r| JsonRow {
                id: r.id,
                created_at: &r.created_at,
                cwd: r.cwd.as_deref(),
                git_root: r.git_root.as_deref(),
                exit_code: r.exit_code,
                command: &r.command,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if records.is_empty() {
        println!("Aucune commande à afficher.");
        return Ok(());
    }

    for r in &records {
        println!("{}", short_line(r));
    }
    Ok(())
}

/// Nom de projet (dernier segment de `git_root`) ou `cwd`, pour l'affichage.
fn location(r: &CommandRecord) -> String {
    if let Some(root) = r.git_root.as_deref().filter(|s| !s.is_empty()) {
        let name = Path::new(root)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(root);
        return name.to_string();
    }
    r.cwd.clone().unwrap_or_else(|| "-".to_string())
}

/// Ligne courte et alignée : `   123  2026-06-14  projet  [0]  commande`.
/// Réutilisée par `mnemo delete` / `mnemo prune` pour prévisualiser.
pub fn short_line(r: &CommandRecord) -> String {
    let date = r
        .created_at
        .split_whitespace()
        .next()
        .unwrap_or(&r.created_at);
    let exit = r
        .exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "-".to_string());
    format!(
        "{:>6}  {:<10}  {:<20}  [{:>3}]  {}",
        r.id,
        date,
        truncate(&location(r), 20),
        exit,
        r.command
    )
}

/// Tronque une chaîne (affichage) à `max` caractères, suffixe `…`.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: i64, command: &str) -> CommandRecord {
        CommandRecord {
            id,
            command: command.to_string(),
            cwd: Some("/home/u/proj/mnemo".to_string()),
            shell: None,
            hostname: None,
            exit_code: Some(0),
            created_at: "2026-06-14 12:00:00".to_string(),
            git_root: Some("/home/u/proj/mnemo".to_string()),
            git_branch: Some("main".to_string()),
            git_remote: None,
            session_id: None,
        }
    }

    #[test]
    fn short_line_contient_id_et_commande() {
        let line = short_line(&rec(123, "cargo build"));
        assert!(line.contains("123"));
        assert!(line.contains("mnemo"));
        assert!(line.contains("cargo build"));
    }

    #[test]
    fn troncature() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
