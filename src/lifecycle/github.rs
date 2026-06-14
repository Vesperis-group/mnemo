//! Interrogation de l'API GitHub Releases.
//!
//! On utilise le client HTTP bloquant `ureq` (TLS rustls, sans runtime async),
//! et on isole la couche réseau de la couche d'analyse - cette dernière, pure,
//! est testée sans réseau dans [`parse_github_latest_release`].

use anyhow::{Context, Result};
use serde::Deserialize;

/// Informations minimales extraites d'une release GitHub.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseInfo {
    /// Tag de la release (ex. `v0.5.0`).
    pub tag_name: String,
    /// Vrai s'il s'agit d'une pré-release. Exposé pour le filtrage et les tests
    /// (`/releases/latest` exclut déjà nativement les pré-releases).
    #[serde(default)]
    #[allow(dead_code)]
    pub prerelease: bool,
}

/// Propriétaire du dépôt (surcharge possible via `MNEMO_OWNER`).
pub fn owner() -> String {
    std::env::var("MNEMO_OWNER").unwrap_or_else(|_| "Vesperis-group".to_string())
}

/// Nom du dépôt (surcharge possible via `MNEMO_REPO`).
pub fn repo() -> String {
    std::env::var("MNEMO_REPO").unwrap_or_else(|_| "mnemo".to_string())
}

/// Base de l'API GitHub (surcharge possible via `MNEMO_GITHUB_API`, utile pour
/// les tests qui pointent vers un serveur local).
pub fn api_base() -> String {
    std::env::var("MNEMO_GITHUB_API")
        .unwrap_or_else(|_| "https://api.github.com".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Base des téléchargements de release (surcharge via `MNEMO_GITHUB_BASE`).
pub fn download_base() -> String {
    std::env::var("MNEMO_GITHUB_BASE")
        .unwrap_or_else(|_| "https://github.com".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Analyse la réponse JSON de `/releases/latest` (fonction pure, testable).
pub fn parse_github_latest_release(json: &str) -> Result<ReleaseInfo> {
    let info: ReleaseInfo =
        serde_json::from_str(json).context("réponse GitHub illisible (JSON invalide)")?;
    if info.tag_name.trim().is_empty() {
        anyhow::bail!("réponse GitHub sans `tag_name`");
    }
    Ok(info)
}

/// URL de l'asset de release pour un tag et un nom de fichier donnés.
pub fn asset_url(tag: &str, file_name: &str) -> String {
    format!(
        "{}/{}/{}/releases/download/{}/{}",
        download_base(),
        owner(),
        repo(),
        tag,
        file_name
    )
}

/// Récupère la dernière release stable via l'API GitHub.
///
/// `/releases/latest` exclut nativement les brouillons et pré-releases.
pub fn fetch_latest_release() -> Result<ReleaseInfo> {
    let url = format!(
        "{}/repos/{}/{}/releases/latest",
        api_base(),
        owner(),
        repo()
    );
    let body = http_get_string(&url).with_context(|| format!("échec de la requête vers {url}"))?;
    parse_github_latest_release(&body)
}

/// GET HTTP renvoyant le corps en texte.
pub fn http_get_string(url: &str) -> Result<String> {
    let resp = ureq::get(url)
        .set("User-Agent", "mnemo-cli")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(map_ureq_error)?;
    resp.into_string()
        .context("corps de réponse HTTP illisible")
}

/// GET HTTP renvoyant le corps en octets (assets binaires).
pub fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url)
        .set("User-Agent", "mnemo-cli")
        .call()
        .map_err(map_ureq_error)?;
    let mut buf = Vec::new();
    use std::io::Read;
    resp.into_reader()
        .read_to_end(&mut buf)
        .context("téléchargement interrompu")?;
    Ok(buf)
}

/// Transforme une erreur `ureq` en message clair (statut HTTP ou transport).
fn map_ureq_error(err: ureq::Error) -> anyhow::Error {
    match err {
        ureq::Error::Status(code, resp) => {
            let url = resp.get_url().to_string();
            anyhow::anyhow!("réponse HTTP {code} pour {url}")
        }
        ureq::Error::Transport(t) => {
            anyhow::anyhow!("erreur réseau : {t}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_release_stable() {
        let json = r#"{"tag_name":"v0.5.0","prerelease":false,"name":"v0.5.0"}"#;
        let info = parse_github_latest_release(json).unwrap();
        assert_eq!(info.tag_name, "v0.5.0");
        assert!(!info.prerelease);
    }

    #[test]
    fn parse_release_prerelease() {
        let json = r#"{"tag_name":"v0.6.0-rc1","prerelease":true}"#;
        let info = parse_github_latest_release(json).unwrap();
        assert_eq!(info.tag_name, "v0.6.0-rc1");
        assert!(info.prerelease);
    }

    #[test]
    fn parse_release_json_invalide() {
        assert!(parse_github_latest_release("pas du json").is_err());
        assert!(parse_github_latest_release(r#"{"name":"x"}"#).is_err());
        assert!(parse_github_latest_release(r#"{"tag_name":""}"#).is_err());
    }

    #[test]
    fn url_asset_construite() {
        std::env::set_var("MNEMO_GITHUB_BASE", "http://localhost:9");
        std::env::set_var("MNEMO_OWNER", "acme");
        std::env::set_var("MNEMO_REPO", "tool");
        let url = asset_url("v0.5.0", "mnemo-v0.5.0-x86_64-unknown-linux-musl.tar.gz");
        assert_eq!(
            url,
            "http://localhost:9/acme/tool/releases/download/v0.5.0/mnemo-v0.5.0-x86_64-unknown-linux-musl.tar.gz"
        );
        std::env::remove_var("MNEMO_GITHUB_BASE");
        std::env::remove_var("MNEMO_OWNER");
        std::env::remove_var("MNEMO_REPO");
    }
}
