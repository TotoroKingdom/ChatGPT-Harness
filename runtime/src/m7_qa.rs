//! M7 Runtime-owned Integration QA.
//!
//! These cases deliberately exercise the production RuntimeCore, MCP adapter,
//! ActivityStore and ExecutionPipeline together.  They are kept in a separate
//! test module so the independent M7 QA run has an explicit case-to-evidence
//! mapping; the lower-level M0-M6 tests remain useful regression coverage but
//! are not used as a substitute for these assertions.

use crate::approval::{ApprovalClock, ApprovalManager};
use crate::audit::{AuditEventType, AuditFailureCategory, AuditOutcome};
use crate::control_read;
use crate::execution::{ExecutionPipeline, SecurityContext};
use crate::mcp::McpService;
use crate::policy::PolicyEngine;
use crate::session_grant::SessionGrantStore;
use crate::state::RuntimeCore;
use crate::tools::{ToolAvailability, ToolRegistry, WriteRequest};
use crate::workspace::{Workspace, WorkspaceGuard};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use harness_control_protocol::control_read::{
    ToolCapability, DIAGNOSTICS_GET, EFFECTIVE_TOOLS_GET,
};
use harness_control_protocol::{
    ApprovalDecision, ApprovalStatus, DecimalU64, ExecutionResultEvidence, PolicyDecision,
    PolicyMatcherConfig, PolicyReason, RuntimeConfigSnapshot, RuntimeLimits,
    SanitizedArgumentsEvidence, SecurityConfig, SecurityMode, ToolConfig, ToolStatusReason,
    UserPolicyRuleConfig, WorkspaceConfig, WriteDisposition,
};
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Barrier, Mutex, OnceLock};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

type QaResult<T = ()> = Result<T, String>;

static QA_EVIDENCE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn qa_evidence_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("m7-qa")
        .join("runtime-integration")
        .join("evidence.jsonl")
}

fn reset_qa_evidence() {
    let path = qa_evidence_path();
    fs::create_dir_all(path.parent().expect("QA evidence parent")).expect("create QA evidence dir");
    fs::write(path, b"").expect("reset QA evidence file");
}

/// Append only typed, bounded, non-secret projections. This logger intentionally
/// never accepts raw MCP results, command output, environment values, or raw
/// tool arguments. Activity entries are summarized through their already
/// sanitized fields and scalar result evidence below.
fn log_evidence(case_id: &str, event: &str, fields: Value) {
    let lock = QA_EVIDENCE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|error| error.into_inner());
    let mut record = serde_json::Map::new();
    record.insert("case".into(), Value::String(case_id.into()));
    record.insert("event".into(), Value::String(event.into()));
    record.insert("fields".into(), fields);
    let line = Value::Object(record);
    let path = qa_evidence_path();
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open QA evidence file");
    writeln!(
        output,
        "{}",
        serde_json::to_string(&line).expect("serialize QA evidence")
    )
    .expect("write QA evidence");
}

fn safe_activity_snapshot(core: &RuntimeCore) -> Value {
    let entries = core
        .activity_snapshot(256)
        .into_iter()
        .map(|entry| {
            json!({
                "requestId": entry.request_id,
                "approvalId": entry.approval_id,
                "eventType": format!("{:?}", entry.event_type),
                "tool": entry.tool_name,
                "toolEnabled": entry.tool_enabled,
                "capability": entry.capability.map(|value| format!("{:?}", value)),
                "decision": entry.decision_summary.map(|value| format!("{:?}", value)),
                "matchedRuleCount": entry.matched_rule_ids.len(),
                "policyReason": entry.policy_reason.map(|value| format!("{:?}", value)),
                "outcome": format!("{:?}", entry.outcome_summary),
                "failureCategory": entry.failure_category.map(|value| format!("{:?}", value)),
                "durationMs": entry.duration_ms,
                "approvalDecision": entry.approval_decision.map(|value| format!("{:?}", value)),
                "sanitizedArguments": entry.sanitized_arguments,
                "resultEvidence": entry.result_evidence,
                "bash": entry.bash.map(|bash| json!({
                    "mode": format!("{:?}", bash.mode),
                    "cwd": format!("{:?}", bash.cwd),
                    "classification": bash.classification.map(|value| format!("{:?}", value)),
                    "riskFlags": bash.risk_flags.iter().map(|value| format!("{:?}", value)).collect::<Vec<_>>(),
                    "approvalRequestId": bash.approval_request_id,
                    "sessionGrantHit": bash.session_grant_hit,
                    "exitCode": bash.exit_code,
                    "durationMs": bash.duration_ms,
                    "commandPreviewLength": bash.command_preview.map(|value| value.len()),
                })),
            })
        })
        .collect::<Vec<_>>();
    Value::Array(entries)
}

fn log_activity(case_id: &str, event: &str, core: &RuntimeCore) {
    let status = core.status().map(|status| {
        json!({
            "activeRequests": status.active_requests,
            "pendingApprovals": status.pending_approvals,
            "runningExecutions": status.running_executions,
            "configRevision": status.config_revision,
        })
    });
    log_evidence(
        case_id,
        event,
        json!({
            "counters": status,
            "activity": safe_activity_snapshot(core),
        }),
    );
}

macro_rules! check {
    ($condition:expr $(,)?) => {
        if !$condition {
            return Err(format!("assertion failed: {}", stringify!($condition)));
        }
    };
    ($condition:expr, $($argument:tt)*) => {
        if !$condition {
            return Err(format!($($argument)*));
        }
    };
}

struct QaWorkspace {
    base: PathBuf,
    root: PathBuf,
}

struct QaManualApprovalClock {
    now: Mutex<DateTime<Utc>>,
}

impl QaManualApprovalClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            now: Mutex::new(Utc::now()),
        })
    }

    fn advance(&self, duration: Duration) {
        let mut now = self.now.lock().unwrap_or_else(|error| error.into_inner());
        *now += ChronoDuration::from_std(duration).expect("QA clock duration fits chrono");
    }
}

impl ApprovalClock for QaManualApprovalClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap_or_else(|error| error.into_inner())
    }
}

impl QaWorkspace {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "chatgpt-harness-m7-runtime-{}",
            uuid::Uuid::new_v4()
        ));
        let root = base.join("workspace");
        fs::create_dir_all(&root).expect("create M7 Runtime QA workspace");
        Self { base, root }
    }

    fn write(&self, path: impl AsRef<Path>, content: impl AsRef<[u8]>) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
        fs::write(path, content).expect("write M7 Runtime QA fixture");
    }

    fn config(&self) -> RuntimeConfigSnapshot {
        RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(1),
            workspace: WorkspaceConfig {
                root_path: self.root.to_string_lossy().into_owned(),
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
            bash: harness_control_protocol::BashConfig::default(),
            policies: harness_control_protocol::PolicyConfig::default(),
        }
    }

    fn core(&self, config: RuntimeConfigSnapshot) -> RuntimeCore {
        let mut core = RuntimeCore::new();
        core.initialize(config)
            .expect("initialize M7 Runtime QA core");
        core.acknowledge().expect("acknowledge M7 Runtime QA core");
        core
    }

    fn service(&self) -> (RuntimeCore, McpService) {
        let core = self.core(self.config());
        let service = core.mcp_service().expect("MCP service after Runtime ack");
        (core, service)
    }

    fn assert_no_atomic_residue(&self) -> QaResult {
        let residue = fs::read_dir(&self.root)
            .map_err(|_| "read workspace after atomic operation failed")?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".chatgpt-harness-")
            })
            .collect::<Vec<_>>();
        check!(residue.is_empty(), "atomic temporary residue remained");
        Ok(())
    }
}

impl Drop for QaWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn report_case(id: &str, result: QaResult, failures: &mut Vec<String>) {
    match result {
        Ok(()) => {
            println!("{id} PASS");
            log_evidence(id, "case_result", json!({"status": "PASS"}));
        }
        Err(error) => {
            println!("{id} FAIL: {error}");
            // Keep failure evidence free of assertion text because an
            // assertion may accidentally contain a tool input in the future.
            log_evidence(id, "case_result", json!({"status": "FAIL"}));
            failures.push(id.to_owned());
        }
    }
}

fn error_code(result: &CallToolResult) -> Option<&str> {
    result
        .structured_content
        .as_ref()
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str)
}

fn activity_json(core: &RuntimeCore) -> String {
    serde_json::to_string(&core.activity_snapshot(256)).expect("serialize Activity snapshot")
}

async fn wait_for_pending(core: &RuntimeCore, expected: usize) -> QaResult {
    for _ in 0..400 {
        if core.approval_manager().pending_count() == expected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Err(format!("pending approval count did not reach {expected}"))
}

async fn wait_for_pending_manager(manager: &ApprovalManager, expected: usize) -> QaResult {
    for _ in 0..400 {
        if manager.pending_count() == expected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Err(format!(
        "standalone approval count did not reach {expected}"
    ))
}

async fn wait_for_counts(core: &RuntimeCore, expected: (u32, u32)) -> QaResult {
    for _ in 0..400 {
        if core.request_counts() == expected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Err(format!(
        "request counters did not reach ({}, {})",
        expected.0, expected.1
    ))
}

fn user_rule(id: &str, decision: PolicyDecision, tool: &str) -> UserPolicyRuleConfig {
    UserPolicyRuleConfig {
        id: id.to_owned(),
        name: format!("M7 Runtime QA {id}"),
        enabled: true,
        decision,
        matcher: PolicyMatcherConfig::tool(tool),
        reason: "M7 Runtime Integration QA policy fixture.".into(),
    }
}

fn write_policy(config: &mut RuntimeConfigSnapshot, decision: PolicyDecision, id: &str) {
    config.policies.tool_defaults.write = decision.clone();
    config
        .policies
        .user_rules
        .push(user_rule(id, decision, "write"));
}

async fn run_approval_write(
    case_id: &str,
    core: &RuntimeCore,
    service: &McpService,
    path: &str,
    decision: ApprovalDecision,
) -> QaResult<uuid::Uuid> {
    let call = {
        let service = service.clone();
        let path = path.to_owned();
        tokio::spawn(async move {
            service
                .call_tool_async(
                    "write",
                    json!({"path": path, "content": "M7_APPROVAL_CONTENT_SENTINEL"}),
                )
                .await
                .map_err(|_| "MCP approval write returned protocol error".to_owned())
        })
    };
    wait_for_pending(core, 1).await?;
    let pending = core.approval_manager().list(Some(ApprovalStatus::Pending));
    let request = pending
        .iter()
        .find(|item| item.scope_summary == path)
        .ok_or_else(|| "pending approval scope was not found".to_owned())?
        .clone();
    log_evidence(
        case_id,
        "approval_pending",
        json!({
            "requestId": request.request_id,
            "approvalId": request.request_id,
            "counters": {
                "activeRequests": core.request_counts().0,
                "pendingApprovals": core.approval_manager().pending_count(),
                "runningExecutions": core.request_counts().1,
            },
            "pendingCount": pending.len(),
        }),
    );
    core.approval_manager()
        .resolve(&request.request_id, decision)
        .map_err(|_| "resolve approval failed".to_owned())?;
    let result = call
        .await
        .map_err(|_| "approval write task failed".to_owned())??;
    if decision == ApprovalDecision::Deny {
        check!(error_code(&result) == Some("APPROVAL_DENIED"));
    } else {
        check!(result.is_error == Some(false));
    }
    log_activity(case_id, "approval_terminal", core);
    log_evidence(
        case_id,
        "approval_terminal_ids",
        json!({
            "requestId": request.request_id,
            "approvalId": request.request_id,
            "decision": format!("{:?}", decision),
            "counters": {
                "activeRequests": core.request_counts().0,
                "pendingApprovals": core.approval_manager().pending_count(),
                "runningExecutions": core.request_counts().1,
            },
        }),
    );
    Ok(uuid::Uuid::parse_str(&request.request_id)
        .map_err(|_| "Runtime approval ID was not a UUID".to_owned())?)
}

async fn int_19_and_24_file_evidence() -> QaResult {
    let fixture = QaWorkspace::new();
    let read_sentinel = "M7_READ_CONTENT_SENTINEL";
    let write_sentinel = "M7_WRITE_CONTENT_SENTINEL";
    let old_sentinel = "M7_EDIT_OLD_SENTINEL";
    let new_sentinel = "M7_EDIT_NEW_SENTINEL";
    fixture.write("read.txt", read_sentinel);
    let (core, service) = fixture.service();

    let read = service
        .call_tool("read_file", json!({"path": "read.txt"}))
        .map_err(|_| "read_file MCP adapter failed".to_owned())?;
    check!(read.is_error == Some(false));
    check!(read.structured_content.as_ref().is_some_and(|v| {
        v["content"] == read_sentinel && v["bytesRead"].as_u64() == Some(read_sentinel.len() as u64)
    }));

    let glob = service
        .call_tool("glob", json!({"pattern": "*.txt"}))
        .map_err(|_| "glob MCP adapter failed".to_owned())?;
    check!(glob.is_error == Some(false));

    let write = service
        .call_tool(
            "write",
            json!({"path": "evidence.txt", "content": write_sentinel}),
        )
        .map_err(|_| "write MCP adapter failed".to_owned())?;
    check!(write.is_error == Some(false));
    check!(write.structured_content.as_ref().is_some_and(|v| {
        v["bytesWritten"].as_u64() == Some(write_sentinel.len() as u64)
            && v["created"] == true
            && v["overwritten"] == false
    }));

    let edit = service
        .call_tool(
            "edit",
            json!({"path": "evidence.txt", "oldText": write_sentinel, "newText": new_sentinel}),
        )
        .map_err(|_| "edit MCP adapter failed".to_owned())?;
    check!(edit.is_error == Some(false));
    check!(edit.structured_content.as_ref().is_some_and(|v| {
        v["replacements"].as_u64() == Some(1)
            && v["bytesWritten"].as_u64() == Some(new_sentinel.len() as u64)
    }));
    check!(fs::read_to_string(fixture.root.join("evidence.txt")).unwrap() == new_sentinel);

    let entries = core.activity_snapshot(256);
    for tool in ["read_file", "glob", "write", "edit"] {
        let terminal = entries
            .iter()
            .rev()
            .find(|entry| {
                entry.tool_name.as_deref() == Some(tool)
                    && entry.event_type == AuditEventType::ExecutionCompleted
            })
            .ok_or_else(|| format!("missing terminal Activity evidence for {tool}"))?;
        check!(terminal.tool_enabled == Some(true));
        check!(terminal.capability.is_some());
        check!(terminal.duration_ms.is_some());
        check!(terminal.result_evidence.is_some());
        check!(terminal.sanitized_arguments.is_some());
        match (tool, terminal.sanitized_arguments.as_ref().unwrap()) {
            ("read_file", SanitizedArgumentsEvidence::ReadFile { logical_path })
            | ("write", SanitizedArgumentsEvidence::Write { logical_path })
            | ("edit", SanitizedArgumentsEvidence::Edit { logical_path }) => {
                check!(!logical_path.contains('\\'));
                check!(!logical_path.contains(".."));
                check!(!logical_path.contains(fixture.root.to_string_lossy().as_ref()));
            }
            ("glob", SanitizedArgumentsEvidence::Glob { pattern }) => {
                check!(pattern == "*.txt");
            }
            _ => return Err(format!("wrong sanitized argument variant for {tool}")),
        }
    }
    let read_terminal = entries
        .iter()
        .rev()
        .find(|entry| {
            entry.tool_name.as_deref() == Some("read_file")
                && entry.event_type == AuditEventType::ExecutionCompleted
        })
        .unwrap();
    check!(matches!(
        read_terminal.result_evidence,
        Some(ExecutionResultEvidence::ReadFile { bytes_read }) if bytes_read == read_sentinel.len() as u64
    ));
    let glob_terminal = entries
        .iter()
        .rev()
        .find(|entry| {
            entry.tool_name.as_deref() == Some("glob")
                && entry.event_type == AuditEventType::ExecutionCompleted
        })
        .unwrap();
    check!(
        matches!(
            glob_terminal.result_evidence,
        Some(ExecutionResultEvidence::Glob { match_count }) if match_count >= 1
        ),
        "glob result evidence was {:?}",
        glob_terminal.result_evidence
    );
    let write_terminal = entries
        .iter()
        .rev()
        .find(|entry| {
            entry.tool_name.as_deref() == Some("write")
                && entry.event_type == AuditEventType::ExecutionCompleted
        })
        .unwrap();
    check!(matches!(
        write_terminal.result_evidence,
        Some(ExecutionResultEvidence::Write {
            bytes_written,
            disposition: WriteDisposition::Created
        }) if bytes_written == write_sentinel.len() as u64
    ));
    let edit_terminal = entries
        .iter()
        .rev()
        .find(|entry| {
            entry.tool_name.as_deref() == Some("edit")
                && entry.event_type == AuditEventType::ExecutionCompleted
        })
        .unwrap();
    check!(matches!(
        edit_terminal.result_evidence,
        Some(ExecutionResultEvidence::Edit {
            replacements,
            bytes_written
        }) if replacements == 1 && bytes_written == new_sentinel.len() as u64
    ));

    let activity = activity_json(&core);
    for forbidden in [
        read_sentinel,
        write_sentinel,
        old_sentinel,
        new_sentinel,
        "oldText",
        "newText",
        fixture.root.to_string_lossy().as_ref(),
    ] {
        check!(
            !activity.contains(forbidden),
            "forbidden Activity evidence: {forbidden}"
        );
    }
    log_activity("INT-19", "file-evidence", &core);
    log_evidence(
        "INT-24",
        "scalar-results",
        json!({
            "sourceCase": "INT-19",
            "fields": ["bytesRead", "matchCount", "bytesWritten", "created", "overwritten", "replacements"],
            "rawResults": false,
            "sanitizedActivity": true,
        }),
    );
    Ok(())
}

fn int_20_tool_capability_and_disabled_evidence() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    config.tools.write.enabled = false;
    let core = fixture.core(config);
    let service = core
        .mcp_service()
        .map_err(|_| "MCP service unavailable".to_owned())?;
    let write_effective = core
        .effective_tools()
        .into_iter()
        .find(|tool| tool.name == "write")
        .ok_or_else(|| "write effective tool missing".to_owned())?;
    check!(write_effective.availability == ToolAvailability::UnavailableDisabled);
    check!(write_effective.status_reason == ToolStatusReason::ToolDisabled);
    check!(
        write_effective.declared_capability == Some(crate::tools::ToolCapability::WorkspaceWrite)
    );
    check!(!service.tools().iter().any(|tool| tool.name == "write"));
    let result = service
        .call_tool(
            "write",
            json!({"path": "disabled.txt", "content": "blocked"}),
        )
        .map_err(|_| "disabled stale MCP call failed at protocol adapter".to_owned())?;
    check!(error_code(&result) == Some("TOOL_DISABLED"));
    check!(!fixture.root.join("disabled.txt").exists());
    let entries = core.activity_snapshot(256);
    let terminal = entries
        .iter()
        .rev()
        .find(|entry| entry.tool_name.as_deref() == Some("write") && entry.duration_ms.is_some())
        .ok_or_else(|| "disabled call has no terminal Activity evidence".to_owned())?;
    check!(terminal.tool_enabled == Some(false));
    check!(terminal.capability == Some(ToolCapability::WorkspaceWrite));
    check!(terminal.failure_category == Some(AuditFailureCategory::ToolDisabled));
    check!(terminal.result_evidence.is_none());
    check!(!entries
        .iter()
        .any(|entry| entry.request_id == terminal.request_id
            && entry.event_type == AuditEventType::ExecutionStarted));
    log_activity("INT-20", "disabled-capability-evidence", &core);
    Ok(())
}

async fn int_21_policy_evidence_is_canonical() -> QaResult {
    for (label, decision, reason_id) in [
        ("allow", PolicyDecision::Allow, "int-21-allow"),
        ("ask", PolicyDecision::Ask, "int-21-ask"),
        ("deny", PolicyDecision::Deny, "int-21-deny"),
    ] {
        let fixture = QaWorkspace::new();
        let mut config = fixture.config();
        write_policy(&mut config, decision.clone(), reason_id);
        let core = fixture.core(config);
        let service = core
            .mcp_service()
            .map_err(|_| "MCP service unavailable".to_owned())?;
        let request_id = if decision == PolicyDecision::Ask {
            run_approval_write(
                "INT-21",
                &core,
                &service,
                "int-21-ask.txt",
                ApprovalDecision::Deny,
            )
            .await?
        } else {
            let result = service
                .call_tool(
                    "write",
                    json!({"path": format!("int-21-{label}.txt"), "content": "policy"}),
                )
                .map_err(|_| "policy MCP adapter failed".to_owned())?;
            if decision == PolicyDecision::Allow {
                check!(result.is_error == Some(false));
            } else {
                check!(error_code(&result) == Some("POLICY_DENIED"));
            }
            core.activity_snapshot(256)
                .iter()
                .rev()
                .find(|entry| entry.tool_name.as_deref() == Some("write"))
                .map(|entry| entry.request_id)
                .ok_or_else(|| "policy request Activity missing".to_owned())?
        };
        let entries = core.activity_snapshot(256);
        let policy = entries
            .iter()
            .find(|entry| {
                entry.request_id == request_id
                    && entry.event_type == AuditEventType::PolicyEvaluated
            })
            .ok_or_else(|| "canonical PolicyEvaluated evidence missing".to_owned())?;
        check!(policy.decision_summary == Some(decision.clone()));
        check!(policy.matched_rule_ids == [reason_id.to_owned()]);
        check!(policy.policy_reason == Some(PolicyReason::MatchedUserRules));
        check!(activity_json(&core).contains(reason_id));
        log_activity("INT-21", "canonical-policy-evidence", &core);
    }
    Ok(())
}

async fn int_22_approval_id_and_decision_correlation() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    let core = fixture.core(config);
    let service = core
        .mcp_service()
        .map_err(|_| "MCP service unavailable".to_owned())?;
    for (path, decision) in [
        ("int-22-once.txt", ApprovalDecision::AllowOnce),
        ("int-22-session.txt", ApprovalDecision::AllowForSession),
        ("int-22-deny.txt", ApprovalDecision::Deny),
    ] {
        let request_id = run_approval_write("INT-22", &core, &service, path, decision).await?;
        let entries = core.activity_snapshot(256);
        let request_entries = entries
            .iter()
            .filter(|entry| entry.request_id == request_id)
            .collect::<Vec<_>>();
        check!(!request_entries.is_empty());
        for entry in request_entries.iter().filter(|entry| {
            matches!(
                entry.event_type,
                AuditEventType::ApprovalRequired
                    | AuditEventType::ApprovalResolved
                    | AuditEventType::ExecutionCompleted
                    | AuditEventType::RequestDenied
            )
        }) {
            check!(entry.approval_id == Some(request_id.to_string()));
        }
        if decision == ApprovalDecision::Deny {
            let terminal = request_entries
                .iter()
                .find(|entry| entry.event_type == AuditEventType::RequestDenied)
                .ok_or_else(|| "approval deny terminal evidence missing".to_owned())?;
            check!(terminal.approval_decision == Some(ApprovalDecision::Deny));
            check!(terminal.outcome_summary == AuditOutcome::Denied);
            check!(terminal.duration_ms.is_some());
        } else {
            let terminal = request_entries
                .iter()
                .find(|entry| entry.event_type == AuditEventType::ExecutionCompleted)
                .ok_or_else(|| "approval allow terminal evidence missing".to_owned())?;
            check!(terminal.approval_decision == Some(decision));
            check!(terminal.result_evidence.is_some());
            check!(terminal.duration_ms.is_some());
        }
        log_activity("INT-22", "approval-correlation", &core);
    }
    Ok(())
}

async fn int_23_terminal_durations() -> QaResult {
    let fixture = QaWorkspace::new();
    fixture.write("duration.txt", "duration");
    let (success_core, success_service) = fixture.service();
    let success = success_service
        .call_tool("read_file", json!({"path": "duration.txt"}))
        .map_err(|_| "success call failed".to_owned())?;
    check!(success.is_error == Some(false));
    let success_entries = success_core.activity_snapshot(256);
    let success_terminal = success_entries
        .iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::ExecutionCompleted)
        .ok_or_else(|| "success terminal evidence missing".to_owned())?;
    check!(success_terminal.duration_ms.is_some());
    log_activity("INT-23", "success-duration", &success_core);

    let deny_fixture = QaWorkspace::new();
    let mut deny_config = deny_fixture.config();
    write_policy(&mut deny_config, PolicyDecision::Deny, "int-23-deny");
    let deny_core = deny_fixture.core(deny_config);
    let deny_service = deny_core
        .mcp_service()
        .map_err(|_| "deny MCP unavailable".to_owned())?;
    let deny = deny_service
        .call_tool("write", json!({"path": "deny.txt", "content": "blocked"}))
        .map_err(|_| "deny call failed".to_owned())?;
    check!(error_code(&deny) == Some("POLICY_DENIED"));
    let deny_entries = deny_core.activity_snapshot(256);
    let deny_terminal = deny_entries
        .iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::RequestDenied)
        .ok_or_else(|| "deny terminal evidence missing".to_owned())?;
    check!(deny_terminal.duration_ms.is_some());
    log_activity("INT-23", "deny-duration", &deny_core);

    let ask_fixture = QaWorkspace::new();
    let mut ask_config = ask_fixture.config();
    ask_config.policies.tool_defaults.write = PolicyDecision::Ask;
    let ask_core = ask_fixture.core(ask_config);
    let ask_service = ask_core
        .mcp_service()
        .map_err(|_| "ask MCP unavailable".to_owned())?;
    let ask_id = run_approval_write(
        "INT-23",
        &ask_core,
        &ask_service,
        "approval-denied.txt",
        ApprovalDecision::Deny,
    )
    .await?;
    let ask_entries = ask_core.activity_snapshot(256);
    let ask_terminal = ask_entries
        .iter()
        .find(|entry| {
            entry.request_id == ask_id && entry.event_type == AuditEventType::RequestDenied
        })
        .ok_or_else(|| "approval-denied terminal evidence missing".to_owned())?;
    check!(ask_terminal.duration_ms.is_some());
    log_activity("INT-23", "approval-denied-duration", &ask_core);

    let failed_fixture = QaWorkspace::new();
    fs::create_dir_all(failed_fixture.root.join("directory-target"))
        .map_err(|_| "create execution-failure directory fixture failed".to_owned())?;
    let (failed_core, failed_service) = failed_fixture.service();
    let failed = failed_service
        .call_tool("read_file", json!({"path": "directory-target"}))
        .map_err(|_| "execution failure call failed".to_owned())?;
    check!(error_code(&failed) == Some("TARGET_NOT_FILE"));
    let failed_entries = failed_core.activity_snapshot(256);
    let failed_terminal = failed_entries
        .iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::ExecutionFailed)
        .ok_or_else(|| {
            format!(
                "execution-failed terminal evidence missing; events={:?}",
                failed_entries
                    .iter()
                    .map(|entry| (entry.event_type, entry.failure_category, entry.duration_ms))
                    .collect::<Vec<_>>()
            )
        })?;
    check!(failed_terminal.duration_ms.is_some());
    log_activity("INT-23", "execution-failed-duration", &failed_core);
    Ok(())
}

#[cfg(windows)]
fn bash_config(fixture: &QaWorkspace, decision: PolicyDecision) -> QaResult<RuntimeConfigSnapshot> {
    let program_files = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .ok_or_else(|| "ProgramFiles is unavailable for Git Bash QA".to_owned())?;
    let executable = program_files.join("Git").join("bin").join("bash.exe");
    check!(executable.is_file(), "Git Bash fixture is unavailable");
    let mut config = fixture.config();
    config.security = SecurityConfig {
        mode: SecurityMode::DeveloperTest,
        developer_test_mode: true,
    };
    config.tools.bash.enabled = true;
    config.bash = harness_control_protocol::BashConfig {
        auto_detect: false,
        executable_path: Some(executable.to_string_lossy().into_owned()),
    };
    config.policies.tool_defaults.bash = decision;
    Ok(config)
}

#[cfg(windows)]
async fn int_25_bash_hardened_evidence() -> QaResult {
    let fixture = QaWorkspace::new();
    let core = fixture.core(bash_config(&fixture, PolicyDecision::Allow)?);
    let service = core
        .mcp_service()
        .map_err(|_| "Bash MCP service unavailable".to_owned())?;

    // This uses the production RuntimeCore-owned BashRuntime and its frozen
    // output cap. The command is deliberately classified so no human approval
    // mock or test-only limit is involved.
    let limited = service
        .call_tool_async("bash", json!({"command": "head -c 300000 /dev/zero"}))
        .await
        .map_err(|_| "Bash output-limit call failed at adapter".to_owned())?;
    check!(error_code(&limited) == Some("BASH_OUTPUT_LIMIT"));
    check!(core.request_counts() == (0, 0));
    check!(core
        .bash_runtime()
        .is_some_and(|runtime| runtime.active_count() == 0));
    let limited_entry = core
        .activity_snapshot(256)
        .into_iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::ExecutionFailed)
        .ok_or_else(|| "Bash output-limit Activity evidence missing".to_owned())?;
    check!(limited_entry.failure_category == Some(AuditFailureCategory::BashOutputLimit));
    check!(limited_entry.duration_ms.is_some());
    check!(limited_entry.bash.as_ref().is_some_and(|bash| {
        bash.command_preview
            .as_ref()
            .is_some_and(|preview| preview.len() <= 1027)
            && bash.classification.is_some()
    }));

    // A second real process covers the cancellation/error path and process
    // tree cleanup. It must never be observed as an approval wait.
    let cancel = CancellationToken::new();
    let call = {
        let service = service.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            service
                .call_tool_async_with_cancel("bash", json!({"command": "sleep 60"}), cancel)
                .await
                .map_err(|_| "Bash cancellation call failed at adapter".to_owned())
        })
    };
    wait_for_counts(&core, (1, 1)).await?;
    check!(core
        .status()
        .is_some_and(|status| status.pending_approvals == 0));
    cancel.cancel();
    let cancelled = call
        .await
        .map_err(|_| "Bash cancellation task failed".to_owned())??;
    check!(error_code(&cancelled) == Some("BASH_CANCELLED"));
    wait_for_counts(&core, (0, 0)).await?;
    check!(core
        .bash_runtime()
        .is_some_and(|runtime| runtime.active_count() == 0));
    let activity = activity_json(&core);
    check!(!activity.contains("stdout"));
    check!(!activity.contains("stderr"));
    let cancelled_entry = core
        .activity_snapshot(256)
        .into_iter()
        .rev()
        .find(|entry| entry.failure_category == Some(AuditFailureCategory::BashCancelled))
        .ok_or_else(|| "Bash cancellation Activity evidence missing".to_owned())?;
    check!(cancelled_entry.duration_ms.is_some());
    log_activity("INT-25", "bash-limit-and-cancel-evidence", &core);
    Ok(())
}

#[cfg(not(windows))]
async fn int_25_bash_hardened_evidence() -> QaResult {
    Err("BLOCKED: M7 Runtime Bash QA requires the Windows Git Bash fixture".into())
}

async fn int_26_bounds_on_long_inputs_and_errors() -> QaResult {
    let fixture = QaWorkspace::new();
    let (core, service) = fixture.service();
    let long_path = format!("{}.txt", "x".repeat(5_000));
    let long_path_result = service
        .call_tool("read_file", json!({"path": long_path}))
        .map_err(|_| "long path request failed at adapter".to_owned())?;
    check!(long_path_result.is_error == Some(true));
    let long_path_error = serde_json::to_string(&long_path_result).unwrap();
    check!(long_path_error.len() < 4_096);
    check!(!long_path_error.contains(&"x".repeat(5_000)));

    let long_pattern = format!("[{}", "p".repeat(5_000));
    let long_pattern_result = service
        .call_tool("glob", json!({"pattern": long_pattern}))
        .map_err(|_| "long pattern request failed at adapter".to_owned())?;
    check!(long_pattern_result.is_error == Some(true));
    let long_pattern_error = serde_json::to_string(&long_pattern_result).unwrap();
    check!(long_pattern_error.len() < 4_096);
    check!(!long_pattern_error.contains(&"p".repeat(5_000)));

    let long_error = activity_json(&core);
    check!(long_error.len() < 100_000);
    for entry in core.activity_snapshot(256) {
        if let Some(arguments) = entry.sanitized_arguments {
            let text = serde_json::to_string(&arguments).unwrap();
            check!(text.len() <= 1_200, "sanitized arguments exceeded bound");
        }
    }
    check!(!long_error.contains(fixture.root.to_string_lossy().as_ref()));
    check!(!long_error.contains("p".repeat(5_000).as_str()));
    log_evidence(
        "INT-26",
        "bounded-error-payloads",
        json!({
            "readErrorCode": error_code(&long_path_result),
            "readErrorBytes": long_path_error.len(),
            "globErrorCode": error_code(&long_pattern_result),
            "globErrorBytes": long_pattern_error.len(),
            "activityBytes": long_error.len(),
            "sanitizedArgumentsBound": 1200,
        }),
    );

    #[cfg(windows)]
    {
        let bash_fixture = QaWorkspace::new();
        let bash_core = bash_fixture.core(bash_config(&bash_fixture, PolicyDecision::Allow)?);
        let bash_service = bash_core
            .mcp_service()
            .map_err(|_| "long Bash MCP service unavailable".to_owned())?;
        let long_command = "x".repeat(crate::bash::MAX_COMMAND_BYTES + 1);
        let long_command_result = bash_service
            .call_tool_async("bash", json!({"command": long_command}))
            .await
            .map_err(|_| "long command request failed at adapter".to_owned())?;
        check!(error_code(&long_command_result) == Some("BASH_COMMAND_TOO_LONG"));
        check!(bash_core.request_counts() == (0, 0));
        let bash_activity = activity_json(&bash_core);
        check!(bash_activity.len() < 100_000);
        check!(!bash_activity.contains(&"x".repeat(crate::bash::MAX_COMMAND_BYTES + 1)));
        let bash_error = serde_json::to_string(&long_command_result).unwrap();
        check!(bash_error.len() < 4_096);
        log_evidence(
            "INT-26",
            "bounded-command-error",
            json!({
                "errorCode": error_code(&long_command_result),
                "errorBytes": bash_error.len(),
                "activityBytes": bash_activity.len(),
            }),
        );
        log_activity("INT-26", "long-bash-error-bounded", &bash_core);
    }
    log_activity("INT-26", "long-input-bounded", &core);
    Ok(())
}

fn int_27_untrusted_rule_identifiers_are_sanitized() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    config.policies.user_rules.push(user_rule(
        "int-27-valid_rule",
        PolicyDecision::Deny,
        "write",
    ));
    config.policies.user_rules.push(UserPolicyRuleConfig {
        id: "PRIVATE RULE ID / SECRET".into(),
        name: "M7 Runtime QA unsafe identifier".into(),
        enabled: true,
        decision: PolicyDecision::Deny,
        matcher: PolicyMatcherConfig::tool("write"),
        reason: "M7 Runtime QA unsafe identifier fixture.".into(),
    });
    let core = fixture.core(config);
    let service = core
        .mcp_service()
        .map_err(|_| "MCP service unavailable".to_owned())?;
    let result = service
        .call_tool(
            "write",
            json!({"path": "untrusted.txt", "content": "blocked"}),
        )
        .map_err(|_| "untrusted identifier call failed".to_owned())?;
    check!(error_code(&result) == Some("POLICY_DENIED"));
    let entries = core.activity_snapshot(256);
    let policy = entries
        .iter()
        .find(|entry| entry.event_type == AuditEventType::PolicyEvaluated)
        .ok_or_else(|| "untrusted identifier policy evidence missing".to_owned())?;
    check!(policy
        .matched_rule_ids
        .contains(&"int-27-valid_rule".to_owned()));
    check!(policy.matched_rule_ids.contains(&"<redacted>".to_owned()));
    let text = activity_json(&core);
    check!(!text.contains("PRIVATE RULE ID / SECRET"));
    log_activity("INT-27", "identifier-sanitization", &core);
    Ok(())
}

async fn int_28_active_request_counter_lifecycle() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    let core = fixture.core(config);
    let service = core
        .mcp_service()
        .map_err(|_| "MCP service unavailable".to_owned())?;
    check!(core.request_counts() == (0, 0));
    log_evidence(
        "INT-28",
        "counters-before",
        json!({"activeRequests": 0, "pendingApprovals": 0, "runningExecutions": 0}),
    );
    let call = {
        let service = service.clone();
        tokio::spawn(async move {
            service
                .call_tool_async("write", json!({"path": "active.txt", "content": "active"}))
                .await
                .map_err(|_| "active request adapter failed".to_owned())
        })
    };
    wait_for_pending(&core, 1).await?;
    check!(core.request_counts() == (1, 0));
    let status = core
        .status()
        .ok_or_else(|| "Runtime status missing".to_owned())?;
    check!(
        (
            status.active_requests,
            status.pending_approvals,
            status.running_executions
        ) == (1, 1, 0)
    );
    log_evidence(
        "INT-28",
        "counters-during",
        json!({
            "requestId": core.approval_manager().list(Some(ApprovalStatus::Pending))[0].request_id,
            "activeRequests": status.active_requests,
            "pendingApprovals": status.pending_approvals,
            "runningExecutions": status.running_executions,
        }),
    );
    let pending = core.approval_manager().list(Some(ApprovalStatus::Pending));
    core.approval_manager()
        .resolve(&pending[0].request_id, ApprovalDecision::Deny)
        .map_err(|_| "deny active request failed".to_owned())?;
    let result = call
        .await
        .map_err(|_| "active request task failed".to_owned())??;
    check!(error_code(&result) == Some("APPROVAL_DENIED"));
    wait_for_counts(&core, (0, 0)).await?;

    let invalid = service
        .call_tool("write", json!({"path": 1}))
        .map_err(|_| "validation failure adapter failed".to_owned())?;
    check!(error_code(&invalid) == Some("INVALID_ARGUMENTS"));
    check!(core.request_counts() == (0, 0));
    log_activity("INT-28", "active-counter-lifecycle", &core);

    let deny_fixture = QaWorkspace::new();
    let mut deny_config = deny_fixture.config();
    deny_config.policies.tool_defaults.write = PolicyDecision::Deny;
    let deny_core = deny_fixture.core(deny_config);
    let deny_service = deny_core
        .mcp_service()
        .map_err(|_| "active DENY MCP service unavailable".to_owned())?;
    let denied = deny_service
        .call_tool("write", json!({"path": "denied.txt", "content": "blocked"}))
        .map_err(|_| "active DENY adapter failed".to_owned())?;
    check!(error_code(&denied) == Some("POLICY_DENIED"));
    check!(deny_core.request_counts() == (0, 0));
    log_activity("INT-28", "deny-terminal-cleanup", &deny_core);

    let failed_fixture = QaWorkspace::new();
    fs::create_dir_all(failed_fixture.root.join("directory-target"))
        .map_err(|_| "create active executor-failure fixture failed".to_owned())?;
    let (failed_core, failed_service) = failed_fixture.service();
    let failed = failed_service
        .call_tool("read_file", json!({"path": "directory-target"}))
        .map_err(|_| "active executor-failure adapter failed".to_owned())?;
    check!(error_code(&failed) == Some("TARGET_NOT_FILE"));
    check!(failed_core.request_counts() == (0, 0));
    log_activity("INT-28", "executor-failure-cleanup", &failed_core);
    Ok(())
}

async fn int_29_pending_approval_counter_is_real() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    let core = fixture.core(config);
    let service = core
        .mcp_service()
        .map_err(|_| "MCP service unavailable".to_owned())?;
    let first = {
        let service = service.clone();
        tokio::spawn(async move {
            service
                .call_tool_async("write", json!({"path": "pending-a.txt", "content": "a"}))
                .await
                .map_err(|_| "first pending call failed".to_owned())
        })
    };
    let second = {
        let service = service.clone();
        tokio::spawn(async move {
            service
                .call_tool_async("write", json!({"path": "pending-b.txt", "content": "b"}))
                .await
                .map_err(|_| "second pending call failed".to_owned())
        })
    };
    wait_for_pending(&core, 2).await?;
    let status = core
        .status()
        .ok_or_else(|| "Runtime status missing".to_owned())?;
    check!(
        (
            status.active_requests,
            status.pending_approvals,
            status.running_executions
        ) == (2, 2, 0)
    );
    log_evidence(
        "INT-29",
        "counters-during",
        json!({
            "activeRequests": status.active_requests,
            "pendingApprovals": status.pending_approvals,
            "runningExecutions": status.running_executions,
            "approvalCount": core.approval_manager().pending_count(),
        }),
    );
    let pending = core.approval_manager().list(Some(ApprovalStatus::Pending));
    check!(pending.len() == 2);
    for item in &pending {
        let decision = if item.scope_summary == "pending-a.txt" {
            ApprovalDecision::AllowOnce
        } else {
            ApprovalDecision::Deny
        };
        core.approval_manager()
            .resolve(&item.request_id, decision)
            .map_err(|_| "resolve concurrent approval failed".to_owned())?;
    }
    let first_result = first
        .await
        .map_err(|_| "first pending task failed".to_owned())??;
    let second_result = second
        .await
        .map_err(|_| "second pending task failed".to_owned())??;
    check!(first_result.is_error == Some(false));
    check!(error_code(&second_result) == Some("APPROVAL_DENIED"));
    wait_for_pending(&core, 0).await?;
    wait_for_counts(&core, (0, 0)).await?;
    log_activity("INT-29", "pending-counter-concurrency", &core);

    let cancel_fixture = QaWorkspace::new();
    let mut cancel_config = cancel_fixture.config();
    cancel_config.policies.tool_defaults.write = PolicyDecision::Ask;
    let mut cancel_core = cancel_fixture.core(cancel_config);
    let cancel_service = cancel_core
        .mcp_service()
        .map_err(|_| "approval-cancel MCP service unavailable".to_owned())?;
    let cancel_call = {
        let service = cancel_service.clone();
        tokio::spawn(async move {
            service
                .call_tool_async(
                    "write",
                    json!({"path": "cancelled.txt", "content": "cancelled"}),
                )
                .await
                .map_err(|_| "approval-cancel adapter failed".to_owned())
        })
    };
    wait_for_pending(&cancel_core, 1).await?;
    let cancel_request_id = cancel_core
        .approval_manager()
        .list(Some(ApprovalStatus::Pending))[0]
        .request_id
        .clone();
    cancel_core.stop();
    let cancelled = cancel_call
        .await
        .map_err(|_| "approval-cancel task failed".to_owned())??;
    check!(error_code(&cancelled) == Some("APPROVAL_CANCELLED"));
    wait_for_counts(&cancel_core, (0, 0)).await?;
    check!(cancel_core.approval_manager().pending_count() == 0);
    log_evidence(
        "INT-29",
        "approval-cancel-terminal",
        json!({
            "requestId": cancel_request_id,
            "approvalId": cancel_request_id,
            "counters": {"activeRequests": 0, "pendingApprovals": 0, "runningExecutions": 0},
            "outcome": "ApprovalCancelled",
        }),
    );
    log_activity("INT-29", "approval-cancel-evidence", &cancel_core);

    // Expiry is exercised through the same production ExecutionPipeline with
    // a deterministic test clock; no wall-clock five-minute wait is used.
    let expire_fixture = QaWorkspace::new();
    let workspace = Workspace::new(&expire_fixture.root)
        .map_err(|_| "approval-expire workspace construction failed".to_owned())?;
    let guard = Arc::new(WorkspaceGuard::new(&workspace));
    let registry = Arc::new(ToolRegistry::normal_mode());
    let mut policy_config = harness_control_protocol::PolicyConfig::default();
    policy_config.tool_defaults.write = PolicyDecision::Ask;
    let policy = PolicyEngine::compile(&policy_config, &registry);
    let ready = Arc::new(AtomicBool::new(true));
    let clock = QaManualApprovalClock::new();
    let manager = ApprovalManager::with_test_clock(
        "m7-expire-runtime",
        "m7-expire-generation",
        Duration::from_secs(5),
        clock.clone(),
    );
    let pipeline = Arc::new(
        ExecutionPipeline::new(
            guard,
            registry,
            policy,
            SecurityContext {
                workspace_id: uuid::Uuid::new_v4(),
                mode: SecurityMode::Normal,
            },
            1_048_576,
            ready,
        )
        .with_approval_state(
            manager.clone(),
            Arc::new(SessionGrantStore::new("m7-expire-runtime")),
        ),
    );
    let expire_pipeline = Arc::clone(&pipeline);
    let expire_call = tokio::spawn(async move {
        expire_pipeline
            .execute_async(|| Ok(WriteRequest::new("expire.txt", "expire")))
            .await
    });
    wait_for_pending_manager(&manager, 1).await?;
    let expire_request_id = manager.list(Some(ApprovalStatus::Pending))[0]
        .request_id
        .clone();
    clock.advance(Duration::from_secs(5));
    manager.expire_due();
    let expire_report = tokio::time::timeout(Duration::from_secs(2), expire_call)
        .await
        .map_err(|_| "approval-expire pipeline did not terminate".to_owned())?
        .map_err(|_| "approval-expire pipeline task failed".to_owned())?;
    check!(matches!(
        &expire_report.result,
        Err(crate::execution::ExecutionError::ApprovalExpired)
    ));
    check!(pipeline.request_counts() == (0, 0));
    check!(manager.pending_count() == 0);
    let expired_event = expire_report
        .audit_events
        .iter()
        .find(|event| event.event_type == AuditEventType::ApprovalExpired)
        .ok_or_else(|| "approval-expire terminal evidence missing".to_owned())?;
    check!(expired_event.approval_id.as_deref() == Some(expire_request_id.as_str()));
    check!(expired_event.duration_ms.is_some());
    log_evidence(
        "INT-29",
        "approval-expire-terminal",
        json!({
            "requestId": expire_request_id,
            "approvalId": expire_request_id,
            "counters": {"activeRequests": 0, "pendingApprovals": 0, "runningExecutions": 0},
            "outcome": "ApprovalExpired",
            "durationPresent": true,
        }),
    );
    Ok(())
}

#[cfg(windows)]
async fn int_30_running_execution_counter_is_real() -> QaResult {
    let fixture = QaWorkspace::new();
    let core = fixture.core(bash_config(&fixture, PolicyDecision::Allow)?);
    let service = core
        .mcp_service()
        .map_err(|_| "Bash MCP service unavailable".to_owned())?;
    let call = {
        let service = service.clone();
        tokio::spawn(async move {
            service
                .call_tool_async("bash", json!({"command": "sleep 1"}))
                .await
                .map_err(|_| "running Bash call failed at adapter".to_owned())
        })
    };
    wait_for_counts(&core, (1, 1)).await?;
    let status = core
        .status()
        .ok_or_else(|| "Runtime status missing".to_owned())?;
    check!(
        (
            status.active_requests,
            status.pending_approvals,
            status.running_executions
        ) == (1, 0, 1)
    );
    log_evidence(
        "INT-30",
        "counters-during",
        json!({
            "activeRequests": status.active_requests,
            "pendingApprovals": status.pending_approvals,
            "runningExecutions": status.running_executions,
        }),
    );
    let result = call
        .await
        .map_err(|_| "running Bash task failed".to_owned())??;
    check!(result.is_error == Some(false));
    wait_for_counts(&core, (0, 0)).await?;
    let entries = core.activity_snapshot(256);
    check!(entries
        .iter()
        .any(|entry| entry.event_type == AuditEventType::ExecutionStarted));
    let terminal = entries
        .iter()
        .rev()
        .find(|entry| entry.event_type == AuditEventType::ExecutionCompleted)
        .ok_or_else(|| "running Bash terminal evidence missing".to_owned())?;
    check!(terminal.duration_ms.is_some());
    check!(terminal
        .bash
        .as_ref()
        .is_some_and(|bash| bash.duration_ms.is_some()));
    log_activity("INT-30", "running-counter-observation", &core);
    Ok(())
}

#[cfg(not(windows))]
async fn int_30_running_execution_counter_is_real() -> QaResult {
    Err("BLOCKED: M7 Runtime runningExecutions QA requires Windows Git Bash".into())
}

#[cfg(windows)]
async fn int_31_counter_race_cleanup() -> QaResult {
    // Approval resolution racing Runtime stop must leave no pending request or
    // active counter, regardless of which operation wins the lock first.
    let approval_fixture = QaWorkspace::new();
    let mut approval_config = approval_fixture.config();
    approval_config.policies.tool_defaults.write = PolicyDecision::Ask;
    let approval_core = Arc::new(Mutex::new(approval_fixture.core(approval_config)));
    let approval_service = approval_core
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .mcp_service()
        .map_err(|_| "approval-race MCP unavailable".to_owned())?;
    let call = {
        let service = approval_service.clone();
        tokio::spawn(async move {
            service
                .call_tool_async("write", json!({"path": "race.txt", "content": "race"}))
                .await
                .map_err(|_| "approval-race adapter failed".to_owned())
        })
    };
    let pending_manager = approval_core
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .approval_manager();
    wait_for_pending_manager(&pending_manager, 1).await?;
    let pending_id = pending_manager.list(Some(ApprovalStatus::Pending))[0]
        .request_id
        .clone();
    log_evidence(
        "INT-31",
        "approval-race-during",
        json!({
            "requestId": pending_id,
            "approvalId": pending_id,
            "activeRequests": 1,
            "pendingApprovals": pending_manager.pending_count(),
            "runningExecutions": 0,
        }),
    );
    let race_barrier = Arc::new(Barrier::new(2));
    let stop_core = Arc::clone(&approval_core);
    let stop_barrier = Arc::clone(&race_barrier);
    let stop_task = tokio::task::spawn_blocking(move || {
        stop_barrier.wait();
        stop_core
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .stop();
    });
    let resolve_manager = Arc::clone(&pending_manager);
    let resolve_barrier = Arc::clone(&race_barrier);
    let resolve_id = pending_id.clone();
    let resolve_task = tokio::task::spawn_blocking(move || {
        resolve_barrier.wait();
        resolve_manager.resolve(&resolve_id, ApprovalDecision::AllowOnce)
    });
    let (_stop_result, resolve_result) = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(stop_task, resolve_task)
    })
    .await
    .map_err(|_| "approval resolve-vs-stop race did not terminate".to_owned())?;
    _stop_result.map_err(|_| "Runtime stop race worker panicked".to_owned())?;
    let resolve_outcome =
        resolve_result.map_err(|_| "approval resolve-vs-stop worker panicked".to_owned())?;
    let _ = resolve_outcome;
    let result = call
        .await
        .map_err(|_| "approval-race task failed".to_owned())??;
    if result.is_error == Some(true) {
        check!(matches!(
            error_code(&result),
            Some("APPROVAL_CANCELLED") | Some("RUNTIME_NOT_READY")
        ));
    } else {
        check!(result.is_error == Some(false));
    }
    {
        let core = approval_core
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        check!(core.request_counts() == (0, 0));
        check!(core.approval_manager().pending_count() == 0);
        log_activity("INT-31", "approval-resolve-vs-stop-race", &core);
    }

    // Cancellation of a real running Bash process covers the other side of
    // the race. The ProcessTree owner must be empty before this case exits.
    let bash_fixture = QaWorkspace::new();
    let bash_core = bash_fixture.core(bash_config(&bash_fixture, PolicyDecision::Allow)?);
    let bash_service = bash_core
        .mcp_service()
        .map_err(|_| "Bash-race MCP unavailable".to_owned())?;
    let cancel = CancellationToken::new();
    let bash_call = {
        let service = bash_service.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            service
                .call_tool_async_with_cancel("bash", json!({"command": "sleep 60"}), cancel)
                .await
                .map_err(|_| "Bash-race adapter failed".to_owned())
        })
    };
    wait_for_counts(&bash_core, (1, 1)).await?;
    log_evidence(
        "INT-31",
        "bash-race-during",
        json!({
            "activeRequests": bash_core.request_counts().0,
            "pendingApprovals": bash_core.approval_manager().pending_count(),
            "runningExecutions": bash_core.request_counts().1,
        }),
    );
    cancel.cancel();
    let bash_result = bash_call
        .await
        .map_err(|_| "Bash-race task failed".to_owned())??;
    check!(error_code(&bash_result) == Some("BASH_CANCELLED"));
    wait_for_counts(&bash_core, (0, 0)).await?;
    check!(bash_core
        .bash_runtime()
        .is_some_and(|runtime| runtime.active_count() == 0));

    let failed_fixture = QaWorkspace::new();
    fs::create_dir_all(failed_fixture.root.join("directory-target"))
        .map_err(|_| "create race executor-failure fixture failed".to_owned())?;
    let (failed_core, failed_service) = failed_fixture.service();
    let _ = failed_service
        .call_tool("read_file", json!({"path": "directory-target"}))
        .map_err(|_| "executor-failure adapter failed".to_owned())?;
    check!(failed_core.request_counts() == (0, 0));
    log_activity("INT-31", "bash-cancel-race", &bash_core);
    log_activity("INT-31", "executor-failure-cleanup", &failed_core);
    Ok(())
}

#[cfg(not(windows))]
async fn int_31_counter_race_cleanup() -> QaResult {
    Err("BLOCKED: M7 Runtime counter-race QA requires Windows Git Bash".into())
}

fn int_32_typed_status_reasons_cover_runtime_classes() -> QaResult {
    fn reason(core: &RuntimeCore, name: &str) -> QaResult<ToolStatusReason> {
        core.effective_tools()
            .into_iter()
            .find(|tool| tool.name == name)
            .map(|tool| tool.status_reason)
            .ok_or_else(|| format!("effective tool {name} missing"))
    }

    let available_fixture = QaWorkspace::new();
    let available = available_fixture.core(available_fixture.config());
    check!(reason(&available, "read_file")? == ToolStatusReason::Available);

    let ask_fixture = QaWorkspace::new();
    let mut ask_config = ask_fixture.config();
    ask_config.policies.tool_defaults.write = PolicyDecision::Ask;
    let ask = ask_fixture.core(ask_config);
    check!(reason(&ask, "write")? == ToolStatusReason::ApprovalRequired);

    let disabled_fixture = QaWorkspace::new();
    let mut disabled_config = disabled_fixture.config();
    disabled_config.tools.write.enabled = false;
    let disabled = disabled_fixture.core(disabled_config);
    check!(reason(&disabled, "write")? == ToolStatusReason::ToolDisabled);

    let denied_fixture = QaWorkspace::new();
    let mut denied_config = denied_fixture.config();
    write_policy(&mut denied_config, PolicyDecision::Deny, "int-32-denied");
    let denied = denied_fixture.core(denied_config);
    check!(reason(&denied, "write")? == ToolStatusReason::PolicyDenied);

    let not_ready_fixture = QaWorkspace::new();
    let mut not_ready = RuntimeCore::new();
    not_ready
        .initialize(not_ready_fixture.config())
        .map_err(|_| "not-ready Runtime initialization failed".to_owned())?;
    check!(reason(&not_ready, "read_file")? == ToolStatusReason::RuntimeNotReady);

    let mode_fixture = QaWorkspace::new();
    let mode = mode_fixture.core(mode_fixture.config());
    check!(reason(&mode, "bash")? == ToolStatusReason::DeveloperModeRequired);

    let bash_unavailable_fixture = QaWorkspace::new();
    let mut bash_unavailable_config = bash_unavailable_fixture.config();
    bash_unavailable_config.security = SecurityConfig {
        mode: SecurityMode::DeveloperTest,
        developer_test_mode: true,
    };
    bash_unavailable_config.tools.bash.enabled = true;
    bash_unavailable_config.bash.auto_detect = false;
    bash_unavailable_config.bash.executable_path = Some(
        bash_unavailable_fixture
            .base
            .join("missing-bash.exe")
            .to_string_lossy()
            .into_owned(),
    );
    let bash_unavailable = bash_unavailable_fixture.core(bash_unavailable_config);
    let bash_unavailable_reason = reason(&bash_unavailable, "bash")?;
    check!(
        bash_unavailable_reason == ToolStatusReason::BashExecutableUnavailable,
        "Bash unavailable reason was {:?}",
        bash_unavailable_reason
    );

    let invalid_fixture = QaWorkspace::new();
    let mut invalid_config = invalid_fixture.config();
    invalid_config.policies.user_rules.push(user_rule(
        "int-32-invalid",
        PolicyDecision::Allow,
        "unknown_tool",
    ));
    let invalid = invalid_fixture.core(invalid_config);
    check!(reason(&invalid, "write")? == ToolStatusReason::InvalidPolicySnapshot);

    // RuntimeNotReady and all other reason values are machine enums. Prose is
    // deliberately excluded from this assertion so it cannot become a hidden
    // UI authorization channel.
    log_evidence(
        "INT-32",
        "typed-reasons",
        json!({
            "reasons": [
                "available",
                "approval_required",
                "tool_disabled",
                "policy_denied",
                "runtime_not_ready",
                "developer_mode_required",
                "bash_executable_unavailable",
                "invalid_policy_snapshot",
            ],
            "proseUsedForDecision": false,
        }),
    );
    Ok(())
}

fn int_33_reason_identity_is_deterministic() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    write_policy(&mut config, PolicyDecision::Deny, "int-33-deny");
    let core = fixture.core(config);
    let first = core.effective_tools();
    let second = core.effective_tools();
    check!(first == second);
    let first_reasons = first
        .into_iter()
        .map(|tool| (tool.name, tool.status_reason))
        .collect::<Vec<_>>();
    let second_reasons = second
        .into_iter()
        .map(|tool| (tool.name, tool.status_reason))
        .collect::<Vec<_>>();
    check!(first_reasons == second_reasons);
    log_evidence(
        "INT-33",
        "deterministic-reason-identity",
        json!({"reasons": first_reasons.into_iter().map(|(name, reason)| json!({"tool": name, "reason": format!("{:?}", reason)})).collect::<Vec<_>>()}),
    );
    Ok(())
}

fn int_34_invalid_policy_fails_closed() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    config.policies.user_rules.push(user_rule(
        "int-34-unknown-tool",
        PolicyDecision::Allow,
        "not_a_runtime_tool",
    ));
    let core = fixture.core(config);
    let write = core
        .effective_tools()
        .into_iter()
        .find(|tool| tool.name == "write")
        .ok_or_else(|| "invalid-policy write tool missing".to_owned())?;
    check!(write.decision == PolicyDecision::Deny);
    check!(write.status_reason == ToolStatusReason::InvalidPolicySnapshot);
    check!(write.availability == ToolAvailability::UnavailableByPolicy);
    let service = core
        .mcp_service()
        .map_err(|_| "invalid-policy MCP unavailable".to_owned())?;
    let result = service
        .call_tool(
            "write",
            json!({"path": "invalid.txt", "content": "blocked"}),
        )
        .map_err(|_| "invalid-policy call failed at adapter".to_owned())?;
    check!(error_code(&result) == Some("POLICY_DENIED"));
    check!(!fixture.root.join("invalid.txt").exists());
    let entries = core.activity_snapshot(256);
    let policy = entries
        .iter()
        .find(|entry| entry.event_type == AuditEventType::PolicyEvaluated)
        .ok_or_else(|| "invalid-policy Activity evidence missing".to_owned())?;
    check!(policy.decision_summary == Some(PolicyDecision::Deny));
    check!(policy.policy_reason == Some(PolicyReason::InvalidPolicySnapshot));
    check!(!activity_json(&core).contains("Allowed"));
    log_activity("INT-34", "invalid-policy-fail-closed", &core);
    Ok(())
}

async fn int_39_diagnostics_agree_with_runtime_state() -> QaResult {
    let fixture = QaWorkspace::new();
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    let core = fixture.core(config);
    let service = core
        .mcp_service()
        .map_err(|_| "diagnostics MCP unavailable".to_owned())?;

    let call = {
        let service = service.clone();
        tokio::spawn(async move {
            service
                .call_tool_async(
                    "write",
                    json!({"path": "diagnostic.txt", "content": "diag"}),
                )
                .await
                .map_err(|_| "diagnostics pending call failed".to_owned())
        })
    };
    wait_for_pending(&core, 1).await?;
    let status = core
        .status()
        .ok_or_else(|| "Runtime status missing".to_owned())?;
    let diagnostics = control_read::read(&core, DIAGNOSTICS_GET, Some(json!({})))
        .map_err(|_| "diagnostics control read failed".to_owned())?;
    check!(diagnostics["requests"]["activeRequests"] == status.active_requests);
    check!(diagnostics["requests"]["pendingApprovals"] == status.pending_approvals);
    check!(diagnostics["requests"]["runningExecutions"] == status.running_executions);
    check!(diagnostics["requests"]["observation"] == "instrumented");
    let effective = control_read::read(&core, EFFECTIVE_TOOLS_GET, Some(json!({})))
        .map_err(|_| "effective-tools control read failed".to_owned())?;
    let write = effective
        .as_array()
        .and_then(|items| items.iter().find(|item| item["name"] == "write"))
        .ok_or_else(|| "diagnostics effective write missing".to_owned())?;
    check!(write["statusReason"] == "approval_required");
    let runtime_tools = core.effective_tools();
    for tool in &runtime_tools {
        let dto = effective
            .as_array()
            .and_then(|items| items.iter().find(|item| item["name"] == tool.name))
            .ok_or_else(|| format!("diagnostics omitted effective tool {}", tool.name))?;
        let expected_availability = match tool.availability {
            ToolAvailability::Available => "available",
            ToolAvailability::AvailableWithApproval => "available_with_approval",
            ToolAvailability::UnavailableByMode => "unavailable_by_mode",
            ToolAvailability::UnavailableByPolicy => "unavailable_by_policy",
            ToolAvailability::UnavailableByRuntime => "unavailable_by_runtime",
            ToolAvailability::UnavailableDisabled => "disabled",
        };
        check!(dto["availability"] == expected_availability);
        check!(dto["decision"] == serde_json::to_value(&tool.decision).unwrap());
        check!(dto["statusReason"] == serde_json::to_value(tool.status_reason).unwrap());
        check!(
            dto["declaredCapability"] == serde_json::to_value(tool.declared_capability).unwrap()
        );
        if dto.get("reason").is_some() {
            check!(dto["reason"] == tool.reason);
        }
    }
    let policy = core
        .effective_policy()
        .per_tool
        .into_iter()
        .find(|tool| tool.tool == "write")
        .ok_or_else(|| "canonical effective write policy missing".to_owned())?;
    check!(policy.decision == PolicyDecision::Ask);
    check!(policy.reason == PolicyReason::ToolDefault);
    check!(policy.matched_rule_ids.is_empty());
    check!(
        policy.availability
            == harness_control_protocol::PolicyToolAvailability::UnavailableByPolicy
    );
    log_activity("INT-39", "diagnostics-during-pending", &core);

    let pending = core.approval_manager().list(Some(ApprovalStatus::Pending));
    core.approval_manager()
        .resolve(&pending[0].request_id, ApprovalDecision::Deny)
        .map_err(|_| "diagnostics approval resolution failed".to_owned())?;
    let result = call
        .await
        .map_err(|_| "diagnostics pending task failed".to_owned())??;
    check!(error_code(&result) == Some("APPROVAL_DENIED"));
    wait_for_counts(&core, (0, 0)).await?;
    let status_after = core
        .status()
        .ok_or_else(|| "Runtime status missing after terminal".to_owned())?;
    let diagnostics_after = control_read::read(&core, DIAGNOSTICS_GET, Some(json!({})))
        .map_err(|_| "diagnostics control read after terminal failed".to_owned())?;
    check!(diagnostics_after["requests"]["activeRequests"] == status_after.active_requests);
    check!(diagnostics_after["requests"]["pendingApprovals"] == status_after.pending_approvals);
    check!(diagnostics_after["requests"]["runningExecutions"] == status_after.running_executions);
    check!(status_after.active_requests == 0);
    check!(status_after.pending_approvals == 0);
    check!(status_after.running_executions == 0);
    log_activity("INT-39", "diagnostics-after-terminal", &core);
    Ok(())
}

#[cfg(windows)]
async fn int_40_runtime_m0_m6_regression() -> QaResult {
    let fixture = QaWorkspace::new();
    fixture.write("regression.txt", "old");
    let (core, service) = fixture.service();
    let write = service
        .call_tool("write", json!({"path": "regression.txt", "content": "new"}))
        .map_err(|_| "M1 write adapter failed".to_owned())?;
    check!(write.is_error == Some(false));
    let edit = service
        .call_tool(
            "edit",
            json!({"path": "regression.txt", "oldText": "new", "newText": "edited"}),
        )
        .map_err(|_| "M1 edit adapter failed".to_owned())?;
    check!(edit.is_error == Some(false));
    check!(fs::read_to_string(fixture.root.join("regression.txt")).unwrap() == "edited");
    fixture.assert_no_atomic_residue()?;

    // M2 toggle/restart: the stale generation stays disabled until the next
    // RuntimeCore generation is initialized from the enabled snapshot.
    let mut disabled_config = fixture.config();
    disabled_config.tools.write.enabled = false;
    let disabled_core = fixture.core(disabled_config);
    let disabled_service = disabled_core
        .mcp_service()
        .map_err(|_| "M2 disabled MCP unavailable".to_owned())?;
    let disabled = disabled_service
        .call_tool("write", json!({"path": "toggle.txt", "content": "blocked"}))
        .map_err(|_| "M2 disabled call failed".to_owned())?;
    check!(error_code(&disabled) == Some("TOOL_DISABLED"));
    let mut enabled_config = fixture.config();
    enabled_config.config_revision = DecimalU64::new(2);
    let mut restarted = RuntimeCore::new();
    restarted
        .initialize(enabled_config)
        .map_err(|_| "M2 enabled generation initialization failed".to_owned())?;
    restarted.acknowledge();
    let restarted_service = restarted
        .mcp_service()
        .map_err(|_| "M2 restarted MCP unavailable".to_owned())?;
    let enabled = restarted_service
        .call_tool("write", json!({"path": "toggle.txt", "content": "enabled"}))
        .map_err(|_| "M2 enabled call failed".to_owned())?;
    check!(enabled.is_error == Some(false));

    // M3 policy fail-closed is exercised through the same RuntimeCore/MCP
    // boundary rather than a direct PolicyEngine unit call.
    let invalid_fixture = QaWorkspace::new();
    let mut invalid_config = invalid_fixture.config();
    invalid_config.policies.user_rules.push(user_rule(
        "m0-m6-invalid-policy",
        PolicyDecision::Allow,
        "unknown_tool",
    ));
    let invalid_core = invalid_fixture.core(invalid_config);
    let invalid_service = invalid_core
        .mcp_service()
        .map_err(|_| "M3 invalid-policy MCP unavailable".to_owned())?;
    let invalid = invalid_service
        .call_tool("write", json!({"path": "policy.txt", "content": "blocked"}))
        .map_err(|_| "M3 invalid-policy call failed".to_owned())?;
    check!(error_code(&invalid) == Some("POLICY_DENIED"));
    check!(!invalid_fixture.root.join("policy.txt").exists());

    // M4 same-request approval resume and correlation.
    let approval_fixture = QaWorkspace::new();
    let mut approval_config = approval_fixture.config();
    approval_config.policies.tool_defaults.write = PolicyDecision::Ask;
    let approval_core = approval_fixture.core(approval_config);
    let approval_service = approval_core
        .mcp_service()
        .map_err(|_| "M4 approval MCP unavailable".to_owned())?;
    let request_id = run_approval_write(
        "INT-40",
        &approval_core,
        &approval_service,
        "approval.txt",
        ApprovalDecision::AllowOnce,
    )
    .await?;
    check!(approval_fixture.root.join("approval.txt").is_file());
    let approval_entries = approval_core.activity_snapshot(256);
    check!(approval_entries.iter().any(|entry| {
        entry.request_id == request_id && entry.event_type == AuditEventType::ExecutionCompleted
    }));

    // M5 real Bash representative safe path and process cleanup.
    let bash_fixture = QaWorkspace::new();
    let bash_core = bash_fixture.core(bash_config(&bash_fixture, PolicyDecision::Allow)?);
    let bash_service = bash_core
        .mcp_service()
        .map_err(|_| "M5 Bash MCP unavailable".to_owned())?;
    let bash = bash_service
        .call_tool_async("bash", json!({"command": "printf m0-m6"}))
        .await
        .map_err(|_| "M5 Bash call failed".to_owned())?;
    check!(bash.is_error == Some(false));
    wait_for_counts(&bash_core, (0, 0)).await?;
    check!(bash_core
        .bash_runtime()
        .is_some_and(|runtime| runtime.active_count() == 0));
    // Runtime snapshot has no UI language field, so language-only preference
    // changes cannot create a Runtime revision or restart debt.
    check!(core
        .status()
        .is_some_and(|status| status.config_revision == DecimalU64::new(1)));
    log_activity("INT-40", "m0-m6-runtime-regression", &core);
    log_activity("INT-40", "m5-bash-regression", &bash_core);
    Ok(())
}

#[cfg(not(windows))]
async fn int_40_runtime_m0_m6_regression() -> QaResult {
    Err("BLOCKED: M0-M6 Runtime regression includes the Windows Git Bash fixture".into())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn m7_runtime_integration_cases() {
    reset_qa_evidence();
    log_evidence(
        "M7-RUNTIME",
        "run_start",
        json!({
            "command": "cargo test -p chatgpt-harness-runtime m7_runtime_integration_cases -- --nocapture --test-threads=1",
            "scope": "INT-19..34, INT-39, INT-40 Runtime",
        }),
    );
    let mut failures = Vec::new();
    let file_evidence = int_19_and_24_file_evidence().await;
    report_case("INT-19", file_evidence.clone(), &mut failures);
    report_case("INT-24", file_evidence, &mut failures);
    report_case(
        "INT-20",
        int_20_tool_capability_and_disabled_evidence(),
        &mut failures,
    );
    report_case(
        "INT-21",
        int_21_policy_evidence_is_canonical().await,
        &mut failures,
    );
    report_case(
        "INT-22",
        int_22_approval_id_and_decision_correlation().await,
        &mut failures,
    );
    report_case("INT-23", int_23_terminal_durations().await, &mut failures);
    report_case(
        "INT-25",
        int_25_bash_hardened_evidence().await,
        &mut failures,
    );
    report_case(
        "INT-26",
        int_26_bounds_on_long_inputs_and_errors().await,
        &mut failures,
    );
    report_case(
        "INT-27",
        int_27_untrusted_rule_identifiers_are_sanitized(),
        &mut failures,
    );
    report_case(
        "INT-28",
        int_28_active_request_counter_lifecycle().await,
        &mut failures,
    );
    report_case(
        "INT-29",
        int_29_pending_approval_counter_is_real().await,
        &mut failures,
    );
    report_case(
        "INT-30",
        int_30_running_execution_counter_is_real().await,
        &mut failures,
    );
    log_evidence(
        "INT-30",
        "lifecycle-coverage",
        json!({
            "runningExecution": "INT-30",
            "timeoutAndOutputLimit": "INT-25",
            "cancelAndShutdownCleanup": "INT-31",
        }),
    );
    report_case("INT-31", int_31_counter_race_cleanup().await, &mut failures);
    report_case(
        "INT-32",
        int_32_typed_status_reasons_cover_runtime_classes(),
        &mut failures,
    );
    report_case(
        "INT-33",
        int_33_reason_identity_is_deterministic(),
        &mut failures,
    );
    report_case(
        "INT-34",
        int_34_invalid_policy_fails_closed(),
        &mut failures,
    );
    report_case(
        "INT-39",
        int_39_diagnostics_agree_with_runtime_state().await,
        &mut failures,
    );
    report_case(
        "INT-40",
        int_40_runtime_m0_m6_regression().await,
        &mut failures,
    );
    assert!(
        failures.is_empty(),
        "M7 Runtime Integration QA failed cases: {failures:?}"
    );
}
