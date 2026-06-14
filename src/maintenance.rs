//! Nettoyage automatique configurable de l'historique (`mnemo maintenance`).
//!
//! La maintenance est **désactivée par défaut** : `mnemo maintenance run` ne
//! supprime jamais rien tant que `[maintenance] auto_prune_enabled` n'est pas
//! activé. Même activée, toute suppression réelle exige `--yes` (ou une
//! confirmation interactive) et, si `auto_backup_before_prune` est vrai, une
//! sauvegarde complète est créée au préalable.

use anyhow::Result;

use crate::config::{Config, MaintenanceConfig};
use crate::db::SearchFilter;
use crate::{backup, config, confirm, db, list, prune};

/// Nombre d'exemples affichés en aperçu.
const PREVIEW_SAMPLES: usize = 5;

/// Calcule l'horodatage de coupe à partir de la configuration de maintenance.
/// Renvoie `None` si `auto_prune_after` est invalide (signalé à l'appelant).
fn cutoff(cfg: &MaintenanceConfig) -> Option<String> {
    let secs = prune::parse_duration(&cfg.auto_prune_after).ok()?;
    Some(prune::cutoff_timestamp(secs))
}

/// `mnemo maintenance status` : affiche l'état et ce qui serait nettoyé.
pub fn run_status() -> Result<()> {
    let cfg = Config::load()?;
    let m = &cfg.maintenance;

    println!("Maintenance :");
    println!(
        "  Nettoyage automatique : {}",
        if m.auto_prune_enabled {
            "activé"
        } else {
            "désactivé"
        }
    );
    println!("  Ancienneté            : {}", m.auto_prune_after);
    println!(
        "  Sauvegarde avant purge: {}",
        if m.auto_backup_before_prune {
            "oui"
        } else {
            "non"
        }
    );

    match cutoff(m) {
        Some(cutoff) => {
            let conn = db::open(&config::db_path()?)?;
            let n = db::count_older_than(&conn, &cutoff, &SearchFilter::default())?;
            println!("  Coupe                 : {cutoff}");
            println!("  Entrées concernées    : {n}");
        }
        None => {
            println!(
                "  [ERREUR] auto_prune_after invalide : {:?} (ex : 180d, 6m, 1y)",
                m.auto_prune_after
            );
        }
    }
    Ok(())
}

/// `mnemo maintenance run` : exécute le nettoyage configuré.
///
/// - Si la maintenance est désactivée, ne supprime rien (sortie informative).
/// - `dry_run` montre uniquement ce qui serait supprimé.
/// - Sans `--yes`, une confirmation interactive est requise (refus en mode non
///   interactif : compatibilité scripts/CI préservée).
pub fn run(dry_run: bool, assume_yes: bool) -> Result<()> {
    let cfg = Config::load()?;
    let m = &cfg.maintenance;

    if !m.auto_prune_enabled {
        println!(
            "Nettoyage automatique désactivé ([maintenance] auto_prune_enabled = false). \
             Rien à faire."
        );
        return Ok(());
    }

    let cutoff = match cutoff(m) {
        Some(c) => c,
        None => {
            anyhow::bail!(
                "maintenance.auto_prune_after invalide : {:?} (ex : 180d, 6m, 1y)",
                m.auto_prune_after
            );
        }
    };

    let conn = db::open(&config::db_path()?)?;
    let filter = SearchFilter::default();
    let total = db::count_older_than(&conn, &cutoff, &filter)?;

    if total == 0 {
        println!("Aucune commande antérieure à {cutoff}. Rien à nettoyer.");
        return Ok(());
    }

    println!("{total} commande(s) antérieure(s) à {cutoff} seront supprimées.");
    let samples = db::fetch_older_than(&conn, &cutoff, &filter, PREVIEW_SAMPLES)?;
    if !samples.is_empty() {
        println!("Exemples :");
        for r in &samples {
            println!("{}", list::short_line(r));
        }
    }

    if dry_run {
        println!("\n[dry-run] Aucune suppression effectuée.");
        return Ok(());
    }

    if !confirm::confirm(&format!("Supprimer ces {total} commande(s) ?"), assume_yes)? {
        println!("Nettoyage annulé.");
        return Ok(());
    }

    if m.auto_backup_before_prune {
        let safety = backup::create_backup(None)?;
        println!("Sauvegarde automatique : {}", safety.path.display());
    }

    let n = db::delete_older_than(&conn, &cutoff, &filter)?;
    println!("{n} commande(s) supprimée(s).");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutoff_none_si_duree_invalide() {
        let m = MaintenanceConfig {
            auto_prune_enabled: true,
            auto_prune_after: "pas-une-duree".to_string(),
            auto_backup_before_prune: true,
        };
        assert!(cutoff(&m).is_none());
    }

    #[test]
    fn cutoff_some_si_duree_valide() {
        let m = MaintenanceConfig {
            auto_prune_enabled: true,
            auto_prune_after: "180d".to_string(),
            auto_backup_before_prune: true,
        };
        assert!(cutoff(&m).is_some());
    }
}
