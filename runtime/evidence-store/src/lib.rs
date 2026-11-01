//! Durable, observational SQLite evidence store.
//!
//! This crate deliberately owns no Runtime policy, approval, grant, permit, or
//! executor code.  It accepts already-sanitized typed evidence and exposes
//! writer, read-only query, migration, and maintenance operations only.

mod dto;
mod maintenance;
mod migration;
mod query;
mod retention;
mod schema;
mod status;
mod writer;

#[cfg(test)]
mod tests;

pub use dto::*;
pub use maintenance::{clear_history_offline, OfflineMaintenance};
pub use migration::{migration_backup_path, MigrationReport};
pub use query::{filter_fingerprint, EvidenceReader};
pub use retention::{file_footprint_bytes, RetentionPolicy};
pub use schema::{computed_schema_checksum, SCHEMA_CHECKSUM, SCHEMA_V1_SQL, SCHEMA_VERSION};
pub use status::{status_for_error, StoreStatusSnapshot};
pub use writer::{EvidenceWriter, WriterLock};

use chrono::{SecondsFormat, Utc};
use rusqlite::ErrorCode;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("writer ownership is busy: {path}")]
    WriterBusy { path: PathBuf },
    #[error("SQLite is busy")]
    Busy,
    #[error("database schema {found} is newer or incompatible")]
    IncompatibleSchema { found: u32 },
    #[error("schema checksum mismatch: expected {expected}, got {actual}")]
    SchemaChecksumMismatch { expected: String, actual: String },
    #[error("corrupt evidence database: {reason}")]
    Corrupt {
        path: Option<PathBuf>,
        reason: String,
    },
    #[error("migration failed; backup retained at {backup_path}: {reason}")]
    MigrationFailed {
        backup_path: PathBuf,
        reason: String,
    },
    #[error("invalid history cursor")]
    InvalidCursor,
    #[error("invalid history limit {requested}; it must be greater than zero")]
    InvalidLimit { requested: u32 },
    #[error("invalid evidence: {0}")]
    InvalidEvidence(String),
    #[error("history request was not found")]
    NotFound,
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::from_sqlite(error)
    }
}

impl StoreError {
    pub(crate) fn from_sqlite(error: rusqlite::Error) -> Self {
        match &error {
            rusqlite::Error::SqliteFailure(failure, _) => match failure.code {
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => Self::Busy,
                ErrorCode::NotADatabase | ErrorCode::NotFound => Self::Corrupt {
                    path: None,
                    reason: error.to_string(),
                },
                _ => Self::Sqlite(error),
            },
            _ => Self::Sqlite(error),
        }
    }
}

pub(crate) fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn open_writer(database: impl AsRef<std::path::Path>) -> Result<EvidenceWriter> {
    EvidenceWriter::open(database)
}

pub fn open_reader(database: impl AsRef<std::path::Path>) -> Result<EvidenceReader> {
    EvidenceReader::open(database)
}
