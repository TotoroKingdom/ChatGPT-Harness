//! M01 real Runtime -> Host notification/approval integration coverage.
//!
//! This runner deliberately starts the packaged/release Runtime executable and
//! drives its real MCP and control protocol. The only test double is the
//! notification sink at the OS boundary, which is the sink explicitly allowed
//! by the M01 Integration plan; Runtime approval state and activity events are
//! never synthesized for the primary cases.

use super::m2_e2e::{runtime_binary, McpClient, PendingMcpCall, RuntimeProcess};
use crate::desktop::notifications::{
    format_approval_notification, route_notification_click, ApprovalClickRoute,
    ApprovalNotificationCoordinator,
};
use harness_control_protocol::{
    ActivityEvent, ActivityEventType, ActivityListResult, ApprovalDecision, ApprovalRequest,
    ApprovalStatus, ApprovalsListResult, PolicyDecision,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(25);
const POLL_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn activity_sink_dispatch_does_not_block_runtime_reader() {
    let supervisor = super::RuntimeSupervisor::new();
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let release_receiver = Arc::new(std::sync::Mutex::new(release_receiver));
    let (returned_sender, returned_receiver) = mpsc::channel();

    supervisor.set_activity_sink(Arc::new(move |_event| {
        started_sender.send(()).expect("activity callback started");
        release_receiver
            .lock()
            .expect("activity callback release lock")
            .recv()
            .expect("activity callback release signal");
    }));
    let sink = supervisor
        .inner
        .lock()
        .expect("runtime supervisor lock")
        .activity_sink
        .clone()
        .expect("activity sink installed");
    let event = ActivityEvent {
        event_id: "m01-reader-dispatch".into(),
        sequence: harness_control_protocol::DecimalU64::new(1),
        event_type: harness_control_protocol::ActivityEventType::ApprovalRequested,
        request_id: Some("m01-request".into()),
        timestamp: "2026-10-24T00:00:00Z".into(),
        summary: "safe".into(),
        metadata: Default::default(),
    };

    let invocation = thread::spawn(move || {
        sink(event);
        returned_sender
            .send(())
            .expect("activity dispatch returned");
    });
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("activity callback did not start");
    let returned = returned_receiver
        .recv_timeout(Duration::from_millis(100))
        .is_ok();
    release_sender.send(()).expect("release activity callback");
    invocation.join().expect("activity dispatch thread");

    assert!(
        returned,
        "activity sink callback blocked the Runtime reader dispatch"
    );
}

type IntegrationResult<T> = Result<T, String>;

#[derive(Default)]
struct RecordingNotificationSink {
    delivered: Vec<String>,
    failed: bool,
}

impl RecordingNotificationSink {
    fn deliver(&mut self, title: &str, body: &str) -> Result<(), ()> {
        if self.failed {
            return Err(());
        }
        self.delivered.push(format!("{title}\n{body}"));
        Ok(())
    }
}

#[test]
#[ignore = "M01 Integration requires the explicitly built candidate Runtime executable"]
fn m01_real_runtime_approval_integration_cases() {
    if let Err(error) = run_suite() {
        panic!("M01 real Runtime integration failed: {error}");
    }
}

fn run_suite() -> IntegrationResult<()> {
    let fixture = Fixture::new()?;
    let binary = runtime_binary().map_err(|error| error.to_string())?;
    let mut config = crate::config::ConfigStore::safe_defaults();
    config.workspace.root_path = fixture.workspace.display().to_string();
    config.policies.tool_defaults.write = PolicyDecision::Ask;

    let mut failures = Vec::new();
    let (mut runtime, mut mcp) =
        RuntimeProcess::start(&binary, &config, "M01-A").map_err(|error| error.to_string())?;

    report_case(
        "M01-INT-04",
        int_04_runtime_pending_projection(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-08",
        int_08_bootstrap_and_dedupe(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-09",
        int_09_delivery_failure_is_advisory(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-10",
        int_10_live_click_routing(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-11",
        int_11_bounded_private_content(&mut runtime, &mut mcp),
        &mut failures,
    );

    let old_runtime_id = runtime.runtime_id().to_owned();
    runtime.stop().map_err(|error| error.to_string())?;
    let (mut rollover_runtime, mut rollover_mcp) =
        RuntimeProcess::start(&binary, &config, "M01-B").map_err(|error| error.to_string())?;
    report_case(
        "M01-INT-08-session-rollover",
        int_08_session_rollover(&mut rollover_runtime, &mut rollover_mcp, &old_runtime_id),
        &mut failures,
    );
    rollover_runtime.stop().map_err(|error| error.to_string())?;

    fixture.cleanup()?;
    if !failures.is_empty() {
        return Err(format!("failed cases: {failures:?}"));
    }
    println!("M01_REAL_RUNTIME_APPROVAL_INTEGRATION=PASS");
    Ok(())
}

fn int_04_runtime_pending_projection(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let initial = pending_approvals(runtime)?;
    let initial_refresh = coordinator.bootstrap(&initial);
    check(
        initial_refresh.pending_count.label() == "0",
        "initial Pending count was not Known(0)",
    )?;

    let pending = start_pending_write(mcp)?;
    let first = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &first[0].request_id)?;
    let refresh = coordinator.observe_activity(&event, &first);
    check(
        refresh.pending_count.label() == "1",
        "new Pending count was not Known(1)",
    )?;

    let second = pending_approvals(runtime)?;
    check(
        second.len() == 1,
        "Runtime approvals.list did not retain one Pending",
    )?;
    let duplicate = coordinator.observe_activity(&event, &second);
    check(
        duplicate.pending_count.label() == "1" && duplicate.notification.is_none(),
        "duplicate activity changed the authoritative Pending projection",
    )?;

    resolve(runtime, &first[0].request_id, ApprovalDecision::Deny)?;
    let terminal = wait_for_zero_pending(runtime)?;
    let terminal_event = wait_for_terminal_event(runtime, &first[0].request_id)?;
    let terminal_refresh = coordinator.observe_activity(&terminal_event, &terminal);
    check(
        terminal_refresh.pending_count.label() == "0",
        "resolved approval did not refresh Pending count to zero",
    )?;
    pending.receive().map_err(|error| error.to_string())?;

    let unavailable = coordinator.runtime_unavailable();
    check(
        unavailable.pending_count == crate::desktop::PendingApprovalCount::Unavailable,
        "Runtime loss did not project Unavailable",
    )?;
    record(
        "M01-INT-04",
        "real Runtime approvals.list + MCP ASK + authoritative refresh",
    )
}

fn int_08_bootstrap_and_dedupe(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let bootstrap = coordinator.bootstrap(&approvals);
    check(
        bootstrap.notification.is_none() && bootstrap.pending_count.label() == "1",
        "pre-existing Pending approval caused a notification burst",
    )?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let seeded_event = coordinator.observe_activity(&event, &approvals);
    check(
        seeded_event.notification.is_none(),
        "bootstrap-seeded approval notified again",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;

    let next = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let first = coordinator.observe_activity(&event, &approvals);
    check(
        first.notification.is_some(),
        "new approval did not produce one decision",
    )?;
    let duplicate = coordinator.observe_activity(&event, &approvals);
    check(
        duplicate.notification.is_none(),
        "duplicate ApprovalRequested produced a second decision",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    next.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-08",
        "real Runtime bootstrap, ActivityEvent and dedupe",
    )
}

fn int_09_delivery_failure_is_advisory(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let mut sink = RecordingNotificationSink::default();
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let refresh = coordinator.observe_activity(&event, &approvals);
    let content = refresh
        .notification
        .ok_or_else(|| "real Pending did not create notification content".to_owned())?;
    sink.failed = true;
    check(
        sink.deliver(&content.title, &content.body).is_err(),
        "failure sink unexpectedly succeeded",
    )?;
    coordinator.record_delivery_failure();
    let current = pending_approvals(runtime)?;
    check(
        current.len() == 1 && current[0].status == ApprovalStatus::Pending,
        "notification delivery failure changed Runtime Approval truth",
    )?;
    let count = coordinator.refresh(&current);
    check(
        count.pending_count.label() == "1",
        "notification failure made Pending count unavailable/stale",
    )?;
    resolve(runtime, &current[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &current[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-09",
        "recording sink failure leaves Runtime Pending and query path intact",
    )
}

fn int_10_live_click_routing(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let pending = start_pending_write(mcp)?;
    let current = wait_for_pending(runtime)?;
    let pending_route =
        route_notification_click(&current[0].runtime_id, &current[0].request_id, &current);
    check(
        pending_route == ApprovalClickRoute::Pending(current[0].request_id.clone()),
        "live Pending click was not actionable",
    )?;
    let id = current[0].request_id.clone();
    resolve(runtime, &id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    let terminal = all_approvals(runtime)?;
    let terminal_route = route_notification_click(&current[0].runtime_id, &id, &terminal);
    check(
        terminal_route == ApprovalClickRoute::Terminal(id.clone()),
        "click after Denied was not routed to terminal state",
    )?;
    let terminal_history = wait_for_terminal_event(runtime, &id)?;
    check(
        matches!(
            terminal_history.event_type,
            ActivityEventType::ApprovalDenied
        ),
        "terminal click route did not observe the Runtime terminal event",
    )?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-10",
        "click decision re-queried live Runtime Approval state",
    )
}

fn int_11_bounded_private_content(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let content = format_approval_notification(&approvals[0])
        .ok_or_else(|| "known write approval was not formatable".to_owned())?;
    let payload = format!("{}\n{}", content.title, content.body);
    let sentinels = [
        "M01_RAW_API_KEY_SENTINEL",
        "M01_PHYSICAL_PATH_SENTINEL",
        "M01_RAW_BASH_SENTINEL",
        "M01_FILE_CONTENT_SENTINEL",
        "M01_ERROR_DETAIL_SENTINEL",
        "M01_METADATA_JSON_SENTINEL",
    ];
    for sentinel in sentinels {
        check(
            !payload.contains(sentinel),
            &format!("notification payload leaked {sentinel}"),
        )?;
    }
    check(
        !payload.contains(&approvals[0].scope_summary),
        "notification payload serialized the full approval scope",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-11",
        "real Approval formatted through bounded production formatter",
    )
}

fn int_08_session_rollover(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    old_runtime_id: &str,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let refresh = coordinator.observe_activity(&event, &approvals);
    check(
        refresh.notification.is_some(),
        "new Runtime session did not notify",
    )?;
    check(
        approvals[0].runtime_id == runtime.runtime_id()
            && approvals[0].runtime_id != old_runtime_id,
        "approval identity did not isolate the active Runtime session",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-08-session-rollover",
        "new Runtime identity has an isolated dedupe key",
    )
}

fn start_pending_write(mcp: &mut McpClient) -> IntegrationResult<PendingMcpCall> {
    Ok(mcp.pending_tool(
        "write",
        json!({
            "path": "m01-pending.txt",
            "content": "M01 approval content"
        }),
    ))
}

fn all_approvals(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    let value = runtime
        .control_request("approvals.list", json!({}))
        .map_err(|error| error.to_string())?;
    serde_json::from_value::<ApprovalsListResult>(value)
        .map(|result| result.items)
        .map_err(|error| format!("decode approvals.list: {error}"))
}

fn pending_approvals(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    let value = runtime
        .control_request("approvals.list", json!({ "status": "pending" }))
        .map_err(|error| error.to_string())?;
    serde_json::from_value::<ApprovalsListResult>(value)
        .map(|result| result.items)
        .map_err(|error| format!("decode approvals.list pending: {error}"))
}

fn wait_for_pending(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    wait_until(runtime, |items| !items.is_empty(), "Pending approval")
}

fn wait_for_zero_pending(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    wait_until(runtime, |items| items.is_empty(), "zero Pending approvals")
}

fn wait_until(
    runtime: &mut RuntimeProcess,
    predicate: impl Fn(&[ApprovalRequest]) -> bool,
    label: &str,
) -> IntegrationResult<Vec<ApprovalRequest>> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let current = pending_approvals(runtime)?;
        if predicate(&current) {
            return Ok(current);
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for {label}"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for_approval_event(
    runtime: &mut RuntimeProcess,
    request_id: &str,
) -> IntegrationResult<ActivityEvent> {
    wait_for_event(runtime, request_id, |event| {
        event.event_type == ActivityEventType::ApprovalRequested
    })
}

fn wait_for_terminal_event(
    runtime: &mut RuntimeProcess,
    request_id: &str,
) -> IntegrationResult<ActivityEvent> {
    wait_for_event(runtime, request_id, |event| {
        matches!(
            event.event_type,
            ActivityEventType::ApprovalDenied
                | ActivityEventType::ApprovalResolved
                | ActivityEventType::ApprovalExpired
                | ActivityEventType::ApprovalCancelled
        )
    })
}

fn wait_for_event(
    runtime: &mut RuntimeProcess,
    request_id: &str,
    predicate: impl Fn(&ActivityEvent) -> bool,
) -> IntegrationResult<ActivityEvent> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let value = runtime
            .control_request("activity.list", json!({ "limit": 256 }))
            .map_err(|error| error.to_string())?;
        let result: ActivityListResult = serde_json::from_value(value)
            .map_err(|error| format!("decode activity.list: {error}"))?;
        if let Some(event) = result
            .items
            .into_iter()
            .find(|event| event.request_id.as_deref() == Some(request_id) && predicate(event))
        {
            return Ok(event);
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for activity event {request_id}"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn resolve(
    runtime: &mut RuntimeProcess,
    request_id: &str,
    decision: ApprovalDecision,
) -> IntegrationResult<()> {
    runtime
        .control_request(
            "approvals.resolve",
            json!({ "requestId": request_id, "decision": decision }),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn check(condition: bool, message: &str) -> IntegrationResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}

fn report_case(case_id: &str, result: IntegrationResult<()>, failures: &mut Vec<String>) {
    match result {
        Ok(()) => {
            println!("{case_id} PASS");
            record(
                case_id,
                "real Runtime/Host notification integration assertions passed",
            )
            .expect("write M01 integration evidence");
        }
        Err(error) => {
            println!("{case_id} FAIL: {error}");
            record(case_id, &format!("FAIL: {error}")).expect("write M01 integration evidence");
            failures.push(case_id.to_owned());
        }
    }
}

fn record(case_id: &str, evidence: &str) -> IntegrationResult<()> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("m01-qa");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("create evidence directory: {error}"))?;
    let path = directory.join("m01-real-runtime-evidence.jsonl");
    let line = json!({
        "case": case_id,
        "evidence": evidence,
        "candidate": std::env::var("HARNESS_M01_CANDIDATE")
            .unwrap_or_else(|_| "unspecified".into()),
    });
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open evidence file: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("write evidence: {error}"))
}

struct Fixture {
    base: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> IntegrationResult<Self> {
        let base = std::env::temp_dir().join(format!(
            "chatgpt-harness-m01-real-runtime-{}",
            uuid::Uuid::new_v4()
        ));
        let workspace = base.join("workspace");
        fs::create_dir_all(&workspace).map_err(|error| format!("create fixture: {error}"))?;
        fs::write(workspace.join("m01-existing.txt"), b"M01 existing")
            .map_err(|error| format!("write fixture: {error}"))?;
        Ok(Self { base, workspace })
    }

    fn cleanup(&self) -> IntegrationResult<()> {
        if self.base.exists() {
            fs::remove_dir_all(&self.base).map_err(|error| format!("cleanup fixture: {error}"))?;
        }
        check(!self.base.exists(), "fixture cleanup left temporary data")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
