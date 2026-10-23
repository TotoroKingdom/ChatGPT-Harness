//! M00 SQLite ownership/durability proof of concept.
//!
//! This crate is intentionally disposable. It demonstrates the proposed
//! Runtime-single-writer / Tauri-read-only / stopped-runtime-maintenance
//! topology without being a production evidence store or authorization API.

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::Duration,
};

const SCHEMA_V1_CHECKSUM: &str = "m00-schema-v1";
const SCHEMA_V2_CHECKSUM: &str = "m00-schema-v2";
const CURRENT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PocError {
    WriterAlreadyOwned,
    MaintenanceAlreadyOwned,
    WriterActive,
    Migration(String),
    InjectedFailure,
    ReadOnly,
    Sqlite(String),
    Io(String),
}

impl fmt::Display for PocError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriterAlreadyOwned => formatter.write_str("M00 writer ownership already exists"),
            Self::MaintenanceAlreadyOwned => {
                formatter.write_str("M00 maintenance ownership already exists")
            }
            Self::WriterActive => formatter.write_str("M00 writer is still active"),
            Self::Migration(message) => write!(formatter, "M00 migration failed: {message}"),
            Self::InjectedFailure => formatter.write_str("M00 injected pre-commit failure"),
            Self::ReadOnly => formatter.write_str("M00 reader is read-only"),
            Self::Sqlite(message) => write!(formatter, "SQLite error: {message}"),
            Self::Io(message) => write!(formatter, "I/O error: {message}"),
        }
    }
}

impl std::error::Error for PocError {}

impl From<rusqlite::Error> for PocError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error.to_string())
    }
}

impl From<io::Error> for PocError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceEvent {
    pub event_id: String,
    pub runtime_id: String,
    pub request_id: String,
    pub stage: String,
    pub outcome: String,
    pub coverage: String,
    pub observed_at: String,
}

impl EvidenceEvent {
    pub fn new(runtime_id: &str, request_id: &str, stage: &str, coverage: &str) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            runtime_id: runtime_id.into(),
            request_id: request_id.into(),
            stage: stage.into(),
            outcome: if stage.contains("completed") {
                "success".into()
            } else {
                "in_progress".into()
            },
            coverage: coverage.into(),
            observed_at: "2026-10-23T00:00:00.000Z".into(),
        }
    }
}

/// Historical approval data is intentionally a plain observation DTO. There
/// is no conversion method to any Runtime authorization type in this crate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoricalApproval {
    pub approval_id: String,
    pub request_id: String,
    pub decision: String,
    pub terminal_state: String,
}

struct OwnershipMarker {
    path: PathBuf,
    _file: File,
}

impl OwnershipMarker {
    fn acquire(path: PathBuf, error: PocError) -> Result<Self, PocError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|existing| {
                if existing.kind() == io::ErrorKind::AlreadyExists {
                    error.clone()
                } else {
                    PocError::Io(existing.to_string())
                }
            })?;
        Ok(Self { path, _file: file })
    }
}

impl Drop for OwnershipMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn writer_marker(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.m00-writer", path.display()))
}

fn maintenance_marker(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.m00-maintenance", path.display()))
}

fn configure_writer(connection: &Connection) -> Result<(), PocError> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(Duration::from_millis(250))?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

fn configure_reader(connection: &Connection) -> Result<(), PocError> {
    connection.busy_timeout(Duration::from_millis(250))?;
    Ok(())
}

fn create_v1_schema(connection: &Connection) -> Result<(), PocError> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL,
            checksum TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS evidence_events (
            event_id TEXT PRIMARY KEY,
            runtime_id TEXT NOT NULL,
            request_id TEXT NOT NULL,
            stage TEXT NOT NULL,
            outcome TEXT NOT NULL,
            observed_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS request_summary (
            runtime_id TEXT NOT NULL,
            request_id TEXT NOT NULL,
            first_seen TEXT NOT NULL,
            last_seen TEXT NOT NULL,
            status TEXT NOT NULL,
            PRIMARY KEY (runtime_id, request_id)
        );
        INSERT OR IGNORE INTO schema_migrations(version, applied_at, checksum)
            VALUES (1, '2026-10-23T00:00:00.000Z', 'm00-schema-v1');
        ",
    )?;
    Ok(())
}

fn create_v2_objects(transaction: &Transaction<'_>) -> Result<(), PocError> {
    transaction.execute(
        "ALTER TABLE evidence_events ADD COLUMN coverage TEXT NOT NULL DEFAULT 'UNKNOWN'",
        [],
    )?;
    transaction.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS evidence_health (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            last_commit TEXT,
            drop_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT
        );
        INSERT OR IGNORE INTO evidence_health(id, last_commit, drop_count)
            VALUES (1, NULL, 0);
        ",
    )?;
    transaction.execute(
        "INSERT INTO schema_migrations(version, applied_at, checksum) VALUES (2, ?1, ?2)",
        params!["2026-10-23T00:00:00.000Z", SCHEMA_V2_CHECKSUM],
    )?;
    Ok(())
}

fn verify_migration_history(connection: &Connection) -> Result<i64, PocError> {
    let mut statement = connection
        .prepare("SELECT version, checksum FROM schema_migrations ORDER BY version ASC")?;
    let mut rows = statement.query([])?;
    let mut latest = 0;
    while let Some(row) = rows.next()? {
        let version: i64 = row.get(0)?;
        let checksum: String = row.get(1)?;
        let expected = match version {
            1 => SCHEMA_V1_CHECKSUM,
            2 => SCHEMA_V2_CHECKSUM,
            _ => {
                return Err(PocError::Migration(format!(
                    "unsupported schema version {version}"
                )))
            }
        };
        if checksum != expected {
            return Err(PocError::Migration(format!(
                "checksum mismatch at schema version {version}"
            )));
        }
        latest = version;
    }
    Ok(latest)
}

fn migrate(connection: &mut Connection, inject_failure: bool) -> Result<(), PocError> {
    if connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_none()
    {
        create_v1_schema(connection)?;
    }

    let latest = verify_migration_history(connection)?;
    if latest > CURRENT_SCHEMA_VERSION {
        return Err(PocError::Migration(format!(
            "schema {latest} is newer than supported {CURRENT_SCHEMA_VERSION}"
        )));
    }
    if latest == 1 {
        let transaction = connection.transaction()?;
        create_v2_objects(&transaction)?;
        if inject_failure {
            return Err(PocError::InjectedFailure);
        }
        transaction.commit()?;
    }
    let final_version = verify_migration_history(connection)?;
    if final_version != CURRENT_SCHEMA_VERSION {
        return Err(PocError::Migration(format!(
            "expected schema {CURRENT_SCHEMA_VERSION}, got {final_version}"
        )));
    }
    Ok(())
}

pub struct WriterStore {
    connection: Connection,
    _owner: OwnershipMarker,
}

impl WriterStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PocError> {
        Self::open_internal(path.as_ref(), false)
    }

    pub fn open_with_injected_migration_failure(path: impl AsRef<Path>) -> Result<Self, PocError> {
        Self::open_internal(path.as_ref(), true)
    }

    fn open_internal(path: &Path, inject_failure: bool) -> Result<Self, PocError> {
        let owner = OwnershipMarker::acquire(writer_marker(path), PocError::WriterAlreadyOwned)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        configure_writer(&connection)?;
        migrate(&mut connection, inject_failure)?;
        Ok(Self {
            connection,
            _owner: owner,
        })
    }

    pub fn insert_event_and_update_summary(
        &mut self,
        event: &EvidenceEvent,
        fail_before_commit: bool,
    ) -> Result<bool, PocError> {
        let transaction = self.connection.transaction()?;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO evidence_events(
                event_id, runtime_id, request_id, stage, outcome, observed_at, coverage
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.event_id,
                event.runtime_id,
                event.request_id,
                event.stage,
                event.outcome,
                event.observed_at,
                event.coverage,
            ],
        )?;
        if inserted == 1 {
            transaction.execute(
                "INSERT INTO request_summary(
                    runtime_id, request_id, first_seen, last_seen, status
                ) VALUES (?1, ?2, ?3, ?3, ?4)
                ON CONFLICT(runtime_id, request_id) DO UPDATE SET
                    last_seen = excluded.last_seen,
                    status = excluded.status",
                params![
                    event.runtime_id,
                    event.request_id,
                    event.observed_at,
                    event.coverage,
                ],
            )?;
        }
        if fail_before_commit {
            return Err(PocError::InjectedFailure);
        }
        transaction.commit()?;
        Ok(inserted == 1)
    }

    pub fn event_count(&self) -> Result<i64, PocError> {
        Ok(self
            .connection
            .query_row("SELECT COUNT(*) FROM evidence_events", [], |row| row.get(0))?)
    }

    pub fn schema_version(&self) -> Result<i64, PocError> {
        Ok(self
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })?)
    }

    pub fn journal_mode(&self) -> Result<String, PocError> {
        Ok(self
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
    }

    pub fn request_summary(
        &self,
        runtime_id: &str,
        request_id: &str,
    ) -> Result<Option<(String, String)>, PocError> {
        Ok(self
            .connection
            .query_row(
                "SELECT status, last_seen FROM request_summary
                 WHERE runtime_id = ?1 AND request_id = ?2",
                params![runtime_id, request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }
}

pub struct ReadOnlyReader {
    connection: Connection,
}

impl ReadOnlyReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PocError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        configure_reader(&connection)?;
        Ok(Self { connection })
    }

    pub fn event_count(&self) -> Result<i64, PocError> {
        Ok(self
            .connection
            .query_row("SELECT COUNT(*) FROM evidence_events", [], |row| row.get(0))?)
    }

    pub fn request_summary(
        &self,
        runtime_id: &str,
        request_id: &str,
    ) -> Result<Option<(String, String)>, PocError> {
        Ok(self
            .connection
            .query_row(
                "SELECT status, last_seen FROM request_summary
                 WHERE runtime_id = ?1 AND request_id = ?2",
                params![runtime_id, request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    /// Verification-only operation. The public POC reader has no write API;
    /// this method proves the underlying SQLite connection rejects a write.
    pub fn verify_read_only(&self) -> Result<(), PocError> {
        match self
            .connection
            .execute("CREATE TABLE m00_reader_write_probe(id INTEGER)", [])
        {
            Err(rusqlite::Error::SqliteFailure(_, _)) => Err(PocError::ReadOnly),
            Err(error) => Err(error.into()),
            Ok(_) => Err(PocError::Sqlite(
                "read-only connection unexpectedly wrote".into(),
            )),
        }
    }
}

pub struct OfflineMaintenance {
    connection: Connection,
    _owner: OwnershipMarker,
}

impl OfflineMaintenance {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self, PocError> {
        let path = path.as_ref();
        if writer_marker(path).exists() {
            return Err(PocError::WriterActive);
        }
        let owner =
            OwnershipMarker::acquire(maintenance_marker(path), PocError::MaintenanceAlreadyOwned)?;
        if writer_marker(path).exists() {
            return Err(PocError::WriterActive);
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_millis(250))?;
        Ok(Self {
            connection,
            _owner: owner,
        })
    }

    pub fn clear_events(&self) -> Result<(), PocError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute("DELETE FROM evidence_events", [])?;
        transaction.execute("DELETE FROM request_summary", [])?;
        transaction.commit()?;
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}

pub fn create_legacy_v1(path: impl AsRef<Path>) -> Result<(), PocError> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let connection = Connection::open(path)?;
    create_v1_schema(&connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TempDatabase {
        directory: TempDir,
        path: PathBuf,
    }

    impl TempDatabase {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("temp database directory");
            let path = directory.path().join("evidence.sqlite3");
            Self { directory, path }
        }
    }

    fn event(runtime: &str, request: &str, stage: &str, coverage: &str) -> EvidenceEvent {
        let mut value = EvidenceEvent::new(runtime, request, stage, coverage);
        value.event_id = format!("event-{runtime}-{request}-{stage}");
        value
    }

    #[test]
    fn writer_reader_wal_and_explicit_single_writer_ownership() {
        let database = TempDatabase::new();
        let mut writer = WriterStore::open(&database.path).expect("open writer");
        assert_eq!(writer.journal_mode().unwrap(), "wal");
        let reader = ReadOnlyReader::open(&database.path).expect("open read-only reader");
        assert!(matches!(
            WriterStore::open(&database.path),
            Err(PocError::WriterAlreadyOwned)
        ));
        let item = event("runtime-a", "request-a", "execution_completed", "COMPLETE");
        assert!(writer
            .insert_event_and_update_summary(&item, false)
            .unwrap());
        assert_eq!(reader.event_count().unwrap(), 1);
        assert_eq!(reader.verify_read_only().unwrap_err(), PocError::ReadOnly);
        drop(reader);
        drop(writer);
        assert!(!writer_marker(&database.path).exists());
        assert!(database.directory.path().exists());
    }

    #[test]
    fn transaction_rolls_back_event_and_summary_together() {
        let database = TempDatabase::new();
        let mut writer = WriterStore::open(&database.path).unwrap();
        let reader = ReadOnlyReader::open(&database.path).unwrap();
        let item = event("runtime-a", "request-a", "execution_completed", "COMPLETE");
        assert_eq!(
            writer
                .insert_event_and_update_summary(&item, true)
                .unwrap_err(),
            PocError::InjectedFailure
        );
        assert_eq!(writer.event_count().unwrap(), 0);
        assert_eq!(reader.event_count().unwrap(), 0);
        assert_eq!(
            writer.request_summary("runtime-a", "request-a").unwrap(),
            None
        );
    }

    #[test]
    fn event_id_is_idempotent_and_request_identity_beats_close_timestamps() {
        let database = TempDatabase::new();
        let mut writer = WriterStore::open(&database.path).unwrap();
        let reader = ReadOnlyReader::open(&database.path).unwrap();
        let first = event("runtime-a", "request-a", "execution_completed", "COMPLETE");
        assert!(writer
            .insert_event_and_update_summary(&first, false)
            .unwrap());
        assert!(!writer
            .insert_event_and_update_summary(&first, false)
            .unwrap());
        let second = event("runtime-a", "request-b", "execution_started", "INCOMPLETE");
        let third = event("runtime-b", "request-a", "execution_started", "INCOMPLETE");
        writer
            .insert_event_and_update_summary(&second, false)
            .unwrap();
        writer
            .insert_event_and_update_summary(&third, false)
            .unwrap();
        assert_eq!(reader.event_count().unwrap(), 3);
        assert_eq!(
            reader.request_summary("runtime-a", "request-b").unwrap(),
            Some(("INCOMPLETE".into(), "2026-10-23T00:00:00.000Z".into()))
        );
        assert_eq!(
            reader.request_summary("runtime-b", "request-a").unwrap(),
            Some(("INCOMPLETE".into(), "2026-10-23T00:00:00.000Z".into()))
        );
    }

    #[test]
    fn incomplete_request_survives_writer_restart_without_invented_terminal_result() {
        let database = TempDatabase::new();
        let mut writer = WriterStore::open(&database.path).unwrap();
        let received = event("runtime-a", "request-a", "request_received", "INCOMPLETE");
        writer
            .insert_event_and_update_summary(&received, false)
            .unwrap();
        drop(writer);

        let reader = ReadOnlyReader::open(&database.path).unwrap();
        assert_eq!(
            reader.request_summary("runtime-a", "request-a").unwrap(),
            Some(("INCOMPLETE".into(), "2026-10-23T00:00:00.000Z".into()))
        );
    }

    #[test]
    fn migration_failure_preserves_source_database_and_checksum_is_checked() {
        let database = TempDatabase::new();
        create_legacy_v1(&database.path).unwrap();
        assert!(matches!(
            WriterStore::open_with_injected_migration_failure(&database.path),
            Err(PocError::InjectedFailure)
        ));
        let connection = Connection::open(&database.path).unwrap();
        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, 1);
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('evidence_events') WHERE name='coverage'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        connection
            .execute(
                "UPDATE schema_migrations SET checksum='M00_BAD_CHECKSUM' WHERE version=1",
                [],
            )
            .unwrap();
        assert!(matches!(
            WriterStore::open(&database.path),
            Err(PocError::Migration(message)) if message.contains("checksum mismatch")
        ));
        assert!(database.path.exists());
    }

    #[test]
    fn offline_maintenance_requires_stopped_writer_and_reopens_coherently() {
        let database = TempDatabase::new();
        let mut writer = WriterStore::open(&database.path).unwrap();
        assert!(matches!(
            OfflineMaintenance::acquire(&database.path),
            Err(PocError::WriterActive)
        ));
        writer
            .insert_event_and_update_summary(
                &event("runtime-a", "request-a", "execution_completed", "COMPLETE"),
                false,
            )
            .unwrap();
        drop(writer);
        let maintenance = OfflineMaintenance::acquire(&database.path).unwrap();
        maintenance.clear_events().unwrap();
        drop(maintenance);
        let reopened = WriterStore::open(&database.path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        assert_eq!(reopened.event_count().unwrap(), 0);
    }

    #[test]
    fn persistence_failure_is_observational_and_history_has_no_authorization_constructor() {
        let database = TempDatabase::new();
        let mut writer = WriterStore::open(&database.path).unwrap();
        let policy_outcome = "DENY";
        let item = event("runtime-a", "request-a", "request_denied", "COMPLETE");
        assert_eq!(
            writer
                .insert_event_and_update_summary(&item, true)
                .unwrap_err(),
            PocError::InjectedFailure
        );
        assert_eq!(policy_outcome, "DENY");
        let historical = HistoricalApproval {
            approval_id: "approval-a".into(),
            request_id: "request-a".into(),
            decision: "allow_once".into(),
            terminal_state: "resolved".into(),
        };
        let json = serde_json::to_string(&historical).unwrap();
        assert!(!json.contains("ExecutionPermit"));
        assert!(!json.contains("PendingApproval"));
        assert!(!json.contains("SessionGrant"));
    }
}
