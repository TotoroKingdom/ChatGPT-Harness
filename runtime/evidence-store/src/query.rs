//! Query-only SQLite facade with bounded keyset pagination.

use rusqlite::{types::Value, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::migration::configure_reader;
use crate::schema::verify_schema_checksum;
use crate::status::{status_for_missing, status_from_connection};
use crate::writer::{duration_from_i64, enum_from_db, optional_enum_from_db};
use crate::{
    DurableSanitizedArgumentsEvidence, EvidenceCoverage, EvidenceErrorCode, EvidenceEventV1,
    EvidenceStoreStatus, ExecutionResultEvidence, HistoryApprovalEvidence, HistoryConfig,
    HistoryGetResult, HistoryQueryParams, HistoryQueryResult, HistoryRequestSummary,
    HistoryStoreNotice, Result, StoreError, EVIDENCE_SCHEMA_VERSION, HISTORY_QUERY_DEFAULT_LIMIT,
    HISTORY_QUERY_MAX_LIMIT,
};

pub struct EvidenceReader {
    database: PathBuf,
    connection: Option<Connection>,
}

impl EvidenceReader {
    pub fn open(database: impl AsRef<Path>) -> Result<Self> {
        let database = database.as_ref().to_path_buf();
        if !database.exists() {
            return Ok(Self {
                database,
                connection: None,
            });
        }

        let connection = Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| attach_path(StoreError::from_sqlite(error), &database))?;
        configure_reader(&connection).map_err(|error| attach_path(error, &database))?;
        verify_schema_checksum(&connection).map_err(|error| attach_path(error, &database))?;
        Ok(Self {
            database,
            connection: Some(connection),
        })
    }

    pub fn database_path(&self) -> &Path {
        &self.database
    }

    pub fn is_initialized(&self) -> bool {
        self.connection.is_some()
    }

    pub fn query(&self, params: &HistoryQueryParams) -> Result<HistoryQueryResult> {
        let Some(connection) = self.connection.as_ref() else {
            return Ok(HistoryQueryResult {
                items: Vec::new(),
                next_cursor: None,
                store_status: status_for_missing(HistoryConfig::default()),
            });
        };

        let limit = normalize_limit(params.limit)?;
        let fingerprint = filter_fingerprint(params);
        let cursor = params
            .cursor
            .as_deref()
            .map(decode_cursor)
            .transpose()?
            .map(|cursor| {
                if cursor.filter_fingerprint != fingerprint {
                    Err(StoreError::InvalidCursor)
                } else {
                    Ok(cursor)
                }
            })
            .transpose()?;

        let mut sql = String::from(
            "SELECT runtime_id, request_id, tool_name, first_seen_at_utc,
                    last_seen_at_utc, terminal_state, outcome, policy_decision,
                    approval_state, failure_category, duration_ms, evidence_coverage
             FROM request_summary WHERE 1 = 1",
        );
        let mut values = Vec::<Value>::new();
        if let Some(from) = &params.from_utc {
            sql.push_str(" AND first_seen_at_utc >= ?");
            values.push(Value::Text(from.clone()));
        }
        if let Some(to) = &params.to_utc {
            sql.push_str(" AND first_seen_at_utc < ?");
            values.push(Value::Text(to.clone()));
        }
        if let Some(tool) = &params.tool {
            sql.push_str(" AND tool_name = ?");
            values.push(Value::Text(tool.clone()));
        }
        if let Some(outcome) = &params.outcome {
            sql.push_str(" AND outcome = ?");
            values.push(Value::Text(crate::writer::enum_to_db(outcome)?));
        }
        if let Some(policy) = &params.policy_decision {
            sql.push_str(" AND policy_decision = ?");
            values.push(Value::Text(crate::writer::enum_to_db(policy)?));
        }
        if let Some(approval) = &params.approval_state {
            sql.push_str(" AND approval_state = ?");
            values.push(Value::Text(crate::writer::enum_to_db(approval)?));
        }
        if let Some(cursor) = &cursor {
            sql.push_str(
                " AND (first_seen_at_utc < ?
                       OR (first_seen_at_utc = ? AND runtime_id < ?)
                       OR (first_seen_at_utc = ? AND runtime_id = ? AND request_id < ?))",
            );
            values.push(Value::Text(cursor.first_seen_at_utc.clone()));
            values.push(Value::Text(cursor.first_seen_at_utc.clone()));
            values.push(Value::Text(cursor.runtime_id.clone()));
            values.push(Value::Text(cursor.first_seen_at_utc.clone()));
            values.push(Value::Text(cursor.runtime_id.clone()));
            values.push(Value::Text(cursor.request_id.clone()));
        }
        sql.push_str(" ORDER BY first_seen_at_utc DESC, runtime_id DESC, request_id DESC LIMIT ?");
        values.push(Value::Integer(i64::from(limit) + 1));

        let mut statement = connection.prepare(&sql).map_err(StoreError::from_sqlite)?;
        let mut rows = statement
            .query(rusqlite::params_from_iter(values))
            .map_err(StoreError::from_sqlite)?;
        let mut items = Vec::with_capacity(limit as usize);
        let mut has_more = false;
        while let Some(row) = rows.next().map_err(StoreError::from_sqlite)? {
            if items.len() >= limit as usize {
                has_more = true;
                break;
            }
            items.push(summary_from_row(row)?);
        }
        let next_cursor = if has_more {
            items.last().map(|item| encode_cursor(item, &fingerprint))
        } else {
            None
        };
        Ok(HistoryQueryResult {
            items,
            next_cursor,
            store_status: status_from_connection(
                connection,
                HistoryConfig::default(),
                Some(&self.database),
            )?,
        })
    }

    pub fn history_query(&self, params: &HistoryQueryParams) -> Result<HistoryQueryResult> {
        self.query(params)
    }

    pub fn get(&self, runtime_id: &str, request_id: &str) -> Result<Option<HistoryGetResult>> {
        let Some(connection) = self.connection.as_ref() else {
            return Ok(None);
        };
        let summary = connection
            .query_row(
                "SELECT runtime_id, request_id, tool_name, first_seen_at_utc,
                        last_seen_at_utc, terminal_state, outcome, policy_decision,
                        approval_state, failure_category, duration_ms, evidence_coverage
                 FROM request_summary WHERE runtime_id = ?1 AND request_id = ?2",
                [runtime_id, request_id],
                summary_from_row,
            )
            .optional()
            .map_err(StoreError::from_sqlite)?;
        let Some(summary) = summary else {
            return Ok(None);
        };

        let (host_session_id, supervisor_generation_id) = connection
            .query_row(
                "SELECT host_session_id, supervisor_generation_id
                 FROM runtime_sessions WHERE runtime_id = ?1",
                [runtime_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(StoreError::from_sqlite)?
            .unwrap_or((None, None));

        let mut statement = connection
            .prepare(
                "SELECT event_id, runtime_id, request_id, approval_id, tool_name,
                        occurred_at_utc, recorded_at_utc, stage, lifecycle_state,
                        workspace_validation, policy_decision, policy_reason,
                        matched_rule_ids_json, approval_decision, outcome,
                        failure_category, duration_ms, sanitized_arguments_json,
                        result_evidence_json, coverage
                 FROM evidence_events
                 WHERE runtime_id = ?1 AND request_id = ?2
                 ORDER BY occurred_at_utc ASC, event_id ASC",
            )
            .map_err(StoreError::from_sqlite)?;
        let events = statement
            .query_map([runtime_id, request_id], |row| {
                event_from_row(
                    row,
                    host_session_id.clone(),
                    supervisor_generation_id.clone(),
                )
            })
            .map_err(StoreError::from_sqlite)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from_sqlite)?;

        let terminal_approval = connection
            .query_row(
                "SELECT runtime_id, approval_id, request_id, tool_name,
                        requested_at_utc, terminal_at_utc, terminal_state, decision
                 FROM approval_history
                 WHERE runtime_id = ?1 AND request_id = ?2
                 ORDER BY terminal_at_utc DESC, approval_id DESC LIMIT 1",
                [runtime_id, request_id],
                |row| {
                    Ok(HistoryApprovalEvidence {
                        runtime_id: row.get(0)?,
                        approval_id: row.get(1)?,
                        request_id: row.get(2)?,
                        tool_name: row.get(3)?,
                        requested_at_utc: row.get(4)?,
                        terminal_at_utc: row.get(5)?,
                        terminal_state: crate::writer::enum_from_db(row.get(6)?)
                            .map_err(to_sqlite_conversion_error)?,
                        decision: crate::writer::optional_enum_from_db(row.get(7)?)
                            .map_err(to_sqlite_conversion_error)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from_sqlite)?;

        let notice = if summary.evidence_coverage != EvidenceCoverage::Complete {
            Some(HistoryStoreNotice {
                code: EvidenceErrorCode::EvidenceQueueOverflow,
                coverage: summary.evidence_coverage,
            })
        } else {
            None
        };
        Ok(Some(HistoryGetResult {
            summary,
            events,
            terminal_approval,
            notice,
            store_status: status_from_connection(
                connection,
                HistoryConfig::default(),
                Some(&self.database),
            )?,
        }))
    }

    pub fn history_get(
        &self,
        runtime_id: &str,
        request_id: &str,
    ) -> Result<Option<HistoryGetResult>> {
        self.get(runtime_id, request_id)
    }

    pub fn get_required(&self, runtime_id: &str, request_id: &str) -> Result<HistoryGetResult> {
        self.get(runtime_id, request_id)?
            .ok_or(StoreError::NotFound)
    }

    pub fn status(&self) -> Result<EvidenceStoreStatus> {
        match self.connection.as_ref() {
            Some(connection) => {
                status_from_connection(connection, HistoryConfig::default(), Some(&self.database))
            }
            None => Ok(status_for_missing(HistoryConfig::default())),
        }
    }

    pub fn connection_is_query_only(&self) -> Result<bool> {
        let Some(connection) = self.connection.as_ref() else {
            return Ok(true);
        };
        let value: i64 = connection
            .query_row("PRAGMA query_only", [], |row| row.get(0))
            .map_err(StoreError::from_sqlite)?;
        Ok(value != 0)
    }
}

pub fn filter_fingerprint(params: &HistoryQueryParams) -> String {
    let canonical = serde_json::json!({
        "fromUtc": params.from_utc.clone(),
        "toUtc": params.to_utc.clone(),
        "tool": params.tool.clone(),
        "outcome": params.outcome,
        "policyDecision": params.policy_decision,
        "approvalState": params.approval_state,
    });
    sha256_hex(&serde_json::to_vec(&canonical).expect("history filters serialize"))
}

pub fn normalize_limit(limit: Option<u32>) -> Result<u32> {
    match limit {
        None => Ok(HISTORY_QUERY_DEFAULT_LIMIT),
        Some(0) => Err(StoreError::InvalidLimit { requested: 0 }),
        Some(value) => Ok(value.min(HISTORY_QUERY_MAX_LIMIT)),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CursorPayload {
    schema_version: u32,
    first_seen_at_utc: String,
    runtime_id: String,
    request_id: String,
    filter_fingerprint: String,
}

fn encode_cursor(item: &HistoryRequestSummary, filter_fingerprint: &str) -> String {
    let payload = CursorPayload {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        first_seen_at_utc: item.first_seen_at_utc.clone(),
        runtime_id: item.runtime_id.clone(),
        request_id: item.request_id.clone(),
        filter_fingerprint: filter_fingerprint.to_owned(),
    };
    format!(
        "v1.{}",
        hex_encode(&serde_json::to_vec(&payload).expect("cursor serializes"))
    )
}

fn decode_cursor(value: &str) -> Result<CursorPayload> {
    let Some(encoded) = value.strip_prefix("v1.") else {
        return Err(StoreError::InvalidCursor);
    };
    let bytes = hex_decode(encoded).ok_or(StoreError::InvalidCursor)?;
    let payload: CursorPayload =
        serde_json::from_slice(&bytes).map_err(|_| StoreError::InvalidCursor)?;
    if payload.schema_version != EVIDENCE_SCHEMA_VERSION
        || payload.first_seen_at_utc.is_empty()
        || payload.runtime_id.is_empty()
        || payload.request_id.is_empty()
    {
        return Err(StoreError::InvalidCursor);
    }
    Ok(payload)
}

fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryRequestSummary> {
    let outcome: Option<String> = row.get(6)?;
    let policy: Option<String> = row.get(7)?;
    let approval: String = row.get(8)?;
    let failure: Option<String> = row.get(9)?;
    let coverage: String = row.get(11)?;
    Ok(HistoryRequestSummary {
        runtime_id: row.get(0)?,
        request_id: row.get(1)?,
        tool_name: row.get(2)?,
        first_seen_at_utc: row.get(3)?,
        last_seen_at_utc: row.get(4)?,
        terminal_state: enum_from_db(row.get(5)?).map_err(to_sqlite_conversion_error)?,
        outcome: optional_enum_from_db(outcome).map_err(to_sqlite_conversion_error)?,
        policy_decision: optional_enum_from_db(policy).map_err(to_sqlite_conversion_error)?,
        approval_state: enum_from_db(approval).map_err(to_sqlite_conversion_error)?,
        failure_category: optional_enum_from_db(failure).map_err(to_sqlite_conversion_error)?,
        duration_ms: duration_from_i64(row.get(10)?).map_err(to_sqlite_conversion_error)?,
        evidence_coverage: enum_from_db(coverage).map_err(to_sqlite_conversion_error)?,
    })
}

fn event_from_row(
    row: &rusqlite::Row<'_>,
    host_session_id: Option<String>,
    supervisor_generation_id: Option<String>,
) -> rusqlite::Result<EvidenceEventV1> {
    let matched_rule_ids_json: Option<String> = row.get(12)?;
    let sanitized_arguments_json: Option<String> = row.get(17)?;
    let result_evidence_json: Option<String> = row.get(18)?;
    Ok(EvidenceEventV1 {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        event_id: row.get(0)?,
        runtime_id: row.get(1)?,
        request_id: row.get(2)?,
        host_session_id,
        supervisor_generation_id,
        approval_id: row.get(3)?,
        tool_name: row.get(4)?,
        occurred_at_utc: row.get(5)?,
        recorded_at_utc: row.get(6)?,
        stage: enum_from_db(row.get(7)?).map_err(to_sqlite_conversion_error)?,
        lifecycle_state: enum_from_db(row.get(8)?).map_err(to_sqlite_conversion_error)?,
        workspace_validation: enum_from_db(row.get(9)?).map_err(to_sqlite_conversion_error)?,
        policy_decision: optional_enum_from_db(row.get(10)?).map_err(to_sqlite_conversion_error)?,
        policy_reason: optional_enum_from_db(row.get(11)?).map_err(to_sqlite_conversion_error)?,
        matched_rule_ids: matched_rule_ids_json
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|error| to_sqlite_conversion_error(StoreError::Json(error)))
            })
            .transpose()?
            .unwrap_or_default(),
        approval_decision: optional_enum_from_db(row.get(13)?)
            .map_err(to_sqlite_conversion_error)?,
        outcome: enum_from_db(row.get(14)?).map_err(to_sqlite_conversion_error)?,
        failure_category: optional_enum_from_db(row.get(15)?)
            .map_err(to_sqlite_conversion_error)?,
        duration_ms: duration_from_i64(row.get(16)?).map_err(to_sqlite_conversion_error)?,
        sanitized_arguments: sanitized_arguments_json
            .map(|value| {
                serde_json::from_str::<DurableSanitizedArgumentsEvidence>(&value)
                    .map_err(|error| to_sqlite_conversion_error(StoreError::Json(error)))
            })
            .transpose()?,
        result_evidence: result_evidence_json
            .map(|value| {
                serde_json::from_str::<ExecutionResultEvidence>(&value)
                    .map_err(|error| to_sqlite_conversion_error(StoreError::Json(error)))
            })
            .transpose()?,
        coverage: enum_from_db(row.get(19)?).map_err(to_sqlite_conversion_error)?,
    })
}

fn attach_path(error: StoreError, path: &Path) -> StoreError {
    match error {
        StoreError::Corrupt { reason, .. } => StoreError::Corrupt {
            path: Some(path.to_path_buf()),
            reason,
        },
        other => other,
    }
}

fn to_sqlite_conversion_error(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}
