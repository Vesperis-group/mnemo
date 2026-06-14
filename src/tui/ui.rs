//! Rendu Ratatui de la TUI (isolé de la logique).
//!
//! Trois zones : barre de recherche + filtres (haut), liste des commandes
//! (gauche), panneau de détails (droite). Des overlays gèrent l'aide, la
//! confirmation de suppression et le panneau de filtres.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::app::{TuiApp, TuiFilters, TuiMode};
use crate::tui::help;

/// Largeur/hauteur minimales en dessous desquelles on affiche un avertissement.
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;

/// Point d'entrée du rendu d'une frame.
pub fn render(f: &mut Frame, app: &mut TuiApp) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(f, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    render_header(f, chunks[0], app);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[1]);

    // La hauteur visible de la liste sert de taille de page.
    app.page_size = body[0].height.saturating_sub(2).max(1) as usize;

    render_list(f, body[0], app);
    render_details(f, body[1], app);
    render_footer(f, chunks[2], app);

    match app.mode {
        TuiMode::Help => render_help_popup(f, area),
        TuiMode::Filters => render_filters_popup(f, area, app),
        TuiMode::ConfirmDelete => render_confirm_popup(f, area, app),
        _ => {}
    }
}

fn render_too_small(f: &mut Frame, area: Rect) {
    let msg = Paragraph::new(format!(
        "Terminal trop petit ({}x{}).\nAgrandissez la fenêtre (min {MIN_WIDTH}x{MIN_HEIGHT}).",
        area.width, area.height
    ))
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::Yellow));
    f.render_widget(msg, area);
}

fn render_header(f: &mut Frame, area: Rect, app: &TuiApp) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(block_inner(f, area, " mnemo TUI "));

    let search = Line::from(vec![
        Span::styled("Recherche ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if app.query.is_empty() {
                "(tapez pour filtrer)".to_string()
            } else {
                app.query.clone()
            },
            if app.query.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            },
        ),
        Span::styled(
            if app.mode == TuiMode::Search {
                " ▏"
            } else {
                ""
            },
            Style::default().fg(Color::Yellow),
        ),
    ]);
    f.render_widget(Paragraph::new(search), inner[0]);
    f.render_widget(Paragraph::new(filters_line(&app.filters)), inner[1]);
}

/// Ligne décrivant les filtres actifs.
fn filters_line(filters: &TuiFilters) -> Line<'static> {
    let mut spans = vec![Span::styled(
        "Filtres ",
        Style::default().fg(Color::DarkGray),
    )];
    if filters.is_empty() {
        spans.push(Span::styled(
            "(aucun)",
            Style::default().fg(Color::DarkGray),
        ));
        return Line::from(spans);
    }
    let mut push = |label: &str, value: String| {
        spans.push(Span::styled(
            format!("{label}="),
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(
            format!("{value} "),
            Style::default().fg(Color::Cyan),
        ));
    };
    if let Some(p) = &filters.project {
        push("projet", p.clone());
    }
    if let Some(b) = &filters.branch {
        push("branche", b.clone());
    }
    if let Some(c) = &filters.cwd {
        push("dossier", c.clone());
    }
    if filters.status != crate::tui::app::StatusFilter::All {
        push("statut", filters.status.label().to_string());
    }
    Line::from(spans)
}

fn render_list(f: &mut Frame, area: Rect, app: &TuiApp) {
    let title = format!(" Commandes ({}) ", app.filtered.len());
    let block = Block::default().borders(Borders::ALL).title(title);

    if app.filtered.is_empty() {
        let hint = if app.records.is_empty() {
            "Aucune commande en base.\nImportez votre historique : `mnemo import`."
        } else {
            "Aucun résultat.\nModifiez la recherche ou videz les filtres (Ctrl+L)."
        };
        let p = Paragraph::new(hint)
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&idx| {
            let r = &app.records[idx];
            let date = r.created_at.get(0..16).unwrap_or(&r.created_at);
            let status = match r.exit_code {
                Some(0) => Span::styled("✓ ", Style::default().fg(Color::Green)),
                Some(_) => Span::styled("✗ ", Style::default().fg(Color::Red)),
                None => Span::raw("  "),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{date}  "), Style::default().fg(Color::DarkGray)),
                status,
                Span::raw(r.command.clone()),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(Color::Yellow),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_details(f: &mut Frame, area: Rect, app: &TuiApp) {
    let block = Block::default().borders(Borders::ALL).title(" Détails ");
    let Some(r) = app.selected_record() else {
        let p = Paragraph::new("(aucune sélection)")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    };

    let field = |label: &str, value: String| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label:<11}"), Style::default().fg(Color::DarkGray)),
            Span::raw(value),
        ])
    };
    let opt = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".to_string());

    let lines = vec![
        field("id", r.id.to_string()),
        Line::from(vec![Span::styled(
            "command",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(Span::styled(
            r.command.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        field("cwd", opt(&r.cwd)),
        field("shell", opt(&r.shell)),
        field("hostname", opt(&r.hostname)),
        field(
            "exit_code",
            r.exit_code
                .map(|c| c.to_string())
                .unwrap_or("-".to_string()),
        ),
        field("created_at", r.created_at.clone()),
        field("git_root", opt(&r.git_root)),
        field("git_branch", opt(&r.git_branch)),
        field("git_remote", opt(&r.git_remote)),
        field("session_id", opt(&r.session_id)),
    ];

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_footer(f: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let hints = match app.mode {
        TuiMode::Details => "j/k naviguer · / recherche · y copier · e exporter · x suppr · f succès/échecs · p/b projet/branche · ? aide · Esc/Ctrl+C quitter",
        _ => "Tab actions · Entrée sélectionner · Ctrl+P/B/D filtrer · Ctrl+L clear · F1 aide · Esc/Ctrl+C quitter",
    };
    f.render_widget(
        Paragraph::new(hints).style(Style::default().fg(Color::DarkGray)),
        chunks[0],
    );

    if let Some(msg) = &app.status_message {
        f.render_widget(
            Paragraph::new(msg.clone()).style(Style::default().fg(Color::Yellow)),
            chunks[1],
        );
    }
}

// -- Overlays --------------------------------------------------------------

fn render_help_popup(f: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 80, area);
    f.render_widget(Clear, popup);

    let lines: Vec<Line> = help::shortcuts()
        .into_iter()
        .map(|(key, desc)| {
            if desc.is_empty() {
                Line::from(Span::styled(
                    key,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("  {key:<20}"), Style::default().fg(Color::Yellow)),
                    Span::raw(desc),
                ])
            }
        })
        .collect();

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Aide - raccourcis (Esc pour fermer) "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

fn render_filters_popup(f: &mut Frame, area: Rect, app: &TuiApp) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);

    let lines = vec![
        filters_line(&app.filters),
        Line::from(""),
        Line::from("Depuis la sélection :"),
        Line::from("  p  filtrer par projet"),
        Line::from("  b  filtrer par branche"),
        Line::from("  w  filtrer par dossier (cwd)"),
        Line::from("  s  statut : tous / succès / échecs"),
        Line::from("  c  effacer tous les filtres"),
        Line::from(""),
        Line::from("Esc / f : fermer ce panneau"),
    ];
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Filtres interactifs "),
    );
    f.render_widget(p, popup);
}

fn render_confirm_popup(f: &mut Frame, area: Rect, app: &TuiApp) {
    let popup = centered_rect(60, 30, area);
    f.render_widget(Clear, popup);

    let id = app
        .selected_id()
        .map(|i| i.to_string())
        .unwrap_or_else(|| "?".to_string());
    let cmd = app
        .selected_record()
        .map(|r| r.command.clone())
        .unwrap_or_default();

    let lines = vec![
        Line::from(Span::styled(
            format!("Supprimer la commande #{id} ?"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::raw(cmd)),
        Line::from(""),
        Line::from("Cette action créera d'abord une sauvegarde."),
        Line::from(Span::styled(
            "[y] confirmer    [n / Esc] annuler",
            Style::default().fg(Color::Yellow),
        )),
    ];
    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirmation "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

// -- Utilitaires de layout -------------------------------------------------

/// Rend un bloc bordé titré et renvoie son aire intérieure.
fn block_inner(f: &mut Frame, area: Rect, title: &str) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.to_string());
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

/// Calcule un rectangle centré occupant `percent_x` × `percent_y` de `area`.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
