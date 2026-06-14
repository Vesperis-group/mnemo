//! Commande `mnemo stats` : statistiques d'usage (texte ou JSON).
//!
//! Le calcul est isolé dans [`compute`] (fonction pure sur une tranche de
//! [`CommandRecord`]) afin d'être testable sans base de données. Le nom des
//! commandes est normalisé par [`normalize_command_name`] pour produire un
//! « Top commandes » utile, débarrassé des tokens parasites (`-`, `|`, `#`…),
//! des préfixes de variables d'environnement et des wrappers (`sudo`, `env`,
//! `time`…).

use anyhow::Result;
use serde::Serialize;
use std::io::Write;

use crate::config;
use crate::db::{self, CommandRecord, SearchFilter};

/// Nombre d'entrées affichées dans chaque palmarès.
const TOP_N: usize = 10;

/// Filtres appliqués au calcul (repris pour l'affichage et le JSON).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Filters {
    pub project: Option<String>,
    pub branch: Option<String>,
}

impl Filters {
    fn is_empty(&self) -> bool {
        self.project.is_none() && self.branch.is_none()
    }
}

/// Statistiques agrégées.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    pub total: usize,
    pub git_projects: usize,
    pub failed: usize,
    /// Nombre de commandes non comptées dans le Top commandes (tokens parasites,
    /// commentaires, lignes vides…).
    pub ignored_for_top_commands: usize,
    pub top_commands: Vec<(String, usize)>,
    pub top_dirs: Vec<(String, usize)>,
    pub top_projects: Vec<(String, usize)>,
}

/// Point d'entrée de la commande.
pub fn run(project: Option<String>, branch: Option<String>, json: bool) -> Result<()> {
    let cfg = config::Config::load()?;
    let ignored = &cfg.stats.ignored_commands;
    let conn = db::open(&config::db_path()?)?;
    let filter = SearchFilter {
        project: project.clone(),
        branch: branch.clone(),
    };
    let records = db::all_commands(&conn, &filter)?;
    let stats = compute(&records, ignored);
    let filters = Filters { project, branch };

    // Sortie via un stdout verrouillé : un `BrokenPipe` (sortie pipée vers
    // `head`, `less`…) remonte comme une erreur propre au lieu de faire
    // paniquer `println!`.
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if json {
        writeln!(out, "{}", render_json(&stats, &filters, ignored))?;
        return Ok(());
    }

    // En mode texte, un filtre sans résultat mérite un message explicite.
    if records.is_empty() && !filters.is_empty() {
        writeln!(out, "Aucune commande trouvée pour ces filtres.")?;
        return Ok(());
    }

    render_text(&mut out, &stats, &filters)?;
    Ok(())
}

/// Calcule les statistiques à partir des commandes (fonction pure).
///
/// `ignored_commands` liste les noms (déjà normalisés en minuscules) à exclure
/// du « Top commandes » ; ces commandes restent comptées dans le total et dans
/// les autres sections, et sont ajoutées à `ignored_for_top_commands`.
pub fn compute(records: &[CommandRecord], ignored_commands: &[String]) -> Stats {
    use std::collections::{HashMap, HashSet};

    let total = records.len();
    let mut git_roots = HashSet::new();
    let mut failed = 0usize;
    let mut ignored = 0usize;

    let mut cmd_counts: HashMap<String, usize> = HashMap::new();
    let mut dir_counts: HashMap<String, usize> = HashMap::new();
    let mut proj_counts: HashMap<String, usize> = HashMap::new();

    for r in records {
        match normalize_command_name(&r.command) {
            Some(name) if is_ignored(&name, ignored_commands) => ignored += 1,
            Some(name) => *cmd_counts.entry(name).or_insert(0) += 1,
            None => ignored += 1,
        }
        if let Some(cwd) = r.cwd.as_deref().filter(|s| !s.is_empty()) {
            *dir_counts.entry(cwd.to_string()).or_insert(0) += 1;
        }
        if let Some(root) = r.git_root.as_deref().filter(|s| !s.is_empty()) {
            git_roots.insert(root.to_string());
            *proj_counts.entry(project_name(root)).or_insert(0) += 1;
        }
        if matches!(r.exit_code, Some(code) if code != 0) {
            failed += 1;
        }
    }

    Stats {
        total,
        git_projects: git_roots.len(),
        failed,
        ignored_for_top_commands: ignored,
        top_commands: top_n(cmd_counts, TOP_N),
        top_dirs: top_n(dir_counts, TOP_N),
        top_projects: top_n(proj_counts, TOP_N),
    }
}

/// Tokens parasites : opérateurs shell et mots-clés de structure qui ne sont
/// jamais des noms de programmes.
const JUNK_TOKENS: &[&str] = &[
    "-", "|", "||", "&&", "&", ";", ";;", "}", "{", ")", "(", "then", "fi", "done", "do", "else",
    "elif", "function", "in", "esac",
];

/// Wrappers qui précèdent la vraie commande et qu'il faut « traverser ».
const WRAPPERS: &[&str] = &["command", "builtin", "exec", "time", "nohup"];

/// Normalise une ligne de commande en nom de programme exploitable.
///
/// Renvoie `None` pour les lignes vides, les commentaires et les tokens
/// parasites. Sinon renvoie le nom du programme réellement invoqué, après avoir
/// retiré les affectations de variables d'environnement, les wrappers (`sudo`,
/// `env`, `time`…) et le chemin éventuel (`/usr/bin/git` → `git`).
pub fn normalize_command_name(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let mut i = 0;

    loop {
        // 1. Sauter les affectations d'environnement en tête (`FOO=bar`).
        while i < tokens.len() && is_env_assignment(tokens[i]) {
            i += 1;
        }
        let tok = match tokens.get(i) {
            Some(t) => *t,
            None => return None,
        };

        // 2. Traverser les wrappers connus.
        match tok {
            "sudo" => {
                i += 1;
                // Options de sudo (`-E`, `-H`, `-n`…). On ne gère pas les
                // options à argument (rare en pratique pour l'enrichissement).
                while i < tokens.len() && tokens[i].starts_with('-') {
                    i += 1;
                }
                continue;
            }
            "env" => {
                i += 1;
                while i < tokens.len() && tokens[i].starts_with('-') {
                    i += 1;
                }
                continue;
            }
            _ if WRAPPERS.contains(&tok) => {
                i += 1;
                continue;
            }
            _ => {}
        }

        // 3. Token réel : on retire le chemin éventuel, puis on filtre le bruit.
        let name = basename(tok);
        if name.is_empty() || JUNK_TOKENS.contains(&name) {
            return None;
        }
        return Some(name.to_string());
    }
}

/// Vrai si le token est une affectation `IDENT=...` (variable d'environnement).
fn is_env_assignment(token: &str) -> bool {
    let Some((key, _)) = token.split_once('=') else {
        return false;
    };
    if key.is_empty() {
        return false;
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Dernier segment d'un chemin (`/usr/bin/git` → `git`, `./x/mnemo` → `mnemo`).
fn basename(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

/// Vrai si un nom de commande normalisé figure dans la liste d'exclusion
/// (comparaison exacte, insensible à la casse).
fn is_ignored(name: &str, ignored_commands: &[String]) -> bool {
    let lowered = name.to_lowercase();
    ignored_commands.iter().any(|c| c == &lowered)
}

/// Nom de projet = dernier segment du chemin de la racine Git.
fn project_name(root: &str) -> String {
    root.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(root)
        .to_string()
}

/// Trie une table de comptage par occurrences décroissantes (puis clé pour un
/// ordre déterministe) et garde les `n` premières entrées.
fn top_n(counts: std::collections::HashMap<String, usize>, n: usize) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

// ---------------------------------------------------------------------------
// Rendu texte.
// ---------------------------------------------------------------------------

fn render_text<W: Write>(out: &mut W, stats: &Stats, filters: &Filters) -> std::io::Result<()> {
    writeln!(out, "mnemo stats - statistiques d'usage")?;
    writeln!(out, "----------------------------------")?;
    if !filters.is_empty() {
        writeln!(
            out,
            "Filtres                : projet={}, branche={}",
            filters.project.as_deref().unwrap_or("-"),
            filters.branch.as_deref().unwrap_or("-"),
        )?;
    }
    writeln!(out, "Commandes enregistrées : {}", stats.total)?;
    writeln!(out, "Projets Git détectés   : {}", stats.git_projects)?;
    writeln!(
        out,
        "Commandes en échec     : {} (exit_code ≠ 0)",
        stats.failed
    )?;

    render_section(out, "Top commandes", &stats.top_commands)?;
    if stats.ignored_for_top_commands > 0 {
        writeln!(
            out,
            "  Entrées ignorées dans le Top commandes : {}",
            stats.ignored_for_top_commands
        )?;
    }
    render_section(out, "Top dossiers", &stats.top_dirs)?;
    render_section(out, "Top projets Git", &stats.top_projects)?;
    Ok(())
}

fn render_section<W: Write>(
    out: &mut W,
    title: &str,
    entries: &[(String, usize)],
) -> std::io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{title} :")?;
    if entries.is_empty() {
        writeln!(out, "  (aucune donnée)")?;
        return Ok(());
    }
    for (label, count) in entries {
        writeln!(out, "  {count:>5}  {label}")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rendu JSON.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct NamedCount {
    name: String,
    count: usize,
}

#[derive(Serialize)]
struct PathCount {
    path: String,
    count: usize,
}

#[derive(Serialize)]
struct JsonOutput {
    total_commands: usize,
    git_projects: usize,
    failed_commands: usize,
    ignored_for_top_commands: usize,
    ignored_commands_config: Vec<String>,
    filters: Filters,
    top_commands: Vec<NamedCount>,
    top_directories: Vec<PathCount>,
    top_projects: Vec<NamedCount>,
}

fn render_json(stats: &Stats, filters: &Filters, ignored_commands: &[String]) -> String {
    let output = JsonOutput {
        total_commands: stats.total,
        git_projects: stats.git_projects,
        failed_commands: stats.failed,
        ignored_for_top_commands: stats.ignored_for_top_commands,
        ignored_commands_config: ignored_commands.to_vec(),
        filters: filters.clone(),
        top_commands: stats
            .top_commands
            .iter()
            .map(|(name, count)| NamedCount {
                name: name.clone(),
                count: *count,
            })
            .collect(),
        top_directories: stats
            .top_dirs
            .iter()
            .map(|(path, count)| PathCount {
                path: path.clone(),
                count: *count,
            })
            .collect(),
        top_projects: stats
            .top_projects
            .iter()
            .map(|(name, count)| NamedCount {
                name: name.clone(),
                count: *count,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(
        command: &str,
        cwd: Option<&str>,
        git_root: Option<&str>,
        exit: Option<i64>,
    ) -> CommandRecord {
        CommandRecord {
            id: 0,
            command: command.to_string(),
            cwd: cwd.map(str::to_string),
            shell: None,
            hostname: None,
            exit_code: exit,
            created_at: "2026-06-14 10:00:00".to_string(),
            git_root: git_root.map(str::to_string),
            git_branch: None,
            git_remote: None,
            session_id: None,
        }
    }

    #[test]
    fn normalize_commandes_valides() {
        assert_eq!(normalize_command_name("git status").as_deref(), Some("git"));
        assert_eq!(
            normalize_command_name("cargo build --release").as_deref(),
            Some("cargo")
        );
        assert_eq!(
            normalize_command_name("docker compose up -d").as_deref(),
            Some("docker")
        );
        assert_eq!(
            normalize_command_name("kubectl get pods").as_deref(),
            Some("kubectl")
        );
        assert_eq!(
            normalize_command_name("npx release-it").as_deref(),
            Some("npx")
        );
        assert_eq!(
            normalize_command_name("npm run build").as_deref(),
            Some("npm")
        );
    }

    #[test]
    fn normalize_sudo_env_wrappers() {
        assert_eq!(
            normalize_command_name("sudo apt update").as_deref(),
            Some("apt")
        );
        assert_eq!(
            normalize_command_name("sudo -E apt update").as_deref(),
            Some("apt")
        );
        assert_eq!(
            normalize_command_name("sudo env FOO=bar cargo test").as_deref(),
            Some("cargo")
        );
        assert_eq!(
            normalize_command_name("env RUST_LOG=debug cargo test").as_deref(),
            Some("cargo")
        );
        assert_eq!(
            normalize_command_name("RUST_LOG=debug cargo test").as_deref(),
            Some("cargo")
        );
        assert_eq!(
            normalize_command_name("FOO=bar BAR=baz npm run build").as_deref(),
            Some("npm")
        );
        assert_eq!(
            normalize_command_name("time cargo test").as_deref(),
            Some("cargo")
        );
        assert_eq!(
            normalize_command_name("command git status").as_deref(),
            Some("git")
        );
    }

    #[test]
    fn normalize_chemins() {
        assert_eq!(
            normalize_command_name("/usr/bin/git status").as_deref(),
            Some("git")
        );
        assert_eq!(
            normalize_command_name("./target/release/mnemo doctor").as_deref(),
            Some("mnemo")
        );
    }

    #[test]
    fn normalize_rejette_le_bruit() {
        assert_eq!(normalize_command_name("# commentaire"), None);
        assert_eq!(normalize_command_name("-"), None);
        assert_eq!(normalize_command_name("|"), None);
        assert_eq!(normalize_command_name("||"), None);
        assert_eq!(normalize_command_name("&&"), None);
        assert_eq!(normalize_command_name(";"), None);
        assert_eq!(normalize_command_name("then"), None);
        assert_eq!(normalize_command_name("fi"), None);
        assert_eq!(normalize_command_name("done"), None);
        assert_eq!(normalize_command_name("function"), None);
        assert_eq!(normalize_command_name(""), None);
        assert_eq!(normalize_command_name("    "), None);
    }

    #[test]
    fn stats_sur_base_vide() {
        let stats = compute(&[], &[]);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.git_projects, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.ignored_for_top_commands, 0);
        assert!(stats.top_commands.is_empty());
        assert!(stats.top_dirs.is_empty());
        assert!(stats.top_projects.is_empty());
    }

    #[test]
    fn stats_ignore_le_bruit_dans_le_top_commandes() {
        let records = vec![
            rec("git status", Some("/p/mnemo"), Some("/p/mnemo"), Some(0)),
            rec(
                "sudo apt update",
                Some("/p/mnemo"),
                Some("/p/mnemo"),
                Some(0),
            ),
            rec("-", Some("/p/mnemo"), Some("/p/mnemo"), Some(0)),
            rec("| grep x", Some("/p/mnemo"), Some("/p/mnemo"), Some(0)),
            rec(
                "# un commentaire",
                Some("/p/mnemo"),
                Some("/p/mnemo"),
                Some(0),
            ),
        ];
        let stats = compute(&records, &[]);

        assert_eq!(stats.total, 5);
        // 3 entrées bruitées : "-", "| grep x", "# un commentaire".
        assert_eq!(stats.ignored_for_top_commands, 3);
        let names: Vec<&str> = stats.top_commands.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"git"));
        assert!(names.contains(&"apt"));
        assert!(!names.contains(&"-"));
        assert!(!names.contains(&"|"));
        assert!(!names.contains(&"#"));
    }

    #[test]
    fn stats_sur_base_remplie() {
        let records = vec![
            rec(
                "cargo build",
                Some("/home/u/proj/mnemo"),
                Some("/home/u/proj/mnemo"),
                Some(0),
            ),
            rec(
                "cargo test",
                Some("/home/u/proj/mnemo"),
                Some("/home/u/proj/mnemo"),
                Some(1),
            ),
            rec(
                "git status",
                Some("/home/u/proj/mnemo"),
                Some("/home/u/proj/mnemo"),
                Some(0),
            ),
            rec("ls -la", Some("/tmp"), None, Some(0)),
            rec(
                "cargo run",
                Some("/home/u/proj/autre"),
                Some("/home/u/proj/autre"),
                Some(0),
            ),
        ];
        let stats = compute(&records, &[]);

        assert_eq!(stats.total, 5);
        assert_eq!(stats.git_projects, 2);
        assert_eq!(stats.failed, 1);
        assert_eq!(
            stats.top_commands.first().unwrap(),
            &("cargo".to_string(), 3)
        );
        assert_eq!(
            stats.top_projects.first().unwrap(),
            &("mnemo".to_string(), 3)
        );
    }

    #[test]
    fn stats_respecte_la_config_ignored_commands() {
        let records = vec![
            rec(
                "create_dir foo",
                Some("/p/mnemo"),
                Some("/p/mnemo"),
                Some(0),
            ),
            rec(
                "create_dir bar",
                Some("/p/mnemo"),
                Some("/p/mnemo"),
                Some(0),
            ),
            rec("cargo build", Some("/p/mnemo"), Some("/p/mnemo"), Some(0)),
            rec("git status", Some("/p/mnemo"), Some("/p/mnemo"), Some(0)),
        ];
        let ignored = vec!["create_dir".to_string()];
        let stats = compute(&records, &ignored);

        // Le total reste complet ; seules les stats du Top commandes changent.
        assert_eq!(stats.total, 4);
        // Les 2 `create_dir` sont comptés comme ignorés, pas dans le Top.
        assert_eq!(stats.ignored_for_top_commands, 2);
        let names: Vec<&str> = stats.top_commands.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"cargo"));
        assert!(names.contains(&"git"));
        assert!(!names.contains(&"create_dir"));
    }

    #[test]
    fn ignored_commands_insensible_a_la_casse() {
        let records = vec![rec("Create_Dir foo", None, None, Some(0))];
        // La liste est normalisée en minuscules par la config.
        let stats = compute(&records, &["create_dir".to_string()]);
        assert_eq!(stats.ignored_for_top_commands, 1);
        assert!(stats.top_commands.is_empty());
    }

    #[test]
    fn json_est_bien_forme() {
        let records = vec![rec(
            "cargo build",
            Some("/home/u/proj/mnemo"),
            Some("/home/u/proj/mnemo"),
            Some(0),
        )];
        let stats = compute(&records, &[]);
        let filters = Filters {
            project: Some("mnemo".to_string()),
            branch: None,
        };
        let s = render_json(&stats, &filters, &["create_dir".to_string()]);
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["total_commands"], 1);
        assert_eq!(parsed["filters"]["project"], "mnemo");
        assert!(parsed["filters"]["branch"].is_null());
        assert_eq!(parsed["top_commands"][0]["name"], "cargo");
        assert_eq!(parsed["top_commands"][0]["count"], 1);
        assert_eq!(parsed["top_directories"][0]["path"], "/home/u/proj/mnemo");
        assert_eq!(parsed["ignored_commands_config"][0], "create_dir");
    }

    #[test]
    fn nom_de_projet() {
        assert_eq!(project_name("/home/u/proj/mnemo"), "mnemo");
        assert_eq!(project_name("/home/u/proj/mnemo/"), "mnemo");
    }
}
