//! SQLite schema v1 and its deterministic migration checksum.

use rusqlite::{Connection, Transaction};
use sha2::{Digest, Sha256};

use crate::{now_utc, Result, StoreError};

/// Physical Evidence Store schema version.  The logical EvidenceEvent wire
/// schema remains v1; this version covers the SQLite shape only.
pub const SCHEMA_VERSION: u32 = 2;
pub const SCHEMA_V1_VERSION: u32 = 1;

/// Canonical SQL for the first durable schema.  Formatting and statement order
/// are frozen because the SHA-256 digest is stored with every migration row.
pub const SCHEMA_V1_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations(
  version INTEGER PRIMARY KEY,
  checksum TEXT NOT NULL,
  applied_at_utc TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runtime_sessions(
  runtime_id TEXT PRIMARY KEY,
  host_session_id TEXT NULL,
  supervisor_generation_id TEXT NULL,
  started_at_utc TEXT NOT NULL,
  ended_at_utc TEXT NULL,
  end_reason TEXT NULL,
  runtime_version TEXT NOT NULL,
  config_revision TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS request_summary(
  runtime_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  first_seen_at_utc TEXT NOT NULL,
  last_seen_at_utc TEXT NOT NULL,
  terminal_state TEXT NOT NULL,
  outcome TEXT NULL,
  policy_decision TEXT NULL,
  approval_state TEXT NOT NULL,
  failure_category TEXT NULL,
  execution_started INTEGER NOT NULL,
  duration_ms INTEGER NULL,
  evidence_coverage TEXT NOT NULL,
  PRIMARY KEY(runtime_id, request_id),
  FOREIGN KEY(runtime_id)
    REFERENCES runtime_sessions(runtime_id)
    ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS evidence_events(
  event_id TEXT PRIMARY KEY,
  runtime_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  approval_id TEXT NULL,
  tool_name TEXT NOT NULL,
  occurred_at_utc TEXT NOT NULL,
  recorded_at_utc TEXT NOT NULL,
  stage TEXT NOT NULL,
  lifecycle_state TEXT NOT NULL,
  workspace_validation TEXT NOT NULL,
  policy_decision TEXT NULL,
  policy_reason TEXT NULL,
  matched_rule_ids_json TEXT NULL,
  approval_decision TEXT NULL,
  outcome TEXT NOT NULL,
  failure_category TEXT NULL,
  duration_ms INTEGER NULL,
  sanitized_arguments_json TEXT NULL,
  result_evidence_json TEXT NULL,
  coverage TEXT NOT NULL,
  FOREIGN KEY(runtime_id, request_id)
    REFERENCES request_summary(runtime_id, request_id)
    ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS approval_history(
  runtime_id TEXT NOT NULL,
  approval_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  requested_at_utc TEXT NULL,
  terminal_at_utc TEXT NOT NULL,
  terminal_state TEXT NOT NULL,
  decision TEXT NULL,
  PRIMARY KEY(runtime_id, approval_id),
  FOREIGN KEY(runtime_id, request_id)
    REFERENCES request_summary(runtime_id, request_id)
    ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS evidence_health(
  id INTEGER PRIMARY KEY CHECK(id = 1),
  schema_version INTEGER NOT NULL,
  last_commit_at_utc TEXT NULL,
  last_checkpoint_at_utc TEXT NULL,
  last_maintenance_at_utc TEXT NULL,
  last_clear_at_utc TEXT NULL,
  drop_count INTEGER NOT NULL,
  last_drop_at_utc TEXT NULL,
  last_error_code TEXT NULL,
  degraded_since_utc TEXT NULL,
  coverage_unknown_since_utc TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_request_summary_first_seen
  ON request_summary(first_seen_at_utc DESC, runtime_id DESC, request_id DESC);
CREATE INDEX IF NOT EXISTS idx_request_summary_tool_first_seen
  ON request_summary(tool_name, first_seen_at_utc DESC, runtime_id DESC, request_id DESC);
CREATE INDEX IF NOT EXISTS idx_request_summary_outcome_first_seen
  ON request_summary(outcome, first_seen_at_utc DESC, runtime_id DESC, request_id DESC);
CREATE INDEX IF NOT EXISTS idx_request_summary_policy_first_seen
  ON request_summary(policy_decision, first_seen_at_utc DESC, runtime_id DESC, request_id DESC);
CREATE INDEX IF NOT EXISTS idx_request_summary_approval_first_seen
  ON request_summary(approval_state, first_seen_at_utc DESC, runtime_id DESC, request_id DESC);

CREATE INDEX IF NOT EXISTS idx_evidence_events_request_order
  ON evidence_events(runtime_id, request_id, occurred_at_utc ASC, event_id ASC);
CREATE INDEX IF NOT EXISTS idx_evidence_events_occurred
  ON evidence_events(occurred_at_utc DESC, event_id DESC);
CREATE INDEX IF NOT EXISTS idx_evidence_events_approval
  ON evidence_events(approval_id) WHERE approval_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_approval_history_terminal
  ON approval_history(terminal_at_utc DESC, runtime_id DESC, approval_id DESC);
"#;

/// Additive v1 → v2 migration.  Existing rows are deliberately left without
/// trace IDs and spans: M08 cannot manufacture historical observations.
pub const SCHEMA_V2_SQL: &str = r#"
ALTER TABLE request_summary ADD COLUMN trace_id TEXT NULL;

CREATE TABLE IF NOT EXISTS request_spans(
  span_id TEXT PRIMARY KEY,
  trace_id TEXT NOT NULL,
  parent_span_id TEXT NULL,
  runtime_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  span_sequence INTEGER NOT NULL,
  span_name TEXT NOT NULL,
  started_at_utc TEXT NOT NULL,
  ended_at_utc TEXT NULL,
  started_offset_ms INTEGER NOT NULL,
  duration_ms INTEGER NULL,
  status TEXT NOT NULL,
  failure_category TEXT NULL,
  coverage TEXT NOT NULL,
  UNIQUE(runtime_id, request_id, span_sequence),
  FOREIGN KEY(runtime_id, request_id)
    REFERENCES request_summary(runtime_id, request_id)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_request_spans_request_order
  ON request_spans(runtime_id, request_id, span_sequence);
CREATE INDEX IF NOT EXISTS idx_request_spans_trace_order
  ON request_spans(trace_id, span_sequence);
CREATE INDEX IF NOT EXISTS idx_request_spans_name_duration
  ON request_spans(span_name, duration_ms);
CREATE INDEX IF NOT EXISTS idx_evidence_events_request_stage
  ON evidence_events(runtime_id, request_id, stage);
"#;

// Keeping both literals in source makes the physical checksum visible in build
// artifacts and lets migrations reject drift.
pub const SCHEMA_V1_CHECKSUM: &str =
    "2fe475c1e1ae4870d4bed95b34e504cfa0fc93956f4eb629e4e7d9b0287d90f7";

// Filled from SCHEMA_V1_SQL + SCHEMA_V2_SQL.
pub const SCHEMA_CHECKSUM: &str =
    "5035bdcaafa3659e1cc7b6eec0c6b7dfa00ee0df415971baa9288c274f1e93d8";

pub fn computed_schema_checksum() -> String {
    let mut source = String::with_capacity(SCHEMA_V1_SQL.len() + SCHEMA_V2_SQL.len());
    source.push_str(SCHEMA_V1_SQL);
    source.push_str(SCHEMA_V2_SQL);
    let digest = Sha256::digest(source.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn create_latest_schema(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(SCHEMA_V1_SQL)?;
    transaction.execute_batch(SCHEMA_V2_SQL)?;
    transaction.execute(
        "INSERT OR REPLACE INTO schema_migrations(version, checksum, applied_at_utc)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![SCHEMA_VERSION, SCHEMA_CHECKSUM, now_utc()],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO evidence_health
         (id, schema_version, drop_count)
         VALUES (1, ?1, 0)",
        rusqlite::params![SCHEMA_VERSION],
    )?;
    Ok(())
}

pub(crate) fn verify_schema_checksum(connection: &Connection) -> Result<()> {
    let latest: Option<i64> =
        connection.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })?;
    match latest {
        Some(version) if version > i64::from(SCHEMA_VERSION) => {
            return Err(StoreError::IncompatibleSchema {
                found: u32::try_from(version).unwrap_or(u32::MAX),
            });
        }
        Some(_) => {}
        None => {
            return Err(StoreError::Corrupt {
                path: None,
                reason: "schema_migrations is empty".to_owned(),
            });
        }
    }
    let version = latest.unwrap_or_default() as u32;
    let actual: String = connection.query_row(
        "SELECT checksum FROM schema_migrations WHERE version = ?1",
        rusqlite::params![version],
        |row| row.get(0),
    )?;
    let expected = if version == SCHEMA_V1_VERSION {
        SCHEMA_V1_CHECKSUM
    } else if version == SCHEMA_VERSION {
        SCHEMA_CHECKSUM
    } else {
        return Err(StoreError::Corrupt {
            path: None,
            reason: format!("unsupported schema version {version}"),
        });
    };
    if actual != expected {
        return Err(StoreError::SchemaChecksumMismatch {
            expected: expected.to_owned(),
            actual,
        });
    }
    if version == SCHEMA_V1_VERSION {
        verify_schema_shape_v1(connection)
    } else {
        verify_schema_shape(connection)
    }
}

pub(crate) fn verify_schema_shape(connection: &Connection) -> Result<()> {
    let required: [(&str, &[&str]); 6] = [
        (
            "schema_migrations",
            &["version", "checksum", "applied_at_utc"],
        ),
        (
            "runtime_sessions",
            &[
                "runtime_id",
                "host_session_id",
                "supervisor_generation_id",
                "started_at_utc",
                "ended_at_utc",
                "end_reason",
                "runtime_version",
                "config_revision",
            ],
        ),
        (
            "request_summary",
            &[
                "runtime_id",
                "request_id",
                "tool_name",
                "first_seen_at_utc",
                "last_seen_at_utc",
                "terminal_state",
                "outcome",
                "policy_decision",
                "approval_state",
                "failure_category",
                "execution_started",
                "duration_ms",
                "evidence_coverage",
                "trace_id",
            ],
        ),
        (
            "evidence_events",
            &[
                "event_id",
                "runtime_id",
                "request_id",
                "approval_id",
                "tool_name",
                "occurred_at_utc",
                "recorded_at_utc",
                "stage",
                "lifecycle_state",
                "workspace_validation",
                "policy_decision",
                "policy_reason",
                "matched_rule_ids_json",
                "approval_decision",
                "outcome",
                "failure_category",
                "duration_ms",
                "sanitized_arguments_json",
                "result_evidence_json",
                "coverage",
            ],
        ),
        (
            "approval_history",
            &[
                "runtime_id",
                "approval_id",
                "request_id",
                "tool_name",
                "requested_at_utc",
                "terminal_at_utc",
                "terminal_state",
                "decision",
            ],
        ),
        (
            "evidence_health",
            &[
                "id",
                "schema_version",
                "last_commit_at_utc",
                "last_checkpoint_at_utc",
                "last_maintenance_at_utc",
                "last_clear_at_utc",
                "drop_count",
                "last_drop_at_utc",
                "last_error_code",
                "degraded_since_utc",
                "coverage_unknown_since_utc",
            ],
        ),
    ];

    for (table, columns) in required {
        let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
        let actual = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if actual
            != columns
                .iter()
                .map(|column| (*column).to_owned())
                .collect::<Vec<_>>()
        {
            return Err(StoreError::Corrupt {
                path: None,
                reason: format!("schema table {table} has unexpected columns"),
            });
        }
    }

    let request_spans: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'request_spans'",
        [],
        |row| row.get(0),
    )?;
    if request_spans != 1 {
        return Err(StoreError::Corrupt {
            path: None,
            reason: "schema table request_spans is missing".to_owned(),
        });
    }
    let span_columns = [
        "span_id",
        "trace_id",
        "parent_span_id",
        "runtime_id",
        "request_id",
        "span_sequence",
        "span_name",
        "started_at_utc",
        "ended_at_utc",
        "started_offset_ms",
        "duration_ms",
        "status",
        "failure_category",
        "coverage",
    ];
    let mut statement = connection.prepare("PRAGMA table_info(request_spans)")?;
    let actual = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if actual
        != span_columns
            .iter()
            .map(|column| (*column).to_owned())
            .collect::<Vec<_>>()
    {
        return Err(StoreError::Corrupt {
            path: None,
            reason: "schema table request_spans has unexpected columns".to_owned(),
        });
    }
    for (index, columns) in [
        (
            "idx_request_spans_request_order",
            &["runtime_id", "request_id", "span_sequence"][..],
        ),
        (
            "idx_request_spans_trace_order",
            &["trace_id", "span_sequence"][..],
        ),
        (
            "idx_request_spans_name_duration",
            &["span_name", "duration_ms"][..],
        ),
        (
            "idx_evidence_events_request_stage",
            &["runtime_id", "request_id", "stage"][..],
        ),
    ] {
        let mut statement = connection.prepare(&format!("PRAGMA index_info({index})"))?;
        let actual = statement
            .query_map([], |row| row.get::<_, String>(2))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if actual != columns {
            return Err(StoreError::Corrupt {
                path: None,
                reason: "evidence index shape is invalid".into(),
            });
        }
    }
    Ok(())
}

/// Verify the immutable v1 shape while a writer is preparing the v2 upgrade.
pub(crate) fn verify_schema_shape_v1(connection: &Connection) -> Result<()> {
    let required: [(&str, &[&str]); 6] = [
        (
            "schema_migrations",
            &["version", "checksum", "applied_at_utc"],
        ),
        (
            "runtime_sessions",
            &[
                "runtime_id",
                "host_session_id",
                "supervisor_generation_id",
                "started_at_utc",
                "ended_at_utc",
                "end_reason",
                "runtime_version",
                "config_revision",
            ],
        ),
        (
            "request_summary",
            &[
                "runtime_id",
                "request_id",
                "tool_name",
                "first_seen_at_utc",
                "last_seen_at_utc",
                "terminal_state",
                "outcome",
                "policy_decision",
                "approval_state",
                "failure_category",
                "execution_started",
                "duration_ms",
                "evidence_coverage",
            ],
        ),
        (
            "evidence_events",
            &[
                "event_id",
                "runtime_id",
                "request_id",
                "approval_id",
                "tool_name",
                "occurred_at_utc",
                "recorded_at_utc",
                "stage",
                "lifecycle_state",
                "workspace_validation",
                "policy_decision",
                "policy_reason",
                "matched_rule_ids_json",
                "approval_decision",
                "outcome",
                "failure_category",
                "duration_ms",
                "sanitized_arguments_json",
                "result_evidence_json",
                "coverage",
            ],
        ),
        (
            "approval_history",
            &[
                "runtime_id",
                "approval_id",
                "request_id",
                "tool_name",
                "requested_at_utc",
                "terminal_at_utc",
                "terminal_state",
                "decision",
            ],
        ),
        (
            "evidence_health",
            &[
                "id",
                "schema_version",
                "last_commit_at_utc",
                "last_checkpoint_at_utc",
                "last_maintenance_at_utc",
                "last_clear_at_utc",
                "drop_count",
                "last_drop_at_utc",
                "last_error_code",
                "degraded_since_utc",
                "coverage_unknown_since_utc",
            ],
        ),
    ];
    verify_required_columns(connection, required)
}

fn verify_required_columns(connection: &Connection, required: [(&str, &[&str]); 6]) -> Result<()> {
    for (table, columns) in required {
        let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
        let actual = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if actual
            != columns
                .iter()
                .map(|column| (*column).to_owned())
                .collect::<Vec<_>>()
        {
            return Err(StoreError::Corrupt {
                path: None,
                reason: format!("schema table {table} has unexpected columns"),
            });
        }
    }
    Ok(())
}

pub(crate) fn integrity_check(connection: &Connection) -> Result<()> {
    let result: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if result != "ok" {
        return Err(StoreError::Corrupt {
            path: None,
            reason: result,
        });
    }
    Ok(())
}
