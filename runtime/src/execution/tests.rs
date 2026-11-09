use super::state::RequestLifecycle;
use super::*;
use crate::audit::{
    AuditEventType, AuditFailureCategory, AuditLifecycleState, AuditOutcome, WorkspaceValidation,
};
use crate::mcp::{McpExecutionContext, McpService};
use crate::policy::{PolicyEngine, PolicyEvaluationReason};
use crate::tools::{
    AtomicPhase, EditError, EditRequest, GlobRequest, ReadFileError, ReadFileRequest, ToolRegistry,
    WriteError, WriteRequest,
};
use crate::workspace::{Workspace, WorkspaceError, WorkspaceGuard};
use harness_control_protocol::{PolicyDecision, PolicyRule, PolicySource, SecurityMode};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

fn reach(state: RequestState) -> RequestLifecycle {
    use RequestState::*;
    let mut machine = RequestLifecycle::new();
    if state == Received {
        return machine;
    }
    machine.transition(Validating).unwrap();
    if state == Validating {
        return machine;
    }
    machine.transition(WorkspaceValidating).unwrap();
    if state == WorkspaceValidating {
        return machine;
    }
    machine.transition(PolicyEvaluating).unwrap();
    if state == PolicyEvaluating {
        return machine;
    }
    if matches!(state, Denied | AwaitingApproval) {
        machine.transition(state).unwrap();
        return machine;
    }
    machine.transition(Executing).unwrap();
    if state != Executing {
        machine.transition(state).unwrap();
    }
    machine
}

#[test]
fn every_state_pair_is_explicitly_allowed_or_rejected_without_mutation() {
    use RequestState::*;
    let states = [
        Received,
        Validating,
        WorkspaceValidating,
        PolicyEvaluating,
        Denied,
        AwaitingApproval,
        Executing,
        Completed,
        Failed,
    ];
    let legal = [
        (Received, Validating),
        (Validating, WorkspaceValidating),
        (Validating, Failed),
        (WorkspaceValidating, PolicyEvaluating),
        (WorkspaceValidating, Failed),
        (PolicyEvaluating, Denied),
        (PolicyEvaluating, AwaitingApproval),
        (PolicyEvaluating, Executing),
        (PolicyEvaluating, Failed),
        (AwaitingApproval, Denied),
        (AwaitingApproval, Executing),
        (AwaitingApproval, Failed),
        (Executing, Completed),
        (Executing, Failed),
    ];
    for from in states {
        for to in states {
            let mut machine = reach(from);
            let result = machine.transition(to);
            if legal.contains(&(from, to)) {
                assert_eq!(result, Ok(()), "{from:?} -> {to:?}");
                assert_eq!(machine.state(), to);
            } else {
                assert_eq!(result, Err(TransitionError { from, to }));
                assert_eq!(machine.state(), from);
            }
        }
    }
}

#[derive(Debug)]
struct Observed {
    id: Uuid,
    workspace_id: Uuid,
    tool: &'static str,
    state: RequestState,
    event: PipelineEvent,
    decision: Option<PolicyDecision>,
    safe_context: String,
}

#[derive(Default)]
struct Recorder(Mutex<Vec<Observed>>);
impl ExecutionHook for Recorder {
    fn on_event(&self, context: &RequestContext, event: PipelineEvent) {
        self.0.lock().unwrap().push(Observed {
            id: context.request_id,
            workspace_id: context.security.workspace_id,
            tool: context.tool_name,
            state: context.state(),
            event,
            decision: context
                .policy_evaluation
                .as_ref()
                .map(|e| e.decision.clone()),
            safe_context: format!("{context:?}"),
        });
    }
}
impl Recorder {
    fn states(&self) -> Vec<RequestState> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|item| match item.event {
                PipelineEvent::RequestReceived | PipelineEvent::StateChanged { .. } => {
                    Some(item.state)
                }
                _ => None,
            })
            .collect()
    }
    fn has(&self, event: PipelineEvent) -> bool {
        self.0
            .lock()
            .unwrap()
            .iter()
            .any(|item| item.event == event)
    }
}

struct Fixture {
    base: PathBuf,
    root: PathBuf,
    guard: Arc<WorkspaceGuard>,
    recorder: Arc<Recorder>,
    ready: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    workspace_id: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("harness-enforcement-{}", Uuid::new_v4()));
        let root = base.join("workspace");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("sample.txt"), "one\ntwo\n").unwrap();
        fs::write(base.join("outside-secret.txt"), "do not read").unwrap();
        Self {
            guard: Arc::new(WorkspaceGuard::new(&Workspace::new(&root).unwrap())),
            base,
            root,
            recorder: Arc::new(Recorder::default()),
            ready: Arc::new(AtomicBool::new(true)),
            calls: Arc::new(AtomicUsize::new(0)),
            workspace_id: Uuid::new_v4(),
        }
    }
    fn pipeline(&self, rules: Vec<PolicyRule>) -> ExecutionPipeline {
        self.pipeline_with_registry(rules, ToolRegistry::normal_mode())
    }

    fn pipeline_with_registry(
        &self,
        rules: Vec<PolicyRule>,
        registry: ToolRegistry,
    ) -> ExecutionPipeline {
        ExecutionPipeline::new(
            Arc::clone(&self.guard),
            Arc::new(registry),
            PolicyEngine::new(rules),
            SecurityContext {
                workspace_id: self.workspace_id,
                mode: SecurityMode::Normal,
            },
            64,
            Arc::clone(&self.ready),
        )
        .with_hook(self.recorder.clone())
    }
    fn spy(&self, path: &str, fail: bool) -> Spy {
        Spy {
            path: path.into(),
            calls: Arc::clone(&self.calls),
            fail,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

struct Spy {
    path: PathBuf,
    calls: Arc<AtomicUsize>,
    fail: bool,
}

struct FailingEdit {
    path: PathBuf,
    error: EditError,
}

impl ToolExecutor for FailingEdit {
    type Output = ();
    const TOOL_NAME: &'static str = "edit";

    fn validate(&self) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn workspace_target(&self) -> WorkspaceTarget<'_> {
        WorkspaceTarget::ExistingPath(&self.path)
    }

    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::Edit(EditError::Workspace(error))
    }

    fn execute(
        &self,
        _permit: &ExecutionPermit,
        _guard: &WorkspaceGuard,
        _limit: u32,
        _search_budgets: crate::tools::search::control::SearchBudgets,
    ) -> Result<Self::Output, ExecutionError> {
        Err(ExecutionError::Edit(self.error.clone()))
    }
}
impl ToolExecutor for Spy {
    type Output = ();
    const TOOL_NAME: &'static str = "read_file";
    fn validate(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
    fn workspace_target(&self) -> WorkspaceTarget<'_> {
        WorkspaceTarget::ExistingPath(&self.path)
    }
    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::ReadFile(ReadFileError::Workspace(error))
    }
    fn execute(
        &self,
        _permit: &ExecutionPermit,
        _guard: &WorkspaceGuard,
        _limit: u32,
        _search_budgets: crate::tools::search::control::SearchBudgets,
    ) -> Result<(), ExecutionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(ExecutionError::ReadFile(ReadFileError::IoFailure {
                path: self.path.clone(),
                kind: std::io::ErrorKind::PermissionDenied,
            }))
        } else {
            Ok(())
        }
    }
}

fn rules(decision: PolicyDecision) -> Vec<PolicyRule> {
    let mut rules = PolicyEngine::system_rules();
    rules.push(PolicyRule {
        id: "test-restriction".into(),
        name: "Test restriction".into(),
        decision,
        scope: "active_workspace".into(),
        reason: "Test".into(),
        source: PolicySource::User,
        editable: true,
        enabled: true,
    });
    rules
}

#[test]
fn deny_and_ask_stop_after_policy_without_executor_or_approval_resume() {
    use RequestState::*;
    for (decision, state, error) in [
        (PolicyDecision::Deny, Denied, ExecutionError::PolicyDenied),
        (
            PolicyDecision::Ask,
            AwaitingApproval,
            ExecutionError::ApprovalRequired,
        ),
    ] {
        let fixture = Fixture::new();
        let report = fixture
            .pipeline(rules(decision.clone()))
            .execute(|| Ok(fixture.spy("sample.txt", false)));
        assert_eq!(report.context.state(), state);
        assert_eq!(report.result, Err(error));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            fixture.recorder.states(),
            [
                Received,
                Validating,
                WorkspaceValidating,
                PolicyEvaluating,
                state
            ]
        );
        assert_eq!(report.context.policy_evaluation.unwrap().decision, decision);
        assert!(!fixture.recorder.has(PipelineEvent::ExecutionStarted));
        assert!(fixture.recorder.has(PipelineEvent::RequestFinished));
    }
}

#[test]
fn disabled_tool_is_rejected_before_workspace_policy_or_executor() {
    use RequestState::*;

    let fixture = Fixture::new();
    let mut config = harness_control_protocol::ToolConfig::default();
    config.read_file.enabled = false;
    let report = fixture
        .pipeline_with_registry(
            PolicyEngine::system_rules(),
            ToolRegistry::from_config(&config),
        )
        .execute::<Spy>(|| panic!("disabled tool must not parse or execute"));

    assert_eq!(report.result, Err(ExecutionError::ToolDisabled));
    assert_eq!(report.context.state(), Failed);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.recorder.states(), [Received, Validating, Failed]);
    assert!(!fixture
        .recorder
        .has(PipelineEvent::WorkspaceValidated { passed: true }));
    assert!(!fixture.recorder.has(PipelineEvent::PolicyDecision));
    assert!(!fixture.recorder.has(PipelineEvent::ExecutionStarted));
    assert!(!fixture.recorder.has(PipelineEvent::ExecutionCompleted));

    let failure = report
        .audit_events
        .iter()
        .find(|event| event.event_type == AuditEventType::ExecutionFailed)
        .expect("disabled rejection audit event");
    assert_eq!(
        failure.failure_category,
        Some(AuditFailureCategory::ToolDisabled)
    );
    assert_eq!(failure.policy_decision, None);
    assert!(!serde_json::to_string(failure)
        .unwrap()
        .contains("POLICY_DENIED"));
}

#[test]
fn allow_executes_exactly_once_and_reports_ordered_lifecycle_and_hooks() {
    use RequestState::*;
    let fixture = Fixture::new();
    let report = fixture
        .pipeline(PolicyEngine::system_rules())
        .execute(|| Ok(fixture.spy("sample.txt", false)));
    assert_eq!(report.result, Ok(()));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        fixture.recorder.states(),
        [
            Received,
            Validating,
            WorkspaceValidating,
            PolicyEvaluating,
            Executing,
            Completed
        ]
    );
    let events = fixture.recorder.0.lock().unwrap();
    assert!(events.iter().all(|e| e.id == report.context.request_id
        && e.workspace_id == fixture.workspace_id
        && e.tool == "read_file"));
    let policy = events
        .iter()
        .position(|e| e.event == PipelineEvent::PolicyDecision)
        .unwrap();
    let started = events
        .iter()
        .position(|e| e.event == PipelineEvent::ExecutionStarted)
        .unwrap();
    assert!(policy < started);
    assert_eq!(events[policy].decision, Some(PolicyDecision::Allow));
    assert_eq!(events.last().unwrap().event, PipelineEvent::RequestFinished);
    assert_eq!(events.last().unwrap().state, Completed);
}

#[test]
fn workspace_denial_precedes_even_allow_policy_and_never_enters_executor() {
    use RequestState::*;
    for decision in [
        PolicyDecision::Allow,
        PolicyDecision::Ask,
        PolicyDecision::Deny,
    ] {
        let fixture = Fixture::new();
        let report = fixture
            .pipeline(rules(decision))
            .execute(|| Ok(fixture.spy("../outside-secret.txt", false)));
        assert_eq!(report.context.state(), Failed);
        assert!(matches!(
            report.result,
            Err(ExecutionError::ReadFile(ReadFileError::Workspace(
                WorkspaceError::WorkspaceEscape { .. }
            )))
        ));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            fixture.recorder.states(),
            [Received, Validating, WorkspaceValidating, Failed]
        );
        assert!(report.context.policy_evaluation.is_none());
        assert!(!fixture.recorder.has(PipelineEvent::PolicyDecision));
        assert!(!fixture.recorder.has(PipelineEvent::ExecutionStarted));
    }
}

struct ReplaceTargetWithOutsideLink {
    target: PathBuf,
    outside: PathBuf,
    linked: Arc<AtomicBool>,
}

impl ExecutionHook for ReplaceTargetWithOutsideLink {
    fn on_event(&self, _: &RequestContext, event: PipelineEvent) {
        if event != PipelineEvent::PolicyDecision {
            return;
        }
        let _ = fs::remove_file(&self.target);
        if link_file(&self.outside, &self.target) {
            self.linked.store(true, Ordering::Release);
        }
    }
}

#[cfg(unix)]
fn link_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

#[cfg(not(any(unix, windows)))]
fn link_file(_: &Path, _: &Path) -> bool {
    false
}

#[test]
fn write_and_edit_revalidate_targets_after_policy_before_mutation() {
    let fixture = Fixture::new();
    let outside = fixture.base.join("outside-target.txt");
    fs::write(&outside, "outside").expect("outside fixture");

    let write_target = fixture.root.join("write-target.txt");
    fs::write(&write_target, "inside").expect("write fixture");
    let write_linked = Arc::new(AtomicBool::new(false));
    let write_report = fixture
        .pipeline(PolicyEngine::system_rules())
        .with_hook(Arc::new(ReplaceTargetWithOutsideLink {
            target: write_target.clone(),
            outside: outside.clone(),
            linked: write_linked.clone(),
        }))
        .execute(|| Ok(WriteRequest::new("write-target.txt", "blocked")));
    if !write_linked.load(Ordering::Acquire) {
        eprintln!("SKIP revalidation symlink test: file symlink creation is unavailable");
        return;
    }
    assert!(matches!(
        write_report.result,
        Err(ExecutionError::Write(WriteError::Workspace(
            WorkspaceError::WorkspaceEscape { .. }
        )))
    ));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");

    let edit_target = fixture.root.join("edit-target.txt");
    fs::write(&edit_target, "inside").expect("edit fixture");
    let edit_linked = Arc::new(AtomicBool::new(false));
    let edit_report = fixture
        .pipeline(PolicyEngine::system_rules())
        .with_hook(Arc::new(ReplaceTargetWithOutsideLink {
            target: edit_target,
            outside: outside.clone(),
            linked: edit_linked.clone(),
        }))
        .execute(|| Ok(EditRequest::new("edit-target.txt", "inside", "blocked")));
    assert!(edit_linked.load(Ordering::Acquire));
    assert!(matches!(
        edit_report.result,
        Err(ExecutionError::Edit(EditError::Workspace(
            WorkspaceError::WorkspaceEscape { .. }
        )))
    ));
    assert_eq!(fs::read_to_string(outside).unwrap(), "outside");
}

#[test]
fn executor_failure_is_failed_not_policy_denied() {
    let fixture = Fixture::new();
    let report = fixture
        .pipeline(PolicyEngine::system_rules())
        .execute(|| Ok(fixture.spy("sample.txt", true)));
    assert_eq!(report.context.state(), RequestState::Failed);
    assert!(matches!(
        report.result,
        Err(ExecutionError::ReadFile(ReadFileError::IoFailure { .. }))
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        report.context.policy_evaluation.unwrap().decision,
        PolicyDecision::Allow
    );
    assert!(fixture.recorder.has(PipelineEvent::ExecutionStarted));
    assert_eq!(
        report.audit_events.last().unwrap().failure_category,
        Some(AuditFailureCategory::IoFailure)
    );
}

#[test]
fn audit_evidence_distinguishes_target_change_and_atomic_failure() {
    for (error, expected) in [
        (
            EditError::TargetChanged {
                path: PathBuf::from("secret.txt"),
            },
            AuditFailureCategory::TargetChanged,
        ),
        (
            EditError::AtomicFailure {
                path: PathBuf::from("secret.txt"),
                phase: AtomicPhase::Commit,
                kind: std::io::ErrorKind::PermissionDenied,
            },
            AuditFailureCategory::AtomicFailure,
        ),
    ] {
        let fixture = Fixture::new();
        let report = fixture.pipeline(PolicyEngine::system_rules()).execute(|| {
            Ok(FailingEdit {
                path: PathBuf::from("sample.txt"),
                error,
            })
        });
        let failure = report
            .audit_events
            .iter()
            .find(|event| event.event_type == AuditEventType::ExecutionFailed)
            .expect("execution failure audit event");
        assert_eq!(failure.failure_category, Some(expected));
        let serialized = serde_json::to_string(failure).unwrap();
        assert!(!serialized.contains("secret.txt"));
        assert!(!serialized.contains("PermissionDenied"));
    }
}

#[test]
fn invalid_arguments_and_patterns_fail_before_workspace_or_policy() {
    use RequestState::*;
    let fixture = Fixture::new();
    let report = fixture
        .pipeline(PolicyEngine::system_rules())
        .execute::<Spy>(|| Err(ExecutionError::InvalidArguments));
    assert_eq!(report.result, Err(ExecutionError::InvalidArguments));
    assert_eq!(fixture.recorder.states(), [Received, Validating, Failed]);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
    fixture.recorder.0.lock().unwrap().clear();
    let report = fixture
        .pipeline(PolicyEngine::system_rules())
        .execute(|| Ok(GlobRequest::new("[").with_base_path("missing")));
    assert!(matches!(
        report.result,
        Err(ExecutionError::Glob(
            crate::tools::GlobError::InvalidPattern { .. }
        ))
    ));
    assert_eq!(fixture.recorder.states(), [Received, Validating, Failed]);
}

#[test]
fn empty_or_invalid_policy_snapshot_is_fail_closed() {
    for rules in [vec![], {
        let mut r = PolicyEngine::system_rules();
        r[0].scope = "unknown".into();
        r
    }] {
        let fixture = Fixture::new();
        let report = fixture
            .pipeline(rules)
            .execute(|| Ok(fixture.spy("sample.txt", false)));
        assert_eq!(report.context.state(), RequestState::Denied);
        assert_eq!(report.result, Err(ExecutionError::PolicyDenied));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            report.context.policy_evaluation.unwrap().reason,
            PolicyEvaluationReason::NoApplicableRules | PolicyEvaluationReason::InvalidRuleSet
        ));
    }
}

#[test]
fn readiness_gate_precedes_parsing_and_is_checked_again_after_policy() {
    let fixture = Fixture::new();
    fixture.ready.store(false, Ordering::Release);
    let report = fixture
        .pipeline(PolicyEngine::system_rules())
        .execute::<Spy>(|| panic!("must not parse"));
    assert_eq!(report.result, Err(ExecutionError::RuntimeNotReady));
    assert_eq!(report.context.state(), RequestState::Failed);
    struct StopAtPolicy(Arc<AtomicBool>);
    impl ExecutionHook for StopAtPolicy {
        fn on_event(&self, _: &RequestContext, event: PipelineEvent) {
            if event == PipelineEvent::PolicyDecision {
                self.0.store(false, Ordering::Release);
            }
        }
    }
    fixture.ready.store(true, Ordering::Release);
    let report = fixture
        .pipeline(PolicyEngine::system_rules())
        .with_hook(Arc::new(StopAtPolicy(fixture.ready.clone())))
        .execute(|| Ok(fixture.spy("sample.txt", false)));
    assert_eq!(report.result, Err(ExecutionError::RuntimeNotReady));
    assert_eq!(report.context.state(), RequestState::Failed);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mcp_deny_ask_and_allow_preserve_catalog_and_existing_result_envelope() {
    for (decision, code, state) in [
        (
            PolicyDecision::Deny,
            Some("POLICY_DENIED"),
            RequestState::Denied,
        ),
        (
            PolicyDecision::Ask,
            Some("APPROVAL_REQUIRED"),
            RequestState::AwaitingApproval,
        ),
        (PolicyDecision::Allow, None, RequestState::Completed),
    ] {
        for (tool, arguments) in [
            ("read_file", json!({"path":"sample.txt"})),
            ("glob", json!({"pattern":"*.txt"})),
        ] {
            let fixture = Fixture::new();
            let service = McpService::new(McpExecutionContext::new(Arc::new(
                fixture.pipeline(rules(decision.clone())),
            )));
            let result = service.call_tool(tool, arguments).unwrap();
            let data = result.structured_content.unwrap();
            if let Some(code) = code {
                assert_eq!(result.is_error, Some(true));
                assert_eq!(data["code"], code);
                assert_eq!(data.as_object().unwrap().len(), 2);
                assert!(!fixture.recorder.has(PipelineEvent::ExecutionStarted));
            } else {
                assert_ne!(result.is_error, Some(true));
                if tool == "read_file" {
                    assert_eq!(data["content"], "one\ntwo\n");
                } else {
                    assert_eq!(data["matches"][0]["path"], "sample.txt");
                }
            }
            assert_eq!(
                fixture.recorder.0.lock().unwrap().last().unwrap().state,
                state
            );
        }
    }
}

#[test]
fn mcp_errors_and_hooks_never_echo_payload_paths_or_file_content() {
    let fixture = Fixture::new();
    let service = McpService::new(McpExecutionContext::new(Arc::new(
        fixture.pipeline(PolicyEngine::system_rules()),
    )));
    let result = service
        .call_tool(
            "read_file",
            json!({"path": fixture.base.join("outside-secret.txt"), "apiKey":"PAYLOAD-SECRET"}),
        )
        .unwrap();
    assert_eq!(
        result.structured_content.as_ref().unwrap()["code"],
        "INVALID_ARGUMENTS"
    );
    let result2 = service
        .call_tool(
            "read_file",
            json!({"path": fixture.base.join("outside-secret.txt")}),
        )
        .unwrap();
    assert_eq!(
        result2.structured_content.as_ref().unwrap()["code"],
        "WORKSPACE_VIOLATION"
    );
    let serialized = format!(
        "{}{}",
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&result2).unwrap()
    );
    for secret in ["PAYLOAD-SECRET", "outside-secret.txt", "do not read"] {
        assert!(!serialized.contains(secret));
    }
    let events = fixture.recorder.0.lock().unwrap();
    for event in events.iter() {
        for secret in [
            "PAYLOAD-SECRET",
            "outside-secret.txt",
            "do not read",
            fixture.root.to_str().unwrap(),
        ] {
            assert!(!event.safe_context.contains(secret));
        }
    }
    let ids = events
        .iter()
        .filter(|e| e.event == PipelineEvent::RequestReceived)
        .map(|e| e.id)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn real_executor_content_error_maps_to_existing_code_and_failed_state() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("binary.txt"), b"private\0payload").unwrap();
    let service = McpService::new(McpExecutionContext::new(Arc::new(
        fixture.pipeline(PolicyEngine::system_rules()),
    )));
    let result = service
        .call_tool("read_file", json!({"path":"binary.txt"}))
        .unwrap();
    assert_eq!(
        result.structured_content.as_ref().unwrap()["code"],
        "UNSUPPORTED_CONTENT"
    );
    assert!(fixture.recorder.has(PipelineEvent::ExecutionStarted));
    assert_eq!(
        fixture.recorder.0.lock().unwrap().last().unwrap().state,
        RequestState::Failed
    );
}

#[test]
fn audit_events_cover_allow_deny_and_ask_branches_in_order() {
    use crate::activity::ActivityStore;
    use AuditEventType::*;

    for (decision, expected_types, expected_terminal) in [
        (
            PolicyDecision::Allow,
            vec![
                RequestReceived,
                WorkspaceValidated,
                PolicyEvaluated,
                ExecutionStarted,
                ExecutionCompleted,
            ],
            AuditOutcome::Success,
        ),
        (
            PolicyDecision::Deny,
            vec![
                RequestReceived,
                WorkspaceValidated,
                PolicyEvaluated,
                RequestDenied,
            ],
            AuditOutcome::Denied,
        ),
        (
            PolicyDecision::Ask,
            vec![
                RequestReceived,
                WorkspaceValidated,
                PolicyEvaluated,
                ApprovalRequired,
            ],
            AuditOutcome::ApprovalRequired,
        ),
    ] {
        let fixture = Fixture::new();
        let activity = Arc::new(ActivityStore::new(Arc::new(ToolRegistry::normal_mode())));
        let report = fixture
            .pipeline(rules(decision.clone()))
            .with_audit_sink(activity.clone())
            .execute(|| Ok(fixture.spy("sample.txt", false)));
        let audit_events = report.audit_events;
        let entries = activity.snapshot(20);
        assert_eq!(entries.len(), audit_events.len());
        for (entry, audit) in entries.iter().zip(&audit_events) {
            assert_eq!(entry.event_id, audit.event_id);
            assert_eq!(entry.request_id, audit.request_id);
            assert_eq!(entry.event_type, audit.event_type);
            assert_eq!(entry.lifecycle_state, audit.lifecycle_state);
            assert_eq!(entry.decision_summary, audit.policy_decision);
            assert_eq!(entry.outcome_summary, audit.outcome);
        }

        assert_eq!(
            audit_events
                .iter()
                .map(|event| event.event_type)
                .collect::<Vec<_>>(),
            expected_types
        );
        assert!(audit_events
            .iter()
            .all(|event| event.request_id == report.context.request_id));
        assert_eq!(
            audit_events[1].workspace_validation,
            WorkspaceValidation::Passed
        );
        assert_eq!(audit_events.last().unwrap().outcome, expected_terminal);
        assert_eq!(
            audit_events.last().unwrap().lifecycle_state,
            match decision {
                PolicyDecision::Allow => AuditLifecycleState::Completed,
                PolicyDecision::Deny => AuditLifecycleState::Denied,
                PolicyDecision::Ask => AuditLifecycleState::AwaitingApproval,
            }
        );
    }
}

#[test]
fn audit_events_have_stable_request_order_and_redacted_argument_boundary() {
    let fixture = Fixture::new();
    let input = json!({
        "secret": "SECRET-VALUE",
        "api_key": "API-KEY-VALUE",
        "credential": "CREDENTIAL-VALUE",
    });
    let report = fixture.pipeline(PolicyEngine::system_rules()).execute(|| {
        assert!(input.get("secret").is_some());
        Ok(fixture.spy("sample.txt", false))
    });

    let serialized = serde_json::to_string(&report.audit_events).unwrap();
    for secret in ["SECRET-VALUE", "API-KEY-VALUE", "CREDENTIAL-VALUE"] {
        assert!(!serialized.contains(secret));
    }
    assert_eq!(
        report
            .audit_events
            .iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>(),
        vec![
            AuditEventType::RequestReceived,
            AuditEventType::WorkspaceValidated,
            AuditEventType::PolicyEvaluated,
            AuditEventType::ExecutionStarted,
            AuditEventType::ExecutionCompleted,
        ]
    );
    assert!(report
        .audit_events
        .windows(2)
        .all(|events| events[0].timestamp <= events[1].timestamp));
}
#[test]
fn mcp_tools_list_exposes_only_available_effective_tools() {
    let fixture = Fixture::new();
    let normal = McpService::new(McpExecutionContext::new(Arc::new(
        fixture.pipeline(PolicyEngine::system_rules()),
    )));
    assert_eq!(
        normal
            .tools()
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>(),
        ["read_file", "glob", "grep", "write", "edit"]
    );

    fixture.ready.store(false, Ordering::Release);
    assert!(normal.tools().is_empty());

    fixture.ready.store(true, Ordering::Release);
    let denied_read_file = McpService::new(McpExecutionContext::new(Arc::new(
        fixture.pipeline_with_registry(
            PolicyEngine::system_rules(),
            ToolRegistry::normal_mode().with_test_scope(0, "outside_active_workspace"),
        ),
    )));
    assert_eq!(
        denied_read_file
            .tools()
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>(),
        ["glob", "grep", "write", "edit"]
    );

    let disabled_read_file = McpService::new(McpExecutionContext::new(Arc::new(
        fixture.pipeline_with_registry(
            PolicyEngine::system_rules(),
            ToolRegistry::normal_mode().with_test_enabled(0, false),
        ),
    )));
    assert_eq!(
        disabled_read_file
            .tools()
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>(),
        ["glob", "grep", "write", "edit"]
    );
}

#[test]
fn m7_file_evidence_records_only_safe_arguments_results_and_terminal_duration() {
    use harness_control_protocol::{ExecutionResultEvidence as R, SanitizedArgumentsEvidence as A};
    let fixture = Fixture::new();
    let pipeline = fixture.pipeline(PolicyEngine::system_rules());
    let read = pipeline.execute(|| Ok(ReadFileRequest::new("sample.txt")));
    assert_eq!(
        read.audit_events.last().unwrap().result_evidence,
        Some(R::ReadFile { bytes_read: 8 })
    );
    assert_eq!(
        read.context.sanitized_arguments,
        Some(A::ReadFile {
            logical_path: "sample.txt".into()
        })
    );
    let glob = pipeline.execute(|| Ok(GlobRequest::new("*.txt")));
    assert_eq!(
        glob.audit_events.last().unwrap().result_evidence,
        Some(R::Glob { match_count: 1 })
    );
    let write = pipeline.execute(|| Ok(WriteRequest::new("new.txt", "WRITE_CONTENT_SENTINEL")));
    let edit = pipeline.execute(|| {
        Ok(EditRequest::new(
            "new.txt",
            "WRITE_CONTENT_SENTINEL",
            "EDIT_NEW_SENTINEL",
        ))
    });
    assert!(matches!(
        write.audit_events.last().unwrap().result_evidence,
        Some(R::Write {
            bytes_written: 22,
            disposition: harness_control_protocol::WriteDisposition::Created
        })
    ));
    assert!(matches!(
        edit.audit_events.last().unwrap().result_evidence,
        Some(R::Edit {
            replacements: 1,
            bytes_written: 17
        })
    ));
    for events in [
        &read.audit_events,
        &glob.audit_events,
        &write.audit_events,
        &edit.audit_events,
    ] {
        assert!(events.last().unwrap().duration_ms.is_some());
        for event in &events[..events.len() - 1] {
            assert!(event.duration_ms.is_none());
            assert!(event.result_evidence.is_none());
        }
        let text = serde_json::to_string(events).unwrap();
        for secret in [
            "WRITE_CONTENT_SENTINEL",
            "EDIT_NEW_SENTINEL",
            fixture.root.to_str().unwrap(),
        ] {
            assert!(!text.contains(secret));
        }
        for event in events.iter() {
            assert_eq!(event.tool_enabled, Some(true));
            assert!(event.capability.is_some());
        }
    }
    let invalid = pipeline.execute::<ReadFileRequest>(|| Err(ExecutionError::InvalidArguments));
    let terminal = invalid.audit_events.last().unwrap();
    assert_eq!(
        terminal.failure_category,
        Some(crate::audit::AuditFailureCategory::InvalidArguments)
    );
    assert!(terminal.duration_ms.is_some());
    assert!(terminal.result_evidence.is_none());
}

#[test]
fn m7_counters_observe_validation_executor_concurrency_and_all_exit_paths() {
    use super::RequestCounters;
    struct CountsHook(Arc<RequestCounters>);
    impl ExecutionHook for CountsHook {
        fn on_event(&self, _: &RequestContext, event: PipelineEvent) {
            let (active, running) = self.0.snapshot();
            if event == PipelineEvent::RequestReceived {
                assert_eq!((active, running), (1, 0));
            }
            if event == PipelineEvent::PolicyDecision {
                assert_eq!((active, running), (1, 0));
            }
            if event == PipelineEvent::ExecutionStarted {
                assert_eq!((active, running), (1, 1));
            }
        }
    }
    for decision in [
        PolicyDecision::Allow,
        PolicyDecision::Deny,
        PolicyDecision::Ask,
    ] {
        let fixture = Fixture::new();
        let counters = Arc::new(RequestCounters::default());
        let pipeline = fixture
            .pipeline(rules(decision))
            .with_counters(counters.clone())
            .with_hook(Arc::new(CountsHook(counters.clone())));
        pipeline.execute(|| {
            assert_eq!(counters.snapshot(), (1, 0));
            Ok(fixture.spy("sample.txt", false))
        });
        assert_eq!(counters.snapshot(), (0, 0));
        pipeline.execute::<ReadFileRequest>(|| Err(ExecutionError::InvalidArguments));
        assert_eq!(counters.snapshot(), (0, 0));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pipeline.execute::<ReadFileRequest>(|| panic!("test unwind"))
        }));
        assert_eq!(counters.snapshot(), (0, 0));
    }
    let fixture = Fixture::new();
    let pipeline = fixture.pipeline(PolicyEngine::system_rules());
    let entered = Arc::new(std::sync::Barrier::new(3));
    let release = Arc::new(std::sync::Barrier::new(3));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let entered = entered.clone();
            let release = release.clone();
            let pipeline = &pipeline;
            let fixture = &fixture;
            scope.spawn(move || {
                pipeline.execute(|| {
                    entered.wait();
                    release.wait();
                    Ok(fixture.spy("sample.txt", false))
                })
            });
        }
        entered.wait();
        assert_eq!(pipeline.request_counts(), (2, 0));
        release.wait();
    });
    assert_eq!(pipeline.request_counts(), (0, 0));
    pipeline.execute(|| Ok(fixture.spy("sample.txt", true)));
    assert_eq!(pipeline.request_counts(), (0, 0));
    pipeline.execute(|| Ok(ReadFileRequest::new("../outside-secret.txt")));
    assert_eq!(pipeline.request_counts(), (0, 0));
}

#[tokio::test]
async fn m7_approval_lifecycle_releases_counters_and_preserves_correlation() {
    struct Clock;
    impl crate::approval::ApprovalClock for Clock {
        fn now(&self) -> chrono::DateTime<chrono::Utc> {
            chrono::Utc::now()
        }
    }
    for action in ["allow", "deny", "expire", "cancel"] {
        let fixture = Fixture::new();
        let manager = crate::approval::ApprovalManager::with_test_clock(
            "r",
            "g",
            std::time::Duration::from_millis(25),
            Arc::new(Clock),
        );
        let pipeline = fixture
            .pipeline(rules(PolicyDecision::Ask))
            .with_approval_state(
                manager.clone(),
                Arc::new(crate::session_grant::SessionGrantStore::new("r")),
            );
        let request =
            pipeline.execute_async(|| Ok(WriteRequest::new("ask.txt", "CONTENT_SENTINEL")));
        let control = async {
            loop {
                if manager.pending_count() == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert_eq!(pipeline.request_counts(), (1, 0));
            let approval = manager.list(None).pop().unwrap();
            match action {
                "allow" => {
                    manager
                        .resolve(
                            &approval.request_id,
                            harness_control_protocol::ApprovalDecision::AllowOnce,
                        )
                        .unwrap();
                }
                "deny" => {
                    manager
                        .resolve(
                            &approval.request_id,
                            harness_control_protocol::ApprovalDecision::Deny,
                        )
                        .unwrap();
                }
                "cancel" => manager.cancel(&approval.request_id),
                _ => {}
            }
        };
        let (report, _) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            tokio::join!(request, control)
        })
        .await
        .unwrap();
        assert_eq!(pipeline.request_counts(), (0, 0));
        assert_eq!(manager.pending_count(), 0);
        let terminal = report.audit_events.last().unwrap();
        assert_eq!(
            terminal.approval_id,
            Some(report.context.request_id.to_string())
        );
        assert!(terminal.duration_ms.is_some());
        assert!(!serde_json::to_string(&report.audit_events)
            .unwrap()
            .contains("CONTENT_SENTINEL"));
        assert_eq!(report.result.is_ok(), action == "allow");
        if action == "expire" {
            assert_eq!(
                terminal.failure_category,
                Some(AuditFailureCategory::ApprovalExpired)
            );
        }
        if action == "cancel" {
            assert_eq!(
                terminal.failure_category,
                Some(AuditFailureCategory::ApprovalCancelled)
            );
        }
    }
}

#[cfg(windows)]
#[tokio::test]
async fn m7_bash_timeout_and_output_limit_release_running_counter() {
    for (command, expected) in [
        ("sleep 1", crate::audit::AuditFailureCategory::BashTimeout),
        (
            "printf 123456789",
            crate::audit::AuditFailureCategory::BashOutputLimit,
        ),
    ] {
        let fixture = Fixture::new();
        let executable =
            crate::bash::resolve_bash(&harness_control_protocol::BashConfig::default());
        let path = executable
            .canonical_path()
            .expect("validated Git Bash")
            .to_path_buf();
        let mut config = harness_control_protocol::ToolConfig::default();
        config.bash.enabled = true;
        let registry = Arc::new(ToolRegistry::from_config_with_bash(&config, &executable));
        let mut policies = harness_control_protocol::PolicyConfig::default();
        policies.tool_defaults.bash = PolicyDecision::Allow;
        let pipeline = ExecutionPipeline::new(
            fixture.guard.clone(),
            registry.clone(),
            PolicyEngine::compile(&policies, &registry),
            SecurityContext {
                workspace_id: fixture.workspace_id,
                mode: SecurityMode::DeveloperTest,
            },
            64,
            fixture.ready.clone(),
        )
        .with_bash_runtime(Arc::new(crate::bash::BashRuntime::with_test_limits(
            Some(path),
            fixture.root.clone(),
            crate::bash::BashLimits::for_test(
                if expected == crate::audit::AuditFailureCategory::BashTimeout {
                    std::time::Duration::from_millis(250)
                } else {
                    std::time::Duration::from_secs(3)
                },
                4,
            ),
        )));
        let report = pipeline
            .execute_async(|| Ok(BashToolRequest::new(command.into())))
            .await;
        assert!(report.result.is_err());
        assert_eq!(pipeline.request_counts(), (0, 0));
        let terminal = report.audit_events.last().unwrap();
        assert_eq!(terminal.failure_category, Some(expected));
        assert!(terminal.duration_ms.is_some());
    }
}

#[test]
fn m7_allow_deny_disabled_workspace_and_failure_evidence_matrix() {
    for row in [
        "allow",
        "deny",
        "disabled",
        "workspace",
        "failure",
        "validation",
    ] {
        let fixture = Fixture::new();
        let registry = if row == "disabled" {
            ToolRegistry::normal_mode().with_test_enabled(0, false)
        } else {
            ToolRegistry::normal_mode()
        };
        let pipeline = fixture.pipeline_with_registry(
            rules(if row == "deny" {
                PolicyDecision::Deny
            } else {
                PolicyDecision::Allow
            }),
            registry,
        );
        let report = if row == "validation" {
            pipeline.execute::<ReadFileRequest>(|| Err(ExecutionError::InvalidArguments))
        } else {
            pipeline.execute(|| {
                Ok(ReadFileRequest::new(if row == "workspace" {
                    "../outside-secret.txt"
                } else if row == "failure" {
                    "."
                } else {
                    "sample.txt"
                }))
            })
        };
        assert_eq!(pipeline.request_counts(), (0, 0), "{row}");
        let terminal = report.audit_events.last().unwrap();
        assert!(terminal.duration_ms.is_some(), "{row}");
        assert!(report
            .audit_events
            .iter()
            .all(|event| event.request_id == report.context.request_id));
        if row != "allow" {
            assert!(report.result.is_err());
            assert!(terminal.result_evidence.is_none());
        }
        if ["deny", "disabled", "workspace", "validation"].contains(&row) {
            assert!(!report
                .audit_events
                .iter()
                .any(|e| e.event_type == AuditEventType::ExecutionStarted));
        }
        assert_eq!(terminal.tool_enabled, Some(row != "disabled"));
        let text = serde_json::to_string(&report.audit_events).unwrap();
        assert!(!text.contains(fixture.root.to_str().unwrap()));
        assert!(!text.contains("do not read"));
    }
}

#[cfg(windows)]
#[tokio::test]
async fn m7_bash_environment_and_output_sentinels_never_enter_evidence() {
    let fixture = Fixture::new();
    let key = format!("M7_EVIDENCE_TEST_{}", uuid::Uuid::new_v4().simple());
    struct EnvGuard(String);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(&self.0);
        }
    }
    std::env::set_var(&key, "BASH_ENV_SECRET_SENTINEL");
    let _environment = EnvGuard(key.clone());
    fs::write(
        fixture.root.join("stdout.txt"),
        "BASH_OUTPUT_CONTENT_SENTINEL",
    )
    .unwrap();
    let executable = crate::bash::resolve_bash(&harness_control_protocol::BashConfig::default());
    let mut tools = harness_control_protocol::ToolConfig::default();
    tools.bash.enabled = true;
    let registry = Arc::new(ToolRegistry::from_config_with_bash(&tools, &executable));
    let mut policies = harness_control_protocol::PolicyConfig::default();
    policies.tool_defaults.bash = PolicyDecision::Allow;
    let manager = crate::approval::ApprovalManager::new("test-runtime");
    let store = Arc::new(crate::activity::ActivityStore::new(registry.clone()));
    let pipeline = ExecutionPipeline::new(
        fixture.guard.clone(),
        registry.clone(),
        PolicyEngine::compile(&policies, &registry),
        SecurityContext {
            workspace_id: fixture.workspace_id,
            mode: SecurityMode::DeveloperTest,
        },
        64,
        fixture.ready.clone(),
    )
    .with_approval_state(
        manager.clone(),
        Arc::new(crate::session_grant::SessionGrantStore::new("test-runtime")),
    )
    .with_audit_sink(store.clone())
    .with_bash_runtime(Arc::new(crate::bash::BashRuntime::new(
        executable.canonical_path().map(|p| p.to_path_buf()),
        fixture.root.clone(),
    )));
    let command = format!("printf \"${key}\"; cat stdout.txt");
    let request = pipeline.execute_async(|| Ok(BashToolRequest::new(command)));
    let control = async {
        loop {
            if manager.pending_count() == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let approval = manager.list(None).pop().unwrap();
        manager
            .resolve(
                &approval.request_id,
                harness_control_protocol::ApprovalDecision::AllowOnce,
            )
            .unwrap();
    };
    let (report, _) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(request, control)
    })
    .await
    .unwrap();
    let output = report.result.unwrap();
    assert!(output.stdout.contains("BASH_ENV_SECRET_SENTINEL"));
    assert!(output.stdout.contains("BASH_OUTPUT_CONTENT_SENTINEL"));
    for encoded in [
        serde_json::to_string(&report.audit_events).unwrap(),
        serde_json::to_string(&store.snapshot(256)).unwrap(),
    ] {
        for sentinel in ["BASH_ENV_SECRET_SENTINEL", "BASH_OUTPUT_CONTENT_SENTINEL"] {
            assert!(!encoded.contains(sentinel));
        }
    }
    assert_eq!(pipeline.request_counts(), (0, 0));
    assert_eq!(manager.pending_count(), 0);
}
