use super::*;
use crate::bash::analyzer::{analyze, BashCommandAnalysis};
use crate::bash::discovery::{BashExecutableSource, BashExecutableState};
use crate::tools::ToolRegistry;
use harness_control_protocol::{
    PolicyConfig, PolicyDecision, PolicyMatcherConfig, PolicyRule, PolicySource, SecurityMode,
    ToolConfig, UserPolicyRuleConfig,
};
use std::path::PathBuf;

fn rule(id: &str, decision: PolicyDecision, scope: &str) -> PolicyRule {
    PolicyRule {
        id: id.into(),
        name: "Test rule".into(),
        decision,
        scope: scope.into(),
        reason: "Test policy".into(),
        source: PolicySource::User,
        editable: false,
        enabled: true,
    }
}

fn evaluate(rules: Vec<PolicyRule>) -> PolicyEvaluation {
    PolicyEngine::new(rules).evaluate(
        &PolicyContext::new("read_file", SecurityMode::Normal, "active_workspace"),
        &ToolRegistry::normal_mode(),
    )
}

fn decisions(decisions: &[PolicyDecision]) -> PolicyEvaluation {
    evaluate(
        decisions
            .iter()
            .enumerate()
            .map(|(index, decision)| {
                rule(
                    &format!("rule-{index}"),
                    decision.clone(),
                    "active_workspace",
                )
            })
            .collect(),
    )
}

fn user_config(id: &str, decision: PolicyDecision, tool: &str) -> UserPolicyRuleConfig {
    UserPolicyRuleConfig {
        id: id.into(),
        name: format!("Rule {id}"),
        enabled: true,
        decision,
        matcher: PolicyMatcherConfig::tool(tool),
        reason: "test rule".into(),
    }
}

fn compiled(config: PolicyConfig) -> PolicyEngine {
    let registry = ToolRegistry::normal_mode();
    PolicyEngine::compile(&config, &registry)
}

fn evaluate_compiled(config: PolicyConfig, tool: &str) -> PolicyEvaluation {
    compiled(config).evaluate(
        &PolicyContext::new(tool, SecurityMode::Normal, "active_workspace"),
        &ToolRegistry::normal_mode(),
    )
}

fn bash_registry() -> ToolRegistry {
    let mut config = ToolConfig::default();
    config.bash.enabled = true;
    let state = BashExecutableState::Ready {
        canonical_path: PathBuf::from("C:/Git/bin/bash.exe"),
        source: BashExecutableSource::Manual,
    };
    ToolRegistry::from_config_with_bash(&config, &state)
}

fn bash_rule(
    id: &str,
    decision: PolicyDecision,
    matcher: PolicyMatcherConfig,
) -> UserPolicyRuleConfig {
    UserPolicyRuleConfig {
        id: id.into(),
        name: format!("Rule {id}"),
        enabled: true,
        decision,
        matcher,
        reason: "Bash policy test".into(),
    }
}

fn evaluate_bash(config: PolicyConfig, command: &str) -> PolicyEvaluation {
    let registry = bash_registry();
    let engine = PolicyEngine::compile(&config, &registry);
    let analysis = analyze(command);
    engine.evaluate(
        &PolicyContext::new("bash", SecurityMode::DeveloperTest, "active_workspace")
            .with_bash_analysis(&analysis),
        &registry,
    )
}

#[test]
fn allow_evaluates_to_canonical_allow() {
    let actual: PolicyDecision = decisions(&[PolicyDecision::Allow]).decision;
    assert_eq!(actual, PolicyDecision::Allow);
}

#[test]
fn ask_evaluates_to_ask_without_creating_approval() {
    assert_eq!(
        decisions(&[PolicyDecision::Ask]).decision,
        PolicyDecision::Ask
    );
}

#[test]
fn deny_evaluates_to_deny() {
    assert_eq!(
        decisions(&[PolicyDecision::Deny]).decision,
        PolicyDecision::Deny
    );
}

#[test]
fn ask_overrides_allow() {
    assert_eq!(
        decisions(&[PolicyDecision::Allow, PolicyDecision::Ask]).decision,
        PolicyDecision::Ask
    );
}

#[test]
fn deny_overrides_allow() {
    assert_eq!(
        decisions(&[PolicyDecision::Allow, PolicyDecision::Deny]).decision,
        PolicyDecision::Deny
    );
}

#[test]
fn deny_overrides_ask() {
    assert_eq!(
        decisions(&[PolicyDecision::Ask, PolicyDecision::Deny]).decision,
        PolicyDecision::Deny
    );
}

#[test]
fn deny_overrides_ask_and_allow() {
    assert_eq!(
        decisions(&[
            PolicyDecision::Allow,
            PolicyDecision::Ask,
            PolicyDecision::Deny
        ])
        .decision,
        PolicyDecision::Deny
    );
}

#[test]
fn every_permutation_has_identical_decision_and_sorted_evidence() {
    let rules = [
        rule("z-allow", PolicyDecision::Allow, "active_workspace"),
        rule("a-ask", PolicyDecision::Ask, "active_workspace"),
        rule("m-deny", PolicyDecision::Deny, "active_workspace"),
    ];
    let expected = PolicyEvaluation {
        decision: PolicyDecision::Deny,
        matched_rule_ids: vec!["a-ask".into(), "m-deny".into(), "z-allow".into()],
        reason: PolicyEvaluationReason::MatchedRules,
    };
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        assert_eq!(
            evaluate(order.map(|index| rules[index].clone()).to_vec()),
            expected
        );
    }
}

#[test]
fn reordered_allow_and_ask_have_identical_evidence() {
    let allow = rule("allow", PolicyDecision::Allow, "active_workspace");
    let ask = rule("ask", PolicyDecision::Ask, "active_workspace");
    let expected = evaluate(vec![allow.clone(), ask.clone()]);
    assert_eq!(expected.decision, PolicyDecision::Ask);
    assert_eq!(evaluate(vec![ask, allow]), expected);
}

#[test]
fn disabled_deny_does_not_override_allow_or_appear_in_matches() {
    let mut deny = rule("disabled", PolicyDecision::Deny, "active_workspace");
    deny.enabled = false;
    let result = evaluate(vec![
        deny,
        rule("allow", PolicyDecision::Allow, "active_workspace"),
    ]);
    assert_eq!(result.decision, PolicyDecision::Allow);
    assert_eq!(result.matched_rule_ids, ["allow"]);
}

#[test]
fn disabled_ask_does_not_override_allow() {
    let mut ask = rule("disabled", PolicyDecision::Ask, "active_workspace");
    ask.enabled = false;
    assert_eq!(
        evaluate(vec![
            ask,
            rule("allow", PolicyDecision::Allow, "active_workspace")
        ])
        .decision,
        PolicyDecision::Allow
    );
}

#[test]
fn no_rules_fail_closed() {
    let result = evaluate(vec![]);
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.reason, PolicyEvaluationReason::NoApplicableRules);
    assert!(result.matched_rule_ids.is_empty());
}

#[test]
fn only_disabled_rules_fail_closed() {
    let mut allow = rule("disabled", PolicyDecision::Allow, "active_workspace");
    allow.enabled = false;
    assert_eq!(evaluate(vec![allow]).decision, PolicyDecision::Deny);
}

#[test]
fn no_applicable_rules_fail_closed() {
    let result = evaluate(vec![rule(
        "elsewhere",
        PolicyDecision::Allow,
        "outside_active_workspace",
    )]);
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.reason, PolicyEvaluationReason::NoApplicableRules);
}

#[test]
fn unknown_or_absent_tools_fail_closed_even_with_allow() {
    let engine = PolicyEngine::new(PolicyEngine::system_rules());
    for mode in [SecurityMode::Normal, SecurityMode::DeveloperTest] {
        for name in [
            "unknown",
            "bash",
            "write_file",
            "edit_file",
            "",
            " read_file",
            "READ_FILE",
        ] {
            let result = engine.evaluate(
                &PolicyContext::new(name, mode.clone(), "active_workspace"),
                &ToolRegistry::normal_mode(),
            );
            assert_eq!(result.decision, PolicyDecision::Deny, "{name}");
            assert_eq!(result.reason, PolicyEvaluationReason::UnsupportedCapability);
        }
    }
}

#[test]
fn unknown_and_malformed_enabled_rule_scopes_invalidate_the_whole_snapshot() {
    for scope in [
        "unknown",
        "",
        " active_workspace",
        "active_workspace ",
        "ACTIVE_WORKSPACE",
        "active_workspace/*",
        "workspace\0",
        "*",
    ] {
        // An invalid DENY must not disappear behind a valid ALLOW, regardless of order.
        let invalid = rule("invalid", PolicyDecision::Deny, scope);
        let allow = rule("allow", PolicyDecision::Allow, "active_workspace");
        let expected = evaluate(vec![invalid.clone(), allow.clone()]);
        assert_eq!(expected.decision, PolicyDecision::Deny, "{scope:?}");
        assert_eq!(expected.reason, PolicyEvaluationReason::InvalidRuleSet);
        assert!(expected.matched_rule_ids.is_empty());
        assert_eq!(evaluate(vec![allow, invalid]), expected);
    }
}

#[test]
fn unknown_allow_scope_also_invalidates_a_valid_allow() {
    assert_eq!(
        evaluate(vec![
            rule("bad", PolicyDecision::Allow, "unknown"),
            rule("good", PolicyDecision::Allow, "active_workspace")
        ])
        .reason,
        PolicyEvaluationReason::InvalidRuleSet
    );
}

#[test]
fn disabled_invalid_rules_are_completely_ignored() {
    let mut invalid = rule("", PolicyDecision::Deny, "unknown");
    invalid.enabled = false;
    assert_eq!(
        evaluate(vec![
            invalid,
            rule("allow", PolicyDecision::Allow, "active_workspace")
        ])
        .decision,
        PolicyDecision::Allow
    );
}

#[test]
fn unknown_and_malformed_context_scopes_fail_closed() {
    let engine = PolicyEngine::new(PolicyEngine::system_rules());
    for scope in [
        "unknown",
        "",
        "workspace ",
        "ACTIVE_WORKSPACE",
        "active_workspace/*",
        "*",
    ] {
        let result = engine.evaluate(
            &PolicyContext::new("read_file", SecurityMode::Normal, scope),
            &ToolRegistry::normal_mode(),
        );
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.reason, PolicyEvaluationReason::InvalidContext);
    }
}

#[test]
fn normal_read_file_in_active_workspace_is_allowed() {
    let result = evaluate(PolicyEngine::system_rules());
    assert_eq!(result.decision, PolicyDecision::Allow);
    assert_eq!(result.matched_rule_ids, ["system-read-workspace"]);
}

#[test]
fn normal_glob_in_active_workspace_is_allowed() {
    assert_eq!(
        PolicyEngine::new(PolicyEngine::system_rules())
            .evaluate(
                &PolicyContext::new("glob", SecurityMode::Normal, "active_workspace"),
                &ToolRegistry::normal_mode()
            )
            .decision,
        PolicyDecision::Allow
    );
}

#[test]
fn workspace_write_is_allowed_in_active_workspace_in_both_supported_modes() {
    let engine = PolicyEngine::new(PolicyEngine::system_rules());
    let registry = ToolRegistry::normal_mode();
    for mode in [SecurityMode::Normal, SecurityMode::DeveloperTest] {
        for tool in ["write", "edit"] {
            let result = engine.evaluate(
                &PolicyContext::new(tool, mode.clone(), "active_workspace"),
                &registry,
            );
            assert_eq!(result.decision, PolicyDecision::Allow, "{tool} / {mode:?}");
        }
    }
}

#[test]
fn workspace_write_is_denied_for_invalid_context_or_unknown_capability() {
    let engine = PolicyEngine::new(PolicyEngine::system_rules());
    let registry = ToolRegistry::normal_mode();
    for (tool, scope, reason) in [
        ("write", "unknown", PolicyEvaluationReason::InvalidContext),
        (
            "unknown_write",
            "active_workspace",
            PolicyEvaluationReason::UnsupportedCapability,
        ),
    ] {
        let result = engine.evaluate(
            &PolicyContext::new(tool, SecurityMode::Normal, scope),
            &registry,
        );
        assert_eq!(result.decision, PolicyDecision::Deny, "{tool}");
        assert_eq!(result.reason, reason, "{tool}");
    }
}

#[test]
fn outside_workspace_is_denied_for_every_registered_tool_in_both_modes() {
    let registry = ToolRegistry::normal_mode();
    let mut rules = PolicyEngine::system_rules();
    rules.push(rule(
        "user-outside-allow",
        PolicyDecision::Allow,
        "outside_active_workspace",
    ));
    rules.push(rule(
        "user-outside-ask",
        PolicyDecision::Ask,
        "outside_active_workspace",
    ));
    let engine = PolicyEngine::new(rules);
    for mode in [SecurityMode::Normal, SecurityMode::DeveloperTest] {
        for tool in registry
            .list()
            .into_iter()
            .filter(|tool| tool.name != "bash")
        {
            let result = engine.evaluate(
                &PolicyContext::new(&tool.name, mode.clone(), "outside_active_workspace"),
                &registry,
            );
            assert_eq!(result.decision, PolicyDecision::Deny);
            assert_eq!(
                result.matched_rule_ids,
                [
                    "system-deny-outside",
                    "user-outside-allow",
                    "user-outside-ask"
                ]
            );
        }
    }
}

#[test]
fn developer_mode_preserves_read_only_defaults_and_does_not_match_test_scope() {
    let mut rules = PolicyEngine::system_rules();
    rules.push(rule(
        "test-only",
        PolicyDecision::Deny,
        "developer_test_capabilities",
    ));
    let engine = PolicyEngine::new(rules);
    let registry = ToolRegistry::normal_mode();
    for tool in registry
        .list()
        .into_iter()
        .filter(|tool| tool.name != "bash")
    {
        let result = engine.evaluate(
            &PolicyContext::new(&tool.name, SecurityMode::DeveloperTest, "active_workspace"),
            &registry,
        );
        assert_eq!(result.decision, PolicyDecision::Allow);
        assert_eq!(result.matched_rule_ids, ["system-read-workspace"]);
    }
}

#[test]
fn developer_scope_cannot_reclassify_a_read_tool() {
    let engine = PolicyEngine::new(vec![rule(
        "allow-test",
        PolicyDecision::Allow,
        "developer_test_capabilities",
    )]);
    for mode in [SecurityMode::Normal, SecurityMode::DeveloperTest] {
        let result = engine.evaluate(
            &PolicyContext::new("read_file", mode, "developer_test_capabilities"),
            &ToolRegistry::normal_mode(),
        );
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.reason, PolicyEvaluationReason::UnsupportedCapability);
    }
}

#[test]
fn known_but_inapplicable_developer_scope_has_no_match() {
    assert_eq!(
        evaluate(vec![rule(
            "test",
            PolicyDecision::Allow,
            "developer_test_capabilities"
        )])
        .reason,
        PolicyEvaluationReason::NoApplicableRules
    );
}

#[test]
fn runtime_workspace_alias_matches_m0_scope_in_both_directions() {
    let registry = ToolRegistry::normal_mode();
    for rule_scope in ["workspace", "active_workspace"] {
        for context_scope in ["workspace", "active_workspace"] {
            let result = PolicyEngine::new(vec![rule("alias", PolicyDecision::Allow, rule_scope)])
                .evaluate(
                    &PolicyContext::new("read_file", SecurityMode::Normal, context_scope),
                    &registry,
                );
            assert_eq!(result.decision, PolicyDecision::Allow);
        }
    }
    for tool in registry
        .list()
        .into_iter()
        .filter(|tool| tool.name != "bash")
    {
        assert_eq!(
            PolicyEngine::new(PolicyEngine::system_rules())
                .evaluate(
                    &PolicyContext::new(&tool.name, SecurityMode::Normal, &tool.scope),
                    &registry
                )
                .decision,
            PolicyDecision::Allow
        );
    }
}

#[test]
fn m3_defaults_are_fallbacks_and_bash_is_not_ready_by_default() {
    let mut config = PolicyConfig::default();
    config.tool_defaults.write = PolicyDecision::Allow;
    assert_eq!(
        evaluate_compiled(config.clone(), "write"),
        PolicyEvaluation {
            decision: PolicyDecision::Allow,
            matched_rule_ids: vec![],
            reason: PolicyEvaluationReason::ToolDefault,
        }
    );
    assert_eq!(config.tool_defaults.bash, PolicyDecision::Ask);
    assert_eq!(
        ToolRegistry::normal_mode().capability("bash"),
        None,
        "Bash remains unavailable until it is enabled and Runtime validates an executable"
    );
}

#[test]
fn explicit_allow_overrides_an_ask_tool_default() {
    let mut config = PolicyConfig::default();
    config.tool_defaults.write = PolicyDecision::Ask;
    config
        .user_rules
        .push(user_config("write-allow", PolicyDecision::Allow, "write"));
    assert_eq!(
        evaluate_compiled(config, "write"),
        PolicyEvaluation {
            decision: PolicyDecision::Allow,
            matched_rule_ids: vec!["write-allow".into()],
            reason: PolicyEvaluationReason::MatchedUserRules,
        }
    );
}

#[test]
fn typed_user_rules_aggregate_deny_ask_allow_in_stable_order() {
    let mut config = PolicyConfig::default();
    config.user_rules = vec![
        user_config("z-allow", PolicyDecision::Allow, "write"),
        user_config("a-ask", PolicyDecision::Ask, "write"),
        user_config("m-deny", PolicyDecision::Deny, "write"),
    ];
    let expected = PolicyEvaluation {
        decision: PolicyDecision::Deny,
        matched_rule_ids: vec!["a-ask".into(), "m-deny".into(), "z-allow".into()],
        reason: PolicyEvaluationReason::MatchedUserRules,
    };
    assert_eq!(evaluate_compiled(config.clone(), "write"), expected);
    config.user_rules.reverse();
    assert_eq!(evaluate_compiled(config, "write"), expected);
}

#[test]
fn enabled_invalid_typed_rule_invalidates_the_whole_snapshot_even_when_unmatched() {
    let mut config = PolicyConfig::default();
    config
        .user_rules
        .push(user_config("valid", PolicyDecision::Allow, "read_file"));
    config.user_rules.push(user_config(
        "unknown",
        PolicyDecision::Allow,
        "unknown_tool",
    ));
    let engine = compiled(config);
    assert!(!engine.is_valid());
    for tool in ["read_file", "glob", "write", "edit"] {
        let result = engine.evaluate(
            &PolicyContext::new(tool, SecurityMode::Normal, "active_workspace"),
            &ToolRegistry::normal_mode(),
        );
        assert_eq!(result.decision, PolicyDecision::Deny, "{tool}");
        assert_eq!(result.reason, PolicyEvaluationReason::InvalidRuleSet);
        assert!(result.matched_rule_ids.is_empty());
    }
}

#[test]
fn duplicate_enabled_ids_invalidate_the_snapshot_and_disabled_invalid_rule_is_ignored() {
    let mut duplicate = PolicyConfig::default();
    duplicate.user_rules = vec![
        user_config("same", PolicyDecision::Allow, "write"),
        user_config("same", PolicyDecision::Deny, "write"),
    ];
    assert!(!compiled(duplicate).is_valid());

    let mut disabled = user_config("", PolicyDecision::Deny, "unknown_tool");
    disabled.enabled = false;
    let mut config = PolicyConfig::default();
    config.user_rules = vec![
        disabled,
        user_config("allow", PolicyDecision::Allow, "write"),
    ];
    let engine = compiled(config);
    assert!(engine.is_valid());
    assert_eq!(
        engine
            .evaluate(
                &PolicyContext::new("write", SecurityMode::Normal, "active_workspace"),
                &ToolRegistry::normal_mode(),
            )
            .decision,
        PolicyDecision::Allow
    );
}

#[test]
fn system_guardrail_denies_outside_workspace_before_user_allow() {
    let mut config = PolicyConfig::default();
    config
        .user_rules
        .push(user_config("outside-allow", PolicyDecision::Allow, "write"));
    let registry = ToolRegistry::normal_mode().with_test_scope(2, "outside_active_workspace");
    let engine = PolicyEngine::compile(&config, &registry);
    let result = engine.evaluate(
        &PolicyContext::new("write", SecurityMode::Normal, "outside_active_workspace"),
        &registry,
    );
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.reason, PolicyEvaluationReason::SystemGuardrail);
    assert_eq!(result.matched_rule_ids, ["system-deny-outside"]);
}

#[test]
fn stronger_alias_rule_is_not_lost() {
    assert_eq!(
        evaluate(vec![
            rule("allow", PolicyDecision::Allow, "active_workspace"),
            rule("deny", PolicyDecision::Deny, "workspace")
        ])
        .decision,
        PolicyDecision::Deny
    );
}

#[test]
fn source_and_editability_do_not_override_severity() {
    for source in [PolicySource::System, PolicySource::User] {
        for editable in [false, true] {
            let mut deny = rule("deny", PolicyDecision::Deny, "active_workspace");
            deny.source = source.clone();
            deny.editable = editable;
            let mut allow = rule("allow", PolicyDecision::Allow, "active_workspace");
            allow.source = match source {
                PolicySource::System => PolicySource::User,
                PolicySource::User => PolicySource::System,
            };
            allow.editable = !editable;
            assert_eq!(
                evaluate(vec![allow.clone(), deny.clone()]),
                evaluate(vec![deny.clone(), allow.clone()])
            );
            assert_eq!(evaluate(vec![allow, deny]).decision, PolicyDecision::Deny);
        }
    }
    let mut rules = PolicyEngine::system_rules();
    rules.push(rule("user-ask", PolicyDecision::Ask, "active_workspace"));
    assert_eq!(evaluate(rules).decision, PolicyDecision::Ask);
}

#[test]
fn duplicate_enabled_rule_ids_invalidate_the_engine_snapshot() {
    for decision in [
        PolicyDecision::Allow,
        PolicyDecision::Ask,
        PolicyDecision::Deny,
    ] {
        let result = evaluate(vec![
            rule("duplicate", PolicyDecision::Allow, "active_workspace"),
            rule("duplicate", decision, "outside_active_workspace"),
        ]);
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.reason, PolicyEvaluationReason::InvalidRuleSet);
        assert!(result.matched_rule_ids.is_empty());
    }
}

#[test]
fn empty_rule_ids_fail_closed_instead_of_losing_evidence() {
    for id in ["", " ", "\n"] {
        assert_eq!(
            evaluate(vec![rule(id, PolicyDecision::Allow, "active_workspace")]).reason,
            PolicyEvaluationReason::InvalidRuleSet
        );
    }
}

#[test]
fn disabled_duplicate_id_does_not_invalidate_enabled_rule() {
    let mut disabled = rule("same", PolicyDecision::Deny, "active_workspace");
    disabled.enabled = false;
    assert_eq!(
        evaluate(vec![
            disabled,
            rule("same", PolicyDecision::Allow, "active_workspace")
        ])
        .decision,
        PolicyDecision::Allow
    );
}

#[test]
fn system_baseline_is_fixed_and_repeatable() {
    let rules = PolicyEngine::system_rules();
    assert_eq!(rules, PolicyEngine::system_rules());
    assert_eq!(rules.len(), 2);
    assert!(rules
        .iter()
        .all(|rule| rule.source == PolicySource::System && !rule.editable && rule.enabled));
    let engine = PolicyEngine::new(rules.clone());
    let registry = ToolRegistry::normal_mode();
    let before = registry.list();
    let context = PolicyContext::new("read_file", SecurityMode::Normal, "active_workspace");
    let first = engine.evaluate(&context, &registry);
    assert_eq!(engine.evaluate(&context, &registry), first);
    assert_eq!(engine.evaluate(&context, &registry), first);
    assert_eq!(registry.list(), before);
    assert_eq!(rules, PolicyEngine::system_rules());
}

#[test]
fn bash_tool_matcher_is_supported_in_developer_mode_and_keeps_default_ask() {
    let registry = bash_registry();
    let mut config = PolicyConfig::default();
    config.user_rules.push(bash_rule(
        "bash-allow",
        PolicyDecision::Allow,
        PolicyMatcherConfig::tool("bash"),
    ));
    let engine = PolicyEngine::compile(&config, &registry);
    let catalog = engine.evaluate(
        &PolicyContext::new("bash", SecurityMode::DeveloperTest, "active_workspace"),
        &registry,
    );
    assert_eq!(catalog.decision, PolicyDecision::Allow);
    assert_eq!(catalog.matched_rule_ids, ["bash-allow"]);
}

#[test]
fn bash_command_matcher_requires_exact_executable_and_argv_prefix() {
    let mut config = PolicyConfig::default();
    config.user_rules.push(bash_rule(
        "git-status",
        PolicyDecision::Allow,
        PolicyMatcherConfig::bash_command("git.exe", vec!["status".into()]),
    ));
    assert_eq!(
        evaluate_bash(config.clone(), "git status --short"),
        PolicyEvaluation {
            decision: PolicyDecision::Allow,
            matched_rule_ids: vec!["git-status".into()],
            reason: PolicyEvaluationReason::MatchedUserRules,
        }
    );
    assert_eq!(
        evaluate_bash(config.clone(), "git statusx").decision,
        PolicyDecision::Ask
    );
    assert_eq!(
        evaluate_bash(config, "git stash").decision,
        PolicyDecision::Ask
    );
}

#[test]
fn bash_compound_policy_aggregates_highest_segment_severity_and_stable_ids() {
    let mut config = PolicyConfig::default();
    config.user_rules = vec![
        bash_rule(
            "git-status-allow",
            PolicyDecision::Allow,
            PolicyMatcherConfig::bash_command("git", vec!["status".into()]),
        ),
        bash_rule(
            "git-push-ask",
            PolicyDecision::Ask,
            PolicyMatcherConfig::bash_command("git", vec!["push".into()]),
        ),
    ];
    let result = evaluate_bash(config, "git status && git push");
    assert_eq!(result.decision, PolicyDecision::Ask);
    assert_eq!(
        result.matched_rule_ids,
        ["git-push-ask", "git-status-allow"]
    );
    assert_eq!(result.reason, PolicyEvaluationReason::MatchedUserRules);
}

#[test]
fn bash_command_with_no_match_uses_bash_default_ask() {
    assert_eq!(
        evaluate_bash(PolicyConfig::default(), "git status").decision,
        PolicyDecision::Ask
    );
}

#[test]
fn broad_bash_allow_cannot_lower_unclassified_command_floor() {
    let mut config = PolicyConfig::default();
    config.user_rules.push(bash_rule(
        "bash-allow",
        PolicyDecision::Allow,
        PolicyMatcherConfig::tool("bash"),
    ));
    let result = evaluate_bash(config, "echo $(whoami)");
    assert_eq!(result.decision, PolicyDecision::Ask);
    assert_eq!(result.matched_rule_ids, ["bash-allow"]);
}

#[test]
fn explicit_bash_deny_remains_deny_above_unclassified_floor() {
    let mut config = PolicyConfig::default();
    config.user_rules.push(bash_rule(
        "bash-deny",
        PolicyDecision::Deny,
        PolicyMatcherConfig::tool("bash"),
    ));
    let result = evaluate_bash(config, "echo $(whoami)");
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.matched_rule_ids, ["bash-deny"]);
}

#[test]
fn enabled_invalid_bash_matcher_invalidates_the_whole_snapshot() {
    let registry = bash_registry();
    let mut config = PolicyConfig::default();
    config.user_rules.push(bash_rule(
        "invalid-bash",
        PolicyDecision::Allow,
        PolicyMatcherConfig::bash_command("git status", vec![]),
    ));
    let engine = PolicyEngine::compile(&config, &registry);
    assert!(!engine.is_valid());
    let result = evaluate_bash(config, "git status");
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.reason, PolicyEvaluationReason::InvalidRuleSet);
}

#[test]
fn concrete_bash_request_without_or_with_empty_facts_fails_closed() {
    let registry = bash_registry();
    let engine = PolicyEngine::compile(&PolicyConfig::default(), &registry);
    let missing = engine.evaluate(
        &PolicyContext::new("bash", SecurityMode::DeveloperTest, "active_workspace")
            .require_bash_analysis(),
        &registry,
    );
    assert_eq!(missing.decision, PolicyDecision::Deny);
    assert_eq!(missing.reason, PolicyEvaluationReason::InvalidContext);

    let empty = BashCommandAnalysis {
        segments: Vec::new(),
        operators: Vec::new(),
        classification: crate::bash::analyzer::BashClassification::Classified,
    };
    let empty_result = engine.evaluate(
        &PolicyContext::new("bash", SecurityMode::DeveloperTest, "active_workspace")
            .with_bash_analysis(&empty),
        &registry,
    );
    assert_eq!(empty_result.decision, PolicyDecision::Deny);
    assert_eq!(empty_result.reason, PolicyEvaluationReason::InvalidContext);
}

#[test]
fn bash_effective_catalog_uses_tool_policy_without_command_rule_matches() {
    let registry = bash_registry();
    let mut config = PolicyConfig::default();
    config.user_rules.push(bash_rule(
        "git-status-allow",
        PolicyDecision::Allow,
        PolicyMatcherConfig::bash_command("git", vec!["status".into()]),
    ));
    let engine = PolicyEngine::compile(&config, &registry);
    let bash = engine
        .effective_snapshot(&registry)
        .per_tool
        .into_iter()
        .find(|tool| tool.tool == "bash")
        .expect("bash catalog row");
    assert_eq!(bash.decision, PolicyDecision::Ask);
    assert!(bash.matched_rule_ids.is_empty());
    assert_eq!(
        bash.availability,
        harness_control_protocol::PolicyToolAvailability::AvailableWithApproval
    );
}

#[test]
fn bash_effective_catalog_reports_unready_runtime_separately_from_policy() {
    let mut config = PolicyConfig::default();
    config.tool_defaults.bash = PolicyDecision::Allow;
    config.user_rules.push(bash_rule(
        "bash-allow",
        PolicyDecision::Allow,
        PolicyMatcherConfig::tool("bash"),
    ));
    let registry = ToolRegistry::from_config(&ToolConfig {
        bash: harness_control_protocol::ToolEnabledConfig { enabled: true },
        ..ToolConfig::default()
    });
    let engine = PolicyEngine::compile(&config, &registry);
    let bash = engine
        .effective_snapshot(&registry)
        .per_tool
        .into_iter()
        .find(|tool| tool.tool == "bash")
        .expect("bash catalog row");
    assert_eq!(
        bash.availability,
        harness_control_protocol::PolicyToolAvailability::UnavailableByRuntime
    );
}
