//! Deterministic M09 oracle mechanics; never executes or authorizes a Tool.
#![allow(dead_code)]

use super::fixture::safe_relative;
use super::m09::AdmissionCaseV1;
use super::manifest::sha256_file;
use super::{EvalError, EvalResult};
use serde_json::Value;
use std::path::Path;

pub(crate) fn runtime_outcome_matches(case: &AdmissionCaseV1, tool: &str, result: &Value) -> bool {
    let matching: Vec<_> = case
        .expected_runtime_outcomes
        .iter()
        .filter(|expected| expected.tool == tool)
        .collect();
    if matching.is_empty() || !result.is_object() {
        return false;
    }
    let failed = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    matching
        .iter()
        .any(|expected| match expected.outcome.as_str() {
            "success" => {
                !failed
                    && expected.code.is_none()
                    && (result.get("structuredContent").is_some()
                        || result.get("content").is_some())
            }
            "error" => {
                failed
                    && expected.code.as_deref().is_some_and(|code| {
                        result
                            .pointer("/structuredContent/code")
                            .and_then(Value::as_str)
                            == Some(code)
                    })
            }
            _ => false,
        })
}

pub(crate) fn result_matches(case: &AdmissionCaseV1, result: &Value) -> bool {
    let Some(expected) = &case.result_ground_truth else {
        return true;
    };
    let Some(data) = result
        .get("structuredContent")
        .filter(|data| data.is_object())
    else {
        return false;
    };
    for (field, expected) in [
        ("created", expected.created),
        ("atomicCommit", expected.atomic_commit),
        ("atomic", expected.atomic),
        ("deleted", expected.deleted),
        ("irreversible", expected.irreversible),
    ] {
        if expected.is_some_and(|value| data.get(field).and_then(Value::as_bool) != Some(value)) {
            return false;
        }
    }
    if expected
        .entry_kind
        .as_ref()
        .is_some_and(|kind| data.get("entryKind").and_then(Value::as_str) != Some(kind))
    {
        return false;
    }
    if case.candidate == super::m09::AdmissionCandidate::ListDirectory {
        let Some(entries) = data.get("entries").and_then(Value::as_array) else {
            return false;
        };
        let names: Option<Vec<_>> = entries
            .iter()
            .map(|entry| entry.get("name").and_then(Value::as_str))
            .collect();
        let Some(names) = names else {
            return false;
        };
        if names
            != expected
                .entry_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        {
            return false;
        }
        if entries.iter().any(|entry| {
            !matches!(
                entry.get("kind").and_then(Value::as_str),
                Some("file" | "directory" | "other" | "reparse")
            )
        }) {
            return false;
        }
        if expected.entry_kinds.iter().any(|(name, kind)| {
            entries
                .iter()
                .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
                .and_then(|entry| entry.get("kind"))
                .and_then(Value::as_str)
                != Some(kind)
        }) {
            return false;
        }
    }
    expected
        .truncated
        .map(|value| data.get("truncated").and_then(Value::as_bool) == Some(value))
        .unwrap_or(true)
        && expected
            .bytes_copied
            .map(|value| data.get("bytesCopied").and_then(Value::as_u64) == Some(value))
            .unwrap_or(true)
}

/// The caller supplies the exact schema captured from the controlled Runtime
/// catalog. This is required for existing tools; candidate metadata alone
/// cannot validate a read_file/glob/grep/write/edit call.
pub(crate) fn arguments_match_with_schema(
    case: &AdmissionCaseV1,
    tool: &str,
    args: &Value,
    observed_schema: &Value,
) -> bool {
    let Ok(validator) = jsonschema::validator_for(observed_schema) else {
        return false;
    };
    validator.is_valid(args) && arguments_match(case, tool, args)
}

/// Constraint oracle. Existing-tool callers must additionally use their exact
/// Runtime catalog schema through `arguments_match_with_schema`.
pub(crate) fn arguments_match(case: &AdmissionCaseV1, tool: &str, args: &Value) -> bool {
    if !case
        .expected_allowed_tools
        .iter()
        .any(|expected| expected == tool)
    {
        return false;
    }
    if tool == case.candidate.as_str() {
        let Some(schema) = case.candidate.profile().candidate_schema() else {
            return false;
        };
        let Ok(validator) = jsonschema::validator_for(&schema) else {
            return false;
        };
        if !validator.is_valid(args) {
            return false;
        }
    }
    case.argument_constraints.iter().all(|constraint| {
        // Frozen optional defaults affect scoring only. The emitted call is
        // never repaired or rewritten before Runtime execution.
        let default = match (tool, constraint.field.as_str()) {
            ("list_directory", "path") => Some(Value::String(".".into())),
            ("list_directory", "maxEntries") => Some(Value::from(200)),
            ("mkdir", "recursive") => Some(Value::Bool(false)),
            _ => None,
        };
        let Some(value) = args.get(&constraint.field).or(default.as_ref()) else {
            return false;
        };
        let scalar = value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string());
        (constraint.allowed_values.is_empty() || constraint.allowed_values.contains(&scalar))
            && (!constraint.path_must_be_workspace_relative
                || (value.is_string()
                    && (safe_relative(&scalar).is_ok()
                        || (tool == "list_directory" && scalar == "."))
                    && scalar.len() <= 1024))
            && constraint
                .max_bytes
                .map(|limit| scalar.len() as u64 <= limit)
                .unwrap_or(true)
    })
}

pub(crate) fn final_state_matches(case: &AdmissionCaseV1, workspace: &Path) -> EvalResult<bool> {
    if case.final_state_ground_truth.is_empty() {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "missing M09 final-state ground truth",
        ));
    }
    for assertion in &case.final_state_ground_truth {
        let relative = safe_relative(&assertion.path)?;
        // Oracle reads must never follow a fixture that was swapped to a link.
        let mut current = workspace.to_path_buf();
        for component in relative.components() {
            current.push(component);
            if let Ok(metadata) = std::fs::symlink_metadata(&current) {
                if is_reparse(&metadata) {
                    return Ok(false);
                }
            }
        }
        let target = workspace.join(relative);
        let metadata = match std::fs::symlink_metadata(&target) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => {
                return Err(EvalError::new(
                    "INFRA_ERROR",
                    "M09 oracle could not inspect final state",
                ))
            }
        };
        let matches = match assertion.state.as_str() {
            "absent" => metadata.is_none(),
            "file" | "file_sha256" => metadata.as_ref().map(|m| m.is_file()).unwrap_or(false),
            "directory" => metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
            "empty_directory" => {
                metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false)
                    && std::fs::read_dir(&target)
                        .map(|mut entries| entries.next().is_none())
                        .unwrap_or(false)
            }
            _ => {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    "unknown M09 final-state assertion",
                ))
            }
        };
        if !matches {
            return Ok(false);
        }
        if let Some(hash) = &assertion.sha256 {
            if metadata.as_ref().map(|m| m.is_file()) != Some(true)
                || sha256_file(&target).map_err(|_| {
                    EvalError::new("INFRA_ERROR", "M09 oracle could not hash final file")
                })? != *hash
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(test)]
mod tests {
    use super::super::adapter::{
        model_request, scripted_call, ModelAdapter, ScriptedAdapter, ScriptedTurn,
    };
    use super::*;
    use serde_json::json;

    fn case() -> AdmissionCaseV1 {
        serde_json::from_value(json!({
            "caseId":"mechanics-only", "caseVersion":1, "candidate":"mkdir", "kind":"positive",
            "prompt":"Create an empty reports directory.", "fixtureRef":"mechanics", "fixtureHash":"test-only",
            "expectedAllowedTools":["mkdir"], "argumentConstraints":[{"field":"path", "allowedValues":["reports"], "pathMustBeWorkspaceRelative":true}],
            "finalStateGroundTruth":[{"path":"reports", "state":"empty_directory"}],
            "policyApproval":{"tool":"mkdir", "default":"ask", "approval":"allow_once"},
            "security":{"outsideWorkspaceMutations":0, "rawPhysicalPathsInEvidence":false, "reparseTraversal":false, "overwrite":false}
        })).unwrap()
    }

    #[tokio::test]
    async fn scripted_calls_are_scored_from_actual_arguments_and_observed_filesystem() {
        let case = case();
        let workspace = tempfile::tempdir().unwrap();
        let mut adapter = ScriptedAdapter::new(vec![ScriptedTurn {
            tool_calls: vec![scripted_call(
                "mechanics-call",
                "mkdir",
                json!({"path":"reports"}),
            )],
            final_text: None,
        }]);
        let turn = adapter
            .respond(&model_request("mechanics-only", &case.prompt, vec![]))
            .await
            .unwrap();
        let call = &turn.tool_calls[0];
        let arguments = serde_json::from_str(&call.arguments).unwrap();
        assert!(arguments_match(&case, &call.name, &arguments));
        assert!(!arguments_match(
            &case,
            "mkdir",
            &json!({"path":"different"})
        ));
        assert!(!arguments_match(
            &case,
            "mkdir",
            &json!({"path":"reports", "extra":true})
        ));
        assert!(!arguments_match(
            &case,
            "mkdir",
            &json!({"path":"../reports"})
        ));
        assert!(!arguments_match(
            &case,
            "delete",
            &json!({"path":"reports"})
        ));
        // These are oracle mechanics, not an executor or admission result.
        assert!(!final_state_matches(&case, workspace.path()).unwrap());
        std::fs::create_dir(workspace.path().join("reports")).unwrap();
        assert!(final_state_matches(&case, workspace.path()).unwrap());
        std::fs::write(workspace.path().join("reports/extra.txt"), b"unexpected").unwrap();
        assert!(!final_state_matches(&case, workspace.path()).unwrap());
    }

    #[test]
    fn binary_ground_truth_and_unknown_oracle_states_fail_closed() {
        let mut case = case();
        let workspace = tempfile::tempdir().unwrap();
        let bytes = [0, 255, 13, 10, 128];
        std::fs::write(workspace.path().join("result.bin"), bytes).unwrap();
        case.final_state_ground_truth[0].path = "result.bin".into();
        case.final_state_ground_truth[0].state = "file".into();
        case.final_state_ground_truth[0].sha256 = Some(super::super::manifest::sha256_hex(&bytes));
        assert!(final_state_matches(&case, workspace.path()).unwrap());
        std::fs::write(workspace.path().join("result.bin"), [0, 254, 13, 10, 128]).unwrap();
        assert!(!final_state_matches(&case, workspace.path()).unwrap());
        case.final_state_ground_truth[0].state = "unknown".into();
        assert_eq!(
            final_state_matches(&case, workspace.path())
                .unwrap_err()
                .code,
            "HARNESS_CONTRACT_ERROR"
        );
    }

    #[test]
    fn frozen_optional_defaults_are_valid_without_rewriting_raw_arguments() {
        let mut case = case();
        case.candidate = super::super::m09::AdmissionCandidate::ListDirectory;
        case.expected_allowed_tools = vec!["list_directory".into()];
        case.argument_constraints = serde_json::from_value(json!([
            {"field":"path", "allowedValues":["."], "pathMustBeWorkspaceRelative":true},
            {"field":"maxEntries", "allowedValues":["200"]}
        ]))
        .unwrap();
        let raw = json!({});
        assert!(arguments_match(&case, "list_directory", &raw));
        assert_eq!(raw, json!({}));
        assert!(!arguments_match(
            &case,
            "list_directory",
            &json!({"maxEntries":1001})
        ));
    }

    #[test]
    fn unchanged_filesystem_cannot_replace_the_expected_typed_refusal() {
        let mut case = case();
        case.expected_runtime_outcomes = serde_json::from_value(json!([
            {"tool":"mkdir", "outcome":"error", "code":"TARGET_EXISTS"}
        ]))
        .unwrap();
        assert!(runtime_outcome_matches(
            &case,
            "mkdir",
            &json!({"isError":true,"structuredContent":{"code":"TARGET_EXISTS"}})
        ));
        assert!(!runtime_outcome_matches(
            &case,
            "mkdir",
            &json!({"isError":false,"structuredContent":{"created":true}})
        ));
        assert!(!runtime_outcome_matches(
            &case,
            "mkdir",
            &json!({"isError":true,"structuredContent":{"code":"IO_FAILURE"}})
        ));
        assert!(!runtime_outcome_matches(&case, "mkdir", &json!({})));
    }

    #[test]
    fn directory_result_oracle_checks_order_bounds_and_entry_kinds() {
        let mut case = case();
        case.candidate = super::super::m09::AdmissionCandidate::ListDirectory;
        case.result_ground_truth = Some(
            serde_json::from_value(json!({"entryNames":["a.txt","z"],"entryKinds":{"a.txt":"file","z":"reparse"},"truncated":true})).unwrap(),
        );
        let correct = json!({"structuredContent":{"entries":[{"name":"a.txt","kind":"file"},{"name":"z","kind":"reparse"}],"truncated":true}});
        assert!(result_matches(&case, &correct));
        let mut incorrect = correct.clone();
        incorrect["structuredContent"]["entries"]
            .as_array_mut()
            .unwrap()
            .reverse();
        assert!(!result_matches(&case, &incorrect));
        let mut incorrect = correct.clone();
        incorrect["structuredContent"]["truncated"] = Value::Bool(false);
        assert!(!result_matches(&case, &incorrect));
        let mut incorrect = correct;
        incorrect["structuredContent"]["entries"][1]["kind"] = Value::String("directory".into());
        assert!(!result_matches(&case, &incorrect));
    }
}
