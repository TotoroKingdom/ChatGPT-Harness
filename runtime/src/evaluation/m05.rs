#![allow(dead_code)]

use super::adapter::{scripted_call, ModelAdapter, ScriptedAdapter, ScriptedTurn};
use super::agent_loop::{execute_case, CaseExecution};
use super::aggregate::{
    compare_m05, compare_provider_drift, AggregateMetrics, BaselineVerdict, RegressionComparison,
};
use super::case::{
    ApprovalPlan, ArgumentConstraint, EvalCaseV1, EvalCategory, FinalResponseAssertion,
    PolicyDefaults, PolicyExpectation, PolicyExpectationValue, ResponseMatchKind, SelectionOracle,
    SelectionStep, SelectionStrategy, StepRepeat,
};
use super::dataset::{DatasetFileV1, EvalDataset};
use super::fixture::{fixture_tree_hash, safe_relative};
use super::manifest::{canonical_hash, sha256_text, SCORER_VERSION};
use super::mcp_client::{tool_catalog_hash, tool_schema_map, EvalRuntimeHarness, McpClient};
use super::profile::EvalProfile;
use super::{EvalError, EvalResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const ADMISSION_SET_ID: &str = "m05-grep-admission-v1";
pub(crate) const ADMISSION_SET_VERSION: u32 = 1;
pub(crate) const ADMISSION_CASE_COUNT: usize = 4;
pub(crate) const ADMISSION_REPETITIONS: u32 = 3;
pub(crate) const ADMISSION_LOGICAL_RUNS: usize = 12;
pub(crate) const ACCEPTED_BASELINE_SOURCE: &str = "accepted-m04-formal-run";

#[derive(Clone, Debug)]
pub(crate) struct AdmissionIdentity {
    pub(crate) tool_schema_hash: String,
    pub(crate) tool_schemas: std::collections::BTreeMap<String, serde_json::Value>,
}

pub(crate) async fn admission_identity(
    dataset: &AdmissionDataset,
    binary: &Path,
) -> EvalResult<AdmissionIdentity> {
    let case = dataset
        .cases
        .first()
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "admission set is empty"))?;
    let (eval_case, synthetic) = synthetic_case_dataset(dataset, case)?;
    let harness = EvalRuntimeHarness::start_with_profile(
        binary,
        &eval_case,
        &synthetic.fixture_path(&eval_case)?,
        EvalProfile::M05GrepCandidate,
    )
    .await?;
    let mut mcp = McpClient::connect(&harness.endpoint_url).await?;
    let tools = mcp.list_tools().await?;
    EvalProfile::M05GrepCandidate.validate_catalog(&tools)?;
    let tool_schema_hash = tool_catalog_hash(&tools)?;
    let tool_schemas = tool_schema_map(&tools)?;
    harness.shutdown().await?;
    Ok(AdmissionIdentity {
        tool_schema_hash,
        tool_schemas,
    })
}

pub(crate) async fn execute_admission_case(
    dataset: &AdmissionDataset,
    identity: &AdmissionIdentity,
    binary: &Path,
    case: &AdmissionCaseV1,
    repetition: u32,
    logical_run_index: usize,
    adapter: Box<dyn ModelAdapter>,
) -> EvalResult<super::aggregate::EvalRunResult> {
    let (eval_case, synthetic) = synthetic_case_dataset(dataset, case)?;
    Ok(execute_case(
        CaseExecution {
            dataset: &synthetic,
            case: &eval_case,
            runtime_binary: binary,
            base_tool_schema_hash: identity.tool_schema_hash.clone(),
            base_tool_schemas: identity.tool_schemas.clone(),
            runner_commit: super::manifest::env_commit("M05_EVAL_RUNNER_COMMIT"),
            runtime_commit: super::manifest::env_commit("M05_EVAL_RUNTIME_COMMIT"),
            logical_run_index,
            repetition,
            cleanup_failure_injection: false,
            profile: EvalProfile::M05GrepCandidate,
        },
        adapter,
    )
    .await)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AdmissionCategory {
    Literal,
    Regex,
    ScopeFilter,
    TruncationFollowUp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionDatasetFile {
    pub(crate) set_id: String,
    pub(crate) set_version: u32,
    pub(crate) repetitions: u32,
    pub(crate) system_prompt_ref: String,
    pub(crate) case_files: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionCaseV1 {
    pub(crate) case_id: String,
    pub(crate) case_version: u32,
    pub(crate) category: AdmissionCategory,
    pub(crate) prompt: String,
    pub(crate) fixture_ref: String,
    pub(crate) fixture_hash: String,
    pub(crate) expected_final_response_contains: String,
    #[serde(default)]
    pub(crate) max_turns: Option<u32>,
    #[serde(default)]
    pub(crate) max_tool_calls: Option<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct AdmissionDataset {
    pub(crate) root: PathBuf,
    pub(crate) dataset_file: AdmissionDatasetFile,
    pub(crate) cases: Vec<AdmissionCaseV1>,
    pub(crate) system_prompt: String,
    pub(crate) dataset_hash: String,
}

impl AdmissionDataset {
    pub(crate) fn load(root: &Path) -> EvalResult<Self> {
        let raw = fs::read_to_string(root.join("dataset.json")).map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not read M05 admission dataset: {error}"),
            )
        })?;
        let dataset_file: AdmissionDatasetFile = serde_json::from_str(&raw).map_err(|error| {
            EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("M05 admission dataset is invalid: {error}"),
            )
        })?;
        if dataset_file.set_id != ADMISSION_SET_ID
            || dataset_file.set_version != ADMISSION_SET_VERSION
            || dataset_file.repetitions != ADMISSION_REPETITIONS
            || dataset_file.case_files.len() != ADMISSION_CASE_COUNT
        {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "M05 admission dataset identity or shape drifted",
            ));
        }
        let system_prompt =
            fs::read_to_string(root.join(safe_relative(&dataset_file.system_prompt_ref)?))
                .map_err(|error| {
                    EvalError::new(
                        "INFRA_ERROR",
                        format!("could not read M05 admission prompt: {error}"),
                    )
                })?;
        if system_prompt.trim().is_empty() {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "M05 admission system prompt is empty",
            ));
        }
        let fixture_root = root.join("workspaces");
        let mut cases = Vec::new();
        let mut ids = BTreeSet::new();
        for case_file in &dataset_file.case_files {
            let value =
                fs::read_to_string(root.join(safe_relative(case_file)?)).map_err(|error| {
                    EvalError::new(
                        "INFRA_ERROR",
                        format!("could not read M05 admission case: {error}"),
                    )
                })?;
            let case: AdmissionCaseV1 = serde_json::from_str(&value).map_err(|error| {
                EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    format!("M05 admission case is invalid: {error}"),
                )
            })?;
            if case.case_version != 1
                || case.prompt.trim().is_empty()
                || case.expected_final_response_contains.trim().is_empty()
                || !ids.insert(case.case_id.clone())
            {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    "M05 admission case identity is invalid",
                ));
            }
            let prompt = case.prompt.to_ascii_lowercase();
            for tool in ["grep", "glob", "read_file", "bash"] {
                if prompt.contains(tool) {
                    return Err(EvalError::new(
                        "HARNESS_CONTRACT_ERROR",
                        format!("M05 admission prompt names forbidden tool '{tool}'"),
                    ));
                }
            }
            let fixture = fixture_root.join(safe_relative(&case.fixture_ref)?);
            let actual_fixture_hash = if fixture.is_dir() {
                Some(fixture_tree_hash(&fixture)?)
            } else {
                None
            };
            if actual_fixture_hash.as_deref() != Some(case.fixture_hash.as_str()) {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    format!(
                        "M05 admission fixture drifted for {} (expected {}, actual {:?})",
                        case.case_id, case.fixture_hash, actual_fixture_hash
                    ),
                ));
            }
            cases.push(case);
        }
        cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        let categories = cases
            .iter()
            .map(|case| case.category)
            .collect::<BTreeSet<_>>();
        if categories.len() != ADMISSION_CASE_COUNT {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "M05 admission set must cover four distinct categories",
            ));
        }
        let dataset_hash = canonical_hash(&json!({
            "setId": dataset_file.set_id,
            "setVersion": dataset_file.set_version,
            "repetitions": dataset_file.repetitions,
            "systemPromptHash": super::manifest::sha256_text(&system_prompt),
            "cases": cases,
        }))?;
        Ok(Self {
            root: root.to_path_buf(),
            dataset_file,
            cases,
            system_prompt,
            dataset_hash,
        })
    }

    pub(crate) fn fixture_path(&self, case: &AdmissionCaseV1) -> EvalResult<PathBuf> {
        Ok(self
            .root
            .join("workspaces")
            .join(safe_relative(&case.fixture_ref)?))
    }
}

fn synthetic_case_dataset(
    dataset: &AdmissionDataset,
    case: &AdmissionCaseV1,
) -> EvalResult<(EvalCaseV1, EvalDataset)> {
    let eval_case = EvalCaseV1 {
        case_id: case.case_id.clone(),
        case_version: 1,
        category: EvalCategory::ContentSearch,
        prompt: case.prompt.clone(),
        fixture_ref: case.fixture_ref.clone(),
        fixture_hash: case.fixture_hash.clone(),
        preconditions: Vec::new(),
        selection_oracle: SelectionOracle {
            strategies: vec![SelectionStrategy {
                id: "admission-observe".into(),
                steps: vec![SelectionStep {
                    tool: "read_file".into(),
                    optional: true,
                    repeat: StepRepeat::One,
                }],
            }],
        },
        argument_constraints: Vec::<ArgumentConstraint>::new(),
        expected_runtime_outcomes: Vec::new(),
        policy_expectation: PolicyExpectation {
            defaults: PolicyDefaults {
                read_file: PolicyExpectationValue::Allow,
                glob: PolicyExpectationValue::Allow,
                write: PolicyExpectationValue::Allow,
                edit: PolicyExpectationValue::Allow,
                bash: PolicyExpectationValue::Allow,
            },
            deny_tools: Vec::new(),
        },
        approval_plan: ApprovalPlan::None,
        final_state_assertions: Vec::new(),
        final_response_assertion: Some(FinalResponseAssertion {
            kind: ResponseMatchKind::Contains,
            text: case.expected_final_response_contains.clone(),
        }),
        max_turns: case.max_turns,
        max_tool_calls: case.max_tool_calls,
        tags: vec!["m05-grep-admission".into()],
    };
    let synthetic = EvalDataset {
        root: dataset.root.clone(),
        dataset_file: DatasetFileV1 {
            dataset_id: ADMISSION_SET_ID.into(),
            dataset_version: 1,
            repetitions: ADMISSION_REPETITIONS,
            system_prompt_ref: "system_prompt.txt".into(),
            case_files: vec![format!("cases/{}.json", case.case_id)],
        },
        cases: vec![eval_case.clone()],
        dataset_hash: dataset.dataset_hash.clone(),
        system_prompt: dataset.system_prompt.clone(),
        system_prompt_hash: sha256_text(&dataset.system_prompt),
        scorer_hash: sha256_text(SCORER_VERSION),
    };
    Ok((eval_case, synthetic))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionRunRecord {
    pub(crate) case_id: String,
    pub(crate) repetition: u32,
    pub(crate) scoreable: bool,
    pub(crate) security_violation: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdmissionValidity {
    pub(crate) verdict: Option<BaselineVerdict>,
    pub(crate) reasons: Vec<String>,
}

pub(crate) fn validate_admission_runs(
    cases: &[AdmissionCaseV1],
    runs: &[AdmissionRunRecord],
) -> AdmissionValidity {
    let mut validity = AdmissionValidity::default();
    if runs.len() != ADMISSION_LOGICAL_RUNS {
        validity.reasons.push("admission runs must equal 12".into());
    }
    if runs.iter().filter(|run| run.scoreable).count() < 11 {
        validity
            .reasons
            .push("admission scoreable runs must be >= 11".into());
    }
    if runs.iter().any(|run| run.security_violation) {
        validity
            .reasons
            .push("admission security violations must be zero".into());
    }
    for case in cases {
        let case_runs = runs.iter().filter(|run| run.case_id == case.case_id);
        let total = case_runs.clone().count();
        if total != ADMISSION_REPETITIONS as usize {
            validity.reasons.push(format!(
                "admission case {} must have exactly 3 runs",
                case.case_id
            ));
        }
        if case_runs.filter(|run| run.scoreable).count() < 2 {
            validity.reasons.push(format!(
                "admission case {} has fewer than 2/3 scoreable runs",
                case.case_id
            ));
        }
    }
    if runs
        .iter()
        .any(|run| !cases.iter().any(|case| case.case_id == run.case_id))
    {
        validity
            .reasons
            .push("admission run references an unknown case".into());
    }
    validity.verdict = Some(if validity.reasons.is_empty() {
        BaselineVerdict::Valid
    } else if runs.iter().any(|run| run.security_violation) {
        BaselineVerdict::Fail
    } else {
        BaselineVerdict::Inconclusive
    });
    validity
}

pub(crate) fn scripted_admission_adapter(case: &AdmissionCaseV1) -> ScriptedAdapter {
    let arguments = match case.category {
        AdmissionCategory::Literal => {
            json!({"query": "TEST_ORCHID_71"})
        }
        AdmissionCategory::Regex => {
            json!({"query": "req=(4|5)\\d\\d", "mode": "regex"})
        }
        AdmissionCategory::ScopeFilter => {
            json!({
                "query": "NEEDLE=scope",
                "scope": "src",
                "include": ["src/**/*.rs"],
                "exclude": ["src/**/*.tmp"]
            })
        }
        AdmissionCategory::TruncationFollowUp => {
            json!({"query": "ROW_", "maxResults": 5})
        }
    };
    ScriptedAdapter::new(vec![
        ScriptedTurn {
            tool_calls: vec![scripted_call(
                format!("admission-{}", case.case_id),
                "grep",
                arguments,
            )],
            final_text: None,
        },
        ScriptedTurn {
            tool_calls: Vec::new(),
            final_text: Some(format!(
                "{} {}",
                case.expected_final_response_contains, "completed"
            )),
        },
    ])
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AcceptedM04BaselineV1 {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) baseline_id: String,
    pub(crate) candidate_commit: String,
    pub(crate) runtime_binary_hashes: Vec<String>,
    pub(crate) dataset_hash: String,
    pub(crate) system_prompt_hash: String,
    pub(crate) scorer_hash: String,
    pub(crate) eval_profile_hash: String,
    pub(crate) tool_schema_hash: String,
    pub(crate) adapter_id: String,
    pub(crate) adapter_version: String,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: String,
    pub(crate) validity: BaselineVerdict,
    pub(crate) metrics: AggregateMetrics,
}

impl AcceptedM04BaselineV1 {
    pub(crate) fn load(path: &Path) -> EvalResult<Self> {
        let raw = fs::read_to_string(path).map_err(|error| {
            EvalError::new(
                "BLOCKED",
                format!("accepted M04 baseline is unavailable: {error}"),
            )
        })?;
        let baseline: Self = serde_json::from_str(&raw).map_err(|error| {
            EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("accepted M04 baseline is invalid: {error}"),
            )
        })?;
        baseline.validate()?;
        Ok(baseline)
    }

    pub(crate) fn validate(&self) -> EvalResult<()> {
        let required = [
            &self.baseline_id,
            &self.candidate_commit,
            &self.dataset_hash,
            &self.system_prompt_hash,
            &self.scorer_hash,
            &self.eval_profile_hash,
            &self.tool_schema_hash,
            &self.adapter_id,
            &self.adapter_version,
            &self.requested_model,
            &self.resolved_model,
        ];
        if self.schema_version != 1
            || self.source != ACCEPTED_BASELINE_SOURCE
            || required.iter().any(|value| value.trim().is_empty())
            || self
                .runtime_binary_hashes
                .iter()
                .any(|value| value.trim().is_empty())
            || self.runtime_binary_hashes.is_empty()
        {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "accepted M04 baseline identity is incomplete or non-formal",
            ));
        }
        if self.validity != BaselineVerdict::Valid {
            return Err(EvalError::new(
                "BLOCKED",
                "accepted M04 baseline validity is not VALID",
            ));
        }
        if self.metrics.total_runs != 72
            || self.metrics.scoreable_runs < 70
            || self.metrics.security_violation_runs != 0
            || self.metrics.cases.len() != 24
            || self.metrics.categories.len() != 6
        {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "accepted M04 baseline aggregate shape is not the frozen 72-run result",
            ));
        }
        Ok(())
    }

    pub(crate) fn compare_candidate(&self, candidate: &AggregateMetrics) -> RegressionComparison {
        compare_m05(&self.metrics, candidate)
    }

    pub(crate) fn compare_paired_control(
        &self,
        paired_control: &AggregateMetrics,
    ) -> RegressionComparison {
        compare_provider_drift(&self.metrics, paired_control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::aggregate::{CaseMetrics, CategoryMetrics, Metric};

    fn metrics() -> AggregateMetrics {
        AggregateMetrics {
            total_runs: 72,
            scoreable_runs: 72,
            selection: Metric {
                numerator: 72,
                denominator: 72,
            },
            argument_validity: Metric {
                numerator: 72,
                denominator: 72,
            },
            runtime_outcome_match: Metric {
                numerator: 72,
                denominator: 72,
            },
            task_completion: Metric {
                numerator: 72,
                denominator: 72,
            },
            unnecessary_calls: Metric {
                numerator: 0,
                denominator: 72,
            },
            cases: (1..=24)
                .map(|index| CaseMetrics {
                    case_id: format!("m04-c{index:02}"),
                    category: format!("category-{}", (index - 1) / 4),
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
            ..AggregateMetrics::default()
        }
    }

    fn baseline() -> AcceptedM04BaselineV1 {
        AcceptedM04BaselineV1 {
            schema_version: 1,
            source: ACCEPTED_BASELINE_SOURCE.into(),
            baseline_id: "m04-accepted".into(),
            candidate_commit: "d1051cb".into(),
            runtime_binary_hashes: vec!["binary".into()],
            dataset_hash: "dataset".into(),
            system_prompt_hash: "prompt".into(),
            scorer_hash: "scorer".into(),
            eval_profile_hash: "profile".into(),
            tool_schema_hash: "tools".into(),
            adapter_id: "openai-responses-v1".into(),
            adapter_version: "1".into(),
            requested_model: "gpt-6.1-terra".into(),
            resolved_model: "gpt-6.1-terra".into(),
            validity: BaselineVerdict::Valid,
            metrics: metrics(),
        }
    }

    #[test]
    fn accepted_baseline_rejects_source_synthesis_and_incomplete_identity() {
        let mut value = baseline();
        assert!(value.validate().is_ok());
        value.source = "source-constants".into();
        assert_eq!(value.validate().unwrap_err().code, "HARNESS_CONTRACT_ERROR");
        let mut value = baseline();
        value.runtime_binary_hashes.clear();
        assert_eq!(value.validate().unwrap_err().code, "HARNESS_CONTRACT_ERROR");
        let mut value = baseline();
        value.validity = BaselineVerdict::Inconclusive;
        assert_eq!(value.validate().unwrap_err().code, "BLOCKED");
    }

    #[test]
    fn accepted_baseline_uses_existing_comparators_without_threshold_copies() {
        let baseline = baseline();
        let candidate = baseline.metrics.clone();
        assert!(baseline.compare_candidate(&candidate).pass);
        let mut regressed = baseline.metrics.clone();
        regressed.task_completion.numerator = 69;
        assert!(!baseline.compare_candidate(&regressed).pass);
        let mut drift = baseline.metrics.clone();
        drift.selection.numerator = 68;
        assert!(baseline.compare_paired_control(&drift).inconclusive);
    }

    #[test]
    fn admission_validity_separates_scoreability_from_quality_and_security() {
        let cases = (1..=4)
            .map(|index| AdmissionCaseV1 {
                case_id: format!("m05-a{index}"),
                case_version: 1,
                category: match index {
                    1 => AdmissionCategory::Literal,
                    2 => AdmissionCategory::Regex,
                    3 => AdmissionCategory::ScopeFilter,
                    _ => AdmissionCategory::TruncationFollowUp,
                },
                prompt: "neutral".into(),
                fixture_ref: "fixture".into(),
                fixture_hash: "hash".into(),
                expected_final_response_contains: "OK".into(),
                max_turns: None,
                max_tool_calls: None,
            })
            .collect::<Vec<_>>();
        let mut runs = Vec::new();
        for case in &cases {
            for repetition in 1..=3 {
                runs.push(AdmissionRunRecord {
                    case_id: case.case_id.clone(),
                    repetition,
                    scoreable: !(case.case_id == "m05-a1" && repetition == 3),
                    security_violation: false,
                });
            }
        }
        assert_eq!(
            validate_admission_runs(&cases, &runs).verdict,
            Some(BaselineVerdict::Valid)
        );
        runs[0].security_violation = true;
        assert_eq!(
            validate_admission_runs(&cases, &runs).verdict,
            Some(BaselineVerdict::Fail)
        );
    }

    #[test]
    fn supplemental_fixture_matches_the_frozen_count_and_neutral_prompts() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("v4")
            .join("m05-eval");
        let dataset = AdmissionDataset::load(&root).unwrap();
        assert_eq!(dataset.dataset_file.set_id, ADMISSION_SET_ID);
        assert_eq!(dataset.cases.len(), 4);
        assert_eq!(dataset.dataset_file.repetitions, 3);
        assert!(!dataset.dataset_hash.is_empty());
    }

    #[test]
    fn m04_dataset_prompt_scorer_and_case_fixtures_remain_byte_identical() {
        const DATASET_HASH: &str =
            "abb1f5988017fcdd11554b750be49fbdbf1c7c6dd69c2cb2bcc2d47443dfff62";
        const SCORER_HASH: &str =
            "69dafd468127f84218425506bdfdecc806e12d6c05806d0fa96b05ccaaff8e92";
        const SYSTEM_PROMPT_HASH: &str =
            "c6790a5a08d0cd849da645501f3adc28abe238d20388e8f939f5de16dc91c8df";
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("v4")
            .join("m04-eval");
        let dataset = super::super::dataset::EvalDataset::load(&root).unwrap();
        assert_eq!(dataset.dataset_hash, DATASET_HASH);
        assert_eq!(dataset.scorer_hash, SCORER_HASH);
        assert_eq!(dataset.system_prompt_hash, SYSTEM_PROMPT_HASH);
        assert_eq!(
            super::super::BASELINE_TOOLS,
            ["read_file", "glob", "write", "edit"]
        );
        let fixture_identity = dataset
            .cases
            .iter()
            .map(|case| json!([case.case_id, case.fixture_hash]))
            .collect::<Vec<_>>();
        assert_eq!(
            canonical_hash(&json!(fixture_identity)).unwrap(),
            "0cff2ba652455ade672e8dd44ee69f8d2554d99c7340a3226be725e3f8571948"
        );
    }
}
