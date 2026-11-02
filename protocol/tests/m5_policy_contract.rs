use harness_control_protocol::{
    PolicyConfig, PolicyDecision, PolicyMatcherConfig, UserPolicyRuleConfig,
};
use serde_json::json;

#[test]
fn legacy_tool_matcher_and_new_bash_matcher_retain_exact_wire_shapes() {
    for value in [
        json!({"type":"tool", "tool":"write"}),
        json!({"type":"tool", "tool":"bash"}),
        json!({"type":"bash_command", "executable":"git", "argvPrefix":["status", "--short"]}),
    ] {
        let matcher: PolicyMatcherConfig = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(matcher).unwrap(), value);
    }
}

#[test]
fn enabled_bash_rules_are_saveable_without_changing_default_ask() {
    let mut policy = PolicyConfig::default();
    for (id, matcher) in [
        ("whole-bash", json!({"type":"tool", "tool":"bash"})),
        (
            "git-status",
            json!({"type":"bash_command", "executable":"git", "argvPrefix":["status"]}),
        ),
    ] {
        let mut rule = UserPolicyRuleConfig::new(
            id,
            id,
            PolicyDecision::Allow,
            "bash",
            "Reviewed command policy.",
        );
        rule.matcher = serde_json::from_value(matcher).unwrap();
        policy.user_rules.push(rule);
    }
    assert!(policy.validate_for_save().is_ok());
    assert_eq!(policy.tool_defaults.bash, PolicyDecision::Ask);
    let loaded: PolicyConfig =
        serde_json::from_value(serde_json::to_value(&policy).unwrap()).unwrap();
    assert_eq!(loaded, policy);
}

#[test]
fn enabled_bash_matcher_cannot_smuggle_shell_structure_into_executable() {
    for executable in [
        "",
        "   ",
        "git status",
        "git;shutdown",
        "$(git)",
        "git|cat",
        "git\0",
    ] {
        let matcher: PolicyMatcherConfig = serde_json::from_value(json!({
            "type":"bash_command", "executable": executable, "argvPrefix": ["status"]
        }))
        .unwrap();
        let mut rule = UserPolicyRuleConfig::new(
            "invalid-bash",
            "Invalid command",
            PolicyDecision::Allow,
            "bash",
            "Must fail closed.",
        );
        rule.matcher = matcher;
        let mut policy = PolicyConfig::default();
        policy.user_rules.push(rule);
        assert!(
            policy.validate_for_save().is_err(),
            "executable: {executable:?}"
        );
    }
}

#[test]
fn matcher_discriminator_and_variant_fields_are_closed() {
    for matcher in [
        json!({"type":"shell_expression", "command":"*"}),
        json!({"type":"tool", "tool":"bash", "executable":"git"}),
        json!({"type":"bash_command", "executable":"git", "argvPrefix":["status"], "tool":"write"}),
        json!({"type":"bash_command", "executable":"git", "argvPrefix":"status"}),
    ] {
        assert!(serde_json::from_value::<PolicyMatcherConfig>(matcher).is_err());
    }
}

#[test]
fn user_policy_matcher_accepts_grep_but_still_rejects_unknown_tools() {
    let mut policy = PolicyConfig::default();
    policy.user_rules.push(UserPolicyRuleConfig::new(
        "allow-grep",
        "Allow grep",
        PolicyDecision::Allow,
        "grep",
        "Reviewed content search.",
    ));
    assert!(policy.validate_for_save().is_ok());

    policy.user_rules[0].matcher = PolicyMatcherConfig::tool("unknown-tool");
    assert!(policy.validate_for_save().is_err());
}
