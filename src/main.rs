mod cli;
mod config;
mod db;
mod doctor;
mod filter;
mod importer;
mod shell;
mod tui;
mod version;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use cli::{Cli, Command};

fn main() -> Result<()> {
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
        } => cmd_search(query.or(query_opt), print, limit),
        Command::Bashrc => {
            print!("{}", shell::bashrc_snippet());
            Ok(())
        }
        Command::Doctor { fix, json } => {
            let code = doctor::run(fix, json)?;
            std::process::exit(code);
        }
        Command::Version => {
            version::run();
            Ok(())
        }
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

    let conn = db::open(&config::db_path()?)?;
    let new = db::NewCommand {
        command: trimmed.to_string(),
        cwd: cwd.or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string())
        }),
        shell: Some("bash".to_string()),
        hostname: hostname(),
        exit_code: Some(exit_code),
        created_at: db::now_timestamp(),
    };
    db::insert_command(&conn, &new)?;
    Ok(())
}

fn cmd_search(query: Option<String>, print: bool, limit: usize) -> Result<()> {
    let cfg = config::Config::load()?;
    let conn = db::open(&config::db_path()?)?;
    let records = db::fetch_all(&conn, cfg.search_limit)?;
    if records.is_empty() {
        eprintln!("Aucune commande enregistrée. Lancez `mnemo import` d'abord.");
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

    if let Some(selected) = tui::run(records, query.unwrap_or_default())? {
        println!("{selected}");
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
