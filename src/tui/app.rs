//! Modèle interne de la TUI et logique métier testable (sans terminal).
//!
//! Tout l'état interactif vit dans [`TuiApp`]. Les méthodes de navigation et de
//! filtrage sont pures (aucune dépendance au terminal ni à la base) afin d'être
//! testées unitairement. Les actions touchant la base passent par le trait
//! [`TuiBackend`], ce qui permet de les simuler dans les tests.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as NucleoConfig, Matcher, Utf32Str};

use crate::db::CommandRecord;
use crate::tui::actions::{Action, TuiBackend};

/// Taille de page par défaut pour PageUp / PageDown (ajustée au rendu selon la
/// hauteur réelle de la liste).
pub const DEFAULT_PAGE_SIZE: usize = 10;

/// Filtre sur le code de sortie des commandes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    /// Toutes les commandes.
    #[default]
    All,
    /// Uniquement les succès (`exit_code == 0`).
    Success,
    /// Uniquement les échecs (`exit_code` présent et ≠ 0).
    Failure,
}

impl StatusFilter {
    /// Fait défiler All -> Success -> Failure -> All.
    pub fn next(self) -> Self {
        match self {
            StatusFilter::All => StatusFilter::Success,
            StatusFilter::Success => StatusFilter::Failure,
            StatusFilter::Failure => StatusFilter::All,
        }
    }

    /// Libellé court pour l'affichage.
    pub fn label(self) -> &'static str {
        match self {
            StatusFilter::All => "tous",
            StatusFilter::Success => "succès",
            StatusFilter::Failure => "échecs",
        }
    }
}

/// Filtres interactifs appliqués en plus de la recherche fuzzy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TuiFilters {
    pub project: Option<String>,
    pub branch: Option<String>,
    pub cwd: Option<String>,
    pub status: StatusFilter,
}

impl TuiFilters {
    /// Vrai si aucun filtre n'est actif.
    pub fn is_empty(&self) -> bool {
        self.project.is_none()
            && self.branch.is_none()
            && self.cwd.is_none()
            && self.status == StatusFilter::All
    }
}

/// Mode courant de l'interface (détermine le rendu et l'interprétation des
/// touches).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiMode {
    /// Saisie de recherche (la frappe édite la requête).
    Search,
    /// Aide plein écran.
    Help,
    /// Confirmation de suppression.
    ConfirmDelete,
    /// Panneau de filtres.
    Filters,
    /// Focus sur la liste/les détails (raccourcis vim, la frappe ne modifie pas
    /// la requête).
    Details,
}

/// État complet de la TUI.
pub struct TuiApp {
    /// Requête de recherche courante.
    pub query: String,
    /// Toutes les commandes chargées en mémoire.
    pub records: Vec<CommandRecord>,
    /// Indices (dans `records`) retenus après filtres + recherche.
    pub filtered: Vec<usize>,
    /// Position sélectionnée dans `filtered`.
    pub selected: usize,
    /// Filtres interactifs actifs.
    pub filters: TuiFilters,
    /// Mode courant.
    pub mode: TuiMode,
    /// Message de statut transitoire affiché en pied de page.
    pub status_message: Option<String>,
    /// Commande retenue par l'utilisateur (imprimée à la sortie).
    pub outcome: Option<String>,
    /// Demande de sortie de la boucle d'événements.
    pub should_quit: bool,
    /// Dernière commande mise dans le tampon interne (raccourci `c`).
    pub copy_buffer: Option<String>,
    /// Taille de page courante (mise à jour au rendu).
    pub page_size: usize,
    matcher: Matcher,
}

impl TuiApp {
    /// Construit l'application à partir des commandes chargées, de filtres
    /// initiaux et d'une requête initiale.
    pub fn new(records: Vec<CommandRecord>, filters: TuiFilters, query: String) -> Self {
        let mut app = Self {
            query,
            records,
            filtered: Vec::new(),
            selected: 0,
            filters,
            mode: TuiMode::Search,
            status_message: None,
            outcome: None,
            should_quit: false,
            copy_buffer: None,
            page_size: DEFAULT_PAGE_SIZE,
            matcher: Matcher::new(NucleoConfig::DEFAULT),
        };
        app.recompute();
        app
    }

    // -- Filtrage et recherche ---------------------------------------------

    /// Vrai si `r` satisfait les filtres structurés actifs.
    fn matches_filters(&self, r: &CommandRecord) -> bool {
        if let Some(project) = &self.filters.project {
            let ok = match &r.git_root {
                Some(root) => root == project || last_segment(root) == project.as_str(),
                None => false,
            };
            if !ok {
                return false;
            }
        }
        if let Some(branch) = &self.filters.branch {
            if r.git_branch.as_deref() != Some(branch.as_str()) {
                return false;
            }
        }
        if let Some(cwd) = &self.filters.cwd {
            if r.cwd.as_deref() != Some(cwd.as_str()) {
                return false;
            }
        }
        match self.filters.status {
            StatusFilter::All => {}
            StatusFilter::Success => {
                if r.exit_code != Some(0) {
                    return false;
                }
            }
            StatusFilter::Failure => match r.exit_code {
                Some(code) if code != 0 => {}
                _ => return false,
            },
        }
        true
    }

    /// Recalcule la liste filtrée (filtres structurés puis recherche fuzzy) et
    /// borne la sélection pour qu'elle reste valide.
    pub fn recompute(&mut self) {
        let candidates: Vec<usize> = (0..self.records.len())
            .filter(|&i| self.matches_filters(&self.records[i]))
            .collect();

        self.filtered = if self.query.trim().is_empty() {
            candidates
        } else {
            let pattern = Pattern::parse(
                self.query.trim(),
                CaseMatching::Ignore,
                Normalization::Smart,
            );
            let mut buf: Vec<char> = Vec::new();
            let mut scored: Vec<(usize, u32)> = candidates
                .into_iter()
                .filter_map(|i| {
                    let haystack = Utf32Str::new(&self.records[i].command, &mut buf);
                    pattern.score(haystack, &mut self.matcher).map(|s| (i, s))
                })
                .collect();
            scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
            scored.into_iter().map(|(i, _)| i).collect()
        };

        self.clamp_selection();
    }

    /// Garde `selected` dans les bornes de `filtered`.
    fn clamp_selection(&mut self) {
        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    // -- Navigation (bornée) -----------------------------------------------

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() && self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn page_down(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = (self.selected + self.page_size).min(self.filtered.len() - 1);
    }

    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.page_size);
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }

    // -- Sélection ---------------------------------------------------------

    /// Référence vers la commande sélectionnée, le cas échéant.
    pub fn selected_record(&self) -> Option<&CommandRecord> {
        self.filtered
            .get(self.selected)
            .map(|&idx| &self.records[idx])
    }

    /// Identifiant de la commande sélectionnée.
    pub fn selected_id(&self) -> Option<i64> {
        self.selected_record().map(|r| r.id)
    }

    // -- Édition de la requête ---------------------------------------------

    pub fn push_query_char(&mut self, c: char) {
        self.query.push(c);
        self.recompute();
    }

    pub fn pop_query_char(&mut self) {
        self.query.pop();
        self.recompute();
    }

    // -- Bascules de mode --------------------------------------------------

    pub fn toggle_help(&mut self) {
        self.mode = if self.mode == TuiMode::Help {
            TuiMode::Search
        } else {
            TuiMode::Help
        };
    }

    pub fn toggle_filters(&mut self) {
        self.mode = if self.mode == TuiMode::Filters {
            TuiMode::Search
        } else {
            TuiMode::Filters
        };
    }

    pub fn toggle_details_focus(&mut self) {
        self.mode = if self.mode == TuiMode::Details {
            TuiMode::Search
        } else {
            TuiMode::Details
        };
    }

    // -- Filtres -----------------------------------------------------------

    pub fn filter_project_from_selection(&mut self) {
        match self.selected_record().and_then(|r| r.git_root.clone()) {
            Some(root) => {
                let name = last_segment(&root).to_string();
                self.filters.project = Some(name.clone());
                self.status_message = Some(format!("Filtre projet = {name}"));
            }
            None => self.status_message = Some("Sélection sans projet Git.".to_string()),
        }
        self.recompute();
    }

    pub fn filter_branch_from_selection(&mut self) {
        match self.selected_record().and_then(|r| r.git_branch.clone()) {
            Some(branch) => {
                self.filters.branch = Some(branch.clone());
                self.status_message = Some(format!("Filtre branche = {branch}"));
            }
            None => self.status_message = Some("Sélection sans branche Git.".to_string()),
        }
        self.recompute();
    }

    pub fn filter_cwd_from_selection(&mut self) {
        match self.selected_record().and_then(|r| r.cwd.clone()) {
            Some(cwd) => {
                self.filters.cwd = Some(cwd.clone());
                self.status_message = Some(format!("Filtre dossier = {cwd}"));
            }
            None => self.status_message = Some("Sélection sans répertoire.".to_string()),
        }
        self.recompute();
    }

    pub fn cycle_status_filter(&mut self) {
        self.filters.status = self.filters.status.next();
        self.status_message = Some(format!("Statut = {}", self.filters.status.label()));
        self.recompute();
    }

    pub fn clear_filters(&mut self) {
        self.filters = TuiFilters::default();
        self.status_message = Some("Filtres réinitialisés.".to_string());
        self.recompute();
    }

    // -- Suppression -------------------------------------------------------

    /// Passe en mode confirmation si une commande est sélectionnée.
    pub fn request_delete(&mut self) {
        if self.selected_id().is_some() {
            self.mode = TuiMode::ConfirmDelete;
        } else {
            self.status_message = Some("Aucune commande à supprimer.".to_string());
        }
    }

    /// Annule la confirmation de suppression.
    pub fn cancel_delete(&mut self) {
        self.mode = TuiMode::Search;
        self.status_message = Some("Suppression annulée.".to_string());
    }

    /// Confirme la suppression : crée un backup puis supprime via le backend.
    /// En cas d'échec du backend (ex. backup impossible), rien n'est retiré.
    pub fn confirm_delete<B: TuiBackend>(&mut self, backend: &mut B) {
        let id = match self.selected_id() {
            Some(id) => id,
            None => {
                self.mode = TuiMode::Search;
                return;
            }
        };
        match backend.backup_and_delete(id) {
            Ok(()) => {
                self.records.retain(|r| r.id != id);
                self.recompute();
                self.status_message = Some(format!("Commande #{id} supprimée (sauvegarde créée)."));
            }
            Err(e) => {
                self.status_message = Some(format!(
                    "Suppression impossible : {e}. Aucune donnée modifiée."
                ));
            }
        }
        self.mode = TuiMode::Search;
    }

    // -- Copie -------------------------------------------------------------

    /// Met la commande sélectionnée dans le tampon interne et tente une copie
    /// vers le presse-papiers système (sans dépendance obligatoire).
    pub fn copy_selection(&mut self) {
        let cmd = match self.selected_record() {
            Some(r) => r.command.clone(),
            None => {
                self.status_message = Some("Aucune commande à copier.".to_string());
                return;
            }
        };
        self.copy_buffer = Some(cmd.clone());
        match crate::tui::clipboard::copy_to_clipboard(&cmd) {
            Ok(true) => {
                self.status_message = Some("Commande copiée dans le presse-papiers.".to_string());
            }
            _ => {
                self.status_message = Some(
                    "Presse-papiers indisponible ; utilisez Entrée pour imprimer la commande."
                        .to_string(),
                );
            }
        }
    }

    // -- Rafraîchissement et sélection -------------------------------------

    /// Recharge les commandes depuis le backend (raccourci `r`).
    pub fn refresh<B: TuiBackend>(&mut self, backend: &mut B) {
        match backend.reload() {
            Ok(records) => {
                self.records = records;
                self.recompute();
                self.status_message = Some("Résultats rafraîchis.".to_string());
            }
            Err(e) => {
                self.status_message = Some(format!("Rafraîchissement impossible : {e}"));
            }
        }
    }

    /// Valide la sélection : retient la commande et demande la sortie.
    pub fn select_and_quit(&mut self) {
        if let Some(r) = self.selected_record() {
            self.outcome = Some(r.command.clone());
        }
        self.should_quit = true;
    }

    // -- Dispatch ----------------------------------------------------------

    /// Applique une action issue du clavier.
    pub fn dispatch<B: TuiBackend>(&mut self, action: Action, backend: &mut B) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Select => self.select_and_quit(),
            Action::Up => self.select_previous(),
            Action::Down => self.select_next(),
            Action::PageUp => self.page_up(),
            Action::PageDown => self.page_down(),
            Action::Home => self.select_first(),
            Action::End => self.select_last(),
            Action::Backspace => self.pop_query_char(),
            Action::Input(c) => self.push_query_char(c),
            Action::ToggleHelp => self.toggle_help(),
            Action::ToggleFilters => self.toggle_filters(),
            Action::ToggleDetailsFocus => self.toggle_details_focus(),
            Action::Refresh => self.refresh(backend),
            Action::Copy => self.copy_selection(),
            Action::RequestDelete => self.request_delete(),
            Action::ConfirmYes => self.confirm_delete(backend),
            Action::ConfirmNo => self.cancel_delete(),
            Action::FilterProjectFromSelection => self.filter_project_from_selection(),
            Action::FilterBranchFromSelection => self.filter_branch_from_selection(),
            Action::FilterCwdFromSelection => self.filter_cwd_from_selection(),
            Action::CycleStatusFilter => self.cycle_status_filter(),
            Action::ClearFilters => self.clear_filters(),
            Action::None => {}
        }
    }
}

/// Dernier segment d'un chemin (`/a/b/c` -> `c`).
pub fn last_segment(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::CommandRecord;
    use crate::tui::actions::TuiBackend;
    use anyhow::Result;

    /// Backend factice : enregistre les suppressions et peut simuler un échec.
    struct FakeBackend {
        fail: bool,
        deleted: Vec<i64>,
        reload_with: Option<Vec<CommandRecord>>,
    }

    impl FakeBackend {
        fn ok() -> Self {
            Self {
                fail: false,
                deleted: Vec::new(),
                reload_with: None,
            }
        }
        fn failing() -> Self {
            Self {
                fail: true,
                deleted: Vec::new(),
                reload_with: None,
            }
        }
    }

    impl TuiBackend for FakeBackend {
        fn backup_and_delete(&mut self, id: i64) -> Result<()> {
            if self.fail {
                anyhow::bail!("backup simulé en échec");
            }
            self.deleted.push(id);
            Ok(())
        }
        fn reload(&mut self) -> Result<Vec<CommandRecord>> {
            Ok(self.reload_with.clone().unwrap_or_default())
        }
    }

    fn rec(id: i64, command: &str) -> CommandRecord {
        CommandRecord {
            id,
            command: command.to_string(),
            cwd: Some("/home/user".to_string()),
            shell: Some("bash".to_string()),
            hostname: Some("host".to_string()),
            exit_code: Some(0),
            created_at: "2026-06-14 10:00:00".to_string(),
            git_root: None,
            git_branch: None,
            git_remote: None,
            session_id: None,
        }
    }

    fn with_git(mut r: CommandRecord, root: &str, branch: &str) -> CommandRecord {
        r.git_root = Some(root.to_string());
        r.git_branch = Some(branch.to_string());
        r
    }

    fn sample() -> Vec<CommandRecord> {
        vec![
            with_git(rec(1, "cargo build"), "/home/user/mnemo", "main"),
            with_git(rec(2, "cargo test"), "/home/user/mnemo", "dev"),
            with_git(rec(3, "git status"), "/home/user/other", "main"),
            {
                let mut r = rec(4, "ls -la");
                r.cwd = Some("/tmp".to_string());
                r.exit_code = Some(1);
                r
            },
            {
                let mut r = rec(5, "false");
                r.exit_code = Some(2);
                r
            },
        ]
    }

    fn app() -> TuiApp {
        TuiApp::new(sample(), TuiFilters::default(), String::new())
    }

    #[test]
    fn navigation_haut_bas_bornee() {
        let mut a = app();
        assert_eq!(a.selected, 0);
        a.select_previous();
        assert_eq!(a.selected, 0, "ne descend pas sous 0");
        a.select_next();
        assert_eq!(a.selected, 1);
        // Va jusqu'au bout puis bute.
        for _ in 0..20 {
            a.select_next();
        }
        assert_eq!(a.selected, a.filtered.len() - 1);
    }

    #[test]
    fn page_up_down_bornees() {
        let mut a = app();
        a.page_size = 2;
        a.page_down();
        assert_eq!(a.selected, 2);
        a.page_down();
        assert_eq!(a.selected, 4);
        a.page_down();
        assert_eq!(a.selected, a.filtered.len() - 1);
        a.page_up();
        assert_eq!(a.selected, 2);
        a.page_up();
        a.page_up();
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn home_end() {
        let mut a = app();
        a.select_last();
        assert_eq!(a.selected, a.filtered.len() - 1);
        a.select_first();
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn filtrage_par_projet() {
        let mut a = app();
        a.filters.project = Some("mnemo".to_string());
        a.recompute();
        assert_eq!(a.filtered.len(), 2);
        assert!(a
            .filtered
            .iter()
            .all(|&i| a.records[i].git_root.as_deref() == Some("/home/user/mnemo")));
    }

    #[test]
    fn filtrage_par_branche() {
        let mut a = app();
        a.filters.branch = Some("main".to_string());
        a.recompute();
        assert_eq!(a.filtered.len(), 2);
        assert!(a
            .filtered
            .iter()
            .all(|&i| a.records[i].git_branch.as_deref() == Some("main")));
    }

    #[test]
    fn filtrage_par_cwd() {
        let mut a = app();
        a.filters.cwd = Some("/tmp".to_string());
        a.recompute();
        assert_eq!(a.filtered.len(), 1);
        assert_eq!(a.records[a.filtered[0]].command, "ls -la");
    }

    #[test]
    fn filtrage_failed_et_success() {
        let mut a = app();
        a.filters.status = StatusFilter::Failure;
        a.recompute();
        assert_eq!(a.filtered.len(), 2, "ls -la (1) et false (2)");
        a.filters.status = StatusFilter::Success;
        a.recompute();
        assert_eq!(a.filtered.len(), 3);
    }

    #[test]
    fn clear_filters_reinitialise() {
        let mut a = app();
        a.filters.project = Some("mnemo".to_string());
        a.filters.status = StatusFilter::Failure;
        a.recompute();
        a.clear_filters();
        assert!(a.filters.is_empty());
        assert_eq!(a.filtered.len(), a.records.len());
    }

    #[test]
    fn recherche_conserve_une_selection_valide() {
        let mut a = app();
        a.select_last();
        let before = a.selected;
        assert!(before > 0);
        a.push_query_char('c');
        a.push_query_char('a');
        // "ca" matche cargo build/test : la sélection reste dans les bornes.
        assert!(a.selected < a.filtered.len());
        assert!(!a.filtered.is_empty());
    }

    #[test]
    fn suppression_confirmee_retire_l_element() {
        let mut a = app();
        let mut backend = FakeBackend::ok();
        a.select_first();
        let id = a.selected_id().unwrap();
        let before = a.records.len();
        a.request_delete();
        assert_eq!(a.mode, TuiMode::ConfirmDelete);
        a.confirm_delete(&mut backend);
        assert_eq!(a.records.len(), before - 1);
        assert!(a.records.iter().all(|r| r.id != id));
        assert_eq!(backend.deleted, vec![id]);
        assert_eq!(a.mode, TuiMode::Search);
    }

    #[test]
    fn suppression_annulee_ne_modifie_rien() {
        let mut a = app();
        let before = a.records.len();
        a.request_delete();
        a.cancel_delete();
        assert_eq!(a.records.len(), before);
        assert_eq!(a.mode, TuiMode::Search);
    }

    #[test]
    fn suppression_echoue_ne_retire_rien() {
        let mut a = app();
        let mut backend = FakeBackend::failing();
        let before = a.records.len();
        a.request_delete();
        a.confirm_delete(&mut backend);
        assert_eq!(
            a.records.len(),
            before,
            "rien n'est supprimé si le backup échoue"
        );
        assert!(a.status_message.as_deref().unwrap().contains("impossible"));
        assert_eq!(a.mode, TuiMode::Search);
    }

    #[test]
    fn etat_vide_ne_panique_pas() {
        let mut a = TuiApp::new(Vec::new(), TuiFilters::default(), String::new());
        let mut backend = FakeBackend::ok();
        a.select_next();
        a.select_previous();
        a.page_down();
        a.page_up();
        a.select_first();
        a.select_last();
        a.request_delete();
        assert_eq!(a.mode, TuiMode::Search);
        a.confirm_delete(&mut backend);
        a.copy_selection();
        a.select_and_quit();
        assert!(a.selected_record().is_none());
        assert!(a.outcome.is_none());
    }

    #[test]
    fn refresh_recharge_les_records() {
        let mut a = app();
        let mut backend = FakeBackend::ok();
        backend.reload_with = Some(vec![rec(99, "echo rafraichi")]);
        a.refresh(&mut backend);
        assert_eq!(a.records.len(), 1);
        assert_eq!(a.records[0].id, 99);
    }

    #[test]
    fn filtre_projet_depuis_selection() {
        let mut a = app();
        a.select_first();
        a.filter_project_from_selection();
        assert_eq!(a.filters.project.as_deref(), Some("mnemo"));
    }

    #[test]
    fn select_and_quit_imprime_la_commande() {
        let mut a = app();
        a.select_first();
        a.select_and_quit();
        assert!(a.should_quit);
        assert_eq!(a.outcome.as_deref(), Some("cargo build"));
    }
}
