//! Offline M09 admission preflight. No command in this binary contacts a Model.
#[path = "../evaluation/mod.rs"]
#[allow(dead_code)]
mod evaluation;

use evaluation::dataset::EvalDataset;
use evaluation::m09::{structural_matrix, AdmissionDataset};
use evaluation::m09_fixture::prepare_fixture;
use evaluation::m09_handoff::{AcceptedM05Handoff, ExpectedM05Identity};
use evaluation::m09_runner::validate_admission_dataset;
use evaluation::manifest::{eval_profile_hash, sha256_file, sha256_hex};
use evaluation::mcp_client::{tool_catalog_hash, EvalRuntimeHarness, McpClient};
use evaluation::profile::EvalProfile;
use evaluation::{EvalError, EvalResult};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        println!(
            "{}",
            json!({"status":error.code,"reason":error.message,"modelCallsExecuted":false,"finalDecisionSetProduced":false})
        );
        std::process::exit(1);
    }
}

async fn run() -> EvalResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let command = args.first().map(String::as_str).unwrap_or("help");
    if command == "help" || command == "--help" {
        println!("m09_eval validate [--dataset DIR]\nm09_eval prepare-fixtures [--dataset DIR]\nm09_eval structural\nm09_eval preflight --accepted-m05 FILE --runtime-bin FILE [--dataset DIR]\nAll commands are offline. Preflight does not authorize or execute real admission.");
        return Ok(());
    }
    let mut dataset = root.join("fixtures/v4/m09-admission");
    let mut handoff = None;
    let mut runtime = None;
    let mut index = 1;
    while index < args.len() {
        let flag = &args[index];
        if flag == "--json" {
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| EvalError::new("INVALID_ARGUMENT", "option requires a value"))?;
        match flag.as_str() {
            "--dataset" => dataset = PathBuf::from(value),
            "--accepted-m05" => handoff = Some(PathBuf::from(value)),
            "--runtime-bin" => runtime = Some(PathBuf::from(value)),
            _ => return Err(EvalError::new("INVALID_ARGUMENT", "unknown M09 option")),
        }
        index += 2;
    }
    let output = match command {
        "validate" => validate(&dataset)?,
        "prepare-fixtures" => prepare_fixtures(&dataset)?,
        "structural" => {
            json!({"status":"MECHANICS_ONLY","matrix":structural_matrix()?,"finalDecisionSetProduced":false})
        }
        "preflight" => {
            let handoff = handoff.ok_or_else(|| {
                EvalError::new("BLOCKED", "accepted M05 quality handoff is required")
            })?;
            let runtime = runtime.ok_or_else(|| {
                EvalError::new("BLOCKED", "exact candidate Runtime binary is required")
            })?;
            preflight(&root, &dataset, &handoff, &runtime).await?
        }
        _ => return Err(EvalError::new("INVALID_ARGUMENT", "unknown M09 command")),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|_| EvalError::new("INFRA_ERROR", "could not serialize M09 report"))?
    );
    if output.get("status").and_then(Value::as_str) == Some("BLOCKED") {
        std::process::exit(2);
    }
    Ok(())
}

fn prepare_fixtures(root: &Path) -> EvalResult<Value> {
    let dataset = AdmissionDataset::load(root)?;
    let mut results = Vec::new();
    let mut blocked = false;
    for case in &dataset.cases {
        match prepare_fixture(&dataset.fixture_path(case)?, &case.fixture_setup) {
            Ok(prepared) => {
                results.push(json!({"caseId":case.case_id,"status":"PREPARED","recipeBoundByFixtureHash":true}));
                drop(prepared);
            }
            Err(error) if error.code == "BLOCKED" => {
                blocked = true;
                results
                    .push(json!({"caseId":case.case_id,"status":"BLOCKED","reason":error.message}));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(
        json!({"status":if blocked {"BLOCKED"} else {"FIXTURE_MECHANICS_ONLY"},
        "cases":results,"modelCallsExecuted":false,"toolCallsExecuted":false,
        "finalDecisionSetProduced":false}),
    )
}

fn validate(root: &Path) -> EvalResult<Value> {
    let summary = validate_admission_dataset(root)?;
    Ok(json!({
        "status":"MECHANICS_ONLY", "datasetId":summary.dataset_id,
        "datasetVersion":summary.dataset_version, "caseCount":summary.case_count,
        "repetitions":summary.repetitions, "logicalRuns":summary.logical_runs,
        "datasetHash":summary.dataset_hash, "systemPromptHash":summary.system_prompt_hash,
        "candidates":summary.candidates.iter().map(|c| json!({
            "candidate":c.candidate,"caseCount":c.case_count,"positiveCount":c.positive_count,
            "negativeCount":c.negative_count,"profileId":c.profile_id,"candidateSchemaHash":c.candidate_schema_hash
        })).collect::<Vec<_>>(),
        "modelCallsExecuted":false, "formalAdmissionEligible":false, "finalDecisionSetProduced":false
    }))
}

async fn preflight(
    root: &Path,
    admission_root: &Path,
    path: &Path,
    binary: &Path,
) -> EvalResult<Value> {
    // Reject absent/malformed handoff before starting even the disposable Runtime.
    let raw = std::fs::read(path)
        .map_err(|_| EvalError::new("BLOCKED", "accepted M05 handoff unavailable"))?;
    let claimed: AcceptedM05Handoff = serde_json::from_slice(&raw)
        .map_err(|_| EvalError::new("BLOCKED", "accepted M05 handoff malformed"))?;
    let admission = validate(admission_root)?;
    let baseline = EvalDataset::load(&root.join("fixtures/v4/m04-eval"))?;
    let case = baseline
        .cases
        .first()
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "missing M04 baseline case"))?;
    let fixture = baseline.fixture_path(case)?;
    let harness = EvalRuntimeHarness::start_with_profile(
        binary,
        case,
        &fixture,
        EvalProfile::M09FiveToolControl,
    )
    .await?;
    let result = async {
        let mut client = McpClient::connect(&harness.endpoint_url).await?;
        let catalog = client.list_tools().await?;
        EvalProfile::M09FiveToolControl.validate_catalog(&catalog)?;
        let schema_hash = tool_catalog_hash(&catalog)?;
        let profile_hash = eval_profile_hash(
            &baseline.system_prompt_hash,
            &claimed.adapter,
            &claimed.adapter_version,
            &claimed.requested_model,
            EvalProfile::M05GrepCandidate,
        )?;
        let ids: BTreeSet<_> = baseline.cases.iter().map(|c| c.case_id.clone()).collect();
        let expected = ExpectedM05Identity {
            dataset_hash: &baseline.dataset_hash,
            scorer_hash: &baseline.scorer_hash,
            system_prompt_hash: &baseline.system_prompt_hash,
            profile_hash: &profile_hash,
            five_tool_schema_hash: &schema_hash,
            case_ids: &ids,
        };
        claimed.validate(&expected)?;
        let accepted = claimed;
        Ok(
            json!({"status":"PREFLIGHT_ONLY", "admissionDataset":admission,
            "acceptedM05HandoffHash":sha256_hex(&raw),"runtimeHash":sha256_file(binary)?,
            "fiveToolSchemaHash":schema_hash,"adapter":accepted.adapter,
            "requestedModel":accepted.requested_model,"resolvedModel":accepted.resolved_model,
            "realAdmission":"BLOCKED_NOT_EXECUTED", "final72Regression":"NOT_EXECUTED",
            "modelCallsExecuted":false,"finalDecisionSetProduced":false,
            "productionCandidatesExposed":false}),
        )
    }
    .await;
    let shutdown = harness.shutdown().await;
    shutdown?;
    result
}
