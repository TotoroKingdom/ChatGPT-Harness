use super::adapter::{ModelAdapter, ModelToolCall, ScriptedAdapter, ScriptedTurn};
use super::agent_loop::{
    execute_case, prepare_tool_schemas_for_run, scripted_adapter_for_case, CaseExecution,
};
use super::aggregate::ScoreableStatus;
use super::correlation;
use super::dataset::EvalDataset;
use super::mcp_client::{exposed_tool_names, tool_catalog_hash, EvalRuntimeHarness, McpClient};
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

fn runtime_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("HARNESS_E2E_RUNTIME_BIN") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
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
}

fn dataset() -> Option<EvalDataset> {
    EvalDataset::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("v4")
            .join("m04-eval"),
    )
    .ok()
}

#[tokio::test]
async fn real_runtime_exposes_frozen_catalog_and_exact_evidence_identity() {
    let (Some(binary), Some(dataset)) = (runtime_binary(), dataset()) else {
        eprintln!("SKIP: compiled Runtime binary is unavailable");
        return;
    };
    let case = dataset.case("m04-c1-01").unwrap();
    let mut harness =
        EvalRuntimeHarness::start(&binary, case, &dataset.fixture_path(case).unwrap())
            .await
            .unwrap();
    let workspace = harness.workspace_root.clone();
    let database = harness.evidence_database.clone();
    let runtime_id = harness.runtime_id.clone();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let tools = mcp.list_tools().await.unwrap();
    assert_eq!(
        exposed_tool_names(&tools).unwrap(),
        vec!["read_file", "glob", "write", "edit"]
    );
    assert_eq!(tool_catalog_hash(&tools).unwrap().len(), 64);
    let result = mcp
        .call_tool("read_file", json!({"path": "config/service.txt"}))
        .await
        .unwrap();
    let key = correlation::extract(&result).expect("exact correlation metadata");
    assert_eq!(key.runtime_id, runtime_id);
    let evidence = correlation::verify_evidence(&database, &key, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(evidence.summary.request_id, key.request_id);
    harness.shutdown().await.unwrap();
    assert!(!workspace.exists());
}

#[tokio::test]
async fn every_enforcement_case_uses_real_runtime_paths() {
    let (Some(binary), Some(dataset)) = (runtime_binary(), dataset()) else {
        eprintln!("SKIP: compiled Runtime binary is unavailable");
        return;
    };
    let probe = dataset.case("m04-c1-01").unwrap();
    let harness = EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
        .await
        .unwrap();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let tools = mcp.list_tools().await.unwrap();
    let schemas = prepare_tool_schemas_for_run(&tools).unwrap();
    let hash = tool_catalog_hash(&tools).unwrap();
    harness.shutdown().await.unwrap();

    for case_id in ["m04-c6-01", "m04-c6-02", "m04-c6-03", "m04-c6-04"] {
        let case = dataset.case(case_id).unwrap();
        let result = execute_case(
            CaseExecution {
                dataset: &dataset,
                case,
                runtime_binary: &binary,
                base_tool_schema_hash: hash.clone(),
                base_tool_schemas: schemas.clone(),
                runner_commit: "integration".into(),
                runtime_commit: "integration".into(),
                logical_run_index: 1,
                repetition: 1,
                cleanup_failure_injection: false,
            },
            Box::new(scripted_adapter_for_case(case)) as Box<dyn ModelAdapter>,
        )
        .await;
        assert_eq!(
            result.scoreable_status,
            ScoreableStatus::Scoreable,
            "{case_id}"
        );
        assert!(result.selection_pass, "{case_id}");
        assert!(result.task_completion_pass, "{case_id}");
        assert_eq!(
            result.runtime_outcome_match_count,
            result.runtime_outcome_total_count
        );
        assert!(result.tool_calls.iter().all(|call| call.evidence_verified));
    }
}

#[tokio::test]
async fn two_runs_get_distinct_runtime_workspace_and_database() {
    let (Some(binary), Some(dataset)) = (runtime_binary(), dataset()) else {
        eprintln!("SKIP: compiled Runtime binary is unavailable");
        return;
    };
    let case = dataset.case("m04-c1-01").unwrap();
    let fixture = dataset.fixture_path(case).unwrap();
    let first = EvalRuntimeHarness::start(&binary, case, &fixture)
        .await
        .unwrap();
    let second = EvalRuntimeHarness::start(&binary, case, &fixture)
        .await
        .unwrap();
    assert_ne!(first.runtime_id, second.runtime_id);
    assert_ne!(first.workspace_root, second.workspace_root);
    assert_ne!(first.evidence_database, second.evidence_database);
    first.shutdown().await.unwrap();
    second.shutdown().await.unwrap();
}

#[tokio::test]
async fn schema_invalid_call_is_preserved_and_never_forwarded() {
    let (Some(binary), Some(dataset)) = (runtime_binary(), dataset()) else {
        eprintln!("SKIP: compiled Runtime binary is unavailable");
        return;
    };
    let probe = dataset.case("m04-c1-01").unwrap();
    let harness = EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
        .await
        .unwrap();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let tools = mcp.list_tools().await.unwrap();
    let schemas = prepare_tool_schemas_for_run(&tools).unwrap();
    let hash = tool_catalog_hash(&tools).unwrap();
    harness.shutdown().await.unwrap();

    let case = dataset.case("m04-c1-01").unwrap();
    let adapter = ScriptedAdapter::new(vec![
        ScriptedTurn {
            tool_calls: vec![ModelToolCall {
                id: "invalid-1".into(),
                name: "read_file".into(),
                arguments: "{not-json".into(),
            }],
            final_text: None,
        },
        ScriptedTurn {
            tool_calls: Vec::new(),
            final_text: Some("PORT=4317".into()),
        },
    ]);
    let result = execute_case(
        CaseExecution {
            dataset: &dataset,
            case,
            runtime_binary: &binary,
            base_tool_schema_hash: hash,
            base_tool_schemas: schemas,
            runner_commit: "integration".into(),
            runtime_commit: "integration".into(),
            logical_run_index: 1,
            repetition: 1,
            cleanup_failure_injection: false,
        },
        Box::new(adapter) as Box<dyn ModelAdapter>,
    )
    .await;
    assert_eq!(result.scoreable_status, ScoreableStatus::Scoreable);
    assert_eq!(result.argument_total_count, 1);
    assert_eq!(result.argument_valid_count, 0);
    assert!(!result.tool_calls[0].schema_valid);
    assert!(!result.tool_calls[0].forwarded);
    assert!(result.tool_calls[0].request_id.is_none());
    assert_eq!(result.runtime_outcome_total_count, 0);
}

#[tokio::test]
async fn max_model_turns_is_reported_as_limit_exceeded_task_failure() {
    let (Some(binary), Some(dataset)) = (runtime_binary(), dataset()) else {
        eprintln!("SKIP: compiled Runtime binary is unavailable");
        return;
    };
    let probe = dataset.case("m04-c1-01").unwrap();
    let harness = EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
        .await
        .unwrap();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let tools = mcp.list_tools().await.unwrap();
    let schemas = prepare_tool_schemas_for_run(&tools).unwrap();
    let hash = tool_catalog_hash(&tools).unwrap();
    harness.shutdown().await.unwrap();

    let case = dataset.case("m04-c1-01").unwrap();
    let turns = (0..9)
        .map(|index| ScriptedTurn {
            tool_calls: vec![ModelToolCall {
                id: format!("call-{index}"),
                name: "read_file".into(),
                arguments: json!({"path": "config/service.txt"}).to_string(),
            }],
            final_text: None,
        })
        .collect::<Vec<_>>();
    let result = execute_case(
        CaseExecution {
            dataset: &dataset,
            case,
            runtime_binary: &binary,
            base_tool_schema_hash: hash,
            base_tool_schemas: schemas,
            runner_commit: "integration".into(),
            runtime_commit: "integration".into(),
            logical_run_index: 1,
            repetition: 1,
            cleanup_failure_injection: false,
        },
        Box::new(ScriptedAdapter::new(turns)) as Box<dyn ModelAdapter>,
    )
    .await;
    assert_eq!(result.scoreable_status, ScoreableStatus::LimitExceeded);
    assert_eq!(result.termination_reason, "limit_exceeded");
    assert!(!result.task_completion_pass);
    assert_eq!(result.model_turns, 8);
}

#[tokio::test]
async fn unexpected_approval_is_denied_and_not_auto_approved() {
    let (Some(binary), Some(dataset)) = (runtime_binary(), dataset()) else {
        eprintln!("SKIP: compiled Runtime binary is unavailable");
        return;
    };
    let probe = dataset.case("m04-c1-01").unwrap();
    let harness = EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
        .await
        .unwrap();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let tools = mcp.list_tools().await.unwrap();
    let schemas = prepare_tool_schemas_for_run(&tools).unwrap();
    let hash = tool_catalog_hash(&tools).unwrap();
    harness.shutdown().await.unwrap();

    let case = dataset.case("m04-c1-01").unwrap();
    let adapter = ScriptedAdapter::new(vec![
        ScriptedTurn {
            tool_calls: vec![ModelToolCall {
                id: "unexpected-write".into(),
                name: "write".into(),
                arguments: json!({"path": "unexpected.txt", "content": "must-not-exist"})
                    .to_string(),
            }],
            final_text: None,
        },
        ScriptedTurn {
            tool_calls: Vec::new(),
            final_text: Some("PORT=4317".into()),
        },
    ]);
    let result = execute_case(
        CaseExecution {
            dataset: &dataset,
            case,
            runtime_binary: &binary,
            base_tool_schema_hash: hash,
            base_tool_schemas: schemas,
            runner_commit: "integration".into(),
            runtime_commit: "integration".into(),
            logical_run_index: 1,
            repetition: 1,
            cleanup_failure_injection: false,
        },
        Box::new(adapter) as Box<dyn ModelAdapter>,
    )
    .await;
    assert_eq!(
        result.scoreable_status,
        ScoreableStatus::HarnessContractError
    );
    assert_eq!(
        result.diagnostic_code.as_deref(),
        Some("HARNESS_CONTRACT_ERROR")
    );
    assert!(result.tool_calls.iter().all(|call| !call.forwarded));
}

#[tokio::test]
async fn result_json_excludes_physical_workspace_and_credentials() {
    let (Some(binary), Some(dataset)) = (runtime_binary(), dataset()) else {
        eprintln!("SKIP: compiled Runtime binary is unavailable");
        return;
    };
    let probe = dataset.case("m04-c1-01").unwrap();
    let harness = EvalRuntimeHarness::start(&binary, probe, &dataset.fixture_path(probe).unwrap())
        .await
        .unwrap();
    let workspace = harness.workspace_root.clone();
    let mut mcp = McpClient::connect(&harness.endpoint_url).await.unwrap();
    let tools = mcp.list_tools().await.unwrap();
    let schemas = prepare_tool_schemas_for_run(&tools).unwrap();
    let hash = tool_catalog_hash(&tools).unwrap();
    harness.shutdown().await.unwrap();

    let case = dataset.case("m04-c1-01").unwrap();
    let result = execute_case(
        CaseExecution {
            dataset: &dataset,
            case,
            runtime_binary: &binary,
            base_tool_schema_hash: hash,
            base_tool_schemas: schemas,
            runner_commit: "integration".into(),
            runtime_commit: "integration".into(),
            logical_run_index: 1,
            repetition: 1,
            cleanup_failure_injection: false,
        },
        Box::new(scripted_adapter_for_case(case)) as Box<dyn ModelAdapter>,
    )
    .await;
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains(&workspace.to_string_lossy().to_string()));
    assert!(!json.contains("sk-"));
    assert!(!json.contains("C:\\\\"));
}
