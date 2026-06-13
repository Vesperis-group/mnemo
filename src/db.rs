use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Données nécessaires pour insérer une nouvelle commande.
#[derive(Debug, Clone)]
pub struct NewCommand {
    pub command: String,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub hostname: Option<String>,
    pub exit_code: Option<i64>,
    pub created_at: String,
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
}

/// Ouvre (ou crée) la base SQLite sur disque et initialise le schéma.
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("création du dossier {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("ouverture de la base {}", path.display()))?;
    init_schema(&conn)?;
    Ok(conn)
}

/// Base SQLite en mémoire (utilisée pour les tests).
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    init_schema(&conn)?;
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

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS commands (
            id          INTEGER PRIMARY KEY,
            command     TEXT NOT NULL,
            cwd         TEXT,
            shell       TEXT,
            hostname    TEXT,
            exit_code   INTEGER,
            created_at  TEXT NOT NULL,
            hash        TEXT UNIQUE
        );
        CREATE INDEX IF NOT EXISTS idx_commands_created_at ON commands(created_at);",
    )?;
    Ok(())
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
            (command, cwd, shell, hostname, exit_code, created_at, hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            cmd.command,
            cmd.cwd,
            cmd.shell,
            cmd.hostname,
            cmd.exit_code,
            cmd.created_at,
            hash,
        ],
    )?;
    Ok(changed > 0)
}

/// Charge les commandes les plus récentes (limite paramétrable).
pub fn fetch_all(conn: &Connection, limit: usize) -> Result<Vec<CommandRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, command, cwd, shell, hostname, exit_code, created_at
         FROM commands
         ORDER BY created_at DESC, id DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |row| {
        Ok(CommandRecord {
            id: row.get(0)?,
            command: row.get(1)?,
            cwd: row.get(2)?,
            shell: row.get(3)?,
            hostname: row.get(4)?,
            exit_code: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
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
}
