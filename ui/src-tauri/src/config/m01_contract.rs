//! M01 production AppConfig v2 -> v3 contract tests.

use super::{ConfigStore, DesktopPreferencesPatch, CURRENT_SCHEMA_VERSION};
use harness_control_protocol::{AppConfig, DecimalU64, RuntimeConfigSnapshot};
use serde_json::Value;
use std::fs;

const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/v4/m00/config/");

fn fixture(name: &str) -> Value {
    let path = format!("{FIXTURE_ROOT}{name}.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn v2_fixture_migrates_to_v3_with_frozen_desktop_and_history_defaults() {
    assert_eq!(CURRENT_SCHEMA_VERSION, 3);
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().into());
    let source = fixture("v2-representative");
    let source_revision = source["configRevision"].as_str().unwrap();
    fs::write(
        directory.path().join("config.json"),
        serde_json::to_vec_pretty(&source).unwrap(),
    )
    .unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.schema_version, 3);
    assert_eq!(
        loaded.config_revision,
        DecimalU64::new(source_revision.parse().unwrap())
    );
    assert!(loaded.desktop.approval_notifications);
    assert!(loaded.desktop.connection_notifications);
    assert!(!loaded.desktop.background_notice_acknowledged);
    assert!(!loaded.desktop.auto_start_harness_on_app_launch);
    assert_eq!(loaded.history.enabled, true);
    assert_eq!(loaded.history.retention_days, 30);
    assert_eq!(loaded.history.max_storage_mib, 256);
    assert!(
        !String::from_utf8_lossy(&fs::read(directory.path().join("config.json")).unwrap())
            .contains("raw")
    );
}

#[test]
fn desktop_preferences_are_host_only_and_do_not_enter_runtime_snapshot() {
    let config: AppConfig = serde_json::from_value(fixture("v3-candidate")).unwrap();
    let snapshot = RuntimeConfigSnapshot {
        config_revision: config.config_revision,
        workspace: config.workspace.clone(),
        security: config.security.clone(),
        runtime_limits: harness_control_protocol::RuntimeLimits {
            max_concurrent_requests: 1,
            max_activity_events: 1,
            max_read_bytes: 1,
        },
        tools: config.tools.clone(),
        bash: config.bash.clone(),
        policies: config.policies.clone(),
    };
    let serialized = serde_json::to_value(snapshot).unwrap();
    assert!(serialized.get("desktop").is_none());
    assert!(serialized.get("history").is_none());
}

#[test]
fn non_default_v2_values_and_secret_reference_survive_migration_without_raw_secret() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().into());
    let source = fixture("v2-non-defaults");
    fs::write(
        directory.path().join("config.json"),
        serde_json::to_vec_pretty(&source).unwrap(),
    )
    .unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.config_revision, DecimalU64::new(9));
    assert_eq!(loaded.workspace.root_path, "D:\\M00OtherWorkspace");
    assert_eq!(loaded.tunnel.tunnel_id, "m00-second-tunnel");
    assert_eq!(
        loaded.tunnel.api_key_ref.as_ref().unwrap().key,
        "m00-second-opaque-ref"
    );
    assert!(!loaded.tools.read_file.enabled);
    assert_eq!(
        loaded.policies.tool_defaults.write,
        harness_control_protocol::PolicyDecision::Ask
    );
    assert!(loaded.bash.auto_detect);
    let persisted =
        String::from_utf8(fs::read(directory.path().join("config.json")).unwrap()).unwrap();
    assert!(!persisted.contains("raw-secret"));
}

#[test]
fn desktop_mutation_is_focused_revision_neutral_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().into());
    let initial = store.save(ConfigStore::safe_defaults()).unwrap();
    let changed = store
        .set_desktop_preferences(DesktopPreferencesPatch {
            approval_notifications: Some(false),
            connection_notifications: Some(false),
            background_notice_acknowledged: Some(true),
            auto_start_harness_on_app_launch: None,
        })
        .unwrap();
    assert_eq!(changed.config_revision, initial.config_revision);
    assert!(!changed.desktop.approval_notifications);
    assert!(!changed.desktop.connection_notifications);
    assert!(changed.desktop.background_notice_acknowledged);
    assert!(!changed.desktop.auto_start_harness_on_app_launch);
    assert_eq!(store.load().unwrap().unwrap(), changed);
    let repeat = store
        .set_desktop_preferences(DesktopPreferencesPatch {
            approval_notifications: Some(false),
            connection_notifications: None,
            background_notice_acknowledged: None,
            auto_start_harness_on_app_launch: None,
        })
        .unwrap();
    assert_eq!(repeat, changed);
}

#[test]
fn malformed_v3_and_future_schema_are_rejected_without_rewriting_source() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().into());
    let mut malformed = fixture("v3-candidate");
    malformed["desktop"]["approvalNotifications"] = Value::String("secret-sentinel".into());
    malformed["history"]["maxStorageMiB"] = Value::String("secret-sentinel".into());
    for raw in [malformed, fixture("v4-unsupported")] {
        let bytes = serde_json::to_vec_pretty(&raw).unwrap();
        fs::write(directory.path().join("config.json"), &bytes).unwrap();
        let error = store.load().unwrap_err();
        assert!(matches!(
            error,
            super::ConfigError::Invalid(_) | super::ConfigError::UnsupportedSchema
        ));
        assert_eq!(
            fs::read(directory.path().join("config.json")).unwrap(),
            bytes
        );
    }
}
