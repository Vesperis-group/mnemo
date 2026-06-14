//! `mnemo update` : vérifie la disponibilité d'une nouvelle version. Par
//! défaut, n'installe rien ; en terminal interactif, propose d'enchaîner
//! `mnemo upgrade`, et l'option `--upgrade` lance directement l'installation.

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use serde::Serialize;

use super::github::fetch_latest_release;
use super::{current_version, normalize_tag, target_triple, update_available};

/// Rapport de vérification de mise à jour (sérialisable en JSON).
#[derive(Debug, Clone, Serialize)]
pub struct UpdateReport {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub asset_target: String,
}

impl UpdateReport {
    /// Construit le rapport à partir des versions courante et distante.
    pub fn new(current: &str, latest: &str) -> Self {
        Self {
            current_version: normalize_tag(current),
            latest_version: normalize_tag(latest),
            update_available: update_available(current, latest),
            asset_target: target_triple().to_string(),
        }
    }
}

/// Interprète une réponse interactive : `o`/`oui`/`y`/`yes` (insensible à la
/// casse) valent « oui » ; tout le reste, y compris une ligne vide (`Entrée`),
/// vaut « non ». Fonction pure, testable sans terminal.
fn interpret_yes(answer: &str) -> bool {
    let a = answer.trim().to_lowercase();
    a == "o" || a == "oui" || a == "y" || a == "yes"
}

/// Demande, en terminal interactif uniquement, s'il faut installer la mise à
/// jour maintenant. Renvoie `false` sans rien afficher dès que stdin **ou**
/// stdout n'est pas un terminal (CI, script, cron, pipe) : `update` reste alors
/// une simple vérification. Réponse par défaut (`Entrée`) : non.
fn prompt_install_now() -> Result<bool> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(false);
    }
    print!("Installer maintenant avec `mnemo upgrade` ? [o/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(interpret_yes(&answer))
}

/// Point d'entrée de la commande.
///
/// - `json` : sortie machine, vérification seule (aucune proposition d'upgrade).
/// - `upgrade` : si une mise à jour est disponible, enchaîne `mnemo upgrade`.
/// - `assume_yes` : avec `upgrade`, installe sans confirmation (automatisation).
pub fn run(json: bool, upgrade: bool, assume_yes: bool) -> Result<()> {
    let current = current_version();
    let latest = fetch_latest_release()?.tag_name;
    let report = UpdateReport::new(&current, &latest);

    if json {
        // Mode machine : on reste strictement en vérification, sans prompt ni
        // installation, pour garder une sortie exploitable par un script.
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("Version installée : {}", report.current_version);
    println!("Dernière version  : {}", report.latest_version);

    if !report.update_available {
        println!("mnemo est à jour ✓");
        return Ok(());
    }

    println!("Mise à jour disponible ✓");

    // Option explicite `--upgrade` : on lance l'installation. `--yes` est
    // transmis tel quel pour permettre un upgrade automatisé ; sans `--yes`,
    // c'est `upgrade` lui-même qui demandera la confirmation finale (un seul
    // prompt, pas de double question).
    if upgrade {
        return super::upgrade::run(false, assume_yes, None, None);
    }

    // Sans `--upgrade` : proposer l'installation uniquement en terminal
    // interactif. Le consentement donné ici vaut confirmation, donc on appelle
    // `upgrade` avec `assume_yes = true` pour éviter un second prompt.
    if prompt_install_now()? {
        return super::upgrade::run(false, true, None, None);
    }

    // Mode vérification seule (non interactif, ou refus de l'utilisateur) :
    // on conserve l'indication classique.
    println!("  Lancez `mnemo upgrade` pour l'installer.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapport_mise_a_jour_disponible() {
        let r = UpdateReport::new("v0.4.0", "v0.5.0");
        assert_eq!(r.current_version, "v0.4.0");
        assert_eq!(r.latest_version, "v0.5.0");
        assert!(r.update_available);
        assert!(r.asset_target.contains("linux-musl"));
    }

    #[test]
    fn rapport_a_jour() {
        let r = UpdateReport::new("v0.5.0", "v0.5.0");
        assert!(!r.update_available);
    }

    #[test]
    fn rapport_serialise_json() {
        let r = UpdateReport::new("0.4.0", "0.5.0");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["current_version"], "v0.4.0");
        assert_eq!(json["latest_version"], "v0.5.0");
        assert_eq!(json["update_available"], true);
        assert!(json["asset_target"].is_string());
    }

    #[test]
    fn interpret_yes_accepte_les_reponses_affirmatives() {
        for ok in ["o", "O", "oui", "OUI", "y", "Y", "yes", "Yes", " oui "] {
            assert!(interpret_yes(ok), "« {ok} » devrait valoir oui");
        }
    }

    #[test]
    fn interpret_yes_refuse_le_reste_et_la_ligne_vide() {
        for no in ["", " ", "\n", "n", "non", "no", "nope", "x", "1"] {
            assert!(!interpret_yes(no), "« {no} » devrait valoir non");
        }
    }
}
