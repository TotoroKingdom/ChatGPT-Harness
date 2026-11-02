use super::adapter::{
    AdapterClass, ModelAdapter, ModelRequest, ModelTurn, ProviderUsage, ScriptedAdapter,
    ScriptedTurn,
};
use super::agent_loop::{
    execute_case, prepare_tool_schemas_for_run, scripted_adapter_for_case, CaseExecution,
};
use super::aggregate::ScoreableStatus;
use super::approvals;
use super::case::{EvalCaseV1, EvalCategory, RuntimeOutcome};
use super::correlation;
use super::dataset::{deterministic_case_order, EvalDataset};
use super::mcp_client::eval_config;
use super::mcp_client::{
    base_policy_config_hash, runtime_binary_hash, tool_catalog_hash, EvalRuntimeHarness, McpClient,
};
use super::privacy::sanitize_bounded_text;
use super::scoring::{normalize_runtime_outcome, validate_schema};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn dataset() -> EvalDataset {
    EvalDataset::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("v4")
            .join("m04-eval"),
    )
    .expect("M04 dataset loads")
}

fn runtime_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("HARNESS_E2E_RUNTIME_BIN")
        .or_else(|| std::env::var_os("M04_EVAL_RUNTIME_BIN"))
    {
        let path = PathBuf::from(path);
        assert!(path.is_file(), "configured Runtime binary is missing");
        return path;
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    [
        manifest
            .join("..")
            .join("target")
            .join("debug")
            .join("chatgpt-harness-runtime.exe"),
        manifest
            .join("..")
            .join("target")
            .join("release")
            .join("chatgpt-harness-runtime.exe"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .expect("compiled Runtime binary is required for formal integration tests")
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("v4")
        .join("m04-eval")
}

fn case<'a>(dataset: &'a EvalDataset, case_id: &str) -> &'a EvalCaseV1 {
    dataset.case(case_id).expect("case exists")
}

async fn probe_tools(
    dataset: &EvalDataset,
    binary: &Path,
) -> (String, std::collections::BTreeMap<String, Value>) {
    let probe = case(dataset, "m04-c1-01");
    let harness = EvalRuntimeHarness::start(binary, probe, &dataset.fixture_path(probe).unwrap())
        .await
        .expect("Runtime starts for probe");
    let mut mcp = McpClient::connect(&harness.endpoint_url)
        .await
        .expect("MCP connects");
    let tools = mcp.list_tools().await.expect("tools/list succeeds");
    let hash = tool_catalog_hash(&tools).expect("catalog hashes");
    let schemas = prepare_tool_schemas_for_run(&tools).expect("schemas parse");
    harness.shutdown().await.expect("probe cleanup");
    (hash, schemas)
}

fn case_execution<'a>(
    dataset: &'a EvalDataset,
    case: &'a EvalCaseV1,
    binary: &'a Path,
    hash: String,
    schemas: std::collections::BTreeMap<String, Value>,
    cleanup_failure_injection: bool,
) -> CaseExecution<'a> {
    CaseExecution {
        dataset,
        case,
        runtime_binary: binary,
        base_tool_schema_hash: hash,
        base_tool_schemas: schemas,
        runner_commit: "integration".into(),
        runtime_commit: "integration".into(),
        logical_run_index: 1,
        repetition: 1,
        cleanup_failure_injection,
        profile: super::profile::EvalProfile::M04Baseline,
    }
}

#[test]
fn int01_dataset_schema_identity_and_limits_are_closed() {
    let dataset = dataset();
    assert_eq!(dataset.cases.len(), 24);
    assert_eq!(dataset.dataset_file.dataset_id, "m04-core-v1");
    assert_eq!(dataset.dataset_file.repetitions, 3);
    assert_eq!(
        dataset
            .cases
            .iter()
            .map(|item| item.category)
            .collect::<BTreeSet<_>>()
            .len(),
        6
    );
    for category in [
        EvalCategory::ExactRead,
        EvalCategory::PathDiscovery,
        EvalCategory::ContentSearch,
        EvalCategory::WriteState,
        EvalCategory::EditState,
        EvalCategory::EnforcementAndRecovery,
    ] {
        assert_eq!(
            dataset
                .cases
                .iter()
                .filter(|item| item.category == category)
                .count(),
            4,
            "{}",
            category.as_str()
        );
    }
    let ids = dataset
        .cases
        .iter()
        .map(|item| item.case_id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 24);
    for item in &dataset.cases {
        assert_eq!(item.case_version, 1);
        assert_eq!(item.fixture_hash.len(), 64);
        assert_eq!(item.case_hash().unwrap().len(), 64);
        assert!(item
            .validate(&fixture_root().join("workspaces"), &dataset.system_prompt)
            .is_ok());
    }

    let original = case(&dataset, "m04-c4-01").clone();
    let mut changed = original.clone();
    changed.prompt.push_str(" changed");
    assert_ne!(original.case_hash().unwrap(), changed.case_hash().unwrap());
    let mut changed_ground_truth = original.clone();
    changed_ground_truth
        .final_response_assertion
        .as_mut()
        .unwrap()
        .text = "OTHER".into();
    assert_ne!(
        original.case_hash().unwrap(),
        changed_ground_truth.case_hash().unwrap()
    );

    let mut unknown_field = serde_json::to_value(&original).unwrap();
    unknown_field["scorerType"] = json!("unknown");
    assert!(serde_json::from_value::<EvalCaseV1>(unknown_field).is_err());
    let mut unknown_category = serde_json::to_value(&original).unwrap();
    unknown_category["category"] = json!("unknown_category");
    assert!(serde_json::from_value::<EvalCaseV1>(unknown_category).is_err());
    let mut unknown_scorer = serde_json::to_value(&original).unwrap();
    unknown_scorer["finalStateAssertions"] = json!([{ "kind": "llm_judge" }]);
    assert!(serde_json::from_value::<EvalCaseV1>(unknown_scorer).is_err());

    let mut missing = original.clone();
    missing.fixture_ref = "missing-fixture".into();
    assert!(missing
        .validate(&fixture_root().join("workspaces"), &dataset.system_prompt)
        .is_err());
    let mut bad_hash = original.clone();
    bad_hash.fixture_hash = "0".repeat(64);
    assert!(bad_hash
        .validate(&fixture_root().join("workspaces"), &dataset.system_prompt)
        .is_err());
    let mut bad_turns = original.clone();
    bad_turns.max_turns = Some(9);
    assert!(bad_turns
        .validate(&fixture_root().join("workspaces"), &dataset.system_prompt)
        .is_err());
    let mut bad_calls = original.clone();
    bad_calls.max_tool_calls = Some(13);
    assert!(bad_calls
        .validate(&fixture_root().join("workspaces"), &dataset.system_prompt)
        .is_err());
    assert_eq!(deterministic_case_order(&dataset.cases).len(), 24);
}

#[tokio::test]
async fn int02_workspace_evidence_isolation_and_cleanup_status() {
    let dataset = dataset();
    let binary = runtime_binary();
    let first = EvalRuntimeHarness::start(
        &binary,
        case(&dataset, "m04-c1-01"),
        &dataset.fixture_path(case(&dataset, "m04-c1-01")).unwrap(),
    )
    .await
    .unwrap();
    let first_root = first.workspace_root.clone();
    let first_db = first.evidence_database.clone();
    let first_id = first.runtime_id.clone();
    std::fs::write(first_root.join("only-first.txt"), "mutation").unwrap();
    assert_eq!(
        std::fs::read(first_root.join("only-first.txt")).unwrap(),
        b"mutation"
    );
    first.shutdown().await.unwrap();
    assert!(!first_root.exists());
    assert!(!first_db.exists());

    let second = EvalRuntimeHarness::start(
        &binary,
        case(&dataset, "m04-c1-01"),
        &dataset.fixture_path(case(&dataset, "m04-c1-01")).unwrap(),
    )
    .await
    .unwrap();
    assert_ne!(first_id, second.runtime_id);
    assert_ne!(first_root, second.workspace_root);
    assert_ne!(first_db, second.evidence_database);
    assert!(!second.workspace_root.join("only-first.txt").exists());
    second.shutdown().await.unwrap();

    let mut injected = EvalRuntimeHarness::start(
        &binary,
        case(&dataset, "m04-c1-01"),
        &dataset.fixture_path(case(&dataset, "m04-c1-01")).unwrap(),
    )
    .await
    .unwrap();
    let injected_root = injected.workspace_root.clone();
    injected.inject_cleanup_failure();
    let cleanup = injected.shutdown().await.unwrap();
    assert!(cleanup.starts_with("cleanup_failed:"), "{cleanup}");
    assert!(!injected_root.exists());

    let (hash, schemas) = probe_tools(&dataset, &binary).await;
    let failed = execute_case(
        case_execution(
            &dataset,
            case(&dataset, "m04-c1-01"),
            &binary,
            hash,
            schemas,
            false,
        ),
        Box::new(ScriptedAdapter::new(Vec::new())),
    )
    .await;
    assert_eq!(failed.scoreable_status, ScoreableStatus::InfraError);
    assert!(failed.cleanup_status.starts_with("stopped_cleaned"));
}

#[tokio::test]
async fn int03_formal_profile_is_provider_free_and_catalog_exact() {
    let dataset = dataset();
    let binary = runtime_binary();
    let (hash, schemas) = probe_tools(&dataset, &binary).await;
    assert_eq!(hash.len(), 64);
    assert_eq!(
        schemas.keys().cloned().collect::<Vec<_>>(),
        vec!["edit", "glob", "read_file", "write"]
    );
    assert!(schemas.get("bash").is_none());
    assert!(schemas.get("grep").is_none());
    let config = eval_config(
        case(&dataset, "m04-c6-02"),
        tempfile::tempdir().unwrap().path(),
    )
    .unwrap();
    assert!(config.tools.edit.enabled);
    assert_eq!(
        config.policies.tool_defaults.edit,
        harness_control_protocol::PolicyDecision::Ask
    );
    assert!(config
        .policies
        .user_rules
        .iter()
        .any(|rule| rule.decision == harness_control_protocol::PolicyDecision::Deny));
    assert!(super::mcp_client::base_policy_config_hash().unwrap().len() == 64);
}

#[tokio::test]
async fn int05_exact_runtime_request_and_m03_identity() {
    let dataset = dataset();
    let binary = runtime_binary();
    let (base_hash, schemas) = probe_tools(&dataset, &binary).await;
    let base_policy = base_policy_config_hash().unwrap();

    for case_id in [
        "m04-c1-01",
        "m04-c4-01",
        "m04-c6-01",
        "m04-c6-02",
        "m04-c6-03",
    ] {
        let result = execute_case(
            case_execution(
                &dataset,
                case(&dataset, case_id),
                &binary,
                base_hash.clone(),
                schemas.clone(),
                false,
            ),
            Box::new(scripted_adapter_for_case(case(&dataset, case_id))),
        )
        .await;
        assert_eq!(
            result.scoreable_status,
            ScoreableStatus::Scoreable,
            "{case_id}"
        );
        assert_eq!(result.observed_tool_catalog_hash, base_hash, "{case_id}");
        assert!(
            result.tool_calls.iter().all(|call| call.forwarded
                && call.evidence_verified
                && call.runtime_id.as_deref() == Some(result.runtime_id.as_str())
                && call.request_id.is_some()),
            "{case_id}"
        );
        if case_id == "m04-c4-01" {
            assert!(result.tool_calls.iter().any(|call| {
                call.approval_request_id.is_some() && call.approval_request_id == call.request_id
            }));
        }
        if case_id == "m04-c6-01" {
            assert!(result.tool_calls.iter().any(|call| {
                call.approval_decision.as_deref() == Some("deny")
                    && call.approval_request_id == call.request_id
            }));
        }
        if case_id == "m04-c6-02" {
            assert_ne!(result.policy_config_hash, base_policy);
            assert_eq!(
                result.tool_calls[0].runtime_outcome,
                Some(RuntimeOutcome::PolicyDenied)
            );
        }
    }

    let probe = case(&dataset, "m04-c1-01");
    let mut harness =
        EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
            .await
            .unwrap();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let result = mcp
        .call_tool("read_file", json!({"path": "config/service.txt"}))
        .await
        .unwrap();
    let key = correlation::extract(&result).expect("exact metadata");
    assert!(
        correlation::verify_live_activity(&mut harness, &key)
            .await
            .unwrap()
            > 0
    );
    let wrong_key = correlation::CorrelationKey {
        runtime_id: key.runtime_id.clone(),
        request_id: "not-the-request".into(),
    };
    let error = correlation::verify_evidence(
        &harness.evidence_database,
        &wrong_key,
        Duration::from_millis(50),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "HARNESS_CONTRACT_ERROR");
    assert!(correlation::extract(&json!({"isError": false})).is_none());
    let mut stripped = result.clone();
    let structured_before = stripped["structuredContent"].clone();
    correlation::strip_model_metadata(&mut stripped);
    assert_eq!(stripped["structuredContent"], structured_before);
    harness.shutdown().await.unwrap();
}

#[tokio::test]
async fn int07_schema_and_semantic_argument_boundaries() {
    let dataset = dataset();
    let binary = runtime_binary();
    let (_hash, schemas) = probe_tools(&dataset, &binary).await;
    let schema = schemas.get("read_file").unwrap();
    assert!(validate_schema(
        schema,
        Some(&json!({"path": "config/service.txt"}))
    ));
    assert!(!validate_schema(schema, Some(&json!({"path": 7}))));
    assert!(!validate_schema(schema, Some(&json!({}))));
    assert!(!validate_schema(
        schema,
        Some(&json!({"path": "config/service.txt", "unknown": true}))
    ));
    assert!(validate_schema(schema, Some(&json!({"path": "wrong.txt"}))));
    let c4 = case(&dataset, "m04-c4-01");
    let mut write_call = super::scoring::ToolCallRecord {
        model_tool_call_id: "c4".into(),
        tool_name: "write".into(),
        schema_valid: true,
        semantic_valid: false,
        forwarded: false,
        runtime_id: None,
        request_id: None,
        runtime_outcome: None,
        safe_error_code: None,
        approval_decision: None,
        approval_request_id: None,
        argument_evidence: super::scoring::bounded_argument_evidence("{}"),
        evidence_verified: false,
        unnecessary: false,
    };
    assert!(!super::scoring::score_arguments(
        c4,
        &write_call,
        Some(&json!({"path": "report_output.txt", "content": "wrong"}))
    ));
    write_call.tool_name = "read_file".into();
    assert!(!super::scoring::score_arguments(
        case(&dataset, "m04-c6-03"),
        &write_call,
        Some(&json!({"path": "C:\\outside\\secret.txt"}))
    ));

    let (hash, schemas) = probe_tools(&dataset, &binary).await;
    let adapter = ScriptedAdapter::new(vec![
        ScriptedTurn {
            tool_calls: vec![super::adapter::ModelToolCall {
                id: "schema-invalid".into(),
                name: "read_file".into(),
                arguments: json!({"path": 7}).to_string(),
            }],
            final_text: None,
        },
        ScriptedTurn {
            tool_calls: Vec::new(),
            final_text: Some("PORT=4317".into()),
        },
    ]);
    let result = execute_case(
        case_execution(
            &dataset,
            case(&dataset, "m04-c1-01"),
            &binary,
            hash,
            schemas,
            false,
        ),
        Box::new(adapter),
    )
    .await;
    assert_eq!(result.argument_total_count, 1);
    assert_eq!(result.argument_valid_count, 0);
    assert!(!result.tool_calls[0].schema_valid);
    assert!(!result.tool_calls[0].forwarded);
    assert!(result.tool_calls[0].argument_evidence.parseable);
    assert!(!result.tool_calls[0]
        .argument_evidence
        .bounded_preview
        .is_empty());
}

#[tokio::test]
async fn int08_runtime_success_is_not_task_completion() {
    let dataset = dataset();
    let binary = runtime_binary();
    let (base_hash, schemas) = probe_tools(&dataset, &binary).await;
    let adapter = ScriptedAdapter::new(vec![
        ScriptedTurn {
            tool_calls: vec![super::adapter::scripted_call(
                "wrong-content",
                "write",
                json!({"path": "report_output.txt", "content": "wrong"}),
            )],
            final_text: None,
        },
        ScriptedTurn {
            tool_calls: Vec::new(),
            final_text: Some("DONE".into()),
        },
    ]);
    let result = execute_case(
        case_execution(
            &dataset,
            case(&dataset, "m04-c4-01"),
            &binary,
            base_hash.clone(),
            schemas.clone(),
            false,
        ),
        Box::new(adapter),
    )
    .await;
    assert_eq!(result.scoreable_status, ScoreableStatus::Scoreable);
    assert!(result.selection_pass);
    assert_eq!(result.runtime_outcome_match_count, 1);
    assert_eq!(result.runtime_outcome_total_count, 1);
    assert!(!result.task_completion_pass);

    let probe = case(&dataset, "m04-c1-01");
    let harness = EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
        .await
        .unwrap();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let invalid = mcp
        .call_tool("read_file", json!({"path": 3}))
        .await
        .unwrap();
    assert_eq!(
        normalize_runtime_outcome(Some(&invalid)),
        RuntimeOutcome::InvalidArguments
    );
    harness.shutdown().await.unwrap();

    let edit_case = case(&dataset, "m04-c5-01");
    let mut edit_harness = EvalRuntimeHarness::start(
        &binary,
        edit_case,
        &dataset.fixture_path(edit_case).unwrap(),
    )
    .await
    .unwrap();
    let mut edit_mcp = McpClient::connect(&edit_harness.endpoint_url)
        .await
        .unwrap();
    let edit_call = edit_mcp.call_tool(
        "edit",
        json!({"path": "settings/app.txt", "oldText": "not-present", "newText": "x"}),
    );
    tokio::pin!(edit_call);
    loop {
        tokio::select! {
            result = &mut edit_call => {
                assert_eq!(
                    normalize_runtime_outcome(Some(&result.unwrap())),
                    RuntimeOutcome::ExecutionFailed
                );
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(20)) => {
                let pending = approvals::list_pending(&mut edit_harness).await.unwrap();
                if let Some(item) = pending.first() {
                    approvals::resolve(&mut edit_harness, &item.request_id, "allow_once")
                        .await
                        .unwrap();
                }
            }
        }
    }
    edit_harness.shutdown().await.unwrap();

    let expected = [
        ("m04-c1-01", RuntimeOutcome::Success),
        ("m04-c4-01", RuntimeOutcome::Success),
        ("m04-c6-01", RuntimeOutcome::ApprovalDenied),
        ("m04-c6-02", RuntimeOutcome::PolicyDenied),
        ("m04-c6-03", RuntimeOutcome::WorkspaceViolation),
        ("m04-c6-04", RuntimeOutcome::TargetNotFound),
    ];
    for (case_id, expected_outcome) in expected {
        let result = execute_case(
            case_execution(
                &dataset,
                case(&dataset, case_id),
                &binary,
                base_hash.clone(),
                schemas.clone(),
                false,
            ),
            Box::new(scripted_adapter_for_case(case(&dataset, case_id))),
        )
        .await;
        assert_eq!(result.runtime_outcome_match_count, 1, "{case_id}");
        assert_eq!(result.runtime_outcome_total_count, 1, "{case_id}");
        assert!(
            result
                .tool_calls
                .iter()
                .any(|call| call.runtime_outcome == Some(expected_outcome)),
            "{case_id}"
        );
    }
}

#[tokio::test]
async fn int10_limits_approval_identity_and_provider_request_profile() {
    let dataset = dataset();
    let binary = runtime_binary();
    assert_eq!(super::RUN_TIMEOUT, Duration::from_secs(90));
    let (base_hash, schemas) = probe_tools(&dataset, &binary).await;
    let turns = vec![
        ScriptedTurn {
            tool_calls: (0..13)
                .map(|index| {
                    super::adapter::scripted_call(
                        format!("limit-{index}"),
                        "read_file",
                        json!({"path": "config/service.txt"}),
                    )
                })
                .collect(),
            final_text: None,
        },
        ScriptedTurn {
            tool_calls: Vec::new(),
            final_text: Some("PORT=4317".into()),
        },
    ];
    let result = execute_case(
        case_execution(
            &dataset,
            case(&dataset, "m04-c1-01"),
            &binary,
            base_hash.clone(),
            schemas.clone(),
            false,
        ),
        Box::new(ScriptedAdapter::new(turns)),
    )
    .await;
    assert_eq!(result.scoreable_status, ScoreableStatus::LimitExceeded);
    assert!(!result.task_completion_pass);
    assert_eq!(result.tool_calls.len(), 13);
    assert_eq!(
        result
            .tool_calls
            .iter()
            .filter(|call| call.forwarded)
            .count(),
        12
    );

    let capture = Arc::new(Mutex::new(Vec::<ModelRequest>::new()));
    struct CaptureAdapter {
        capture: Arc<Mutex<Vec<ModelRequest>>>,
    }
    #[async_trait]
    impl ModelAdapter for CaptureAdapter {
        fn adapter_id(&self) -> &str {
            "capture"
        }
        fn adapter_version(&self) -> &str {
            "1"
        }
        fn requested_model(&self) -> &str {
            "capture-model"
        }
        fn class(&self) -> AdapterClass {
            AdapterClass::NonFormal
        }
        async fn respond(
            &mut self,
            request: &ModelRequest,
        ) -> Result<ModelTurn, super::adapter::AdapterError> {
            self.capture.lock().unwrap().push(request.clone());
            Ok(ModelTurn {
                resolved_model: "capture-model".into(),
                api_surface: Some("scripted".into()),
                tool_calls: Vec::new(),
                final_text: Some("PORT=4317".into()),
                usage: ProviderUsage::default(),
                stop_reason: "completed".into(),
                attempts: 1,
            })
        }
    }
    let result = execute_case(
        case_execution(
            &dataset,
            case(&dataset, "m04-c1-01"),
            &binary,
            base_hash,
            schemas,
            false,
        ),
        Box::new(CaptureAdapter {
            capture: Arc::clone(&capture),
        }),
    )
    .await;
    assert!(result.task_completion_pass);
    let requests = capture.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].max_output_tokens, 2048);
}

#[tokio::test]
async fn int14_output_privacy_sentinels_are_sanitized() {
    let dataset = dataset();
    let binary = runtime_binary();
    let (hash, schemas) = probe_tools(&dataset, &binary).await;
    let adapter = ScriptedAdapter::new(vec![ScriptedTurn {
        tool_calls: Vec::new(),
        final_text: Some("sk-live-secret https://user:pass@example.test/path".into()),
    }]);
    let result = execute_case(
        case_execution(
            &dataset,
            case(&dataset, "m04-c1-01"),
            &binary,
            hash,
            schemas,
            false,
        ),
        Box::new(adapter),
    )
    .await;
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("sk-live-secret"));
    assert!(!json.contains("user:pass"));
    assert!(json.contains("sk-[REDACTED]"));
    assert!(result.final_state_evidence.final_response.is_some());
    let sanitized = sanitize_bounded_text(
        "C:\\Temp\\m04-workspace sk-live https://u:p@host/x",
        Some(Path::new("C:\\Temp\\m04-workspace")),
        128,
    );
    assert!(!sanitized.contains("C:\\Temp\\m04-workspace"));
    assert!(!sanitized.contains("sk-live"));
    assert!(!sanitized.contains("u:p"));
}

#[tokio::test]
async fn int15_approvals_are_live_request_scoped_and_not_reused() {
    let dataset = dataset();
    let binary = runtime_binary();
    let probe = case(&dataset, "m04-c4-01");
    let mut first =
        EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
            .await
            .unwrap();
    assert!(approvals::list_pending(&mut first)
        .await
        .unwrap()
        .is_empty());
    let stale = approvals::resolve(&mut first, "stale-approval", "allow_once").await;
    assert!(stale.is_err());
    let mut mcp = McpClient::connect(&first.endpoint_url).await.unwrap();
    let call = mcp.call_tool(
        "write",
        json!({"path": "report_output.txt", "content": "report=ok"}),
    );
    tokio::pin!(call);
    let mut first_pending = None;
    loop {
        tokio::select! {
            _ = &mut call => break,
            _ = tokio::time::sleep(Duration::from_millis(20)) => {
                let pending = approvals::list_pending(&mut first).await.unwrap();
                if let Some(item) = pending.first() {
                    first_pending = Some(item.request_id.clone());
                    approvals::resolve(&mut first, &item.request_id, "deny").await.unwrap();
                }
            }
        }
    }
    let first_pending = first_pending.expect("live approval observed");
    let first_runtime_id = first.runtime_id.clone();
    first.shutdown().await.unwrap();

    let mut second =
        EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
            .await
            .unwrap();
    assert!(approvals::list_pending(&mut second)
        .await
        .unwrap()
        .is_empty());
    assert_ne!(first_runtime_id, second.runtime_id);
    let mut second_mcp = McpClient::connect(&second.endpoint_url).await.unwrap();
    let second_call = second_mcp.call_tool(
        "write",
        json!({"path": "report_output.txt", "content": "report=ok"}),
    );
    tokio::pin!(second_call);
    let mut second_pending = None;
    loop {
        tokio::select! {
            _ = &mut second_call => break,
            _ = tokio::time::sleep(Duration::from_millis(20)) => {
                let pending = approvals::list_pending(&mut second).await.unwrap();
                if let Some(item) = pending.first() {
                    second_pending = Some(item.request_id.clone());
                    approvals::resolve(&mut second, &item.request_id, "deny").await.unwrap();
                }
            }
        }
    }
    let second_pending = second_pending.expect("new runtime gets a new live approval");
    assert_ne!(first_pending, second_pending);
    second.shutdown().await.unwrap();
}

#[tokio::test]
async fn int16_eval_isolation_invariants_are_visible_in_source_contract() {
    let dataset = dataset();
    assert!(dataset
        .cases
        .iter()
        .all(|item| item.category != EvalCategory::ContentSearch
            || item
                .selection_oracle
                .strategies
                .iter()
                .flat_map(|strategy| strategy.steps.iter())
                .all(|step| step.tool != "grep")));
    let binary = runtime_binary();
    let hash = runtime_binary_hash(&binary).unwrap();
    assert_eq!(hash.len(), 64);
    let (catalog_hash, schemas) = probe_tools(&dataset, &binary).await;
    assert_eq!(catalog_hash.len(), 64);
    assert_eq!(
        schemas.keys().cloned().collect::<Vec<_>>(),
        vec!["edit", "glob", "read_file", "write"]
    );
}
