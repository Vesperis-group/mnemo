//! Commande `mnemo stats` : statistiques d'usage en texte simple.
//!
//! Le calcul est isolé dans [`compute`] (fonction pure sur une tranche de
//! [`CommandRecord`]) afin d'être testable sans base de données. Un futur
//! format `--json` pourra réutiliser la même structure [`Stats`].

use anyhow::Result;

use crate::config;
use crate::db::{self, CommandRecord};

/// Nombre d'entrées affichées dans chaque palmarès.
const TOP_N: usize = 10;

/// Statistiques agrégées.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    pub total: usize,
    pub git_projects: usize,
    pub failed: usize,
    pub top_commands: Vec<(String, usize)>,
    pub top_dirs: Vec<(String, usize)>,
    pub top_projects: Vec<(String, usize)>,
}

/// Point d'entrée de la commande.
pub fn run() -> Result<()> {
    let conn = db::open(&config::db_path()?)?;
    let records = db::all_commands(&conn)?;
    let stats = compute(&records);
    render(&stats);
    Ok(())
}

/// Calcule les statistiques à partir des commandes (fonction pure).
pub fn compute(records: &[CommandRecord]) -> Stats {
    let total = records.len();

    let mut git_roots = std::collections::HashSet::new();
    let mut failed = 0usize;

    let mut cmd_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut dir_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut proj_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for r in records {
        // Préfixe de commande = premier mot (le programme invoqué).
        if let Some(prefix) = command_prefix(&r.command) {
            *cmd_counts.entry(prefix).or_insert(0) += 1;
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
        top_commands: top_n(cmd_counts, TOP_N),
        top_dirs: top_n(dir_counts, TOP_N),
        top_projects: top_n(proj_counts, TOP_N),
    }
}

/// Premier mot d'une commande (préfixe), ou `None` si la commande est vide.
fn command_prefix(command: &str) -> Option<String> {
    command.split_whitespace().next().map(str::to_string)
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

/// Affiche les statistiques en texte simple.
fn render(stats: &Stats) {
    println!("mnemo stats — statistiques d'usage");
    println!("----------------------------------");
    println!("Commandes enregistrées : {}", stats.total);
    println!("Projets Git détectés   : {}", stats.git_projects);
    println!("Commandes en échec     : {} (exit_code ≠ 0)", stats.failed);

    render_section("Top commandes", &stats.top_commands);
    render_section("Top dossiers", &stats.top_dirs);
    render_section("Top projets Git", &stats.top_projects);
}

fn render_section(title: &str, entries: &[(String, usize)]) {
    println!();
    println!("{title} :");
    if entries.is_empty() {
        println!("  (aucune donnée)");
        return;
    }
    for (label, count) in entries {
        println!("  {count:>5}  {label}");
    }
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
    fn stats_sur_base_vide() {
        let stats = compute(&[]);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.git_projects, 0);
        assert_eq!(stats.failed, 0);
        assert!(stats.top_commands.is_empty());
        assert!(stats.top_dirs.is_empty());
        assert!(stats.top_projects.is_empty());
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
        let stats = compute(&records);

        assert_eq!(stats.total, 5);
        assert_eq!(stats.git_projects, 2); // mnemo + autre
        assert_eq!(stats.failed, 1); // cargo test exit 1

        // cargo est la commande la plus fréquente (3 occurrences).
        assert_eq!(
            stats.top_commands.first().unwrap(),
            &("cargo".to_string(), 3)
        );

        // Le projet mnemo domine (3 commandes).
        assert_eq!(
            stats.top_projects.first().unwrap(),
            &("mnemo".to_string(), 3)
        );
    }

    #[test]
    fn prefixe_et_nom_de_projet() {
        assert_eq!(command_prefix("git commit -m x"), Some("git".to_string()));
        assert_eq!(command_prefix("   "), None);
        assert_eq!(project_name("/home/u/proj/mnemo"), "mnemo");
        assert_eq!(project_name("/home/u/proj/mnemo/"), "mnemo");
    }
}
