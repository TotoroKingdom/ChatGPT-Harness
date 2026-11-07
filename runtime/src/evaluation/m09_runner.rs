//! Deterministic M09 admission mechanics runner.
//!
//! The runner is intentionally separate from the production MCP service.  It
//! consumes a caller-provided `ScriptedAdapter` turn and scores selection,
//! schema/argument validity, and final workspace state.  It never mutates a
//! workspace and it always marks the result as non-formal; real Model runs and
//! the cumulative M04 regression remain required for any admission decision.

use super::adapter::{
    ConversationItem, ModelAdapter, ModelRequest, ModelToolCall, ModelToolSpec, ScriptedAdapter,
    ScriptedTurn,
};
use super::m09::{
    AdmissionCandidate, AdmissionCaseKind, AdmissionCaseV1, AdmissionDataset,
    ScriptedMechanicsResult,
};
use super::m09_scoring::{
    arguments_match, arguments_match_with_schema, final_state_matches, result_matches,
    runtime_outcome_matches,
};
use super::{EvalError, EvalResult};
use serde_json::json;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MechanicsAggregate {
    pub(crate) total_cases: usize,
    pub(crate) scoreable_cases: usize,
    pub(crate) selection_passes: usize,
    pub(crate) argument_validity_passes: usize,
    pub(crate) final_state_passes: usize,
    pub(crate) runtime_outcome_passes: usize,
    pub(crate) result_passes: usize,
    pub(crate) negative_candidate_misuse: usize,
    pub(crate) formal_admission_eligible: bool,
}

impl MechanicsAggregate {
    pub(crate) fn record(&mut self, result: &ScriptedMechanicsResult) {
        self.total_cases += 1;
        self.scoreable_cases += usize::from(result.scoreable);
        self.selection_passes += usize::from(result.selection_pass);
        self.argument_validity_passes += usize::from(result.argument_validity_pass);
        self.final_state_passes += usize::from(result.final_state_pass);
        self.runtime_outcome_passes += usize::from(result.runtime_outcome_pass);
        self.result_passes += usize::from(result.result_pass);
        self.negative_candidate_misuse += usize::from(result.candidate_misuse);
        // Scripted evidence cannot turn this flag on.  Keep this assertion in
        // the aggregate so a future caller cannot accidentally infer ADMIT.
        self.formal_admission_eligible = false;
    }
}

/// Score one actual ScriptedAdapter turn against an M09 case.
pub(crate) async fn score_scripted_case(
    case: &AdmissionCaseV1,
    observed_call: ModelToolCall,
    workspace: &Path,
) -> EvalResult<ScriptedMechanicsResult> {
    score_scripted_case_with_observation(case, observed_call, workspace, None, None).await
}

/// Variant used by a real preflight/runner which captured the exact
/// five-tool schema from `tools/list`. Existing control calls are BLOCKED when
/// that catalog is absent; a hand-written approximation is never accepted.
pub(crate) async fn score_scripted_case_with_schema(
    case: &AdmissionCaseV1,
    observed_call: ModelToolCall,
    workspace: &Path,
    observed_schema: Option<&serde_json::Value>,
) -> EvalResult<ScriptedMechanicsResult> {
    score_scripted_case_with_observation(case, observed_call, workspace, observed_schema, None)
        .await
}

/// Full mechanics observation path. `observed_result` must be the exact MCP
/// result captured from the same invocation; omitted results remain
/// unscoreable for runtime/result dimensions.
pub(crate) async fn score_scripted_case_with_observation(
    case: &AdmissionCaseV1,
    observed_call: ModelToolCall,
    workspace: &Path,
    observed_schema: Option<&serde_json::Value>,
    observed_result: Option<&serde_json::Value>,
) -> EvalResult<ScriptedMechanicsResult> {
    let observed_name = observed_call.name.clone();
    let parsed = observed_call.parsed_arguments();
    let arguments_valid = if observed_name == case.candidate.as_str() {
        parsed
            .parsed
            .as_ref()
            .map(|arguments| arguments_match(case, &observed_name, arguments))
            .unwrap_or(false)
    } else {
        let Some(schema) = observed_schema else {
            return Err(EvalError::new(
                "BLOCKED",
                "exact M09 control schema is required for scripted argument scoring",
            ));
        };
        parsed
            .parsed
            .as_ref()
            .map(|arguments| arguments_match_with_schema(case, &observed_name, arguments, schema))
            .unwrap_or(false)
    };

    // Route the supplied call through the same adapter interface used by the
    // formal runner.  This proves the mechanics path preserves the exact
    // model-emitted argument string; it does not execute the call.
    let mut adapter = ScriptedAdapter::new(vec![ScriptedTurn {
        tool_calls: vec![observed_call],
        final_text: None,
    }]);
    let request = ModelRequest {
        system_prompt: "m09-scripted-mechanics".into(),
        items: vec![ConversationItem::User {
            text: case.prompt.clone(),
        }],
        tools: vec![ModelToolSpec {
            name: observed_name.clone(),
            description: "evaluation-only scripted tool shape".into(),
            input_schema: if observed_name == case.candidate.as_str() {
                case.candidate
                    .profile()
                    .candidate_schema()
                    .unwrap_or_else(|| json!({}))
            } else {
                observed_schema.cloned().unwrap_or_else(|| json!({}))
            },
        }],
        max_output_tokens: super::MAX_OUTPUT_TOKENS_PER_TURN,
    };
    let turn = adapter.respond(&request).await.map_err(|error| {
        EvalError::new(
            "NON_FORMAL",
            format!("scripted mechanics adapter failed: {error}"),
        )
    })?;
    let emitted = turn
        .tool_calls
        .first()
        .ok_or_else(|| EvalError::new("NON_FORMAL", "scripted adapter emitted no tool call"))?;
    let observed_name = emitted.name.as_str();
    let selection_pass = case
        .expected_allowed_tools
        .iter()
        .any(|expected| expected == observed_name);
    let candidate_misuse = case.kind == AdmissionCaseKind::NegativeSentinel
        && observed_name == case.candidate.as_str();
    let runtime_outcome_pass = observed_result
        .map(|result| runtime_outcome_matches(case, observed_name, result))
        .unwrap_or(false);
    let result_pass = observed_result
        .map(|result| result_matches(case, result))
        .unwrap_or(false);
    let final_state_pass = final_state_matches(case, workspace)?;
    Ok(ScriptedMechanicsResult {
        candidate: case.candidate,
        expected_selection: match case.kind {
            AdmissionCaseKind::Positive => case.candidate.as_str().into(),
            AdmissionCaseKind::NegativeSentinel => "control".into(),
        },
        observed_selection: Some(observed_name.to_owned()),
        selection_pass,
        argument_validity_pass: arguments_valid,
        runtime_outcome_pass,
        result_pass,
        final_state_pass,
        candidate_misuse,
        scoreable: true,
        formal_admission_eligible: false,
    })
}

/// Validate the complete M09 fixture set before any Model call.  This is the
/// only operation the T1 CLI needs from the dataset; it emits hashes and
/// counts without producing a decision set.
pub(crate) fn validate_admission_dataset(root: &Path) -> EvalResult<AdmissionDatasetSummary> {
    let dataset = AdmissionDataset::load(root)?;
    let mut candidates = Vec::new();
    for candidate in AdmissionCandidate::ALL {
        let cases = dataset.candidate_cases(candidate);
        candidates.push(CandidateDatasetSummary {
            candidate,
            case_count: cases.len(),
            positive_count: cases
                .iter()
                .filter(|case| case.kind == AdmissionCaseKind::Positive)
                .count(),
            negative_count: cases
                .iter()
                .filter(|case| case.kind == AdmissionCaseKind::NegativeSentinel)
                .count(),
            profile_id: candidate.profile().id().into(),
            candidate_schema_hash: candidate
                .profile()
                .candidate_schema_hash()?
                .unwrap_or_default(),
        });
    }
    Ok(AdmissionDatasetSummary {
        dataset_id: dataset.manifest.dataset_id,
        dataset_version: dataset.manifest.dataset_version,
        repetitions: dataset.manifest.repetitions,
        case_count: dataset.cases.len(),
        logical_runs: dataset.cases.len() * dataset.manifest.repetitions as usize,
        dataset_hash: dataset.dataset_hash,
        system_prompt_hash: dataset.system_prompt_hash,
        candidates,
        formal_admission_eligible: false,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdmissionDatasetSummary {
    pub(crate) dataset_id: String,
    pub(crate) dataset_version: u32,
    pub(crate) repetitions: u32,
    pub(crate) case_count: usize,
    pub(crate) logical_runs: usize,
    pub(crate) dataset_hash: String,
    pub(crate) system_prompt_hash: String,
    pub(crate) candidates: Vec<CandidateDatasetSummary>,
    pub(crate) formal_admission_eligible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateDatasetSummary {
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) case_count: usize,
    pub(crate) positive_count: usize,
    pub(crate) negative_count: usize,
    pub(crate) profile_id: String,
    pub(crate) candidate_schema_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dataset_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("v4")
            .join("m09-admission")
    }

    #[tokio::test]
    async fn dataset_summary_is_108_runs_and_never_formal() {
        let summary = validate_admission_dataset(&dataset_root()).unwrap();
        assert_eq!(summary.dataset_id, "m09-file-tool-admission-v1");
        assert_eq!(summary.case_count, 36);
        assert_eq!(summary.logical_runs, 108);
        assert_eq!(summary.candidates.len(), 6);
        assert!(summary.candidates.iter().all(|candidate| {
            candidate.case_count == 6
                && candidate.positive_count == 3
                && candidate.negative_count == 3
                && candidate.candidate_schema_hash.len() == 64
        }));
        assert!(!summary.formal_admission_eligible);
    }

    #[tokio::test]
    async fn scripted_case_preserves_invalid_json_and_never_admits() {
        let dataset = super::super::m09::AdmissionDataset::load(&dataset_root()).unwrap();
        let case = dataset
            .cases
            .iter()
            .find(|case| case.case_id == "mkdir-p1")
            .unwrap();
        let workspace = dataset.fixture_path(case).unwrap();
        let result = score_scripted_case(
            case,
            ModelToolCall {
                id: "scripted-1".into(),
                name: "mkdir".into(),
                arguments: "{not-json".into(),
            },
            &workspace,
        )
        .await
        .unwrap();
        assert!(!result.argument_validity_pass);
        assert!(!result.formal_admission_eligible);
    }
}
