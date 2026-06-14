//! `mnemo upgrade` : télécharge la dernière release (ou une version donnée),
//! vérifie son intégrité SHA-256, sauvegarde les données puis remplace le
//! binaire de façon atomique.
//!
//! Garanties :
//! - HTTPS par défaut, vérification SHA-256 **avant** extraction ;
//! - aucune exécution de script distant (on ne récupère que des assets) ;
//! - les données (`history.db`, `config.toml`, sauvegardes) ne sont **jamais**
//!   touchées ;
//! - en cas d'échec, le binaire en place reste intact.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{backup, config, confirm};

use super::github::{asset_url, fetch_latest_release, http_get_bytes, http_get_string};
use super::uninstall::bin_path;
use super::{
    asset_names_for_version, current_version, normalize_tag, parse_sha256_file, signature,
    target_triple, update_available, verify_sha256,
};

/// Point d'entrée de la commande.
///
/// `require_signature` rend la vérification Sigstore **obligatoire** : `cosign`
/// doit être disponible, le bundle de signature téléchargeable et la
/// vérification valide, sinon l'upgrade est refusé. Sans ce drapeau, la
/// vérification reste best-effort (avertissement si `cosign` est absent, le
/// SHA-256 obligatoire demeurant le contrôle bloquant).
pub fn run(
    dry_run: bool,
    assume_yes: bool,
    version: Option<String>,
    target: Option<String>,
    require_signature: bool,
) -> Result<()> {
    let current = current_version();
    let target_triple = target.unwrap_or_else(|| target_triple().to_string());

    // 1. Résolution de la version cible.
    let (tag, explicit) = match version {
        Some(v) => (normalize_tag(&v), true),
        None => (normalize_tag(&fetch_latest_release()?.tag_name), false),
    };

    println!("Version installée : {current}");
    println!("Version cible     : {tag} ({target_triple})");

    // 2. Déjà à jour ? (sauf si une version explicite est demandée.)
    if !explicit && !update_available(&current, &tag) {
        println!("mnemo est déjà à jour ✓ - rien à faire.");
        return Ok(());
    }

    let (archive_name, sha_name) = asset_names_for_version(&tag, &target_triple);
    let archive_url = asset_url(&tag, &archive_name);
    let sha_url = asset_url(&tag, &sha_name);
    let bin = bin_path();

    println!("Archive           : {archive_url}");

    if dry_run {
        println!("\nSimulation : aucun téléchargement ni remplacement effectué.");
        println!("Le binaire {} serait remplacé par {tag}.", bin.display());
        return Ok(());
    }

    // 3. Confirmation avant remplacement.
    let ok = confirm::confirm(
        &format!("Installer {tag} en remplacement de {current} ?"),
        assume_yes,
    )?;
    if !ok {
        println!("Mise à niveau annulée.");
        return Ok(());
    }

    // 4. Téléchargement de l'archive et de sa somme de contrôle.
    println!("Téléchargement de l'archive…");
    let archive_bytes = http_get_bytes(&archive_url)
        .with_context(|| format!("téléchargement de {archive_url} échoué"))?;
    let sha_text =
        http_get_string(&sha_url).with_context(|| format!("téléchargement de {sha_url} échoué"))?;
    let expected = parse_sha256_file(&sha_text)
        .context("fichier .sha256 illisible (somme attendue introuvable)")?;

    // 5. Vérification d'intégrité AVANT toute extraction.
    if !verify_sha256(&archive_bytes, &expected) {
        anyhow::bail!(
            "vérification SHA-256 échouée - installation refusée (archive corrompue ou altérée)"
        );
    }
    println!("Intégrité SHA-256 vérifiée ✓");

    // 5 bis. Vérification Sigstore (défense en profondeur). Le SHA-256 ci-dessus
    // reste l'unique contrôle obligatoire ; cette étape ne s'exécute qu'après
    // lui. En mode strict (`require_signature`), toute impossibilité de vérifier
    // refuse l'installation ; sinon, elle est best-effort.
    let tmp = tempdir()?;
    enforce_signature(
        &tag,
        &archive_name,
        &archive_bytes,
        require_signature,
        tmp.path(),
    )?;

    // 6. Extraction dans le dossier temporaire.
    extract_targz(&archive_bytes, tmp.path()).context("extraction de l'archive échouée")?;
    let extracted =
        find_binary(tmp.path(), "mnemo").context("binaire `mnemo` introuvable dans l'archive")?;

    // 7. Le nouveau binaire répond-il ?
    set_executable(&extracted)?;
    verify_binary_runs(&extracted)
        .context("le binaire téléchargé ne s'exécute pas correctement")?;

    // 8. Sauvegarde des données AVANT remplacement (si une base existe).
    if config::db_path().map(|p| p.exists()).unwrap_or(false) {
        match backup::create_backup(None) {
            Ok(info) => println!("Sauvegarde des données : {}", info.path.display()),
            Err(e) => eprintln!("Avertissement : sauvegarde impossible ({e})"),
        }
    }

    // 9. Remplacement atomique du binaire.
    replace_binary(&extracted, &bin)
        .with_context(|| format!("remplacement de {} échoué", bin.display()))?;

    println!("\nmnemo mis à niveau : {current} → {tag} ✓");
    println!("Binaire : {}", bin.display());
    Ok(())
}

/// Applique la politique de signature Sigstore après la vérification SHA-256.
///
/// SHA-256 reste l'unique contrôle **obligatoire** ; cette étape est une
/// défense en profondeur :
/// - `cosign` absent + mode best-effort → avertissement, on continue ;
/// - `cosign` absent + mode strict → refus ;
/// - bundle indisponible + best-effort → avertissement, on continue ;
/// - bundle indisponible + strict → refus ;
/// - signature invalide → refus **dans tous les cas** ;
/// - signature valide → on continue.
fn enforce_signature(
    tag: &str,
    archive_name: &str,
    archive_bytes: &[u8],
    require_signature: bool,
    workdir: &Path,
) -> Result<()> {
    if !signature::cosign_available() {
        if require_signature {
            anyhow::bail!("Signature Sigstore obligatoire mais cosign est introuvable.");
        }
        println!(
            "Signature Sigstore non vérifiée : cosign absent (continuité autorisée car SHA-256 \
             vérifié). Utilisez --require-signature pour rendre ce contrôle obligatoire."
        );
        return Ok(());
    }

    // cosign disponible : récupération du bundle de signature de l'archive.
    let bundle_name = signature::signature_asset_name(archive_name);
    let bundle_url = asset_url(tag, &bundle_name);
    let bundle_bytes = match http_get_bytes(&bundle_url) {
        Ok(b) => b,
        Err(e) => {
            if require_signature {
                anyhow::bail!(
                    "Signature Sigstore obligatoire mais le bundle {bundle_name} est \
                     indisponible : {e}"
                );
            }
            println!(
                "Signature Sigstore non vérifiée : bundle indisponible ({e}) (continuité \
                 autorisée car SHA-256 vérifié)."
            );
            return Ok(());
        }
    };

    // Écriture de l'asset et du bundle sur disque pour `cosign verify-blob`.
    let asset_path = workdir.join(archive_name);
    std::fs::write(&asset_path, archive_bytes)
        .context("écriture de l'archive temporaire échouée")?;
    let bundle_path = workdir.join(&bundle_name);
    std::fs::write(&bundle_path, &bundle_bytes)
        .context("écriture du bundle de signature échouée")?;

    match signature::verify_sigstore_bundle(&asset_path, &bundle_path) {
        Ok(()) => {
            println!("Signature Sigstore vérifiée ✓");
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Signature Sigstore invalide - installation refusée : {e}");
        }
    }
}

/// Crée un dossier temporaire dédié.
fn tempdir() -> Result<TempDir> {
    TempDir::new().context("création d'un dossier temporaire échouée")
}

/// Décompresse une archive `.tar.gz` (en mémoire) vers `dest`.
fn extract_targz(bytes: &[u8], dest: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    let decoder = GzDecoder::new(bytes);
    let archive = Archive::new(decoder);
    crate::archive::safe_unpack(archive, dest)?;
    Ok(())
}

/// Recherche récursive d'un fichier de nom `name` sous `dir`.
pub fn find_binary(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(path);
        }
        if path.is_dir() {
            subdirs.push(path);
        }
    }
    for sub in subdirs {
        if let Some(found) = find_binary(&sub, name) {
            return Some(found);
        }
    }
    None
}

/// Rend un fichier exécutable (`chmod u+rwx,go+rx`).
fn set_executable(path: &Path) -> Result<()> {
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

/// Vérifie que le binaire répond à `--version` (code de sortie nul).
fn verify_binary_runs(path: &Path) -> Result<()> {
    let status = std::process::Command::new(path)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("exécution de {} impossible", path.display()))?;
    if !status.success() {
        anyhow::bail!("`--version` a renvoyé un code non nul");
    }
    Ok(())
}

/// Remplace `dest` par `src` de façon atomique (écriture dans le même dossier
/// puis `rename`). Le remplacement du binaire en cours d'exécution est sûr sous
/// Linux (l'inode reste valide jusqu'à la fin du processus).
fn replace_binary(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let tmp_dest = dest.with_extension("mnemo-new");
    std::fs::copy(src, &tmp_dest)
        .with_context(|| format!("copie vers {} échouée", tmp_dest.display()))?;
    set_executable(&tmp_dest)?;
    std::fs::rename(&tmp_dest, dest).map_err(|e| {
        // Nettoyage : on ne laisse pas de binaire temporaire derrière nous.
        let _ = std::fs::remove_file(&tmp_dest);
        anyhow::anyhow!("renommage atomique échoué : {e}")
    })?;
    Ok(())
}

/// Dossier temporaire minimaliste supprimé à la destruction (sans dépendance
/// supplémentaire : `tempfile` n'est disponible qu'en dev).
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = base.join(format!("mnemo-upgrade-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Petit helper de test : crée une archive `.tar.gz` contenant un binaire.
#[cfg(test)]
pub fn make_test_archive(bin_name: &str, dir_prefix: &str, content: &[u8]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let path = format!("{dir_prefix}/{bin_name}");
    builder.append_data(&mut header, path, content).unwrap();
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recherche_binaire_recursive() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("mnemo-v0.5.0-x86_64-unknown-linux-musl");
        std::fs::create_dir_all(&sub).unwrap();
        let bin = sub.join("mnemo");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let found = find_binary(tmp.path(), "mnemo").unwrap();
        assert_eq!(found, bin);
        assert!(find_binary(tmp.path(), "absent").is_none());
    }

    #[test]
    fn extraction_archive() {
        let archive = make_test_archive(
            "mnemo",
            "mnemo-v0.5.0-x86_64-unknown-linux-musl",
            b"#!/bin/sh\necho ok\n",
        );
        let tmp = TempDir::new().unwrap();
        extract_targz(&archive, tmp.path()).unwrap();
        let found = find_binary(tmp.path(), "mnemo").unwrap();
        let content = std::fs::read(&found).unwrap();
        assert!(content.starts_with(b"#!/bin/sh"));
    }

    #[test]
    fn remplacement_atomique() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dest = tmp.path().join("dest");
        std::fs::write(&src, b"nouveau").unwrap();
        std::fs::write(&dest, b"ancien").unwrap();
        replace_binary(&src, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"nouveau");
    }
}
