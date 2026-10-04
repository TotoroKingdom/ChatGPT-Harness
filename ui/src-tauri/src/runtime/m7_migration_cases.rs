//! M7 migration black-box cases E2E-01..07.
//!
//! The parent `m7_e2e` module owns the runner, fixture lifecycle, evidence
//! writer, and real Runtime/MCP helpers.  Keeping these migration cases in a
//! child module lets them assert the production boundary in one place while
//! the parent module continues to own the remaining E2E cases.

use super::*;

fn migrated_expectation(raw: &Value) -> M7Result<AppConfig> {
    let mut expected: AppConfig =
        serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
    expected.schema_version = 2;
    expected.security.developer_test_mode =
        matches!(&expected.security.mode, SecurityMode::DeveloperTest);
    Ok(expected)
}

fn assert_migrated_config(loaded: &AppConfig, expected: &AppConfig) -> M7Result<()> {
    check!(loaded.schema_version == 2);
    check!(loaded.config_revision == expected.config_revision);
    check!(loaded.workspace == expected.workspace);
    check!(loaded.tunnel == expected.tunnel);
    check!(loaded.security == expected.security);
    check!(loaded.ui == expected.ui);
    check!(loaded.tools == expected.tools);
    check!(loaded.policies == expected.policies);
    check!(loaded.bash == expected.bash);
    Ok(())
}

fn assert_persisted_v2(installed: &InstalledFixture, loaded: &AppConfig) -> M7Result<Value> {
    let persisted = read_json(&installed.target)?;
    check!(persisted["schemaVersion"] == json!(2));
    check!(persisted["configRevision"] == json!(loaded.config_revision.get().to_string()));
    check!(!installed.temp_residue());
    check!(persisted == serde_json::to_value(loaded).map_err(|error| error.to_string())?);
    Ok(persisted)
}

fn effective_items<'a>(value: &'a Value) -> M7Result<&'a [Value]> {
    if let Some(items) = value.as_array() {
        return Ok(items.as_slice());
    }
    value["items"]
        .as_array()
        .map(|items| items.as_slice())
        .ok_or_else(|| "effective tools response did not return items".to_owned())
}

fn effective_item<'a>(value: &'a Value, name: &str) -> M7Result<&'a Value> {
    effective_items(value)?
        .iter()
        .find(|item| item["name"] == json!(name) || item["tool"] == json!(name))
        .ok_or_else(|| format!("effective tools omitted {name}"))
}

fn policy_item<'a>(policies: &'a Value, name: &str) -> M7Result<&'a Value> {
    policies["effective"]["perTool"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["tool"] == json!(name)))
        .ok_or_else(|| format!("effective policies omitted {name}"))
}

pub(super) fn run_e2e_01(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let installed = fixture.install_fixture("e2e-01", "v1-v2-era.json")?;
    let expected = migrated_expectation(&installed.before_json)?;
    let loaded = installed
        .store()
        .load()
        .map_err(|error| format!("ConfigStore load failed: {}", error.code()))?
        .ok_or_else(|| "ConfigStore returned no migrated config".to_owned())?;
    assert_migrated_config(&loaded, &expected)?;
    let post_json = assert_persisted_v2(&installed, &loaded)?;
    let mut details = case_migration_evidence(
        &installed,
        &post_json,
        "CONFIG_MIGRATION_SUCCEEDED",
        vec![
            "load_success",
            "schema_v1_to_v2_persisted",
            "config_revision_preserved",
            "workspace_tunnel_security_and_ui_theme_preserved",
            "all_missing_v3_fields_match_frozen_defaults",
            "no_temp_residue",
        ],
    )?;
    details.runtime.insert(
        "preservedFields".into(),
        json!({
            "workspace": loaded.workspace,
            "tunnel": loaded.tunnel,
            "security": loaded.security,
            "ui": loaded.ui,
            "tools": loaded.tools,
            "policies": loaded.policies,
            "bash": loaded.bash,
        }),
    );
    Ok(details)
}

pub(super) fn run_e2e_02(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let installed = fixture.install_fixture("e2e-02", "v1-transitional-m6.json")?;
    let expected = migrated_expectation(&installed.before_json)?;
    let loaded = installed
        .store()
        .load()
        .map_err(|error| format!("ConfigStore load failed: {}", error.code()))?
        .ok_or_else(|| "ConfigStore returned no transitional config".to_owned())?;
    assert_migrated_config(&loaded, &expected)?;
    check!(loaded.tools.write.enabled == expected.tools.write.enabled);
    check!(loaded.policies.tool_defaults == expected.policies.tool_defaults);
    check!(loaded.policies.user_rules == expected.policies.user_rules);
    check!(loaded.bash.auto_detect == expected.bash.auto_detect);
    check!(loaded.bash.executable_path == expected.bash.executable_path);
    check!(loaded.ui.language == Some(UiLanguage::ZhCn));
    check!(loaded.tunnel.api_key_ref.is_some());
    let post_json = assert_persisted_v2(&installed, &loaded)?;
    let mut details = case_migration_evidence(
        &installed,
        &post_json,
        "CONFIG_MIGRATION_SUCCEEDED",
        vec![
            "load_success",
            "all_tool_states_preserved",
            "all_policy_defaults_and_user_rule_fields_preserved",
            "bash_auto_detect_and_manual_path_preserved",
            "language_workspace_tunnel_proxy_api_key_ref_and_security_preserved",
            "config_revision_preserved",
            "no_temp_residue",
        ],
    )?;
    details.runtime.insert(
        "preservedFields".into(),
        json!({
            "workspace": loaded.workspace,
            "tunnel": loaded.tunnel,
            "security": loaded.security,
            "ui": loaded.ui,
            "tools": loaded.tools,
            "policies": loaded.policies,
            "bash": loaded.bash,
        }),
    );
    Ok(details)
}

pub(super) fn run_e2e_03(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let cases = [
        ("e2e-03-pure", "v1-v2-era.json"),
        ("e2e-03-transitional", "v1-transitional-m6.json"),
    ];
    let mut details: Option<CaseEvidence> = None;
    for (name, source) in cases {
        let (installed, first) = runtime_fixture_config(fixture, name, source, |_| {})?;
        let store = installed.store();
        let first_bytes = fs::read(&installed.target).map_err(|error| error.to_string())?;
        let first_sha = bytes_sha256(&first_bytes)?;
        let (mut runtime, _mcp) = start_runtime(&first, &format!("M7-E2E-03-{name}"))?;
        let pid = runtime.pid();
        let runtime_id = runtime.runtime_id().to_owned();
        let second = store
            .load()
            .map_err(|error| format!("{name} second ConfigStore load failed: {}", error.code()))?
            .ok_or_else(|| format!("{name} second load returned no config"))?;
        let second_bytes = fs::read(&installed.target).map_err(|error| error.to_string())?;
        check!(first.schema_version == 2 && second.schema_version == 2);
        check!(first.config_revision == second.config_revision);
        check!(first.workspace == second.workspace);
        check!(first.tunnel == second.tunnel);
        check!(first.security == second.security);
        check!(first.ui == second.ui);
        check!(first.tools == second.tools);
        check!(first.policies == second.policies);
        check!(first.bash == second.bash);
        check!(bytes_sha256(&second_bytes)? == first_sha);
        check!(!installed.temp_residue());
        let active_status = control(&mut runtime, "runtime.status", json!({}))?;
        check!(active_status["configRevision"] == json!(second.config_revision.get().to_string()));
        check!(active_status["configRevision"] == json!(first.config_revision.get().to_string()));
        check!(active_status["activeRequests"] == json!(0));
        check!(active_status["pendingApprovals"] == json!(0));
        check!(active_status["runningExecutions"] == json!(0));
        stop_runtime(&mut runtime)?;
        check!(runtime.stopped_and_endpoint_closed());

        if details.is_none() {
            let post_json = read_json(&installed.target)?;
            let mut primary = case_migration_evidence(
                &installed,
                &post_json,
                "CONFIG_LOAD_V2_STABLE",
                vec![
                    "pure_and_transitional_fixtures_each_loaded_twice",
                    "second_load_schema_v2_without_rewrite",
                    "actual_config_revision_unchanged",
                    "real_runtime_status_revision_matches_second_load",
                    "runtime_affecting_projection_unchanged",
                    "runtime_stopped_and_endpoint_closed_per_fixture",
                    "no_temp_residue",
                ],
            )?;
            primary.fixture = Some("v1-v2-era.json + v1-transitional-m6.json".into());
            primary.runtime.insert(
                "pure".into(),
                json!({
                    "firstRevision": first.config_revision.get(),
                    "secondRevision": second.config_revision.get(),
                    "activeRuntimeRevision": active_status["configRevision"],
                    "runtimePid": pid,
                    "runtimeId": runtime_id,
                    "firstSha256": first_sha,
                    "secondSha256": bytes_sha256(&second_bytes)?,
                    "runtimeStopped": true,
                }),
            );
            details = Some(primary);
        } else if let Some(primary) = details.as_mut() {
            primary.runtime.insert(
                "transitional".into(),
                json!({
                    "firstRevision": first.config_revision.get(),
                    "secondRevision": second.config_revision.get(),
                    "activeRuntimeRevision": active_status["configRevision"],
                    "runtimePid": pid,
                    "runtimeId": runtime_id,
                    "firstSha256": first_sha,
                    "secondSha256": bytes_sha256(&second_bytes)?,
                    "tempResidue": installed.temp_residue(),
                    "runtimeStopped": true,
                }),
            );
        }
    }
    details.ok_or_else(|| "E2E-03 produced no fixture evidence".to_owned())
}

pub(super) fn run_e2e_04(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let installed = fixture.install_fixture("e2e-04", "v1-invalid-policy.json")?;
    let before_sha = installed.before_sha256()?;
    let store = installed.store();
    let error = store
        .load()
        .expect_err("invalid policy migration must fail");
    check!(error.code() == "CONFIG_MIGRATION_FAILED");
    check!(fs::read(&installed.target).map_err(|error| error.to_string())? == installed.before);
    check!(installed.after_sha256()? == before_sha);
    check!(!installed.temp_residue());
    let metadata = store.metadata();
    check!(PathBuf::from(&metadata.config_file) == installed.target);
    check!(PathBuf::from(&metadata.config_directory) == installed.directory);
    check!(Path::new(&metadata.config_file).is_file());
    let mut details = case_migration_evidence(
        &installed,
        &installed.before_json,
        error.code(),
        vec![
            "typed_migration_failure",
            "original_bytes_and_sha256_unchanged",
            "no_partial_v2",
            "no_temp_residue",
            "config_metadata_exact_path_remains_available",
        ],
    )?;
    details.runtime.insert(
        "metadata".into(),
        json!({
            "configDirectory": metadata.config_directory,
            "configFile": metadata.config_file,
        }),
    );
    Ok(details)
}

pub(super) fn run_e2e_05(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let installed = fixture.install_fixture("e2e-05", "v1-future-schema.json")?;
    let before_sha = installed.before_sha256()?;
    check!(installed.before_json["schemaVersion"] == json!(3));
    let error = installed
        .store()
        .load()
        .expect_err("future schema must fail closed");
    check!(error.code() == "CONFIG_SCHEMA_UNSUPPORTED");
    check!(fs::read(&installed.target).map_err(|error| error.to_string())? == installed.before);
    check!(installed.after_sha256()? == before_sha);
    check!(!installed.temp_residue());
    check!(installed.target.is_file());
    Ok(case_migration_evidence(
        &installed,
        &installed.before_json,
        error.code(),
        vec![
            "typed_unsupported_schema",
            "schema_3_source_bytes_and_sha256_unchanged",
            "no_downgrade_or_reset",
            "no_temp_residue",
        ],
    )?)
}

pub(super) fn run_e2e_06(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let installed = fixture.install_fixture("e2e-06", "v1-invalid-secret-like-field.json")?;
    let before_sha = installed.before_sha256()?;
    check!(String::from_utf8_lossy(&installed.before).contains(SECRET_SENTINEL));
    let error = installed
        .store()
        .load()
        .expect_err("raw secret-like migration field must fail");
    check!(error.code() == "CONFIG_MIGRATION_FAILED");
    check!(!error.message().contains(SECRET_SENTINEL));
    check!(fs::read(&installed.target).map_err(|error| error.to_string())? == installed.before);
    check!(installed.after_sha256()? == before_sha);
    check!(!installed.temp_residue());

    let evidence_directory = repo_root().join("target").join("m7-qa").join("e2e");
    let mut generated_evidence_files_scanned = 0u64;
    if evidence_directory.exists() {
        for entry in fs::read_dir(&evidence_directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_file() {
                generated_evidence_files_scanned += 1;
                check!(!String::from_utf8_lossy(
                    &fs::read(&path).map_err(|error| error.to_string())?
                )
                .contains(SECRET_SENTINEL));
            }
        }
    }
    let mut details = case_migration_evidence(
        &installed,
        &installed.before_json,
        error.code(),
        vec![
            "typed_migration_failure",
            "sentinel_absent_from_returned_error",
            "original_bytes_and_sha256_unchanged",
            "no_partial_v2",
            "no_temp_residue",
            "generated_evidence_files_scanned_without_sentinel",
            "activity_and_diagnostics_not_invoked_for_config_only_failure",
        ],
    )?;
    details.runtime.insert(
        "generatedEvidenceFilesScanned".into(),
        json!(generated_evidence_files_scanned),
    );
    details.runtime.insert(
        "activitySurface".into(),
        json!("not_invoked_config_migration_only"),
    );
    details.runtime.insert(
        "diagnosticsSurface".into(),
        json!("not_invoked_config_migration_only"),
    );
    Ok(details)
}

pub(super) fn run_e2e_07(fixture: &Fixture) -> M7Result<CaseEvidence> {
    let (installed, config) =
        runtime_fixture_config(fixture, "e2e-07", "v1-transitional-m6.json", |value| {
            allow_file_tools(value);
        })?;
    let binary = m2_e2e::runtime_binary().map_err(|error| error.to_string())?;
    let binary_sha = file_sha256(&binary)?;
    let (mut runtime, mut mcp) = start_runtime(&config, "M7-E2E-07")?;
    let pid = runtime.pid();
    let runtime_id = runtime.runtime_id().to_owned();
    let endpoint = control(&mut runtime, "mcp.endpoint.get", json!({}))?;
    let status = control(&mut runtime, "runtime.status", json!({}))?;
    check!(status["configRevision"] == json!(config.config_revision.get().to_string()));
    check!(status["activeWorkspace"] == json!(fixture.workspace.display().to_string()));
    check!(status["securityMode"] == json!("developer_test"));
    check!(status["activeRequests"] == json!(0));
    check!(status["pendingApprovals"] == json!(0));
    check!(status["runningExecutions"] == json!(0));

    let effective = control(&mut runtime, "control.effective_tools.get", json!({}))?;
    for (name, enabled, decision) in [
        (
            "read_file",
            config.tools.read_file.enabled,
            serde_json::to_value(&config.policies.tool_defaults.read_file).unwrap(),
        ),
        (
            "glob",
            config.tools.glob.enabled,
            serde_json::to_value(&config.policies.tool_defaults.glob).unwrap(),
        ),
        (
            "write",
            config.tools.write.enabled,
            serde_json::to_value(&config.policies.tool_defaults.write).unwrap(),
        ),
        (
            "edit",
            config.tools.edit.enabled,
            serde_json::to_value(&config.policies.tool_defaults.edit).unwrap(),
        ),
        (
            "bash",
            config.tools.bash.enabled,
            serde_json::to_value(&config.policies.tool_defaults.bash).unwrap(),
        ),
    ] {
        let item = effective_item(&effective, name)?;
        if enabled {
            check!(item["decision"] == decision);
            check!(item["statusReason"] == json!("available"));
        } else {
            check!(item["decision"] == json!("deny"));
            check!(item["statusReason"] == json!("tool_disabled"));
        }
    }
    let tool_names = list_tool_names(mcp.list_tools().map_err(|error| error.to_string())?)?;
    check!(tool_names.iter().any(|name| name == "read_file"));
    check!(tool_names.iter().any(|name| name == "glob"));
    check!(tool_names.iter().any(|name| name == "write"));
    check!(tool_names.iter().any(|name| name == "edit"));
    check!(!tool_names.iter().any(|name| name == "bash"));

    let policies = control(&mut runtime, "policies.list", json!({}))?;
    check!(policies["effective"]["valid"] == json!(true));
    check!(
        policies["effective"]["toolDefaults"]
            == serde_json::to_value(&config.policies.tool_defaults).unwrap()
    );
    check!(policies["effective"]["userRules"]
        .as_array()
        .is_some_and(|rules| rules.is_empty()));
    for name in ["read_file", "glob", "write", "edit"] {
        let item = policy_item(&policies, name)?;
        check!(item["decision"] == json!("allow"));
        check!(item["reason"] == json!("tool_default"));
        check!(item["matchedRuleIds"]
            .as_array()
            .is_some_and(|ids| ids.is_empty()));
    }
    let diagnostics = control(&mut runtime, "control.diagnostics.get", json!({}))?;
    check!(
        diagnostics["runtime"]["configRevision"] == json!(config.config_revision.get().to_string())
    );
    check!(diagnostics["requests"]["activeRequests"] == json!(0));
    stop_runtime(&mut runtime)?;

    let mut details = case_migration_evidence(
        &installed,
        &read_json(&installed.target)?,
        "CONFIG_MIGRATION_SUCCEEDED_RUNTIME_STARTED",
        vec![
            "production_configstore_migration_before_runtime_start",
            "initialize_initialized_and_runtime_revision_match",
            "workspace_and_security_match_migrated_config",
            "effective_tools_match_migrated_enabled_and_policy_state",
            "effective_policy_defaults_and_rules_match_migrated_config",
            "bash_disabled_state_matches_migrated_config",
            "real_mcp_catalog_matches_effective_tools",
            "diagnostics_revision_and_counters_observed",
        ],
    )?;
    details
        .runtime
        .insert("binarySha256".into(), json!(binary_sha));
    details.runtime.insert("pid".into(), json!(pid));
    details
        .runtime
        .insert("runtimeId".into(), json!(runtime_id));
    details.runtime.insert("mcpEndpoint".into(), endpoint);
    details.runtime.insert("status".into(), status);
    details.runtime.insert("effectiveTools".into(), effective);
    details.runtime.insert("effectivePolicies".into(), policies);
    details.runtime.insert("diagnostics".into(), diagnostics);
    Ok(details)
}
