#[path = "../evaluation/mod.rs"]
mod evaluation;

use evaluation::adapter::{ensure_formal_adapter, AdapterClass, ModelAdapter};
use evaluation::agent_loop::{
    execute_case, prepare_tool_schemas_for_run, scripted_adapter_for_case, CaseExecution,
};
use evaluation::aggregate::{
    aggregate, validate_baseline, AggregateMetrics, AggregateReport, BaselineVerdict, RunIdentity,
};
use evaluation::case::EvalCategory;
use evaluation::dataset::{deterministic_case_order, EvalDataset};
use evaluation::manifest::{
    env_commit, eval_profile_hash, ADAPTER_ID, ADAPTER_VERSION, REQUESTED_MODEL, RUNNER_VERSION,
};
use evaluation::mcp_client::{
    base_policy_config_hash, exposed_tool_names, runtime_binary_hash, tool_catalog_hash,
    tool_schema_map, EvalRuntimeHarness, McpClient,
};
use evaluation::openai_responses::OpenAiResponsesAdapter;
use evaluation::output::{print_human_summary, report_json};
use evaluation::profile::EvalProfile;
use evaluation::{EvalError, EvalResult};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("m04_eval {}: {}", error.code, error.message);
        std::process::exit(1);
    }
}

async fn run() -> EvalResult<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Err(EvalError::new("INVALID_ARGUMENT", "missing command"));
    };
    let options = Options::parse(&args[1..])?;
    match command {
        "validate" => validate(&options),
        "preflight" => preflight(&options).await,
        "run" => run_suite(&options).await,
        "m05-admission" => run_admission(&options).await,
        "compare" => compare_eval(&options),
        _ => {
            print_usage();
            Err(EvalError::new(
                "INVALID_ARGUMENT",
                format!("unknown command '{command}'"),
            ))
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage:\n  m04_eval validate [--dataset PATH]\n  m04_eval preflight [--dataset PATH] [--runtime-bin PATH] [--profile PROFILE] [--json]\n  m04_eval run --adapter scripted|openai [--dataset PATH] [--runtime-bin PATH] [--profile PROFILE] [--case ID] [--repetitions N] [--json]\n  m04_eval m05-admission --adapter scripted|openai [--dataset PATH] [--runtime-bin PATH] [--json]\n  m04_eval compare --baseline PATH --candidate PATH [--json]"
    );
}

#[derive(Default)]
struct Options {
    dataset: Option<PathBuf>,
    runtime_bin: Option<PathBuf>,
    adapter: Option<String>,
    case_id: Option<String>,
    repetitions: Option<u32>,
    profile: Option<EvalProfile>,
    baseline: Option<PathBuf>,
    candidate: Option<PathBuf>,
    json: bool,
}

impl Options {
    fn parse(args: &[String]) -> EvalResult<Self> {
        let mut options = Self::default();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--dataset" => {
                    index += 1;
                    options.dataset = Some(PathBuf::from(required(args, index, "--dataset")?));
                }
                "--runtime-bin" => {
                    index += 1;
                    options.runtime_bin =
                        Some(PathBuf::from(required(args, index, "--runtime-bin")?));
                }
                "--adapter" => {
                    index += 1;
                    options.adapter = Some(required(args, index, "--adapter")?.to_owned());
                }
                "--case" => {
                    index += 1;
                    options.case_id = Some(required(args, index, "--case")?.to_owned());
                }
                "--repetitions" => {
                    index += 1;
                    options.repetitions = Some(
                        required(args, index, "--repetitions")?
                            .parse()
                            .map_err(|_| {
                                EvalError::new(
                                    "INVALID_ARGUMENT",
                                    "--repetitions must be a positive integer",
                                )
                            })?,
                    );
                }
                "--profile" => {
                    index += 1;
                    options.profile =
                        Some(EvalProfile::parse(required(args, index, "--profile")?)?);
                }
                "--baseline" => {
                    index += 1;
                    options.baseline = Some(PathBuf::from(required(args, index, "--baseline")?));
                }
                "--candidate" => {
                    index += 1;
                    options.candidate = Some(PathBuf::from(required(args, index, "--candidate")?));
                }
                "--json" => options.json = true,
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => {
                    return Err(EvalError::new(
                        "INVALID_ARGUMENT",
                        format!("unknown option '{other}'"),
                    ))
                }
            }
            index += 1;
        }
        Ok(options)
    }

    fn dataset_path(&self) -> PathBuf {
        self.dataset.clone().unwrap_or_else(default_dataset_path)
    }

    fn profile(&self) -> EvalProfile {
        self.profile.unwrap_or(EvalProfile::M04Baseline)
    }

    fn runtime_binary(&self) -> EvalResult<PathBuf> {
        if let Some(path) = &self.runtime_bin {
            if path.is_file() {
                return Ok(path.clone());
            }
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                format!("Runtime binary does not exist: {}", path.display()),
            ));
        }
        if let Some(path) = std::env::var_os("M04_EVAL_RUNTIME_BIN") {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Ok(path);
            }
        }
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for candidate in [
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
        ] {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        Err(EvalError::new(
            "INVALID_ARGUMENT",
            "Runtime binary not found; pass --runtime-bin or set M04_EVAL_RUNTIME_BIN",
        ))
    }
}

fn required<'a>(args: &'a [String], index: usize, name: &str) -> EvalResult<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| EvalError::new("INVALID_ARGUMENT", format!("{name} requires a value")))
}

fn default_dataset_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("v4")
        .join("m04-eval")
}

fn default_admission_dataset_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("v4")
        .join("m05-eval")
}

fn validate(options: &Options) -> EvalResult<()> {
    let dataset = EvalDataset::load(&options.dataset_path())?;
    println!(
        "M04_VALIDATE=PASS dataset={} version={} cases={} categories={} repetitions={}",
        dataset.dataset_file.dataset_id,
        dataset.dataset_file.dataset_version,
        dataset.cases.len(),
        dataset
            .cases
            .iter()
            .map(|case| case.category)
            .collect::<BTreeSet<_>>()
            .len(),
        dataset.dataset_file.repetitions
    );
    println!("M04_DATASET_HASH={}", dataset.dataset_hash);
    println!("M04_SCORER_HASH={}", dataset.scorer_hash);
    println!("M04_SYSTEM_PROMPT_HASH={}", dataset.system_prompt_hash);
    for case in dataset.ordered_cases() {
        println!(
            "M04_CASE={} category={} fixtureHash={}",
            case.case_id,
            case.category.as_str(),
            case.fixture_hash
        );
    }
    Ok(())
}

fn compare_eval(options: &Options) -> EvalResult<()> {
    let baseline_path = options
        .baseline
        .as_deref()
        .ok_or_else(|| EvalError::new("INVALID_ARGUMENT", "compare requires --baseline PATH"))?;
    let candidate_path = options
        .candidate
        .as_deref()
        .ok_or_else(|| EvalError::new("INVALID_ARGUMENT", "compare requires --candidate PATH"))?;
    let baseline = evaluation::m05::AcceptedM04BaselineV1::load(baseline_path)?;
    let raw = std::fs::read_to_string(candidate_path).map_err(|error| {
        EvalError::new(
            "INVALID_ARGUMENT",
            format!("could not read candidate result: {error}"),
        )
    })?;
    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("candidate result is invalid JSON: {error}"),
        )
    })?;
    let metrics_value = value
        .get("report")
        .and_then(|value| value.get("metrics"))
        .or_else(|| value.get("metrics"))
        .unwrap_or(&value)
        .clone();
    let candidate: AggregateMetrics = serde_json::from_value(metrics_value).map_err(|error| {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("candidate aggregate metrics are invalid: {error}"),
        )
    })?;
    let comparison = baseline.compare_candidate(&candidate);
    if options.json {
        println!(
            "{}",
            json!({
                "profileId": evaluation::profile::M05_CANDIDATE_PROFILE_ID,
                "baselineId": baseline.baseline_id,
                "comparison": comparison,
            })
        );
    } else {
        println!(
            "M05_COMPARE={} baseline={}",
            if comparison.pass { "PASS" } else { "FAIL" },
            baseline.baseline_id
        );
        for failure in &comparison.failures {
            println!("M05_COMPARE_FAILURE={failure}");
        }
    }
    if comparison.pass {
        Ok(())
    } else {
        Err(EvalError::new(
            "REGRESSION_FAIL",
            "candidate did not pass the frozen M04 compare_m05 gate",
        ))
    }
}

async fn run_admission(options: &Options) -> EvalResult<()> {
    let adapter_name = options.adapter.as_deref().ok_or_else(|| {
        EvalError::new(
            "INVALID_ARGUMENT",
            "m05-admission requires --adapter scripted|openai",
        )
    })?;
    let formal = adapter_name == "openai";
    if formal {
        if std::env::var("M05_EVAL_ALLOW_NETWORK").ok().as_deref() != Some("1") {
            return Err(EvalError::new(
                "BLOCKED",
                "real M05 supplemental execution requires explicit M05_EVAL_ALLOW_NETWORK=1 authorization",
            ));
        }
        if options.case_id.is_some() || options.repetitions.is_some() {
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                "supplemental admission set rejects case/repetition overrides",
            ));
        }
    } else if adapter_name != "scripted" {
        return Err(EvalError::new(
            "INVALID_ARGUMENT",
            format!("unknown adapter '{adapter_name}'"),
        ));
    }
    let dataset_path = options
        .dataset
        .clone()
        .unwrap_or_else(default_admission_dataset_path);
    let dataset = evaluation::m05::AdmissionDataset::load(&dataset_path)?;
    let binary = options.runtime_binary()?;
    let identity = evaluation::m05::admission_identity(&dataset, &binary).await?;
    let mut records = Vec::new();
    let mut runs = Vec::new();
    let mut logical_index = 0;
    for repetition in 1..=evaluation::m05::ADMISSION_REPETITIONS {
        for case in &dataset.cases {
            logical_index += 1;
            let adapter: Box<dyn ModelAdapter> = if formal {
                Box::new(
                    OpenAiResponsesAdapter::from_env()
                        .map_err(|error| EvalError::new("BLOCKED", error.message))?,
                )
            } else {
                Box::new(evaluation::m05::scripted_admission_adapter(case))
            };
            if formal {
                ensure_formal_adapter(adapter.as_ref())?;
            }
            let run = evaluation::m05::execute_admission_case(
                &dataset,
                &identity,
                &binary,
                case,
                repetition,
                logical_index,
                adapter,
            )
            .await?;
            let record = evaluation::m05::AdmissionRunRecord {
                case_id: case.case_id.clone(),
                repetition,
                scoreable: run.scoreable_status.is_scoreable(),
                security_violation: run.scoreable_status
                    == evaluation::aggregate::ScoreableStatus::SecurityViolation,
            };
            eprintln!(
                "M05_ADMISSION_RUN case={} repetition={} status={:?} tools={:?}",
                case.case_id,
                repetition,
                run.scoreable_status,
                run.tool_calls
                    .iter()
                    .map(|call| call.tool_name.as_str())
                    .collect::<Vec<_>>()
            );
            records.push(record);
            runs.push(run);
        }
    }
    let validity = evaluation::m05::validate_admission_runs(&dataset.cases, &records);
    if options.json {
        println!(
            "{}",
            json!({
                "setId": evaluation::m05::ADMISSION_SET_ID,
                "datasetHash": dataset.dataset_hash,
                "adapterClass": if formal { "formal" } else { "NON_FORMAL" },
                "formal": formal,
                "validity": validity,
                "runs": runs,
            })
        );
    } else {
        println!(
            "M05_ADMISSION={} set={} runs={} scoreable={} security={}",
            match validity.verdict {
                Some(BaselineVerdict::Valid) => "VALID",
                Some(BaselineVerdict::Fail) => "FAIL",
                _ => "INCONCLUSIVE",
            },
            evaluation::m05::ADMISSION_SET_ID,
            records.len(),
            records.iter().filter(|run| run.scoreable).count(),
            records.iter().filter(|run| run.security_violation).count()
        );
        for reason in &validity.reasons {
            println!("M05_ADMISSION_REASON={reason}");
        }
    }
    if validity.verdict == Some(BaselineVerdict::Valid) {
        Ok(())
    } else {
        Err(EvalError::new(
            "ADMISSION_INVALID",
            "M05 supplemental set did not meet validity requirements",
        ))
    }
}

async fn preflight(options: &Options) -> EvalResult<()> {
    let dataset = EvalDataset::load(&options.dataset_path())?;
    let profile = options.profile();
    let binary = options.runtime_binary()?;
    let binary_hash = runtime_binary_hash(&binary)?;
    let case = dataset
        .cases
        .iter()
        .find(|case| case.category == EvalCategory::ExactRead)
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "no exact_read probe case"))?;
    let fixture = dataset.fixture_path(case)?;
    let harness = EvalRuntimeHarness::start_with_profile(&binary, case, &fixture, profile).await?;
    let mut mcp = McpClient::connect(&harness.endpoint_url).await?;
    let tools = mcp.list_tools().await?;
    let names = exposed_tool_names(&tools)?;
    let expected = profile
        .tool_names()
        .iter()
        .map(|tool| (*tool).to_owned())
        .collect::<Vec<_>>();
    if names != expected {
        let _ = harness.shutdown().await;
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("preflight catalog drifted: {names:?}"),
        ));
    }
    let schema_map = tool_schema_map(&tools)?;
    let schema_hash = tool_catalog_hash(&tools)?;
    let schema_hashes = schema_map
        .iter()
        .map(|(name, schema)| Ok((name.clone(), evaluation::manifest::canonical_hash(schema)?)))
        .collect::<EvalResult<BTreeMap<_, _>>>()?;
    let probe_path = case
        .argument_constraints
        .iter()
        .find(|constraint| constraint.tool == "read_file")
        .and_then(|constraint| constraint.path.clone())
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "probe case has no read path"))?;
    let result = mcp
        .call_tool("read_file", json!({ "path": probe_path }))
        .await?;
    let key = evaluation::correlation::extract(&result).ok_or_else(|| {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "preflight correlation metadata missing",
        )
    })?;
    if key.runtime_id != harness.runtime_id {
        let _ = harness.shutdown().await;
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "preflight runtime_id mismatch",
        ));
    }
    let evidence = evaluation::correlation::verify_evidence(
        &harness.evidence_database,
        &key,
        Duration::from_secs(5),
    )
    .await?;
    let policy_hash = base_policy_config_hash()?;
    let eval_profile = eval_profile_hash(
        &dataset.system_prompt_hash,
        ADAPTER_ID,
        ADAPTER_VERSION,
        REQUESTED_MODEL,
        profile,
    )?;
    let shutdown = harness.shutdown().await?;
    let report = json!({
        "command": "preflight",
        "providerRequestMade": false,
        "requestedModel": REQUESTED_MODEL,
        "adapterId": ADAPTER_ID,
        "adapterVersion": ADAPTER_VERSION,
        "runnerVersion": RUNNER_VERSION,
        "profileId": profile.id(),
        "datasetHash": dataset.dataset_hash,
        "scorerHash": dataset.scorer_hash,
        "systemPromptHash": dataset.system_prompt_hash,
        "evalProfileHash": eval_profile,
        "toolSchemaHash": schema_hash,
        "toolSchemaHashes": schema_hashes,
        "toolNames": names,
        "policyConfigHash": policy_hash,
        "runtimeBinaryHash": binary_hash,
        "runtimeId": key.runtime_id,
        "probeRequestId": key.request_id,
        "evidenceTerminalState": format!("{:?}", evidence.summary.terminal_state),
        "cases": dataset.cases.len(),
        "repetitions": dataset.dataset_file.repetitions,
        "logicalRuns": dataset.cases.len() * dataset.dataset_file.repetitions as usize,
        "maxModelTurns": evaluation::MAX_MODEL_TURNS,
        "maxToolCalls": evaluation::MAX_TOOL_CALLS,
        "maxOutputTokensPerTurn": evaluation::MAX_OUTPUT_TOKENS_PER_TURN,
        "runTimeoutSeconds": evaluation::RUN_TIMEOUT.as_secs(),
        "runSpendCapUsd": evaluation::RUN_SPEND_CAP_USD,
        "suiteSpendCapUsd": evaluation::SUITE_SPEND_CAP_USD,
        "cleanup": shutdown,
    });
    if options.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!("M04_PREFLIGHT=PASS providerRequestMade=false");
        println!(
            "model={} dataset={} tools={} cases={} repetitions={} logicalRuns={}",
            REQUESTED_MODEL,
            report["datasetHash"],
            report["toolSchemaHash"],
            report["cases"],
            report["repetitions"],
            report["logicalRuns"]
        );
        println!(
            "runtimeBinaryHash={} policyConfigHash={} cleanup={}",
            binary_hash, policy_hash, report["cleanup"]
        );
        println!(
            "probe runtime_id={} request_id={} evidence={}",
            key.runtime_id, key.request_id, report["evidenceTerminalState"]
        );
    }
    Ok(())
}

async fn run_suite(options: &Options) -> EvalResult<()> {
    let profile = options.profile();
    if profile.is_m09() {
        return Err(EvalError::new(
            "BLOCKED",
            "M09 requires its admission evidence boundary; legacy M04 run cannot authorize an M09 profile. Use m09_eval for offline validation/preflight.",
        ));
    }
    let adapter = options.adapter.as_deref().ok_or_else(|| {
        EvalError::new("INVALID_ARGUMENT", "run requires --adapter scripted|openai")
    })?;
    let dataset = EvalDataset::load(&options.dataset_path())?;
    let binary = options.runtime_binary()?;
    let formal = adapter == "openai";
    if formal {
        if std::env::var("M04_EVAL_ALLOW_NETWORK").ok().as_deref() != Some("1") {
            return Err(EvalError::new(
                "BLOCKED",
                "real provider execution requires explicit M04_EVAL_ALLOW_NETWORK=1 authorization",
            ));
        }
        if options.case_id.is_some() || options.repetitions.is_some() {
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                "formal baseline rejects case subsets and repetition overrides",
            ));
        }
        if dataset.dataset_file.repetitions != evaluation::REPETITIONS {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "formal baseline requires frozen repetitions",
            ));
        }
    } else if adapter != "scripted" {
        return Err(EvalError::new(
            "INVALID_ARGUMENT",
            format!("unknown adapter '{adapter}'"),
        ));
    }
    let preflight = preflight_identity(&dataset, &binary, profile).await?;
    let cases = if let Some(case_id) = &options.case_id {
        vec![dataset.case(case_id).ok_or_else(|| {
            EvalError::new("INVALID_ARGUMENT", format!("unknown case '{case_id}'"))
        })?]
    } else {
        deterministic_case_order(&dataset.cases)
    };
    let repetitions = if formal {
        evaluation::REPETITIONS
    } else {
        options
            .repetitions
            .unwrap_or(1)
            .clamp(1, evaluation::REPETITIONS)
    };
    let runner_commit = env_commit("M04_EVAL_RUNNER_COMMIT");
    let runtime_commit = env_commit("M04_EVAL_RUNTIME_COMMIT");
    let mut runs = Vec::new();
    let mut logical_index = 0;
    let mut suite_cost = 0.0_f64;
    'suite: for repetition in 1..=repetitions {
        for case in &cases {
            logical_index += 1;
            let adapter: Box<dyn ModelAdapter> = if formal {
                Box::new(
                    OpenAiResponsesAdapter::from_env()
                        .map_err(|error| EvalError::new("BLOCKED", error.message))?,
                )
            } else {
                Box::new(scripted_adapter_for_case(case))
            };
            if formal {
                ensure_formal_adapter(adapter.as_ref())?;
            } else if adapter.class() != AdapterClass::NonFormal {
                return Err(EvalError::new(
                    "INVALID_ARGUMENT",
                    "scripted mode requires a non-formal adapter",
                ));
            }
            let result = execute_case(
                CaseExecution {
                    dataset: &dataset,
                    case,
                    runtime_binary: &binary,
                    base_tool_schema_hash: preflight.tool_schema_hash.clone(),
                    base_tool_schemas: preflight.schema_map.clone(),
                    runner_commit: runner_commit.clone(),
                    runtime_commit: runtime_commit.clone(),
                    logical_run_index: logical_index,
                    repetition,
                    cleanup_failure_injection: false,
                    profile,
                },
                adapter,
            )
            .await;
            eprintln!(
                "M04_RUN case={} repetition={} status={:?} selection={} task={} termination={}",
                result.case_id,
                result.repetition,
                result.scoreable_status,
                result.selection_pass,
                result.task_completion_pass,
                result.termination_reason
            );
            if let Some(cost) = result.usage.as_ref().and_then(|usage| usage.cost_usd) {
                suite_cost += cost;
                if suite_cost > evaluation::SUITE_SPEND_CAP_USD {
                    runs.push(result);
                    eprintln!(
                        "M04_SUITE_SPEND_LIMIT costUsd={suite_cost} capUsd={}",
                        evaluation::SUITE_SPEND_CAP_USD
                    );
                    break 'suite;
                }
            }
            runs.push(result);
        }
    }
    let categories = dataset
        .cases
        .iter()
        .map(|case| (case.case_id.clone(), case.category))
        .collect::<BTreeMap<_, _>>();
    let metrics = aggregate(&runs, &categories);
    let identities = runs
        .iter()
        .filter(|run| run.scoreable_status.is_scoreable())
        .map(|run| RunIdentity {
            dataset_hash: run.dataset_hash.clone(),
            scorer_hash: dataset.scorer_hash.clone(),
            system_prompt_hash: run.system_prompt_hash.clone(),
            eval_profile_hash: eval_profile_hash(
                &run.system_prompt_hash,
                &run.adapter_id,
                &run.adapter_version,
                &run.requested_model,
                profile,
            )
            .unwrap_or_else(|_| "unknown".into()),
            tool_schema_hash: run.tool_schema_hash.clone(),
            requested_model: run.requested_model.clone(),
            resolved_model: run.resolved_model.clone(),
            adapter_id: run.adapter_id.clone(),
            adapter_version: run.adapter_version.clone(),
            runtime_commit: run.runtime_commit.clone(),
            runtime_binary_hash: run.runtime_binary_hash.clone(),
        })
        .collect::<BTreeSet<_>>();
    let validity = validate_baseline(&runs, &metrics, &identities, formal);
    let report = AggregateReport {
        adapter_class: if formal {
            "formal".into()
        } else {
            "NON_FORMAL".into()
        },
        formal_baseline: formal,
        metrics,
        validity,
    };
    if options.json {
        println!(
            "{}",
            json!({
                "profileId": profile.id(),
                "report": serde_json::from_str::<Value>(&report_json(&report).unwrap()).unwrap(),
                "runs": runs,
            })
        );
    } else {
        print_human_summary(&report, std::io::stderr())
            .map_err(|error| EvalError::new("INTERNAL", error.to_string()))?;
        println!(
            "M04_RESULT={} formal={} runs={}",
            if report.formal_baseline {
                "FORMAL"
            } else {
                "NON_FORMAL"
            },
            report.formal_baseline,
            runs.len()
        );
        println!(
            "M04_VALIDITY={:?}",
            report
                .validity
                .verdict
                .unwrap_or(BaselineVerdict::Inconclusive)
        );
    }
    Ok(())
}

struct PreflightIdentity {
    tool_schema_hash: String,
    schema_map: BTreeMap<String, Value>,
}

#[cfg(test)]
mod m09_boundary_tests {
    use super::*;

    #[tokio::test]
    async fn legacy_runner_rejects_m09_before_adapter_dataset_or_provider_access() {
        for id in [
            "m09-five-tool-control-v1",
            "m09-mkdir-candidate-v1",
            "m09-list-directory-candidate-v1",
            "m09-copy-candidate-v1",
            "m09-move-candidate-v1",
            "m09-rename-candidate-v1",
            "m09-delete-candidate-v1",
            "m09-final-candidate-v1",
        ] {
            let options = Options {
                profile: Some(EvalProfile::parse(id).unwrap()),
                ..Default::default()
            };
            assert_eq!(run_suite(&options).await.unwrap_err().code, "BLOCKED");
        }
    }
}

async fn preflight_identity(
    dataset: &EvalDataset,
    binary: &std::path::Path,
    profile: EvalProfile,
) -> EvalResult<PreflightIdentity> {
    let case = dataset
        .cases
        .iter()
        .find(|case| case.category == EvalCategory::ExactRead)
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "no preflight case"))?;
    let harness =
        EvalRuntimeHarness::start_with_profile(binary, case, &dataset.fixture_path(case)?, profile)
            .await?;
    let mut mcp = McpClient::connect(&harness.endpoint_url).await?;
    let tools = mcp.list_tools().await?;
    profile.validate_catalog(&tools)?;
    let schema_map = prepare_tool_schemas_for_run(&tools)?;
    let hash = tool_catalog_hash(&tools)?;
    harness.shutdown().await?;
    Ok(PreflightIdentity {
        tool_schema_hash: hash,
        schema_map,
    })
}
