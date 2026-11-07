//! Accepted M05 evidence boundary. Synthetic mechanics never constitute admission.
#![allow(dead_code)]

use super::aggregate::{
    compare_m05, compare_provider_drift, AggregateMetrics, RegressionComparison,
};
use super::{EvalError, EvalResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AcceptedM05Handoff {
    pub schema_version: u32,
    pub source: String,
    pub acceptance_status: String,
    pub candidate_commit: String,
    pub runtime_hash: String,
    pub runner_hash: String,
    pub dataset_hash: String,
    pub scorer_hash: String,
    pub system_prompt_hash: String,
    pub profile_hash: String,
    pub five_tool_schema_hash: String,
    pub adapter: String,
    pub adapter_version: String,
    pub requested_model: String,
    pub resolved_model: String,
    pub validity: String,
    pub metrics: AggregateMetrics,
}

pub(crate) struct ExpectedM05Identity<'a> {
    pub dataset_hash: &'a str,
    pub scorer_hash: &'a str,
    pub system_prompt_hash: &'a str,
    pub profile_hash: &'a str,
    pub five_tool_schema_hash: &'a str,
    pub case_ids: &'a BTreeSet<String>,
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl AcceptedM05Handoff {
    pub(crate) fn load(path: &Path, expected: &ExpectedM05Identity<'_>) -> EvalResult<Self> {
        let bytes = std::fs::read(path)
            .map_err(|_| EvalError::new("BLOCKED", "accepted M05 handoff unavailable"))?;
        let handoff: Self = serde_json::from_slice(&bytes)
            .map_err(|_| EvalError::new("BLOCKED", "accepted M05 handoff malformed"))?;
        handoff.validate(expected)?;
        Ok(handoff)
    }

    pub(crate) fn validate(&self, expected: &ExpectedM05Identity<'_>) -> EvalResult<()> {
        let reject = || EvalError::new("BLOCKED", "accepted M05 handoff invalid or incompatible");
        if self.schema_version != 1
            || self.source != "accepted-m05-quality-evidence"
            || self.acceptance_status != "ACCEPTED/FROZEN"
            || self.validity != "VALID"
            || !hex(&self.candidate_commit, 40)
            || [
                &self.runtime_hash,
                &self.runner_hash,
                &self.dataset_hash,
                &self.scorer_hash,
                &self.system_prompt_hash,
                &self.profile_hash,
                &self.five_tool_schema_hash,
            ]
            .iter()
            .any(|hash| !hex(hash, 64))
            || [
                &self.adapter,
                &self.adapter_version,
                &self.requested_model,
                &self.resolved_model,
            ]
            .iter()
            .any(|value| value.trim().is_empty())
            || self.adapter.to_ascii_lowercase().contains("scripted")
            || self.dataset_hash != expected.dataset_hash
            || self.scorer_hash != expected.scorer_hash
            || self.system_prompt_hash != expected.system_prompt_hash
            || self.profile_hash != expected.profile_hash
            || self.five_tool_schema_hash != expected.five_tool_schema_hash
        {
            return Err(reject());
        }
        let m = &self.metrics;
        if m.unnecessary_calls.denominator == 0
            || m.unnecessary_calls.numerator > m.unnecessary_calls.denominator
            || m.categories.iter().any(|c| {
                c.unnecessary_calls.denominator == 0
                    || c.unnecessary_calls.numerator > c.unnecessary_calls.denominator
            })
            || m.categories
                .iter()
                .map(|c| c.unnecessary_calls.numerator)
                .sum::<u64>()
                != m.unnecessary_calls.numerator
            || m.categories
                .iter()
                .map(|c| c.unnecessary_calls.denominator)
                .sum::<u64>()
                != m.unnecessary_calls.denominator
        {
            return Err(reject());
        }
        let ids: BTreeSet<_> = m.cases.iter().map(|case| case.case_id.clone()).collect();
        let categories: BTreeSet<_> = m
            .categories
            .iter()
            .map(|category| category.category.clone())
            .collect();
        if m.total_runs != 72
            || !(70..=72).contains(&m.scoreable_runs)
            || m.security_violation_runs != 0
            || m.harness_contract_error_runs != 0
            || m.cases.len() != 24
            || ids != *expected.case_ids
            || ids.len() != 24
            || m.categories.len() != 6
            || categories.len() != 6
            || m.cases.iter().any(|case| {
                case.total_runs != 3
                    || !(2..=3).contains(&case.scoreable_runs)
                    || case.selection_passes > case.scoreable_runs
                    || case.task_completion_passes > case.scoreable_runs
                    || !categories.contains(&case.category)
            })
            || m.categories.iter().any(|category| {
                category.total_runs != 12 || !(11..=12).contains(&category.scoreable_runs)
            })
            || m.cases.iter().map(|case| case.scoreable_runs).sum::<u64>() != m.scoreable_runs
            || m.categories
                .iter()
                .map(|category| category.scoreable_runs)
                .sum::<u64>()
                != m.scoreable_runs
        {
            return Err(reject());
        }
        for category in &m.categories {
            let cases: Vec<_> = m
                .cases
                .iter()
                .filter(|case| case.category == category.category)
                .collect();
            if cases.len() != 4
                || cases.iter().map(|case| case.scoreable_runs).sum::<u64>()
                    != category.scoreable_runs
                || category.selection.denominator != category.scoreable_runs
                || category.task_completion.denominator != category.scoreable_runs
                || cases.iter().map(|case| case.selection_passes).sum::<u64>()
                    != category.selection.numerator
                || cases
                    .iter()
                    .map(|case| case.task_completion_passes)
                    .sum::<u64>()
                    != category.task_completion.numerator
            {
                return Err(reject());
            }
        }
        if m.selection.denominator != m.scoreable_runs
            || m.task_completion.denominator != m.scoreable_runs
            || m.cases
                .iter()
                .map(|case| case.selection_passes)
                .sum::<u64>()
                != m.selection.numerator
            || m.cases
                .iter()
                .map(|case| case.task_completion_passes)
                .sum::<u64>()
                != m.task_completion.numerator
            || m.categories
                .iter()
                .map(|c| c.argument_validity.numerator)
                .sum::<u64>()
                != m.argument_validity.numerator
            || m.categories
                .iter()
                .map(|c| c.argument_validity.denominator)
                .sum::<u64>()
                != m.argument_validity.denominator
            || m.categories
                .iter()
                .map(|c| c.runtime_outcome_match.numerator)
                .sum::<u64>()
                != m.runtime_outcome_match.numerator
            || m.categories
                .iter()
                .map(|c| c.runtime_outcome_match.denominator)
                .sum::<u64>()
                != m.runtime_outcome_match.denominator
        {
            return Err(reject());
        }
        for metrics in std::iter::once((
            &m.selection,
            &m.argument_validity,
            &m.runtime_outcome_match,
            &m.task_completion,
        ))
        .chain(m.categories.iter().map(|c| {
            (
                &c.selection,
                &c.argument_validity,
                &c.runtime_outcome_match,
                &c.task_completion,
            )
        })) {
            for metric in [metrics.0, metrics.1, metrics.2, metrics.3] {
                if metric.denominator == 0 || metric.numerator > metric.denominator {
                    return Err(reject());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn compare(&self, candidate: &AggregateMetrics) -> RegressionComparison {
        compare_m05(&self.metrics, candidate)
    }

    pub(crate) fn compare_drift(&self, control: &AggregateMetrics) -> RegressionComparison {
        compare_provider_drift(&self.metrics, control)
    }
}

#[cfg(test)]
mod tests {
    use super::super::aggregate::{CaseMetrics, CategoryMetrics, Metric};
    use super::*;

    fn synthetic_handoff() -> AcceptedM05Handoff {
        let perfect = Metric {
            numerator: 72,
            denominator: 72,
        };
        AcceptedM05Handoff {
            schema_version: 1,
            source: "accepted-m05-quality-evidence".into(),
            acceptance_status: "ACCEPTED/FROZEN".into(),
            candidate_commit: "a".repeat(40),
            runtime_hash: "a".repeat(64),
            runner_hash: "a".repeat(64),
            dataset_hash: "a".repeat(64),
            scorer_hash: "a".repeat(64),
            system_prompt_hash: "a".repeat(64),
            profile_hash: "a".repeat(64),
            five_tool_schema_hash: "a".repeat(64),
            adapter: "synthetic-formal-shape-test-only".into(),
            adapter_version: "1".into(),
            requested_model: "test-only".into(),
            resolved_model: "test-only".into(),
            validity: "VALID".into(),
            metrics: AggregateMetrics {
                total_runs: 72,
                scoreable_runs: 72,
                selection: perfect,
                argument_validity: perfect,
                runtime_outcome_match: perfect,
                task_completion: perfect,
                unnecessary_calls: Metric {
                    numerator: 0,
                    denominator: 72,
                },
                cases: (0..24)
                    .map(|i| CaseMetrics {
                        case_id: format!("case-{i}"),
                        category: format!("category-{}", i / 4),
                        total_runs: 3,
                        scoreable_runs: 3,
                        selection_passes: 3,
                        task_completion_passes: 3,
                    })
                    .collect(),
                categories: (0..6)
                    .map(|i| CategoryMetrics {
                        category: format!("category-{i}"),
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
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn rejects_scripted_identity_hash_drift_duplicates_and_invalid_counts() {
        let baseline = synthetic_handoff();
        let ids = baseline
            .metrics
            .cases
            .iter()
            .map(|c| c.case_id.clone())
            .collect();
        let hash = "a".repeat(64);
        let expected = ExpectedM05Identity {
            dataset_hash: &hash,
            scorer_hash: &hash,
            system_prompt_hash: &hash,
            profile_hash: &hash,
            five_tool_schema_hash: &hash,
            case_ids: &ids,
        };
        assert!(baseline.validate(&expected).is_ok());
        let changes: Vec<Box<dyn Fn(&mut AcceptedM05Handoff)>> = vec![
            Box::new(|h| h.adapter = "scripted".into()),
            Box::new(|h| h.dataset_hash = "b".repeat(64)),
            Box::new(|h| h.metrics.cases[1].case_id = h.metrics.cases[0].case_id.clone()),
            Box::new(|h| h.metrics.cases[0].scoreable_runs = 1),
            Box::new(|h| h.metrics.security_violation_runs = 1),
            Box::new(|h| h.acceptance_status = "DEVELOPMENT_COMPLETE".into()),
            Box::new(|h| h.metrics.argument_validity.numerator = 73),
        ];
        for change in changes {
            let mut invalid = baseline.clone();
            change(&mut invalid);
            assert_eq!(invalid.validate(&expected).unwrap_err().code, "BLOCKED");
        }
    }

    #[test]
    fn missing_evidence_is_blocked_without_leaking_physical_path() {
        let ids = BTreeSet::new();
        let expected = ExpectedM05Identity {
            dataset_hash: "",
            scorer_hash: "",
            system_prompt_hash: "",
            profile_hash: "",
            five_tool_schema_hash: "",
            case_ids: &ids,
        };
        let error = AcceptedM05Handoff::load(Path::new("missing-private-baseline.json"), &expected)
            .unwrap_err();
        assert_eq!(error.code, "BLOCKED");
        assert!(!error.message.contains("private"));
    }
}
