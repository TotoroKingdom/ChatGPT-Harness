//! Single-owner SQLite writer and atomic event transaction implementation.

use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::maintenance::clear_terminal_connection;
use crate::migration::{configure_writer, initialize_or_migrate, open_connection};
use crate::retention::{run_retention, RetentionPolicy};
use crate::schema::SCHEMA_VERSION;
use crate::status::status_from_connection;
use crate::{
    now_utc, ApprovalDecision, EvidenceCoverage, EvidenceEventV1, EvidenceLifecycleState,
    EvidenceStage, EvidenceStoreStatus, HistoryApprovalState, HistoryOutcome, HistoryTerminalState,
    RecordOutcome, Result, RuntimeSession, StoreError, StorePaths, WriterOptions,
};

/// OS-level writer/maintenance ownership.  The lock is held for the whole
/// lifetime of `EvidenceWriter` or `OfflineMaintenance`.
#[derive(Debug)]
pub struct WriterLock {
    file: File,
    path: PathBuf,
}

impl WriterLock {
    pub fn try_acquire(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;
        if let Err(error) = file.try_lock_exclusive() {
            let lock_conflict = matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::PermissionDenied
            ) || matches!(error.raw_os_error(), Some(32 | 33));
            return Err(if lock_conflict {
                StoreError::WriterBusy { path }
            } else {
                StoreError::Io(error)
            });
        }
        Ok(Self { file, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug)]
pub struct EvidenceWriter {
    database: PathBuf,
    lock: WriterLock,
    connection: Connection,
    options: WriterOptions,
    current_runtime_id: Option<String>,
    loss_marked_requests: HashSet<(String, String)>,
}

impl EvidenceWriter {
    pub fn open(database: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(database, WriterOptions::default())
    }

    pub fn open_with_options(database: impl AsRef<Path>, options: WriterOptions) -> Result<Self> {
        options
            .history_config
            .validate()
            .map_err(|error| StoreError::InvalidEvidence(error.to_owned()))?;
        let paths = StorePaths::from_database(database);
        if let Some(parent) = paths.database.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock = WriterLock::try_acquire(&paths.lock)?;
        let had_source = paths.database.exists()
            && fs::metadata(&paths.database)
                .map(|metadata| metadata.len() > 0)
                .unwrap_or(false);
        let mut connection = match open_connection(&paths.database) {
            Ok(connection) => connection,
            Err(error) => return Err(error),
        };
        if had_source {
            if let Err(error) = initialize_or_migrate(&mut connection, &paths.database) {
                return Err(attach_path(error, &paths.database));
            }
            if let Err(error) = configure_writer(&connection) {
                return Err(attach_path(error, &paths.database));
            }
        } else {
            if let Err(error) = configure_writer(&connection) {
                return Err(attach_path(error, &paths.database));
            }
            if let Err(error) = initialize_or_migrate(&mut connection, &paths.database) {
                return Err(attach_path(error, &paths.database));
            }
        }
        let mut writer = Self {
            database: paths.database,
            lock,
            connection,
            options,
            current_runtime_id: None,
            loss_marked_requests: HashSet::new(),
        };
        writer.recover_unclean_sessions(&now_utc())?;
        Ok(writer)
    }

    pub fn database_path(&self) -> &Path {
        &self.database
    }

    pub fn lock_path(&self) -> &Path {
        self.lock.path()
    }

    pub fn open_session(&mut self, session: &RuntimeSession) -> Result<()> {
        if session.runtime_id.is_empty()
            || session.started_at_utc.is_empty()
            || session.runtime_version.is_empty()
            || session.config_revision.is_empty()
        {
            return Err(StoreError::InvalidEvidence(
                "runtime session has an empty required field".to_owned(),
            ));
        }
        self.connection
            .execute(
                "INSERT INTO runtime_sessions(
                runtime_id, host_session_id, supervisor_generation_id,
                started_at_utc, runtime_version, config_revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(runtime_id) DO UPDATE SET
                host_session_id = excluded.host_session_id,
                supervisor_generation_id = excluded.supervisor_generation_id,
                started_at_utc = excluded.started_at_utc,
                ended_at_utc = NULL,
                end_reason = NULL,
                runtime_version = excluded.runtime_version,
                config_revision = excluded.config_revision",
                params![
                    session.runtime_id,
                    session.host_session_id,
                    session.supervisor_generation_id,
                    session.started_at_utc,
                    session.runtime_version,
                    session.config_revision,
                ],
            )
            .map_err(StoreError::from_sqlite)?;
        self.current_runtime_id = Some(session.runtime_id.clone());
        Ok(())
    }

    pub fn record_event(&mut self, event: &EvidenceEventV1) -> Result<RecordOutcome> {
        let outcomes = self.record_events(std::slice::from_ref(event))?;
        Ok(RecordOutcome {
            inserted: outcomes == 1,
        })
    }

    /// Commits the whole batch atomically.  A validation or SQLite failure in
    /// any item rolls back every item in the batch.
    pub fn record_events(&mut self, events: &[EvidenceEventV1]) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(StoreError::from_sqlite)?;
        let mut inserted = 0usize;
        let mut last_commit = None;
        for event in events {
            validate_event(event)?;
            let request_has_known_loss = self
                .loss_marked_requests
                .contains(&(event.runtime_id.clone(), event.request_id.clone()));
            if record_event_transaction(&transaction, event, request_has_known_loss)? {
                inserted += 1;
                last_commit = Some(event.recorded_at_utc.clone());
            }
        }
        if let Some(last_commit) = last_commit {
            transaction
                .execute(
                    "UPDATE evidence_health
                     SET last_commit_at_utc = ?1, schema_version = ?2
                     WHERE id = 1",
                    params![last_commit, SCHEMA_VERSION],
                )
                .map_err(StoreError::from_sqlite)?;
        }
        transaction.commit().map_err(StoreError::from_sqlite)?;
        Ok(inserted)
    }

    pub fn recover_unclean_sessions(&mut self, ended_at_utc: &str) -> Result<u64> {
        let transaction = self
            .connection
            .transaction()
            .map_err(StoreError::from_sqlite)?;
        let sessions = transaction
            .execute(
                "UPDATE runtime_sessions
                 SET ended_at_utc = ?1, end_reason = 'unclean_restart'
                 WHERE ended_at_utc IS NULL",
                [ended_at_utc],
            )
            .map_err(StoreError::from_sqlite)? as u64;
        transaction
            .execute(
                "UPDATE request_summary
                 SET terminal_state = 'incomplete',
                     evidence_coverage = 'INCOMPLETE'
                 WHERE terminal_state = 'open'
                   AND runtime_id IN (
                       SELECT runtime_id FROM runtime_sessions
                       WHERE end_reason = 'unclean_restart'
                   )",
                [],
            )
            .map_err(StoreError::from_sqlite)?;
        transaction.commit().map_err(StoreError::from_sqlite)?;
        Ok(sessions)
    }

    pub fn close_session(
        &mut self,
        runtime_id: &str,
        ended_at_utc: &str,
        end_reason: &str,
    ) -> Result<u64> {
        let transaction = self
            .connection
            .transaction()
            .map_err(StoreError::from_sqlite)?;
        let incomplete = transaction.execute(
            "UPDATE request_summary
                 SET terminal_state = 'incomplete',
                     evidence_coverage = CASE
                         WHEN evidence_coverage = 'UNKNOWN' THEN 'UNKNOWN'
                         ELSE 'INCOMPLETE'
                     END
                 WHERE runtime_id = ?1 AND terminal_state = 'open'",
            [runtime_id],
        )? as u64;
        transaction.execute(
            "UPDATE runtime_sessions
             SET ended_at_utc = ?1, end_reason = ?2
             WHERE runtime_id = ?3 AND ended_at_utc IS NULL",
            params![ended_at_utc, end_reason, runtime_id],
        )?;
        transaction.commit().map_err(StoreError::from_sqlite)?;
        if self.current_runtime_id.as_deref() == Some(runtime_id) {
            self.current_runtime_id = None;
        }
        Ok(incomplete)
    }

    /// Marks a known affected request as incomplete without manufacturing an
    /// evidence event or terminal outcome.
    pub fn mark_request_incomplete(&mut self, runtime_id: &str, request_id: &str) -> Result<()> {
        self.loss_marked_requests
            .insert((runtime_id.to_owned(), request_id.to_owned()));
        self.connection
            .execute(
                "UPDATE request_summary
                 SET evidence_coverage = CASE
                     WHEN evidence_coverage = 'UNKNOWN' THEN 'UNKNOWN'
                     ELSE 'INCOMPLETE'
                 END
                 WHERE runtime_id = ?1 AND request_id = ?2",
                params![runtime_id, request_id],
            )
            .map_err(StoreError::from_sqlite)?;
        Ok(())
    }

    /// Records that a loss mapping was not available for at least one dropped
    /// item.  This is global health metadata only; it never creates a request
    /// or event row.
    pub fn mark_coverage_unknown(&mut self, at_utc: &str) -> Result<()> {
        self.connection
            .execute(
                "UPDATE evidence_health
                 SET coverage_unknown_since_utc = COALESCE(coverage_unknown_since_utc, ?1),
                     degraded_since_utc = COALESCE(degraded_since_utc, ?1)
                 WHERE id = 1",
                [at_utc],
            )
            .map_err(StoreError::from_sqlite)?;
        Ok(())
    }

    /// Persists an aggregate drop count in one SQL update.  The caller may
    /// pass a whole queue-overflow batch; this method intentionally never
    /// loops over individual dropped items.
    pub fn record_drop_count(
        &mut self,
        count: u64,
        dropped_at_utc: &str,
        error_code: &str,
    ) -> Result<()> {
        if count == 0 {
            return Ok(());
        }
        let count = i64::try_from(count).map_err(|_| {
            StoreError::InvalidEvidence("drop count exceeds SQLite INTEGER".to_owned())
        })?;
        self.connection
            .execute(
                "UPDATE evidence_health
                 SET drop_count = drop_count + ?1,
                     last_drop_at_utc = ?2,
                     last_error_code = ?3,
                     degraded_since_utc = COALESCE(degraded_since_utc, ?2)
                 WHERE id = 1",
                params![count, dropped_at_utc, error_code],
            )
            .map_err(StoreError::from_sqlite)?;
        Ok(())
    }

    /// Backwards-friendly single-item convenience wrapper.
    pub fn record_drop(&mut self, error_code: &str, dropped_at_utc: &str) -> Result<()> {
        self.record_drop_count(1, dropped_at_utc, error_code)
    }

    /// Alias used by queue owners that naturally describe the operation as a
    /// batch of dropped items.
    pub fn record_drops(
        &mut self,
        count: u64,
        dropped_at_utc: &str,
        error_code: &str,
    ) -> Result<()> {
        self.record_drop_count(count, dropped_at_utc, error_code)
    }

    pub fn status(&self) -> Result<EvidenceStoreStatus> {
        status_from_connection(
            &self.connection,
            self.options.history_config.clone(),
            Some(self.database.as_path()),
        )
    }

    pub fn apply_retention(
        &mut self,
        now_utc: &str,
        policy: &RetentionPolicy,
    ) -> Result<crate::RetentionReport> {
        run_retention(&mut self.connection, &self.database, now_utc, policy)
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(StoreError::from_sqlite)
    }

    pub fn clear_terminal_history(&mut self) -> Result<crate::HistoryClearResult> {
        clear_terminal_connection(&mut self.connection, &self.database)
    }

    pub fn connection_is_query_only(&self) -> Result<bool> {
        let value: i64 = self
            .connection
            .query_row("PRAGMA query_only", [], |row| row.get(0))?;
        Ok(value != 0)
    }
}

fn attach_path(error: StoreError, database: &Path) -> StoreError {
    match error {
        StoreError::Corrupt { reason, .. } => StoreError::Corrupt {
            path: Some(database.to_path_buf()),
            reason,
        },
        other => other,
    }
}

fn validate_event(event: &EvidenceEventV1) -> Result<()> {
    if event.schema_version != crate::EVIDENCE_SCHEMA_VERSION {
        return Err(StoreError::InvalidEvidence(format!(
            "unsupported evidence schema version {}",
            event.schema_version
        )));
    }
    for (name, value) in [
        ("event_id", event.event_id.as_str()),
        ("runtime_id", event.runtime_id.as_str()),
        ("request_id", event.request_id.as_str()),
        ("tool_name", event.tool_name.as_str()),
        ("occurred_at_utc", event.occurred_at_utc.as_str()),
        ("recorded_at_utc", event.recorded_at_utc.as_str()),
    ] {
        if value.is_empty() || value.len() > 512 {
            return Err(StoreError::InvalidEvidence(format!(
                "{name} is empty or exceeds 512 bytes"
            )));
        }
    }
    if event.matched_rule_ids.len() > 128
        || event
            .matched_rule_ids
            .iter()
            .any(|value| value.is_empty() || value.len() > 256)
    {
        return Err(StoreError::InvalidEvidence(
            "matched rule IDs are outside the bounded allowlist".to_owned(),
        ));
    }
    if serde_json::to_vec(event)?.len() > 32 * 1024 {
        return Err(StoreError::InvalidEvidence(
            "durable evidence exceeds the 32 KiB bound".to_owned(),
        ));
    }
    Ok(())
}

fn record_event_transaction(
    transaction: &Transaction<'_>,
    event: &EvidenceEventV1,
    request_has_known_loss: bool,
) -> Result<bool> {
    let already_present: Option<String> = transaction
        .query_row(
            "SELECT event_id FROM evidence_events WHERE event_id = ?1",
            [event.event_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if already_present.is_some() {
        return Ok(false);
    }
    ensure_runtime_session(transaction, event)?;
    let projection = SummaryProjection::from_event(event, request_has_known_loss)?;
    let inserted_summary = transaction.execute(
        "INSERT OR IGNORE INTO request_summary(
            runtime_id, request_id, tool_name, first_seen_at_utc, last_seen_at_utc,
            terminal_state, outcome, policy_decision, approval_state, failure_category,
            execution_started, duration_ms, evidence_coverage
         ) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            event.runtime_id,
            event.request_id,
            event.tool_name,
            event.occurred_at_utc,
            projection.terminal_state,
            projection.outcome,
            projection.policy_decision,
            projection.approval_state,
            projection.failure_category,
            projection.execution_started,
            projection.duration_ms,
            projection.coverage,
        ],
    )?;
    debug_assert!(inserted_summary <= 1);

    let inserted = transaction.execute(
        "INSERT OR IGNORE INTO evidence_events(
            event_id, runtime_id, request_id, approval_id, tool_name,
            occurred_at_utc, recorded_at_utc, stage, lifecycle_state,
            workspace_validation, policy_decision, policy_reason,
            matched_rule_ids_json, approval_decision, outcome, failure_category,
            duration_ms, sanitized_arguments_json, result_evidence_json, coverage
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
         )",
        params![
            event.event_id,
            event.runtime_id,
            event.request_id,
            event.approval_id,
            event.tool_name,
            event.occurred_at_utc,
            event.recorded_at_utc,
            enum_to_db(&event.stage)?,
            enum_to_db(&event.lifecycle_state)?,
            enum_to_db(&event.workspace_validation)?,
            optional_enum_to_db(event.policy_decision.as_ref())?,
            optional_enum_to_db(event.policy_reason.as_ref())?,
            if event.matched_rule_ids.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&event.matched_rule_ids)?)
            },
            optional_enum_to_db(event.approval_decision.as_ref())?,
            enum_to_db(&event.outcome)?,
            optional_enum_to_db(event.failure_category.as_ref())?,
            duration_to_i64(event.duration_ms)?,
            event
                .sanitized_arguments
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            event
                .result_evidence
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            enum_to_db(&event.coverage)?,
        ],
    )?;
    if inserted == 0 {
        return Ok(false);
    }

    let current =
        load_summary(transaction, &event.runtime_id, &event.request_id)?.ok_or_else(|| {
            StoreError::InvalidEvidence("summary disappeared in transaction".to_owned())
        })?;
    let merged = current.merge(projection);
    transaction.execute(
        "UPDATE request_summary
         SET tool_name = ?3,
             first_seen_at_utc = ?4,
             last_seen_at_utc = ?5,
             terminal_state = ?6,
             outcome = ?7,
             policy_decision = ?8,
             approval_state = ?9,
             failure_category = ?10,
             execution_started = ?11,
             duration_ms = ?12,
             evidence_coverage = ?13
         WHERE runtime_id = ?1 AND request_id = ?2",
        params![
            event.runtime_id,
            event.request_id,
            merged.tool_name,
            merged.first_seen_at_utc,
            merged.last_seen_at_utc,
            merged.terminal_state,
            merged.outcome,
            merged.policy_decision,
            merged.approval_state,
            merged.failure_category,
            merged.execution_started,
            merged.duration_ms,
            merged.evidence_coverage,
        ],
    )?;

    if let Some(mut approval) = terminal_approval(event)? {
        approval.requested_at_utc = transaction
            .query_row(
                "SELECT MIN(occurred_at_utc) FROM evidence_events
                 WHERE runtime_id = ?1 AND request_id = ?2 AND approval_id = ?3",
                params![event.runtime_id, event.request_id, approval.approval_id],
                |row| row.get::<_, Option<String>>(0),
            )?
            .unwrap_or_else(|| approval.requested_at_utc.clone());
        transaction.execute(
            "INSERT OR IGNORE INTO approval_history(
                runtime_id, approval_id, request_id, tool_name, requested_at_utc,
                terminal_at_utc, terminal_state, decision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.runtime_id,
                approval.approval_id,
                event.request_id,
                event.tool_name,
                approval.requested_at_utc,
                event.occurred_at_utc,
                approval.terminal_state,
                approval.decision,
            ],
        )?;
    }
    Ok(true)
}

fn ensure_runtime_session(transaction: &Transaction<'_>, event: &EvidenceEventV1) -> Result<()> {
    transaction.execute(
        "INSERT OR IGNORE INTO runtime_sessions(
            runtime_id, host_session_id, supervisor_generation_id,
            started_at_utc, runtime_version, config_revision
         ) VALUES (?1, ?2, ?3, ?4, 'unknown', 'unknown')",
        params![
            event.runtime_id,
            event.host_session_id,
            event.supervisor_generation_id,
            event.occurred_at_utc,
        ],
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct SummaryProjection {
    first_seen_at_utc: String,
    last_seen_at_utc: String,
    terminal_state: String,
    outcome: Option<String>,
    policy_decision: Option<String>,
    approval_state: String,
    failure_category: Option<String>,
    execution_started: bool,
    duration_ms: Option<i64>,
    coverage: String,
}

impl SummaryProjection {
    fn from_event(event: &EvidenceEventV1, request_has_known_loss: bool) -> Result<Self> {
        let terminal_state = terminal_state(event);
        let approval_state = approval_state(event);
        Ok(Self {
            first_seen_at_utc: event.occurred_at_utc.clone(),
            last_seen_at_utc: event.occurred_at_utc.clone(),
            terminal_state: enum_to_db(&terminal_state)?,
            outcome: if matches!(event.outcome, HistoryOutcome::InProgress) {
                None
            } else {
                Some(enum_to_db(&event.outcome)?)
            },
            policy_decision: optional_enum_to_db(event.policy_decision.as_ref())?,
            approval_state: enum_to_db(&approval_state)?,
            failure_category: optional_enum_to_db(event.failure_category.as_ref())?,
            execution_started: matches!(
                event.stage,
                EvidenceStage::ExecutionStarted
                    | EvidenceStage::ExecutionCompleted
                    | EvidenceStage::ExecutionFailed
            ) || matches!(
                event.lifecycle_state,
                EvidenceLifecycleState::Executing
                    | EvidenceLifecycleState::Completed
                    | EvidenceLifecycleState::Failed
            ),
            duration_ms: duration_to_i64(event.duration_ms)?,
            coverage: if request_has_known_loss {
                enum_to_db(&EvidenceCoverage::Incomplete)?
            } else {
                enum_to_db(&event.coverage)?
            },
        })
    }
}

#[derive(Clone, Debug)]
struct StoredSummary {
    tool_name: String,
    first_seen_at_utc: String,
    last_seen_at_utc: String,
    terminal_state: String,
    outcome: Option<String>,
    policy_decision: Option<String>,
    approval_state: String,
    failure_category: Option<String>,
    execution_started: bool,
    duration_ms: Option<i64>,
    evidence_coverage: String,
}

impl StoredSummary {
    fn merge(self, incoming: SummaryProjection) -> Self {
        let terminal_state = if incoming.terminal_state == "open" {
            self.terminal_state.clone()
        } else {
            incoming.terminal_state
        };
        let outcome = incoming.outcome.or(self.outcome);
        let policy_decision = incoming.policy_decision.or(self.policy_decision);
        let approval_state = if incoming.approval_state == "none" {
            self.approval_state.clone()
        } else {
            incoming.approval_state
        };
        let failure_category = incoming.failure_category.or(self.failure_category);
        let evidence_coverage = merge_coverage(&self.evidence_coverage, &incoming.coverage);
        Self {
            tool_name: self.tool_name,
            first_seen_at_utc: self.first_seen_at_utc.min(incoming.first_seen_at_utc),
            last_seen_at_utc: self.last_seen_at_utc.max(incoming.last_seen_at_utc),
            terminal_state,
            outcome,
            policy_decision,
            approval_state,
            failure_category,
            execution_started: self.execution_started || incoming.execution_started,
            duration_ms: incoming.duration_ms.or(self.duration_ms),
            evidence_coverage,
        }
    }
}

fn load_summary(
    transaction: &Transaction<'_>,
    runtime_id: &str,
    request_id: &str,
) -> Result<Option<StoredSummary>> {
    transaction
        .query_row(
            "SELECT tool_name, first_seen_at_utc, last_seen_at_utc, terminal_state,
                    outcome, policy_decision, approval_state, failure_category,
                    execution_started, duration_ms, evidence_coverage
             FROM request_summary WHERE runtime_id = ?1 AND request_id = ?2",
            params![runtime_id, request_id],
            |row| {
                Ok(StoredSummary {
                    tool_name: row.get(0)?,
                    first_seen_at_utc: row.get(1)?,
                    last_seen_at_utc: row.get(2)?,
                    terminal_state: row.get(3)?,
                    outcome: row.get(4)?,
                    policy_decision: row.get(5)?,
                    approval_state: row.get(6)?,
                    failure_category: row.get(7)?,
                    execution_started: row.get::<_, i64>(8)? != 0,
                    duration_ms: row.get(9)?,
                    evidence_coverage: row.get(10)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from_sqlite)
}

#[derive(Clone, Debug)]
struct TerminalApproval {
    approval_id: String,
    requested_at_utc: String,
    terminal_state: String,
    decision: Option<String>,
}

fn terminal_approval(event: &EvidenceEventV1) -> Result<Option<TerminalApproval>> {
    let Some(approval_id) = event.approval_id.clone() else {
        return Ok(None);
    };
    let terminal_state = match event.stage {
        EvidenceStage::ApprovalResolved => match event.approval_decision {
            Some(ApprovalDecision::AllowOnce) => Some(HistoryApprovalState::AllowOnce),
            Some(ApprovalDecision::AllowForSession) => Some(HistoryApprovalState::AllowForSession),
            Some(ApprovalDecision::Deny) => Some(HistoryApprovalState::Denied),
            None => None,
        },
        EvidenceStage::ApprovalExpired => Some(HistoryApprovalState::Expired),
        EvidenceStage::ApprovalCancelled => Some(HistoryApprovalState::Cancelled),
        _ => None,
    };
    let Some(terminal_state) = terminal_state else {
        return Ok(None);
    };
    Ok(Some(TerminalApproval {
        approval_id,
        requested_at_utc: event.occurred_at_utc.clone(),
        terminal_state: enum_to_db(&terminal_state)?,
        decision: optional_enum_to_db(event.approval_decision.as_ref())?,
    }))
}

fn terminal_state(event: &EvidenceEventV1) -> HistoryTerminalState {
    match event.outcome {
        HistoryOutcome::Success => HistoryTerminalState::Succeeded,
        HistoryOutcome::Failed => HistoryTerminalState::Failed,
        HistoryOutcome::Denied => HistoryTerminalState::Denied,
        HistoryOutcome::Cancelled => HistoryTerminalState::Cancelled,
        HistoryOutcome::TimedOut => HistoryTerminalState::TimedOut,
        HistoryOutcome::ApprovalExpired => HistoryTerminalState::ApprovalExpired,
        HistoryOutcome::ApprovalCancelled => HistoryTerminalState::ApprovalCancelled,
        HistoryOutcome::InProgress | HistoryOutcome::ApprovalRequired => HistoryTerminalState::Open,
    }
}

fn approval_state(event: &EvidenceEventV1) -> HistoryApprovalState {
    match event.stage {
        EvidenceStage::ApprovalRequired => HistoryApprovalState::RequiredObserved,
        EvidenceStage::ApprovalResolved => match event.approval_decision {
            Some(ApprovalDecision::AllowOnce) => HistoryApprovalState::AllowOnce,
            Some(ApprovalDecision::AllowForSession) => HistoryApprovalState::AllowForSession,
            Some(ApprovalDecision::Deny) => HistoryApprovalState::Denied,
            None => HistoryApprovalState::Unknown,
        },
        EvidenceStage::ApprovalExpired => HistoryApprovalState::Expired,
        EvidenceStage::ApprovalCancelled => HistoryApprovalState::Cancelled,
        _ => HistoryApprovalState::None,
    }
}

fn merge_coverage(current: &str, incoming: &str) -> String {
    if current == "UNKNOWN" || incoming == "UNKNOWN" {
        "UNKNOWN".to_owned()
    } else if incoming == "COMPLETE" {
        // A terminal event completes an otherwise healthy request. Requests
        // with a recorded loss are forced to INCOMPLETE before this merge.
        "COMPLETE".to_owned()
    } else if current == "COMPLETE" {
        "COMPLETE".to_owned()
    } else {
        "INCOMPLETE".to_owned()
    }
}

pub(crate) fn enum_to_db<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_value(value)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            StoreError::InvalidEvidence("closed enum did not serialize as a string".to_owned())
        })
}

pub(crate) fn optional_enum_to_db<T: serde::Serialize>(
    value: Option<&T>,
) -> Result<Option<String>> {
    value.map(enum_to_db).transpose()
}

pub(crate) fn enum_from_db<T: serde::de::DeserializeOwned>(value: String) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(value)).map_err(StoreError::from)
}

pub(crate) fn optional_enum_from_db<T: serde::de::DeserializeOwned>(
    value: Option<String>,
) -> Result<Option<T>> {
    value.map(enum_from_db).transpose()
}

pub(crate) fn duration_to_i64(value: Option<u64>) -> Result<Option<i64>> {
    value
        .map(|value| {
            i64::try_from(value).map_err(|_| {
                StoreError::InvalidEvidence("duration exceeds SQLite INTEGER".to_owned())
            })
        })
        .transpose()
}

pub(crate) fn duration_from_i64(value: Option<i64>) -> Result<Option<u64>> {
    value
        .map(|value| {
            u64::try_from(value).map_err(|_| StoreError::Corrupt {
                path: None,
                reason: "negative duration in evidence database".to_owned(),
            })
        })
        .transpose()
}
