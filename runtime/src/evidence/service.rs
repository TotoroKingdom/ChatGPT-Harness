use super::projector::{RuntimeEvidenceContext, SafeEvidenceProjector};
use crate::audit::{AuditEvent, AuditSink};
use chatgpt_harness_evidence_store::{
    status_for_error, EvidenceEventV1, EvidenceWriter, HistoryClearResult, RetentionPolicy,
    RuntimeSession, StoreError, WriterOptions,
};
use harness_control_protocol::{
    EvidenceErrorCode, EvidenceStoreState, EvidenceStoreStatus, HistoryConfig,
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) const EVIDENCE_QUEUE_CAPACITY: usize = 1024;
const EVIDENCE_BATCH_MAX: usize = 64;
const EVIDENCE_BATCH_WAIT: Duration = Duration::from_millis(50);
const EVIDENCE_STOP_DRAIN: Duration = Duration::from_secs(2);
const EVIDENCE_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);
const RETENTION_EVENT_INTERVAL: u64 = 256;
const RETENTION_TIME_INTERVAL: Duration = Duration::from_secs(60);
const LOSS_TRACKER_CAPACITY: usize = 256;

enum WriterCommand {
    Event(EvidenceEventV1),
    Clear(SyncSender<Result<HistoryClearResult, EvidenceErrorCode>>),
    Shutdown(SyncSender<Result<(), EvidenceErrorCode>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LostRequest {
    runtime_id: String,
    request_id: String,
}

#[derive(Default)]
struct LossTracker {
    requests: VecDeque<LostRequest>,
    global_unknown: bool,
}

impl LossTracker {
    fn record(&mut self, event: &EvidenceEventV1) {
        if self.requests.len() >= LOSS_TRACKER_CAPACITY {
            self.global_unknown = true;
            return;
        }
        let key = LostRequest {
            runtime_id: event.runtime_id.clone(),
            request_id: event.request_id.clone(),
        };
        if !self.requests.contains(&key) {
            self.requests.push_back(key);
        }
    }

    fn take(&mut self) -> (Vec<LostRequest>, bool) {
        (
            self.requests.drain(..).collect(),
            std::mem::take(&mut self.global_unknown),
        )
    }
}

struct Shared {
    status: Mutex<EvidenceStoreStatus>,
    queue_depth: AtomicUsize,
    drop_count: AtomicU64,
    persisted_drop_count: AtomicU64,
    last_drop_at_utc: Mutex<Option<String>>,
    losses: Mutex<LossTracker>,
    stopping: AtomicBool,
}

impl Shared {
    fn status(&self) -> EvidenceStoreStatus {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        status.queue_depth = self
            .queue_depth
            .load(Ordering::Acquire)
            .min(u32::MAX as usize) as u32;
        status.queue_capacity = EVIDENCE_QUEUE_CAPACITY as u32;
        status.drop_count = status
            .drop_count
            .max(self.drop_count.load(Ordering::Acquire));
        status.last_drop_at_utc = self
            .last_drop_at_utc
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .or(status.last_drop_at_utc);
        if status.drop_count > 0 && status.state == EvidenceStoreState::Healthy {
            status.state = EvidenceStoreState::Degraded;
        }
        if status.drop_count > 0 {
            status.last_error_code = Some(EvidenceErrorCode::EvidenceQueueOverflow);
        }
        status
    }

    fn record_drop(&self, event: &EvidenceEventV1) {
        let now = now_utc();
        self.drop_count.fetch_add(1, Ordering::AcqRel);
        *self
            .last_drop_at_utc
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(now);
        self.losses
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record(event);
    }

    fn mark_error(&self, code: EvidenceErrorCode) {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        status.state = EvidenceStoreState::Degraded;
        status.last_error_code = Some(code);
    }
}

pub(crate) struct EvidenceService {
    sender: Option<SyncSender<WriterCommand>>,
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl EvidenceService {
    pub(crate) fn disabled(config: HistoryConfig) -> Self {
        let shared = Arc::new(Shared {
            status: Mutex::new(status_template(
                EvidenceStoreState::Disabled,
                None,
                config,
                None,
            )),
            queue_depth: AtomicUsize::new(0),
            drop_count: AtomicU64::new(0),
            persisted_drop_count: AtomicU64::new(0),
            last_drop_at_utc: Mutex::new(None),
            losses: Mutex::new(LossTracker::default()),
            stopping: AtomicBool::new(false),
        });
        Self {
            sender: None,
            shared,
            thread: Mutex::new(None),
        }
    }

    pub(crate) fn unavailable(error: &StoreError, config: HistoryConfig) -> Self {
        let mut status = status_for_error(error, config);
        if matches!(error, StoreError::WriterBusy { .. }) {
            status.state = EvidenceStoreState::Degraded;
            status.last_error_code = Some(EvidenceErrorCode::EvidenceWriterOwnership);
        }
        Self::without_thread(status)
    }

    fn without_thread(status: EvidenceStoreStatus) -> Self {
        let shared = Arc::new(Shared {
            status: Mutex::new(status),
            queue_depth: AtomicUsize::new(0),
            drop_count: AtomicU64::new(0),
            persisted_drop_count: AtomicU64::new(0),
            last_drop_at_utc: Mutex::new(None),
            losses: Mutex::new(LossTracker::default()),
            stopping: AtomicBool::new(true),
        });
        Self {
            sender: None,
            shared,
            thread: Mutex::new(None),
        }
    }

    pub(crate) fn start(
        database_path: impl AsRef<std::path::Path>,
        config: HistoryConfig,
        context: RuntimeEvidenceContext,
        started_at_utc: String,
    ) -> Self {
        if !config.enabled {
            return Self::disabled(config);
        }
        let options = WriterOptions {
            history_config: config.clone(),
            runtime_version: context.runtime_version.clone(),
            config_revision: context.config_revision.clone(),
        };
        let mut writer = match EvidenceWriter::open_with_options(database_path, options) {
            Ok(writer) => writer,
            Err(error) => return Self::unavailable(&error, config),
        };
        let session = RuntimeSession {
            runtime_id: context.runtime_id.clone(),
            host_session_id: context.host_session_id.clone(),
            supervisor_generation_id: context.supervisor_generation_id.clone(),
            started_at_utc,
            runtime_version: context.runtime_version,
            config_revision: context.config_revision,
        };
        if let Err(error) = writer.open_session(&session) {
            return Self::unavailable(&error, config);
        }
        let initial_status = match writer.status() {
            Ok(status) => status,
            Err(error) => status_for_error(&error, config.clone()),
        };
        let shared = Arc::new(Shared {
            status: Mutex::new(initial_status),
            queue_depth: AtomicUsize::new(0),
            drop_count: AtomicU64::new(0),
            persisted_drop_count: AtomicU64::new(0),
            last_drop_at_utc: Mutex::new(None),
            losses: Mutex::new(LossTracker::default()),
            stopping: AtomicBool::new(false),
        });
        let (sender, receiver) = mpsc::sync_channel(EVIDENCE_QUEUE_CAPACITY);
        let worker_shared = Arc::clone(&shared);
        let runtime_id = session.runtime_id.clone();
        let thread = match thread::Builder::new()
            .name("harness-evidence-writer".into())
            .spawn(move || {
                run_writer(writer, receiver, worker_shared, runtime_id, config);
            }) {
            Ok(thread) => Some(thread),
            Err(_) => {
                shared.mark_error(EvidenceErrorCode::EvidenceIo);
                None
            }
        };
        let sender = thread.as_ref().map(|_| sender);
        Self {
            sender,
            shared,
            thread: Mutex::new(thread),
        }
    }

    pub(crate) fn enqueue(&self, event: EvidenceEventV1) {
        let Some(sender) = &self.sender else {
            return;
        };
        if self.shared.stopping.load(Ordering::Acquire) {
            return;
        }
        match sender.try_send(WriterCommand::Event(event)) {
            Ok(()) => {
                self.shared.queue_depth.fetch_add(1, Ordering::AcqRel);
            }
            Err(TrySendError::Full(command)) => {
                if let WriterCommand::Event(event) = command {
                    self.shared.record_drop(&event);
                }
                self.shared
                    .mark_error(EvidenceErrorCode::EvidenceQueueOverflow);
            }
            Err(TrySendError::Disconnected(command)) => {
                if let WriterCommand::Event(event) = command {
                    self.shared.record_drop(&event);
                }
                self.shared.mark_error(EvidenceErrorCode::EvidenceIo);
            }
        }
    }

    pub(crate) fn status(&self) -> EvidenceStoreStatus {
        self.shared.status()
    }

    pub(crate) fn clear_online(&self) -> Result<HistoryClearResult, EvidenceErrorCode> {
        let Some(sender) = &self.sender else {
            return Err(self
                .shared
                .status()
                .last_error_code
                .unwrap_or(EvidenceErrorCode::EvidenceIo));
        };
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        match sender.try_send(WriterCommand::Clear(response_sender)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return Err(EvidenceErrorCode::EvidenceBusy),
            Err(TrySendError::Disconnected(_)) => return Err(EvidenceErrorCode::EvidenceIo),
        }
        response_receiver
            .recv_timeout(EVIDENCE_OPERATION_TIMEOUT)
            .map_err(|_| EvidenceErrorCode::EvidenceBusy)?
    }

    /// Stops accepting events and waits for a bounded writer drain.
    pub(crate) fn stop(&self) {
        if self.sender.is_none() || self.shared.stopping.swap(true, Ordering::AcqRel) {
            return;
        }
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        let Some(sender) = self.sender.as_ref() else {
            return;
        };
        let deadline = Instant::now() + EVIDENCE_STOP_DRAIN;
        let mut command = Some(WriterCommand::Shutdown(response_sender));
        let queued = loop {
            let Some(candidate) = command.take() else {
                break false;
            };
            match sender.try_send(candidate) {
                Ok(()) => break true,
                Err(TrySendError::Full(candidate)) => {
                    if Instant::now() >= deadline {
                        break false;
                    }
                    command = Some(candidate);
                    thread::sleep(Duration::from_millis(5));
                }
                Err(TrySendError::Disconnected(_)) => break false,
            }
        };
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        if queued && response_receiver.recv_timeout(remaining).is_ok() {
            if let Some(thread) = self
                .thread
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                let _ = thread.join();
            }
        } else {
            self.shared.mark_error(EvidenceErrorCode::EvidenceIo);
        }
    }
}

impl Drop for EvidenceService {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) struct EvidenceAuditSink {
    projector: SafeEvidenceProjector,
    service: Arc<EvidenceService>,
}

impl EvidenceAuditSink {
    pub(crate) fn new(context: RuntimeEvidenceContext, service: Arc<EvidenceService>) -> Self {
        Self {
            projector: SafeEvidenceProjector::new(context),
            service,
        }
    }
}

impl AuditSink for EvidenceAuditSink {
    fn on_audit_event(&self, event: &AuditEvent) {
        self.service.enqueue(self.projector.project(event));
    }
}

fn run_writer(
    mut writer: EvidenceWriter,
    receiver: Receiver<WriterCommand>,
    shared: Arc<Shared>,
    runtime_id: String,
    config: HistoryConfig,
) {
    let retention = RetentionPolicy {
        retention_days: config.retention_days,
        max_storage_mib: config.max_storage_mib,
    };
    match writer.apply_retention(&now_utc(), &retention) {
        Ok(report) if report.capacity_degraded => {
            shared.mark_error(EvidenceErrorCode::EvidenceCapacityDegraded);
        }
        Ok(_) => {}
        Err(error) => shared.mark_error(error_code(&error, &config)),
    }
    update_status(&writer, &shared);
    let mut batch = Vec::with_capacity(EVIDENCE_BATCH_MAX);
    let mut batch_started = None::<Instant>;
    let mut committed_since_retention = 0u64;
    let mut last_retention = Instant::now();

    loop {
        let timeout = batch_started
            .map(|started| {
                EVIDENCE_BATCH_WAIT
                    .saturating_sub(started.elapsed())
                    .max(Duration::from_millis(1))
            })
            .unwrap_or(EVIDENCE_BATCH_WAIT);
        match receiver.recv_timeout(timeout) {
            Ok(WriterCommand::Event(event)) => {
                shared.queue_depth.fetch_sub(1, Ordering::AcqRel);
                batch_started.get_or_insert_with(Instant::now);
                batch.push(event);
                if batch.len() >= EVIDENCE_BATCH_MAX {
                    flush_batch(
                        &mut writer,
                        &shared,
                        &mut batch,
                        &retention,
                        &config,
                        &mut committed_since_retention,
                        &mut last_retention,
                    );
                    batch_started = None;
                }
            }
            Ok(WriterCommand::Clear(response)) => {
                flush_batch(
                    &mut writer,
                    &shared,
                    &mut batch,
                    &retention,
                    &config,
                    &mut committed_since_retention,
                    &mut last_retention,
                );
                batch_started = None;
                let result = writer
                    .clear_terminal_history()
                    .map_err(|error| error_code(&error, &config));
                if let Err(code) = &result {
                    shared.mark_error(*code);
                }
                let _ = response.send(result);
                update_status(&writer, &shared);
            }
            Ok(WriterCommand::Shutdown(response)) => {
                while let Ok(command) = receiver.try_recv() {
                    if let WriterCommand::Event(event) = command {
                        shared.queue_depth.fetch_sub(1, Ordering::AcqRel);
                        batch.push(event);
                        if batch.len() >= EVIDENCE_BATCH_MAX {
                            flush_batch(
                                &mut writer,
                                &shared,
                                &mut batch,
                                &retention,
                                &config,
                                &mut committed_since_retention,
                                &mut last_retention,
                            );
                        }
                    }
                }
                flush_batch(
                    &mut writer,
                    &shared,
                    &mut batch,
                    &retention,
                    &config,
                    &mut committed_since_retention,
                    &mut last_retention,
                );
                let result = writer
                    .close_session(&runtime_id, &now_utc(), "normal_stop")
                    .and_then(|_| writer.checkpoint())
                    .map_err(|error| error_code(&error, &config));
                if let Err(code) = &result {
                    shared.mark_error(*code);
                }
                let _ = response.send(result);
                update_status(&writer, &shared);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                if batch_started.is_some_and(|started| started.elapsed() >= EVIDENCE_BATCH_WAIT) {
                    flush_batch(
                        &mut writer,
                        &shared,
                        &mut batch,
                        &retention,
                        &config,
                        &mut committed_since_retention,
                        &mut last_retention,
                    );
                    batch_started = None;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                flush_batch(
                    &mut writer,
                    &shared,
                    &mut batch,
                    &retention,
                    &config,
                    &mut committed_since_retention,
                    &mut last_retention,
                );
                let _ = writer.close_session(&runtime_id, &now_utc(), "normal_stop");
                let _ = writer.checkpoint();
                update_status(&writer, &shared);
                break;
            }
        }
    }
}

fn flush_batch(
    writer: &mut EvidenceWriter,
    shared: &Shared,
    batch: &mut Vec<EvidenceEventV1>,
    retention: &RetentionPolicy,
    config: &HistoryConfig,
    committed_since_retention: &mut u64,
    last_retention: &mut Instant,
) {
    let current_drops = shared.drop_count.load(Ordering::Acquire);
    let persisted_drops = shared.persisted_drop_count.load(Ordering::Acquire);
    if current_drops > persisted_drops {
        let last_drop = shared
            .last_drop_at_utc
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_else(now_utc);
        let delta = current_drops - persisted_drops;
        if writer
            .record_drop_count(delta, &last_drop, "EVIDENCE_QUEUE_OVERFLOW")
            .is_ok()
        {
            shared
                .persisted_drop_count
                .store(current_drops, Ordering::Release);
        }
    }
    let (lost, global_unknown) = shared
        .losses
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    for request in lost {
        let _ = writer.mark_request_incomplete(&request.runtime_id, &request.request_id);
    }
    if global_unknown {
        let _ = writer.mark_coverage_unknown(&now_utc());
    }

    if !batch.is_empty() {
        match writer.record_events(batch) {
            Ok(count) => {
                *committed_since_retention = committed_since_retention.saturating_add(count as u64);
            }
            Err(error) => {
                let code = error_code(&error, config);
                shared.mark_error(code);
                for event in batch.iter() {
                    shared.record_drop(event);
                    let _ = writer.mark_request_incomplete(&event.runtime_id, &event.request_id);
                }
            }
        }
        batch.clear();
    }

    if *committed_since_retention >= RETENTION_EVENT_INTERVAL
        || last_retention.elapsed() >= RETENTION_TIME_INTERVAL
    {
        let report = writer.apply_retention(&now_utc(), retention);
        if report.is_err() || report.as_ref().is_ok_and(|report| report.capacity_degraded) {
            shared.mark_error(EvidenceErrorCode::EvidenceCapacityDegraded);
        }
        *committed_since_retention = 0;
        *last_retention = Instant::now();
    }
    update_status(writer, shared);
}

fn update_status(writer: &EvidenceWriter, shared: &Shared) {
    if let Ok(status) = writer.status() {
        let live = shared.status();
        let mut status = status;
        status.queue_depth = live.queue_depth;
        status.queue_capacity = live.queue_capacity;
        status.drop_count = status.drop_count.max(live.drop_count);
        status.last_drop_at_utc = live.last_drop_at_utc;
        if live.state == EvidenceStoreState::Degraded {
            status.state = EvidenceStoreState::Degraded;
        }
        if live.last_error_code.is_some() {
            status.last_error_code = live.last_error_code;
        }
        if status.drop_count > 0 {
            status.last_error_code = Some(EvidenceErrorCode::EvidenceQueueOverflow);
        }
        *shared
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = status;
    }
}

fn status_template(
    state: EvidenceStoreState,
    code: Option<EvidenceErrorCode>,
    config: HistoryConfig,
    schema_version: Option<u32>,
) -> EvidenceStoreStatus {
    EvidenceStoreStatus {
        state,
        schema_version,
        queue_depth: 0,
        queue_capacity: EVIDENCE_QUEUE_CAPACITY as u32,
        drop_count: 0,
        last_drop_at_utc: None,
        last_commit_at_utc: None,
        last_error_code: code,
        effective_history_config: config,
        footprint_bytes: None,
        coverage_unknown_since_utc: None,
    }
}

fn error_code(error: &StoreError, config: &HistoryConfig) -> EvidenceErrorCode {
    status_for_error(error, config.clone())
        .last_error_code
        .unwrap_or(EvidenceErrorCode::EvidenceIo)
}

fn now_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chatgpt_harness_evidence_store::EvidenceReader;

    fn context() -> RuntimeEvidenceContext {
        RuntimeEvidenceContext {
            runtime_id: "runtime-service".into(),
            host_session_id: Some("host-session".into()),
            supervisor_generation_id: Some("generation".into()),
            runtime_version: "0.1.3".into(),
            config_revision: "7".into(),
        }
    }

    fn evidence(index: usize) -> EvidenceEventV1 {
        EvidenceEventV1 {
            schema_version: 1,
            event_id: format!("event-{index}"),
            runtime_id: "runtime-service".into(),
            request_id: format!("request-{index}"),
            host_session_id: None,
            supervisor_generation_id: None,
            approval_id: None,
            tool_name: "read_file".into(),
            occurred_at_utc: format!("2026-11-01T00:00:{:02}.000Z", index % 60),
            recorded_at_utc: "2026-11-01T00:00:00.000Z".into(),
            stage: harness_control_protocol::EvidenceStage::RequestReceived,
            lifecycle_state: harness_control_protocol::EvidenceLifecycleState::Received,
            workspace_validation:
                harness_control_protocol::EvidenceWorkspaceValidation::NotAttempted,
            policy_decision: None,
            policy_reason: None,
            matched_rule_ids: Vec::new(),
            approval_decision: None,
            outcome: harness_control_protocol::HistoryOutcome::InProgress,
            failure_category: None,
            duration_ms: None,
            sanitized_arguments: None,
            result_evidence: None,
            coverage: harness_control_protocol::EvidenceCoverage::Incomplete,
        }
    }

    #[test]
    fn disabled_service_creates_no_database_and_reports_disabled() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let service = EvidenceService::disabled(HistoryConfig {
            enabled: false,
            ..HistoryConfig::default()
        });
        service.enqueue(evidence(1));
        assert_eq!(service.status().state, EvidenceStoreState::Disabled);
        assert!(!path.exists());
    }

    #[test]
    fn writer_thread_commits_and_clear_barrier_preserves_open_request() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let service = Arc::new(EvidenceService::start(
            &path,
            HistoryConfig::default(),
            context(),
            "2026-11-01T00:00:00.000Z".into(),
        ));
        let mut terminal = evidence(1);
        terminal.stage = harness_control_protocol::EvidenceStage::ExecutionCompleted;
        terminal.lifecycle_state = harness_control_protocol::EvidenceLifecycleState::Completed;
        terminal.outcome = harness_control_protocol::HistoryOutcome::Success;
        terminal.coverage = harness_control_protocol::EvidenceCoverage::Complete;
        service.enqueue(evidence(2));
        service.enqueue(terminal);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let reader = EvidenceReader::open(&path).unwrap();
            let result = reader.query(&Default::default()).unwrap();
            if result.items.len() >= 2 {
                break;
            }
            assert!(Instant::now() < deadline, "writer did not commit");
            std::thread::sleep(Duration::from_millis(10));
        }
        let cleared = service.clear_online().unwrap();
        assert_eq!(cleared.deleted_request_count, 1);
        let reader = EvidenceReader::open(&path).unwrap();
        let result = reader.query(&Default::default()).unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(
            result.items[0].terminal_state,
            harness_control_protocol::HistoryTerminalState::Open
        );
        service.stop();
    }

    #[test]
    fn bounded_stop_drains_enqueued_evidence_and_closes_open_requests_as_incomplete() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let service = EvidenceService::start(
            &path,
            HistoryConfig::default(),
            context(),
            "2026-11-01T00:00:00.000Z".into(),
        );
        service.enqueue(evidence(1));
        service.stop();
        let reader = EvidenceReader::open(&path).unwrap();
        let detail = reader
            .get("runtime-service", "request-1")
            .unwrap()
            .expect("drained request");
        assert_eq!(
            detail.summary.terminal_state,
            harness_control_protocol::HistoryTerminalState::Incomplete
        );
        assert_eq!(
            detail.summary.evidence_coverage,
            harness_control_protocol::EvidenceCoverage::Incomplete
        );
    }

    #[test]
    fn loss_tracker_is_bounded_and_marks_global_unknown_when_identity_mapping_overflows() {
        let mut tracker = LossTracker::default();
        for index in 0..=LOSS_TRACKER_CAPACITY {
            tracker.record(&evidence(index));
        }
        assert_eq!(tracker.requests.len(), LOSS_TRACKER_CAPACITY);
        let (requests, global_unknown) = tracker.take();
        assert_eq!(requests.len(), LOSS_TRACKER_CAPACITY);
        assert!(global_unknown);
        assert!(tracker.requests.is_empty());
        assert!(!tracker.global_unknown);
    }

    #[test]
    fn queue_overflow_is_observable_and_does_not_invent_terminal_evidence() {
        let config = HistoryConfig::default();
        let shared = Shared {
            status: Mutex::new(status_template(
                EvidenceStoreState::Healthy,
                None,
                config,
                Some(1),
            )),
            queue_depth: AtomicUsize::new(0),
            drop_count: AtomicU64::new(0),
            persisted_drop_count: AtomicU64::new(0),
            last_drop_at_utc: Mutex::new(None),
            losses: Mutex::new(LossTracker::default()),
            stopping: AtomicBool::new(false),
        };
        shared.record_drop(&evidence(1));
        let status = shared.status();
        assert_eq!(status.drop_count, 1);
        assert_eq!(status.state, EvidenceStoreState::Degraded);
        assert_eq!(
            status.last_error_code,
            Some(EvidenceErrorCode::EvidenceQueueOverflow)
        );
        let (lost, unknown) = shared
            .losses
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        assert_eq!(lost.len(), 1);
        assert!(!unknown);
    }
}
