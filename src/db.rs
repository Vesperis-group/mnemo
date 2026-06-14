use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::migrations;

/// Données nécessaires pour insérer une nouvelle commande.
#[derive(Debug, Clone, Default)]
pub struct NewCommand {
    pub command: String,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub hostname: Option<String>,
    pub exit_code: Option<i64>,
    pub created_at: String,
    /// Racine du dépôt Git (`git rev-parse --show-toplevel`), si applicable.
    pub git_root: Option<String>,
    /// Branche Git courante, si applicable.
    pub git_branch: Option<String>,
    /// URL du remote `origin`, si applicable.
    pub git_remote: Option<String>,
    /// Identifiant de session shell, si fourni (`MNEMO_SESSION_ID`).
    pub session_id: Option<String>,
}

/// Commande lue depuis la base.
///
/// Certains champs (id, shell, hostname, exit_code) font partie du modèle de
/// données mais ne sont pas tous affichés par le MVP de la TUI.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub id: i64,
    pub command: String,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub hostname: Option<String>,
    pub exit_code: Option<i64>,
    pub created_at: String,
    pub git_root: Option<String>,
    pub git_branch: Option<String>,
    pub git_remote: Option<String>,
    pub session_id: Option<String>,
}

/// Filtre optionnel appliqué à la recherche (contexte Git).
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Filtre sur le projet : nom du dossier racine Git ou chemin `git_root`.
    pub project: Option<String>,
    /// Filtre sur la branche Git.
    pub branch: Option<String>,
}

impl SearchFilter {
    /// Vrai si aucun critère n'est défini.
    pub fn is_empty(&self) -> bool {
        self.project.is_none() && self.branch.is_none()
    }
}

/// Ouvre (ou crée) la base SQLite sur disque et initialise le schéma.
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("création du dossier {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("ouverture de la base {}", path.display()))?;
    migrations::apply(&conn)?;
    Ok(conn)
}

/// Ouvre la base et renvoie aussi le résultat des migrations appliquées.
/// Utilisé par `mnemo migrate` pour rendre compte de la transition de schéma.
pub fn open_and_migrate(path: &Path) -> Result<(Connection, migrations::Outcome)> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("création du dossier {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("ouverture de la base {}", path.display()))?;
    let outcome = migrations::apply(&conn)?;
    Ok((conn, outcome))
}

/// Base SQLite en mémoire (utilisée pour les tests).
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrations::apply(&conn)?;
    Ok(conn)
}

/// Ouvre une base existante en lecture seule, SANS créer ni modifier le schéma.
/// Utilisé par `mnemo doctor` pour ne jamais altérer le système.
pub fn open_readonly(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", path.display()))?;
    Ok(conn)
}

/// Indique si une table donnée existe dans la base.
pub fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

/// Hash FNV-1a 64 bits, déterministe, utilisé pour le dédoublonnage.
///
/// Le hash combine la commande et le répertoire courant : une même commande
/// dans deux répertoires différents n'est donc pas considérée comme doublon.
pub fn compute_hash(command: &str, cwd: Option<&str>) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for b in command.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    // Séparateur explicite entre commande et cwd.
    hash ^= 0x1f;
    hash = hash.wrapping_mul(FNV_PRIME);
    if let Some(cwd) = cwd {
        for b in cwd.bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    format!("{hash:016x}")
}

/// Insère une commande. Retourne `true` si elle a été insérée, `false` si
/// c'était un doublon (même hash déjà présent).
pub fn insert_command(conn: &Connection, cmd: &NewCommand) -> Result<bool> {
    let hash = compute_hash(&cmd.command, cmd.cwd.as_deref());
    let changed = conn.execute(
        "INSERT OR IGNORE INTO commands
            (command, cwd, shell, hostname, exit_code, created_at, hash,
             git_root, git_branch, git_remote, session_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            cmd.command,
            cmd.cwd,
            cmd.shell,
            cmd.hostname,
            cmd.exit_code,
            cmd.created_at,
            hash,
            cmd.git_root,
            cmd.git_branch,
            cmd.git_remote,
            cmd.session_id,
        ],
    )?;
    Ok(changed > 0)
}

/// Charge les commandes les plus récentes (limite paramétrable).
#[allow(dead_code)]
pub fn fetch_all(conn: &Connection, limit: usize) -> Result<Vec<CommandRecord>> {
    fetch_filtered(conn, &SearchFilter::default(), limit)
}

/// Charge les commandes les plus récentes en appliquant un filtre de contexte
/// Git optionnel (projet / branche). Le filtrage fuzzy sur le texte de la
/// commande reste à la charge de l'appelant.
pub fn fetch_filtered(
    conn: &Connection,
    filter: &SearchFilter,
    limit: usize,
) -> Result<Vec<CommandRecord>> {
    // `project` correspond soit au chemin complet `git_root`, soit au nom du
    // dossier racine (dernier segment du chemin).
    let project_suffix = filter.project.as_ref().map(|p| format!("%/{p}"));
    let mut stmt = conn.prepare(
        "SELECT id, command, cwd, shell, hostname, exit_code, created_at,
                git_root, git_branch, git_remote, session_id
         FROM commands
         WHERE (?1 IS NULL OR git_branch = ?1)
           AND (?2 IS NULL OR git_root = ?2 OR git_root LIKE ?3)
         ORDER BY created_at DESC, id DESC
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![filter.branch, filter.project, project_suffix, limit as i64],
        row_to_record,
    )?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Charge toutes les commandes correspondant au filtre Git (sans limite), pour
/// le calcul des statistiques. Un filtre vide renvoie l'intégralité de la base.
pub fn all_commands(conn: &Connection, filter: &SearchFilter) -> Result<Vec<CommandRecord>> {
    let project_suffix = filter.project.as_ref().map(|p| format!("%/{p}"));
    let mut stmt = conn.prepare(
        "SELECT id, command, cwd, shell, hostname, exit_code, created_at,
                git_root, git_branch, git_remote, session_id
         FROM commands
         WHERE (?1 IS NULL OR git_branch = ?1)
           AND (?2 IS NULL OR git_root = ?2 OR git_root LIKE ?3)
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![filter.branch, filter.project, project_suffix],
        row_to_record,
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Convertit une ligne SQL en [`CommandRecord`].
fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<CommandRecord> {
    Ok(CommandRecord {
        id: row.get(0)?,
        command: row.get(1)?,
        cwd: row.get(2)?,
        shell: row.get(3)?,
        hostname: row.get(4)?,
        exit_code: row.get(5)?,
        created_at: row.get(6)?,
        git_root: row.get(7)?,
        git_branch: row.get(8)?,
        git_remote: row.get(9)?,
        session_id: row.get(10)?,
    })
}

/// Nombre total de commandes stockées.
pub fn count(conn: &Connection) -> Result<i64> {
    let n = conn.query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))?;
    Ok(n)
}

/// Horodatage courant au format `YYYY-MM-DD HH:MM:SS` (UTC).
pub fn now_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_timestamp(secs)
}

/// Convertit un timestamp Unix (secondes UTC) en `YYYY-MM-DD HH:MM:SS`.
pub fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {hour:02}:{min:02}:{sec:02}")
}

/// Algorithme de Howard Hinnant : jours depuis l'époque -> (année, mois, jour).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_stable_et_distingue_le_cwd() {
        let a = compute_hash("ls -la", Some("/home"));
        let b = compute_hash("ls -la", Some("/home"));
        let c = compute_hash("ls -la", Some("/tmp"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn insertion_et_dedoublonnage() {
        let conn = open_in_memory().unwrap();
        let cmd = NewCommand {
            command: "echo hi".into(),
            cwd: Some("/tmp".into()),
            shell: Some("bash".into()),
            hostname: Some("host".into()),
            exit_code: Some(0),
            created_at: now_timestamp(),
            ..Default::default()
        };
        assert!(insert_command(&conn, &cmd).unwrap());
        // Même hash -> doublon ignoré.
        assert!(!insert_command(&conn, &cmd).unwrap());
        assert_eq!(count(&conn).unwrap(), 1);
    }

    #[test]
    fn fetch_renvoie_les_commandes() {
        let conn = open_in_memory().unwrap();
        for c in ["a", "b", "c"] {
            insert_command(
                &conn,
                &NewCommand {
                    command: c.into(),
                    cwd: None,
                    shell: None,
                    hostname: None,
                    exit_code: None,
                    created_at: now_timestamp(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let all = fetch_all(&conn, 100).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn format_timestamp_connu() {
        // 1609459200 = 2021-01-01 00:00:00 UTC
        assert_eq!(format_timestamp(1_609_459_200), "2021-01-01 00:00:00");
        // 0 = 1970-01-01 00:00:00 UTC
        assert_eq!(format_timestamp(0), "1970-01-01 00:00:00");
    }

    #[test]
    fn fetch_filtered_par_projet_et_branche() {
        let conn = open_in_memory().unwrap();
        let insert = |command: &str, root: &str, branch: &str| {
            insert_command(
                &conn,
                &NewCommand {
                    command: command.into(),
                    cwd: Some(root.into()),
                    created_at: now_timestamp(),
                    git_root: Some(root.into()),
                    git_branch: Some(branch.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        };
        insert("cargo build", "/home/u/proj/mnemo", "main");
        insert("cargo test", "/home/u/proj/mnemo", "dev");
        insert("ls", "/home/u/proj/autre", "main");

        // Filtre par nom de projet (dernier segment du chemin).
        let by_name = fetch_filtered(
            &conn,
            &SearchFilter {
                project: Some("mnemo".into()),
                branch: None,
            },
            100,
        )
        .unwrap();
        assert_eq!(by_name.len(), 2);
        assert!(by_name
            .iter()
            .all(|r| r.git_root.as_deref() == Some("/home/u/proj/mnemo")));

        // Filtre par chemin git_root complet.
        let by_path = fetch_filtered(
            &conn,
            &SearchFilter {
                project: Some("/home/u/proj/autre".into()),
                branch: None,
            },
            100,
        )
        .unwrap();
        assert_eq!(by_path.len(), 1);

        // Filtre par branche.
        let by_branch = fetch_filtered(
            &conn,
            &SearchFilter {
                project: None,
                branch: Some("main".into()),
            },
            100,
        )
        .unwrap();
        assert_eq!(by_branch.len(), 2);

        // Combinaison projet + branche.
        let both = fetch_filtered(
            &conn,
            &SearchFilter {
                project: Some("mnemo".into()),
                branch: Some("dev".into()),
            },
            100,
        )
        .unwrap();
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].command, "cargo test");
    }
}
