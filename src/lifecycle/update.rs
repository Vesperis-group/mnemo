//! `mnemo update` : vérifie la disponibilité d'une nouvelle version **sans rien
//! installer**.

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

/// Point d'entrée de la commande.
pub fn run(json: bool) -> Result<()> {
    let current = current_version();
    let latest = fetch_latest_release()?.tag_name;
    let report = UpdateReport::new(&current, &latest);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Version installée : {}", report.current_version);
        println!("Dernière version  : {}", report.latest_version);
        if report.update_available {
            println!("Mise à jour disponible ✓");
            println!("  Lancez `mnemo upgrade` pour l'installer.");
        } else {
            println!("mnemo est à jour ✓");
        }
    }
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
}
