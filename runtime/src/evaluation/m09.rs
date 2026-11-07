//! M09 file-tool admission mechanics.
//!
//! This module owns the versioned, evaluation-only admission dataset and the
//! deterministic structural gate.  It deliberately does not register tools
//! with the production Runtime.  A later phase may consume a final,
//! externally-backed decision set; scripted mechanics in this module can
//! never produce an ADMIT verdict.

use super::fixture::safe_relative;
use super::m09_fixture::{fixture_hash, FixtureSetup};
use super::manifest::{canonical_hash, sha256_file};
use super::profile::EvalProfile;
use super::{EvalError, EvalResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const M09_DATASET_ID: &str = "m09-file-tool-admission-v1";
pub(crate) const M09_DATASET_VERSION: u32 = 1;
pub(crate) const M09_REPETITIONS: u32 = 3;
pub(crate) const M09_CASE_COUNT: usize = 36;
pub(crate) const M09_CASES_PER_CANDIDATE: usize = 6;
pub(crate) const M09_POSITIVE_CASES_PER_CANDIDATE: usize = 3;
pub(crate) const M09_NEGATIVE_CASES_PER_CANDIDATE: usize = 3;
const M09_TYPED_ERROR_CODES: &[&str] = &[
    "APPROVAL_DENIED",
    "APPROVAL_REQUIRED",
    "ATOMIC_FAILURE",
    "CROSS_VOLUME_UNSUPPORTED",
    "DIRECTORY_NOT_EMPTY",
    "FILE_TOO_LARGE",
    "INVALID_ARGUMENTS",
    "IO_FAILURE",
    "NOT_A_DIRECTORY",
    "NOT_A_FILE",
    "POLICY_DENIED",
    "TARGET_CHANGED",
    "TARGET_EXISTS",
    "TARGET_NOT_FOUND",
    "TOOL_DISABLED",
    "UNSUPPORTED_REPARSE",
    "WORKSPACE_VIOLATION",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AdmissionCandidate {
    Mkdir,
    ListDirectory,
    Copy,
    Move,
    Rename,
    Delete,
}

impl AdmissionCandidate {
    pub(crate) const ALL: [Self; 6] = [
        Self::Mkdir,
        Self::ListDirectory,
        Self::Copy,
        Self::Move,
        Self::Rename,
        Self::Delete,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Mkdir => "mkdir",
            Self::ListDirectory => "list_directory",
            Self::Copy => "copy",
            Self::Move => "move",
            Self::Rename => "rename",
            Self::Delete => "delete",
        }
    }

    pub(crate) fn profile(self) -> EvalProfile {
        match self {
            Self::Mkdir => EvalProfile::M09MkdirCandidate,
            Self::ListDirectory => EvalProfile::M09ListDirectoryCandidate,
            Self::Copy => EvalProfile::M09CopyCandidate,
            Self::Move => EvalProfile::M09MoveCandidate,
            Self::Rename => EvalProfile::M09RenameCandidate,
            Self::Delete => EvalProfile::M09DeleteCandidate,
        }
    }
}

impl TryFrom<&str> for AdmissionCandidate {
    type Error = EvalError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "mkdir" => Ok(Self::Mkdir),
            "list_directory" => Ok(Self::ListDirectory),
            "copy" => Ok(Self::Copy),
            "move" => Ok(Self::Move),
            "rename" => Ok(Self::Rename),
            "delete" => Ok(Self::Delete),
            _ => Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("unknown M09 candidate '{value}'"),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AdmissionCaseKind {
    Positive,
    NegativeSentinel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionArgumentConstraint {
    pub(crate) field: String,
    #[serde(default)]
    pub(crate) allowed_values: Vec<String>,
    #[serde(default)]
    pub(crate) path_must_be_workspace_relative: bool,
    #[serde(default)]
    pub(crate) max_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionFinalState {
    pub(crate) path: String,
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionPolicyExpectation {
    pub(crate) tool: String,
    pub(crate) default: String,
    pub(crate) approval: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionSecurityExpectation {
    pub(crate) outside_workspace_mutations: u32,
    pub(crate) raw_physical_paths_in_evidence: bool,
    pub(crate) reparse_traversal: bool,
    pub(crate) overwrite: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionRuntimeOutcome {
    pub(crate) tool: String,
    pub(crate) outcome: String,
    #[serde(default)]
    pub(crate) code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionResultGroundTruth {
    #[serde(default)]
    pub(crate) created: Option<bool>,
    #[serde(default)]
    pub(crate) atomic_commit: Option<bool>,
    #[serde(default)]
    pub(crate) atomic: Option<bool>,
    #[serde(default)]
    pub(crate) deleted: Option<bool>,
    #[serde(default)]
    pub(crate) irreversible: Option<bool>,
    #[serde(default)]
    pub(crate) entry_kind: Option<String>,
    #[serde(default)]
    pub(crate) entry_names: Vec<String>,
    #[serde(default)]
    pub(crate) entry_kinds: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) truncated: Option<bool>,
    #[serde(default)]
    pub(crate) bytes_copied: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionCaseV1 {
    #[serde(default)]
    pub(crate) fixture_setup: FixtureSetup,
    pub(crate) case_id: String,
    pub(crate) case_version: u32,
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) kind: AdmissionCaseKind,
    pub(crate) prompt: String,
    pub(crate) fixture_ref: String,
    pub(crate) fixture_hash: String,
    pub(crate) expected_allowed_tools: Vec<String>,
    pub(crate) argument_constraints: Vec<AdmissionArgumentConstraint>,
    pub(crate) final_state_ground_truth: Vec<AdmissionFinalState>,
    pub(crate) policy_approval: AdmissionPolicyExpectation,
    #[serde(default)]
    pub(crate) expected_runtime_outcomes: Vec<AdmissionRuntimeOutcome>,
    #[serde(default)]
    pub(crate) result_ground_truth: Option<AdmissionResultGroundTruth>,
    pub(crate) security: AdmissionSecurityExpectation,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

impl AdmissionCaseV1 {
    pub(crate) fn case_hash(&self) -> EvalResult<String> {
        canonical_hash(
            &serde_json::to_value(self)
                .map_err(|error| EvalError::new("HARNESS_CONTRACT_ERROR", error.to_string()))?,
        )
    }

    fn validate(&self, fixture_root: &Path) -> EvalResult<()> {
        if self.case_version != M09_DATASET_VERSION {
            return Err(self.invalid("unsupported caseVersion"));
        }
        if self.case_id.trim().is_empty() || self.prompt.trim().is_empty() {
            return Err(self.invalid("caseId and prompt are required"));
        }
        if self.expected_allowed_tools.is_empty() {
            return Err(self.invalid("expectedAllowedTools must not be empty"));
        }
        if self.expected_runtime_outcomes.is_empty() {
            return Err(self.invalid("expectedRuntimeOutcomes must not be empty"));
        }
        for tool in &self.expected_allowed_tools {
            if !super::profile::M09_CONTROL_TOOLS.contains(&tool.as_str())
                && tool != self.candidate.as_str()
            {
                return Err(self.invalid(&format!("unknown expected tool '{tool}'")));
            }
        }
        for outcome in &self.expected_runtime_outcomes {
            if outcome.tool.trim().is_empty() || outcome.outcome.trim().is_empty() {
                return Err(self.invalid("expectedRuntimeOutcomes contains an empty field"));
            }
            if !self
                .expected_allowed_tools
                .iter()
                .any(|expected| expected == &outcome.tool)
            {
                return Err(self.invalid("runtime outcome tool is outside expected selection"));
            }
            if !matches!(outcome.outcome.as_str(), "success" | "error")
                || (outcome.outcome == "error"
                    && outcome
                        .code
                        .as_deref()
                        .map(|code| !M09_TYPED_ERROR_CODES.contains(&code))
                        .unwrap_or(true))
            {
                return Err(self.invalid("expected runtime outcome must be success or typed error"));
            }
        }
        if !matches!(
            self.policy_approval.default.as_str(),
            "allow" | "ask" | "deny" | "disabled"
        ) || !matches!(
            self.policy_approval.approval.as_str(),
            "none" | "allow_once" | "deny"
        ) {
            return Err(self.invalid("invalid Policy/Approval expectation"));
        }
        if !self
            .expected_allowed_tools
            .iter()
            .any(|tool| tool == &self.policy_approval.tool)
        {
            return Err(self.invalid("Policy/Approval tool is outside expected selection"));
        }
        if self.kind == AdmissionCaseKind::Positive
            && self.expected_runtime_outcomes.iter().any(|outcome| {
                outcome.tool == self.candidate.as_str() && outcome.outcome == "success"
            })
        {
            let Some(result) = &self.result_ground_truth else {
                return Err(self.invalid("successful positive case requires resultGroundTruth"));
            };
            let complete = match self.candidate {
                AdmissionCandidate::Mkdir => result.created == Some(true),
                AdmissionCandidate::ListDirectory => self.result_ground_truth.is_some(),
                AdmissionCandidate::Copy => {
                    result.atomic_commit == Some(true) && result.bytes_copied.is_some()
                }
                AdmissionCandidate::Move | AdmissionCandidate::Rename => {
                    result.atomic == Some(true) && result.entry_kind.is_some()
                }
                AdmissionCandidate::Delete => {
                    result.deleted == Some(true)
                        && result.irreversible == Some(true)
                        && result.entry_kind.is_some()
                }
            };
            if !complete {
                return Err(self
                    .invalid("successful positive case has incomplete typed result ground truth"));
            }
        }
        let relative = safe_relative(&self.fixture_ref)?;
        let fixture = fixture_root.join(relative);
        if !fixture.is_dir() {
            return Err(self.invalid("fixtureRef does not resolve to a directory"));
        }
        let actual_fixture_hash = fixture_hash(&fixture, &self.fixture_setup)?;
        if actual_fixture_hash != self.fixture_hash {
            return Err(self.invalid("fixtureHash does not match fixture tree"));
        }
        for assertion in &self.final_state_ground_truth {
            safe_relative(&assertion.path)?;
            if !matches!(
                assertion.state.as_str(),
                "absent" | "file" | "file_sha256" | "directory" | "empty_directory" | "reparse"
            ) {
                return Err(self.invalid("unknown final-state ground-truth state"));
            }
        }
        if self.security.outside_workspace_mutations != 0
            || self.security.raw_physical_paths_in_evidence
            || self.security.reparse_traversal
            || self.security.overwrite
        {
            return Err(self.invalid("security expectations permit an unsafe outcome"));
        }
        Ok(())
    }

    fn invalid(&self, message: &str) -> EvalError {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("case '{}': {message}", self.case_id),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmissionDatasetFileV1 {
    pub(crate) dataset_id: String,
    pub(crate) dataset_version: u32,
    pub(crate) repetitions: u32,
    pub(crate) system_prompt_ref: String,
    pub(crate) case_files: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AdmissionDataset {
    pub(crate) root: PathBuf,
    pub(crate) manifest: AdmissionDatasetFileV1,
    pub(crate) cases: Vec<AdmissionCaseV1>,
    pub(crate) dataset_hash: String,
    pub(crate) system_prompt_hash: String,
}

impl AdmissionDataset {
    pub(crate) fn load(root: &Path) -> EvalResult<Self> {
        let raw = fs::read_to_string(root.join("dataset.json")).map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not read M09 dataset: {error}"),
            )
        })?;
        let manifest: AdmissionDatasetFileV1 = serde_json::from_str(&raw).map_err(|error| {
            EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("invalid M09 dataset: {error}"),
            )
        })?;
        if manifest.dataset_id != M09_DATASET_ID
            || manifest.dataset_version != M09_DATASET_VERSION
            || manifest.repetitions != M09_REPETITIONS
            || manifest.case_files.len() != M09_CASE_COUNT
        {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "M09 dataset identity, repetitions, or case count drifted",
            ));
        }
        let prompt_path = root.join(safe_relative(&manifest.system_prompt_ref)?);
        let prompt = fs::read_to_string(&prompt_path).map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not read M09 system prompt: {error}"),
            )
        })?;
        if prompt.trim().is_empty() {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "M09 system prompt is empty",
            ));
        }
        let fixture_root = root.join("workspaces");
        let mut cases = Vec::with_capacity(M09_CASE_COUNT);
        let mut ids = BTreeSet::new();
        for case_file in &manifest.case_files {
            let path = root.join(safe_relative(case_file)?);
            let raw = fs::read_to_string(&path).map_err(|error| {
                EvalError::new("INFRA_ERROR", format!("could not read M09 case: {error}"))
            })?;
            let case: AdmissionCaseV1 = serde_json::from_str(&raw).map_err(|error| {
                EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    format!("invalid M09 case: {error}"),
                )
            })?;
            if !ids.insert(case.case_id.clone()) {
                return Err(case.invalid("duplicate caseId"));
            }
            case.validate(&fixture_root)?;
            cases.push(case);
        }
        for candidate in AdmissionCandidate::ALL {
            let candidate_cases = cases
                .iter()
                .filter(|case| case.candidate == candidate)
                .collect::<Vec<_>>();
            if candidate_cases.len() != M09_CASES_PER_CANDIDATE
                || candidate_cases
                    .iter()
                    .filter(|case| case.kind == AdmissionCaseKind::Positive)
                    .count()
                    != M09_POSITIVE_CASES_PER_CANDIDATE
                || candidate_cases
                    .iter()
                    .filter(|case| case.kind == AdmissionCaseKind::NegativeSentinel)
                    .count()
                    != M09_NEGATIVE_CASES_PER_CANDIDATE
            {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    format!(
                        "candidate {} does not have 3 positive and 3 negative cases",
                        candidate.as_str()
                    ),
                ));
            }
        }
        cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        let case_values = cases
            .iter()
            .map(|case| Ok(json!({"caseId": case.case_id, "caseHash": case.case_hash()?})))
            .collect::<EvalResult<Vec<_>>>()?;
        let system_prompt_hash = super::manifest::sha256_text(&prompt);
        let dataset_hash = canonical_hash(&json!({
            "datasetId": manifest.dataset_id,
            "datasetVersion": manifest.dataset_version,
            "repetitions": manifest.repetitions,
            "systemPromptHash": system_prompt_hash,
            "cases": case_values,
        }))?;
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
            cases,
            dataset_hash,
            system_prompt_hash,
        })
    }

    pub(crate) fn fixture_path(&self, case: &AdmissionCaseV1) -> EvalResult<PathBuf> {
        Ok(self
            .root
            .join("workspaces")
            .join(safe_relative(&case.fixture_ref)?))
    }

    pub(crate) fn candidate_cases(&self, candidate: AdmissionCandidate) -> Vec<&AdmissionCaseV1> {
        self.cases
            .iter()
            .filter(|case| case.candidate == candidate)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StructuralAdmissionRow {
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) positive_case_count: u8,
    pub(crate) unique_positive_case_count: u8,
    pub(crate) unique_user_value: bool,
    pub(crate) overlap_positive_case_count: u8,
    #[serde(default)]
    pub(crate) conditional_overlap_with: Vec<AdmissionCandidate>,
    pub(crate) existing_tool_overlap: String,
    pub(crate) schema_ambiguity: bool,
    pub(crate) capability: String,
    pub(crate) default_policy: String,
    pub(crate) destructive: bool,
    pub(crate) dual_path: bool,
    pub(crate) overwrite: String,
    pub(crate) cross_volume: String,
    pub(crate) intermediate_state: String,
    pub(crate) recovery_boundary: String,
    pub(crate) reparse_toctou_feasibility: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StructuralAdmissionMatrixV1 {
    pub(crate) matrix_version: u32,
    pub(crate) rows: Vec<StructuralAdmissionRow>,
    pub(crate) matrix_hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StructuralVerdict {
    Eligible,
    Reject,
}

pub(crate) fn structural_matrix() -> EvalResult<StructuralAdmissionMatrixV1> {
    let rows = AdmissionCandidate::ALL
        .into_iter()
        .map(|candidate| {
            let (unique_positive_case_count, overlap_positive_case_count) = match candidate {
                AdmissionCandidate::Mkdir => (3, 0),
                AdmissionCandidate::ListDirectory => (2, 1),
                AdmissionCandidate::Copy => (3, 0),
                AdmissionCandidate::Move => (2, 1),
                // Rename has two unique cases relative to the frozen
                // five-tool control. Its overlap with move is conditional:
                // it is material only if move is eventually ADMITTED.
                AdmissionCandidate::Rename => (2, 1),
                AdmissionCandidate::Delete => (3, 0),
            };
            StructuralAdmissionRow {
                candidate,
                positive_case_count: 3,
                unique_positive_case_count,
                unique_user_value: unique_positive_case_count >= 2,
                overlap_positive_case_count,
                existing_tool_overlap: match candidate {
                    AdmissionCandidate::ListDirectory => {
                        "glob_can_discover_names_but_not_bounded_metadata".into()
                    }
                    AdmissionCandidate::Rename => {
                        "fully_subsumed_if_move_is_admitted_cumulative_overlap_evaluation_required"
                            .into()
                    }
                    _ => "no_equivalent_control_semantics".into(),
                },
                schema_ambiguity: false,
                conditional_overlap_with: if candidate == AdmissionCandidate::Rename {
                    vec![AdmissionCandidate::Move]
                } else {
                    Vec::new()
                },
                capability: match candidate {
                    AdmissionCandidate::ListDirectory => "workspace_read",
                    AdmissionCandidate::Mkdir
                    | AdmissionCandidate::Copy
                    | AdmissionCandidate::Move
                    | AdmissionCandidate::Rename
                    | AdmissionCandidate::Delete => "workspace_write",
                }
                .into(),
                default_policy: if candidate == AdmissionCandidate::ListDirectory {
                    "allow".into()
                } else {
                    "ask".into()
                },
                destructive: matches!(
                    candidate,
                    AdmissionCandidate::Copy
                        | AdmissionCandidate::Move
                        | AdmissionCandidate::Rename
                        | AdmissionCandidate::Delete
                ),
                dual_path: matches!(
                    candidate,
                    AdmissionCandidate::Copy | AdmissionCandidate::Move
                ),
                overwrite: "forbidden_target_exists".into(),
                cross_volume: if candidate == AdmissionCandidate::Move {
                    "unsupported_no_copy_delete_fallback".into()
                } else {
                    "not_applicable".into()
                },
                intermediate_state: match candidate {
                    AdmissionCandidate::Copy => "temporary_destination_then_atomic_commit",
                    AdmissionCandidate::Mkdir => "recursive_partial_creation_reported",
                    AdmissionCandidate::Delete => "single_entry_only_no_partial_tree",
                    _ => "single_atomic_filesystem_operation",
                }
                .into(),
                recovery_boundary: match candidate {
                    AdmissionCandidate::Delete => "irreversible_no_undo_promise",
                    _ => "failure_preserves_or_reports_state",
                }
                .into(),
                reparse_toctou_feasibility: "guard_and_revalidate_before_mutation".into(),
            }
        })
        .collect::<Vec<_>>();
    let hash = canonical_hash(&json!({"matrixVersion": 1, "rows": rows}))?;
    Ok(StructuralAdmissionMatrixV1 {
        matrix_version: 1,
        rows,
        matrix_hash: hash,
    })
}

pub(crate) fn structural_verdict(row: &StructuralAdmissionRow) -> StructuralVerdict {
    if !row.unique_user_value || row.schema_ambiguity {
        StructuralVerdict::Reject
    } else {
        StructuralVerdict::Eligible
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptedMechanicsResult {
    pub(crate) candidate: AdmissionCandidate,
    pub(crate) expected_selection: String,
    pub(crate) observed_selection: Option<String>,
    pub(crate) selection_pass: bool,
    pub(crate) argument_validity_pass: bool,
    pub(crate) runtime_outcome_pass: bool,
    pub(crate) result_pass: bool,
    pub(crate) final_state_pass: bool,
    pub(crate) candidate_misuse: bool,
    pub(crate) scoreable: bool,
    pub(crate) formal_admission_eligible: bool,
}

/// Score only deterministic mechanics.  The returned `formal_admission_eligible`
/// flag is permanently false: real Model evidence and the cumulative M04
/// regression are required before an ADMIT verdict can be written.
pub(crate) fn score_scripted_mechanics(
    candidate: AdmissionCandidate,
    kind: AdmissionCaseKind,
    observed_selection: Option<&str>,
    argument_validity_pass: bool,
    final_state_pass: bool,
) -> ScriptedMechanicsResult {
    let expected = match kind {
        AdmissionCaseKind::Positive => candidate.as_str(),
        AdmissionCaseKind::NegativeSentinel => "control",
    };
    let selection_pass = match kind {
        AdmissionCaseKind::Positive => observed_selection == Some(candidate.as_str()),
        AdmissionCaseKind::NegativeSentinel => observed_selection
            .map(|value| super::profile::M09_CONTROL_TOOLS.contains(&value))
            .unwrap_or(false),
    };
    let candidate_misuse = kind == AdmissionCaseKind::NegativeSentinel
        && observed_selection == Some(candidate.as_str());
    ScriptedMechanicsResult {
        candidate,
        expected_selection: expected.into(),
        observed_selection: observed_selection.map(str::to_owned),
        selection_pass,
        argument_validity_pass,
        runtime_outcome_pass: false,
        result_pass: false,
        final_state_pass,
        candidate_misuse,
        scoreable: observed_selection.is_some(),
        formal_admission_eligible: false,
    }
}

pub(crate) fn validate_m09_profile_catalog(profile: EvalProfile, names: &[&str]) -> EvalResult<()> {
    let expected = profile.tool_names();
    let observed = names
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let expected = expected
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    if observed != expected {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!(
                "{} catalog drifted: expected {expected:?}, observed {observed:?}",
                profile.id()
            ),
        ));
    }
    if profile.is_m09() && names.iter().any(|name| *name == "bash") {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "bash is forbidden in M09 profiles",
        ));
    }
    Ok(())
}

pub(crate) fn admission_profile_schema_hashes() -> EvalResult<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for candidate in AdmissionCandidate::ALL {
        let profile = candidate.profile();
        hashes.insert(
            profile.id().to_owned(),
            profile.candidate_schema_hash()?.ok_or_else(|| {
                EvalError::new("HARNESS_CONTRACT_ERROR", "missing candidate schema")
            })?,
        );
    }
    Ok(hashes)
}

/// Hash only the fixture manifest itself.  This helper is used by T1 checks
/// and never persists physical workspace paths into admission evidence.
pub(crate) fn admission_fixture_manifest_hash(root: &Path) -> EvalResult<String> {
    let path = root.join("dataset.json");
    sha256_file(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_candidate_profiles_are_strict_and_have_one_extra_tool() {
        for candidate in AdmissionCandidate::ALL {
            let profile = candidate.profile();
            assert_eq!(profile.m09_candidate(), Some(candidate.as_str()));
            assert_eq!(profile.tool_names().len(), 6);
            assert_eq!(
                profile.tool_names()[..5],
                super::super::profile::M09_CONTROL_TOOLS
            );
            assert_eq!(profile.tool_names()[5], candidate.as_str());
            validate_m09_profile_catalog(profile, &profile.tool_names()).unwrap();
            assert!(profile.candidate_schema_hash().unwrap().unwrap().len() == 64);
        }
    }

    #[test]
    fn profile_catalog_drift_is_rejected_and_bash_is_never_allowed() {
        let profile = AdmissionCandidate::Copy.profile();
        assert!(validate_m09_profile_catalog(profile, &["read_file"]).is_err());
        assert!(validate_m09_profile_catalog(
            profile,
            &["read_file", "glob", "grep", "write", "edit", "copy", "bash"]
        )
        .is_err());
    }

    #[test]
    fn structural_matrix_is_stable_and_does_not_admit_by_itself() {
        let first = structural_matrix().unwrap();
        let second = structural_matrix().unwrap();
        assert_eq!(first.matrix_hash, second.matrix_hash);
        assert_eq!(first.rows.len(), 6);
        assert_eq!(
            first
                .rows
                .iter()
                .filter(|row| structural_verdict(row) == StructuralVerdict::Eligible)
                .count(),
            6
        );
        let rename = first
            .rows
            .iter()
            .find(|row| row.candidate == AdmissionCandidate::Rename)
            .unwrap();
        assert_eq!(structural_verdict(rename), StructuralVerdict::Eligible);
        assert_eq!(rename.unique_positive_case_count, 2);
        assert_eq!(
            rename.conditional_overlap_with,
            vec![AdmissionCandidate::Move]
        );
        let mechanics = score_scripted_mechanics(
            AdmissionCandidate::Delete,
            AdmissionCaseKind::Positive,
            Some("delete"),
            true,
            true,
        );
        assert!(!mechanics.formal_admission_eligible);
    }

    #[test]
    fn candidate_schema_hash_changes_when_schema_changes() {
        let first = EvalProfile::M09CopyCandidate
            .candidate_schema_hash()
            .unwrap()
            .unwrap();
        let mut changed = EvalProfile::M09CopyCandidate.candidate_schema().unwrap();
        changed["properties"]["sourcePath"]["maxLength"] = json!(512);
        assert_ne!(first, canonical_hash(&changed).unwrap());
    }

    #[test]
    fn dataset_constants_are_explicitly_versioned() {
        assert_eq!(M09_DATASET_ID, "m09-file-tool-admission-v1");
        assert_eq!(M09_CASE_COUNT, 6 * 6);
        assert_eq!(M09_CASE_COUNT * M09_REPETITIONS as usize, 108);
        assert_eq!(
            super::super::profile::M09_CANDIDATE_PROFILE_IDS.len(),
            super::super::profile::M09_CANDIDATE_NAMES.len()
        );
    }
}
