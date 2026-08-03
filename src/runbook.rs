//! Commande `mnemo runbook` : génère un runbook Markdown réutilisable à partir
//! des commandes d'une session ou d'un projet Git.
//!
//! Le runbook liste chaque commande dans une section Markdown numérotée avec son
//! répertoire de travail, triées par ordre chronologique (le plus ancien en
//! premier). Les lignes vides sont exclues. Le résultat est stable et
//! déterministe : idéal pour une documentation ou un wiki.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config;
use crate::db;
use crate::mdfmt::{display_home, md_code_block};

/// Une entrée du runbook : une commande avec son contexte minimal.
#[derive(Debug, Clone)]
pub struct RunbookEntry {
    /// Commande shell (toujours non vide après filtrage).
    pub command: String,
    /// Répertoire de travail au moment de l'exécution (raccourci `~/…`).
    pub cwd: String,
    /// Horodatage de la commande (`YYYY-MM-DD HH:MM:SS`).
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Fonctions de récupération
// ---------------------------------------------------------------------------

/// Charge les commandes de la dernière session, en ordre chronologique croissant.
/// Exclut les commandes vides (après trim).
///
/// Renvoie une erreur si aucune session n'est enregistrée.
pub fn fetch_last_session_commands(
    conn: &Connection,
    limit: Option<u32>,
) -> Result<Vec<RunbookEntry>> {
    let session_id = db::latest_session_id(conn)?.ok_or_else(|| {
        anyhow::anyhow!(
            "Aucune session trouvée. Les commandes importées ou enregistrées \
             sans MNEMO_SESSION_ID ne sont pas rattachées à une session."
        )
    })?;
    fetch_session_commands(conn, &session_id, limit)
}

/// Charge les commandes d'une session explicite, en ordre chronologique croissant.
///
/// Renvoie une erreur si la session est introuvable.
pub fn fetch_session_commands(
    conn: &Connection,
    session_id: &str,
    limit: Option<u32>,
) -> Result<Vec<RunbookEntry>> {
    let lim = limit.map(|n| n as usize);
    let records = db::session_commands(conn, session_id, lim)?;
    if records.is_empty() {
        bail!("Session introuvable : {session_id}");
    }
    Ok(records_to_entries(records))
}

/// Charge les commandes d'un projet (par nom court ou chemin `git_root`), en
/// ordre chronologique croissant. Toutes les racines correspondantes sont
/// agrégées avant tri.
///
/// Renvoie une erreur si aucune racine ne correspond à `name_or_path`.
pub fn fetch_project_commands(
    conn: &Connection,
    name_or_path: &str,
    limit: Option<u32>,
) -> Result<Vec<RunbookEntry>> {
    let roots = db::match_project_roots(conn, name_or_path)?;
    if roots.is_empty() {
        bail!("Projet introuvable : {name_or_path}");
    }
    let mut all: Vec<db::CommandRecord> = Vec::new();
    for root in &roots {
        // project_records retourne DESC ; on collecte tout et on trie ensuite.
        let records = db::project_records(conn, root, None, None, false, None)?;
        all.extend(records);
    }
    // Tri chronologique croissant (le plus ancien en premier).
    all.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    // Appliquer la limite après le tri.
    if let Some(n) = limit {
        all.truncate(n as usize);
    }
    Ok(records_to_entries(all))
}

/// Convertit des [`db::CommandRecord`] en [`RunbookEntry`], en excluant les
/// commandes vides (après trim).
fn records_to_entries(records: Vec<db::CommandRecord>) -> Vec<RunbookEntry> {
    records
        .into_iter()
        .filter(|r| !r.command.trim().is_empty())
        .map(|r| RunbookEntry {
            command: r.command,
            cwd: r
                .cwd
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(display_home)
                .unwrap_or_else(|| "-".to_string()),
            timestamp: r.created_at,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Rendu Markdown
// ---------------------------------------------------------------------------

/// Génère le document Markdown d'un runbook.
///
/// - `title` : titre du runbook (remplace la section `# Runbook - …`).
/// - `source_desc` : description lisible de la source (session, projet…).
/// - `entries` : commandes à inclure (les vides ont déjà été exclues en amont).
///
/// Quand `entries` est vide, le document reste cohérent (section Commands avec
/// un message explicite, pas de panic).
pub fn render_markdown(title: &str, source_desc: &str, entries: &[RunbookEntry]) -> String {
    let generated_at = db::now_timestamp();
    let mut out = String::new();

    out.push_str(&format!("# Runbook - {title}\n\n"));

    out.push_str("## Metadata\n\n");
    out.push_str(&format!("- Source: {source_desc}\n"));
    out.push_str(&format!("- Generated at: {generated_at}\n"));
    out.push_str(&format!("- Commands: {}\n\n", entries.len()));

    out.push_str("## Commands\n\n");

    if entries.is_empty() {
        out.push_str("_Aucune commande._\n");
        return out;
    }

    for (i, entry) in entries.iter().enumerate() {
        out.push_str(&format!("### {}. {}\n\n", i + 1, entry.cwd));
        out.push_str(&md_code_block(std::slice::from_ref(&entry.command)));
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Point d'entrée de la commande
// ---------------------------------------------------------------------------

/// Point d'entrée de `mnemo runbook`.
///
/// Exactement un des drapeaux `last`, `session`, `project` doit être fourni
/// (mutuellement exclusifs côté clap). Si aucun n'est fourni, une erreur claire
/// est retournée.
pub fn run(
    last: bool,
    session: Option<String>,
    project: Option<String>,
    output: Option<PathBuf>,
    force: bool,
    limit: Option<u32>,
    title: Option<String>,
) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;

    let (entries, source_desc, default_title): (Vec<RunbookEntry>, String, String) = if last {
        let sid = db::latest_session_id(&conn)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Aucune session trouvée. Les commandes importées ou enregistrées \
                     sans MNEMO_SESSION_ID ne sont pas rattachées à une session."
            )
        })?;
        let e = fetch_session_commands(&conn, &sid, limit)?;
        let desc = format!("dernière session ({sid})");
        let dtitle = sid.clone();
        (e, desc, dtitle)
    } else if let Some(ref sid) = session {
        let e = fetch_session_commands(&conn, sid, limit)?;
        (e, format!("session {sid}"), sid.clone())
    } else if let Some(ref proj) = project {
        let e = fetch_project_commands(&conn, proj, limit)?;
        (e, format!("projet {proj}"), proj.clone())
    } else {
        bail!(
            "Préciser une source : --last, --session <ID> ou --project <NOM>.\n\
                 Utilisez `mnemo runbook --help` pour voir les options disponibles."
        );
    };

    let resolved_title = title.unwrap_or(default_title);
    let content = render_markdown(&resolved_title, &source_desc, &entries);

    match output {
        Some(ref path) => {
            if path.exists() && !force {
                bail!(
                    "Le fichier {} existe déjà. Utilisez --force pour l'écraser.",
                    path.display()
                );
            }
            std::fs::write(path, content.as_bytes())
                .with_context(|| format!("écriture du runbook {}", path.display()))?;
            eprintln!(
                "Runbook écrit dans {} ({} commandes).",
                path.display(),
                entries.len()
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

// ---------------------------------------------------------------------------
// Tests unitaires
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entries(cmds: &[(&str, &str)]) -> Vec<RunbookEntry> {
        cmds.iter()
            .enumerate()
            .map(|(i, (cmd, cwd))| RunbookEntry {
                command: cmd.to_string(),
                cwd: cwd.to_string(),
                timestamp: format!("2026-01-01 10:{i:02}:00"),
            })
            .collect()
    }

    #[test]
    fn render_contient_titre_et_sections() {
        let entries = make_entries(&[("cargo build", "~/proj")]);
        let md = render_markdown("mon runbook", "session s1", &entries);
        assert!(md.contains("# Runbook - mon runbook"));
        assert!(md.contains("## Metadata"));
        assert!(md.contains("## Commands"));
        assert!(md.contains("Source: session s1"));
        assert!(md.contains("Commands: 1"));
    }

    #[test]
    fn render_numerote_les_sections() {
        let entries = make_entries(&[("git pull", "~/a"), ("cargo test", "~/b")]);
        let md = render_markdown("test", "session s1", &entries);
        assert!(md.contains("### 1. ~/a"));
        assert!(md.contains("### 2. ~/b"));
        assert!(md.contains("git pull"));
        assert!(md.contains("cargo test"));
    }

    #[test]
    fn render_zero_commandes_reste_coherent() {
        let md = render_markdown("vide", "session s1", &[]);
        assert!(md.contains("# Runbook - vide"));
        assert!(md.contains("## Commands"));
        assert!(md.contains("Commands: 0"));
        assert!(md.contains("_Aucune commande._"));
    }

    #[test]
    fn render_echappe_les_backticks_dans_les_blocs() {
        let entries = make_entries(&[("echo `date`", "~/proj")]);
        let md = render_markdown("bt", "session s1", &entries);
        // La clôture du bloc doit être plus longue que 1 backtick.
        let fences: Vec<&str> = md.lines().filter(|l| l.starts_with("```")).collect();
        assert_eq!(fences.len() % 2, 0, "blocs non équilibrés");
    }

    #[test]
    fn records_to_entries_exclut_les_commandes_vides() {
        let records = vec![
            db::CommandRecord {
                id: 1,
                command: "  ".to_string(),
                cwd: Some("/tmp".to_string()),
                shell: None,
                hostname: None,
                exit_code: Some(0),
                created_at: "2026-01-01 10:00:00".to_string(),
                git_root: None,
                git_branch: None,
                git_remote: None,
                session_id: Some("s1".to_string()),
            },
            db::CommandRecord {
                id: 2,
                command: "ls".to_string(),
                cwd: Some("/tmp".to_string()),
                shell: None,
                hostname: None,
                exit_code: Some(0),
                created_at: "2026-01-01 10:01:00".to_string(),
                git_root: None,
                git_branch: None,
                git_remote: None,
                session_id: Some("s1".to_string()),
            },
        ];
        let entries = records_to_entries(records);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "ls");
    }
}
