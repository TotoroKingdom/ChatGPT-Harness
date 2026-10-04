use harness_control_protocol::{AppConfig, BashConfig, RuntimeConfigSnapshot};
use serde_json::{Value, json};

fn legacy_fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/v1")
        .join(name);
    let mut value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("bash");
    if value.get("schemaVersion").is_some() {
        value["schemaVersion"] = json!(1);
    }
    value
}

#[test]
fn legacy_v1_config_and_snapshot_receive_safe_bash_defaults() {
    let config: AppConfig =
        serde_json::from_value(legacy_fixture("app-config.example.json")).unwrap();
    let snapshot: RuntimeConfigSnapshot =
        serde_json::from_value(legacy_fixture("runtime-config-snapshot.example.json")).unwrap();
    assert_eq!(config.schema_version, 1);
    assert_eq!(config.bash, BashConfig::default());
    assert_eq!(snapshot.bash, BashConfig::default());
    assert!(config.bash.auto_detect);
    assert_eq!(config.bash.executable_path, None);
    assert!(!config.tools.bash.enabled);
    assert!(!snapshot.tools.bash.enabled);
    assert_eq!(
        config.policies.tool_defaults.bash,
        harness_control_protocol::PolicyDecision::Ask
    );
}

#[test]
fn manual_override_survives_config_and_snapshot_serialization_without_schema_bump() {
    let desired = BashConfig {
        auto_detect: false,
        executable_path: Some(r"C:\CustomGit\bin\bash.exe".into()),
    };
    let mut config: AppConfig =
        serde_json::from_value(legacy_fixture("app-config.example.json")).unwrap();
    let mut snapshot: RuntimeConfigSnapshot =
        serde_json::from_value(legacy_fixture("runtime-config-snapshot.example.json")).unwrap();
    config.bash = desired.clone();
    snapshot.bash = desired.clone();
    let reloaded_config: AppConfig =
        serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
    let reloaded_snapshot: RuntimeConfigSnapshot =
        serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert_eq!(reloaded_config.schema_version, 1);
    assert_eq!(reloaded_config.bash, desired);
    assert_eq!(reloaded_snapshot.bash, desired);
}

#[test]
fn bash_config_defaults_partial_values_and_rejects_unknown_authority_fields() {
    assert_eq!(
        serde_json::from_value::<BashConfig>(json!({})).unwrap(),
        BashConfig::default()
    );
    assert_eq!(
        serde_json::from_value::<BashConfig>(json!({"executablePath": null})).unwrap(),
        BashConfig::default()
    );
    for field in ["cwd", "timeout", "environment", "maxOutputBytes", "ready"] {
        assert!(
            serde_json::from_value::<BashConfig>(json!({field: true})).is_err(),
            "unexpected authority: {field}"
        );
    }
}
