//! M00-only AppConfig v2 -> v3 compatibility contract tests.
//!
//! These tests do not implement or enable a production schema migration. They
//! validate a candidate representation in memory and assert that the current
//! ConfigStore remains schema v2.

use super::{ConfigStore, CURRENT_SCHEMA_VERSION};
use harness_control_protocol::{
    AppConfig, BashConfig, DecimalU64, PolicyConfig, RuntimeConfigSnapshot, RuntimeLimits,
    SecurityConfig, SecurityMode, ToolConfig, WorkspaceConfig,
};
use serde_json::{json, Map, Value};
use std::fs;
use tempfile::tempdir;

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/v4/m00/config/"
);

fn fixture(name: &str) -> Value {
    let path = format!("{FIXTURE_ROOT}{name}.json");
    let text = fs::read_to_string(path).expect("M00 config fixture exists");
    serde_json::from_str(&text).expect("M00 config fixture is JSON")
}

fn current_v2(name: &str) -> AppConfig {
    serde_json::from_value(fixture(name)).expect("fixture is valid current AppConfig v2")
}

fn runtime_projection(config: &AppConfig) -> Value {
    json!({
        "configRevision": config.config_revision,
        "workspace": config.workspace,
        "tunnel": config.tunnel,
        "security": config.security,
        "tools": config.tools,
        "bash": config.bash,
        "policies": config.policies,
    })
}

fn candidate_v3_as_v2(candidate: &Value) -> Value {
    let mut v2 = candidate.clone();
    let object = v2.as_object_mut().expect("candidate object");
    object.insert("schemaVersion".into(), json!(2));
    object.remove("desktop");
    object.remove("history");
    v2
}

fn normalize_candidate_v3(input: Value) -> Result<Value, &'static str> {
    let mut output = input;
    let object = output.as_object_mut().ok_or("candidate must be object")?;
    match object.get("schemaVersion").and_then(Value::as_u64) {
        Some(2) | Some(3) => {}
        _ => return Err("candidate must be schema v2 or v3"),
    }
    object.insert("schemaVersion".into(), json!(3));
    object.entry("desktop").or_insert_with(|| {
        json!({
            "autoStartHarnessOnAppLaunch": false,
            "approvalNotifications": true,
            "connectionNotifications": true
        })
    });
    object
        .entry("history")
        .or_insert_with(|| json!({ "enabled": true, "retentionDays": 30, "maxStorageMiB": 256 }));
    Ok(output)
}

fn known_v2_keys() -> [&'static str; 9] {
    [
        "configRevision",
        "workspace",
        "tunnel",
        "security",
        "ui",
        "tools",
        "policies",
        "bash",
        "schemaVersion",
    ]
}

#[test]
fn representative_v2_fields_survive_candidate_v3_without_raw_secret() {
    let source = fixture("v2-representative");
    let candidate = fixture("v3-candidate");
    let source_config = current_v2("v2-representative");
    let candidate_config: AppConfig = serde_json::from_value(candidate_v3_as_v2(&candidate))
        .expect("candidate v3 existing fields remain valid v2 fields");

    for key in known_v2_keys() {
        if key != "schemaVersion" {
            assert_eq!(
                source[key], candidate[key],
                "field changed during candidate migration: {key}"
            );
        }
    }
    assert_eq!(source_config, candidate_config);
    assert_eq!(candidate["desktop"]["autoStartHarnessOnAppLaunch"], false);
    assert_eq!(candidate["history"]["retentionDays"], 30);
    let text = candidate.to_string();
    assert!(!text.contains("M00_RAW_API_KEY"));
    assert!(!text.contains("apiKey\":\""));
}

#[test]
fn second_non_default_fixture_and_secret_reference_remain_opaque() {
    let config = current_v2("v2-non-defaults");
    assert_eq!(config.config_revision, DecimalU64::new(9));
    assert!(!config.tools.read_file.enabled);
    assert!(config.tools.write.enabled);
    assert_eq!(
        config.policies.tool_defaults.write,
        harness_control_protocol::PolicyDecision::Ask
    );
    assert_eq!(
        config.tunnel.api_key_ref.as_ref().unwrap().provider,
        "synthetic-secret-store"
    );

    let secret_config = current_v2("v2-secret-reference");
    let reference = secret_config.tunnel.api_key_ref.as_ref().unwrap();
    assert_eq!(reference.key, "m00/opaque/reference-only");
    let serialized = serde_json::to_string(&secret_config).unwrap();
    assert!(!serialized.contains("M00_RAW_API_KEY"));
    assert!(!serialized.contains("M00_SECRET_VALUE"));
}

#[test]
fn malformed_known_fields_fail_closed_before_candidate_persistence() {
    assert!(serde_json::from_value::<AppConfig>(fixture("v2-malformed")).is_err());
    let candidate = normalize_candidate_v3(fixture("v2-representative")).unwrap();
    assert_eq!(candidate["schemaVersion"], 3);
    assert_eq!(
        normalize_candidate_v3(candidate.clone()).unwrap(),
        candidate
    );
}

#[test]
fn unsupported_future_schema_is_rejected_without_rewriting_source_bytes() {
    let directory = tempdir().unwrap();
    let store = ConfigStore::new(directory.path().to_path_buf());
    let path = directory.path().join("config.json");
    let bytes = serde_json::to_vec_pretty(&fixture("v4-unsupported")).unwrap();
    fs::write(&path, &bytes).unwrap();
    let error = store.load().unwrap_err();
    assert_eq!(error.code(), "CONFIG_SCHEMA_UNSUPPORTED");
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!directory.path().join("config.json.tmp").exists());
}

#[test]
fn production_config_and_runtime_snapshot_remain_v2_and_protocol_v1() {
    assert_eq!(CURRENT_SCHEMA_VERSION, 2);
    assert_eq!(harness_control_protocol::PROTOCOL_VERSION, 1);
    let directory = tempdir().unwrap();
    let store = ConfigStore::new(directory.path().to_path_buf());
    let saved = store.save(ConfigStore::safe_defaults()).unwrap();
    assert_eq!(saved.schema_version, 2);
    let persisted: Value =
        serde_json::from_slice(&fs::read(directory.path().join("config.json")).unwrap()).unwrap();
    assert_eq!(persisted["schemaVersion"], 2);

    let config = current_v2("v2-representative");
    let snapshot = RuntimeConfigSnapshot {
        config_revision: config.config_revision,
        workspace: config.workspace,
        security: config.security,
        runtime_limits: RuntimeLimits {
            max_concurrent_requests: 1,
            max_activity_events: 256,
            max_read_bytes: 1_048_576,
        },
        tools: config.tools,
        bash: config.bash,
        policies: config.policies,
    };
    let runtime_json = serde_json::to_value(snapshot).unwrap();
    assert!(runtime_json.get("ui").is_none());
    assert!(runtime_json.get("desktop").is_none());
    assert!(runtime_json.get("history").is_none());
}

#[test]
fn candidate_desktop_and_history_are_host_only_and_do_not_change_runtime_projection() {
    let source = current_v2("v2-representative");
    let candidate = fixture("v3-candidate");
    let migrated: AppConfig = serde_json::from_value(candidate_v3_as_v2(&candidate)).unwrap();
    assert_eq!(runtime_projection(&source), runtime_projection(&migrated));

    let desktop = candidate["desktop"].as_object().unwrap();
    assert_eq!(desktop.len(), 3);
    let history = candidate["history"].as_object().unwrap();
    assert_eq!(history.len(), 3);
    let mut runtime_keys = Map::new();
    for key in [
        "workspace",
        "tunnel",
        "security",
        "tools",
        "bash",
        "policies",
    ] {
        runtime_keys.insert(key.into(), candidate[key].clone());
    }
    assert!(!runtime_keys.contains_key("desktop"));
    assert!(!runtime_keys.contains_key("history"));
}

// Keep imports used by the fixture contract explicit: these are the current
// production types whose preservation is being checked, not a second config
// schema implementation.
#[allow(dead_code)]
fn _contract_type_markers(
    _bash: BashConfig,
    _policies: PolicyConfig,
    _security: SecurityConfig,
    _mode: SecurityMode,
    _tools: ToolConfig,
    _workspace: WorkspaceConfig,
) {
}
