//! Request-local, observation-only trace projection.
//!
//! The Runtime already emits a canonical [`AuditEvent`] for every observable
//! pipeline stage.  This module turns those events into a bounded timeline.
//! It deliberately does not inspect tool payloads, call Policy, or decide an
//! outcome.  A store integration can subscribe through [`TraceObserver`] and
//! persist the returned span mutations using the normal evidence writer.

use super::{AuditEvent, AuditEventType, AuditFailureCategory};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

pub(crate) const MAX_SPANS_PER_REQUEST: usize = 16;
pub(crate) const MAX_ACTIVE_TRACE_REQUESTS: usize = 1024;
const MAX_COMPLETED_TRACES: usize = MAX_ACTIVE_TRACE_REQUESTS;

/// Safe stage names exposed by the trace DTO.  This is intentionally closed;
/// callers cannot persist arbitrary payload-derived stage names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum TraceSpanName {
    RequestWall,
    RequestReceived,
    ToolResolution,
    WorkspaceValidation,
    PolicyEvaluation,
    ApprovalWait,
    Execution,
    Terminal,
}

impl TraceSpanName {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RequestWall => "request_wall",
            Self::RequestReceived => "request_received",
            Self::ToolResolution => "tool_resolution",
            Self::WorkspaceValidation => "workspace_validation",
            Self::PolicyEvaluation => "policy_evaluation",
            Self::ApprovalWait => "approval_wait",
            Self::Execution => "execution",
            Self::Terminal => "terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum TraceSpanStatus {
    InProgress,
    Success,
    Failed,
    Denied,
    Cancelled,
    TimedOut,
    Incomplete,
    Unknown,
}

impl TraceSpanStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Incomplete => "incomplete",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum TraceCoverage {
    Complete,
    Incomplete,
    Unknown,
    Degraded,
}

impl TraceCoverage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
            Self::Unknown => "unknown",
            Self::Degraded => "degraded",
        }
    }
}

/// A bounded, sanitized span snapshot.  The structure contains only IDs,
/// timing scalars and closed enums; no arguments, paths, output or secrets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct TraceSpan {
    pub(crate) span_id: Uuid,
    pub(crate) trace_id: Uuid,
    pub(crate) parent_span_id: Option<Uuid>,
    pub(crate) request_id: Uuid,
    pub(crate) span_sequence: u32,
    pub(crate) span_name: TraceSpanName,
    pub(crate) started_at_utc: String,
    pub(crate) ended_at_utc: Option<String>,
    pub(crate) started_offset_ms: u64,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) status: TraceSpanStatus,
    pub(crate) failure_category: Option<AuditFailureCategory>,
    pub(crate) coverage: TraceCoverage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TraceMutation {
    pub(crate) runtime_id: String,
    pub(crate) request_id: Uuid,
    pub(crate) trace_id: Uuid,
    pub(crate) span: TraceSpan,
    /// All bounded snapshots touched by this event.  Sending the complete
    /// bounded set lets a durable observer upsert a root that just closed as
    /// well as the newly-created terminal node without a second writer.
    pub(crate) spans: Vec<TraceSpan>,
}

/// A trace update observer is passive.  It cannot return a Policy decision or
/// alter execution, and is called after the canonical event has been formed.
pub(crate) trait TraceObserver: Send + Sync {
    fn on_trace_mutation(&self, mutation: &TraceMutation);
}

struct ActiveTrace {
    trace_id: Uuid,
    request_id: Uuid,
    root_span_id: Uuid,
    root_started: std::time::Instant,
    root_started_at_utc: String,
    next_sequence: u32,
    span_count: usize,
    coverage: TraceCoverage,
    spans: Vec<TraceSpan>,
    approval_span: Option<usize>,
    execution_span: Option<usize>,
}

impl ActiveTrace {
    fn new(event: &AuditEvent) -> Self {
        let trace_id = Uuid::new_v4();
        let root_span_id = Uuid::new_v4();
        let root = TraceSpan {
            span_id: root_span_id,
            trace_id,
            parent_span_id: None,
            request_id: event.request_id,
            span_sequence: 0,
            span_name: TraceSpanName::RequestWall,
            started_at_utc: event.timestamp.clone(),
            ended_at_utc: None,
            started_offset_ms: 0,
            duration_ms: None,
            status: TraceSpanStatus::InProgress,
            failure_category: None,
            coverage: TraceCoverage::Incomplete,
        };
        let coverage = if event.event_type == AuditEventType::RequestReceived {
            TraceCoverage::Incomplete
        } else {
            // A trace can be created by a late/cancelled event when the
            // earlier RequestReceived observation was dropped.  Keep that
            // fact explicit instead of fabricating a received stage.
            TraceCoverage::Unknown
        };
        Self {
            trace_id,
            request_id: event.request_id,
            root_span_id,
            root_started: std::time::Instant::now(),
            root_started_at_utc: event.timestamp.clone(),
            next_sequence: 1,
            span_count: 1,
            coverage,
            spans: vec![root],
            approval_span: None,
            execution_span: None,
        }
    }

    fn offset_ms(&self) -> u64 {
        (self
            .root_started
            .elapsed()
            .as_millis()
            .min(u64::MAX as u128) as u64)
    }

    fn new_span(
        &mut self,
        name: TraceSpanName,
        event: &AuditEvent,
        status: TraceSpanStatus,
        parent: Option<Uuid>,
    ) -> Option<usize> {
        if self.span_count >= MAX_SPANS_PER_REQUEST {
            self.coverage = TraceCoverage::Degraded;
            return None;
        }
        let span = TraceSpan {
            span_id: Uuid::new_v4(),
            trace_id: self.trace_id,
            parent_span_id: parent.or(Some(self.root_span_id)),
            request_id: self.request_id,
            span_sequence: self.next_sequence,
            span_name: name,
            started_at_utc: event.timestamp.clone(),
            ended_at_utc: None,
            started_offset_ms: self.offset_ms(),
            duration_ms: None,
            status,
            failure_category: None,
            coverage: if status == TraceSpanStatus::InProgress {
                self.coverage
            } else {
                closed_coverage(self.coverage)
            },
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.span_count += 1;
        self.spans.push(span);
        Some(self.spans.len() - 1)
    }

    fn close_span(
        &mut self,
        index: usize,
        event: &AuditEvent,
        status: TraceSpanStatus,
        failure_category: Option<AuditFailureCategory>,
    ) {
        let duration_ms = (self
            .root_started
            .elapsed()
            .as_millis()
            .min(u64::MAX as u128) as u64)
            .saturating_sub(self.spans[index].started_offset_ms);
        let span = &mut self.spans[index];
        span.ended_at_utc = Some(event.timestamp.clone());
        span.duration_ms = Some(duration_ms);
        span.status = status;
        span.failure_category = failure_category;
        span.coverage = closed_coverage(self.coverage);
    }

    fn root_index(&self) -> usize {
        0
    }

    fn close_root(&mut self, event: &AuditEvent, status: TraceSpanStatus) {
        let duration_ms = self
            .root_started
            .elapsed()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        let root = &mut self.spans[0];
        root.ended_at_utc = Some(event.timestamp.clone());
        root.duration_ms = Some(duration_ms);
        root.status = status;
        root.failure_category = safe_failure_category(event.failure_category);
        root.coverage = self.coverage;
        if self.coverage != TraceCoverage::Unknown && self.coverage != TraceCoverage::Degraded {
            self.coverage = TraceCoverage::Complete;
            for span in &mut self.spans {
                if span.coverage == TraceCoverage::Incomplete {
                    span.coverage = self.coverage;
                }
            }
        }
    }
}

struct ProjectorState {
    active: HashMap<Uuid, ActiveTrace>,
    completed: VecDeque<ActiveTrace>,
    untracked: HashSet<Uuid>,
}

/// Projects canonical events into a bounded local timeline.
pub(crate) struct TraceProjector {
    runtime_id: String,
    state: Mutex<ProjectorState>,
    observer: Option<Arc<dyn TraceObserver>>,
    dropped_count: AtomicU64,
}

impl TraceProjector {
    pub(crate) fn new(runtime_id: impl Into<String>) -> Self {
        Self {
            runtime_id: runtime_id.into(),
            state: Mutex::new(ProjectorState {
                active: HashMap::new(),
                completed: VecDeque::new(),
                untracked: HashSet::new(),
            }),
            observer: None,
            dropped_count: AtomicU64::new(0),
        }
    }

    pub(crate) fn with_observer(mut self, observer: Arc<dyn TraceObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Applies one canonical event and returns the latest span snapshot.
    /// `None` means the active-request cap was exhausted; execution is still
    /// unaffected and the caller can surface degraded coverage.
    pub(crate) fn project(&self, event: &AuditEvent) -> Option<TraceMutation> {
        let (mutation, completed) = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let key = event.request_id;
            if state.untracked.contains(&key)
                || state.completed.iter().any(|trace| trace.request_id == key)
            {
                return None;
            }
            if !state.active.contains_key(&key) {
                if event.event_type != AuditEventType::RequestReceived {
                    // A late event cannot safely establish the request root;
                    // retain the key so a later event cannot create a second,
                    // fabricated trace identity.
                    if state.untracked.len() >= MAX_ACTIVE_TRACE_REQUESTS {
                        state.untracked.clear();
                    }
                    state.untracked.insert(key);
                    return None;
                }
                if state.active.len() >= MAX_ACTIVE_TRACE_REQUESTS {
                    self.dropped_count.fetch_add(1, Ordering::Relaxed);
                    if state.untracked.len() >= MAX_ACTIVE_TRACE_REQUESTS {
                        state.untracked.clear();
                    }
                    state.untracked.insert(key);
                    let trace_id = Uuid::new_v4();
                    let root = TraceSpan {
                        span_id: Uuid::new_v4(),
                        trace_id,
                        parent_span_id: None,
                        request_id: key,
                        span_sequence: 0,
                        span_name: TraceSpanName::RequestWall,
                        started_at_utc: event.timestamp.clone(),
                        ended_at_utc: None,
                        started_offset_ms: 0,
                        duration_ms: None,
                        status: TraceSpanStatus::Unknown,
                        failure_category: None,
                        coverage: TraceCoverage::Unknown,
                    };
                    let mutation = TraceMutation {
                        runtime_id: self.runtime_id.clone(),
                        request_id: key,
                        trace_id,
                        span: root.clone(),
                        spans: vec![root],
                    };
                    drop(state);
                    if let Some(observer) = &self.observer {
                        observer.on_trace_mutation(&mutation);
                    }
                    return Some(mutation);
                }
                state.active.insert(key, ActiveTrace::new(event));
            }
            let trace = state.active.get_mut(&key).expect("trace inserted");
            project_event(trace, event);
            let latest = trace.spans.last().cloned().expect("root span exists");
            let spans = trace.spans.clone();
            let completed = is_terminal(event);
            let mutation = TraceMutation {
                runtime_id: self.runtime_id.clone(),
                request_id: event.request_id,
                trace_id: trace.trace_id,
                span: latest,
                spans,
            };
            if completed {
                let trace = state.active.remove(&key).expect("trace exists");
                state.completed.push_back(trace);
                while state.completed.len() > MAX_COMPLETED_TRACES {
                    state.completed.pop_front();
                }
            }
            (mutation, completed)
        };
        let _ = completed;
        if let Some(observer) = &self.observer {
            observer.on_trace_mutation(&mutation);
        }
        Some(mutation)
    }

    pub(crate) fn trace(&self, request_id: Uuid) -> Option<Vec<TraceSpan>> {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state
            .active
            .get(&request_id)
            .or_else(|| {
                state
                    .completed
                    .iter()
                    .find(|trace| trace.request_id == request_id)
            })
            .map(|trace| trace.spans.clone())
    }

    pub(crate) fn active_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active
            .len()
    }

    pub(crate) fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }
}

impl super::AuditSink for TraceProjector {
    fn on_audit_event(&self, event: &AuditEvent) {
        let _ = self.project(event);
    }
}

impl TraceSpan {
    /// Converts the internal scalar projection into the additive Control
    /// Protocol DTO used by the read-only evidence facade.
    pub(crate) fn to_protocol(
        &self,
        runtime_id: &str,
    ) -> harness_control_protocol::RequestTraceSpan {
        use harness_control_protocol::{EvidenceCoverage, TraceSpanStatus as WireStatus};
        harness_control_protocol::RequestTraceSpan {
            span_id: self.span_id.to_string(),
            trace_id: self.trace_id.to_string(),
            parent_span_id: self.parent_span_id.map(|value| value.to_string()),
            runtime_id: runtime_id.to_owned(),
            request_id: self.request_id.to_string(),
            span_sequence: self.span_sequence,
            span_name: match self.span_name {
                TraceSpanName::RequestWall => harness_control_protocol::TraceSpanName::RequestWall,
                TraceSpanName::RequestReceived => {
                    harness_control_protocol::TraceSpanName::RequestReceived
                }
                TraceSpanName::ToolResolution => {
                    harness_control_protocol::TraceSpanName::ToolResolution
                }
                TraceSpanName::WorkspaceValidation => {
                    harness_control_protocol::TraceSpanName::WorkspaceValidation
                }
                TraceSpanName::PolicyEvaluation => {
                    harness_control_protocol::TraceSpanName::PolicyEvaluation
                }
                TraceSpanName::ApprovalWait => {
                    harness_control_protocol::TraceSpanName::ApprovalWait
                }
                TraceSpanName::Execution => harness_control_protocol::TraceSpanName::Execution,
                TraceSpanName::Terminal => harness_control_protocol::TraceSpanName::Terminal,
            },
            started_at_utc: self.started_at_utc.clone(),
            ended_at_utc: self.ended_at_utc.clone(),
            started_offset_ms: self.started_offset_ms.min(i64::MAX as u64) as i64,
            duration_ms: self.duration_ms,
            status: match self.status {
                TraceSpanStatus::InProgress => WireStatus::Open,
                TraceSpanStatus::Success => WireStatus::Success,
                TraceSpanStatus::Failed => WireStatus::Failed,
                TraceSpanStatus::Denied => WireStatus::Denied,
                TraceSpanStatus::Cancelled => WireStatus::Cancelled,
                TraceSpanStatus::TimedOut => WireStatus::TimedOut,
                TraceSpanStatus::Incomplete => WireStatus::Incomplete,
                TraceSpanStatus::Unknown => WireStatus::Unknown,
            },
            failure_category: self.failure_category.map(failure_category_to_wire),
            coverage: match self.coverage {
                TraceCoverage::Complete => EvidenceCoverage::Complete,
                TraceCoverage::Incomplete => EvidenceCoverage::Incomplete,
                TraceCoverage::Unknown | TraceCoverage::Degraded => EvidenceCoverage::Unknown,
            },
        }
    }
}

fn project_event(trace: &mut ActiveTrace, event: &AuditEvent) {
    match event.event_type {
        AuditEventType::RequestReceived => {
            let _ = trace.new_span(
                TraceSpanName::RequestReceived,
                event,
                TraceSpanStatus::Success,
                None,
            );
            // Resolution is a Runtime-observable boundary.  The registry has
            // already supplied the capability on this event; no payload is
            // copied into the trace.
            let _ = trace.new_span(
                TraceSpanName::ToolResolution,
                event,
                if event.tool_enabled == Some(true) && event.capability.is_some() {
                    TraceSpanStatus::Success
                } else {
                    TraceSpanStatus::Failed
                },
                None,
            );
        }
        AuditEventType::WorkspaceValidated => {
            let status = if event.workspace_validation == super::WorkspaceValidation::Passed {
                TraceSpanStatus::Success
            } else {
                TraceSpanStatus::Failed
            };
            let _ = trace.new_span(TraceSpanName::WorkspaceValidation, event, status, None);
        }
        AuditEventType::PolicyEvaluated => {
            let status = match event.policy_decision {
                Some(harness_control_protocol::PolicyDecision::Deny) => TraceSpanStatus::Denied,
                _ => TraceSpanStatus::Success,
            };
            let _ = trace.new_span(TraceSpanName::PolicyEvaluation, event, status, None);
        }
        AuditEventType::ApprovalRequired => {
            if trace.approval_span.is_none() {
                trace.approval_span = trace.new_span(
                    TraceSpanName::ApprovalWait,
                    event,
                    TraceSpanStatus::InProgress,
                    None,
                );
            }
        }
        AuditEventType::ApprovalResolved
        | AuditEventType::ApprovalExpired
        | AuditEventType::ApprovalCancelled => {
            if let Some(index) = trace.approval_span.take() {
                let status = match event.event_type {
                    AuditEventType::ApprovalExpired => TraceSpanStatus::TimedOut,
                    AuditEventType::ApprovalCancelled => TraceSpanStatus::Cancelled,
                    AuditEventType::ApprovalResolved
                        if event.approval_decision
                            == Some(harness_control_protocol::ApprovalDecision::Deny) =>
                    {
                        TraceSpanStatus::Denied
                    }
                    _ => TraceSpanStatus::Success,
                };
                trace.close_span(
                    index,
                    event,
                    status,
                    safe_failure_category(event.failure_category),
                );
            }
        }
        AuditEventType::ExecutionStarted => {
            if trace.execution_span.is_none() {
                trace.execution_span = trace.new_span(
                    TraceSpanName::Execution,
                    event,
                    TraceSpanStatus::InProgress,
                    None,
                );
            }
        }
        AuditEventType::ExecutionCompleted | AuditEventType::ExecutionFailed => {
            if let Some(index) = trace.execution_span.take() {
                let status = execution_status(event);
                trace.close_span(
                    index,
                    event,
                    status,
                    safe_failure_category(event.failure_category),
                );
            } else if event.failure_category == Some(AuditFailureCategory::BashUnavailable) {
                // Bash can fail its Runtime-generation availability check
                // after workspace validation but before the executor starts.
                // Keep that pre-execution failure on the existing resolver
                // node; fabricating an execution span would claim work that
                // never began and hide the truthful failure stage.
                if let Some(index) = trace
                    .spans
                    .iter()
                    .position(|span| span.span_name == TraceSpanName::ToolResolution)
                {
                    trace.close_span(
                        index,
                        event,
                        TraceSpanStatus::Failed,
                        safe_failure_category(event.failure_category),
                    );
                }
            }
        }
        AuditEventType::RequestDenied => {
            if let Some(index) = trace.execution_span.take() {
                trace.close_span(
                    index,
                    event,
                    TraceSpanStatus::Denied,
                    safe_failure_category(event.failure_category),
                );
            }
        }
        AuditEventType::SessionGrantMatched => {}
    }

    if is_terminal(event) {
        let status = terminal_status(event);
        trace.close_root(event, status);
        let _ = trace.new_span(TraceSpanName::Terminal, event, status, None);
    }
}

fn execution_status(event: &AuditEvent) -> TraceSpanStatus {
    match event.failure_category {
        Some(AuditFailureCategory::BashTimeout | AuditFailureCategory::GrepTimeout) => {
            TraceSpanStatus::TimedOut
        }
        Some(AuditFailureCategory::BashCancelled | AuditFailureCategory::GrepCancelled) => {
            TraceSpanStatus::Cancelled
        }
        _ if event.event_type == AuditEventType::ExecutionCompleted => TraceSpanStatus::Success,
        _ => TraceSpanStatus::Failed,
    }
}

fn terminal_status(event: &AuditEvent) -> TraceSpanStatus {
    match event.event_type {
        AuditEventType::ExecutionCompleted => TraceSpanStatus::Success,
        AuditEventType::RequestDenied => TraceSpanStatus::Denied,
        AuditEventType::ApprovalExpired => TraceSpanStatus::TimedOut,
        AuditEventType::ApprovalCancelled => TraceSpanStatus::Cancelled,
        AuditEventType::ExecutionFailed => execution_status(event),
        AuditEventType::WorkspaceValidated
            if event.workspace_validation == super::WorkspaceValidation::Failed =>
        {
            TraceSpanStatus::Failed
        }
        AuditEventType::ApprovalResolved
            if event.approval_decision
                == Some(harness_control_protocol::ApprovalDecision::Deny) =>
        {
            TraceSpanStatus::Denied
        }
        _ => TraceSpanStatus::Incomplete,
    }
}

fn safe_failure_category(value: Option<AuditFailureCategory>) -> Option<AuditFailureCategory> {
    value
}

fn closed_coverage(value: TraceCoverage) -> TraceCoverage {
    match value {
        TraceCoverage::Unknown => TraceCoverage::Unknown,
        TraceCoverage::Degraded => TraceCoverage::Degraded,
        TraceCoverage::Complete | TraceCoverage::Incomplete => TraceCoverage::Complete,
    }
}

fn failure_category_to_wire(
    value: AuditFailureCategory,
) -> harness_control_protocol::control_read::ActivityFailureCategory {
    use harness_control_protocol::control_read::ActivityFailureCategory as W;
    match value {
        AuditFailureCategory::ApprovalDenied => W::ApprovalDenied,
        AuditFailureCategory::ApprovalExpired => W::ApprovalExpired,
        AuditFailureCategory::ApprovalCancelled => W::ApprovalCancelled,
        AuditFailureCategory::ToolDisabled => W::ToolDisabled,
        AuditFailureCategory::TargetChanged => W::TargetChanged,
        AuditFailureCategory::AtomicFailure => W::AtomicFailure,
        AuditFailureCategory::IoFailure => W::IoFailure,
        AuditFailureCategory::WorkspaceViolation => W::WorkspaceViolation,
        AuditFailureCategory::InvalidArguments => W::InvalidArguments,
        AuditFailureCategory::ExecutionFailed => W::ExecutionFailed,
        AuditFailureCategory::BashUnavailable => W::BashUnavailable,
        AuditFailureCategory::BashInvalidCommand => W::BashInvalidCommand,
        AuditFailureCategory::BashCommandTooLong => W::BashCommandTooLong,
        AuditFailureCategory::BashTimeout => W::BashTimeout,
        AuditFailureCategory::BashOutputLimit => W::BashOutputLimit,
        AuditFailureCategory::BashConcurrencyLimit => W::BashConcurrencyLimit,
        AuditFailureCategory::BashSpawnFailed => W::BashSpawnFailed,
        AuditFailureCategory::BashCancelled => W::BashCancelled,
        AuditFailureCategory::BashProcessFailed => W::BashProcessFailed,
        AuditFailureCategory::GrepTimeout => W::GrepTimeout,
        AuditFailureCategory::GrepCancelled => W::GrepCancelled,
    }
}

fn is_terminal(event: &AuditEvent) -> bool {
    matches!(
        event.event_type,
        AuditEventType::ExecutionCompleted
            | AuditEventType::ExecutionFailed
            | AuditEventType::RequestDenied
            | AuditEventType::ApprovalExpired
            | AuditEventType::ApprovalCancelled
    ) || (event.event_type == AuditEventType::WorkspaceValidated
        && event.workspace_validation == super::WorkspaceValidation::Failed)
        || (event.event_type == AuditEventType::ApprovalResolved
            && event.approval_decision == Some(harness_control_protocol::ApprovalDecision::Deny))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditLifecycleState, AuditOutcome, WorkspaceValidation};
    use harness_control_protocol::control_read::ToolCapability;

    fn event(kind: AuditEventType, request_id: Uuid) -> AuditEvent {
        AuditEvent {
            tool_enabled: Some(true),
            capability: Some(ToolCapability::WorkspaceRead),
            sanitized_arguments: None,
            approval_id: None,
            duration_ms: None,
            result_evidence: None,
            event_id: Uuid::new_v4(),
            request_id,
            timestamp: "2026-11-06T00:00:00.000Z".into(),
            event_type: kind,
            tool_name: "read_file".into(),
            lifecycle_state: AuditLifecycleState::Received,
            workspace_id: "active".into(),
            workspace_validation: WorkspaceValidation::Passed,
            policy_decision: None,
            matched_rule_ids: Vec::new(),
            policy_reason: None,
            approval_decision: None,
            outcome: AuditOutcome::InProgress,
            failure_category: None,
            bash: None,
        }
    }

    #[test]
    fn allow_success_has_bounded_root_and_execution_timeline() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        projector.project(&event(AuditEventType::RequestReceived, request));
        projector.project(&event(AuditEventType::WorkspaceValidated, request));
        projector.project(&event(AuditEventType::PolicyEvaluated, request));
        projector.project(&event(AuditEventType::ExecutionStarted, request));
        let mut terminal = event(AuditEventType::ExecutionCompleted, request);
        terminal.outcome = AuditOutcome::Success;
        projector.project(&terminal);
        let spans = projector.trace(request).expect("completed trace");
        assert!(spans.len() <= MAX_SPANS_PER_REQUEST);
        assert_eq!(spans[0].span_name, TraceSpanName::RequestWall);
        assert_eq!(spans[0].status, TraceSpanStatus::Success);
        assert!(spans
            .iter()
            .any(|span| span.span_name == TraceSpanName::Execution));
    }

    #[test]
    fn deny_never_fabricates_execution_span() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        projector.project(&event(AuditEventType::RequestReceived, request));
        projector.project(&event(AuditEventType::PolicyEvaluated, request));
        let mut denied = event(AuditEventType::RequestDenied, request);
        denied.outcome = AuditOutcome::Denied;
        projector.project(&denied);
        let spans = projector.trace(request).unwrap();
        assert!(!spans
            .iter()
            .any(|span| span.span_name == TraceSpanName::Execution));
        assert_eq!(spans[0].status, TraceSpanStatus::Denied);
    }

    #[test]
    fn disabled_or_unknown_tool_resolution_is_explicitly_failed() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        let mut received = event(AuditEventType::RequestReceived, request);
        received.tool_enabled = Some(false);
        received.capability = Some(ToolCapability::WorkspaceRead);
        projector.project(&received);
        let spans = projector.trace(request).unwrap();
        let resolution = spans
            .iter()
            .find(|span| span.span_name == TraceSpanName::ToolResolution)
            .unwrap();
        assert_eq!(resolution.status, TraceSpanStatus::Failed);
        assert!(!spans
            .iter()
            .any(|span| span.span_name == TraceSpanName::Execution));
    }

    #[test]
    fn bash_unavailable_before_execution_closes_resolution_as_failure() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        projector.project(&event(AuditEventType::RequestReceived, request));
        projector.project(&event(AuditEventType::WorkspaceValidated, request));
        let mut failed = event(AuditEventType::ExecutionFailed, request);
        failed.failure_category = Some(AuditFailureCategory::BashUnavailable);
        projector.project(&failed);
        let spans = projector.trace(request).expect("completed trace");
        let resolution = spans
            .iter()
            .find(|span| span.span_name == TraceSpanName::ToolResolution)
            .expect("tool resolution span");
        assert_eq!(resolution.status, TraceSpanStatus::Failed);
        assert_eq!(
            resolution.failure_category,
            Some(AuditFailureCategory::BashUnavailable)
        );
        assert!(resolution.ended_at_utc.is_some());
        assert!(resolution.duration_ms.is_some());
        assert!(!spans
            .iter()
            .any(|span| span.span_name == TraceSpanName::Execution));
    }

    #[test]
    fn open_execution_remains_incomplete_without_guessed_end_time() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        projector.project(&event(AuditEventType::RequestReceived, request));
        projector.project(&event(AuditEventType::ExecutionStarted, request));
        let spans = projector.trace(request).unwrap();
        assert!(spans[0].ended_at_utc.is_none());
        assert!(spans[0].duration_ms.is_none());
        let execution = spans
            .iter()
            .find(|span| span.span_name == TraceSpanName::Execution)
            .unwrap();
        assert_eq!(execution.status, TraceSpanStatus::InProgress);
        assert!(execution.ended_at_utc.is_none());
        assert!(execution.duration_ms.is_none());
    }

    #[test]
    fn active_capacity_yields_unknown_root_without_later_fabrication() {
        let projector = TraceProjector::new("runtime-a");
        for _ in 0..MAX_ACTIVE_TRACE_REQUESTS {
            let request = Uuid::new_v4();
            assert!(projector
                .project(&event(AuditEventType::RequestReceived, request))
                .is_some());
        }
        let request = Uuid::new_v4();
        let mutation = projector
            .project(&event(AuditEventType::RequestReceived, request))
            .expect("capacity degradation keeps a bounded identity");
        assert_eq!(mutation.span.status, TraceSpanStatus::Unknown);
        assert_eq!(mutation.span.coverage, TraceCoverage::Unknown);
        assert!(projector
            .project(&event(AuditEventType::ExecutionStarted, request))
            .is_none());
        assert!(projector.trace(request).is_none());
    }

    #[test]
    fn approval_wait_is_separate_from_execution_and_closes_on_deny() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        projector.project(&event(AuditEventType::RequestReceived, request));
        projector.project(&event(AuditEventType::ApprovalRequired, request));
        let mut resolved = event(AuditEventType::ApprovalResolved, request);
        resolved.approval_decision = Some(harness_control_protocol::ApprovalDecision::Deny);
        projector.project(&resolved);
        let spans = projector.trace(request).unwrap();
        let approval = spans
            .iter()
            .find(|span| span.span_name == TraceSpanName::ApprovalWait)
            .unwrap();
        assert_eq!(approval.status, TraceSpanStatus::Denied);
        assert!(approval.duration_ms.is_some());
        assert!(!spans
            .iter()
            .any(|span| span.span_name == TraceSpanName::Execution));
    }

    #[test]
    fn approval_wait_is_complete_even_when_execution_remains_open() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        projector.project(&event(AuditEventType::RequestReceived, request));
        projector.project(&event(AuditEventType::ApprovalRequired, request));
        let mut allowed = event(AuditEventType::ApprovalResolved, request);
        allowed.approval_decision = Some(harness_control_protocol::ApprovalDecision::AllowOnce);
        projector.project(&allowed);
        projector.project(&event(AuditEventType::ExecutionStarted, request));
        let spans = projector.trace(request).unwrap();
        let approval = spans
            .iter()
            .find(|span| span.span_name == TraceSpanName::ApprovalWait)
            .unwrap();
        assert_eq!(approval.status, TraceSpanStatus::Success);
        assert_eq!(approval.coverage, TraceCoverage::Complete);
        assert!(approval.duration_ms.is_some());
        let execution = spans
            .iter()
            .find(|span| span.span_name == TraceSpanName::Execution)
            .unwrap();
        assert!(execution.duration_ms.is_none());
    }

    #[test]
    fn missing_received_event_is_unknown_but_does_not_guess_payload() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        let mut terminal = event(AuditEventType::ExecutionFailed, request);
        terminal.capability = None;
        terminal.failure_category = Some(AuditFailureCategory::ExecutionFailed);
        assert!(projector.project(&terminal).is_none());
        assert!(projector.trace(request).is_none());
    }

    #[test]
    fn privacy_projection_contains_no_payload_fields() {
        let projector = TraceProjector::new("runtime-a");
        let request = Uuid::new_v4();
        let mut source = event(AuditEventType::RequestReceived, request);
        source.sanitized_arguments = Some(
            harness_control_protocol::SanitizedArgumentsEvidence::ReadFile {
                logical_path: "C:\\PRIVATE\\secret.txt".into(),
            },
        );
        let mutation = projector.project(&source).unwrap();
        let serialized = serde_json::to_string(&mutation.span).unwrap();
        assert!(!serialized.contains("PRIVATE"));
        assert!(!serialized.contains("secret.txt"));
        assert!(!serialized.contains("sanitizedArguments"));
    }
}
