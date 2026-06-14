//! Export des commandes (`mnemo export`) en JSON ou CSV.
//!
//! Réutilise les filtres de contexte Git `--project` / `--branch`. Sans
//! `--output`, l'export est écrit sur stdout ; sinon dans le fichier indiqué.

use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;

use crate::config;
use crate::db::{self, CommandRecord, SearchFilter};

/// Format d'export demandé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
}

/// Ligne d'export sérialisable (tous les champs utiles d'une commande).
#[derive(Serialize)]
struct ExportRow<'a> {
    id: i64,
    command: &'a str,
    cwd: Option<&'a str>,
    shell: Option<&'a str>,
    hostname: Option<&'a str>,
    exit_code: Option<i64>,
    created_at: &'a str,
    git_root: Option<&'a str>,
    git_branch: Option<&'a str>,
    git_remote: Option<&'a str>,
    session_id: Option<&'a str>,
}

impl<'a> From<&'a CommandRecord> for ExportRow<'a> {
    fn from(r: &'a CommandRecord) -> Self {
        ExportRow {
            id: r.id,
            command: &r.command,
            cwd: r.cwd.as_deref(),
            shell: r.shell.as_deref(),
            hostname: r.hostname.as_deref(),
            exit_code: r.exit_code,
            created_at: &r.created_at,
            git_root: r.git_root.as_deref(),
            git_branch: r.git_branch.as_deref(),
            git_remote: r.git_remote.as_deref(),
            session_id: r.session_id.as_deref(),
        }
    }
}

/// Point d'entrée de `mnemo export`.
pub fn run(
    format: ExportFormat,
    project: Option<String>,
    branch: Option<String>,
    output: Option<PathBuf>,
    gzip: bool,
) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;
    let filter = SearchFilter { project, branch };
    let records = db::all_commands(&conn, &filter)?;

    let content = match format {
        ExportFormat::Json => render_json(&records)?,
        ExportFormat::Csv => render_csv(&records),
    };

    match output {
        Some(path) => {
            let path = if gzip { gz_path(path) } else { path };
            let bytes = if gzip {
                gzip_bytes(content.as_bytes())?
            } else {
                content.into_bytes()
            };
            std::fs::write(&path, &bytes)
                .with_context(|| format!("écriture de l'export {}", path.display()))?;
            eprintln!(
                "Export écrit dans {} ({} commandes).",
                path.display(),
                records.len()
            );
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            if gzip {
                stdout.write_all(&gzip_bytes(content.as_bytes())?)?;
            } else {
                stdout.write_all(content.as_bytes())?;
            }
        }
    }
    Ok(())
}

/// Ajoute l'extension `.gz` à un chemin de sortie si elle est absente.
fn gz_path(path: PathBuf) -> PathBuf {
    match path.extension().and_then(|e| e.to_str()) {
        Some("gz") => path,
        _ => {
            let mut name = path.into_os_string();
            name.push(".gz");
            PathBuf::from(name)
        }
    }
}

/// Compresse des octets au format gzip.
fn gzip_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

/// Sérialise les commandes en tableau JSON.
fn render_json(records: &[CommandRecord]) -> Result<String> {
    let rows: Vec<ExportRow> = records.iter().map(ExportRow::from).collect();
    Ok(serde_json::to_string_pretty(&rows)?)
}

/// Sérialise une sélection de commandes en JSON stable (réutilisé par
/// `mnemo search --print --json`).
pub fn records_to_json(records: &[&CommandRecord]) -> Result<String> {
    let rows: Vec<ExportRow> = records.iter().map(|r| ExportRow::from(*r)).collect();
    Ok(serde_json::to_string_pretty(&rows)?)
}

/// En-têtes CSV, dans l'ordre des colonnes.
const CSV_HEADERS: &[&str] = &[
    "id",
    "command",
    "cwd",
    "shell",
    "hostname",
    "exit_code",
    "created_at",
    "git_root",
    "git_branch",
    "git_remote",
    "session_id",
];

/// Sérialise les commandes en CSV (RFC 4180 : guillemets doublés, champs
/// contenant `,`/`"`/saut de ligne entre guillemets).
fn render_csv(records: &[CommandRecord]) -> String {
    let mut out = String::new();
    out.push_str(&CSV_HEADERS.join(","));
    out.push('\n');

    for r in records {
        let fields = [
            r.id.to_string(),
            r.command.clone(),
            r.cwd.clone().unwrap_or_default(),
            r.shell.clone().unwrap_or_default(),
            r.hostname.clone().unwrap_or_default(),
            r.exit_code.map(|c| c.to_string()).unwrap_or_default(),
            r.created_at.clone(),
            r.git_root.clone().unwrap_or_default(),
            r.git_branch.clone().unwrap_or_default(),
            r.git_remote.clone().unwrap_or_default(),
            r.session_id.clone().unwrap_or_default(),
        ];
        let escaped: Vec<String> = fields.iter().map(|f| csv_escape(f)).collect();
        out.push_str(&escaped.join(","));
        out.push('\n');
    }
    out
}

/// Échappe un champ CSV selon RFC 4180.
fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_echappe_virgules_et_guillemets() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("dit \"bonjour\""), "\"dit \"\"bonjour\"\"\"");
        assert_eq!(csv_escape("ligne1\nligne2"), "\"ligne1\nligne2\"");
    }

    #[test]
    fn csv_a_un_entete_propre() {
        let out = render_csv(&[]);
        assert_eq!(
            out.trim(),
            "id,command,cwd,shell,hostname,exit_code,created_at,git_root,git_branch,git_remote,session_id"
        );
    }

    #[test]
    fn gzip_roundtrip_conserve_le_contenu() {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let original = b"id,command\n1,ls -la\n";
        let compressed = gzip_bytes(original).unwrap();
        assert_ne!(compressed, original, "les octets doivent être compressés");

        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut restored = Vec::new();
        decoder.read_to_end(&mut restored).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn gz_path_ajoute_extension_si_absente() {
        assert_eq!(gz_path(PathBuf::from("e.json")), PathBuf::from("e.json.gz"));
        assert_eq!(
            gz_path(PathBuf::from("e.json.gz")),
            PathBuf::from("e.json.gz")
        );
    }
}
