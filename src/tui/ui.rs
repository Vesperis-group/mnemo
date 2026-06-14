//! Rendu Ratatui de la TUI, style « ops dashboard ».
//!
//! Disposition verticale :
//! - **barre de commande** (haut) : badges identité (mnemo + version + projet +
//!   branche + total), barre de recherche, puces de filtres actifs ;
//! - **synthèse / KPI** : total, visibles, succès, échecs, taux d'échec,
//!   projets, shell dominant (masquée sur terminal court) ;
//! - **corps** : liste des commandes (gauche) et panneau de détails sectionné
//!   (droite, masqué sur terminal étroit) ;
//! - **pied** : raccourcis essentiels et message de statut.
//!
//! Toutes les couleurs passent par [`crate::tui::theme`] ; le formatage des
//! chaînes par [`crate::tui::format`]. Le rendu est responsive et ne panique
//! jamais, y compris en dimensions réduites.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::app::{Overview, TuiApp, TuiFilters, TuiMode};
use crate::tui::{format, help, theme};

/// Largeur/hauteur minimales en dessous desquelles on affiche un avertissement.
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;
/// En dessous de cette hauteur, la barre de synthèse est masquée.
const OVERVIEW_MIN_HEIGHT: u16 = 18;
/// En dessous de cette largeur, le panneau de détails est masqué.
const DETAILS_MIN_WIDTH: u16 = 84;

/// Version compilée de mnemo, affichée comme badge d'identité.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Point d'entrée du rendu d'une frame.
pub fn render(f: &mut Frame, app: &mut TuiApp) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(f, area);
        return;
    }

    let show_overview = area.height >= OVERVIEW_MIN_HEIGHT;
    let overview_h = if show_overview { 3 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // barre de commande (badges + recherche + filtres)
            Constraint::Length(overview_h),
            Constraint::Min(1),    // corps
            Constraint::Length(2), // pied
        ])
        .split(area);

    render_command_bar(f, chunks[0], app);
    if show_overview {
        render_overview(f, chunks[1], &app.overview());
    }

    let show_details = area.width >= DETAILS_MIN_WIDTH;
    let body = if show_details {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[2])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(chunks[2])
    };

    // La hauteur visible de la liste sert de taille de page.
    app.page_size = body[0].height.saturating_sub(2).max(1) as usize;

    render_list(f, body[0], app);
    if show_details {
        render_details(f, body[1], app);
    }
    render_footer(f, chunks[3], app);

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
    .style(Style::default().fg(theme::WARNING));
    f.render_widget(msg, area);
}

// -- Barre de commande -----------------------------------------------------

/// Badge « clé valeur » avec une couleur d'accent sur la valeur.
fn badge(label: &str, value: &str, color: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("{label} "), theme::label()),
        Span::styled(value.to_string(), theme::badge(color)),
    ]
}

fn render_command_bar(f: &mut Frame, area: Rect, app: &TuiApp) {
    let inner = block_inner(f, area, " mnemo ");
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // badges
            Constraint::Length(1), // recherche
            Constraint::Length(1), // filtres
        ])
        .split(inner);

    f.render_widget(Paragraph::new(badges_line(app)), rows[0]);
    f.render_widget(Paragraph::new(search_line(app)), rows[1]);
    f.render_widget(Paragraph::new(filters_line(&app.filters)), rows[2]);
}

/// Ligne d'identité : mnemo + version + projet + branche + total.
fn badges_line(app: &TuiApp) -> Line<'static> {
    let mut spans = vec![
        Span::styled("mnemo ", theme::title()),
        Span::styled(format!("v{VERSION}"), theme::badge(theme::INFO)),
    ];
    let sep = || Span::styled("  ·  ", theme::label());

    if let Some(project) = &app.current_project {
        spans.push(sep());
        spans.extend(badge("projet", project, theme::INFO));
    }
    if let Some(branch) = &app.current_branch {
        spans.push(sep());
        spans.extend(badge("branche", branch, theme::ACCENT));
    }
    spans.push(sep());
    spans.extend(badge(
        "total",
        &app.records.len().to_string(),
        theme::WARNING,
    ));
    Line::from(spans)
}

/// Ligne de recherche avec curseur en mode Search.
fn search_line(app: &TuiApp) -> Line<'static> {
    let (text, style) = if app.query.is_empty() {
        (
            "(tapez pour filtrer)".to_string(),
            Style::default().fg(theme::MUTED),
        )
    } else {
        (app.query.clone(), theme::strong())
    };
    Line::from(vec![
        Span::styled("Recherche ", theme::label()),
        Span::styled(text, style),
        Span::styled(
            if app.mode == TuiMode::Search {
                "▏"
            } else {
                ""
            },
            Style::default().fg(theme::WARNING),
        ),
    ])
}

/// Ligne de puces décrivant les filtres actifs.
fn filters_line(filters: &TuiFilters) -> Line<'static> {
    let mut spans = vec![Span::styled("Filtres ", theme::label())];
    if filters.is_empty() {
        spans.push(Span::styled("[aucun filtre]", theme::label()));
        return Line::from(spans);
    }
    let mut chip = |label: &str, value: String, color: Color| {
        spans.push(Span::styled(format!("[{label}: "), theme::label()));
        spans.push(Span::styled(value, theme::badge(color)));
        spans.push(Span::styled("] ", theme::label()));
    };
    if let Some(p) = &filters.project {
        chip("projet", p.clone(), theme::INFO);
    }
    if let Some(b) = &filters.branch {
        chip("branche", b.clone(), theme::ACCENT);
    }
    if let Some(c) = &filters.cwd {
        chip("dossier", format::truncate_middle(c, 28), theme::INFO);
    }
    if filters.status != crate::tui::app::StatusFilter::All {
        let color = match filters.status {
            crate::tui::app::StatusFilter::Success => theme::SUCCESS,
            crate::tui::app::StatusFilter::Failure => theme::DANGER,
            crate::tui::app::StatusFilter::All => theme::MUTED,
        };
        chip("statut", filters.status.label().to_string(), color);
    }
    Line::from(spans)
}

// -- Synthèse / KPI --------------------------------------------------------

fn render_overview(f: &mut Frame, area: Rect, ov: &Overview) {
    let inner = block_inner(f, area, " Synthèse ");

    let rate = ov.failure_rate();
    let rate_color = if rate >= 25.0 {
        theme::DANGER
    } else if rate >= 10.0 {
        theme::WARNING
    } else {
        theme::SUCCESS
    };

    let mut spans = vec![
        Span::styled("Total ", theme::label()),
        Span::styled(ov.total.to_string(), theme::strong()),
        Span::styled("   Visibles ", theme::label()),
        Span::styled(ov.visible.to_string(), theme::strong()),
        Span::styled("   Succès ", theme::label()),
        Span::styled(ov.success.to_string(), theme::badge(theme::SUCCESS)),
        Span::styled("   Échecs ", theme::label()),
        Span::styled(ov.failed.to_string(), theme::badge(theme::DANGER)),
        Span::styled("   Taux d'échec ", theme::label()),
        Span::styled(format!("{rate:.1}%"), theme::badge(rate_color)),
        Span::styled("   Projets ", theme::label()),
        Span::styled(ov.projects.to_string(), theme::badge(theme::INFO)),
    ];
    if let Some(shell) = &ov.top_shell {
        spans.push(Span::styled("   Shell ", theme::label()));
        spans.push(Span::styled(shell.clone(), theme::badge(theme::ACCENT)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), inner);
}

// -- Liste -----------------------------------------------------------------

fn render_list(f: &mut Frame, area: Rect, app: &TuiApp) {
    let title = format!(" Commandes ({}) ", app.filtered.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(title, theme::title()));

    if app.filtered.is_empty() {
        let hint = if app.records.is_empty() {
            "Aucune commande en base.\nImportez votre historique : mnemo import."
        } else {
            "Aucun résultat.\nAjustez la recherche ou videz les filtres (Ctrl+L)."
        };
        let p = Paragraph::new(hint)
            .block(block)
            .alignment(Alignment::Center)
            .style(theme::label());
        f.render_widget(p, area);
        return;
    }

    // Largeur disponible pour la commande, après colonnes heure/statut/contexte.
    let inner_width = area.width.saturating_sub(2) as usize;
    let ctx_width = 14usize;
    let fixed = 2 /* symbole > */ + 5 /* heure */ + 2 + 2 /* statut */ + ctx_width + 1;
    let cmd_width = inner_width.saturating_sub(fixed).max(8);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&idx| {
            let r = &app.records[idx];
            let time = format::short_time(&r.created_at);
            let status = Span::styled(
                format!("{} ", format::status_symbol(r.exit_code)),
                Style::default().fg(theme::status_color(r.exit_code)),
            );
            let ctx = format!(
                "{:<width$}",
                format::truncate_end(&format::context_label(r), ctx_width),
                width = ctx_width
            );
            ListItem::new(Line::from(vec![
                Span::styled(format!("{time}  "), Style::default().fg(theme::WARNING)),
                status,
                Span::styled(ctx, Style::default().fg(theme::INFO)),
                Span::raw(" "),
                Span::styled(format::truncate_end(&r.command, cmd_width), theme::value()),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::selected())
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(list, area, &mut state);
}

// -- Détails ---------------------------------------------------------------

fn render_details(f: &mut Frame, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(" Détails ", theme::title()));
    let Some(r) = app.selected_record() else {
        let p = Paragraph::new("Sélectionnez une commande pour voir ses détails.")
            .block(block)
            .alignment(Alignment::Center)
            .style(theme::label());
        f.render_widget(p, area);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    let opt = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".to_string());

    // Section COMMAND : commande + badge de statut.
    lines.push(section("COMMAND"));
    lines.push(Line::from(Span::styled(r.command.clone(), theme::strong())));
    lines.push(Line::from(vec![
        Span::styled("statut     ", theme::label()),
        Span::styled(
            format::status_text(r.exit_code),
            theme::badge(theme::status_color(r.exit_code)),
        ),
    ]));
    lines.push(Line::from(""));

    // Section CONTEXT.
    lines.push(section("CONTEXT"));
    lines.push(field("cwd", opt(&r.cwd)));
    lines.push(field("hostname", opt(&r.hostname)));
    lines.push(field("shell", opt(&r.shell)));
    lines.push(Line::from(""));

    // Section EXECUTION.
    lines.push(section("EXECUTION"));
    lines.push(field(
        "exit_code",
        r.exit_code
            .map(|c| c.to_string())
            .unwrap_or("-".to_string()),
    ));
    lines.push(field("created_at", r.created_at.clone()));
    lines.push(Line::from(""));

    // Section GIT.
    lines.push(section("GIT"));
    lines.push(field("root", opt(&r.git_root)));
    lines.push(field("branch", opt(&r.git_branch)));
    lines.push(field("remote", opt(&r.git_remote)));
    lines.push(Line::from(""));

    // Section METADATA.
    lines.push(section("METADATA"));
    lines.push(field("id", r.id.to_string()));
    lines.push(field("session", opt(&r.session_id)));

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

/// Titre de section dans le panneau de détails.
fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    ))
}

/// Champ « libellé valeur » avec libellé atténué.
fn field(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<11}"), theme::label()),
        Span::styled(value, theme::value()),
    ])
}

// -- Pied ------------------------------------------------------------------

fn render_footer(f: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    f.render_widget(Paragraph::new(footer_hints(app.mode)), chunks[0]);

    if let Some(msg) = &app.status_message {
        f.render_widget(
            Paragraph::new(msg.clone()).style(Style::default().fg(theme::WARNING)),
            chunks[1],
        );
    }
}

/// Construit la ligne de raccourcis du pied selon le mode.
fn footer_hints(mode: TuiMode) -> Line<'static> {
    let pairs: &[(&str, &str)] = match mode {
        TuiMode::Details => &[
            ("Enter", "sélection"),
            ("/", "recherche"),
            ("j/k", "naviguer"),
            ("F", "filtres"),
            ("y", "copier"),
            ("e", "export"),
            ("x", "suppr"),
            ("?", "aide"),
            ("Esc", "quitter"),
        ],
        _ => &[
            ("Enter", "sélection"),
            ("Tab", "détails"),
            ("Ctrl+P/B/D", "filtrer"),
            ("Ctrl+L", "clear"),
            ("F1", "aide"),
            ("Esc", "quitter"),
        ],
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (key, desc)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme::label()));
        }
        spans.push(Span::styled(format!("[{key}]"), theme::badge(theme::INFO)));
        spans.push(Span::styled(format!(" {desc}"), theme::label()));
    }
    Line::from(spans)
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
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("  {key:<20}"), theme::badge(theme::INFO)),
                    Span::styled(desc, theme::value()),
                ])
            }
        })
        .collect();

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border())
                .title(Span::styled(
                    " Aide - raccourcis (Esc pour fermer) ",
                    theme::title(),
                )),
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
        Line::from(Span::styled("Depuis la sélection :", theme::label())),
        field_help("p", "filtrer par projet"),
        field_help("b", "filtrer par branche"),
        field_help("w", "filtrer par dossier (cwd)"),
        field_help("s", "statut : tous / succès / échecs"),
        field_help("c", "effacer tous les filtres"),
        Line::from(""),
        Line::from(Span::styled("Esc / f : fermer ce panneau", theme::label())),
    ];
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Filtres interactifs ", theme::title())),
    );
    f.render_widget(p, popup);
}

/// Ligne d'aide « touche  description » pour le panneau de filtres.
fn field_help(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key:<3}"), theme::badge(theme::INFO)),
        Span::styled(desc.to_string(), theme::value()),
    ])
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
            Style::default()
                .fg(theme::DANGER)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(cmd, theme::strong())),
        Line::from(""),
        Line::from(Span::styled(
            "Une sauvegarde est créée avant suppression.",
            theme::label(),
        )),
        Line::from(vec![
            Span::styled("[y]", theme::badge(theme::SUCCESS)),
            Span::styled(" confirmer   ", theme::label()),
            Span::styled("[n / Esc]", theme::badge(theme::WARNING)),
            Span::styled(" annuler", theme::label()),
        ]),
    ];
    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::DANGER))
                .title(Span::styled(
                    " Confirmation ",
                    Style::default()
                        .fg(theme::DANGER)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

// -- Utilitaires de layout -------------------------------------------------

/// Rend un bloc bordé titré et renvoie son aire intérieure.
fn block_inner(f: &mut Frame, area: Rect, title: &str) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(title.to_string(), theme::title()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::CommandRecord;
    use crate::tui::app::{StatusFilter, TuiFilters};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn rec(id: i64, command: &str, exit: Option<i64>) -> CommandRecord {
        CommandRecord {
            id,
            command: command.to_string(),
            cwd: Some("/home/killian/mnemo".to_string()),
            shell: Some("bash".to_string()),
            hostname: Some("host".to_string()),
            exit_code: exit,
            created_at: "2026-06-14 17:29:11".to_string(),
            git_root: Some("/home/killian/mnemo".to_string()),
            git_branch: Some("main".to_string()),
            git_remote: None,
            session_id: Some("abcd".to_string()),
        }
    }

    fn sample() -> Vec<CommandRecord> {
        vec![
            rec(1, "cargo build", Some(0)),
            rec(2, "cargo test", Some(1)),
            rec(3, &"git commit -m ".repeat(40), Some(0)),
        ]
    }

    fn app() -> TuiApp {
        let mut a = TuiApp::new(sample(), TuiFilters::default(), String::new());
        a.set_current_context(Some("mnemo".to_string()), Some("main".to_string()));
        a
    }

    /// Rend l'app dans un backend de test de dimensions données : ne doit pas
    /// paniquer et doit produire une frame.
    fn render_at(width: u16, height: u16, app: &mut TuiApp) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
    }

    #[test]
    fn rendu_dimensions_standard_sans_panique() {
        render_at(120, 40, &mut app());
    }

    #[test]
    fn rendu_terminal_etroit_masque_details() {
        // Largeur < DETAILS_MIN_WIDTH : pas de panneau de détails, pas de panique.
        render_at(70, 30, &mut app());
    }

    #[test]
    fn rendu_terminal_court_masque_synthese() {
        // Hauteur < OVERVIEW_MIN_HEIGHT : pas de barre de synthèse.
        render_at(120, 14, &mut app());
    }

    #[test]
    fn rendu_dimensions_minimales_sans_panique() {
        render_at(MIN_WIDTH, MIN_HEIGHT, &mut app());
    }

    #[test]
    fn rendu_trop_petit_affiche_avertissement() {
        render_at(40, 8, &mut app());
    }

    #[test]
    fn rendu_liste_vide_sans_panique() {
        let mut a = TuiApp::new(Vec::new(), TuiFilters::default(), String::new());
        render_at(120, 40, &mut a);
    }

    #[test]
    fn rendu_commande_tres_longue_sans_panique() {
        let mut a = app();
        a.push_query_char('g');
        a.push_query_char('i');
        a.push_query_char('t');
        render_at(100, 30, &mut a);
    }

    #[test]
    fn rendu_tous_les_overlays_sans_panique() {
        for mode in [TuiMode::Help, TuiMode::Filters, TuiMode::ConfirmDelete] {
            let mut a = app();
            a.mode = mode;
            render_at(120, 40, &mut a);
        }
    }

    #[test]
    fn ligne_filtres_aucun() {
        let line = filters_line(&TuiFilters::default());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("[aucun filtre]"));
    }

    #[test]
    fn ligne_filtres_affiche_les_puces_actives() {
        let filters = TuiFilters {
            project: Some("mnemo".to_string()),
            branch: Some("main".to_string()),
            cwd: None,
            status: StatusFilter::Failure,
        };
        let line = filters_line(&filters);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("projet"));
        assert!(text.contains("mnemo"));
        assert!(text.contains("branche"));
        assert!(text.contains("main"));
        assert!(text.contains("statut"));
        assert!(text.contains("échecs"));
    }

    #[test]
    fn badges_contiennent_version_et_contexte() {
        let line = badges_line(&app());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("mnemo"));
        assert!(text.contains(&format!("v{VERSION}")));
        assert!(text.contains("projet"));
        assert!(text.contains("branche"));
        assert!(text.contains("total"));
    }

    #[test]
    fn pied_details_contient_les_actions_cles() {
        let line = footer_hints(TuiMode::Details);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Enter"));
        assert!(text.contains("recherche"));
        assert!(text.contains("export"));
        assert!(text.contains("quitter"));
    }
}
