//! Extraction sûre d'archives `tar` (défense en profondeur).
//!
//! La crate `tar` valide déjà les chemins lors de `unpack`, mais mnemo manipule
//! des archives potentiellement fournies par l'utilisateur (`mnemo restore`) ou
//! téléchargées (`mnemo upgrade`). On ajoute donc une validation **explicite et
//! testable** de chaque entrée avant écriture :
//! - rejet des chemins absolus (`/tmp/evil`) ;
//! - rejet des remontées de dossier (`../evil`) ;
//! - rejet des liens (symboliques / physiques) dont la cible sort de la racine.
//!
//! Garantie : aucune écriture hors du dossier de destination fourni.

use std::io::Read;
use std::path::{Component, Path};

use anyhow::{bail, Result};

/// Vérifie qu'un chemin d'entrée d'archive est sûr (fonction pure).
///
/// Accepte uniquement des chemins relatifs composés de segments normaux
/// (`a/b/c`) ou `.`. Rejette tout chemin absolu ou contenant `..`.
pub fn validate_entry_path(path: &Path) -> Result<()> {
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => {
                bail!(
                    "entrée d'archive rejetée (remontée de dossier `..`) : {}",
                    path.display()
                )
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "entrée d'archive rejetée (chemin absolu) : {}",
                    path.display()
                )
            }
        }
    }
    Ok(())
}

/// Extrait une archive `tar` dans `dest` en validant chaque entrée **avant**
/// écriture. Le chemin de l'entrée et, le cas échéant, la cible d'un lien sont
/// contrôlés par [`validate_entry_path`].
pub fn safe_unpack<R: Read>(mut archive: tar::Archive<R>, dest: &Path) -> Result<()> {
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        validate_entry_path(&path)?;

        // Les liens (symboliques ou physiques) doivent eux aussi pointer dans
        // la racine : on valide leur cible.
        if let Some(link) = entry.link_name()? {
            validate_entry_path(&link)?;
        }

        // `unpack_in` revalide de son côté (défense en profondeur) et renvoie
        // `false` s'il refuse d'écrire hors de `dest`.
        let unpacked = entry.unpack_in(dest)?;
        if !unpacked {
            bail!(
                "entrée d'archive refusée (hors du dossier cible) : {}",
                path.display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn accepte_chemins_relatifs_normaux() {
        for p in ["history.db", "dir/config.toml", "./a/b.json"] {
            assert!(validate_entry_path(&PathBuf::from(p)).is_ok(), "{p}");
        }
    }

    #[test]
    fn rejette_remontee_parent() {
        for p in ["../evil", "a/../../evil", "../../etc/passwd"] {
            assert!(validate_entry_path(&PathBuf::from(p)).is_err(), "{p}");
        }
    }

    #[test]
    fn rejette_chemin_absolu() {
        for p in ["/tmp/evil", "/etc/passwd"] {
            assert!(validate_entry_path(&PathBuf::from(p)).is_err(), "{p}");
        }
    }

    #[test]
    fn safe_unpack_extrait_archive_saine() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let mut header = tar::Header::new_gnu();
        let data = b"hello";
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = tar::Builder::new(enc);
        builder
            .append_data(&mut header, "sub/file.txt", &data[..])
            .unwrap();
        let bytes = builder.into_inner().unwrap().finish().unwrap();

        let tmp = std::env::temp_dir().join(format!("mnemo-archive-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let archive = tar::Archive::new(flate2::read::GzDecoder::new(&bytes[..]));
        safe_unpack(archive, &tmp).unwrap();
        assert_eq!(std::fs::read(tmp.join("sub/file.txt")).unwrap(), data);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn safe_unpack_rejette_path_traversal() {
        // Archive construite à la main avec un chemin `../evil` (le champ nom du
        // header est écrit directement pour contourner la validation du builder).
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let data = b"evil";
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        {
            let gnu = header.as_gnu_mut().unwrap();
            let name = b"../evil.txt";
            gnu.name[..name.len()].copy_from_slice(name);
        }
        header.set_cksum();
        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = tar::Builder::new(enc);
        builder.append(&header, &data[..]).unwrap();
        let bytes = builder.into_inner().unwrap().finish().unwrap();

        let tmp = std::env::temp_dir().join(format!(
            "mnemo-archive-evil-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let archive = tar::Archive::new(flate2::read::GzDecoder::new(&bytes[..]));
        let result = safe_unpack(archive, &tmp);
        assert!(result.is_err(), "le path traversal doit être rejeté");
        // Rien ne doit avoir été écrit hors du tempdir.
        assert!(!tmp.parent().unwrap().join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
