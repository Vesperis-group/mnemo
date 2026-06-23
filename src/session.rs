//! Commande `mnemo session` : navigation, consultation et export des sessions
//! de travail.
//!
//! Une session regroupe les commandes partageant un même `session_id`, capturé
//! par l'intégration shell via `MNEMO_SESSION_ID`. Les commandes importées ou
//! enregistrées sans cet identifiant ne sont pas rattachées à une session et
//! sont ignorées par ces commandes (jamais de session artificielle).
//!
//! Cette feature est en lecture seule : elle n'écrit dans la base aucune donnée
//! et ne modifie pas le schéma.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::cli::SessionFormat;
use crate::config;
use crate::db::{self, CommandRecord};

/// Métadonnées agrégées d'une session, dérivées de ses commandes.
struct SessionMeta {
    session_id: String,
    count: usize,
    started_at: String,
    ended_at: String,
    git_root: Option<String>,
    git_branch: Option<String>,
}

/// `mnemo session list` : liste les sessions, de la plus récente à la plus
/// ancienne.
pub fn run_list(limit: Option<usize>) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;
    let sessions = db::session_summaries(&conn, limit)?;

    let stdout = io::stdout();
    let mut out = stdout.lock();

    if sessions.is_empty() {
        writeln!(out, "Aucune session enregistrée.")?;
        writeln!(
            out,
            "Les commandes importées ou enregistrées sans MNEMO_SESSION_ID ne sont"
        )?;
        writeln!(
            out,
            "pas rattachées à une session. Réinstallez l'intégration shell"
        )?;
        writeln!(out, "(`mnemo init`) pour capturer les prochaines sessions.")?;
        return Ok(());
    }

    writeln!(out, "Sessions récentes")?;
    writeln!(out)?;
    writeln!(
        out,
        "{:<24}  {:>9}  {:<16}  {:<16}  PROJET",
        "SESSION ID", "COMMANDES", "DÉBUT", "FIN"
    )?;
    for s in &sessions {
        let projet = s
            .git_root
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(display_home)
            .unwrap_or_else(|| "-".to_string());
        writeln!(
            out,
            "{:<24}  {:>9}  {:<16}  {:<16}  {}",
            s.session_id,
            s.count,
            short_datetime(&s.started_at),
            short_datetime(&s.ended_at),
            projet
        )?;
    }
    Ok(())
}

/// `mnemo session show <id>` : affiche les commandes d'une session dans l'ordre
/// chronologique.
pub fn run_show(session_id: String, limit: Option<usize>) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;
    // On charge toutes les commandes pour des métadonnées exactes (nombre,
    // bornes temporelles), puis on n'affiche que les `limit` premières.
    let all = db::session_commands(&conn, &session_id, None)?;
    if all.is_empty() {
        bail!("Session introuvable : {session_id}");
    }
    let meta = meta_from_commands(&session_id, &all);

    let stdout = io::stdout();
    let mut out = stdout.lock();

    writeln!(out, "Session {}", meta.session_id)?;
    writeln!(out, "Projet : {}", opt_home(&meta.git_root))?;
    writeln!(out, "Branche : {}", opt(&meta.git_branch))?;
    writeln!(out, "Commandes : {}", meta.count)?;
    writeln!(out, "Début : {}", short_datetime(&meta.started_at))?;
    writeln!(out, "Fin : {}", short_datetime(&meta.ended_at))?;
    writeln!(out)?;

    let shown = match limit {
        Some(n) => &all[..all.len().min(n)],
        None => &all[..],
    };
    for c in shown {
        // Le code de sortie n'est affiché que pour les échecs, afin de garder la
        // liste lisible (le succès est le cas courant).
        match c.exit_code {
            Some(code) if code != 0 => {
                writeln!(
                    out,
                    "[{}] {} (exit {code})",
                    time_part(&c.created_at),
                    c.command
                )?;
            }
            _ => writeln!(out, "[{}] {}", time_part(&c.created_at), c.command)?,
        }
    }
    Ok(())
}

/// `mnemo session export` : exporte une session en Markdown (défaut) ou JSON.
pub fn run_export(
    session_id: Option<String>,
    last: bool,
    format: SessionFormat,
    output: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;

    let session_id = resolve_session_id(&conn, session_id, last)?;
    let cmds = db::session_commands(&conn, &session_id, None)?;
    if cmds.is_empty() {
        bail!("Session introuvable : {session_id}");
    }
    let meta = meta_from_commands(&session_id, &cmds);

    let content = match format {
        SessionFormat::Markdown => render_markdown(&meta, &cmds),
        SessionFormat::Json => render_json(&meta, &cmds)?,
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
                .with_context(|| format!("écriture de l'export {}", path.display()))?;
            eprintln!(
                "Session {} exportée dans {} ({} commandes).",
                meta.session_id,
                path.display(),
                meta.count
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

/// Détermine la session ciblée à partir d'un identifiant explicite ou `--last`.
fn resolve_session_id(
    conn: &rusqlite::Connection,
    session_id: Option<String>,
    last: bool,
) -> Result<String> {
    if last {
        return match db::latest_session_id(conn)? {
            Some(id) => Ok(id),
            None => bail!(
                "Aucune session trouvée. Les commandes importées ou enregistrées \
                 sans MNEMO_SESSION_ID ne sont pas rattachées à une session."
            ),
        };
    }
    match session_id {
        Some(id) => Ok(id),
        None => bail!("Préciser un identifiant de session ou utiliser --last."),
    }
}

/// Construit les métadonnées d'une session à partir de ses commandes triées par
/// ordre chronologique croissant.
fn meta_from_commands(session_id: &str, cmds: &[CommandRecord]) -> SessionMeta {
    let started_at = cmds
        .first()
        .map(|c| c.created_at.clone())
        .unwrap_or_default();
    let ended_at = cmds
        .last()
        .map(|c| c.created_at.clone())
        .unwrap_or_default();
    let last = cmds.last();
    SessionMeta {
        session_id: session_id.to_string(),
        count: cmds.len(),
        started_at,
        ended_at,
        git_root: last
            .and_then(|c| c.git_root.clone())
            .filter(|s| !s.is_empty()),
        git_branch: last
            .and_then(|c| c.git_branch.clone())
            .filter(|s| !s.is_empty()),
    }
}

/// Rendu Markdown d'une session, directement réutilisable (documentation,
/// audit, procédure).
fn render_markdown(meta: &SessionMeta, cmds: &[CommandRecord]) -> String {
    let mut out = String::new();
    out.push_str("# Session mnemo\n\n");
    out.push_str(&format!(
        "- Session : {}\n",
        md_inline_code(&meta.session_id)
    ));
    out.push_str(&format!("- Début : {}\n", short_datetime(&meta.started_at)));
    out.push_str(&format!("- Fin : {}\n", short_datetime(&meta.ended_at)));
    out.push_str(&format!("- Commandes : {}\n", meta.count));
    out.push_str(&format!("- Projet : {}\n", opt_home(&meta.git_root)));
    out.push_str(&format!("- Branche : {}\n", opt(&meta.git_branch)));
    out.push('\n');

    out.push_str("## Commandes\n\n");
    let commands: Vec<String> = cmds.iter().map(|c| c.command.clone()).collect();
    out.push_str(&md_code_block(&commands));
    out.push('\n');

    out.push_str("## Détail chronologique\n\n");
    out.push_str("| Heure | Code retour | Dossier | Commande |\n");
    out.push_str("| --- | ---: | --- | --- |\n");
    for c in cmds {
        let code = c
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());
        let dossier = c
            .cwd
            .as_deref()
            .or(c.git_root.as_deref())
            .filter(|s| !s.is_empty())
            .map(display_home)
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            time_part(&c.created_at),
            code,
            md_table_cell_text(&dossier),
            md_table_cell_code(&c.command)
        ));
    }
    out
}

/// Document JSON exporté pour une session.
#[derive(Serialize)]
struct SessionJson<'a> {
    session_id: &'a str,
    started_at: &'a str,
    ended_at: &'a str,
    command_count: usize,
    git_root: Option<&'a str>,
    git_branch: Option<&'a str>,
    commands: Vec<SessionCommandJson<'a>>,
}

/// Commande sérialisée dans l'export JSON d'une session.
#[derive(Serialize)]
struct SessionCommandJson<'a> {
    created_at: &'a str,
    cwd: Option<&'a str>,
    exit_code: Option<i64>,
    git_branch: Option<&'a str>,
    command: &'a str,
}

/// Rendu JSON stable et lisible d'une session.
fn render_json(meta: &SessionMeta, cmds: &[CommandRecord]) -> Result<String> {
    let commands = cmds
        .iter()
        .map(|c| SessionCommandJson {
            created_at: &c.created_at,
            cwd: c.cwd.as_deref(),
            exit_code: c.exit_code,
            git_branch: c.git_branch.as_deref(),
            command: &c.command,
        })
        .collect();
    let doc = SessionJson {
        session_id: &meta.session_id,
        started_at: &meta.started_at,
        ended_at: &meta.ended_at,
        command_count: meta.count,
        git_root: meta.git_root.as_deref(),
        git_branch: meta.git_branch.as_deref(),
        commands,
    };
    Ok(serde_json::to_string_pretty(&doc)?)
}

/// Plus longue suite consécutive de backticks dans `s`.
fn longest_backtick_run(s: &str) -> usize {
    let mut max = 0;
    let mut cur = 0;
    for ch in s.chars() {
        if ch == '`' {
            cur += 1;
            max = max.max(cur);
        } else {
            cur = 0;
        }
    }
    max
}

/// Encadre `commands` dans un bloc de code Markdown, en choisissant une clôture
/// plus longue que toute suite de backticks présente, pour ne jamais casser le
/// bloc.
fn md_code_block(commands: &[String]) -> String {
    let max_run = commands
        .iter()
        .map(|c| longest_backtick_run(c))
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(max_run.max(2) + 1);
    let mut out = String::new();
    out.push_str(&fence);
    out.push_str("bash\n");
    for c in commands {
        out.push_str(c);
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

/// Rend une chaîne en code en ligne Markdown, robuste aux backticks internes.
fn md_inline_code(s: &str) -> String {
    let ticks = "`".repeat(longest_backtick_run(s) + 1);
    let pad = if s.starts_with('`') || s.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{s}{pad}{ticks}")
}

/// Échappe une cellule de tableau Markdown en texte simple (pipes et retours).
fn md_table_cell_text(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// Rend une commande dans une cellule de tableau Markdown, en code en ligne,
/// sans casser la structure du tableau.
fn md_table_cell_code(s: &str) -> String {
    let oneline = s.replace(['\n', '\r'], " ");
    let escaped = oneline.replace('|', "\\|");
    md_inline_code(&escaped)
}

/// Raccourcit un chemin sous le répertoire personnel en `~/...`.
fn display_home(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Some(home_str) = home.to_str() {
            if let Some(rest) = path.strip_prefix(home_str) {
                let rest = rest.trim_start_matches('/');
                if rest.is_empty() {
                    return "~".to_string();
                }
                return format!("~/{rest}");
            }
        }
    }
    path.to_string()
}

/// Affiche une option de chemin avec raccourci `~`, ou `-` si absente.
fn opt_home(value: &Option<String>) -> String {
    value
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(display_home)
        .unwrap_or_else(|| "-".to_string())
}

/// Affiche une option textuelle, ou `-` si absente.
fn opt(value: &Option<String>) -> String {
    value
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("-")
        .to_string()
}

/// Partie horaire (`HH:MM:SS`) d'un horodatage `YYYY-MM-DD HH:MM:SS`.
fn time_part(created_at: &str) -> &str {
    created_at.split_whitespace().nth(1).unwrap_or(created_at)
}

/// Tronque un horodatage à la minute (`YYYY-MM-DD HH:MM`).
fn short_datetime(created_at: &str) -> &str {
    created_at.get(..16).unwrap_or(created_at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_code_protege_les_backticks() {
        assert_eq!(md_inline_code("ls"), "`ls`");
        // Une commande contenant un backtick doit être encadrée par deux,
        // avec une espace de chaque côté (retirée au rendu CommonMark).
        assert_eq!(md_inline_code("echo `date`"), "`` echo `date` ``");
    }

    #[test]
    fn code_block_choisit_une_cloture_assez_longue() {
        let block = md_code_block(&["echo ```x```".to_string()]);
        assert!(block.starts_with("````bash\n"));
        assert!(block.trim_end().ends_with("````"));
    }

    #[test]
    fn table_cell_echappe_les_pipes() {
        let cell = md_table_cell_code("grep -E 'a|b'");
        assert!(cell.contains("\\|"));
        assert!(!cell.contains("a|b"));
    }

    #[test]
    fn horaire_et_date_courte() {
        assert_eq!(time_part("2026-06-23 10:12:01"), "10:12:01");
        assert_eq!(short_datetime("2026-06-23 10:12:01"), "2026-06-23 10:12");
    }
}
