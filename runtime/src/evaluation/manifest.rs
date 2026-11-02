use super::profile::EvalProfile;
use super::{EvalError, EvalResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(crate) const RUNNER_VERSION: &str = "m04-runner-v1";
pub(crate) const SCORER_VERSION: &str = "m04-scorer-v1";
pub(crate) const SYSTEM_PROMPT_ID: &str = "m04-agent-v1";
pub(crate) const ADAPTER_ID: &str = "openai-responses-v1";
pub(crate) const ADAPTER_VERSION: &str = "1";
pub(crate) const REQUESTED_MODEL: &str = "gpt-6.1-terra";
pub(crate) const API_SURFACE: &str = "responses-v1";
pub(crate) const SCRIPTED_ADAPTER_ID: &str = "scripted-v1";

pub(crate) fn canonical_json(value: &Value) -> EvalResult<Vec<u8>> {
    serde_json::to_vec(value)
        .map_err(|error| EvalError::new("HARNESS_CONTRACT_ERROR", error.to_string()))
}

pub(crate) fn canonical_hash(value: &Value) -> EvalResult<String> {
    Ok(sha256_hex(&canonical_json(value)?))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn sha256_text(text: &str) -> String {
    sha256_hex(&normalize_lf(text.as_bytes()))
}

pub(crate) fn sha256_file(path: &Path) -> EvalResult<String> {
    let bytes = fs::read(path).map_err(|error| {
        EvalError::new(
            "INFRA_ERROR",
            format!("could not read {}: {error}", path.display()),
        )
    })?;
    Ok(sha256_hex(&bytes))
}

pub(crate) fn normalize_lf(bytes: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            normalized.push(b'\n');
            index += 2;
        } else {
            normalized.push(bytes[index]);
            index += 1;
        }
    }
    normalized
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunManifest {
    pub(crate) run_id: String,
    pub(crate) logical_run_index: usize,
    pub(crate) case_id: String,
    pub(crate) case_version: u32,
    pub(crate) repetition: u32,
    pub(crate) dataset_hash: String,
    pub(crate) scorer_hash: String,
    pub(crate) system_prompt_hash: String,
    pub(crate) eval_profile_hash: String,
    pub(crate) tool_schema_hash: String,
    pub(crate) policy_config_hash: String,
    pub(crate) adapter_id: String,
    pub(crate) adapter_version: String,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: String,
    pub(crate) runner_commit: String,
    pub(crate) runtime_commit: String,
    pub(crate) runtime_binary_hash: String,
    pub(crate) runner_version: String,
    pub(crate) scorer_version: String,
}

impl RunManifest {
    pub(crate) fn manifest_hash(&self) -> EvalResult<String> {
        canonical_hash(
            &serde_json::to_value(self)
                .map_err(|error| EvalError::new("HARNESS_CONTRACT_ERROR", error.to_string()))?,
        )
    }
}

pub(crate) fn eval_profile_hash(
    system_prompt_hash: &str,
    adapter_id: &str,
    adapter_version: &str,
    requested_model: &str,
    profile: EvalProfile,
) -> EvalResult<String> {
    canonical_hash(&json!({
        "systemPromptHash": system_prompt_hash,
        "systemPromptId": SYSTEM_PROMPT_ID,
        "adapterId": adapter_id,
        "adapterVersion": adapter_version,
        "requestedModel": requested_model,
        "apiSurface": API_SURFACE,
        "maxModelTurns": super::MAX_MODEL_TURNS,
        "maxToolCalls": super::MAX_TOOL_CALLS,
        "maxOutputTokensPerTurn": super::MAX_OUTPUT_TOKENS_PER_TURN,
        "runTimeoutSeconds": super::RUN_TIMEOUT.as_secs(),
        "runSpendCapUsd": super::RUN_SPEND_CAP_USD,
        "suiteSpendCapUsd": super::SUITE_SPEND_CAP_USD,
        "repetitions": super::REPETITIONS,
        "orderSeed": super::ORDER_SEED,
        "profileId": profile.id(),
        "tools": profile.tool_names(),
        "bashEnabled": false,
        "builtInProviderTools": [],
        "temperature": null,
        "seed": null,
    }))
}

pub(crate) fn env_commit(name: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_manifest_hash_is_stable_and_identity_sensitive() {
        let manifest = RunManifest {
            run_id: "run-1".into(),
            logical_run_index: 1,
            case_id: "case-1".into(),
            case_version: 1,
            repetition: 1,
            dataset_hash: "d".into(),
            scorer_hash: "s".into(),
            system_prompt_hash: "p".into(),
            eval_profile_hash: "e".into(),
            tool_schema_hash: "t".into(),
            policy_config_hash: "c".into(),
            adapter_id: "a".into(),
            adapter_version: "1".into(),
            requested_model: "m".into(),
            resolved_model: "m".into(),
            runner_commit: "r".into(),
            runtime_commit: "r".into(),
            runtime_binary_hash: "b".into(),
            runner_version: RUNNER_VERSION.into(),
            scorer_version: SCORER_VERSION.into(),
        };
        let first = manifest.manifest_hash().unwrap();
        assert_eq!(first, manifest.manifest_hash().unwrap());
        let mut changed = manifest;
        changed.resolved_model = "other".into();
        assert_ne!(first, changed.manifest_hash().unwrap());
    }
}
