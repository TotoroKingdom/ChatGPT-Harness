//! Offline maintenance operations guarded by the same writer lock.

use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};

use crate::migration::{configure_writer, initialize_or_migrate, open_connection};
use crate::retention::file_footprint_bytes;
use crate::schema::verify_schema_checksum;
use crate::writer::WriterLock;
use crate::{now_utc, HistoryClearResult, Result, StoreError, StorePaths};

#[derive(Debug)]
pub struct OfflineMaintenance {
    database: PathBuf,
    _lock: WriterLock,
    connection: Option<Connection>,
}

impl OfflineMaintenance {
    pub fn open(database: impl AsRef<Path>) -> Result<Self> {
        let paths = StorePaths::from_database(database);
        if let Some(parent) = paths.database.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock = WriterLock::try_acquire(&paths.lock)?;
        if !paths.database.exists() {
            return Ok(Self {
                database: paths.database,
                _lock: lock,
                connection: None,
            });
        }
        let had_source = fs::metadata(&paths.database)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false);
        let mut connection = open_connection(&paths.database)?;
        if had_source {
            // Explicit offline maintenance may operate on compatible v1
            // History, but normal v1->v2 migration belongs to Runtime.
            verify_schema_checksum(&connection)?;
            // Keep the existing journal/schema untouched until an explicit
            // maintenance operation writes data. These are connection-local.
            connection.execute_batch("PRAGMA foreign_keys = ON;")?;
            connection.busy_timeout(std::time::Duration::from_millis(1_000))?;
        } else {
            configure_writer(&connection)?;
            initialize_or_migrate(&mut connection, &paths.database)?;
        }
        Ok(Self {
            database: paths.database,
            _lock: lock,
            connection: Some(connection),
        })
    }

    pub fn database_path(&self) -> &Path {
        &self.database
    }

    pub fn clear_all(&mut self) -> Result<HistoryClearResult> {
        let Some(connection) = self.connection.as_mut() else {
            return Ok(HistoryClearResult {
                deleted_request_count: 0,
                deleted_event_count: 0,
                deleted_approval_count: 0,
                footprint_bytes: 0,
                completed_at_utc: now_utc(),
            });
        };
        let completed_at_utc = now_utc();
        let transaction = connection.transaction().map_err(StoreError::from_sqlite)?;
        let deleted_approval_count: u64 =
            transaction.query_row("SELECT COUNT(*) FROM approval_history", [], |row| {
                row.get::<_, i64>(0)
            })? as u64;
        let deleted_event_count: u64 =
            transaction.query_row("SELECT COUNT(*) FROM evidence_events", [], |row| {
                row.get::<_, i64>(0)
            })? as u64;
        let deleted_request_count: u64 =
            transaction.query_row("SELECT COUNT(*) FROM request_summary", [], |row| {
                row.get::<_, i64>(0)
            })? as u64;
        transaction.execute("DELETE FROM approval_history", [])?;
        transaction.execute("DELETE FROM evidence_events", [])?;
        transaction.execute("DELETE FROM request_summary", [])?;
        transaction.execute("DELETE FROM runtime_sessions", [])?;
        transaction.execute(
            "UPDATE evidence_health
             SET last_clear_at_utc = ?1, last_maintenance_at_utc = ?1
             WHERE id = 1",
            params![completed_at_utc],
        )?;
        transaction.commit().map_err(StoreError::from_sqlite)?;
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA incremental_vacuum;")
            .map_err(StoreError::from_sqlite)?;
        Ok(HistoryClearResult {
            deleted_request_count,
            deleted_event_count,
            deleted_approval_count,
            footprint_bytes: file_footprint_bytes(&self.database)?,
            completed_at_utc,
        })
    }
}

pub fn clear_history_offline(database: impl AsRef<Path>) -> Result<HistoryClearResult> {
    let mut maintenance = OfflineMaintenance::open(database)?;
    maintenance.clear_all()
}

/// Deletes only terminal/incomplete request families, retaining open rows and
/// the current session for an online clear barrier.
pub(crate) fn clear_terminal_connection(
    connection: &mut Connection,
    database: &Path,
) -> Result<HistoryClearResult> {
    let completed_at_utc = now_utc();
    let transaction = connection.transaction().map_err(StoreError::from_sqlite)?;
    let mut statement = transaction.prepare(
        "SELECT runtime_id, request_id FROM request_summary
         WHERE terminal_state <> 'open'",
    )?;
    let keys = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<(String, String)>, _>>()?;
    drop(statement);
    let mut result = HistoryClearResult {
        deleted_request_count: 0,
        deleted_event_count: 0,
        deleted_approval_count: 0,
        footprint_bytes: 0,
        completed_at_utc: completed_at_utc.clone(),
    };
    for (runtime_id, request_id) in keys {
        result.deleted_approval_count += transaction.query_row(
            "SELECT COUNT(*) FROM approval_history
             WHERE runtime_id = ?1 AND request_id = ?2",
            params![runtime_id, request_id],
            |row| row.get::<_, i64>(0),
        )? as u64;
        result.deleted_event_count += transaction.query_row(
            "SELECT COUNT(*) FROM evidence_events
             WHERE runtime_id = ?1 AND request_id = ?2",
            params![runtime_id, request_id],
            |row| row.get::<_, i64>(0),
        )? as u64;
        transaction.execute(
            "DELETE FROM request_summary WHERE runtime_id = ?1 AND request_id = ?2",
            params![runtime_id, request_id],
        )?;
        result.deleted_request_count += 1;
    }
    transaction.execute(
        "DELETE FROM runtime_sessions
         WHERE ended_at_utc IS NOT NULL
           AND NOT EXISTS (
               SELECT 1 FROM request_summary
               WHERE request_summary.runtime_id = runtime_sessions.runtime_id
           )",
        [],
    )?;
    transaction.execute(
        "UPDATE evidence_health SET last_clear_at_utc = ?1 WHERE id = 1",
        [completed_at_utc],
    )?;
    transaction.commit().map_err(StoreError::from_sqlite)?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA incremental_vacuum;")
        .map_err(StoreError::from_sqlite)?;
    result.footprint_bytes = file_footprint_bytes(database)?;
    Ok(result)
}
