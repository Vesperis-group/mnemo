//! Traduction des événements clavier en [`Action`], selon le mode courant.
//!
//! Fonction pure et testable : aucune dépendance au terminal. Deux contextes de
//! saisie coexistent :
//! - **Search** : la frappe édite la requête ; les raccourcis lettre ne sont pas
//!   actifs (sauf via `Ctrl`).
//! - **Details** : focus liste (raccourcis vim `j`/`k` et lettres `d`/`r`/`f`/
//!   `c`/`?`/`q` actifs) ; la frappe ne modifie pas la requête.
//!
//! `Tab` bascule entre les deux. `Esc` quitte partout. `F1` ouvre l'aide partout.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::tui::actions::Action;
use crate::tui::app::TuiMode;

/// Résout une touche en action selon le mode.
pub fn map_key(mode: TuiMode, code: KeyCode, mods: KeyModifiers) -> Action {
    // `Ctrl+C` quitte toujours l'application, quel que soit le mode : c'est la
    // convention terminal attendue. Aucun mode ne l'intercepte pour autre chose.
    if mods.contains(KeyModifiers::CONTROL)
        && matches!(code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        return Action::Quit;
    }

    match mode {
        TuiMode::ConfirmDelete => map_confirm(code),
        TuiMode::Help => map_help(code),
        TuiMode::Filters => map_filters(code),
        TuiMode::Search => map_common(code, mods, true),
        TuiMode::Details => map_common(code, mods, false),
    }
}

fn map_confirm(code: KeyCode) -> Action {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('o') | KeyCode::Char('O') => {
            Action::ConfirmYes
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Action::ConfirmNo,
        _ => Action::None,
    }
}

fn map_help(code: KeyCode) -> Action {
    match code {
        KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::F(1) => {
            Action::ToggleHelp
        }
        _ => Action::None,
    }
}

fn map_filters(code: KeyCode) -> Action {
    match code {
        KeyCode::Esc | KeyCode::Char('f') => Action::ToggleFilters,
        KeyCode::Char('p') => Action::FilterProjectFromSelection,
        KeyCode::Char('b') => Action::FilterBranchFromSelection,
        KeyCode::Char('w') => Action::FilterCwdFromSelection,
        KeyCode::Char('s') => Action::CycleStatusFilter,
        KeyCode::Char('c') => Action::ClearFilters,
        _ => Action::None,
    }
}

fn map_common(code: KeyCode, mods: KeyModifiers, input_focus: bool) -> Action {
    // Raccourcis Ctrl (filtres rapides) prioritaires.
    if mods.contains(KeyModifiers::CONTROL) {
        return match code {
            KeyCode::Char('p') => Action::FilterProjectFromSelection,
            KeyCode::Char('b') => Action::FilterBranchFromSelection,
            KeyCode::Char('d') => Action::FilterCwdFromSelection,
            KeyCode::Char('l') => Action::ClearFilters,
            _ => Action::None,
        };
    }

    match code {
        KeyCode::Esc => Action::Quit,
        KeyCode::Enter => Action::Select,
        KeyCode::Up => Action::Up,
        KeyCode::Down => Action::Down,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        KeyCode::Tab => Action::ToggleDetailsFocus,
        KeyCode::F(1) => Action::ToggleHelp,
        KeyCode::Backspace if input_focus => Action::Backspace,
        KeyCode::Char(c) if input_focus => Action::Input(c),
        // Mode Details : raccourcis lettre (la frappe n'édite pas la requête).
        KeyCode::Char('/') => Action::FocusSearch,
        KeyCode::Char('k') => Action::Up,
        KeyCode::Char('j') => Action::Down,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('d') | KeyCode::Char('x') => Action::RequestDelete,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('f') => Action::CycleStatusFilter,
        KeyCode::Char('F') => Action::ToggleFilters,
        KeyCode::Char('p') => Action::FilterProjectCurrent,
        KeyCode::Char('b') => Action::FilterBranchCurrent,
        KeyCode::Char('c') | KeyCode::Char('y') => Action::Copy,
        KeyCode::Char('e') => Action::ExportResults,
        KeyCode::Char('?') => Action::ToggleHelp,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: KeyModifiers = KeyModifiers::NONE;

    #[test]
    fn search_la_frappe_edite_la_requete() {
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::Char('a'), NONE),
            Action::Input('a')
        );
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::Backspace, NONE),
            Action::Backspace
        );
        // 'j' en mode recherche est du texte, pas une navigation.
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::Char('j'), NONE),
            Action::Input('j')
        );
    }

    #[test]
    fn details_raccourcis_vim() {
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('j'), NONE),
            Action::Down
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('k'), NONE),
            Action::Up
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('d'), NONE),
            Action::RequestDelete
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('x'), NONE),
            Action::RequestDelete
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('q'), NONE),
            Action::Quit
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('c'), NONE),
            Action::Copy
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('y'), NONE),
            Action::Copy
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('e'), NONE),
            Action::ExportResults
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('f'), NONE),
            Action::CycleStatusFilter
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('F'), NONE),
            Action::ToggleFilters
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('p'), NONE),
            Action::FilterProjectCurrent
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('b'), NONE),
            Action::FilterBranchCurrent
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('/'), NONE),
            Action::FocusSearch
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('r'), NONE),
            Action::Refresh
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::Char('?'), NONE),
            Action::ToggleHelp
        );
    }

    #[test]
    fn navigation_commune_aux_deux_modes() {
        for mode in [TuiMode::Search, TuiMode::Details] {
            assert_eq!(map_key(mode, KeyCode::Up, NONE), Action::Up);
            assert_eq!(map_key(mode, KeyCode::Down, NONE), Action::Down);
            assert_eq!(map_key(mode, KeyCode::PageUp, NONE), Action::PageUp);
            assert_eq!(map_key(mode, KeyCode::PageDown, NONE), Action::PageDown);
            assert_eq!(map_key(mode, KeyCode::Home, NONE), Action::Home);
            assert_eq!(map_key(mode, KeyCode::End, NONE), Action::End);
            assert_eq!(map_key(mode, KeyCode::Enter, NONE), Action::Select);
            assert_eq!(map_key(mode, KeyCode::Esc, NONE), Action::Quit);
            assert_eq!(
                map_key(mode, KeyCode::Tab, NONE),
                Action::ToggleDetailsFocus
            );
        }
    }

    #[test]
    fn raccourcis_ctrl_filtres() {
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::Char('p'), KeyModifiers::CONTROL),
            Action::FilterProjectFromSelection
        );
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::Char('b'), KeyModifiers::CONTROL),
            Action::FilterBranchFromSelection
        );
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::Char('d'), KeyModifiers::CONTROL),
            Action::FilterCwdFromSelection
        );
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::Char('l'), KeyModifiers::CONTROL),
            Action::ClearFilters
        );
    }

    #[test]
    fn ctrl_c_quitte_dans_tous_les_modes() {
        for mode in [
            TuiMode::Search,
            TuiMode::Details,
            TuiMode::Help,
            TuiMode::Filters,
            TuiMode::ConfirmDelete,
        ] {
            assert_eq!(
                map_key(mode, KeyCode::Char('c'), KeyModifiers::CONTROL),
                Action::Quit,
                "Ctrl+C doit quitter en mode {mode:?}"
            );
        }
    }

    #[test]
    fn confirmation_suppression() {
        assert_eq!(
            map_key(TuiMode::ConfirmDelete, KeyCode::Char('y'), NONE),
            Action::ConfirmYes
        );
        assert_eq!(
            map_key(TuiMode::ConfirmDelete, KeyCode::Char('n'), NONE),
            Action::ConfirmNo
        );
        assert_eq!(
            map_key(TuiMode::ConfirmDelete, KeyCode::Esc, NONE),
            Action::ConfirmNo
        );
        // En confirmation, les autres touches ne font rien.
        assert_eq!(
            map_key(TuiMode::ConfirmDelete, KeyCode::Char('x'), NONE),
            Action::None
        );
    }

    #[test]
    fn aide_se_ferme() {
        assert_eq!(
            map_key(TuiMode::Help, KeyCode::Esc, NONE),
            Action::ToggleHelp
        );
        assert_eq!(
            map_key(TuiMode::Help, KeyCode::Char('?'), NONE),
            Action::ToggleHelp
        );
    }

    #[test]
    fn f1_ouvre_l_aide_partout() {
        assert_eq!(
            map_key(TuiMode::Search, KeyCode::F(1), NONE),
            Action::ToggleHelp
        );
        assert_eq!(
            map_key(TuiMode::Details, KeyCode::F(1), NONE),
            Action::ToggleHelp
        );
    }
}
