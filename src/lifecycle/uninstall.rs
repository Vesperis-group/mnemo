//! `mnemo uninstall` : retire le binaire et l'intégration shell ; conserve les
//! données par défaut. `--purge` supprime aussi config, base et sauvegardes,
//! après sauvegarde et confirmation explicite.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{backup, config, confirm};

/// Marqueur d'ouverture du bloc d'intégration dans `~/.bashrc`.
pub const BASHRC_BEGIN_MARKER: &str = "# >>> mnemo init >>>";
/// Marqueur de fermeture du bloc d'intégration dans `~/.bashrc`.
pub const BASHRC_END_MARKER: &str = "# <<< mnemo init <<<";

/// Actions décidées par [`plan_uninstall`] (cœur logique pur et testable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UninstallActions {
    /// Supprimer le binaire (s'il est présent).
    pub remove_bin: bool,
    /// Retirer le bloc mnemo du `.bashrc` (s'il est présent).
    pub remove_bashrc_block: bool,
    /// Supprimer le dossier de configuration (uniquement en purge).
    pub remove_config: bool,
    /// Supprimer le dossier de données / sauvegardes (uniquement en purge).
    pub remove_data: bool,
}

/// Décide des actions à mener selon la présence des éléments et le mode `purge`.
///
/// Sans `purge`, les données (config + base + sauvegardes) sont **toujours**
/// conservées.
pub fn plan_uninstall(
    bin_present: bool,
    bashrc_has_block: bool,
    config_present: bool,
    data_present: bool,
    purge: bool,
) -> UninstallActions {
    UninstallActions {
        remove_bin: bin_present,
        remove_bashrc_block: bashrc_has_block,
        remove_config: purge && config_present,
        remove_data: purge && data_present,
    }
}

/// Vrai si le contenu d'un `.bashrc` contient le bloc mnemo.
pub fn bashrc_has_block(content: &str) -> bool {
    content.lines().any(|l| l.trim_end() == BASHRC_BEGIN_MARKER)
}

/// Retire le bloc mnemo (marqueurs inclus) d'un contenu `.bashrc`.
///
/// Fonction pure et **idempotente** : sans bloc, le contenu est inchangé.
pub fn remove_bashrc_block(content: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    let ends_with_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();
    for line in &lines {
        let trimmed = line.trim_end();
        if trimmed == BASHRC_BEGIN_MARKER {
            skipping = true;
            continue;
        }
        if trimmed == BASHRC_END_MARKER {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    // On préserve l'absence de newline final si le fichier d'origine n'en avait
    // pas (et qu'il reste du contenu).
    if !ends_with_newline && out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Chemin du binaire installé (`MNEMO_BIN_PATH` ou `~/.local/bin/mnemo`).
pub fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("MNEMO_BIN_PATH") {
        return PathBuf::from(p);
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".local").join("bin").join("mnemo")
}

/// Chemin du `.bashrc` de l'utilisateur.
fn bashrc_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".bashrc")
}

/// Sauvegarde un fichier vers `<chemin>.mnemo.bak.<horodatage>` (no-op si
/// absent). Renvoie le chemin de la sauvegarde créée.
fn backup_file(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let stamp = crate::db::format_timestamp(secs);
    let digits: String = stamp.chars().filter(|c| c.is_ascii_digit()).collect();
    let (date, time) = digits.split_at(8.min(digits.len()));
    let backup = path.with_file_name(format!(
        "{}.mnemo.bak.{date}-{time}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("bashrc")
    ));
    std::fs::copy(path, &backup)
        .with_context(|| format!("sauvegarde de {} échouée", path.display()))?;
    Ok(Some(backup))
}

/// Point d'entrée de la commande.
pub fn run(dry_run: bool, assume_yes: bool, purge: bool) -> Result<()> {
    let bin = bin_path();
    let bashrc = bashrc_path();
    let config_dir = config::config_dir()?;
    let data_dir = config::data_dir()?;

    let bin_present = bin.exists();
    let bashrc_block = std::fs::read_to_string(&bashrc)
        .map(|c| bashrc_has_block(&c))
        .unwrap_or(false);
    let config_present = config_dir.exists();
    let data_present = data_dir.exists();

    let actions = plan_uninstall(
        bin_present,
        bashrc_block,
        config_present,
        data_present,
        purge,
    );

    // Récapitulatif.
    println!(
        "Plan de désinstallation{} :",
        if dry_run { " (simulation)" } else { "" }
    );
    println!(
        "  binaire        : {} {}",
        bin.display(),
        if actions.remove_bin {
            "→ suppression"
        } else {
            "(absent)"
        }
    );
    println!(
        "  bloc .bashrc   : {} {}",
        bashrc.display(),
        if actions.remove_bashrc_block {
            "→ retrait"
        } else {
            "(absent)"
        }
    );
    if purge {
        println!(
            "  configuration  : {} {}",
            config_dir.display(),
            if actions.remove_config {
                "→ SUPPRESSION"
            } else {
                "(absente)"
            }
        );
        println!(
            "  données        : {} {}",
            data_dir.display(),
            if actions.remove_data {
                "→ SUPPRESSION (base + sauvegardes)"
            } else {
                "(absentes)"
            }
        );
    } else {
        println!("  configuration  : {} (conservée)", config_dir.display());
        println!("  données        : {} (conservées)", data_dir.display());
    }

    if dry_run {
        println!("\nSimulation : aucune modification effectuée.");
        return Ok(());
    }

    // Purge : sauvegarde + confirmation explicite avant toute suppression.
    if purge && (actions.remove_config || actions.remove_data) {
        // Sauvegarde de sécurité, placée HORS du dossier de données pour
        // survivre à la purge.
        if data_present {
            let dest = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            match backup::create_backup(Some(&dest)) {
                Ok(info) => println!("Sauvegarde de sécurité créée : {}", info.path.display()),
                Err(e) => eprintln!("Avertissement : sauvegarde impossible ({e})"),
            }
        }
        let ok = confirm::confirm(
            "Cette action supprimera config, base et sauvegardes. Continuer ?",
            assume_yes,
        )?;
        if !ok {
            println!("Purge annulée. Aucune donnée supprimée.");
            return Ok(());
        }
    }

    // 1. Binaire.
    if actions.remove_bin {
        std::fs::remove_file(&bin)
            .with_context(|| format!("suppression du binaire {} échouée", bin.display()))?;
        println!("Binaire supprimé : {}", bin.display());
    }

    // 2. Bloc .bashrc (avec sauvegarde).
    if actions.remove_bashrc_block {
        let content = std::fs::read_to_string(&bashrc).unwrap_or_default();
        backup_file(&bashrc)?;
        let cleaned = remove_bashrc_block(&content);
        std::fs::write(&bashrc, cleaned)
            .with_context(|| format!("écriture de {} échouée", bashrc.display()))?;
        println!(
            "Bloc mnemo retiré de {} (sauvegarde créée)",
            bashrc.display()
        );
    }

    // 3. Données (purge uniquement).
    if actions.remove_config && config_dir.exists() {
        std::fs::remove_dir_all(&config_dir)
            .with_context(|| format!("suppression de {} échouée", config_dir.display()))?;
        println!("Configuration supprimée : {}", config_dir.display());
    }
    if actions.remove_data && data_dir.exists() {
        std::fs::remove_dir_all(&data_dir)
            .with_context(|| format!("suppression de {} échouée", data_dir.display()))?;
        println!("Données supprimées : {}", data_dir.display());
    }

    if !purge {
        println!("\nDonnées conservées (config, base et sauvegardes intactes).");
        println!("Pour tout supprimer : mnemo uninstall --purge");
    } else {
        println!("\nDésinstallation complète terminée.");
    }
    println!("Pensez à recharger votre shell : source ~/.bashrc");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_sans_purge_conserve_donnees() {
        let a = plan_uninstall(true, true, true, true, false);
        assert!(a.remove_bin);
        assert!(a.remove_bashrc_block);
        assert!(!a.remove_config);
        assert!(!a.remove_data);
    }

    #[test]
    fn plan_purge_supprime_donnees() {
        let a = plan_uninstall(true, true, true, true, true);
        assert!(a.remove_bin);
        assert!(a.remove_bashrc_block);
        assert!(a.remove_config);
        assert!(a.remove_data);
    }

    #[test]
    fn plan_purge_respecte_absence() {
        // Purge demandée mais rien n'est présent : aucune action.
        let a = plan_uninstall(false, false, false, false, true);
        assert!(!a.remove_bin);
        assert!(!a.remove_bashrc_block);
        assert!(!a.remove_config);
        assert!(!a.remove_data);
    }

    #[test]
    fn retrait_bloc_bashrc() {
        let content = "\
export PATH=$HOME/bin:$PATH
# >>> mnemo init >>>
source /tmp/mnemo.sh
# <<< mnemo init <<<
alias ll='ls -la'
";
        let cleaned = remove_bashrc_block(content);
        assert!(!cleaned.contains("mnemo"));
        assert!(cleaned.contains("export PATH"));
        assert!(cleaned.contains("alias ll"));
    }

    #[test]
    fn retrait_bloc_idempotent() {
        let content = "alias g='git'\nexport EDITOR=vim\n";
        // Aucun bloc : contenu inchangé.
        assert_eq!(remove_bashrc_block(content), content);
        // Double application : stable.
        let once = remove_bashrc_block(content);
        assert_eq!(remove_bashrc_block(&once), once);
    }

    #[test]
    fn detection_bloc() {
        assert!(bashrc_has_block("a\n# >>> mnemo init >>>\nb\n"));
        assert!(!bashrc_has_block("a\nb\n"));
    }
}
