//! Versioned schema initialization and migration preservation rules.

use rusqlite::{Connection, OpenFlags, Transaction};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use crate::schema::{
    create_v1_schema, integrity_check, verify_schema_checksum, verify_schema_shape, SCHEMA_VERSION,
};
use crate::{Result, StoreError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationReport {
    pub migrated: bool,
    pub backup_path: Option<PathBuf>,
}

pub fn migration_backup_path(database: impl AsRef<Path>) -> PathBuf {
    let database = database.as_ref();
    let mut path = database.as_os_str().to_os_string();
    path.push(".migration.bak");
    PathBuf::from(path)
}

pub(crate) fn open_connection(path: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(StoreError::from_sqlite)?;
    Ok(connection)
}

pub(crate) fn configure_writer(connection: &Connection) -> Result<()> {
    // journal_mode and auto_vacuum are intentionally set before schema work.
    // execute_batch is used for journal_mode because SQLite returns a result
    // row for that PRAGMA on some versions.
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA auto_vacuum = INCREMENTAL;
             PRAGMA wal_autocheckpoint = 1000;",
        )
        .map_err(StoreError::from_sqlite)?;
    connection
        .busy_timeout(std::time::Duration::from_millis(1_000))
        .map_err(StoreError::from_sqlite)?;
    Ok(())
}

pub(crate) fn configure_reader(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "PRAGMA query_only = ON;
             PRAGMA foreign_keys = ON;",
        )
        .map_err(StoreError::from_sqlite)?;
    connection
        .busy_timeout(std::time::Duration::from_millis(250))
        .map_err(StoreError::from_sqlite)?;
    Ok(())
}

pub(crate) fn initialize_or_migrate(
    connection: &mut Connection,
    database: &Path,
) -> Result<MigrationReport> {
    let has_migrations = table_exists(connection, "schema_migrations")?;
    if !has_migrations {
        if has_product_tables(connection)? {
            return Err(StoreError::Corrupt {
                path: Some(database.to_path_buf()),
                reason: "schema_migrations table is missing".to_owned(),
            });
        }
        apply_schema(connection, database, false)
    } else {
        let latest = latest_schema_version(connection)?;
        if latest > SCHEMA_VERSION as i64 {
            return Err(StoreError::IncompatibleSchema {
                found: latest as u32,
            });
        }
        if latest == SCHEMA_VERSION as i64 {
            verify_schema_checksum(connection)
                .map_err(|error| attach_database_path(error, database))?;
            Ok(MigrationReport {
                migrated: false,
                backup_path: None,
            })
        } else if latest < 0 {
            Err(StoreError::Corrupt {
                path: Some(database.to_path_buf()),
                reason: "negative schema version".to_owned(),
            })
        } else {
            apply_schema(connection, database, true)
        }
    }
}

fn apply_schema(
    connection: &mut Connection,
    database: &Path,
    migration: bool,
) -> Result<MigrationReport> {
    let backup = if migration && database.exists() && fs::metadata(database)?.len() > 0 {
        checkpoint(connection)?;
        Some(create_backup(database)?)
    } else {
        None
    };

    let transaction = connection.transaction().map_err(StoreError::from_sqlite)?;
    let result = apply_schema_transaction(&transaction);
    if let Err(error) = result {
        let _ = transaction.rollback();
        if let Some(backup_path) = backup {
            return Err(StoreError::MigrationFailed {
                backup_path,
                reason: error.to_string(),
            });
        }
        return Err(attach_database_path(error, database));
    }
    transaction.commit().map_err(StoreError::from_sqlite)?;

    if let Err(error) = integrity_check(connection) {
        if let Some(backup_path) = backup {
            return Err(StoreError::MigrationFailed {
                backup_path,
                reason: error.to_string(),
            });
        }
        return Err(attach_database_path(error, database));
    }

    if let Some(backup_path) = &backup {
        // A successful migration has passed the integrity check.  The source
        // backup is only a recovery artifact and can now be removed.
        let _ = fs::remove_file(backup_path);
    }
    Ok(MigrationReport {
        migrated: migration,
        backup_path: backup,
    })
}

fn apply_schema_transaction(transaction: &Transaction<'_>) -> Result<()> {
    create_v1_schema(transaction)?;
    verify_schema_shape(transaction)?;
    integrity_check(transaction)?;
    Ok(())
}

fn create_backup(database: &Path) -> Result<PathBuf> {
    let preferred = migration_backup_path(database);
    let backup = if !preferred.exists() {
        preferred
    } else {
        let mut index = 1u32;
        loop {
            let mut candidate = preferred.as_os_str().to_os_string();
            candidate.push(format!(".{index}"));
            let candidate = PathBuf::from(candidate);
            if !candidate.exists() {
                break candidate;
            }
            index += 1;
        }
    };

    // create_new avoids overwriting an earlier preserved recovery copy.
    let _file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|error| StoreError::Io(error))?;
    drop(_file);
    if let Err(error) = fs::copy(database, &backup) {
        let _ = fs::remove_file(&backup);
        return Err(StoreError::Io(error));
    }
    Ok(backup)
}

fn checkpoint(connection: &Connection) -> Result<()> {
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(StoreError::from_sqlite)
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
             )",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(StoreError::from_sqlite)
}

fn has_product_tables(connection: &Connection) -> Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             )",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(StoreError::from_sqlite)
}

fn latest_schema_version(connection: &Connection) -> Result<i64> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(StoreError::from_sqlite)
}

fn attach_database_path(error: StoreError, database: &Path) -> StoreError {
    match error {
        StoreError::Corrupt { reason, .. } => StoreError::Corrupt {
            path: Some(database.to_path_buf()),
            reason,
        },
        other => other,
    }
}

/// Creates a syntactically valid v0 shell for tests and future migration
/// development.  It is not a production schema and is intentionally private
/// to the crate's test support.
#[cfg(test)]
pub(crate) fn create_legacy_v0(path: &Path) -> Result<()> {
    let connection = Connection::open(path).map_err(StoreError::from_sqlite)?;
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations(
                version INTEGER PRIMARY KEY,
                checksum TEXT NOT NULL,
                applied_at_utc TEXT NOT NULL
             );
             INSERT INTO schema_migrations(version, checksum, applied_at_utc)
             VALUES (0, 'legacy-v0', '1970-01-01T00:00:00.000Z');",
        )
        .map_err(StoreError::from_sqlite)?;
    Ok(())
}
