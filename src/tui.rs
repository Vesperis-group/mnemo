//! Interface TUI de mnemo.
//!
//! Le module racine gère le terminal et la boucle d'événements ; la logique est
//! répartie dans des sous-modules testables :
//! - [`app`] : modèle et logique métier (navigation, filtres, suppression) ;
//! - [`events`] : mapping clavier -> action ;
//! - [`actions`] : énumération des actions et accès base isolé ;
//! - [`ui`] : rendu Ratatui ;
//! - [`help`] : texte d'aide ;
//! - [`clipboard`] : copie système optionnelle.

pub mod actions;
pub mod app;
pub mod clipboard;
pub mod events;
pub mod help;
pub mod ui;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as NucleoConfig, Matcher, Utf32Str};
use ratatui::backend::CrosstermBackend;
use ratatui::{Frame, Terminal};
use std::fs::{File, OpenOptions};

use crate::db::CommandRecord;
use actions::{DbBackend, TuiBackend};
use app::{TuiApp, TuiFilters};

/// Lance la TUI avancée. Retourne la commande sélectionnée (Entrée), ou `None`
/// si l'utilisateur a quitté.
///
/// `limit` borne le rechargement (`r`). `backend` est injecté pour rester
/// testable, mais l'appelant utilise normalement [`DbBackend`].
pub fn run_interactive(
    records: Vec<CommandRecord>,
    filters: TuiFilters,
    initial_query: String,
    limit: usize,
) -> Result<Option<String>> {
    let mut backend = DbBackend::open(limit)?;
    let mut app = TuiApp::new(records, filters, initial_query);

    let mut terminal = setup_terminal()?;
    let result = event_loop(&mut terminal, &mut app, &mut backend);
    restore_terminal(&mut terminal)?;
    result?;

    Ok(app.outcome.clone())
}

/// On rend l'interface sur `/dev/tty` (et non stdout) afin que `mnemo search`
/// fonctionne dans une substitution de commande `$(mnemo search)`.
fn setup_terminal() -> Result<Terminal<CrosstermBackend<File>>> {
    enable_raw_mode()?;
    let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    execute!(tty, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(tty);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<File>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn event_loop<B: TuiBackend>(
    terminal: &mut Terminal<CrosstermBackend<File>>,
    app: &mut TuiApp,
    backend: &mut B,
) -> Result<()> {
    loop {
        terminal.draw(|f: &mut Frame| ui::render(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let action = events::map_key(app.mode, key.code, key.modifiers);
            app.dispatch(action, backend);
            if app.should_quit {
                break;
            }
        }
    }
    Ok(())
}

/// Filtre fuzzy partagé avec le mode `--print`.
///
/// Retourne les indices des `records` correspondant à `query`, triés par score
/// décroissant. Une requête vide renvoie tous les indices dans l'ordre reçu.
pub fn fuzzy_filter(records: &[CommandRecord], query: &str, matcher: &mut Matcher) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..records.len()).collect();
    }
    let pattern = Pattern::parse(query.trim(), CaseMatching::Ignore, Normalization::Smart);
    let mut buf: Vec<char> = Vec::new();
    let mut scored: Vec<(usize, u32)> = records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let haystack = Utf32Str::new(&r.command, &mut buf);
            pattern.score(haystack, matcher).map(|score| (i, score))
        })
        .collect();
    scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
    scored.into_iter().map(|(i, _)| i).collect()
}

/// Recherche non interactive : renvoie les commandes correspondantes (au plus
/// `limit`), pour le mode `--print` et les scripts/CI.
pub fn search_print(records: &[CommandRecord], query: &str, limit: usize) -> Vec<String> {
    let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
    fuzzy_filter(records, query, &mut matcher)
        .into_iter()
        .take(limit)
        .map(|i| records[i].command.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::CommandRecord;

    fn rec(id: i64, command: &str) -> CommandRecord {
        CommandRecord {
            id,
            command: command.to_string(),
            cwd: None,
            shell: None,
            hostname: None,
            exit_code: None,
            created_at: "2026-06-13 10:00:00".to_string(),
            git_root: None,
            git_branch: None,
            git_remote: None,
            session_id: None,
        }
    }

    fn sample() -> Vec<CommandRecord> {
        vec![
            rec(1, "cargo build --release"),
            rec(2, "git status"),
            rec(3, "cargo test"),
            rec(4, "ls -la"),
        ]
    }

    #[test]
    fn print_filtre_par_requete() {
        let records = sample();
        let out = search_print(&records, "cargo", 10);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|c| c.contains("cargo")));
    }

    #[test]
    fn print_respecte_la_limite() {
        let records = sample();
        let out = search_print(&records, "", 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn requete_vide_renvoie_tout() {
        let records = sample();
        let out = search_print(&records, "   ", 100);
        assert_eq!(out.len(), records.len());
    }
}
