//! Durable health/status projection.  Live queue depth remains a Runtime
//! concern; the DAL only reports values persisted in evidence_health.

use rusqlite::Connection;
use std::path::Path;

use crate::retention::file_footprint_bytes;
use crate::{
    EvidenceErrorCode, EvidenceStoreState, EvidenceStoreStatus, HistoryConfig, Result, StoreError,
    SCHEMA_VERSION,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreStatusSnapshot {
    pub status: EvidenceStoreStatus,
}

pub(crate) fn status_from_connection(
    connection: &Connection,
    config: HistoryConfig,
    database: Option<&Path>,
) -> Result<EvidenceStoreStatus> {
    let health = connection.query_row(
        "SELECT schema_version, last_commit_at_utc, drop_count,
                last_drop_at_utc, last_error_code, degraded_since_utc,
                coverage_unknown_since_utc
         FROM evidence_health WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        },
    )?;
    let last_error_code = health
        .4
        .and_then(|value| serde_json::from_value(serde_json::Value::String(value)).ok());
    let state = if health.5.is_some() || health.2 > 0 {
        EvidenceStoreState::Degraded
    } else {
        EvidenceStoreState::Healthy
    };
    let footprint_bytes = database.and_then(|path| file_footprint_bytes(path).ok());
    Ok(EvidenceStoreStatus {
        state,
        schema_version: Some(health.0),
        queue_depth: 0,
        queue_capacity: 0,
        drop_count: health.2.max(0) as u64,
        last_drop_at_utc: health.3,
        last_commit_at_utc: health.1,
        last_error_code,
        effective_history_config: config,
        footprint_bytes,
        coverage_unknown_since_utc: health.6,
    })
}

pub(crate) fn status_for_missing(config: HistoryConfig) -> EvidenceStoreStatus {
    EvidenceStoreStatus {
        state: EvidenceStoreState::Unavailable,
        schema_version: None,
        queue_depth: 0,
        queue_capacity: 0,
        drop_count: 0,
        last_drop_at_utc: None,
        last_commit_at_utc: None,
        last_error_code: Some(EvidenceErrorCode::EvidenceIo),
        effective_history_config: config,
        footprint_bytes: None,
        coverage_unknown_since_utc: None,
    }
}

pub fn status_for_error(error: &StoreError, config: HistoryConfig) -> EvidenceStoreStatus {
    let (state, code) = match error {
        StoreError::WriterBusy { .. } | StoreError::Busy => {
            (EvidenceStoreState::Busy, EvidenceErrorCode::EvidenceBusy)
        }
        StoreError::IncompatibleSchema { .. } => (
            EvidenceStoreState::Incompatible,
            EvidenceErrorCode::EvidenceIncompatibleSchema,
        ),
        StoreError::SchemaChecksumMismatch { .. } => (
            EvidenceStoreState::Incompatible,
            EvidenceErrorCode::EvidenceIncompatibleSchema,
        ),
        StoreError::Corrupt { .. } => (
            EvidenceStoreState::Corrupt,
            EvidenceErrorCode::EvidenceCorrupt,
        ),
        StoreError::MigrationFailed { .. } => (
            EvidenceStoreState::Degraded,
            EvidenceErrorCode::EvidenceMigrationFailed,
        ),
        StoreError::InvalidCursor | StoreError::InvalidLimit { .. } => (
            EvidenceStoreState::Healthy,
            EvidenceErrorCode::EvidenceInvalidCursor,
        ),
        _ => (EvidenceStoreState::Degraded, EvidenceErrorCode::EvidenceIo),
    };
    EvidenceStoreStatus {
        state,
        schema_version: Some(SCHEMA_VERSION),
        queue_depth: 0,
        queue_capacity: 0,
        drop_count: 0,
        last_drop_at_utc: None,
        last_commit_at_utc: None,
        last_error_code: Some(code),
        effective_history_config: config,
        footprint_bytes: None,
        coverage_unknown_since_utc: None,
    }
}
