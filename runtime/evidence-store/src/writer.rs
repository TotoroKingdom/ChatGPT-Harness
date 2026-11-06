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
    now_utc, ApprovalDecision, EvidenceCoverage, EvidenceEventV1, EvidenceStage,
    EvidenceStoreStatus, HistoryApprovalState, HistoryOutcome, HistoryTerminalState, RecordOutcome,
    RequestTraceSpan, Result, RuntimeSession, StoreError, StorePaths, WriterOptions,
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
        self.record_events_with_spans(events, &[])
            .map(|(count, _)| count)
    }

    /// Commits canonical evidence and its request-local spans in one SQLite
    /// transaction.  This is the Runtime path for a terminal request where
    /// event semantics and observability projection must share commit fate.
    pub fn record_events_with_spans(
        &mut self,
        events: &[EvidenceEventV1],
        spans: &[RequestTraceSpan],
    ) -> Result<(usize, usize)> {
        if events.is_empty() && spans.is_empty() {
            return Ok((0, 0));
        }
        if spans.len() > 16 {
            return Err(StoreError::InvalidEvidence(
                "request trace exceeds the 16 span bound".to_owned(),
            ));
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(StoreError::from_sqlite)?;
        let mut inserted_events = 0usize;
        let mut last_commit = None;
        for event in events {
            validate_event(event)?;
            let request_has_known_loss = self
                .loss_marked_requests
                .contains(&(event.runtime_id.clone(), event.request_id.clone()));
            if record_event_transaction(&transaction, event, request_has_known_loss)? {
                inserted_events += 1;
                last_commit = Some(event.recorded_at_utc.clone());
            }
        }
        let (inserted_spans, span_commit) = persist_spans_transaction(&transaction, spans)?;
        if span_commit.is_some() {
            last_commit = span_commit;
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
        Ok((inserted_events, inserted_spans))
    }

    /// Atomically commits one canonical event and its request-local trace
    /// snapshots.  The event is inserted first so the span foreign key can
    /// never outlive its request summary.  This is the coherence boundary for
    /// M08: a crash can lose the whole observational update, never only one
    /// half of it.
    pub fn record_evidence(
        &mut self,
        event: &EvidenceEventV1,
        spans: &[RequestTraceSpan],
    ) -> Result<usize> {
        let (events, persisted_spans) =
            self.record_events_with_spans(std::slice::from_ref(event), spans)?;
        Ok(events + persisted_spans)
    }

    /// Associates the local M08 trace identity with an existing canonical
    /// request.  The identity is write-once; a conflicting value is rejected
    /// instead of silently joining two requests.
    pub fn set_trace_id(
        &mut self,
        runtime_id: &str,
        request_id: &str,
        trace_id: &str,
    ) -> Result<()> {
        validate_trace_identity(runtime_id, request_id, trace_id)?;
        let changed = self.connection.execute(
            "UPDATE request_summary
             SET trace_id = ?3
             WHERE runtime_id = ?1 AND request_id = ?2
               AND (trace_id IS NULL OR trace_id = ?3)",
            params![runtime_id, request_id, trace_id],
        )?;
        if changed == 0 {
            let exists: Option<Option<String>> = self
                .connection
                .query_row(
                    "SELECT trace_id FROM request_summary
                     WHERE runtime_id = ?1 AND request_id = ?2",
                    params![runtime_id, request_id],
                    |row| row.get(0),
                )
                .optional()?;
            return match exists {
                Some(Some(existing)) if existing != trace_id => Err(StoreError::InvalidEvidence(
                    "request trace identity cannot change".to_owned(),
                )),
                Some(None) => Err(StoreError::InvalidEvidence(
                    "request trace identity was not updated".to_owned(),
                )),
                None => Err(StoreError::NotFound),
                _ => Ok(()),
            };
        }
        Ok(())
    }

    /// Atomically persists bounded request-local spans using the same Runtime
    /// writer connection as canonical evidence events.
    pub fn record_spans(&mut self, spans: &[RequestTraceSpan]) -> Result<usize> {
        self.record_events_with_spans(&[], spans)
            .map(|(_, span_count)| span_count)
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
                     evidence_coverage = CASE
                         WHEN evidence_coverage = 'UNKNOWN' THEN 'UNKNOWN'
                         ELSE 'INCOMPLETE'
                     END
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

    /// Marks a request as having unknown evidence coverage after a trace
    /// mutation was dropped.  This does not invent a terminal outcome.
    pub fn mark_request_unknown(&mut self, runtime_id: &str, request_id: &str) -> Result<()> {
        self.loss_marked_requests
            .insert((runtime_id.to_owned(), request_id.to_owned()));
        self.connection
            .execute(
                "UPDATE request_summary
                 SET evidence_coverage = 'UNKNOWN'
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

fn persist_spans_transaction(
    transaction: &Transaction<'_>,
    spans: &[RequestTraceSpan],
) -> Result<(usize, Option<String>)> {
    if spans.is_empty() {
        return Ok((0, None));
    }
    let mut inserted = 0usize;
    let mut last_commit = None;
    for span in spans {
        validate_trace_span(span)?;
        let associated = transaction.execute(
            "UPDATE request_summary
             SET trace_id = ?3
             WHERE runtime_id = ?1 AND request_id = ?2
               AND (trace_id IS NULL OR trace_id = ?3)",
            params![span.runtime_id, span.request_id, span.trace_id],
        )?;
        if associated == 0 {
            let existing: Option<Option<String>> = transaction
                .query_row(
                    "SELECT trace_id FROM request_summary
                     WHERE runtime_id = ?1 AND request_id = ?2",
                    params![span.runtime_id, span.request_id],
                    |row| row.get(0),
                )
                .optional()?;
            match existing {
                Some(Some(value)) if value == span.trace_id => {}
                Some(Some(_)) => {
                    return Err(StoreError::InvalidEvidence(
                        "request trace identity cannot change".to_owned(),
                    ));
                }
                Some(None) => {
                    return Err(StoreError::InvalidEvidence(
                        "request trace identity was not updated".to_owned(),
                    ));
                }
                None => return Err(StoreError::NotFound),
            }
        }
        let changed = transaction.execute(
            "INSERT INTO request_spans(
                span_id, trace_id, parent_span_id, runtime_id, request_id,
                span_sequence, span_name, started_at_utc, ended_at_utc,
                started_offset_ms, duration_ms, status, failure_category, coverage
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(span_id) DO UPDATE SET
                ended_at_utc = excluded.ended_at_utc,
                duration_ms = excluded.duration_ms,
                status = excluded.status,
                failure_category = excluded.failure_category,
                coverage = CASE
                    WHEN request_spans.coverage = 'UNKNOWN' THEN 'UNKNOWN'
                    WHEN request_spans.coverage = 'COMPLETE'
                         AND excluded.coverage <> 'COMPLETE' THEN request_spans.coverage
                    ELSE excluded.coverage
                END
             WHERE request_spans.trace_id = excluded.trace_id
               AND request_spans.runtime_id = excluded.runtime_id
               AND request_spans.request_id = excluded.request_id
               AND request_spans.span_sequence = excluded.span_sequence
               AND (request_spans.ended_at_utc IS NULL OR excluded.ended_at_utc IS NOT NULL)
               AND (request_spans.status IN ('open','incomplete','unknown')
                    OR excluded.status NOT IN ('open','incomplete','unknown'))
               AND (request_spans.ended_at_utc IS NOT excluded.ended_at_utc
                    OR request_spans.duration_ms IS NOT excluded.duration_ms
                    OR request_spans.status IS NOT excluded.status
                    OR request_spans.failure_category IS NOT excluded.failure_category
                    OR request_spans.coverage IS NOT excluded.coverage)",
            params![
                span.span_id,
                span.trace_id,
                span.parent_span_id,
                span.runtime_id,
                span.request_id,
                i64::from(span.span_sequence),
                enum_to_db(&span.span_name)?,
                span.started_at_utc,
                span.ended_at_utc,
                span.started_offset_ms,
                duration_to_i64(span.duration_ms)?,
                enum_to_db(&span.status)?,
                optional_enum_to_db(span.failure_category.as_ref())?,
                enum_to_db(&span.coverage)?,
            ],
        )?;
        if changed == 0 {
            let existing: Option<(String, String, String, i64)> = transaction
                .query_row(
                    "SELECT trace_id, runtime_id, request_id, span_sequence
                     FROM request_spans WHERE span_id = ?1",
                    [span.span_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            if let Some((trace_id, runtime_id, request_id, sequence)) = existing {
                if trace_id != span.trace_id
                    || runtime_id != span.runtime_id
                    || request_id != span.request_id
                    || sequence != i64::from(span.span_sequence)
                {
                    return Err(StoreError::InvalidEvidence(
                        "span identity cannot change".to_owned(),
                    ));
                }
            }
        }
        if changed > 0 {
            inserted += changed as usize;
            last_commit = span
                .ended_at_utc
                .clone()
                .or_else(|| Some(span.started_at_utc.clone()));
        }
    }
    Ok((inserted, last_commit))
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

fn validate_trace_identity(runtime_id: &str, request_id: &str, trace_id: &str) -> Result<()> {
    for (name, value) in [
        ("runtime_id", runtime_id),
        ("request_id", request_id),
        ("trace_id", trace_id),
    ] {
        if value.is_empty() || value.len() > 128 {
            return Err(StoreError::InvalidEvidence(format!(
                "{name} is empty or exceeds 128 bytes"
            )));
        }
    }
    Ok(())
}

fn validate_trace_span(span: &RequestTraceSpan) -> Result<()> {
    validate_trace_identity(&span.runtime_id, &span.request_id, &span.trace_id)?;
    if span.span_id.is_empty() || span.span_id.len() > 128 {
        return Err(StoreError::InvalidEvidence(
            "span_id is empty or exceeds 128 bytes".to_owned(),
        ));
    }
    if span.span_sequence >= 16 {
        return Err(StoreError::InvalidEvidence(
            "span sequence exceeds the 16 span bound".to_owned(),
        ));
    }
    if span.span_name == harness_control_protocol::TraceSpanName::RequestWall
        && span.parent_span_id.is_some()
    {
        return Err(StoreError::InvalidEvidence(
            "request_wall root cannot have a parent span".to_owned(),
        ));
    }
    if span.started_at_utc.is_empty() || span.started_at_utc.len() > 64 {
        return Err(StoreError::InvalidEvidence(
            "span started_at_utc is empty or exceeds 64 bytes".to_owned(),
        ));
    }
    if span
        .ended_at_utc
        .as_deref()
        .is_some_and(|value| value.len() > 64)
    {
        return Err(StoreError::InvalidEvidence(
            "span ended_at_utc exceeds 64 bytes".to_owned(),
        ));
    }
    if span.started_offset_ms < 0 {
        return Err(StoreError::InvalidEvidence(
            "span started_offset_ms cannot be negative".to_owned(),
        ));
    }
    if serde_json::to_vec(span)?.len() > 8 * 1024 {
        return Err(StoreError::InvalidEvidence(
            "trace span exceeds the 8 KiB bound".to_owned(),
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
            // Only the canonical ExecutionStarted observation proves that an
            // executor actually began.  Terminal or lifecycle failure events
            // may represent a pre-execution failure and must not imply start.
            execution_started: event.stage == EvidenceStage::ExecutionStarted,
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
