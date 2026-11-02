use super::case::{validate_category_counts, EvalCaseV1};
use super::fixture::safe_relative;
use super::manifest::{canonical_hash, sha256_text};
use super::{EvalError, EvalResult, CASE_COUNT, DATASET_ID, DATASET_VERSION, REPETITIONS};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DatasetFileV1 {
    pub(crate) dataset_id: String,
    pub(crate) dataset_version: u32,
    pub(crate) repetitions: u32,
    pub(crate) system_prompt_ref: String,
    pub(crate) case_files: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct EvalDataset {
    pub(crate) root: PathBuf,
    pub(crate) dataset_file: DatasetFileV1,
    pub(crate) cases: Vec<EvalCaseV1>,
    pub(crate) dataset_hash: String,
    pub(crate) system_prompt: String,
    pub(crate) system_prompt_hash: String,
    pub(crate) scorer_hash: String,
}

impl EvalDataset {
    pub(crate) fn load(root: &Path) -> EvalResult<Self> {
        let dataset_path = root.join("dataset.json");
        let raw = fs::read_to_string(&dataset_path).map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not read dataset.json: {error}"),
            )
        })?;
        let dataset_file: DatasetFileV1 = serde_json::from_str(&raw).map_err(|error| {
            EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("dataset.json is invalid: {error}"),
            )
        })?;
        if dataset_file.dataset_id != DATASET_ID {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("unexpected datasetId '{}'", dataset_file.dataset_id),
            ));
        }
        if dataset_file.dataset_version != DATASET_VERSION {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "unsupported datasetVersion",
            ));
        }
        if dataset_file.repetitions != REPETITIONS {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "frozen repetitions must be 3",
            ));
        }
        if dataset_file.case_files.len() != CASE_COUNT {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "dataset must reference exactly 24 case files",
            ));
        }
        let prompt_path = root.join(safe_relative(&dataset_file.system_prompt_ref)?);
        let system_prompt = fs::read_to_string(&prompt_path).map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not read system prompt: {error}"),
            )
        })?;
        if system_prompt.trim().is_empty() {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "system prompt is empty",
            ));
        }
        let fixture_root = root.join("workspaces");
        let mut cases = Vec::with_capacity(CASE_COUNT);
        let mut ids = BTreeSet::new();
        for case_file in &dataset_file.case_files {
            let case_path = root.join(safe_relative(case_file)?);
            let raw = fs::read_to_string(&case_path).map_err(|error| {
                EvalError::new("INFRA_ERROR", format!("could not read case file: {error}"))
            })?;
            let case: EvalCaseV1 = serde_json::from_str(&raw).map_err(|error| {
                EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    format!("case file {} is invalid: {error}", case_path.display()),
                )
            })?;
            if !ids.insert(case.case_id.clone()) {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    format!("duplicate caseId '{}'", case.case_id),
                ));
            }
            case.validate(&fixture_root, &system_prompt)?;
            cases.push(case);
        }
        validate_category_counts(&cases)?;
        cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        let case_values = cases
            .iter()
            .map(|case| {
                Ok(json!({
                    "caseId": case.case_id,
                    "caseVersion": case.case_version,
                    "caseHash": case.case_hash()?,
                    "scoreSignature": case.score_signature()?,
                }))
            })
            .collect::<EvalResult<Vec<_>>>()?;
        let dataset_hash = canonical_hash(&json!({
            "datasetId": dataset_file.dataset_id,
            "datasetVersion": dataset_file.dataset_version,
            "repetitions": dataset_file.repetitions,
            "systemPromptHash": sha256_text(&system_prompt),
            "cases": case_values,
        }))?;
        let scorer_hash = sha256_text(super::manifest::SCORER_VERSION);
        Ok(Self {
            root: root.to_path_buf(),
            dataset_file,
            cases,
            dataset_hash,
            system_prompt_hash: sha256_text(&system_prompt),
            system_prompt,
            scorer_hash,
        })
    }

    pub(crate) fn case(&self, case_id: &str) -> Option<&EvalCaseV1> {
        self.cases.iter().find(|case| case.case_id == case_id)
    }

    pub(crate) fn fixture_path(&self, case: &EvalCaseV1) -> EvalResult<PathBuf> {
        Ok(self
            .root
            .join("workspaces")
            .join(safe_relative(&case.fixture_ref)?))
    }

    pub(crate) fn ordered_cases(&self) -> Vec<&EvalCaseV1> {
        deterministic_case_order(&self.cases)
    }
}

pub(crate) fn deterministic_case_order(cases: &[EvalCaseV1]) -> Vec<&EvalCaseV1> {
    let mut ordered = cases.iter().collect::<Vec<_>>();
    let seed = super::ORDER_SEED;
    ordered.sort_by(|left, right| {
        let left_key = seeded_key(&left.case_id, seed);
        let right_key = seeded_key(&right.case_id, seed);
        left_key
            .cmp(&right_key)
            .then_with(|| left.case_id.cmp(&right.case_id))
    });
    ordered
}

fn seeded_key(value: &str, seed: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64 ^ seed;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
