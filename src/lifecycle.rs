//! Commandes de cycle de vie : `update`, `upgrade`, `uninstall`.
//!
//! Le module racine regroupe les helpers **purs et testables** (cible,
//! comparaison de versions, noms d'assets, vérification SHA-256, gestion du bloc
//! `.bashrc`). Les effets de bord (réseau, système de fichiers) vivent dans les
//! sous-modules :
//! - [`github`] : interrogation de l'API GitHub Releases ;
//! - [`update`] : « y a-t-il une nouvelle version ? » (aucune installation) ;
//! - [`upgrade`] : téléchargement + vérification + remplacement du binaire ;
//! - [`signature`] : vérification optionnelle des signatures Sigstore (cosign) ;
//! - [`uninstall`] : retrait du binaire / bloc `.bashrc`, purge optionnelle.
//!
//! Sécurité : HTTPS par défaut, SHA-256 obligatoire avant toute extraction,
//! aucune exécution de script distant. Les données (`history.db`, `config.toml`,
//! sauvegardes) ne sont jamais touchées par `upgrade`, et seulement par
//! `uninstall --purge` après confirmation et sauvegarde.

pub mod github;
pub mod signature;
pub mod uninstall;
pub mod update;
pub mod upgrade;

use std::cmp::Ordering;

/// Triplet cible utilisé pour nommer les assets de release.
///
/// Linux est la seule plateforme supportée ; on privilégie le binaire **musl**
/// statique, le plus portable.
pub fn target_triple() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64-unknown-linux-musl",
        // x86_64 et repli par défaut.
        _ => "x86_64-unknown-linux-musl",
    }
}

/// Version courante du binaire, préfixée par `v` (ex. `v0.5.0`).
pub fn current_version() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// Garantit le préfixe `v` sur un tag (`0.5.0` -> `v0.5.0`, `v0.5.0` inchangé).
pub fn normalize_tag(version: &str) -> String {
    let v = version.trim();
    if v.starts_with('v') {
        v.to_string()
    } else {
        format!("v{v}")
    }
}

/// Découpe une version `vX.Y.Z` (suffixe de pré-release ignoré) en triplet
/// numérique. Renvoie `None` si le format est inexploitable.
pub fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let v = version.trim().trim_start_matches('v');
    // On ignore un éventuel suffixe de pré-release (`-rc1`, `+build`…).
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Compare deux versions sémantiques simples (`vX.Y.Z`). Les versions
/// illisibles sont considérées « égales » (prudence : pas de fausse mise à jour).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (parse_version(a), parse_version(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        _ => Ordering::Equal,
    }
}

/// Vrai si `latest` est strictement plus récent que `current`.
pub fn update_available(current: &str, latest: &str) -> bool {
    compare_versions(current, latest) == Ordering::Less
}

/// Noms des assets de release pour un tag et une cible donnés :
/// `(archive .tar.gz, fichier .sha256)`.
pub fn asset_names_for_version(tag: &str, target: &str) -> (String, String) {
    let tag = normalize_tag(tag);
    let archive = format!("mnemo-{tag}-{target}.tar.gz");
    let sha = format!("{archive}.sha256");
    (archive, sha)
}

/// Calcule la somme SHA-256 d'un contenu et la rend en hexadécimal minuscule.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Extrait la somme hexadécimale d'un fichier `.sha256` au format `sha256sum`
/// (`"<hex>  <nom de fichier>"`), ou d'une ligne ne contenant que le hex.
pub fn parse_sha256_file(content: &str) -> Option<String> {
    let token = content.split_whitespace().next()?;
    let lower = token.to_lowercase();
    if lower.len() == 64 && lower.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(lower)
    } else {
        None
    }
}

/// Vérifie que `data` correspond à la somme attendue (hex, casse ignorée).
pub fn verify_sha256(data: &[u8], expected_hex: &str) -> bool {
    sha256_hex(data) == expected_hex.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparaison_versions() {
        assert_eq!(compare_versions("v0.4.0", "v0.5.0"), Ordering::Less);
        assert_eq!(compare_versions("v0.5.0", "v0.4.0"), Ordering::Greater);
        assert_eq!(compare_versions("v0.5.0", "v0.5.0"), Ordering::Equal);
        // Préfixe `v` optionnel et patch implicite.
        assert_eq!(compare_versions("0.5", "v0.5.0"), Ordering::Equal);
        assert_eq!(compare_versions("v0.5.1", "v0.5.0"), Ordering::Greater);
        assert_eq!(compare_versions("v1.0.0", "v0.9.9"), Ordering::Greater);
    }

    #[test]
    fn update_disponible() {
        assert!(update_available("v0.4.0", "v0.5.0"));
        assert!(!update_available("v0.5.0", "v0.5.0"));
        assert!(!update_available("v0.5.0", "v0.4.0"));
    }

    #[test]
    fn parse_version_robuste() {
        assert_eq!(parse_version("v0.5.0"), Some((0, 5, 0)));
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v0.5.0-rc1"), Some((0, 5, 0)));
        assert_eq!(parse_version("0.5"), Some((0, 5, 0)));
        assert_eq!(parse_version("abc"), None);
        assert_eq!(parse_version("0.5.0.1"), None);
    }

    #[test]
    fn cible_par_defaut() {
        // Sur la machine de test (x86_64), on attend la cible musl.
        let t = target_triple();
        assert!(t.contains("linux-musl"));
    }

    #[test]
    fn noms_assets() {
        let (archive, sha) = asset_names_for_version("v0.5.0", "x86_64-unknown-linux-musl");
        assert_eq!(archive, "mnemo-v0.5.0-x86_64-unknown-linux-musl.tar.gz");
        assert_eq!(sha, "mnemo-v0.5.0-x86_64-unknown-linux-musl.tar.gz.sha256");
        // Tag sans `v` : normalisé.
        let (archive2, _) = asset_names_for_version("0.5.0", "x86_64-unknown-linux-musl");
        assert_eq!(archive2, "mnemo-v0.5.0-x86_64-unknown-linux-musl.tar.gz");
    }

    #[test]
    fn sha256_ok_et_ko() {
        let data = b"hello mnemo";
        let hex = sha256_hex(data);
        assert_eq!(hex.len(), 64);
        assert!(verify_sha256(data, &hex));
        assert!(verify_sha256(data, &hex.to_uppercase()));
        assert!(!verify_sha256(data, &"0".repeat(64)));
        assert!(!verify_sha256(b"autre contenu", &hex));
    }

    #[test]
    fn parse_fichier_sha256() {
        let hex = "a".repeat(64);
        let line = format!("{hex}  mnemo-v0.5.0-x86_64-unknown-linux-musl.tar.gz\n");
        assert_eq!(parse_sha256_file(&line), Some(hex.clone()));
        // Hex seul.
        assert_eq!(parse_sha256_file(&hex), Some(hex.clone()));
        // Invalide.
        assert_eq!(parse_sha256_file("pas un hash"), None);
        assert_eq!(parse_sha256_file(""), None);
    }
}
