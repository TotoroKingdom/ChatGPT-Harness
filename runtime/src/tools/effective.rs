use crate::policy::{PolicyContext, PolicyEngine, PolicyEvaluationReason};
use crate::tools::{ToolCapability, ToolRegistry};
use harness_control_protocol::{PolicyDecision, SecurityMode, ToolReadiness};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAvailability {
    Available,
    UnavailableByPolicy,
    UnavailableByRuntime,
    UnavailableDisabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveTool {
    pub name: String,
    pub description: String,
    pub availability: ToolAvailability,
    pub reason: String,
    pub decision: PolicyDecision,
    pub declared_capability: Option<ToolCapability>,
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
                let mut effective = EffectiveTool {
                    name: descriptor.name.clone(),
                    description: descriptor.description.clone(),
                    availability: ToolAvailability::UnavailableByRuntime,
                    reason: "runtime capability is unavailable".into(),
                    decision: PolicyDecision::Deny,
                    declared_capability: capability,
                };
                if descriptor.name.trim().is_empty()
                    || descriptor.description.trim().is_empty()
                    || descriptor.scope.trim().is_empty()
                    || !descriptor.implemented
                    || capability.is_none()
                {
                    effective.reason = "tool metadata or declared capability is missing".into();
                    return effective;
                }
                if !descriptor.enabled || !descriptor.exposed {
                    effective.availability = ToolAvailability::UnavailableDisabled;
                    effective.reason = "tool is disabled or not exposed".into();
                    return effective;
                }
                if !runtime_ready || descriptor.readiness != ToolReadiness::Ready {
                    return effective;
                }
                let evaluation = policy.evaluate(
                    &PolicyContext::new(&descriptor.name, security_mode.clone(), &descriptor.scope),
                    registry,
                );
                effective.decision = evaluation.decision.clone();
                match evaluation.decision {
                    PolicyDecision::Allow => {
                        effective.availability = ToolAvailability::Available;
                        effective.reason = "allowed by policy".into();
                    }
                    PolicyDecision::Ask => {
                        effective.availability = ToolAvailability::UnavailableByPolicy;
                        effective.reason = "approval is required by policy".into();
                    }
                    PolicyDecision::Deny => {
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
        PolicyEvaluationReason::MatchedRules => "denied by policy",
        PolicyEvaluationReason::NoApplicableRules => "no applicable policy rule; denied by default",
        PolicyEvaluationReason::InvalidRuleSet => "policy evaluation failed; denied by default",
        PolicyEvaluationReason::InvalidContext => "policy context is invalid; denied by default",
        PolicyEvaluationReason::UnsupportedCapability => {
            "declared capability is unsupported by policy"
        }
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
    fn ask_is_not_directly_available() {
        let ts = catalog(vec![rule("ask", PolicyDecision::Ask)], true);
        assert!(ts
            .iter()
            .all(|t| t.availability == ToolAvailability::UnavailableByPolicy
                && t.decision == PolicyDecision::Ask));
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
        );
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "write", "edit"]
        );
        assert!(tools[2..].iter().all(|tool| {
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
}
