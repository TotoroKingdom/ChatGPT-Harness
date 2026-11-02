#![allow(dead_code)]

use super::adapter::ProviderUsage;
use super::case::EvalCategory;
use super::scoring::{FinalStateEvidence, ToolCallRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScoreableStatus {
    Scoreable,
    InfraError,
    LimitExceeded,
    SecurityViolation,
    HarnessContractError,
}

impl ScoreableStatus {
    pub(crate) fn is_scoreable(self) -> bool {
        matches!(self, Self::Scoreable | Self::LimitExceeded)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvalRunResult {
    pub(crate) run_id: String,
    pub(crate) logical_run_index: usize,
    pub(crate) case_id: String,
    pub(crate) case_version: u32,
    pub(crate) dataset_hash: String,
    pub(crate) fixture_hash: String,
    pub(crate) runner_commit: String,
    pub(crate) runtime_commit: String,
    pub(crate) runtime_binary_hash: String,
    pub(crate) runtime_id: String,
    pub(crate) request_ids: Vec<String>,
    pub(crate) tool_schema_hash: String,
    pub(crate) observed_tool_catalog_hash: String,
    pub(crate) policy_config_hash: String,
    pub(crate) adapter_id: String,
    pub(crate) adapter_version: String,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: String,
    pub(crate) system_prompt_hash: String,
    pub(crate) repetition: u32,
    pub(crate) termination_reason: String,
    pub(crate) model_turns: u32,
    pub(crate) tool_calls: Vec<ToolCallRecord>,
    pub(crate) selection_pass: bool,
    pub(crate) argument_valid_count: u32,
    pub(crate) argument_total_count: u32,
    pub(crate) runtime_outcome_match_count: u32,
    pub(crate) runtime_outcome_total_count: u32,
    pub(crate) task_completion_pass: bool,
    pub(crate) unnecessary_call_count: u32,
    pub(crate) scoreable_status: ScoreableStatus,
    pub(crate) final_state_evidence: FinalStateEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) usage: Option<ProviderUsage>,
    pub(crate) elapsed_ms: u64,
    pub(crate) cleanup_status: String,
    pub(crate) provider_attempts: u32,
    pub(crate) manifest_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_code: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Metric {
    pub(crate) numerator: u64,
    pub(crate) denominator: u64,
}

impl Metric {
    pub(crate) fn rate(&self) -> f64 {
        if self.denominator == 0 {
            0.0
        } else {
            self.numerator as f64 / self.denominator as f64
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CategoryMetrics {
    pub(crate) category: String,
    pub(crate) total_runs: u64,
    pub(crate) scoreable_runs: u64,
    pub(crate) selection: Metric,
    pub(crate) argument_validity: Metric,
    pub(crate) runtime_outcome_match: Metric,
    pub(crate) task_completion: Metric,
    pub(crate) unnecessary_calls: Metric,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaseMetrics {
    pub(crate) case_id: String,
    pub(crate) category: String,
    pub(crate) total_runs: u64,
    pub(crate) scoreable_runs: u64,
    pub(crate) selection_passes: u64,
    pub(crate) task_completion_passes: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AggregateMetrics {
    pub(crate) total_runs: u64,
    pub(crate) scoreable_runs: u64,
    pub(crate) infra_error_runs: u64,
    pub(crate) limit_exceeded_runs: u64,
    pub(crate) security_violation_runs: u64,
    pub(crate) harness_contract_error_runs: u64,
    pub(crate) selection: Metric,
    pub(crate) argument_validity: Metric,
    pub(crate) runtime_outcome_match: Metric,
    pub(crate) task_completion: Metric,
    pub(crate) unnecessary_calls: Metric,
    pub(crate) categories: Vec<CategoryMetrics>,
    pub(crate) cases: Vec<CaseMetrics>,
    pub(crate) termination_reasons: BTreeMap<String, u64>,
    pub(crate) mean_tool_calls: f64,
    pub(crate) median_tool_calls: f64,
    pub(crate) evidence_covered_runs: u64,
    pub(crate) usage: Option<ProviderUsage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AggregateReport {
    pub(crate) adapter_class: String,
    pub(crate) formal_baseline: bool,
    pub(crate) metrics: AggregateMetrics,
    pub(crate) validity: BaselineValidity,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum BaselineVerdict {
    Valid,
    Inconclusive,
    Fail,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BaselineValidity {
    pub(crate) verdict: Option<BaselineVerdict>,
    pub(crate) reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunIdentity {
    pub(crate) dataset_hash: String,
    pub(crate) scorer_hash: String,
    pub(crate) system_prompt_hash: String,
    pub(crate) eval_profile_hash: String,
    pub(crate) tool_schema_hash: String,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: String,
    pub(crate) adapter_id: String,
    pub(crate) adapter_version: String,
    pub(crate) runtime_commit: String,
    pub(crate) runtime_binary_hash: String,
}

pub(crate) fn aggregate(
    runs: &[EvalRunResult],
    categories: &BTreeMap<String, EvalCategory>,
) -> AggregateMetrics {
    let mut metrics = AggregateMetrics {
        total_runs: runs.len() as u64,
        ..AggregateMetrics::default()
    };
    let mut category_values = BTreeMap::<String, CategoryMetrics>::new();
    let mut case_values = BTreeMap::<String, CaseMetrics>::new();
    let mut tool_call_counts = Vec::new();
    let mut usage = ProviderUsage::default();

    for run in runs {
        match run.scoreable_status {
            ScoreableStatus::Scoreable => metrics.scoreable_runs += 1,
            ScoreableStatus::InfraError => metrics.infra_error_runs += 1,
            ScoreableStatus::LimitExceeded => {
                metrics.scoreable_runs += 1;
                metrics.limit_exceeded_runs += 1;
            }
            ScoreableStatus::SecurityViolation => metrics.security_violation_runs += 1,
            ScoreableStatus::HarnessContractError => metrics.harness_contract_error_runs += 1,
        }
        *metrics
            .termination_reasons
            .entry(run.termination_reason.clone())
            .or_default() += 1;
        let forwarded = run.tool_calls.iter().filter(|call| call.forwarded).count();
        if forwarded > 0
            && run
                .tool_calls
                .iter()
                .filter(|call| call.forwarded)
                .all(|call| call.evidence_verified)
        {
            metrics.evidence_covered_runs += 1;
        }
        if run.scoreable_status.is_scoreable() {
            tool_call_counts.push(run.tool_calls.len() as u64);
        }
        if let Some(run_usage) = &run.usage {
            add_usage(&mut usage, run_usage);
            metrics.usage = Some(usage.clone());
        }
        let category = categories
            .get(&run.case_id)
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| "unknown".into());
        let category_entry =
            category_values
                .entry(category.clone())
                .or_insert_with(|| CategoryMetrics {
                    category: category.clone(),
                    ..CategoryMetrics::default()
                });
        category_entry.total_runs += 1;
        let case_entry = case_values
            .entry(run.case_id.clone())
            .or_insert_with(|| CaseMetrics {
                case_id: run.case_id.clone(),
                category: category.clone(),
                ..CaseMetrics::default()
            });
        case_entry.total_runs += 1;
        if !run.scoreable_status.is_scoreable() {
            continue;
        }
        category_entry.scoreable_runs += 1;
        case_entry.scoreable_runs += 1;
        metrics.selection.denominator += 1;
        metrics.task_completion.denominator += 1;
        category_entry.selection.denominator += 1;
        category_entry.task_completion.denominator += 1;
        if run.selection_pass {
            metrics.selection.numerator += 1;
            category_entry.selection.numerator += 1;
            case_entry.selection_passes += 1;
        }
        if run.task_completion_pass {
            metrics.task_completion.numerator += 1;
            category_entry.task_completion.numerator += 1;
            case_entry.task_completion_passes += 1;
        }
        metrics.argument_validity.numerator += run.argument_valid_count as u64;
        metrics.argument_validity.denominator += run.argument_total_count as u64;
        category_entry.argument_validity.numerator += run.argument_valid_count as u64;
        category_entry.argument_validity.denominator += run.argument_total_count as u64;
        metrics.runtime_outcome_match.numerator += run.runtime_outcome_match_count as u64;
        metrics.runtime_outcome_match.denominator += run.runtime_outcome_total_count as u64;
        category_entry.runtime_outcome_match.numerator += run.runtime_outcome_match_count as u64;
        category_entry.runtime_outcome_match.denominator += run.runtime_outcome_total_count as u64;
        metrics.unnecessary_calls.numerator += run.unnecessary_call_count as u64;
        metrics.unnecessary_calls.denominator += run.tool_calls.len() as u64;
        category_entry.unnecessary_calls.numerator += run.unnecessary_call_count as u64;
        category_entry.unnecessary_calls.denominator += run.tool_calls.len() as u64;
    }
    metrics.categories = category_values.into_values().collect();
    metrics.cases = case_values.into_values().collect();
    tool_call_counts.sort_unstable();
    metrics.mean_tool_calls = if tool_call_counts.is_empty() {
        0.0
    } else {
        tool_call_counts.iter().sum::<u64>() as f64 / tool_call_counts.len() as f64
    };
    metrics.median_tool_calls = if tool_call_counts.is_empty() {
        0.0
    } else {
        let middle = tool_call_counts.len() / 2;
        if tool_call_counts.len() % 2 == 0 {
            (tool_call_counts[middle - 1] + tool_call_counts[middle]) as f64 / 2.0
        } else {
            tool_call_counts[middle] as f64
        }
    };
    if metrics.usage.is_none() {
        metrics.usage = None;
    }
    metrics
}

fn add_usage(total: &mut ProviderUsage, value: &ProviderUsage) {
    total.input_tokens = add_optional(total.input_tokens, value.input_tokens);
    total.output_tokens = add_optional(total.output_tokens, value.output_tokens);
    total.total_tokens = add_optional(total.total_tokens, value.total_tokens);
    total.cost_usd = add_optional_f64(total.cost_usd, value.cost_usd);
}

fn add_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn add_optional_f64(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

pub(crate) fn validate_baseline(
    runs: &[EvalRunResult],
    metrics: &AggregateMetrics,
    identities: &std::collections::BTreeSet<RunIdentity>,
    formal_baseline: bool,
) -> BaselineValidity {
    let mut validity = BaselineValidity::default();
    if !formal_baseline {
        validity.verdict = Some(BaselineVerdict::Inconclusive);
        validity
            .reasons
            .push("scripted/mechanics results cannot satisfy formal baseline validity".into());
        return validity;
    }
    if metrics.total_runs != super::LOGICAL_RUNS as u64 {
        validity.reasons.push("logical runs must equal 72".into());
    }
    if metrics.scoreable_runs < 70 {
        validity.reasons.push("scoreable runs must be >= 70".into());
    }
    for case in &metrics.cases {
        if case.scoreable_runs < 2 {
            validity.reasons.push(format!(
                "case {} has fewer than 2/3 scoreable runs",
                case.case_id
            ));
        }
    }
    for category in &metrics.categories {
        if category.scoreable_runs < 11 {
            validity.reasons.push(format!(
                "category {} has fewer than 11/12 scoreable runs",
                category.category
            ));
        }
    }
    if identities.len() != 1 {
        validity.reasons.push(format!(
            "identity drift across {} run identities",
            identities.len()
        ));
    }
    if metrics.security_violation_runs != 0 || runs.iter().any(is_security_violation) {
        validity
            .reasons
            .push("security violations must be zero".into());
    }
    validity.verdict = Some(if validity.reasons.is_empty() {
        BaselineVerdict::Valid
    } else if metrics.security_violation_runs != 0 || runs.iter().any(is_security_violation) {
        BaselineVerdict::Fail
    } else {
        BaselineVerdict::Inconclusive
    });
    validity
}

fn is_security_violation(run: &EvalRunResult) -> bool {
    run.scoreable_status == ScoreableStatus::SecurityViolation
}

pub(crate) const M05_MAX_TASK_REGRESSION: f64 = 0.02;
pub(crate) const M05_MAX_SELECTION_REGRESSION: f64 = 0.03;
pub(crate) const M05_MAX_ARGUMENT_REGRESSION: f64 = 0.01;
pub(crate) const M05_MAX_OUTCOME_REGRESSION: f64 = 0.01;
pub(crate) const M05_MAX_UNNECESSARY_REGRESSION: f64 = 0.05;
pub(crate) const M05_CATEGORY_TASK_GUARD: f64 = 0.08;
pub(crate) const M05_CATEGORY_SELECTION_GUARD: f64 = 0.10;
pub(crate) const M05_DRIFT_TASK: f64 = 0.05;
pub(crate) const M05_DRIFT_SELECTION: f64 = 0.05;
pub(crate) const M05_DRIFT_ARGUMENT: f64 = 0.02;
pub(crate) const M05_DRIFT_OUTCOME: f64 = 0.02;
pub(crate) const M05_DRIFT_UNNECESSARY: f64 = 0.05;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegressionComparison {
    pub(crate) pass: bool,
    pub(crate) inconclusive: bool,
    pub(crate) failures: Vec<String>,
}

pub(crate) fn compare_m05(
    baseline: &AggregateMetrics,
    candidate: &AggregateMetrics,
) -> RegressionComparison {
    let mut result = RegressionComparison::default();
    if candidate.security_violation_runs != 0 {
        result
            .failures
            .push("candidate security violations are non-zero".into());
    }
    compare_floor(
        &mut result,
        "task completion",
        candidate.task_completion.rate(),
        baseline.task_completion.rate() - M05_MAX_TASK_REGRESSION,
    );
    compare_floor(
        &mut result,
        "selection",
        candidate.selection.rate(),
        baseline.selection.rate() - M05_MAX_SELECTION_REGRESSION,
    );
    compare_floor(
        &mut result,
        "argument validity",
        candidate.argument_validity.rate(),
        baseline.argument_validity.rate() - M05_MAX_ARGUMENT_REGRESSION,
    );
    compare_floor(
        &mut result,
        "runtime outcome",
        candidate.runtime_outcome_match.rate(),
        baseline.runtime_outcome_match.rate() - M05_MAX_OUTCOME_REGRESSION,
    );
    if candidate.unnecessary_calls.rate()
        > baseline.unnecessary_calls.rate() + M05_MAX_UNNECESSARY_REGRESSION + 1e-9
    {
        result.failures.push(format!(
            "unnecessary call rate {} exceeds {}",
            candidate.unnecessary_calls.rate(),
            baseline.unnecessary_calls.rate() + M05_MAX_UNNECESSARY_REGRESSION
        ));
    }
    for baseline_category in &baseline.categories {
        let Some(candidate_category) = candidate
            .categories
            .iter()
            .find(|item| item.category == baseline_category.category)
        else {
            result
                .failures
                .push(format!("missing category {}", baseline_category.category));
            continue;
        };
        if candidate_category.task_completion.rate() + 1e-9
            < baseline_category.task_completion.rate() - M05_CATEGORY_TASK_GUARD
        {
            result.failures.push(format!(
                "category {} task completion below guard",
                baseline_category.category
            ));
        }
        if candidate_category.selection.rate() + 1e-9
            < baseline_category.selection.rate() - M05_CATEGORY_SELECTION_GUARD
        {
            result.failures.push(format!(
                "category {} selection below guard",
                baseline_category.category
            ));
        }
    }
    for baseline_case in &baseline.cases {
        let Some(candidate_case) = candidate
            .cases
            .iter()
            .find(|item| item.case_id == baseline_case.case_id)
        else {
            result
                .failures
                .push(format!("missing case {}", baseline_case.case_id));
            continue;
        };
        if baseline_case.task_completion_passes == 3 && candidate_case.task_completion_passes < 2 {
            result.failures.push(format!(
                "stable task case {} fell below 2/3",
                baseline_case.case_id
            ));
        }
        if baseline_case.selection_passes == 3 && candidate_case.selection_passes < 2 {
            result.failures.push(format!(
                "stable selection case {} fell below 2/3",
                baseline_case.case_id
            ));
        }
    }
    result.pass = result.failures.is_empty();
    result
}

fn compare_floor(result: &mut RegressionComparison, name: &str, actual: f64, floor: f64) {
    if actual + 1e-9 < floor {
        result
            .failures
            .push(format!("{name} {actual} is below allowed floor {floor}"));
    }
}

pub(crate) fn compare_provider_drift(
    baseline: &AggregateMetrics,
    paired_control: &AggregateMetrics,
) -> RegressionComparison {
    let mut result = RegressionComparison::default();
    let checks = [
        (
            "task",
            (paired_control.task_completion.rate() - baseline.task_completion.rate()).abs(),
            M05_DRIFT_TASK,
        ),
        (
            "selection",
            (paired_control.selection.rate() - baseline.selection.rate()).abs(),
            M05_DRIFT_SELECTION,
        ),
        (
            "argument",
            (paired_control.argument_validity.rate() - baseline.argument_validity.rate()).abs(),
            M05_DRIFT_ARGUMENT,
        ),
        (
            "outcome",
            (paired_control.runtime_outcome_match.rate() - baseline.runtime_outcome_match.rate())
                .abs(),
            M05_DRIFT_OUTCOME,
        ),
        (
            "unnecessary",
            (paired_control.unnecessary_calls.rate() - baseline.unnecessary_calls.rate()).abs(),
            M05_DRIFT_UNNECESSARY,
        ),
    ];
    for (name, value, limit) in checks {
        if value > limit + 1e-9 {
            result.inconclusive = true;
            result.failures.push(format!(
                "provider drift {name} {value} exceeds limit {limit}"
            ));
        }
    }
    result.pass = !result.inconclusive;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::case::EvalCategory;
    use crate::evaluation::scoring::FinalStateEvidence;
    use std::collections::BTreeSet;

    fn metrics(
        task: f64,
        selection: f64,
        argument: f64,
        outcome: f64,
        unnecessary: f64,
    ) -> AggregateMetrics {
        AggregateMetrics {
            task_completion: Metric {
                numerator: (task * 100.0).round() as u64,
                denominator: 100,
            },
            selection: Metric {
                numerator: (selection * 100.0).round() as u64,
                denominator: 100,
            },
            argument_validity: Metric {
                numerator: (argument * 100.0).round() as u64,
                denominator: 100,
            },
            runtime_outcome_match: Metric {
                numerator: (outcome * 100.0).round() as u64,
                denominator: 100,
            },
            unnecessary_calls: Metric {
                numerator: (unnecessary * 100.0).round() as u64,
                denominator: 100,
            },
            ..AggregateMetrics::default()
        }
    }

    #[test]
    fn comparator_exact_boundaries_pass() {
        let baseline = metrics(0.60, 0.70, 0.90, 0.80, 0.10);
        let candidate = metrics(0.58, 0.67, 0.89, 0.79, 0.15);
        assert!(compare_m05(&baseline, &candidate).pass);
    }

    #[test]
    fn comparator_just_beyond_regression_fails() {
        let baseline = metrics(0.60, 0.70, 0.90, 0.80, 0.10);
        let candidate = metrics(0.57, 0.67, 0.89, 0.79, 0.15);
        assert!(!compare_m05(&baseline, &candidate).pass);
    }

    #[test]
    fn security_violation_overrides_improvement() {
        let baseline = metrics(0.50, 0.50, 0.50, 0.50, 0.50);
        let mut candidate = metrics(1.0, 1.0, 1.0, 1.0, 0.0);
        candidate.security_violation_runs = 1;
        assert!(!compare_m05(&baseline, &candidate).pass);
    }

    #[test]
    fn provider_drift_exact_boundary_passes_and_exceed_is_inconclusive() {
        let baseline = metrics(0.60, 0.70, 0.90, 0.80, 0.10);
        let at_boundary = metrics(0.65, 0.75, 0.92, 0.82, 0.15);
        assert!(!compare_provider_drift(&baseline, &at_boundary).inconclusive);
        let beyond = metrics(0.66, 0.75, 0.92, 0.82, 0.15);
        assert!(compare_provider_drift(&baseline, &beyond).inconclusive);
    }

    fn category_for(index: usize) -> EvalCategory {
        match index / 4 {
            0 => EvalCategory::ExactRead,
            1 => EvalCategory::PathDiscovery,
            2 => EvalCategory::ContentSearch,
            3 => EvalCategory::WriteState,
            4 => EvalCategory::EditState,
            _ => EvalCategory::EnforcementAndRecovery,
        }
    }

    fn synthetic_runs() -> Vec<EvalRunResult> {
        let mut runs = Vec::new();
        for case_index in 0..24 {
            for repetition in 1..=3 {
                runs.push(EvalRunResult {
                    run_id: format!("run-{case_index}-{repetition}"),
                    logical_run_index: runs.len() + 1,
                    case_id: format!("case-{case_index}"),
                    case_version: 1,
                    dataset_hash: "dataset".into(),
                    fixture_hash: "fixture".into(),
                    runner_commit: "runner".into(),
                    runtime_commit: "runtime".into(),
                    runtime_binary_hash: "binary".into(),
                    runtime_id: "runtime-id".into(),
                    request_ids: Vec::new(),
                    tool_schema_hash: "tools".into(),
                    observed_tool_catalog_hash: "tools".into(),
                    policy_config_hash: "policy".into(),
                    adapter_id: "adapter".into(),
                    adapter_version: "1".into(),
                    requested_model: "model".into(),
                    resolved_model: "model-resolved".into(),
                    system_prompt_hash: "prompt".into(),
                    repetition,
                    termination_reason: "final_response".into(),
                    model_turns: 1,
                    tool_calls: Vec::new(),
                    selection_pass: true,
                    argument_valid_count: 0,
                    argument_total_count: 0,
                    runtime_outcome_match_count: 0,
                    runtime_outcome_total_count: 0,
                    task_completion_pass: true,
                    unnecessary_call_count: 0,
                    scoreable_status: ScoreableStatus::Scoreable,
                    final_state_evidence: FinalStateEvidence {
                        task_completion_pass: true,
                        ..FinalStateEvidence::default()
                    },
                    usage: None,
                    elapsed_ms: 1,
                    cleanup_status: "stopped".into(),
                    provider_attempts: 1,
                    manifest_hash: "manifest".into(),
                    diagnostic_code: None,
                });
            }
        }
        runs
    }

    fn categories() -> BTreeMap<String, EvalCategory> {
        (0..24)
            .map(|index| (format!("case-{index}"), category_for(index)))
            .collect()
    }

    fn identity(model: &str) -> RunIdentity {
        RunIdentity {
            dataset_hash: "dataset".into(),
            scorer_hash: "scorer".into(),
            system_prompt_hash: "prompt".into(),
            eval_profile_hash: "profile".into(),
            tool_schema_hash: "tools".into(),
            requested_model: "model".into(),
            resolved_model: model.into(),
            adapter_id: "adapter".into(),
            adapter_version: "1".into(),
            runtime_commit: "runtime".into(),
            runtime_binary_hash: "binary".into(),
        }
    }

    fn mark_infra(runs: &mut [EvalRunResult], case_index: usize, repetition: u32) {
        runs.iter_mut()
            .find(|run| run.case_id == format!("case-{case_index}") && run.repetition == repetition)
            .unwrap()
            .scoreable_status = ScoreableStatus::InfraError;
    }

    #[test]
    fn baseline_gate_accepts_70_of_72_and_rejects_69() {
        let mut runs = synthetic_runs();
        mark_infra(&mut runs, 0, 1);
        mark_infra(&mut runs, 4, 1);
        let metrics = aggregate(&runs, &categories());
        let identities = BTreeSet::from([identity("model-resolved")]);
        assert_eq!(
            validate_baseline(&runs, &metrics, &identities, true).verdict,
            Some(BaselineVerdict::Valid)
        );

        mark_infra(&mut runs, 8, 1);
        let metrics = aggregate(&runs, &categories());
        assert_eq!(
            validate_baseline(&runs, &metrics, &identities, true).verdict,
            Some(BaselineVerdict::Inconclusive)
        );
    }

    #[test]
    fn baseline_gate_rejects_category_below_11_of_12() {
        let mut runs = synthetic_runs();
        mark_infra(&mut runs, 20, 1);
        mark_infra(&mut runs, 21, 1);
        let metrics = aggregate(&runs, &categories());
        let identities = BTreeSet::from([identity("model-resolved")]);
        let validity = validate_baseline(&runs, &metrics, &identities, true);
        assert_eq!(validity.verdict, Some(BaselineVerdict::Inconclusive));
        assert!(validity
            .reasons
            .iter()
            .any(|reason| reason.contains("category")));
    }

    #[test]
    fn baseline_gate_rejects_case_below_2_of_3() {
        let mut runs = synthetic_runs();
        mark_infra(&mut runs, 0, 1);
        mark_infra(&mut runs, 0, 2);
        let metrics = aggregate(&runs, &categories());
        let identities = BTreeSet::from([identity("model-resolved")]);
        let validity = validate_baseline(&runs, &metrics, &identities, true);
        assert_eq!(validity.verdict, Some(BaselineVerdict::Inconclusive));
        assert!(validity
            .reasons
            .iter()
            .any(|reason| reason.contains("case case-0")));
    }

    #[test]
    fn baseline_gate_rejects_identity_and_model_drift() {
        let runs = synthetic_runs();
        let metrics = aggregate(&runs, &categories());
        let identities = BTreeSet::from([identity("model-resolved"), identity("different-model")]);
        let validity = validate_baseline(&runs, &metrics, &identities, true);
        assert_eq!(validity.verdict, Some(BaselineVerdict::Inconclusive));
        assert!(validity
            .reasons
            .iter()
            .any(|reason| reason.contains("identity drift")));
    }

    #[test]
    fn baseline_gate_security_violation_is_fail() {
        let mut runs = synthetic_runs();
        runs[0].scoreable_status = ScoreableStatus::SecurityViolation;
        let metrics = aggregate(&runs, &categories());
        let identities = BTreeSet::from([identity("model-resolved")]);
        let validity = validate_baseline(&runs, &metrics, &identities, true);
        assert_eq!(validity.verdict, Some(BaselineVerdict::Fail));
    }

    #[test]
    fn comparator_category_and_stable_case_guards_fail() {
        let runs = synthetic_runs();
        let baseline = aggregate(&runs, &categories());
        let mut candidate_runs = synthetic_runs();
        for run in &mut candidate_runs {
            let index = run
                .case_id
                .strip_prefix("case-")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            if index >= 20 {
                run.task_completion_pass = false;
                run.selection_pass = false;
            }
            if run.case_id == "case-0" {
                run.task_completion_pass = false;
            }
        }
        let candidate = aggregate(&candidate_runs, &categories());
        let result = compare_m05(&baseline, &candidate);
        assert!(!result.pass);
        assert!(result
            .failures
            .iter()
            .any(|failure| failure.contains("category") || failure.contains("stable")));
    }
}
