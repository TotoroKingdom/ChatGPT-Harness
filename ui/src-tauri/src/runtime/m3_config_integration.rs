use super::runtime_config_snapshot;
use crate::config::ConfigStore;
use harness_control_protocol::{
    AppConfig, DecimalU64, PolicyDecision, PolicyMatcherConfig, UserPolicyRuleConfig,
};
use serde_json::{json, Value};
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
}

impl Fixture {
    fn new() -> QaResult<Self> {
        let base = std::env::temp_dir().join(format!(
            "chatgpt-harness-m3-config-integration-{}",
            uuid::Uuid::new_v4()
        ));
        let workspace = base.join("workspace");
        fs::create_dir_all(&workspace).map_err(|_| "create config integration workspace failed")?;
        Ok(Self { base, workspace })
    }

    fn config_dir(&self, name: &str) -> PathBuf {
        self.base.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn base_config(fixture: &Fixture) -> AppConfig {
    let mut config = ConfigStore::safe_defaults();
    config.workspace.root_path = fixture.workspace.to_string_lossy().into_owned();
    config
}

fn user_rule(
    id: &str,
    decision: PolicyDecision,
    tool: &str,
    enabled: bool,
) -> UserPolicyRuleConfig {
    UserPolicyRuleConfig {
        id: id.into(),
        name: format!("Config integration {id}"),
        enabled,
        decision,
        matcher: PolicyMatcherConfig::tool(tool),
        reason: "Config integration fixture.".into(),
    }
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
fn m3_config_integration_cases() {
    let fixture = Fixture::new().expect("create M3 config integration fixture");
    let mut failures = Vec::new();
    report_case("INT-01", int_01(&fixture), &mut failures);
    report_case("INT-02", int_02(&fixture), &mut failures);
    report_case("INT-03", int_03(&fixture), &mut failures);
    report_case("INT-04", int_04(&fixture), &mut failures);
    report_case("INT-05", int_05(&fixture), &mut failures);
    assert!(
        failures.is_empty(),
        "failed M3 config Integration cases: {failures:?}"
    );
}

fn int_01(fixture: &Fixture) -> QaResult<()> {
    let directory = fixture.config_dir("int-01");
    fs::create_dir_all(&directory).map_err(|_| "create INT-01 config directory")?;
    let mut value =
        serde_json::to_value(base_config(fixture)).map_err(|_| "encode INT-01 config")?;
    value.as_object_mut().unwrap().remove("policies");
    fs::write(
        directory.join("config.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .map_err(|_| "write INT-01 legacy config")?;
    let loaded = ConfigStore::new(directory)
        .load()
        .map_err(|_| "load INT-01 config")?
        .ok_or_else(|| "INT-01 config missing".to_owned())?;
    check!(loaded.schema_version == 1);
    check!(loaded.policies.tool_defaults.read_file == PolicyDecision::Allow);
    check!(loaded.policies.tool_defaults.glob == PolicyDecision::Allow);
    check!(loaded.policies.tool_defaults.write == PolicyDecision::Allow);
    check!(loaded.policies.tool_defaults.edit == PolicyDecision::Allow);
    check!(loaded.policies.tool_defaults.bash == PolicyDecision::Ask);
    check!(loaded.policies.user_rules.is_empty());
    check!(loaded.tools == base_config(fixture).tools);
    Ok(())
}

fn int_02(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-02"));
    let first = store
        .save(base_config(fixture))
        .map_err(|_| "save INT-02 defaults")?;
    check!(first.config_revision == DecimalU64::new(1));
    let mut changed = first.clone();
    changed.policies.tool_defaults.write = PolicyDecision::Deny;
    let changed = store
        .save(changed)
        .map_err(|_| "save INT-02 default change")?;
    check!(changed.config_revision == DecimalU64::new(2));
    check!(store.load().unwrap().unwrap().policies.tool_defaults.write == PolicyDecision::Deny);
    check!(store.save(changed.clone()).unwrap().config_revision == DecimalU64::new(2));
    let mut with_rule = changed.clone();
    with_rule.policies.user_rules.push(user_rule(
        "revision-rule",
        PolicyDecision::Allow,
        "write",
        true,
    ));
    let with_rule = store
        .save(with_rule)
        .map_err(|_| "save INT-02 create rule")?;
    check!(with_rule.config_revision == DecimalU64::new(3));
    let mut disabled = with_rule.clone();
    disabled.policies.user_rules[0].enabled = false;
    let disabled = store
        .save(disabled)
        .map_err(|_| "save INT-02 disable rule")?;
    check!(disabled.config_revision == DecimalU64::new(4));
    let mut deleted = disabled.clone();
    deleted.policies.user_rules.clear();
    let deleted = store.save(deleted).map_err(|_| "save INT-02 delete rule")?;
    check!(deleted.config_revision == DecimalU64::new(5));
    Ok(())
}

fn int_03(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-03"));
    let mut config = base_config(fixture);
    config
        .policies
        .user_rules
        .push(user_rule("crud-rule", PolicyDecision::Allow, "write", true));
    let saved = store.save(config).map_err(|_| "save INT-03 create")?;
    let mut edited = saved.clone();
    edited.policies.user_rules[0].name = "edited name".into();
    edited.policies.user_rules[0].reason = "edited reason".into();
    edited.policies.user_rules[0].decision = PolicyDecision::Ask;
    edited.policies.user_rules[0].enabled = false;
    edited.policies.user_rules[0].matcher = PolicyMatcherConfig::tool("edit");
    let edited = store.save(edited).map_err(|_| "save INT-03 edit")?;
    let loaded = store.load().unwrap().unwrap();
    let rule = &loaded.policies.user_rules[0];
    check!(rule.id == "crud-rule");
    check!(rule.name == "edited name" && rule.reason == "edited reason");
    check!(rule.decision == PolicyDecision::Ask && !rule.enabled);
    check!(rule.matcher.tool == "edit");
    let wire = serde_json::to_value(&edited).unwrap();
    check!(wire["policies"]["userRules"][0].get("source").is_none());
    check!(wire["policies"]["userRules"][0].get("editable").is_none());
    let mut deleted = edited;
    deleted.policies.user_rules.clear();
    store.save(deleted).map_err(|_| "save INT-03 delete")?;
    check!(store
        .load()
        .unwrap()
        .unwrap()
        .policies
        .user_rules
        .is_empty());
    Ok(())
}

fn int_04(fixture: &Fixture) -> QaResult<()> {
    let directory = fixture.config_dir("int-04");
    let store = ConfigStore::new(directory.clone());
    let config = store
        .save(base_config(fixture))
        .map_err(|_| "save INT-04 baseline")?;
    let original = fs::read(directory.join("config.json")).map_err(|_| "read INT-04 baseline")?;
    let malformed = [
        (
            "invalid decision",
            json!({"policies":{"toolDefaults":{"readFile":"allow","glob":"allow","write":"invalid","edit":"allow","bash":"ask"},"userRules":[]}}),
        ),
        (
            "malformed matcher",
            json!({"policies":{"toolDefaults":{"readFile":"allow","glob":"allow","write":"allow","edit":"allow","bash":"ask"},"userRules":[{"id":"bad","name":"bad","enabled":true,"decision":"allow","matcher":true,"reason":"bad"}]}}),
        ),
        (
            "wrong field type",
            json!({"policies":{"toolDefaults":{"readFile":"allow","glob":"allow","write":"allow","edit":"allow","bash":"ask"},"userRules":[{"id":"bad","name":"bad","enabled":"true","decision":"allow","matcher":{"type":"tool","tool":"write"},"reason":"bad"}]}}),
        ),
        (
            "unknown matcher discriminator",
            json!({"policies":{"toolDefaults":{"readFile":"allow","glob":"allow","write":"allow","edit":"allow","bash":"ask"},"userRules":[{"id":"bad","name":"bad","enabled":true,"decision":"allow","matcher":{"type":"scope","tool":"write"},"reason":"bad"}]}}),
        ),
    ];
    for (name, policies) in malformed {
        let mut value: Value = serde_json::from_slice(&original).unwrap();
        value["policies"] = policies["policies"].clone();
        fs::write(
            directory.join("config.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .map_err(|_| format!("write INT-04 {name}"))?;
        check!(store.load().is_err(), "INT-04 {name} loaded unexpectedly");
        fs::write(directory.join("config.json"), &original).map_err(|_| "restore INT-04")?;
    }
    let mut invalid_save = config;
    invalid_save.policies.user_rules.push(user_rule(
        "invalid-save",
        PolicyDecision::Allow,
        "bash",
        true,
    ));
    check!(store.save(invalid_save).is_err());
    check!(fs::read(directory.join("config.json")).unwrap() == original);
    Ok(())
}

fn int_05(fixture: &Fixture) -> QaResult<()> {
    let mut config = base_config(fixture);
    config.config_revision = DecimalU64::new(41);
    config.tunnel.api_key_ref = Some(harness_control_protocol::SecretReference {
        provider: "memory".into(),
        key: "opaque-secret-locator".into(),
    });
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    config.policies.user_rules.push(user_rule(
        "snapshot-rule",
        PolicyDecision::Deny,
        "write",
        true,
    ));
    let snapshot = runtime_config_snapshot(&config);
    check!(snapshot.config_revision == DecimalU64::new(41));
    check!(snapshot.policies == config.policies);
    check!(snapshot.tools == config.tools);
    let encoded = serde_json::to_string(&snapshot).unwrap();
    check!(!encoded.contains("tunnel"));
    check!(!encoded.contains("opaque-secret-locator"));
    Ok(())
}
