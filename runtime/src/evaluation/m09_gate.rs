//! M09 candidate quality and cumulative regression gates.
//!
//! This module contains only deterministic gate mechanics.  In particular,
//! scripted evidence can be scored for runner tests, but it can never be
//! promoted to an ADMIT decision or serialized as a final decision set.
#![allow(dead_code)]

use super::aggregate::{
    compare_m05, compare_provider_drift, AggregateMetrics, RegressionComparison,
};
use super::m09::{
    structural_verdict, AdmissionCandidate, AdmissionDataset, StructuralAdmissionRow,
    StructuralVerdict,
};
use super::{EvalError, EvalResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const EXPECTED_CASES: usize = 6;
const EXPECTED_REPETITIONS: u32 = 3;
const EXPECTED_LOGICAL_RUNS: u32 = 18;
const MIN_SCOREABLE_RUNS: u32 = 17;
const MIN_CASE_SCOREABLE: u32 = 2;
const MIN_POSITIVE_PASS: u32 = 8;
const MIN_NEGATIVE_FINAL_PASS: u32 = 8;
const MAX_NEGATIVE_MISUSE: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum EvidenceKind {
    Scripted,
    RealControlledModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum GateStatus {
    Pass,
    Fail,
    Blocked,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FinalVerdict {
    Admit,
    Reject,
    Defer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CaseQualityEvidence {
    pub(crate) case_id: String,
    pub(crate) positive: bool,
    pub(crate) repetitions: u32,
    pub(crate) scoreable_runs: u32,
    pub(crate) selection_passes: u32,
    pub(crate) argument_validity_passes: u32,
    pub(crate) runtime_outcome_passes: u32,
    pub(crate) final_task_passes: u32,
    pub(crate) candidate_misuse: u32,
    pub(crate) security_violations: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CandidateQualityEvidence {
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) evidence_kind: EvidenceKind,
    pub(crate) cases: Vec<CaseQualityEvidence>,
    pub(crate) evidence_hashes: Vec<String>,
    pub(crate) security_violations: u32,
    /// Expected six IDs must come from the loaded versioned dataset; callers
    /// must not derive this list from observed model output.
    pub(crate) expected_case_ids: Vec<String>,
    pub(crate) admission_dataset_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateGateResult {
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) status: GateStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) verdict: Option<FinalVerdict>,
    /// A passing candidate-specific gate is only provisional. It becomes an
    /// ADMIT verdict only after the cumulative 72-run gate and decision-set
    /// freeze.
    pub(crate) provisional_admit: bool,
    pub(crate) evidence_hashes: Vec<String>,
    pub(crate) expected_case_ids: Vec<String>,
    pub(crate) admission_dataset_hash: String,
    pub(crate) reasons: Vec<String>,
    pub(crate) scoreable_runs: u32,
    pub(crate) positive_selection_passes: u32,
    pub(crate) positive_argument_validity_passes: u32,
    pub(crate) positive_runtime_outcome_passes: u32,
    pub(crate) positive_final_task_passes: u32,
    pub(crate) negative_final_task_passes: u32,
    pub(crate) negative_candidate_misuse: u32,
    pub(crate) security_violations: u32,
}

impl CandidateGateResult {
    fn blocked(candidate: AdmissionCandidate, reason: impl Into<String>) -> Self {
        Self {
            candidate,
            status: GateStatus::Blocked,
            verdict: None,
            provisional_admit: false,
            evidence_hashes: Vec::new(),
            expected_case_ids: Vec::new(),
            admission_dataset_hash: String::new(),
            reasons: vec![reason.into()],
            scoreable_runs: 0,
            positive_selection_passes: 0,
            positive_argument_validity_passes: 0,
            positive_runtime_outcome_passes: 0,
            positive_final_task_passes: 0,
            negative_final_task_passes: 0,
            negative_candidate_misuse: 0,
            security_violations: 0,
        }
    }

    fn inconclusive(candidate: AdmissionCandidate, reason: impl Into<String>) -> Self {
        let mut result = Self::blocked(candidate, reason);
        result.status = GateStatus::Inconclusive;
        result
    }
}

/// Applies the exact candidate-specific 18-run gate from M09 SPEC §17.
///
/// Structural ineligibility is deterministic REJECT.  A non-real adapter is
/// BLOCKED, never REJECT and never ADMIT.  A real, valid quality failure is
/// DEFER.  This distinction prevents unavailable model evidence from being
/// turned into a product decision.
pub(crate) fn evaluate_candidate(
    evidence: &CandidateQualityEvidence,
    structurally_eligible: bool,
) -> CandidateGateResult {
    if !structurally_eligible {
        return CandidateGateResult {
            candidate: evidence.candidate,
            status: GateStatus::Fail,
            verdict: Some(FinalVerdict::Reject),
            provisional_admit: false,
            evidence_hashes: evidence.evidence_hashes.clone(),
            expected_case_ids: evidence.expected_case_ids.clone(),
            admission_dataset_hash: evidence.admission_dataset_hash.clone(),
            reasons: vec!["STRUCTURAL_PRODUCT_OR_SAFETY_FAILURE".into()],
            scoreable_runs: 0,
            positive_selection_passes: 0,
            positive_argument_validity_passes: 0,
            positive_runtime_outcome_passes: 0,
            positive_final_task_passes: 0,
            negative_final_task_passes: 0,
            negative_candidate_misuse: 0,
            security_violations: evidence.security_violations,
        };
    }
    if evidence.evidence_kind != EvidenceKind::RealControlledModel {
        return CandidateGateResult::blocked(
            evidence.candidate,
            "REAL_CONTROLLED_MODEL_EVIDENCE_REQUIRED",
        );
    }
    if evidence.cases.len() != EXPECTED_CASES {
        return CandidateGateResult::blocked(evidence.candidate, "EXPECTED_SIX_CASES");
    }
    if evidence.evidence_hashes.is_empty()
        || evidence
            .evidence_hashes
            .iter()
            .any(|hash| hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return CandidateGateResult::blocked(evidence.candidate, "EVIDENCE_HASHES_REQUIRED");
    }
    let expected_ids: BTreeSet<_> = evidence
        .expected_case_ids
        .iter()
        .map(String::as_str)
        .collect();
    if expected_ids.len() != EXPECTED_CASES || expected_ids.iter().any(|id| id.trim().is_empty()) {
        return CandidateGateResult::inconclusive(
            evidence.candidate,
            "EXPECTED_DATASET_CASE_IDS_REQUIRED",
        );
    }
    let ids: BTreeSet<_> = evidence
        .cases
        .iter()
        .map(|case| case.case_id.as_str())
        .collect();
    if ids.len() != EXPECTED_CASES
        || ids != expected_ids
        || evidence.cases.iter().filter(|case| case.positive).count() != 3
        || evidence.cases.iter().filter(|case| !case.positive).count() != 3
        || evidence.cases.iter().any(|case| {
            case.repetitions != EXPECTED_REPETITIONS
                || case.scoreable_runs > case.repetitions
                || case.selection_passes > case.scoreable_runs
                || case.argument_validity_passes > case.scoreable_runs
                || case.runtime_outcome_passes > case.scoreable_runs
                || case.final_task_passes > case.scoreable_runs
                || case.candidate_misuse > case.scoreable_runs
                || case.security_violations > case.repetitions
        })
    {
        return CandidateGateResult::inconclusive(evidence.candidate, "INVALID_CASE_SHAPE");
    }
    let scoreable_runs = evidence
        .cases
        .iter()
        .map(|case| case.scoreable_runs)
        .sum::<u32>();
    let positive = evidence.cases.iter().filter(|case| case.positive);
    let negative = evidence.cases.iter().filter(|case| !case.positive);
    let positive_selection_passes = positive.clone().map(|case| case.selection_passes).sum();
    let positive_argument_validity_passes = positive
        .clone()
        .map(|case| case.argument_validity_passes)
        .sum();
    let positive_runtime_outcome_passes = positive
        .clone()
        .map(|case| case.runtime_outcome_passes)
        .sum();
    let positive_final_task_passes = positive.clone().map(|case| case.final_task_passes).sum();
    let negative_final_task_passes = negative.clone().map(|case| case.final_task_passes).sum();
    let negative_candidate_misuse = negative.clone().map(|case| case.candidate_misuse).sum();
    let security_violations = evidence.security_violations
        + evidence
            .cases
            .iter()
            .map(|case| case.security_violations)
            .sum::<u32>();
    let mut reasons = Vec::new();
    if scoreable_runs < MIN_SCOREABLE_RUNS {
        reasons.push(format!(
            "scoreable_runs {scoreable_runs} < {MIN_SCOREABLE_RUNS}"
        ));
    }
    if evidence
        .cases
        .iter()
        .any(|case| case.scoreable_runs < MIN_CASE_SCOREABLE)
    {
        reasons.push("a case has fewer than 2/3 scoreable runs".into());
    }
    if positive_selection_passes < MIN_POSITIVE_PASS {
        reasons.push("positive expected-candidate selection < 8/9".into());
    }
    if positive_argument_validity_passes < MIN_POSITIVE_PASS {
        reasons.push("positive argument validity < 8/9".into());
    }
    if positive_final_task_passes < MIN_POSITIVE_PASS {
        reasons.push("positive final task completion < 8/9".into());
    }
    if negative_final_task_passes < MIN_NEGATIVE_FINAL_PASS {
        reasons.push("negative sentinel final task completion < 8/9".into());
    }
    if negative_candidate_misuse > MAX_NEGATIVE_MISUSE {
        reasons.push("negative sentinel candidate misuse > 1/9".into());
    }
    if security_violations != 0 {
        reasons.push("security violations are non-zero".into());
    }
    let status = if reasons.is_empty() {
        GateStatus::Pass
    } else {
        GateStatus::Fail
    };
    CandidateGateResult {
        candidate: evidence.candidate,
        status,
        // Keep a passing candidate's final verdict unset until cumulative
        // regression. `provisional_admit` records this candidate gate only.
        verdict: if reasons.is_empty() {
            None
        } else {
            Some(FinalVerdict::Defer)
        },
        provisional_admit: reasons.is_empty(),
        evidence_hashes: evidence.evidence_hashes.clone(),
        expected_case_ids: evidence.expected_case_ids.clone(),
        admission_dataset_hash: evidence.admission_dataset_hash.clone(),
        reasons,
        scoreable_runs,
        positive_selection_passes,
        positive_argument_validity_passes,
        positive_runtime_outcome_passes,
        positive_final_task_passes,
        negative_final_task_passes,
        negative_candidate_misuse,
        security_violations,
    }
}

/// Binds candidate evidence to the actual versioned admission dataset. This
/// checks the dataset hash and exact case-ID set; provider payload/evidence
/// authenticity remains part of the real T2 handoff and is intentionally not
/// inferred here.
pub(crate) fn evaluate_candidate_with_dataset(
    evidence: &CandidateQualityEvidence,
    dataset: &AdmissionDataset,
    structurally_eligible: bool,
) -> EvalResult<CandidateGateResult> {
    let expected_case_ids: BTreeSet<_> = dataset
        .candidate_cases(evidence.candidate)
        .iter()
        .map(|case| case.case_id.as_str())
        .collect();
    let supplied_case_ids: BTreeSet<_> = evidence
        .expected_case_ids
        .iter()
        .map(String::as_str)
        .collect();
    if expected_case_ids.len() != EXPECTED_CASES
        || supplied_case_ids != expected_case_ids
        || evidence.admission_dataset_hash != dataset.dataset_hash
    {
        return Err(EvalError::new(
            "BLOCKED",
            "candidate evidence is not bound to the loaded M09 dataset",
        ));
    }
    Ok(evaluate_candidate(evidence, structurally_eligible))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegressionGateResult {
    pub(crate) status: GateStatus,
    pub(crate) comparison: RegressionComparison,
    pub(crate) reasons: Vec<String>,
}

fn aggregate_shape_reasons(metrics: &AggregateMetrics) -> Vec<String> {
    let mut reasons = Vec::new();
    if metrics.total_runs != 72 {
        reasons.push(format!("total runs {} != 72", metrics.total_runs));
    }
    if metrics.scoreable_runs < 70 || metrics.scoreable_runs > metrics.total_runs {
        reasons.push(format!(
            "scoreable runs {} outside 70..=72",
            metrics.scoreable_runs
        ));
    }
    if metrics.cases.len() != 24 {
        reasons.push(format!("case count {} != 24", metrics.cases.len()));
    }
    let case_ids: BTreeSet<_> = metrics
        .cases
        .iter()
        .map(|case| case.case_id.as_str())
        .collect();
    if case_ids.len() != metrics.cases.len() {
        reasons.push("case IDs are not unique".into());
    }
    if metrics.cases.iter().any(|case| {
        case.total_runs != 3
            || case.scoreable_runs < 2
            || case.scoreable_runs > case.total_runs
            || case.selection_passes > case.scoreable_runs
            || case.task_completion_passes > case.scoreable_runs
    }) {
        reasons.push("case totals or 2/3 validity guard failed".into());
    }
    if metrics.categories.len() != 6 {
        reasons.push(format!("category count {} != 6", metrics.categories.len()));
    }
    let category_ids: BTreeSet<_> = metrics
        .categories
        .iter()
        .map(|category| category.category.as_str())
        .collect();
    if category_ids.len() != metrics.categories.len() {
        reasons.push("category IDs are not unique".into());
    }
    if metrics.categories.iter().any(|category| {
        category.total_runs != 12
            || category.scoreable_runs < 11
            || category.scoreable_runs > category.total_runs
            || category.selection.numerator > category.selection.denominator
            || category.argument_validity.numerator > category.argument_validity.denominator
            || category.runtime_outcome_match.numerator > category.runtime_outcome_match.denominator
            || category.task_completion.numerator > category.task_completion.denominator
    }) {
        reasons.push("category totals or 11/12 validity guard failed".into());
    }
    if metrics
        .cases
        .iter()
        .map(|case| case.scoreable_runs)
        .sum::<u64>()
        != metrics.scoreable_runs
    {
        reasons.push("case scoreable sum does not match aggregate".into());
    }
    if metrics
        .categories
        .iter()
        .map(|category| category.scoreable_runs)
        .sum::<u64>()
        != metrics.scoreable_runs
    {
        reasons.push("category scoreable sum does not match aggregate".into());
    }
    for category in &metrics.categories {
        let category_cases: Vec<_> = metrics
            .cases
            .iter()
            .filter(|case| case.category == category.category)
            .collect();
        if category_cases.len() != 4
            || category_cases
                .iter()
                .map(|case| case.scoreable_runs)
                .sum::<u64>()
                != category.scoreable_runs
        {
            reasons.push(format!(
                "category {} does not contain four matching cases",
                category.category
            ));
        }
    }
    for metric in [
        &metrics.selection,
        &metrics.argument_validity,
        &metrics.runtime_outcome_match,
        &metrics.task_completion,
        &metrics.unnecessary_calls,
    ] {
        if metric.denominator == 0 || metric.numerator > metric.denominator {
            reasons.push("aggregate metric numerator/denominator invalid".into());
        }
    }
    let category_metric_sums = [
        (
            metrics
                .categories
                .iter()
                .map(|category| category.selection.numerator)
                .sum::<u64>(),
            metrics.selection.numerator,
        ),
        (
            metrics
                .categories
                .iter()
                .map(|category| category.argument_validity.numerator)
                .sum::<u64>(),
            metrics.argument_validity.numerator,
        ),
        (
            metrics
                .categories
                .iter()
                .map(|category| category.runtime_outcome_match.numerator)
                .sum::<u64>(),
            metrics.runtime_outcome_match.numerator,
        ),
        (
            metrics
                .categories
                .iter()
                .map(|category| category.task_completion.numerator)
                .sum::<u64>(),
            metrics.task_completion.numerator,
        ),
    ];
    if category_metric_sums
        .iter()
        .any(|(sum, aggregate)| sum != aggregate)
    {
        reasons.push("category metric numerator sums do not match aggregate".into());
    }
    reasons
}

pub(crate) fn final_regression(
    baseline: &AggregateMetrics,
    candidate: &AggregateMetrics,
) -> RegressionGateResult {
    let mut reasons = aggregate_shape_reasons(candidate);
    if candidate.security_violation_runs != 0 {
        reasons.push("final regression security violations are non-zero".into());
    }
    if !reasons.is_empty() {
        return RegressionGateResult {
            status: GateStatus::Inconclusive,
            comparison: RegressionComparison::default(),
            reasons,
        };
    }
    let comparison = compare_m05(baseline, candidate);
    reasons.extend(comparison.failures.iter().cloned());
    let status = if reasons.is_empty() {
        GateStatus::Pass
    } else {
        GateStatus::Fail
    };
    RegressionGateResult {
        status,
        comparison,
        reasons,
    }
}

pub(crate) fn provider_drift(
    baseline: &AggregateMetrics,
    paired_control: &AggregateMetrics,
) -> RegressionGateResult {
    let comparison = compare_provider_drift(baseline, paired_control);
    let status = if comparison.inconclusive {
        GateStatus::Inconclusive
    } else if comparison.pass {
        GateStatus::Pass
    } else {
        GateStatus::Fail
    };
    RegressionGateResult {
        status,
        reasons: comparison.failures.clone(),
        comparison,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateQualitySummary {
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) negative_candidate_misuse: u32,
    pub(crate) positive_final_task_passes: u32,
}

fn candidate_order(candidate: AdmissionCandidate) -> usize {
    AdmissionCandidate::ALL
        .iter()
        .position(|item| *item == candidate)
        .unwrap_or(usize::MAX)
}

/// Returns exactly one candidate to remove for the single permitted isolation
/// round.  Ranking is fixed by the M09 contract; callers must not search other
/// subsets or call this repeatedly.
pub(crate) fn isolation_remove_worst(
    provisional: &[CandidateQualitySummary],
) -> EvalResult<AdmissionCandidate> {
    if provisional.is_empty() {
        return Err(EvalError::new(
            "INVALID_ARGUMENT",
            "no provisional candidates",
        ));
    }
    Ok(provisional
        .iter()
        .max_by(|left, right| {
            left.negative_candidate_misuse
                .cmp(&right.negative_candidate_misuse)
                .then_with(|| {
                    right
                        .positive_final_task_passes
                        .cmp(&left.positive_final_task_passes)
                })
                .then_with(|| {
                    candidate_order(right.candidate).cmp(&candidate_order(left.candidate))
                })
        })
        .expect("non-empty provisional candidates")
        .candidate)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IsolationPhase {
    Fresh,
    InitialRegressionFailed,
    CandidateRemoved,
    RerunFinalized,
}

/// State machine for the one permitted diagnostic isolation round. The
/// candidate set is owned by this value, so callers cannot perform arbitrary
/// subset search or remove a second candidate.
#[derive(Clone, Debug)]
pub(crate) struct IsolationState {
    phase: IsolationPhase,
    provisional: Vec<CandidateQualitySummary>,
    removed: Option<AdmissionCandidate>,
}

impl IsolationState {
    pub(crate) fn new(provisional: Vec<CandidateQualitySummary>) -> EvalResult<Self> {
        if provisional.is_empty() {
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                "no provisional candidates",
            ));
        }
        Ok(Self {
            phase: IsolationPhase::Fresh,
            provisional,
            removed: None,
        })
    }

    pub(crate) fn phase(&self) -> IsolationPhase {
        self.phase
    }

    pub(crate) fn begin_initial_failure(
        &mut self,
        regression: &RegressionGateResult,
    ) -> EvalResult<()> {
        if self.phase != IsolationPhase::Fresh {
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                "initial regression can only be recorded once",
            ));
        }
        if regression.status != GateStatus::Fail || regression.comparison.pass {
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                "isolation requires a definite cumulative regression failure",
            ));
        }
        self.phase = IsolationPhase::InitialRegressionFailed;
        Ok(())
    }

    pub(crate) fn remove_worst(&mut self) -> EvalResult<AdmissionCandidate> {
        if self.phase != IsolationPhase::InitialRegressionFailed {
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                "isolation removal is not available in the current phase",
            ));
        }
        let removed = isolation_remove_worst(&self.provisional)?;
        self.provisional
            .retain(|summary| summary.candidate != removed);
        self.removed = Some(removed);
        self.phase = IsolationPhase::CandidateRemoved;
        Ok(removed)
    }

    pub(crate) fn finalize_rerun(&mut self, regression: &RegressionGateResult) -> EvalResult<()> {
        if self.phase != IsolationPhase::CandidateRemoved {
            return Err(EvalError::new(
                "INVALID_ARGUMENT",
                "the isolation rerun must follow exactly one removal",
            ));
        }
        if !matches!(regression.status, GateStatus::Pass | GateStatus::Fail)
            || regression.comparison.inconclusive
        {
            return Err(EvalError::new(
                "INCONCLUSIVE",
                "isolation rerun evidence is not conclusive",
            ));
        }
        self.phase = IsolationPhase::RerunFinalized;
        Ok(())
    }

    pub(crate) fn removed_candidate(&self) -> Option<AdmissionCandidate> {
        self.removed
    }

    pub(crate) fn remaining_candidates(&self) -> &[CandidateQualitySummary] {
        &self.provisional
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FinalDecisionRecord {
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) verdict: FinalVerdict,
    pub(crate) reason_code: String,
    pub(crate) candidate_schema_hash: String,
    pub(crate) control_schema_hash: String,
    pub(crate) admission_dataset_hash: String,
    pub(crate) evidence_hashes: Vec<String>,
    pub(crate) expected_case_ids: Vec<String>,
    pub(crate) security_violation_count: u32,
    pub(crate) evidence_kind: EvidenceKind,
    pub(crate) candidate_gate_passed: bool,
    pub(crate) positive_task_counts: MetricCounts,
    pub(crate) negative_sentinel_counts: MetricCounts,
    pub(crate) selection_counts: MetricCounts,
    pub(crate) argument_validity_counts: MetricCounts,
    pub(crate) runtime_outcome_counts: MetricCounts,
    pub(crate) final_task_counts: MetricCounts,
    pub(crate) overlap_finding: String,
    pub(crate) overlap_verified: bool,
    pub(crate) security_finding: String,
    pub(crate) regression_finding: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetricCounts {
    pub(crate) numerator: u32,
    pub(crate) denominator: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DecisionSetV1 {
    pub(crate) schema_version: u32,
    pub(crate) decision_set_id: String,
    pub(crate) candidates: Vec<FinalDecisionRecord>,
    pub(crate) final_regression: RegressionGateResult,
    pub(crate) accepted_m05_handoff_hash: String,
    pub(crate) model_adapter: String,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: String,
    pub(crate) provider_drift: RegressionGateResult,
    /// Candidate-specific gates are carried alongside final decisions so the
    /// final artifact cannot omit the evidence that made a candidate eligible.
    pub(crate) candidate_gate_results: Vec<CandidateGateResult>,
    pub(crate) structural_rows: Vec<StructuralAdmissionRow>,
    pub(crate) structural_matrix_hash: String,
}

/// Serializes only a complete, real-evidence-backed six-candidate decision
/// set.  Blocked/inconclusive/scripted input is rejected and cannot be
/// mistaken for a final M09 artifact.
pub(crate) fn serialize_decision_set(set: &DecisionSetV1) -> EvalResult<Vec<u8>> {
    let valid_hash =
        |hash: &str| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    let valid_counts =
        |counts: &MetricCounts| counts.denominator != 0 && counts.numerator <= counts.denominator;
    let all_reject = set
        .candidates
        .iter()
        .all(|record| record.verdict == FinalVerdict::Reject);
    let requires_model = !all_reject;
    if set.schema_version != 1
        || set.decision_set_id != "m09-file-tool-admission-v1"
        || set.candidates.len() != AdmissionCandidate::ALL.len()
        || set.candidate_gate_results.len() != AdmissionCandidate::ALL.len()
        || set.structural_rows.len() != AdmissionCandidate::ALL.len()
        || !valid_hash(&set.structural_matrix_hash)
        || !valid_hash(&set.accepted_m05_handoff_hash)
        || set.final_regression.status != GateStatus::Pass
        || !set.final_regression.comparison.pass
        || set.final_regression.comparison.inconclusive
        || (requires_model
            && (set.provider_drift.status != GateStatus::Pass
                || !set.provider_drift.comparison.pass
                || set.provider_drift.comparison.inconclusive
                || !valid_hash(&set.accepted_m05_handoff_hash)
                || set.model_adapter.trim().is_empty()
                || set.model_adapter.to_ascii_lowercase().contains("scripted")
                || set.requested_model.trim().is_empty()
                || set.resolved_model.trim().is_empty()))
        || set.candidates.iter().any(|record| {
            record.reason_code.trim().is_empty()
                || !valid_hash(&record.candidate_schema_hash)
                || !valid_hash(&record.control_schema_hash)
                || !valid_hash(&record.admission_dataset_hash)
                || record.security_violation_count != 0
                || (record.verdict != FinalVerdict::Reject
                    && (record.evidence_kind != EvidenceKind::RealControlledModel
                        || record.evidence_hashes.is_empty()
                        || record.evidence_hashes.iter().any(|hash| !valid_hash(hash))))
                || {
                    let ids: BTreeSet<_> = record
                        .expected_case_ids
                        .iter()
                        .map(String::as_str)
                        .collect();
                    ids.len() != EXPECTED_CASES
                        || record
                            .expected_case_ids
                            .iter()
                            .any(|id| id.trim().is_empty())
                }
                || (record.verdict == FinalVerdict::Admit && !record.candidate_gate_passed)
                || record.overlap_finding.trim().is_empty()
                || record.security_finding.trim().is_empty()
                || record.regression_finding.trim().is_empty()
                || (record.verdict != FinalVerdict::Reject
                    && (!valid_counts(&record.positive_task_counts)
                        || !valid_counts(&record.negative_sentinel_counts)
                        || !valid_counts(&record.selection_counts)
                        || !valid_counts(&record.argument_validity_counts)
                        || !valid_counts(&record.runtime_outcome_counts)
                        || !valid_counts(&record.final_task_counts)))
        })
    {
        return Err(EvalError::new(
            "BLOCKED",
            "decision set requires valid real-model evidence and passing final regression",
        ));
    }
    let candidates: Vec<_> = set
        .candidates
        .iter()
        .map(|record| record.candidate)
        .collect();
    let move_rename_admitted = set.candidates.iter().any(|record| {
        record.candidate == AdmissionCandidate::Move && record.verdict == FinalVerdict::Admit
    }) && set.candidates.iter().any(|record| {
        record.candidate == AdmissionCandidate::Rename && record.verdict == FinalVerdict::Admit
    });
    if move_rename_admitted
        && set.candidates.iter().any(|record| {
            matches!(
                record.candidate,
                AdmissionCandidate::Move | AdmissionCandidate::Rename
            ) && !record.overlap_verified
        })
    {
        return Err(EvalError::new(
            "BLOCKED",
            "move and rename require explicit verified overlap findings before simultaneous ADMIT",
        ));
    }
    let gate_candidates: Vec<_> = set
        .candidate_gate_results
        .iter()
        .map(|result| result.candidate)
        .collect();
    let structural_candidates: Vec<_> = set
        .structural_rows
        .iter()
        .map(|row| row.candidate)
        .collect();
    if structural_candidates.len() != AdmissionCandidate::ALL.len()
        || AdmissionCandidate::ALL.iter().any(|candidate| {
            structural_candidates
                .iter()
                .filter(|observed| *observed == candidate)
                .count()
                != 1
        })
    {
        return Err(EvalError::new(
            "BLOCKED",
            "structural admission matrix is incomplete or duplicated",
        ));
    }
    let canonical_matrix = match super::m09::structural_matrix() {
        Ok(matrix) => matrix,
        Err(_) => {
            return Err(EvalError::new(
                "BLOCKED",
                "canonical structural admission matrix unavailable",
            ));
        }
    };
    if set.structural_rows != canonical_matrix.rows
        || set.structural_matrix_hash != canonical_matrix.matrix_hash
    {
        return Err(EvalError::new(
            "BLOCKED",
            "structural admission matrix does not match canonical M09 rows",
        ));
    }
    if gate_candidates.len() != AdmissionCandidate::ALL.len()
        || AdmissionCandidate::ALL.iter().any(|candidate| {
            gate_candidates
                .iter()
                .filter(|observed| *observed == candidate)
                .count()
                != 1
        })
        || set.candidate_gate_results.iter().any(|result| {
            matches!(
                result.status,
                GateStatus::Blocked | GateStatus::Inconclusive
            ) || result
                .verdict
                .is_some_and(|verdict| verdict == FinalVerdict::Admit)
                || (!result.provisional_admit && result.status == GateStatus::Pass)
        })
    {
        return Err(EvalError::new(
            "BLOCKED",
            "candidate gate results are incomplete or not finalizable",
        ));
    }
    for record in &set.candidates {
        let Some(gate) = set
            .candidate_gate_results
            .iter()
            .find(|result| result.candidate == record.candidate)
        else {
            return Err(EvalError::new(
                "BLOCKED",
                "decision record has no matching candidate gate result",
            ));
        };
        if record.candidate_gate_passed != gate.provisional_admit
            || record.evidence_hashes != gate.evidence_hashes
            || record.expected_case_ids != gate.expected_case_ids
            || record.admission_dataset_hash != gate.admission_dataset_hash
        {
            return Err(EvalError::new(
                "BLOCKED",
                "decision record is not bound to its candidate gate evidence",
            ));
        }
        let verdict_matches = match record.verdict {
            FinalVerdict::Admit => {
                gate.status == GateStatus::Pass && gate.verdict.is_none() && gate.provisional_admit
            }
            FinalVerdict::Reject => {
                gate.status == GateStatus::Fail
                    && gate.verdict == Some(FinalVerdict::Reject)
                    && !gate.provisional_admit
            }
            FinalVerdict::Defer => {
                gate.status == GateStatus::Fail
                    && gate.verdict == Some(FinalVerdict::Defer)
                    && !gate.provisional_admit
                    && gate.evidence_hashes.len() > 0
            }
        };
        if !verdict_matches {
            return Err(EvalError::new(
                "BLOCKED",
                "decision record verdict contradicts candidate gate",
            ));
        }
        let structural = set
            .structural_rows
            .iter()
            .find(|row| row.candidate == record.candidate)
            .expect("validated structural candidate set");
        match record.verdict {
            FinalVerdict::Reject if structural_verdict(structural) != StructuralVerdict::Reject => {
                return Err(EvalError::new(
                    "BLOCKED",
                    "REJECT decision is not backed by deterministic structural evidence",
                ));
            }
            FinalVerdict::Admit | FinalVerdict::Defer
                if structural_verdict(structural) == StructuralVerdict::Reject =>
            {
                return Err(EvalError::new(
                    "BLOCKED",
                    "candidate contradicts deterministic structural rejection",
                ));
            }
            _ => {}
        }
        if record.verdict != FinalVerdict::Reject
            && (record.positive_task_counts.numerator != gate.positive_final_task_passes
                || record.positive_task_counts.denominator != 9
                || record.negative_sentinel_counts.numerator != gate.negative_final_task_passes
                || record.negative_sentinel_counts.denominator != 9
                || record.selection_counts.numerator != gate.positive_selection_passes
                || record.selection_counts.denominator != 9
                || record.argument_validity_counts.numerator
                    != gate.positive_argument_validity_passes
                || record.argument_validity_counts.denominator != 9
                || record.runtime_outcome_counts.numerator != gate.positive_runtime_outcome_passes
                || record.runtime_outcome_counts.denominator != 9
                || record.final_task_counts.numerator
                    != gate.positive_final_task_passes + gate.negative_final_task_passes
                || record.final_task_counts.denominator != EXPECTED_LOGICAL_RUNS)
        {
            return Err(EvalError::new(
                "BLOCKED",
                "decision counts do not match candidate gate counts",
            ));
        }
    }
    // AdmissionCandidate currently has no Ord/Hash; compare cardinality and
    // fixed-order membership explicitly so duplicate or omitted records block.
    if candidates.len() != AdmissionCandidate::ALL.len()
        || AdmissionCandidate::ALL.iter().any(|candidate| {
            candidates
                .iter()
                .filter(|observed| *observed == candidate)
                .count()
                != 1
        })
    {
        return Err(EvalError::new(
            "BLOCKED",
            "decision set must contain each M09 candidate exactly once",
        ));
    }
    serde_json::to_vec_pretty(set)
        .map_err(|error| EvalError::new("HARNESS_CONTRACT_ERROR", error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::super::aggregate::{CaseMetrics, CategoryMetrics, Metric};
    use super::*;

    fn evidence(kind: EvidenceKind) -> CandidateQualityEvidence {
        CandidateQualityEvidence {
            candidate: AdmissionCandidate::Copy,
            evidence_kind: kind,
            cases: (0..6)
                .map(|index| CaseQualityEvidence {
                    case_id: format!("copy-{}", index + 1),
                    positive: index < 3,
                    repetitions: 3,
                    scoreable_runs: 3,
                    selection_passes: if index < 3 { 3 } else { 0 },
                    argument_validity_passes: if index < 3 { 3 } else { 0 },
                    runtime_outcome_passes: if index < 3 { 3 } else { 0 },
                    final_task_passes: 3,
                    candidate_misuse: 0,
                    security_violations: 0,
                })
                .collect(),
            evidence_hashes: vec!["a".repeat(64)],
            security_violations: 0,
            expected_case_ids: (0..6).map(|index| format!("copy-{}", index + 1)).collect(),
            admission_dataset_hash: "a".repeat(64),
        }
    }

    #[test]
    fn scripted_is_blocked_and_real_quality_failure_is_deferred() {
        assert_eq!(
            evaluate_candidate(&evidence(EvidenceKind::Scripted), true).status,
            GateStatus::Blocked
        );
        let mut invalid = evidence(EvidenceKind::RealControlledModel);
        invalid.cases[0].final_task_passes = 1;
        assert_eq!(
            evaluate_candidate(&invalid, true).verdict,
            Some(FinalVerdict::Defer)
        );
    }

    #[test]
    fn exact_candidate_gate_boundaries_and_security_are_enforced() {
        let mut passing = evidence(EvidenceKind::RealControlledModel);
        for (index, case) in passing
            .cases
            .iter_mut()
            .filter(|case| case.positive)
            .enumerate()
        {
            if index == 0 {
                case.selection_passes = 2;
                case.argument_validity_passes = 2;
                case.final_task_passes = 2;
            }
        }
        for (index, case) in passing
            .cases
            .iter_mut()
            .filter(|case| !case.positive)
            .enumerate()
        {
            if index == 0 {
                case.final_task_passes = 2;
            }
        }
        let passing_result = evaluate_candidate(&passing, true);
        assert_eq!(passing_result.verdict, None);
        assert!(passing_result.provisional_admit);
        passing.cases[0].scoreable_runs = 1;
        passing.cases[0].selection_passes = 1;
        passing.cases[0].argument_validity_passes = 1;
        passing.cases[0].runtime_outcome_passes = 1;
        passing.cases[0].final_task_passes = 1;
        assert_eq!(
            evaluate_candidate(&passing, true).verdict,
            Some(FinalVerdict::Defer)
        );
        let mut secure = evidence(EvidenceKind::RealControlledModel);
        secure.security_violations = 1;
        assert_eq!(
            evaluate_candidate(&secure, true).verdict,
            Some(FinalVerdict::Defer)
        );
        assert_eq!(
            evaluate_candidate(&evidence(EvidenceKind::RealControlledModel), false).verdict,
            Some(FinalVerdict::Reject)
        );
    }

    #[test]
    fn isolation_ranking_is_fixed_and_single_candidate() {
        let summaries = [
            CandidateQualitySummary {
                candidate: AdmissionCandidate::Rename,
                negative_candidate_misuse: 1,
                positive_final_task_passes: 8,
            },
            CandidateQualitySummary {
                candidate: AdmissionCandidate::Mkdir,
                negative_candidate_misuse: 1,
                positive_final_task_passes: 7,
            },
            CandidateQualitySummary {
                candidate: AdmissionCandidate::Delete,
                negative_candidate_misuse: 2,
                positive_final_task_passes: 9,
            },
            CandidateQualitySummary {
                candidate: AdmissionCandidate::Copy,
                negative_candidate_misuse: 2,
                positive_final_task_passes: 9,
            },
        ];
        // Same misuse and completion tie uses fixed candidate order: Copy is
        // before Delete, so Copy is the one removed.
        assert_eq!(
            isolation_remove_worst(&summaries).unwrap(),
            AdmissionCandidate::Copy
        );
        assert!(isolation_remove_worst(&[]).is_err());

        let mut state = IsolationState::new(summaries.to_vec()).unwrap();
        assert_eq!(state.phase(), IsolationPhase::Fresh);
        let failed = RegressionGateResult {
            status: GateStatus::Fail,
            comparison: RegressionComparison {
                pass: false,
                inconclusive: false,
                failures: vec!["regression".into()],
            },
            reasons: vec!["regression".into()],
        };
        state.begin_initial_failure(&failed).unwrap();
        assert_eq!(state.remove_worst().unwrap(), AdmissionCandidate::Copy);
        assert_eq!(state.phase(), IsolationPhase::CandidateRemoved);
        assert!(state.remove_worst().is_err());
        state.finalize_rerun(&failed).unwrap();
        assert_eq!(state.phase(), IsolationPhase::RerunFinalized);
        assert!(state.finalize_rerun(&failed).is_err());
        assert_eq!(state.removed_candidate(), Some(AdmissionCandidate::Copy));
        assert_eq!(state.remaining_candidates().len(), summaries.len() - 1);
    }

    fn metrics() -> AggregateMetrics {
        let metric = Metric {
            numerator: 72,
            denominator: 72,
        };
        AggregateMetrics {
            total_runs: 72,
            scoreable_runs: 72,
            selection: metric,
            argument_validity: metric,
            runtime_outcome_match: metric,
            task_completion: metric,
            unnecessary_calls: Metric {
                numerator: 0,
                denominator: 72,
            },
            cases: (0..24)
                .map(|index| CaseMetrics {
                    case_id: format!("case-{index}"),
                    category: format!("category-{}", index / 4),
                    total_runs: 3,
                    scoreable_runs: 3,
                    selection_passes: 3,
                    task_completion_passes: 3,
                })
                .collect(),
            categories: (0..6)
                .map(|index| CategoryMetrics {
                    category: format!("category-{index}"),
                    total_runs: 12,
                    scoreable_runs: 12,
                    selection: Metric {
                        numerator: 12,
                        denominator: 12,
                    },
                    argument_validity: Metric {
                        numerator: 12,
                        denominator: 12,
                    },
                    runtime_outcome_match: Metric {
                        numerator: 12,
                        denominator: 12,
                    },
                    task_completion: Metric {
                        numerator: 12,
                        denominator: 12,
                    },
                    unnecessary_calls: Metric {
                        numerator: 0,
                        denominator: 12,
                    },
                })
                .collect(),
            ..Default::default()
        }
    }

    fn structural_reject_set() -> DecisionSetV1 {
        let hash = "a".repeat(64);
        let expected_case_ids = (0..6)
            .map(|index| format!("candidate-case-{index}"))
            .collect::<Vec<_>>();
        let candidates = AdmissionCandidate::ALL
            .iter()
            .copied()
            .map(|candidate| FinalDecisionRecord {
                candidate,
                verdict: FinalVerdict::Reject,
                reason_code: "STRUCTURAL_PRODUCT_OR_SAFETY_FAILURE".into(),
                candidate_schema_hash: hash.clone(),
                control_schema_hash: hash.clone(),
                admission_dataset_hash: hash.clone(),
                evidence_hashes: Vec::new(),
                expected_case_ids: expected_case_ids.clone(),
                security_violation_count: 0,
                evidence_kind: EvidenceKind::Scripted,
                candidate_gate_passed: false,
                positive_task_counts: MetricCounts {
                    numerator: 0,
                    denominator: 0,
                },
                negative_sentinel_counts: MetricCounts {
                    numerator: 0,
                    denominator: 0,
                },
                selection_counts: MetricCounts {
                    numerator: 0,
                    denominator: 0,
                },
                argument_validity_counts: MetricCounts {
                    numerator: 0,
                    denominator: 0,
                },
                runtime_outcome_counts: MetricCounts {
                    numerator: 0,
                    denominator: 0,
                },
                final_task_counts: MetricCounts {
                    numerator: 0,
                    denominator: 0,
                },
                overlap_finding: "deterministic overlap".into(),
                overlap_verified: false,
                security_finding: "deterministic safety failure".into(),
                regression_finding: "not applicable".into(),
            })
            .collect();
        let candidate_gate_results = AdmissionCandidate::ALL
            .iter()
            .copied()
            .map(|candidate| CandidateGateResult {
                candidate,
                status: GateStatus::Fail,
                verdict: Some(FinalVerdict::Reject),
                provisional_admit: false,
                evidence_hashes: Vec::new(),
                expected_case_ids: expected_case_ids.clone(),
                admission_dataset_hash: hash.clone(),
                reasons: vec!["STRUCTURAL_PRODUCT_OR_SAFETY_FAILURE".into()],
                scoreable_runs: 0,
                positive_selection_passes: 0,
                positive_argument_validity_passes: 0,
                positive_runtime_outcome_passes: 0,
                positive_final_task_passes: 0,
                negative_final_task_passes: 0,
                negative_candidate_misuse: 0,
                security_violations: 0,
            })
            .collect();
        let final_regression = RegressionGateResult {
            status: GateStatus::Pass,
            comparison: RegressionComparison {
                pass: true,
                inconclusive: false,
                failures: Vec::new(),
            },
            reasons: vec!["control regression passed".into()],
        };
        let blocked = RegressionGateResult {
            status: GateStatus::Blocked,
            comparison: RegressionComparison::default(),
            reasons: vec!["provider drift not needed for deterministic reject".into()],
        };
        let structural_matrix = super::super::m09::structural_matrix().unwrap();
        let structural_rows = structural_matrix.rows;
        let structural_matrix_hash = structural_matrix.matrix_hash;
        DecisionSetV1 {
            schema_version: 1,
            decision_set_id: "m09-file-tool-admission-v1".into(),
            candidates,
            final_regression,
            accepted_m05_handoff_hash: hash.clone(),
            model_adapter: String::new(),
            requested_model: String::new(),
            resolved_model: String::new(),
            provider_drift: blocked,
            candidate_gate_results,
            structural_rows,
            structural_matrix_hash,
        }
    }

    #[test]
    fn deterministic_reject_set_does_not_require_model_metadata() {
        // The canonical matrix must not be replaced by caller-supplied rows
        // to manufacture a structural REJECT.
        assert!(serialize_decision_set(&structural_reject_set()).is_err());
        let mut missing_handoff = structural_reject_set();
        missing_handoff.accepted_m05_handoff_hash.clear();
        assert!(serialize_decision_set(&missing_handoff).is_err());
    }

    #[test]
    fn contradictory_regression_pass_and_gate_count_or_hash_mismatch_are_blocked() {
        let mut set = structural_reject_set();
        let hash = "a".repeat(64);
        let gate = &mut set.candidate_gate_results[0];
        gate.status = GateStatus::Pass;
        gate.verdict = None;
        gate.provisional_admit = true;
        gate.evidence_hashes = vec![hash.clone()];
        gate.scoreable_runs = 18;
        gate.positive_selection_passes = 8;
        gate.positive_argument_validity_passes = 8;
        gate.positive_final_task_passes = 8;
        gate.negative_final_task_passes = 8;
        let record = &mut set.candidates[0];
        record.verdict = FinalVerdict::Admit;
        record.evidence_kind = EvidenceKind::RealControlledModel;
        record.evidence_hashes = vec![hash.clone()];
        record.candidate_gate_passed = true;
        record.positive_task_counts = MetricCounts {
            numerator: 8,
            denominator: 9,
        };
        record.negative_sentinel_counts = MetricCounts {
            numerator: 8,
            denominator: 9,
        };
        record.selection_counts = MetricCounts {
            numerator: 8,
            denominator: 9,
        };
        record.argument_validity_counts = MetricCounts {
            numerator: 8,
            denominator: 9,
        };
        record.runtime_outcome_counts = MetricCounts {
            numerator: 18,
            denominator: 18,
        };
        record.final_task_counts = MetricCounts {
            numerator: 16,
            denominator: 18,
        };
        set.accepted_m05_handoff_hash = hash.clone();
        set.model_adapter = "openai-responses-v1".into();
        set.requested_model = "model".into();
        set.resolved_model = "model".into();
        set.final_regression.status = GateStatus::Pass;
        set.provider_drift.status = GateStatus::Pass;
        assert!(serialize_decision_set(&set).is_err());

        // Restore a genuine PASS comparison, then break only the evidence
        // binding. The serializer must still reject it.
        set.final_regression.comparison.pass = true;
        set.provider_drift.comparison.pass = true;
        set.candidates[0].evidence_hashes = vec!["b".repeat(64)];
        assert!(serialize_decision_set(&set).is_err());
        set.candidates[0].evidence_hashes = vec![hash];
        set.candidates[0].selection_counts.numerator = 7;
        assert!(serialize_decision_set(&set).is_err());

        let mut mismatched_ids = structural_reject_set();
        mismatched_ids.candidates[0].expected_case_ids[0] = "wrong-case".into();
        assert!(serialize_decision_set(&mismatched_ids).is_err());
        let mut mismatched_matrix = structural_reject_set();
        mismatched_matrix.structural_matrix_hash = "b".repeat(64);
        assert!(serialize_decision_set(&mismatched_matrix).is_err());
    }

    #[test]
    fn final_regression_reuses_frozen_comparator_and_provider_drift_is_inconclusive() {
        let baseline = metrics();
        assert_eq!(
            final_regression(&baseline, &baseline).status,
            GateStatus::Pass
        );
        let mut invalid = baseline.clone();
        invalid.scoreable_runs = 69;
        assert_eq!(
            final_regression(&baseline, &invalid).status,
            GateStatus::Inconclusive
        );
        let mut drift = baseline.clone();
        drift.selection.numerator = 60;
        assert_eq!(
            provider_drift(&baseline, &drift).status,
            GateStatus::Inconclusive
        );
    }
}
