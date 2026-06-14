mod archive;
mod backup;
mod cli;
mod config;
mod confirm;
mod db;
mod doctor;
mod export;
mod filter;
mod gitctx;
mod importer;
mod lifecycle;
mod list;
mod migrations;
mod prune;
mod shell;
mod stats;
mod tui;
mod version;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use cli::{Cli, Command};

fn main() {
    if let Err(err) = run() {
        // Une sortie pipée vers `head`, `less`, etc. ferme stdout en avance :
        // mnemo reçoit alors un `BrokenPipe`. C'est le comportement Unix normal,
        // pas une erreur applicative : on quitte silencieusement avec le code 0.
        if err.chain().any(is_broken_pipe_error) {
            std::process::exit(0);
        }
        // Toute autre erreur reste visible et échoue avec un code non nul.
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

/// Détecte une erreur `BrokenPipe`, y compris lorsqu'elle est enveloppée dans
/// une chaîne de causes (`source()`).
fn is_broken_pipe_error(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = current {
        if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
            if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                return true;
            }
        }
        current = e.source();
    }
    false
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init => cmd_init(),
        Command::Import { file } => cmd_import(file),
        Command::Add {
            cmd,
            cwd,
            exit_code,
        } => cmd_add(cmd, cwd, exit_code),
        Command::Search {
            query,
            query_opt,
            print,
            limit,
            project,
            branch,
        } => cmd_search(query.or(query_opt), print, limit, project, branch),
        Command::Tui {
            query,
            project,
            branch,
            cwd,
            failed,
        } => cmd_tui(query, project, branch, cwd, failed),
        Command::Bashrc => {
            print!("{}", shell::bashrc_snippet());
            Ok(())
        }
        Command::Migrate => cmd_migrate(),
        Command::Stats {
            project,
            branch,
            json,
        } => stats::run(project, branch, json),
        Command::Doctor { fix, json } => {
            let code = doctor::run(fix, json)?;
            std::process::exit(code);
        }
        Command::Config { action } => cmd_config(action),
        Command::Backup { output, json } => backup::run(output, json),
        Command::Restore {
            archive,
            dry_run,
            yes,
        } => backup::restore_run(&archive, dry_run, yes),
        Command::Export {
            format,
            project,
            branch,
            output,
        } => export::run(format, project, branch, output),
        Command::List {
            limit,
            project,
            branch,
            json,
        } => list::run(limit, project, branch, json),
        Command::Delete { id, dry_run, yes } => prune::delete_run(id, dry_run, yes),
        Command::Prune {
            older_than,
            project,
            branch,
            dry_run,
            yes,
        } => prune::prune_run(older_than, project, branch, dry_run, yes),
        Command::Version => {
            version::run();
            Ok(())
        }
        Command::Update { json, upgrade, yes } => lifecycle::update::run(json, upgrade, yes),
        Command::Upgrade {
            dry_run,
            yes,
            version,
            target,
        } => lifecycle::upgrade::run(dry_run, yes, version, target),
        Command::Uninstall {
            dry_run,
            yes,
            purge,
        } => lifecycle::uninstall::run(dry_run, yes, purge),
    }
}

fn cmd_init() -> Result<()> {
    let cfg_path = config::config_path()?;
    if cfg_path.exists() {
        println!("Configuration existante : {}", cfg_path.display());
    } else {
        config::Config::default().save(&cfg_path)?;
        println!("Configuration créée : {}", cfg_path.display());
    }

    let db_path = config::db_path()?;
    db::open(&db_path)?;
    println!("Base de données : {}", db_path.display());

    println!();
    println!("Ajoutez ces lignes à votre ~/.bashrc :");
    println!("------------------------------------------------------------");
    print!("{}", shell::bashrc_snippet());
    println!("------------------------------------------------------------");
    println!("Puis rechargez : source ~/.bashrc");
    Ok(())
}

fn cmd_import(file: Option<PathBuf>) -> Result<()> {
    let cfg = config::Config::load()?;
    let conn = db::open(&config::db_path()?)?;
    let path = match file {
        Some(p) => p,
        None => dirs::home_dir()
            .context("répertoire personnel introuvable")?
            .join(".bash_history"),
    };

    let stats = importer::import_bash_history(&conn, &path, &cfg)?;
    println!("Import depuis {}", path.display());
    println!("  Lignes traitées     : {}", stats.total);
    println!("  Importées           : {}", stats.imported);
    println!("  Sensibles ignorées  : {}", stats.skipped_sensitive);
    println!("  Doublons ignorés    : {}", stats.skipped_duplicate);
    Ok(())
}

fn cmd_add(cmd: String, cwd: Option<String>, exit_code: i64) -> Result<()> {
    let cfg = config::Config::load()?;
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    // Ne jamais enregistrer une commande sensible ou explicitement ignorée.
    if filter::is_sensitive(trimmed, &cfg.sensitive_keywords) {
        return Ok(());
    }
    if cfg
        .ignore_prefixes
        .iter()
        .any(|p| trimmed.starts_with(p.as_str()))
    {
        return Ok(());
    }

    // Répertoire de travail effectif : argument explicite ou répertoire courant.
    let resolved_cwd = cwd.or_else(|| {
        std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string())
    });

    // Détection Git (optionnelle) sur ce répertoire.
    let git = resolved_cwd
        .as_deref()
        .map(|p| gitctx::detect(std::path::Path::new(p)))
        .unwrap_or_default();

    let conn = db::open(&config::db_path()?)?;
    let new = db::NewCommand {
        command: trimmed.to_string(),
        cwd: resolved_cwd,
        shell: Some("bash".to_string()),
        hostname: hostname(),
        exit_code: Some(exit_code),
        created_at: db::now_timestamp(),
        git_root: git.root,
        git_branch: git.branch,
        git_remote: git.remote,
        session_id: std::env::var("MNEMO_SESSION_ID")
            .ok()
            .filter(|s| !s.trim().is_empty()),
    };
    db::insert_command(&conn, &new)?;
    Ok(())
}

fn cmd_search(
    query: Option<String>,
    print: bool,
    limit: usize,
    project: Option<String>,
    branch: Option<String>,
) -> Result<()> {
    let cfg = config::Config::load()?;
    let conn = db::open(&config::db_path()?)?;
    let filter = db::SearchFilter { project, branch };
    let records = db::fetch_filtered(&conn, &filter, cfg.search_limit)?;
    if records.is_empty() {
        if filter.is_empty() {
            eprintln!("Aucune commande enregistrée. Lancez `mnemo import` d'abord.");
        } else {
            eprintln!("Aucune commande ne correspond à ce filtre Git.");
        }
        return Ok(());
    }

    // Mode non interactif : affiche les résultats sur stdout (scripts/CI).
    if print {
        let query = query.unwrap_or_default();
        for cmd in tui::search_print(&records, &query, limit) {
            println!("{cmd}");
        }
        return Ok(());
    }

    // Sinon : même moteur TUI que `mnemo tui`, avec les filtres CLI pré-remplis.
    let seed = tui::app::TuiFilters {
        project: filter.project.clone(),
        branch: filter.branch.clone(),
        ..Default::default()
    };
    if let Some(selected) =
        tui::run_interactive(records, seed, query.unwrap_or_default(), cfg.search_limit)?
    {
        println!("{selected}");
    }
    Ok(())
}

/// Sous-commande `mnemo tui` : interface interactive principale.
fn cmd_tui(
    query: Option<String>,
    project: Option<String>,
    branch: Option<String>,
    cwd: Option<String>,
    failed: bool,
) -> Result<()> {
    // Base absente : proposer `mnemo init` plutôt que de la créer en silence.
    let db_path = config::db_path()?;
    if !db_path.exists() {
        eprintln!("Base de données absente. Lancez `mnemo init` puis `mnemo import`.");
        return Ok(());
    }

    let cfg = config::Config::load()?;
    let records = {
        let conn = db::open(&db_path)?;
        db::fetch_filtered(&conn, &db::SearchFilter::default(), cfg.search_limit)?
    };

    let filters = tui::app::TuiFilters {
        project,
        branch,
        cwd,
        status: if failed {
            tui::app::StatusFilter::Failure
        } else {
            tui::app::StatusFilter::All
        },
    };

    if let Some(selected) = tui::run_interactive(
        records,
        filters,
        query.unwrap_or_default(),
        cfg.search_limit,
    )? {
        println!("{selected}");
    }
    Ok(())
}

/// Applique les migrations de schéma en attente et rend compte de la transition.
fn cmd_migrate() -> Result<()> {
    let db_path = config::db_path()?;
    let (_conn, outcome) = db::open_and_migrate(&db_path)?;
    println!("Base de données : {}", db_path.display());
    if outcome.migrated() {
        println!("Schéma migré : v{} -> v{}", outcome.from, outcome.to);
    } else {
        println!("Schéma déjà à jour : v{} (aucune migration)", outcome.to);
    }
    Ok(())
}

/// Sous-commande `mnemo config …` : gestion de la configuration locale.
fn cmd_config(action: cli::ConfigCommand) -> Result<()> {
    match action {
        cli::ConfigCommand::StatsIgnore { action } => cmd_config_stats_ignore(action),
    }
}

fn cmd_config_stats_ignore(action: cli::StatsIgnoreCommand) -> Result<()> {
    use cli::StatsIgnoreCommand;

    let path = config::config_path()?;
    let mut cfg = config::Config::load()?;

    match action {
        StatsIgnoreCommand::Add { name } => {
            let normalized = config::Config::normalize_ignored(&name);
            if cfg.add_ignored_command(&name) {
                cfg.save(&path)?;
                println!("Commande ignorée ajoutée : {normalized}");
            } else {
                println!("Commande déjà présente : {normalized}");
            }
        }
        StatsIgnoreCommand::Remove { name } => {
            let normalized = config::Config::normalize_ignored(&name);
            if cfg.remove_ignored_command(&name) {
                cfg.save(&path)?;
                println!("Commande retirée : {normalized}");
            } else {
                println!("Commande absente : {normalized}");
            }
        }
        StatsIgnoreCommand::List => {
            if cfg.stats.ignored_commands.is_empty() {
                println!("Aucune commande ignorée configurée.");
            } else {
                println!("Commandes ignorées dans stats :");
                for c in &cfg.stats.ignored_commands {
                    println!("  {c}");
                }
            }
        }
    }
    Ok(())
}

/// Détermine le nom d'hôte de la machine.
fn hostname() -> Option<String> {
    if let Ok(h) = std::env::var("HOSTNAME") {
        let h = h.trim();
        if !h.is_empty() {
            return Some(h.to_string());
        }
    }
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::is_broken_pipe_error;
    use std::io::{Error, ErrorKind};

    #[test]
    fn detecte_un_broken_pipe_direct() {
        let err = Error::new(ErrorKind::BrokenPipe, "broken pipe");
        assert!(is_broken_pipe_error(&err));
    }

    #[test]
    fn ignore_les_autres_erreurs_io() {
        let err = Error::new(ErrorKind::NotFound, "absent");
        assert!(!is_broken_pipe_error(&err));
    }

    #[test]
    fn detecte_un_broken_pipe_enveloppe_via_source() {
        // Erreur custom dont la source est un BrokenPipe.
        #[derive(Debug)]
        struct Wrapper(Error);
        impl std::fmt::Display for Wrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "wrapper")
            }
        }
        impl std::error::Error for Wrapper {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        let wrapped = Wrapper(Error::new(ErrorKind::BrokenPipe, "broken pipe"));
        assert!(is_broken_pipe_error(&wrapped));

        let wrapped_other = Wrapper(Error::new(ErrorKind::PermissionDenied, "refusé"));
        assert!(!is_broken_pipe_error(&wrapped_other));
    }

    #[test]
    fn anyhow_chain_contient_le_broken_pipe() {
        // Reproduit ce que voit `main()` : une anyhow::Error avec contexte.
        let io = Error::new(ErrorKind::BrokenPipe, "broken pipe");
        let err = anyhow::Error::new(io).context("écriture sur stdout");
        assert!(err.chain().any(is_broken_pipe_error));
    }
}
