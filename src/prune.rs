//! Suppression sûre de commandes (`mnemo delete`) et nettoyage par ancienneté
//! (`mnemo prune`).
//!
//! Sécurité (v0.3) :
//! - on affiche toujours ce qui sera touché avant d'agir ;
//! - `--dry-run` ne supprime rien ;
//! - sans `--yes`, une confirmation interactive est demandée (refus en mode
//!   non interactif) ;
//! - une sauvegarde automatique est créée avant toute suppression réelle ;
//! - les suppressions s'exécutent dans une transaction SQLite.

use anyhow::{bail, Context, Result};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::SearchFilter;
use crate::{backup, config, confirm, db, list};

/// Nombre d'exemples affichés en aperçu d'un `prune`.
const PREVIEW_SAMPLES: usize = 5;

/// Convertit une durée lisible (`24h`, `30d`, `12w`, `6m`, `1y`) en secondes.
///
/// Approximations documentées : `w` = 7 jours, `m` = 30 jours, `y` = 365 jours.
pub fn parse_duration(spec: &str) -> Result<u64> {
    const HOUR: u64 = 3_600;
    const DAY: u64 = 86_400;
    let spec = spec.trim();

    // L'unité est le dernier caractère ; on découpe sur une frontière de
    // caractère UTF-8 pour ne jamais paniquer sur une entrée multi-octets.
    let unit = spec
        .chars()
        .next_back()
        .with_context(|| format!("durée invalide : {spec:?} (exemples : 24h, 30d, 12w, 6m, 1y)"))?;
    let num = &spec[..spec.len() - unit.len_utf8()];
    if num.is_empty() {
        bail!("durée invalide : {spec:?} (exemples : 24h, 30d, 12w, 6m, 1y)");
    }
    let n: u64 = num
        .parse()
        .with_context(|| format!("durée invalide : {spec:?} (exemples : 24h, 30d, 12w, 6m, 1y)"))?;
    if n == 0 {
        bail!("durée invalide : {spec:?} (doit être strictement positive)");
    }
    let per_unit: u64 = match unit {
        'h' => HOUR,
        'd' => DAY,
        'w' => 7 * DAY,
        'm' => 30 * DAY,
        'y' => 365 * DAY,
        other => bail!("unité de durée inconnue : {other:?} (utilisez h, d, w, m ou y)"),
    };
    // `checked_mul` évite tout débordement sur des valeurs déraisonnables.
    let secs = n
        .checked_mul(per_unit)
        .with_context(|| format!("durée trop grande : {spec:?}"))?;
    Ok(secs)
}

/// Horodatage `YYYY-MM-DD HH:MM:SS` correspondant à « il y a `secs` secondes ».
pub(crate) fn cutoff_timestamp(secs: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    db::format_timestamp(now.saturating_sub(secs))
}

/// Point d'entrée de `mnemo delete <id>`.
pub fn delete_run(id: i64, dry_run: bool, assume_yes: bool) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;

    let record = match db::get_command(&conn, id)? {
        Some(r) => r,
        None => {
            println!("Aucune commande avec l'ID {id}.");
            return Ok(());
        }
    };

    println!("Commande ciblée :");
    println!("{}", list::short_line(&record));

    if dry_run {
        println!("\n[dry-run] Aucune suppression effectuée.");
        return Ok(());
    }

    if !confirm::confirm(
        &format!("Supprimer définitivement la commande {id} ?"),
        assume_yes,
    )? {
        println!("Suppression annulée.");
        return Ok(());
    }

    let safety = backup::create_backup(None)?;
    println!("Sauvegarde automatique : {}", safety.path.display());

    let n = db::delete_command(&conn, id)?;
    println!("{n} commande supprimée.");
    Ok(())
}

/// Point d'entrée de `mnemo prune --older-than <durée>`.
pub fn prune_run(
    older_than: String,
    project: Option<String>,
    branch: Option<String>,
    dry_run: bool,
    assume_yes: bool,
) -> Result<()> {
    let secs = parse_duration(&older_than)?;
    let cutoff = cutoff_timestamp(secs);

    let conn = db::open(&config::db_path()?)?;
    let filter = SearchFilter { project, branch };

    let total = db::count_older_than(&conn, &cutoff, &filter)?;
    if total == 0 {
        println!("Aucune commande antérieure à {cutoff} (--older-than {older_than}).");
        return Ok(());
    }

    println!(
        "{total} commande(s) antérieure(s) à {cutoff} (--older-than {older_than}) seront supprimées."
    );
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

    let safety = backup::create_backup(None)?;
    println!("Sauvegarde automatique : {}", safety.path.display());

    let n = db::delete_older_than(&conn, &cutoff, &filter)?;
    println!("{n} commande(s) supprimée(s).");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_valide() {
        const DAY: u64 = 86_400;
        assert_eq!(parse_duration("24h").unwrap(), 24 * 3_600);
        assert_eq!(parse_duration("30d").unwrap(), 30 * DAY);
        assert_eq!(parse_duration("12w").unwrap(), 12 * 7 * DAY);
        assert_eq!(parse_duration("6m").unwrap(), 6 * 30 * DAY);
        assert_eq!(parse_duration("1y").unwrap(), 365 * DAY);
    }

    #[test]
    fn parse_duration_erreurs() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("d").is_err());
        assert!(parse_duration("30").is_err());
        assert!(parse_duration("30x").is_err());
        assert!(parse_duration("0d").is_err());
        assert!(parse_duration("abc").is_err());
    }

    #[test]
    fn parse_duration_multibyte_ne_panique_pas() {
        // Régression (fuzzing) : une entrée multi-octets ne doit jamais paniquer
        // en découpant l'unité au milieu d'un caractère UTF-8.
        assert!(parse_duration("Ώ").is_err());
        assert!(parse_duration("1Ώ").is_err());
        assert!(parse_duration("é").is_err());
        assert!(parse_duration("24é").is_err());
    }

    #[test]
    fn parse_duration_overflow_est_une_erreur() {
        // Régression (fuzzing) : une durée démesurée renvoie une erreur propre,
        // sans débordement arithmétique.
        assert!(parse_duration("99999999999999999999y").is_err());
        assert!(parse_duration(&format!("{}y", u64::MAX)).is_err());
    }
}
