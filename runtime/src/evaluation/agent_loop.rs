use super::adapter::{
    model_request, ConversationItem, ModelAdapter, ModelRequest, ModelToolCall, ModelToolSpec,
    ProviderUsage, ScriptedAdapter, ScriptedTurn,
};
use super::aggregate::{EvalRunResult, ScoreableStatus};
use super::approvals;
use super::case::{ApprovalPlan, EvalCaseV1, PolicyExpectationValue};
use super::correlation::{self, CorrelationKey};
use super::dataset::EvalDataset;
use super::manifest::{eval_profile_hash, sha256_text, RunManifest};
use super::mcp_client::{
    base_policy_config_hash, exposed_tool_names, runtime_binary_hash, tool_catalog_hash,
    tool_schema_map, EvalRuntimeHarness, McpClient,
};
use super::privacy::sanitize_bounded_text;
use super::scoring::{
    bounded_argument_evidence, normalize_runtime_outcome, score_arguments, score_final_state,
    score_runtime_outcomes, score_selection, validate_schema, ToolCallRecord,
};
use super::{EvalError, EvalResult};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) struct CaseExecution<'a> {
    pub(crate) dataset: &'a EvalDataset,
    pub(crate) case: &'a EvalCaseV1,
    pub(crate) runtime_binary: &'a Path,
    pub(crate) base_tool_schema_hash: String,
    pub(crate) base_tool_schemas: BTreeMap<String, Value>,
    pub(crate) runner_commit: String,
    pub(crate) runtime_commit: String,
    pub(crate) logical_run_index: usize,
    pub(crate) repetition: u32,
    pub(crate) cleanup_failure_injection: bool,
}

#[derive(Default)]
struct LoopState {
    calls: Vec<ToolCallRecord>,
    model_turns: u32,
    final_response: Option<String>,
    termination_reason: String,
    resolved_model: String,
    usage: ProviderUsage,
    provider_attempts: u32,
    status_override: Option<ScoreableStatus>,
    diagnostic_code: Option<String>,
}

pub(crate) async fn execute_case(
    execution: CaseExecution<'_>,
    mut adapter: Box<dyn ModelAdapter>,
) -> EvalRunResult {
    let started = Instant::now();
    let run_id = format!(
        "m04-{}-r{}-{}",
        execution.case.case_id,
        execution.repetition,
        uuid::Uuid::new_v4()
    );
    let binary_hash =
        runtime_binary_hash(execution.runtime_binary).unwrap_or_else(|_| "unknown".into());
    let fixture_source = match execution.dataset.fixture_path(execution.case) {
        Ok(value) => value,
        Err(error) => {
            return infra_result(execution, run_id, started, binary_hash, error);
        }
    };
    let mut harness =
        match EvalRuntimeHarness::start(execution.runtime_binary, execution.case, &fixture_source)
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return infra_result(execution, run_id, started, binary_hash, error);
            }
        };
    let workspace_root = harness.workspace_root.clone();
    let evidence_database = harness.evidence_database.clone();
    let runtime_id = harness.runtime_id.clone();
    let policy_config_hash = harness.policy_config_hash.clone();
    if execution.cleanup_failure_injection {
        harness.inject_cleanup_failure();
    }
    let mut mcp = match McpClient::connect(&harness.endpoint_url).await {
        Ok(value) => value,
        Err(error) => {
            let _ = harness.shutdown().await;
            return infra_result(execution, run_id, started, binary_hash, error);
        }
    };
    let tools_result = match mcp.list_tools().await {
        Ok(value) => value,
        Err(error) => {
            let _ = harness.shutdown().await;
            return infra_result(execution, run_id, started, binary_hash, error);
        }
    };
    if let Err(error) = validate_catalog(execution.case, &tools_result) {
        let _ = harness.shutdown().await;
        return contract_result(execution, run_id, started, binary_hash, runtime_id, error);
    }
    let observed_catalog_hash =
        tool_catalog_hash(&tools_result).unwrap_or_else(|_| "unknown".into());
    if observed_catalog_hash != execution.base_tool_schema_hash {
        let _ = harness.shutdown().await;
        return contract_result(
            execution,
            run_id,
            started,
            binary_hash,
            runtime_id,
            EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "MCP Tool catalog hash differs from the frozen formal profile",
            ),
        );
    }
    let model_tools = match model_tools(&tools_result) {
        Ok(value) => value,
        Err(error) => {
            let _ = harness.shutdown().await;
            return contract_result(execution, run_id, started, binary_hash, runtime_id, error);
        }
    };
    let request = model_request(
        execution.dataset.system_prompt.clone(),
        execution.case.prompt.clone(),
        model_tools,
    );
    let mut state = LoopState {
        termination_reason: "not_started".into(),
        ..LoopState::default()
    };
    let loop_result = tokio::time::timeout(
        super::RUN_TIMEOUT,
        run_loop(
            &mut harness,
            &mut mcp,
            execution.case,
            &execution.base_tool_schemas,
            &evidence_database,
            adapter.as_mut(),
            request,
            &mut state,
        ),
    )
    .await;
    match loop_result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let code = error.code;
            state.diagnostic_code = Some(code.to_owned());
            state.status_override = Some(match code {
                "SECURITY_VIOLATION" => ScoreableStatus::SecurityViolation,
                "HARNESS_CONTRACT_ERROR" => ScoreableStatus::HarnessContractError,
                "INFRA_ERROR" if state.calls.is_empty() => ScoreableStatus::InfraError,
                _ => ScoreableStatus::Scoreable,
            });
            if state.termination_reason == "not_started" {
                state.termination_reason = if code == "INFRA_ERROR" {
                    "provider_or_transport_error".into()
                } else {
                    "contract_error".into()
                };
            }
        }
        Err(_) => {
            state.termination_reason = "limit_exceeded".into();
            state.status_override = Some(ScoreableStatus::LimitExceeded);
        }
    }

    let mut final_state = match score_final_state(
        execution.case,
        &workspace_root,
        state.final_response.as_deref(),
    ) {
        Ok(value) => value,
        Err(error) => {
            if state.status_override != Some(ScoreableStatus::SecurityViolation) {
                state.diagnostic_code = Some(error.code.to_owned());
                state.status_override = Some(ScoreableStatus::HarnessContractError);
            }
            Default::default()
        }
    };
    let shutdown = harness.shutdown().await;
    let cleanup_status = match shutdown {
        Ok(value) => value,
        Err(error) => {
            state.diagnostic_code = Some(error.code.to_owned());
            state.status_override = Some(ScoreableStatus::InfraError);
            format!("{}: {}", error.code, error.message)
        }
    };
    let mut status = state.status_override.unwrap_or(ScoreableStatus::Scoreable);
    let selection_pass = score_selection(execution.case, &mut state.calls);
    let argument_total = state.calls.len() as u32;
    let argument_valid = state
        .calls
        .iter()
        .filter(|call| call.schema_valid && call.semantic_valid)
        .count() as u32;
    let (runtime_matches, runtime_total) = score_runtime_outcomes(execution.case, &state.calls);
    let unnecessary = state.calls.iter().filter(|call| call.unnecessary).count() as u32;
    let mut task_completion_pass = final_state.task_completion_pass;
    if status == ScoreableStatus::LimitExceeded {
        task_completion_pass = false;
    }
    if !matches!(
        status,
        ScoreableStatus::Scoreable | ScoreableStatus::LimitExceeded
    ) {
        task_completion_pass = false;
    }
    if status == ScoreableStatus::LimitExceeded && state.calls.is_empty() {
        status = ScoreableStatus::InfraError;
    }
    final_state.task_completion_pass = task_completion_pass;
    let eval_profile = eval_profile_hash(
        &sha256_text(&execution.dataset.system_prompt),
        adapter.adapter_id(),
        adapter.adapter_version(),
        adapter.requested_model(),
    )
    .unwrap_or_else(|_| "unknown".into());
    let policy_hash = policy_config_hash;
    let resolved_model = if state.resolved_model.is_empty() {
        "unknown".to_owned()
    } else {
        state.resolved_model.clone()
    };
    let manifest_hash = RunManifest {
        run_id: run_id.clone(),
        logical_run_index: execution.logical_run_index,
        case_id: execution.case.case_id.clone(),
        case_version: execution.case.case_version,
        repetition: execution.repetition,
        dataset_hash: execution.dataset.dataset_hash.clone(),
        scorer_hash: execution.dataset.scorer_hash.clone(),
        system_prompt_hash: execution.dataset.system_prompt_hash.clone(),
        eval_profile_hash: eval_profile,
        tool_schema_hash: execution.base_tool_schema_hash.clone(),
        policy_config_hash: policy_hash.clone(),
        adapter_id: adapter.adapter_id().to_owned(),
        adapter_version: adapter.adapter_version().to_owned(),
        requested_model: adapter.requested_model().to_owned(),
        resolved_model: resolved_model.clone(),
        runner_commit: execution.runner_commit.clone(),
        runtime_commit: execution.runtime_commit.clone(),
        runtime_binary_hash: binary_hash.clone(),
        runner_version: super::manifest::RUNNER_VERSION.into(),
        scorer_version: super::manifest::SCORER_VERSION.into(),
    }
    .manifest_hash()
    .unwrap_or_else(|_| "unknown".into());
    EvalRunResult {
        run_id,
        logical_run_index: execution.logical_run_index,
        case_id: execution.case.case_id.clone(),
        case_version: execution.case.case_version,
        dataset_hash: execution.dataset.dataset_hash.clone(),
        fixture_hash: execution.case.fixture_hash.clone(),
        runner_commit: execution.runner_commit.clone(),
        runtime_commit: execution.runtime_commit.clone(),
        runtime_binary_hash: binary_hash,
        runtime_id,
        request_ids: state
            .calls
            .iter()
            .filter_map(|call| call.request_id.clone())
            .collect(),
        tool_schema_hash: execution.base_tool_schema_hash.clone(),
        observed_tool_catalog_hash: observed_catalog_hash,
        policy_config_hash: policy_hash,
        adapter_id: adapter.adapter_id().to_owned(),
        adapter_version: adapter.adapter_version().to_owned(),
        requested_model: adapter.requested_model().to_owned(),
        resolved_model,
        system_prompt_hash: execution.dataset.system_prompt_hash.clone(),
        repetition: execution.repetition,
        termination_reason: if state.termination_reason == "not_started" {
            "contract_error".into()
        } else {
            state.termination_reason
        },
        model_turns: state.model_turns,
        tool_calls: state.calls,
        selection_pass,
        argument_valid_count: argument_valid,
        argument_total_count: argument_total,
        runtime_outcome_match_count: runtime_matches,
        runtime_outcome_total_count: runtime_total,
        task_completion_pass,
        unnecessary_call_count: unnecessary,
        scoreable_status: status,
        final_state_evidence: final_state,
        usage: if state.usage == ProviderUsage::default() {
            None
        } else {
            Some(state.usage)
        },
        elapsed_ms: started.elapsed().as_millis() as u64,
        cleanup_status,
        provider_attempts: state.provider_attempts,
        manifest_hash,
        diagnostic_code: state.diagnostic_code,
    }
}

async fn run_loop(
    harness: &mut EvalRuntimeHarness,
    mcp: &mut McpClient,
    case: &EvalCaseV1,
    base_tool_schemas: &BTreeMap<String, Value>,
    evidence_database: &Path,
    adapter: &mut dyn ModelAdapter,
    mut request: ModelRequest,
    state: &mut LoopState,
) -> EvalResult<()> {
    let mut first_resolved_model: Option<String> = None;
    loop {
        if state.model_turns >= case.max_turns() {
            state.termination_reason = "limit_exceeded".into();
            state.status_override = Some(ScoreableStatus::LimitExceeded);
            return Ok(());
        }
        let turn = adapter.respond(&request).await.map_err(|error| {
            state.termination_reason = "provider_error".into();
            EvalError::new("INFRA_ERROR", error.message)
        })?;
        state.model_turns += 1;
        merge_usage(&mut state.usage, &turn.usage);
        state.provider_attempts += turn.attempts;
        if state
            .usage
            .cost_usd
            .is_some_and(|cost| cost > super::RUN_SPEND_CAP_USD)
        {
            state.termination_reason = "limit_exceeded".into();
            state.status_override = Some(ScoreableStatus::LimitExceeded);
            return Ok(());
        }
        if state.resolved_model.is_empty() {
            state.resolved_model = turn.resolved_model.clone();
        } else if state.resolved_model != turn.resolved_model {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "resolved model identity changed during one logical run",
            ));
        }
        if let Some(first) = &first_resolved_model {
            if first != &turn.resolved_model {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    "resolved model identity changed during one logical run",
                ));
            }
        } else {
            first_resolved_model = Some(turn.resolved_model.clone());
        }
        if turn.tool_calls.is_empty() {
            state.final_response = turn.final_text;
            state.termination_reason = if state.final_response.is_some() {
                "final_response".into()
            } else {
                "empty_model_turn".into()
            };
            if state.final_response.is_none() {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    "model turn contained neither Tool calls nor a final response",
                ));
            }
            return Ok(());
        }
        request.items.push(ConversationItem::AssistantToolCalls {
            tool_calls: turn.tool_calls.clone(),
        });
        for call in turn.tool_calls {
            if state.calls.len() >= case.max_tool_calls() as usize {
                let record = emitted_record(case, base_tool_schemas, &call);
                state.calls.push(record);
                state.termination_reason = "limit_exceeded".into();
                state.status_override = Some(ScoreableStatus::LimitExceeded);
                return Ok(());
            }
            let (record, tool_result) = forward_call(
                harness,
                mcp,
                case,
                base_tool_schemas,
                evidence_database,
                &call,
            )
            .await?;
            request.items.push(ConversationItem::ToolResult {
                call_id: call.id.clone(),
                name: call.name.clone(),
                content: model_visible_result(&tool_result),
                is_error: tool_result
                    .get("isError")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            });
            state.calls.push(record);
        }
        if let Some(text) = turn.final_text {
            state.final_response = Some(text);
            state.termination_reason = "final_response".into();
            return Ok(());
        }
    }
}

async fn forward_call(
    harness: &mut EvalRuntimeHarness,
    mcp: &mut McpClient,
    case: &EvalCaseV1,
    base_tool_schemas: &BTreeMap<String, Value>,
    evidence_database: &Path,
    call: &ModelToolCall,
) -> EvalResult<(ToolCallRecord, Value)> {
    let parsed = call.parsed_arguments();
    let schema_valid = base_tool_schemas
        .get(&call.name)
        .is_some_and(|schema| validate_schema(schema, parsed.parsed.as_ref()));
    let mut record = ToolCallRecord {
        model_tool_call_id: sanitize_bounded_text(&call.id, None, 128),
        tool_name: sanitize_bounded_text(&call.name, None, 64),
        schema_valid,
        semantic_valid: false,
        forwarded: false,
        runtime_id: None,
        request_id: None,
        runtime_outcome: None,
        safe_error_code: None,
        approval_decision: None,
        approval_request_id: None,
        argument_evidence: bounded_argument_evidence(&call.arguments),
        evidence_verified: false,
        unnecessary: false,
    };
    record.semantic_valid = score_arguments(case, &record, parsed.parsed.as_ref());
    if !schema_valid {
        return Ok((
            record,
            json!({
                "isError": true,
                "structuredContent": {
                    "code": "INVALID_ARGUMENTS",
                    "message": "model arguments did not validate against the real tool schema"
                }
            }),
        ));
    }
    let arguments = parsed.parsed.ok_or_else(|| {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "schema validation accepted non-JSON arguments",
        )
    })?;
    let expected_policy = case.policy_expectation_value(&call.name);
    let expects_approval = matches!(expected_policy, Some(PolicyExpectationValue::Ask));
    let approval_arguments = arguments.clone();
    let call_future = mcp.call_tool(&call.name, arguments);
    tokio::pin!(call_future);
    let mut observed_approval = false;
    let result = loop {
        tokio::select! {
            result = &mut call_future => break result?,
            _ = tokio::time::sleep(Duration::from_millis(20)) => {
                let pending = approvals::list_pending(harness).await?;
                if let Some(approval) = pending.into_iter().next() {
                    if approval.runtime_id != harness.runtime_id {
                        return Err(EvalError::new(
                            "SECURITY_VIOLATION",
                            "pending approval belongs to a different Runtime",
                        ));
                    }
                    if !approval_matches_call(&approval, &call.name, &approval_arguments) {
                        let _ = approvals::resolve(harness, &approval.request_id, "deny").await;
                        return Err(EvalError::new(
                            "HARNESS_CONTRACT_ERROR",
                            "unexpected approval for a different tool",
                        ));
                    }
                    let decision = match case.approval_plan {
                        ApprovalPlan::AllowOnce => "allow_once",
                        ApprovalPlan::Deny => "deny",
                        ApprovalPlan::None => {
                            let _ = approvals::resolve(harness, &approval.request_id, "deny").await;
                            return Err(EvalError::new(
                                "HARNESS_CONTRACT_ERROR",
                                "unexpected approval was not auto-approved",
                            ));
                        }
                    };
                    approvals::resolve(harness, &approval.request_id, decision).await?;
                    observed_approval = true;
                    record.approval_decision = Some(decision.into());
                    record.approval_request_id = Some(approval.request_id);
                }
            }
        }
    };
    if expects_approval && !observed_approval {
        return Err(EvalError::new(
            "SECURITY_VIOLATION",
            "expected ASK invocation did not use the real approval path",
        ));
    }
    let key: CorrelationKey = correlation::extract(&result).ok_or_else(|| {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "MCP Tool result omitted exact runtime_id + request_id metadata",
        )
    })?;
    if key.runtime_id != harness.runtime_id {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "MCP Tool result runtime_id does not match the isolated Runtime",
        ));
    }
    if let Some(approval_request_id) = &record.approval_request_id {
        if approval_request_id != &key.request_id {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "ASK approval request_id does not match the forwarded invocation identity",
            ));
        }
    }
    correlation::verify_live_activity(harness, &key).await?;
    let evidence =
        correlation::verify_evidence(evidence_database, &key, Duration::from_secs(5)).await?;
    record.evidence_verified = evidence.summary.request_id == key.request_id;
    record.runtime_id = Some(key.runtime_id.clone());
    record.request_id = Some(key.request_id.clone());
    record.forwarded = true;
    record.runtime_outcome = Some(normalize_runtime_outcome(Some(&result)));
    record.safe_error_code = result
        .get("structuredContent")
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut model_result = result;
    correlation::strip_model_metadata(&mut model_result);
    Ok((record, model_result))
}

fn model_visible_result(result: &Value) -> String {
    let mut text = serde_json::to_string(result).unwrap_or_else(|_| "{}".into());
    text.truncate(16_384);
    text
}

fn approval_matches_call(
    approval: &approvals::PendingApproval,
    tool: &str,
    arguments: &Value,
) -> bool {
    if approval.tool != tool {
        return false;
    }
    let expected_path = approval.sanitized_arguments.get("path");
    match (expected_path, arguments.get("path").and_then(Value::as_str)) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn emitted_record(
    case: &EvalCaseV1,
    base_tool_schemas: &BTreeMap<String, Value>,
    call: &ModelToolCall,
) -> ToolCallRecord {
    let parsed = call.parsed_arguments();
    let schema_valid = base_tool_schemas
        .get(&call.name)
        .is_some_and(|schema| validate_schema(schema, parsed.parsed.as_ref()));
    let mut record = ToolCallRecord {
        model_tool_call_id: sanitize_bounded_text(&call.id, None, 128),
        tool_name: sanitize_bounded_text(&call.name, None, 64),
        schema_valid,
        semantic_valid: false,
        forwarded: false,
        runtime_id: None,
        request_id: None,
        runtime_outcome: None,
        safe_error_code: None,
        approval_decision: None,
        approval_request_id: None,
        argument_evidence: bounded_argument_evidence(&call.arguments),
        evidence_verified: false,
        unnecessary: false,
    };
    record.semantic_valid = score_arguments(case, &record, parsed.parsed.as_ref());
    record
}

fn model_tools(tools_result: &Value) -> EvalResult<Vec<ModelToolSpec>> {
    let tools = tools_result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "tools/list omitted tools"))?;
    tools
        .iter()
        .map(|tool| {
            Ok(ModelToolSpec {
                name: tool
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "tool omitted name"))?
                    .to_owned(),
                description: tool
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                input_schema: tool.get("inputSchema").cloned().ok_or_else(|| {
                    EvalError::new("HARNESS_CONTRACT_ERROR", "tool omitted inputSchema")
                })?,
            })
        })
        .collect()
}

fn validate_catalog(_case: &EvalCaseV1, tools_result: &Value) -> EvalResult<()> {
    let names = exposed_tool_names(tools_result)?;
    let expected = super::BASELINE_TOOLS
        .iter()
        .map(|tool| (*tool).to_owned())
        .collect::<Vec<_>>();
    if names != expected {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("tools/list catalog drifted: expected {expected:?}, observed {names:?}"),
        ));
    }
    if names.iter().any(|name| name == "bash" || name == "grep") {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "forbidden tool is exposed in the M04 profile",
        ));
    }
    Ok(())
}

fn merge_usage(total: &mut ProviderUsage, value: &ProviderUsage) {
    total.input_tokens = add(total.input_tokens, value.input_tokens);
    total.output_tokens = add(total.output_tokens, value.output_tokens);
    total.total_tokens = add(total.total_tokens, value.total_tokens);
    total.cost_usd = add_f64(total.cost_usd, value.cost_usd);
}

fn add(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn add_f64(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn infra_result(
    execution: CaseExecution<'_>,
    run_id: String,
    started: Instant,
    binary_hash: String,
    error: EvalError,
) -> EvalRunResult {
    base_result(
        execution,
        run_id,
        started,
        binary_hash,
        ScoreableStatus::InfraError,
        error,
    )
}

fn contract_result(
    execution: CaseExecution<'_>,
    run_id: String,
    started: Instant,
    binary_hash: String,
    runtime_id: String,
    error: EvalError,
) -> EvalRunResult {
    let mut result = base_result(
        execution,
        run_id,
        started,
        binary_hash,
        if error.code == "SECURITY_VIOLATION" {
            ScoreableStatus::SecurityViolation
        } else {
            ScoreableStatus::HarnessContractError
        },
        error,
    );
    result.runtime_id = runtime_id;
    result
}

fn base_result(
    execution: CaseExecution<'_>,
    run_id: String,
    started: Instant,
    binary_hash: String,
    status: ScoreableStatus,
    error: EvalError,
) -> EvalRunResult {
    let policy_hash = base_policy_config_hash().unwrap_or_else(|_| "unknown".into());
    EvalRunResult {
        run_id,
        logical_run_index: execution.logical_run_index,
        case_id: execution.case.case_id.clone(),
        case_version: execution.case.case_version,
        dataset_hash: execution.dataset.dataset_hash.clone(),
        fixture_hash: execution.case.fixture_hash.clone(),
        runner_commit: execution.runner_commit.clone(),
        runtime_commit: execution.runtime_commit.clone(),
        runtime_binary_hash: binary_hash,
        runtime_id: String::new(),
        request_ids: Vec::new(),
        tool_schema_hash: execution.base_tool_schema_hash.clone(),
        observed_tool_catalog_hash: "unknown".into(),
        policy_config_hash: policy_hash,
        adapter_id: "unavailable".into(),
        adapter_version: "0".into(),
        requested_model: "unknown".into(),
        resolved_model: "unknown".into(),
        system_prompt_hash: execution.dataset.system_prompt_hash.clone(),
        repetition: execution.repetition,
        termination_reason: "infra_error".into(),
        model_turns: 0,
        tool_calls: Vec::new(),
        selection_pass: false,
        argument_valid_count: 0,
        argument_total_count: 0,
        runtime_outcome_match_count: 0,
        runtime_outcome_total_count: 0,
        task_completion_pass: false,
        unnecessary_call_count: 0,
        scoreable_status: status,
        final_state_evidence: Default::default(),
        usage: None,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cleanup_status: "not_started".into(),
        provider_attempts: 0,
        manifest_hash: "unknown".into(),
        diagnostic_code: Some(error.code.into()),
    }
}

pub(crate) fn scripted_adapter_for_case(case: &EvalCaseV1) -> ScriptedAdapter {
    let mut calls = Vec::new();
    let strategy = &case.selection_oracle.strategies[0];
    let mut call_index = 0_u32;
    for step in &strategy.steps {
        if step.optional {
            continue;
        }
        let constraints = case
            .argument_constraints
            .iter()
            .filter(|constraint| constraint.tool == step.tool)
            .collect::<Vec<_>>();
        let repeat = if matches!(step.repeat, super::case::StepRepeat::OneOrMore) {
            constraints.len().max(1)
        } else {
            1
        };
        for constraint in constraints.iter().take(repeat) {
            if matches!(step.repeat, super::case::StepRepeat::OneOrMore)
                && !constraint.path_any.is_empty()
            {
                for path in &constraint.path_any {
                    call_index += 1;
                    calls.push(super::adapter::scripted_call(
                        format!("scripted-{}-{call_index}", case.case_id),
                        &step.tool,
                        json!({ "path": path }),
                    ));
                }
            } else {
                call_index += 1;
                calls.push(super::adapter::scripted_call(
                    format!("scripted-{}-{call_index}", case.case_id),
                    &step.tool,
                    scripted_arguments(&step.tool, constraint),
                ));
            }
        }
        if calls.is_empty() {
            call_index += 1;
            calls.push(super::adapter::scripted_call(
                format!("scripted-{}-{call_index}", case.case_id),
                &step.tool,
                json!({}),
            ));
        }
    }
    let final_text = case
        .final_response_assertion
        .as_ref()
        .map(|assertion| assertion.text.clone())
        .unwrap_or_else(|| "DONE".into());
    ScriptedAdapter::new(vec![
        ScriptedTurn {
            tool_calls: calls,
            final_text: None,
        },
        ScriptedTurn {
            tool_calls: Vec::new(),
            final_text: Some(final_text),
        },
    ])
}

fn scripted_arguments(tool: &str, constraint: &super::case::ArgumentConstraint) -> Value {
    match tool {
        "read_file" => {
            if let Some(path) = &constraint.path {
                json!({ "path": path })
            } else if let Some(path) = constraint.path_any.first() {
                json!({ "path": path })
            } else if let Some(suffix) = &constraint.path_suffix {
                json!({ "path": format!("../{suffix}") })
            } else {
                json!({})
            }
        }
        "glob" => {
            if let Some(pattern) = &constraint.pattern {
                json!({ "pattern": pattern })
            } else {
                json!({ "pattern": "**/*" })
            }
        }
        "write" => json!({
            "path": constraint.path,
            "content": constraint.content,
        }),
        "edit" => json!({
            "path": constraint.path,
            "oldText": constraint.old_text,
            "newText": constraint.new_text,
        }),
        _ => json!({}),
    }
}

pub(crate) fn prepare_tool_schemas_for_run(
    tools_result: &Value,
) -> EvalResult<BTreeMap<String, Value>> {
    tool_schema_map(tools_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn case() -> EvalCaseV1 {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("v4")
            .join("m04-eval")
            .join("cases")
            .join("m04-c1-01.json");
        serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap()
    }

    #[test]
    fn catalog_drift_is_rejected_instead_of_becoming_a_fuzzy_profile() {
        let exact = json!({
            "tools": [
                {"name": "read_file"},
                {"name": "glob"},
                {"name": "write"},
                {"name": "edit"}
            ]
        });
        assert!(validate_catalog(&case(), &exact).is_ok());

        let missing = json!({
            "tools": [
                {"name": "read_file"},
                {"name": "glob"},
                {"name": "write"}
            ]
        });
        let error = validate_catalog(&case(), &missing).unwrap_err();
        assert_eq!(error.code, "HARNESS_CONTRACT_ERROR");

        let extra = json!({
            "tools": [
                {"name": "read_file"},
                {"name": "glob"},
                {"name": "write"},
                {"name": "edit"},
                {"name": "grep"}
            ]
        });
        let error = validate_catalog(&case(), &extra).unwrap_err();
        assert_eq!(error.code, "HARNESS_CONTRACT_ERROR");
    }
}
