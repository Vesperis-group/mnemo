//! Sauvegarde et restauration locales (`mnemo backup` / `mnemo restore`).
//!
//! Une sauvegarde est une archive `.tar.gz` autonome contenant :
//! - `history.db` : la base SQLite complète ;
//! - `config.toml` : la configuration (si présente) ;
//! - `metadata.json` : des métadonnées de traçabilité (version, date, tailles…).
//!
//! Sécurité : la restauration valide systématiquement l'archive (DB ouvrable,
//! table `commands`, version de schéma) et crée automatiquement une sauvegarde
//! de l'état courant avant tout remplacement. `--dry-run` ne modifie rien et
//! sans `--yes` une confirmation interactive est demandée.

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{config, confirm, db, migrations};
/// Métadonnées embarquées dans `metadata.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub mnemo_version: String,
    pub created_at: String,
    pub db_path: String,
    pub config_path: String,
    pub db_size_bytes: u64,
    pub command_count: i64,
    pub schema_version: i64,
}

/// Résultat d'une création de sauvegarde.
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub metadata: BackupMetadata,
}

/// Secondes Unix + horodatage formaté `YYYY-MM-DD HH:MM:SS`.
fn timestamp_parts() -> (u64, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs, db::format_timestamp(secs))
}

/// `"2026-06-14 12:34:56"` -> `"2026-06-14T12:34:56Z"` (ISO 8601 UTC).
fn iso_from(formatted: &str) -> String {
    format!("{}Z", formatted.replacen(' ', "T", 1))
}

/// `"2026-06-14 12:34:56"` -> `"mnemo-backup-20260614-123456.tar.gz"`.
fn archive_filename(formatted: &str) -> String {
    let digits: String = formatted.chars().filter(|c| c.is_ascii_digit()).collect();
    let (date, time) = digits.split_at(8.min(digits.len()));
    format!("mnemo-backup-{date}-{time}.tar.gz")
}

/// Garantit un chemin d'archive non existant : si une sauvegarde a déjà été
/// créée dans la même seconde, on suffixe `-1`, `-2`… pour ne jamais écraser.
fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let stem = name.strip_suffix(".tar.gz").unwrap_or(name);
    for i in 1.. {
        let candidate = parent.join(format!("{stem}-{i}.tar.gz"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

/// Crée une sauvegarde complète dans `dest_dir` (ou le dossier par défaut
/// `~/.local/share/mnemo/backups/`). Réutilisée comme sauvegarde automatique
/// avant les opérations destructives.
pub fn create_backup(dest_dir: Option<&Path>) -> Result<BackupInfo> {
    let db_path = config::db_path()?;
    let config_path = config::config_path()?;

    let dest = match dest_dir {
        Some(d) => d.to_path_buf(),
        None => config::data_dir()?.join("backups"),
    };
    fs::create_dir_all(&dest)
        .with_context(|| format!("création du dossier de sauvegarde {}", dest.display()))?;
    config::harden_dir(&dest);

    let (secs, formatted) = timestamp_parts();

    // Métadonnées : on s'assure que la base existe et est migrée, puis on lit
    // ses caractéristiques sans la modifier davantage.
    let (command_count, schema_version) = {
        let conn = db::open(&db_path)?;
        let count = db::count(&conn)?;
        let ver = migrations::schema_version(&conn)?;
        (count, ver)
    };
    let db_size_bytes = fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let metadata = BackupMetadata {
        mnemo_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: iso_from(&formatted),
        db_path: db_path.display().to_string(),
        config_path: config_path.display().to_string(),
        db_size_bytes,
        command_count,
        schema_version,
    };

    let archive_path = dest.join(archive_filename(&formatted));
    let archive_path = unique_path(archive_path);
    write_archive(&archive_path, &db_path, &config_path, &metadata, secs)?;
    // L'archive contient l'historique et la configuration : permissions privées.
    config::harden_file(&archive_path);
    Ok(BackupInfo {
        path: archive_path,
        metadata,
    })
}

/// Dossier standard des sauvegardes (`~/.local/share/mnemo/backups`).
pub fn backups_dir() -> Result<PathBuf> {
    Ok(config::data_dir()?.join("backups"))
}

/// Liste les archives de sauvegarde (`*.tar.gz`) présentes dans `dir`.
///
/// Renvoie un vecteur vide si le dossier est absent ou illisible (jamais
/// d'erreur : utilisé par des contrôles best-effort).
pub fn list_archives(dir: &Path) -> Vec<PathBuf> {
    let mut archives = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_archive = path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".tar.gz"))
                    .unwrap_or(false);
            if is_archive {
                archives.push(path);
            }
        }
    }
    archives
}

/// Écrit l'archive `.tar.gz` sur disque.
fn write_archive(
    archive_path: &Path,
    db_path: &Path,
    config_path: &Path,
    metadata: &BackupMetadata,
    mtime: u64,
) -> Result<()> {
    let file = File::create(archive_path)
        .with_context(|| format!("création de l'archive {}", archive_path.display()))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(enc);

    builder
        .append_path_with_name(db_path, "history.db")
        .context("ajout de history.db à l'archive")?;

    if config_path.exists() {
        builder
            .append_path_with_name(config_path, "config.toml")
            .context("ajout de config.toml à l'archive")?;
    }

    let meta_json = serde_json::to_vec_pretty(metadata)?;
    let mut header = tar::Header::new_gnu();
    header.set_size(meta_json.len() as u64);
    header.set_mode(0o600);
    header.set_mtime(mtime);
    header.set_cksum();
    builder
        .append_data(&mut header, "metadata.json", meta_json.as_slice())
        .context("ajout de metadata.json à l'archive")?;

    let enc = builder.into_inner().context("finalisation du tar")?;
    enc.finish()
        .context("finalisation de la compression gzip")?;
    Ok(())
}

/// Point d'entrée de `mnemo backup`.
pub fn run(output: Option<PathBuf>, json: bool) -> Result<()> {
    let info = create_backup(output.as_deref())?;

    if json {
        let value = serde_json::json!({
            "backup_path": info.path.display().to_string(),
            "metadata": info.metadata,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Sauvegarde créée : {}", info.path.display());
        println!("  Commandes        : {}", info.metadata.command_count);
        println!(
            "  Taille DB        : {} octets",
            info.metadata.db_size_bytes
        );
        println!("  Version schéma   : {}", info.metadata.schema_version);
    }
    Ok(())
}

/// Fichiers attendus extraits d'une archive de restauration.
struct ExtractedBackup {
    dir: PathBuf,
    db: PathBuf,
    config: Option<PathBuf>,
    metadata: Option<BackupMetadata>,
}

impl Drop for ExtractedBackup {
    fn drop(&mut self) {
        // Nettoyage best-effort du dossier temporaire d'extraction.
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Extrait l'archive dans un dossier temporaire et localise les fichiers.
fn extract_archive(archive: &Path) -> Result<ExtractedBackup> {
    if !archive.exists() {
        bail!("archive introuvable : {}", archive.display());
    }

    let (secs, _) = timestamp_parts();
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("mnemo-restore-{pid}-{secs}"));
    fs::create_dir_all(&dir)
        .with_context(|| format!("création du dossier temporaire {}", dir.display()))?;

    let file = File::open(archive)
        .with_context(|| format!("ouverture de l'archive {}", archive.display()))?;
    let dec = GzDecoder::new(file);
    let ar = tar::Archive::new(dec);
    crate::archive::safe_unpack(ar, &dir)
        .with_context(|| format!("extraction de l'archive {}", archive.display()))?;

    let db = dir.join("history.db");
    if !db.exists() {
        bail!("archive invalide : history.db manquant");
    }
    let config = {
        let c = dir.join("config.toml");
        c.exists().then_some(c)
    };
    let metadata = {
        let m = dir.join("metadata.json");
        if m.exists() {
            let raw = fs::read_to_string(&m)?;
            serde_json::from_str::<BackupMetadata>(&raw).ok()
        } else {
            None
        }
    };

    Ok(ExtractedBackup {
        dir,
        db,
        config,
        metadata,
    })
}

/// Valide que la base extraite est saine et compatible.
fn validate_db(db_path: &Path) -> Result<i64> {
    let conn = db::open_readonly(db_path)
        .with_context(|| "la base restaurée n'est pas ouvrable (archive invalide)")?;
    if !db::table_exists(&conn, "commands")? {
        bail!("archive invalide : table `commands` absente");
    }
    let schema = migrations::schema_version(&conn)?;
    if schema > migrations::SCHEMA_VERSION {
        bail!(
            "archive incompatible : schéma v{} > schéma supporté v{} (mnemo trop ancien)",
            schema,
            migrations::SCHEMA_VERSION
        );
    }
    Ok(schema)
}

/// Remplace un fichier de façon atomique (copie vers un `.tmp` puis renommage).
fn replace_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dst.with_extension("mnemo-tmp");
    fs::copy(src, &tmp).with_context(|| format!("copie vers {}", tmp.display()))?;
    fs::rename(&tmp, dst).with_context(|| format!("remplacement de {}", dst.display()))?;
    Ok(())
}

/// Point d'entrée de `mnemo restore`.
pub fn restore_run(archive: &Path, dry_run: bool, assume_yes: bool) -> Result<()> {
    let extracted = extract_archive(archive)?;
    let schema = validate_db(&extracted.db)?;

    // Aperçu de ce qui sera restauré.
    println!("Archive : {}", archive.display());
    if let Some(meta) = &extracted.metadata {
        println!("  Version mnemo    : {}", meta.mnemo_version);
        println!("  Date sauvegarde  : {}", meta.created_at);
        println!("  Commandes        : {}", meta.command_count);
    }
    println!("  Version schéma   : {schema}");
    println!(
        "  Configuration    : {}",
        if extracted.config.is_some() {
            "incluse"
        } else {
            "absente"
        }
    );
    println!("Cibles :");
    println!("  DB     -> {}", config::db_path()?.display());
    println!("  Config -> {}", config::config_path()?.display());

    if dry_run {
        println!("\n[dry-run] Aucune modification effectuée.");
        return Ok(());
    }

    if !confirm::confirm(
        "Restaurer cette sauvegarde remplacera la base et la config actuelles. Continuer ?",
        assume_yes,
    )? {
        println!("Restauration annulée.");
        return Ok(());
    }

    // Sauvegarde automatique de l'état courant avant remplacement.
    let safety = create_backup(None)?;
    println!("Sauvegarde de sécurité créée : {}", safety.path.display());

    replace_file(&extracted.db, &config::db_path()?)?;
    if let Some(cfg) = &extracted.config {
        replace_file(cfg, &config::config_path()?)?;
    }

    println!("Restauration terminée.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nom_archive_bien_forme() {
        assert_eq!(
            archive_filename("2026-06-14 12:34:56"),
            "mnemo-backup-20260614-123456.tar.gz"
        );
    }

    #[test]
    fn iso_bien_forme() {
        assert_eq!(iso_from("2026-06-14 12:34:56"), "2026-06-14T12:34:56Z");
    }
}
