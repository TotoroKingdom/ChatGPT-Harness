use super::*;
use crate::tools::ToolRegistry;
use harness_control_protocol::{PolicyDecision, PolicyRule, PolicySource, SecurityMode};

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
        for tool in registry.list() {
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
    for tool in registry.list() {
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
    for tool in registry.list() {
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
