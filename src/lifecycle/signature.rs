//! Vérification **optionnelle** des signatures Sigstore (`cosign`) des assets de
//! release.
//!
//! Le contrôle d'intégrité **obligatoire et bloquant** reste le SHA-256 (cf.
//! [`super::verify_sha256`]). La vérification Sigstore est une **défense en
//! profondeur** : elle atteste que l'artefact a bien été signé, en mode keyless
//! (OIDC GitHub Actions), par le workflow de release du dépôt mnemo. Elle est :
//! - **best-effort** par défaut : si `cosign` est absent, on émet un
//!   avertissement clair puis on continue (le SHA-256 ayant déjà été vérifié) ;
//! - **bloquante** en mode strict (`--require-signature` côté `mnemo upgrade`,
//!   `MNEMO_REQUIRE_SIGNATURE=1` côté `install.sh`) : toute impossibilité de
//!   vérifier refuse l'installation.
//!
//! Les fonctions pures (construction du nom de bundle, identité/issuer attendus)
//! sont testables sans réseau ni `cosign`. L'exécutable `cosign` est
//! surchargeable via `MNEMO_COSIGN_BIN`, ce qui permet de mocker les appels
//! système dans les tests d'intégration sans dépendre d'un vrai `cosign`.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Nom (ou chemin) de l'exécutable `cosign`. Surchargé par `MNEMO_COSIGN_BIN`
/// (utile pour injecter un faux `cosign` dans les tests).
fn cosign_bin() -> String {
    std::env::var("MNEMO_COSIGN_BIN").unwrap_or_else(|_| "cosign".to_string())
}

/// Identité keyless attendue du certificat de signature : le workflow de release
/// du dépôt mnemo, sur la branche `main`. Surchargeable via
/// `MNEMO_SIGN_IDENTITY` (tests / configuration avancée).
pub fn expected_identity() -> String {
    std::env::var("MNEMO_SIGN_IDENTITY").unwrap_or_else(|_| {
        "https://github.com/Vesperis-group/mnemo/.github/workflows/release.yml@refs/heads/main"
            .to_string()
    })
}

/// Émetteur OIDC attendu (GitHub Actions). Surchargeable via
/// `MNEMO_SIGN_OIDC_ISSUER`.
pub fn expected_oidc_issuer() -> String {
    std::env::var("MNEMO_SIGN_OIDC_ISSUER")
        .unwrap_or_else(|_| "https://token.actions.githubusercontent.com".to_string())
}

/// Nom du bundle de signature Sigstore associé à un asset.
///
/// Pour `mnemo-v0.8.0-x86_64-unknown-linux-musl.tar.gz`, le bundle attendu est
/// `mnemo-v0.8.0-x86_64-unknown-linux-musl.tar.gz.sigstore.json`.
pub fn signature_asset_name(asset_name: &str) -> String {
    format!("{asset_name}.sigstore.json")
}

/// Vrai si l'exécutable `cosign` est disponible et répond (`cosign version`).
///
/// Renvoie `false` si le binaire est introuvable ou ne s'exécute pas : aucune
/// erreur n'est propagée, l'appelant décide de la politique (best-effort ou
/// strict).
pub fn cosign_available() -> bool {
    Command::new(cosign_bin())
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Vérifie un bundle Sigstore (`cosign verify-blob --bundle`) pour `asset`.
///
/// L'identité du certificat et l'émetteur OIDC attendus sont contraints
/// (keyless GitHub Actions). Renvoie `Ok(())` si la signature est valide, une
/// erreur sinon (cosign injoignable ou vérification refusée).
pub fn verify_sigstore_bundle(asset: &Path, bundle: &Path) -> Result<()> {
    let status = Command::new(cosign_bin())
        .arg("verify-blob")
        .arg("--bundle")
        .arg(bundle)
        .arg("--certificate-identity")
        .arg(expected_identity())
        .arg("--certificate-oidc-issuer")
        .arg(expected_oidc_issuer())
        .arg(asset)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("exécution de `cosign verify-blob` impossible")?;
    if !status.success() {
        anyhow::bail!("`cosign verify-blob` a échoué (signature invalide ou non conforme)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nom_bundle_signature_construit() {
        assert_eq!(
            signature_asset_name("mnemo-v0.8.0-x86_64-unknown-linux-musl.tar.gz"),
            "mnemo-v0.8.0-x86_64-unknown-linux-musl.tar.gz.sigstore.json"
        );
        assert_eq!(
            signature_asset_name("mnemo-v0.8.0-aarch64-unknown-linux-musl.tar.gz"),
            "mnemo-v0.8.0-aarch64-unknown-linux-musl.tar.gz.sigstore.json"
        );
    }

    #[test]
    fn identite_et_issuer_par_defaut() {
        // Valeurs par défaut attendues (hors surcharge d'environnement).
        std::env::remove_var("MNEMO_SIGN_IDENTITY");
        std::env::remove_var("MNEMO_SIGN_OIDC_ISSUER");
        assert_eq!(
            expected_identity(),
            "https://github.com/Vesperis-group/mnemo/.github/workflows/release.yml@refs/heads/main"
        );
        assert_eq!(
            expected_oidc_issuer(),
            "https://token.actions.githubusercontent.com"
        );
    }

    #[test]
    fn cosign_absent_renvoie_faux() {
        // Un binaire inexistant ne doit jamais être considéré comme disponible.
        std::env::set_var("MNEMO_COSIGN_BIN", "/nonexistent/cosign-mnemo-test");
        assert!(!cosign_available());
        std::env::remove_var("MNEMO_COSIGN_BIN");
    }
}
