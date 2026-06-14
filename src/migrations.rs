//! Système de migrations du schéma SQLite, basé sur `PRAGMA user_version`.
//!
//! Principes :
//! - Chaque base porte une version de schéma entière (`PRAGMA user_version`).
//!   SQLite vaut `0` par défaut : c'est le cas des bases historiques de mnemo
//!   (créées avant l'introduction des migrations) **et** des bases neuves.
//! - Les migrations sont appliquées en séquence, de la version courante jusqu'à
//!   [`SCHEMA_VERSION`]. Chaque étape est **idempotente** et **non
//!   destructive** : on ne supprime ni ne réécrit jamais de données existantes.
//! - `mnemo doctor` lit la version sans jamais migrer (lecture seule).
//!
//! Historique des versions :
//! - `1` : schéma de base (table `commands`).
//! - `2` : ajout du contexte Git (`git_root`, `git_branch`, `git_remote`,
//!   `session_id`).

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Version cible du schéma. Toute base ouverte par mnemo est migrée jusqu'ici.
pub const SCHEMA_VERSION: i64 = 2;

/// Résultat d'une application de migrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// Version du schéma avant migration.
    pub from: i64,
    /// Version du schéma après migration (== [`SCHEMA_VERSION`]).
    pub to: i64,
}

impl Outcome {
    /// Vrai si au moins une migration a effectivement été appliquée.
    pub fn migrated(&self) -> bool {
        self.from != self.to
    }
}

/// Lit la version de schéma stockée dans la base (`PRAGMA user_version`).
pub fn schema_version(conn: &Connection) -> Result<i64> {
    let v: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("lecture de PRAGMA user_version")?;
    Ok(v)
}

/// Écrit la version de schéma dans la base.
fn set_schema_version(conn: &Connection, version: i64) -> Result<()> {
    // `PRAGMA user_version` n'accepte pas de paramètre lié : on formate un
    // entier validé, sans interpolation de donnée externe.
    conn.execute_batch(&format!("PRAGMA user_version = {version};"))
        .context("écriture de PRAGMA user_version")?;
    Ok(())
}

/// Applique toutes les migrations en attente jusqu'à [`SCHEMA_VERSION`].
///
/// Renvoie les versions avant/après. Sûr à appeler à chaque ouverture de base :
/// si la base est déjà à jour, aucune écriture n'est effectuée.
pub fn apply(conn: &Connection) -> Result<Outcome> {
    let from = schema_version(conn)?;

    if from > SCHEMA_VERSION {
        anyhow::bail!(
            "base créée par une version plus récente de mnemo (schéma v{from}, \
             cette version gère v{SCHEMA_VERSION}). Mettez mnemo à jour."
        );
    }

    let mut version = from;
    while version < SCHEMA_VERSION {
        match version {
            0 => migrate_0_to_1(conn)?,
            1 => migrate_1_to_2(conn)?,
            other => anyhow::bail!("aucune migration définie pour le schéma v{other}"),
        }
        version += 1;
        set_schema_version(conn, version)?;
    }

    Ok(Outcome {
        from,
        to: SCHEMA_VERSION,
    })
}

/// v0 → v1 : schéma de base. `CREATE TABLE IF NOT EXISTS` rend l'étape
/// idempotente, y compris pour les bases historiques qui possèdent déjà la
/// table `commands` mais dont `user_version` valait encore `0`.
fn migrate_0_to_1(conn: &Connection) -> Result<()> {
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
    )
    .context("migration v0 -> v1 (schéma de base)")?;
    Ok(())
}

/// v1 → v2 : ajout des colonnes de contexte Git. Les colonnes sont ajoutées une
/// à une et uniquement si elles sont absentes, ce qui rend l'étape idempotente
/// et tolérante à une migration partielle interrompue. Aucune donnée existante
/// n'est touchée : les nouvelles colonnes valent `NULL` pour les lignes déjà
/// présentes.
fn migrate_1_to_2(conn: &Connection) -> Result<()> {
    for column in ["git_root", "git_branch", "git_remote", "session_id"] {
        if !column_exists(conn, "commands", column)? {
            conn.execute_batch(&format!("ALTER TABLE commands ADD COLUMN {column} TEXT;"))
                .with_context(|| format!("migration v1 -> v2 (ajout colonne {column})"))?;
        }
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_commands_git_root ON commands(git_root);
         CREATE INDEX IF NOT EXISTS idx_commands_git_branch ON commands(git_branch);",
    )
    .context("migration v1 -> v2 (index Git)")?;
    Ok(())
}

/// Indique si une colonne existe dans une table (via `PRAGMA table_info`).
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Construit une base « héritée » au schéma v1 : table `commands` sans les
    /// colonnes Git, et `user_version` laissé à 0 (comme les bases d'avant les
    /// migrations).
    fn legacy_v1_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE commands (
                id          INTEGER PRIMARY KEY,
                command     TEXT NOT NULL,
                cwd         TEXT,
                shell       TEXT,
                hostname    TEXT,
                exit_code   INTEGER,
                created_at  TEXT NOT NULL,
                hash        TEXT UNIQUE
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO commands (command, cwd, created_at, hash)
             VALUES ('ls -la', '/tmp', '2026-06-13 10:00:00', 'deadbeef')",
            [],
        )
        .unwrap();
        conn
    }

    fn has_column(conn: &Connection, column: &str) -> bool {
        column_exists(conn, "commands", column).unwrap()
    }

    #[test]
    fn migration_v1_vers_v2_ajoute_les_colonnes_git() {
        let conn = legacy_v1_db();
        assert_eq!(schema_version(&conn).unwrap(), 0);

        let outcome = apply(&conn).unwrap();
        assert_eq!(outcome.from, 0);
        assert_eq!(outcome.to, SCHEMA_VERSION);
        assert!(outcome.migrated());
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);

        for col in ["git_root", "git_branch", "git_remote", "session_id"] {
            assert!(has_column(&conn, col), "colonne {col} attendue");
        }
    }

    #[test]
    fn ancienne_base_reste_utilisable_apres_migration() {
        let conn = legacy_v1_db();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, 1);

        apply(&conn).unwrap();

        // La donnée historique est toujours là, colonnes Git à NULL.
        let after: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(after, 1);
        let git_root: Option<String> = conn
            .query_row(
                "SELECT git_root FROM commands WHERE command = 'ls -la'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(git_root.is_none());
    }

    #[test]
    fn migration_idempotente() {
        let conn = legacy_v1_db();
        let first = apply(&conn).unwrap();
        assert!(first.migrated());

        // Deuxième passage : aucune migration, aucune erreur.
        let second = apply(&conn).unwrap();
        assert_eq!(second.from, SCHEMA_VERSION);
        assert_eq!(second.to, SCHEMA_VERSION);
        assert!(!second.migrated());

        // Troisième passage pour être sûr.
        let third = apply(&conn).unwrap();
        assert!(!third.migrated());
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn base_neuve_atteint_la_version_cible() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 0);
        let outcome = apply(&conn).unwrap();
        assert_eq!(outcome.to, SCHEMA_VERSION);
        for col in ["git_root", "git_branch", "git_remote", "session_id"] {
            assert!(has_column(&conn, col));
        }
    }

    #[test]
    fn base_plus_recente_est_refusee() {
        let conn = Connection::open_in_memory().unwrap();
        set_schema_version(&conn, SCHEMA_VERSION + 1).unwrap();
        let err = apply(&conn).unwrap_err();
        assert!(err.to_string().contains("version plus récente"));
    }
}
