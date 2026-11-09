use crate::{
    bash::analyzer::BashCommandAnalysis,
    tools::{ToolCapability, ToolRegistry},
};
use harness_control_protocol::{
    normalize_bash_executable, EffectivePolicyRule, EffectivePolicyTool, PolicyConfig,
    PolicyDecision, PolicyEffectiveSnapshot, PolicyMatcherConfig, PolicyReason, PolicyRule,
    PolicySnapshotStatus, PolicySource, PolicyToolAvailability, SecurityMode, ToolPolicyDefaults,
    UserPolicyRuleConfig,
};
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
    pub(crate) tool_name: &'a str,
    pub(crate) security_mode: SecurityMode,
    scope: Option<PolicyScope>,
    bash_analysis: Option<&'a BashCommandAnalysis>,
    require_bash_analysis: bool,
}

impl<'a> PolicyContext<'a> {
    pub(crate) fn new(tool_name: &'a str, security_mode: SecurityMode, scope: &str) -> Self {
        Self {
            tool_name,
            security_mode,
            scope: PolicyScope::parse(scope),
            bash_analysis: None,
            require_bash_analysis: false,
        }
    }

    /// Attach analyzer facts to a concrete Bash request.  The original raw
    /// command is deliberately absent; the execution pipeline retains it
    /// separately and this context carries only non-secret policy facts.
    pub(crate) fn with_bash_analysis(mut self, analysis: &'a BashCommandAnalysis) -> Self {
        self.bash_analysis = Some(analysis);
        self.require_bash_analysis = true;
        self
    }

    /// Mark a Bash context as a concrete request even before analysis is
    /// available.  Runtime then fails closed instead of falling back to the
    /// catalog-level Bash default.
    pub(crate) fn require_bash_analysis(mut self) -> Self {
        self.require_bash_analysis = true;
        self
    }

    /// Observational only: RequestContext sets this scope after WorkspaceGuard
    /// validation. This method supplies no execution authorization.
    pub(crate) fn has_workspace_scope(&self) -> bool {
        self.scope == Some(PolicyScope::ActiveWorkspace)
    }
}

/// Internal, argument-free evidence for the request/audit pipeline.
/// No raw tool arguments, paths, credentials, or tool output enter this model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PolicyEvaluation {
    pub(crate) decision: PolicyDecision,
    /// All applicable enabled rules, sorted lexicographically by unique rule ID.
    pub(crate) matched_rule_ids: Vec<String>,
    pub(crate) reason: PolicyEvaluationReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PolicyEvaluationReason {
    /// Legacy M0-M2 rule evaluation evidence.
    MatchedRules,
    /// M3 typed user-rule evidence.
    MatchedUserRules,
    /// Legacy no-match behavior retained for the compatibility constructor.
    NoApplicableRules,
    ToolDefault,
    SystemGuardrail,
    AnalyzerRiskFloor,
    InvalidRuleSet,
    InvalidContext,
    UnsupportedCapability,
    ToolDisabled,
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

#[derive(Clone)]
struct CompiledUserRule {
    config: UserPolicyRuleConfig,
    matcher: CompiledMatcher,
}

#[derive(Clone)]
enum CompiledMatcher {
    Tool {
        tool: String,
    },
    BashCommand {
        executable: String,
        argv_prefix: Vec<String>,
    },
}

#[derive(Clone)]
struct CompiledPolicySnapshot {
    defaults: ToolPolicyDefaults,
    all_user_rules: Vec<UserPolicyRuleConfig>,
    enabled_user_rules: Vec<CompiledUserRule>,
    system_rules: Vec<PolicyRule>,
    valid: bool,
}

#[derive(Clone)]
enum EngineSnapshot {
    /// Compatibility path used by M0-M2 unit tests and old internal callers.
    Legacy { rules: Vec<PolicyRule> },
    /// Generation-specific M3 snapshot. All fields are owned and immutable.
    Compiled(CompiledPolicySnapshot),
}

/// Runtime-owned immutable policy snapshot.
///
/// `new` intentionally remains as a compatibility constructor for the frozen
/// scope-based M0-M2 tests. Runtime generations use `compile`, which is the
/// authoritative M3 typed matcher path.
#[derive(Clone)]
pub(crate) struct PolicyEngine {
    snapshot: EngineSnapshot,
}

impl PolicyEngine {
    /// Uses exactly the supplied legacy rules, including an empty or invalid
    /// snapshot. Callers composing old policy fixtures retain the fixed system
    /// baseline explicitly.
    pub(crate) fn new(rules: Vec<PolicyRule>) -> Self {
        Self {
            snapshot: EngineSnapshot::Legacy { rules },
        }
    }

    /// Compiles a generation-specific M3 policy snapshot. Registry metadata is
    /// the only authoritative source for executable tool identity.
    pub(crate) fn compile(config: &PolicyConfig, registry: &ToolRegistry) -> Self {
        let mut ids = BTreeSet::new();
        let mut enabled_user_rules = Vec::new();
        let mut valid = true;

        for rule in &config.user_rules {
            if !rule.enabled {
                // Disabled semantic-invalid rules remain observable in the
                // desired/effective projection but do not poison evaluation.
                continue;
            }
            let id = rule.id.trim();
            let base_well_formed = !id.is_empty()
                && !rule.name.trim().is_empty()
                && !rule.reason.trim().is_empty()
                && ids.insert(id.to_owned())
                && rule.matcher.validate().is_ok();
            let matcher = match &rule.matcher {
                PolicyMatcherConfig::Tool { tool } => {
                    let tool = tool.trim();
                    // `declared_capability` includes configured-but-disabled
                    // built-ins, so a persisted rule can be compiled before
                    // the next generation becomes ready.
                    base_well_formed
                        .then(|| registry.declared_capability(tool).is_some())
                        .and_then(|known| {
                            known.then(|| CompiledMatcher::Tool {
                                tool: tool.to_owned(),
                            })
                        })
                }
                PolicyMatcherConfig::BashCommand {
                    executable,
                    argv_prefix,
                } => base_well_formed
                    .then(|| normalize_bash_executable(executable))
                    .flatten()
                    .filter(|_| registry.declared_capability("bash").is_some())
                    .map(|executable| CompiledMatcher::BashCommand {
                        executable,
                        argv_prefix: argv_prefix.clone(),
                    }),
            };
            if matcher.is_none() {
                valid = false;
                continue;
            }
            enabled_user_rules.push(CompiledUserRule {
                config: rule.clone(),
                matcher: matcher.expect("matcher checked above"),
            });
        }

        Self {
            snapshot: EngineSnapshot::Compiled(CompiledPolicySnapshot {
                defaults: config.tool_defaults.clone(),
                all_user_rules: config.user_rules.clone(),
                enabled_user_rules,
                system_rules: Self::system_rules(),
                valid,
            }),
        }
    }

    /// Minimum fixed system rules; developer/test shell and approval are not
    /// implemented. These rules are Runtime-owned and never persisted.
    pub(crate) fn system_rules() -> Vec<PolicyRule> {
        vec![
            PolicyRule {
                id: "system-read-workspace".into(),
                name: "Allow workspace capabilities".into(),
                decision: PolicyDecision::Allow,
                scope: "active_workspace".into(),
                reason: "Workspace capability baseline.".into(),
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

    pub(crate) fn is_valid(&self) -> bool {
        match &self.snapshot {
            EngineSnapshot::Legacy { .. } => true,
            EngineSnapshot::Compiled(snapshot) => snapshot.valid,
        }
    }

    pub(crate) fn evaluate(
        &self,
        context: &PolicyContext<'_>,
        registry: &ToolRegistry,
    ) -> PolicyEvaluation {
        match &self.snapshot {
            EngineSnapshot::Legacy { rules } => Self::evaluate_legacy(rules, context, registry),
            EngineSnapshot::Compiled(snapshot) => {
                Self::evaluate_compiled(snapshot, context, registry)
            }
        }
    }

    fn evaluate_legacy(
        rules: &[PolicyRule],
        context: &PolicyContext<'_>,
        registry: &ToolRegistry,
    ) -> PolicyEvaluation {
        // Validate every enabled rule, including scopes that would not match.
        // Use one failure reason so invalid snapshot ordering cannot change evidence.
        let mut ids = BTreeSet::new();
        let mut parsed_rules = Vec::new();
        for rule in rules.iter().filter(|rule| rule.enabled) {
            let Some(scope) = PolicyScope::parse(&rule.scope) else {
                return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidRuleSet);
            };
            if rule.id.trim().is_empty() || !ids.insert(rule.id.as_str()) {
                return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidRuleSet);
            }
            parsed_rules.push((rule, scope));
        }

        let Some(scope) = context.scope else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidContext);
        };
        // Capability metadata belongs to the existing registry, not another
        // name list. This does not read effective policy or compute a catalog.
        let Some(capability) = registry.capability(context.tool_name) else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability);
        };
        match (&context.security_mode, capability, scope) {
            (
                SecurityMode::Normal | SecurityMode::DeveloperTest,
                ToolCapability::WorkspaceRead | ToolCapability::WorkspaceWrite,
                PolicyScope::ActiveWorkspace | PolicyScope::OutsideActiveWorkspace,
            ) => {}
            // Merely switching modes cannot turn a read request into a test capability.
            _ => return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability),
        }

        let mut matched = parsed_rules
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

    fn evaluate_compiled(
        snapshot: &CompiledPolicySnapshot,
        context: &PolicyContext<'_>,
        registry: &ToolRegistry,
    ) -> PolicyEvaluation {
        if !snapshot.valid {
            return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidRuleSet);
        }
        let Some(scope) = context.scope else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidContext);
        };
        let Some(capability) = registry.capability(context.tool_name) else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability);
        };
        match (&context.security_mode, capability, scope) {
            (
                SecurityMode::Normal | SecurityMode::DeveloperTest,
                ToolCapability::WorkspaceRead | ToolCapability::WorkspaceWrite,
                PolicyScope::ActiveWorkspace,
            ) => {}
            // The fixed system guardrail is evaluated before User Rules and
            // defaults. A user ALLOW can never lower this DENY.
            (
                SecurityMode::Normal | SecurityMode::DeveloperTest,
                ToolCapability::WorkspaceRead | ToolCapability::WorkspaceWrite,
                PolicyScope::OutsideActiveWorkspace,
            ) => {
                return PolicyEvaluation {
                    decision: PolicyDecision::Deny,
                    matched_rule_ids: vec!["system-deny-outside".into()],
                    reason: PolicyEvaluationReason::SystemGuardrail,
                }
            }
            (
                SecurityMode::DeveloperTest,
                ToolCapability::DeveloperShell,
                PolicyScope::ActiveWorkspace,
            ) => {}
            (
                SecurityMode::Normal,
                ToolCapability::DeveloperShell,
                PolicyScope::ActiveWorkspace,
            ) => return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability),
            (
                SecurityMode::DeveloperTest,
                ToolCapability::DeveloperShell,
                PolicyScope::OutsideActiveWorkspace,
            ) => {
                return PolicyEvaluation {
                    decision: PolicyDecision::Deny,
                    matched_rule_ids: vec!["system-deny-outside".into()],
                    reason: PolicyEvaluationReason::SystemGuardrail,
                }
            }
            // Typed user rules do not create capabilities for other modes or
            // unknown logical scopes.
            _ => return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability),
        }

        if context.tool_name == "bash" && context.require_bash_analysis {
            let Some(analysis) = context.bash_analysis else {
                return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidContext);
            };
            return Self::evaluate_bash_request(snapshot, analysis);
        }

        let mut matched = snapshot
            .enabled_user_rules
            .iter()
            .filter(|rule| matches_tool(&rule.matcher, context.tool_name))
            .collect::<Vec<_>>();
        matched.sort_unstable_by(|left, right| left.config.id.cmp(&right.config.id));
        if let Some(decisive) = matched
            .iter()
            .max_by_key(|rule| severity(&rule.config.decision))
        {
            return PolicyEvaluation {
                decision: decisive.config.decision.clone(),
                matched_rule_ids: matched.iter().map(|rule| rule.config.id.clone()).collect(),
                reason: PolicyEvaluationReason::MatchedUserRules,
            };
        }

        let Some(decision) = snapshot.defaults.decision_for(context.tool_name) else {
            return PolicyEvaluation::denied(PolicyEvaluationReason::UnsupportedCapability);
        };
        PolicyEvaluation {
            decision,
            matched_rule_ids: Vec::new(),
            reason: PolicyEvaluationReason::ToolDefault,
        }
    }

    fn evaluate_bash_request(
        snapshot: &CompiledPolicySnapshot,
        analysis: &BashCommandAnalysis,
    ) -> PolicyEvaluation {
        if analysis.segments.is_empty()
            || analysis.operators.len().saturating_add(1) != analysis.segments.len()
            || analysis
                .segments
                .iter()
                .any(|segment| segment.executable.is_empty())
        {
            return PolicyEvaluation::denied(PolicyEvaluationReason::InvalidContext);
        }
        let mut matched_ids = BTreeSet::new();
        let mut aggregate = PolicyDecision::Allow;
        let mut any_explicit = false;
        let mut floor_raised = false;

        for segment in &analysis.segments {
            let mut segment_decision: Option<PolicyDecision> = None;
            for rule in &snapshot.enabled_user_rules {
                let matched = match &rule.matcher {
                    CompiledMatcher::Tool { tool } => tool == "bash",
                    CompiledMatcher::BashCommand {
                        executable,
                        argv_prefix,
                    } => {
                        segment.executable == *executable
                            && argv_prefix.len() <= segment.argv.len()
                            && argv_prefix
                                .iter()
                                .zip(&segment.argv)
                                .all(|(expected, actual)| expected == actual)
                    }
                };
                if matched {
                    any_explicit = true;
                    matched_ids.insert(rule.config.id.clone());
                    segment_decision =
                        Some(max_decision(segment_decision, rule.config.decision.clone()));
                }
            }
            let decision = segment_decision
                .or_else(|| snapshot.defaults.decision_for("bash"))
                .unwrap_or(PolicyDecision::Deny);
            let decision = if matches!(
                analysis.classification,
                crate::bash::analyzer::BashClassification::UnclassifiedHighRisk
            ) || !segment.destructive.is_empty()
            {
                let floored = max_decision(Some(decision.clone()), PolicyDecision::Ask);
                if floored != decision {
                    floor_raised = true;
                }
                floored
            } else {
                decision
            };
            aggregate = max_decision(Some(aggregate), decision);
        }

        let reason = if floor_raised && !matches!(aggregate, PolicyDecision::Deny) {
            PolicyEvaluationReason::AnalyzerRiskFloor
        } else if any_explicit {
            PolicyEvaluationReason::MatchedUserRules
        } else {
            PolicyEvaluationReason::ToolDefault
        };
        PolicyEvaluation {
            decision: aggregate,
            matched_rule_ids: matched_ids.into_iter().collect(),
            reason,
        }
    }

    /// Runtime-owned effective policy read model. It never asks the UI to
    /// reproduce matcher or precedence logic.
    pub(crate) fn effective_snapshot(&self, registry: &ToolRegistry) -> PolicyEffectiveSnapshot {
        let (defaults, all_user_rules, system_rules, valid) = match &self.snapshot {
            EngineSnapshot::Legacy { rules } => (
                ToolPolicyDefaults::default(),
                rules
                    .iter()
                    .filter(|rule| rule.source == PolicySource::User)
                    .map(user_config_from_legacy)
                    .collect::<Vec<_>>(),
                rules
                    .iter()
                    .filter(|rule| rule.source == PolicySource::System)
                    .cloned()
                    .collect::<Vec<_>>(),
                true,
            ),
            EngineSnapshot::Compiled(snapshot) => (
                snapshot.defaults.clone(),
                snapshot.all_user_rules.clone(),
                snapshot.system_rules.clone(),
                snapshot.valid,
            ),
        };

        let system_rules = system_rules
            .iter()
            .map(|rule| EffectivePolicyRule {
                id: rule.id.clone(),
                name: rule.name.clone(),
                decision: rule.decision.clone(),
                matcher: None,
                reason: rule.reason.clone(),
                source: PolicySource::System,
                editable: false,
                enabled: rule.enabled,
            })
            .collect();
        let user_rules = all_user_rules
            .iter()
            .map(|rule| EffectivePolicyRule {
                id: rule.id.clone(),
                name: rule.name.clone(),
                decision: rule.decision.clone(),
                matcher: Some(rule.matcher.clone()),
                reason: rule.reason.clone(),
                source: PolicySource::User,
                editable: true,
                enabled: rule.enabled,
            })
            .collect();

        let per_tool = registry
            .list()
            .into_iter()
            .map(|descriptor| {
                if !descriptor.enabled {
                    return EffectivePolicyTool {
                        tool: descriptor.name,
                        decision: PolicyDecision::Deny,
                        matched_rule_ids: Vec::new(),
                        reason: PolicyReason::ToolDisabled,
                        availability: PolicyToolAvailability::Disabled,
                    };
                }
                if descriptor.name == "bash"
                    && descriptor.readiness != harness_control_protocol::ToolReadiness::Ready
                {
                    return EffectivePolicyTool {
                        tool: descriptor.name,
                        decision: PolicyDecision::Deny,
                        matched_rule_ids: Vec::new(),
                        reason: PolicyReason::UnsupportedCapability,
                        availability: PolicyToolAvailability::UnavailableByRuntime,
                    };
                }
                let evaluation = if valid {
                    let mode = if descriptor.name == "bash" {
                        // Policy projection is mode-independent for catalog
                        // rows; DeveloperTest evaluates the Bash default and
                        // Tool-level rules without treating Normal-mode
                        // exposure as a policy denial.
                        SecurityMode::DeveloperTest
                    } else {
                        SecurityMode::Normal
                    };
                    self.evaluate(
                        &PolicyContext::new(&descriptor.name, mode, &descriptor.scope),
                        registry,
                    )
                } else {
                    PolicyEvaluation::denied(PolicyEvaluationReason::InvalidRuleSet)
                };
                let availability = match evaluation.decision {
                    PolicyDecision::Allow => PolicyToolAvailability::Available,
                    PolicyDecision::Ask if descriptor.name == "bash" => {
                        PolicyToolAvailability::AvailableWithApproval
                    }
                    PolicyDecision::Ask | PolicyDecision::Deny => {
                        PolicyToolAvailability::UnavailableByPolicy
                    }
                };
                EffectivePolicyTool {
                    tool: descriptor.name,
                    decision: evaluation.decision,
                    matched_rule_ids: evaluation.matched_rule_ids,
                    reason: to_wire_reason(evaluation.reason),
                    availability,
                }
            })
            .collect();

        PolicyEffectiveSnapshot {
            valid,
            status: if valid {
                PolicySnapshotStatus::Valid
            } else {
                PolicySnapshotStatus::Invalid
            },
            status_reason: (!valid).then_some(PolicyReason::InvalidPolicySnapshot),
            system_rules,
            user_rules,
            tool_defaults: defaults,
            per_tool,
        }
    }

    pub(crate) fn legacy_rules_for_list(&self, registry: &ToolRegistry) -> Vec<PolicyRule> {
        match &self.snapshot {
            EngineSnapshot::Legacy { rules } => rules.clone(),
            EngineSnapshot::Compiled(snapshot) => {
                let mut rules = snapshot.system_rules.clone();
                rules.extend(snapshot.all_user_rules.iter().map(|rule| PolicyRule {
                    id: rule.id.clone(),
                    name: rule.name.clone(),
                    decision: rule.decision.clone(),
                    scope: legacy_scope(&rule.matcher),
                    reason: rule.reason.clone(),
                    source: PolicySource::User,
                    editable: true,
                    enabled: rule.enabled,
                }));
                // Keep the registry argument explicit at this boundary: list
                // output must never be based on request-supplied tool names.
                let _ = registry;
                rules
            }
        }
    }
}

fn user_config_from_legacy(rule: &PolicyRule) -> UserPolicyRuleConfig {
    UserPolicyRuleConfig {
        id: rule.id.clone(),
        name: rule.name.clone(),
        enabled: rule.enabled,
        decision: rule.decision.clone(),
        matcher: harness_control_protocol::PolicyMatcherConfig::tool(rule.scope.clone()),
        reason: rule.reason.clone(),
    }
}

fn matches_tool(matcher: &CompiledMatcher, tool: &str) -> bool {
    matches!(matcher, CompiledMatcher::Tool { tool: matcher_tool } if matcher_tool == tool)
}

fn max_decision(current: Option<PolicyDecision>, candidate: PolicyDecision) -> PolicyDecision {
    match current {
        Some(current) if severity(&current) >= severity(&candidate) => current,
        _ => candidate,
    }
}

fn legacy_scope(matcher: &PolicyMatcherConfig) -> String {
    match matcher {
        PolicyMatcherConfig::Tool { tool } => format!("tool:{tool}"),
        PolicyMatcherConfig::BashCommand {
            executable,
            argv_prefix,
        } => format!(
            "bash_command:{}:{}",
            executable,
            serde_json::to_string(argv_prefix).unwrap_or_else(|_| "[]".into())
        ),
    }
}

fn severity(decision: &PolicyDecision) -> u8 {
    match decision {
        PolicyDecision::Deny => 2,
        PolicyDecision::Ask => 1,
        PolicyDecision::Allow => 0,
    }
}

pub(crate) fn to_wire_reason(reason: PolicyEvaluationReason) -> PolicyReason {
    match reason {
        PolicyEvaluationReason::MatchedRules | PolicyEvaluationReason::MatchedUserRules => {
            PolicyReason::MatchedUserRules
        }
        PolicyEvaluationReason::NoApplicableRules | PolicyEvaluationReason::ToolDefault => {
            PolicyReason::ToolDefault
        }
        PolicyEvaluationReason::SystemGuardrail => PolicyReason::SystemGuardrail,
        PolicyEvaluationReason::AnalyzerRiskFloor => PolicyReason::AnalyzerRiskFloor,
        PolicyEvaluationReason::InvalidRuleSet => PolicyReason::InvalidPolicySnapshot,
        PolicyEvaluationReason::InvalidContext => PolicyReason::InvalidContext,
        PolicyEvaluationReason::UnsupportedCapability => PolicyReason::UnsupportedCapability,
        PolicyEvaluationReason::ToolDisabled => PolicyReason::ToolDisabled,
    }
}
