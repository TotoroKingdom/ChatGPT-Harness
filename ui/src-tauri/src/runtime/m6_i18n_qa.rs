//! Independent M6 host/config and release-Runtime QA.
//!
//! The integration cases exercise the real `ConfigStore`.  The ignored local
//! black-box cases use the same release Runtime subprocess and loopback MCP
//! primitives as the M2 runner; they deliberately do not use a mock Runtime.

use super::m2_e2e::{self, McpClient, RuntimeProcess};
use super::runtime_config_snapshot;
use crate::config::{ConfigError, ConfigStore, CURRENT_SCHEMA_VERSION};
use harness_control_protocol::{AppConfig, DecimalU64, SecurityMode, UiLanguage};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

type QaResult<T> = Result<T, String>;

macro_rules! check {
    ($condition:expr $(,)?) => {
        if !$condition {
            return Err("M6 QA assertion failed".into());
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
    fn new(label: &str) -> QaResult<Self> {
        let base = std::env::temp_dir().join(format!(
            "chatgpt-harness-m6-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        let workspace = base.join("workspace");
        fs::create_dir_all(&workspace)
            .map_err(|error| format!("create M6 fixture workspace: {error}"))?;
        fs::write(workspace.join("baseline.txt"), b"m6 baseline")
            .map_err(|error| format!("create M6 read fixture: {error}"))?;
        Ok(Self { base, workspace })
    }

    fn config_dir(&self, name: &str) -> PathBuf {
        self.base.join(name)
    }

    fn config(&self, language: Option<UiLanguage>) -> AppConfig {
        let mut config = ConfigStore::safe_defaults();
        config.workspace.root_path = self.workspace.to_string_lossy().into_owned();
        config.ui.language = language;
        config
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn save(store: &ConfigStore, config: AppConfig) -> QaResult<AppConfig> {
    store
        .save(config)
        .map_err(|error| format!("ConfigStore save: {error:?}"))
}

fn load(store: &ConfigStore) -> QaResult<AppConfig> {
    store
        .load()
        .map_err(|error| format!("ConfigStore load: {error:?}"))?
        .ok_or_else(|| "ConfigStore returned no config".into())
}

fn evidence_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("artifacts")
        .join("M6-i18n")
        .join("qa")
        .join("host-runtime-evidence.md")
}

fn append_evidence(section: &str, lines: &[String]) {
    let path = evidence_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "\n## {section}");
        for line in lines {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn report_case(
    id: &str,
    result: QaResult<()>,
    failures: &mut Vec<String>,
    evidence: &mut Vec<String>,
) {
    match result {
        Ok(()) => {
            println!("{id} PASS");
            evidence.push(format!("- `{id}`: PASS"));
        }
        Err(error) => {
            println!("{id} FAIL: {error}");
            evidence.push(format!("- `{id}`: FAIL — {error}"));
            failures.push(id.to_owned());
        }
    }
}

#[test]
fn m6_host_integration_cases() {
    let fixture = Fixture::new("integration").expect("create M6 integration fixture");
    let mut failures = Vec::new();
    let mut evidence = vec![
        "- Harness: real `ConfigStore` integration test in `ui/src-tauri`".into(),
        format!("- Disposable fixture root: `{}`", fixture.base.display()),
    ];

    report_case("INT-01", int_01(&fixture), &mut failures, &mut evidence);
    report_case("INT-02", int_02(&fixture), &mut failures, &mut evidence);
    report_case("INT-03", int_03(&fixture), &mut failures, &mut evidence);
    report_case("INT-04", int_04(&fixture), &mut failures, &mut evidence);
    report_case("INT-05", int_05(&fixture), &mut failures, &mut evidence);
    report_case("INT-06", int_06(&fixture), &mut failures, &mut evidence);
    report_case("INT-07", int_07(&fixture), &mut failures, &mut evidence);
    report_case("INT-08", int_08(&fixture), &mut failures, &mut evidence);
    report_case("INT-09", int_09(&fixture), &mut failures, &mut evidence);
    report_case("INT-10", int_10(&fixture), &mut failures, &mut evidence);
    report_case("INT-11", int_11(&fixture), &mut failures, &mut evidence);
    report_case("INT-36", int_36(&fixture), &mut failures, &mut evidence);

    append_evidence("Host Integration INT-01..11, INT-36", &evidence);
    assert!(
        failures.is_empty(),
        "failed M6 host Integration cases: {failures:?}"
    );
}

fn int_01(fixture: &Fixture) -> QaResult<()> {
    let directory = fixture.config_dir("int-01");
    fs::create_dir_all(&directory).map_err(|error| format!("create INT-01 directory: {error}"))?;
    let mut config = fixture.config(None);
    config.tools.write.enabled = false;
    let mut value = serde_json::to_value(&config).map_err(|error| error.to_string())?;
    value["schemaVersion"] = serde_json::json!(1);
    value["ui"].as_object_mut().unwrap().remove("language");
    fs::write(
        directory.join("config.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .map_err(|error| format!("write INT-01 fixture: {error}"))?;
    let loaded = load(&ConfigStore::new(directory))?;
    check!(loaded.schema_version == CURRENT_SCHEMA_VERSION);
    check!(loaded.ui.language.is_none());
    check!(loaded.tools.write.enabled == config.tools.write.enabled);
    check!(loaded.workspace == config.workspace);
    let mut expected = config.clone();
    expected.tools.grep.enabled = false;
    check!(loaded == expected);
    Ok(())
}

fn int_02(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-02"));
    let initial = save(&store, fixture.config(None))?;
    check!(initial.ui.language.is_none());
    let english = store
        .set_ui_language(Some(UiLanguage::En))
        .map_err(|error| format!("save INT-02 en: {error:?}"))?;
    check!(english.ui.language == Some(UiLanguage::En));
    check!(load(&store)?.ui.language == Some(UiLanguage::En));
    let chinese = store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("save INT-02 zh-CN: {error:?}"))?;
    check!(chinese.ui.language == Some(UiLanguage::ZhCn));
    check!(load(&store)?.ui.language == Some(UiLanguage::ZhCn));
    let follow_system = store
        .set_ui_language(None)
        .map_err(|error| format!("save INT-02 follow system: {error:?}"))?;
    check!(follow_system.ui.language.is_none());
    check!(load(&store)?.ui.language.is_none());
    Ok(())
}

fn int_03(fixture: &Fixture) -> QaResult<()> {
    for (index, language) in ["fr", "zh-TW"].iter().enumerate() {
        let directory = fixture.config_dir(&format!("int-03-{index}"));
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let mut value = serde_json::to_value(fixture.config(None)).unwrap();
        value["ui"]["language"] = Value::String((*language).into());
        fs::write(
            directory.join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .map_err(|error| format!("write invalid {language}: {error}"))?;
        let result = ConfigStore::new(directory).load();
        check!(
            matches!(result, Err(ConfigError::Invalid(_))),
            "invalid persisted language {language} did not fail typed validation"
        );
    }
    Ok(())
}

fn int_04(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-04"));
    let initial = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let changed = store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("save INT-04 language: {error:?}"))?;
    check!(changed.ui.language == Some(UiLanguage::ZhCn));
    check!(changed.config_revision == initial.config_revision);
    check!(load(&store)?.config_revision == initial.config_revision);
    Ok(())
}

fn int_05(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-05"));
    let initial = save(&store, fixture.config(Some(UiLanguage::ZhCn)))?;
    let changed = store
        .set_ui_language(None)
        .map_err(|error| format!("save INT-05 follow system: {error:?}"))?;
    check!(changed.ui.language.is_none());
    check!(changed.config_revision == initial.config_revision);
    Ok(())
}

fn int_06(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-06"));
    let initial = save(&store, fixture.config(Some(UiLanguage::ZhCn)))?;
    let mut changed = initial.clone();
    changed.tools.write.enabled = !changed.tools.write.enabled;
    let changed = save(&store, changed)?;
    check!(changed.config_revision == DecimalU64::new(initial.config_revision.get() + 1));
    check!(changed.ui.language == Some(UiLanguage::ZhCn));
    Ok(())
}

fn int_07(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-07"));
    let initial = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let mut changed = initial.clone();
    changed.ui.language = Some(UiLanguage::ZhCn);
    changed.security.mode = SecurityMode::DeveloperTest;
    changed.security.developer_test_mode = true;
    let changed = save(&store, changed)?;
    check!(changed.config_revision == DecimalU64::new(initial.config_revision.get() + 1));
    let language_only = store
        .set_ui_language(Some(UiLanguage::En))
        .map_err(|error| format!("save INT-07 language-only follow-up: {error:?}"))?;
    check!(language_only.config_revision == changed.config_revision);
    Ok(())
}

fn int_08(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-08"));
    let active = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let mut runtime_change = active.clone();
    runtime_change.workspace.root_path = fixture
        .workspace
        .join("next")
        .to_string_lossy()
        .into_owned();
    let latest_before_language = save(&store, runtime_change)?;
    check!(latest_before_language.config_revision == DecimalU64::new(2));
    let latest_after_language = store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("save INT-08 language: {error:?}"))?;
    check!(latest_after_language.config_revision == latest_before_language.config_revision);
    check!(latest_after_language.config_revision != active.config_revision);
    // The persisted desired revision remains N+1 while active remains N:
    // this is the observable restart debt that a real Runtime E2E checks.
    let restart_required = latest_after_language.config_revision != active.config_revision;
    check!(restart_required);
    Ok(())
}

fn int_09(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-09"));
    let active = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let latest = store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("save INT-09 language: {error:?}"))?;
    check!(latest.config_revision == active.config_revision);
    check!(latest.config_revision == active.config_revision);
    let restart_required = latest.config_revision != active.config_revision;
    check!(!restart_required);
    Ok(())
}

fn int_10(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-10"));
    let mut persisted = fixture.config(Some(UiLanguage::En));
    persisted.workspace.root_path = fixture.workspace.to_string_lossy().into_owned();
    persisted.tunnel.tunnel_id = "persisted-tunnel".into();
    persisted.tools.write.enabled = false;
    persisted.bash.auto_detect = false;
    let persisted = save(&store, persisted)?;

    // These are unsaved Settings draft values and must never enter the
    // isolated language mutation transaction.
    let mut draft = persisted.clone();
    draft.workspace.root_path = "draft-workspace".into();
    draft.tunnel.tunnel_id = "draft-tunnel".into();
    draft.security.mode = SecurityMode::DeveloperTest;
    draft.bash.auto_detect = true;
    draft.tools.write.enabled = true;
    let saved = store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("save INT-10 language: {error:?}"))?;
    check!(saved.ui.language == Some(UiLanguage::ZhCn));
    check!(saved.workspace == persisted.workspace);
    check!(saved.tunnel == persisted.tunnel);
    check!(saved.security == persisted.security);
    check!(saved.bash == persisted.bash);
    check!(saved.tools == persisted.tools);
    check!(draft.workspace != saved.workspace && draft.tunnel != saved.tunnel);
    Ok(())
}

fn int_11(fixture: &Fixture) -> QaResult<()> {
    let store = ConfigStore::new(fixture.config_dir("int-11"));
    let original = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let target = fixture.config_dir("int-11").join("config.json");
    let original_bytes = fs::read(&target).map_err(|error| error.to_string())?;
    let tmp = fixture.config_dir("int-11").join("config.json.tmp");
    fs::create_dir(&tmp)
        .map_err(|error| format!("create deterministic INT-11 failure: {error}"))?;
    let result = store.set_ui_language(Some(UiLanguage::ZhCn));
    check!(
        matches!(result, Err(ConfigError::Io(_))),
        "INT-11 did not produce a deterministic ConfigStore write failure"
    );
    check!(fs::read(&target).unwrap() == original_bytes);
    check!(load(&store)? == original);
    fs::remove_dir(&tmp).map_err(|error| error.to_string())?;
    Ok(())
}

fn int_36(fixture: &Fixture) -> QaResult<()> {
    let mut english = fixture.config(Some(UiLanguage::En));
    english.config_revision = DecimalU64::new(17);
    let mut chinese = english.clone();
    chinese.ui.language = Some(UiLanguage::ZhCn);
    let english_snapshot = runtime_config_snapshot(&english);
    let chinese_snapshot = runtime_config_snapshot(&chinese);
    let english_wire = serde_json::to_value(&english_snapshot).unwrap();
    let chinese_wire = serde_json::to_value(&chinese_snapshot).unwrap();
    check!(english_snapshot == chinese_snapshot);
    check!(english_wire == chinese_wire);
    check!(english_wire.get("ui").is_none());
    check!(!serde_json::to_string(&english_wire)
        .unwrap()
        .contains("zh-CN"));
    check!(english_snapshot.tools == chinese_snapshot.tools);
    check!(english_snapshot.policies == chinese_snapshot.policies);
    println!(
        "INT-36 EVIDENCE: same RuntimeConfigSnapshot revision={} tools/policies; no ui/language wire field",
        english.config_revision.get()
    );
    Ok(())
}

fn e2e_start(
    binary: &Path,
    config: &AppConfig,
    generation: &str,
) -> QaResult<(RuntimeProcess, McpClient)> {
    m2_e2e::RuntimeProcess::start(binary, config, generation).map_err(|error| error.to_string())
}

fn e2e_stop(runtime: &mut RuntimeProcess) -> QaResult<()> {
    runtime.stop().map_err(|error| error.to_string())
}

fn status(runtime: &mut RuntimeProcess) -> QaResult<Value> {
    runtime
        .control_request("runtime.status", json!({}))
        .map_err(|error| error.to_string())
}

fn effective_tools(runtime: &mut RuntimeProcess) -> QaResult<Value> {
    runtime
        .control_request("control.effective_tools.get", json!({}))
        .map_err(|error| error.to_string())
}

fn policies(runtime: &mut RuntimeProcess) -> QaResult<Value> {
    runtime
        .control_request("policies.list", json!({}))
        .map_err(|error| error.to_string())
}

fn list_tools(mcp: &mut McpClient) -> QaResult<Value> {
    mcp.list_tools().map_err(|error| error.to_string())
}

fn read_baseline(mcp: &mut McpClient) -> QaResult<Value> {
    mcp.call_tool("read_file", json!({ "path": "baseline.txt" }))
        .map_err(|error| error.to_string())
}

fn result_payload(value: &Value) -> Value {
    value
        .get("result")
        .cloned()
        .unwrap_or_else(|| value.clone())
}

fn status_semantics(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("runtimeId");
        object.remove("startedAt");
    }
    value
}

fn bash_item(value: &Value) -> Option<Value> {
    value
        .as_array()?
        .iter()
        .find(|item| item.get("name") == Some(&Value::String("bash".into())))
        .cloned()
}

fn saved_revision(store: &ConfigStore) -> QaResult<DecimalU64> {
    Ok(load(store)?.config_revision)
}

#[test]
#[ignore = "M6 release Runtime black-box E2E requires an explicitly built release Runtime"]
fn m6_host_runtime_release_black_box_e2e() {
    let binary = match m2_e2e::runtime_binary() {
        Ok(binary) => binary,
        Err(error) => panic!("M6 release Runtime unavailable: {error}"),
    };
    println!("M6_RUNTIME_BINARY={}", binary.display());

    let mut failures = Vec::new();
    let mut evidence = vec![format!("- Release Runtime: `{}`", binary.display())];
    report_case("E2E-10", e2e_10(&binary), &mut failures, &mut evidence);
    report_case("E2E-11", e2e_11(&binary), &mut failures, &mut evidence);
    report_case("E2E-17", e2e_17(&binary), &mut failures, &mut evidence);
    report_case("E2E-18", e2e_18(&binary), &mut failures, &mut evidence);
    report_case("E2E-20", e2e_20(&binary), &mut failures, &mut evidence);
    report_case("E2E-22", e2e_22(&binary), &mut failures, &mut evidence);
    append_evidence(
        "Release Runtime E2E-10, E2E-11, E2E-17, E2E-18, E2E-20, E2E-22",
        &evidence,
    );
    assert!(
        failures.is_empty(),
        "failed M6 host Runtime E2E cases: {failures:?}"
    );
}

fn e2e_10(binary: &Path) -> QaResult<()> {
    let fixture = Fixture::new("e2e-10")?;
    let store = ConfigStore::new(fixture.config_dir("config"));
    let initial = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let (mut runtime, mut mcp) = e2e_start(binary, &initial, "M6-E2E-10")?;
    let before = status(&mut runtime)?;
    let pid = runtime.pid();
    let runtime_id = runtime.runtime_id().to_owned();
    let language = store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("E2E-10 language mutation: {error:?}"))?;
    let after = status(&mut runtime)?;
    let persisted_revision = saved_revision(&store)?;
    check!(language.config_revision == initial.config_revision);
    check!(persisted_revision == initial.config_revision);
    check!(before["configRevision"] == after["configRevision"]);
    check!(before["configRevision"] == json!(initial.config_revision.get().to_string()));
    check!(runtime.pid() == pid && runtime.runtime_id() == runtime_id);
    check!(result_payload(&list_tools(&mut mcp)?)
        .get("tools")
        .is_some());
    let restart_required = persisted_revision.get().to_string() != after["configRevision"];
    check!(!restart_required);
    println!(
        "E2E-10 EVIDENCE: configRevision={}→{} PID={} runtimeId={} restartRequired=false",
        before["configRevision"], after["configRevision"], pid, runtime_id
    );
    e2e_stop(&mut runtime)
}

fn e2e_11(binary: &Path) -> QaResult<()> {
    let fixture = Fixture::new("e2e-11")?;
    let store = ConfigStore::new(fixture.config_dir("config"));
    let initial = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let (mut runtime, mut mcp) = e2e_start(binary, &initial, "M6-E2E-11")?;
    let active = status(&mut runtime)?;
    let pid = runtime.pid();
    let runtime_id = runtime.runtime_id().to_owned();
    let mut runtime_change = initial.clone();
    runtime_change.tools.write.enabled = false;
    let desired = save(&store, runtime_change)?;
    check!(desired.config_revision == DecimalU64::new(initial.config_revision.get() + 1));
    let before_language = status(&mut runtime)?;
    let after_language_config = store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("E2E-11 language mutation: {error:?}"))?;
    let after = status(&mut runtime)?;
    let latest = load(&store)?;
    check!(active["configRevision"] == json!(initial.config_revision.get().to_string()));
    check!(before_language["configRevision"] == active["configRevision"]);
    check!(after["configRevision"] == active["configRevision"]);
    check!(latest.config_revision == desired.config_revision);
    check!(after_language_config.config_revision == desired.config_revision);
    check!(
        latest.config_revision
            != DecimalU64::new(after["configRevision"].as_str().unwrap().parse().unwrap())
    );
    check!(runtime.pid() == pid && runtime.runtime_id() == runtime_id);
    // The active generation remains the old one, including its live MCP
    // catalog; language persistence did not perform an implicit restart.
    let tools = result_payload(&list_tools(&mut mcp)?);
    check!(tools["tools"]
        .as_array()
        .is_some_and(|items| { items.iter().any(|item| item["name"] == json!("write")) }));
    println!(
        "E2E-11 EVIDENCE: active={} latest={} restartRequired=true PID={} runtimeId={} generation unchanged",
        after["configRevision"], latest.config_revision.get(), pid, runtime_id
    );
    e2e_stop(&mut runtime)
}

fn e2e_17(binary: &Path) -> QaResult<()> {
    let fixture = Fixture::new("e2e-17")?;
    let store = ConfigStore::new(fixture.config_dir("config"));
    let english = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let mut chinese = english.clone();
    chinese.ui.language = Some(UiLanguage::ZhCn);
    let english_snapshot = serde_json::to_value(runtime_config_snapshot(&english)).unwrap();
    let chinese_snapshot = serde_json::to_value(runtime_config_snapshot(&chinese)).unwrap();
    check!(english_snapshot == chinese_snapshot);
    check!(english_snapshot.get("ui").is_none());
    check!(!serde_json::to_string(&english_snapshot)
        .unwrap()
        .contains("zh-CN"));

    let (mut en_runtime, mut en_mcp) = e2e_start(binary, &english, "M6-E2E-17-en")?;
    let en_status = status(&mut en_runtime)?;
    let en_effective = effective_tools(&mut en_runtime)?;
    let en_policies = policies(&mut en_runtime)?;
    let en_tools = list_tools(&mut en_mcp)?;
    let en_read = read_baseline(&mut en_mcp)?;
    e2e_stop(&mut en_runtime)?;

    let (mut zh_runtime, mut zh_mcp) = e2e_start(binary, &chinese, "M6-E2E-17-zh")?;
    let zh_status = status(&mut zh_runtime)?;
    let zh_effective = effective_tools(&mut zh_runtime)?;
    let zh_policies = policies(&mut zh_runtime)?;
    let zh_tools = list_tools(&mut zh_mcp)?;
    let zh_read = read_baseline(&mut zh_mcp)?;
    check!(status_semantics(en_status) == status_semantics(zh_status));
    check!(en_effective == zh_effective);
    check!(en_policies == zh_policies);
    check!(result_payload(&en_tools) == result_payload(&zh_tools));
    check!(result_payload(&en_read) == result_payload(&zh_read));
    e2e_stop(&mut zh_runtime)?;
    println!(
        "E2E-17 EVIDENCE: identical en/zh snapshot and Runtime status/effective/policies/read behavior; ui.language absent from initialize snapshot"
    );
    Ok(())
}

fn e2e_18(binary: &Path) -> QaResult<()> {
    let fixture = Fixture::new("e2e-18")?;
    let store = ConfigStore::new(fixture.config_dir("config"));
    let english = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let mut chinese = english.clone();
    chinese.ui.language = Some(UiLanguage::ZhCn);

    let (mut en_runtime, mut en_mcp) = e2e_start(binary, &english, "M6-E2E-18-en")?;
    let en_tools = list_tools(&mut en_mcp)?;
    let en_read = read_baseline(&mut en_mcp)?;
    let en_effective = effective_tools(&mut en_runtime)?;
    e2e_stop(&mut en_runtime)?;
    let (mut zh_runtime, mut zh_mcp) = e2e_start(binary, &chinese, "M6-E2E-18-zh")?;
    let zh_tools = list_tools(&mut zh_mcp)?;
    let zh_read = read_baseline(&mut zh_mcp)?;
    let zh_effective = effective_tools(&mut zh_runtime)?;
    check!(result_payload(&en_tools) == result_payload(&zh_tools));
    check!(result_payload(&en_read) == result_payload(&zh_read));
    check!(en_effective == zh_effective);
    e2e_stop(&mut zh_runtime)?;
    println!(
        "E2E-18 EVIDENCE: loopback MCP tools/list tool names+schemas, read_file result, and effective tools identical for en/zh"
    );
    Ok(())
}

fn e2e_20(binary: &Path) -> QaResult<()> {
    let fixture = Fixture::new("e2e-20")?;
    let store = ConfigStore::new(fixture.config_dir("config"));
    let bash_path = std::env::var_os("HARNESS_E2E_BASH_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"));
    check!(
        bash_path.is_file(),
        "real Git Bash executable is unavailable at {}",
        bash_path.display()
    );
    let mut initial = fixture.config(Some(UiLanguage::En));
    initial.security.mode = SecurityMode::DeveloperTest;
    initial.security.developer_test_mode = true;
    initial.tools.bash.enabled = true;
    initial.bash.auto_detect = false;
    initial.bash.executable_path = Some(bash_path.to_string_lossy().into_owned());
    let initial = save(&store, initial)?;
    let (mut runtime, mut mcp) = e2e_start(binary, &initial, "M6-E2E-20")?;
    let pid = runtime.pid();
    let before = effective_tools(&mut runtime)?;
    let before_bash = bash_item(&before)
        .ok_or_else(|| "Bash effective item missing before language switch".to_owned())?;
    check!(before_bash["availability"] == json!("available_with_approval"));
    check!(before_bash["readiness"] == json!("ready"));
    let before_policies = policies(&mut runtime)?;
    let before_read = read_baseline(&mut mcp)?;
    store
        .set_ui_language(Some(UiLanguage::ZhCn))
        .map_err(|error| format!("E2E-20 language mutation: {error:?}"))?;
    let after = effective_tools(&mut runtime)?;
    let after_bash = bash_item(&after)
        .ok_or_else(|| "Bash effective item missing after language switch".to_owned())?;
    let after_policies = policies(&mut runtime)?;
    let after_read = read_baseline(&mut mcp)?;
    check!(before == after);
    check!(before_bash == after_bash);
    check!(before_policies == after_policies);
    check!(result_payload(&before_read) == result_payload(&after_read));
    check!(runtime.pid() == pid);
    println!(
        "E2E-20 EVIDENCE: real Git Bash {} available_with_approval/ready and policy data unchanged before/after language switch; PID={}",
        bash_path.display(),
        pid
    );
    e2e_stop(&mut runtime)
}

fn e2e_22(binary: &Path) -> QaResult<()> {
    let fixture = Fixture::new("e2e-22")?;
    let store = ConfigStore::new(fixture.config_dir("config"));
    let initial = save(&store, fixture.config(Some(UiLanguage::En)))?;
    let (mut runtime, mut mcp) = e2e_start(binary, &initial, "M6-E2E-22")?;
    let pid = runtime.pid();
    let runtime_id = runtime.runtime_id().to_owned();
    let _ = list_tools(&mut mcp)?;
    for language in [Some(UiLanguage::ZhCn), Some(UiLanguage::En), None] {
        store
            .set_ui_language(language)
            .map_err(|error| format!("E2E-22 language mutation: {error:?}"))?;
        let state = status(&mut runtime)?;
        check!(state["configRevision"] == json!(initial.config_revision.get().to_string()));
        check!(runtime.pid() == pid && runtime.runtime_id() == runtime_id);
    }
    e2e_stop(&mut runtime)?;
    check!(
        runtime.stopped_and_endpoint_closed(),
        "E2E-22 Runtime process or loopback endpoint remained alive after stop"
    );
    let persisted = load(&store)?;
    check!(persisted.ui.language.is_none());
    check!(persisted.config_revision == initial.config_revision);
    println!(
        "E2E-22 EVIDENCE: Runtime exited and loopback endpoint closed; no extra generation/process (PID={} runtimeId={}); config reload succeeded",
        pid, runtime_id
    );
    Ok(())
}
