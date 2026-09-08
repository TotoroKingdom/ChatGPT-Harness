use super::state::RequestLifecycle;
use super::*;
use crate::audit::{AuditEventType, AuditLifecycleState, AuditOutcome, WorkspaceValidation};
use crate::mcp::{McpExecutionContext, McpService};
use crate::policy::{PolicyEngine, PolicyEvaluationReason};
use crate::tools::{GlobRequest, ReadFileError, ToolRegistry};
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
impl ToolExecutor for Spy {
    type Output = ();
    const TOOL_NAME: &'static str = "read_file";
    fn validate(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
    fn workspace_target(&self) -> &Path {
        &self.path
    }
    fn workspace_error(error: WorkspaceError) -> ExecutionError {
        ExecutionError::ReadFile(ReadFileError::Workspace(error))
    }
    fn execute(
        &self,
        _permit: &ExecutionPermit,
        _guard: &WorkspaceGuard,
        _limit: u32,
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
        let report = fixture
            .pipeline(rules(decision.clone()))
            .execute(|| Ok(fixture.spy("sample.txt", false)));
        let audit_events = report.audit_events;

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
        ["read_file", "glob"]
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
        ["glob"]
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
        ["glob"]
    );
}
