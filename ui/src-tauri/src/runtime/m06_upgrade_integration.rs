//! M06 upgrade/rollback contract checks.
//!
//! These cases use the product ConfigStore migration path, representative
//! fixtures and the public evidence-store API. They do not launch the app,
//! mutate the user's AppData, or synthesize an older product binary.

use crate::config::ConfigStore;
use chatgpt_harness_evidence_store::{
    EvidenceEventV1, EvidenceReader, EvidenceWriter, RuntimeSession, WriterOptions,
};
use harness_control_protocol::{
    AppConfig, DesktopConfig, HistoryConfig, HistoryQueryParams, HistoryTerminalState,
    PolicyDecision,
};
use std::fs;
use std::path::{Path, PathBuf};

type UpgradeResult<T> = Result<T, String>;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("v4")
        .join("m06")
        .join("upgrade")
}

fn fixture(name: &str) -> PathBuf {
    fixture_root().join(name)
}

fn install_config(directory: &Path, name: &str) -> UpgradeResult<Vec<u8>> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let bytes = fs::read(fixture(name)).map_err(|error| error.to_string())?;
    fs::write(directory.join("config.json"), &bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn load(directory: &Path) -> UpgradeResult<AppConfig> {
    ConfigStore::new(directory.to_path_buf())
        .load()
        .map_err(|error| format!("{}: {error:?}", error.code()))?
        .ok_or_else(|| "config was not created".to_owned())
}

fn temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "chatgpt-harness-m06-{prefix}-{}",
        uuid::Uuid::new_v4()
    ))
}

fn assert_v2_upgrade(directory: &Path) -> UpgradeResult<()> {
    let config = load(directory)?;
    if config.schema_version != 3 || u64::from(config.config_revision) != 41 {
        return Err("v2 upgrade did not produce v3 revision 41".into());
    }
    if config.workspace.root_path != "C:\\M06UpgradeWorkspace"
        || config.tunnel.tunnel_id != "m06-upgrade-tunnel"
        || config.tunnel.control_plane_http_proxy.as_deref() != Some("http://127.0.0.1:7890")
    {
        return Err("v2 user values were not preserved".into());
    }
    let reference = config
        .tunnel
        .api_key_ref
        .as_ref()
        .ok_or_else(|| "SecretReference was not preserved".to_owned())?;
    if reference.provider != "windows-credential-manager"
        || reference.key != "m06/upgrade/opaque-tunnel-key"
    {
        return Err("SecretReference changed during migration".into());
    }
    if config.tools.write.enabled
        || !config.tools.edit.enabled
        || config.tools.bash.enabled
        || config.tools.grep.enabled
    {
        return Err("v2 Tool toggles or pre-M05 grep semantics changed".into());
    }
    if config.policies.tool_defaults.write != PolicyDecision::Deny
        || config.policies.tool_defaults.grep != PolicyDecision::Allow
        || config.policies.user_rules.len() != 1
        || config.policies.user_rules[0].id != "m06-upgrade-write-deny"
    {
        return Err("v2 Policy settings were not preserved".into());
    }
    if config.desktop != DesktopConfig::default() || config.history != HistoryConfig::default() {
        return Err("v2 host-only defaults were not applied additively".into());
    }
    Ok(())
}

fn assert_v3_pre_core_upgrade(directory: &Path) -> UpgradeResult<()> {
    let config = load(directory)?;
    if u64::from(config.config_revision) != 47 || config.tools.grep.enabled {
        return Err("pre-Core v3 grep normalization changed the revision or exposure".into());
    }
    if config.desktop.approval_notifications
        || !config.desktop.connection_notifications
        || !config.desktop.background_notice_acknowledged
    {
        return Err("pre-Core desktop preferences were not preserved".into());
    }
    if config.history.enabled
        || config.history.retention_days != 7
        || config.history.max_storage_mib != 64
    {
        return Err("pre-Core History settings were not preserved".into());
    }
    Ok(())
}

fn assert_v3_grep_enabled(directory: &Path) -> UpgradeResult<()> {
    let config = load(directory)?;
    if u64::from(config.config_revision) != 48
        || !config.tools.grep.enabled
        || config.policies.tool_defaults.grep != PolicyDecision::Allow
    {
        return Err("explicit M05 grep enablement was not preserved".into());
    }
    if config.history.retention_days != 90 || config.history.max_storage_mib != 512 {
        return Err("v3 History settings were not preserved".into());
    }
    Ok(())
}

fn assert_invalid_config_preserved(directory: &Path, original: &[u8]) -> UpgradeResult<()> {
    let store = ConfigStore::new(directory.to_path_buf());
    if store.load().is_ok() {
        return Err("invalid v3 config was silently accepted".into());
    }
    let after = fs::read(directory.join("config.json")).map_err(|error| error.to_string())?;
    if after != original {
        return Err("invalid config bytes were rewritten".into());
    }
    Ok(())
}

fn write_pending_history(database: &Path) -> UpgradeResult<()> {
    let raw = fs::read(fixture("history-pending-event.json")).map_err(|error| error.to_string())?;
    let event: EvidenceEventV1 =
        serde_json::from_slice(&raw).map_err(|error| format!("history fixture: {error}"))?;
    let mut writer = EvidenceWriter::open_with_options(
        database,
        WriterOptions {
            history_config: HistoryConfig::default(),
            runtime_version: "m06-upgrade-test".into(),
            config_revision: "41".into(),
        },
    )
    .map_err(|error| error.to_string())?;
    writer
        .open_session(&RuntimeSession {
            runtime_id: event.runtime_id.clone(),
            host_session_id: Some("m06-host-session".into()),
            supervisor_generation_id: Some("m06-generation".into()),
            started_at_utc: event.occurred_at_utc.clone(),
            runtime_version: "m06-upgrade-test".into(),
            config_revision: "41".into(),
        })
        .map_err(|error| error.to_string())?;
    writer
        .record_event(&event)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_history_observational(database: &Path) -> UpgradeResult<()> {
    let reader = EvidenceReader::open(database).map_err(|error| error.to_string())?;
    let result = reader
        .query(&HistoryQueryParams::default())
        .map_err(|error| error.to_string())?;
    if result.items.len() != 1 {
        return Err("M03-compatible History was not readable after upgrade".into());
    }
    let item = &result.items[0];
    if item.runtime_id != "m06-upgrade-runtime"
        || item.request_id != "m06-upgrade-request-1"
        || item.tool_name != "write"
    {
        return Err("History identity changed during upgrade".into());
    }
    if item.terminal_state != HistoryTerminalState::Open
        || item.approval_state != harness_control_protocol::HistoryApprovalState::RequiredObserved
    {
        return Err("pending history was not preserved as observational state".into());
    }
    if format!("{:?}", item.approval_state).contains("Allow") {
        return Err("History resurrected a SessionGrant/Approval decision".into());
    }
    Ok(())
}

#[test]
fn m06_upgrade_and_rollback_contract_cases() {
    let root = temp_dir("upgrade");
    fs::create_dir_all(&root).expect("create M06 upgrade temp root");
    let result = (|| -> UpgradeResult<()> {
        let v2_dir = root.join("v2");
        let v2_original = install_config(&v2_dir, "v2-representative.json")?;
        assert_v2_upgrade(&v2_dir)?;

        let v3_dir = root.join("v3-pre-core");
        install_config(&v3_dir, "v3-pre-core.json")?;
        assert_v3_pre_core_upgrade(&v3_dir)?;

        let grep_dir = root.join("v3-grep");
        install_config(&grep_dir, "v3-grep-enabled.json")?;
        assert_v3_grep_enabled(&grep_dir)?;

        let invalid_dir = root.join("invalid");
        let invalid_original = install_config(&invalid_dir, "v3-invalid-decision.json")?;
        assert_invalid_config_preserved(&invalid_dir, &invalid_original)?;

        let history_dir = root.join("history");
        fs::create_dir_all(&history_dir).map_err(|error| error.to_string())?;
        let database = history_dir.join("history.sqlite3");
        write_pending_history(&database)?;
        assert_history_observational(&database)?;

        // Rollback prep: the pre-upgrade config backup remains byte-identical
        // and can be restored without reconstructing secrets or deleting History.
        fs::write(v2_dir.join("config.json"), &v2_original).map_err(|error| error.to_string())?;
        assert_v2_upgrade(&v2_dir)?;
        assert_history_observational(&database)?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&root);
    result.expect("M06 upgrade and rollback contract");
}
