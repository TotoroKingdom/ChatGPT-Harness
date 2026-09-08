use crate::tools::{ToolCapability, ToolRegistry};
use harness_control_protocol::{PolicyDecision, PolicyRule, PolicySource, SecurityMode};
use std::collections::BTreeSet;

/// These are logical scopes, never filesystem paths or path authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyScope {
    ActiveWorkspace,
    OutsideActiveWorkspace,
    DeveloperTestCapabilities,
}

impl PolicyScope {
    fn parse(scope: &str) -> Option<Self> {
        match scope {
            // M5's registry uses `workspace`; M0 policy fixtures use the longer name.
            "workspace" | "active_workspace" => Some(Self::ActiveWorkspace),
            "outside_active_workspace" => Some(Self::OutsideActiveWorkspace),
            "developer_test_capabilities" => Some(Self::DeveloperTestCapabilities),
            _ => None,
        }
    }
}

/// Non-secret request facts supplied by the future validated Runtime pipeline.
///
/// An active-workspace assertion is NOT a substitute for WorkspaceGuard approval.
/// Invalid scope input is retained as unknown and always evaluates to DENY.
#[derive(Clone, Debug)]
pub(crate) struct PolicyContext<'a> {
    tool_name: &'a str,
    security_mode: SecurityMode,
    scope: Option<PolicyScope>,
}

impl<'a> PolicyContext<'a> {
    pub(crate) fn new(tool_name: &'a str, security_mode: SecurityMode, scope: &str) -> Self {
        Self {
            tool_name,
            security_mode,
            scope: PolicyScope::parse(scope),
        }
    }
}

/// Internal, argument-free evidence for the future request/audit pipeline.
/// No wire model or UI representation is introduced here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PolicyEvaluation {
    pub(crate) decision: PolicyDecision,
    /// All applicable enabled rules, sorted lexicographically by unique rule ID.
    pub(crate) matched_rule_ids: Vec<String>,
    pub(crate) reason: PolicyEvaluationReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PolicyEvaluationReason {
    MatchedRules,
    NoApplicableRules,
    InvalidRuleSet,
    InvalidContext,
    UnsupportedCapability,
}

impl PolicyEvaluation {
    fn denied(reason: PolicyEvaluationReason) -> Self {
        Self {
            decision: PolicyDecision::Deny,
            matched_rule_ids: Vec::new(),
            reason,
        }
    }
}

/// Immutable rule snapshot. Invalid enabled rules invalidate the entire evaluation;
/// they must never be skipped in favor of a valid ALLOW elsewhere in the snapshot.
/// There is deliberately no default-ALLOW or source-based precedence.
#[derive(Clone)]
pub(crate) struct PolicyEngine {
    rules: Vec<PolicyRule>,
}

impl PolicyEngine {
    /// Uses exactly the supplied rules (including an empty or invalid snapshot).
    /// Callers composing user policies must retain the fixed system baseline.
    pub(crate) fn new(rules: Vec<PolicyRule>) -> Self {
        Self { rules }
    }

    /// Minimum fixed V1 rules; developer/test shell and approval are not implemented.
    pub(crate) fn system_rules() -> Vec<PolicyRule> {
        vec![
            PolicyRule {
                id: "system-read-workspace".into(),
                name: "Allow workspace reads".into(),
                decision: PolicyDecision::Allow,
                scope: "active_workspace".into(),
                reason: "Read-only default capability.".into(),
                source: PolicySource::System,
                editable: false,
                enabled: true,
            },
            PolicyRule {
                id: "system-deny-outside".into(),
                name: "Deny outside workspace".into(),
                decision: PolicyDecision::Deny,
                scope: "outside_active_workspace".into(),
                reason: "Workspace boundary.".into(),
                source: PolicySource::System,
                editable: false,
                enabled: true,
            },
        ]
    }

    pub(crate) fn evaluate(
        &self,
        context: &PolicyContext<'_>,
        registry: &ToolRegistry,
    ) -> PolicyEvaluation {
        // Validate every enabled rule, including scopes that would not match.
        // Use one failure reason so invalid snapshot ordering cannot change evidence.
        let mut ids = BTreeSet::new();
        let mut rules = Vec::new();
        for rule in self.rules.iter().filter(|rule| rule.enabled) {
            let Some(scope) = PolicyScope::parse(&rule.scope) else {
                return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidRuleSet);
            };
            if rule.id.trim().is_empty() || !ids.insert(rule.id.as_str()) {
                return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidRuleSet);
            }
            rules.push((rule, scope));
        }

        let Some(scope) = context.scope else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidContext);
        };
        // Capability metadata belongs to the existing registry, not another name list.
        // This does not read ToolDescriptor.effective_policy or compute a catalog.
        let Some(capability) = registry.capability(context.tool_name) else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability);
        };
        match (&context.security_mode, capability, scope) {
            (
                SecurityMode::Normal | SecurityMode::DeveloperTest,
                ToolCapability::WorkspaceRead,
                PolicyScope::ActiveWorkspace | PolicyScope::OutsideActiveWorkspace,
            ) => {}
            // Merely switching modes cannot turn a read request into a test capability.
            _ => return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability),
        }

        let mut matched = rules
            .into_iter()
            .filter(|(_, rule_scope)| *rule_scope == scope)
            .map(|(rule, _)| rule)
            .collect::<Vec<_>>();
        matched.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let Some(decisive) = matched.iter().max_by_key(|rule| severity(&rule.decision)) else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::NoApplicableRules);
        };
        PolicyEvaluation {
            decision: decisive.decision.clone(),
            matched_rule_ids: matched.iter().map(|rule| rule.id.clone()).collect(),
            reason: PolicyEvaluationReason::MatchedRules,
        }
    }
}

fn severity(decision: &PolicyDecision) -> u8 {
    match decision {
        PolicyDecision::Deny => 2,
        PolicyDecision::Ask => 1,
        PolicyDecision::Allow => 0,
    }
}
