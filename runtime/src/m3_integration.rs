use crate::audit::{AuditEventType, AuditFailureCategory, AuditOutcome};
use crate::policy::{PolicyContext, PolicyEngine, PolicyEvaluationReason};
use crate::state::RuntimeCore;
use crate::tools::{EditRequest, GlobRequest, ReadFileRequest, ToolRegistry, WriteRequest};
use harness_control_protocol::{
    DecimalU64, EffectivePolicyTool, PolicyConfig, PolicyDecision, PolicyMatcherConfig,
    PolicyReason, PolicySnapshotStatus, PolicyToolAvailability, RuntimeConfigSnapshot,
    RuntimeLimits, SecurityConfig, SecurityMode, ToolConfig, UserPolicyRuleConfig, WorkspaceConfig,
};
use std::fs;
use std::path::PathBuf;

type QaResult<T> = Result<T, String>;

macro_rules! check {
    ($condition:expr $(,)?) => {
        if !$condition {
            return Err("integration assertion failed".into());
        }
    };
    ($condition:expr, $($argument:tt)*) => {
        if !$condition {
            return Err(format!($($argument)*));
        }
    };
}

struct Fixture {
    base: PathBuf,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> QaResult<Self> {
        let base = std::env::temp_dir().join(format!(
            "chatgpt-harness-runtime-m3-integration-{}",
            uuid::Uuid::new_v4()
        ));
        let workspace = base.join("workspace");
        let outside = base.join("outside");
        fs::create_dir_all(&workspace)
            .map_err(|_| "create runtime integration workspace failed")?;
        fs::create_dir_all(&outside).map_err(|_| "create runtime integration outside failed")?;
        fs::write(workspace.join("read.txt"), b"integration read")
            .map_err(|_| "create runtime read fixture failed")?;
        fs::write(workspace.join("edit.txt"), b"old edit")
            .map_err(|_| "create runtime edit fixture failed")?;
        fs::write(outside.join("outside.txt"), b"outside")
            .map_err(|_| "create runtime outside fixture failed")?;
        Ok(Self {
            base,
            workspace,
            outside,
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn base_snapshot(fixture: &Fixture) -> RuntimeConfigSnapshot {
    RuntimeConfigSnapshot {
        config_revision: DecimalU64::new(1),
        workspace: WorkspaceConfig {
            root_path: fixture.workspace.to_string_lossy().into_owned(),
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
        policies: PolicyConfig::default(),
    }
}

fn user_rule(
    id: &str,
    decision: PolicyDecision,
    tool: &str,
    enabled: bool,
) -> UserPolicyRuleConfig {
    UserPolicyRuleConfig {
        id: id.into(),
        name: format!("Runtime integration {id}"),
        enabled,
        decision,
        matcher: PolicyMatcherConfig::tool(tool),
        reason: "Runtime integration fixture.".into(),
    }
}

fn running_core(config: &RuntimeConfigSnapshot) -> QaResult<RuntimeCore> {
    let mut core = RuntimeCore::new();
    core.initialize(config.clone())
        .map_err(|_| "RuntimeCore initialization failed".to_owned())?;
    core.acknowledge()
        .ok_or_else(|| "RuntimeCore acknowledgement failed".to_owned())?;
    Ok(core)
}

fn effective_tool(core: &RuntimeCore, name: &str) -> QaResult<EffectivePolicyTool> {
    core.effective_policy()
        .per_tool
        .into_iter()
        .find(|tool| tool.tool == name)
        .ok_or_else(|| "effective policy omitted expected tool".into())
}

fn assert_effective(
    core: &RuntimeCore,
    tool: &str,
    decision: PolicyDecision,
    reason: PolicyReason,
    ids: &[&str],
) -> QaResult<()> {
    let actual = effective_tool(core, tool)?;
    check!(
        actual.decision == decision,
        "effective decision mismatch for {tool}"
    );
    check!(
        actual.reason == reason,
        "effective reason mismatch for {tool}"
    );
    let actual_ids = actual
        .matched_rule_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    check!(
        actual_ids == ids,
        "effective matched IDs mismatch for {tool}"
    );
    Ok(())
}

fn report_case(id: &str, result: QaResult<()>, failures: &mut Vec<String>) {
    match result {
        Ok(()) => println!("{id} PASS"),
        Err(error) => {
            println!("{id} FAIL: {error}");
            failures.push(id.into());
        }
    }
}

#[test]
fn m3_runtime_integration_cases() {
    let fixture = Fixture::new().expect("create M3 runtime integration fixture");
    let mut failures = Vec::new();
    report_case("INT-06", int_06(&fixture), &mut failures);
    report_case("INT-07", int_07(&fixture), &mut failures);
    report_case("INT-08", int_08(&fixture), &mut failures);
    report_case("INT-09", int_09(&fixture), &mut failures);
    report_case("INT-10", int_10(&fixture), &mut failures);
    report_case("INT-11", int_11(&fixture), &mut failures);
    report_case("INT-12", int_12(&fixture), &mut failures);
    report_case("INT-13", int_13(&fixture), &mut failures);
    report_case("INT-14", int_14(&fixture), &mut failures);
    report_case("INT-15", int_15(&fixture), &mut failures);
    report_case("INT-16", int_16(&fixture), &mut failures);
    report_case("INT-17", int_17(&fixture), &mut failures);
    report_case("INT-18", int_18(&fixture), &mut failures);
    report_case("INT-19", int_19(&fixture), &mut failures);
    report_case("INT-20", int_20(&fixture), &mut failures);
    report_case("INT-25", int_25(&fixture), &mut failures);
    assert!(
        failures.is_empty(),
        "failed M3 runtime Integration cases: {failures:?}"
    );
}

fn int_06(fixture: &Fixture) -> QaResult<()> {
    let first = base_snapshot(fixture);
    let mut desired = first.clone();
    desired.config_revision = DecimalU64::new(2);
    desired.policies.tool_defaults.write = PolicyDecision::Deny;
    let generation_a = running_core(&first)?;
    check!(generation_a.status().unwrap().config_revision == first.config_revision);
    check!(generation_a
        .write(&WriteRequest::new("int-06-a.txt", "A"))
        .is_ok());
    check!(effective_tool(&generation_a, "write")?.decision == PolicyDecision::Allow);
    let generation_b = running_core(&desired)?;
    check!(generation_b.status().unwrap().config_revision == desired.config_revision);
    check!(generation_b
        .write(&WriteRequest::new("int-06-b.txt", "B"))
        .is_err());
    Ok(())
}

fn int_07(fixture: &Fixture) -> QaResult<()> {
    for decision in [
        PolicyDecision::Allow,
        PolicyDecision::Ask,
        PolicyDecision::Deny,
    ] {
        let mut config = base_snapshot(fixture);
        config.policies.tool_defaults.write = decision.clone();
        let core = running_core(&config)?;
        assert_effective(&core, "write", decision, PolicyReason::ToolDefault, &[])?;
    }
    Ok(())
}

fn int_08(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    config.policies.user_rules.push(user_rule(
        "ask-default-allow",
        PolicyDecision::Allow,
        "write",
        true,
    ));
    assert_effective(
        &running_core(&config)?,
        "write",
        PolicyDecision::Allow,
        PolicyReason::MatchedUserRules,
        &["ask-default-allow"],
    )
}

fn int_09(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.policies.tool_defaults.write = PolicyDecision::Deny;
    config.policies.user_rules.push(user_rule(
        "deny-default-allow",
        PolicyDecision::Allow,
        "write",
        true,
    ));
    assert_effective(
        &running_core(&config)?,
        "write",
        PolicyDecision::Allow,
        PolicyReason::MatchedUserRules,
        &["deny-default-allow"],
    )
}

fn int_10(fixture: &Fixture) -> QaResult<()> {
    for (rules, expected) in [
        (
            vec![user_rule("allow", PolicyDecision::Allow, "write", true)],
            PolicyDecision::Allow,
        ),
        (
            vec![
                user_rule("allow", PolicyDecision::Allow, "write", true),
                user_rule("ask", PolicyDecision::Ask, "write", true),
            ],
            PolicyDecision::Ask,
        ),
        (
            vec![
                user_rule("z-allow", PolicyDecision::Allow, "write", true),
                user_rule("a-ask", PolicyDecision::Ask, "write", true),
                user_rule("m-deny", PolicyDecision::Deny, "write", true),
            ],
            PolicyDecision::Deny,
        ),
    ] {
        let mut config = base_snapshot(fixture);
        config.policies.user_rules = rules;
        let actual = effective_tool(&running_core(&config)?, "write")?;
        check!(actual.decision == expected);
        check!(actual.reason == PolicyReason::MatchedUserRules);
    }
    Ok(())
}

fn int_11(fixture: &Fixture) -> QaResult<()> {
    let mut first = base_snapshot(fixture);
    first.policies.user_rules = vec![
        user_rule("z-allow", PolicyDecision::Allow, "write", true),
        user_rule("a-ask", PolicyDecision::Ask, "write", true),
        user_rule("m-deny", PolicyDecision::Deny, "write", true),
    ];
    let mut second = first.clone();
    second.policies.user_rules.reverse();
    let a = effective_tool(&running_core(&first)?, "write")?;
    let b = effective_tool(&running_core(&second)?, "write")?;
    check!(a == b);
    check!(a.matched_rule_ids == ["a-ask", "m-deny", "z-allow"]);
    Ok(())
}

fn int_12(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.policies.user_rules.push(user_rule(
        "outside-allow",
        PolicyDecision::Allow,
        "write",
        true,
    ));
    let core = running_core(&config)?;
    let target = fixture.outside.join("int-12-must-not-exist.txt");
    check!(core.write(&WriteRequest::new(&target, "outside")).is_err());
    check!(!target.exists());
    let entries = core.activity_snapshot(256);
    check!(entries
        .iter()
        .any(|entry| entry.failure_category == Some(AuditFailureCategory::WorkspaceViolation)));
    let registry = ToolRegistry::normal_mode();
    let engine = PolicyEngine::compile(&PolicyConfig::default(), &registry);
    let unsupported = engine.evaluate(
        &PolicyContext::new("bash", SecurityMode::Normal, "active_workspace"),
        &registry,
    );
    check!(unsupported.decision == PolicyDecision::Deny);
    check!(unsupported.reason == PolicyEvaluationReason::UnsupportedCapability);
    Ok(())
}

fn int_13(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.policies.user_rules = vec![
        user_rule("duplicate", PolicyDecision::Allow, "write", true),
        user_rule("duplicate", PolicyDecision::Deny, "write", true),
    ];
    let core = running_core(&config)?;
    check!(!core.effective_policy().valid);
    check!(effective_tool(&core, "write")?.reason == PolicyReason::InvalidPolicySnapshot);
    let target = fixture.workspace.join("int-13.txt");
    check!(core.write(&WriteRequest::new(&target, "blocked")).is_err());
    check!(!target.exists());
    Ok(())
}

fn int_14(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.policies.user_rules = vec![
        user_rule("valid", PolicyDecision::Allow, "write", true),
        user_rule("invalid", PolicyDecision::Allow, "unknown_tool", true),
    ];
    let core = running_core(&config)?;
    check!(!core.effective_policy().valid);
    check!(effective_tool(&core, "write")?.decision == PolicyDecision::Deny);
    check!(effective_tool(&core, "write")?.reason == PolicyReason::InvalidPolicySnapshot);
    Ok(())
}

fn int_15(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.policies.user_rules = vec![
        user_rule("valid", PolicyDecision::Allow, "write", true),
        user_rule(
            "disabled-invalid",
            PolicyDecision::Allow,
            "unknown_tool",
            false,
        ),
    ];
    let core = running_core(&config)?;
    check!(core.effective_policy().valid);
    check!(core
        .write(&WriteRequest::new("int-15.txt", "allowed"))
        .is_ok());
    config.policies.user_rules[1].enabled = true;
    let enabled = running_core(&config)?;
    check!(!enabled.effective_policy().valid);
    check!(enabled
        .write(&WriteRequest::new("int-15-enabled.txt", "blocked"))
        .is_err());
    Ok(())
}

fn int_16(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config
        .policies
        .user_rules
        .push(user_rule("deny-write", PolicyDecision::Deny, "write", true));
    let core = running_core(&config)?;
    let target = fixture.workspace.join("int-16.txt");
    check!(core.write(&WriteRequest::new(&target, "blocked")).is_err());
    check!(!target.exists());
    let entries = core.activity_snapshot(256);
    check!(entries.iter().any(|entry| {
        entry.event_type == AuditEventType::PolicyEvaluated
            && entry.decision_summary == Some(PolicyDecision::Deny)
            && entry.matched_rule_ids == ["deny-write"]
            && entry.policy_reason == Some(PolicyReason::MatchedUserRules)
    }));
    check!(!entries
        .iter()
        .any(|entry| entry.event_type == AuditEventType::ExecutionCompleted));
    Ok(())
}

fn int_17(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config
        .policies
        .user_rules
        .push(user_rule("ask-write", PolicyDecision::Ask, "write", true));
    let core = running_core(&config)?;
    let target = fixture.workspace.join("int-17.txt");
    check!(core
        .write(&WriteRequest::new(&target, "must not write"))
        .is_err());
    check!(!target.exists());
    let entries = core.activity_snapshot(256);
    check!(entries.iter().any(|entry| {
        entry.event_type == AuditEventType::ApprovalRequired
            && entry.decision_summary == Some(PolicyDecision::Ask)
            && entry.outcome_summary == AuditOutcome::ApprovalRequired
            && entry.matched_rule_ids == ["ask-write"]
    }));
    Ok(())
}

fn int_18(fixture: &Fixture) -> QaResult<()> {
    let core = running_core(&base_snapshot(fixture))?;
    check!(core.read_file(&ReadFileRequest::new("read.txt")).is_ok());
    check!(core.glob(&GlobRequest::new("**/*.txt")).is_ok());
    check!(core.write(&WriteRequest::new("int-18.txt", "old")).is_ok());
    check!(core
        .edit(&EditRequest::new("int-18.txt", "old", "new"))
        .is_ok());
    check!(fs::read(fixture.workspace.join("int-18.txt")).unwrap() == b"new");
    Ok(())
}

fn int_19(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.tools.write.enabled = false;
    config.policies.user_rules.push(user_rule(
        "disabled-write-allow",
        PolicyDecision::Allow,
        "write",
        true,
    ));
    let core = running_core(&config)?;
    let target = fixture.workspace.join("int-19.txt");
    check!(core.write(&WriteRequest::new(&target, "blocked")).is_err());
    check!(!target.exists());
    let entries = core.activity_snapshot(256);
    check!(entries
        .iter()
        .any(|entry| entry.failure_category == Some(AuditFailureCategory::ToolDisabled)));
    check!(effective_tool(&core, "write")?.availability == PolicyToolAvailability::Disabled);
    Ok(())
}

fn int_20(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_snapshot(fixture);
    config.policies.user_rules.push(user_rule(
        "projection-allow",
        PolicyDecision::Allow,
        "write",
        true,
    ));
    let core = running_core(&config)?;
    let snapshot = core.effective_policy();
    check!(snapshot.status == PolicySnapshotStatus::Valid);
    check!(snapshot.system_rules.iter().all(|rule| rule.source
        == harness_control_protocol::PolicySource::System
        && !rule.editable));
    check!(snapshot.user_rules.iter().any(|rule| {
        rule.id == "projection-allow"
            && rule.source == harness_control_protocol::PolicySource::User
            && rule.editable
    }));
    assert_effective(
        &core,
        "write",
        PolicyDecision::Allow,
        PolicyReason::MatchedUserRules,
        &["projection-allow"],
    )
}

fn int_25(fixture: &Fixture) -> QaResult<()> {
    let sentinel = "M3_SECRET_CONTENT_SENTINEL";
    let core = running_core(&base_snapshot(fixture))?;
    check!(core.read_file(&ReadFileRequest::new("read.txt")).is_ok());
    check!(core.glob(&GlobRequest::new("**/*.txt")).is_ok());
    check!(core
        .write(&WriteRequest::new("int-25.txt", sentinel))
        .is_ok());
    check!(core
        .edit(&EditRequest::new("int-25.txt", sentinel, "redacted"))
        .is_ok());
    let outside = fixture.outside.join("int-25.txt");
    check!(core.write(&WriteRequest::new(&outside, "outside")).is_err());
    let mut disabled = base_snapshot(fixture);
    disabled.tools.write.enabled = false;
    let disabled_core = running_core(&disabled)?;
    check!(disabled_core
        .write(&WriteRequest::new("int-25-disabled.txt", "blocked"))
        .is_err());
    let activity = serde_json::to_string(&core.activity_snapshot(256)).unwrap();
    check!(!activity.contains(sentinel));
    let workspace_path = fixture.workspace.to_string_lossy();
    check!(!activity.contains(workspace_path.as_ref()));
    check!(!activity.contains("int-25.txt"));
    Ok(())
}
