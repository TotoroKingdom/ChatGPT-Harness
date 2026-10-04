//! M7 local black-box E2E.
//!
//! This runner deliberately uses the production `ConfigStore` boundary and
//! the release Runtime subprocess.  It is ignored by default because the
//! Runtime binary must be built explicitly before the test is run.

use super::m2_e2e::{self, McpClient, PendingMcpCall, RuntimeProcess};
use crate::config::ConfigStore;
use harness_control_protocol::{AppConfig, SecurityMode, UiLanguage};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const POLL_TIMEOUT: Duration = Duration::from_secs(5);
const SECRET_SENTINEL: &str = "M7_SECRET_SENTINEL_DO_NOT_LEAK";

static OWNED_BASH_PIDS: OnceLock<Mutex<Vec<u32>>> = OnceLock::new();
static CURRENT_CASE_ID: OnceLock<Mutex<String>> = OnceLock::new();

type M7Result<T> = Result<T, String>;

macro_rules! check {
    ($condition:expr $(,)?) => {
        if !$condition {
            return Err("M7 E2E assertion failed".into());
        }
    };
    ($condition:expr, $($argument:tt)*) => {
        if !$condition {
            return Err(format!($($argument)*));
        }
    };
}

#[path = "m7_migration_cases.rs"]
mod migration_cases;
use migration_cases::*;

#[test]
#[ignore = "M7 local black-box E2E requires an explicitly built release Runtime"]
fn m7_local_black_box_e2e() {
    if let Err(error) = run_suite() {
        panic!("M7 local black-box E2E failed: {error}");
    }
}

fn run_suite() -> M7Result<()> {
    let fixture = Fixture::new()?;
    let mut evidence = EvidenceLog::new(&fixture)?;
    evidence.write_run_header(&fixture)?;

    let mut failures = Vec::new();
    run_case(
        &mut evidence,
        "E2E-01",
        || run_e2e_01(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-02",
        || run_e2e_02(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-03",
        || run_e2e_03(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-04",
        || run_e2e_04(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-05",
        || run_e2e_05(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-06",
        || run_e2e_06(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-07",
        || run_e2e_07(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-08",
        || run_e2e_08(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-09",
        || run_e2e_09(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-10",
        || run_e2e_10(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-11",
        || run_e2e_11(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-12",
        || run_e2e_12(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-13",
        || run_e2e_13(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-14",
        || run_e2e_14(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-15",
        || run_e2e_15(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-16",
        || run_e2e_16(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-17",
        || run_e2e_17(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-18",
        || run_e2e_18(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-19",
        || run_e2e_19(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-20",
        || run_e2e_20(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-21",
        || run_e2e_21(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-22",
        || run_e2e_22(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-23",
        || run_e2e_23(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-24",
        || run_e2e_24(&fixture),
        &mut failures,
    );
    run_case(
        &mut evidence,
        "E2E-25",
        || run_e2e_25(&fixture),
        &mut failures,
    );

    // Keep the failure list case-local so one failed scenario cannot hide the
    // status of later scenarios.  The marker is emitted only after every
    // required case has reported PASS.
    if evidence.write_failures {
        failures.push("EVIDENCE_WRITE".into());
    }
    if let Err(error) = scan_final_evidence() {
        println!("M7_E2E_EVIDENCE_SCAN_FAIL: {error}");
        failures.push("EVIDENCE_SCAN".into());
    }
    if !failures.is_empty() {
        println!("M7_E2E_EVIDENCE_FILE={}", evidence.path.display());
        return Err(format!("failed M7 E2E cases: {failures:?}"));
    }
    println!("M7_E2E_EVIDENCE_FILE={}", evidence.path.display());
    fixture.cleanup()?;
    println!("LOCAL_M7_BLACK_BOX_E2E=PASS");
    Ok(())
}

fn run_case<F>(evidence: &mut EvidenceLog, case_id: &str, case: F, failures: &mut Vec<String>)
where
    F: FnOnce() -> M7Result<CaseEvidence>,
{
    *CURRENT_CASE_ID
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .map_err(|_| "case evidence lock poisoned")
        .expect("set current QA case") = case_id.to_owned();
    match case() {
        Ok(details) => {
            println!("{case_id} PASS");
            evidence.record(case_id, "PASS", details, None);
        }
        Err(error) => {
            let status = if error.starts_with("BLOCKED:") {
                "BLOCKED"
            } else {
                "FAIL"
            };
            println!("{case_id} {status}: {error}");
            failures.push(case_id.to_owned());
            evidence.record(
                case_id,
                status,
                CaseEvidence::default(),
                Some(sanitize_error(&error)),
            );
        }
    }
}

#[derive(Clone, Debug, Default)]
struct CaseEvidence {
    fixture: Option<String>,
    pre_sha256: Option<String>,
    post_sha256: Option<String>,
    pre_schema: Option<Value>,
    post_schema: Option<Value>,
    pre_revision: Option<Value>,
    post_revision: Option<Value>,
    result_code: Option<String>,
    assertions: Vec<String>,
    temp_residue: Option<bool>,
    runtime: BTreeMap<String, Value>,
}

struct EvidenceLog {
    path: PathBuf,
    binary: PathBuf,
    binary_sha256: String,
    run_id: String,
    write_failures: bool,
}

impl EvidenceLog {
    fn new(fixture: &Fixture) -> M7Result<Self> {
        let binary = m2_e2e::runtime_binary().map_err(|error| error.to_string())?;
        let binary_sha256 = file_sha256(&binary)?;
        let target = repo_root().join("target").join("m7-qa").join("e2e");
        fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        let path = target.join(format!("m7-e2e-{stamp}-{}.jsonl", std::process::id()));
        let _ = fixture;
        Ok(Self {
            path,
            binary,
            binary_sha256,
            run_id: format!("m7-e2e-{}-{stamp}", std::process::id()),
            write_failures: false,
        })
    }

    fn write_run_header(&mut self, fixture: &Fixture) -> M7Result<()> {
        let entry = json!({
            "recordType": "run",
            "runId": self.run_id,
            "binary": self.binary.display().to_string(),
            "binarySha256": self.binary_sha256,
            "pid": std::process::id(),
            "workspace": fixture.workspace.display().to_string(),
            "command": "cargo test -p chatgpt-harness-ui runtime::m7_e2e::m7_local_black_box_e2e -- --exact --ignored --nocapture --test-threads=1"
        });
        self.append(&entry)
    }

    fn record(
        &mut self,
        case_id: &str,
        status: &str,
        details: CaseEvidence,
        error: Option<String>,
    ) {
        let mut entry = json!({
            "recordType": "case",
            "runId": self.run_id,
            "case": case_id,
            "status": status,
            "fixture": details.fixture,
            "preSha256": details.pre_sha256,
            "postSha256": details.post_sha256,
            "preSchemaVersion": details.pre_schema,
            "postSchemaVersion": details.post_schema,
            "preConfigRevision": details.pre_revision,
            "postConfigRevision": details.post_revision,
            "resultCode": details.result_code,
            "assertions": details.assertions,
            "tempResidue": details.temp_residue,
            "runtime": details.runtime,
            "error": error,
        });
        if let Some(object) = entry.as_object_mut() {
            object.insert(
                "binarySha256".into(),
                Value::String(self.binary_sha256.clone()),
            );
        }
        if let Err(error) = self.append(&entry) {
            self.write_failures = true;
            eprintln!("M7_E2E_EVIDENCE_WRITE_FAIL case={case_id}: {error}");
        }
    }

    fn append(&self, entry: &Value) -> M7Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        serde_json::to_writer(&mut file, entry).map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())?;
        file.sync_data().map_err(|error| error.to_string())
    }
}

struct Fixture {
    base: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> M7Result<Self> {
        let base =
            std::env::temp_dir().join(format!("chatgpt-harness-m7-e2e-{}", uuid::Uuid::new_v4()));
        let workspace = base.join("workspace");
        fs::create_dir_all(&workspace).map_err(|error| error.to_string())?;
        fs::write(workspace.join("baseline.txt"), b"M7 baseline").map_err(|e| e.to_string())?;
        fs::write(workspace.join("read.txt"), b"M7 read content").map_err(|e| e.to_string())?;
        fs::write(workspace.join("edit.txt"), b"M7 old text").map_err(|e| e.to_string())?;
        Ok(Self { base, workspace })
    }

    fn config_dir(&self, name: &str) -> PathBuf {
        self.base.join("config").join(name)
    }

    fn install_fixture(&self, name: &str, source: &str) -> M7Result<InstalledFixture> {
        let source_path = fixture_path(source);
        let directory = self.config_dir(name);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let target = directory.join("config.json");
        fs::copy(&source_path, &target).map_err(|error| error.to_string())?;
        let before = fs::read(&target).map_err(|error| error.to_string())?;
        let raw: Value = serde_json::from_slice(&before).map_err(|error| error.to_string())?;
        Ok(InstalledFixture {
            source: source.to_owned(),
            directory,
            target,
            before,
            before_json: raw,
        })
    }

    fn install_value(
        &self,
        name: &str,
        source: &str,
        mut value: Value,
    ) -> M7Result<InstalledFixture> {
        let directory = self.config_dir(name);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let target = directory.join("config.json");
        if let Some(object) = value.as_object_mut() {
            if source == "future" {
                object.insert("schemaVersion".into(), json!(3));
            }
            if source == "secret" {
                value["tunnel"]["apiKey"] = json!(SECRET_SENTINEL);
            }
        }
        let bytes = serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?;
        fs::write(&target, &bytes).map_err(|error| error.to_string())?;
        Ok(InstalledFixture {
            source: source.to_owned(),
            directory,
            target,
            before: bytes,
            before_json: value,
        })
    }

    fn cleanup(&self) -> M7Result<()> {
        if self.base.exists() {
            fs::remove_dir_all(&self.base).map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

struct InstalledFixture {
    source: String,
    directory: PathBuf,
    target: PathBuf,
    before: Vec<u8>,
    before_json: Value,
}

impl InstalledFixture {
    fn store(&self) -> ConfigStore {
        ConfigStore::new(self.directory.clone())
    }

    fn before_sha256(&self) -> M7Result<String> {
        bytes_sha256(&self.before)
    }

    fn after_sha256(&self) -> M7Result<String> {
        file_sha256(&self.target)
    }

    fn temp_residue(&self) -> bool {
        self.target.with_extension("json.tmp").exists()
    }
}

fn run_e2e_08(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-08", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(false);
        })?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-08")?;
    let names = list_tool_names(mcp.list_tools().map_err(|error| error.to_string())?)?;
    check!(!names.iter().any(|name| name == "write"));
    let target = fixture.workspace.join("migrated-disabled-write.txt");
    let before = activity_snapshot(&mut runtime)?;
    let response = mcp
        .call_tool(
            "write",
            json!({ "path": "migrated-disabled-write.txt", "content": "must-not-write" }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_error_code(&response, "TOOL_DISABLED")?;
    check!(!target.exists());
    let after = activity_snapshot(&mut runtime)?;
    let new_items = activity_delta(&before, &after);
    check!(new_items.iter().any(|item| {
        item["toolName"] == json!("write")
            && item["failureCategory"] == json!("tool_disabled")
            && item["toolEnabled"] == json!(false)
    }));
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    check!(status["pendingApprovals"] == json!(0));
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "TOOL_DISABLED",
        vec![
            "migrated_write_off_reached_runtime_snapshot",
            "tools_list_omits_disabled_tool",
            "stale_call_rejected_before_executor",
            "activity_records_disabled_reason",
            "no_filesystem_side_effect",
        ],
    )?;
    details
        .runtime
        .insert("activeRequests".into(), status["activeRequests"].clone());
    details.runtime.insert(
        "pendingApprovals".into(),
        status["pendingApprovals"].clone(),
    );
    details.runtime.insert(
        "runningExecutions".into(),
        status["runningExecutions"].clone(),
    );
    Ok(details)
}

fn run_e2e_09(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-09", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("deny");
        })?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-09")?;
    let target = fixture.workspace.join("migrated-policy-deny.txt");
    let before = activity_snapshot(&mut runtime)?;
    let response = mcp
        .call_tool(
            "write",
            json!({ "path": "migrated-policy-deny.txt", "content": "must-not-write" }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_error_code(&response, "POLICY_DENIED")?;
    check!(!target.exists());
    let policies = control(&mut runtime, "policies.list", json!({}))?;
    let write = policies["effective"]["perTool"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["tool"] == json!("write")))
        .ok_or_else(|| "policies.list omitted migrated write policy".to_owned())?;
    check!(write["decision"] == json!("deny"));
    check!(write["matchedRuleIds"]
        .as_array()
        .is_some_and(|ids| { ids.iter().any(|id| id == "fake-deny-write") }));
    let after = activity_snapshot(&mut runtime)?;
    let new_items = activity_delta(&before, &after);
    check!(new_items.iter().any(|item| {
        item["toolName"] == json!("write")
            && item["decisionSummary"] == json!("deny")
            && item["policyReason"] == json!("matched_user_rules")
            && item["matchedRuleIds"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == "fake-deny-write"))
    }));
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    check!(status["pendingApprovals"] == json!(0));
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "POLICY_DENIED",
        vec![
            "migrated_user_policy_reached_runtime",
            "deny_request_did_not_execute",
            "effective_policy_matches_fixture",
            "activity_contains_decision_reason_and_rule_id",
            "counters_clean",
        ],
    )?;
    details
        .runtime
        .insert("activeRequests".into(), status["activeRequests"].clone());
    details.runtime.insert(
        "pendingApprovals".into(),
        status["pendingApprovals"].clone(),
    );
    details.runtime.insert(
        "runningExecutions".into(),
        status["runningExecutions"].clone(),
    );
    let (ask_installed, ask_config) =
        runtime_fixture_config(fixture, "e2e-09-ask", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("allow");
            value["policies"]["userRules"] = json!([{
                "id": "m7-ask-write",
                "name": "M7 ASK write",
                "enabled": true,
                "decision": "ask",
                "matcher": { "type": "tool", "tool": "write" },
                "reason": "M7 explicit ASK rule"
            }]);
        })?;
    let (mut ask_runtime, mut ask_mcp) = start_runtime(&ask_config, "M7-E2E-09-ask")?;
    let ask_call = ask_mcp.pending_tool(
        "write",
        json!({ "path": "migrated-policy-ask.txt", "content": "must-not-write" }),
    );
    let ask_approval = wait_pending_approval(&mut ask_runtime, "migrated-policy-ask.txt")?;
    let ask_request_id = ask_approval["requestId"]
        .as_str()
        .ok_or_else(|| "migrated ASK approval omitted requestId".to_owned())?
        .to_owned();
    let ask_status = control(&mut ask_runtime, "runtime.status", json!({}))?;
    check!(ask_status["activeRequests"] == json!(1));
    check!(ask_status["pendingApprovals"] == json!(1));
    check!(ask_status["runningExecutions"] == json!(0));
    check!(ask_approval["matchedRuleIds"]
        .as_array()
        .is_some_and(|ids| { ids.iter().any(|id| id == "m7-ask-write") }));
    let ask_before = activity_snapshot(&mut ask_runtime)?;
    let resolved = control(
        &mut ask_runtime,
        "approvals.resolve",
        json!({ "requestId": ask_request_id, "decision": "deny" }),
    )?;
    check!(resolved["approval"]["decision"] == json!("deny"));
    let ask_result = ask_call.receive().map_err(|error| error.to_string())?;
    assert_mcp_error_code(&ask_result, "APPROVAL_DENIED")?;
    check!(!fixture.workspace.join("migrated-policy-ask.txt").exists());
    let ask_final_status = wait_for_zero_counters(&mut ask_runtime)?;
    let ask_after = activity_snapshot(&mut ask_runtime)?;
    let ask_delta = activity_delta(&ask_before, &ask_after);
    check!(ask_delta.iter().any(|item| {
        item["requestId"] == json!(ask_request_id)
            && item["policyReason"] == json!("matched_user_rules")
            && item["matchedRuleIds"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == "m7-ask-write"))
    }));
    check!(ask_delta.iter().any(|item| {
        item["requestId"] == json!(ask_request_id)
            && item["approvalId"].is_string()
            && item["approvalDecision"] == json!("deny")
    }));
    stop_runtime(&mut ask_runtime)?;
    details
        .assertions
        .push("same migration case explicit ASK enters pending and resolves Deny".into());
    details
        .runtime
        .insert("askRequestId".into(), json!(ask_request_id));
    details
        .runtime
        .insert("askPendingCounters".into(), ask_status);
    details
        .runtime
        .insert("askFinalCounters".into(), ask_final_status);
    let _ = ask_installed;
    Ok(details)
}

fn run_e2e_10(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = runtime_fixture_config(
        fixture,
        "e2e-10",
        "v1-transitional-m6.json",
        allow_file_tools,
    )?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-10")?;
    let before = activity_snapshot(&mut runtime)?;
    let response = mcp
        .call_tool("read_file", json!({ "path": "read.txt" }))
        .map_err(|error| error.to_string())?;
    assert_mcp_success(&response)?;
    check!(response["result"]["structuredContent"]["content"] == json!("M7 read content"));
    let after = activity_snapshot(&mut runtime)?;
    let delta = activity_delta(&before, &after);
    let terminal = latest_terminal(&delta, "read_file")?;
    assert_file_evidence(
        terminal,
        "read_file",
        "read.txt",
        "workspace_read",
        "allow",
        "tool_default",
    )?;
    check!(terminal["outcomeSummary"] == json!("success"));
    check!(terminal["durationMs"].as_u64().is_some());
    check!(terminal["resultEvidence"]["tool"] == json!("read_file"));
    check!(terminal["resultEvidence"]["bytesRead"] == json!(15));
    let serialized = serde_json::to_string(&after).map_err(|error| error.to_string())?;
    check!(!serialized.contains("M7 read content"));
    check!(!serialized.contains(&fixture.workspace.display().to_string()));
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "READ_FILE_EVIDENCE_COMPLETE",
        vec![
            "request_id_tool_and_sanitized_logical_path",
            "enabled_capability_and_policy_evidence",
            "terminal_outcome_and_duration",
            "bytes_read_scalar",
            "content_and_absolute_workspace_path_absent",
        ],
    )?;
    details
        .runtime
        .insert("activeRequests".into(), status["activeRequests"].clone());
    details.runtime.insert(
        "pendingApprovals".into(),
        status["pendingApprovals"].clone(),
    );
    details.runtime.insert(
        "runningExecutions".into(),
        status["runningExecutions"].clone(),
    );
    Ok(details)
}

fn run_e2e_11(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = runtime_fixture_config(
        fixture,
        "e2e-11",
        "v1-transitional-m6.json",
        allow_file_tools,
    )?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-11")?;
    let path = "e2e-11-write.txt";
    let sentinel = "M7_WRITE_CONTENT_SENTINEL";
    let before = activity_snapshot(&mut runtime)?;
    let response = mcp
        .call_tool("write", json!({ "path": path, "content": sentinel }))
        .map_err(|error| error.to_string())?;
    assert_mcp_success(&response)?;
    check!(
        fs::read(fixture.workspace.join(path)).map_err(|e| e.to_string())? == sentinel.as_bytes()
    );
    let after = activity_snapshot(&mut runtime)?;
    let delta = activity_delta(&before, &after);
    let terminal = latest_terminal(&delta, "write")?;
    assert_file_evidence(
        terminal,
        "write",
        path,
        "workspace_write",
        "allow",
        "tool_default",
    )?;
    check!(terminal["outcomeSummary"] == json!("success"));
    check!(terminal["durationMs"].as_u64().is_some());
    check!(terminal["resultEvidence"]["tool"] == json!("write"));
    check!(terminal["resultEvidence"]["bytesWritten"] == json!(sentinel.len()));
    check!(terminal["resultEvidence"]["disposition"] == json!("created"));
    let serialized = serde_json::to_string(&after).map_err(|error| error.to_string())?;
    check!(!serialized.contains(sentinel));
    check!(!serialized.contains(&fixture.workspace.display().to_string()));
    let diagnostics = control(&mut runtime, "control.diagnostics.get", json!({}))?;
    check!(!serde_json::to_string(&diagnostics)
        .map_err(|error| error.to_string())?
        .contains(sentinel));
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "WRITE_EVIDENCE_COMPLETE",
        vec![
            "logical_target_path",
            "bytes_written_and_created_scalar",
            "filesystem_content_correct",
            "raw_write_sentinel_absent_from_activity_and_diagnostics",
        ],
    )?;
    details
        .runtime
        .insert("activeRequests".into(), status["activeRequests"].clone());
    details.runtime.insert(
        "pendingApprovals".into(),
        status["pendingApprovals"].clone(),
    );
    details.runtime.insert(
        "runningExecutions".into(),
        status["runningExecutions"].clone(),
    );
    Ok(details)
}

fn run_e2e_12(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = runtime_fixture_config(
        fixture,
        "e2e-12",
        "v1-transitional-m6.json",
        allow_file_tools,
    )?;
    let path = fixture.workspace.join("edit.txt");
    let old = "M7_EDIT_OLD_SENTINEL";
    let new = "M7_EDIT_NEW_SENTINEL";
    fs::write(&path, old).map_err(|error| error.to_string())?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-12")?;
    let before = activity_snapshot(&mut runtime)?;
    let response = mcp
        .call_tool(
            "edit",
            json!({ "path": "edit.txt", "oldText": old, "newText": new }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_success(&response)?;
    check!(fs::read(&path).map_err(|e| e.to_string())? == new.as_bytes());
    let after = activity_snapshot(&mut runtime)?;
    let delta = activity_delta(&before, &after);
    let terminal = latest_terminal(&delta, "edit")?;
    assert_file_evidence(
        terminal,
        "edit",
        "edit.txt",
        "workspace_write",
        "allow",
        "tool_default",
    )?;
    check!(terminal["outcomeSummary"] == json!("success"));
    check!(terminal["durationMs"].as_u64().is_some());
    check!(terminal["resultEvidence"]["tool"] == json!("edit"));
    check!(terminal["resultEvidence"]["replacements"] == json!(1));
    check!(terminal["resultEvidence"]["bytesWritten"] == json!(new.len()));
    let serialized = serde_json::to_string(&after).map_err(|error| error.to_string())?;
    check!(!serialized.contains(old));
    check!(!serialized.contains(new));
    check!(!serialized.contains(&fixture.workspace.display().to_string()));
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "EDIT_EVIDENCE_COMPLETE",
        vec![
            "logical_target_path",
            "replacement_and_bytes_written_scalars",
            "filesystem_result_correct",
            "old_new_text_absent_from_activity",
        ],
    )?;
    details
        .runtime
        .insert("activeRequests".into(), status["activeRequests"].clone());
    details.runtime.insert(
        "pendingApprovals".into(),
        status["pendingApprovals"].clone(),
    );
    details.runtime.insert(
        "runningExecutions".into(),
        status["runningExecutions"].clone(),
    );
    Ok(details)
}

fn run_e2e_13(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-13", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("allow");
        })?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-13")?;
    let path = "e2e-13-denied.txt";
    let before = activity_snapshot(&mut runtime)?;
    let response = mcp
        .call_tool(
            "write",
            json!({ "path": path, "content": "M7_DENIED_SENTINEL" }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_error_code(&response, "POLICY_DENIED")?;
    check!(!fixture.workspace.join(path).exists());
    let after = activity_snapshot(&mut runtime)?;
    let delta = activity_delta(&before, &after);
    let terminal = latest_terminal(&delta, "write")?;
    println!(
        "M7_E2E_13_TERMINAL eventType={} failureCategory={} outcome={} decision={} policyReason={}",
        terminal["eventType"],
        terminal["failureCategory"],
        terminal["outcomeSummary"],
        terminal["decisionSummary"],
        terminal["policyReason"]
    );
    check!(terminal["decisionSummary"] == json!("deny"));
    check!(terminal["policyReason"] == json!("matched_user_rules"));
    check!(terminal["matchedRuleIds"]
        .as_array()
        .is_some_and(|ids| { ids.iter().any(|id| id == "fake-deny-write") }));
    check!(terminal["outcomeSummary"] == json!("denied"));
    check!(terminal["durationMs"].as_u64().is_some());
    let serialized = serde_json::to_string(&after).map_err(|error| error.to_string())?;
    check!(!serialized.contains("M7_DENIED_SENTINEL"));
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    check!(status["pendingApprovals"] == json!(0));
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "POLICY_DENIED",
        vec![
            "deny_request_did_not_execute",
            "activity_policy_deny_and_reason",
            "terminal_denied_outcome_and_duration",
            "counters_zero",
        ],
    )?;
    details
        .runtime
        .insert("activeRequests".into(), status["activeRequests"].clone());
    details.runtime.insert(
        "pendingApprovals".into(),
        status["pendingApprovals"].clone(),
    );
    details.runtime.insert(
        "runningExecutions".into(),
        status["runningExecutions"].clone(),
    );
    Ok(details)
}

fn run_e2e_14(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-14", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("ask");
            value["policies"]["userRules"] = json!([]);
        })?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-14")?;
    let path = "e2e-14-allow-once.txt";
    let before = activity_snapshot(&mut runtime)?;
    let pending_call = mcp.pending_tool("write", json!({ "path": path, "content": "M7 approval" }));
    let approval = wait_pending_approval(&mut runtime, path)?;
    let request_id = approval["requestId"]
        .as_str()
        .ok_or_else(|| "pending approval omitted requestId".to_owned())?
        .to_owned();
    let pending_status = control(&mut runtime, "runtime.status", json!({}))?;
    check!(pending_status["activeRequests"] == json!(1));
    check!(pending_status["pendingApprovals"] == json!(1));
    check!(pending_status["runningExecutions"] == json!(0));
    let pending_activity = activity_snapshot(&mut runtime)?;
    let pending_entry = pending_activity
        .iter()
        .find(|item| item["requestId"] == json!(request_id) && item["approvalId"].is_string())
        .ok_or_else(|| "pending Activity omitted request".to_owned())?;
    check!(pending_entry["approvalId"].is_string());
    let pending_approval_id = pending_entry["approvalId"].clone();
    check!(pending_approval_id == json!(request_id));
    let resolved = control(
        &mut runtime,
        "approvals.resolve",
        json!({ "requestId": request_id, "decision": "allow_once" }),
    )?;
    check!(resolved["approval"]["status"] == json!("approved"));
    check!(resolved["approval"]["decision"] == json!("allow_once"));
    let resolved_approval_id = resolved["approval"]["requestId"].clone();
    check!(resolved_approval_id == json!(request_id));
    check!(pending_approval_id == resolved_approval_id);
    let result = pending_call.receive().map_err(|error| error.to_string())?;
    assert_mcp_success(&result)?;
    check!(fs::read(fixture.workspace.join(path)).map_err(|e| e.to_string())? == b"M7 approval");
    let final_status = wait_for_zero_counters(&mut runtime)?;
    let after = activity_snapshot(&mut runtime)?;
    let delta = activity_delta(&before, &after);
    check!(delta.iter().any(|item| {
        item["requestId"] == json!(request_id)
            && item["approvalId"] == resolved_approval_id
            && item["approvalDecision"] == json!("allow_once")
    }));
    check!(delta.iter().any(|item| {
        item["requestId"] == json!(request_id)
            && item["outcomeSummary"] == json!("success")
            && item["durationMs"].as_u64().is_some()
    }));
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "APPROVAL_ALLOW_ONCE_RESUMED",
        vec![
            "pending_counters_active_1_approval_1_running_0",
            "pending_activity_has_approval_id",
            "real_control_allow_once_resolution",
            "same_request_resumed_and_filesystem_effect_correct",
            "terminal_correlation_and_zero_cleanup",
        ],
    )?;
    details
        .runtime
        .insert("requestId".into(), json!(request_id));
    details
        .runtime
        .insert("approvalId".into(), resolved_approval_id);
    details
        .runtime
        .insert("pendingCounters".into(), pending_status);
    details.runtime.insert("finalCounters".into(), final_status);
    Ok(details)
}

fn run_e2e_15(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-15", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("ask");
            value["policies"]["userRules"] = json!([]);
        })?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-15")?;
    let path = "e2e-15-denied-approval.txt";
    let before = activity_snapshot(&mut runtime)?;
    let pending_call = mcp.pending_tool("write", json!({ "path": path, "content": "M7 deny" }));
    let approval = wait_pending_approval(&mut runtime, path)?;
    let request_id = approval["requestId"]
        .as_str()
        .ok_or_else(|| "pending approval omitted requestId".to_owned())?
        .to_owned();
    let pending_status = control(&mut runtime, "runtime.status", json!({}))?;
    check!(pending_status["activeRequests"]
        .as_u64()
        .is_some_and(|count| count >= 1));
    check!(pending_status["pendingApprovals"] == json!(1));
    let resolved = control(
        &mut runtime,
        "approvals.resolve",
        json!({ "requestId": request_id, "decision": "deny" }),
    )?;
    check!(resolved["approval"]["status"] == json!("denied"));
    let result = pending_call.receive().map_err(|error| error.to_string())?;
    assert_mcp_error_code(&result, "APPROVAL_DENIED")?;
    check!(!fixture.workspace.join(path).exists());
    let final_status = wait_for_zero_counters(&mut runtime)?;
    let after = activity_snapshot(&mut runtime)?;
    let delta = activity_delta(&before, &after);
    check!(delta.iter().any(|item| {
        item["requestId"] == json!(request_id)
            && item["approvalDecision"] == json!("deny")
            && item["approvalId"].is_string()
    }));
    check!(delta.iter().any(|item| {
        item["requestId"] == json!(request_id) && item["outcomeSummary"] == json!("denied")
    }));
    check!(!delta.iter().any(|item| {
        item["requestId"] == json!(request_id) && item["eventType"] == json!("execution_completed")
    }));
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "APPROVAL_DENIED",
        vec![
            "same_original_call_terminated",
            "no_filesystem_side_effect",
            "approval_id_and_deny_decision_correlated",
            "terminal_denied_outcome",
            "counters_zero",
        ],
    )?;
    details
        .runtime
        .insert("requestId".into(), json!(request_id));
    details
        .runtime
        .insert("pendingCounters".into(), pending_status);
    details.runtime.insert("finalCounters".into(), final_status);
    Ok(details)
}

fn run_e2e_16(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = bash_runtime_config(fixture, "e2e-16", "allow")?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-16")?;
    let pending_call = mcp.pending_tool("bash", json!({ "command": "sleep 3" }));
    let during = wait_for_running_execution(&mut runtime)?;
    check!(during["activeRequests"]
        .as_u64()
        .is_some_and(|count| count >= 1));
    check!(during["runningExecutions"] == json!(1));
    check!(during["pendingApprovals"] == json!(0));
    let result = pending_call.receive().map_err(|error| error.to_string())?;
    assert_mcp_success(&result)?;
    check!(result["result"]["structuredContent"]["exitCode"] == json!(0));
    let after = wait_for_zero_counters(&mut runtime)?;
    let activity = activity_snapshot(&mut runtime)?;
    let terminal = latest_terminal(&activity, "bash")?;
    check!(terminal["durationMs"].as_u64().is_some());
    check!(terminal["bash"].is_object());
    check!(terminal["resultEvidence"]["tool"] == json!("bash"));
    check!(terminal["resultEvidence"]["exitCode"] == json!(0));
    let serialized = serde_json::to_string(&activity).map_err(|error| error.to_string())?;
    check!(!serialized.contains("stdout"));
    check!(!serialized.contains("stderr"));
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "BASH_RUNNING_EXECUTION_OBSERVED",
        vec![
            "real_bash_request_active",
            "running_executions_one_during_executor",
            "pending_approvals_zero_for_allow_path",
            "terminal_duration_and_exit_code_evidence",
            "counters_zero_after_completion",
        ],
    )?;
    details.runtime.insert("duringCounters".into(), during);
    details.runtime.insert("afterCounters".into(), after);
    Ok(details)
}

fn run_e2e_17(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = bash_runtime_config(fixture, "e2e-17", "allow")?;
    let bash_before = process_ids_by_image("bash.exe")?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-17")?;
    let runtime_pid = runtime.pid();
    let pending_call = mcp.pending_tool("bash", json!({ "command": "sleep 60" }));
    let during = wait_for_running_execution(&mut runtime)?;
    check!(during["runningExecutions"] == json!(1));
    let during_tree = process_tree_descendants(runtime_pid)?;
    check!(
        !during_tree.is_empty(),
        "running Bash request had no observable Runtime child process"
    );
    let timeout_result = pending_call.receive().map_err(|error| error.to_string())?;
    assert_mcp_error_code(&timeout_result, "BASH_TIMEOUT")?;
    let after = wait_for_zero_counters(&mut runtime)?;
    check!(after["activeRequests"] == json!(0));
    check!(after["pendingApprovals"] == json!(0));
    check!(after["runningExecutions"] == json!(0));
    let activity = activity_snapshot(&mut runtime)?;
    let terminal = latest_terminal(&activity, "bash")?;
    check!(terminal["failureCategory"] == json!("bash_timeout"));
    check!(terminal["durationMs"].as_u64().is_some());
    check!(terminal["bash"].is_object());
    let serialized = serde_json::to_string(&activity).map_err(|error| error.to_string())?;
    check!(!serialized.contains("stdout"));
    check!(!serialized.contains("stderr"));
    let bash_after = process_ids_by_image("bash.exe")?;
    check!(
        bash_after.iter().all(|pid| bash_before.contains(pid)),
        "Bash timeout left an unexpected bash.exe process"
    );
    let after_tree = process_tree_descendants(runtime_pid)?;
    check!(
        during_tree
            .iter()
            .all(|process| !after_tree.iter().any(|item| item.pid == process.pid)),
        "Bash timeout left an owned process-tree descendant"
    );
    let owned_pids = during_tree
        .iter()
        .map(|process| process.pid)
        .collect::<Vec<_>>();
    remember_owned_bash_pids(&owned_pids);
    stop_runtime(&mut runtime)?;
    check!(runtime.stopped_and_endpoint_closed());
    let owned_pids = owned_bash_pids();
    check!(
        owned_pids.iter().all(|pid| !process_exists(*pid)),
        "a Bash process-tree descendant remained after final shutdown"
    );
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "BASH_TIMEOUT",
        vec![
            "real_bash_running_before_stop",
            "runtime_30_second_timeout_returns_typed_result",
            "activity_failure_category_bash_timeout",
            "duration_and_bounded_bash_evidence",
            "counters_zero_while_runtime_remains_alive",
            "runtime_exit_endpoint_closed_after_assertions",
        ],
    )?;
    details.runtime.insert("duringCounters".into(), during);
    details.runtime.insert("afterCounters".into(), after);
    details
        .runtime
        .insert("bashBeforePids".into(), json!(bash_before));
    details
        .runtime
        .insert("bashAfterPids".into(), json!(bash_after));
    details.runtime.insert(
        "duringProcessTree".into(),
        json!(during_tree
            .iter()
            .map(|process| json!({
                "pid": process.pid,
                "parentPid": process.parent_pid,
                "name": process.name,
            }))
            .collect::<Vec<_>>()),
    );
    details
        .runtime
        .insert("afterProcessTreeCount".into(), json!(after_tree.len()));
    Ok(details)
}

fn run_e2e_18(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (disabled_installed, disabled_config) = runtime_fixture_config(
        fixture,
        "e2e-18-disabled",
        "v1-transitional-m6.json",
        |value| value["tools"]["write"]["enabled"] = json!(false),
    )?;
    let (mut disabled_runtime, mut disabled_mcp) =
        start_runtime(&disabled_config, "M7-E2E-18-disabled")?;
    let disabled_effective = control(
        &mut disabled_runtime,
        "control.effective_tools.get",
        json!({}),
    )?;
    let disabled_reason = effective_tool_reason(&disabled_effective, "write")?;
    check!(effective_tool_item(&disabled_effective, "write")?["availability"] == json!("disabled"));
    let disabled_response = disabled_mcp
        .call_tool(
            "write",
            json!({ "path": "e2e-18-disabled.txt", "content": "x" }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_error_code(&disabled_response, "TOOL_DISABLED")?;
    stop_runtime(&mut disabled_runtime)?;

    let (denied_installed, denied_config) = runtime_fixture_config(
        fixture,
        "e2e-18-denied",
        "v1-transitional-m6.json",
        |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("deny");
            value["policies"]["userRules"] = json!([]);
        },
    )?;
    let (mut denied_runtime, mut denied_mcp) = start_runtime(&denied_config, "M7-E2E-18-denied")?;
    let denied_effective = control(
        &mut denied_runtime,
        "control.effective_tools.get",
        json!({}),
    )?;
    let denied_reason = effective_tool_reason(&denied_effective, "write")?;
    check!(
        effective_tool_item(&denied_effective, "write")?["availability"]
            == json!("unavailable_by_policy")
    );
    let denied_response = denied_mcp
        .call_tool(
            "write",
            json!({ "path": "e2e-18-denied.txt", "content": "x" }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_error_code(&denied_response, "POLICY_DENIED")?;
    stop_runtime(&mut denied_runtime)?;

    let (ask_installed, ask_config) =
        runtime_fixture_config(fixture, "e2e-18-ask", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("ask");
            value["policies"]["userRules"] = json!([]);
        })?;
    let (mut ask_runtime, mut ask_mcp) = start_runtime(&ask_config, "M7-E2E-18-ask")?;
    let ask_effective = control(&mut ask_runtime, "control.effective_tools.get", json!({}))?;
    let ask_reason = effective_tool_reason(&ask_effective, "write")?;
    check!(
        effective_tool_item(&ask_effective, "write")?["availability"]
            == json!("available_with_approval")
    );
    let pending_call =
        ask_mcp.pending_tool("write", json!({ "path": "e2e-18-ask.txt", "content": "x" }));
    let approval = wait_pending_approval(&mut ask_runtime, "e2e-18-ask.txt")?;
    let ask_request = approval["requestId"]
        .as_str()
        .ok_or_else(|| "ASK request omitted requestId".to_owned())?
        .to_owned();
    control(
        &mut ask_runtime,
        "approvals.resolve",
        json!({ "requestId": ask_request, "decision": "deny" }),
    )?;
    let ask_response = pending_call.receive().map_err(|error| error.to_string())?;
    assert_mcp_error_code(&ask_response, "APPROVAL_DENIED")?;
    wait_for_zero_counters(&mut ask_runtime)?;
    stop_runtime(&mut ask_runtime)?;

    let (unavailable_installed, unavailable_config) = runtime_fixture_config(
        fixture,
        "e2e-18-unavailable",
        "v1-transitional-m6.json",
        |value| {
            value["security"]["mode"] = json!("normal");
            value["security"]["developerTestMode"] = json!(false);
            value["tools"]["bash"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["bash"] = json!("allow");
        },
    )?;
    let (mut unavailable_runtime, mut unavailable_mcp) =
        start_runtime(&unavailable_config, "M7-E2E-18-unavailable")?;
    let unavailable_effective = control(
        &mut unavailable_runtime,
        "control.effective_tools.get",
        json!({}),
    )?;
    let unavailable_reason = effective_tool_reason(&unavailable_effective, "bash")?;
    check!(
        effective_tool_item(&unavailable_effective, "bash")?["availability"]
            == json!("unavailable_by_mode")
    );
    let unavailable_response = unavailable_mcp
        .call_tool("bash", json!({ "command": "printf unavailable" }))
        .map_err(|error| error.to_string())?;
    check!(unavailable_response["result"]["isError"] == json!(true));
    stop_runtime(&mut unavailable_runtime)?;
    check!(disabled_reason != denied_reason);
    check!(denied_reason != ask_reason);
    check!(ask_reason != unavailable_reason);
    check!(disabled_installed.target.exists());
    check!(denied_installed.target.exists());
    check!(ask_installed.target.exists());
    check!(unavailable_installed.target.exists());
    let mut details = CaseEvidence::default();
    details.assertions = vec![
        "disabled has distinct TOOL_DISABLED/statusReason".into(),
        "denied has distinct POLICY_DENIED/statusReason".into(),
        "ask has pending approval/APPROVAL_DENIED statusReason".into(),
        "unavailable Bash has distinct mode/runtime statusReason".into(),
    ];
    details.result_code = Some("MACHINE_DISTINCT_STATES".into());
    details
        .runtime
        .insert("disabledReason".into(), json!(disabled_reason));
    details
        .runtime
        .insert("deniedReason".into(), json!(denied_reason));
    details
        .runtime
        .insert("askReason".into(), json!(ask_reason));
    details
        .runtime
        .insert("unavailableReason".into(), json!(unavailable_reason));
    Ok(details)
}

fn bash_runtime_config(
    fixture: &Fixture,
    name: &str,
    bash_decision: &str,
) -> M7Result<(InstalledFixture, AppConfig)> {
    runtime_fixture_config(fixture, name, "v1-transitional-m6.json", |value| {
        value["security"]["mode"] = json!("developer_test");
        value["security"]["developerTestMode"] = json!(true);
        value["tools"]["bash"]["enabled"] = json!(true);
        value["bash"]["autoDetect"] = json!(true);
        value["bash"]["executablePath"] = Value::Null;
        value["policies"]["toolDefaults"]["bash"] = json!(bash_decision);
        value["policies"]["userRules"] = json!([]);
    })
}

fn wait_for_running_execution(runtime: &mut RuntimeProcess) -> M7Result<Value> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut approval_resolved = false;
    loop {
        let status = control(runtime, "runtime.status", json!({}))?;
        if status["runningExecutions"] == json!(1) {
            return Ok(status);
        }
        if !approval_resolved
            && status["pendingApprovals"]
                .as_u64()
                .is_some_and(|count| count > 0)
        {
            let pending = control(runtime, "approvals.list", json!({ "status": "pending" }))?;
            if let Some(request_id) = pending["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["requestId"].as_str())
            {
                control(
                    runtime,
                    "approvals.resolve",
                    json!({ "requestId": request_id, "decision": "allow_once" }),
                )?;
                approval_resolved = true;
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("Bash did not enter runningExecutions=1: {status}"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn receive_fast_bash(runtime: &mut RuntimeProcess, call: PendingMcpCall) -> M7Result<Value> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut approval_resolved = false;
    loop {
        let status = control(runtime, "runtime.status", json!({}))?;
        if !approval_resolved
            && status["pendingApprovals"]
                .as_u64()
                .is_some_and(|count| count > 0)
        {
            let pending = control(runtime, "approvals.list", json!({ "status": "pending" }))?;
            if let Some(request_id) = pending["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["requestId"].as_str())
            {
                control(
                    runtime,
                    "approvals.resolve",
                    json!({ "requestId": request_id, "decision": "allow_once" }),
                )?;
                approval_resolved = true;
            }
        }
        if status["activeRequests"] == json!(0) {
            break;
        }
        if Instant::now() >= deadline {
            return Err(format!("short Bash request did not settle: {status}"));
        }
        thread::sleep(POLL_INTERVAL);
    }
    call.receive().map_err(|error| error.to_string())
}

fn effective_tool_reason(value: &Value, tool: &str) -> M7Result<String> {
    Ok(effective_tool_item(value, tool)?["statusReason"]
        .as_str()
        .ok_or_else(|| format!("effective tools omitted typed statusReason for {tool}"))?
        .to_owned())
}

fn effective_tool_item<'a>(value: &'a Value, tool: &str) -> M7Result<&'a Value> {
    let items = value
        .as_array()
        .or_else(|| value["items"].as_array())
        .ok_or_else(|| "effective tools response did not return items".to_owned())?;
    items
        .iter()
        .find(|item| item["name"] == json!(tool) || item["tool"] == json!(tool))
        .ok_or_else(|| format!("effective tools omitted {tool}"))
}

fn run_e2e_19(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (disabled_installed, disabled_config) = runtime_fixture_config(
        fixture,
        "e2e-19-disabled",
        "v1-transitional-m6.json",
        |value| value["tools"]["write"]["enabled"] = json!(false),
    )?;
    let (mut disabled_runtime, _disabled_mcp) =
        start_runtime(&disabled_config, "M7-E2E-19-disabled")?;
    let disabled = control(
        &mut disabled_runtime,
        "control.effective_tools.get",
        json!({}),
    )?;
    check!(effective_tool_reason(&disabled, "write")? == "tool_disabled");
    let (denied_installed, denied_config) = runtime_fixture_config(
        fixture,
        "e2e-19-denied",
        "v1-transitional-m6.json",
        |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("deny");
            value["policies"]["userRules"] = json!([]);
        },
    )?;
    let (mut denied_runtime, _denied_mcp) = start_runtime(&denied_config, "M7-E2E-19-denied")?;
    let denied = control(
        &mut denied_runtime,
        "control.effective_tools.get",
        json!({}),
    )?;
    check!(effective_tool_reason(&denied, "write")? == "policy_denied");
    let (ask_installed, ask_config) =
        runtime_fixture_config(fixture, "e2e-19-ask", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("ask");
            value["policies"]["userRules"] = json!([]);
        })?;
    let (mut ask_runtime, _ask_mcp) = start_runtime(&ask_config, "M7-E2E-19-ask")?;
    let ask = control(&mut ask_runtime, "control.effective_tools.get", json!({}))?;
    check!(effective_tool_reason(&ask, "write")? == "approval_required");
    let (unavailable_installed, unavailable_config) = runtime_fixture_config(
        fixture,
        "e2e-19-unavailable",
        "v1-transitional-m6.json",
        |value| {
            value["security"]["mode"] = json!("developer_test");
            value["security"]["developerTestMode"] = json!(true);
            value["tools"]["bash"]["enabled"] = json!(true);
            value["bash"]["autoDetect"] = json!(false);
            value["bash"]["executablePath"] = json!("C:\\M7\\missing-git-bash.exe");
            value["policies"]["toolDefaults"]["bash"] = json!("allow");
        },
    )?;
    let (mut unavailable_runtime, _unavailable_mcp) =
        start_runtime(&unavailable_config, "M7-E2E-19-unavailable")?;
    let unavailable = control(
        &mut unavailable_runtime,
        "control.effective_tools.get",
        json!({}),
    )?;
    check!(effective_tool_reason(&unavailable, "bash")? == "bash_executable_unavailable");

    stop_runtime(&mut disabled_runtime)?;
    stop_runtime(&mut denied_runtime)?;
    stop_runtime(&mut ask_runtime)?;
    stop_runtime(&mut unavailable_runtime)?;
    let script = repo_root()
        .join("ui")
        .join("scripts")
        .join("m7-remediation-probe.mjs");
    check!(
        script.is_file(),
        "production remediation probe script is missing"
    );
    let payload = json!({
        "cases": [
            {
                "label": "disabled",
                "dto": disabled,
                "tool": "write",
                "expectedReason": "tool_disabled",
                "expectedAvailability": "disabled",
                "expectedRemediationKey": "m7.remediation.tool_disabled"
            },
            {
                "label": "denied",
                "dto": denied,
                "tool": "write",
                "expectedReason": "policy_denied",
                "expectedAvailability": "unavailable_by_policy",
                "expectedRemediationKey": "m7.remediation.policy_denied"
            },
            {
                "label": "ask",
                "dto": ask,
                "tool": "write",
                "expectedReason": "approval_required",
                "expectedAvailability": "available_with_approval",
                "expectedRemediationKey": "m7.remediation.approval_required"
            },
            {
                "label": "unavailable",
                "dto": unavailable,
                "tool": "bash",
                "expectedReason": "bash_executable_unavailable",
                "expectedAvailability": "unavailable_by_runtime",
                "expectedRemediationKey": "m7.remediation.bash_executable_unavailable"
            }
        ]
    });
    let mut child = Command::new("node")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn production remediation probe failed: {error}"))?;
    let input = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
    child
        .stdin
        .take()
        .ok_or_else(|| "production remediation probe stdin unavailable".to_owned())?
        .write_all(&input)
        .map_err(|error| error.to_string())?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait production remediation probe failed: {error}"))?;
    check!(
        output.status.success(),
        "production remediation probe failed"
    );
    let probe: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("production remediation probe returned invalid JSON: {error}"))?;
    let mut details = CaseEvidence::default();
    details.fixture = Some(disabled_installed.source.clone());
    details.pre_sha256 = Some(disabled_installed.before_sha256()?);
    details.post_sha256 = Some(disabled_installed.after_sha256()?);
    details.pre_schema = disabled_installed.before_json.get("schemaVersion").cloned();
    details.post_schema = read_json(&disabled_installed.target)?
        .get("schemaVersion")
        .cloned();
    details.pre_revision = disabled_installed
        .before_json
        .get("configRevision")
        .cloned();
    details.post_revision = read_json(&disabled_installed.target)?
        .get("configRevision")
        .cloned();
    details.result_code = Some("REMEDIATION_MAPPING_PRODUCTION_PASS".into());
    details.assertions = vec![
        "disabled reason maps to production tool_disabled remediation".into(),
        "policy denied reason maps to production policy_denied remediation".into(),
        "ASK reason maps to production approval_required remediation".into(),
        "Bash unavailable reason maps to production bash_executable_unavailable remediation".into(),
        "production en and zh-CN strings present".into(),
    ];
    details.runtime.insert("probe".into(), probe);
    let _ = (denied_installed, ask_installed, unavailable_installed);
    Ok(details)
}

fn run_e2e_20(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = runtime_fixture_config(
        fixture,
        "e2e-20",
        "v1-transitional-m6.json",
        allow_file_tools,
    )?;
    let store = installed.store();
    let (mut runtime, _mcp) = start_runtime(&config, "M7-E2E-20-A")?;
    let pid_a = runtime.pid();
    let runtime_id_a = runtime.runtime_id().to_owned();
    let current = control(&mut runtime, "runtime.status", json!({}))?;
    let mut changed = config.clone();
    changed.tools.write.enabled = false;
    let desired = store
        .save(changed)
        .map_err(|error| format!("save desired revision failed: {}", error.code()))?;
    check!(desired.config_revision.get() == config.config_revision.get() + 1);
    let saved = store
        .load()
        .map_err(|error| format!("reload desired revision failed: {}", error.code()))?
        .ok_or_else(|| "desired v2 disappeared".to_owned())?;
    check!(saved.config_revision == desired.config_revision);
    let active = control(&mut runtime, "runtime.status", json!({}))?;
    check!(active["configRevision"] == current["configRevision"]);
    check!(active["configRevision"] != json!(desired.config_revision.get().to_string()));
    let effective = control(&mut runtime, "control.effective_tools.get", json!({}))?;
    check!(effective_tool_reason(&effective, "write")? == "available");
    stop_runtime(&mut runtime)?;
    let (mut restarted, mut restarted_mcp) = start_runtime(&desired, "M7-E2E-20-B")?;
    let pid_b = restarted.pid();
    let runtime_id_b = restarted.runtime_id().to_owned();
    let restarted_status = control(&mut restarted, "runtime.status", json!({}))?;
    check!(restarted_status["configRevision"] == json!(desired.config_revision.get().to_string()));
    check!(!list_tool_names(
        restarted_mcp
            .list_tools()
            .map_err(|error| error.to_string())?
    )?
    .iter()
    .any(|name| name == "write"));
    stop_runtime(&mut restarted)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "DESIRED_EFFECTIVE_REVISION_MISMATCH_TRUTHFUL",
        vec![
            "saved_revision_n_plus_1",
            "current_runtime_remains_revision_n",
            "effective_tools_remain_current_generation",
            "restart_loads_revision_n_plus_1",
            "desired_effective_converge_after_restart",
        ],
    )?;
    details
        .runtime
        .insert("pidBeforeRestart".into(), json!(pid_a));
    details
        .runtime
        .insert("runtimeIdBeforeRestart".into(), json!(runtime_id_a));
    details
        .runtime
        .insert("pidAfterRestart".into(), json!(pid_b));
    details
        .runtime
        .insert("runtimeIdAfterRestart".into(), json!(runtime_id_b));
    details.runtime.insert("activeBeforeRestart".into(), active);
    details.runtime.insert(
        "savedRevision".into(),
        json!(desired.config_revision.get().to_string()),
    );
    details
        .runtime
        .insert("activeAfterRestart".into(), restarted_status);
    Ok(details)
}

fn run_e2e_21(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = runtime_fixture_config(
        fixture,
        "e2e-21",
        "v1-transitional-m6.json",
        allow_file_tools,
    )?;
    let store = installed.store();
    let (mut runtime, _mcp) = start_runtime(&config, "M7-E2E-21")?;
    let pid = runtime.pid();
    let runtime_id = runtime.runtime_id().to_owned();
    let before = control(&mut runtime, "runtime.status", json!({}))?;
    let changed = store
        .set_ui_language(Some(UiLanguage::En))
        .map_err(|error| format!("set UI language failed: {}", error.code()))?;
    check!(changed.config_revision == config.config_revision);
    check!(changed.ui.language == Some(UiLanguage::En));
    let reloaded = store
        .load()
        .map_err(|error| format!("reload UI language failed: {}", error.code()))?
        .ok_or_else(|| "UI language config disappeared".to_owned())?;
    check!(reloaded.config_revision == config.config_revision);
    check!(reloaded.ui.language == Some(UiLanguage::En));
    let after = control(&mut runtime, "runtime.status", json!({}))?;
    check!(after["configRevision"] == before["configRevision"]);
    check!(runtime.pid() == pid);
    check!(runtime.runtime_id() == runtime_id);
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "UI_LANGUAGE_REVISION_NEUTRAL",
        vec![
            "language_persisted",
            "config_revision_unchanged",
            "runtime_pid_and_id_unchanged",
            "no_restart_required_debt_observed",
        ],
    )?;
    details.runtime.insert("pid".into(), json!(pid));
    details
        .runtime
        .insert("runtimeId".into(), json!(runtime_id));
    details.runtime.insert("before".into(), before);
    details.runtime.insert("after".into(), after);
    Ok(details)
}

fn run_e2e_22(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-22", "v1-transitional-m6.json", |value| {
            allow_file_tools(value);
            value["tools"]["bash"]["enabled"] = json!(true);
            value["bash"]["autoDetect"] = json!(true);
            value["bash"]["executablePath"] = Value::Null;
            value["policies"]["toolDefaults"]["bash"] = json!("allow");
        })?;
    let output_sentinel = "M7_BASH_OUTPUT_SENTINEL";
    let env_name = "M7_BASH_ENV_SENTINEL";
    let env_value = "M7_BASH_ENV_VALUE_SENTINEL";
    fs::write(fixture.workspace.join("bash-output.txt"), output_sentinel)
        .map_err(|error| error.to_string())?;
    std::env::set_var(env_name, env_value);
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-22")?;
    std::env::remove_var(env_name);
    let write_sentinel = "M7_WRITE_ACTIVITY_SENTINEL";
    let edit_old = "M7_EDIT_ACTIVITY_OLD_SENTINEL";
    let edit_new = "M7_EDIT_ACTIVITY_NEW_SENTINEL";
    fs::write(fixture.workspace.join("edit.txt"), edit_old).map_err(|error| error.to_string())?;
    assert_mcp_success(
        &mcp.call_tool(
            "write",
            json!({ "path": "sentinel-write.txt", "content": write_sentinel }),
        )
        .map_err(|error| error.to_string())?,
    )?;
    assert_mcp_success(
        &mcp.call_tool(
            "edit",
            json!({ "path": "edit.txt", "oldText": edit_old, "newText": edit_new }),
        )
        .map_err(|error| error.to_string())?,
    )?;
    let bash_output_call = mcp.pending_tool("bash", json!({ "command": "cat bash-output.txt" }));
    let bash_output = receive_fast_bash(&mut runtime, bash_output_call)?;
    assert_mcp_success(&bash_output)?;
    println!(
        "M7_E2E_22_BASH_OUTPUT len={} expectedLen={} equal={}",
        bash_output["result"]["structuredContent"]["stdout"]
            .as_str()
            .map(str::len)
            .unwrap_or_default(),
        output_sentinel.len(),
        bash_output["result"]["structuredContent"]["stdout"] == json!(output_sentinel)
    );
    check!(bash_output["result"]["structuredContent"]["stdout"] == json!(output_sentinel));
    let bash_env_call = mcp.pending_tool(
        "bash",
        json!({ "command": "printenv M7_BASH_ENV_SENTINEL" }),
    );
    let bash_env = receive_fast_bash(&mut runtime, bash_env_call)?;
    assert_mcp_success(&bash_env)?;
    let expected_env_output = format!("{env_value}\n");
    println!(
        "M7_E2E_22_BASH_ENV len={} expectedLen={} equal={}",
        bash_env["result"]["structuredContent"]["stdout"]
            .as_str()
            .map(str::len)
            .unwrap_or_default(),
        expected_env_output.len(),
        bash_env["result"]["structuredContent"]["stdout"] == json!(expected_env_output)
    );
    check!(bash_env["result"]["structuredContent"]["stdout"] == json!(expected_env_output));
    let activity = activity_snapshot(&mut runtime)?;
    let diagnostics = control(&mut runtime, "control.diagnostics.get", json!({}))?;
    let serialized_activity =
        serde_json::to_string(&activity).map_err(|error| error.to_string())?;
    let serialized_diagnostics =
        serde_json::to_string(&diagnostics).map_err(|error| error.to_string())?;
    for sentinel in [
        write_sentinel,
        edit_old,
        edit_new,
        output_sentinel,
        env_value,
    ] {
        check!(!serialized_activity.contains(sentinel));
        check!(!serialized_diagnostics.contains(sentinel));
    }
    let control_error = control(
        &mut runtime,
        "control.activity.snapshot",
        json!({ "limit": env_value }),
    )
    .expect_err("invalid control read should return a typed error");
    for sentinel in [
        write_sentinel,
        edit_old,
        edit_new,
        output_sentinel,
        env_value,
    ] {
        check!(!control_error.contains(sentinel));
    }
    let evidence_dir = repo_root().join("target").join("m7-qa").join("e2e");
    if evidence_dir.exists() {
        for entry in fs::read_dir(&evidence_dir).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_file() {
                let bytes = fs::read(&path).map_err(|error| error.to_string())?;
                let text = String::from_utf8_lossy(&bytes);
                for sentinel in [
                    write_sentinel,
                    edit_old,
                    edit_new,
                    output_sentinel,
                    env_value,
                ] {
                    check!(!text.contains(sentinel));
                }
            }
        }
    }
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "ACTIVITY_SANITIZATION_SENTINELS_ABSENT",
        vec![
            "write_content_sentinel_absent",
            "edit_old_new_sentinels_absent",
            "bash_output_and_environment_sentinels_absent",
            "diagnostics_and_activity_scanned",
        ],
    )?;
    details
        .runtime
        .insert("activityEntries".into(), json!(activity.len()));
    details
        .runtime
        .insert("controlErrorSanitized".into(), json!(true));
    Ok(details)
}

fn run_e2e_23(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) = runtime_fixture_config(
        fixture,
        "e2e-23",
        "v1-transitional-m6.json",
        allow_file_tools,
    )?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-23")?;
    for _ in 0..270 {
        let response = mcp
            .call_tool("read_file", json!({ "path": "baseline.txt" }))
            .map_err(|error| error.to_string())?;
        assert_mcp_success(&response)?;
    }
    let activity = activity_snapshot(&mut runtime)?;
    check!(activity.len() == 256);
    check!(activity.iter().any(|item| {
        item["toolName"] == json!("read_file") && item["eventType"] == json!("execution_completed")
    }));
    let result = mcp
        .call_tool(
            "write",
            json!({ "path": "bounded-still-works.txt", "content": "bounded" }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_success(&result)?;
    check!(
        fs::read(fixture.workspace.join("bounded-still-works.txt")).map_err(|e| e.to_string())?
            == b"bounded"
    );
    let after = activity_snapshot(&mut runtime)?;
    check!(after.len() == 256);
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    stop_runtime(&mut runtime)?;
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "ACTIVITY_BOUNDED_256",
        vec![
            "more_than_capacity_safe_requests_generated",
            "snapshot_bounded_at_256",
            "latest_entries_readable",
            "runtime_authorization_and_execution_still_work",
        ],
    )?;
    details
        .runtime
        .insert("beforeWriteEntries".into(), json!(activity.len()));
    details
        .runtime
        .insert("afterWriteEntries".into(), json!(after.len()));
    details.runtime.insert("counters".into(), status);
    Ok(details)
}

fn run_e2e_24(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (allow_installed, allow_config) = runtime_fixture_config(
        fixture,
        "e2e-24-allow",
        "v1-transitional-m6.json",
        |value| {
            allow_file_tools(value);
            value["security"]["mode"] = json!("developer_test");
            value["security"]["developerTestMode"] = json!(true);
            value["tools"]["bash"]["enabled"] = json!(true);
            value["bash"]["autoDetect"] = json!(true);
            value["bash"]["executablePath"] = Value::Null;
            value["policies"]["toolDefaults"]["bash"] = json!("allow");
        },
    )?;
    let (mut allow_runtime, mut allow_mcp) = start_runtime(&allow_config, "M7-E2E-24-allow")?;
    assert_mcp_success(
        &allow_mcp
            .call_tool("read_file", json!({ "path": "baseline.txt" }))
            .map_err(|error| error.to_string())?,
    )?;
    assert_mcp_success(
        &allow_mcp
            .call_tool("glob", json!({ "pattern": "**/*.txt" }))
            .map_err(|error| error.to_string())?,
    )?;
    assert_mcp_success(
        &allow_mcp
            .call_tool(
                "write",
                json!({ "path": "m7-regression-write.txt", "content": "regression" }),
            )
            .map_err(|error| error.to_string())?,
    )?;
    assert_mcp_success(
        &allow_mcp
            .call_tool(
                "edit",
                json!({ "path": "m7-regression-write.txt", "oldText": "regression", "newText": "edited" }),
            )
            .map_err(|error| error.to_string())?,
    )?;
    check!(
        fs::read(fixture.workspace.join("m7-regression-write.txt")).map_err(|e| e.to_string())?
            == b"edited"
    );
    let bash_call = allow_mcp.pending_tool("bash", json!({ "command": "printf m7-regression" }));
    let bash_result = receive_fast_bash(&mut allow_runtime, bash_call)?;
    assert_mcp_success(&bash_result)?;
    check!(bash_result["result"]["structuredContent"]["stdout"] == json!("m7-regression"));
    stop_runtime(&mut allow_runtime)?;

    let (disabled_installed, disabled_config) = runtime_fixture_config(
        fixture,
        "e2e-24-disabled",
        "v1-transitional-m6.json",
        |value| value["tools"]["write"]["enabled"] = json!(false),
    )?;
    let (mut disabled_runtime, mut disabled_mcp) =
        start_runtime(&disabled_config, "M7-E2E-24-disabled")?;
    assert_mcp_error_code(
        &disabled_mcp
            .call_tool("write", json!({ "path": "m7-stale.txt", "content": "x" }))
            .map_err(|error| error.to_string())?,
        "TOOL_DISABLED",
    )?;
    stop_runtime(&mut disabled_runtime)?;

    let (denied_installed, denied_config) = runtime_fixture_config(
        fixture,
        "e2e-24-denied",
        "v1-transitional-m6.json",
        |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("deny");
            value["policies"]["userRules"] = json!([]);
        },
    )?;
    let (mut denied_runtime, mut denied_mcp) = start_runtime(&denied_config, "M7-E2E-24-denied")?;
    assert_mcp_error_code(
        &denied_mcp
            .call_tool("write", json!({ "path": "m7-denied.txt", "content": "x" }))
            .map_err(|error| error.to_string())?,
        "POLICY_DENIED",
    )?;
    stop_runtime(&mut denied_runtime)?;

    let (ask_installed, ask_config) =
        runtime_fixture_config(fixture, "e2e-24-ask", "v1-transitional-m6.json", |value| {
            value["tools"]["write"]["enabled"] = json!(true);
            value["policies"]["toolDefaults"]["write"] = json!("ask");
            value["policies"]["userRules"] = json!([]);
        })?;
    let (mut ask_runtime, mut ask_mcp) = start_runtime(&ask_config, "M7-E2E-24-ask")?;
    let ask_call = ask_mcp.pending_tool(
        "write",
        json!({ "path": "m7-ask.txt", "content": "ask-resumed" }),
    );
    let ask_approval = wait_pending_approval(&mut ask_runtime, "m7-ask.txt")?;
    let ask_id = ask_approval["requestId"]
        .as_str()
        .ok_or_else(|| "M7 regression ASK omitted requestId".to_owned())?
        .to_owned();
    let resolved = control(
        &mut ask_runtime,
        "approvals.resolve",
        json!({ "requestId": ask_id, "decision": "allow_once" }),
    )?;
    check!(resolved["approval"]["decision"] == json!("allow_once"));
    assert_mcp_success(&ask_call.receive().map_err(|error| error.to_string())?)?;
    check!(
        fs::read(fixture.workspace.join("m7-ask.txt")).map_err(|e| e.to_string())?
            == b"ask-resumed"
    );
    stop_runtime(&mut ask_runtime)?;
    let mut details = CaseEvidence::default();
    details.assertions = vec![
        "M0-M1 read_file/glob/write/edit real MCP PASS".into(),
        "M2 disabled stale call real MCP PASS".into(),
        "M3 policy DENY real MCP PASS".into(),
        "M4 ASK AllowOnce same-request resume real Control/MCP PASS".into(),
        "M5 Bash representative safe path real executor PASS".into(),
        "M6 migrated language/runtime revision behavior covered by E2E-21".into(),
    ];
    details.result_code = Some("M0_M6_REGRESSION_PASS".into());
    details.runtime.insert("askRequestId".into(), json!(ask_id));
    let _ = (
        allow_installed,
        disabled_installed,
        denied_installed,
        ask_installed,
    );
    Ok(details)
}

fn run_e2e_25(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-25", "v1-v2-era.json", |value| {
            value["security"]["mode"] = json!("normal");
            value["security"]["developerTestMode"] = json!(false);
        })?;
    let (mut runtime, _mcp) = start_runtime(&config, "M7-E2E-25")?;
    let endpoint = control(&mut runtime, "mcp.endpoint.get", json!({}))?;
    let before = control(&mut runtime, "runtime.status", json!({}))?;
    check!(before["activeRequests"] == json!(0));
    check!(before["pendingApprovals"] == json!(0));
    check!(before["runningExecutions"] == json!(0));
    stop_runtime(&mut runtime)?;
    check!(runtime.stopped_and_endpoint_closed());
    let owned_pids = owned_bash_pids();
    check!(
        owned_pids.iter().all(|pid| !process_exists(*pid)),
        "a Bash process-tree descendant remained after final shutdown"
    );
    let post_config = installed
        .store()
        .load()
        .map_err(|error| format!("migrated config reload failed: {}", error.code()))?
        .ok_or_else(|| "migrated config disappeared after clean shutdown".to_owned())?;
    check!(post_config.schema_version == 2);
    check!(!installed.temp_residue());
    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "CLEAN_SHUTDOWN",
        vec![
            "active_pending_running_zero_before_stop",
            "runtime_clean_exit",
            "mcp_endpoint_closed",
            "migrated_config_remains_valid_v2",
            "no_temp_residue",
        ],
    )?;
    details.runtime.insert("endpoint".into(), endpoint);
    details.runtime.insert("beforeCounters".into(), before);
    details
        .runtime
        .insert("ownedBashPids".into(), json!(owned_pids));
    Ok(details)
}

fn runtime_fixture_config<F>(
    fixture: &Fixture,
    name: &str,
    source: &str,
    mutate: F,
) -> M7Result<(InstalledFixture, AppConfig)>
where
    F: FnOnce(&mut Value),
{
    let mut raw = read_json(&fixture_path(source))?;
    raw["workspace"]["rootPath"] = json!(fixture.workspace.display().to_string());
    mutate(&mut raw);
    let installed = fixture.install_value(name, source, raw)?;
    let config = installed
        .store()
        .load()
        .map_err(|error| format!("runtime fixture ConfigStore load failed: {}", error.code()))?
        .ok_or_else(|| "runtime fixture ConfigStore returned no config".to_owned())?;
    check!(config.workspace.root_path == fixture.workspace.display().to_string());
    let case_id = CURRENT_CASE_ID
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .map_err(|_| "case evidence lock poisoned".to_owned())?
        .clone();
    let path = repo_root()
        .join("target")
        .join("m7-qa")
        .join("e2e")
        .join(format!("migration-{case_id}-{name}.jsonl"));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Migration evidence write failed: {error}"))?;
    serde_json::to_writer(
        &mut file,
        &json!({
            "recordType": "migration", "case": case_id, "variant": name,
            "fixture": source, "preSha256": installed.before_sha256()?,
            "postSha256": installed.after_sha256()?,
            "preSchemaVersion": installed.before_json["schemaVersion"],
            "postSchemaVersion": config.schema_version,
            "preConfigRevision": installed.before_json["configRevision"],
            "postConfigRevision": config.config_revision.get().to_string(),
            "tempResidue": installed.temp_residue(),
            "boundary": "production ConfigStore::load",
        }),
    )
    .map_err(|error| format!("Migration evidence serialization failed: {error}"))?;
    writeln!(file).map_err(|error| format!("Migration evidence append failed: {error}"))?;
    Ok((installed, config))
}

fn allow_file_tools(value: &mut Value) {
    for tool in ["readFile", "glob", "write", "edit"] {
        value["tools"][tool]["enabled"] = json!(true);
    }
    value["tools"]["bash"]["enabled"] = json!(false);
    for tool in ["readFile", "glob", "write", "edit"] {
        value["policies"]["toolDefaults"][tool] = json!("allow");
    }
    value["policies"]["userRules"] = json!([]);
}

fn latest_terminal<'a>(items: &'a [Value], tool: &str) -> M7Result<&'a Value> {
    items
        .iter()
        .rev()
        .find(|item| {
            item["toolName"] == json!(tool)
                && matches!(
                    item["eventType"].as_str(),
                    Some("execution_completed")
                        | Some("execution_failed")
                        | Some("request_denied")
                        | Some("approval_denied")
                        | Some("approval_cancelled")
                        | Some("approval_expired")
                )
        })
        .ok_or_else(|| format!("Activity omitted terminal evidence for {tool}"))
}

fn assert_file_evidence(
    item: &Value,
    tool: &str,
    logical_path: &str,
    capability: &str,
    decision: &str,
    policy_reason: &str,
) -> M7Result<()> {
    check!(item["requestId"].as_str().is_some());
    check!(item["toolName"] == json!(tool));
    check!(item["toolEnabled"] == json!(true));
    check!(item["capability"] == json!(capability));
    check!(item["decisionSummary"] == json!(decision));
    check!(item["policyReason"] == json!(policy_reason));
    check!(item["sanitizedArguments"]["tool"] == json!(tool));
    check!(item["sanitizedArguments"]["logicalPath"] == json!(logical_path));
    check!(item["sanitizedArguments"]["path"].is_null());
    Ok(())
}

fn wait_pending_approval(runtime: &mut RuntimeProcess, scope: &str) -> M7Result<Value> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let status = control(runtime, "runtime.status", json!({}))?;
        if status["pendingApprovals"]
            .as_u64()
            .is_some_and(|count| count >= 1)
        {
            let pending = control(runtime, "approvals.list", json!({ "status": "pending" }))?;
            if let Some(item) = pending["items"].as_array().and_then(|items| {
                items
                    .iter()
                    .find(|item| item["scopeSummary"] == json!(scope))
            }) {
                return Ok(item.clone());
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("pending approval for {scope} was not observable"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for_zero_counters(runtime: &mut RuntimeProcess) -> M7Result<Value> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let status = control(runtime, "runtime.status", json!({}))?;
        if status["activeRequests"] == json!(0)
            && status["pendingApprovals"] == json!(0)
            && status["runningExecutions"] == json!(0)
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Runtime counters did not converge to zero: {status}"
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn start_runtime(config: &AppConfig, generation: &str) -> M7Result<(RuntimeProcess, McpClient)> {
    let binary = m2_e2e::runtime_binary().map_err(|error| error.to_string())?;
    RuntimeProcess::start(&binary, config, generation).map_err(|error| error.to_string())
}

fn control(runtime: &mut RuntimeProcess, method: &str, params: Value) -> M7Result<Value> {
    let result = runtime
        .control_request(method, params)
        .map_err(|error| error.to_string());
    // Persist real safe read DTOs, never MCP raw content/arguments. This keeps
    // every observed request/approval ID and lifecycle counter auditable.
    let case_id = CURRENT_CASE_ID
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .map_err(|_| "case evidence lock poisoned".to_owned())?
        .clone();
    let path = repo_root()
        .join("target")
        .join("m7-qa")
        .join("e2e")
        .join(format!("control-{}-{case_id}.jsonl", runtime.pid()));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Control evidence write failed: {error}"))?;
    let observation = json!({
        "case": case_id,
        "runtimePid": runtime.pid(),
        "runtimeId": runtime.runtime_id(),
        "method": method,
        "result": match &result {
            Ok(dto) => json!({"ok": dto}),
            Err(error) => json!({"error": sanitize_error(error)}),
        },
    });
    serde_json::to_writer(&mut file, &observation)
        .map_err(|error| format!("Control evidence serialization failed: {error}"))?;
    writeln!(file).map_err(|error| format!("Control evidence append failed: {error}"))?;
    result
}

fn stop_runtime(runtime: &mut RuntimeProcess) -> M7Result<()> {
    runtime.stop().map_err(|error| error.to_string())
}

fn list_tool_names(value: Value) -> M7Result<Vec<String>> {
    value["result"]["tools"]
        .as_array()
        .or_else(|| value["items"].as_array())
        .ok_or_else(|| "tools/list did not return a tools array".to_owned())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["name"].as_str().map(ToOwned::to_owned))
                .collect()
        })
}

fn assert_mcp_success(result: &Value) -> M7Result<()> {
    if !result["error"].is_null() {
        return Err(format!(
            "MCP returned a JSON-RPC error code={}",
            result["error"]["data"]["code"]
        ));
    }
    if result["result"]["isError"] == json!(true) {
        return Err(format!(
            "MCP returned a tool error code={}",
            result["result"]["structuredContent"]["code"]
        ));
    }
    Ok(())
}

fn assert_mcp_error_code(result: &Value, expected: &str) -> M7Result<()> {
    check!(result["error"].is_null(), "MCP returned a transport error");
    check!(
        result["result"]["isError"] == json!(true),
        "MCP unexpectedly succeeded"
    );
    check!(
        result["result"]["structuredContent"]["code"] == json!(expected),
        "MCP error code mismatch: expected {expected}"
    );
    Ok(())
}

fn activity_snapshot(runtime: &mut RuntimeProcess) -> M7Result<Vec<Value>> {
    let value = control(
        runtime,
        "control.activity.snapshot",
        json!({ "limit": 256 }),
    )?;
    value
        .as_array()
        .or_else(|| value["items"].as_array())
        .cloned()
        .ok_or_else(|| "control.activity.snapshot did not return items".to_owned())
}

fn activity_delta(before: &[Value], after: &[Value]) -> Vec<Value> {
    let known = before
        .iter()
        .filter_map(|item| item["eventId"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    after
        .iter()
        .filter(|item| {
            item["eventId"]
                .as_str()
                .is_none_or(|id| !known.contains(id))
        })
        .cloned()
        .collect()
}

fn case_migration_evidence(
    installed: &InstalledFixture,
    post_json: &Value,
    result_code: &str,
    assertions: Vec<&str>,
) -> M7Result<CaseEvidence> {
    let post_sha256 = installed.after_sha256()?;
    Ok(CaseEvidence {
        fixture: Some(installed.source.clone()),
        pre_sha256: Some(installed.before_sha256()?),
        post_sha256: Some(post_sha256),
        pre_schema: installed.before_json.get("schemaVersion").cloned(),
        post_schema: post_json.get("schemaVersion").cloned(),
        pre_revision: installed.before_json.get("configRevision").cloned(),
        post_revision: post_json.get("configRevision").cloned(),
        result_code: Some(result_code.to_owned()),
        assertions: assertions.into_iter().map(str::to_owned).collect(),
        temp_residue: Some(installed.temp_residue()),
        runtime: BTreeMap::new(),
    })
}

fn fixture_path(name: &str) -> PathBuf {
    repo_root().join("fixtures").join("config").join(name)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ui/src-tauri is inside repository")
        .to_path_buf()
}

fn read_json(path: &Path) -> M7Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn bytes_sha256(bytes: &[u8]) -> M7Result<String> {
    let base = std::env::temp_dir().join(format!("m7-sha-{}.bin", uuid::Uuid::new_v4()));
    fs::write(&base, bytes).map_err(|error| error.to_string())?;
    let result = file_sha256(&base);
    let _ = fs::remove_file(&base);
    result
}

fn file_sha256(path: &Path) -> M7Result<String> {
    #[cfg(windows)]
    {
        let output = Command::new("certutil")
            .args(["-hashfile", &path.display().to_string(), "SHA256"])
            .output()
            .map_err(|error| error.to_string())?;
        check!(output.status.success(), "certutil hash failed");
        let text = String::from_utf8_lossy(&output.stdout);
        let line = text
            .lines()
            .find(|line| {
                line.trim().len() == 64 && line.trim().chars().all(|c| c.is_ascii_hexdigit())
            })
            .ok_or_else(|| "certutil returned no SHA-256 digest".to_owned())?;
        return Ok(line.trim().to_ascii_uppercase());
    }
    #[cfg(not(windows))]
    {
        let output = Command::new("sha256sum")
            .arg(path)
            .output()
            .map_err(|error| error.to_string())?;
        check!(output.status.success(), "sha256sum hash failed");
        let digest = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .ok_or_else(|| "sha256sum returned no digest".to_owned())?;
        Ok(digest.to_ascii_uppercase())
    }
}

fn process_ids_by_image(image: &str) -> M7Result<Vec<u32>> {
    #[cfg(windows)]
    {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {image}"), "/FO", "CSV", "/NH"])
            .output()
            .map_err(|error| format!("tasklist failed: {error}"))?;
        check!(output.status.success(), "tasklist returned failure");
        let text = String::from_utf8_lossy(&output.stdout);
        let mut pids = Vec::new();
        for line in text.lines() {
            let fields = line.split(',').map(|field| field.trim_matches('"'));
            let fields = fields.collect::<Vec<_>>();
            if fields.len() >= 2 {
                if let Ok(pid) = fields[1].parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
        return Ok(pids);
    }
    #[cfg(not(windows))]
    {
        let _ = image;
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug)]
struct ProcessInfo {
    pid: u32,
    parent_pid: u32,
    name: String,
}

fn process_tree_descendants(root_pid: u32) -> M7Result<Vec<ProcessInfo>> {
    #[cfg(windows)]
    {
        let script =
            "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name | ConvertTo-Json -Compress";
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|error| format!("query process tree failed: {error}"))?;
        check!(
            output.status.success(),
            "process tree query returned failure"
        );
        let raw: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("process tree JSON invalid: {error}"))?;
        let rows = raw.as_array().cloned().unwrap_or_else(|| vec![raw]);
        let processes = rows
            .into_iter()
            .filter_map(|row| {
                Some(ProcessInfo {
                    pid: row["ProcessId"].as_u64()? as u32,
                    parent_pid: row["ParentProcessId"].as_u64()? as u32,
                    name: row["Name"].as_str().unwrap_or("<unknown>").to_owned(),
                })
            })
            .collect::<Vec<_>>();
        let mut frontier = vec![root_pid];
        let mut descendants = Vec::new();
        while let Some(parent) = frontier.pop() {
            for process in processes.iter().filter(|item| item.parent_pid == parent) {
                if descendants
                    .iter()
                    .any(|item: &ProcessInfo| item.pid == process.pid)
                {
                    continue;
                }
                descendants.push(process.clone());
                frontier.push(process.pid);
            }
        }
        return Ok(descendants);
    }
    #[cfg(not(windows))]
    {
        let _ = root_pid;
        Ok(Vec::new())
    }
}

fn remember_owned_bash_pids(pids: &[u32]) {
    let storage = OWNED_BASH_PIDS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut values) = storage.lock() {
        for pid in pids {
            if !values.contains(pid) {
                values.push(*pid);
            }
        }
    }
}

fn owned_bash_pids() -> Vec<u32> {
    OWNED_BASH_PIDS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .map(|values| values.clone())
        .unwrap_or_default()
}

fn process_exists(pid: u32) -> bool {
    #[cfg(windows)]
    {
        return Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!("Get-Process -Id {pid} -ErrorAction SilentlyContinue"),
            ])
            .output()
            .map(|output| !output.stdout.is_empty())
            .unwrap_or(false);
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}

fn scan_final_evidence() -> M7Result<()> {
    let directory = repo_root().join("target").join("m7-qa").join("e2e");
    if !directory.exists() {
        return Err("M7 E2E evidence directory is missing".into());
    }
    let forbidden = [
        SECRET_SENTINEL,
        "FAKE_SECRET_SENTINEL",
        "M7_WRITE_CONTENT_SENTINEL",
        "M7_EDIT_ACTIVITY_OLD_SENTINEL",
        "M7_EDIT_ACTIVITY_NEW_SENTINEL",
        "M7_BASH_OUTPUT_SENTINEL",
        "M7_BASH_ENV_VALUE_SENTINEL",
    ];
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        for value in forbidden {
            if text.contains(value) {
                return Err(format!("forbidden sentinel appeared in {}", path.display()));
            }
        }
    }
    Ok(())
}

fn sanitize_error(error: &str) -> String {
    if error.contains(SECRET_SENTINEL) {
        "error contained a forbidden sentinel".into()
    } else {
        error.chars().take(512).collect()
    }
}

// Keep these imports and helpers close to the runner while the Runtime cases
// are added.  They also make it explicit that future cases use the same real
// subprocess/MCP boundary as M2, rather than an internal test double.
#[allow(dead_code)]
fn _runtime_boundary_types(_runtime: Option<&mut RuntimeProcess>, _mcp: Option<&mut McpClient>) {}
