//! Actions de la TUI et accès à la base isolé derrière un trait.
//!
//! [`Action`] décrit toutes les intentions issues du clavier (le mapping touche
//! -> action vit dans [`crate::tui::events`]). [`TuiBackend`] isole les effets
//! de bord (suppression avec sauvegarde, rechargement) afin que la logique de
//! [`crate::tui::app::TuiApp`] reste testable sans base réelle.

use anyhow::{bail, Result};

use crate::db::{self, CommandRecord, SearchFilter};
use crate::{backup, config};

/// Intention utilisateur résolue à partir d'un événement clavier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Quitter la TUI sans sélectionner.
    Quit,
    /// Sélectionner la commande courante puis quitter (l'imprime).
    Select,
    /// Élément précédent.
    Up,
    /// Élément suivant.
    Down,
    /// Page précédente.
    PageUp,
    /// Page suivante.
    PageDown,
    /// Premier élément.
    Home,
    /// Dernier élément.
    End,
    /// Effacer le dernier caractère de la requête.
    Backspace,
    /// Ajouter un caractère à la requête.
    Input(char),
    /// Ouvrir/fermer l'aide.
    ToggleHelp,
    /// Ouvrir/fermer le panneau de filtres.
    ToggleFilters,
    /// Basculer le focus liste/détails.
    ToggleDetailsFocus,
    /// Donner le focus à la recherche (saisie).
    FocusSearch,
    /// Recharger les résultats depuis la base.
    Refresh,
    /// Copier/émettre la commande sélectionnée.
    Copy,
    /// Exporter les résultats filtrés vers un fichier JSON.
    ExportResults,
    /// Demander la suppression de la commande sélectionnée.
    RequestDelete,
    /// Confirmer la suppression.
    ConfirmYes,
    /// Annuler la suppression.
    ConfirmNo,
    /// Filtrer par le projet de la sélection.
    FilterProjectFromSelection,
    /// Filtrer par la branche de la sélection.
    FilterBranchFromSelection,
    /// Filtrer par le répertoire de la sélection.
    FilterCwdFromSelection,
    /// Filtrer par le projet courant (dossier de lancement).
    FilterProjectCurrent,
    /// Filtrer par la branche courante (dossier de lancement).
    FilterBranchCurrent,
    /// Faire défiler le filtre de statut (tous/succès/échecs).
    CycleStatusFilter,
    /// Vider tous les filtres.
    ClearFilters,
    /// Aucune action.
    None,
}

/// Effets de bord de la TUI sur la base de données.
pub trait TuiBackend {
    /// Crée une sauvegarde puis supprime la commande `id`.
    ///
    /// **Contrat de sécurité** : si la sauvegarde échoue, la suppression ne doit
    /// pas avoir lieu et la méthode renvoie `Err`.
    fn backup_and_delete(&mut self, id: i64) -> Result<()>;

    /// Recharge l'ensemble des commandes (jusqu'à la limite de configuration).
    fn reload(&mut self) -> Result<Vec<CommandRecord>>;
}

/// Backend réel : ouvre la base locale et applique les opérations.
pub struct DbBackend {
    conn: rusqlite::Connection,
    limit: usize,
}

impl DbBackend {
    /// Ouvre la base locale et prépare le backend.
    pub fn open(limit: usize) -> Result<Self> {
        let conn = db::open(&config::db_path()?)?;
        Ok(Self { conn, limit })
    }
}

impl TuiBackend for DbBackend {
    fn backup_and_delete(&mut self, id: i64) -> Result<()> {
        // Sauvegarde AVANT toute modification : en cas d'échec, on n'altère rien.
        let info = backup::create_backup(None)?;
        let _ = info;
        let n = db::delete_command(&self.conn, id)?;
        if n == 0 {
            bail!("commande #{id} introuvable");
        }
        Ok(())
    }

    fn reload(&mut self) -> Result<Vec<CommandRecord>> {
        db::fetch_filtered(&self.conn, &SearchFilter::default(), self.limit)
    }
}
