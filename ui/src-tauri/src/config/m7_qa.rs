//! M7 configuration Integration QA.
//!
//! These cases intentionally exercise the production `ConfigStore` boundary
//! with source-controlled fixtures and disposable on-disk config directories.
//! They are kept separate from the implementation unit tests so the M7 report
//! can map every required Integration case to an explicit command and result.

use super::{ConfigError, ConfigStore, CURRENT_SCHEMA_VERSION};
use crate::secrets::{MemorySecretStore, SecretStore};
use harness_control_protocol::{
    AppConfig, BashConfig, DecimalU64, PolicyConfig, PolicyDecision, SecurityMode, ToolConfig,
    UiLanguage,
};
use serde_json::{json, Value};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

const COMMAND: &str =
    "cargo test -p chatgpt-harness-ui config::m7_qa::m7_config_integration_cases --lib -- --nocapture";

type QaResult<T = ()> = Result<T, String>;

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> QaResult<Self> {
        tempfile::tempdir()
            .map(|root| Self { root })
            .map_err(|error| format!("create disposable M7 config root: {error}"))
    }

    fn directory(&self, case: &str) -> PathBuf {
        self.root.path().join(case)
    }

    fn store(&self, case: &str) -> QaResult<ConfigStore> {
        let directory = self.directory(case);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("create {case} config directory: {error}"))?;
        Ok(ConfigStore::new(directory))
    }
}

#[derive(Default)]
struct Evidence {
    cases: Vec<String>,
    failures: Vec<String>,
}

impl Evidence {
    fn pass(&mut self, id: &str, detail: impl Into<String>) {
        let detail = detail.into();
        println!("{id} PASS — {detail}");
        self.cases.push(format!("| {id} | PASS | {detail} |"));
    }

    fn fail(&mut self, id: &str, detail: impl Into<String>) {
        let detail = detail.into();
        println!("{id} FAIL — {detail}");
        self.cases.push(format!("| {id} | FAIL | {detail} |"));
        self.failures.push(id.to_owned());
    }

    fn write_report(&self) -> QaResult {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("m7-qa")
            .join("config-integration");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("create config Integration evidence directory: {error}"))?;
        let report = format!(
            concat!(
                "# M7 Config Integration QA\n\nCommand: `{}`\n\n",
                "Boundary: production `ConfigStore` with disposable on-disk directories; ",
                "source-controlled fixtures under `fixtures/config/`.\n\n",
                "No Manual Acceptance, Portable, or External ChatGPT case was run by this test.\n\n",
                "| Case | Status | Evidence |\n|---|---|---|\n{}\n\nResult: {}\n"
            ),
            COMMAND,
            self.cases.join("\n"),
            if self.failures.is_empty() {
                "CONFIG_INTEGRATION_PASS"
            } else {
                "CONFIG_INTEGRATION_FAIL"
            }
        );
        fs::write(directory.join("INT_CONFIG_RESULTS.md"), report)
            .map_err(|error| format!("write config Integration evidence: {error}"))?;
        Ok(())
    }
}

#[test]
fn m7_config_integration_cases() {
    let fixture = Fixture::new().expect("create M7 config Integration fixture");
    let mut evidence = Evidence::default();

    run_case("INT-01", int_01(&fixture), &mut evidence);
    run_case("INT-02", int_02(&fixture), &mut evidence);
    run_case("INT-03", int_03(&fixture), &mut evidence);
    run_case("INT-04", int_04(&fixture), &mut evidence);
    run_case("INT-05", int_05(&fixture), &mut evidence);
    run_case("INT-06", int_06(&fixture), &mut evidence);
    run_case("INT-07", int_07(&fixture), &mut evidence);
    run_case("INT-08", int_08(&fixture), &mut evidence);
    run_case("INT-09", int_09(&fixture), &mut evidence);
    run_case("INT-10", int_10(&fixture), &mut evidence);
    run_case("INT-11", int_11(&fixture), &mut evidence);
    run_case("INT-12", int_12(&fixture), &mut evidence);
    run_case("INT-13", int_13(&fixture), &mut evidence);
    run_case("INT-14", int_14(&fixture), &mut evidence);
    run_case("INT-15", int_15(&fixture), &mut evidence);
    run_case("INT-16", int_16(&fixture), &mut evidence);
    run_case("INT-17", int_17(&fixture), &mut evidence);
    run_case("INT-18", int_18(&fixture), &mut evidence);
    run_case("INT-36", int_36(&fixture), &mut evidence);

    evidence.write_report().expect("write M7 config evidence");
    assert!(
        evidence.failures.is_empty(),
        "failed M7 config Integration cases: {:?}",
        evidence.failures
    );
}

fn run_case(id: &str, result: QaResult<String>, evidence: &mut Evidence) {
    match result {
        Ok(detail) => evidence.pass(id, detail),
        Err(error) => evidence.fail(id, error),
    }
}

fn source_fixture(name: &str) -> QaResult<Value> {
    let source = match name {
        "v1-v2-era" => include_str!("../../../../fixtures/config/v1-v2-era.json"),
        "v1-transitional-m6" => {
            include_str!("../../../../fixtures/config/v1-transitional-m6.json")
        }
        "v1-transitional-m2-tools" => {
            include_str!("../../../../fixtures/config/v1-transitional-m2-tools.json")
        }
        "v1-transitional-m3-policy" => {
            include_str!("../../../../fixtures/config/v1-transitional-m3-policy.json")
        }
        "v1-transitional-m5-bash" => {
            include_str!("../../../../fixtures/config/v1-transitional-m5-bash.json")
        }
        "v1-transitional-m6-i18n" => {
            include_str!("../../../../fixtures/config/v1-transitional-m6-i18n.json")
        }
        "v1-invalid-policy" => {
            include_str!("../../../../fixtures/config/v1-invalid-policy.json")
        }
        "v1-invalid-proxy" => {
            include_str!("../../../../fixtures/config/v1-invalid-proxy.json")
        }
        "v1-unsupported-field" => {
            include_str!("../../../../fixtures/config/v1-unsupported-field.json")
        }
        "v1-future-schema" => {
            include_str!("../../../../fixtures/config/v1-future-schema.json")
        }
        "v1-invalid-secret-like-field" => {
            include_str!("../../../../fixtures/config/v1-invalid-secret-like-field.json")
        }
        "v2-current" => include_str!("../../../../fixtures/config/v2-current.json"),
        _ => return Err(format!("unknown source-controlled fixture {name}")),
    };
    serde_json::from_str(source).map_err(|error| format!("parse fixture {name}: {error}"))
}

fn write_value(store: &ConfigStore, value: &Value) -> QaResult<Vec<u8>> {
    let directory = store.metadata().config_file;
    let path = PathBuf::from(directory);
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn read_file(store: &ConfigStore) -> QaResult<Vec<u8>> {
    fs::read(store.metadata().config_file).map_err(|error| error.to_string())
}

fn schema(store: &ConfigStore) -> QaResult<u64> {
    let value: Value =
        serde_json::from_slice(&read_file(store)?).map_err(|error| error.to_string())?;
    value["schemaVersion"]
        .as_u64()
        .ok_or_else(|| "persisted schemaVersion is not an integer".into())
}

fn temp_residue(store: &ConfigStore) -> bool {
    PathBuf::from(store.metadata().config_file)
        .with_extension("json.tmp")
        .exists()
}

fn error_code(result: Result<Option<AppConfig>, ConfigError>) -> QaResult<String> {
    Ok(match result {
        Ok(_) => "NO_ERROR".to_owned(),
        Err(error) => error.code().to_owned(),
    })
}

fn byte_digest(bytes: &[u8]) -> String {
    // The assertions compare complete byte arrays. This compact digest is
    // only an evidence label; Local E2E records cryptographic SHA-256 at its
    // process boundary.
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn assert_original_unchanged(
    store: &ConfigStore,
    original: &[u8],
    expected_code: &str,
) -> QaResult<String> {
    let code = error_code(store.load())?;
    if code != expected_code {
        return Err(format!("expected {expected_code}, got {code}"));
    }
    let current = read_file(store)?;
    if current != original {
        return Err(format!(
            "source changed after {expected_code}: {} -> {}",
            byte_digest(original),
            byte_digest(&current)
        ));
    }
    Ok(format!(
        "code={expected_code}, sourceByteDigest={}, bytes unchanged, tempResidue={}",
        byte_digest(original),
        temp_residue(store)
    ))
}

fn int_01(fixture: &Fixture) -> QaResult<String> {
    if CURRENT_SCHEMA_VERSION != 2 {
        return Err(format!("CURRENT_SCHEMA_VERSION={CURRENT_SCHEMA_VERSION}"));
    }
    let defaults = ConfigStore::safe_defaults();
    if defaults.schema_version != 2
        || defaults.tools != ToolConfig::default()
        || defaults.policies != PolicyConfig::default()
        || defaults.bash != BashConfig::default()
    {
        return Err("safe defaults are not complete V3 defaults".into());
    }
    let store = fixture.store("int-01")?;
    let loaded = store
        .load_or_create_defaults()
        .map_err(|error| format!("default load failed: {}", error.code()))?;
    let wire_protocol = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocol/src/v1");
    if loaded.schema_version != 2 || !wire_protocol.is_dir() {
        return Err("schema/protocol boundary assertion failed".into());
    }
    Ok("CURRENT_SCHEMA_VERSION=2, default schemaVersion=2, V3 defaults complete, wire=protocol/src/v1".into())
}

fn int_02(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-02")?;
    let raw = source_fixture("v1-v2-era")?;
    let original = write_value(&store, &raw)?;
    let loaded = store
        .load()
        .map_err(|error| format!("migration failed: {}", error.code()))?
        .ok_or_else(|| "migrated config missing".to_owned())?;
    let expected: AppConfig = serde_json::from_value(raw).map_err(|error| error.to_string())?;
    if loaded.schema_version != 2
        || loaded.config_revision != DecimalU64::new(41)
        || loaded.workspace != expected.workspace
        || loaded.tunnel != expected.tunnel
        || loaded.security.mode != expected.security.mode
        || loaded.ui.theme != expected.ui.theme
        || loaded.tools != ToolConfig::default()
        || loaded.policies != PolicyConfig::default()
        || loaded.bash != BashConfig::default()
        || loaded.ui.language.is_some()
        || schema(&store)? != 2
        || temp_residue(&store)
    {
        return Err("pure V2-era migration preservation/default assertion failed".into());
    }
    let persisted: AppConfig =
        serde_json::from_slice(&read_file(&store)?).map_err(|error| error.to_string())?;
    if persisted != loaded || original == read_file(&store)? {
        return Err("migration did not atomically persist a v2 representation".into());
    }
    Ok(format!(
        "fixture=v1-v2-era, schema=1→2, revision=41→41, preserved workspace/tunnel/security/theme, defaults applied, sourceByteDigest={}, persistedByteDigest={}",
        byte_digest(&original),
        byte_digest(&read_file(&store)?)
    ))
}

fn int_03(fixture: &Fixture) -> QaResult<String> {
    let staged = [
        "v1-transitional-m2-tools",
        "v1-transitional-m3-policy",
        "v1-transitional-m5-bash",
        "v1-transitional-m6-i18n",
        "v1-transitional-m6",
    ];
    for name in staged {
        let store = fixture.store(&format!("int-03-{name}"))?;
        let raw = source_fixture(name)?;
        let expected: AppConfig =
            serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
        write_value(&store, &raw)?;
        let loaded = store
            .load()
            .map_err(|error| format!("{name} transitional migration failed: {}", error.code()))?
            .ok_or_else(|| format!("{name} transitional config missing"))?;
        if loaded.schema_version != 2
            || loaded.config_revision != expected.config_revision
            || loaded.tools != expected.tools
            || loaded.policies != expected.policies
            || loaded.bash != expected.bash
            || loaded.ui.language != expected.ui.language
            || loaded.workspace != expected.workspace
            || loaded.tunnel != expected.tunnel
            || loaded.security.mode != expected.security.mode
            || temp_residue(&store)
        {
            return Err(format!("{name} transitional settings were not preserved"));
        }
    }
    Ok("staged fixtures M2 tools, M3 policy, M5 Bash, M6 language plus combined M6 preserved all present settings and defaults".into())
}

fn int_04(fixture: &Fixture) -> QaResult<String> {
    for (case, name) in [
        ("int-04-pure", "v1-v2-era"),
        ("int-04-transitional", "v1-transitional-m6"),
    ] {
        let store = fixture.store(case)?;
        let raw = source_fixture(name)?;
        let expected_revision = raw["configRevision"].as_str().unwrap_or_default();
        write_value(&store, &raw)?;
        let loaded = store
            .load()
            .map_err(|error| format!("{name} migration failed: {}", error.code()))?
            .ok_or_else(|| "config missing".to_owned())?;
        if loaded.config_revision.get().to_string() != expected_revision {
            return Err(format!(
                "{name} revision changed {expected_revision}→{}",
                loaded.config_revision.get()
            ));
        }
    }
    Ok("pure and transitional fixtures preserve configRevision=41 without increment".into())
}

fn int_05(fixture: &Fixture) -> QaResult<String> {
    let cases = [
        ("normal", false, SecurityMode::Normal, false),
        ("developer", false, SecurityMode::DeveloperTest, true),
    ];
    for (name, _original_bool, mode, expected_bool) in cases {
        let store = fixture.store(&format!("int-05-{name}"))?;
        let mut raw = source_fixture("v1-v2-era")?;
        raw["security"]["mode"] = match &mode {
            SecurityMode::Normal => json!("normal"),
            SecurityMode::DeveloperTest => json!("developer_test"),
        };
        raw["security"]["developerTestMode"] = json!(!expected_bool);
        write_value(&store, &raw)?;
        let loaded = store
            .load()
            .map_err(|error| format!("security migration failed: {}", error.code()))?
            .ok_or_else(|| "config missing".to_owned())?;
        if loaded.security.mode != mode || loaded.security.developer_test_mode != expected_bool {
            return Err(format!("security mode was not authoritative for {name}"));
        }
    }
    Ok(
        "mode=normal normalized developerTestMode=false; mode=developer_test normalized true"
            .into(),
    )
}

fn int_06(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-06")?;
    let raw = source_fixture("v1-v2-era")?;
    let expected: AppConfig =
        serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
    let secrets = MemorySecretStore::default();
    secrets
        .set("opaque-fake-reference", "API_SECRET_CONTENT_SENTINEL")
        .map_err(|error| format!("seed secret fixture: {error}"))?;
    write_value(&store, &raw)?;
    let loaded = store
        .load()
        .map_err(|error| format!("tunnel migration failed: {}", error.code()))?
        .ok_or_else(|| "config missing".to_owned())?;
    let persisted = String::from_utf8(read_file(&store)?).map_err(|error| error.to_string())?;
    if loaded.tunnel.tunnel_id != expected.tunnel.tunnel_id
        || loaded.tunnel.control_plane_http_proxy != expected.tunnel.control_plane_http_proxy
        || loaded.tunnel.tunnel_client_path != expected.tunnel.tunnel_client_path
        || loaded.tunnel.auto_detect != expected.tunnel.auto_detect
        || loaded.tunnel.api_key_ref != expected.tunnel.api_key_ref
        || persisted.contains("API_SECRET_CONTENT_SENTINEL")
        || persisted.contains("raw-api-key")
        || secrets
            .get("opaque-fake-reference")
            .map_err(|error| format!("read secret fixture: {error}"))?
            .as_deref()
            != Some("API_SECRET_CONTENT_SENTINEL")
    {
        return Err("tunnel/API key reference preservation or secret boundary failed".into());
    }
    Ok("tunnelId/proxy/clientPath/autoDetect/apiKeyRef preserved; persisted bytes contain no raw secret".into())
}

fn int_07(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-07")?;
    let raw = source_fixture("v1-v2-era")?;
    write_value(&store, &raw)?;
    let loaded = store
        .load()
        .map_err(|error| format!("default migration failed: {}", error.code()))?
        .ok_or_else(|| "config missing".to_owned())?;
    let defaults = ConfigStore::safe_defaults();
    if loaded.tools != defaults.tools
        || loaded.policies != defaults.policies
        || loaded.bash != defaults.bash
        || loaded.ui.language != None
        || !loaded.tools.read_file.enabled
        || !loaded.tools.glob.enabled
        || !loaded.tools.write.enabled
        || !loaded.tools.edit.enabled
        || loaded.tools.bash.enabled
        || loaded.policies.tool_defaults.read_file != PolicyDecision::Allow
        || loaded.policies.tool_defaults.glob != PolicyDecision::Allow
        || loaded.policies.tool_defaults.write != PolicyDecision::Allow
        || loaded.policies.tool_defaults.edit != PolicyDecision::Allow
        || loaded.policies.tool_defaults.bash != PolicyDecision::Ask
        || !loaded.policies.user_rules.is_empty()
        || !loaded.bash.auto_detect
        || loaded.bash.executable_path.is_some()
    {
        return Err("frozen missing-V3 defaults mismatch".into());
    }
    Ok("tools read/glob/write/edit ON, bash OFF; policy ALLOW/ASK; userRules=[]; Bash autoDetect=true/path=null; language=null".into())
}

fn int_08(fixture: &Fixture) -> QaResult<String> {
    let base = source_fixture("v1-transitional-m6")?;
    for name in [
        "write-type",
        "policy-matcher",
        "bash-type",
        "language-value",
    ] {
        let store = fixture.store(&format!("int-08-{name}"))?;
        let mut raw = base.clone();
        match name {
            "write-type" => raw["tools"]["write"]["enabled"] = json!("yes"),
            "policy-matcher" => {
                raw["policies"]["userRules"][0]["matcher"] = json!({"type":"scope","tool":"write"})
            }
            "bash-type" => raw["bash"]["autoDetect"] = json!("true"),
            "language-value" => raw["ui"]["language"] = json!("fr-FR"),
            _ => unreachable!(),
        }
        let original = write_value(&store, &raw)?;
        let detail = assert_original_unchanged(&store, &original, "CONFIG_MIGRATION_FAILED")?;
        if temp_residue(&store) {
            return Err(format!("{name} left a migration temp residue"));
        }
        if detail.is_empty() {
            return Err("missing malformed-field evidence".into());
        }
    }
    Ok(
        "malformed tools/policy matcher/Bash/language each failed closed with exact source bytes"
            .into(),
    )
}

fn int_09(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-09")?;
    let raw = source_fixture("v1-invalid-policy")?;
    let original = write_value(&store, &raw)?;
    assert_original_unchanged(&store, &original, "CONFIG_MIGRATION_FAILED")
        .map(|detail| format!("enabled invalid policy rejected as a whole; {detail}"))
}

fn int_10(fixture: &Fixture) -> QaResult<String> {
    let base = source_fixture("v1-v2-era")?;
    let cases = [
        ("zero", Some(json!(0))),
        ("future", Some(json!(3))),
        ("missing", None),
        ("wrong-type", Some(json!("1"))),
    ];
    for (name, version) in cases {
        let store = fixture.store(&format!("int-10-{name}"))?;
        let mut raw = if name == "future" {
            source_fixture("v1-future-schema")?
        } else {
            base.clone()
        };
        if let Some(version) = version {
            raw["schemaVersion"] = version;
        } else {
            raw.as_object_mut()
                .ok_or_else(|| "fixture root is not an object".to_owned())?
                .remove("schemaVersion");
        }
        let original = write_value(&store, &raw)?;
        assert_original_unchanged(&store, &original, "CONFIG_SCHEMA_UNSUPPORTED")?;
    }
    Ok("schemaVersion 0/future/missing/wrong-type rejected with CONFIG_SCHEMA_UNSUPPORTED and unchanged bytes".into())
}

fn int_11(fixture: &Fixture) -> QaResult<String> {
    let fields = [
        "workspace",
        "tunnel",
        "security",
        "ui",
        "tools",
        "policies",
        "bash",
    ];
    for field in fields {
        let store = fixture.store(&format!("int-11-{field}"))?;
        let mut raw = if matches!(field, "tools" | "policies" | "bash") {
            source_fixture("v1-transitional-m6")?
        } else {
            source_fixture("v1-v2-era")?
        };
        match field {
            "tools" => raw["tools"]["write"]["m7UnknownOwnedField"] = json!(true),
            "policies" => raw["policies"]["m7UnknownOwnedField"] = json!(true),
            "bash" => raw["bash"]["m7UnknownOwnedField"] = json!(true),
            _ => raw[field]["m7UnknownOwnedField"] = json!(true),
        }
        let original = write_value(&store, &raw)?;
        assert_original_unchanged(&store, &original, "CONFIG_MIGRATION_FAILED")?;
    }
    Ok("unknown fields in migration-owned workspace/tunnel/security/ui/tools/policies/bash objects failed closed; none were dropped".into())
}

fn int_12(fixture: &Fixture) -> QaResult<String> {
    for name in ["v1-unsupported-field", "v1-invalid-secret-like-field"] {
        let store = fixture.store(&format!("int-12-{name}"))?;
        let raw = source_fixture(name)?;
        let original = write_value(&store, &raw)?;
        let result = store.load();
        let message = result
            .as_ref()
            .err()
            .map(ConfigError::message)
            .unwrap_or_default();
        assert_original_unchanged(&store, &original, "CONFIG_MIGRATION_FAILED")?;
        let persisted = read_file(&store)?;
        let persisted_json: Value =
            serde_json::from_slice(&persisted).map_err(|error| error.to_string())?;
        if message.contains("SECRET_SENTINEL") || persisted_json["schemaVersion"] == json!(2) {
            return Err(format!(
                "{name} raw secret sentinel leaked through error or a v2 replacement was written"
            ));
        }
    }
    Ok("source-controlled raw-secret fixtures rejected without echo or v2 persistence".into())
}

fn int_13(fixture: &Fixture) -> QaResult<String> {
    let sources = [
        "v1-v2-era",
        "v1-transitional-m2-tools",
        "v1-transitional-m3-policy",
        "v1-transitional-m5-bash",
        "v1-transitional-m6-i18n",
        "v1-transitional-m6",
    ];
    let mut digests = Vec::new();
    for name in sources {
        let store = fixture.store(&format!("int-13-{name}"))?;
        let raw = source_fixture(name)?;
        write_value(&store, &raw)?;
        let loaded = store
            .load()
            .map_err(|error| format!("{name} migration persist failed: {}", error.code()))?
            .ok_or_else(|| format!("{name} config missing"))?;
        let bytes = read_file(&store)?;
        let parsed: AppConfig =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        if parsed != loaded || schema(&store)? != 2 || temp_residue(&store) {
            return Err(format!("{name} persisted result is not complete valid v2"));
        }
        digests.push(format!("{name}={}", byte_digest(&bytes)));
    }
    Ok(format!(
        "all six positive historical fixtures atomically persisted valid v2 with no temp residue: {}",
        digests.join(", ")
    ))
}

fn int_14(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-14")?;
    let raw = source_fixture("v1-v2-era")?;
    let original = write_value(&store, &raw)?;
    let temporary = PathBuf::from(store.metadata().config_file).with_extension("json.tmp");
    fs::create_dir(&temporary)
        .map_err(|error| format!("create deterministic replace blocker: {error}"))?;
    let code = error_code(store.load())?;
    let current = read_file(&store)?;
    let blocker_still_present = temporary.is_dir();
    fs::remove_dir(&temporary).map_err(|error| format!("remove deterministic blocker: {error}"))?;
    if code != "CONFIG_WRITE_FAILED" || current != original || !blocker_still_present {
        return Err(format!("atomic failure contract mismatch: code={code}, sourceUnchanged={}, blocker={blocker_still_present}", current == original));
    }
    Ok(format!("deterministic atomic replace failure code=CONFIG_WRITE_FAILED, originalByteDigest={}, source unchanged, no partial v2; pre-existing temp blocker cleaned by fixture teardown", byte_digest(&original)))
}

fn int_15(fixture: &Fixture) -> QaResult<String> {
    let sources = [
        "v1-v2-era",
        "v1-transitional-m2-tools",
        "v1-transitional-m3-policy",
        "v1-transitional-m5-bash",
        "v1-transitional-m6-i18n",
        "v1-transitional-m6",
    ];
    for name in sources {
        let store = fixture.store(&format!("int-15-{name}"))?;
        let raw = source_fixture(name)?;
        write_value(&store, &raw)?;
        let first = store
            .load()
            .map_err(|error| format!("{name} first migration failed: {}", error.code()))?
            .ok_or_else(|| format!("{name} first config missing"))?;
        let bytes = read_file(&store)?;
        let second = store
            .load()
            .map_err(|error| format!("{name} second v2 load failed: {}", error.code()))?
            .ok_or_else(|| format!("{name} second config missing"))?;
        if first != second
            || bytes != read_file(&store)?
            || second.config_revision != DecimalU64::new(41)
        {
            return Err(format!(
                "{name} second load rewrote bytes or changed revision"
            ));
        }
    }
    Ok("all six positive historical fixtures stable on second load with revision unchanged".into())
}

fn int_16(fixture: &Fixture) -> QaResult<String> {
    // Each subcase starts from a real v2 fixture so revision semantics are
    // checked at the ordinary ConfigStore save boundary, not by a helper.
    let baseline = source_fixture("v2-current")?;
    let baseline_config: AppConfig =
        serde_json::from_value(baseline).map_err(|error| error.to_string())?;
    let mut details = Vec::new();

    {
        let store = fixture.store("int-16-identical")?;
        let initial = store
            .save(baseline_config.clone())
            .map_err(|error| format!("identical save: {}", error.code()))?;
        let saved = store
            .save(baseline_config.clone())
            .map_err(|error| format!("repeat save: {}", error.code()))?;
        if saved.config_revision != initial.config_revision {
            return Err("identical runtime semantics incremented revision".into());
        }
        details.push(format!("identical={}", saved.config_revision.get()));
    }

    {
        let store = fixture.store("int-16-ui")?;
        let initial = store
            .save(baseline_config.clone())
            .map_err(|error| error.code().to_owned())?;
        let mut ui = initial.clone();
        ui.ui.theme = Some("dark".into());
        ui.ui.language = Some(UiLanguage::ZhCn);
        let saved = store
            .save(ui)
            .map_err(|error| format!("UI save: {}", error.code()))?;
        if saved.config_revision != initial.config_revision {
            return Err("theme/language UI-only save incremented revision".into());
        }
        details.push(format!("uiOnly={}", saved.config_revision.get()));
    }

    for name in ["tool", "policy", "security", "bash", "workspace"] {
        let store = fixture.store(&format!("int-16-{name}"))?;
        let initial = store
            .save(baseline_config.clone())
            .map_err(|error| error.code().to_owned())?;
        let mut changed = initial.clone();
        match name {
            "tool" => changed.tools.write.enabled = !changed.tools.write.enabled,
            "policy" => changed.policies.tool_defaults.write = PolicyDecision::Allow,
            "security" => {
                changed.security.mode = SecurityMode::Normal;
                changed.security.developer_test_mode = false;
            }
            "bash" => changed.bash.auto_detect = !changed.bash.auto_detect,
            "workspace" => changed.workspace.root_path.push_str("-next"),
            _ => unreachable!(),
        }
        let saved = store
            .save(changed)
            .map_err(|error| format!("{name} save: {}", error.code()))?;
        if saved.config_revision != DecimalU64::new(initial.config_revision.get() + 1) {
            return Err(format!(
                "{name} runtime-affecting change did not increment revision"
            ));
        }
        details.push(format!(
            "{name}={}→{}",
            initial.config_revision.get(),
            saved.config_revision.get()
        ));
    }
    Ok(format!("v2 save semantics: {}", details.join(", ")))
}

fn int_17(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-17")?;
    let raw = source_fixture("v1-invalid-policy")?;
    let original = write_value(&store, &raw)?;
    if error_code(store.load())? != "CONFIG_MIGRATION_FAILED" {
        return Err("configGet/load unexpectedly succeeded".into());
    }
    let metadata = store.metadata();
    if metadata.config_file != store.metadata().config_file
        || metadata.config_directory != store.metadata().config_directory
        || !Path::new(&metadata.config_file).is_file()
        || read_file(&store)? != original
    {
        return Err("config metadata/recovery path unavailable after migration failure".into());
    }
    Ok(format!(
        "config load failed closed; metadata retained configDirectory={} configFile={}",
        metadata.config_directory, metadata.config_file
    ))
}

fn int_18(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-18")?;
    let raw = source_fixture("v1-invalid-policy")?;
    let original = write_value(&store, &raw)?;
    if error_code(store.load())? != "CONFIG_MIGRATION_FAILED" || read_file(&store)? != original {
        return Err("migration failure changed source or did not fail".into());
    }
    if !Path::new(&store.metadata().config_file).is_file() {
        return Err("migration failure implicitly reset config".into());
    }
    store
        .reset()
        .map_err(|error| format!("explicit reset failed: {}", error.code()))?;
    if Path::new(&store.metadata().config_file).exists() {
        return Err("explicit reset did not remove config".into());
    }
    Ok("migration failure left source intact; only explicit ConfigStore::reset removed it".into())
}

fn runtime_projection(config: &AppConfig) -> Value {
    let mut projection = serde_json::to_value(config).expect("serialize AppConfig");
    if let Some(object) = projection.as_object_mut() {
        object.remove("ui");
        object.remove("configRevision");
    }
    projection
}

fn int_36(fixture: &Fixture) -> QaResult<String> {
    let store = fixture.store("int-36")?;
    let raw = source_fixture("v1-v2-era")?;
    let mut expected: AppConfig =
        serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
    let revision_before = expected.config_revision;
    write_value(&store, &raw)?;
    let migrated = store
        .load()
        .map_err(|error| format!("migration failed: {}", error.code()))?
        .ok_or_else(|| "migrated config missing".to_owned())?;
    expected.schema_version = 2;
    expected.security.developer_test_mode = true;
    if migrated.config_revision != revision_before
        || runtime_projection(&migrated) != runtime_projection(&expected)
    {
        return Err("migration introduced Runtime-affecting revision/projection drift".into());
    }
    let before_bytes = read_file(&store)?;
    let mut ui_only = migrated.clone();
    ui_only.ui.language = Some(UiLanguage::ZhCn);
    let saved = store
        .save(ui_only)
        .map_err(|error| format!("UI-only save failed: {}", error.code()))?;
    if saved.config_revision != revision_before
        || runtime_projection(&saved) != runtime_projection(&migrated)
        || before_bytes == read_file(&store)?
    {
        return Err(
            "UI-only post-migration save created revision debt or failed to persist language"
                .into(),
        );
    }
    Ok(format!(
        "migration revision={} preserved; Runtime projection stable; UI-only language save stayed revision-neutral",
        revision_before.get()
    ))
}
