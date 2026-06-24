mod archive;
mod backup;
mod cli;
mod completions;
mod config;
mod confirm;
mod db;
mod doctor;
mod export;
mod filter;
mod gitctx;
mod importer;
mod init;
mod lifecycle;
mod list;
mod maintenance;
mod mdfmt;
mod migrations;
mod project;
mod prune;
mod secrets;
mod session;
mod shell;
mod show;
mod stats;
mod tui;
mod version;

use anyhow::{Context, Result};
use clap::Parser;
use std::io::Write;
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
        Command::Init { wizard, yes } => init::run(wizard, yes),
        Command::Completions { shell } => completions::run(shell),
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
            exit_code,
            failed,
            since,
            before,
            cwd,
            shell,
            json,
            id_only,
        } => cmd_search(cli::SearchArgs {
            query: query.or(query_opt),
            print,
            limit,
            project,
            branch,
            exit_code,
            failed,
            since,
            before,
            cwd,
            shell,
            json,
            id_only,
        }),
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
        Command::Shell { action } => match action {
            cli::ShellCommand::Upgrade => init::run_shell_upgrade(),
        },
        Command::Migrate => cmd_migrate(),
        Command::Stats {
            project,
            branch,
            since,
            json,
        } => stats::run(project, branch, since, json),
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
            gzip,
        } => export::run(format, project, branch, output, gzip),
        Command::List {
            limit,
            project,
            branch,
            json,
        } => list::run(limit, project, branch, json),
        Command::Show { id } => show::run_show(id),
        Command::Print { id } => show::run_print(id),
        Command::Delete { id, dry_run, yes } => prune::delete_run(id, dry_run, yes),
        Command::Prune {
            older_than,
            project,
            branch,
            dry_run,
            yes,
        } => prune::prune_run(older_than, project, branch, dry_run, yes),
        Command::Version => {
            version::run()?;
            Ok(())
        }
        Command::Update {
            json,
            upgrade,
            yes,
            require_signature,
        } => lifecycle::update::run(json, upgrade, yes, require_signature),
        Command::Upgrade {
            dry_run,
            yes,
            version,
            target,
            require_signature,
        } => lifecycle::upgrade::run(dry_run, yes, version, target, require_signature),
        Command::Uninstall {
            dry_run,
            yes,
            purge,
        } => lifecycle::uninstall::run(dry_run, yes, purge),
        Command::Project { action } => match action {
            cli::ProjectCommand::Current => project::run_current(),
            cli::ProjectCommand::List { limit, json } => project::run_list(limit, json),
            cli::ProjectCommand::Show {
                project,
                current,
                limit,
                json,
            } => project::run_show(project, current, limit, json),
            cli::ProjectCommand::Report {
                project,
                current,
                since,
                until,
                format,
                output,
                force,
                limit,
            } => project::run_report(project, current, since, until, format, output, force, limit),
        },
        Command::Maintenance { action } => match action {
            cli::MaintenanceCommand::Status => maintenance::run_status(),
            cli::MaintenanceCommand::Run { dry_run, yes } => maintenance::run(dry_run, yes),
        },
        Command::Session { action } => match action {
            cli::SessionCommand::List { limit } => session::run_list(limit),
            cli::SessionCommand::Show { session_id, limit } => session::run_show(session_id, limit),
            cli::SessionCommand::Export {
                session_id,
                last,
                format,
                output,
                force,
            } => session::run_export(session_id, last, format, output, force),
        },
        Command::Secrets { action } => match action {
            cli::SecretsCommand::Scan { limit, json } => secrets::run_scan(limit, json),
            cli::SecretsCommand::Redact {
                dry_run,
                apply,
                yes,
                backup,
            } => secrets::run_redact(dry_run, apply, yes, backup),
        },
    }
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

fn cmd_search(args: cli::SearchArgs) -> Result<()> {
    let cfg = config::Config::load()?;
    let conn = db::open(&config::db_path()?)?;

    // `--project current` se résout vers le nom du projet du dossier courant.
    let project = resolve_project_filter(args.project);

    // Bornes temporelles : une spec invalide est ignorée proprement (pas de
    // panique), avec un simple avertissement sur stderr.
    let since = resolve_time_bound(args.since.as_deref(), db::resolve_since, "--since");
    let before = resolve_time_bound(args.before.as_deref(), db::resolve_before, "--before");

    let filter = db::QueryFilter {
        project,
        branch: args.branch,
        cwd: args.cwd,
        shell: args.shell,
        exit_code: args.exit_code,
        failed: args.failed,
        since,
        before,
    };
    let no_filter = matches!(
        &filter,
        db::QueryFilter {
            project: None,
            branch: None,
            cwd: None,
            shell: None,
            exit_code: None,
            failed: false,
            since: None,
            before: None,
        }
    );

    let records = db::fetch_query(&conn, &filter, Some(cfg.search_limit))?;
    if records.is_empty() {
        if no_filter {
            eprintln!("Aucune commande enregistrée. Lancez `mnemo import` d'abord.");
        } else {
            eprintln!("Aucune commande ne correspond à ces filtres.");
        }
        return Ok(());
    }

    let query = args.query.unwrap_or_default();

    // `--json` et `--id-only` produisent une sortie destinée aux scripts : ils
    // impliquent le mode non interactif (inutile d'ouvrir la TUI pour ensuite
    // émettre du JSON ou une liste d'IDs).
    if args.print || args.json || args.id_only {
        // Sortie destinée aux scripts : écriture via un stdout verrouillé pour
        // qu'un `BrokenPipe` (sortie pipée vers `head`…) remonte comme une
        // erreur propre interceptée dans `main` plutôt que de paniquer.
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        if args.id_only {
            for rec in tui::search_records(&records, &query, args.limit) {
                writeln!(out, "{}", rec.id)?;
            }
        } else if args.json {
            let matching = tui::search_records(&records, &query, args.limit);
            writeln!(out, "{}", export::records_to_json(&matching)?)?;
        } else {
            for cmd in tui::search_print(&records, &query, args.limit) {
                writeln!(out, "{cmd}")?;
            }
        }
        return Ok(());
    }

    // Sinon : même moteur TUI que `mnemo tui`, avec les filtres CLI pré-remplis.
    let seed = tui::app::TuiFilters {
        project: filter.project.clone(),
        branch: filter.branch.clone(),
        cwd: filter.cwd.clone(),
        status: if filter.failed {
            tui::app::StatusFilter::Failure
        } else {
            tui::app::StatusFilter::All
        },
    };
    if let Some(selected) = tui::run_interactive(records, seed, query, cfg.search_limit)? {
        println!("{selected}");
    }
    Ok(())
}

/// Résout `--project current` vers le nom du projet du dossier courant ; laisse
/// toute autre valeur inchangée.
fn resolve_project_filter(project: Option<String>) -> Option<String> {
    match project {
        Some(p) if p.eq_ignore_ascii_case("current") => {
            let resolved = project::current_name();
            if resolved.is_none() {
                eprintln!("Projet courant indéterminé : filtre projet ignoré.");
            }
            resolved
        }
        other => other,
    }
}

/// Applique un résolveur de borne temporelle, en avertissant si la spec est
/// invalide (la borne est alors ignorée plutôt que de provoquer une panique).
fn resolve_time_bound(
    spec: Option<&str>,
    resolver: fn(&str) -> Option<String>,
    flag: &str,
) -> Option<String> {
    let spec = spec?;
    match resolver(spec) {
        Some(bound) => Some(bound),
        None => {
            eprintln!("Valeur {flag} invalide ({spec:?}) : filtre temporel ignoré.");
            None
        }
    }
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
        cli::ConfigCommand::Show => cmd_config_show(),
        cli::ConfigCommand::Path => cmd_config_path(),
        cli::ConfigCommand::Edit => cmd_config_edit(),
        cli::ConfigCommand::Validate => cmd_config_validate(),
        cli::ConfigCommand::StatsIgnore { action } => cmd_config_stats_ignore(action),
    }
}

/// `mnemo config show` : affiche la configuration effective (défauts inclus).
fn cmd_config_show() -> Result<()> {
    let cfg = config::Config::load()?;
    print!("{}", toml::to_string_pretty(&cfg)?);
    Ok(())
}

/// `mnemo config path` : affiche le chemin du fichier de configuration.
fn cmd_config_path() -> Result<()> {
    println!("{}", config::config_path()?.display());
    Ok(())
}

/// `mnemo config edit` : ouvre la configuration dans l'éditeur préféré.
///
/// Crée une configuration par défaut si elle est absente, puis en fait toujours
/// une sauvegarde avant ouverture (l'éditeur peut l'écraser).
fn cmd_config_edit() -> Result<()> {
    let path = config::config_path()?;
    if !path.exists() {
        config::Config::default().save(&path)?;
        println!("Configuration créée : {}", path.display());
    }
    if let Some(backup) = config::backup_existing(&path)? {
        println!("Sauvegarde : {}", backup.display());
    }

    let editor = resolve_editor();
    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("lancement de l'éditeur {editor:?}"))?;
    if !status.success() {
        eprintln!("L'éditeur {editor:?} s'est terminé sans succès.");
        return Ok(());
    }

    // Revalide après édition pour signaler une éventuelle erreur de saisie.
    match config::load_and_validate(&path) {
        Ok((_, issues)) if issues.is_empty() => println!("Configuration valide."),
        Ok((_, issues)) => print_config_issues(&issues),
        Err(err) => eprintln!("Configuration invalide : {err:#}"),
    }
    Ok(())
}

/// Détermine l'éditeur à utiliser : `$EDITOR`, puis `$VISUAL`, sinon `nano`,
/// sinon `vi`.
fn resolve_editor() -> String {
    for var in ["EDITOR", "VISUAL"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    if which("nano") {
        "nano".to_string()
    } else {
        "vi".to_string()
    }
}

/// Vrai si un exécutable est présent dans le `PATH`.
fn which(program: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
}

/// `mnemo config validate` : vérifie syntaxe TOML et valeurs connues.
fn cmd_config_validate() -> Result<()> {
    let path = config::config_path()?;
    if !path.exists() {
        println!(
            "Aucun fichier de configuration ({}) : valeurs par défaut utilisées (valides).",
            path.display()
        );
        return Ok(());
    }

    match config::load_and_validate(&path) {
        Ok((_, issues)) if issues.is_empty() => {
            println!("Configuration valide : {}", path.display());
            Ok(())
        }
        Ok((_, issues)) => {
            print_config_issues(&issues);
            let has_error = issues.iter().any(|i| i.level == config::IssueLevel::Error);
            if has_error {
                std::process::exit(1);
            }
            Ok(())
        }
        Err(err) => {
            eprintln!("Configuration invalide : {err:#}");
            std::process::exit(1);
        }
    }
}

/// Affiche les problèmes de configuration relevés.
fn print_config_issues(issues: &[config::ConfigIssue]) {
    for issue in issues {
        let tag = match issue.level {
            config::IssueLevel::Error => "ERREUR",
            config::IssueLevel::Warning => "ATTENTION",
        };
        println!("  [{tag}] {}", issue.message);
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
