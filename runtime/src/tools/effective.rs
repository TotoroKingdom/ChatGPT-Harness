use crate::policy::{to_wire_reason, PolicyContext, PolicyEngine, PolicyEvaluationReason};
use crate::tools::{ToolCapability, ToolRegistry};
use harness_control_protocol::{PolicyDecision, PolicyReason, SecurityMode, ToolReadiness};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAvailability {
    Available,
    AvailableWithApproval,
    UnavailableByMode,
    UnavailableByPolicy,
    UnavailableByRuntime,
    UnavailableDisabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveTool {
    pub status_reason: harness_control_protocol::ToolStatusReason,
    pub name: String,
    pub description: String,
    pub availability: ToolAvailability,
    pub reason: String,
    pub decision: PolicyDecision,
    pub matched_rule_ids: Vec<String>,
    pub policy_reason: PolicyReason,
    pub declared_capability: Option<ToolCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<ToolReadiness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EffectiveToolCatalog;

impl EffectiveToolCatalog {
    pub fn compute(
        registry: &ToolRegistry,
        policy: &PolicyEngine,
        security_mode: SecurityMode,
        runtime_ready: bool,
    ) -> Vec<EffectiveTool> {
        registry
            .list()
            .into_iter()
            .map(|descriptor| {
                let capability = registry.declared_capability(&descriptor.name);
                use harness_control_protocol::ToolStatusReason as R;
                let mut effective = EffectiveTool {
                    status_reason: R::CapabilityUnavailable,
                    name: descriptor.name.clone(),
                    description: descriptor.description.clone(),
                    availability: ToolAvailability::UnavailableByRuntime,
                    reason: "runtime capability is unavailable".into(),
                    decision: PolicyDecision::Deny,
                    matched_rule_ids: Vec::new(),
                    policy_reason: PolicyReason::UnsupportedCapability,
                    declared_capability: capability,
                    readiness: (descriptor.name == "bash").then(|| descriptor.readiness.clone()),
                    readiness_reason: (descriptor.name == "bash")
                        .then(|| registry.readiness_reason(&descriptor.name))
                        .flatten(),
                };
                if descriptor.name.trim().is_empty()
                    || descriptor.description.trim().is_empty()
                    || descriptor.scope.trim().is_empty()
                    || !descriptor.implemented
                    || capability.is_none()
                {
                    effective.status_reason = R::MetadataInvalid;
                    effective.reason = "tool metadata or declared capability is missing".into();
                    return effective;
                }
                if capability == Some(ToolCapability::DeveloperShell)
                    && matches!(&security_mode, SecurityMode::Normal)
                {
                    effective.status_reason = R::DeveloperModeRequired;
                    effective.availability = ToolAvailability::UnavailableByMode;
                    effective.reason =
                        "DeveloperShell is available only in Developer/Test mode".into();
                    effective.policy_reason = PolicyReason::UnsupportedCapability;
                    return effective;
                }
                if !descriptor.enabled || !descriptor.exposed {
                    effective.status_reason = R::ToolDisabled;
                    effective.availability = ToolAvailability::UnavailableDisabled;
                    effective.reason = "tool is disabled or not exposed".into();
                    effective.policy_reason = PolicyReason::ToolDisabled;
                    return effective;
                }
                if !runtime_ready || descriptor.readiness != ToolReadiness::Ready {
                    effective.status_reason = if !runtime_ready {
                        R::RuntimeNotReady
                    } else if descriptor.name == "bash" {
                        R::BashExecutableUnavailable
                    } else {
                        R::CapabilityUnavailable
                    };
                    if descriptor.readiness != ToolReadiness::Ready {
                        effective.reason = registry
                            .readiness_reason(&descriptor.name)
                            .unwrap_or_else(|| "runtime capability is unavailable".into());
                    }
                    return effective;
                }
                let evaluation = policy.evaluate(
                    &PolicyContext::new(&descriptor.name, security_mode.clone(), &descriptor.scope),
                    registry,
                );
                effective.decision = evaluation.decision.clone();
                effective.matched_rule_ids = evaluation.matched_rule_ids.clone();
                effective.policy_reason = to_wire_reason(evaluation.reason);
                match evaluation.decision {
                    PolicyDecision::Allow => {
                        effective.status_reason = R::Available;
                        effective.availability = ToolAvailability::Available;
                        effective.reason = "allowed by policy".into();
                    }
                    PolicyDecision::Ask => {
                        effective.status_reason = R::ApprovalRequired;
                        effective.availability = ToolAvailability::AvailableWithApproval;
                        effective.reason = "approval is required by policy".into();
                    }
                    PolicyDecision::Deny => {
                        effective.status_reason = match evaluation.reason {
                            PolicyEvaluationReason::InvalidRuleSet => R::InvalidPolicySnapshot,
                            PolicyEvaluationReason::InvalidContext => R::InvalidPolicyContext,
                            PolicyEvaluationReason::UnsupportedCapability => {
                                R::UnsupportedCapability
                            }
                            _ => R::PolicyDenied,
                        };
                        effective.availability = ToolAvailability::UnavailableByPolicy;
                        effective.reason = policy_reason(evaluation.reason).into();
                    }
                }
                effective
            })
            .collect()
    }
}

fn policy_reason(reason: PolicyEvaluationReason) -> &'static str {
    match reason {
        PolicyEvaluationReason::MatchedRules | PolicyEvaluationReason::MatchedUserRules => {
            "denied by policy"
        }
        PolicyEvaluationReason::NoApplicableRules => "no applicable policy rule; denied by default",
        PolicyEvaluationReason::ToolDefault => "tool default denied the request",
        PolicyEvaluationReason::SystemGuardrail => "denied by system guardrail",
        PolicyEvaluationReason::InvalidRuleSet => "policy evaluation failed; denied by default",
        PolicyEvaluationReason::InvalidContext => "policy context is invalid; denied by default",
        PolicyEvaluationReason::UnsupportedCapability => {
            "declared capability is unsupported by policy"
        }
        PolicyEvaluationReason::ToolDisabled => "tool is disabled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::{PolicyRule, PolicySource};
    fn rule(id: &str, decision: PolicyDecision) -> PolicyRule {
        PolicyRule {
            id: id.into(),
            name: "test".into(),
            decision,
            scope: "active_workspace".into(),
            reason: "test".into(),
            source: PolicySource::User,
            editable: false,
            enabled: true,
        }
    }
    fn catalog(rules: Vec<PolicyRule>, ready: bool) -> Vec<EffectiveTool> {
        EffectiveToolCatalog::compute(
            &ToolRegistry::normal_mode(),
            &PolicyEngine::new(rules),
            SecurityMode::Normal,
            ready,
        )
        .into_iter()
        .filter(|tool| tool.name != "bash")
        .collect()
    }
    #[test]
    fn allow_exposes_all_declared_tools() {
        assert!(catalog(vec![rule("allow", PolicyDecision::Allow)], true)
            .iter()
            .all(|t| t.availability == ToolAvailability::Available));
    }
    #[test]
    fn deny_hides_tools() {
        assert!(catalog(vec![rule("deny", PolicyDecision::Deny)], true)
            .iter()
            .all(|t| t.availability == ToolAvailability::UnavailableByPolicy));
    }
    #[test]
    fn policy_denial_for_one_declared_tool_does_not_hide_another() {
        let registry = ToolRegistry::normal_mode().with_test_scope(0, "outside_active_workspace");
        let tools = EffectiveToolCatalog::compute(
            &registry,
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::Normal,
            true,
        );

        assert_eq!(tools[0].name, "read_file");
        assert_eq!(tools[0].availability, ToolAvailability::UnavailableByPolicy);
        assert_eq!(tools[0].decision, PolicyDecision::Deny);
        assert_eq!(tools[1].name, "glob");
        assert_eq!(tools[1].availability, ToolAvailability::Available);
        assert_eq!(tools[1].decision, PolicyDecision::Allow);
    }

    #[test]
    fn ask_is_exposed_with_approval() {
        let ts = catalog(vec![rule("ask", PolicyDecision::Ask)], true);
        assert!(ts.iter().all(
            |t| t.availability == ToolAvailability::AvailableWithApproval
                && t.decision == PolicyDecision::Ask
        ));
    }
    #[test]
    fn policy_errors_fail_closed() {
        let mut r = rule("invalid", PolicyDecision::Allow);
        r.scope = "unknown-scope".into();
        let ts = catalog(vec![r], true);
        assert!(ts
            .iter()
            .all(|t| t.availability == ToolAvailability::UnavailableByPolicy
                && t.decision == PolicyDecision::Deny));
    }
    #[test]
    fn runtime_not_ready_is_unavailable() {
        assert!(catalog(vec![rule("allow", PolicyDecision::Allow)], false)
            .iter()
            .all(|t| t.availability == ToolAvailability::UnavailableByRuntime));
    }
    #[test]
    fn missing_metadata_is_not_exposed() {
        let reg = ToolRegistry::normal_mode().with_test_metadata(0, "");
        let ts = EffectiveToolCatalog::compute(
            &reg,
            &PolicyEngine::new(vec![rule("allow", PolicyDecision::Allow)]),
            SecurityMode::Normal,
            true,
        );
        assert_eq!(ts[0].availability, ToolAvailability::UnavailableByRuntime);
        assert!(ts[0].reason.contains("metadata"));
    }
    #[test]
    fn effective_tools_serialize_for_future_consumers() {
        let tools = catalog(vec![rule("allow", PolicyDecision::Allow)], true);
        let value = serde_json::to_value(&tools[0]).expect("effective tool serializes");

        assert_eq!(value["availability"], "available");
        assert_eq!(value["declaredCapability"], "workspace_read");
    }

    #[test]
    fn write_tools_keep_the_workspace_write_capability_in_the_effective_catalog() {
        let tools = EffectiveToolCatalog::compute(
            &ToolRegistry::normal_mode(),
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::Normal,
            true,
        )
        .into_iter()
        .filter(|tool| tool.name != "bash")
        .collect::<Vec<_>>();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "grep", "write", "edit"]
        );
        assert!(tools[3..].iter().all(|tool| {
            tool.availability == ToolAvailability::Available
                && tool.declared_capability == Some(ToolCapability::WorkspaceWrite)
        }));
    }

    #[test]
    fn repeated_computation_is_deterministic() {
        let reg = ToolRegistry::normal_mode();
        let p = PolicyEngine::new(vec![rule("allow", PolicyDecision::Allow)]);
        let first = EffectiveToolCatalog::compute(&reg, &p, SecurityMode::Normal, true);
        for _ in 0..5 {
            assert_eq!(
                EffectiveToolCatalog::compute(&reg, &p, SecurityMode::Normal, true),
                first
            );
        }
    }

    #[test]
    fn normal_mode_marks_developer_shell_unavailable_by_mode() {
        let tools = EffectiveToolCatalog::compute(
            &ToolRegistry::normal_mode(),
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::Normal,
            true,
        );
        let bash = tools.iter().find(|tool| tool.name == "bash").unwrap();
        assert_eq!(bash.availability, ToolAvailability::UnavailableByMode);
        assert_eq!(bash.decision, PolicyDecision::Deny);
    }

    #[test]
    fn developer_mode_distinguishes_disabled_and_unready_bash() {
        let disabled = EffectiveToolCatalog::compute(
            &ToolRegistry::normal_mode(),
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::DeveloperTest,
            true,
        );
        let bash = disabled.iter().find(|tool| tool.name == "bash").unwrap();
        assert_eq!(bash.availability, ToolAvailability::UnavailableDisabled);

        let mut config = harness_control_protocol::ToolConfig::default();
        config.bash.enabled = true;
        let unready = EffectiveToolCatalog::compute(
            &ToolRegistry::from_config(&config),
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::DeveloperTest,
            true,
        );
        let bash = unready.iter().find(|tool| tool.name == "bash").unwrap();
        assert_eq!(bash.availability, ToolAvailability::UnavailableByRuntime);
    }

    #[test]
    fn bash_executable_observation_remains_separate_from_mode_and_enablement() {
        let mut config = harness_control_protocol::ToolConfig::default();
        config.bash.enabled = true;
        let executable = crate::bash::discovery::BashExecutableState::Ready {
            canonical_path: std::path::PathBuf::from(r"C:\Git\bin\bash.exe"),
            source: crate::bash::discovery::BashExecutableSource::Manual,
        };
        let registry = ToolRegistry::from_config_with_bash(&config, &executable);
        let policy = PolicyEngine::compile(
            &harness_control_protocol::PolicyConfig::default(),
            &registry,
        );
        let normal = EffectiveToolCatalog::compute(&registry, &policy, SecurityMode::Normal, true);
        let bash = normal.into_iter().find(|tool| tool.name == "bash").unwrap();
        assert_eq!(bash.availability, ToolAvailability::UnavailableByMode);
        let observation =
            serde_json::to_value(crate::diagnostics::ToolDiagnostic::from(bash)).unwrap();
        assert_eq!(observation["availability"], "unavailable_by_mode");
        assert_eq!(observation["readiness"], "ready");
        assert!(observation["reason"]
            .as_str()
            .unwrap()
            .contains("Developer/Test"));

        config.bash.enabled = false;
        let registry = ToolRegistry::from_config_with_bash(&config, &executable);
        let tools =
            EffectiveToolCatalog::compute(&registry, &policy, SecurityMode::DeveloperTest, true);
        let bash = tools.into_iter().find(|tool| tool.name == "bash").unwrap();
        assert_eq!(bash.availability, ToolAvailability::UnavailableDisabled);
        assert_eq!(bash.readiness, Some(ToolReadiness::Ready));
    }

    #[test]
    fn bash_readiness_remediation_is_observed_without_disclosing_desired_path() {
        let registry = ToolRegistry::normal_mode();
        let tools = EffectiveToolCatalog::compute(
            &registry,
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::Normal,
            true,
        );
        for tool in tools {
            let name = tool.name.clone();
            let observation =
                serde_json::to_value(crate::diagnostics::ToolDiagnostic::from(tool)).unwrap();
            if name == "bash" {
                assert_eq!(observation["readiness"], "unavailable");
                assert!(observation["readinessReason"]
                    .as_str()
                    .unwrap()
                    .contains("manual"));
            } else {
                assert!(observation.get("readiness").is_none());
                assert!(observation.get("reason").is_none());
                assert!(observation.get("readinessReason").is_none());
            }
        }
    }
    #[test]
    fn m7_reason_codes_are_deterministic_for_eligibility_and_policy_states() {
        use harness_control_protocol::ToolStatusReason as R;
        for (decision, reason) in [
            (PolicyDecision::Allow, R::Available),
            (PolicyDecision::Ask, R::ApprovalRequired),
            (PolicyDecision::Deny, R::PolicyDenied),
        ] {
            assert!(catalog(vec![rule("user", decision)], true)
                .iter()
                .all(|t| t.status_reason == reason));
        }
        assert!(catalog(vec![rule("user", PolicyDecision::Allow)], false)
            .iter()
            .all(|t| t.status_reason == R::RuntimeNotReady));
        let mut invalid = rule("bad", PolicyDecision::Allow);
        invalid.scope = "bad-scope".into();
        assert!(catalog(vec![invalid], true)
            .iter()
            .all(|t| t.status_reason == R::InvalidPolicySnapshot));
        let registry = ToolRegistry::normal_mode().with_test_enabled(0, false);
        let tools = EffectiveToolCatalog::compute(
            &registry,
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::Normal,
            true,
        );
        assert_eq!(tools[0].status_reason, R::ToolDisabled);
        assert_eq!(
            tools.last().unwrap().status_reason,
            R::DeveloperModeRequired
        );
        let mut config = harness_control_protocol::ToolConfig::default();
        config.bash.enabled = true;
        let registry = ToolRegistry::from_config(&config);
        let tools = EffectiveToolCatalog::compute(
            &registry,
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::DeveloperTest,
            true,
        );
        assert_eq!(
            tools.last().unwrap().status_reason,
            R::BashExecutableUnavailable
        );
        let registry = ToolRegistry::normal_mode().with_test_scope(0, "");
        let metadata = EffectiveToolCatalog::compute(
            &registry,
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::Normal,
            true,
        );
        assert_eq!(metadata[0].status_reason, R::MetadataInvalid);
        let registry = ToolRegistry::normal_mode().with_test_scope(0, "invalid-scope");
        let context = EffectiveToolCatalog::compute(
            &registry,
            &PolicyEngine::new(PolicyEngine::system_rules()),
            SecurityMode::Normal,
            true,
        );
        assert_eq!(context[0].status_reason, R::InvalidPolicyContext);
    }
}
