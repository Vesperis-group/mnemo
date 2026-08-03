//! Génération de runbook Markdown/JSON depuis une session ou un projet.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config;
use crate::db;
use crate::mdfmt::{display_home, md_code_block};
use crate::secrets;

/// Format de sortie du runbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RunbookFormat {
    /// Rendu Markdown (défaut).
    Markdown,
    /// JSON structuré, stable et déterministe.
    Json,
}

/// Mode de groupement des commandes dans le runbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GroupBy {
    /// Liste plate, une section numérotée par commande (défaut).
    None,
    /// Sections par répertoire de travail.
    Cwd,
    /// Sections par racine Git (`git_root`).
    Project,
}

/// Une entrée du runbook : une commande avec son contexte minimal.
#[derive(Debug, Clone)]
pub struct RunbookEntry {
    /// Commande shell (toujours non vide après filtrage).
    pub command: String,
    /// Répertoire de travail au moment de l'exécution (raccourci `~/…`).
    pub cwd: String,
    /// Horodatage de la commande (`YYYY-MM-DD HH:MM:SS`).
    pub timestamp: String,
    /// Racine Git (`git_root`), utilisée pour le groupement par projet.
    pub git_root: Option<String>,
}

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
        let records = db::project_records(conn, root, None, None, false, None)?;
        all.extend(records);
    }
    all.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
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
            git_root: r.git_root,
        })
        .collect()
}

/// Génère le document Markdown d'un runbook.
///
/// - `title` : titre du runbook (remplace la section `# Runbook - …`).
/// - `source_desc` : description lisible de la source (session, projet…).
/// - `entries` : commandes à inclure (les vides ont déjà été exclues en amont).
/// - `group_by` : mode de groupement des commandes.
///
/// Quand `entries` est vide, le document reste cohérent (section Commands avec
/// un message explicite, pas de panic).
pub fn render_markdown(
    title: &str,
    source_desc: &str,
    entries: &[RunbookEntry],
    group_by: GroupBy,
) -> String {
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

    match group_by {
        GroupBy::None => {
            for (i, entry) in entries.iter().enumerate() {
                out.push_str(&format!("### {}. {}\n\n", i + 1, entry.cwd));
                out.push_str(&md_code_block(std::slice::from_ref(&entry.command)));
                out.push('\n');
            }
        }
        GroupBy::Cwd | GroupBy::Project => {
            let grouped = group_entries(entries, group_by);
            for (group_key, group_entries) in &grouped {
                out.push_str(&format!("## {group_key}\n\n"));
                for (i, entry) in group_entries.iter().enumerate() {
                    out.push_str(&format!("### {}.\n\n", i + 1));
                    out.push_str(&md_code_block(std::slice::from_ref(&entry.command)));
                    out.push('\n');
                }
            }
        }
    }

    out
}

/// Ligne JSON pour une commande du runbook.
#[derive(Serialize)]
struct JsonCommand<'a> {
    n: usize,
    cwd: &'a str,
    timestamp: &'a str,
    command: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,
}

/// Document JSON racine du runbook.
#[derive(Serialize)]
struct JsonRunbook<'a> {
    title: &'a str,
    source: &'a str,
    generated_at: String,
    commands: Vec<JsonCommand<'a>>,
}

/// Génère la représentation JSON d'un runbook.
///
/// Structure stable et déterministe : même entrées → même JSON.
/// Avec `group_by` ≠ `None`, chaque commande porte un champ `"group"`.
pub fn render_json(
    title: &str,
    source_desc: &str,
    entries: &[RunbookEntry],
    group_by: GroupBy,
) -> Result<String> {
    let generated_at = db::now_timestamp();

    let commands: Vec<JsonCommand<'_>> = match group_by {
        GroupBy::None => entries
            .iter()
            .enumerate()
            .map(|(i, e)| JsonCommand {
                n: i + 1,
                cwd: &e.cwd,
                timestamp: &e.timestamp,
                command: &e.command,
                group: None,
            })
            .collect(),
        GroupBy::Cwd | GroupBy::Project => {
            let grouped = group_entries(entries, group_by);
            let mut cmds = Vec::with_capacity(entries.len());
            let mut n = 1usize;
            for (group_key, group_entries) in &grouped {
                for entry in group_entries {
                    cmds.push(JsonCommand {
                        n,
                        cwd: &entry.cwd,
                        timestamp: &entry.timestamp,
                        command: &entry.command,
                        group: Some(group_key.clone()),
                    });
                    n += 1;
                }
            }
            cmds
        }
    };

    let doc = JsonRunbook {
        title,
        source: source_desc,
        generated_at,
        commands,
    };

    serde_json::to_string_pretty(&doc).context("sérialisation JSON du runbook")
}

/// Retourne les entrées groupées par clé (alphabétique), les commandes dans
/// chaque groupe étant dans l'ordre de `entries` (chronologique).
fn group_entries<'a>(
    entries: &'a [RunbookEntry],
    group_by: GroupBy,
) -> BTreeMap<String, Vec<&'a RunbookEntry>> {
    let mut map: BTreeMap<String, Vec<&'a RunbookEntry>> = BTreeMap::new();
    for entry in entries {
        let key = match group_by {
            GroupBy::None => unreachable!(),
            GroupBy::Cwd => entry.cwd.clone(),
            GroupBy::Project => entry
                .git_root
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(display_home)
                .unwrap_or_else(|| "(sans projet)".to_string()),
        };
        map.entry(key).or_default().push(entry);
    }
    map
}

/// Point d'entrée de `mnemo runbook`.
///
/// Exactement un des drapeaux `last`, `session`, `project` doit être fourni
/// (mutuellement exclusifs côté clap). Si aucun n'est fourni, une erreur claire
/// est retournée.
///
/// Les secrets sont redactés par défaut ; `no_redact = true` désactive ce
/// comportement.
#[allow(clippy::too_many_arguments)]
pub fn run(
    last: bool,
    session: Option<String>,
    project: Option<String>,
    output: Option<PathBuf>,
    force: bool,
    limit: Option<u32>,
    title: Option<String>,
    format: RunbookFormat,
    no_redact: bool,
    group_by: GroupBy,
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

    let entries = if no_redact {
        entries
    } else {
        let cfg = config::Config::load()?;
        entries
            .into_iter()
            .map(|mut e| {
                if let Some(finding) = secrets::analyze(&e.command, &cfg.sensitive_keywords) {
                    e.command = finding.redacted;
                }
                e
            })
            .collect()
    };

    let resolved_title = title.unwrap_or(default_title);
    let content = match format {
        RunbookFormat::Markdown => {
            render_markdown(&resolved_title, &source_desc, &entries, group_by)
        }
        RunbookFormat::Json => render_json(&resolved_title, &source_desc, &entries, group_by)?,
    };

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
                git_root: None,
            })
            .collect()
    }

    #[test]
    fn render_contient_titre_et_sections() {
        let entries = make_entries(&[("cargo build", "~/proj")]);
        let md = render_markdown("mon runbook", "session s1", &entries, GroupBy::None);
        assert!(md.contains("# Runbook - mon runbook"));
        assert!(md.contains("## Metadata"));
        assert!(md.contains("## Commands"));
        assert!(md.contains("Source: session s1"));
        assert!(md.contains("Commands: 1"));
    }

    #[test]
    fn render_numerote_les_sections() {
        let entries = make_entries(&[("git pull", "~/a"), ("cargo test", "~/b")]);
        let md = render_markdown("test", "session s1", &entries, GroupBy::None);
        assert!(md.contains("### 1. ~/a"));
        assert!(md.contains("### 2. ~/b"));
        assert!(md.contains("git pull"));
        assert!(md.contains("cargo test"));
    }

    #[test]
    fn render_zero_commandes_reste_coherent() {
        let md = render_markdown("vide", "session s1", &[], GroupBy::None);
        assert!(md.contains("# Runbook - vide"));
        assert!(md.contains("## Commands"));
        assert!(md.contains("Commands: 0"));
        assert!(md.contains("_Aucune commande._"));
    }

    #[test]
    fn render_echappe_les_backticks_dans_les_blocs() {
        let entries = make_entries(&[("echo `date`", "~/proj")]);
        let md = render_markdown("bt", "session s1", &entries, GroupBy::None);
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

    #[test]
    fn format_json_produit_un_json_valide() {
        let entries = make_entries(&[("cargo build", "~/proj"), ("cargo test", "~/proj")]);
        let json = render_json("mon runbook", "session s1", &entries, GroupBy::None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).expect("JSON invalide");
        assert!(v["title"].is_string());
        assert!(v["source"].is_string());
        assert!(v["generated_at"].is_string());
        assert!(v["commands"].is_array());
        assert_eq!(v["commands"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn format_json_champ_command_present() {
        let entries = make_entries(&[("git status", "~/repo")]);
        let json = render_json("t", "s", &entries, GroupBy::None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let cmd = &v["commands"][0];
        assert!(cmd["command"].is_string());
        assert!(cmd["cwd"].is_string());
        assert!(cmd["timestamp"].is_string());
        assert_eq!(cmd["n"], 1);
    }

    #[test]
    fn group_by_none_liste_plate() {
        let entries = make_entries(&[("cmd1", "~/a"), ("cmd2", "~/b"), ("cmd3", "~/a")]);
        let md = render_markdown("t", "s", &entries, GroupBy::None);
        assert!(md.contains("### 1. ~/a"));
        assert!(md.contains("### 2. ~/b"));
        assert!(md.contains("### 3. ~/a"));
    }

    #[test]
    fn group_by_cwd_groupe_les_sections() {
        let entries = make_entries(&[
            ("cmd1", "~/proj/foo"),
            ("cmd2", "~/proj/bar"),
            ("cmd3", "~/proj/foo"),
        ]);
        let md = render_markdown("t", "s", &entries, GroupBy::Cwd);
        assert!(md.contains("## ~/proj/bar"));
        assert!(md.contains("## ~/proj/foo"));
        assert!(md.contains("### 1."));
        assert!(md.contains("### 2."));
        assert!(md.contains("cmd1"));
        assert!(md.contains("cmd2"));
        assert!(md.contains("cmd3"));
    }

    #[test]
    fn group_by_project_groupe_par_git_root() {
        let entries = vec![
            RunbookEntry {
                command: "cargo build".to_string(),
                cwd: "~/proj/a".to_string(),
                timestamp: "2026-01-01 10:00:00".to_string(),
                git_root: Some("/home/user/proj/a".to_string()),
            },
            RunbookEntry {
                command: "npm test".to_string(),
                cwd: "~/proj/b".to_string(),
                timestamp: "2026-01-01 10:01:00".to_string(),
                git_root: Some("/home/user/proj/b".to_string()),
            },
            RunbookEntry {
                command: "make".to_string(),
                cwd: "~/proj/a".to_string(),
                timestamp: "2026-01-01 10:02:00".to_string(),
                git_root: None,
            },
        ];
        let md = render_markdown("t", "s", &entries, GroupBy::Project);
        assert!(md.contains("(sans projet)"), "groupe sans projet attendu");
        assert!(md.contains("cargo build"));
        assert!(md.contains("npm test"));
        assert!(md.contains("make"));
    }

    #[test]
    fn group_by_json_ajoute_champ_group() {
        let entries = make_entries(&[("cmd1", "~/a"), ("cmd2", "~/b")]);
        let json = render_json("t", "s", &entries, GroupBy::Cwd).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let cmds = v["commands"].as_array().unwrap();
        for cmd in cmds {
            assert!(cmd["group"].is_string(), "champ group manquant");
        }
    }

    #[test]
    fn group_by_json_none_pas_de_champ_group() {
        let entries = make_entries(&[("cmd1", "~/a")]);
        let json = render_json("t", "s", &entries, GroupBy::None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let cmd = &v["commands"][0];
        assert!(
            cmd.get("group").is_none() || cmd["group"].is_null(),
            "group ne doit pas être présent en mode None"
        );
    }
}
