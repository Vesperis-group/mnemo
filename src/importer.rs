use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;

use crate::config::Config;
use crate::db::{self, NewCommand};
use crate::filter;

/// Statistiques renvoyées après un import.
#[derive(Debug, Default, Clone)]
pub struct ImportStats {
    pub total: usize,
    pub imported: usize,
    pub skipped_sensitive: usize,
    pub skipped_duplicate: usize,
}

/// Importe un fichier d'historique Bash dans la base.
pub fn import_bash_history(conn: &Connection, path: &Path, config: &Config) -> Result<ImportStats> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("lecture de l'historique {}", path.display()))?;
    import_from_str(conn, &content, config)
}

/// Importe le contenu d'un historique (séparé en testable).
pub fn import_from_str(conn: &Connection, content: &str, config: &Config) -> Result<ImportStats> {
    let mut stats = ImportStats::default();
    // Déduplication intra-fichier (en plus de la contrainte UNIQUE en base).
    let mut seen: HashSet<String> = HashSet::new();
    let created_at = db::now_timestamp();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // Lignes de timestamp bash (HISTTIMEFORMAT) : `#1700000000`.
        if is_history_timestamp(line) {
            continue;
        }

        stats.total += 1;

        if filter::is_sensitive(line, &config.sensitive_keywords) {
            stats.skipped_sensitive += 1;
            continue;
        }

        let hash = db::compute_hash(line, None);
        if !seen.insert(hash) {
            stats.skipped_duplicate += 1;
            continue;
        }

        let cmd = NewCommand {
            command: line.to_string(),
            cwd: None,
            shell: Some("bash".to_string()),
            hostname: None,
            exit_code: None,
            created_at: created_at.clone(),
            ..Default::default()
        };

        if db::insert_command(conn, &cmd)? {
            stats.imported += 1;
        } else {
            stats.skipped_duplicate += 1;
        }
    }

    Ok(stats)
}

fn is_history_timestamp(line: &str) -> bool {
    line.len() > 1 && line.starts_with('#') && line[1..].chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn import_filtre_et_dedoublonne() {
        let conn = db::open_in_memory().unwrap();
        let cfg = Config::default();
        let content = "ls -la\n#1700000000\nls -la\nexport TOKEN=abc\n\ngit status\n";

        let stats = import_from_str(&conn, content, &cfg).unwrap();

        assert_eq!(stats.total, 4);
        assert_eq!(stats.imported, 2); // "ls -la" et "git status"
        assert_eq!(stats.skipped_sensitive, 1); // "export TOKEN=abc"
        assert_eq!(stats.skipped_duplicate, 1); // "ls -la" répété
        assert_eq!(db::count(&conn).unwrap(), 2);
    }

    #[test]
    fn detecte_les_lignes_timestamp() {
        assert!(is_history_timestamp("#1700000000"));
        assert!(!is_history_timestamp("# un commentaire"));
        assert!(!is_history_timestamp("echo #1234"));
    }
}
