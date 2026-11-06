//! M08 T5 production-pipeline regression tests.
//!
//! These tests deliberately enter through `RuntimeCore` and its real
//! `ExecutionPipeline`.  They do not synthesize evidence rows: the canonical
//! audit events, trace projection and one EvidenceService writer must produce
//! the rows observed by the read-only Metrics/Trace facade.

use crate::audit::AuditEventType;
use crate::state::RuntimeCore;
use crate::tools::{GlobRequest, ReadFileRequest};
use chatgpt_harness_evidence_store::{EvidenceReader, MetricsQueryParams, TraceGetParams};
use chrono::{Duration as ChronoDuration, Utc};
use harness_control_protocol::{
    ApprovalStatus, BashConfig, DecimalU64, EvidenceStoreLaunchConfig, PolicyConfig,
    RuntimeConfigSnapshot, RuntimeHostContext, RuntimeLimits, SecurityConfig, SecurityMode,
    ToolConfig, WorkspaceConfig,
};
use serde_json::json;
use std::fs;
use std::path::Path;

fn runtime_config(
    workspace: &Path,
    database: &Path,
) -> (RuntimeConfigSnapshot, RuntimeHostContext) {
    let config = RuntimeConfigSnapshot {
        config_revision: DecimalU64::new(1),
        workspace: WorkspaceConfig {
            root_path: workspace.to_string_lossy().into_owned(),
        },
        security: SecurityConfig {
            mode: SecurityMode::Normal,
            developer_test_mode: false,
        },
        runtime_limits: RuntimeLimits {
            max_concurrent_requests: 4,
            max_activity_events: 256,
            max_read_bytes: 1_048_576,
        },
        tools: ToolConfig::default(),
        bash: BashConfig::default(),
        policies: PolicyConfig::default(),
    };
    let host = RuntimeHostContext {
        host_session_id: Some("m08-real-pipeline-host".into()),
        supervisor_generation_id: Some("m08-real-pipeline-generation".into()),
        evidence_store: Some(EvidenceStoreLaunchConfig {
            enabled: true,
            database_path: database.to_string_lossy().into_owned(),
            retention_days: 30,
            max_storage_mib: 256,
        }),
    };
    (config, host)
}

#[test]
fn m08_real_runtime_pipeline_persists_canonical_trace_and_metrics() {
    let directory = tempfile::tempdir().expect("create disposable M08 runtime fixture");
    let workspace = directory.path().join("workspace");
    fs::create_dir_all(&workspace).expect("create disposable workspace");
    let database = directory.path().join("evidence.sqlite3");
    let secret = "M08_PIPELINE_RAW_CONTENT_SENTINEL";
    fs::write(workspace.join("input.txt"), secret).expect("write disposable fixture file");

    // Keep the range narrow and derived from the real clock; the metrics
    // contract caps a query at thirty days and Runtime emits UTC timestamps.
    let from = (Utc::now() - ChronoDuration::minutes(1)).to_rfc3339();
    let mut core = RuntimeCore::new();
    let runtime_id = core.runtime_id.clone();
    let (config, host) = runtime_config(&workspace, &database);
    core.initialize_with_host(config, Some(host))
        .expect("initialize production RuntimeCore with evidence");
    core.acknowledge();

    let output = core
        .read_file(&ReadFileRequest::new("input.txt"))
        .expect("allow-listed production read should succeed");
    assert_eq!(output.content, secret);

    let completed = core
        .activity_snapshot(64)
        .into_iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::ExecutionCompleted)
        .expect("real pipeline must publish an execution-completed audit observation");
    let request_id = completed.request_id.to_string();
    let to = (Utc::now() + ChronoDuration::minutes(1)).to_rfc3339();

    // RuntimeCore::stop_evidence is the controlled writer shutdown barrier;
    // no active-user database or external process is involved in this test.
    core.stop();
    core.stop_evidence();

    let reader = EvidenceReader::open(&database).expect("open persisted evidence read-only");
    let detail = reader
        .get(&runtime_id, &request_id)
        .expect("read canonical history detail")
        .expect("production request must have a summary");
    assert_eq!(detail.summary.runtime_id, runtime_id);
    assert_eq!(detail.summary.request_id, request_id);
    assert_eq!(detail.summary.tool_name, "read_file");
    assert_eq!(
        detail.events.len(),
        5,
        "canonical pipeline stages are retained"
    );

    let trace = reader
        .trace_get(&TraceGetParams {
            runtime_id: runtime_id.clone(),
            request_id: request_id.clone(),
        })
        .expect("query trace by exact canonical identity");
    assert_eq!(
        trace.availability,
        harness_control_protocol::TraceAvailability::Available
    );
    assert_eq!(
        trace.coverage,
        harness_control_protocol::TraceCoverage::Complete
    );
    assert_eq!(trace.tool_name, "read_file");
    assert_eq!(
        trace.outcome,
        Some(harness_control_protocol::HistoryOutcome::Success)
    );
    assert!(trace.executor_started);
    assert!(trace.trace_id.is_some());
    let names: Vec<_> = trace.spans.iter().map(|span| span.span_name).collect();
    for required in [
        harness_control_protocol::TraceSpanName::RequestWall,
        harness_control_protocol::TraceSpanName::RequestReceived,
        harness_control_protocol::TraceSpanName::ToolResolution,
        harness_control_protocol::TraceSpanName::WorkspaceValidation,
        harness_control_protocol::TraceSpanName::PolicyEvaluation,
        harness_control_protocol::TraceSpanName::Execution,
        harness_control_protocol::TraceSpanName::Terminal,
    ] {
        assert!(
            names.contains(&required),
            "missing real trace stage: {required:?}"
        );
    }
    let execution = trace
        .spans
        .iter()
        .find(|span| span.span_name == harness_control_protocol::TraceSpanName::Execution)
        .expect("real executor stage");
    assert!(execution.duration_ms.is_some());
    assert_eq!(
        execution.status,
        harness_control_protocol::TraceSpanStatus::Success
    );

    let metrics = reader
        .metrics_query(&MetricsQueryParams {
            from_utc: from,
            to_utc: to,
            tool: Some("read_file".into()),
        })
        .expect("query metrics from the same persisted request");
    assert_eq!(metrics.counts.received_calls, 1);
    assert_eq!(metrics.counts.execution_attempts, 1);
    assert_eq!(metrics.counts.successful_calls, 1);
    assert_eq!(metrics.counts.failed_calls, 0);
    assert_eq!(metrics.counts.completed_execution_denominator, 1);
    assert_eq!(metrics.counts.execution_success_numerator, 1);
    assert_eq!(metrics.counts.execution_success_rate, Some(1.0));
    assert_eq!(metrics.execution.sample_count, 1);
    assert!(metrics.execution.average_ms.is_some());
    assert_eq!(metrics.request_wall.sample_count, 1);
    assert_eq!(metrics.coverage.complete_request_count, 1);
    assert_eq!(metrics.coverage.legacy_untraced_request_count, 0);

    // The projection is intentionally closed and cannot leak the read result,
    // physical workspace path, or raw payload through Trace/Metrics.
    let trace_json = serde_json::to_string(&trace).expect("serialize trace projection");
    let metrics_json = serde_json::to_string(&metrics).expect("serialize metrics projection");
    for serialized in [trace_json, metrics_json] {
        assert!(!serialized.contains(secret));
        assert!(!serialized.contains(&workspace.to_string_lossy().to_string()));
    }
}

#[test]
fn m08_real_policy_deny_has_no_executor_span_or_attempt() {
    let directory = tempfile::tempdir().expect("create disposable M08 deny fixture");
    let workspace = directory.path().join("workspace");
    fs::create_dir_all(&workspace).expect("create disposable workspace");
    fs::write(workspace.join("denied.txt"), "must-not-read").expect("write deny fixture");
    let database = directory.path().join("evidence.sqlite3");
    let mut core = RuntimeCore::new();
    let runtime_id = core.runtime_id.clone();
    let (mut config, host) = runtime_config(&workspace, &database);
    config.policies.tool_defaults.read_file = harness_control_protocol::PolicyDecision::Deny;
    core.initialize_with_host(config, Some(host))
        .expect("initialize RuntimeCore with evidence");
    core.acknowledge();
    assert!(core.read_file(&ReadFileRequest::new("denied.txt")).is_err());
    let denied = core
        .activity_snapshot(64)
        .into_iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::RequestDenied)
        .expect("policy DENY must emit a canonical request-denied event");
    let request_id = denied.request_id.to_string();
    core.stop();
    core.stop_evidence();

    let reader = EvidenceReader::open(&database).expect("open deny evidence");
    let trace = reader
        .trace_get(&TraceGetParams {
            runtime_id: runtime_id.clone(),
            request_id: request_id.clone(),
        })
        .expect("read deny trace");
    assert_eq!(
        trace.outcome,
        Some(harness_control_protocol::HistoryOutcome::Denied)
    );
    assert!(!trace.executor_started);
    assert!(trace
        .spans
        .iter()
        .all(|span| span.span_name != harness_control_protocol::TraceSpanName::Execution));
    let from = (Utc::now() - ChronoDuration::minutes(1)).to_rfc3339();
    let to = (Utc::now() + ChronoDuration::minutes(1)).to_rfc3339();
    let metrics = reader
        .metrics_query(&MetricsQueryParams {
            from_utc: from,
            to_utc: to,
            tool: Some("read_file".into()),
        })
        .expect("query deny metrics");
    assert_eq!(metrics.counts.received_calls, 1);
    assert_eq!(metrics.counts.execution_attempts, 0);
    assert_eq!(metrics.counts.denied_calls, 1);
    assert_eq!(metrics.counts.pre_execution_failed_calls, 0);
}

#[test]
fn m08_real_pre_execution_validation_failure_is_not_an_executor_attempt() {
    let directory = tempfile::tempdir().expect("create disposable M08 pre-exec fixture");
    let workspace = directory.path().join("workspace");
    fs::create_dir_all(&workspace).expect("create disposable workspace");
    let database = directory.path().join("evidence.sqlite3");
    let mut core = RuntimeCore::new();
    let runtime_id = core.runtime_id.clone();
    let (config, host) = runtime_config(&workspace, &database);
    core.initialize_with_host(config, Some(host))
        .expect("initialize RuntimeCore with evidence");
    core.acknowledge();
    // Invalid glob syntax fails in ToolExecutor::validate before workspace,
    // policy or executor start; this exercises the pre-execution classifier
    // through the production pipeline rather than a hand-written row.
    assert!(core.glob(&GlobRequest::new("[")).is_err());
    let failed = core
        .activity_snapshot(64)
        .into_iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::ExecutionFailed)
        .expect("validation failure must emit canonical execution-failed event");
    let request_id = failed.request_id.to_string();
    core.stop();
    core.stop_evidence();

    let reader = EvidenceReader::open(&database).expect("open pre-exec evidence");
    let trace = reader
        .trace_get(&TraceGetParams {
            runtime_id: runtime_id.clone(),
            request_id: request_id.clone(),
        })
        .expect("read pre-exec trace");
    assert!(!trace.executor_started);
    assert!(trace
        .spans
        .iter()
        .all(|span| span.span_name != harness_control_protocol::TraceSpanName::Execution));
    let metrics = reader
        .metrics_query(&MetricsQueryParams {
            from_utc: (Utc::now() - ChronoDuration::minutes(1)).to_rfc3339(),
            to_utc: (Utc::now() + ChronoDuration::minutes(1)).to_rfc3339(),
            tool: Some("glob".into()),
        })
        .expect("query pre-exec metrics");
    assert_eq!(metrics.counts.received_calls, 1);
    assert_eq!(metrics.counts.execution_attempts, 0);
    assert_eq!(metrics.counts.pre_execution_failed_calls, 1);
    assert_eq!(metrics.counts.failed_calls, 0);
    assert_eq!(metrics.execution.sample_count, 0);
}

#[tokio::test]
async fn m08_real_approval_wait_shutdown_is_incomplete_without_executor_duration() {
    let directory = tempfile::tempdir().expect("create disposable M08 approval fixture");
    let workspace = directory.path().join("workspace");
    fs::create_dir_all(&workspace).expect("create disposable workspace");
    let database = directory.path().join("evidence.sqlite3");
    let mut core = RuntimeCore::new();
    let runtime_id = core.runtime_id.clone();
    let (mut config, host) = runtime_config(&workspace, &database);
    config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
    core.initialize_with_host(config, Some(host))
        .expect("initialize RuntimeCore with evidence");
    core.acknowledge();
    let service = core.mcp_service().expect("MCP service after Runtime ack");
    let pending_call = tokio::spawn(async move {
        service
            .call_tool_async(
                "write",
                json!({
                    "path": "approval-wait.txt",
                    "content": "M08_APPROVAL_WAIT_RAW_CONTENT_SENTINEL"
                }),
            )
            .await
    });

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while core.approval_manager().pending_count() != 1 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "real pipeline did not reach ApprovalRequired"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let pending = core.approval_manager().list(Some(ApprovalStatus::Pending));
    assert_eq!(pending.len(), 1);
    let request_id = pending[0].request_id.clone();

    // This models a controlled crash/unclean writer shutdown while approval is
    // still pending: no approval terminal event is fabricated and no executor
    // stage can be inferred.  The task is aborted only after the writer drain,
    // so this test never executes a real external process or user instance.
    core.stop_evidence();
    pending_call.abort();
    let _ = pending_call.await;

    let reader = EvidenceReader::open(&database).expect("reopen evidence after approval crash");
    let detail = reader
        .get(&runtime_id, &request_id)
        .expect("read incomplete approval request")
        .expect("approval request summary retained");
    assert_eq!(
        detail.summary.terminal_state,
        harness_control_protocol::HistoryTerminalState::Incomplete
    );
    assert_eq!(
        detail.summary.evidence_coverage,
        harness_control_protocol::EvidenceCoverage::Incomplete
    );
    assert!(detail
        .events
        .iter()
        .any(|event| event.stage == harness_control_protocol::EvidenceStage::ApprovalRequired));
    assert!(!detail
        .events
        .iter()
        .any(|event| event.stage == harness_control_protocol::EvidenceStage::ExecutionStarted));

    let trace = reader
        .trace_get(&TraceGetParams {
            runtime_id: runtime_id.clone(),
            request_id: request_id.clone(),
        })
        .expect("read incomplete trace by exact identity");
    assert_eq!(
        trace.coverage,
        harness_control_protocol::TraceCoverage::Incomplete
    );
    assert!(trace.executor_started == false);
    assert!(trace
        .spans
        .iter()
        .all(|span| span.span_name != harness_control_protocol::TraceSpanName::Execution));
    let root = trace
        .spans
        .iter()
        .find(|span| span.span_name == harness_control_protocol::TraceSpanName::RequestWall)
        .expect("open request root span");
    assert!(root.ended_at_utc.is_none());
    assert!(root.duration_ms.is_none());
    let approval = trace
        .spans
        .iter()
        .find(|span| span.span_name == harness_control_protocol::TraceSpanName::ApprovalWait)
        .expect("open approval wait span");
    assert!(approval.ended_at_utc.is_none());
    assert!(approval.duration_ms.is_none());

    let from = (Utc::now() - ChronoDuration::minutes(1)).to_rfc3339();
    let to = (Utc::now() + ChronoDuration::minutes(1)).to_rfc3339();
    let metrics = reader
        .metrics_query(&MetricsQueryParams {
            from_utc: from,
            to_utc: to,
            tool: Some("write".into()),
        })
        .expect("query incomplete approval metrics");
    assert_eq!(metrics.counts.received_calls, 1);
    assert_eq!(metrics.counts.execution_attempts, 0);
    assert_eq!(metrics.counts.incomplete_calls, 1);
    assert_eq!(metrics.execution.sample_count, 0);
    assert_eq!(metrics.approval_wait.sample_count, 0);
    assert_eq!(metrics.request_wall.sample_count, 0);
}
