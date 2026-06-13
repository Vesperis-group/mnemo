use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as NucleoConfig, Matcher, Utf32Str};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};
use std::fs::{File, OpenOptions};

use crate::db::CommandRecord;

/// Lance la TUI de recherche. Retourne la commande sélectionnée (Entrée),
/// ou `None` si l'utilisateur a quitté (Esc / Ctrl+C).
pub fn run(records: Vec<CommandRecord>, initial_query: String) -> Result<Option<String>> {
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, records, initial_query);
    restore_terminal(&mut terminal)?;
    result
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

struct App {
    records: Vec<CommandRecord>,
    query: String,
    filtered: Vec<usize>,
    state: ListState,
    matcher: Matcher,
}

impl App {
    fn new(records: Vec<CommandRecord>, query: String) -> Self {
        Self {
            records,
            query,
            filtered: Vec::new(),
            state: ListState::default(),
            matcher: Matcher::new(NucleoConfig::DEFAULT),
        }
    }

    /// Recalcule la liste filtrée à partir de la requête courante.
    fn recompute(&mut self) {
        self.filtered = fuzzy_filter(&self.records, &self.query, &mut self.matcher);

        if self.filtered.is_empty() {
            self.state.select(None);
        } else {
            let sel = self
                .state
                .selected()
                .unwrap_or(0)
                .min(self.filtered.len() - 1);
            self.state.select(Some(sel));
        }
    }

    fn next(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) if i + 1 < self.filtered.len() => i + 1,
            Some(i) => i,
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(0) | None => 0,
            Some(i) => i - 1,
        };
        self.state.select(Some(i));
    }

    fn selected_command(&self) -> Option<String> {
        self.state
            .selected()
            .and_then(|i| self.filtered.get(i))
            .map(|&idx| self.records[idx].command.clone())
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<File>>,
    records: Vec<CommandRecord>,
    initial_query: String,
) -> Result<Option<String>> {
    let mut app = App::new(records, initial_query);
    app.recompute();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match (key.code, key.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(None),
                (KeyCode::Esc, _) => return Ok(None),
                (KeyCode::Enter, _) => return Ok(app.selected_command()),
                (KeyCode::Up, _) => app.previous(),
                (KeyCode::Down, _) => app.next(),
                (KeyCode::Backspace, _) => {
                    app.query.pop();
                    app.recompute();
                }
                (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                    app.query.push(c);
                    app.recompute();
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Barre de recherche.
    let search = Paragraph::new(app.query.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" mnemo — recherche "),
    );
    f.render_widget(search, chunks[0]);

    // Liste filtrée.
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&idx| {
            let r = &app.records[idx];
            let date = short_date(&r.created_at);
            let cwd = shorten_cwd(r.cwd.as_deref().unwrap_or(""));
            let line = Line::from(vec![
                Span::styled(format!("{date}  "), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{cwd}  "), Style::default().fg(Color::Blue)),
                Span::raw(r.command.clone()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} résultat(s) ", app.filtered.len())),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(Color::Yellow),
        )
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[1], &mut app.state);

    // Aide.
    let help = Paragraph::new("↑/↓ naviguer   Entrée sélectionner   Esc/Ctrl+C quitter")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, chunks[2]);
}

/// Tronque l'horodatage `YYYY-MM-DD HH:MM:SS` à la minute.
fn short_date(ts: &str) -> &str {
    ts.get(0..16).unwrap_or(ts)
}

/// Remplace le répertoire personnel par `~` pour un affichage plus court.
fn shorten_cwd(cwd: &str) -> String {
    if cwd.is_empty() {
        return String::new();
    }
    if let Some(home) = dirs::home_dir() {
        let home = home.display().to_string();
        if let Some(rest) = cwd.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    cwd.to_string()
}

/// Filtre fuzzy partagé entre la TUI et le mode `--print`.
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
    // Score décroissant ; à score égal, on conserve l'ordre récent d'origine.
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
