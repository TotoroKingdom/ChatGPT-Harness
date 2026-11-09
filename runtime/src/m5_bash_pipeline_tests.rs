//! Code-level Unit verification through in-process RuntimeCore/MCP methods.
//! No Runtime subprocess, Control RPC, HTTP transport, or external QA client.
use crate::approval::ApprovalManagerError;
use crate::mcp::McpService;
use crate::state::RuntimeCore;
use harness_control_protocol::{
    ApprovalDecision, ApprovalRequest, ApprovalStatus, BashConfig, BashDestructiveReason,
    DecimalU64, PolicyDecision, PolicyMatcherConfig, RuntimeConfigSnapshot, RuntimeLimits,
    SecurityConfig, SecurityMode, ToolConfig, UserPolicyRuleConfig, WorkspaceConfig,
};
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

struct Fixture {
    root: PathBuf,
    core: RuntimeCore,
}
impl Fixture {
    fn new(change: impl FnOnce(&mut RuntimeConfigSnapshot)) -> Self {
        let root =
            std::env::temp_dir().join(format!("m5-bash-pipeline-unit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut tools = ToolConfig::default();
        tools.bash.enabled = true;
        let mut config = RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(1),
            workspace: WorkspaceConfig {
                root_path: root.to_string_lossy().into_owned(),
            },
            security: SecurityConfig {
                mode: SecurityMode::DeveloperTest,
                developer_test_mode: false,
            },
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 4,
                max_activity_events: 256,
                max_read_bytes: 4096,
            },
            tools,
            bash: BashConfig::default(),
            policies: Default::default(),
            search: Default::default(),
        };
        change(&mut config);
        let mut core = RuntimeCore::new();
        core.initialize(config).unwrap();
        core.acknowledge().unwrap();
        Self { root, core }
    }
    fn service(&self) -> McpService {
        self.core.mcp_service().unwrap()
    }
    fn resolve(
        &self,
        request: &ApprovalRequest,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRequest, ApprovalManagerError> {
        let grants = self.core.session_grants();
        self.core.approval_manager().resolve_with(
            &request.request_id,
            decision,
            |approval, scope| {
                if decision == ApprovalDecision::AllowForSession {
                    grants.insert_from_approved_scope(
                        &approval.generation_id,
                        &approval.request_id,
                        &approval.tool,
                        scope.expect("eligible private grant scope"),
                        chrono::Utc::now(),
                    );
                }
            },
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.core.stop();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn call(service: McpService, command: &str) -> tokio::task::JoinHandle<CallToolResult> {
    let command = command.to_owned();
    tokio::spawn(async move {
        service
            .call_tool_async("bash", json!({"command": command}))
            .await
            .unwrap()
    })
}
async fn pending(fixture: &Fixture) -> ApprovalRequest {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let entries = fixture
                .core
                .approval_manager()
                .list(Some(ApprovalStatus::Pending));
            if let Some(entry) = entries.into_iter().next() {
                return entry;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Bash did not enter same-request Pending")
}
async fn finish(task: tokio::task::JoinHandle<CallToolResult>) -> CallToolResult {
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("Bash request failed to complete")
        .unwrap()
}
fn code(result: &CallToolResult) -> &str {
    result.structured_content.as_ref().unwrap()["code"]
        .as_str()
        .unwrap()
}
fn output(result: &CallToolResult) -> &Value {
    assert_eq!(result.is_error, Some(false));
    result.structured_content.as_ref().unwrap()
}

#[tokio::test]
async fn normal_mode_ignores_legacy_developer_flag_and_never_spawns() {
    let fixture = Fixture::new(|cfg| {
        cfg.security.mode = SecurityMode::Normal;
        cfg.security.developer_test_mode = true;
    });
    assert!(!fixture
        .service()
        .tools()
        .iter()
        .any(|tool| tool.name == "bash"));
    let result = finish(call(fixture.service(), "printf wrong > forbidden.txt")).await;
    assert_eq!(code(&result), "BASH_UNAVAILABLE");
    assert!(!fixture.root.join("forbidden.txt").exists());
    assert_eq!(fixture.core.bash_runtime().unwrap().active_count(), 0);
    assert_eq!(fixture.core.approval_manager().pending_count(), 0);
}

#[tokio::test]
async fn disabled_and_unready_bash_are_distinct_and_file_tools_stay_available() {
    for disabled in [true, false] {
        let fixture = Fixture::new(|cfg| {
            if disabled {
                cfg.tools.bash.enabled = false;
            } else {
                cfg.bash.executable_path = Some("C:\\not-existing-m5\\bash.exe".into());
            }
        });
        let service = fixture.service();
        assert!(!service.tools().iter().any(|tool| tool.name == "bash"));
        assert!(service.tools().iter().any(|tool| tool.name == "read_file"));
        let result = finish(call(service, "printf wrong > forbidden.txt")).await;
        assert_eq!(
            code(&result),
            if disabled {
                "TOOL_DISABLED"
            } else {
                "BASH_UNAVAILABLE"
            }
        );
        assert!(!fixture.root.join("forbidden.txt").exists());
    }
}

#[tokio::test]
async fn default_ask_resumes_exact_original_call_once_and_records_safe_correlation() {
    let fixture = Fixture::new(|_| {});
    assert!(
        fixture.core.bash_executable().is_ready(),
        "validated Git Bash required"
    );
    let task = call(fixture.service(), "printf approved > once.txt");
    let approval = pending(&fixture).await;
    assert!(!task.is_finished());
    assert!(!fixture.root.join("once.txt").exists());
    assert_eq!(fixture.core.bash_runtime().unwrap().active_count(), 0);
    fixture
        .resolve(&approval, ApprovalDecision::AllowOnce)
        .unwrap();
    let result = finish(task).await;
    assert_eq!(output(&result)["exitCode"], 0);
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("once.txt")).unwrap(),
        "approved"
    );
    assert_eq!(fixture.core.session_grants().len(), 0);
    let entries = fixture.core.activity_snapshot(256);
    let completion = entries
        .iter()
        .find(|entry| {
            entry.request_id.to_string() == approval.request_id
                && entry
                    .bash
                    .as_ref()
                    .is_some_and(|bash| bash.exit_code == Some(0))
        })
        .unwrap();
    let bash = completion.bash.as_ref().unwrap();
    assert_eq!(
        bash.approval_request_id.as_deref(),
        Some(approval.request_id.as_str())
    );
    assert!(bash.duration_ms.is_some());
    assert_eq!(
        completion.approval_id.as_deref(),
        Some(approval.request_id.as_str())
    );
    assert!(completion.duration_ms.is_some());
    assert!(matches!(
        completion.result_evidence,
        Some(harness_control_protocol::ExecutionResultEvidence::Bash { exit_code: 0 })
    ));
    let serialized = serde_json::to_string(&entries).unwrap();
    assert!(!serialized.contains("stdout"));
    assert!(!serialized.contains("stderr"));
    assert!(!serialized.contains("grantScope"));
}

#[tokio::test]
async fn deny_and_explicit_policy_deny_prevent_shell_spawn() {
    let fixture = Fixture::new(|_| {});
    let task = call(fixture.service(), "printf denied > denied.txt");
    let approval = pending(&fixture).await;
    fixture.resolve(&approval, ApprovalDecision::Deny).unwrap();
    assert_eq!(code(&finish(task).await), "APPROVAL_DENIED");
    assert!(!fixture.root.join("denied.txt").exists());
    let fixture = Fixture::new(|cfg| {
        cfg.policies.user_rules.push(UserPolicyRuleConfig::new(
            "deny-bash",
            "Deny Bash",
            PolicyDecision::Deny,
            "bash",
            "No shell execution.",
        ));
    });
    let result = finish(call(fixture.service(), "printf denied > denied.txt")).await;
    assert_eq!(code(&result), "POLICY_DENIED");
    assert!(!fixture.root.join("denied.txt").exists());
    assert_eq!(fixture.core.approval_manager().pending_count(), 0);
}

#[tokio::test]
async fn classified_session_grant_matches_full_normalized_shape_and_not_wider_prefix() {
    let fixture = Fixture::new(|_| {});
    let first = call(fixture.service(), "printf granted");
    let approval = pending(&fixture).await;
    assert_eq!(approval.allow_for_session_eligible, Some(true));
    fixture
        .resolve(&approval, ApprovalDecision::AllowForSession)
        .unwrap();
    assert_eq!(output(&finish(first).await)["stdout"], "granted");
    let next = finish(call(fixture.service(), "printf 'granted'")).await;
    assert_eq!(output(&next)["stdout"], "granted");
    assert_eq!(fixture.core.approval_manager().pending_count(), 0);
    assert!(fixture.core.activity_snapshot(256).iter().any(|entry| entry
        .bash
        .as_ref()
        .is_some_and(|bash| bash.session_grant_hit)));
    let wider = call(fixture.service(), "printf granted extra");
    let request = pending(&fixture).await;
    assert!(!wider.is_finished());
    fixture.resolve(&request, ApprovalDecision::Deny).unwrap();
    assert_eq!(code(&finish(wider).await), "APPROVAL_DENIED");
}

#[tokio::test]
async fn destructive_redirect_requires_approval_and_exact_session_grant() {
    let fixture = Fixture::new(|cfg| {
        cfg.policies.tool_defaults.bash = PolicyDecision::Allow;
    });
    let first = call(fixture.service(), "printf granted > granted.txt");
    let approval = pending(&fixture).await;
    assert_eq!(approval.allow_for_session_eligible, Some(true));
    let details = approval.bash_details.as_ref().unwrap();
    assert!(details.destructive);
    assert!(details
        .destructive_reasons
        .contains(&BashDestructiveReason::Overwrite));
    fixture
        .resolve(&approval, ApprovalDecision::AllowForSession)
        .unwrap();
    let _ = finish(first).await;
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("granted.txt")).unwrap(),
        "granted"
    );

    let _ = finish(call(fixture.service(), "printf granted > granted.txt")).await;
    let different = call(fixture.service(), "printf granted > other.txt");
    let request = pending(&fixture).await;
    assert!(!different.is_finished());
    fixture.resolve(&request, ApprovalDecision::Deny).unwrap();
    assert_eq!(code(&finish(different).await), "APPROVAL_DENIED");
    assert!(!fixture.root.join("other.txt").exists());
}

#[tokio::test]
async fn broad_allow_does_not_bypass_unclassified_floor_or_create_a_grant() {
    let fixture = Fixture::new(|cfg| {
        cfg.policies.user_rules.push(UserPolicyRuleConfig::new(
            "broad-bash",
            "Broad Bash",
            PolicyDecision::Allow,
            "bash",
            "Reviewed eligibility.",
        ));
    });
    let task = call(fixture.service(), "printf '%s' \"$((1+1))\"");
    let approval = pending(&fixture).await;
    assert_eq!(approval.allow_for_session_eligible, Some(false));
    assert!(matches!(
        fixture.resolve(&approval, ApprovalDecision::AllowForSession),
        Err(ApprovalManagerError::SessionGrantNotEligible)
    ));
    assert_eq!(fixture.core.approval_manager().pending_count(), 1);
    assert_eq!(fixture.core.session_grants().len(), 0);
    assert!(!task.is_finished());
    fixture
        .resolve(&approval, ApprovalDecision::AllowOnce)
        .unwrap();
    assert_eq!(output(&finish(task).await)["stdout"], "2");
    assert_eq!(fixture.core.session_grants().len(), 0);
}

#[tokio::test]
async fn command_rule_deny_does_not_hide_catalog_and_nonzero_exit_is_structured() {
    let fixture = Fixture::new(|cfg| {
        cfg.policies.tool_defaults.bash = PolicyDecision::Allow;
        let mut rule = UserPolicyRuleConfig::new(
            "deny-printf",
            "Deny printf",
            PolicyDecision::Deny,
            "bash",
            "Block exact command prefix.",
        );
        rule.matcher = PolicyMatcherConfig::bash_command("printf", vec!["blocked".into()]);
        cfg.policies.user_rules.push(rule);
    });
    assert!(fixture
        .service()
        .tools()
        .iter()
        .any(|tool| tool.name == "bash"));
    let result = finish(call(fixture.service(), "printf blocked > denied.txt")).await;
    assert_eq!(code(&result), "POLICY_DENIED");
    assert!(!fixture.root.join("denied.txt").exists());
    let result = finish(call(fixture.service(), "exit 7")).await;
    assert_eq!(output(&result)["exitCode"], 7);
}

#[tokio::test]
async fn bash_schema_rejects_client_authority_overrides_and_declares_high_risk_annotations() {
    let fixture = Fixture::new(|_| {});
    let tool = fixture
        .service()
        .tools()
        .into_iter()
        .find(|tool| tool.name == "bash")
        .unwrap();
    let schema = serde_json::to_value(tool).unwrap();
    assert_eq!(schema["annotations"]["readOnlyHint"], false);
    assert_eq!(schema["annotations"]["destructiveHint"], true);
    assert_eq!(schema["annotations"]["openWorldHint"], true);
    assert_eq!(schema["inputSchema"]["additionalProperties"], false);
    assert!(
        schema["outputSchema"].is_object(),
        "missing Bash output schema"
    );
    for property in ["cwd", "timeout", "env", "stdin"] {
        let mut args = json!({"command":"printf unsafe"});
        args[property] = json!("override");
        let result = fixture
            .service()
            .call_tool_async("bash", args)
            .await
            .unwrap();
        assert_eq!(code(&result), "INVALID_ARGUMENTS");
    }
    assert_eq!(fixture.core.approval_manager().pending_count(), 0);
    assert_eq!(fixture.core.bash_runtime().unwrap().active_count(), 0);
}
