use harness_control_protocol::{
    export,
    rpc::{RequestId, RpcErrorResponse, RpcMessage, RpcNotification, RpcRequest, RpcResponse},
    v1::*,
};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(format!("fixtures/v1/{name}")).unwrap()).unwrap()
}

fn validate<T>(value: Value) -> T
where
    T: DeserializeOwned + JsonSchema,
{
    let schema = serde_json::to_value(schema_for!(T)).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    if let Err(error) = validator.validate(&value) {
        panic!("schema validation failed: {error}");
    }
    serde_json::from_value(value).unwrap()
}

fn field(value: &Value, name: &str) -> Value {
    value.get(name).unwrap().clone()
}

#[test]
fn legacy_bash_approval_details_default_destructive_fields() {
    let details: BashApprovalDetails = serde_json::from_value(serde_json::json!({
        "commandPreview": "git status",
        "classification": "classified",
        "normalizedSegments": [],
        "riskFlags": [],
        "cwd": "active_workspace",
        "previewRedacted": false,
        "previewTruncated": false
    }))
    .unwrap();
    assert!(!details.destructive);
    assert!(details.destructive_reasons.is_empty());
}

#[test]
fn rpc_envelopes_round_trip_with_integer_and_string_ids() {
    let request: RpcRequest = validate(fixture("initialize.request.json"));
    assert_eq!(request.id, RequestId::Integer(1));
    let serialized = serde_json::to_value(&request).unwrap();
    assert!(serialized.get("jsonrpc").is_none());
    assert_eq!(
        serde_json::from_value::<RpcRequest>(serialized).unwrap(),
        request
    );

    let response: RpcResponse = validate(fixture("runtime.status.response.json"));
    assert_eq!(response.id, RequestId::String("status-1".into()));
    assert_eq!(
        serde_json::from_value::<RpcResponse>(serde_json::to_value(response).unwrap())
            .unwrap()
            .id,
        RequestId::String("status-1".into())
    );

    let notification: RpcNotification = validate(fixture("initialized.notification.json"));
    assert_eq!(notification.method, "initialized");
    let error: RpcErrorResponse = validate(fixture("rpc.error.not_initialized.json"));
    assert_eq!(error.error.code, -32001);
    assert!(matches!(
        serde_json::from_value::<RpcMessage>(fixture("rpc.error.protocol_mismatch.json")).unwrap(),
        RpcMessage::Error(_)
    ));
}

#[test]
fn canonical_models_round_trip_and_fixtures_validate_against_schemas() {
    let initialize_request = fixture("initialize.request.json");
    let _: InitializeParams = validate(field(&initialize_request, "params"));
    let initialize_response = fixture("initialize.response.json");
    let _: InitializeResult = validate(field(&initialize_response, "result"));
    let initialized = fixture("initialized.notification.json");
    let _: InitializedParams = validate(field(&initialized, "params"));

    let status = fixture("runtime.status.response.json");
    let _: RuntimeState = validate(field(&status, "result"));
    let normal_tools = fixture("tools.list.normal.response.json");
    let tools: ToolsListResult = validate(field(&normal_tools, "result"));
    assert_eq!(
        tools
            .items
            .iter()
            .filter(|tool| tool.exposed)
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["read_file", "glob"]
    );
    let developer_tools = fixture("tools.list.developer.response.json");
    let _: ToolsListResult = validate(field(&developer_tools, "result"));
    let policies = fixture("policies.list.response.json");
    let _: PoliciesListResult = validate(field(&policies, "result"));
    let activity = fixture("activity.list.response.json");
    let _: ActivityListResult = validate(field(&activity, "result"));
    let activity_notification = fixture("activity.event.notification.json");
    let _: ActivityEvent = validate(field(&activity_notification, "params"));
    let approvals = fixture("approvals.list.response.json");
    let _: ApprovalsListResult = validate(field(&approvals, "result"));
    let approval_resolution = fixture("approvals.resolve.request.json");
    let _: ApprovalsResolveParams = validate(field(&approval_resolution, "params"));
    let diagnostics = fixture("diagnostics.run.response.json");
    let _: DiagnosticRun = validate(field(&diagnostics, "result"));
    let _: AppConfig = validate(fixture("app-config.example.json"));
    let _: RuntimeConfigSnapshot = validate(fixture("runtime-config-snapshot.example.json"));
}

#[test]
fn generated_artifacts_are_current() {
    let output = PathBuf::from(std::env::temp_dir()).join(format!(
        "harness-control-protocol-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&output);
    fs::create_dir_all(&output).unwrap();
    export::generate_all(&output).unwrap();
    let canonical_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in generated_files(&canonical_root.join("generated")) {
        assert_eq!(
            fs::read(output.join(&relative)).unwrap(),
            fs::read(canonical_root.join(&relative)).unwrap(),
            "generated artifact is stale: {}",
            relative.display()
        );
    }
    fs::remove_dir_all(output).unwrap();
}

fn generated_files(directory: &std::path::Path) -> Vec<PathBuf> {
    fn collect(root: &std::path::Path, current: &std::path::Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                files.push(
                    path.strip_prefix(root.parent().unwrap())
                        .unwrap()
                        .to_owned(),
                );
            }
        }
    }
    let mut files = Vec::new();
    collect(directory, directory, &mut files);
    files.sort();
    files
}

#[test]
fn control_method_registry_is_exact() {
    assert_eq!(
        REQUEST_METHODS,
        [
            "initialize",
            "runtime.status",
            "runtime.stop",
            "tools.list",
            "policies.list",
            "activity.list",
            "activity.subscribe",
            "approvals.list",
            "approvals.resolve",
            "diagnostics.run",
            "mcp.endpoint.get",
            "evidence.status",
            "history.clear",
            "control.diagnostics.get",
            "control.activity.snapshot",
            "control.effective_tools.get",
        ]
    );
    assert_eq!(NOTIFICATION_METHODS, ["initialized", "activity.event"]);
}

#[test]
fn decimal_u64_has_strict_decimal_string_wire_roundtrip() {
    for value in ["0", "1", "18446744073709551615"] {
        let parsed: DecimalU64 = serde_json::from_str(&format!("\"{value}\"")).unwrap();
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            format!("\"{value}\"")
        );
    }
    for value in ["", "-1", "1.0", "1e3", "18446744073709551616"] {
        assert!(
            serde_json::from_str::<DecimalU64>(&format!("\"{value}\"")).is_err(),
            "accepted invalid DecimalU64: {value}"
        );
    }
    assert!(serde_json::from_str::<DecimalU64>("1").is_err());
    assert!(DecimalU64::new(u64::MAX).checked_add_one().is_none());
}

#[test]
fn config_and_activity_fixtures_validate_decimal_wire_fields() {
    let config = fixture("app-config.example.json");
    let activity = fixture("activity.list.response.json");
    let config_revision = config.get("configRevision").unwrap();
    let latest_sequence = activity
        .get("result")
        .unwrap()
        .get("latestSequence")
        .unwrap();
    assert!(config_revision.is_string());
    assert!(latest_sequence.is_string());
    let _: AppConfig = validate(config);
    let _: ActivityListResult = validate(activity.get("result").unwrap().clone());
}

#[test]
fn runtime_limits_round_trip_with_explicit_read_size_and_reject_invalid_u32_values() {
    let limits = RuntimeLimits {
        max_concurrent_requests: 4,
        max_activity_events: 1000,
        max_read_bytes: 1_048_576,
    };
    let value = serde_json::to_value(&limits).unwrap();
    assert_eq!(value["maxReadBytes"], 1_048_576);
    assert_eq!(
        serde_json::from_value::<RuntimeLimits>(value).unwrap(),
        limits
    );

    for invalid in [
        serde_json::json!({
            "maxConcurrentRequests": 4,
            "maxActivityEvents": 1000,
            "maxReadBytes": -1
        }),
        serde_json::json!({
            "maxConcurrentRequests": 4,
            "maxActivityEvents": 1000,
            "maxReadBytes": 4_294_967_296u64
        }),
        serde_json::json!({
            "maxConcurrentRequests": 4,
            "maxActivityEvents": 1000
        }),
    ] {
        assert!(
            serde_json::from_value::<RuntimeLimits>(invalid).is_err(),
            "accepted invalid or incomplete RuntimeLimits"
        );
    }
}

#[test]
fn generated_runtime_limits_contract_contains_max_read_bytes() {
    let generated = fs::read_to_string("generated/typescript/v1.ts").unwrap();
    assert!(generated.contains(
        "export type RuntimeLimits = { maxConcurrentRequests: number, maxActivityEvents: number, maxReadBytes: number, };"
    ));

    let schema: Value = serde_json::from_str(
        &fs::read_to_string("generated/json-schema/runtime-config-snapshot.schema.json").unwrap(),
    )
    .unwrap();
    let read_bytes = &schema["$defs"]["RuntimeLimits"]["properties"]["maxReadBytes"];
    assert_eq!(read_bytes["type"], "integer");
    assert_eq!(read_bytes["format"], "uint32");
    assert!(
        schema["$defs"]["RuntimeLimits"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "maxReadBytes")
    );
}

#[test]
fn generated_typescript_uses_strings_for_revision_and_sequence_fields() {
    let generated = fs::read_to_string("generated/typescript/v1.ts").unwrap();
    for field in [
        "configRevision",
        "sequence",
        "afterSequence",
        "latestSequence",
    ] {
        assert!(
            !generated.contains(&format!("{field}: bigint")),
            "generated {field} still uses bigint"
        );
    }
    assert!(generated.contains("configRevision: string"));
    assert!(generated.contains("sequence: string"));
    assert!(generated.contains("afterSequence?: string"));
    assert!(generated.contains("latestSequence: string"));
}

#[test]
fn startup_and_diagnostic_v1_additions_round_trip_with_unknown_and_safe_fields() {
    let diagnostic = serde_json::json!({
        "runId": "diagnostic-contract",
        "startedAt": "2026-10-25T00:00:00Z",
        "finishedAt": "2026-10-25T00:00:01Z",
        "overallStatus": "unknown",
        "checks": [{
            "id": "credential_presence",
            "status": "unknown",
            "summary": "SecretStore is unavailable.",
            "scope": "credential",
            "evidence": [{"key": "error_code", "value": "SECRET_STORE_FAILED"}],
            "remediation": "Retry later.",
            "remediationDetail": {"code": "credential_retry", "summary": "Retry later.", "target": "settings_connection"}
        }]
    });
    let run: DiagnosticRun = validate(diagnostic);
    assert_eq!(run.overall_status, DiagnosticStatus::Unknown);
    assert_eq!(run.checks[0].scope, Some(DiagnosticScope::Credential));
    assert_eq!(run.checks[0].evidence[0].value, "SECRET_STORE_FAILED");
    assert_eq!(run.checks[0].remediation.as_deref(), Some("Retry later."));
    assert_eq!(
        run.checks[0]
            .remediation_detail
            .as_ref()
            .map(|value| value.code.as_str()),
        Some("credential_retry")
    );

    let startup = serde_json::json!({
        "attemptId": "attempt-contract",
        "harnessSessionId": "session-contract",
        "cohort": "cold",
        "startedAt": "2026-10-25T00:00:00Z",
        "outcome": "ready",
        "totalDurationMs": 42,
        "phases": [{
            "phase": "harness_ready",
            "startedOffsetMs": 42,
            "durationMs": 0,
            "status": "completed"
        }],
        "appVersion": "0.1.3"
    });
    let attempt: StartupAttempt = validate(startup);
    assert_eq!(attempt.outcome, StartupOutcome::Ready);
    assert_eq!(attempt.phases[0].phase, StartupPhase::HarnessReady);
}

#[test]
fn history_v1_contract_is_additive_closed_and_has_no_live_authority_shape() {
    let params: HistoryQueryParams = serde_json::from_value(serde_json::json!({
        "fromUtc": "2026-11-01T00:00:00Z",
        "tool": "read_file",
        "outcome": "success",
        "policyDecision": "allow",
        "approvalState": "none",
        "limit": 50
    }))
    .unwrap();
    assert_eq!(params.tool.as_deref(), Some("read_file"));
    assert_eq!(params.outcome, Some(HistoryOutcome::Success));
    assert_eq!(params.approval_state, Some(HistoryApprovalState::None));

    let initialize: InitializeParams =
        serde_json::from_value(fixture("initialize.request.json")["params"].clone()).unwrap();
    assert!(initialize.host_context.is_none());

    let with_host: InitializeParams = serde_json::from_value(serde_json::json!({
        "clientInfo": {"name": "test", "title": "Test", "version": "1"},
        "protocolVersion": harness_control_protocol::PROTOCOL_VERSION,
        "capabilities": {},
        "runtimeConfig": serde_json::from_value::<Value>(
            fixture("initialize.request.json")["params"]["runtimeConfig"].clone()
        ).unwrap(),
        "hostContext": {
            "hostSessionId": "host-session",
            "supervisorGenerationId": "generation",
            "evidenceStore": {
                "enabled": true,
                "databasePath": "C:\\AppData\\history\\history.sqlite3",
                "retentionDays": 30,
                "maxStorageMiB": 256
            }
        }
    }))
    .unwrap();
    let host = with_host.host_context.expect("host context");
    assert_eq!(host.host_session_id.as_deref(), Some("host-session"));
    assert!(host.evidence_store.is_some());

    let evidence = EvidenceEventV1 {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        event_id: "event-1".into(),
        runtime_id: "runtime-1".into(),
        request_id: "request-1".into(),
        host_session_id: Some("host-session".into()),
        supervisor_generation_id: Some("generation".into()),
        approval_id: None,
        tool_name: "read_file".into(),
        occurred_at_utc: "2026-11-01T00:00:00Z".into(),
        recorded_at_utc: "2026-11-01T00:00:01Z".into(),
        stage: EvidenceStage::ExecutionCompleted,
        lifecycle_state: EvidenceLifecycleState::Completed,
        workspace_validation: EvidenceWorkspaceValidation::Passed,
        policy_decision: Some(PolicyDecision::Allow),
        policy_reason: Some(PolicyReason::ToolDefault),
        matched_rule_ids: vec![],
        approval_decision: None,
        outcome: HistoryOutcome::Success,
        failure_category: None,
        duration_ms: Some(12),
        sanitized_arguments: Some(DurableSanitizedArgumentsEvidence::ReadFile {
            logical_path: "notes.txt".into(),
        }),
        result_evidence: Some(ExecutionResultEvidence::ReadFile { bytes_read: 12 }),
        coverage: EvidenceCoverage::Complete,
    };
    let json = serde_json::to_string(&evidence).unwrap();
    assert!(!json.contains("approvalRequest"));
    assert!(!json.contains("executionPermit"));
    assert!(!json.contains("sessionGrant"));
    assert!(!json.contains("databasePath"));
    let _: EvidenceEventV1 = serde_json::from_str(&json).unwrap();
}

#[test]
fn grep_evidence_is_closed_typed_and_excludes_raw_search_text() {
    let evidence = EvidenceEventV1 {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        event_id: "event-grep".into(),
        runtime_id: "runtime-grep".into(),
        request_id: "request-grep".into(),
        host_session_id: None,
        supervisor_generation_id: None,
        approval_id: None,
        tool_name: "grep".into(),
        occurred_at_utc: "2026-11-02T00:00:00Z".into(),
        recorded_at_utc: "2026-11-02T00:00:01Z".into(),
        stage: EvidenceStage::ExecutionCompleted,
        lifecycle_state: EvidenceLifecycleState::Completed,
        workspace_validation: EvidenceWorkspaceValidation::Passed,
        policy_decision: Some(PolicyDecision::Allow),
        policy_reason: Some(PolicyReason::ToolDefault),
        matched_rule_ids: vec![],
        approval_decision: None,
        outcome: HistoryOutcome::Success,
        failure_category: None,
        duration_ms: Some(12),
        sanitized_arguments: Some(DurableSanitizedArgumentsEvidence::Grep {
            mode: GrepMode::Literal,
            logical_scope: "docs".into(),
            query_bytes: 12,
            include_count: 1,
            exclude_count: 1,
            max_results: 100,
            max_output_bytes: 65_536,
        }),
        result_evidence: Some(ExecutionResultEvidence::Grep {
            match_count: 2,
            scanned_files: 3,
            scanned_bytes: 128,
            skipped_files: 1,
            truncated: false,
        }),
        coverage: EvidenceCoverage::Complete,
    };
    let json = serde_json::to_string(&evidence).unwrap();
    assert!(!json.contains("raw query"));
    assert!(!json.contains("preview"));
    let _: EvidenceEventV1 = serde_json::from_str(&json).unwrap();
    assert!(
        serde_json::from_str::<DurableSanitizedArgumentsEvidence>(
            r#"{"tool":"grep","mode":"literal","logicalScope":"docs","queryBytes":1,"includeCount":0,"excludeCount":0,"maxResults":10,"maxOutputBytes":1024,"query":"SECRET"}"#
        )
        .is_err()
    );
}
