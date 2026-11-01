//! Whole-request age and capacity retention helpers.

use chrono::{DateTime, Duration, SecondsFormat};
use rusqlite::{params, Connection, Transaction};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Result, RetentionReport, StoreError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub retention_days: u32,
    pub max_storage_mib: u32,
}

impl RetentionPolicy {
    pub fn validate(&self) -> Result<()> {
        if !(1..=3650).contains(&self.retention_days) {
            return Err(StoreError::InvalidEvidence(
                "retention_days must be between 1 and 3650".to_owned(),
            ));
        }
        if !(32..=4096).contains(&self.max_storage_mib) {
            return Err(StoreError::InvalidEvidence(
                "max_storage_mib must be between 32 and 4096".to_owned(),
            ));
        }
        Ok(())
    }
}

pub fn file_footprint_bytes(database: impl AsRef<Path>) -> Result<u64> {
    let database = database.as_ref();
    let main = fs::metadata(database)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let wal = wal_path(database);
    let wal = fs::metadata(wal)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    Ok(main.saturating_add(wal))
}

pub(crate) fn run_retention(
    connection: &mut Connection,
    database: &Path,
    now: &str,
    policy: &RetentionPolicy,
) -> Result<RetentionReport> {
    policy.validate()?;
    let parsed = DateTime::parse_from_rfc3339(now).map_err(|error| {
        StoreError::InvalidEvidence(format!("retention timestamp is not RFC3339: {error}"))
    })?;
    let cutoff = (parsed - Duration::days(i64::from(policy.retention_days)))
        .to_rfc3339_opts(SecondsFormat::Millis, true);

    let mut report = RetentionReport::default();
    {
        let transaction = connection.transaction().map_err(StoreError::from_sqlite)?;
        let keys = eligible_keys(&transaction, Some(&cutoff), 10_000)?;
        let deleted = delete_keys(&transaction, &keys)?;
        report.age_cleanup_requests = deleted.requests;
        report.deleted_request_count += deleted.requests;
        report.deleted_event_count += deleted.events;
        report.deleted_approval_count += deleted.approvals;
        transaction.commit().map_err(StoreError::from_sqlite)?;
    }
    checkpoint_and_vacuum(connection)?;

    let max_bytes = u64::from(policy.max_storage_mib) * 1024 * 1024;
    let target_bytes = max_bytes.saturating_mul(90) / 100;
    while file_footprint_bytes(database)? > max_bytes {
        let transaction = connection.transaction().map_err(StoreError::from_sqlite)?;
        let keys = eligible_keys(&transaction, None, 256)?;
        if keys.is_empty() {
            transaction.rollback().map_err(StoreError::from_sqlite)?;
            // Incremental auto-vacuum may leave enough free pages that the
            // physical file still exceeds the cap after all eligible request
            // families are gone. A one-time bounded rebuild reclaims those
            // pages; if it still cannot meet the cap, the store is degraded.
            rebuild_database(connection)?;
            report.capacity_degraded = file_footprint_bytes(database)? > max_bytes;
            break;
        }
        let deleted = delete_keys(&transaction, &keys)?;
        report.capacity_cleanup_requests += deleted.requests;
        report.deleted_request_count += deleted.requests;
        report.deleted_event_count += deleted.events;
        report.deleted_approval_count += deleted.approvals;
        transaction.commit().map_err(StoreError::from_sqlite)?;
        checkpoint_and_vacuum(connection)?;
        if file_footprint_bytes(database)? <= target_bytes {
            break;
        }
    }
    report.footprint_bytes = file_footprint_bytes(database)?;
    if report.footprint_bytes > max_bytes {
        report.capacity_degraded = true;
    }
    if report.capacity_degraded {
        connection
            .execute(
                "UPDATE evidence_health
                 SET last_error_code = 'EVIDENCE_CAPACITY_DEGRADED',
                     degraded_since_utc = COALESCE(degraded_since_utc, ?1),
                     last_maintenance_at_utc = ?1
                 WHERE id = 1",
                [now],
            )
            .map_err(StoreError::from_sqlite)?;
    } else {
        connection
            .execute(
                "UPDATE evidence_health SET last_maintenance_at_utc = ?1 WHERE id = 1",
                [now],
            )
            .map_err(StoreError::from_sqlite)?;
    }
    Ok(report)
}

fn eligible_keys(
    transaction: &Transaction<'_>,
    cutoff: Option<&str>,
    limit: u32,
) -> Result<Vec<(String, String)>> {
    let mut sql = String::from(
        "SELECT runtime_id, request_id FROM request_summary
         WHERE terminal_state <> 'open'",
    );
    if cutoff.is_some() {
        sql.push_str(" AND last_seen_at_utc < ?");
    }
    sql.push_str(" ORDER BY last_seen_at_utc ASC, runtime_id ASC, request_id ASC LIMIT ?");

    let mut statement = transaction.prepare(&sql).map_err(StoreError::from_sqlite)?;
    let mut values = Vec::new();
    if let Some(cutoff) = cutoff {
        values.push(rusqlite::types::Value::Text(cutoff.to_owned()));
    }
    values.push(rusqlite::types::Value::Integer(i64::from(limit)));
    let keys = statement
        .query_map(rusqlite::params_from_iter(values), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(keys)
}

struct DeleteCounts {
    requests: u64,
    events: u64,
    approvals: u64,
}

fn delete_keys(transaction: &Transaction<'_>, keys: &[(String, String)]) -> Result<DeleteCounts> {
    let mut counts = DeleteCounts {
        requests: 0,
        events: 0,
        approvals: 0,
    };
    for (runtime_id, request_id) in keys {
        counts.approvals += transaction.query_row(
            "SELECT COUNT(*) FROM approval_history WHERE runtime_id = ?1 AND request_id = ?2",
            params![runtime_id, request_id],
            |row| row.get::<_, i64>(0),
        )? as u64;
        counts.events += transaction.query_row(
            "SELECT COUNT(*) FROM evidence_events WHERE runtime_id = ?1 AND request_id = ?2",
            params![runtime_id, request_id],
            |row| row.get::<_, i64>(0),
        )? as u64;
        transaction.execute(
            "DELETE FROM request_summary WHERE runtime_id = ?1 AND request_id = ?2",
            params![runtime_id, request_id],
        )?;
        counts.requests += 1;
    }
    Ok(counts)
}

fn checkpoint_and_vacuum(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA incremental_vacuum;",
        )
        .map_err(StoreError::from_sqlite)
}

fn rebuild_database(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "VACUUM;
             PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA incremental_vacuum;",
        )
        .map_err(StoreError::from_sqlite)
}

fn wal_path(database: &Path) -> PathBuf {
    let mut path = database.as_os_str().to_os_string();
    path.push("-wal");
    PathBuf::from(path)
}
